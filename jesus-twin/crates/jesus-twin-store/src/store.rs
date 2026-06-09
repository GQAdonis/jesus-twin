//! The [`Store`] trait — the swap point between embedded and remote SurrealDB.

use async_trait::async_trait;
use thiserror::Error;

use crate::mindmap::MindmapDelta;
use crate::retrieve::{Passage, RetrievalSet};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("database error: {0}")]
    Db(#[from] surrealdb::Error),
    #[error("reading corpus {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed corpus record on line {line}: {source}")]
    Parse {
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("embedding failed: {0}")]
    Embedding(String),
    #[error("not yet implemented: {0}")]
    NotImplemented(&'static str),
}

/// Data access for the twin. Implementations: embedded SurrealDB (in-process, SurrealKV)
/// now; a remote node (`ws://`) later — same trait (ARCHITECTURE.md §9).
#[async_trait]
pub trait Store: Send + Sync {
    /// Hybrid retrieval: vector + BM25 fused with RRF, then graph-expanded. `query` is the
    /// user's question in either register (ancient or modern); both embedding columns are
    /// searched (training_data_spec.md §3).
    async fn retrieve(&self, query: &str, limit: usize) -> Result<RetrievalSet, StoreError>;

    /// Load the RAG corpus (`build/rag_corpus.jsonl`) and build indexes + the
    /// move/parallels graph. This is build-sequence step 1.
    async fn ingest_corpus(&self, jsonl_path: &str) -> Result<usize, StoreError>;

    /// Fetch a single saying by its exact scripture `ref` (e.g. "Mark 12:17"), or `None`.
    /// Backs the `lookup_saying` skill — exact citation, not fuzzy retrieval.
    async fn get_by_ref(&self, scripture_ref: &str) -> Result<Option<Passage>, StoreError>;

    /// Fetch sayings tagged with a reasoning move (e.g. "M02"), up to `limit`. Backs the
    /// `find_by_move` skill. Returns empty until the corpus is annotated with moves.
    async fn find_by_move(&self, move_id: &str, limit: usize) -> Result<Vec<Passage>, StoreError>;

    /// Project a mind-map around `topic`: the sayings that match it, their reasoning-move nodes
    /// (`uses_move` edges), and `parallels` edges between sayings that share a move. Backs the
    /// `mindmap` skill (ARCHITECTURE.md §7/§8). Graph richness grows as the corpus is annotated.
    ///
    /// Default: retrieve, then project in Rust ([`crate::mindmap::project_topic`]) — works for
    /// any `Store` impl. Override only if a backend can project the graph more directly.
    async fn mindmap(&self, topic: &str, limit: usize) -> Result<MindmapDelta, StoreError> {
        let set = self.retrieve(topic, limit).await?;
        Ok(crate::mindmap::project_topic(topic, &set.passages))
    }
}

/// `Arc<S>` is itself a `Store`, so a single store handle can be shared (e.g. between the
/// orchestrator and a `SkillCtx`) without cloning the backend.
#[async_trait]
impl<S: Store + ?Sized> Store for std::sync::Arc<S> {
    async fn retrieve(&self, query: &str, limit: usize) -> Result<RetrievalSet, StoreError> {
        (**self).retrieve(query, limit).await
    }
    async fn ingest_corpus(&self, jsonl_path: &str) -> Result<usize, StoreError> {
        (**self).ingest_corpus(jsonl_path).await
    }
    async fn get_by_ref(&self, scripture_ref: &str) -> Result<Option<Passage>, StoreError> {
        (**self).get_by_ref(scripture_ref).await
    }
    async fn find_by_move(&self, move_id: &str, limit: usize) -> Result<Vec<Passage>, StoreError> {
        (**self).find_by_move(move_id, limit).await
    }
    // `mindmap` uses the trait default (retrieve + project) — it delegates through `retrieve`.
}
