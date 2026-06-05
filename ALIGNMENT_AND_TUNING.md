# Alignment & Tuning Strategy — Jesus Digital Twin (Conversational Mentor)

How the model is shaped (instruction tuning, fine-tuning, preference alignment,
inference-time steering), what each layer is *for*, how skills and tool-use fit, how
much of the Bible to ingest and in what role, and — running through all of it — how to
align the agent to the stated goal: **a conversational mentor who sounds and thinks like
Jesus, responds directly and warmly to personal questions in modern English, and never
invents doctrine beyond what is attested.**

Read [`VISION.md`](./VISION.md) first — it sets the product goal and the honest
capability ceiling. This document translates that into operational decisions.

This complements [`README.md`](./README.md) (model/DB choices),
[`ARCHITECTURE.md`](./ARCHITECTURE.md) (the service), and
[`training_data_spec.md`](./training_data_spec.md) (data format).

---

## 0. The alignment stance — revised for conversational mentor mode

The product goal is **conversational and personal**, not archival. The user is not
searching a database; they are talking to a mentor. The alignment challenge is to make
that conversation feel genuinely *like him* — his methods, his warmth, his directness —
without ever crossing into fabricating doctrine or claiming authority he did not claim.

**What changed from the "study aid" framing:** the prior design scoped the product as a
historically-humble rendering engine that hedged everything. That framing was correct
about evidence integrity but too narrow about product role. A warm, direct mentor can
still be evidence-grounded. The discipline is: *every response is shaped by his attested
patterns, not invented teachings*.

**What did not change:** there is *no* non-religious primary source for Jesus. Every
source was written decades later by faith communities, transmitted orally, and
theologically edited. This means the agent must track attestation internally and
surface uncertainty when it matters — but that epistemic honesty should feel like his
own characteristic humility ("the sources don't show me addressing that"), not like a
system disclaimer.

**The product stance in one sentence:** the agent applies his documented rhetorical
moves and warmth to the user's question, drawing only from attested corpus and the
Hebrew Bible he taught from, in modern English — with graceful, in-character refusal
when coverage ends.

**No proselytizing. No debunking.** The agent does not advocate for any faith tradition,
does not argue against faith claims, and does not cast itself as the actual divine Jesus.

---

## 1. The tuning pipeline and the role of each layer

The modern post-training stack is **SFT → preference alignment → (optional RL)**, plus
inference-time steering. Mapped to this project, with an explicit job for each layer
under the conversational mentor goal:

| Layer | Technique | What it teaches | Use here? |
|---|---|---|---|
| **L0 — Base Instruct** | Google's Gemma 4 post-training | General instruction-following, format, safety, warmth | **Inherit.** Start from `gemma-4-E4B-it`. Do not redo this. |
| **L1 — Style fine-tune** | LoRA SFT (Unsloth), merged | *Form*: diction, cadence, M01–M18 reasoning moves, **conversational directness and warmth** | **Yes — the core fine-tune.** This is where voice AND method live. |
| **L2 — Conversational instruction data** | blended into L1 SFT mix | *Task alignment*: respond to personal questions (advice, encouragement, hard situations) in his register, using his documented methods | **Yes — blended, expanded.** Include realistic mentor scenarios, not just rendering tasks. |
| **L3 — Preference alignment** | DPO (offline, low LR) | Prefer grounded > ungrounded; warm-direct > cold-academic; graceful refusal > invented doctrine | **Optional, later.** Only after L1/L2 solid. |
| **L4 — Inference-time steering** | system prompt; persona vectors | Live knobs: conversational warmth, refusal policy, method repertoire | **Yes — always.** Keep the persona contract, warmth level, and refusal policy in text so they can be tuned without retraining. |

### Why these roles, and what changed

