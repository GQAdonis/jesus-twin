//! Retrieval types: a [`Passage`] is one grounded saying; a [`RetrievalSet`] is the fused
//! result the coverage gate scores.
//!
//! Mirrors the RAG record shape in `build/rag_corpus.jsonl` (training_data_spec.md §3).
//! The hybrid query (vector + full-text fused with `search::rrf`, then graph-expanded by
//! `uses_move` / `parallels`) lands here when SurrealDB is wired — ARCHITECTURE.md §7.

use serde::{Deserialize, Serialize};
use surrealdb::types::SurrealValue;

/// One retrievable saying. `ref_` must survive to the cited answer.
///
/// `SurrealValue` is for the DB round-trip (`.take()`); serde is for serializing the
/// passage out to protocol adapters as JSON. The two rename attributes are kept in lockstep
/// so the same field maps to `ref` / `move` in both directions.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Passage {
    pub id: String,
    #[serde(rename = "ref")]
    #[surreal(rename = "ref")]
    pub ref_: String,
    pub book_author: String,
    pub text_original: String,
    #[serde(default)]
    pub text_modern: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub occasion: String,
    /// Reasoning move id (M01..M18), empty until the row is annotated. The JSONL/SurrealDB
    /// field is `move`; renamed here because `move` is a Rust keyword.
    #[serde(rename = "move", default)]
    #[surreal(rename = "move")]
    pub move_: String,
    #[serde(default)]
    pub translation: String,
    /// Fused retrieval score, populated by the query. `None` for stored records.
    #[serde(default)]
    pub score: Option<f32>,
}

/// One line of `build/rag_corpus.jsonl` (training_data_spec.md §3) — the ingest shape.
///
/// Distinct from [`Passage`] (a retrieval result) and from the stored `saying` row: the
/// corpus file carries no embeddings or score, so parsing it into a dedicated type keeps
/// the "what's on disk" contract explicit and avoids smuggling empty `score`/`emb` fields.
#[derive(Debug, Clone, Deserialize)]
pub struct RagRecord {
    pub id: String,
    #[serde(rename = "ref")]
    pub ref_: String,
    #[serde(default)]
    pub book_author: String,
    pub text_original: String,
    #[serde(default)]
    pub text_modern: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub location: String,
    #[serde(default)]
    pub occasion: String,
    #[serde(rename = "move", default)]
    pub move_: String,
    #[serde(default)]
    pub translation: String,
}

/// The fused, ranked result of one retrieval. The coverage gate reads `top_score`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalSet {
    pub passages: Vec<Passage>,
}

impl RetrievalSet {
    /// Best fused score in the set, or `0.0` when empty (treated as no coverage).
    pub fn top_score(&self) -> f32 {
        self.passages.first().and_then(|p| p.score).unwrap_or(0.0)
    }
}
