//! The object-safe [`Agent`] trait.
//!
//! [`Orchestrator`](crate::Orchestrator) is generic over its store/engine/gatekeeper, which
//! is awkward to carry through a web layer's shared state. `Agent` erases those generics
//! behind a trait object so adapters can hold `Arc<dyn Agent>` (Send + Sync) and stay
//! decoupled from the concrete wiring — the thin-adapter boundary (ARCHITECTURE.md §1).

use async_trait::async_trait;

use crate::event::AgentEvent;
use crate::session::Session;

/// Runs a turn and returns the canonical event stream. The one capability every protocol
/// adapter needs; the error carries a coarse [`AgentErrorKind`] so adapters can choose the
/// right status (e.g. admission rejection -> 503) without depending on `OrchestratorError`.
#[async_trait]
pub trait Agent: Send + Sync {
    async fn run(&self, session: &Session) -> Result<Vec<AgentEvent>, AgentError>;
}

/// Coarse classification of an agent failure, enough for adapters to pick a wire status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentErrorKind {
    /// The client's request was malformed (e.g. no user message) -> 400.
    BadRequest,
    /// Admission control rejected/timed out the request -> 503 (backpressure).
    Overloaded,
    /// An internal failure (store, inference) -> 500.
    Internal,
}

/// A boundary error for the `Agent` trait. Concrete orchestrator errors are flattened to a
/// kind + message so trait objects stay simple; adapters map the kind to a wire status.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct AgentError {
    pub kind: AgentErrorKind,
    pub message: String,
}

impl AgentError {
    pub fn new(kind: AgentErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}
