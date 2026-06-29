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
    /// Life-domain tags (principle-index-v1) — machine-tagged retrieval facets (e.g.
    /// `["fear/anxiety","provision"]`). NEVER displayed or trained: they steer retrieval and feed
    /// Tier-2 principle-bridging. `context_lines` uses only `text_original`.
    #[serde(default)]
    pub domains: Vec<String>,
    /// Governing principles this saying establishes — short statements, each derived from the
    /// saying itself (never invented). Same facet rules as `domains`. Used by `principle-tier` to
    /// bridge an adjacent question to the principle the cited passages establish.
    #[serde(default)]
    pub principles: Vec<String>,
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

/// The fused, ranked result of one retrieval. The coverage gate reads `top_legs_matched`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RetrievalSet {
    pub passages: Vec<Passage>,
    /// How many independent retrieval legs ranked the **top** fused passage — the coverage
    /// gate's tier signal (≥2 = grounded, 1 = low-confidence, 0/empty = no coverage). Gating
    /// on leg agreement rather than raw RRF score is robust to the annotation program reviving
    /// the modern legs (gate-calibration change, `docs/FINDINGS.md`). The BM25-only fallback
    /// (no embedder) caps this at 1 by definition — a single retrieval modality ran.
    #[serde(default)]
    pub top_legs_matched: u8,
}

impl RetrievalSet {
    /// Best fused score in the set, or `0.0` when empty. Retained for diagnostics/tests; the
    /// gate keys on [`RetrievalSet::top_legs_matched`], not this scalar.
    pub fn top_score(&self) -> f32 {
        self.passages.first().and_then(|p| p.score).unwrap_or(0.0)
    }
}

/// One episodic memory — a fact about the **user and the relationship**, never about Jesus
/// (episodic-memory; pre-planning 03). A distinct type and a distinct `memory` table, so corpus
/// retrieval can never surface a memory as if it were scripture, and a memory can never be phrased
/// as the mentor's belief about the world. `scope` isolates one relationship's memories.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct Memory {
    pub id: String,
    /// `observation` | `reflection` | `preference`.
    #[serde(default)]
    pub kind: String,
    /// The relationship key (user id, else session id) — every query is scoped to it.
    #[serde(default)]
    pub scope: String,
    pub text: String,
    #[serde(default)]
    pub importance: i64,
    /// ISO-8601 timestamp the store stamped at record time.
    #[serde(default)]
    pub at: String,
    /// Citations from the reply this observation came from (provenance, not doctrine).
    #[serde(default)]
    pub refs: Vec<String>,
}

/// One retrieved Tanakh verse — **his source material, NOT his words** (hebrew-bible). A distinct
/// type from [`Passage`] so the two corpora can never be conflated: adapters label these "source
/// material" / "what he drew on", never as the twin's own teaching.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct SourcePassage {
    #[serde(rename = "ref")]
    #[surreal(rename = "ref")]
    pub ref_: String,
    pub text: String,
    #[serde(default)]
    pub book: String,
    /// `torah` | `prophets` | `writings`.
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub translation: String,
    /// Fused retrieval score, populated by the query. `None` for stored records.
    #[serde(default)]
    pub score: Option<f32>,
}

/// One retrieved Gospel-narrative passage — **what the record shows he DID, not his words**
/// (gospel-context-kb). A distinct type so "example by deed" can never be rendered as his teaching.
/// `attestation` (`single`|`multiply`) + `witnesses` flag how broadly the record attests it.
#[derive(Debug, Clone, Serialize, Deserialize, SurrealValue)]
pub struct NarrativePassage {
    #[serde(rename = "ref")]
    #[surreal(rename = "ref")]
    pub ref_: String,
    pub text: String,
    #[serde(default)]
    pub book: String,
    /// `single` | `multiply` — how many Gospels attest it (mechanical attestation is a follow-up;
    /// defaults to `single`).
    #[serde(default)]
    pub attestation: String,
    #[serde(default)]
    pub witnesses: Vec<String>,
    /// Fused retrieval score, populated by the query. `None` for stored records.
    #[serde(default)]
    pub score: Option<f32>,
}
