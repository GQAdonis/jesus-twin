# MEMORY — jesus-twin

Durable working memory for this repo: decisions, gotchas, and the current operational state, so
they are not relearned. Full narrative in [`docs/FINDINGS.md`](./docs/FINDINGS.md); build/run in
[`jesus-twin/README.md`](./jesus-twin/README.md).

## Current state (2026-06-09)

- **Operational RAG-first release.** CUDA build + 4-bit (ISQ Q4K) generation + embeddinggemma
  hybrid retrieval + citations + persistent vectorized store, verified end-to-end on an NVIDIA
  L4. Runs on the **base `google/gemma-4-E4B-it`**.
- **The merged fine-tune (`jesus-twin-merged/`) is degenerate and deferred.** Over-trained on
  75 SFT examples at lr 2e-4 × 3 epochs → collapses to looping output in mistral.rs *and*
  llama.cpp. Not a code/build/quant/engine/retrieval bug — the weights are bad. Re-enable later
  by dropping a sound checkpoint in a dir and pointing `JESUS_TWIN_MODEL` at it.

## Load-bearing decisions

- **4-bit = ISQ on safetensors, NOT the GGUF.** This mistral.rs rev (`@b7746a85`) has no Gemma
  arch in its GGUF loader, so `unsloth.Q4_K_M.gguf` is unusable in-process. Serve the BF16
  safetensors with `with_isq(Q4K)` (Gemma 4 is a VLM → use `MultimodalModelBuilder`). GGUFs are
  only for llama.cpp/Ollama. `JESUS_TWIN_ISQ=none` serves full BF16.
- **Build flag:** `cargo build --release --features cuda -p jesus-twin-cli`
  (`CUDA_COMPUTE_CAP=89` for the L4; nvcc on PATH). `cuda` implies the `mistralrs` backend.
- **Models are local + env-driven:** `JESUS_TWIN_MODEL` (generation) and
  `JESUS_TWIN_EMBED_MODEL` (embeddinggemma) point at local dirs so serving is offline. Fetch via
  `scripts/download-models.sh`. models.yaml is reference-only; the binary reads the env vars.
- **embeddinggemma is gated** (Google manual approval) and 768-dim (must match store
  `EMBEDDING_DIM`). Wired to the store via a `StoreEmbedder` adapter + `with_embedder` →
  hybrid BM25+vector+RRF. `ingest --db` once to persist HNSW vectors; serve reuses them.
- **Always cap output tokens** (`MAX_OUTPUT_TOKENS=512`); uncapped `do_sample` runs to the
  context limit (~45 min/answer).

## Gotchas hit (don't repeat)

- `pkill -f "<pattern that is also in this command's own argv>"` self-kills the shell (exit
  144). Use a pattern that doesn't match the killer.
- `hf download REPO --exclude "*.gguf" "a" "b"` — extra positional globs are treated as explicit
  filenames and `--exclude` is ignored → nothing downloads (exit 0). Pass one repo, no stray
  positionals.
- `/usr/bin/time` isn't installed here; use the bash `time` builtin.
