//! The [`Engine`] trait — text generation, abstracted over the mistral.rs runtime.
//!
//! Trait-bounded so the core can be tested with a double and so the real
//! `MultimodalModelBuilder`-backed impl (built from `google/gemma-4-E4B-it` or the local
//! merged checkpoint) can be swapped in without touching `jesus-twin-core`.

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("model load failed: {0}")]
    Load(String),
    #[error("generation failed: {0}")]
    Generate(String),
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// A unit of streamed generation. Tool-call deltas are carried separately so `stream.rs`
/// can map them to `ToolCall*` events (Gemma 4's `gemma4`/`gemma4_strict` parser output).
#[derive(Debug, Clone)]
pub enum GenChunk {
    Text(String),
    ToolCallName(String),
    ToolCallArgsDelta(String),
    Done,
}

/// Prompt + conditioning context for one generation request.
#[derive(Debug, Clone)]
pub struct GenRequest {
    pub system: String,
    pub context: String,
    pub user: String,
}

/// Text generation over the merged Gemma 4 checkpoint.
#[async_trait]
pub trait Engine: Send + Sync {
    /// Generate a full completion. Streaming variant lands in `stream.rs` once the runtime
    /// is wired; this non-streaming form is enough for adapters to build against.
    async fn generate(&self, req: GenRequest) -> Result<String, EngineError>;
}
