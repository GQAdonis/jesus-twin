//! Shared test harness for the adapter integration tests: a canned `Agent` double, router
//! builders, and HTTP helpers. Used by openai.rs / agui.rs / a2a.rs.

#![allow(dead_code)] // each test file uses a subset of these helpers

use std::sync::Arc;

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

use jesus_twin_api::{AppState, router};
use jesus_twin_core::event::{AgentEvent, FinishReason, RefusalReason, Role};
use jesus_twin_core::{Agent, AgentError, AgentErrorKind, Session};

/// A canned agent: a grounded-and-cited stream for a normal query; a refusal when the last
/// user message contains "crypto".
pub struct FakeAgent;

#[async_trait]
impl Agent for FakeAgent {
    async fn run(&self, session: &Session) -> Result<Vec<AgentEvent>, AgentError> {
        let run_id = Uuid::nil();
        let refuse = session
            .turns
            .iter()
            .rev()
            .find(|t| t.role == Role::User)
            .is_some_and(|t| t.content.contains("crypto"));

        if refuse {
            return Ok(vec![
                AgentEvent::RunStarted {
                    run_id,
                    session_id: session.id,
                },
                AgentEvent::Refusal {
                    reason: RefusalReason::NoCoverage,
                },
                AgentEvent::RunFinished {
                    run_id,
                    finish: FinishReason::Refusal,
                },
            ]);
        }
        let mid = Uuid::nil();
        Ok(vec![
            AgentEvent::RunStarted {
                run_id,
                session_id: session.id,
            },
            AgentEvent::Citation {
                ref_: "Mark 12:17".into(),
                score: 8.8,
                span: None,
            },
            AgentEvent::TextMessageStart {
                message_id: mid,
                role: Role::Assistant,
            },
            AgentEvent::TextMessageDelta {
                message_id: mid,
                delta: "Give Caesar his coin.".into(),
            },
            AgentEvent::TextMessageEnd { message_id: mid },
            AgentEvent::RunFinished {
                run_id,
                finish: FinishReason::Stop,
            },
        ])
    }
}

/// An agent that always rejects with an `Overloaded` error (admission backpressure -> 503).
pub struct OverloadedAgent;

#[async_trait]
impl Agent for OverloadedAgent {
    async fn run(&self, _session: &Session) -> Result<Vec<AgentEvent>, AgentError> {
        Err(AgentError::new(
            AgentErrorKind::Overloaded,
            "system at capacity",
        ))
    }
}

pub fn app() -> Router {
    router(AppState::new(Arc::new(FakeAgent)))
}

pub fn app_with(agent: impl Agent + 'static) -> Router {
    router(AppState::new(Arc::new(agent)))
}

/// POST `body` as JSON to `uri` on the default (FakeAgent) app.
pub async fn post_json(uri: &str, body: Value) -> (StatusCode, String) {
    post_json_to(app(), uri, body).await
}

/// GET `uri` on the default app.
pub async fn get(uri: &str) -> (StatusCode, String) {
    let resp = app()
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    collect(resp).await
}

pub async fn post_json_to(router: Router, uri: &str, body: Value) -> (StatusCode, String) {
    let resp = router
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    collect(resp).await
}

async fn collect(resp: axum::response::Response) -> (StatusCode, String) {
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}
