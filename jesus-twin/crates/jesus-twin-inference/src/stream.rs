//! Maps mistral.rs token/tool-call output into [`GenChunk`]s.
//!
//! `jesus-twin-core` is responsible for turning these into `AgentEvent`s; this module's job
//! is only to normalize the engine's native stream into the crate-local `GenChunk` shape.
//! Stub until the runtime is wired (ARCHITECTURE.md §5).

use crate::engine::GenChunk;

/// Placeholder for the streaming adapter. The real implementation consumes the mistral.rs
/// response stream and yields `GenChunk`s; for now it returns an empty terminal stream.
pub fn empty_stream() -> Vec<GenChunk> {
    vec![GenChunk::Done]
}
