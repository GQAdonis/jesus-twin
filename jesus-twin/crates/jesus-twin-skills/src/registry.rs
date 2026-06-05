//! The [`Registry`] — one registry backing the CLI, MCP server, and model tool-list, and the
//! single point where authorization is enforced before any skill runs.

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::{Value as Json, json};

use crate::authz::{Authorizer, AutoAllowReadOnly, Decision};
use crate::skill::{Skill, SkillCtx, SkillError, ToolSchema};

/// Holds the registered skills plus the [`Authorizer`] that gates them. Cheap to clone
/// (Arc-backed). Defaults to [`AutoAllowReadOnly`] — the safe policy when no approval channel
/// exists.
#[derive(Clone)]
pub struct Registry {
    skills: HashMap<String, Arc<dyn Skill>>,
    authorizer: Arc<dyn Authorizer>,
}

impl Default for Registry {
    fn default() -> Self {
        Self {
            skills: HashMap::new(),
            authorizer: Arc::new(AutoAllowReadOnly),
        }
    }
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Use a specific authorizer (e.g. a `HumanCheckpoint`) instead of the read-only default.
    pub fn with_authorizer(mut self, authorizer: Arc<dyn Authorizer>) -> Self {
        self.authorizer = authorizer;
        self
    }

    /// Register a skill, returning a new registry (immutable builder style).
    pub fn with(mut self, skill: Arc<dyn Skill>) -> Self {
        self.skills.insert(skill.name().to_string(), skill);
        self
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Skill>> {
        self.skills.get(name)
    }

    /// Sorted skill names (stable ordering for CLI listing / tests).
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.skills.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }

    /// OpenAI-format tool specs for every skill, for injection into the model's tool-call list
    /// and the MCP tool registry. Each is `{ type: "function", function: { name, description,
    /// parameters } }`.
    pub fn tool_specs(&self) -> Vec<Json> {
        let mut specs: Vec<Json> = self
            .skills
            .values()
            .map(|s| {
                json!({
                    "type": "function",
                    "function": {
                        "name": s.name(),
                        "description": s.description(),
                        "parameters": s.schema(),
                    }
                })
            })
            .collect();
        specs.sort_by(|a, b| {
            a["function"]["name"]
                .as_str()
                .cmp(&b["function"]["name"].as_str())
        });
        specs
    }

    /// Raw argument schemas (kept for callers that want just the parameter schemas).
    pub fn schemas(&self) -> Vec<ToolSchema> {
        self.skills.values().map(|s| s.schema()).collect()
    }

    /// Authorize, then invoke `name` with `args`. This is the **only** path skills run through,
    /// so the policy gate can't be bypassed (ALIGNMENT_AND_TUNING.md §3). Unknown name ->
    /// `Unknown`; denied -> `NotAuthorized`; otherwise the skill's result.
    pub async fn invoke(&self, name: &str, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError> {
        let skill = self
            .get(name)
            .ok_or_else(|| SkillError::Unknown(name.to_string()))?;
        match self.authorizer.authorize(name, skill.risk()) {
            Decision::Allow => {
                tracing::info!(skill = name, risk = ?skill.risk(), "skill authorized");
                skill.invoke(args, ctx).await
            }
            Decision::Deny(reason) => {
                tracing::warn!(skill = name, %reason, "skill denied");
                Err(SkillError::NotAuthorized(reason))
            }
        }
    }
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("skills", &self.names())
            .finish()
    }
}
