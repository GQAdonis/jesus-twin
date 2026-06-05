//! Built-in skills (ARCHITECTURE.md §8). All are **read-only** ([`RiskClass::ReadOnly`]) —
//! the default tool set for a study aid is informational (ALIGNMENT_AND_TUNING.md §3). Each is
//! a thin wrapper over the store/engine in [`SkillCtx`].
//!
//! - `lookup_saying(ref)`   — fetch one saying by exact citation.
//! - `find_by_move(move)`   — list sayings tagged with a reasoning move (M01..M18).
//! - `parallels(query)`     — find structurally/semantically related sayings.
//! - `render_modern(ref)`   — render a saying's original text in present-day English.

use async_trait::async_trait;
use serde_json::{Value as Json, json};

use jesus_twin_inference::GenRequest;

use crate::skill::{RiskClass, Skill, SkillCtx, SkillError, ToolSchema};

/// Register all built-in (read-only) skills onto a registry.
pub fn register_builtins(registry: crate::Registry) -> crate::Registry {
    registry
        .with(std::sync::Arc::new(LookupSaying))
        .with(std::sync::Arc::new(FindByMove))
        .with(std::sync::Arc::new(Parallels))
        .with(std::sync::Arc::new(RenderModern))
        .with(std::sync::Arc::new(Mindmap))
}

/// Pull a required string argument or return an `InvalidArgs` error.
fn arg_str(args: &Json, key: &str) -> Result<String, SkillError> {
    args.get(key)
        .and_then(Json::as_str)
        .map(str::to_string)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| SkillError::InvalidArgs(format!("missing string argument '{key}'")))
}

fn string_param(name: &str, desc: &str) -> ToolSchema {
    json!({
        "type": "object",
        "properties": { name: { "type": "string", "description": desc } },
        "required": [name],
    })
}

// ---- lookup_saying ---------------------------------------------------------------------

pub struct LookupSaying;

#[async_trait]
impl Skill for LookupSaying {
    fn name(&self) -> &str {
        "lookup_saying"
    }
    fn description(&self) -> &str {
        "Fetch a single recorded saying by its exact scripture reference (e.g. 'Mark 12:17')."
    }
    fn risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        string_param("ref", "Scripture reference, e.g. 'Mark 12:17'.")
    }
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError> {
        let reference = arg_str(&args, "ref")?;
        let found = ctx
            .store
            .get_by_ref(&reference)
            .await
            .map_err(|e| SkillError::Execution(e.to_string()))?;
        match found {
            Some(p) => Ok(json!({
                "ref": p.ref_, "text_original": p.text_original,
                "text_modern": p.text_modern, "move": p.move_,
            })),
            None => Ok(json!({ "ref": reference, "found": false })),
        }
    }
}

// ---- find_by_move ----------------------------------------------------------------------

pub struct FindByMove;

#[async_trait]
impl Skill for FindByMove {
    fn name(&self) -> &str {
        "find_by_move"
    }
    fn description(&self) -> &str {
        "List sayings that use a given reasoning move (M01..M18). May be empty until the \
         corpus is annotated with moves."
    }
    fn risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        string_param("move", "Reasoning move id, e.g. 'M02'.")
    }
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError> {
        let move_id = arg_str(&args, "move")?;
        let found = ctx
            .store
            .find_by_move(&move_id, 20)
            .await
            .map_err(|e| SkillError::Execution(e.to_string()))?;
        let sayings: Vec<Json> = found
            .iter()
            .map(|p| json!({ "ref": p.ref_, "text_original": p.text_original }))
            .collect();
        Ok(json!({ "move": move_id, "count": sayings.len(), "sayings": sayings }))
    }
}

// ---- parallels -------------------------------------------------------------------------

pub struct Parallels;

#[async_trait]
impl Skill for Parallels {
    fn name(&self) -> &str {
        "parallels"
    }
    fn description(&self) -> &str {
        "Find sayings related to a topic or phrase (synoptic parallels / semantically close)."
    }
    fn risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        string_param("query", "A topic or phrase to find related sayings for.")
    }
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError> {
        let query = arg_str(&args, "query")?;
        let set = ctx
            .store
            .retrieve(&query, 5)
            .await
            .map_err(|e| SkillError::Execution(e.to_string()))?;
        let related: Vec<Json> = set
            .passages
            .iter()
            .map(|p| json!({ "ref": p.ref_, "text_original": p.text_original, "score": p.score }))
            .collect();
        Ok(json!({ "query": query, "related": related }))
    }
}

// ---- render_modern ---------------------------------------------------------------------

pub struct RenderModern;

#[async_trait]
impl Skill for RenderModern {
    fn name(&self) -> &str {
        "render_modern"
    }
    fn description(&self) -> &str {
        "Render a saying (by reference) in present-day English, preserving its force. The \
         transform is always anchored to the cited original text — never invented."
    }
    fn risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        string_param("ref", "Scripture reference to render, e.g. 'Mark 12:17'.")
    }
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError> {
        let reference = arg_str(&args, "ref")?;
        let saying = ctx
            .store
            .get_by_ref(&reference)
            .await
            .map_err(|e| SkillError::Execution(e.to_string()))?
            .ok_or_else(|| SkillError::Execution(format!("no saying at '{reference}'")))?;

        // Grounded on the real cited line; the engine renders, it does not generate doctrine.
        let modern = ctx
            .engine
            .generate(GenRequest {
                system: "Render the supplied saying in present-day English, preserving its \
                         force. Use only the supplied text; never invent."
                    .to_string(),
                context: format!("{}: {}", saying.ref_, saying.text_original),
                user: format!("Render {} in present-day English.", saying.ref_),
            })
            .await
            .map_err(|e| SkillError::Execution(e.to_string()))?;

        Ok(json!({ "ref": saying.ref_, "original": saying.text_original, "modern": modern }))
    }
}

// ---- mindmap ---------------------------------------------------------------------------

pub struct Mindmap;

#[async_trait]
impl Skill for Mindmap {
    fn name(&self) -> &str {
        "mindmap"
    }
    fn description(&self) -> &str {
        "Project a mind-map graph around a topic: the related sayings, their reasoning moves, \
         and parallels between sayings that share a move. Returns nodes + edges."
    }
    fn risk(&self) -> RiskClass {
        RiskClass::ReadOnly
    }
    fn schema(&self) -> ToolSchema {
        string_param("topic", "A topic or phrase to build the mind-map around.")
    }
    async fn invoke(&self, args: Json, ctx: &SkillCtx) -> Result<Json, SkillError> {
        let topic = arg_str(&args, "topic")?;
        let delta = ctx
            .store
            .mindmap(&topic, 8)
            .await
            .map_err(|e| SkillError::Execution(e.to_string()))?;
        serde_json::to_value(&delta).map_err(|e| SkillError::Execution(e.to_string()))
    }
}
