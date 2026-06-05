# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

A study-aid **"digital twin"** of Jesus of Nazareth: it renders his recorded teachings
(the red-letter corpus, World English Bible) in present-day English, preserves his
reasoning patterns (the `M01–M18` move rubric), and **never fabricates sayings** —
retrieval grounds every answer in a cited verse.

**Current stage: design + data-pipeline scaffolding. No Rust code exists yet.** The repo
is the design spec (Markdown), a working Python data pipeline, and an extracted corpus
that is **not yet annotated**. The work ahead is (a) building the Rust agent service per
`ARCHITECTURE.md`, and (b) annotating the corpus to unlock the style LoRA.

## The load-bearing principle (governs every design decision)

**Retrieval owns *truth*; the fine-tune owns *voice*; the agent layer owns
*stance/honesty*.** The model is never asked to generate doctrine — its worst case is a
paraphrase of a cited line. Carry this into any change: don't move truth into the weights,
don't bake the (non-religious, historically-humble) stance into the fine-tune, don't let
a persona become an authorization argument.

## Engineering principles (apply to every change, all languages)

These govern *how* you work in this repo. They are not negotiable defaults.

**1. Think before coding.** Don't assume; don't hide confusion; surface tradeoffs. Before
implementing: state assumptions explicitly; if uncertain, ask; if multiple interpretations
exist, present them; if a simpler approach exists, say so; if something is unclear, stop
and ask.

**2. Simplicity first.** Minimum code that solves the problem. No features beyond what was
requested, no speculative abstractions, no unnecessary configurability, no unrequested
future-proofing, no overengineering. If 50 lines solves it, don't write 200.

**3. Surgical changes.** Touch only what is necessary. Don't refactor or reformat unrelated
code; match existing conventions; remove only artifacts your change created; *mention*
unrelated issues, don't fix them.

**4. Goal-driven execution.** Define success criteria first. Convert vague requests into
testable outcomes; verify completion; run tests where available; don't stop at
implementation — stop only when success criteria are satisfied.

**5. Truth over fluency.** Never prefer a confident answer over a correct one. Distinguish
facts from assumptions and observations from conclusions; state uncertainty explicitly;
never invent APIs, functions, files, or behavior.

**6. Evidence before conclusions.** When making claims: cite evidence, show reasoning,
explain tradeoffs, and explain why alternatives were rejected.

**7. Preserve user intent.** Optimize for the user's actual goal. Don't substitute your own
preferences; don't silently expand or reduce scope; clarify when requirements conflict.

**8. Minimize irreversible actions.** Before destructive actions: confirm intent, explain
consequences, prefer reversible approaches, create rollback paths when possible.

**9. Maintain architectural consistency.** Prefer consistency over novelty. Follow existing
architecture, patterns, and naming conventions; avoid introducing new frameworks without
justification.

**10. Keep context explicit.** Never rely on hidden assumptions. State dependencies,
constraints, and limitations; record decisions.

**11. Architecture before code.** Before implementation, identify affected subsystems, data
flow, contracts, persistence impact, and UI impact. Never start coding until the
architecture is understood.

**12. Open standards first.** Prefer MCP, OpenAI-compatible APIs, A2A, AG-UI, A2UI, the
WASM Component Model, and open standards. Avoid vendor lock-in.

**13. No hidden state.** State belongs in the database, event streams, or explicit stores.
Never hide business state in UI components.

**14. Cross-platform parity.** Any feature proposal must consider web, mobile, desktop,
local execution, and cloud execution before implementation.

**15. Human override always exists.** Every automated decision must support inspection,
auditability, override, and recovery.

**16. Consult memory, then validate.** *Before generating code*, query memory
(`surreal-memory` MCP first, then the file-based memory) for prior decisions, fixes, and
mistakes so you don't repeat them. *When adding new architecture* (a new crate, framework,
library, pattern, or dependency), **validate it with web search** against current best
practices before committing to it — cite what you found (principle 6) and record the
decision in memory.

Use **feature-based clean architecture** everywhere — Rust crates and frontend alike.
**No file in any language exceeds 500 lines**; when a file reaches that size, create a
directory and split it into logical pieces.

## Documentation map — read in this order

