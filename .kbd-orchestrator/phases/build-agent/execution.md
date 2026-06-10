# Execution Dispatch — build-agent

**Phase:** build-agent  
**Date:** 2026-06-05  
**Backend:** hybrid (OpenSpec detected, native execution for immediate changes)

## Backend Selection

OpenSpec is detected (`openspec/` directory, `spec-driven` schema). However, no OpenSpec changes exist yet. Changes are executed natively with progress tracked in `progress.json`. OpenSpec changes should be created for traceability as a follow-up.

## Wave Plan (executed)

### Wave 1 (parallel, both complete)

| Change | Status | Notes |
|--------|--------|-------|
| `rag-prototype` | ✅ complete | Verified Rust compile; updated SYSTEM_PROMPT in prompt.rs; updated refusal_text() in openai.rs to in-voice variants per RefusalReason |
| `annotation-guide` | ✅ complete | Wrote `docs/annotation-guide.md` with method labels and rendering rules |

### Wave 2 (discovered already complete in background)

| Change | Status | Notes |
|--------|--------|-------|
| `annotate-50` | ✅ complete | `build/annotated_50_sft.jsonl` exists with 50 records; system prompts aligned to mentor voice |
| `mentor-examples` | ✅ complete | `build/l2_conversational_mentor.jsonl` exists with 25 records |

### Wave 3 (in progress)

| Change | Status | Notes |
|--------|--------|-------|
| `eval-suite` | ✅ complete | 6 JSONL files, 145 tests, `eval/run.py` runner, `eval/README.md` guide |
| `lora-train` | 🚧 in progress | `train_lora.py` written; needs GPU execution; `build/sft_merged.jsonl` ready (75 records) |
| `hebrew-bible` | 🚧 in progress | `ingest_tanakh.py` written; needs corpus fetch from sacred-texts.com |

### Wave 4 (blocked)

| Change | Status | Notes |
|--------|--------|-------|
| `production-lora` | ⛔ blocked | Depends on lora-train completion and eval results |

## Critical Issues Resolved

### System prompt inconsistency

The four sources of the system prompt were inconsistent before this session:

- `prompt.rs` (Rust serving) — had original "study aid" voice
- `build_training_jsonl.py` (Python training data generator) — had "study aid" voice
- `build/annotated_50_sft.jsonl` (SFT rendering data) — had "study aid" voice
- `build/l2_conversational_mentor.jsonl` (L2 mentor data) — had "conversational mentor" voice (the new VISION.md voice)

This was a **training/serving drift risk**: if the LoRA trained on the study-aid voice but the orchestrator served the mentor voice, the model would be out of distribution at inference time.

**Fix applied:** aligned all four sources to the same "conversational mentor" voice (the VISION.md persona).

## Dispatch Contract

1. AI agent executes Wave 1 (rag-prototype, annotation-guide) — ✅ done
2. Annotation work (annotate-50, mentor-examples) was already in the repo — ✅ done
3. AI agent executes eval-suite (data + runner) — ✅ done
4. Human executes lora-train on GPU box (Colab/Kaggle/local) — pending
5. AI agent + Human execute hebrew-bible corpus preparation — pending
6. AI agent + Human execute production-lora when Wave 3 complete — blocked

## Per-Change QA Gate

- `rag-prototype`: code change, 2 files modified — skipped QA per rule (< 3 files)
- `annotation-guide`: documentation-only — skipped QA per rule
- `eval-suite`: 8 files created — candidate for QA, but data files are not "artifacts" in the refiner sense
- `lora-train`: pending GPU run

## Artifacts Not In Plan But Discovered

- `docs/policy-spec.md` — 938-line safety policy spec (complements the work)
---

## Execution — fix-context-attribution (appended 2026-06-09)

**Backend:** `openspec` (change `fix-context-attribution`). Single-author Claude Code edits.

### Dispatch
Stop retrieved RAG passages reading as user speech. Tasks per
`openspec/changes/fix-context-attribution/tasks.md`.

Edit set:
1. `jesus-twin-core/src/prompt.rs` — SYSTEM_PROMPT provenance clause + `assemble_context`
   provenance-framed instruction line + regression test.
2. Parity mirrors: `build_training_jsonl.py`, `ollama/Modelfile.jesus-twin`, `PROMPTS.md`.
3. `jesus-twin-inference/src/mistral.rs:92` — question-first, passages-last.
4. `jesus-twin-inference/src/mock.rs` — mirror order; keep tests green.

QA gate: `cargo fmt` + `cargo clippy -D warnings` + `cargo test`. artifact-refiner optional
(prompt-string change, one test).

---

## Execution — Wave 1 / gate-calibration ASSESS phase (2026-06-09)

