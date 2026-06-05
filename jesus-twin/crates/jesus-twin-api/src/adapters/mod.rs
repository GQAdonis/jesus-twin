//! Protocol adapters. Each is a thin translation of the core `AgentEvent` stream into a
//! wire format — see the adapter mapping table in ARCHITECTURE.md §4. No agent loops here.

pub mod a2a;
pub mod agui;
pub mod common;
pub mod mcp;
pub mod openai;
