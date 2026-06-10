# Current Waypoint

**Phase:** build-agent
**Change:** Wave 1 — gate-calibration first
**Status:** planned (plan.md Amendment 1, 2026-06-09)
**Last updated:** 2026-06-09

## The plan (automation-first restructure)

**Goal:** the twin answers ANY life question — grounded and authentic — via the three-tier
grounding router. **Premise correction (gate-calibration doc):** the gate is currently
*disabled* (`DEFAULT_COVERAGE_THRESHOLD = 0.0`) — out-of-corpus questions are answered
silently. Wave 1 fixes that live bright-line violation first.

```
WAVE 1 (pure dev, zero human work): 10a gate-calibration (leg-agreement tiers, PMPO halts,
        per docs/gate-calibration-claude-code-prompt.md) · 19 modern-legs-v1 (doc2query
        expansion, retrieval-only) · 12 hebrew-bible · 15 eval tier benchmark · guide fix
WAVE 2 (zero human work): 11 principle-index-v1 (machine-tagged) · 10b principle-tier
        (T2 principle-bridging) · 13 gospel-context-kb (auto attestation v1) · 17 memory
        ── "relatively close" checkpoint: any life question, real citations, no human hours ──
WAVE 3 (the hard manual work): 14 annotation-300 · tag/draft review-and-promote ·
        16 retrain-dual-base (human rows only) · 18 honesty-surface-ui · scholarly attestation
```

**Safety rule (Waves 1–2):** machine-generated text steers *retrieval only* — never
displayed (display = `text_original`), never trained (SFT = human-verified xlsx rows only).

## Next action

Start `gate-calibration` Phase 1 (Assess): verify the doc's four findings against current
source, build the `gate calibrate` CLI instrument, run the 95-query calibration, report
distributions — then HALT for approval per the PMPO discipline in the doc.

## Blockers

- `retrain-dual-base` ← Wave 3 (≥300 human-verified rows; machine drafts never train).
- `honesty-surface-ui` ← React UI does not exist yet.

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