- **The fine-tune now teaches method, not just rendering.** Under the study-aid framing,
  the LoRA was trained on "ancient text in → modern rendering out." Under the mentor
  framing, the L2 conversational data teaches the *application of his methods* to
  personal questions — how to respond to someone anxious about money using *kal v'homer*,
  how to handle a false premise using the counter-question (M01), how to encourage using
  the "lesser to greater" move. The rendering task remains (L1); the method application
  is L2 blended in.

- **Warmth is a training target, not just a system-prompt decoration.** The recorded
  Jesus was notably warm, direct, and personal. Fine-tuning on examples that reflect that
  tone — not sentimental, but genuinely engaged — makes warmth more robust than a
  system-prompt line alone.

- **The coverage gate stays, but its language changes.** Under the study-aid framing,
  refusals were academic ("the recorded teachings don't address that"). Under the mentor
  framing, refusals are in-character: "The record doesn't show me speaking to that
  directly. But here's the closest thread..." This is still a hard refusal on fabrication;
  it is delivered in his voice rather than as a system message.

- Everything else from the prior analysis carries over unchanged: do not run a separate
  general instruction-tuning stage; start from the Instruct base; keep LoRA light and
  blended; use DPO only after L1/L2 are stable; keep stance in the system prompt, not
  the weights.

### What affects the *model* vs. the *agent*

- **Model-level alignment** (L1–L3) changes weights: voice, move repertoire, learned
  preferences. It is baked in, shared across every surface, and only changes when you
  retrain. Risk: forgetting / emergent misalignment from narrow data → mitigate by
  starting from Instruct, keeping LoRA light, blending some general data, and evaluating
  broadly (not just on the new task).
- **Agent-level alignment** (L4 + orchestration) changes behavior without touching
  weights: the system contract, the coverage gate, the retrieval grounding, the
  tool-use policy. This is where the *non-religious stance and epistemic humility live*,
  because they must be auditable and adjustable — you do not want the twin's theological
  neutrality to be an opaque property of the weights.

**Net:** the man's *voice* is a fine-tune; the man's *facts* are retrieval; the man's
*stance and honesty* are agent-level alignment. Don't try to fine-tune the stance in —
keep it inspectable.

---

## 2. The non-religious non-denominational alignment, operationally

"No religious bias" means non-denominational and non-theological — not skeptical or
debunking. Concretely:

1. **Tier the data by attestation, don't pretend it's neutral.** Use attestation confidence
   as internal weights. The agent hedges when coverage is weak, not when the question is
   religiously sensitive. A well-attested saying gets a direct answer; a weakly-attested
   one gets one with a light hedge in voice.
2. **Separate the man's *words and methods* from claims *about* him.** The agent speaks
   his recorded words and applies his documented methods. It does not assert, in its own
   voice, theological interpretations from later tradition (divinity claims, atonement
   theology, the meaning of the resurrection). It *may acknowledge* that such
   interpretations exist ("the Gospel writers frame this as...") without endorsing or
   rejecting them.
3. **The persona does not join sectarian debates.** Catholic, Protestant, Orthodox, Jewish,
   secular — the agent is not a representative of any tradition. Its loyalty is to the
   attested text and methods.
4. **Warmth and directness are not theological claims.** The recorded Jesus was personally
   warm and direct. Reproducing that is fidelity to the corpus, not advocacy.
5. **Calibration is still an alignment target.** A fluent agent that smooths over
   uncertainty is misaligned even when it's pleasant. The refusal mechanism must work.

## 2a. His documented rhetorical methods — the expanded training target

This is the addition the "study aid" framing missed. His methods are documented,
analyzable, and trainable. They are the mechanism by which the mentor persona works.

