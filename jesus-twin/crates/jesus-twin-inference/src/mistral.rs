//! Real mistral.rs-backed [`Engine`] / [`Embedder`] — gated behind the `mistralrs` feature.
//!
//! Off by default so normal builds stay fast and avoid the candle version-coupling build
//! errors (CLAUDE.md gotcha). When the feature is on, this wraps the `mistralrs` fork as a
//! **library** (ARCHITECTURE.md §5): a `TextModelBuilder` serves the Unsloth-merged Gemma 4
//! checkpoint (generation, thinking mode OFF) and an `EmbeddingModelBuilder` serves Embedding
//! Gemma — one process, no separate embedding service.
//!
//! API verified against GQAdonis/mistral.rs @ b7746a85 (mistralrs 0.8.3):
//! `MultimodalModelBuilder::new(id).with_isq(..).build().await -> Model`
//! (Gemma 4 is `Gemma4ForConditionalGeneration` — a VLM — so TextModelBuilder
//! rejects it; MultimodalModelBuilder covers both text-only and multimodal prompts);
//! `Model::send_chat_request(TextMessages) -> ChatCompletionResponse`
//! (`.choices[0].message.content: Option<String>`);
//! `EmbeddingModelBuilder::new(id).build().await -> Model`;
//! `Model::generate_embeddings(EmbeddingRequestBuilder::new().add_prompt(..).build()?) ->
//! Vec<Vec<f32>>`.

use async_trait::async_trait;
use mistralrs::{
    EmbeddingModelBuilder, EmbeddingRequestBuilder, IsqType, Model, MultimodalModelBuilder,
    RequestBuilder, TextMessageRole,
};

use crate::embed::Embedder;
use crate::engine::{Engine, EngineError, GenRequest};

/// Hard cap on generated tokens per answer. The twin renders a short, cited saying in modern
/// English; without a bound the merged model's sampler (`do_sample=true`) rambles to the context
/// limit — minutes of GPU time per request. Generous for the task; tighten if needed.
const MAX_OUTPUT_TOKENS: usize = 512;

/// Configuration for the real backend. Populated from `models.yaml` by the binary.
#[derive(Debug, Clone)]
pub struct MistralConfig {
    /// HF id (e.g. `google/gemma-4-E4B-it`) or a local merged-checkpoint path.
    pub model: String,
    /// Embedding model id (e.g. `google/embedding-gemma`).
    pub embed_model: String,
    /// In-situ quantization at load (e.g. Q4K). `None` serves full precision.
    pub isq: Option<IsqType>,
}

impl MistralConfig {
    /// Defaults matching `models.yaml`: Gemma 4 E4B + Embedding Gemma, Q4K.
    pub fn defaults() -> Self {
        Self {
            model: "google/gemma-4-E4B-it".to_string(),
            embed_model: "google/embeddinggemma-300m".to_string(),
            isq: Some(IsqType::Q4K),
        }
    }
}

/// mistral.rs-backed engine + embedder. Holds the two model runtimes once built.
pub struct MistralEngine {
    generation: Model,
    embedding: Model,
}

impl MistralEngine {
    /// Build both runtimes from `config`. Loads weights (multi-GB) — call once at startup.
    /// Thinking mode is left off here and disabled per-request (diction fidelity, ARCH §5).
    pub async fn build(config: MistralConfig) -> Result<Self, EngineError> {
        // Gemma 4 uses `Gemma4ForConditionalGeneration` — a multimodal class —
        // so TextModelBuilder rejects it. MultimodalModelBuilder handles both
        // text-only and multimodal inference with the same send_chat_request API.
        let mut gen_builder = MultimodalModelBuilder::new(&config.model).with_logging();
        if let Some(isq) = config.isq {
            gen_builder = gen_builder.with_isq(isq);
        }
        let generation = gen_builder
            .build()
            .await
            .map_err(|e| EngineError::Load(format!("generation model: {e}")))?;

        let mut embed_builder = EmbeddingModelBuilder::new(&config.embed_model).with_logging();
        if let Some(isq) = config.isq {
            embed_builder = embed_builder.with_isq(isq);
        }
        let embedding = embed_builder
            .build()
            .await
            .map_err(|e| EngineError::Load(format!("embedding model: {e}")))?;

        Ok(Self {
            generation,
            embedding,
        })
    }
}

#[async_trait]
impl Engine for MistralEngine {
    async fn generate(&self, req: GenRequest) -> Result<String, EngineError> {
        // System contract, then retrieved context + the user ask in one user turn. Thinking
        // OFF so the twin renders the saying rather than emitting reasoning traces.
        let messages = RequestBuilder::default()
            .add_message(TextMessageRole::System, &req.system)
            .add_message(
                TextMessageRole::User,
                format!("{}\n\n{}", req.context, req.user),
            )
            .enable_thinking(false)
            .set_sampler_max_len(MAX_OUTPUT_TOKENS);

        let response = self
            .generation
            .send_chat_request(messages)
            .await
            .map_err(|e| EngineError::Generate(e.to_string()))?;

        response
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| EngineError::Generate("model returned no content".into()))
    }
}

#[async_trait]
impl Embedder for MistralEngine {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EngineError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        // `generate_embeddings` takes the builder and calls `.build()` internally.
        let mut builder = EmbeddingRequestBuilder::new();
        for t in texts {
            builder = builder.add_prompt(t.clone());
        }
        self.embedding
            .generate_embeddings(builder)
            .await
            .map_err(|e| EngineError::Generate(e.to_string()))
    }
}
