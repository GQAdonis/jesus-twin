//! Integration test for the hybrid (BM25 + vector + RRF) retrieval path.
//!
//! Attaches a deterministic, lexical fake embedder to the store, ingests the real corpus
//! (which embeds every passage into the HNSW indexes), and verifies that the Rust-side RRF
//! fusion over the BM25 + vector legs returns grounded, cited passages. One ingest (the embed
//! step is the slow part — HNSW insertion), several assertions.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use jesus_twin_store::embed::DIM;
use jesus_twin_store::{Embed, Store, StoreError, SurrealStore};

fn corpus_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../build/rag_corpus.jsonl")
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("../../../build/rag_corpus.jsonl"))
}

/// Deterministic lexical embedder: hashed bag-of-words -> L2-normalized DIM-vector. Shared
/// vocabulary -> closer vectors, enough to exercise the HNSW + RRF path. NOT semantic.
struct LexicalEmbedder;

#[async_trait]
impl Embed for LexicalEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, StoreError> {
        Ok(texts.iter().map(|t| embed_one(t)).collect())
    }
}

fn embed_one(text: &str) -> Vec<f32> {
    let mut v = vec![0.0f32; DIM];
    for token in text.split(|c: char| !c.is_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        let mut h: u64 = 0xcbf29ce484222325;
        for b in token.to_lowercase().bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        v[h as usize % DIM] += 1.0;
    }
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

#[tokio::test]
async fn hybrid_retrieval_fuses_vector_and_bm25() {
    let path = corpus_path();
    if !path.exists() {
        eprintln!("skipping: corpus not found at {}", path.display());
        return;
    }

    let store = SurrealStore::memory()
        .await
        .expect("store")
        .with_embedder(Arc::new(LexicalEmbedder));

    // Ingest also embeds every passage (populating the HNSW indexes) — the slow step.
    let count = store
        .ingest_corpus(path.to_str().unwrap())
        .await
        .expect("ingest+embed");
    assert_eq!(count, 927);

    // Covered query: the Rust RRF fuses the BM25 + vector legs and returns grounded passages.
    let results = store
        .retrieve("render to Caesar", 5)
        .await
        .expect("hybrid retrieve");
    assert!(
        !results.passages.is_empty(),
        "hybrid retrieval returned nothing"
    );
    assert!(
        results
            .passages
            .iter()
            .any(|p| p.text_original.contains("Caesar")),
        "fused results should surface the Caesar saying"
    );
    for p in &results.passages {
        assert!(!p.ref_.is_empty(), "every passage carries a citation");
        assert!(p.score.is_some(), "fused passages carry an RRF score");
    }

    // Out-of-corpus (stop-words only after stripping) -> empty -> the gate refuses.
    let empty = store.retrieve("what about", 5).await.expect("retrieve");
    assert!(empty.passages.is_empty());
}
