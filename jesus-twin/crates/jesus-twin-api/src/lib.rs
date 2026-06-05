//! HTTP/protocol surface. One Axum app mounts the protocol adapters on the single agent core
//! (ARCHITECTURE.md §4). Adapters only *translate* the canonical `AgentEvent` stream into
//! their wire format — they contain no agent logic.

pub mod adapters;
pub mod app;
pub mod error;
pub mod state;

pub use app::router;
pub use state::AppState;

/// Bind `addr` and serve the app over `state`. Keeps `axum::serve` inside this crate so the
/// CLI depends only on `jesus-twin-api`, not on axum directly (thin-adapter boundary).
pub async fn serve(addr: &str, state: AppState) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router(state)).await
}
