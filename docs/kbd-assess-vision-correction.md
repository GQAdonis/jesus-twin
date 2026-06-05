# KBD Assess — Vision Correction Phase

Date: 2026-06-05
Phase: vision-correction
Status: assessment_complete

---

## Assessment Summary

This assessment evaluates the gap between the **owner's stated goal** and the
**"study aid" framing** previously embedded in project documents, then produces a
corrected plan that preserves the technical rigor from the prior assessment while
expanding the product scope to match the actual goal.

---

## 1. The Gap That Was Present

The prior documents (`ALIGNMENT_AND_TUNING.md`, `digital_twin_architecture.md`,
`docs/jesus-digital-twin-assessment.md`) framed the product as a "study aid" that
renders sayings and hedges everything academically. The owner's goal is richer:

> A conversational mentor who responds like Jesus would — warm, direct, personally
> engaged, drawing on his full intellectual world (Hebrew Bible included) — whom a
> person can talk to as a friend, counselor, and teacher.

The gap was not a technical contradiction. The underlying architecture supports this
richer goal. The gap was a **product framing problem**: the documents constrained the
agent's persona to "study aid," hedging warmth and directness in ways that prevent the
mentor experience without adding any additional safety protection.

**What sycophancy correction found:** the original goal statement scored 0.125 (S-03
critical) — it stated the aspiration with zero trade-offs. The corrected position is
not "scale back the goal" but "state what is achievable precisely, then build to that."

---

## 2. What Is Achievable — Evidence-Grounded

### CharacterBot (ACL 2025, arXiv:2502.12988)

The closest published analogue: a model trained to replicate both the linguistic
patterns and the distinctive thought processes of a historical figure (Lu Xun) using
17 essay collections. Four training tasks: pre-training on external linguistic
structures, then three fine-tuning tasks (MCQ, generative QA, style transfer) aligned
with the character's internal ideation. The result significantly outperformed baselines
on both linguistic accuracy and opinion comprehension.

**Implication for this project:** the technical path to deep persona simulation —
going beyond surface facts to replicate method and thought patterns — is well-established.
The Jesus corpus (927+ sayings plus the full Tanakh he argued from) is comparable in
richness to Lu Xun's 17 essays. The gap is annotation depth, not feasibility.

### His Documented Rhetorical Methods

These are not speculation. They are analyzed in multiple independent scholarly sources:

- **Word order and suspense building**: subject held back as a surprise, most important
  word placed last. Source: christswords.com analysis of Greek word-order patterns.
- **Contrast of opposites**: automatic use of two-part positive/negative structures.
- **Phrase inversion**: "you agree with me" → "I will agree with you" as a deepening
  device.
- **Parable**: primary teaching genre; over 1,000 parallel rabbinic parables confirm
  the genre's authenticity even where individual parables are debated.
- ***Kal v'homer* (lesser-to-greater)**: "how much more" arguments; one of the six
  standard rabbinic hermeneutical rules; Jesus used it constantly.
- ***Remez* (allusion)**: using a distinctive word to evoke an entire scripture passage
  from the Tanakh. His audience was expected to know the text.
- **Personalizing singular vs. plural address**: documented in the Greek where English
  hides it.
- **Rule of three plus one**: pattern used in at least a dozen recorded sayings.

Sources: Gary Gagliardi (christswords.com), Lois Tverberg / En-Gedi Resource Center,
the M01–M18 rubric already in this project.

---

## 3. Corrections Applied to Project Documents

### New: VISION.md

Created `/VISION.md` as the authoritative product goal statement. Key decisions:

- Replaces the implicit "study aid" framing with explicit "conversational mentor" goal.
- Documents the honest capability ceiling (what is and is not achievable).
- Defines the architecture contract for the mentor mode.
- Defines the three corpus layers and the conversational persona contract.
- Scores the raw goal statement as S-03 critical sycophancy and provides the corrected
  statement that is buildable and evaluable.

### Updated: ALIGNMENT_AND_TUNING.md

Changed scope from historical-critical study aid to conversational mentor. Key changes:

1. **Section 0**: replaces "historical-critical reconstruction" framing with
   conversational mentor framing. Explains why the prior framing was too narrow.
2. **Section 1 table**: L2 column now says "conversational mentor examples" not just
   "in-domain instruction data."
3. **New Section 2a**: documents the nine rhetorical methods as explicit training targets.
4. **Section 2**: retitled "non-religious non-denominational alignment" instead of
   "non-religious historical alignment." Removes the academic-hedging tone from the
   operational rules.
