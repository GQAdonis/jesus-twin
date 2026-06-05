# jesus-twin

Rust agent-service workspace for the Jesus digital twin. See the design docs in the repo
root — `../ARCHITECTURE.md` is the authoritative build spec — and `../CLAUDE.md` for the
engineering rules that govern this code.

## Status: compile-clean scaffold

All 7 crates exist with their traits, the canonical `AgentEvent` model, and stubbed
implementations. The workspace **builds, formats, and passes clippy clean**. Heavy /
fork-pinned dependencies (mistral.rs, candle, SurrealDB, prometheus-parking-lot, rmcp) are
**not wired in yet** — they are tracked as `# TODO(deps)` in each crate's `Cargo.toml` and
in `[workspace.metadata.pending-deps]`. This avoids the candle version-coupling build
errors (CLAUDE.md gotcha) until real implementations need them.

## Crates (dependency direction is strictly downward)

| Crate | Role |
|---|---|
| `jesus-twin-core` | Agent core: `AgentEvent` stream, orchestrator, coverage gate. No protocol, no I/O. |
| `jesus-twin-inference` | `Engine` + `Embedder` traits; wraps mistral.rs as a library. |
| `jesus-twin-store` | `Store` trait; SurrealDB 3.1 graphrag (vector + graph + BM25 + RRF). |
| `jesus-twin-skills` | `Skill` registry → CLI + MCP server + model tool-list. |
| `jesus-twin-admission` | `Gatekeeper` trait; parking-lot admission control. |
| `jesus-twin-api` | Axum 0.8 app + four thin adapters (openai, mcp, agui, a2a). |
| `jesus-twin-cli` | The `jesus-twin` binary: `serve` / `skill` / `chat`. |

## Develop

```bash
cargo build              # compile-clean
cargo clippy --all-targets
cargo fmt --all
cargo run --bin jesus-twin -- --help
cargo run --bin jesus-twin -- serve   # binds the /health router (127.0.0.1:8080)
```

## Next (build sequence, ARCHITECTURE.md §11)

1. Implement `jesus-twin-store`: wire SurrealDB, ingest `../build/rag_corpus.jsonl`, build
   indexes + the move/parallels graph, verify the hybrid retrieval query.
2. Implement `jesus-twin-inference`: embed mistral.rs (Gemma 4 E4B base, thinking off) +
   Embedding Gemma.
3. Flesh out `jesus-twin-core::Orchestrator::run` (retrieve → gate → generate). **Ship a
   RAG-first, base-model build here.**
4. OpenAI adapter end-to-end (streaming).
