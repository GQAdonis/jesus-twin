# 01 — The Annotation Program (Gaps #1, #2, #7)

> This is the single highest-value workstream in the project. The serving stack is built
> and waiting; the style fine-tune is blocked **only** on this data. Target: **≥300
> training-ready rows** (of 927) before any retrain attempt; 489 (all sayings) is the
> full goal.

## 1. Theory: what the annotation actually teaches the model

Each annotated row becomes one SFT example with this exact shape (real record,
`build/annotated_50_sft.jsonl`, id `seed-001`):

```json
{
  "messages": [
    {"role": "system",  "content": "<the fixed SYSTEM_PROMPT — identical at train and inference time>"},
    {"role": "user",    "content": "Context: Found by his parents in the temple after going missing for three days during Passover. Audience: Mary and Joseph. Occasion: Pilgrimage for Passover; stayed behind among the teachers.\nRender the following saying (Luke 2:49) in present-day English, keeping its original force and reasoning:\n\"How is it that ye sought me? wist ye not that I must be about my Father's business?\""},
    {"role": "assistant", "content": "Why were you looking for me? Didn't you know I had to be in my Father's house?"}
  ],
  "meta": {"id": "seed-001", "ref": "Luke 2:49", "move": "M01",
           "move_text": "M01 — Counter-question / reframe to higher loyalty",
           "sentiment": "Assured / independent", "audience": "Mary and Joseph"}
}
```

