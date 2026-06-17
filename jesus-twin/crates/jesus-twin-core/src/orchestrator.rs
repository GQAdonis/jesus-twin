//! The agent orchestrator: `retrieve -> gate -> generate -> citations`.
//!
//! This is the single agent loop (ARCHITECTURE.md §1, §3). It composes the store
//! (retrieval/truth), the inference engine (voice), the coverage gate (stance), and the
//! admission gatekeeper (backpressure) — and emits the canonical [`AgentEvent`] stream that
//! every protocol surface projects from.
//!
//! This is the RAG-first, base-model build (build-sequence step 3): retrieval grounds the
//! answer, the coverage gate refuses out-of-corpus questions before the model runs, and
//! generation is always conditioned on the retrieved, cited passages. The tool loop is
//! structural only until skills are registered (build step 7).

use thiserror::Error;
use uuid::Uuid;

use async_trait::async_trait;

use jesus_twin_admission::{Cost, Gatekeeper};
use jesus_twin_inference::{Engine, GenRequest};
use jesus_twin_skills::Registry;
use jesus_twin_store::{RetrievalSet, Store};

use crate::agent::{Agent, AgentError, AgentErrorKind};
use crate::event::{AgentEvent, FinishReason, RefusalReason, Role};
use crate::gate::{Coverage, CoverageGate};
use crate::prompt::{self, SYSTEM_PROMPT};
use crate::session::Session;

/// How many passages to retrieve as grounding context per turn.
const RETRIEVE_LIMIT: usize = 5;

/// Errors the orchestrator can surface to an adapter.
#[derive(Debug, Error)]
pub enum OrchestratorError {
    #[error("no user message in session")]
    NoUserMessage,
    #[error("admission rejected: {0}")]
    AdmissionRejected(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("inference error: {0}")]
    Inference(String),
}

/// Owns the collaborators and runs turns. Generic over the trait objects so the store can be
/// embedded or remote, and the engine real or a test double, without touching the core.
pub struct Orchestrator<S, E, G> {
    store: S,
    engine: E,
    gatekeeper: G,
    #[allow(dead_code)] // consumed by the tool loop once skills are registered (build step 7)
    skills: Registry,
    gate: CoverageGate,
}

impl<S, E, G> Orchestrator<S, E, G>
where
    S: Store,
    E: Engine,
    G: Gatekeeper,
{
    pub fn new(store: S, engine: E, gatekeeper: G, skills: Registry, gate: CoverageGate) -> Self {
        Self {
            store,
            engine,
            gatekeeper,
            skills,
            gate,
        }
    }

    /// Run one turn against `session`, returning the ordered [`AgentEvent`]s.
    ///
    /// Pipeline: admit -> retrieve -> coverage gate (refuse if uncovered, before the model
    /// runs) -> emit citations + state -> generate (grounded on the retrieved passages).
    /// Each step appends to the event stream; a refusal short-circuits with no generation.
    pub async fn run(&self, session: &Session) -> Result<Vec<AgentEvent>, OrchestratorError> {
        let run_id = Uuid::new_v4();
        let mut events = vec![AgentEvent::RunStarted {
            run_id,
            session_id: session.id,
        }];

        // Admission control: hold the permit for the whole turn; drop releases it.
        let _permit = self
            .gatekeeper
            .admit(Cost(1))
            .await
            .map_err(|e| OrchestratorError::AdmissionRejected(e.to_string()))?;

        let query = latest_user_message(session).ok_or(OrchestratorError::NoUserMessage)?;

        // 1. Retrieve (truth).
        let set = self
            .store
            .retrieve(query, RETRIEVE_LIMIT)
            .await
            .map_err(|e| OrchestratorError::Store(e.to_string()))?;

        // 2. Coverage gate (stance): classify by leg agreement. NoCoverage refuses before the
        // model runs; LowConfidence answers but flags the turn; Grounded answers normally.
        let coverage = self.gate.classify(set.passages.len(), set.top_legs_matched);
        if coverage == Coverage::NoCoverage {
            events.push(AgentEvent::Refusal {
                reason: RefusalReason::NoCoverage,
            });
            events.push(AgentEvent::RunFinished {
                run_id,
                finish: FinishReason::Refusal,
            });
            return Ok(events);
        }
        let low_confidence = coverage == Coverage::LowConfidence;

        // 3. Citations + state snapshot: every grounded claim traces to a verse.
        for p in &set.passages {
            events.push(AgentEvent::Citation {
                ref_: p.ref_.clone(),
                score: p.score.unwrap_or(0.0),
                span: None,
            });
        }
        events.push(AgentEvent::StateSnapshot {
            state: state_snapshot(&set),
        });

        // Tier 2: flag the turn for the honesty surface (single-leg grounding). Additive,
        // namespaced — standard clients ignore it; the AG-UI adapter projects it verbatim.
        if low_confidence {
            events.push(AgentEvent::Custom {
                name: "x-jesus-twin/low-confidence".to_string(),
                data: serde_json::json!({ "legs_matched": set.top_legs_matched }),
            });
        }

        // 4. Generate (voice), conditioned on the retrieved passages. On a low-confidence turn
        // the context carries the in-voice hedge addendum (a per-turn injection; SYSTEM_PROMPT
        // is untouched, preserving train/inference parity).
        let context = if low_confidence {
            prompt::assemble_context_low_confidence(&context_lines(&set))
        } else {
            prompt::assemble_context(&context_lines(&set))
        };
        let message_id = Uuid::new_v4();
        events.push(AgentEvent::TextMessageStart {
            message_id,
            role: Role::Assistant,
        });
        let text = self
            .engine
            .generate(GenRequest {
                system: SYSTEM_PROMPT.to_string(),
                context,
                user: query.to_string(),
            })
            .await
            .map_err(|e| OrchestratorError::Inference(e.to_string()))?;
        events.push(AgentEvent::TextMessageDelta {
            message_id,
            delta: text,
        });
        events.push(AgentEvent::TextMessageEnd { message_id });

        events.push(AgentEvent::RunFinished {
            run_id,
            finish: FinishReason::Stop,
        });
        Ok(events)
    }
}

