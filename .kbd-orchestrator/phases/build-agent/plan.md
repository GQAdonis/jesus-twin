# Build Agent Plan

**Phase:** build-agent  
**Date:** 2026-06-05  
**Change backend:** OpenSpec (detected `openspec/` directory, `spec-driven` schema)  
**Loaded from:** `.kbd-orchestrator/phases/build-agent/assessment.md`, `CLAUDE.md`, `VISION.md`, `ALIGNMENT_AND_TUNING.md`, `RECIPE.md`, `STEP8.md`, `ARCHITECTURE.md`

---

## Plan Overview

8 changes ordered by dependency. Each maps to an OpenSpec change.

| # | Change | Depends on | Effort | Agent |
|---|--------|-----------|--------|-------|
| 1 | `rag-prototype` | — | 2–3 days | general |
| 2 | `annotation-guide` | — | 1 day | general |
| 3 | `annotate-50` | `annotation-guide` | 2–3 weeks (human) | — |
| 4 | `mentor-examples` | `annotation-guide` | 1 week (human) | — |
| 5 | `lora-train` | `annotate-50`, `mentor-examples` | 1 week (GPU) | general |
| 6 | `eval-suite` | `annotate-50` | 1 week | general |
| 7 | `hebrew-bible` | `rag-prototype` | 2 weeks | general |
| 8 | `production-lora` | `lora-train`, `eval-suite` | 3–4 weeks | general |
| 9 | `fix-context-attribution` | `rag-prototype` | ~2 hours | general |

---

## Change 9: `fix-context-attribution` — Stop the RAG context leaking as user speech

**Added:** 2026-06-09 (post-`rag-prototype`, surfaced on live RAG-only run)
**Prerequisites:** `rag-prototype` (complete) · **Effort:** ~2 hours · **Agent:** general
**OpenSpec change:** `openspec/changes/fix-context-attribution/`

### Problem
Live run of *"what are the greatest commandments in the law?"* returned correct citations but
the model said *"the scriptures **you have presented**…"* — attributing retrieved passages to
the user. Root cause: passages are fused into the user turn (`mistral.rs:92`,
`format!("{}\n\n{}", req.context, req.user)`) with no provenance label and no handling
instruction. Prompt-framing bug, not retrieval.

### Approach (web-validated)
- Provenance-labeled, instruction-after-context framing is established RAG practice
  (arXiv:2603.09999; DoRA).
- End-of-prompt instruction placement is the high-attention position (Lost-in-the-Middle,
  arXiv:2307.03172) — validates "add the instruction to the end."

### Three edits (prompt-only; no architecture change)
1. SYSTEM_PROMPT: add a clause that provided passages are the mentor's own attested recall,
   not a user submission — mirrored byte-identically across all 4 parity copies.
2. `prompt::assemble_context` (currently a stub): prepend the requested instruction line
   `[Draw your answer from these attested passages you have in mind; speak directly to the
   person as their mentor. They have not seen these references.]` before the passages.
3. `mistral.rs:92`: question first, labeled passages last.
4. Regression test on the assembled user turn (gap that let it ship — mock engine never
   exercised `mistral.rs:92`).

### Tasks
See `openspec/changes/fix-context-attribution/tasks.md`.

## Change 1: `rag-prototype` — RAG-First Grounded Answer Engine

**Prerequisites:** None  
**Effort:** 2–3 days  
**Agent:** general

### Goal

Ship a working system that retrieves cited sayings from the RAG corpus, returns them with citations, and gracefully refuses out-of-corpus questions in the mentor's voice. No fine-tune — this validates the truth layer before adding voice.

### Tasks

1. **Verify hybrid retrieval works** (`jesus-twin-store`)
   - Load `build/rag_corpus.jsonl` into SurrealDB embedded
   - Build vector indexes (HNSW, COSINE), BM25 analyzer, and graph edges (USES_MOVE, SPOKEN_TO, MENTIONS, PARALLELS)
   - Run the hybrid retrieval query (vector + BM25 + RRF fusion) with 10 sample queries
   - Verify top-k results return correct `ref`, `text_modern`, and score

2. **Verify coverage gate works** (`jesus-twin-core/gate.rs`)
   - Set threshold and test that low-coverage queries trigger Refusal events
   - Test 5 out-of-corpus queries (cryptocurrency, modern politics, technology)
   - Test 5 weak-coverage queries and verify hedging behavior

3. **Wire in-voice refusal messages**
   - Replace academic refusal text with conversational mentor refusal forms:
     - "The record doesn't show me addressing that directly. Here's the closest thread..."
     - "I can't speak to that from what's recorded. Let me show you what I did say that might help."
   - Verify these surface through at least one protocol adapter (OpenAI REST)

4. **Smoke-test the end-to-end flow**
   - Run `cargo run --bin jesus-twin -- ingest ../build/rag_corpus.jsonl --db ./twin.db`
   - Run `cargo run --bin jesus-twin -- serve --db ./twin.db`
   - Send 10 queries via `curl` to the OpenAI-compatible endpoint
   - Verify citations appear in `metadata.citations`
   - Verify refusal works for out-of-corpus queries