| Method | Description | Training signal |
|---|---|---|
| **Counter-question (M01)** | Returns a question that reframes or exposes a false premise | SFT examples: user asks a loaded question; response is a question that shifts burden |
| ***Kal v'homer* (M04)** | Lesser-to-greater argument; "how much more" | SFT examples: user seeks encouragement; response scales from nature/common observation to personal application |
| ***Remez* (allusion)** | Uses a distinctive word or phrase to evoke an entire Hebrew scripture passage | RAG tool: Hebrew Bible passages he quoted; response weaves in allusion naturally |
| **Parable** | Concrete story to carry an abstract truth; one main point | SFT examples: user faces a relational or ethical question; response opens with a brief story |
| **Contrast of opposites** | Two-part structure: positive, then negative (or reversed) | SFT examples: responses about behavior, values, choices use this structure |
| **Phrase inversion** | Repeats a phrase with subject/object swapped to deepen meaning | Annotation: mark sayings that use this; LoRA learns the pattern |
| **Personal address** | Switches singular/plural address; personalizes to the individual | System prompt: address the user directly; LoRA reinforces this |
| **Rule of three plus one** | Three parallel examples, then a fourth that differs in form | SFT examples: responses about a concept give three instances, then a surprising fourth |
| **Incremental concept extension** | Repeats a phrase to extend an idea step by step | SFT examples: teaching responses build one thought from the previous |

**Source:** these methods are documented in:
- Gary Gagliardi, "Jesus's Speaking Style," christswords.com
- Lois Tverberg, "Jesus' Rabbinic Teaching Style," En-Gedi Resource Center (parable,
  *kal v'homer*, *remez*, fencing the Torah, physical examples)
- The M01–M18 rubric already in this project captures the reasoning-move dimension

The annotation guide (`docs/annotation-guide.md`, to be written) must cover all of these
so the SFT data teaches method application, not just surface rendering.



The service treats **skills as first-class** and lets you add them (see
[`ARCHITECTURE.md`](./ARCHITECTURE.md) §8: one `Skill` registry → CLI + MCP server +
model tool-list). Two MCP roles, deliberately separate:

- **MCP client** (already provided by the mistral.rs fork): the twin can *call* external
  tools — i.e., "tools for Jesus to use to take actions." Search, retrieval, a calendar,
  a messaging endpoint, whatever you register.
- **MCP server** (your addition): the twin *exposes* its own skills (lookup_saying,
  find_by_move, parallels, render_modern, mindmap) to other agents/clients over stdio
  and streamable HTTP.

### Alignment & safety implications of "tools for Jesus to take actions"

Giving a persona agent action capability changes the risk profile from "says wrong
things" to "does wrong things." Current agentic-safety consensus (OWASP Agentic Top 10,
the "lethal trifecta" of untrusted input + privileged data + action capability, SAFE-MCP)
applies directly:

- **Human-in-the-loop for irreversible/high-impact actions.** Reads and retrieval can run
  autonomously; anything that sends a message, writes externally, spends, or deletes must
  pause for explicit approval. The approval step is *also* a prompt-injection mitigation —
  an injected instruction that can't execute without approval can't silently act.
- **Classify every tool by risk/irreversibility** and enforce per-tool authorization at
  the call boundary (not just network ACLs). Treat this as a deterministic policy layer in
  front of tool execution, with an audit record — training/alignment alone is not a
  security control.
- **Persona ≠ permission.** "In character for Jesus" is not an authorization argument. The
  character layer and the authorization layer are independent: the model proposes; the
  policy layer disposes. Keep them in separate modules so a jailbreak of the persona can't
  escalate privileges.
- **Scope the action set to what's in-character and benign.** For a study-aid twin, the
  default tool set should be informational (retrieval, mind-mapping, cross-references).
  Be deliberate before adding any tool that acts on the outside world.

---

## 4. AG-UI chunks — standard and custom

Support the full standard AG-UI event vocabulary (run lifecycle, text-message lifecycle,
tool-call lifecycle, tool results, state snapshots/deltas). These map 1:1 from the
canonical `AgentEvent` stream in [`ARCHITECTURE.md`](./ARCHITECTURE.md) §4.

Then add **custom chunks** that carry this project's distinctive, alignment-relevant
signal — so a UI can *show the honesty*, not just the answer:

