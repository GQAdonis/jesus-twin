//! The store-local [`Embed`] trait.
//!
//! The store needs to vectorize passages (at ingest) and queries (at retrieve) to drive the
//! HNSW vector leg, but it must NOT depend on `jesus-twin-inference` (its sibling — that would
//! couple two leaf crates). So the store declares its own minimal embedding interface here;
//! the wiring layer adapts the real `jesus_twin_inference::Embedder` to it.
//!
//! The embedder is **optional**: without one, the store runs BM25-only (the graceful default).
//! With one, retrieval fuses vector + BM25 via `search::rrf` (ARCHITECTURE.md §7).

use async_trait::async_trait;

use crate::schema::EMBEDDING_DIM;
use crate::store::StoreError;

/// Produces embedding vectors for text. Output length must equal [`EMBEDDING_DIM`] or the
/// HNSW index rejects the vectors.
#[async_trait]
pub trait Embed: Send + Sync {
    /// Embed a batch of texts, one vector per input in order.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, StoreError>;

    /// Embed a single query (convenience over [`Embed::embed`]).
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, StoreError> {
        let mut v = self.embed(std::slice::from_ref(&text.to_string())).await?;
        v.pop()
            .ok_or_else(|| StoreError::Embedding("embedder returned no vector".into()))
    }
}

/// The embedding dimension the store expects (re-exported for adapters).
pub const DIM: usize = EMBEDDING_DIM;
