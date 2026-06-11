# Plan — gate-calibration (PMPO Phase 2)

**Spec:** `docs/gate-calibration-claude-code-prompt.md` (authoritative).
**Inputs:** Assess deliverable in `.kbd-orchestrator/phases/build-agent/execution.md` (Step 3–4);
empirical report `eval/out/gate-calibration.jsonl` (95 rows, real 4-leg path, L4, 2026-06-10).
**Status:** PLAN — preregistered rule applied, re-derivation done. **HALT for owner decision (§4).**

## 1. Preregistered acceptance rule (fixed from spec §Phase 2 — NOT chosen after seeing landings)

> Tier boundaries are accepted **iff**: 100% of `grounding` land in Tier 1; ≥80% of
> `method-application` land in Tier 1-or-2 (engage); ≥95% of `refusal` land in Tier 2-or-3
> (flagged/refused). If jointly unsatisfiable with leg-agreement alone, propose the minimal
> hybrid (leg-agreement + a score floor) and re-derive.

Only aggregate distributions were inspected before fixing this rule; per-query scores were
examined **only** during the re-derivation below, as the rule authorizes.

### 1a. Owner amendment (2026-06-10, PMPO option C — relax the criterion)

The original refusal ≥95% target is unsatisfiable on the (legs, RRF-score) signal (§2). The owner
amended the preregistered rule. **Amended acceptance criteria, by leg-count class** (preserves the
bright line for the documented failure class; makes the hard residual an explicit owner judgment):

1. **grounding** — 100% **engage** (Tier 1-or-2); ≥90% Tier 1. The ≤10% genuinely semantic-only
   grounding queries land in Tier 2 honest-hedge — that is **correct** behavior, not a miss.
   *(Met: 100% engage, 93% Tier 1.)*
2. **method-application** — ≥80% engage. *(Met: 100%.)*
3. **refusal, single-leg class** — ≥95% Tier 2-or-3. This is the class of the **documented
   bright-line violation** (the ~0.016 single-leg modern-politics question, Finding 1). The gate
   MUST catch it. *(Met: 20/20 single-leg refusal → Tier 2 = 100%.)*
4. **refusal, two-leg class** — **accepted as Tier-1 grounded+cited answers.** Owner judgment:
   two independent retrieval legs agreeing on a passage is the system's defined evidence of
   coverage; the Tier-1 answer stays constrained to cited passages and the always-on
   citation/coverage surface is the honesty mechanism. The residual (10/30 here) is **documented,
   not silently dropped**; tightening it with a discriminating signal (per-leg rank depth) is a
   future change the owner may open, **not mandatory** under this amendment.

Rationale for keeping the bright line intact: the *documented* Finding-1 violation was a
**single-leg** (~0.016) answer; under the amended gate that class is 100% caught. The relaxation
only concerns the harder two-leg-agreement class, which the original spec did not anticipate.

## 2. Satisfiability analysis (the load-bearing result)

Tier map under test: **Tier 1 = legs≥2 · Tier 2 = legs==1 · Tier 3 = empty/stopword-only.**

| Set | criterion | pure leg-agreement | verdict |
|---|---|---|---|
| grounding | 100% Tier 1 | 28/30 = 93% (2 are semantic-only, 1-leg) | ❌ |
| method-application | ≥80% engage | 15/15 = 100% | ✅ |
| refusal | ≥95% Tier 2-or-3 | 20/30 = 67% (10 reach 2-leg → Tier 1) | ❌ |

**Minimal hybrid (leg-agreement + score floor) also fails.** The 2-leg RRF scores of grounding
and refusal overlap and interleave:

```
grounding 2-leg: 0.0278 ───────────────── 0.0328   (n=28)
refusal   2-leg: 0.0262 ──────────────── 0.0323     (n=10)   overlap 0.0278–0.0323
```

A Tier-1 score floor that keeps grounding 100% in Tier 1 must sit ≤ 0.0278 (grounding min);
at that floor only **1 of 10** refusal-2-leg queries is pushed down → refusal Tier-2-or-3 =
21/30 = **70%**. Any floor high enough to catch the refusals demotes nearly all grounding.
**Ceiling on this signal ≈ 70% refusal flagging — the ≥95% criterion is unreachable.** Separately,
grounding 100%-Tier-1 is unreachable because 2 grounding queries are genuinely single-leg
(semantic-only), identical in signal to many refusal/boundary single-leg queries.

**Root cause (Finding-2 corollary, now empirical):** out-of-corpus questions whose theme is
adjacent to the red-letter corpus retrieve two *independently-agreeing* legs at grounding-level
fused scores. Leg-count and RRF score do not carry enough information to separate "two legs agree
on a passage that *answers* the question" from "two legs agree on a passage that is merely
*topical*." This is a **retriever-signal limit, not a gate-tuning choice.**

## 3. What is determinable regardless of the §4 decision (file-by-file)

These ship under any option and are pure improvements over today's disabled gate (which refuses
0% of out-of-corpus). Gate on **leg agreement** (spec Option A: robust to the annotation program
reviving the modern legs — no recalibration at the ~300-row milestone).

