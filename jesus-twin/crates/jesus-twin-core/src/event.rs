//! The canonical agent event stream.
//!
//! `jesus-twin-core` emits exactly one stream type. It is a *superset* modeled on AG-UI's
//! event vocabulary, because A2A task updates and OpenAI chunks both project from it
//! cleanly (ARCHITECTURE.md §4). Every protocol adapter is a translation of this stream —
//! never its own agent loop.

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use uuid::Uuid;

/// Message author role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// Why a run finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    Refusal,
    Error,
}

/// Why the coverage gate refused (ARCHITECTURE.md §7; the historically-humble stance).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalReason {
    /// No retrieved passage cleared the coverage threshold — out-of-corpus question.
    NoCoverage,
    /// The question asks for interpretation beyond the man's recorded words/deeds.
    OutOfScope,
    /// Source coverage exists but is too weakly attested to answer with confidence.
    InsufficientAttestation,
}

/// A span into a source passage, as `(start, end)` byte offsets.
pub type Span = (usize, usize);

/// The one event type every surface reshapes. Modeled on AG-UI; OpenAI/MCP/A2A project
/// from it (see the adapter mapping table in ARCHITECTURE.md §4).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentEvent {
    RunStarted {
        run_id: Uuid,
        session_id: Uuid,
    },
    TextMessageStart {
        message_id: Uuid,
        role: Role,
    },
    TextMessageDelta {
        message_id: Uuid,
        delta: String,
    },
    TextMessageEnd {
        message_id: Uuid,
    },
    ToolCallStart {
        tool_call_id: Uuid,
        name: String,
    },
    ToolCallArgsDelta {
        tool_call_id: Uuid,
        delta: String,
    },
    ToolCallEnd {
        tool_call_id: Uuid,
    },
    ToolResult {
        tool_call_id: Uuid,
        content: Json,
    },
    /// Every grounded claim traces to a verse the user can verify.
    Citation {
        #[serde(rename = "ref")]
        ref_: String,
        score: f32,
        span: Option<Span>,
    },
    /// Graph/mindmap context and the retrieval set (drives the `STATE_SNAPSHOT` chunk).
    StateSnapshot {
        state: Json,
    },
    /// The coverage gate fired before generation.
    Refusal {
        reason: RefusalReason,
    },
    /// An additive, namespaced custom chunk (e.g. `x-jesus-twin/low-confidence`). Standard
    /// clients ignore `type`s they don't recognize; adapters project it verbatim (CLAUDE.md —
    /// custom AG-UI chunks must be additive + namespaced). Emitted on a Tier-2 (low-confidence)
    /// turn so the honesty surface can flag single-leg grounding.
    Custom {
        name: String,
        data: Json,
    },
    RunFinished {
        run_id: Uuid,
        finish: FinishReason,
    },
    Error {
        code: String,
        message: String,
    },
}
