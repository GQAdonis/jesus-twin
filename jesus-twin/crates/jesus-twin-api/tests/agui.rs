//! Integration tests for the AG-UI SSE adapter.

mod common;

use axum::http::StatusCode;
use common::post_json;
use serde_json::json;

#[tokio::test]
async fn agui_streams_standard_and_custom_events() {
    let (status, body) = post_json(
        "/agui",
        json!({ "messages": [{ "role": "user", "content": "render to Caesar" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Standard AG-UI lifecycle events.
    assert!(body.contains("RUN_STARTED"), "expected RUN_STARTED");
    assert!(
        body.contains("TEXT_MESSAGE_CONTENT"),
        "expected text content event"
    );
    assert!(body.contains("Give Caesar his coin."));
    assert!(body.contains("RUN_FINISHED"), "expected RUN_FINISHED");
    // Custom namespaced citation chunk.
    assert!(
        body.contains("x-jesus-twin/citation"),
        "expected namespaced citation chunk"
    );
    assert!(body.contains("Mark 12:17"));
}

#[tokio::test]
async fn agui_refusal_emits_namespaced_chunk() {
    let (status, body) = post_json(
        "/agui",
        json!({ "messages": [{ "role": "user", "content": "what about crypto" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("x-jesus-twin/refusal"),
        "expected a refusal chunk"
    );
    assert!(
        !body.contains("TEXT_MESSAGE_CONTENT"),
        "no generation on refusal"
    );
}

#[tokio::test]
async fn agui_missing_user_is_bad_request() {
    let (status, _) = post_json("/agui", json!({ "messages": [] })).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