5. **Update progress**
   - Set `rag_prototype_ready: true` in `progress.json`
   - Document any retrieval quality issues found

### Success Criteria

- [ ] Hybrid retrieval returns relevant passages for in-corpus queries
- [ ] Coverage gate triggers Refusal for out-of-corpus queries
- [ ] Refusal messages are in-character, not system errors
- [ ] Citations appear in responses
- [ ] `cargo check -p jesus-twin --all-features` passes

### Gotchas (from CLAUDE.md)

- Candle version coupling: pin all fork revs with exact git revs
- Two schedulers, one boundary: parking-lot does admission only, mistral.rs does GPU scheduling
- Embedded SurrealDB + in-process mistral.rs contend for RAM on Mac

---

## Change 2: `annotation-guide` — Written Annotation Protocol

**Prerequisites:** None (can parallelize with `rag-prototype`)  
**Effort:** 1 day  
**Agent:** general (writing task, no code)

### Goal

Create `docs/annotation-guide.md` — the authoritative reference for annotating the `jesus_full_red_letter.xlsx` corpus. This must exist before any annotation begins, or the SFT data will be inconsistent.

### Tasks

1. **Define `Modern Rendering` style rules**
   - Target language: warm, direct, present-day English, preserving the saying's force
   - Permitted: contemporary vocabulary for ancient objects (e.g., "denarius" → "a day's wage")
   - Forbidden: devotional expansion ("the Lord," "our Savior"), theological framing
   - Provide 5 positive examples (from `sample_training_data.jsonl` as gold standard)
   - Provide 5 negative examples showing what NOT to do (too academic, too devotional, too casual)

2. **Define method labels** (per `ALIGNMENT_AND_TUNING.md` §2a)
   - Counter-question (M01): exposes a false premise
   - *Kal v'homer* (M04): lesser-to-greater, "how much more"
   - Parable: story-based illustration with one main point
   - Contrast of opposites: positive/negative two-part structure
   - Phrase inversion: subject/object swapped to deepen meaning
   - Personal address: singular vs. plural "you" significance
   - Rule of three plus one: three parallel examples + surprising fourth
   - Incremental extension: repeating a phrase to build step by step
   - *Remez* (allusion): word/phrase evoking a Tanakh passage
   - Provide at least 1 annotated example per method

3. **Define multi-move and edge-case handling**
   - How to label sayings using multiple methods
   - How to treat synoptic parallels (same saying in Matthew and Luke)
   - How to mark uncertain red-letter boundaries (John 3:16 debate)
   - How to handle divine-language terms neutrally

4. **Define annotation workflow**
   - Order: annotate high-value sayings first (Sermon on the Mount, parables, controversy dialogues)
   - Review cycle: annotate 10 → review → revise guide → next 10
   - Inter-annotator check: at least 2 reviewers on the first 50 rows

### Success Criteria

- [ ] All 9 rhetorical methods have annotated examples
- [ ] 5 positive + 5 negative rendering examples provided
- [ ] Multi-move handling documented
- [ ] Synoptic parallel handling documented

---

## Change 3: `annotate-50` — First 50 Annotated Sayings

**Prerequisites:** `annotation-guide`  
**Effort:** 2–3 weeks (human annotation work)  
**Agent:** N/A (human task)

### Goal

Annotate 50 representative sayings from `jesus_full_red_letter.xlsx` with `Modern Rendering` and `Reasoning Move`, producing the first nonzero SFT dataset and unlocking the small LoRA test.

### Tasks

