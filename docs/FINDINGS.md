# Findings — bringing up the CUDA release (2026-06-09)

This records the work that turned the compile-clean scaffold into a **running, GPU-accelerated
RAG-first release**, and the investigation that diagnosed why the fine-tuned checkpoint had to
be set aside. It is the durable account behind the per-file changes; pair it with
[`../jesus-twin/README.md`](../jesus-twin/README.md) for how to build and run.

## TL;DR

- The application is **operational**: CUDA build, 4-bit (ISQ Q4K) generation, embeddinggemma
  hybrid retrieval, citations, persistent vectorized store. Verified end-to-end on an NVIDIA L4.
- It ships **RAG-first on the base `google/gemma-4-E4B-it`** (ARCHITECTURE.md step 3).
- The Unsloth-**merged fine-tune is degenerate** and is deferred — root-caused to over-training
  on too little data, *not* a bug in the build, quantization, engine, or retrieval.

## Environment

NVIDIA L4 (23 GB, compute capability **8.9**), 31 GB RAM, 8 vCPU, CUDA toolkit 12.4, driver
550, Rust 1.96. The mistral.rs fork is pinned at `GQAdonis/mistral.rs @ b7746a85` (mistralrs
0.8.3), which pulls `GQAdonis/candle`.

## What was built / wired

1. **CUDA enabled on mistral.rs.** Added a `cuda` feature to `jesus-twin-inference` and
   `jesus-twin-cli` that forwards to `mistralrs/cuda` (plus optional `flash-attn`, `cudnn`).
   `cuda` implies `mistralrs`, so the release flag is a single `--features cuda`. The linked
   binary loads `libcudart/libcublas/libcurand` — real GPU, not a CPU fallback. First build
   ~25 min (candle CUDA kernels); incremental ~6 min.

2. **4-bit serving via ISQ, not GGUF.** This mistral.rs rev's GGUF loader recognizes only
   Llama/Qwen/Phi/Mistral3/etc. — **no Gemma** (verified in its `GGUFArchitecture` enum). So
   `unsloth.Q4_K_M.gguf` cannot be loaded here. The 4-bit path is **in-situ quantization
   (ISQ Q4K) of the BF16 safetensors at load** (`MultimodalModelBuilder::with_isq(Q4K)`;
   Gemma 4 is a VLM class, so the text-only builder rejects it). The runtime weights are still
   4-bit — quantized in-process on the GPU rather than read pre-quantized. The `.gguf` files in
   the model dirs are only useful for llama.cpp/Ollama. A `JESUS_TWIN_ISQ=none` env toggle was
   added to serve full-precision BF16 when wanted.

3. **embeddinggemma wired into retrieval.** Downloaded `google/embeddinggemma-300m` (gated;
   manual Google approval) locally (768-dim — matches the store's `EMBEDDING_DIM`). Added a
   `StoreEmbedder` adapter (inference `Embedder` → store `Embed`) and attached it via
   `with_embedder` in `serve`/`ask`/`ingest`, upgrading retrieval from BM25-only to **hybrid
   BM25 + HNSW-vector fused by RRF**. The embed-model id in config was also corrected
   (`google/embedding-gemma` → `google/embeddinggemma-300m`).

4. **Output-token cap.** Generation had no length bound; the `do_sample=true` model ran to the
   context limit (~45 min/answer). `generate()` now sets `RequestBuilder::set_sampler_max_len`
   (`MAX_OUTPUT_TOKENS = 512`).

## The degeneration investigation

**Symptom.** Every answer collapsed into repetition ("a field of life a field of life…",
"the bones the bones…") and never stopped.

**Isolation — three independent runtimes, same failure:**

| Engine | Precision / source | Decoding | Result |
|---|---|---|---|
| mistral.rs | BF16 safetensors (no ISQ) | sampled | degenerate |
| mistral.rs | Q4K ISQ | sampled | degenerate |
| llama.cpp (built from source, CUDA) | `unsloth.Q4_K_M.gguf` | **greedy (temp 0)** | degenerate |

Because mistral.rs loaded the **safetensors directly at BF16** and still failed, it is **not**
quantization, **not** a GGUF-conversion artifact, **not** mistral.rs's new gemma4 path, **not**
the chat template (the `<|turn>`/`<turn|>` markers are the model's real special tokens), and
**not** sampling (greedy in a second engine reproduced it). The only common factor is the
**weights**.

**Root cause.** `build/sft_merged.jsonl` has only **75 SFT records** with ultra-short targets,
trained (`train_lora.py`) at **lr 2e-4 for 3 epochs** with response-only masking. A few hundred
trained tokens at an aggressive LR collapsed a 4B model. This is the project's known
**annotation bottleneck**, not a code defect. The masking delimiters and chat-template
application in `train_lora.py` were checked and are correct.

**Proof the stack is sound.** The same pipeline on the **base `google/gemma-4-E4B-it`** (no
fine-tune) returns coherent, grounded, correctly-cited answers — e.g. for "love your enemies"
it quotes Luke 6:27-35 verbatim with citations; for "the greatest commandments" it quotes
Mark 12:29-31 / Matthew 22:37-40. So the build, ISQ, retrieval, citations, and orchestration
are all correct; only the merged checkpoint was bad.

## Decision

Ship **RAG-first on the base model** (ARCHITECTURE.md step 3: "useful and safe with no
fine-tune"). The fine-tune is optional voice polish and is deferred until annotation yields
enough data for a stable style LoRA. When that exists, re-train with gentler hyperparameters
(lr ~2e-5, ~1 epoch), drop the merged checkpoint into a directory, and point `JESUS_TWIN_MODEL`
at it — no serving-code change is needed to pick up the LoRA voice.

## Sample output (base model + RAG, ISQ Q4K)

Prompt: *"what are the greatest commandments in the law?"* → cites Mark 12:29-31,
Matthew 22:37-40, Mark 10:3, Luke 14:3, Luke 10:26; answer quotes the Shema +
"love your neighbor as yourself … no other commandment greater than these," rendered in modern
first-person voice, stops cleanly. ~1m21s against the pre-vectorized persistent store (model
load dominates; retrieval/generation are seconds).
