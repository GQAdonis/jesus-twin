# Current Waypoint

**Phase:** build-agent
**Change:** hebrew-bible (next candidate)
**Status:** fix-context-attribution archived
**Last updated:** 2026-06-09

## Just completed

`fix-context-attribution` — **archived** to
`openspec/changes/archive/2026-06-09-fix-context-attribution/`. Capability spec
`grounded-generation` synced to `openspec/specs/` (validates). Live model re-run
operator-confirmed at archive time.

## Next action

No active OpenSpec changes. Candidates:
- `hebrew-bible` — Tanakh source tool (depends on `rag-prototype`, complete); `ingest_tanakh.py`
  already drafted.
- Resume annotation toward the ≥300-row gate that unblocks the deferred `lora-train`.

## Blockers

- None active. `lora-train` deferred (data gate); `production-lora` blocked on it.

## Progress

| Change | Status |
|--------|--------|
| `rag-prototype` | complete |
| `annotation-guide` | complete |
| `annotate-50` | complete |
| `mentor-examples` | complete |
| `eval-suite` | complete |
| `fix-context-attribution` | planned ← **next** |
| `lora-train` | deferred (assessment: gate on ≥300 annotated rows; lr 2e-4×3 on 75 collapsed) |
| `hebrew-bible` | pending (needs rag-prototype) |
| `production-lora` | blocked (needs lora-train + eval-suite) |
