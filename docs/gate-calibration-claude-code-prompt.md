# Claude Code Session Prompt — Coverage Gate Calibration & Two-Tier Refusal

**Project:** Jesus Digital Twin — `/Users/gqadonis/Projects/bible`
**Workstream:** Gate calibration (feeds pre-planning Workstream 3: Coverage Gate + Refusal Eval Set; shares AG-UI confidence-chunk work with Workstream 4: Visible Honesty Surface)
**Discipline:** PMPO — Assess → Plan → Execute → Reflect. Halt at each phase boundary for explicit approval before proceeding. Phase violations are hard quality failures.
**Date authored:** 2026-06-09

---

## Context: findings from the source review (treat as Assess-phase input, verify before relying on)

A review of `jesus-twin/crates/jesus-twin-core/src/gate.rs`, `orchestrator.rs`, `jesus-twin-store/src/{surreal,retrieve}.rs`, and `eval/` produced four findings. **Verify each against the current code before planning — do not assume this summary is still accurate.**

### Finding 1 — The gate is structurally disabled

`DEFAULT_COVERAGE_THRESHOLD = 0.0` in `gate.rs`. `evaluate_set` refuses on empty sets (`NoCoverage`) but the `top_score >= threshold` branch passes any non-empty result, because RRF scores are strictly positive. `InsufficientAttestation` is dead code in production. The doc comment already marks the threshold "Provisional."

**Observed consequence:** an out-of-corpus modern-politics question retrieved five thematically-adjacent passages at fused score ~0.016 (vs. ~0.033 on-corpus baseline) and was answered silently instead of being refused or flagged.

### Finding 2 — RRF scores are leg-agreement counts in disguise

`retrieve_hybrid` fuses four legs (BM25-original, BM25-modern, vector-original, vector-modern) with RRF: `score = Σ 1/(60 + rank)`, rank 1-based, CANDIDATES=20 per leg. The arithmetic produces structurally interpretable bands:

| Agreement pattern              | Fused score        |
|--------------------------------|--------------------|
| #1 in exactly one leg          | 1/61 ≈ **0.0164**  |
| #1 in two legs                 | 2/61 ≈ **0.0328**  |
| Single-leg maximum possible    | 0.0164             |
| Two-leg minimum (both rank 20) | 0.0250             |

- ~0.016 ≈ one leg agreed (vector-only semantic adjacency, no lexical anchor)
- ~0.033 ≈ two legs agreed (lexical + semantic)
- There is a **guaranteed gap** between single-leg max (0.0164) and two-leg min (0.0250). A threshold in that band — e.g. 0.02 — encodes "refuse unless lexical and semantic retrieval independently agree." It is a semantically meaningful cut, not curve-fitting.

**Compounding factor:** with 0 of 927 rows annotated, `text_modern` is empty corpus-wide, so the two modern-register legs are dead. Realistic ceiling today is 2/61 ≈ 0.033 — the on-corpus baseline IS the current maximum. Any raw-score threshold calibrated now shifts meaning when annotation revives the modern legs.

### Finding 3 — `eval/retrieval.jsonl` asserts an unreachable score scale

Every retrieval test specifies `"min_score": 0.3`. The RRF ceiling is 4/61 ≈ 0.066 (0.033 today). No retrieval test can pass its score assertion even with perfect ranking. The eval was evidently written against a different scale (raw BM25 or cosine). Must be fixed in the same change that fixes the gate — one instrument, one scale.

### Finding 4 — A hard binary gate would break `method-application`

`method-application.jsonl` (15 tests, ≥80% must ENGAGE) contains personal modern-register questions ("I'm anxious about money") with near-zero lexical overlap with WEB diction. These may legitimately be vector-only (single-leg) matches today. A hard 2-leg gate could refuse them — the lazy-gate false positive the pre-planning refusal-set design explicitly warns against (near-miss questions where refusing is a FAIL).

---

## The target design (pending Assess-phase confirmation)