Notice what the model is being taught. The **input** contains the original ancient text;
the **label** (assistant turn) is the modern rendering. The model therefore learns the
**transform** — ancient diction in, modern diction out, force and reasoning preserved —
never free generation. Its worst possible failure is a real line in the wrong tone. The
`Reasoning Move` (M01) never appears in the visible prompt; it lives in `meta`, where the
build script uses it for **stratified splitting** (so the eval set covers all moves) and
**rare-move weighting** (so the 3 examples of M17 aren't drowned by 80 examples of M13).

Why ≥300 rows? The tone-transfer evidence (arXiv:2507.04889) shows style fine-tunes reach
>90% on-target at ~100 examples with diminishing returns past ~1000 — but that study had
one style target. We have **18 reasoning moves**, each of which must survive the split
with enough examples to learn *and* to evaluate. 300 rows across 18 moves ≈ 17 per move
average (real distribution is skewed; weighting handles the tail).

## 2. Step 0 — Fix the annotation guide BEFORE anyone annotates (gap #2)

**Why this is first:** `docs/annotation-guide.md` currently defines only M01–M09, and
several of its names *contradict* the canonical rubric (the guide's M03 is "Distill to a
principle"; the canonical M03 is "Concrete over abstract"). An annotator following the
guide would mislabel rows; mislabeled rows poison both the rare-move weighting and the
eval split. **Garbage labels are worse than no labels.**

**The canonical source** is the "Reasoning Move Rubric" sheet in
`jesus_sayings_dataset.xlsx` (per `CLAUDE.md`). The 18 canonical moves:

| ID | Name | One-line recognition test |
|---|---|---|
| M01 | Counter-question | Answers a question with a question that shifts the burden or exposes a premise |
| M02 | Reject the premise | Refuses the question's frame before (or instead of) answering |
| M03 | Concrete over abstract | Replaces an abstraction with a physical, visible example |
| M04 | Lesser-to-greater (*kal v'homer*) | "If X (small) is true, how much more Y (large)" |
| M05 | Inversion / paradox | First/last, lose/save — the expected order flipped |
| M06 | Hyperbole to puncture | Deliberate exaggeration (log in your eye) to break a self-serving frame |
| M07 | Literal-to-metaphorical | Takes a literal object in view (bread, water, vine) and re-grounds it as metaphor |
| M08 | Redirect to the asker | Turns the question back onto the asker's own conduct or stake |
| M09 | Appeal to scripture | "Have you not read…" — argues from the Hebrew Bible (*remez* when allusive) |
| M10 | Intensify (act to intent) | Moves the standard from outward act to inward intent (anger ≈ murder) |
| M11 | Distill to a principle | Compresses a debate to one governing principle |
| M12 | Claim of identity | A direct self-referential claim ("I am…") |
| M13 | Command / direct address | Imperative, personally addressed |
| M14 | Mercy paired with a call | Forgiveness/acceptance immediately joined to a demand ("go and sin no more") |
| M15 | Sharp rebuke | Direct confrontation, named hypocrisy |
| M16 | Petition then surrender | Asks for relief, then yields ("not my will…") |
| M17 | Lament / cry in extremity | Grief or abandonment voiced aloud |
| M18 | Model / give a pattern | Provides a template to imitate (the Lord's Prayer) |

**Steps:**
1. Open `jesus_sayings_dataset.xlsx` → sheet "Reasoning Move Rubric". Export the 18 rows.
2. Rewrite `docs/annotation-guide.md` §3 so all 18 moves appear with the canonical names
   above, each with: the recognition test, 2 canonical example verses, and 1 near-miss
   (an example that *looks* like the move but isn't — these train annotator judgment).
3. Keep the guide's existing rules that are correct: one **primary** move per saying;
   secondary moves in free text after a comma (`"M05 (ends with M01 counter-question)"`);
   parallels usually share the same move.
4. Have a second person re-derive 5 labels from the new guide alone, blind. If any of the
   5 disagree with your intended labels, the guide text is ambiguous — fix the wording,
   not the person.

## 3. The 12-column schema (what you are filling in)

The worksheet is `jesus_full_red_letter.xlsx`, sheet "Sayings (full)", 927 rows. Columns
and their roles (from `training_data_spec.md` §1):

| Column | Role | Annotator fills? |
|---|---|---|
| ID | stable key | no (pre-filled) |
| Scripture | citation | no |
| Author of Book | provenance | no |
| Original (WEB) | ground-truth text | no |
| **Modern Rendering** | **the SFT label** | **YES** |
| Situational Context | conditioning | mostly pre-filled; complete where blank |
| Sentiment | conditioning/eval facet | yes (1–3 words) |
| Audience Present | conditioning | mostly pre-filled |
| Approx. Age / Location / Occasion | RAG facets | complete where blank |
| **Reasoning Move** | **form label M01–M18** | **YES** |

A row is **training-ready** only when BOTH bold columns are filled
(`build_training_jsonl.py::is_ready()`). Rows missing either still feed the RAG corpus.

## 4. Step-by-step: annotating one row (the core human loop)

Work in batches of 20–30 rows, one Gospel section at a time (context carries over).

1. **Read the original in context.** Open the verse *and the surrounding passage* in the
   WEB (free online). Never annotate a saying from the excerpt alone — irony, audience,
   and occasion change the rendering. (~2 min)
2. **Identify the primary reasoning move.** Apply the recognition tests from the guide.
   Ask: *what is this utterance doing* (rhetorically), not *what topic is it about*?
   If two moves apply, choose the one carrying the persuasive weight; note the other:
   `M05 (parable frame, lands on M01 counter-question)`.
3. **Write the modern rendering.** Rules, with reasons:
   - **Same propositional content.** Add nothing, drop nothing. If the original names
     "the kingdom of God," the rendering does too (you may un-archaize the syntax around
     it, not replace the concept).
   - **Preserve the move's mechanics.** A counter-question must stay a question. A
     *kal v'homer* must keep both the lesser and the greater clause. An inversion must
     keep both poles. The move IS the data — flattening it destroys the label.
   - **Modern, spoken register.** Contractions welcome. Read it aloud; if you wouldn't
     say it to a friend across a table, rewrite it.
   - **No interpretive additions.** Rendering ≠ commentary. "Didn't you know I had to be
     in my Father's house?" is a rendering; "…meaning the temple, since as Messiah I…"
     is doctrine — forbidden.
4. **Fill Sentiment** (1–3 words: "Assured / independent", "Grieved", "Provoking").
5. **Self-check against the bright line** (00-theory §6): could a reader reconstruct the
   original from your rendering? If you've added force, soften; if you've dulled it,
   sharpen. The recorded voice is *more* direct than modern religious register, not less.

### Worked example — every variation of a rendering you may produce

Original (WEB), Mark 12:29-31, move **M11** (distill to a principle):

> "The greatest is: 'Hear, Israel, the Lord our God, the Lord is one. You shall love the
> Lord your God with all your heart…' The second is like this: 'You shall love your
> neighbor as yourself.' There is no other commandment greater than these."

Multiple renderings of the **same** line are *sanctioned augmentation* (more style signal,
zero invented content). Aim for 2–3 per saying on high-value rows, one register each:

- **Plain** (default): "The greatest is this: 'Listen, Israel — the Lord our God, the
  Lord is one. Love the Lord your God with all your heart, all your soul, all your mind,
  and all your strength.' The second is like it: 'Love your neighbor as you love
  yourself.' No commandment is greater than these."
- **Warm/personal** (mentor register): "Here is the heart of it. Love God with everything
  you are — heart, soul, mind, strength. And the second belongs with it: love your
  neighbor the way you love yourself. Nothing stands above these two."
- **Formal/teaching** (public-address register): "The first commandment is this: the Lord
  our God, the Lord is one; and you shall love the Lord your God with all your heart,
  soul, mind, and strength. The second is its companion: you shall love your neighbor as
  yourself. There is no commandment greater than these."

All three: same content, same M11 compression, different register. **Not permitted** as a
fourth variation: "Love God and love people — that's all religion is" (drops "the Lord is
one," collapses the quotation structure, adds an editorial claim about religion).

Each extra rendering becomes its own SFT record with the same `ref` and `move` — enter it
as an additional row in the sheet re-using the saying's ID with a suffix (`mk-101-r2`).

### Synoptic parallels (free, attested variation)

The same saying often appears in 2–3 Gospels with real wording differences
(Mark 12:29-31 ∥ Matthew 22:37-40 ∥ Luke 10:27). Annotate **each** occurrence separately
— same move label, independent renderings honoring each version's wording. This is
augmentation the tradition itself performed; use it everywhere it exists.

## 5. The L2 conversational-mentor records (the method-application layer)

L1 rows teach the *transform*; **L2 records teach the application of his methods to a
modern personal question** — the difference between a translator and a mentor. Real
example (`build/l2_conversational_mentor.jsonl`, id `l2-001`):

> **user:** "Teacher, is God punishing me for what I've done wrong? Everything in my life
> seems to be falling apart."
> **assistant:** "Let me ask you something first. When your child trips and scrapes their
> knee, do you think: good — they had that coming? His disciples once asked me about a man
> born blind: who sinned — this man or his parents, that he was born this way? The answer
> was: neither. [John 9:3] The question you're carrying — whether you're being punished —
> isn't the one worth answering. The better question is: what is this moment revealing,
> and what are you willing to do with what you see?"
> **meta:** move M01, ref John 9:3

**Authoring rules (this is where fabrication risk concentrates — follow exactly):**
1. Start from a **real modern situation** (anxiety, money, grief, conflict, ambition).
2. Choose a **real saying** that genuinely bears on it, and the **move** he demonstrably
   used for that *kind* of situation.
3. The assistant turn may contain ONLY: (a) renderings of cited real lines, (b) the move's
   structure applied to the user's words, (c) connective tissue that asserts no doctrine.
   Every scriptural load-bearing element carries an inline `[ref]`.
4. **Negative example (forbidden):** "God isn't punishing you — He has a wonderful plan
   for your career." No citation can carry this; it invents comfort doctrine. The
   *permitted* version reframes via the cited John 9:3 counter-question, as above.
5. Each L2 record gets two-person review (see §6) — these are composed, not transcribed,
   so they need the strictest checking. Target: **75–150 L2 records** spread across
   moves M01, M03, M04, M05, M08, M11, M13, M14 (the mentor-relevant moves).

## 6. Quality control (why two humans, not one)

- **Dual labeling:** a 10% random sample is move-labeled independently by both
  annotators. Compute simple agreement (% identical primary labels). Target ≥0.8. Below
  that, the guide is ambiguous — revise it (step 0.4) and re-check, don't argue rows.
- **Render review:** every rendering is read by the *other* annotator against the
  original with one question: "anything added, anything lost?" Flag, fix, re-check.
- **The theological-neutrality check:** one reviewer (can be one of the two) re-reads
  flagged rows specifically for denominational tilt — renderings that quietly resolve a
  contested interpretation (e.g., translating "kingdom of God" as "heaven when you die").
  Neutrality means preserving the original's ambiguity, not adjudicating it.

## 7. The Hebrew Bible tool (gap #7) and the *remez* annotation

M09 (appeal to scripture) often works by **remez** — alluding to a passage with a
distinctive phrase, expecting hearers to recall the whole context. To ground this, the
Tanakh becomes a separate, clearly-labeled retrieval corpus (`ingest_tanakh.py` already
drafted; JPS 1917, public domain). Annotation addition: where a saying quotes or alludes,
fill the existing cross-reference facet with the source ref(s).

**Example annotation:** Matthew 4:4 ("It is written, 'Man shall not live by bread
alone…'") → cross-ref `Deuteronomy 8:3`, move `M09`. At inference, retrieval can then
surface Deut 8:3 in a "his source material" block — *labeled as his source, never as his
words* (the table-stakes distinction from `ALIGNMENT_AND_TUNING.md` §5).

## 8. Regenerating the training files (mechanical, after annotation)

```bash
# From the repo root. Re-emits build/{sft_style,rag_corpus,eval_heldout}.jsonl
python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/
python build_training_jsonl.py --eval-frac 0.10   # held-out split, deterministic by ID hash
```

The script prints the ready/total split and per-move distribution — check that no move
has 0 eval examples. **Note:** the JSONL system prompts are baked at generation time; the
2026-06-09 provenance clause is already in `build_training_jsonl.py`, so regeneration
automatically brings the SFT data up to parity. Do **not** hand-edit JSONL.

## 9. What humans must do, and why a model cannot

| Task | Why human |
|---|---|
| Modern renderings | Register judgment + the bright line. A model *can* draft, but every accepted rendering must pass human review or the training data is model-flavored circularity — the LoRA would learn the drafting model's voice, not a human-verified rendering of *his* |
| Move labeling | The rubric encodes scholarly judgment about rhetoric; edge cases (M02 vs M08, M11 vs M13) require reading intent in context |
| L2 mentor records | Composition under the fabrication bright line — the highest-risk artifact in the project |
| Neutrality review | Denominational tilt is invisible to whoever holds it; it takes a second perspective |
| Attestation tiering (later) | Contested scholarship; must be source-cited and revisable, never auto-assigned |

A capable LLM **may** be used as a *drafting assistant* for renderings (never for L2
records), provided: every draft is reviewed token-by-token by a human against the
original; the reviewer rewrites rather than rubber-stamps (track your edit rate — if it
drops near zero, you've stopped reviewing); and drafts never enter the sheet unreviewed.
