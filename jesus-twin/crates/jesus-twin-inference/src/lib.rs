//! Inference layer: mistral.rs used **as a library**, not a subprocess (ARCHITECTURE.md §5).
//!
//! One runtime feeds both generation (Gemma 4 E4B, merged LoRA, thinking mode off) and
//! embeddings (Embedding Gemma / Qwen3-Embedding) — no separate embedding service. The
//! token/tool-call output is mapped into the core's `AgentEvent` stream in `stream.rs`.
//!
//! Two backends implement the [`Engine`] / [`Embedder`] traits:
//! - [`mock`] — deterministic stand-ins (always available), so the rest of the system can
//!   be built and tested without a model. The default.
//! - [`mistral`] — the real mistral.rs runtime, behind the off-by-default `mistralrs`
//!   feature (keeps the candle-coupling build cost out of normal builds; CLAUDE.md gotcha).

pub mod embed;
pub mod engine;
pub mod mock;
pub mod stream;

#[cfg(feature = "mistralrs")]
pub mod mistral;

pub use embed::Embedder;
pub use engine::{Engine, EngineError, GenChunk, GenRequest};
pub use mock::{MOCK_EMBEDDING_DIM, MockEmbedder, MockEngine};

#[cfg(feature = "mistralrs")]
pub use mistral::{MistralConfig, MistralEngine};
