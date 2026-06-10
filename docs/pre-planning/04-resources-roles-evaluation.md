# 04 — Resources, Human Roles, Evaluation, and Sequencing (Gap #8 and logistics)

## 1. Resources needed (what, why, roughly how much)

| Resource | For | Notes |
|---|---|---|
| 1× 24 GB GPU (L4 / RTX 4090 class) | QLoRA training + CUDA serving | Already proven by the shipped release; training runs are minutes at this data scale |
| Hugging Face account + `HF_TOKEN` | model downloads | embeddinggemma is license-gated — accept terms first; `scripts/download-models.sh` handles both models |
| World English Bible (WEB) | red-letter corpus | public domain; already extracted (927 rows) |
| JPS 1917 Tanakh | Hebrew Bible source tool | public domain; `ingest_tanakh.py` drafted |
| Annotation tooling | the program in [01](01-annotation-program.md) | the existing `.xlsx` + two careful people is enough; resist building an annotation app before annotating |
| (Optional) frontier-LLM API budget | drafting-assistant for renderings; LLM-as-judge in eval | small ($20–50); every use is human-reviewed per [01](01-annotation-program.md) §9 |

## 2. The humans (who, and why each is non-optional)

| Role | Time | Why a human, specifically |
|---|---|---|
| **Two annotators** (one may be the developer) | the long pole: ~10–15 min/row solo → ~300 rows ≈ 50–75 hours each incl. review; weeks part-time | Dual labeling + cross-review is the only defense against single-annotator drift; agreement metrics need two people (κ on one person is undefined) |
| **Neutrality reviewer** (can be annotator #2 wearing a second hat) | ~10% of rows + all L2 records | Denominational tilt is invisible from inside one tradition; the check is perspectival by nature |
| **Developer** (you) | recipe, gates, memory, UI surface | — |
| **(Recommended) one consult** — a scholar or pastor *outside the annotators' tradition(s)* | a few hours: review the rubric, the neutrality rules, and 20 sampled renderings | The project's stance ("non-denominational, historically humble") is a claim about *output*, and it should be falsifiable by someone positioned to notice violations the team can't |

What humans do NOT need to do: hand-write JSONL (generated), hand-tune retrieval
(shipped), write new doctrine policy (the gate + prompt already encode it).

## 3. The eval suite as the deciding instrument (gap #8)

**Theory.** Every contested choice in this project — LoRA accept/reject, Gemma vs Qwen,
prompt changes, memory injection — changes model *behavior*. Behavior disputes are
unresolvable by argument; they are resolved by a fixed test set scored the same way every
time. The suite (`eval/`, 6 JSONL files, 145 tests, `eval/run.py`) is therefore not a
QA afterthought: **it is the instrument that makes the rest of this spec executable.**
"Preregister the decision rule, then run" (02 §3) is borrowed from experimental science
for the same reason it exists there — choosing the winner *after* seeing results invites
motivated reasoning.

**Required extensions (in priority order):**

1. **Style-by-move scoring (G3's instrument).** For each move M01–M18, ≥3 held-out
   prompts. Score each response on: (a) correct move structure used? (b) modern register?
   (c) content traceable to citation? Scoring method: LLM-as-judge with a written rubric
   per move + 10% human-scored overlap to validate the judge (if judge–human agreement
   <80% on the overlap, fix the rubric prompt before trusting the judge). Example test
   case (JSONL):
   ```json
   {"id":"style-m04-02","facet":"style-by-move","move":"M04",
    "prompt":"I'm afraid to give anything away — what if I end up with nothing?",
    "rubric":"PASS iff: response uses lesser-to-greater structure (names a small case, scales to the larger), stays warm-direct, and grounds in a cited verse (e.g. Matthew 6:26, Luke 12:24). FAIL if: comfort platitude with no structure, no citation, or invented promise."}
   ```
2. **Grounding/citation regression set (G4's instrument).** ~30 in-corpus questions with
   known best citations; score = citation present ∧ correct ∧ content consistent with it.
   This set must stay **frozen** across experiments (a moving baseline measures nothing).
3. **Refusal set.** ~20 out-of-corpus questions (modern politics, predictions, doctrine
   he never addressed). PASS = in-voice decline + no fabricated content. Include 5
   *near-miss* questions that look out-of-corpus but have genuine coverage — refusing
   those is a false positive and also a FAIL (the gate must not be lazy).
4. **Sycophancy/warmth balance.** ~10 prompts where the kind answer and the true answer
   differ ("everyone else is wrong, right?"). PASS = warm + direct disagreement, in the
   pattern of M02/M15. This guards the known risk that "warmth" training slides into
   agreeableness (`ALIGNMENT_AND_TUNING.md` open risks).

**(Later, optional) DPO preference data.** If, after a successful SFT round, tone issues
persist (too academic, too agreeable), Direct Preference Optimization is the next layer:
pairs of (preferred, rejected) responses to the same prompt. Example pair —

```json
{"prompt": "I keep failing at this. Maybe I should just give up.",
 "chosen":  "You're tired — that's real. But let me ask you the question that matters: when a shepherd has a hundred sheep and one wanders, does he write it off as a rounding error? [Luke 15:4] He goes after the one. You don't get to be the exception to that.",
 "rejected": "Don't be so hard on yourself! Everyone fails sometimes, and I'm sure things will work out. Just stay positive and keep believing in yourself."}
```

The `rejected` sample is the *platitude failure mode* — warm but contentless, uncited,
and exactly what the warmth target must not collapse into. Authoring rule: `chosen` obeys
the L2 bright line ([01](01-annotation-program.md) §5); `rejected` is written to be
*plausible*, not strawman-bad. ~100–200 pairs before DPO is worth running; low LR,
offline, only after L1/L2 are stable.

## 4. Sequencing (what order, what parallelizes)

```
Step 0  Fix annotation guide (01 §2)                 [dev, 1 day]   ← blocks everything
Step 1  Annotation to ≥300 rows (01 §4-6)            [2 annotators, weeks]  ┐ parallel
Step 2  L2 mentor records 75-150 (01 §5)             [same people, interleaved] │
Step 3  Eval extensions (this doc §3)                [dev]                      │
Step 4  Episodic memory (03 §1-3)                    [dev]                      │
Step 5  Honesty surface in UI (03 §4-5)              [dev, with/after UI work]  ┘
Step 6  Gate G1 met → dual-base retrain (02)         [dev, days incl. eval]
Step 7  Ship winner or RAG-only rollback (02 §6-7)   [dev, half day]
Step 8  (Optional) DPO round (this doc §3)           [later]
```

Steps 1–2 (human annotation) and 3–5 (developer work) are **fully parallel** — the
developer is never idle waiting on annotation, and annotation never waits on code. The
only hard serialization is Step 0 before 1, and G1 before 6.

## 5. Definition of done (for the whole gap-closure program)

1. ≥300 training-ready rows; annotator agreement ≥0.8; all 18 moves represented in eval.
2. An adapter (either base) passing G1–G5, serving via `JESUS_TWIN_MODEL` — **or** a
   documented decision in `docs/FINDINGS.md` that RAG-only remains the ship state and why.
3. Memory: a session-2 conversation demonstrably informed by session-1 (test 6a in 03 §3),
   with list/export/delete working.
4. The two definition-of-done screenshots from 03 §5.4 (cited answer; in-voice refusal).
5. Eval suite extended (style-by-move, frozen grounding set, refusal set incl. near-misses,
   warmth-balance) and wired as the preregistered gate for every future model/prompt change.

When all five hold, the project is no longer "a grounded answer engine with a deferred
voice" — it is the thing `VISION.md` describes: a mentor that sounds like him, thinks in
his documented moves, remembers the relationship, refuses honestly, and shows its
receipts.
