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
    pub turns: Vec<Turn>,
}

impl Session {
    /// Start a fresh session with a random id.
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            turns: Vec::new(),
        }
    }

    /// Append a turn, returning a new `Session` (immutability — CLAUDE.md coding style).
    pub fn with_turn(mut self, turn: Turn) -> Self {
        self.turns.push(turn);
        self
    }
}
