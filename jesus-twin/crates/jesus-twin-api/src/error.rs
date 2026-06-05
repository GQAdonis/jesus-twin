//! Shared error response shaping for the adapters.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// An OpenAI-style error envelope at `status`. Used by every adapter for uniform errors.
pub fn error_json(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({ "error": { "message": message, "type": "request_error" } })),
    )
        .into_response()
}
