//! Deterministic mock implementations of [`Engine`] and [`Embedder`].
//!
//! These let `jesus-twin-core` build and test the full orchestrator, and let
//! `jesus-twin-store` exercise its vector path, **without** loading a multi-GB model or
//! pulling the mistral.rs/candle forks. They are deterministic (no RNG — see CLAUDE.md note
//! that `Math::random` is unavailable in some contexts anyway), so tests are reproducible.
//!
//! Honesty boundary (CLAUDE.md principle 5): a mock embedding is a hashed bag-of-words
//! projection, NOT a semantic embedding. It gives *lexical* similarity (shared tokens →
//! closer vectors), enough to wire and test the retrieval plumbing — but it must never be
//! mistaken for the real Embedding Gemma vectors. Swap in [`crate::mistral`] (the
//! `mistralrs` feature) for real semantics.

use async_trait::async_trait;

use crate::embed::Embedder;
use crate::engine::{Engine, EngineError, GenRequest};

/// Embedding dimension produced by [`MockEmbedder`]. Must equal the store's HNSW
/// `DIMENSION` (`jesus-twin-store::schema::EMBEDDING_DIM`, currently 768) or the index will
/// reject the vectors.
pub const MOCK_EMBEDDING_DIM: usize = 768;

/// A deterministic stand-in for the Gemma generation engine.
///
/// It does not generate language — it echoes the request in a fixed, inspectable shape so
/// the orchestrator and adapters have a real `Engine` to drive. Output makes the grounding
/// contract visible: it restates the user ask and the supplied context, never inventing.
#[derive(Debug, Default, Clone)]
pub struct MockEngine;

impl MockEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Engine for MockEngine {
    async fn generate(&self, req: GenRequest) -> Result<String, EngineError> {
        if req.user.trim().is_empty() {
            return Err(EngineError::Generate("empty user prompt".into()));
        }
        let mut out = String::new();
        if !req.context.trim().is_empty() {
            out.push_str("[grounded on retrieved context] ");
        }
        out.push_str("(mock) ");
        out.push_str(req.user.trim());
        Ok(out)
    }
}

/// A deterministic stand-in for the embedding model.
///
/// Produces an L2-normalized [`MOCK_EMBEDDING_DIM`]-dim vector from a hashed bag of
/// lowercased word tokens: each token is hashed to a bucket whose weight is incremented.
/// Shared vocabulary → higher cosine similarity, so the store's vector + RRF path can be
/// exercised. Purely lexical — no semantics.
#[derive(Debug, Default, Clone)]
pub struct MockEmbedder;

impl MockEmbedder {
    pub fn new() -> Self {
        Self
    }

    fn embed_one(text: &str) -> Vec<f32> {
        let mut v = vec![0.0f32; MOCK_EMBEDDING_DIM];
        for token in text.split(|c: char| !c.is_alphanumeric()) {
            if token.is_empty() {
                continue;
            }
            let bucket = fnv1a(&token.to_lowercase()) as usize % MOCK_EMBEDDING_DIM;
            v[bucket] += 1.0;
        }
        l2_normalize(&mut v);
        v
    }
}

#[async_trait]
impl Embedder for MockEmbedder {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EngineError> {
        Ok(texts.iter().map(|t| Self::embed_one(t)).collect())
    }
}

/// FNV-1a hash — small, fast, deterministic; no external dep, no RNG.
fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in s.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// Normalize in place to unit L2 length (cosine-ready). A zero vector is left as zeros.
fn l2_normalize(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_engine_grounds_and_never_empty() {
        let e = MockEngine::new();
        let out = e
            .generate(GenRequest {
                system: "contract".into(),
                context: "Mark 12:17 ...".into(),
                user: "what about taxes".into(),
            })
            .await
            .unwrap();
        assert!(
            out.contains("grounded"),
            "context present should mark grounding"
        );
        assert!(out.contains("what about taxes"));
    }

    #[tokio::test]
    async fn mock_engine_rejects_empty_prompt() {
        let e = MockEngine::new();
        assert!(
            e.generate(GenRequest {
                system: String::new(),
                context: String::new(),
                user: "  ".into()
            })
            .await
            .is_err()
        );
    }

    #[tokio::test]
    async fn mock_embedder_is_deterministic_and_normalized() {
        let e = MockEmbedder::new();
        let a = e.embed(&["love your neighbor".into()]).await.unwrap();
        let b = e.embed(&["love your neighbor".into()]).await.unwrap();
        assert_eq!(a, b, "embedding must be deterministic");
        assert_eq!(a[0].len(), MOCK_EMBEDDING_DIM);
        let norm: f32 = a[0].iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-5,
            "expected unit length, got {norm}"
        );
    }

    #[tokio::test]
    async fn shared_vocabulary_is_more_similar() {
        let e = MockEmbedder::new();
        let v = e
            .embed(&[
                "love your neighbor".into(),
                "love your enemies".into(),
                "render unto Caesar".into(),
            ])
            .await
            .unwrap();
        let sim = |a: &[f32], b: &[f32]| a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>();
        let shared = sim(&v[0], &v[1]); // share "love your"
        let unrelated = sim(&v[0], &v[2]); // share nothing
        assert!(
            shared > unrelated,
            "shared-vocab pair should be more similar ({shared} vs {unrelated})"
        );
    }
}