#[async_trait]
impl<S, E, G> Agent for Orchestrator<S, E, G>
where
    S: Store,
    E: Engine,
    G: Gatekeeper,
{
    async fn run(&self, session: &Session) -> Result<Vec<AgentEvent>, AgentError> {
        // Delegate to the inherent method, classifying the error so adapters pick the right
        // wire status (admission rejection -> 503, bad input -> 400, else 500).
        Orchestrator::run(self, session).await.map_err(|e| {
            let kind = match e {
                OrchestratorError::NoUserMessage => AgentErrorKind::BadRequest,
                OrchestratorError::AdmissionRejected(_) => AgentErrorKind::Overloaded,
                OrchestratorError::Store(_) | OrchestratorError::Inference(_) => {
                    AgentErrorKind::Internal
                }
            };
            AgentError::new(kind, e.to_string())
        })
    }
}

/// The most recent user turn's content, if any.
fn latest_user_message(session: &Session) -> Option<&str> {
    session
        .turns
        .iter()
        .rev()
        .find(|t| t.role == Role::User)
        .map(|t| t.content.as_str())
}

/// Build the conditioning context lines from the retrieved passages: each is the cited
/// **original** text (the ground truth the model paraphrases — never invents).
///
/// SAFETY (modern-legs-v1 bright line): this uses `text_original` ONLY. `text_modern` may hold a
/// machine draft (`machine_draft = true`) that exists for retrieval indexing only and must never
/// be displayed or fed to the model. The `display_uses_original_not_modern` test pins this.
fn context_lines(set: &RetrievalSet) -> Vec<String> {
    set.passages
        .iter()
        .map(|p| format!("{}: {}", p.ref_, p.text_original))
        .collect()
}

/// A compact JSON view of the retrieval set for the `StateSnapshot` event (drives the
/// mind-map / debug surfaces). Refs + scores only; full text is already in `Citation`s.
fn state_snapshot(set: &RetrievalSet) -> serde_json::Value {
    serde_json::json!({
        "retrieval": set.passages.iter().map(|p| serde_json::json!({
            "ref": p.ref_,
            "score": p.score,
            "move": p.move_,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use jesus_twin_store::{Passage, RetrievalSet};

    /// modern-legs-v1 bright line: the display/generation context must use `text_original`,
    /// never a (possibly machine-draft) `text_modern`.
    #[test]
    fn display_uses_original_not_modern() {
        let set = RetrievalSet {
            passages: vec![Passage {
                id: "wj-1".into(),
                ref_: "Mark 12:17".into(),
                book_author: "Mark".into(),
                text_original: "Render to Caesar the things that are Caesar's.".into(),
                text_modern: "MACHINE_DRAFT_SENTINEL give the government what is theirs".into(),
                context: String::new(),
                location: String::new(),
                occasion: String::new(),
                move_: String::new(),
                translation: String::new(),
                score: Some(0.03),
            }],
            top_legs_matched: 2,
        };
        let lines = context_lines(&set).join("\n");
        assert!(
            lines.contains("Render to Caesar"),
            "must show the original text"
        );
        assert!(
            !lines.contains("MACHINE_DRAFT_SENTINEL"),
            "machine-draft text_modern must NEVER reach the display/generation context"
        );
        // The state snapshot must likewise not leak the draft text.
        let snap = state_snapshot(&set).to_string();
        assert!(!snap.contains("MACHINE_DRAFT_SENTINEL"));
    }
}
