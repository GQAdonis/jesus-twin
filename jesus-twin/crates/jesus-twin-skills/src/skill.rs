//! The [`Skill`] trait, its risk classification, and the execution context.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value as Json;
use thiserror::Error;

use jesus_twin_inference::Engine;
use jesus_twin_store::Store;

#[derive(Debug, Error)]
pub enum SkillError {
    #[error("unknown skill: {0}")]
    Unknown(String),
    #[error("invalid arguments: {0}")]
    InvalidArgs(String),
    #[error("execution failed: {0}")]
    Execution(String),
    #[error("not authorized: {0}")]
    NotAuthorized(String),
}

/// A JSON Schema describing a skill's parameters, in OpenAI tool format. Carried as a JSON
/// value so it can be emitted verbatim to the model tool-list and to MCP.
pub type ToolSchema = Json;

/// How risky/irreversible a skill's effect is. This — NOT the persona — drives authorization
/// (ALIGNMENT_AND_TUNING.md §3: "persona ≠ permission"). Reads run autonomously; anything that
/// acts on the outside world or can't be undone must pass a deterministic, human-checkpointed
/// gate before execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskClass {
    /// Read-only: retrieval, rendering, cross-references. Safe to run autonomously.
    ReadOnly,
    /// Sends/writes externally (message, email, API write). Requires approval.
    Outbound,
    /// Irreversible (delete, spend, overwrite). Requires explicit approval.
    Irreversible,
}

/// Backend handles a skill may use. Trait objects so skills stay decoupled from concrete
/// store/engine impls (and testable with doubles).
#[derive(Clone)]
pub struct SkillCtx {
    pub store: Arc<dyn Store>,
    pub engine: Arc<dyn Engine>,
}

impl SkillCtx {
    pub fn new(store: Arc<dyn Store>, engine: Arc<dyn Engine>) -> Self {
        Self { store, engine }
    }
}

/// A capability the twin can expose. Defined once, surfaced via CLI + MCP + model tool-list.
#[async_trait]
pub trait Skill: Send + Sync {
    /// Stable tool name (e.g. `lookup_saying`).
    fn name(&self) -> &str;

    /// Human-readable description (used in the tool-list / MCP tool metadata).
    fn description(&self) -> &str;

    /// The skill's risk class — the authorization input (see [`RiskClass`]).
    fn risk(&self) -> RiskClass;

    /// JSON schema for the skill's arguments (OpenAI tool format).
    fn schema(&self) -> ToolSchema;

    /// Execute the skill. The registry enforces authorization *before* this is called, so an
    /// implementation may assume it is cleared to run.
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError>;
}
