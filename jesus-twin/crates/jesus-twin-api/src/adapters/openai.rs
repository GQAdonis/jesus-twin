//! OpenAI REST adapter: `/v1/chat/completions` (stream + non-stream) and `/v1/embeddings`.
//!
//! A thin projection of the canonical `AgentEvent` stream (ARCHITECTURE.md §4), no agent
//! logic. Mapping:
//! - `TextMessageDelta` -> `choices[].delta.content` (stream) / message content (non-stream)
//! - `Citation`         -> appended to content + mirrored in `metadata.citations`
//! - `Refusal`          -> a normal assistant message (honest refusal text)
//! - `RunFinished`      -> `finish_reason` (`stop` | `content_filter` for a refusal)
//!
//! `/v1/embeddings` is a deferred stub (501) until an `Embedder` is plumbed into state.

use std::convert::Infallible;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use futures::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use jesus_twin_core::event::{AgentEvent, FinishReason, RefusalReason};

use crate::adapters::common::{WireMessage, session_from_messages, status_for};
use crate::error::error_json;
use crate::state::AppState;

const MODEL_NAME: &str = "jesus-twin";

/// Mount the OpenAI-compatible routes onto a router carrying [`AppState`].
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/embeddings", post(embeddings))
}

// ---- request / response wire types -----------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    #[serde(default)]
    pub model: Option<String>,
    pub messages: Vec<WireMessage>,
    #[serde(default)]
    pub stream: bool,
}

#[derive(Debug, Serialize)]
struct ChatResponse {
    id: String,
    object: &'static str,
    model: &'static str,
    choices: Vec<Choice>,
    metadata: ResponseMetadata,
}

#[derive(Debug, Serialize)]
struct Choice {
    index: u32,
    message: OutMessage,
    finish_reason: &'static str,
}

#[derive(Debug, Serialize)]
struct OutMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct ResponseMetadata {
    citations: Vec<CitationOut>,
}

#[derive(Debug, Clone, Serialize)]
struct CitationOut {
    #[serde(rename = "ref")]
    ref_: String,
    score: f32,
}

// ---- handlers --------------------------------------------------------------------------

/// `POST /v1/chat/completions`. Streams SSE when `stream: true`, else returns one JSON body.
async fn chat_completions(State(state): State<AppState>, Json(req): Json<ChatRequest>) -> Response {
    let session = match session_from_messages(&req.messages) {
        Some(s) => s,
        None => {
            return error_json(
                axum::http::StatusCode::BAD_REQUEST,
                "no user message in request",
            );
        }
    };

    let events = match state.agent.run(&session).await {
        Ok(events) => events,
        Err(e) => return error_json(status_for(e.kind), &e.to_string()),
    };

    if req.stream {
        stream_response(events).into_response()
    } else {
        Json(project_non_stream(events)).into_response()
    }
}

/// `POST /v1/embeddings` — deferred until an `Embedder` is wired into state (CLAUDE.md
/// honesty: a clear 501, not a fake vector).
async fn embeddings() -> Response {
    error_json(
        axum::http::StatusCode::NOT_IMPLEMENTED,
        "embeddings are not wired yet (needs an Embedder in app state)",
    )
}

// ---- projection: AgentEvent stream -> OpenAI shapes -------------------------------------

/// Collapse the (already complete) event vector into a single non-streaming response.
fn project_non_stream(events: Vec<AgentEvent>) -> ChatResponse {
    let mut content = String::new();
    let mut citations = Vec::new();
    let mut finish = "stop";

    for ev in &events {
        match ev {
            AgentEvent::TextMessageDelta { delta, .. } => content.push_str(delta),
            AgentEvent::Refusal { reason } => {
                content.push_str(&refusal_text(reason));
                finish = "content_filter";
            }
            AgentEvent::Citation { ref_, score, .. } => {
                citations.push(CitationOut {
                    ref_: ref_.clone(),
                    score: *score,
                });
            }
            AgentEvent::RunFinished { finish: f, .. } => {
                finish = finish_reason_str(*f);
            }
            _ => {}
        }
    }
    // Surface citations inline too, so a plain text client still sees them.
    if !citations.is_empty() {
        content.push_str("\n\nSources: ");
        content.push_str(
            &citations
                .iter()
                .map(|c| c.ref_.as_str())
                .collect::<Vec<_>>()
                .join("; "),
        );
    }

    ChatResponse {
        id: format!("chatcmpl-{}", Uuid::new_v4()),
        object: "chat.completion",
        model: MODEL_NAME,
        choices: vec![Choice {
            index: 0,
            message: OutMessage {
                role: "assistant",
                content,
            },
            finish_reason: finish,
        }],
        metadata: ResponseMetadata { citations },
    }
}

