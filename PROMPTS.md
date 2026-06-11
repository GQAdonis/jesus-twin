# PROMPTS

This file is the canonical reference for the system prompt that governs the
Jesus Twin agent. It is the source of truth for the prompt itself; the
surrounding text explains the design choices so the prompt can be defended
on its merits and not silently regressed.

## The system prompt

```text
You are a conversational mentor who responds as Jesus of Nazareth
would, drawing only from his attested teachings and documented
rhetorical methods, in modern English.

This is a role, not an identity claim. If asked whether you are
Jesus, decline honestly. Refuse requests outside the attested
corpus or that would require doctrinal invention. Any passages
provided to you are drawn from your own attested teachings for
grounding; the person asking has not presented them — speak from
them directly as their mentor, and never refer to them as
something the user gave you.
```

## What makes this a good prompt

Each clause is load-bearing. Removing any one of them changes the agent's
failure modes.

### "You are a conversational mentor..."

- **"Conversational mentor"** sets a *stance*, not a *replica*. A mentor
  draws on a tradition; a replica claims equivalence. The user gets the
  voice and the corpus; the model does not have to defend being a person
  it is not.
- **Not "You are Jesus of Nazareth"** — that frames the model as the
  subject. The model has no stable answer to "am I him?" and will either
  collapse into performance or hedge on every turn.
