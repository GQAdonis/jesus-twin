//! Embedded SurrealDB implementation of [`Store`].
//!
//! Backed by SurrealKV (`kv-surrealkv`), SurrealDB's own pure-Rust embedded store.
//! Retrieval is **BM25-only when no
//! embedder is attached** (the graceful default) and **hybrid BM25 + HNSW-vector fused by RRF**
//! when one is (`with_embedder`): ingest then vectorizes every saying into `emb_original` /
//! `emb_modern` and `retrieve` fuses the full-text and vector legs (ARCHITECTURE.md §7). RRF is
//! computed in Rust (see [`rrf_fuse`]) — SurrealDB's `search::rrf` proved fragile in 3.1.2. The
//! move/parallels graph is built at ingest so graph expansion can be added without re-ingesting.
//!
//! [`schema`]: crate::schema

use std::path::Path;
use std::sync::Arc;

use surrealdb::Surreal;
use surrealdb::engine::local::{Db, Mem, SurrealKv};
use surrealdb::types::SurrealValue;

use crate::embed::Embed;
use crate::ingest;
use crate::retrieve::{Passage, RetrievalSet};
use crate::schema;
use crate::store::{Store, StoreError};

/// Namespace and database the twin uses. Single-tenant at the edge; a remote deployment can
/// override these without touching callers.
const NS: &str = "jesus_twin";
const DB: &str = "corpus";

/// An embedded SurrealDB store.
///
/// With no embedder set, retrieval is BM25-only (the graceful default). Attach one via
/// [`SurrealStore::with_embedder`] to populate `emb_*` at ingest and fuse the HNSW vector leg
/// into retrieval via `search::rrf` (ARCHITECTURE.md §7).
pub struct SurrealStore {
    db: Surreal<Db>,
    embedder: Option<Arc<dyn Embed>>,
}

