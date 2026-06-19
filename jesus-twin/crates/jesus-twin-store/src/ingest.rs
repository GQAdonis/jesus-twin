//! Corpus ingestion: `build/rag_corpus.jsonl` -> `saying` rows + the reasoning-move graph.
//!
//! One JSONL line per saying (training_data_spec.md §3). Each becomes a `saying` record
//! keyed by its stable corpus id (`wj-N`), so re-ingesting upserts rather than duplicating.
//! When a row carries a reasoning move (M01..M18) we `RELATE saying ->uses_move-> move`;
//! most rows are blank today (annotation pending — see the annotation-bottleneck note), so
//! that edge is populated opportunistically.
//!
//! Embeddings are computed by [`embed_all`] only when an embedder is attached to the store;
//! otherwise `emb_original` / `emb_modern` stay unset and retrieval runs BM25-only.
//! `parallels` edges are not in the corpus data, so none are created — that graph is populated
//! separately when synoptic-parallel data exists.

use std::io::{BufRead, BufReader};

use serde::Deserialize;
use surrealdb::Surreal;
use surrealdb::engine::local::Db;
use surrealdb::types::SurrealValue;

use crate::embed::Embed;
use crate::retrieve::RagRecord;
use crate::store::StoreError;

/// Read `jsonl_path` and load every record into `db`. Returns the number of sayings
/// ingested. Idempotent: each record is upserted by its `wj-N` id.
pub async fn ingest_corpus(db: &Surreal<Db>, jsonl_path: &str) -> Result<usize, StoreError> {
    let records = read_records(jsonl_path)?;
    let total = records.len();

    for rec in records {
        upsert_saying(db, &rec).await?;
        if !rec.move_.is_empty() {
            relate_move(db, &rec.id, &rec.move_).await?;
        }
    }

    tracing::info!(count = total, path = jsonl_path, "ingested rag corpus");
    Ok(total)
}