**Backend:** openspec (change `gate-calibration`). PMPO discipline per
`docs/gate-calibration-claude-code-prompt.md`: Assess → HALT → Plan → HALT → Execute → Reflect.

### Step 1 — four findings VERIFIED against current source (all confirmed)

1. **Gate disabled — CONFIRMED.** `gate.rs:14` `DEFAULT_COVERAGE_THRESHOLD = 0.0`;
   `evaluate_set` (`:33`) passes any non-empty set because RRF scores are strictly positive
   (`rrf_fuse` `surreal.rs:297`, `score = Σ 1/(60+rank) > 0`). `InsufficientAttestation`
   branch (`:40`) is unreachable in production. Doc comment already says "Provisional."
2. **RRF = leg-agreement bands — CONFIRMED.** `retrieve_hybrid` (`surreal.rs:107`) fuses 4
   legs (ft_o, ft_m, v_o, v_m), CANDIDATES=20, K=60. Arithmetic bands hold. **Compounding
   factor CONFIRMED empirically: text_modern is 0/927 non-empty** → the two modern-register
   legs (ft_m, v_m) are dead → realistic ceiling 2/61 ≈ 0.033 today, exactly as the doc states.
3. **eval/retrieval.jsonl unreachable scale — CONFIRMED.** All 30 tests assert
   `"min_score": 0.3`; RRF ceiling is 4/61 ≈ 0.066 (0.033 today). No test can pass its score
   assertion. Must be rescaled in the gate change.
4. **Hard binary gate breaks method-application — PLAUSIBLE, pending the run.** 15 tests,
   modern-register personal questions; whether they are single-leg matches is the empirical
   question the calibration run must answer.

### Step 2 — calibration instrument: BLOCKER surfaced (HALT for decision)

The doc's instrument ("run 95 queries through `Store::retrieve`, log per-leg ranks +
legs_matched") has two unmet dependencies on THIS host:

- **No CUDA GPU / no embeddinggemma / no merged checkpoint** (host is arm64 macOS; HF cache
  has only base gemma-4-E4B). The real 4-leg hybrid path needs the GPU embedder; without it
  `Store::retrieve` falls back to **BM25-only (1 leg)** — calibration of a 4-leg gate on a
  1-leg path would be meaningless. MockEmbedder exists but its lexical-hash vectors don't
  reflect real semantic adjacency.
- **No corpus DB on disk** — needs an ingest run (cheap; gated on the above).
- **API visibility:** `rank_bm25`/`rank_vector`/`rrf_fuse` are private to `surreal.rs` and
  `rrf_fuse` collapses leg membership to a scalar. Logging `legs_matched` requires exposing
  per-leg provenance — which is Execute-phase production change, not Assess instrumentation.

**Conclusion of Assess:** the four findings are confirmed and the design is sound, but the
empirical calibration RUN (Step 3–4) requires the GPU box the release runs on. Options put to
the owner before proceeding. HALT.

### Step 2b — calibration instrument BUILT (Assess deliverable, committed)

- `SurrealStore::calibrate_query()` + `CalibrationRow` (jesus-twin-store/src/surreal.rs):
  read-only; runs the 4 legs, reports top_legs_matched / live_legs / top_score / top3_ids /
  path. 3 unit tests pin the leg-agreement math (2-leg=2/61, 1-leg=1/61, dead-leg live count).
- `jesus-twin gate calibrate` CLI subcommand (jesus-twin-cli/src/main.rs): runs the 4 eval
  sets (handles both `user_query` and `query` keys), writes a JSONL report + prints per-set
  distributions. Warns + degrades to bm25-only without --features mistralrs.
- Verified: default build compiles; 14 store tests pass (3 new); clippy -D warnings clean
  (fixed a PRE-EXISTING dead-code blocker, `build_orchestrator_mock`, with #[allow] + note —
  not introduced by this change).
- Smoke run (BM25-only, this host): instrument works end-to-end. As expected on a 1-leg path,
  grounding/refusal/method-application = 100% single-leg; boundary already differentiates
  (12/20 retrieve nothing → would refuse). Confirms top_score=0.0164 = single-leg (doc
  arithmetic). NOT the real baseline — the 4-leg run is the GPU box's job.

### HALT — handoff written

`docs/HANDOFF-gate-calibration.md` gives the GPU operator the exact ingest + calibrate
commands, sanity checks, and how to resume at Plan with the preregistered rule. PMPO halt
respected: the gate REDESIGN (three-tier type, production legs_matched plumbing, Tier-2
addendum, AG-UI chunk, retrieval.jsonl rescale) is deferred to Execute, after the GPU run
and Plan approval.