impl SurrealStore {
    /// Open (or create) a persistent store at `path` and apply the schema.
    pub async fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let db = Surreal::new::<SurrealKv>(path.as_ref()).await?;
        Self::init(db).await
    }

    /// Open an in-memory store (tests, ephemeral runs) and apply the schema.
    pub async fn memory() -> Result<Self, StoreError> {
        let db = Surreal::new::<Mem>(()).await?;
        Self::init(db).await
    }

    async fn init(db: Surreal<Db>) -> Result<Self, StoreError> {
        db.use_ns(NS).use_db(DB).await?;
        // DEFINE statements are idempotent (IF NOT EXISTS in the schema), so re-opening an
        // existing store is safe.
        db.query(schema::SCHEMA).await?.check()?;
        Ok(Self { db, embedder: None })
    }

    /// Attach an embedder, enabling the vector + RRF retrieval path (builder style).
    pub fn with_embedder(mut self, embedder: Arc<dyn Embed>) -> Self {
        self.embedder = Some(embedder);
        self
    }

    /// Borrow the underlying handle (for `mindmap` projections and future skills).
    pub fn db(&self) -> &Surreal<Db> {
        &self.db
    }

    /// BM25-only retrieval across both text registers (the fallback when no embedder is set).
    async fn retrieve_bm25(&self, q: &str, limit: usize) -> Result<RetrievalSet, StoreError> {
        // The `@{n},OR@` form gives a scored predicate (ref `n`, read by `search::score(n)`)
        // with OR semantics between terms — a multi-word question ranks any passage matching
        // ANY term. Scores from both registers are summed so a hit in either ranks it.
        let sql = "
            SELECT record::id(id) AS id, ref, book_author, text_original, text_modern,
                   context, location, occasion, move, translation,
                   (search::score(0) + search::score(1)) AS score
            FROM saying
            WHERE text_original @0,OR@ $q OR text_modern @1,OR@ $q
            ORDER BY score DESC
            LIMIT $limit;
        ";
        let mut res = self
            .db
            .query(sql)
            .bind(("q", q.to_string()))
            .bind(("limit", limit))
            .await?;
        Ok(RetrievalSet {
            passages: res.take(0)?,
        })
    }

    /// Hybrid retrieval: BM25 (both registers) + HNSW vector (both registers), fused with
    /// Reciprocal Rank Fusion. Used when an embedder is attached.
    ///
    /// RRF is computed in Rust rather than via SurrealDB's `search::rrf`: that builtin proved
    /// fragile in 3.1.2 (it silently returned nothing once the candidate objects carried more
    /// than `id` + one field). The algorithm is trivial — `score(id) = Σ 1/(k + rank)` over the
    /// lists a doc appears in — and doing it here keeps it correct, testable, and decoupled
    /// from that quirk. Each leg returns only a ranked list of string ids; we fuse, take the
    /// top-k, then fetch those passages.
    async fn retrieve_hybrid(
        &self,
        q: &str,
        embedding: Vec<f32>,
        limit: usize,
    ) -> Result<RetrievalSet, StoreError> {
        const CANDIDATES: usize = 20; // per-leg depth before fusion
        const K: f32 = 60.0; // RRF smoothing constant

        let ft_o = self.rank_bm25(q, 0, "text_original", CANDIDATES).await?;
        let ft_m = self.rank_bm25(q, 1, "text_modern", CANDIDATES).await?;
        let v_o = self
            .rank_vector("emb_original", &embedding, CANDIDATES)
            .await?;
        let v_m = self
            .rank_vector("emb_modern", &embedding, CANDIDATES)
            .await?;

        let fused = rrf_fuse(&[ft_o, ft_m, v_o, v_m], K, limit);
        if fused.is_empty() {
            return Ok(RetrievalSet::default());
        }
        self.fetch_ranked(&fused).await
    }

    /// Diagnostic: run the four retrieval legs and report per-leg rank of every candidate,
    /// plus the fused score and `legs_matched` count. Read-only instrumentation for gate
    /// calibration (`docs/gate-calibration-claude-code-prompt.md`) — it does NOT change
    /// retrieval or the gate; it exposes the leg-agreement signal `rrf_fuse` collapses into a
    /// scalar. Mirrors `retrieve_hybrid`'s leg setup exactly so the numbers match production.
    pub async fn calibrate_query(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<CalibrationRow, StoreError> {
        const CANDIDATES: usize = 20;
        const K: f32 = 60.0;

        let q = crate::stopwords::strip(query);
        if q.is_empty() {
            return Ok(CalibrationRow::empty(query, "stopword-only"));
        }
        // The hybrid path needs an embedder; without one only the BM25 legs run (1–2 legs).
        let Some(embedder) = &self.embedder else {
            let ft_o = self.rank_bm25(&q, 0, "text_original", CANDIDATES).await?;
            let ft_m = self.rank_bm25(&q, 1, "text_modern", CANDIDATES).await?;
            return Ok(Self::fuse_calibration(
                query,
                "bm25-only",
                &[("ft_o", ft_o), ("ft_m", ft_m)],
                K,
                limit,
            ));
        };
        let emb = embedder.embed_query(query).await?;
        let ft_o = self.rank_bm25(&q, 0, "text_original", CANDIDATES).await?;
        let ft_m = self.rank_bm25(&q, 1, "text_modern", CANDIDATES).await?;
        let v_o = self.rank_vector("emb_original", &emb, CANDIDATES).await?;
        let v_m = self.rank_vector("emb_modern", &emb, CANDIDATES).await?;
        Ok(Self::fuse_calibration(
            query,
            "hybrid-4leg",
            &[("ft_o", ft_o), ("ft_m", ft_m), ("v_o", v_o), ("v_m", v_m)],
            K,
            limit,
        ))
    }

    /// Fuse named legs into a [`CalibrationRow`]: top fused id, its score, how many legs it
    /// appeared in (`legs_matched`), the live (non-empty) leg count, and the top-3 ids.
    fn fuse_calibration(
        query: &str,
        path: &str,
        legs: &[(&str, Vec<String>)],
        k: f32,
        limit: usize,
    ) -> CalibrationRow {
        use std::collections::HashMap;
        let live_legs = legs.iter().filter(|(_, l)| !l.is_empty()).count();
        // Fused score + per-leg membership for each id.
        let mut score: HashMap<&str, f32> = HashMap::new();
        let mut legs_for: HashMap<&str, Vec<&str>> = HashMap::new();
        for (name, list) in legs {
            for (rank, id) in list.iter().enumerate() {
                *score.entry(id).or_insert(0.0) += 1.0 / (k + (rank as f32 + 1.0));
                legs_for.entry(id).or_default().push(name);
            }
        }
        let mut ranked: Vec<(&str, f32)> = score.into_iter().collect();
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.0.cmp(b.0))
        });
        let top = ranked.first();
        CalibrationRow {
            query: query.to_string(),
            path: path.to_string(),
            live_legs,
            top_score: top.map(|(_, s)| *s).unwrap_or(0.0),
            top_legs_matched: top
                .map(|(id, _)| legs_for.get(id).map(|v| v.len()).unwrap_or(0))
                .unwrap_or(0),
            top3_ids: ranked
                .iter()
                .take(limit.min(3))
                .map(|(id, _)| id.to_string())
                .collect(),
        }
    }

    /// One BM25 leg: ranked string ids for `q` against full-text index `idx` on `field`.
    async fn rank_bm25(
        &self,
        q: &str,
        idx: u8,
        field: &str,
        limit: usize,
    ) -> Result<Vec<String>, StoreError> {
        // `@{idx},OR@` is the scored-OR match operator. The order field must also be selected
        // (SurrealDB requires the `search::score` idiom to appear in the projection).
        let sql = format!(
            "SELECT record::id(id) AS id, search::score({idx}) AS s FROM saying
             WHERE {field} @{idx},OR@ $q ORDER BY s DESC LIMIT $limit;"
        );
        let mut res = self
            .db
            .query(sql)
            .bind(("q", q.to_string()))
            .bind(("limit", limit))
            .await?;
        Ok(res
            .take::<Vec<IdScoreRow>>(0)?
            .into_iter()
            .map(|r| r.id)
            .collect())
    }

    /// One vector leg: ranked string ids for `embedding` against HNSW index on `field`.
    async fn rank_vector(
        &self,
        field: &str,
        embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<String>, StoreError> {
        // `<|k,ef|>` is the HNSW knn operator; results come back nearest-first.
        let sql =
            format!("SELECT record::id(id) AS id FROM saying WHERE {field} <|{limit},64|> $emb;");
        let mut res = self.db.query(sql).bind(("emb", embedding.to_vec())).await?;
        Ok(res
            .take::<Vec<IdRow>>(0)?
            .into_iter()
            .map(|r| r.id)
            .collect())
    }

    /// Fetch passages for `ids` (in order), tagging each with its fused `score`.
    async fn fetch_ranked(&self, ids: &[(String, f32)]) -> Result<RetrievalSet, StoreError> {
        let id_list: Vec<String> = ids.iter().map(|(id, _)| id.clone()).collect();
        let sql = "
            SELECT record::id(id) AS id, ref, book_author, text_original, text_modern,
                   context, location, occasion, move, translation
            FROM saying WHERE record::id(id) IN $ids;
        ";
        let mut res = self.db.query(sql).bind(("ids", id_list)).await?;
        let mut by_id: std::collections::HashMap<String, Passage> = res
            .take::<Vec<Passage>>(0)?
            .into_iter()
            .map(|p| (p.id.clone(), p))
            .collect();

        // Reassemble in fused-rank order, attaching the RRF score.
        let passages = ids
            .iter()
            .filter_map(|(id, score)| {
                by_id.remove(id).map(|mut p| {
                    p.score = Some(*score);
                    p
                })
            })
            .collect();
        Ok(RetrievalSet { passages })
    }
}