1. `README.md` — overview; model + DB choices and the research behind them.
2. `ARCHITECTURE.md` — **the authoritative Rust build spec** (crate layout, the canonical
   `AgentEvent` model + 4-adapter mapping, mistral.rs-as-library, parking-lot/engine
   boundary, SurrealDB graphrag schema, build sequence). Prefer it on any conflict.
3. `ALIGNMENT_AND_TUNING.md` — tuning-layer roles, the non-religious historical alignment,
   skills/MCP tool safety, AG-UI custom chunks, the Bible-scope verdict.
4. `training_data_spec.md` — SFT/RAG JSONL formats from the 12-column annotation sheet.
5. `DATA_EXTRACTION.md` — the WEB red-letter corpus extractor.
6. `HANDOFF.md` — the full handoff brief that seeded this project (decisions + gotchas).
7. `digital_twin_architecture.md` — the **superseded** Python-era conceptual doc. Still
   conceptually valid, but `ARCHITECTURE.md` wins on build details.

## Data pipeline (Python — the only runnable code today)

```bash
pip install openpyxl

# Extract the red-letter corpus from the public-domain WEB (downloads USFX from eBible.org)
python extract_red_letter_corpus.py --out jesus_full_red_letter.xlsx
python extract_red_letter_corpus.py --usfx engwebp_usfx.xml --out out.xlsx   # use local XML
python extract_red_letter_corpus.py --selftest

# Convert the annotated xlsx → build/{sft_style,rag_corpus,eval_heldout}.jsonl
python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/
python build_training_jsonl.py --eval-frac 0.10   # held-out split (deterministic by ID hash)
```

`build_training_jsonl.py` prints a ready/total split and the per-move distribution so
annotation progress is visible. It is verified working.

### The bottleneck is annotation, not code

`jesus_full_red_letter.xlsx` has **489 sayings (927 verse-rows)** but the `Modern
Rendering` and `Reasoning Move` columns are **blank**. A row is SFT-ready only when
*both* are filled — so `build/sft_style.jsonl` and `build/eval_heldout.jsonl` are
currently empty, while `build/rag_corpus.jsonl` (927 passages) is populated from original
text alone. **The RAG path needs no annotation; the style LoRA is blocked on it.** Gold
standard for annotation: the 44-entry seed + the M01–M18 rubric in
`jesus_sayings_dataset.xlsx`, and the 6 validated records in `sample_training_data.jsonl`.

### Two datasets, one sheet — keep them shaped differently

- **SFT record** (`training_data_spec.md` §2): OpenAI-style `messages`. The **fixed system
  prompt** (defined in `build_training_jsonl.py`) is identical at train and inference time.
  Original WEB text goes in the **user** turn; `Modern Rendering` is the **label** — so the
  adapter learns the *transform* (ancient→modern), never free generation. `Reasoning Move`
  is metadata only (stratified split + rare-move weighting), **never visible prompt text**.
- **RAG record** (§3): one passage per saying; index *both* `text_original` and
  `text_modern`; always cite `ref`.
- **Forbidden augmentation:** synthetic Q→A pairs where a model invents "Jesus answers."
  Permitted: multiple human-checked modern renderings of the *same real line*.

## Rust workflow — load the Rust skills

For **all** Rust work in this repo, load the Rust skills before writing or fixing code.
Start with `Skill(rust-skills:rust-router)` to get routing, then load the relevant Layer-1
module skill for the problem (e.g. `rust-skills:m01-ownership`, `rust-skills:m06-error-handling`,
`rust-skills:m07-concurrency`) **and** the matching Layer-3 domain skill — `domain-web` for the
Axum service/adapters, `domain-cli` for `jesus-twin-cli`, `domain-ml` for the inference/embedding
path. When a borrow/lifetime/Send-Sync error appears, trace up to the domain constraint and back
down to the pattern rather than just patching the compile error. Also use `rust-patterns` and
`rust-testing` for idioms and TDD. Use the **karpathy** skills to keep a running wiki and
incrementally improve the code.

## The planned Rust service (`ARCHITECTURE.md` — not yet built)

A single **Axum 0.8+** agent service, 7-crate Cargo workspace under `jesus-twin/`.
Dependency direction is strictly downward — `jesus-twin-core` knows nothing about HTTP/MCP/AG-UI/A2A:

