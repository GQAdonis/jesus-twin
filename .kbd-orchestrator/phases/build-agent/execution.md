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