// NOTE: the embedded engine has no graceful-shutdown API (surrealdb#2399). Writes are
// committed per-statement (our ingest awaits each `.query().check()`), so data is durable —
// but if the process exits while the engine's background worker holds an auto-started
// transaction, SurrealDB logs "transaction was dropped without being committed". That log is
// cosmetic, not data loss. Callers that want to avoid it should drop the store and yield to
// the runtime before exiting (see the CLI).

#[async_trait::async_trait]
impl Store for SurrealStore {
    async fn retrieve(&self, query: &str, limit: usize) -> Result<RetrievalSet, StoreError> {
        // Strip stop words first: retrieval uses OR semantics, so without this a question
        // like "what about cryptocurrency" would match on "what"/"about" and slip past the
        // coverage gate. If nothing but stop words remains, there is no content to match ->
        // return empty so the gate refuses (the historically-humble stance).
        let q = crate::stopwords::strip(query);
        if q.is_empty() {
            return Ok(RetrievalSet::default());
        }

        // With an embedder: hybrid BM25 + vector fused by RRF. Without: BM25-only fallback.
        // The hybrid path embeds the *original* (un-stripped) query so the vector leg sees the
        // full phrase, while BM25 uses the stop-word-stripped form.
        match &self.embedder {
            Some(embedder) => {
                let emb = embedder.embed_query(query).await?;
                self.retrieve_hybrid(&q, emb, limit).await
            }
            None => self.retrieve_bm25(&q, limit).await,
        }
    }

