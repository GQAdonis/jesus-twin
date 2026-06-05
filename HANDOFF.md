# Project Handoff — Jesus Digital Twin (Rust)

Handoff brief for continuing this project in Claude Code. It captures the goal, every
decision made so far, what exists in the repo, the external forks, the build plan, and
the constraints that will bite if forgotten. Read this first; the linked docs are the
detail.

> **Tip:** this file can seed a `CLAUDE.md` for the Rust workspace once you scaffold it.
> Keep §2 (decisions) and §6 (gotchas) in whatever Claude Code reads automatically.

---

## 1. What this project is

A study-aid "digital twin" that renders the recorded teachings of Jesus of Nazareth in
present-day English, preserves his reasoning patterns (the `M01–M18` move rubric), and
**never fabricates sayings** — retrieval grounds every answer in a cited verse.

**Alignment stance (non-negotiable):** historical-critical, *not* religious. The man and
how he thought — no theological interpretation beyond what he himself said and did. The
twin neither preaches nor debunks. Because every source about Jesus is itself a later
faith document, the twin is *epistemically humble*: confidence tracks attestation, it
flags interpretation vs. the man's own words, and it refuses out-of-corpus questions.
Full reasoning in [`ALIGNMENT_AND_TUNING.md`](./ALIGNMENT_AND_TUNING.md).

**The one rule that governs everything:** retrieval owns *truth*, the fine-tune owns
*voice*, the agent layer owns *stance/honesty*. The model is never asked to generate
doctrine — its worst case is a paraphrase of a cited line.

---

## 2. Decisions already made (don't relitigate without reason)

| Area | Decision | Where / why |
|---|---|---|
| **Base model** | **Gemma 4 E4B (Instruct)**, thinking mode **off**. Qwen3-4B is the proven fallback. 26B-A4B MoE if a 24 GB GPU is available. | README §1 — Apache-2.0, native tool calling, runs small |
| **Fine-tuning** | **LoRA via Unsloth** (style + M01–M18 moves), **merged** into base for serving | README §3; runtime LoRA for Gemma 4 is unsupported ecosystem-wide → must merge |
| **Instruction tuning** | **No separate stage.** Blend a small in-domain instruction set into the same LoRA SFT mix | ALIGNMENT_AND_TUNING §1 — avoids emergent misalignment on tiny corpus |
| **Preference alignment** | **DPO optional, later** (grounded≻ungrounded, refuse≻fabricate), gentle LR | ALIGNMENT_AND_TUNING §1 |
| **Inference engine** | **mistral.rs (GQAdonis fork) as a library** | Has Gemma 4 + `gemma4.rs`/`gemma4_strict.rs` tool parsers + LoRA/X-LoRA + embeddings; candle-vllm fork does **not** support Gemma 4 |
| **Store / RAG** | **SurrealDB 3.1 embedded**, behind a `Store` trait (vector + graph + BM25 + RRF in one query) | README §2, ARCHITECTURE §7 — gives graphrag + mindmap; portable to pgvector later |
| **Embeddings** | **Embedding Gemma** (or Qwen3-Embedding) via the same mistral.rs runtime | No separate embedding service |
| **Service shape** | Custom **Axum 0.8+** app, mistral.rs as lib, **Prometheus parking-lot** for admission control | ARCHITECTURE §2, §6 |
| **Surfaces** | OpenAI REST, MCP (stdio + streamable HTTP), AG-UI (+ custom chunks), A2A; CLI | ARCHITECTURE §4, ALIGNMENT_AND_TUNING §4 |
| **Skills** | One `Skill` registry → CLI + MCP server + model tool-list | ARCHITECTURE §8 |
| **Bible scope** | Red-letter = train+RAG core. Hebrew Bible = **source tool** (his cited material). Gospel narrative = attestation-flagged context KB. **Epistles/Acts/Revelation = excluded from persona** (quarantine at most) | ALIGNMENT_AND_TUNING §5 |

---

## 3. Repo inventory (`/Users/gqadonis/Projects/bible`)

**Authoritative docs (read in this order):**
- [`README.md`](./README.md) — overview, model + DB choices, Rust stack, sources.
- [`ARCHITECTURE.md`](./ARCHITECTURE.md) — the build spec: crate layout, `AgentEvent`
  model + 4-adapter mapping, mistral.rs-as-library, parking-lot/engine boundary,
  SurrealDB graphrag schema (SurrealQL), deployment modes, build sequence.
