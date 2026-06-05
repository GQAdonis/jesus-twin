//! Agent core for the Jesus digital twin.
//!
//! This crate owns the one canonical [`event::AgentEvent`] stream and the orchestrator
//! that produces it (`retrieve -> coverage gate -> generate -> tool loop`). It knows
//! nothing about HTTP, MCP, AG-UI, or A2A — those are thin projections living in
//! `jesus-twin-api`. Dependency direction is strictly downward (see ARCHITECTURE.md §2).
//!
//! The load-bearing principle: retrieval owns *truth*, the fine-tune owns *voice*, the
//! agent layer (here) owns *stance/honesty*. The coverage gate can short-circuit to a
//! refusal before the model ever runs.

pub mod agent;
pub mod event;
pub mod gate;
pub mod orchestrator;
pub mod prompt;
pub mod session;

pub use agent::{Agent, AgentError, AgentErrorKind};
pub use event::{AgentEvent, FinishReason, RefusalReason, Role};
pub use orchestrator::{Orchestrator, OrchestratorError};
pub use session::{Session, Turn};