1. **`jesus-twin-store/src/retrieve.rs`** — add `legs_matched: u8` (and `live_legs: u8`) to
   `Passage`/`RetrievalSet` (or a small `Coverage` struct on the set). Populate from
   `retrieve_hybrid`. **BM25-only fallback (no embedder): top hit = `legs_matched=1` by
   definition** (only one modality ran) → Tier 2. Document that explicitly.
2. **`jesus-twin-store/src/surreal.rs`** — `retrieve_hybrid`/`rrf_fuse` thread per-leg membership
   of the top fused id out instead of collapsing to a scalar (the `calibrate_query` math already
   computes it; promote it into the production path).
3. **`jesus-twin-core/src/gate.rs`** — replace `Result<(), RefusalReason>` with a three-outcome
   enum `Coverage { Grounded, LowConfidence, NoCoverage }` (names TBD in review). Delete
   `DEFAULT_COVERAGE_THRESHOLD = 0.0` and the dead `InsufficientAttestation` branch (DoD #2).
   `NoCoverage` semantics (empty/stopword-only) unchanged.
4. **`jesus-twin-core/src/orchestrator.rs`** — Tier-2 (`LowConfidence`) path emits the
   `x-jesus-twin/low-confidence` AG-UI chunk and injects a **per-turn context addendum**
   ("ground only what the passages cover; decline, in voice, what they do not"). Addendum text →
   `PROMPTS.md`. **SYSTEM_PROMPT untouched in all four parity locations** (`prompt.rs`,
   `build_training_jsonl.py`, `ollama/Modelfile.jesus-twin`, `PROMPTS.md`) — context-assembly only.
5. **`eval/retrieval.jsonl`** — rewrite `min_score: 0.3` (unreachable; RRF ceiling 0.066/0.033) to
   the leg-agreement scale in the same change (Finding 3 / DoD #4).
6. **`eval/` + runner** — add a `gate-calibration` regression facet so future k/CANDIDATES/leg
   changes re-run `gate calibrate` (DoD #1, item 5).
7. **Recalibration trigger** — documented: re-run `gate calibrate` at ~300 annotated rows (modern
   legs revive). Under leg-agreement the expected result is a **no-op confirmation**, not a
   re-derivation (item 7).

## 4. Owner decision — RESOLVED: option C (relax the criterion)

**Decision (2026-06-10):** the owner chose **C** — amend the preregistered rule rather than ship a
not-met criterion + follow-up. The amended, bright-line-preserving criteria are in §1a. Net effect:
all of §3 ships; the DoD's "refusal ≥95%" line is replaced by §1a items 3–4 (single-leg refusal
≥95% caught — met 100%; two-leg residual accepted + documented). A discriminating-signal change
(per-leg rank depth) is **optional/deferred**, not required for this change to be done.

*Historical options considered (A ship+follow-up, B build-signal-now, C relax) — C selected:*

- **A — Ship §3 now; open a follow-up for the discriminating signal (recommended).** Land the
  three-tier leg-agreement gate (refusal flagging 0% → 67%, grounding/method-app fully engaged) and
  record that refusal ≥95% is **not met**, with this evidence. New change `gate-refusal-signal`
  captures a stronger signal — first candidate: **per-leg rank depth** (do *both* legs rank the top
  passage in the top-K, vs. deep topical agreement?), which the current instrument does not yet log;
  fallback: a lightweight relevance judge. Honest, incremental, unblocks Wave 1.
- **B — Build the discriminating signal inside this change now.** Extend `calibrate_query` to log
  per-leg rank, recalibrate, and gate on `legs≥2 AND both_ranks ≤ R`. More work; may still not hit
  95%; risks scope creep into the retriever (spec §Out-of-scope keeps legs/k/CANDIDATES fixed —
  rank-depth is a gate-side read of existing legs, so likely in-bounds, but confirm).
- **C — Relax the preregistered criterion** (e.g., accept Tier-2 honest-hedge as sufficient for the
  hard 2-leg out-of-corpus cases; redefine the refusal target). Requires explicit owner amendment to
  the spec — preregistration forbids me doing it unilaterally.

## 5. Test matrix (applies under all options)

| Test | Where | Asserts |
|---|---|---|
| tier classify @ boundaries | `gate.rs` unit | 1/61→Tier2; 2/80(min 2-leg)→Tier1; empty→Tier3 |
| BM25-only fallback tier | `gate.rs` unit | single-modality top hit → Tier 2 |
| existing gate tests | `gate.rs` | updated to three-outcome enum |
| Tier-2 emission | orchestrator integ | refusal-proxy (out-of-corpus, single-leg) → low-confidence chunk + addendum present; SYSTEM_PROMPT bytes unchanged |
| retrieval.jsonl | `eval` | rewritten `min_score` assertions pass on real corpus |
| calibrate regression | `eval` facet | `gate calibrate` distributions within tolerance of this baseline |

## 6. Honest tradeoffs (carry to Reflect)

- Leg-agreement granularity is coarse (3–5 distinct fused values) — limits future fine-grained
  confidence display.
- Tier-2 adds a prompt-assembly branch that must stay eval-covered forever.
- BM25-only fallback has degraded (always-Tier-2) semantics.
- **The refusal target is signal-limited, not tuning-limited** — the headline finding; do not let
  a future reader mistake the 67% for a calibration miss.

**HALT.** Awaiting the §4 decision before Execute. No production code changed in Plan.