- [`ALIGNMENT_AND_TUNING.md`](./ALIGNMENT_AND_TUNING.md) — tuning-layer roles, the
  non-religious alignment, skills/MCP tool safety, AG-UI chunks, Bible-scope verdict.
- [`training_data_spec.md`](./training_data_spec.md) — SFT/RAG JSONL formats from the
  12-column annotation sheet + M01–M18 rubric.

**Data + code:**
- `jesus_full_red_letter.xlsx` — 489 sayings (927 verse-rows), 12-column schema.
  **Annotation columns (`Modern Rendering`, `Reasoning Move`) are still blank** — this is
  the real bottleneck (see §5).
- `jesus_sayings_dataset.xlsx` — 44-entry annotated seed + the **M01–M18 Reasoning Move
  Rubric** (the labeling guide; this is the canonical rubric).
- `build_training_jsonl.py` — converts the xlsx → `build/{sft_style,rag_corpus,eval_heldout}.jsonl`.
  Verified working; correctly reports 0 training-ready rows until annotation is done.
- `sample_training_data.jsonl` — 6 hand-built, validated example records (the target format).
- `extract_red_letter_corpus.py` — WEB red-letter extractor (see `DATA_EXTRACTION.md`).
- `build/` — generated JSONL (RAG corpus populated; SFT empty pending annotation).

**Provenance note:** `digital_twin_architecture.md` is the earlier, Python-era conceptual
doc (RAG+LoRA hybrid, with a Mermaid diagram). It's still conceptually valid but
**`ARCHITECTURE.md` is the authoritative Rust build spec** — prefer it on any conflict.

---

## 4. External forks & dependencies (the user's, on GitHub + local)

| Component | Repo / path | Role | Notes |
|---|---|---|---|
| **mistral.rs** | `github.com/GQAdonis/mistral.rs` (local: `/Users/gqadonis/Projects/references/baseline/mistral.rs`) | Inference engine, as a **library** | Has Gemma 4, `gemma4`/`gemma4_strict` tool parsers, LoRA/X-LoRA, embeddings, `mistralrs-server-core` (embeddable in Axum) |
| **prometheus-parking-lot-rs** | `github.com/Prometheus-AGS/prometheus-parking-lot-rs` | Admission control / backpressure | Already used by the candle-vllm fork |
| **candle (fork)** | `github.com/GQAdonis/candle` | ML backend under mistral.rs | **Version coupling is the sharp edge** — see §6 |
| **candle-vllm (fork)** | `/Users/gqadonis/Projects/references/candle-vllm` | *Not* used for the twin | No Gemma 4 support, no PEFT LoRA; keep only if its parking-lot scheduler/queue infra is wanted later |
| **Unsloth** | `/Users/gqadonis/Projects/references/unsloth` | Fine-tuning (Python, offline) | Day-0 Gemma 4 support; `save_pretrained_merged` → serve merged checkpoint |
| **SurrealDB** | `surrealdb` crate, v3.1 | Embedded store | Confirm HNSW + BM25 + `search::rrf` are enabled in selected crate features |

These are accessible via the filesystem MCP server (Desktop Commander) under
`/Users/gqadonis/Projects/references/...`.

---

## 5. Where things stand & the immediate critical path

**Status:** design + data-pipeline scaffolding complete; no Rust code written yet; corpus
extracted but **not annotated**.

**The bottleneck is annotation, not code.** `build_training_jsonl.py` reports 0
training-ready rows because `Modern Rendering` and `Reasoning Move` are blank in
`jesus_full_red_letter.xlsx`. Nothing downstream of the style LoRA can proceed until
these are filled (target: a few hundred rows minimum; use the 44-entry seed +
M01–M18 rubric as the gold standard). **The RAG path does not need annotation** — it can
ship on original text alone.

**Recommended first moves in Claude Code (from ARCHITECTURE §11 / ALIGNMENT §6):**
1. Scaffold the `jesus-twin/` Cargo workspace (7 crates per ARCHITECTURE §2). Pin fork revs.
2. `jesus-twin-store`: load `build/rag_corpus.jsonl` into SurrealDB; build vector + BM25
   indexes + the move/parallels graph; verify the hybrid retrieval query.
3. `jesus-twin-inference`: embed mistral.rs as a lib; serve Gemma 4 E4B (base, thinking off) +
   Embedding Gemma; map output → `AgentEvent`.
