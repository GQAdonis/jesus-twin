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
-- `machine_draft` flags rows whose `text_modern` is a doc2query-style MACHINE draft
-- (`modern-legs-v1`), present for RETRIEVAL INDEXING ONLY. Such text is never displayed
-- (`context_lines` uses `text_original`) and never trained (SFT reads the human xlsx, not the
-- sidecar). Human-verified renderings (the `Modern Rendering` column) set this back to false.
DEFINE FIELD IF NOT EXISTS machine_draft  ON saying TYPE bool DEFAULT false;
-- principle-index-v1 facets: life-domain tags + the governing principles a saying establishes.
-- Machine-tagged (`machine_tagged = true`) retrieval metadata ONLY — they steer retrieval and
-- feed Tier-2 principle-bridging, are never displayed (`context_lines` uses `text_original`) or
-- trained (SFT reads the human xlsx). Human review later promotes them (`machine_tagged = false`).
DEFINE FIELD IF NOT EXISTS domains        ON saying TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS principles     ON saying TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS machine_tagged ON saying TYPE bool DEFAULT false;
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

-- `tanakh` is a SEPARATE corpus (hebrew-bible): the Hebrew Bible (JPS 1917, public domain) as
-- HIS SOURCE MATERIAL — what he quoted and reasoned from — never blended with the red-letter
-- `saying` table and always labeled as source, not his words (CLAUDE.md Bible scope). One text
-- register, so one BM25 + one HNSW index (vs. saying's original/modern pair).
DEFINE TABLE IF NOT EXISTS tanakh SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS ref         ON tanakh TYPE string;
DEFINE FIELD IF NOT EXISTS text        ON tanakh TYPE string;
DEFINE FIELD IF NOT EXISTS book        ON tanakh TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS category    ON tanakh TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS translation ON tanakh TYPE string DEFAULT "JPS 1917";
DEFINE FIELD IF NOT EXISTS emb         ON tanakh TYPE option<array<float>>;
DEFINE INDEX IF NOT EXISTS tanakh_ft  ON tanakh FIELDS text FULLTEXT ANALYZER twin_an BM25;
DEFINE INDEX IF NOT EXISTS tanakh_vec ON tanakh FIELDS emb  HNSW DIMENSION 768 DIST COSINE;

-- `gospel_narrative` is a THIRD labeled corpus (gospel-context-kb): the NON-red-letter Gospel
-- narrative (his deeds, settings, the dialogue around the sayings) — "what the record shows he
-- did," never his words. Attestation-flagged (`attestation` single|multiply, `witnesses`); the
-- automated multiply-vs-single computation needs synoptic-parallel data (a documented follow-up),
-- so it defaults to single for now. Same one-register BM25 + HNSW shape as `tanakh`.
DEFINE TABLE IF NOT EXISTS gospel_narrative SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS ref         ON gospel_narrative TYPE string;
DEFINE FIELD IF NOT EXISTS text        ON gospel_narrative TYPE string;
DEFINE FIELD IF NOT EXISTS book        ON gospel_narrative TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS attestation ON gospel_narrative TYPE string DEFAULT "single";
DEFINE FIELD IF NOT EXISTS witnesses   ON gospel_narrative TYPE array<string> DEFAULT [];
DEFINE FIELD IF NOT EXISTS emb         ON gospel_narrative TYPE option<array<float>>;
DEFINE INDEX IF NOT EXISTS gospel_ft  ON gospel_narrative FIELDS text FULLTEXT ANALYZER twin_an BM25;
DEFINE INDEX IF NOT EXISTS gospel_vec ON gospel_narrative FIELDS emb  HNSW DIMENSION 768 DIST COSINE;

-- `memory` is the FOURTH surface (episodic-memory): facts about the USER and the relationship,
-- never about Jesus. A separate table makes it structurally impossible for corpus retrieval to
-- return a memory as if it were scripture. `scope` keys one relationship (user id, else session id)
-- and every query filters on it — memories never cross relationships. `at` is an ISO string stamped
-- by the store (`<string> time::now()`), so lexical ordering is chronological.
DEFINE TABLE IF NOT EXISTS memory SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS scope      ON memory TYPE string;
DEFINE FIELD IF NOT EXISTS kind       ON memory TYPE string DEFAULT "observation";
DEFINE FIELD IF NOT EXISTS text       ON memory TYPE string;
DEFINE FIELD IF NOT EXISTS importance ON memory TYPE int DEFAULT 5;
DEFINE FIELD IF NOT EXISTS at         ON memory TYPE string DEFAULT "";
DEFINE FIELD IF NOT EXISTS refs       ON memory TYPE array<string> DEFAULT [];
DEFINE INDEX IF NOT EXISTS memory_ft ON memory FIELDS text FULLTEXT ANALYZER twin_an BM25;
"#;

/// Embedding dimension for the HNSW indexes. Must match the embedding model wired in
/// `jesus-twin-inference` (Embedding Gemma / Qwen3-Embedding).
pub const EMBEDDING_DIM: usize = 768;
