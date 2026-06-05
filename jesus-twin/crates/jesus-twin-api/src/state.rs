//! Shared application state.
//!
//! Holds the single agent core behind `Arc<dyn Agent>` so every protocol adapter projects
//! from the same orchestrator (ARCHITECTURE.md §1). `Arc<dyn Agent>` is `Send + Sync`, which
//! Axum requires of state shared across handlers running on any worker thread.
//!
//! Optionally also carries the skill [`Registry`] + a [`SkillCtx`] so the MCP adapter can
//! surface the registered skills as MCP tools (the registry enforces authorization). When
//! absent, MCP exposes only the `ask` tool.

use std::sync::Arc;

use jesus_twin_core::Agent;
use jesus_twin_skills::{Registry, SkillCtx};

/// The skill registry plus the context needed to invoke its skills.
#[derive(Clone)]
pub struct Skills {
    pub registry: Registry,
    pub ctx: Arc<SkillCtx>,
}

/// Cloneable handle to the app's shared services. Clone is cheap (Arc bump).
#[derive(Clone)]
pub struct AppState {
    pub agent: Arc<dyn Agent>,
    pub skills: Option<Skills>,
}

impl AppState {
    /// State with just the agent (MCP exposes only `ask`).
    pub fn new(agent: Arc<dyn Agent>) -> Self {
        Self {
            agent,
            skills: None,
        }
    }

    /// Also expose the skill registry over MCP.
    pub fn with_skills(mut self, registry: Registry, ctx: Arc<SkillCtx>) -> Self {
        self.skills = Some(Skills { registry, ctx });
        self
    }
}
