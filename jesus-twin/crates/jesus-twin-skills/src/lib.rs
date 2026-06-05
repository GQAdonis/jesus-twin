//! The skill system: one definition, three frontends.
//!
//! A [`skill::Skill`] is defined once and exposed via the CLI, the MCP server, and the
//! model's tool-call list (ARCHITECTURE.md §8). Built-in skills: `lookup_saying`,
//! `find_by_move`, `parallels`, `render_modern` — each a thin wrapper over the store /
//! inference engine.
//!
//! Safety (ALIGNMENT_AND_TUNING.md §3): persona is NOT permission. Every skill runs through
//! [`registry::Registry::invoke`], which enforces a deterministic, risk-classified
//! [`authz::Authorizer`] *before* execution — independent of the persona. The default policy
//! ([`authz::AutoAllowReadOnly`]) runs read-only skills autonomously and denies anything
//! outbound/irreversible unless a [`authz::HumanCheckpoint`] approval channel is wired.

pub mod authz;
pub mod builtins;
pub mod registry;
pub mod skill;

pub use authz::{Authorizer, AutoAllowReadOnly, Decision, HumanCheckpoint};
pub use builtins::register_builtins;
pub use registry::Registry;
pub use skill::{RiskClass, Skill, SkillCtx, SkillError, ToolSchema};
