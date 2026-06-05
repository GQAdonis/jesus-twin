//! Integration tests for the A2A JSON-RPC adapter + agent card.

mod common;

use axum::http::StatusCode;
use common::{get, post_json};
use serde_json::{Value, json};

#[tokio::test]
async fn message_send_returns_completed_task_with_artifacts() {
    let (status, body) = post_json(
        "/a2a",
        json!({
            "jsonrpc": "2.0", "id": 1, "method": "message/send",
            "params": { "message": { "role": "user", "parts": [{ "text": "render to Caesar" }] } }
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["result"]["status"]["state"], "completed");
    let artifacts = v["result"]["artifacts"].as_array().unwrap();
    let message = artifacts.iter().find(|a| a["type"] == "message").unwrap();
    assert!(
        message["content"]
            .as_str()
            .unwrap()
            .contains("Give Caesar his coin.")
    );
    let cites = artifacts
        .iter()
        .find(|a| a["type"] == "x-jesus-twin/citations")
        .unwrap();
    assert_eq!(cites["citations"][0]["ref"], "Mark 12:17");
}

#[tokio::test]
async fn unknown_method_returns_method_not_found() {
    let (status, body) = post_json(
        "/a2a",
        json!({ "jsonrpc": "2.0", "id": 2, "method": "does/not/exist", "params": {} }),
    )
    .await;
    assert_eq!(status, StatusCode::OK); // JSON-RPC errors ride in the body, not the HTTP status
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["error"]["code"], -32601);
}

#[tokio::test]
async fn agent_card_is_served() {
    let (status, body) = get("/.well-known/agent.json").await;
    assert_eq!(status, StatusCode::OK);
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(v["name"], "jesus-twin");
    assert_eq!(v["protocolVersion"], "1.0");
    assert!(
        v["skills"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["id"] == "ask")
    );
}