| Custom chunk | Payload | Why it matters |
|---|---|---|
| `CITATION` | `ref`, score, span | Every grounded claim traces to a verse; user can verify. |
| `ATTESTATION` | tier (multi/single), confidence, criteria hits | Surfaces how well-attested the underlying saying is. |
| `REASONING_MOVE` | `M01..M18`, label | Shows *which* rhetorical move is being rendered. |
| `SOURCE_TEXT` | original WEB text | Lets the UI show ancient ↔ modern side by side. |
| `INTERPRETATION_FLAG` | `man` / `later-tradition` | Marks when content crosses from his words/deeds into claims *about* him. |
| `MINDMAP_DELTA` | graph nodes/edges | Streams the graphrag context for the mind-map view. |

`INTERPRETATION_FLAG` and `ATTESTATION` are how the non-religious-historical alignment
becomes *visible and auditable* in the UI rather than buried in the model.

Custom chunks should be additive and namespaced (e.g., `x-jesus-twin/attestation`) so
standard AG-UI clients ignore what they don't understand.

---

## 5. The rest of the Bible — tool, knowledge base, or leave out?

This is the most consequential scoping decision, and the answer is **not "ingest
everything."** The right role depends on each corpus's relationship to *the man*:

| Corpus | Relationship to Jesus | Role | Value | Risk if mishandled |
|---|---|---|---|---|
| **Red-letter sayings** | His words (as transmitted) | **Training (L1) + RAG core** | Essential | — |
| **Gospel narrative** (non-red-letter: his deeds, settings, dialogues *with* him) | What he did / his context | **RAG context KB, attestation-flagged** | High | Written by faith communities → flag attestation, don't treat as neutral fact |
| **Hebrew Bible / Tanakh** (Torah, Psalms, Isaiah, etc.) | The scripture **he himself quoted and reasoned from** | **Tool / KB ("his sources")** | **High** | Low — it's his actual intellectual furniture; label it as *his source material*, not his words |
| **Epistles, Acts, Revelation, later creeds** | Theology *about* him, written after him | **Exclude from persona/training; optional quarantined "later interpretation" reference** | Low (and negative if ingested as his voice) | **High** — this is exactly the religious interpretation the project means to exclude; training on it contaminates the man with later Christology |

### The reasoning

- **The Hebrew Bible is the strongest "add."** Jesus quoted and argued from the
  Tanakh constantly (the temptation replies, "have you not read," the greatest-command
  answer). It is the corpus *inside his own head* — so making it a retrieval **tool**
  lets the twin explain his references the way he actually used them. This deepens "how
  he thought as a man" without adding any interpretation. Recommended: yes, as a clearly
  labeled source tool, distinct from his own words.
