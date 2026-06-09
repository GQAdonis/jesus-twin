# jesus-twin

Rust agent-service workspace for the Jesus digital twin. See the design docs in the repo
root — `../ARCHITECTURE.md` is the authoritative build spec — and `../CLAUDE.md` for the
engineering rules that govern this code. The story of bringing this up on GPU and the
fine-tune investigation is in [`../docs/FINDINGS.md`](../docs/FINDINGS.md).

## Status: operational RAG-first release

The service runs end-to-end on CUDA: **mistral.rs (Gemma 4, 4-bit ISQ)**, **SurrealDB**
embedded store, and **embeddinggemma** hybrid retrieval are wired in behind the `cuda`
feature. It ships **RAG-first on the base `google/gemma-4-E4B-it`** (ARCHITECTURE.md step 3):
retrieval grounds every answer in cited verses, and the coverage gate refuses out-of-corpus
questions. The Unsloth-merged fine-tune is **deferred** — it over-trained on too little data
and is degenerate (see `../docs/FINDINGS.md`); the serving stack needs no change to adopt a
sound checkpoint later.

> Without the `cuda`/`mistralrs` feature the binary still builds and runs against a
> deterministic mock engine (BM25-only retrieval) — handy for fast dev and tests.

## Crates (dependency direction is strictly downward)

| Crate | Role |
|---|---|
| `jesus-twin-core` | Agent core: `AgentEvent` stream, orchestrator, coverage gate. No protocol, no I/O. |
| `jesus-twin-inference` | `Engine` + `Embedder` traits; wraps mistral.rs as a library (Gemma 4 + embeddinggemma). |
| `jesus-twin-store` | `Store` trait; SurrealDB 3.1 graphrag (vector + graph + BM25 + RRF). |
| `jesus-twin-skills` | `Skill` registry → CLI + MCP server + model tool-list. |
| `jesus-twin-admission` | `Gatekeeper` trait; parking-lot admission control. |
| `jesus-twin-api` | Axum 0.8 app + four thin adapters (openai, mcp, agui, a2a). |
| `jesus-twin-cli` | The `jesus-twin` binary: `serve` / `ingest` / `retrieve` / `ask` / `skill`. |

## Running the release

### 1. Get the models

The binary loads two models from **local directories** so it runs offline:

- **Generation:** `google/gemma-4-E4B-it` → `../jesus-twin-base/` (ungated).
- **Embedder:** `google/embeddinggemma-300m` → `../jesus-twin-embeddinggemma/` (**gated** by
  Google — accept its license at <https://huggingface.co/google/embeddinggemma-300m> while
  logged in, or the download 403s). 768-dim, which must match the store's `EMBEDDING_DIM`.

```bash
HF_TOKEN=hf_xxx ../scripts/download-models.sh        # both (or: base | embed)
```

Model weights are git-ignored — never commit them.

### 2. Build (CUDA)

```bash
export PATH=/usr/local/cuda/bin:$PATH
export CUDA_COMPUTE_CAP=89          # NVIDIA L4 = 8.9; set to your GPU's compute capability
cargo build --release --features cuda -p jesus-twin-cli
```

`cuda` implies the `mistralrs` backend (forwards to `mistralrs/cuda`); `flash-attn` and
`cudnn` features are available too. First build pulls and compiles candle's CUDA kernels
(~25 min); incremental builds are ~6 min. Requires the CUDA toolkit (`nvcc`).

### 3. Ingest the corpus (once, persistent + vectorized)

```bash
export JESUS_TWIN_MODEL=$PWD/../jesus-twin-base
export JESUS_TWIN_EMBED_MODEL=$PWD/../jesus-twin-embeddinggemma
./target/release/jesus-twin ingest ../build/rag_corpus.jsonl --db ./twin.db
```

With the embedder attached this writes both BM25 indexes **and** the HNSW vectors
(`emb_original`/`emb_modern`) for all 927 passages, so serving starts instantly without
re-embedding. Re-running is idempotent (UPSERT by stable saying id).

### 4. Serve or ask

```bash
# one-shot
./target/release/jesus-twin ask "what are the greatest commandments in the law?" --db ./twin.db

# HTTP service (OpenAI surface; MCP/AG-UI/A2A to follow)
./target/release/jesus-twin serve --db ./twin.db --addr 0.0.0.0:8080
```

(Omit `--db` to use an ephemeral in-memory store that ingests + vectorizes
`../build/rag_corpus.jsonl` at startup.)

### How generation, retrieval, and citations fit together

- **4-bit ISQ.** The generation model is served at **4-bit** via mistral.rs **in-situ
  quantization (`isq: Q4K`)** applied to the BF16 safetensors at load — you do *not* download
  a quantized file. (This mistral.rs rev can't load a Gemma GGUF; the `.gguf` exports are only
  for llama.cpp/Ollama.) Set `JESUS_TWIN_ISQ=none` to serve full-precision BF16 instead.
- **Hybrid retrieval.** embeddinggemma vectorizes each passage and the query; the store fuses a
  BM25 full-text leg and an HNSW vector leg via **RRF**. Without an embedder it degrades
  gracefully to BM25-only.
- **Citations are the product.** Every answer streams `Citation` events (`ref` + score) for the
  verses that grounded it, and the coverage gate refuses questions the corpus doesn't address —
  the model is never asked to generate doctrine, only to render cited lines in modern English.

## Develop (no GPU / no models)

```bash
cargo build                          # mock backend, BM25-only — fast
cargo clippy --all-targets
cargo fmt --all
cargo run --bin jesus-twin -- --help
cargo run --bin jesus-twin -- retrieve "forgiveness and mercy" --db ./twin.db
```

## Next (build sequence, ARCHITECTURE.md §11)

Steps 1–3 are done (store + inference + RAG-first orchestrator on the base model). Remaining:

4. OpenAI adapter end-to-end (streaming).
5. `jesus-twin-admission` gatekeeper in front of the engine.
6. Remaining adapters (MCP stdio+HTTP, AG-UI, A2A).
7. `jesus-twin-skills` + interactive `chat`.
8. Fine-tune (Unsloth LoRA) — **blocked on annotation**; re-train with gentler hyperparameters
   (lr ~2e-5, ~1 epoch) once enough annotated data exists, then merge and repoint
   `JESUS_TWIN_MODEL`. Keep the LoRA only if it improves style without hurting grounding.
