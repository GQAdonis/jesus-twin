# Training Data Specification — Jesus Digital Twin

This spec defines how the annotated red-letter corpus becomes the two datasets
the system actually consumes: a **style LoRA** dataset and a **RAG corpus**.
It builds directly on the existing artifacts — `jesus_full_red_letter.xlsx`
(489 sayings, 12-column schema) and the `M01–M18` Reasoning Move Rubric — and is
implemented by `build_training_jsonl.py`.

The governing design decision, carried over from the prior analysis: **fine-tuning
teaches *form* (voice, cadence, reasoning move); retrieval supplies *content*
(what he actually said, with a citation).** Every rule below follows from keeping
those two jobs separate so the model never has to *invent* substance.

---

## 1. Source of truth: the annotation sheet

The 12 columns already in the corpus map to three roles:

| Column | Role | Used by |
|---|---|---|
| ID | Stable key / dedupe / split | both |
| Scripture | Citation (must survive to output) | RAG |
| Author of Book | Provenance | RAG |
| Original (WEB) | Ground-truth text (public domain) | RAG + SFT input |
| **Modern Rendering** | Target voice (the label) | **SFT label** |
| Situational Context | Conditioning | SFT input + RAG |
| Sentiment | Conditioning / eval facet | SFT meta |
| Audience Present | Conditioning | SFT input |
| Approx. Age | Filter / facet | RAG meta |
| Location | Filter / facet | RAG meta |
| Reason Present / Occasion | Conditioning | SFT input + RAG |
| **Reasoning Move** | Form label (M01–M18) | **SFT meta / weighting** |

A row is **training-ready** only when both `Modern Rendering` and `Reasoning Move`
are filled. Rows missing either are still emitted to the RAG corpus (the original
text is enough to retrieve and cite) but are skipped for SFT. The converter prints
the ready/total split so annotation progress is visible.

---

## 2. The SFT (style LoRA) record

Format: OpenAI-style `messages` chat records, one JSON object per line (JSONL).
This is accepted by both Alibaba Model Studio fine-tuning and local trainers
(Unsloth/PEFT/Axolotl), so the same file ports across either path.

```json
{
  "messages": [
    {"role": "system", "content": "<fixed system contract>"},
    {"role": "user", "content": "Context: …\nRender the following saying (REF) in present-day English…:\n“<Original WEB text>”"},
    {"role": "assistant", "content": "<Modern Rendering>"}
  ],
  "meta": {"id": "...", "ref": "...", "move": "M02", "sentiment": "...", "audience": "..."}
}
```

Rules:

- **The system prompt is fixed and identical at train and inference time.** It is
  the behavioral contract (speak only from source, never invent, preserve the
  move). Conditioning the adapter on a constant system message makes the learned
  behavior reproducible at serving time.
- **The Original WEB text goes in the *user* turn, the Modern Rendering is the
  *label*.** The adapter therefore learns the *transform* (ancient → modern voice
  that keeps the force), not a free-floating "talk like Jesus" style. This is what
  protects against fabrication: the model is always anchored to a real source line.
- **`Reasoning Move` is metadata, never visible text.** Do not paste "M02" into the
  prompt. Use it for (a) **stratified splitting** so every move appears in both
  train and eval, (b) **class weighting / oversampling** of rare moves (M16, M17
  occur once or twice), and (c) **per-move eval** (see §5).
- **Context is assembled, not dumped.** Situational Context + Audience + Occasion
  are concatenated into one short conditioning line; empty fields are omitted.

### Why this shape and not "instruction → free generation"

A naïve persona dataset (`"What do you think about X?" → "<invented Jesus-like
answer>"`) trains the model to *generate doctrine*, which is exactly the failure
mode to avoid. Anchoring every label to a cited source line means the worst-case
output is a paraphrase of real text, not a hallucinated teaching.

---

## 3. The RAG record

Every saying with original text becomes one retrievable passage:

```json
{
  "id": "wj-42", "ref": "Mark 12:17",
  "book_author": "Mark (trad.)",
  "text_original": "…", "text_modern": "…",
  "context": "…", "location": "…", "occasion": "…",
  "move": "M02", "translation": "World English Bible (public domain)"
}
```

- Index **both** `text_original` and `text_modern` as embeddings, so a query in
  either register retrieves the passage; always cite `ref` in the answer.
- `move`, `location`, `audience`, `Approx. Age` are metadata filters for scoped
  retrieval ("what did he say to opponents?", "sayings from the passion week").
- One translation only (WEB) keeps it public-domain and citation-clean. Other
  translations can be added later as parallel fields, never as separate rows.

---

## 4. Data volume, augmentation, and the honest limit

- 489 sayings → on the order of a few hundred training-ready records once
  annotation completes. That is **enough for a style LoRA, thin for anything more.**
- **Permitted augmentation:** multiple *human-checked* modern renderings per saying
  (2–3 registers — plain, formal, conversational), each a separate record sharing
  the source line. This multiplies data without inventing content.
- **Forbidden augmentation:** synthetic Q→A pairs where a model generates new
  "Jesus answers." That reintroduces fabrication and trains the voice on a
  paraphraser's hallucinations. If you must scale, scale *renderings of real lines*,
  not *new lines*.
- Hold out ~10% (deterministic by ID hash) for eval; stratify by Reasoning Move.

---

## 5. Evaluation hooks baked into the data

Because every record carries `ref`, `move`, and `sentiment`, evaluation can be
automatic and faceted:

- **Grounding / no-fabrication (most important):** for each eval item, the model's
  output must be entailed by `text_original`. Score with an NLI/entailment check or
  an LLM judge; any claim not supported by the source line is a hard failure.
- **Style fidelity:** embedding similarity and an LLM judge comparing output to the
  held-out `Modern Rendering`, *per Reasoning Move* — so you can see e.g. that the
  adapter nails M04 (a fortiori) but flattens M06 (hyperbole).
- **Citation integrity:** the served answer must surface the correct `ref`.
- **Refusal behavior:** seed adversarial prompts with no source coverage
  ("What did Jesus say about cryptocurrency?"); correct behavior is an explicit
  "the recorded teachings don't address that," not a confident paraphrase.

---

## 6. Pipeline summary

```
jesus_full_red_letter.xlsx  (annotate Modern Rendering + Reasoning Move)
        │
        ▼  build_training_jsonl.py
   ┌──────────────┬───────────────┬──────────────────┐
   │ sft_style    │ eval_heldout  │ rag_corpus       │
   │ .jsonl       │ .jsonl        │ .jsonl           │
   └──────────────┴───────────────┴──────────────────┘
        │                │                │
   LoRA training     style+grounding   embed + index
   (voice/form)       evaluation       (content/citation)
```

See `digital_twin_architecture.md` for how these feed the running system.
