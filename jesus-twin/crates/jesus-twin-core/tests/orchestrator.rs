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
        CoverageGate::default(),
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
