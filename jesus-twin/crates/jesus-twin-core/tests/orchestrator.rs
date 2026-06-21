//! Integration test for the RAG-first orchestrator pipeline (build-sequence step 3).
//!
//! Wires the REAL store (embedded SurrealDB + the corpus) with the deterministic
//! `MockEngine` / `MockEmbedder` and `OpenGatekeeper`, and asserts the emitted `AgentEvent`
//! stream: a covered query grounds + generates with citations; an out-of-corpus query
//! refuses before the model runs.

use std::path::PathBuf;

use jesus_twin_admission::OpenGatekeeper;
use jesus_twin_core::event::Role;
use jesus_twin_core::event::{AgentEvent, FinishReason, RefusalReason};
use jesus_twin_core::gate::CoverageGate;
use jesus_twin_core::{Orchestrator, Session, Turn};
use jesus_twin_inference::MockEngine;
use jesus_twin_skills::Registry;
use jesus_twin_store::{Store, SurrealStore};
use uuid::Uuid;

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../build/rag_corpus.jsonl")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../../../build/rag_corpus.jsonl"))
}

/// Build an orchestrator over a freshly ingested in-memory store, or `None` if the corpus
/// is absent (so the suite still runs on a bare checkout).
async fn make_orchestrator() -> Option<Orchestrator<SurrealStore, MockEngine, OpenGatekeeper>> {
    let path = corpus_path();
    if !path.exists() {
        eprintln!("skipping: corpus not found at {}", path.display());
        return None;
    }
    let store = SurrealStore::memory().await.expect("store");
    store
        .ingest_corpus(path.to_str().unwrap())
        .await
        .expect("ingest");
    Some(Orchestrator::new(
        store,
        MockEngine::new(),
        OpenGatekeeper,
        Registry::new(),
        CoverageGate,
    ))
}

fn session_with(query: &str) -> Session {
    Session::new(Uuid::new_v4()).with_turn(Turn::new(Role::User, query))
}

#[tokio::test]
async fn covered_query_grounds_and_generates_with_citations() {
    let Some(orch) = make_orchestrator().await else {
        return;
    };

    let events = orch
        .run(&session_with("render to Caesar"))
        .await
        .expect("run");

    // This harness has no embedder attached, so retrieval is BM25-only = a single modality =
    // Tier 2 (low-confidence) by definition: a covered query still grounds, cites, and generates,
    // but the low-confidence honesty chunk is emitted.
    let low_conf: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Custom { name, .. } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        low_conf,
        vec!["x-jesus-twin/low-confidence"],
        "BM25-only fallback is Tier 2 → exactly one low-confidence chunk"
    );

    // Lifecycle: starts and finishes with Stop (not Refusal).
    assert!(matches!(
        events.first(),
        Some(AgentEvent::RunStarted { .. })
    ));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunFinished {
            finish: FinishReason::Stop,
            ..
        })
    ));

    // Grounding: at least one Citation carrying a verse ref.
    let citations: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Citation { ref_, .. } => Some(ref_.as_str()),
            _ => None,
        })
        .collect();
    assert!(!citations.is_empty(), "covered query must emit citations");
    assert!(
        citations
            .iter()
            .any(|r| r.contains("Mark") || r.contains("Luke")),
        "expected the Caesar saying's citation, got {citations:?}"
    );

    // Generation happened, and (mock) restated the user ask grounded on context.
    let generated: String = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::TextMessageDelta { delta, .. } => Some(delta.clone()),
            _ => None,
        })
        .collect();
    assert!(
        generated.contains("grounded"),
        "generation should be grounded on context"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::Refusal { .. }))
    );
}

#[tokio::test]
async fn out_of_corpus_query_refuses_before_generation() {
    let Some(orch) = make_orchestrator().await else {
        return;
    };

    // Stop-word-laden phrasing must still refuse: query-side stop-word stripping leaves only
    // "cryptocurrency", which has no corpus coverage (guards the gate-leak regression).
    let events = orch
        .run(&session_with("what about cryptocurrency"))
        .await
        .expect("run");

    // Refusal fired, finished as Refusal, and NO generation/citation occurred.
    assert!(events.iter().any(|e| matches!(
        e,
        AgentEvent::Refusal {
            reason: RefusalReason::NoCoverage
        }
    )));
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunFinished {
            finish: FinishReason::Refusal,
            ..
        })
    ));
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::TextMessageDelta { .. })),
        "the model must not run when the gate refuses"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::Citation { .. }))
    );
}

#[tokio::test]
async fn session_without_user_message_errors() {
    let Some(orch) = make_orchestrator().await else {
        return;
    };
    let empty = Session::new(Uuid::new_v4());
    assert!(orch.run(&empty).await.is_err());
}