    async fn ingest_corpus(&self, jsonl_path: &str) -> Result<usize, StoreError> {
        let count = ingest::ingest_corpus(&self.db, jsonl_path).await?;
        // If an embedder is attached, vectorize every saying so the HNSW indexes are populated
        // (otherwise emb_* stay unset and only the BM25 leg runs).
        if let Some(embedder) = &self.embedder {
            ingest::embed_all(&self.db, embedder.as_ref()).await?;
        }
        Ok(count)
    }

    async fn get_by_ref(&self, scripture_ref: &str) -> Result<Option<Passage>, StoreError> {
        let sql = "
            SELECT record::id(id) AS id, ref, book_author, text_original, text_modern,
                   context, location, occasion, move, translation
            FROM saying WHERE ref = $ref LIMIT 1;
        ";
        let mut res = self
            .db
            .query(sql)
            .bind(("ref", scripture_ref.to_string()))
            .await?;
        let mut found: Vec<Passage> = res.take(0)?;
        Ok(found.pop())
    }

    async fn find_by_move(&self, move_id: &str, limit: usize) -> Result<Vec<Passage>, StoreError> {
        let sql = "
            SELECT record::id(id) AS id, ref, book_author, text_original, text_modern,
                   context, location, occasion, move, translation
            FROM saying WHERE move = $move LIMIT $limit;
        ";
        let mut res = self
            .db
            .query(sql)
            .bind(("move", move_id.to_string()))
            .bind(("limit", limit))
            .await?;
        Ok(res.take(0)?)
    }
    // `mindmap` uses the trait's default (retrieve + project_topic) — no SurrealDB-specific
    // override needed; the move/parallels layer is reconstructed from each saying's `move`.
}

/// One query's gate-calibration measurement. Serialized to the calibration report JSONL.
///
/// `top_legs_matched` is the load-bearing signal: how many independent retrieval legs ranked
/// the top fused passage. `live_legs` records how many legs had any results (today the two
/// modern-register legs are dead because `text_modern` is empty corpus-wide, so the hybrid
/// path's ceiling is 2 live legs until annotation revives them).
#[derive(Debug, Clone, serde::Serialize)]
pub struct CalibrationRow {
    pub query: String,
    /// Which retrieval path ran: `hybrid-4leg`, `bm25-only`, or `stopword-only`.
    pub path: String,
    /// Number of legs that returned any candidates (≤ 4).
    pub live_legs: usize,
    /// Fused RRF score of the top passage.
    pub top_score: f32,
    /// How many legs ranked the top passage (the gate's proposed tier signal).
    pub top_legs_matched: usize,
    /// Top-3 fused passage ids (for spot-checking what was retrieved).
    pub top3_ids: Vec<String>,
}