/// Parse the JSONL file into records, reporting the offending line on a parse error rather
/// than failing opaquely (CLAUDE.md: validate at boundaries, fail with clear messages).
fn read_records(jsonl_path: &str) -> Result<Vec<RagRecord>, StoreError> {
    let file = std::fs::File::open(jsonl_path).map_err(|source| StoreError::Io {
        path: jsonl_path.to_string(),
        source,
    })?;
    let reader = BufReader::new(file);

    let mut records = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| StoreError::Io {
            path: jsonl_path.to_string(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let rec: RagRecord = serde_json::from_str(&line).map_err(|source| StoreError::Parse {
            line: idx + 1,
            source,
        })?;
        records.push(rec);
    }
    Ok(records)
}

/// Upsert one saying by its stable corpus id (`type::record('saying', $id)`).
async fn upsert_saying(db: &Surreal<Db>, rec: &RagRecord) -> Result<(), StoreError> {
    let sql = "
        UPSERT type::record('saying', $id) CONTENT {
            ref: $ref, book_author: $book_author,
            text_original: $text_original, text_modern: $text_modern,
            context: $context, location: $location, occasion: $occasion,
            move: $move, translation: $translation
        };
    ";
    db.query(sql)
        .bind(("id", rec.id.clone()))
        .bind(("ref", rec.ref_.clone()))
        .bind(("book_author", rec.book_author.clone()))
        .bind(("text_original", rec.text_original.clone()))
        .bind(("text_modern", rec.text_modern.clone()))
        .bind(("context", rec.context.clone()))
        .bind(("location", rec.location.clone()))
        .bind(("occasion", rec.occasion.clone()))
        .bind(("move", rec.move_.clone()))
        .bind(("translation", rec.translation.clone()))
        .await?
        .check()?;
    Ok(())
}

/// Ensure the reasoning_move node exists and link `saying ->uses_move-> move`.
async fn relate_move(db: &Surreal<Db>, saying_id: &str, move_id: &str) -> Result<(), StoreError> {
    let sql = "
        UPSERT type::record('reasoning_move', $move);
        RELATE type::record('saying', $saying)->uses_move->type::record('reasoning_move', $move);
    ";
    db.query(sql)
        .bind(("saying", saying_id.to_string()))
        .bind(("move", move_id.to_string()))
        .await?
        .check()?;
    Ok(())
}

/// The id + text fields needed to embed a saying.
#[derive(Debug, Deserialize, SurrealValue)]
struct EmbedRow {
    id: String,
    text_original: String,
    #[serde(default)]
    text_modern: String,
}

/// Embed every saying's original (and modern, when present) text and write the vectors into
/// `emb_original` / `emb_modern`, populating the HNSW indexes. Idempotent: re-running just
/// overwrites the vectors. Modern text is only embedded when non-empty (annotation pending).
pub async fn embed_all(db: &Surreal<Db>, embedder: &dyn Embed) -> Result<(), StoreError> {
    let mut res = db
        .query("SELECT record::id(id) AS id, text_original, text_modern FROM saying;")
        .await?;
    let rows: Vec<EmbedRow> = res.take(0)?;
    if rows.is_empty() {
        return Ok(());
    }

    // Embed originals in one batch (order preserved), then write each back.
    let originals: Vec<String> = rows.iter().map(|r| r.text_original.clone()).collect();
    let orig_vecs = embedder.embed(&originals).await?;

    for (row, emb_o) in rows.iter().zip(orig_vecs) {
        let emb_m = if row.text_modern.trim().is_empty() {
            None
        } else {
            Some(embedder.embed_query(&row.text_modern).await?)
        };
        db.query("UPDATE type::record('saying', $id) SET emb_original = $eo, emb_modern = $em;")
            .bind(("id", row.id.clone()))
            .bind(("eo", emb_o))
            .bind(("em", emb_m))
            .await?
            .check()?;
    }
    tracing::info!(count = rows.len(), "embedded corpus for vector retrieval");
    Ok(())
}

/// One sidecar line of machine-draft modern text (`modern-legs-v1`).
#[derive(Debug, Deserialize)]
struct ModernDraft {
    id: String,
    text_modern: String,
}

/// Apply machine-draft `text_modern` from a sidecar JSONL (`{id, text_modern}` per line) onto
/// existing `saying` rows, flagging each `machine_draft = true`. **Retrieval indexing only**:
/// this revives the dead modern legs (`text_modern` BM25 + `emb_modern` vector) without touching
/// the corpus file, the xlsx, or anything displayed/trained. Callers re-run [`embed_all`]
/// afterwards to populate `emb_modern`. Idempotent; a human-verified rendering later overwrites
/// the draft and resets the flag. Empty drafts are skipped. Returns the number applied.
pub async fn apply_modern_drafts(db: &Surreal<Db>, jsonl_path: &str) -> Result<usize, StoreError> {
    let file = std::fs::File::open(jsonl_path).map_err(|source| StoreError::Io {
        path: jsonl_path.to_string(),
        source,
    })?;
    let mut applied = 0usize;
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|source| StoreError::Io {
            path: jsonl_path.to_string(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let draft: ModernDraft =
            serde_json::from_str(&line).map_err(|source| StoreError::Parse {
                line: i + 1,
                source,
            })?;
        if draft.text_modern.trim().is_empty() {
            continue;
        }
        db.query("UPDATE type::record('saying', $id) SET text_modern = $tm, machine_draft = true;")
            .bind(("id", draft.id))
            .bind(("tm", draft.text_modern))
            .await?
            .check()?;
        applied += 1;
    }
    tracing::info!(
        count = applied,
        path = jsonl_path,
        "applied machine-draft modern text"
    );
    Ok(applied)
}

/// One line of `build/tanakh.jsonl` (produced by `ingest_tanakh.py`).
#[derive(Debug, Deserialize)]
struct TanakhRecord {
    #[serde(rename = "ref")]
    ref_: String,
    text: String,
    #[serde(default)]
    book: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    translation: String,
}

/// Load the Tanakh JSONL (JPS 1917 verses) into the `tanakh` table — a SEPARATE corpus from
/// `saying` (hebrew-bible: his source material, never his words). Upserts by verse `ref`, so
/// re-ingesting is idempotent. Returns the number of verses loaded.
pub async fn ingest_tanakh(db: &Surreal<Db>, jsonl_path: &str) -> Result<usize, StoreError> {
    let file = std::fs::File::open(jsonl_path).map_err(|source| StoreError::Io {
        path: jsonl_path.to_string(),
        source,
    })?;
    let mut total = 0usize;
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|source| StoreError::Io {
            path: jsonl_path.to_string(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let rec: TanakhRecord =
            serde_json::from_str(&line).map_err(|source| StoreError::Parse {
                line: idx + 1,
                source,
            })?;
        let sql = "UPSERT type::record('tanakh', $ref) CONTENT {
            ref: $ref, text: $text, book: $book, category: $category, translation: $translation
        };";
        db.query(sql)
            .bind(("ref", rec.ref_.clone()))
            .bind(("text", rec.text))
            .bind(("book", rec.book))
            .bind(("category", rec.category))
            .bind((
                "translation",
                if rec.translation.is_empty() {
                    "JPS 1917".to_string()
                } else {
                    rec.translation
                },
            ))
            .await?
            .check()?;
        total += 1;
    }
    tracing::info!(count = total, path = jsonl_path, "ingested tanakh corpus");
    Ok(total)
}

/// The id + text needed to embed a Tanakh verse.
#[derive(Debug, Deserialize, SurrealValue)]
struct TanakhEmbedRow {
    id: String,
    text: String,
}

/// Embed every Tanakh verse's text into `emb`, populating the HNSW index. Idempotent.
pub async fn embed_tanakh(db: &Surreal<Db>, embedder: &dyn Embed) -> Result<(), StoreError> {
    let mut res = db
        .query("SELECT record::id(id) AS id, text FROM tanakh;")
        .await?;
    let rows: Vec<TanakhEmbedRow> = res.take(0)?;
    if rows.is_empty() {
        return Ok(());
    }
    let texts: Vec<String> = rows.iter().map(|r| r.text.clone()).collect();
    let vecs = embedder.embed(&texts).await?;
    for (row, emb) in rows.iter().zip(vecs) {
        db.query("UPDATE type::record('tanakh', $id) SET emb = $emb;")
            .bind(("id", row.id.clone()))
            .bind(("emb", emb))
            .await?
            .check()?;
    }
    tracing::info!(count = rows.len(), "embedded tanakh for vector retrieval");
    Ok(())
}

/// One sidecar line of machine-tagged life-domain facets (`principle-index-v1`).
#[derive(Debug, Deserialize)]
struct PrincipleTag {
    id: String,
    #[serde(default)]
    domains: Vec<String>,
    #[serde(default)]
    principles: Vec<String>,
}

/// Apply machine-tagged `domains` / `principles` facets from a sidecar JSONL
/// (`{id, domains, principles}` per line) onto existing `saying` rows, flagging each
/// `machine_tagged = true`. **Retrieval metadata only** — facets steer retrieval and feed Tier-2
/// principle-bridging; they are never displayed (`context_lines` uses `text_original`) or trained
/// (SFT reads the human xlsx). Idempotent; a wrong tag costs at most a retrieval miss, never
/// fabricated content. Human review later promotes tags (`machine_tagged = false`). Returns the
/// number of rows tagged.
pub async fn apply_principle_tags(db: &Surreal<Db>, jsonl_path: &str) -> Result<usize, StoreError> {
    let file = std::fs::File::open(jsonl_path).map_err(|source| StoreError::Io {
        path: jsonl_path.to_string(),
        source,
    })?;
    let mut tagged = 0usize;
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|source| StoreError::Io {
            path: jsonl_path.to_string(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let tag: PrincipleTag =
            serde_json::from_str(&line).map_err(|source| StoreError::Parse {
                line: i + 1,
                source,
            })?;
        if tag.domains.is_empty() && tag.principles.is_empty() {
            continue;
        }
        db.query(
            "UPDATE type::record('saying', $id)
             SET domains = $domains, principles = $principles, machine_tagged = true;",
        )
        .bind(("id", tag.id))
        .bind(("domains", tag.domains))
        .bind(("principles", tag.principles))
        .await?
        .check()?;
        tagged += 1;
    }
    tracing::info!(
        count = tagged,
        path = jsonl_path,
        "applied principle-index facets"
    );
    Ok(tagged)
}
