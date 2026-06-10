# Change: gate-calibration

**Phase:** build-agent · Wave 1 (first) · **Backend:** OpenSpec
**Spec source:** `docs/gate-calibration-claude-code-prompt.md` (authoritative; PMPO phases).

## Why

The coverage gate is structurally disabled: `DEFAULT_COVERAGE_THRESHOLD = 0.0` passes any
non-empty retrieval, because RRF scores are strictly positive. An out-of-corpus question was
answered silently from thematically-adjacent passages at fused score ~0.016. This is a live
bright-line violation (the twin must refuse out-of-corpus questions, not confabulate). It also
blocks the general-mentor work: the three-tier grounding router needs a calibrated gate.

## What

Replace the binary gate with a **leg-agreement three-outcome gate** (Tier 1 ≥2 legs agree →
answer; Tier 2 = 1 leg → answer with a low-confidence signal + in-voice "decline what the
passages don't cover"; Tier 3 = empty → refuse). Gate on how many of the 4 RRF legs agree,
not raw score — robust to the annotation program reviving the currently-dead modern legs.
Fix `eval/retrieval.jsonl`'s unreachable `min_score` in the same change. Emit
`x-jesus-twin/low-confidence`. SYSTEM_PROMPT untouched (Tier-2 addendum is context-assembly).

## Status

- **Assess:** findings verified in source; calibration instrument built (`gate calibrate` CLI
  + `SurrealStore::calibrate_query`). **The empirical 4-leg run is blocked on a GPU host** —
  see `docs/HANDOFF-gate-calibration.md`. PMPO **HALT** here.
- **Plan / Execute / Reflect:** pending the GPU run + preregistered rule (doc governs).

## Impact

- Specs: extends `grounded-generation` (gate tiers) — delta authored at Plan phase.
- Code (Execute): `jesus-twin-core/src/gate.rs` (three-tier type), `orchestrator.rs` (Tier-2
  path + chunk), `jesus-twin-store/src/{surreal,retrieve}.rs` (`legs_matched` to production),
  `eval/retrieval.jsonl` (rescale), new `gate-calibration` eval facet.
- Already shipped (Assess, read-only): the calibration instrument + 3 unit tests.
