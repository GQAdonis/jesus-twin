//! Integration test for the MCP streamable-HTTP adapter.
//!
//! A full MCP handshake (initialize -> tools/list -> tools/call) needs the streamable-HTTP
//! session protocol with the right Accept headers; that is exercised live via curl in the
//! step's smoke test. Here we assert the service is actually mounted at `/mcp` (a bare POST
//! is handled by the MCP service, not a 404 from the router).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::app;
use tower::ServiceExt;

#[tokio::test]
async fn mcp_endpoint_is_mounted() {
    // A POST with no MCP session/headers is rejected by the MCP service itself (e.g. bad
    // request / not acceptable), NOT a 404 — which proves the service is mounted at /mcp.
    let resp = app()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp")
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "/mcp should be mounted"
    );
}
