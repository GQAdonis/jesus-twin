//! AG-UI adapter: a single SSE endpoint emitting AG-UI event JSON.
//!
//! AG-UI is the canonical shape the core stream is modeled on, so this mapping is nearly 1:1
//! (ARCHITECTURE.md §4). Standard events (`RUN_STARTED`, `TEXT_MESSAGE_*`, `TOOL_CALL_*`,
//! `STATE_SNAPSHOT`, `RUN_FINISHED`) project directly. The project's distinctive signal rides
//! as **custom, namespaced** chunks (`x-jesus-twin/citation`, `.../refusal`) so standard
//! AG-UI clients ignore what they don't understand (ALIGNMENT_AND_TUNING.md §4).

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use futures::stream::{self, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};

use jesus_twin_core::event::AgentEvent;

use crate::adapters::common::{WireMessage, session_from_messages, status_for};
use crate::error::error_json;
use crate::state::AppState;

/// Mount the AG-UI route.
pub fn routes() -> Router<AppState> {
    Router::new().route("/agui", post(run))
}

/// AG-UI run input: the running thread of messages. (A subset of the AG-UI `RunAgentInput`
/// — enough to drive a turn; thread/run ids are echoed if present.)
#[derive(Debug, Deserialize)]
struct RunInput {
    #[serde(default)]
    messages: Vec<WireMessage>,
}

/// `POST /agui` — run a turn and stream AG-UI events as SSE.
async fn run(State(state): State<AppState>, Json(input): Json<RunInput>) -> Response {
    let session = match session_from_messages(&input.messages) {
        Some(s) => s,
        None => return error_json(axum::http::StatusCode::BAD_REQUEST, "no user message"),
    };
    let events = match state.agent.run(&session).await {
        Ok(e) => e,
        Err(e) => return error_json(status_for(e.kind), &e.to_string()),
    };

    let frames: Vec<Event> = events.iter().filter_map(to_agui_event).collect();
    let body = stream::iter(frames).map(Ok::<_, Infallible>);
    Sse::new(body)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Project one core event into an AG-UI SSE event, or `None` to drop it.
fn to_agui_event(ev: &AgentEvent) -> Option<Event> {
    let payload: Value = match ev {
        AgentEvent::RunStarted { run_id, session_id } => {
            json!({ "type": "RUN_STARTED", "runId": run_id, "threadId": session_id })
        }
        AgentEvent::TextMessageStart { message_id, role } => {
            json!({ "type": "TEXT_MESSAGE_START", "messageId": message_id, "role": role })
        }
        AgentEvent::TextMessageDelta { message_id, delta } => {
            json!({ "type": "TEXT_MESSAGE_CONTENT", "messageId": message_id, "delta": delta })
        }
        AgentEvent::TextMessageEnd { message_id } => {
            json!({ "type": "TEXT_MESSAGE_END", "messageId": message_id })
        }
        AgentEvent::ToolCallStart { tool_call_id, name } => {
            json!({ "type": "TOOL_CALL_START", "toolCallId": tool_call_id, "toolCallName": name })
        }
        AgentEvent::ToolCallArgsDelta {
            tool_call_id,
            delta,
        } => {
            json!({ "type": "TOOL_CALL_ARGS", "toolCallId": tool_call_id, "delta": delta })
        }
        AgentEvent::ToolCallEnd { tool_call_id } => {
            json!({ "type": "TOOL_CALL_END", "toolCallId": tool_call_id })
        }
        AgentEvent::ToolResult {
            tool_call_id,
            content,
        } => {
            json!({ "type": "TOOL_CALL_RESULT", "toolCallId": tool_call_id, "content": content })
        }
        AgentEvent::StateSnapshot { state } => {
            json!({ "type": "STATE_SNAPSHOT", "snapshot": state })
        }
        // Custom, namespaced chunks — standard clients ignore unknown `type`s.
        AgentEvent::Citation { ref_, score, span } => {
            json!({ "type": "x-jesus-twin/citation", "ref": ref_, "score": score, "span": span })
        }
        AgentEvent::Refusal { reason } => {
            json!({ "type": "x-jesus-twin/refusal", "reason": reason })
        }
        // Already-namespaced custom chunk (e.g. x-jesus-twin/low-confidence) — pass through.
        AgentEvent::Custom { name, data } => {
            json!({ "type": name, "data": data })
        }
        AgentEvent::RunFinished { run_id, finish } => {
            json!({ "type": "RUN_FINISHED", "runId": run_id, "finish": finish })
        }
        AgentEvent::Error { code, message } => {
            json!({ "type": "RUN_ERROR", "code": code, "message": message })
        }
    };
    Some(Event::default().data(payload.to_string()))
}
