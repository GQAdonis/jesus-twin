# HANDOFF — Gate Calibration Run (GPU box)

**For:** continuing the `gate-calibration` change on a CUDA GPU host.
**Authored:** 2026-06-09 (from an arm64 macOS dev box with no GPU).
**Status:** Assess phase **partially complete** — four findings verified in source, the
calibration instrument is **built and committed**; the empirical RUN is blocked on this
machine and is your job. After the run, the change resumes at **Plan** (preregistered rule).

Read these first, in order:
1. `docs/gate-calibration-claude-code-prompt.md` — the authoritative change spec (PMPO phases,
   target design, preregistered rule, definition of done). This handoff does NOT replace it.
2. `.kbd-orchestrator/phases/build-agent/plan.md` → "Amendment 1" — where this change sits in
   the wave plan (Wave 1, first).
3. `.kbd-orchestrator/phases/build-agent/execution.md` → the gate-calibration Assess section
   — the verified findings.

---

## 1. Why the run didn't happen here

The dev box is arm64 macOS, no CUDA GPU, no embeddinggemma, no merged checkpoint. The real
**4-leg hybrid** retrieval needs the embeddinggemma embedder (vector legs); without it
`Store::retrieve` runs **BM25-only (1 leg)**. Calibrating a leg-agreement gate on a 1-leg path
measures the wrong thing — every query is single-leg by construction (confirmed: the BM25-only
smoke run put 100% of grounding/refusal/method-application at `1leg`). The discrimination the
gate keys on (1-leg vs 2-leg agreement) only appears once the vector + modern legs are live.

So the run must happen on the GPU host where the CUDA release already runs.

## 2. What is already done (committed)

- **Findings verified** against current source (`gate.rs`, `surreal.rs`, `retrieve.rs`,
  `eval/*`): gate disabled (`DEFAULT_COVERAGE_THRESHOLD = 0.0`); RRF leg-agreement bands hold;
  `text_modern` empty 0/927 (2 of 4 legs dead today); `eval/retrieval.jsonl` asserts an
  unreachable `min_score: 0.3` (RRF ceiling 0.066).
- **Calibration instrument built** (read-only; changes neither retrieval nor the gate):
  - `SurrealStore::calibrate_query()` + `CalibrationRow` (`jesus-twin-store/src/surreal.rs`) —
    runs the 4 legs, reports `top_legs_matched`, `live_legs`, `top_score`, `top3_ids`, `path`.
  - `jesus-twin gate calibrate` CLI subcommand (`jesus-twin-cli/src/main.rs`).
  - 3 unit tests pin the leg-agreement math; smoke-tested BM25-only end-to-end.
- This was NOT done (correctly — it's Execute-phase, gated on your Plan approval): the gate
  redesign itself (three-tier output type), the `legs_matched` plumbing through production
  `retrieve_hybrid`, the Tier-2 prompt addendum, the AG-UI low-confidence chunk, the
  retrieval.jsonl rescale.

## 3. Run it (the exact commands on the GPU box)

```bash
# 0. Pull this branch (or main, if merged).
git pull

# 1. Get the models if not present (embeddinggemma is license-gated — accept first).
HF_TOKEN=hf_xxx scripts/download-models.sh
#    Point JESUS_TWIN_MODEL at the base Gemma 4 (or a merged checkpoint) and
#    JESUS_TWIN_EMBED_MODEL at embeddinggemma if not using the HF default ids.

# 2. Ingest the corpus WITH embeddings into a persistent store (populates emb_original;
#    emb_modern stays empty until annotation revives text_modern).
cd jesus-twin
cargo run --release --features mistralrs -p jesus-twin-cli -- \
  ingest ../build/rag_corpus.jsonl --db ../twin.db

# 3. Run the calibration against that store — the REAL 4-leg path.
cargo run --release --features mistralrs -p jesus-twin-cli -- \
  gate calibrate --eval-dir ../eval --db ../twin.db \
  --out ../eval/out/gate-calibration.jsonl
```

Expected console output: one line per eval set with the `legs_matched` distribution, e.g.
`grounding n=30 legs_matched: [2leg=27 1leg=3]`. The per-query JSONL lands in
`eval/out/gate-calibration.jsonl` (fields: `set`, `eval_id`, `query`, `path`, `live_legs`,
`top_score`, `top_legs_matched`, `top3_ids`).

**Sanity checks before trusting it:**
- `path` must read `hybrid-4leg` (not `bm25-only`) — if it says bm25-only, the embedder didn't
  attach; check `--features mistralrs` and the model env vars.
- `live_legs` will be **2 today** (original BM25 + original vector; both modern legs dead).
  That is expected and correct — note it; it is why the ~300-row annotation milestone triggers
  a recalibration (doc §Plan item 7).

## 4. What to do with the results (resume the change)

1. **Report the four distributions** (grounding / refusal / method-application / boundary) and
   confirm or refute: single-leg vs two-leg interpretation; whether any method-application
   queries are single-leg (the doc's Finding 4 risk); whether any refusal queries reach
   two-leg agreement (the genuinely hard cases). Write this into the Assess deliverable. **HALT.**
2. **Plan phase** — preregister the tier rule BEFORE looking at where individual queries land:
   > accepted iff grounding 100% Tier 1; method-application ≥80% Tier 1-or-2; refusal ≥95%
   > Tier 2-or-3. If unsatisfiable with leg-agreement alone, propose the minimal hybrid
   > (leg-agreement + a score floor in Tier 2) and re-derive.
   File-by-file change list + test matrix per the doc. **HALT.**
3. **Execute / Reflect** per the doc. SYSTEM_PROMPT stays untouched in all four parity
   locations — the Tier-2 addendum is context-assembly only.

## 5. KBD process state (so any tool resumes cleanly)

- Active phase: `build-agent`. Active change: `gate-calibration` (Wave 1, first).
- `.kbd-orchestrator/current-waypoint.json` `next_action` points at this run.
- After the run + Plan, mark `gate-calibration` execute-ready in the waypoint and proceed
  with `/kbd-execute`. The rest of Wave 1 (`modern-legs-v1`, `hebrew-bible`, the eval tier
  benchmark, the annotation-guide fix) is GPU-independent and can proceed in parallel.

## 6. The bright line (do not cross, on any host)

Machine-generated text (the later `modern-legs-v1` doc2query drafts) may steer retrieval only —
never displayed (`context_lines` uses `text_original`), never trained (SFT = human-verified
xlsx rows only). The gate work here touches none of that, but the same rule governs the rest of
Wave 1.
