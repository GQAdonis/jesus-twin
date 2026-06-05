//! The Axum router: mounts every protocol adapter on one shared core.
//!
//! Build order (ARCHITECTURE.md §11): OpenAI adapter first (step 4), then MCP, AG-UI, A2A
//! (step 6). The router carries [`AppState`] (the shared `Arc<dyn Agent>`); each adapter is
//! a `Router<AppState>` merged in, then state is applied once at the end.

use axum::Router;
use axum::routing::get;

use crate::adapters;
use crate::state::AppState;

/// Build the application router over `state`: the health check plus every protocol surface on
/// the one shared core. The state-driven adapters (OpenAI, AG-UI, A2A) are merged and have
/// state applied; MCP is merged separately because its `StreamableHttpService` captures the
/// agent in a per-session factory closure rather than using Axum `State`.
pub fn router(state: AppState) -> Router {
    let stateful = Router::new()
        .route("/health", get(health))
        .merge(adapters::openai::routes())
        .merge(adapters::agui::routes())
        .merge(adapters::a2a::routes())
        .with_state(state.clone());

    stateful.merge(adapters::mcp::router(state.agent, state.skills))
}

async fn health() -> &'static str {
    "ok"
}