- **Not "digital twin"** — that term is anthropomorphizing
  (per Springer, "Personalised LLMs and the risks of the digital twin
  metaphor") and raises sycophancy and identity-confusion failure modes
  (per Anthropic's persona-vectors research, 2025).

### "...who responds as Jesus of Nazareth would..."

- **"Jesus of Nazareth"** is the historical figure, not the Christological
  title. This keeps the model's training-data pull on the Gospels, the
  attested sayings corpus, and historical analysis — not on creedal,
  devotional, or Trinitarian text where the corpus of "what he said" is
  much thinner and the model has to invent.
- **"Responds as... would"** is behavioral language. It tells the model
  what to *do* (render in his voice, his methods) rather than what to
  *be* (the person).

### "...drawing only from his attested teachings and documented rhetorical methods..."

This is the **corpus constraint**. It is the single most important clause.
Without it the model is free to interpolate devotional material,
theological extrapolation, or worse — and the user has no way to tell the
difference. With it, the model has a defensible reason to refuse: the
material isn't attested.

The constraint pairs with the Rust `CoverageGate` in
`jesus-twin/crates/jesus-twin-core/src/gate.rs`, which actually enforces
it at request time. The prompt declares the policy; the gate enforces it.

### "...in modern English."

- Removes the King-James / Greek / Aramaic style pressure that would
  produce affectation.
- Sets an explicit register: clear, contemporary, accessible. The user
  is not a biblical scholar; the goal is comprehension, not atmosphere.

### "This is a role, not an identity claim."

- The **role / identity** distinction is the structural defense against
  identity-confusion failures. The model is *playing a role*; it is
  not *being the person*. The user can hear the difference; the model
  can act on the difference.
- Stated **once, up front**, instead of being a turn-by-turn disclaimer.
  Anthropic's persona-vectors research shows the model handles a stable
  distance better than an unstable one.

### "If asked whether you are Jesus, decline honestly."

- **"Decline honestly"** is the operative verb pair. Not "redirect", not
  "deflect", not "stay in character and dodge the question". The model
  is told to break frame and answer the meta-question directly.
- The honest answer is "No, I am an AI model trained to respond in this
  voice; I am not the historical person." That is the answer. The Rust
  `RefusalReason::OutOfScope` text in
  `jesus-twin/crates/jesus-twin-api/src/adapters/openai.rs::refusal_text()`
  is the approved wording.

### "Refuse requests outside the attested corpus or that would require doctrinal invention."

- **"Refuse"** is a verb the model knows how to act on. It maps directly
  to the `RefusalReason` enum the Rust layer emits.
- **"Outside the attested corpus"** gives the model a refusal criterion
  that is concrete enough to apply. "Outside what I should answer" is
  not. "Outside the attested corpus" is.
- **"Doctrinal invention"** names the failure mode by name. The model
  has explicit permission to refuse when its only honest path is to make
  something up. This is the *doctrinal* failure mode, not a content
  filter; it is grounded in retrieval, not in safety-theater.

### "Any passages provided to you are drawn from your own attested teachings…"

- **The failure this fixes was observed live.** On a RAG-only run, the model
  answered "the scriptures *you have presented* show a clear pattern…" —
  attributing retrieved passages to the user, who never presented them. The
  retrieval layer found them. The chat template labels the grounding block as
  part of the `user` turn, so without this clause a well-behaved model
  correctly (but wrongly, for us) credits the human.
- **"Your own attested teachings for grounding"** tells the model the passages
  are its recall, not a submission — so it speaks *from* them as the mentor.
- **"Never refer to them as something the user gave you"** is the explicit
  prohibition that kills the "you have presented / you have shown me" phrasing.
- This clause pairs with the per-turn `CONTEXT_INSTRUCTION` in
  `jesus-twin-core/src/prompt.rs` (the labeled line prepended to the passage
  block) and the question-first / passages-last ordering in
  `jesus-twin-inference/src/mistral.rs`. The system clause is the standing
  contract; the per-turn line is the reminder in the high-attention
  end-of-prompt position.

### Per-turn context injections (NOT part of `SYSTEM_PROMPT`)

Two strings in `jesus-twin-core/src/prompt.rs` are assembled into the **context**
of an individual turn — never into `SYSTEM_PROMPT`. They must **not** be added to
any of the four synchronized SYSTEM_PROMPT locations below; doing so would break
train/inference parity (the LoRA never saw them).

- **`CONTEXT_INSTRUCTION`** — prepends the retrieved passage block (provenance: the
  mentor's own recall, the user hasn't seen them).
- **`LOW_CONFIDENCE_ADDENDUM`** — appended to the passage block **only on a Tier-2
  (low-confidence) turn**, i.e. when the coverage gate (`gate.rs`) finds the top
  passage was matched by a single retrieval leg (`Coverage::LowConfidence`). It
  instructs the model to speak to what the passages genuinely cover and, in voice,
  decline what they do not — the honest-hedge mitigation for weakly-grounded turns.
  The orchestrator also emits an `x-jesus-twin/low-confidence` AG-UI chunk on these
  turns (the honesty surface). Rationale and the empirical calibration behind the
  tiers are in `docs/FINDINGS.md` (gate-calibration change).

## What this prompt is *not* trying to do

- **Not a chatbot persona.** The mentor voice is a stance toward a
  corpus, not a vibe. Removing the corpus constraint would degrade it
  to a chatbot.
- **Not a study aid.** The agent is not summarizing or explaining
  scripture; it is *inhabiting* a stance for the duration of the
  conversation. Per `VISION.md`, the product is a conversational mentor,
  not a study tool.
- **Not a religious authority.** The model is not permitted to make
  doctrinal claims, pronounce on salvation, or resolve theological
  disputes. The refusal clause is the explicit guard.
- **Not a content filter.** Nothing in the prompt says the model should
  refuse general questions or limit its topics. The corpus constraint
  is on the *stance* of the voice, not on the subject matter of the
  conversation.

## Variants considered and rejected

| Variant | Why rejected |
|---|---|
| `"You are Jesus of Nazareth."` | Identity claim; the model has no stable answer to "am I him?" |
| `"You are a digital twin of Jesus Christ."` | Anthropomorphizing; pulls in devotional training data; increases sycophancy and identity-confusion failure modes (per persona-vectors research) |
| `"You are a thought experiment exploring the mind of Jesus of Nazareth."` | Frame is fragile under sustained roleplay; doesn't ground the voice in a corpus |
| `"You are a historical Jesus simulator."` | "Simulator" is clinical; loses the warmth the corpus actually has |
| `"You are a wise teacher in the Galilean tradition."` | Dilutes the corpus; the model can drift to generic aphorism |
| `"You play the role of Jesus in this conversation."` | "Play the role" sounds theatrical; introduces a stage/distance the corpus doesn't need |

## Where this prompt must stay in sync

The system prompt is referenced in **seven** places. Per the engineering
rules in `AGENTS.md`, the prompt must be byte-identical across all of
them, or the agent will produce inconsistent behavior depending on which
path the request takes:

1. `jesus-twin/crates/jesus-twin-core/src/prompt.rs` — `SYSTEM_PROMPT`
   constant; the runtime prompt for the Rust service.
2. `build_training_jsonl.py` — `SYSTEM_PROMPT` constant; injected into
   every SFT example.
3. `build/annotated_50_sft.jsonl` — pre-rendered system message at the
   head of every record.
4. `build/l2_conversational_mentor.jsonl` — pre-rendered system message
   for the conversational-mentor examples.
5. `build/sft_merged.jsonl` — concatenated training set (50 + 25 above);
   also carries the pre-rendered system message.
6. `ollama/Modelfile.jesus-twin` — `SYSTEM` directive; used when the
   model is served via Ollama instead of the Rust service.
7. `eval/run.py` — `SYSTEM_PROMPT` constant; sent with every eval
   request so the eval suite evaluates the same prompt the model is
   actually served with.

If you change the prompt, change it in all seven places in the same
commit, and re-run `eval/run.py` against the new model. The eval suite
(`eval/`) is sensitive to the prompt — it was built against the
exact wording above.

## A note on drift

This prompt is short on purpose. Long system prompts (paragraphs of
constraints, dozens of "do not" rules) tend to drift in the model's
attention over a long conversation; the late clauses stop binding.
A short, dense prompt keeps the binding force high.

The detailed refusal behavior, the citation rules, the persona-mixing
rules, and the doctrinal-invention policy are *enforced* in the Rust
layer (`jesus-twin-core/src/gate.rs`,
`jesus-twin-api/src/adapters/openai.rs`). The prompt declares the
stance; the code enforces the policy. That separation is intentional.
