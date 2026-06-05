//! SurrealQL schema for the graphrag store (ARCHITECTURE.md §7).
//!
//! Held as a string constant at scaffold time so the shape is reviewable before the
//! SurrealDB dependency is wired. Applied at store init once the backend lands.
//!
//! Graph edges (via `RELATE`): saying ->uses_move-> reasoning_move,
//! ->spoken_to-> audience, ->at-> location, ->mentions-> concept,
//! ->parallels-> saying (synoptic parallels).

/// The schema definition, applied (idempotently) on store initialization.
///
/// `SCHEMAFULL` + every persisted field is declared explicitly, so ingest must not write a
/// field the schema doesn't know. The full-text index `saying_ft` is the only SEARCH index,
/// hence it is index `0` — referenced as `@0@` / `search::score(0)` in queries. The HNSW
/// vector indexes are defined now but the `emb_*` fields stay empty until the embedder
/// lands. `IF NOT EXISTS` everywhere makes re-opening an existing store a no-op.
pub const SCHEMA: &str = r#"
DEFINE TABLE IF NOT EXISTS saying SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS ref           ON saying TYPE string;
DEFINE FIELD IF NOT EXISTS book_author   ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS text_original ON saying TYPE string;
DEFINE FIELD IF NOT EXISTS text_modern   ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS context       ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS location      ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS occasion      ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS `move`        ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS translation   ON saying TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS emb_original  ON saying TYPE option<array<float>>;
DEFINE FIELD IF NOT EXISTS emb_modern    ON saying TYPE option<array<float>>;

DEFINE INDEX IF NOT EXISTS saying_vec_orig ON saying FIELDS emb_original HNSW DIMENSION 768 DIST COSINE;
DEFINE INDEX IF NOT EXISTS saying_vec_mod  ON saying FIELDS emb_modern   HNSW DIMENSION 768 DIST COSINE;
DEFINE ANALYZER IF NOT EXISTS twin_an TOKENIZERS blank,class FILTERS lowercase,snowball(english);
-- `FULLTEXT` (3.0.0-beta+) replaced the older `SEARCH` keyword. A FULLTEXT index covers
-- exactly one column, so the two text registers get one index each: index 0 = original,
-- index 1 = modern. Queries reference them as `@0@` / `@1@` and `search::score(0/1)`.
DEFINE INDEX IF NOT EXISTS saying_ft_orig ON saying FIELDS text_original FULLTEXT ANALYZER twin_an BM25;
DEFINE INDEX IF NOT EXISTS saying_ft_mod  ON saying FIELDS text_modern  FULLTEXT ANALYZER twin_an BM25;

DEFINE TABLE IF NOT EXISTS reasoning_move SCHEMALESS;
DEFINE TABLE IF NOT EXISTS audience SCHEMALESS;
DEFINE TABLE IF NOT EXISTS location SCHEMALESS;
DEFINE TABLE IF NOT EXISTS concept SCHEMALESS;
"#;

/// Embedding dimension for the HNSW indexes. Must match the embedding model wired in
/// `jesus-twin-inference` (Embedding Gemma / Qwen3-Embedding).
pub const EMBEDDING_DIM: usize = 768;