- `jesus-twin-core` — agent core: the canonical `AgentEvent` stream, orchestrator
  (retrieve → coverage gate → generate → tool-loop), the refusal gate. No protocol, no I/O.
- `jesus-twin-inference` — wraps **mistral.rs as a library** (not a subprocess); Gemma 4 +
  Embedding Gemma; token stream → `AgentEvent`.
- `jesus-twin-store` — **SurrealDB 3.1 embedded** behind a `Store` trait; vector + graph +
  BM25 + RRF in one query; mindmap projections.
- `jesus-twin-skills` — one `Skill` registry → CLI + MCP server + model tool-list.
- `jesus-twin-admission` — Prometheus parking-lot gatekeeper (admit/queue/backpressure/503).
- `jesus-twin-api` — Axum app + four thin adapters: `openai`, `mcp`, `agui`, `a2a`.
- `jesus-twin-cli` — `serve` / `skill <name>` / `chat`.

**Build sequence — ship value at step 3** (grounded RAG over one surface; everything after
is surface area):
1. `jesus-twin-store` + load `build/rag_corpus.jsonl`; build vector/BM25 indexes + the
   move/parallels graph; verify the hybrid retrieval query.
2. `jesus-twin-inference` — embed mistral.rs; serve Gemma 4 E4B (base, **thinking off**) +
   Embedding Gemma.
3. `jesus-twin-core` — orchestrator + coverage gate. **Ship a RAG-first, base-model build
   here** (useful and safe with no fine-tune).
4. `jesus-twin-api` OpenAI adapter end-to-end (streaming).
5. `jesus-twin-admission` gatekeeper in front of the engine.
6. Remaining adapters (MCP stdio+HTTP, AG-UI, A2A) as thin event translations.
7. `jesus-twin-skills` + `jesus-twin-cli`.
8. Fine-tune (Unsloth LoRA) → merge → repoint `jesus-twin-inference`. Keep the LoRA only if
   it improves style-by-move without hurting grounding.

## Frontend / UI (React — not yet built)

All UI is **React 19 + Vite 8**, organized by **feature-based clean architecture**. Vite
projects are **built by `build.rs`** files in the owning Rust crate: each `build.rs` reads a
designated environment variable for the static output directory and falls back to a default
directory into which the built Vite bundle is emitted, so the Rust binary serves the
compiled frontend.

### React/TypeScript rules (hard requirements)

- **TypeScript 6, strong typing only.** No implicit or explicit `any`, ever.
- **kebab-case for every generated file name** (all languages).
- **No file over 500 lines.** At that size, make a directory and split into logical pieces.
- **shadcn-ui on base-ui, *not* radix.** (Use the `shadcn` MCP tools to add/inspect items.)
- **State management: Zustand + Immer.** Strict layering:
  - Components **never** talk to stores directly — they use **hooks**.
  - **Hooks** talk to stores. Hooks **never** talk directly to APIs or databases.
  - **Only stores** talk to APIs / databases / persistence.
- **No hidden business state in components** (principle 13) — state lives in stores / DB.
- **Entity data: `@prometheus-ags/prometheus-entity-management` for ALL entities.** Do **not**
  use TanStack Query. (See the `prometheus-entity-skills` skill suite + the `entity-*` skills.)
- **Client-side persistence: PGlite**, with **pgvector** for the client-side RAG agent, and
  **Drizzle ORM** over PGlite.
- For UI/UX generation, use the **ui-ux-designer / "ui ux pro max"** skills plus the Anthropic
  web-design and Vercel React guidance (`frontend-patterns`, `react-vite-stack`, `a11y-architect`).

## Cross-cutting tooling (memory + wiki)

- **Memory: the `surreal-memory` MCP server is the primary store.** **Recall before you
  code** — query it for prior decisions, fixes, and mistakes so you don't repeat them
  (principle 16). Create a memory after **each code fix or feature**, capturing what changed
  and why. Use the file-based memory under `~/.claude/projects/.../memory/` as a backup.
- **Wiki/learning: the karpathy skills** keep a running wiki and drive incremental code
  improvement.

## Gotchas that will bite (do not relearn the hard way)