impl CalibrationRow {
    /// A row for a query that produced no retrieval (e.g. stopword-only).
    fn empty(query: &str, path: &str) -> Self {
        Self {
            query: query.to_string(),
            path: path.to_string(),
            live_legs: 0,
            top_score: 0.0,
            top_legs_matched: 0,
            top3_ids: Vec::new(),
        }
    }
}

/// A single string id projected from a `record::id(id)` query.
#[derive(Debug, serde::Deserialize, SurrealValue)]
struct IdRow {
    id: String,
}

/// A string id + its leg score (the score is only used for ordering within the leg).
#[derive(Debug, serde::Deserialize, SurrealValue)]
struct IdScoreRow {
    id: String,
    #[allow(dead_code)]
    s: f32,
}

/// Reciprocal Rank Fusion over several ranked id lists. Returns up to `limit` `(id, score)`
/// pairs, highest fused score first. `score = Σ 1/(k + rank)` (0-based rank + 1) across every
/// list a given id appears in. Pure function — the unit tests pin its behavior.
fn rrf_fuse(lists: &[Vec<String>], k: f32, limit: usize) -> Vec<(String, f32)> {
    use std::collections::HashMap;
    let mut scores: HashMap<&str, f32> = HashMap::new();
    for list in lists {
        for (rank, id) in list.iter().enumerate() {
            *scores.entry(id).or_insert(0.0) += 1.0 / (k + (rank as f32 + 1.0));
        }
    }
    let mut ranked: Vec<(String, f32)> = scores
        .into_iter()
        .map(|(id, s)| (id.to_string(), s))
        .collect();
    // Sort by score desc; tie-break by id for determinism.
    ranked.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    ranked.truncate(limit);
    ranked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rrf_rewards_agreement_across_lists() {
        // "b" appears high in both lists -> should win; "a" and "c" appear in one each.
        let a = vec!["a".to_string(), "b".to_string()];
        let b = vec!["b".to_string(), "c".to_string()];
        let fused = rrf_fuse(&[a, b], 60.0, 10);
        assert_eq!(fused[0].0, "b", "doc in both lists ranks first");
        assert_eq!(fused.len(), 3);
    }

    #[test]
    fn rrf_respects_limit_and_empty() {
        assert!(rrf_fuse(&[], 60.0, 5).is_empty());
        let one = vec!["x".to_string(), "y".to_string(), "z".to_string()];
        assert_eq!(rrf_fuse(&[one], 60.0, 2).len(), 2);
    }

    #[test]
    fn calibration_counts_leg_agreement() {
        // "b" is #1 in two legs -> top_legs_matched = 2, score = 2/61.
        let legs = [
            ("ft_o", vec!["b".to_string(), "a".to_string()]),
            ("v_o", vec!["b".to_string(), "c".to_string()]),
        ];
        let row = SurrealStore::fuse_calibration("q", "hybrid-4leg", &legs, 60.0, 5);
        assert_eq!(row.top_legs_matched, 2, "top passage appeared in both legs");
        assert_eq!(row.live_legs, 2);
        assert!((row.top_score - 2.0 / 61.0).abs() < 1e-6);
    }

    #[test]
    fn calibration_single_leg_is_one() {
        // Disjoint legs -> the winner appeared in exactly one (tie broken by id "a").
        let legs = [
            ("ft_o", vec!["a".to_string()]),
            ("v_o", vec!["z".to_string()]),
        ];
        let row = SurrealStore::fuse_calibration("q", "hybrid-4leg", &legs, 60.0, 5);
        assert_eq!(row.top_legs_matched, 1);
        assert!((row.top_score - 1.0 / 61.0).abs() < 1e-6);
    }

    #[test]
    fn calibration_dead_legs_lower_live_count() {
        // Modern legs empty (today's reality) -> live_legs = 1 of the 2 supplied.
        let legs = [("ft_o", vec!["a".to_string()]), ("ft_m", Vec::new())];
        let row = SurrealStore::fuse_calibration("q", "hybrid-4leg", &legs, 60.0, 5);
        assert_eq!(row.live_legs, 1);
    }
}