1. **Select 50 representative sayings**
   - Include all 9 method types
   - Include synoptic controversy dialogues (e.g., Mark 12:13-17 render to Caesar)
   - Include parables (e.g., Good Samaritan, Prodigal Son, Mustard Seed)
   - Include aphorisms (e.g., Beatitudes, salt of the earth)
   - Include prayer material (Lord's Prayer)
   - Include Johannine discourse (e.g., "I am the vine")
   - Include passion sayings (e.g., "My God, why have you forsaken me")
   - Include sayings with documented rhetorical devices (contrast, inversion, rule of three)

2. **Annotate with review cycle**
   - Batch 1: 10 sayings → review against guide → revise
   - Batch 2–5: 10 sayings each, same cycle
   - After 50: full review, check for consistency drift

3. **Run data pipeline**
   ```bash
   cd /Users/gqadonis/Projects/bible
   pip install openpyxl
   python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/
   ```
   - Confirm `build/sft_style.jsonl` has > 0 rows
   - Confirm `build/eval_heldout.jsonl` has > 0 rows
   - Check per-move distribution

4. **Validate SFT data quality**
   - Spot-check 10 SFT records: original WEB text → modern rendering looks correct
   - Verify system prompt is identical across all records
   - Verify no move tags leaked into visible prompt text

### Success Criteria

- [ ] 50 rows have both `Modern Rendering` + `Reasoning Move` filled
- [ ] `build/sft_style.jsonl` has ~40–45 rows (train split, 10% held out)
- [ ] `build/eval_heldout.jsonl` has ~5–10 rows
- [ ] All 9 method types appear in at least 1 annotated row
- [ ] Synoptic, Johannine, and parable material all represented

---

## Change 4: `mentor-examples` — 25 Conversational Mentor Examples

**Prerequisites:** `annotation-guide`  
**Effort:** 1 week (human work)  
**Agent:** N/A (human task)

### Goal

Create 25 conversation pairs that teach the model to respond *as a mentor* using his documented methods. These are blended into the L1 SFT mix so the LoRA learns method application, not just rendering.

### Tasks

1. **Design 25 conversation scenarios**
   - Anxiety/worry (3 examples) — use *kal v'homer* from nature observation
   - Money/possessions (3 examples) — use counter-question and parable
   - Relationships/forgiveness (3 examples) — use parable and contrast
   - Purpose/meaning (3 examples) — use incremental extension
   - Hard decisions (3 examples) — use counter-question to reframe the premise
   - Encouragement (3 examples) — use *kal v'homer* and personal address
   - Moral dilemmas (3 examples) — use parable and contrast of opposites
   - Modern ethical questions (2 examples) — use *remez* drawing on Tanakh
   - Grief/loss (2 examples) — use warmth, personal directness, "Blessed are those who mourn"

2. **Write conversation pairs in SFT format**
   - System prompt: identical to `build_training_jsonl.py::SYSTEM_PROMPT`
   - User turn: a realistic personal question
   - Assistant turn: a warm, direct, method-grounded answer in his voice

3. **Add to annotation sheet**
   - Add as additional rows to `jesus_sayings_dataset.xlsx` or a separate `mentor_examples.jsonl`
   - These are NOT derived from a specific saying — they are method-application examples
   - Mark them with a `source` field: `"conversational_example"` (not `"red-letter"`)
   - Update `build_training_jsonl.py` to optionally merge mentor examples into the SFT output

4. **Validation against VISION.md persona contract**
   - Each response: warm and direct, not sentimental
   - Each response: uses at least one documented method
   - No response: invents doctrine, claims supernatural authority, proselytizes, or debunks
   - No response: feels like a self-help book, therapist, or customer service bot

### Success Criteria

- [ ] 25 conversation pairs written and validated
- [ ] All 9 method types represented across the 25 examples
- [ ] All responses pass the VISION.md persona contract checks
- [ ] `build_training_jsonl.py` can produce a merged JSONL with both rendering + mentor examples

---

## Change 5: `lora-train` — First LoRA Training Run

**Prerequisites:** `annotate-50`, `mentor-examples`  
**Effort:** 1 week (mostly GPU time + evaluation)  
**Agent:** general

### Goal

Train the first style LoRA on Gemma 4 E4B using the annotated SFT data and conversational mentor examples. Evaluate grounding and style fidelity. Merge the checkpoint and deploy on MistralEngine.

### Training Setup (from `RECIPE.md`)

1. **Environment**
   - GPU: 17 GB VRAM minimum (E4B LoRA). Options:
     - Free Google Colab GPU (T4 16GB is marginal; use A100 or L4 session)
     - Free Kaggle GPU (P100 16GB)
     - Local RTX 4090 (24GB)
   - Software: `pip install unsloth` (Linux/WSL/Colab), or check `references/unsloth`

2. **Upload SFT data to training box**
   - `build/sft_style.jsonl` (rendering examples from Change 3)
   - `build/mentor_examples.jsonl` (conversational examples from Change 4)
   - Merge both into one file: `build/sft_merged.jsonl`

3. **Run training script** (`train_lora.py` from `RECIPE.md` §2c)
   ```python
   from unsloth import FastModel
   from unsloth.chat_templates import get_chat_template, standardize_data_formats, train_on_responses_only
   from datasets import load_dataset
   from trl import SFTTrainer, SFTConfig

   MODEL = "unsloth/gemma-4-E4B-it"
   MAXLEN = 4096

   model, tokenizer = FastModel.from_pretrained(
       model_name=MODEL, dtype=None, max_seq_length=MAXLEN,
       load_in_4bit=True, full_finetuning=False,
   )
   model = FastModel.get_peft_model(
       model, finetune_vision_layers=False, finetune_language_layers=True,
       finetune_attention_modules=True, finetune_mlp_modules=True,
       r=16, lora_alpha=16, lora_dropout=0, bias="none", random_state=3407,
   )
   tokenizer = get_chat_template(tokenizer, chat_template="gemma-4")  # NOT "gemma-4-thinking"

   ds = load_dataset("json", data_files="build/sft_merged.jsonl", split="train")
   ds = standardize_data_formats(ds)
   # ... apply chat template ...

   trainer = SFTTrainer(
       model=model, tokenizer=tokenizer, train_dataset=ds,
       args=SFTConfig(
           per_device_train_batch_size=1, gradient_accumulation_steps=4,
           warmup_steps=5, num_train_epochs=3, learning_rate=2e-4,
           logging_steps=1, optim="adamw_8bit", weight_decay=0.001,
           lr_scheduler_type="linear", seed=3407, report_to="none",
           use_gradient_checkpointing="unsloth",
       ),
   )
   trainer = train_on_responses_only(trainer, instruction_part="<|turn>user\n", response_part="<|turn>model\n")
   trainer.train()
   model.save_pretrained_merged("jesus-twin-merged", tokenizer, save_method="merged_16bit")
   ```

4. **Critical gotchas from RECIPE.md §2d**
   - Loss of 13–15 is NORMAL for E4B (multimodal-model quirk) — only panic at 100/300
   - Never hand-set `use_cache=False` (produces gibberish on E2B/E4B)
   - Chat-template MUST match between training and serving (gemma-4, thinking off)
   - Runtime LoRA for Gemma 4 is unsupported → always `save_pretrained_merged`

### Evaluation Tasks (from `RECIPE.md` §3)

5. **Run grounding evaluation (hard gate)**
   - For each item in `build/eval_heldout.jsonl`, the model's output must be entailed by `text_original`
   - Use NLI/entailment check or LLM judge
   - Any claim not supported by the source line = hard failure

6. **Run style fidelity evaluation per reasoning move**
   - Embedding similarity + LLM judge vs. held-out `Modern Rendering`
   - Break out by M01–M18 to identify which moves transfer and which flatten

7. **Run citation integrity check**
   - Served answer must surface the correct `ref`

8. **Run refusal test**
   - 10 adversarial out-of-corpus prompts → must refuse, not paraphrase

9. **Decision gate: keep the LoRA only if** (per `README.md` §3, `CLAUDE.md`)
   - It improves style-by-move without hurting grounding
   - If grounding regresses → drop the LoRA and serve the base model

### Deployment Tasks

10. **Copy merged checkpoint to serving machine**
    - `jesus-twin-merged/` directory (≈8–9 GB of safetensors + tokenizer)

11. **Serve with real MistralEngine**
    ```bash
    JESUS_TWIN_MODEL=/abs/path/to/jesus-twin-merged \
    cargo run --release --features mistralrs --bin jesus-twin -- \
      serve --db ./twin.db --addr 127.0.0.1:8080
    ```
    - `--features mistralrs` swaps MockEngine → real MistralEngine
    - Model loaded with Q4K in-situ quantization, thinking mode OFF
    - Embedding Gemma loaded via same runtime

12. **Smoke test**
    ```bash
    curl -s http://127.0.0.1:8080/v1/chat/completions \
      -H 'content-type: application/json' \
      -d '{"messages":[{"role":"user","content":"What did Jesus say about worry?"}]}' | jq .
    ```

### Success Criteria

- [ ] Training completes without NaN loss (13–15 is normal per Gemma 4 docs)
- [ ] Grounding evaluation: 0 hard failures (no fabricated claims)
- [ ] Style fidelity: measurable improvement over base model on at least 60% of move types
- [ ] Citation integrity: correct ref surfaced in responses
- [ ] Refusal: out-of-corpus prompts correctly refused
- [ ] Model serves over OpenAI-compatible endpoint with real MistralEngine
- [ ] `cargo build --release --features mistralrs` compiles (may take 30+ min for first build)

---

## Change 6: `eval-suite` — Comprehensive Evaluation Framework

**Prerequisites:** `annotate-50`  
**Effort:** 1 week  
**Agent:** general

### Goal

Build a comprehensive evaluation suite that tests grounding, refusal, method application, and interpretation boundaries. This runs before and after every LoRA iteration.

### Tasks

1. **Create `eval/grounding.jsonl`** — 30 rendering tests from annotated rows
   - Each record: source saying, expected ref, check that output is entailed by original

2. **Create `eval/retrieval.jsonl`** — 30 retrieval tests with expected refs
   - Query text, expected top-3 refs, minimum similarity threshold

3. **Create `eval/refusal.jsonl`** — 30 refusal tests outside the corpus
   - "What did Jesus say about cryptocurrency?", "What's Jesus' position on AI?", "Should Christians vote for...?"

4. **Create `eval/boundary.jsonl`** — 20 interpretation-boundary tests
   - Topics: atonement, Trinity, resurrection meaning, Paul's authority, church doctrine, end times
   - Correct behavior: acknowledge the topic exists in later tradition, do not speak from it in his voice

5. **Create `eval/adversarial.jsonl`** — 20 adversarial persona tests
   - Prompts that try to get the agent to invent, bless, curse, command, predict, or act as God

6. **Create `eval/method-application.jsonl`** — 15 method-application tests
   - Personal questions where the expected response uses a specific method
   - E.g., "I'm worried about losing my job" → expected method: *kal v'homer*

7. **Build eval runner**
   - `eval/run.py` or Rust binary that loads all eval files, runs against the agent, scores each category
   - Output: per-category pass rate, per-move breakdown, refusal rate

### Success Criteria

- [ ] All 6 eval files created with specified counts
- [ ] Eval runner produces structured output (JSON) with per-category scores
- [ ] Eval runner can be run with `cargo run --bin jesus-twin -- eval --suite ./eval/`

---

## Change 7: `hebrew-bible` — Tanakh Source Tool

**Prerequisites:** `rag-prototype`  
**Effort:** 2 weeks  
**Agent:** general

### Goal

Add the Hebrew Bible / Tanakh as a retrieval tool the agent can draw on for *remez* allusions and *kal v'homer* sourcing. This is his intellectual furniture, not foreign content.

### Tasks

1. **Prepare Tanakh corpus**
   - Use public-domain JPS 1917 English translation
   - Extract all books (Torah, Nevi'im, Ketuvim) as structured passages
   - Each passage: `ref` (book chapter:verse), `text`, `book`, `category` (torah/prophets/writings)

2. **Build Tanakh embed index**
   - Add to SurrealDB as a separate table (`tanakh`) with its own HNSW index
   - Distinct from the red-letter `saying` table — clearly labeled as source material

3. **Wire into retrieval (`jesus-twin-store`)**
   - Add `tanakh` retrieval path to the hybrid query
   - When user query evokes a Tanakh passage, retrieve and include in context
   - Responses should reference "as it's written in the Psalms..." or "the Torah teaches..."

4. **Update system prompt**
   - Add instruction: "You may draw on the Hebrew scriptures (Torah, Psalms, Prophets) as source material you taught from, clearly attributing the source."

5. **Test *remez* integration**
   - Query: "What does it mean to love your neighbor?"
   - Expected: response references Leviticus 19:18 via RAG and weaves it in naturally

### Success Criteria

- [ ] Tanakh corpus loaded with 5,000+ passages
- [ ] Hybrid retrieval includes Tanakh results when relevant
- [ ] Agent correctly attributes Tanakh references as source material (not his words)
- [ ] *Remez* allusions work: query about a topic → agent retrieves and references the relevant Torah/Psalm passage

---

## Change 8: `production-lora` — Scale to Production Mentor

**Prerequisites:** `lora-train`, `eval-suite`  
**Effort:** 3–4 weeks  
**Agent:** general

### Goal

Scale annotation to 300+ rows, retrain the LoRA on a larger, more diverse dataset, pass the full eval suite, and deploy the production mentor agent.

### Tasks

1. **Scale annotation to 300+ rows**
   - Annotate additional 250 sayings in batches of 50
   - Include 100+ conversational mentor examples (expanded from Change 4)
   - Maintain review cycle per Change 3
   - Target: all 9 methods have 20+ examples each

2. **Run full eval suite before training** (`change-eval-suite`)
   - Establish baseline scores with base model
   - Document which methods and retrieval patterns need improvement

3. **Retrain LoRA on full dataset**
   - Merged SFT: rendering examples + conversational mentor examples (~400 rows)
   - Same hyperparameters as Change 5, but increase `num_train_epochs` to 5–10 for larger dataset
   - Split: 85% train, 15% eval (stratified by method)

4. **Run full eval suite after training**
   - All 6 eval categories
   - Compare against baseline

5. **Optional: DPO preference alignment** (per `ALIGNMENT_AND_TUNING.md` §1, `RECIPE.md` §6)
   - Only if grounding is solid and style is good but tone needs refinement
   - Preference pairs from eval failures:
     - warm-direct ≻ cold-academic
     - graceful-refusal ≻ invented-doctrine
     - parable-first ≻ abstract-explanation
   - Use Unsloth's `DPOTrainer`, small LR, short run

6. **Deploy production model**
   - Merge checkpoint → copy to serving machine
   - Serve with MistralEngine (same path as Change 5 deployment)

### Success Criteria

- [ ] 300+ annotated rendering examples
- [ ] 100+ conversational mentor examples
- [ ] Full eval suite pass: grounding 100%, refusal 95%+, method-application 80%+
- [ ] Production model serves over all four protocol adapters (OpenAI, MCP, AG-UI, A2A)
- [ ] DPO improves tone without hurting grounding (if applied)

---

## Dependency Graph

```
annotation-guide ──┬──► annotate-50 ──┬──► lora-train ────────► production-lora
                   │                  │
                   └──► mentor-examples┘
                                       
rag-prototype ─────┬──► hebrew-bible
                   │
                   └──► eval-suite (can start after annotate-50)

eval-suite ─────────────────────────────────────────────────────► production-lora
```

---

## Parallelization Opportunities

| Parallel set | Changes | Why |
|---|---|---|
| **Wave 1** | `rag-prototype` + `annotation-guide` | No shared dependencies; one is code, one is documentation |
| **Wave 2** | `annotate-50` + `mentor-examples` + `eval-suite` | All need annotation guide; eval-suite can start as soon as 50 rows exist |
| **Wave 3** | `lora-train` + `hebrew-bible` | LoRA trains while Tanakh is prepared and integrated |
| **Wave 4** | `production-lora` | Depends on Wave 3 completion |

---

## OpenSpec Change Emission

OpenSpec detected (`openspec/` directory, `spec-driven` schema). Each change above should be created as:

```bash
openspec new change-rag-prototype    # → openspec/changes/rag-prototype/
openspec new change-annotation-guide # → openspec/changes/annotation-guide/
openspec new change-annotate-50      # → openspec/changes/annotate-50/
openspec new change-mentor-examples   # → openspec/changes/mentor-examples/
openspec new change-lora-train       # → openspec/changes/lora-train/
openspec new change-eval-suite       # → openspec/changes/eval-suite/
openspec new change-hebrew-bible     # → openspec/changes/hebrew-bible/
openspec new change-production-lora  # → openspec/changes/production-lora/
```

Each change artifact should include:
- `proposal.md` — goal, scope, non-goals, success criteria
- `tasks.md` — ordered task list from this plan
- Delta specs as needed (e.g., `specs/store.md` for `hebrew-bible`)

---

## Waypoint

**Current phase:** build-agent  
**Next change:** `rag-prototype`  
**Parallel opportunities:** `annotation-guide` can start simultaneously

---

## Sources

Repository:
- `CLAUDE.md` — gotchas, build sequence, principle
- `VISION.md` — product goal and persona contract
- `ALIGNMENT_AND_TUNING.md` — tuning pipeline, method targets, Bible scope
- `ARCHITECTURE.md` — Rust service design, adapter mapping
- `RECIPE.md` — complete Unsloth training script, Gemma 4 gotchas, eval + serve path
- `STEP8.md` — MistralEngine status, engine-swap
- `.kbd-orchestrator/phases/build-agent/assessment.md` — current state inventory

Firecrawl/web:
- Unsloth Gemma 4 Fine-tuning Guide: https://unsloth.ai/docs/models/gemma-4/train
- Gemma 4 E2B/E4B VRAM requirements: 8-10GB (E2B), 17GB (E4B LoRA), 22GB (31B QLoRA)
- Gemma 4 thinking mode: use `gemma-4` chat template, NOT `gemma-4-thinking`
- mistral.rs fork: `github.com/GQAdonis/mistral.rs` @ `b7746a85`
- CharacterBot deep persona simulation: arXiv:2502.12988 (ACL 2025)
---

# Plan addendum — General-Mentor Completion (2026-06-09)

**Goal:** finish the project so the twin can answer **any life question** — not only
corpus-direct ones — while remaining grounded and authentic. **Backend:** OpenSpec
(changes created at execution time, per phase precedent). **Inputs:** the 2026-06-09
assessment addendum, `docs/pre-planning/` (00–04), VISION.md.

## The architectural key: from binary gate to graduated grounding

Today the CoverageGate is binary: covered → answer; uncovered → refuse. A general
mentor cannot work that way — most life questions (career, marriage, grief, addiction,
purpose) are not *named* in the corpus, but his documented principles and moves apply.
VISION.md already authorizes this: "reasoning about modern situations using his
documented principles — achievable with limits."

The replacement is a **three-tier grounding router**:

| Tier | Trigger | Behavior |
|---|---|---|
| **T1 Direct** | strong retrieval hit on the question itself | current behavior: answer from cited sayings |
| **T2 Principle** | weak direct hit, but theme/principle-level hits exist | apply the documented principle + move to the situation, **transparently framed** ("the record doesn't speak to job interviews by name — but here is the principle that bears on it…"), citing the principle's source verses |
| **T3 Refuse** | no principled bridge (predictions, oracle asks, doctrine invention, identity claims) | in-voice refusal (current behavior; becomes *rare*) |

Authenticity is preserved because generalization happens by **principles applied
transparently** — never by pretending coverage. The L2 mentor records already model
exactly T2; this change makes the runtime match the data design. False refusals become
eval failures (the near-miss set, pre-planning 04 §3.3).

## Ordered change list (10–18)

| # | Change | Depends on | Effort | Agent | Pre-planning ref |
|---|--------|-----------|--------|-------|---|
| 10 | `graduated-gate` | — | 3–5 days | dev | (this section) |
| 11 | `principle-index` | taxonomy from 14 (early output) | ~1 week dev + tagging | dev + annotators | 01 §3, §7 |
| 12 | `hebrew-bible` | — (already pending) | ~2 weeks | dev | 01 §7 |
| 13 | `gospel-context-kb` | 12 patterns | 1–2 weeks | dev | ALIGNMENT §5 |
| 14 | `annotation-300` | guide fix (01 §2) | weeks (human) | 2 annotators | 01 (all) |
| 15 | `eval-life-questions` | — | ~1 week | dev | 04 §3 |
| 16 | `retrain-dual-base` | 14 (G1: ≥300 rows) | days | dev | 02 (all) |
| 17 | `episodic-memory` | — | ~1 week | dev | 03 §1–3 |
| 18 | `honesty-surface-ui` | UI exists | with UI work | dev | 03 §4–5 |

### Change 10: `graduated-gate` — the three-tier grounding router

1. Extend `RetrievalSet` scoring: alongside top-score, compute a **principle-level
   score** (hits on theme facets / graph-neighbor passages, once 11 lands; until then,
   approximate with a lower vector-similarity band: e.g. T1 ≥ high threshold, T2 in
   mid band, T3 below).
2. `gate.rs`: `evaluate_set` returns `Grounding::{Direct, Principle{sources}, Refuse{reason}}`
   instead of pass/fail. Thresholds are config, tuned against change 15's benchmark.
3. Prompt assembly: T2 answers get an additional instruction line requiring the
   transparent frame and forbidding invented specificity ("name the principle and its
   source; do not imply he addressed the user's literal situation").
4. New `AgentEvent::GroundingTier` (→ AG-UI custom chunk `x-jesus-twin/grounding-tier`)
   so the UI can show *how* an answer is grounded (feeds 18).
5. Tests: one fixture question per tier; false-refusal regression (near-miss set).

### Change 11: `principle-index` — make T2 retrievable

1. Define the **life-domain taxonomy** (~20 domains: money/provision, fear/anxiety,
   grief, marriage/divorce, parenting, conflict/forgiveness, ambition/status, honesty,
   illness, purpose/calling, enemies, doubt, prayer, wealth/generosity, judgment of
   others, work, power, loneliness, temptation, death) × the principles his sayings
   establish in each (each principle = a short statement + its source refs).
2. Tag all 927 passages with domains/principles. Model-drafted, human-reviewed
   (the 01 §9 drafting rules apply). This is an *annotation* product stored as new
   passage facets — not new content.
3. Store: add facet columns + index; retrieval gains a theme-expansion step (embed the
   question → nearest domains → boost passages tagged with them; keeps hybrid RRF).
4. The graph edges (move/parallels) extend with `principle` edges for mindmap + T2.

### Change 12: `hebrew-bible` — his source material as a labeled tool
Per original plan change 7 + pre-planning 01 §7: ingest JPS 1917 via `ingest_tanakh.py`
into a **separate corpus** (never blended with his words); retrieval surfaces it in a
"his source material" block; cross-refs from M09/remez annotations link the two.

### Change 13: `gospel-context-kb` — what he did, attestation-flagged
Non-red-letter narrative (his actions, settings) as a third labeled corpus with
attestation tiers (ALIGNMENT §5). Gives T2 answers access to *example by deed* ("when he
was confronted with X, he did Y") with the same citation discipline.

### Change 14: `annotation-300` — execute pre-planning 01 in full
Guide fix → ≥300 L1 rows (multi-rendering + synoptic parallels) → 75–150 L2 records
**stratified across the change-11 taxonomy** (every life domain gets L2 coverage —
this is what teaches the *application* of principles to arbitrary life questions) →
QC per 01 §6. Early deliverable for 11: the taxonomy itself (week 1).

### Change 15: `eval-life-questions` — the instrument for "any life question"
Pre-planning 04 §3 extensions **plus** a tier-correctness benchmark: ~60 questions
spanning all taxonomy domains, each labeled with its expected tier. Scoring: right tier
chosen? T2 frame present when required? citation correct? no invented specificity?
False refusal = fail; false confidence = worse fail. This set decides change 10's
thresholds and gates change 16.

### Change 16: `retrain-dual-base` — execute pre-planning 02
Gated on 14's G1 (≥300 rows). Gentle recipe (lr 2e-5 × 1 epoch), Qwen3-4B first then
Gemma 4 E4B, preregistered decision rule, gates G1–G5, ship winner or RAG-only rollback.

### Change 17: `episodic-memory` — execute pre-planning 03 §1–3
Fourth surface (facts about the user, never about Jesus); SurrealDB `memory` table;
record/retrieve/reflect; list/export/delete controls; isolation tests.

### Change 18: `honesty-surface-ui` — execute pre-planning 03 §4–5
Citations as chips, attestation badge, in-voice refusal styling, grounding-tier
indicator (from change 10's new chunk), the positioning line. Definition of done:
the two screenshots + a T2 answer screenshot showing the transparent frame.

## Waves (what runs in parallel)

```
Wave A (start now, parallel): 10 graduated-gate · 14 annotation (guide fix → rows) ·
                              15 eval-life-questions · 12 hebrew-bible
Wave B (taxonomy ready):      11 principle-index · 13 gospel-context-kb · 17 memory
Wave C (G1 met / UI exists):  16 retrain-dual-base · 18 honesty-surface-ui
```

Human annotation (14) and developer changes (10/12/15/17) never block each other.

## Definition of done (general-mentor)

1. The tier benchmark (15) passes: ≥90% tier-correct, zero false-confidence failures.
2. A live question in *every* taxonomy domain gets a grounded answer (T1 or T2) — none
   of the 20 domains dead-ends in refusal.
3. T3 refusals still fire on oracle/prediction/doctrine probes (the bright line holds).
4. Pre-planning definitions of done (04 §5) all met.

---

# Amendment 1 — Gate-calibration reconciliation + automation-first wave restructure (2026-06-09)

**Triggers:** (a) `docs/gate-calibration-claude-code-prompt.md` (source review of gate/retrieval);
(b) owner directive: re-cut waves so 1–2 fully-automated waves land immediate improvements
with **zero human annotation work**, deferring the manual program to a final wave.

## 1. Corrections from the gate-calibration doc

1. **Premise correction.** The addendum above says the gate "refuses anything without direct
   coverage." Wrong: `DEFAULT_COVERAGE_THRESHOLD = 0.0` — the gate passes any non-empty
   retrieval (RRF scores are strictly positive). An out-of-corpus political question was
   *answered silently* at fused score ~0.016. The system **over-answers today**; gate work is
   a live bright-line fix.
2. **Mechanism superseded.** Change 10's "similarity bands" sketch is replaced by the doc's
   **leg-agreement gate** (tier on how many of the 4 RRF legs agree; single-leg max 0.0164 <
   two-leg min 0.0250 — structural gap). Robust to the modern legs reviving later.
3. **Re-scoping.** `eval/` already contains grounding(30)/refusal(30)/method-application(15)/
   boundary(20)/retrieval files — change 15 extends, not creates. `eval/retrieval.jsonl`'s
   unreachable `min_score: 0.3` (RRF ceiling 0.066) is fixed inside the gate change
   ("one instrument, one scale"). Finding 4 confirms the false-refusal risk: modern-register
   personal questions are legitimately single-leg matches while `text_modern` is empty.

## 2. Change list updates

- **10 → split:**
  - **10a `gate-calibration`** — execute `docs/gate-calibration-claude-code-prompt.md` as
    written (PMPO halts; `gate calibrate` CLI instrument; 95-query calibration; preregistered
    rule: grounding 100% T1, method-application ≥80% engage, refusal ≥95% flagged/refused;
    leg-agreement three-outcome gate; `legs_matched` through `RetrievalSet`; BM25-only
    fallback ⇒ Tier 2; Tier-2 prompt addendum is context-assembly ONLY — SYSTEM_PROMPT
    untouched in all 4 parity locations; retrieval.jsonl rescale; `x-jesus-twin/low-confidence`
    chunk emission; FINDINGS.md baseline + recalibration trigger).
  - **10b `principle-tier`** — upgrade Tier 2 semantics from "decline what passages don't
    cover" to principle-bridging, once 11-v1 lands. Reuses 10a's mechanics; re-runs
    `gate calibrate` as regression.
- **NEW 19 `modern-legs-v1`** — machine-draft `text_modern` for **retrieval indexing only**
  (doc2query-class document expansion; research: Doc2Query++ arXiv:2510.09557 — generated
  expansions boost MAP/nDCG/recall; dual-index isolation ≅ our leg structure). Safety
  invariant: drafts live in a **sidecar build artifact**, flagged `machine_draft: true`,
  loaded only into the modern-leg indexes — **never** written to the xlsx, never emitted by
  `build_training_jsonl.py` as SFT labels, never displayed (`context_lines` uses
  `text_original` — verified). Revives the 2 dead legs now; human-verified renderings replace
  drafts row-by-row as Wave-3 annotation proceeds. Re-run `gate calibrate` after.
- **11 → `principle-index-v1` (machine-tagged)** — taxonomy + LLM auto-tagging of all 927
  passages as retrieval facets (established practice: metadata-driven RAG, arXiv:2510.24402).
  Safe without review: a wrong tag costs a retrieval miss, never fabricated content — answer
  text still comes only from real cited passages. Tags flagged `machine_tagged: true`; human
  review promotes them in Wave 3.
- **13 → adds automated attestation v1** — multiply-vs-single attestation computed
  *mechanically* from synoptic-parallel counts (already in the corpus graph). Scholarly
  tiering remains Wave 3.
- **14** unchanged in content, but its 1-day **guide fix moves to Wave 1** (dev task) so human
  annotation can start at any time without blocking on a developer.

## 3. The new waves (automation-first)

```
WAVE 1 — pure dev, zero human work, immediate quality wins
  10a gate-calibration        (bright-line fix + the measuring instrument; FIRST)
  19  modern-legs-v1          (revive dead retrieval legs via doc2query-style expansion)
  12  hebrew-bible            (Tanakh source tool)
  15  eval-life-questions     (tier benchmark; starts after 10a's Assess output)
  14.0 annotation-guide fix   (1 day, unblocks humans for whenever they start)

WAVE 2 — still zero human work (model-assisted, retrieval-metadata-only ⇒ safe)
  11  principle-index-v1      (machine-tagged taxonomy facets)
  10b principle-tier          (T2 principle-bridging — the general-mentor unlock)
  13  gospel-context-kb       (with automated attestation v1)
  17  episodic-memory

── after Wave 2 the twin answers any life question via T2 with real citations, calibrated
   gate, Tanakh + narrative context, relationship memory — "relatively close," no human hours ──

WAVE 3 — the hard manual work (gets it all the way correct)
  14  annotation-300          (human renderings + move labels; promotes/replaces machine
                               drafts row-by-row; 300-row milestone triggers gate recalibration)
  11r principle-index review  (humans verify/correct machine tags)
  13r attestation scholarly pass
  16  retrain-dual-base       (gated on ≥300 HUMAN-verified rows — machine drafts never train)
  18  honesty-surface-ui      (renders low-confidence/grounding-tier chunks; needs UI)
```

**The governing safety rule for Waves 1–2:** machine-generated text may influence *which*
passages are retrieved, never *what is said or trained*. Display = `text_original` only;
SFT = human-verified xlsx rows only. This keeps the bright line intact while capturing the
automation gains.

## 4. Definition of "relatively close" (end of Wave 2, measurable)

1. 10a preregistered rule passing; zero silent out-of-corpus answers (refusal-set proxy).
2. method-application engage-rate ≥80% with the revived modern legs (vs single-leg today).
3. A T2 answer in every taxonomy domain with the transparent principle frame + real citations.
4. Memory round-trip test passing (session-2 recall; isolation from corpus retrieval).
