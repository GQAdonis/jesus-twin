//! Conversation state: a [`Session`] is an ordered list of [`Turn`]s.
//!
//! Kept deliberately minimal at scaffold time. Persistence and pruning policy land when
//! the orchestrator is implemented (CLAUDE.md principle 13: no hidden business state —
//! conversation state is explicit here, not buried in an adapter).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::event::Role;

/// A single message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: Role,
    pub content: String,
}

impl Turn {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }
}

/// A conversation: a stable id plus the turns so far.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    /// Stable identity of the person across conversations (episodic-memory). `None` for an
    /// anonymous session; then memory is scoped to this single conversation ([`Session::id`]).
    #[serde(default)]
    pub user_id: Option<Uuid>,
    pub turns: Vec<Turn>,
}

impl Session {
    /// Start a fresh session with a random id (anonymous — no cross-session memory).
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            user_id: None,
            turns: Vec::new(),
        }
    }

    /// Attach a stable user identity so episodic memory persists across this person's sessions.
    pub fn with_user(mut self, user_id: Uuid) -> Self {
        self.user_id = Some(user_id);
        self
    }

    /// The episodic-memory scope key: the user id if known, else this conversation's id. Every
    /// memory read/write is filtered to this key — memories never cross relationships.
    pub fn memory_scope(&self) -> String {
        self.user_id.unwrap_or(self.id).to_string()
    }

    /// Append a turn, returning a new `Session` (immutability — CLAUDE.md coding style).
    pub fn with_turn(mut self, turn: Turn) -> Self {
        self.turns.push(turn);
        self
    }
}