4. `jesus-twin-core`: orchestrator (retrieve → coverage gate → generate) + the event stream.
   **Ship a RAG-first, base-model build here** — useful and safe with no fine-tune.
5. `jesus-twin-api` OpenAI adapter end-to-end (streaming).
6. `jesus-twin-admission` (parking-lot gatekeeper in front of the engine).
7. Remaining adapters (MCP server, AG-UI, A2A) as thin event translations.
8. `jesus-twin-skills` + `jesus-twin-cli`.
9. In parallel/independently: finish annotation → run `build_training_jsonl.py` → Unsloth
   LoRA → merge → point `jesus-twin-inference` at the merged checkpoint. Keep the LoRA only if
   it improves style-by-move without hurting grounding.

Ship value at step 4. Everything after is surface area and polish.

---

## 6. Gotchas that will bite (carry these into CLAUDE.md)

- **Gemma 4 = merge, not adapter.** Runtime LoRA loading for Gemma 4 is unsupported
  across vLLM/SGLang/mistral.rs today (KV-sharing aliases + the multimodal class doesn't
  declare `SupportsLoRA`). Always Unsloth-merge the adapter and serve the merged model.
- **Candle version coupling.** mistral.rs pulls its own candle; the candle-vllm fork
  needed a `[patch]` redirect. If anything else in the workspace touches candle, align the
  git revs or you'll get duplicate-candle / trait-mismatch build errors.
- **Two schedulers, one boundary.** parking-lot = *admission control* (admit/queue/
  backpressure/503). mistral.rs internal engine = *micro-batching/PagedAttention/
  sampling*. Never make parking-lot do per-token GPU scheduling. Contract:
  `gatekeeper.admit(cost) -> Permit`, held for the generation.
- **One agent core, four thin adapters.** Don't implement four agent loops. One canonical
  `AgentEvent` stream (shaped like AG-UI's); OpenAI/MCP/A2A are projections.
- **Persona ≠ permission.** "In character for Jesus" is not authorization. Tool execution
  goes through a deterministic, risk-classified, human-checkpointed authz layer,
  independent of the persona. Irreversible/outbound actions require human approval.
- **Embedded vs. scalable are different deployments.** Embedded SurrealDB + in-process
  mistral.rs contend for RAM (acute on Mac unified memory). Keep the `Store` behind a
  trait so it can move to a remote node; decide edge-single-binary vs. scaled-service.
- **Attestation tiering is a contested scholarly judgment.** Make it source-cited and
  revisable; don't hardcode one school's verdict as truth.
- **Custom AG-UI chunks** (`ATTESTATION`, `INTERPRETATION_FLAG`, `CITATION`,
  `REASONING_MOVE`, `SOURCE_TEXT`, `MINDMAP_DELTA`) must be additive + namespaced so
  standard clients ignore them.

---

## 7. Open decisions / risks to resolve

- **Deployment target:** edge single-binary vs. scaled service (drives whether SurrealDB
  stays embedded). Affects memory budget on Mac.
- **Annotation labor:** who/what fills `Modern Rendering` + `Reasoning Move` for ~489
  sayings, and whether to allow 2–3 paraphrase registers per saying (permitted
  augmentation) — see `training_data_spec.md` §4.
- **DPO data:** if/when you do preference alignment, where the preference pairs come from.
- **Hebrew Bible source ingestion:** which translation (public-domain), and the tool
  schema for "his cited sources."
- **A2A / AG-UI Rust SDK maturity:** community-tier; vendor behind adapter modules, expect
  spec churn (`ra2a` for A2A, AG-UI Rust SDK).
- **Eval harness:** broad eval (not just task metrics) to catch emergent misalignment;
  grounding/entailment check is the top gate.

---

## 8. Environment facts

- **Repo:** `/Users/gqadonis/Projects/bible` (git; latest commits are extractor fixes).
- **Forks/references:** `/Users/gqadonis/Projects/references/...` (mistral.rs under
  `baseline/`, candle-vllm at top level, unsloth at top level).
- **Python pipeline:** `pip install openpyxl`; `python build_training_jsonl.py`.
- **Doc map:** README → ARCHITECTURE → ALIGNMENT_AND_TUNING → training_data_spec;
  DATA_EXTRACTION for the corpus extractor; digital_twin_architecture.md is the
  Python-era predecessor (superseded by ARCHITECTURE.md for build details).