A **two-tier gate** replacing the binary pass/refuse:

```
Tier 1: legs_matched >= 2          → answer normally
Tier 2: legs_matched == 1          → answer WITH low-confidence signal:
                                      (a) emit x-jesus-twin/low-confidence AG-UI chunk
                                      (b) prompt addendum instructing: ground what the
                                          passages actually cover; explicitly decline,
                                          in voice, what they do not
Tier 3: empty set / stopword-only  → in-voice refusal (existing NoCoverage path)
```

**Gate on leg agreement, not raw RRF score** (Option A from the review). Rationale:

1. Deterministic and human-explainable ("two independent retrieval methods agreed").
2. Robust to every scheduled parameter change: k, CANDIDATES, leg count, and — critically — the annotation program reviving the modern legs. Raw-score thresholds (Option C, 0.02) require mandatory recalibration at the ~300-row annotation milestone; leg-agreement does not.
3. Feeds the honesty surface directly: "grounded by N of M retrieval methods" is a renderable confidence signal (Workstream 4).

**Interim guard (acceptable if approved at Plan phase):** raw threshold 0.02 as a one-hour stopgap with an `// INTERIM` comment pointing at this document, while Option A is built. State this tradeoff explicitly in the Plan; do not silently choose.

**Open question for Assess to resolve empirically:** whether BM25 found lexical anchors the arithmetic-based reading assumed it didn't (e.g., "Pharisees" appears in the red-letter corpus — "Beware of the leaven of the Pharisees"). The leg-rank data from the calibration run is the arbiter, not this summary.

---

## Phase 1 — ASSESS (halt for approval before Plan)

1. Read and confirm/correct the four findings against current source:
   - `jesus-twin/crates/jesus-twin-core/src/gate.rs`
   - `jesus-twin/crates/jesus-twin-core/src/orchestrator.rs`
   - `jesus-twin/crates/jesus-twin-store/src/surreal.rs` (esp. `retrieve_hybrid`, `rrf_fuse`, `rank_bm25`, `rank_vector`, the stopword strip in `Store::retrieve`)
   - `jesus-twin/crates/jesus-twin-store/src/retrieve.rs` (`Passage`, `RetrievalSet::top_score`)
   - `eval/README.md` and all six eval JSONL files
2. Build the calibration instrument: a `jesus-twin-cli` subcommand `gate calibrate` (preferred; Rust, hits the store directly) OR a Python script against a running store — justify the choice. It must run every query from `grounding.jsonl` (30), `refusal.jsonl` (30), `method-application.jsonl` (15), and `boundary.jsonl` (20) through `Store::retrieve` and log, per query: top fused score, per-leg rank of the top passage, legs_matched count, and the top-3 refs.
3. Run it against the production corpus DB. Report the four score/agreement distributions.
4. Confirm or refute: (a) the single-leg vs two-leg interpretation of 0.016/0.033; (b) whether any `method-application` queries are single-leg matches; (c) whether any `refusal.jsonl` queries reach two-leg agreement (these would be the genuinely hard cases).

**Deliverable:** an assessment document with the empirical distributions and a confirmed/corrected version of the four findings. HALT.

## Phase 2 — PLAN (halt for approval before Execute)

Preregister the decision rule BEFORE examining where individual queries land (borrowed from experimental science; choosing after seeing results invites motivated reasoning):

> **Tier boundaries are accepted iff:** 100% of `grounding.jsonl` queries land in Tier 1; ≥80% of `method-application.jsonl` queries land in Tier 1 or 2 (engage); ≥95% of `refusal.jsonl` queries land in Tier 2 or 3 (flagged or refused). If the empirical data makes these jointly unsatisfiable with leg-agreement alone, propose the minimal hybrid rule (e.g., leg-agreement + a score floor within Tier 2) and re-derive.

