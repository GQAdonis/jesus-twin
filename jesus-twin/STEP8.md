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

## Serving the real engine (now wired)

`serve` is engine-pluggable: built `--features mistralrs`, it loads the real Gemma 4 from
`$JESUS_TWIN_MODEL` (the merged checkpoint) via mistral.rs; otherwise it uses `MockEngine`.
Everything downstream (orchestrator, four adapters, admission gatekeeper, skills) is
engine-agnostic.

```bash
JESUS_TWIN_MODEL=/abs/path/to/jesus-twin-merged \
cargo run --release --features mistralrs --bin jesus-twin -- serve --db ./twin.db
```

**See `RECIPE.md` for the full end-to-end path** (annotate → Unsloth LoRA → merge → serve),
including the verified Unsloth Gemma 4 training script, the Gemma-4 gotchas, and the
fork-pinning/candle-coupling notes.

## Already closed since this doc was written

- Real **Embedder vector + RRF path** in the store (hybrid BM25 + HNSW, RRF in Rust). ✓
- The **`mindmap` skill** + graph projection. ✓
- **Full skill registry over MCP** (list_skills / invoke_skill). ✓
- The **`serve` engine swap** (above). ✓
- Expose the **full skill registry over MCP** (currently only the `ask` tool).