// --- Deterministic tier coverage via a fake store (BM25-only can't produce a 2-leg Tier-1) ---

use jesus_twin_store::{Passage, RetrievalSet, StoreError};

/// A store that returns one passage with a configurable `top_legs_matched`, so the orchestrator's
/// tier branching can be exercised without an embedder/vectors.
struct FakeStore {
    legs: u8,
}

#[async_trait::async_trait]
impl Store for FakeStore {
    async fn retrieve(&self, _q: &str, _limit: usize) -> Result<RetrievalSet, StoreError> {
        let p = Passage {
            id: "saying-1".into(),
            ref_: "Mark 12:29-31".into(),
            book_author: "Mark".into(),
            text_original: "Hear, Israel...".into(),
            text_modern: String::new(),
            context: String::new(),
            location: String::new(),
            occasion: String::new(),
            move_: String::new(),
            translation: String::new(),
            domains: vec!["conflict/forgiveness".into()],
            principles: vec!["Love of God and neighbor is the whole of the law.".into()],
            score: Some(0.03),
        };
        Ok(RetrievalSet {
            passages: vec![p],
            top_legs_matched: self.legs,
        })
    }
    async fn ingest_corpus(&self, _p: &str) -> Result<usize, StoreError> {
        Ok(0)
    }
    async fn get_by_ref(&self, _r: &str) -> Result<Option<Passage>, StoreError> {
        Ok(None)
    }
    async fn find_by_move(&self, _m: &str, _l: usize) -> Result<Vec<Passage>, StoreError> {
        Ok(vec![])
    }
}

fn fake_orchestrator(legs: u8) -> Orchestrator<FakeStore, MockEngine, OpenGatekeeper> {
    Orchestrator::new(
        FakeStore { legs },
        MockEngine::new(),
        OpenGatekeeper,
        Registry::new(),
        CoverageGate,
    )
}

#[tokio::test]
async fn tier1_two_legs_emits_no_low_confidence_chunk() {
    let events = fake_orchestrator(2)
        .run(&session_with("the greatest commandment"))
        .await
        .expect("run");
    // Grounded: generates + cites, finishes Stop, and NO low-confidence chunk.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::Citation { .. })),
        "Tier 1 still cites"
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::Custom { .. })),
        "two agreeing legs must NOT emit the low-confidence chunk"
    );
    assert!(matches!(
        events.last(),
        Some(AgentEvent::RunFinished {
            finish: FinishReason::Stop,
            ..
        })
    ));
}

