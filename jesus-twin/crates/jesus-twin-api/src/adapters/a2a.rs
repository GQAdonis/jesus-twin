//! A2A adapter: JSON-RPC 2.0 (`message/send`, `tasks/get`) + an Agent Card at
//! `/.well-known/agent.json` (ARCHITECTURE.md §4).
//!
//! A2A wraps the canonical event stream in a *task* envelope: a `message/send` runs a turn
//! synchronously and returns a completed `Task` whose artifacts are the assistant message and
//! the citations. `tasks/get` returns a stored task by id. Hand-rolled (no `ra2a` dep) for the
//! two core methods; the structure leaves room to add streaming/push later.

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use jesus_twin_core::event::AgentEvent;

use crate::adapters::common::{WireMessage, session_from_messages};
use crate::state::AppState;

/// In-memory store of completed tasks, keyed by task id, for `tasks/get`. Bounded only by
/// process lifetime — fine for the edge build; a scaled deployment would back this with the
/// store. Wrapped in the router as extension state.
type TaskStore = Arc<Mutex<HashMap<String, Value>>>;

/// Mount the A2A routes (JSON-RPC endpoint + agent card).
pub fn routes() -> Router<AppState> {
    let tasks: TaskStore = Arc::new(Mutex::new(HashMap::new()));
    Router::new()
        .route("/a2a", post(jsonrpc))
        .route("/.well-known/agent.json", get(agent_card))
        .layer(axum::Extension(tasks))
}

#[derive(Debug, Deserialize)]
struct RpcRequest {
    #[serde(default)]
    id: Value,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC 2.0 dispatch. Unknown methods get the standard `-32601`.
async fn jsonrpc(
    State(state): State<AppState>,
    axum::Extension(tasks): axum::Extension<TaskStore>,
    Json(req): Json<RpcRequest>,
) -> Response {
    let result = match req.method.as_str() {
        "message/send" => message_send(&state, &tasks, req.params).await,
        "tasks/get" => tasks_get(&tasks, req.params).await,
        _ => Err(rpc_error(-32601, "method not found")),
    };
    match result {
        Ok(value) => {
            Json(json!({ "jsonrpc": "2.0", "id": req.id, "result": value })).into_response()
        }
        Err(error) => {
            Json(json!({ "jsonrpc": "2.0", "id": req.id, "error": error })).into_response()
        }
    }
}

/// `message/send`: run a turn, build a completed Task with message + citation artifacts.
async fn message_send(state: &AppState, tasks: &TaskStore, params: Value) -> Result<Value, Value> {
    let messages = extract_messages(&params);
    let session =
        session_from_messages(&messages).ok_or_else(|| rpc_error(-32602, "no user message"))?;
    let events = state
        .agent
        .run(&session)
        .await
        .map_err(|e| rpc_error(-32000, &e.to_string()))?;

    let task_id = Uuid::new_v4().to_string();
    let task = build_task(&task_id, &events);
    tasks.lock().await.insert(task_id, task.clone());
    Ok(task)
}

/// `tasks/get`: return a previously stored task by id.
async fn tasks_get(tasks: &TaskStore, params: Value) -> Result<Value, Value> {
    let id = params
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| rpc_error(-32602, "missing id"))?;
    tasks
        .lock()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| rpc_error(-32001, "task not found"))
}

/// Project the event stream into an A2A Task object (status + artifacts).
fn build_task(id: &str, events: &[AgentEvent]) -> Value {
    let mut text = String::new();
    let mut citations = Vec::new();
    let mut refused = false;

    for ev in events {
        match ev {
            AgentEvent::TextMessageDelta { delta, .. } => text.push_str(delta),
            AgentEvent::Refusal { .. } => {
                refused = true;
                text.push_str("The recorded teachings of Jesus don't address that.");
            }
            AgentEvent::Citation { ref_, score, .. } => {
                citations.push(json!({ "ref": ref_, "score": score }));
            }
            _ => {}
        }
    }

    json!({
        "id": id,
        "status": { "state": "completed" },
        "artifacts": [
            { "type": "message", "role": "assistant", "content": text },
            { "type": "x-jesus-twin/citations", "citations": citations },
        ],
        "metadata": { "refused": refused },
    })
}

/// The Agent Card served at `/.well-known/agent.json` (A2A discovery).
async fn agent_card() -> Json<Value> {
    Json(json!({
        "name": "jesus-twin",
        "description": "A study aid that renders the recorded teachings of Jesus of Nazareth \
                        in present-day English, grounded in cited verses; refuses out-of-corpus \
                        questions.",
        "version": "0.1.0",
        "protocolVersion": "1.0",
        "capabilities": { "streaming": false, "pushNotifications": false },
        "skills": [{
            "id": "ask",
            "name": "Ask the twin",
            "description": "Answer a question grounded in the red-letter corpus, with citations."
        }],
        "url": "/a2a",
    }))
}

fn extract_messages(params: &Value) -> Vec<WireMessage> {
    // Accept either `{ message: {...} }` (single) or `{ messages: [...] }`.
    if let Some(arr) = params.get("messages").and_then(Value::as_array) {
        return arr.iter().filter_map(value_to_message).collect();
    }
    params
        .get("message")
        .and_then(value_to_message)
        .into_iter()
        .collect()
}

fn value_to_message(v: &Value) -> Option<WireMessage> {
    let role = v
        .get("role")
        .and_then(Value::as_str)
        .unwrap_or("user")
        .to_string();
    // A2A messages carry `parts`; fall back to a plain `content` string.
    let content = v
        .get("parts")
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("")
        })
        .or_else(|| v.get("content").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    Some(WireMessage { role, content })
}

fn rpc_error(code: i32, message: &str) -> Value {
    json!({ "code": code, "message": message })
}
