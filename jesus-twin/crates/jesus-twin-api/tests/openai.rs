//! Integration tests for the OpenAI adapter — driven through the real Axum router with a
//! canned `Agent` double (see `common`), exercising the AgentEvent -> OpenAI projection.

mod common;

use axum::http::StatusCode;
use common::{OverloadedAgent, app_with, post_json, post_json_to};
use serde_json::{Value, json};

#[tokio::test]
async fn non_stream_chat_returns_content_and_citations() {
    let (status, body) = post_json(
        "/v1/chat/completions",
        json!({ "messages": [{ "role": "user", "content": "render to Caesar" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["object"], "chat.completion");
    let content = v["choices"][0]["message"]["content"].as_str().unwrap();
    assert!(content.contains("Give Caesar his coin."), "got: {content}");
    assert!(
        content.contains("Mark 12:17"),
        "citation should surface inline"
    );
    assert_eq!(v["choices"][0]["finish_reason"], "stop");
    assert_eq!(v["metadata"]["citations"][0]["ref"], "Mark 12:17");
}

#[tokio::test]
async fn non_stream_refusal_uses_content_filter_finish() {
    let (status, body) = post_json(
        "/v1/chat/completions",
        json!({ "messages": [{ "role": "user", "content": "what about crypto" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["choices"][0]["finish_reason"], "content_filter");
    assert!(
        v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap()
            .contains("don't address that")
    );
}

#[tokio::test]
async fn stream_chat_emits_chunks_and_done() {
    let (status, body) = post_json(
        "/v1/chat/completions",
        json!({ "messages": [{ "role": "user", "content": "render to Caesar" }], "stream": true }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("chat.completion.chunk"),
        "expected chunk objects"
    );
    assert!(
        body.contains("Give Caesar his coin."),
        "expected the delta content"
    );
    assert!(
        body.contains("\"finish_reason\":\"stop\""),
        "expected a finish chunk"
    );
    assert!(
        body.trim_end().ends_with("[DONE]"),
        "stream must terminate with [DONE]"
    );
}

#[tokio::test]
async fn missing_user_message_is_bad_request() {
    let (status, _) = post_json(
        "/v1/chat/completions",
        json!({ "messages": [{ "role": "system", "content": "hi" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn embeddings_is_not_implemented() {
    let (status, _) = post_json("/v1/embeddings", json!({ "input": "x" })).await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
}

#[tokio::test]
async fn admission_rejection_maps_to_503() {
    let (status, _) = post_json_to(
        app_with(OverloadedAgent),
        "/v1/chat/completions",
        json!({ "messages": [{ "role": "user", "content": "hi" }] }),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
}
