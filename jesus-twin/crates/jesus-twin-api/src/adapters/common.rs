//! Helpers shared across protocol adapters: the common wire message shape, session
//! construction, and the agent-error -> HTTP status mapping. Keeps each adapter a thin
//! projection without duplicating these concerns (DRY).

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use jesus_twin_core::{AgentErrorKind, Role, Session, Turn};

/// A role/content message, the lowest common denominator across OpenAI / AG-UI / A2A inputs.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WireMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
}

/// Build a core [`Session`] from wire messages. System/tool messages are dropped — the twin
/// owns its fixed system contract; only user/assistant turns carry in. Returns `None` if no
/// user message is present (the adapters map that to 400).
pub fn session_from_messages(messages: &[WireMessage]) -> Option<Session> {
    let mut session = Session::new(Uuid::new_v4());
    let mut saw_user = false;
    for m in messages {
        let role = match m.role.as_str() {
            "user" => {
                saw_user = true;
                Role::User
            }
            "assistant" => Role::Assistant,
            _ => continue,
        };
        session = session.with_turn(Turn::new(role, m.content.clone()));
    }
    saw_user.then_some(session)
}

/// Map an agent error kind to a wire status: admission rejection -> 503 (backpressure),
/// bad input -> 400, else 500.
pub fn status_for(kind: AgentErrorKind) -> StatusCode {
    match kind {
        AgentErrorKind::BadRequest => StatusCode::BAD_REQUEST,
        AgentErrorKind::Overloaded => StatusCode::SERVICE_UNAVAILABLE,
        AgentErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