Plan must cover:
1. `RetrievalSet`/`Passage` changes to carry `legs_matched` (and per-leg provenance if cheap) out of `retrieve_hybrid`. Note the BM25-only fallback path (no embedder): define its tier semantics explicitly (likely: BM25-only top hit = Tier 2 by definition, since only one modality ran).
2. `CoverageGate` redesign: three-tier output type replacing `Result<(), RefusalReason>` — name the variants, keep `NoCoverage` semantics intact.
3. Orchestrator changes: Tier 2 path = new AG-UI custom chunk `x-jesus-twin/low-confidence` + prompt addendum. The addendum text must be added to `PROMPTS.md` and respect the SYSTEM_PROMPT parity invariant (00-theory §5): it is a per-turn context injection, NOT a SYSTEM_PROMPT edit — confirm this keeps train/inference parity untouched.
4. `eval/retrieval.jsonl` `min_score` rewrite to the chosen scale, same change.
5. New eval facet `gate-calibration` so future changes to k/CANDIDATES/legs re-run the calibration as a regression gate.
6. Test plan: unit tests for tier classification at the boundary values (1/61, 2/80, 2/61); the existing gate tests updated; an integration test that the Trump-class query (use refusal-set proxies, not the literal political question) lands in Tier 2 or 3.
7. Recalibration trigger documented: re-run `gate calibrate` when annotation crosses ~300 rows (modern legs go live). Under leg-agreement this should be a no-op confirmation, not a re-derivation — state that as the expected result.

**Deliverable:** the plan with preregistered rule, file-by-file change list, and test matrix. HALT.

## Phase 3 — EXECUTE

Implement per the approved plan. Constraints:
- Rust-first; no new dependencies without justification.
- SurrealDB 3.x syntax discipline applies to any new queries.
- Do not touch `SYSTEM_PROMPT` in any of its four synchronized locations (`prompt.rs`, `build_training_jsonl.py`, `ollama/Modelfile.jesus-twin`, `PROMPTS.md`) — the Tier-2 addendum is context-assembly only.
- Every commit message references the gap (#3 product signal: "tighten coverage gate / low-confidence flagging").

## Phase 4 — REFLECT

1. Re-run the full calibration + the affected eval suites (`refusal`, `method-application`, `grounding`, `retrieval`); report pass rates against the preregistered rule.
2. Record outcome in `docs/FINDINGS.md`: the empirical distributions, the chosen tier rule, the rejected alternatives (raw 0.02 threshold, normalized score) and why, and the scheduled recalibration trigger.
3. Surface tradeoffs honestly: name at least one cost of the chosen design (candidates: Tier 2 adds a prompt-assembly branch that must be eval-covered forever; leg-agreement granularity is coarse — only 3–5 distinct values — limiting future fine-grained confidence display; BM25-only fallback path has degraded tier semantics).
4. List what is stubbed or deferred (e.g., UI rendering of the low-confidence chunk belongs to Workstream 4 and is out of scope here — emission only).

---

## Definition of done

1. `gate calibrate` subcommand exists, runs against the corpus DB, and its output is committed as the baseline in `docs/FINDINGS.md`.
2. Two-tier (three-outcome) gate shipped; `DEFAULT_COVERAGE_THRESHOLD = 0.0` and the dead `InsufficientAttestation` branch are gone or repurposed with accurate doc comments.
3. `x-jesus-twin/low-confidence` chunk emitted on Tier 2 turns (verifiable via SSE curl + grep).
4. `eval/retrieval.jsonl` score assertions are satisfiable and passing.
5. Preregistered rule met: grounding 100% Tier 1; method-application ≥80% engage; refusal ≥95% flagged-or-refused.
6. Recalibration trigger documented for the ~300-row annotation milestone.

## Out of scope (do not drift)

- Rendering the low-confidence signal in any UI (Workstream 4).
- The annotation program itself (Workstream 1).
- Any retraining or LoRA work (Workstream 2).
- Changing retrieval legs, k, CANDIDATES, or RETRIEVE_LIMIT — calibrate the gate to the retriever as-is; retriever changes are a separate, eval-gated decision.
