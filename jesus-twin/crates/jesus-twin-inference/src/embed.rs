//! The [`Embedder`] trait — query/corpus vectorization via the same runtime as generation.
//!
//! Feeds the HNSW indexes in `jesus-twin-store` (output dimension must match
//! `schema::EMBEDDING_DIM`). Embedding Gemma / Qwen3-Embedding loaded through the same
//! mistral.rs runtime — no separate embedding service (ARCHITECTURE.md §5).

use async_trait::async_trait;

use crate::engine::EngineError;

/// Produces embedding vectors for text.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Embed a batch of texts. The returned vectors' length must equal the store's
    /// configured embedding dimension.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EngineError>;
}
