//! MCP server adapter: exposes the twin as an MCP tool over **streamable HTTP** (the remote
//! transport), nested at `/mcp` (ARCHITECTURE.md §4). The stdio transport is deferred to the
//! CLI. Built on the official `rmcp` SDK.
//!
//! The twin is *also* an MCP **client** (via mistral.rs, for external tools) — that is a
//! separate role and lives elsewhere (ALIGNMENT_AND_TUNING.md §3). This module is only the
//! server: it surfaces one `ask` tool whose result is the grounded, cited answer.
//!
//! Unlike the other adapters, MCP can't use Axum `State`: `StreamableHttpService` builds a
//! fresh server per session via a factory closure, so the `Arc<dyn Agent>` is captured in the
//! closure instead. Hence [`router`] takes the agent directly and returns a plain `Router`
//! that the app merges *before* `.with_state(...)`.

use std::sync::Arc;

use axum::Router;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, Content, ProtocolVersion, ServerCapabilities, ServerInfo};
use rmcp::transport::streamable_http_server::StreamableHttpService;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;
use serde_json::json;

use jesus_twin_core::event::AgentEvent;
use jesus_twin_core::{Agent, Role, Session, Turn};

use crate::state::Skills;

/// The MCP server instance. Cloned per session by the transport's factory; the agent handle
/// (and optional skills) are shared (Arc), so all sessions drive the same core.
#[derive(Clone)]
pub struct TwinMcp {
    agent: Arc<dyn Agent>,
    skills: Option<Skills>,
    // Read by the `#[tool_handler]`-generated `ServerHandler` impl, not directly here.
    #[allow(dead_code)]
    tool_router: ToolRouter<TwinMcp>,
}

/// Arguments for the `ask` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AskArgs {
    /// The question to put to the twin.
    pub question: String,
}

/// Arguments for the `invoke_skill` tool.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct InvokeSkillArgs {
    /// The skill name (see `list_skills`), e.g. "lookup_saying".
    pub name: String,
    /// JSON arguments object for the skill, e.g. {"ref":"Mark 12:17"}.
    #[serde(default)]
    pub args: serde_json::Value,
}

#[tool_router]
impl TwinMcp {
    pub fn new(agent: Arc<dyn Agent>, skills: Option<Skills>) -> Self {
        Self {
            agent,
            skills,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "Ask the digital twin of Jesus a question. Returns a present-day-English \
                       answer grounded in cited verses, or a refusal if the recorded teachings \
                       don't address it. Never fabricates."
    )]
    async fn ask(
        &self,
        Parameters(AskArgs { question }): Parameters<AskArgs>,
    ) -> Result<CallToolResult, McpError> {
        let session = Session::new(uuid::Uuid::new_v4()).with_turn(Turn::new(Role::User, question));
        let events = self
            .agent
            .run(&session)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        let (text, citations) = project(&events);
        // Return the answer as text plus a structured citations block, so MCP clients can both
        // display the answer and verify the grounding.
        Ok(CallToolResult::success(vec![
            Content::text(text),
            Content::text(json!({ "citations": citations }).to_string()),
        ]))
    }

    #[tool(
        description = "List the twin's available skills with their OpenAI-format tool specs \
                       (name, description, parameters). Use `invoke_skill` to run one."
    )]
    async fn list_skills(&self) -> Result<CallToolResult, McpError> {
        let specs = match &self.skills {
            Some(s) => s.registry.tool_specs(),
            None => Vec::new(),
        };
        Ok(CallToolResult::success(vec![Content::text(
            json!({ "skills": specs }).to_string(),
        )]))
    }

    #[tool(
        description = "Invoke one of the twin's registered skills by name with JSON arguments. \
                       Authorization is enforced by the registry (read-only skills run; \
                       outbound/irreversible ones are denied without an approval channel)."
    )]
    async fn invoke_skill(
        &self,
        Parameters(InvokeSkillArgs { name, args }): Parameters<InvokeSkillArgs>,
    ) -> Result<CallToolResult, McpError> {
        let skills = self
            .skills
            .as_ref()
            .ok_or_else(|| McpError::internal_error("skills not configured", None))?;
        let result = skills
            .registry
            .invoke(&name, args, &skills.ctx)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for TwinMcp {
    fn get_info(&self) -> ServerInfo {
        // ServerInfo / Implementation are #[non_exhaustive] (cross-crate), so they can't be
        // built with a struct literal — start from Default and set the fields we care about.
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::V_2025_03_26;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info.name = "jesus-twin".to_string();
        info.server_info.version = "0.1.0".to_string();
        info.instructions = Some(
            "A study aid rendering the recorded teachings of Jesus of Nazareth in present-day \
             English, grounded in cited verses. Use `ask` for a full answer, `list_skills` to \
             discover the read-only skills, and `invoke_skill` to run one."
                .to_string(),
        );
        info
    }
}

/// Build the MCP streamable-HTTP router, nested at `/mcp`. The agent + skills are captured in
/// the per-session factory closure (MCP can't use Axum `State`).
pub fn router(agent: Arc<dyn Agent>, skills: Option<Skills>) -> Router {
    let service = StreamableHttpService::new(
        move || Ok(TwinMcp::new(agent.clone(), skills.clone())),
        LocalSessionManager::default().into(),
        Default::default(),
    );
    Router::new().nest_service("/mcp", service)
}

/// Collapse the event stream into (answer text, citation list) for the tool result.
fn project(events: &[AgentEvent]) -> (String, Vec<serde_json::Value>) {
    let mut text = String::new();
    let mut citations = Vec::new();
    for ev in events {
        match ev {
            AgentEvent::TextMessageDelta { delta, .. } => text.push_str(delta),
            AgentEvent::Refusal { .. } => {
                text.push_str("The recorded teachings of Jesus don't address that.");
            }
            AgentEvent::Citation { ref_, score, .. } => {
                citations.push(json!({ "ref": ref_, "score": score }));
            }
            _ => {}
        }
    }
    (text, citations)
}
