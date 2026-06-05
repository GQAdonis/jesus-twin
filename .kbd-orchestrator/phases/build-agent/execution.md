# Execution Dispatch — build-agent

**Phase:** build-agent  
**Date:** 2026-06-05  
**Backend:** hybrid (OpenSpec detected, native execution for immediate changes)

## Backend Selection

OpenSpec is detected (`openspec/` directory, `spec-driven` schema). However, no OpenSpec changes exist yet. Changes are executed natively with progress tracked in `progress.json`. OpenSpec changes should be created for traceability as a follow-up.

## Wave Plan

### Wave 1 (parallel)

| Change | Type | Executor | Status |
|--------|------|----------|--------|
| `rag-prototype` | Code verification | AI agent | pending |
| `annotation-guide` | Documentation | AI agent | pending |

### Wave 2 (serial, after annotation-guide)

| Change | Type | Executor | Status |
|--------|------|----------|--------|
| `annotate-50` | Human annotation | Human | blocked |
| `mentor-examples` | Human writing | Human | blocked |

### Wave 3 (after Waves 2 complete)

| Change | Type | Executor | Status |
|--------|------|----------|--------|
| `lora-train` | GPU training + Rust | AI agent + Human | blocked |
| `hebrew-bible` | Code + corpus | AI agent | blocked |
| `eval-suite` | Code | AI agent | blocked |

### Wave 4 (after Waves 3 complete)

| Change | Type | Executor | Status |
|--------|------|----------|--------|
| `production-lora` | GPU training + deployment | AI agent + Human | blocked |

## Dispatch Contract

1. AI agent executes `rag-prototype` and `annotation-guide` in parallel (Wave 1)
2. Human executes `annotate-50` and `mentor-examples` (Wave 2)
3. AI agent executes `lora-train`, `hebrew-bible`, `eval-suite` when unblocked (Wave 3)
4. AI agent + Human execute `production-lora` when Waves 2-3 complete (Wave 4)

## Per-Change QA Gate

- `rag-prototype`: documentation-only (execution.md) → skip QA
- `annotation-guide`: documentation-only → skip QA
- Subsequent changes: run artifact-refiner QA when applicable