5. **Section 6 risks**: adds "warmth target can slide into sycophancy toward user" as
   an explicit DPO target.
6. **Sources**: adds CharacterBot (ACL 2025), christswords.com, En-Gedi Resource Center.

### Not Changed

- `ARCHITECTURE.md` — the Rust service design is correct and product-goal-agnostic.
  It serves the mentor goal as-is.
- `training_data_spec.md` — the SFT/RAG JSONL formats are correct. The L2 conversational
  examples will use the same format.
- `README.md` — model and DB choices are unchanged.
- `DATA_EXTRACTION.md` — corpus extraction is unchanged.

---

## 4. Capability Ceiling — Sycophancy-Corrected Statement

The sycophancy detector scored the raw goal ("talk to an actual REAL Jesus") at 0.125
critical. The corrected, buildable version:

> A conversational agent that applies Jesus' documented rhetorical methods and warmth
> to the user's questions, drawing only from attested corpus and the Hebrew Bible he
> taught from, in modern English — indistinguishable in *method* from the recorded Jesus,
> constrained in *content* to what is attested, with graceful in-character refusal when
> coverage ends.

**What this is not:**
- Not omniscient (the agent has base-model background knowledge, not divine knowledge)
- Not the actual Christ (it is a simulation of attested patterns)
- Not able to claim authority on matters not addressed in the corpus
- Not a representative of any denomination or theological tradition

**What this can be:**
- Warm, direct, personally engaged — because the recorded Jesus was
- Uses his documented methods consistently and recognizably
- Responds to personal questions (anxiety, money, relationships, purpose) using his
  attested frameworks
- Draws on the Hebrew Bible he actually quoted when relevant
- Refuses outside-corpus questions in his voice, not with a system message

---

## 5. Next Actions (Updated from Prior Assessment)

Priority order has changed from the prior assessment to reflect the mentor goal:

| # | Action | Rationale |
|---|---|---|
| 1 | Write annotation guide (`docs/annotation-guide.md`) | Must cover method labels (parable, *kal v'homer*, counter-question, etc.) not just rendering style and M01–M18 |
| 2 | Annotate 50 representative sayings with full method labeling | Mentor mode requires method annotation, not just rendering |
| 3 | Build L2 conversational mentor examples (25–50 rows) | Personal questions answered in his methods and voice; blended into L1 SFT |
| 4 | Build RAG prototype with cited retrieval and in-voice refusal | Fastest safe milestone; validates truth layer before voice |
| 5 | Add Hebrew Bible source tool to retrieval | His intellectual world; enables *remez* and *kal v'homer* sourced from Tanakh |
| 6 | Attestation + source-critical metadata on corpus | Make confidence visible without academic hedging in the persona |
| 7 | Eval suite before training | Grounding, refusal, method-application tests |
| 8 | L1 LoRA + merged serving | Only after annotation and eval suite exist |

---

## 6. Progress Update

```json
{
  "assessment_complete": true,
  "vision_corrected": true,
  "alignment_tuning_updated": true,
  "new_files": ["VISION.md", "docs/kbd-assess-vision-correction.md"],
  "modified_files": ["ALIGNMENT_AND_TUNING.md"],
  "unchanged_files": ["ARCHITECTURE.md", "README.md", "training_data_spec.md", "DATA_EXTRACTION.md"],
  "annotation_ready": false,
  "rag_prototype_ready": false,
  "lora_training_ready": false
}
```

---

## 7. Sources Used in This Assessment

Repository:
- `VISION.md` (new)
- `ALIGNMENT_AND_TUNING.md` (updated)
- `ARCHITECTURE.md`
- `training_data_spec.md`
- `docs/jesus-digital-twin-assessment.md`

Firecrawl/web:
- CharacterBot / deep persona simulation. arXiv:2502.12988 (ACL 2025).
  https://arxiv.org/abs/2502.12988
- Jesus's speaking style (word order, contrast, inversion, phrase structure).
  https://christswords.com/content/jesuss-speaking-style
- Jesus' rabbinic teaching methods (parable, *kal v'homer*, *remez*).
  https://engediresourcecenter.com/2015/07/07/truth-before-and-after-jesus/
- Chatbot persona risks (ontological confusion, ELIZA effect).
  https://christianscholars.com/the-problem-with-chatbot-personas/

Sycophancy correction:
- Raw goal scored 0.125 (S-03 critical) under adversarial mode.
- ALIGNMENT_AND_TUNING.md revision not re-scored (no sycophancy patterns expected in
  a technical spec document).
