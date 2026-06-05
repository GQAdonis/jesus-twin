# Current Waypoint

**Phase:** build-agent  
**Change:** rag-prototype  
**Status:** in_progress  
**Last updated:** 2026-06-05

## Next action

Implement Change 1: `rag-prototype` — RAG-First Grounded Answer Engine.

**Parallel opportunity:** `annotation-guide` can start simultaneously (no shared dependency).

## Blockers

- Annotation blocked (0 SFT rows) — unblocks after `annotate-50` completes, not needed for `rag-prototype`
- `rag-prototype` and `annotation-guide` have no blockers

## Progress

| Change | Status |
|--------|--------|
| `rag-prototype` | pending |
| `annotation-guide` | pending |
| `annotate-50` | blocked (needs annotation-guide) |
| `mentor-examples` | blocked (needs annotation-guide) |
| `lora-train` | blocked (needs annotate-50 + mentor-examples) |
| `eval-suite` | blocked (needs annotate-50) |
| `hebrew-bible` | blocked (needs rag-prototype) |
| `production-lora` | blocked (needs lora-train + eval-suite) |