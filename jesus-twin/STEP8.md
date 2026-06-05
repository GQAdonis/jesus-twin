# Step 8 — Fine-tune & swap to the real model

Build steps 1–7 ship a working, grounded twin on the **deterministic `MockEngine`**. Step 8
swaps in the real Gemma 4 served by the mistral.rs fork. The Rust half (the `MistralEngine`)
is **done and compile-verified** against the fork API; the rest is offline, GPU-bound work
you run yourself.

## What's already done (Rust)

- `jesus-twin-inference` has a real `MistralEngine` behind the **`mistralrs` feature**
  (`crates/jesus-twin-inference/src/mistral.rs`), implementing `Engine` (Gemma 4 generation,
  thinking mode OFF) and `Embedder` (Embedding Gemma) via the fork's `TextModelBuilder` /
  `EmbeddingModelBuilder`. Verified with `cargo check -p jesus-twin-inference --features
  mistralrs` (pulls `GQAdonis/mistral.rs @ b7746a85` + its candle fork; **no candle-coupling
  errors at this rev**).

```bash
cargo check -p jesus-twin-inference --features mistralrs   # compile-correct, no weights
```

## The blocker: annotation

The style LoRA needs `build/sft_style.jsonl`, which is empty until the
`Modern Rendering` + `Reasoning Move` columns in `jesus_full_red_letter.xlsx` are filled
(see the repo's annotation note). The RAG path already works without this; only the *voice*
fine-tune is blocked.

## The pipeline (offline, your GPU)

1. **Annotate** the corpus (target: a few hundred ready rows; gold standard =
   `jesus_sayings_dataset.xlsx` seed + the M01–M18 rubric).
2. **Build the SFT data:** `python build_training_jsonl.py` → `build/sft_style.jsonl`,
   `build/eval_heldout.jsonl`.
3. **Train the LoRA** with Unsloth/QLoRA on Gemma 4 E4B Instruct, thinking mode off, blending
   a small in-domain instruction set (README §3, ALIGNMENT_AND_TUNING §1). Export **merged**
   (`save_pretrained_merged`) — runtime LoRA for Gemma 4 is unsupported, so you serve the
   merged checkpoint (CLAUDE.md gotcha).
4. **Evaluate** on `build/eval_heldout.jsonl`, faceted by reasoning move: grounding /
   no-fabrication first, then style fidelity, citation integrity, refusal behavior.
5. **Serve the merged checkpoint:** point `MistralConfig.model` at the local merged path and
   build with `--features mistralrs`. Keep the LoRA only if it improves style-by-move without
   hurting grounding.

## Wiring the engine into `serve`/`ask` (the small remaining Rust change)

`build_orchestrator` currently hardcodes `MockEngine::new()`. To use the real engine, make it
generic over `E: Engine` (as it already is over the gatekeeper) and construct under the
feature:

```rust
#[cfg(feature = "mistralrs")]
let engine = jesus_twin_inference::MistralEngine::build(
    jesus_twin_inference::MistralConfig {
        model: "/models/jesus-twin-merged".into(),     // the merged checkpoint
        embed_model: "google/embedding-gemma".into(),
        isq: Some(mistralrs::IsqType::Q4K),
    },
).await?;
#[cfg(not(feature = "mistralrs"))]
let engine = jesus_twin_inference::MockEngine::new();
```

Then add `mistralrs = ["jesus-twin-inference/mistralrs"]` to `jesus-twin-cli`'s `[features]`
and build the binary with `--features mistralrs`. The orchestrator, all four adapters, the
admission gatekeeper, and the skills are engine-agnostic — nothing else changes.

## Also still open (smaller, no GPU needed)

- Real **Embedder vector + RRF path** in the store (the schema/HNSW indexes already exist;
  the `MistralEngine`/`MockEmbedder` can populate `emb_*`).
- The **`mindmap` skill** + graph projection.
- Expose the **full skill registry over MCP** (currently only the `ask` tool).