#[tokio::test]
async fn tier2_one_leg_emits_low_confidence_chunk() {
    let events = fake_orchestrator(1)
        .run(&session_with("how do I handle money anxiety"))
        .await
        .expect("run");
    let chunk = events.iter().find_map(|e| match e {
        AgentEvent::Custom { name, data } => Some((name.clone(), data.clone())),
        _ => None,
    });
    let (name, data) = chunk.expect("Tier 2 emits a custom chunk");
    assert_eq!(name, "x-jesus-twin/low-confidence");
    assert_eq!(data["legs_matched"], serde_json::json!(1));
    // principle-tier: the retrieved passage's principle facet rides on the chunk (and into the
    // bridging context). The FakeStore tags it with one principle.
    assert_eq!(
        data["principles"],
        serde_json::json!(["Love of God and neighbor is the whole of the law."])
    );
    // Still answers (Tier 2 engages, with the hedge); does not refuse.
    assert!(
        events
            .iter()
            .any(|e| matches!(e, AgentEvent::TextMessageDelta { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, AgentEvent::Refusal { .. }))
    );
}

// --- source/narrative wiring: the two SEPARATE labeled corpora reach the context + AG-UI chunks ---

use std::sync::{Arc, Mutex};

use jesus_twin_core::prompt;
use jesus_twin_inference::{Engine, EngineError, GenRequest};
use jesus_twin_store::{NarrativePassage, SourcePassage};

/// A store that returns a grounded (2-leg) red-letter passage PLUS one Tanakh source verse and one
/// Gospel-narrative deed — so the orchestrator's source/narrative wiring can be exercised end to end.
struct SupplStore;

#[async_trait::async_trait]
impl Store for SupplStore {
    async fn retrieve(&self, _q: &str, _l: usize) -> Result<RetrievalSet, StoreError> {
        Ok(RetrievalSet {
            passages: vec![Passage {
                id: "saying-1".into(),
                ref_: "Matthew 4:4".into(),
                book_author: "Matthew".into(),
                text_original: "Man shall not live by bread alone.".into(),
                text_modern: String::new(),
                context: String::new(),
                location: String::new(),
                occasion: String::new(),
                move_: String::new(),
                translation: String::new(),
                domains: vec![],
                principles: vec![],
                score: Some(0.05),
            }],
            top_legs_matched: 2,
        })
    }
    async fn ingest_corpus(&self, _p: &str) -> Result<usize, StoreError> {
        Ok(0)
    }
    async fn get_by_ref(&self, _r: &str) -> Result<Option<Passage>, StoreError> {
        Ok(None)
    }
    async fn find_by_move(&self, _m: &str, _l: usize) -> Result<Vec<Passage>, StoreError> {
        Ok(vec![])
    }
    async fn retrieve_tanakh(
        &self,
        _q: &str,
        _l: usize,
    ) -> Result<Vec<SourcePassage>, StoreError> {
        Ok(vec![SourcePassage {
            ref_: "Deuteronomy 8:3".into(),
            text: "man doth not live by bread only".into(),
            book: "Deuteronomy".into(),
            category: "torah".into(),
            translation: "JPS 1917".into(),
            score: Some(0.04),
        }])
    }
    async fn retrieve_gospel_narrative(
        &self,
        _q: &str,
        _l: usize,
    ) -> Result<Vec<NarrativePassage>, StoreError> {
        Ok(vec![NarrativePassage {
            ref_: "Matthew 4:1".into(),
            text: "Jesus was led up by the Spirit into the wilderness to be tempted.".into(),
            book: "Matthew".into(),
            attestation: "single".into(),
            witnesses: vec!["Matthew".into()],
            score: Some(0.04),
        }])
    }
}

/// An engine that records the generation context it was handed, so the test can assert the labeled
/// blocks (and their order) actually reached generation — not merely that events were emitted.
#[derive(Clone)]
struct CapturingEngine(Arc<Mutex<String>>);

#[async_trait::async_trait]
impl Engine for CapturingEngine {
    async fn generate(&self, req: GenRequest) -> Result<String, EngineError> {
        *self.0.lock().unwrap() = req.context.clone();
        Ok(format!("(mock) {}", req.user.trim()))
    }
}

#[tokio::test]
async fn source_and_narrative_corpora_wire_as_distinct_labeled_blocks() {
    let captured = Arc::new(Mutex::new(String::new()));
    let orch = Orchestrator::new(
        SupplStore,
        CapturingEngine(captured.clone()),
        OpenGatekeeper,
        Registry::new(),
        CoverageGate,
    );

    let events = orch
        .run(&session_with("was he tempted in the wilderness"))
        .await
        .expect("run");

    // 1. Each corpus is surfaced as its OWN additive, namespaced AG-UI chunk, carrying the facets
    // that label it distinctly (source = scripture category; narrative = attestation).
    let custom: std::collections::HashMap<String, serde_json::Value> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::Custom { name, data } => Some((name.clone(), data.clone())),
            _ => None,
        })
        .collect();
    let source = custom
        .get("x-jesus-twin/source-text")
        .expect("source-text chunk emitted");
    assert_eq!(source["passages"][0]["ref"], serde_json::json!("Deuteronomy 8:3"));
    assert_eq!(source["passages"][0]["category"], serde_json::json!("torah"));
    let narrative = custom
        .get("x-jesus-twin/narrative-context")
        .expect("narrative-context chunk emitted");
    assert_eq!(narrative["passages"][0]["ref"], serde_json::json!("Matthew 4:1"));
    assert_eq!(
        narrative["passages"][0]["attestation"],
        serde_json::json!("single")
    );

    // 2. Both labeled blocks reach the generation context, DISTINCT from each other and from the
    // red-letter grounding block — the bright line: source material / deeds are never his words.
    let ctx = captured.lock().unwrap().clone();
    assert!(
        ctx.contains(prompt::SOURCE_INSTRUCTION),
        "source-material block must be labeled in context"
    );
    assert!(ctx.contains("Deuteronomy 8:3"), "source verse present");
    assert!(
        ctx.contains(prompt::NARRATIVE_INSTRUCTION),
        "narrative block must be labeled in context"
    );
    assert!(ctx.contains("Matthew 4:1"), "narrative deed present");
    assert!(
        ctx.contains(prompt::CONTEXT_INSTRUCTION),
        "red-letter grounding block must still be present"
    );

    // 3. Ordering: his own words (the grounding block) sit LAST — the high-attention end-of-prompt
    // position — after his source material and his deeds.
    let src = ctx.find(prompt::SOURCE_INSTRUCTION).unwrap();
    let nar = ctx.find(prompt::NARRATIVE_INSTRUCTION).unwrap();
    let grd = ctx.find(prompt::CONTEXT_INSTRUCTION).unwrap();
    assert!(
        src < grd && nar < grd,
        "grounding (his words) must come after source + narrative blocks"
    );
}
