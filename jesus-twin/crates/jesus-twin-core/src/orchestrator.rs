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
use jesus_twin_store::{NarrativePassage, RetrievalSet, SourcePassage, Store};

use crate::agent::{Agent, AgentError, AgentErrorKind};
use crate::event::{AgentEvent, FinishReason, RefusalReason, Role};
use crate::gate::{Coverage, CoverageGate};
use crate::prompt::{self, SYSTEM_PROMPT};
use crate::session::Session;

/// How many passages to retrieve as grounding context per turn.
const RETRIEVE_LIMIT: usize = 5;

/// How many of the most-salient episodic memories to recall per turn (episodic-memory).
const MEMORY_LIMIT: usize = 3;

/// How many passages to pull from each SUPPLEMENTARY labeled corpus per turn — his source material
/// (Tanakh) and his deeds (Gospel narrative). Kept small: they contextualize the grounded answer,
/// they never dominate it (the red-letter `set` is the truth the model paraphrases).
const SUPP_LIMIT: usize = 2;

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

        // Episodic memory (the fourth surface): recall facts about THIS person, scoped to the
        // relationship (user id, else this conversation). Non-fatal — a memory failure must never
        // break the answer. Facts about the user only; injected as context, never SYSTEM_PROMPT.
        let memory_scope = session.memory_scope();
        let memory_block = {
            let memories = self
                .store
                .retrieve_memories(&memory_scope, MEMORY_LIMIT)
                .await
                .unwrap_or_default();
            let texts: Vec<String> = memories.into_iter().map(|m| m.text).collect();
            prompt::assemble_memory_block(&texts)
        };

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

        // Tier 2 (principle-tier): when the retrieved passages carry principle facets
        // (principle-index-v1), bridge the adjacent question to those principles; else the plain
        // low-confidence hedge. The principles ride on the honesty chunk too.
        let principles = if low_confidence {
            collect_principles(&set)
        } else {
            Vec::new()
        };
        if low_confidence {
            events.push(AgentEvent::Custom {
                name: "x-jesus-twin/low-confidence".to_string(),
                data: serde_json::json!({
                    "legs_matched": set.top_legs_matched,
                    "principles": principles,
                }),
            });
        }

        // 3b. The two SEPARATE labeled corpora, retrieved with the same query: his SOURCE MATERIAL
        // (Tanakh — what he drew on) and his DEEDS (Gospel narrative — what the record shows he
        // did). Injected as DISTINCT, labeled context blocks so the model can reference what he read
        // and how he acted, while never rendering either as his own words (the bright line). Each is
        // also surfaced as its own additive, namespaced AG-UI chunk. Non-fatal: a failure here must
        // never break the grounded answer (mirrors the memory recall above).
        let source_hits = self
            .store
            .retrieve_tanakh(query, SUPP_LIMIT)
            .await
            .unwrap_or_default();
        let narrative_hits = self
            .store
            .retrieve_gospel_narrative(query, SUPP_LIMIT)
            .await
            .unwrap_or_default();
        if !source_hits.is_empty() {
            events.push(AgentEvent::Custom {
                name: "x-jesus-twin/source-text".to_string(),
                data: serde_json::json!({
                    "passages": source_hits.iter().map(|p| serde_json::json!({
                        "ref": p.ref_, "text": p.text, "category": p.category,
                    })).collect::<Vec<_>>(),
                }),
            });
        }
        if !narrative_hits.is_empty() {
            events.push(AgentEvent::Custom {
                name: "x-jesus-twin/narrative-context".to_string(),
                data: serde_json::json!({
                    "passages": narrative_hits.iter().map(|p| serde_json::json!({
                        "ref": p.ref_, "text": p.text,
                        "attestation": p.attestation, "witnesses": p.witnesses,
                    })).collect::<Vec<_>>(),
                }),
            });
        }
        let source_block = prompt::assemble_source_block(&source_lines(&source_hits));
        let narrative_block = prompt::assemble_narrative_block(&narrative_lines(&narrative_hits));

        // 4. Generate (voice), conditioned on the retrieved passages. On a low-confidence turn the
        // context carries the in-voice hedge (principle-bridging when principles exist) — a per-turn
        // injection; SYSTEM_PROMPT is untouched, preserving train/inference parity.
        let grounding = if low_confidence {
            if principles.is_empty() {
                prompt::assemble_context_low_confidence(&context_lines(&set))
            } else {
                prompt::assemble_context_principle_tier(&context_lines(&set), &principles)
            }
        } else {
            prompt::assemble_context(&context_lines(&set))
        };
        // Compose the turn context from the labeled blocks, in ascending attention order: memory
        // (about the person), then his source material, then his deeds, then the grounding block
        // (his own words) last — the high-attention end-of-prompt position. Empty blocks drop out.
        let context = [
            memory_block.as_str(),
            source_block.as_str(),
            narrative_block.as_str(),
            grounding.as_str(),
        ]
        .into_iter()
        .filter(|b| !b.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");
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

        // Post-turn: record an observation about THIS person (episodic-memory) — what they asked
        // and the verses that grounded the reply. A fact about the user, not doctrine; scoped to
        // the relationship. Non-fatal — recording must never break the turn.
        let refs: Vec<String> = set.passages.iter().map(|p| p.ref_.clone()).collect();
        let observation = format!("Asked: {query}");
        let _ = self
            .store
            .record_memory(&memory_scope, "observation", &observation, 5, &refs)
            .await;

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

/// Build the labeled context lines for the Tanakh SOURCE-MATERIAL block: `ref: text` per verse
/// (hebrew-bible). Distinct from [`context_lines`] so his source material can never be conflated
/// with his own words — the block carries its own provenance label.
fn source_lines(hits: &[SourcePassage]) -> Vec<String> {
    hits.iter()
        .map(|p| format!("{}: {}", p.ref_, p.text))
        .collect()
}

/// Build the labeled context lines for the Gospel-NARRATIVE block: `ref: text` per passage
/// (gospel-context-kb). Deeds, never words — the block carries its own provenance label, and the
/// attestation flag rides on the AG-UI chunk rather than the model context.
fn narrative_lines(hits: &[NarrativePassage]) -> Vec<String> {
    hits.iter()
        .map(|p| format!("{}: {}", p.ref_, p.text))
        .collect()
}

/// Collect the distinct governing principles of the retrieved passages (principle-index-v1
/// facets), in retrieval order, capped at a few. Tier-2 principle-bridging speaks to these — they
/// are machine-tagged metadata, never the model's own invention. Empty until passages are tagged.
fn collect_principles(set: &RetrievalSet) -> Vec<String> {
    const MAX_PRINCIPLES: usize = 3;
    let mut out: Vec<String> = Vec::new();
    for p in &set.passages {
        for principle in &p.principles {
            let principle = principle.trim();
            if !principle.is_empty() && !out.iter().any(|x| x == principle) {
                out.push(principle.to_string());
                if out.len() >= MAX_PRINCIPLES {
                    return out;
                }
            }
        }
    }
    out
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
                domains: Vec::new(),
                principles: vec!["Trust the Father's provision over anxious striving.".into()],
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