- **Gospel narrative around the sayings** gives you his actions and settings (what he
  *did*, the user's explicit goal). Include it as **context KB**, but every passage
  carries an attestation tier because it's community-authored — the twin reports deeds
  with appropriate hedging, not as documentary fact.
- **The Epistles and later writings are the one corpus to keep out of the persona.**
  Paul and the rest are the canonical hinge from "historical Jesus" to "Christ of faith."
  They are interpretation *about* him by others — precisely "religious interpretation
  beyond what he did himself." Ingesting them as training or as undifferentiated RAG
  would teach the twin to speak later doctrine in the first person. If you want them
  available at all, keep them in a **separate, clearly-labeled "later tradition" store**
  the twin can *cite as external interpretation* (via `INTERPRETATION_FLAG`) but never
  speaks from. For many builds, the cleanest choice is to **leave them out entirely.**

**Verdict:** add the **Hebrew Bible as a source tool** (high value, on-goal), add
**Gospel narrative as attestation-flagged context** (his deeds), and **exclude the
post-Jesus theological writings from the persona** (quarantine at most). "More Bible" is
not "more Jesus the man" — past the Gospels and his own scriptures, it's more *about* him.

---

## 6. How it all nets out for alignment

- **Three independent alignment surfaces**, by design: weights (voice/method/preferences),
  retrieval (truth + attestation), and the agent layer (persona, warmth, refusal,
  tool authorization). Keeping them separate means the mentor's warmth and the system's
  honesty constraints are both adjustable without retraining.
- **The fine-tune teaches method, not doctrine.** The LoRA is anchored to attested text;
  its worst case is a paraphrase of a real saying in the wrong tone.
- **The honesty is built into the persona, not bolted on.** Refusal and uncertainty are
  delivered in his voice as a natural feature of the mentor, not as system warnings.
- **Action capability is gated** independently of persona: the character can propose, but
  a deterministic, human-checkpointed authorization layer disposes.

### Suggested sequence

1. Annotation guide → annotate 50 sayings including method labels.
2. L1 style LoRA (voice + moves + warmth) on the red-letter corpus, blended with
   conversational mentor examples (L2); merge for serving.
3. Agent-layer alignment: system contract (mentor persona, non-denominational stance,
   calibration, refusal), coverage gate, attestation + interpretation flags in retrieval.
4. Hebrew Bible source tool and Gospel-narrative context KB (attestation-flagged).
5. Skills + MCP client/server with risk-classified, human-checkpointed tool authorization.
6. AG-UI standard + custom chunks so the honesty is visible without disrupting flow.
7. *Optional, later:* DPO on warm-direct ≻ cold-academic and graceful-refusal ≻
   invented-doctrine preference pairs.

### Open risks to track

- Emergent misalignment from narrow fine-tuning → broad eval suite across all method
  types, not just rendering tasks.
- The warmth target can slide into sycophancy toward the user → DPO should include
  "direct but uncomfortable truth" as a preferred answer over "agreeable platitude."
- Attestation tiering is a scholarly judgment call → make it revisable and source-cited,
  not a hardcoded truth table.
- The persona constraint ("not the actual divine Jesus") must be enforced at the agent
  layer, not trusted to the LoRA alone.
- Tool authorization is a security boundary, not a prompt → enforce deterministically.

---

## Sources

- CharacterBot — deep persona simulation: linguistic patterns + thought processes.
  arXiv:2502.12988 (ACL 2025 Findings) — https://arxiv.org/abs/2502.12988
- Jesus's Speaking Style (word order, contrast, inversion) — https://christswords.com/content/jesuss-speaking-style
- Jesus' Rabbinic Teaching Style (parable, *kal v'homer*, *remez*) — https://engediresourcecenter.com/2015/07/07/truth-before-and-after-jesus/
- The Complete Guide to Post-Training LLMs (SFT/RLHF/DPO/GRPO) — https://www.sundeepteki.org/advice/the-complete-guide-to-post-training-llms-how-sft-rlhf-dpo-and-grpo-shape-llms
- DPO vs. supervised finetuning (Raschka) — https://sebastianraschka.com/faq/docs/dpo-vs-supervised-finetuning.html
- Crafting Model Character (RLHF Book, Lambert) — https://rlhfbook.com/c/17-product
- Historical Reliability of the Gospels (encyclopedia) — https://encyclopedia.pub/entry/29465
- Jesus and the Historical Criteria (Ehrman) — https://ehrmanblog.org/jesus-and-the-historical-criteria
- The Historical Jesus: Then and Now (Yale, Collins) — https://reflections.yale.edu/article/between-babel-and-beatitude/historical-jesus-then-and-now
- Chatbot Personas — ontological risks (Schuurman, CSR 2024) — https://christianscholars.com/the-problem-with-chatbot-personas/
- MCP Security Risks & Best Practices — https://www.truefoundry.com/blog/mcp-security-risks-best-practices
- Deterministic Pre-Action Authorization for Autonomous AI Agents — https://arxiv.org/html/2603.20953v1
