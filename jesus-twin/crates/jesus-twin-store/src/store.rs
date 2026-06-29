//! The [`Store`] trait — the swap point between embedded and remote SurrealDB.

use async_trait::async_trait;
use thiserror::Error;

use crate::mindmap::MindmapDelta;
use crate::retrieve::{Memory, NarrativePassage, Passage, RetrievalSet, SourcePassage};

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

    // --- The two SEPARATE, LABELED corpora — distinct types so an adapter can never render them
    // as the twin's own words. Default empty so non-SurrealDB stores / test doubles opt in. ---

    /// Retrieve Tanakh verses — HIS SOURCE MATERIAL, never his words (hebrew-bible). Callers MUST
    /// label results as "what he drew on", never as the twin's teaching. Default: empty.
    async fn retrieve_tanakh(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<SourcePassage>, StoreError> {
        Ok(Vec::new())
    }

    /// Retrieve Gospel-narrative passages — HIS DEEDS / CONTEXT, never his words
    /// (gospel-context-kb). Carries the attestation flag; callers label these "what the record
    /// shows he did", never as his teaching. Default: empty.
    async fn retrieve_gospel_narrative(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<NarrativePassage>, StoreError> {
        Ok(Vec::new())
    }

    // --- episodic-memory (the fourth surface) — facts about the USER, never about Jesus. ---
    // Default no-ops so non-SurrealDB stores and test doubles opt in explicitly. These are the
    // ONLY way to touch the `memory` table; `retrieve` never returns memories (isolation).

    /// Record an episodic memory. `scope` keys the relationship (user id, else session id).
    /// Returns the new memory id (empty for a no-op store).
    async fn record_memory(
        &self,
        _scope: &str,
        _kind: &str,
        _text: &str,
        _importance: i64,
        _refs: &[String],
    ) -> Result<String, StoreError> {
        Ok(String::new())
    }

    /// The most salient memories for `scope` (importance, then recency), up to `limit`. Memories
    /// NEVER cross scopes and NEVER appear in [`Store::retrieve`].
    async fn retrieve_memories(
        &self,
        _scope: &str,
        _limit: usize,
    ) -> Result<Vec<Memory>, StoreError> {
        Ok(Vec::new())
    }

    /// All memories for a `scope`, newest first — backs the human inspect/export control.
    async fn list_memories(&self, _scope: &str) -> Result<Vec<Memory>, StoreError> {
        Ok(Vec::new())
    }

    /// Delete one memory by id (human override; CLAUDE.md principle 15).
    async fn delete_memory(&self, _id: &str) -> Result<(), StoreError> {
        Ok(())
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

    // The remaining methods carry meaningful default impls (empty / no-op), so without explicit
    // forwarding `Arc<SurrealStore>` would silently use those defaults instead of the inner store.
    // The served orchestrator holds the store behind an `Arc`, so these MUST forward.
    async fn retrieve_tanakh(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SourcePassage>, StoreError> {
        (**self).retrieve_tanakh(query, limit).await
    }
    async fn retrieve_gospel_narrative(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<NarrativePassage>, StoreError> {
        (**self).retrieve_gospel_narrative(query, limit).await
    }
    async fn record_memory(
        &self,
        scope: &str,
        kind: &str,
        text: &str,
        importance: i64,
        refs: &[String],
    ) -> Result<String, StoreError> {
        (**self)
            .record_memory(scope, kind, text, importance, refs)
            .await
    }
    async fn retrieve_memories(
        &self,
        scope: &str,
        limit: usize,
    ) -> Result<Vec<Memory>, StoreError> {
        (**self).retrieve_memories(scope, limit).await
    }
    async fn list_memories(&self, scope: &str) -> Result<Vec<Memory>, StoreError> {
        (**self).list_memories(scope).await
    }
    async fn delete_memory(&self, id: &str) -> Result<(), StoreError> {
        (**self).delete_memory(id).await
    }
}