/// Project the events into an SSE stream of OpenAI `chat.completion.chunk`s, terminated by
/// the `[DONE]` sentinel the OpenAI client protocol expects.
fn stream_response(events: Vec<AgentEvent>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let id = format!("chatcmpl-{}", Uuid::new_v4());
    let mut chunks: Vec<Event> = Vec::new();

    for ev in &events {
        match ev {
            AgentEvent::TextMessageDelta { delta, .. } => {
                chunks.push(delta_chunk(&id, json!({ "content": delta }), None));
            }
            AgentEvent::Refusal { reason } => {
                chunks.push(delta_chunk(
                    &id,
                    json!({ "content": refusal_text(reason) }),
                    None,
                ));
            }
            AgentEvent::Citation { ref_, score, .. } => {
                // Citations ride as a namespaced metadata-only chunk so OpenAI clients that
                // ignore unknown delta fields still work.
                chunks.push(delta_chunk(
                    &id,
                    json!({}),
                    Some(json!({ "x-jesus-twin-citation": { "ref": ref_, "score": score } })),
                ));
            }
            AgentEvent::RunFinished { finish, .. } => {
                chunks.push(finish_chunk(&id, finish_reason_str(*finish)));
            }
            _ => {}
        }
    }

    let body = stream::iter(chunks)
        .map(Ok)
        .chain(stream::once(async { Ok(Event::default().data("[DONE]")) }));
    Sse::new(body).keep_alive(KeepAlive::default())
}

/// One `chat.completion.chunk` SSE event carrying a `delta` (and optional extra fields).
fn delta_chunk(id: &str, delta: serde_json::Value, extra: Option<serde_json::Value>) -> Event {
    let mut choice = json!({ "index": 0, "delta": delta, "finish_reason": null });
    if let Some(extra) = extra {
        choice["delta"]
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().cloned().unwrap_or_default());
    }
    Event::default().data(
        json!({
            "id": id, "object": "chat.completion.chunk", "model": MODEL_NAME,
            "choices": [choice],
        })
        .to_string(),
    )
}

/// The terminal chunk that carries `finish_reason`.
fn finish_chunk(id: &str, finish: &str) -> Event {
    Event::default().data(
        json!({
            "id": id, "object": "chat.completion.chunk", "model": MODEL_NAME,
            "choices": [{ "index": 0, "delta": {}, "finish_reason": finish }],
        })
        .to_string(),
    )
}

// ---- helpers ---------------------------------------------------------------------------

fn finish_reason_str(f: FinishReason) -> &'static str {
    match f {
        FinishReason::Stop => "stop",
        FinishReason::Length => "length",
        FinishReason::ToolCalls => "tool_calls",
        FinishReason::Refusal => "content_filter",
        FinishReason::Error => "stop",
    }
}

fn refusal_text(reason: &RefusalReason) -> String {
    match reason {
        RefusalReason::NoCoverage => {
            "I can't speak to that from what's recorded. Let me show you what I did say \
            that might help."
                .to_string()
        }
        RefusalReason::OutOfScope => {
            "The writings about me from later generations speak to that — but the record \
            of my own words and life doesn't go there directly. Here is what I did teach \
            that bears on it."
                .to_string()
        }
        RefusalReason::InsufficientAttestation => {
            "The record doesn't show me addressing that clearly enough that I can answer \
            in my own voice. Here's the closest thread I do have."
                .to_string()
        }
    }
}