- **Gemma 4 = merge, not adapter.** Runtime LoRA for Gemma 4 is unsupported across
  vLLM/SGLang/mistral.rs today. Always Unsloth-`save_pretrained_merged` the adapter and
  serve the merged checkpoint. Disable **thinking mode** for diction fidelity.
- **Candle version coupling is the sharp edge.** mistral.rs pulls its own candle. If any
  other crate touches candle, align the git revs (expect a `[patch]` redirect) or you'll
  get duplicate-candle / trait-mismatch build errors. Pin all fork revs with exact git
  revs in the workspace `Cargo.toml`.
- **Two schedulers, one boundary.** parking-lot = *admission control* only
  (`gatekeeper.admit(cost) -> Permit`, held for the generation). mistral.rs's internal
  engine owns micro-batching / PagedAttention / sampling. Never make parking-lot do
  per-token GPU scheduling.
- **One agent core, four thin adapters.** Don't implement four agent loops. One canonical
  `AgentEvent` stream (modeled on AG-UI's vocabulary); OpenAI/MCP/A2A are lossy projections.
- **Persona ≠ permission.** "In character for Jesus" is not authorization. Tool execution
  goes through a deterministic, risk-classified, human-checkpointed authz layer,
  independent of the persona. Irreversible/outbound actions require human approval.
- **Embedded vs. scalable are different deployments.** Embedded SurrealDB + in-process
  mistral.rs contend for RAM (acute on Mac unified memory). Keep `Store` behind a trait so
  it can move to a remote node.
- **Bible scope is deliberate, not "ingest everything."** Red-letter = train + RAG core;
  Hebrew Bible = *his source material* (a labeled tool, not his words); Gospel narrative =
  attestation-flagged context KB; **Epistles/Acts/Revelation = excluded from the persona**
  (quarantine at most). More Bible ≠ more Jesus-the-man.
- **Attestation tiering is contested scholarship.** Make it source-cited and revisable;
  don't hardcode one school's verdict as truth.
- **Custom AG-UI chunks** (`CITATION`, `ATTESTATION`, `REASONING_MOVE`, `SOURCE_TEXT`,
  `INTERPRETATION_FLAG`, `MINDMAP_DELTA`) must be additive + namespaced (e.g.
  `x-jesus-twin/…`) so standard clients ignore what they don't understand.

## External forks & dependencies (the user's, under `/Users/gqadonis/Projects/references/`)

| Component | Path / repo | Role |
|---|---|---|
| **mistral.rs** | `references/baseline/mistral.rs` · `github.com/GQAdonis/mistral.rs` | Inference engine, **as a library**. Has Gemma 4, `gemma4`/`gemma4_strict` tool parsers, LoRA/X-LoRA, embeddings, `mistralrs-server-core`. |
| **prometheus-parking-lot-rs** | `github.com/Prometheus-AGS/prometheus-parking-lot-rs` | Admission control / backpressure. |
| **candle (fork)** | `github.com/GQAdonis/candle` | ML backend under mistral.rs — the version-coupling sharp edge. |
| **Unsloth** | `references/unsloth` | Fine-tuning (Python, offline). Day-0 Gemma 4 support; `save_pretrained_merged`. |
| **candle-vllm (fork)** | `references/candle-vllm` | **Not** used for the twin (no Gemma 4, no PEFT LoRA). Keep only for parking-lot scheduler infra ideas. |
| **SurrealDB** | `surrealdb` crate v3.1 | Embedded store. Confirm HNSW + BM25 + `search::rrf` are enabled in selected crate features. |

References are reachable via the filesystem MCP server (Desktop Commander).

## Alignment stance (non-negotiable, and a behavioral contract — not a data filter)

Historical-critical, **not** religious: the man and how he thought, no theological
interpretation beyond what he himself said and did. The twin neither preaches nor debunks.
Because every source about Jesus is itself a later faith document, it is *epistemically
humble*: confidence tracks attestation, it flags the man's own words vs. later
interpretation (`INTERPRETATION_FLAG`), and it **refuses out-of-corpus questions** via the
coverage gate rather than confabulating. The honesty (citations, attestation tiers,
refusal-on-no-coverage) is the product, not garnish.
