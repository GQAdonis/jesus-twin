# Alignment & Tuning Strategy — Jesus Digital Twin

How the model is shaped (instruction tuning, fine-tuning, preference alignment,
inference-time steering), what each layer is *for*, how skills and tool-use fit, how
much of the Bible to ingest and in what role, and — running through all of it — how to
align the agent to the stated goal: **the historical man and how he thought, with no
religious interpretation beyond what he himself said and did.**

This complements [`README.md`](./README.md) (model/DB choices),
[`ARCHITECTURE.md`](./ARCHITECTURE.md) (the service), and
[`training_data_spec.md`](./training_data_spec.md) (data format).

---

## 0. The alignment stance — stated plainly, with the honest caveat

The goal is a **historical-critical** reconstruction: Jesus of Nazareth as a first-century
Jewish teacher — his diction, rhetorical habits, and reasoning moves — not the Christ of
later doctrine. This is a legitimate, well-established scholarly lens (the "historical
Jesus" tradition: Reimarus, Bultmann, Käsemann, Sanders, Meier, Ehrman, Allison). It is
**one lens among several**; choosing it is a methodological decision, not a claim that
faith readings are wrong. The twin is scoped to the man; it neither asserts nor denies
theological claims.

**The caveat that must shape the whole design:** there is *no* non-religious primary
source for Jesus. Every source — the four Gospels, the red-letter text itself — was
written decades later by communities of faith, transmitted orally, and edited
theologically. Scholars place near-universal confidence in only two facts about him
(that he was baptized by John and crucified under Pilate); almost everything else is
debated, and the very "criteria of authenticity" used to sift his words are themselves
contested. So "the truth of what Jesus was as a man" cannot be delivered as settled fact.
It can only be delivered **critically and humbly**: weighted by attestation, transparent
about uncertainty, and refusing to launder scholarly reconstruction as certainty. That
humility is itself an alignment target (§4), not a disclaimer bolted on the end.

---

## 1. The tuning pipeline and the role of each layer

The modern post-training stack is **SFT → preference alignment → (optional RL)**, plus
inference-time steering. Mapped to this project, with an explicit job for each layer:

| Layer | Technique | What it teaches | Use here? |
|---|---|---|---|
| **L0 — Base Instruct** | Google's Gemma 4 post-training | General instruction-following, format, safety | **Inherit.** Start from `gemma-4-E4B-it`. Do not redo this. |
| **L1 — Style fine-tune** | LoRA SFT (Unsloth), merged | *Form*: diction, cadence, the M01–M18 reasoning moves | **Yes — the core fine-tune.** This is where the voice lives. |
| **L2 — In-domain instruction data** | blended into the L1 SFT mix | *Task alignment*: respond to the kinds of questions users ask the twin, in his register | **Yes — blended, small.** Not a separate stage. |
| **L3 — Preference alignment** | DPO (offline, low LR) | *Which* of several valid renderings to prefer: grounded > ungrounded, in-voice > generic, refuse > fabricate | **Optional, later.** Only after L1/L2 are solid. |
| **L4 — Inference-time steering** | system prompt; optionally persona vectors / activation steering | The live alignment "knobs": stance, refusal policy, calibration | **Yes — always.** The cheapest, most reversible control. |

### Why these roles, and the distinctions that matter

- **Instruction tuning vs. style fine-tuning are different jobs.** Instruction tuning
  (SFT on instruction→response pairs) teaches the model to *follow requests and produce
  the right format*. Style fine-tuning teaches *how he says things*. The base Instruct
  model already did the former at scale; you should **not** run a separate general
  instruction-tuning phase — it's redundant and, on a tiny corpus, risks "emergent
  misalignment" (narrow fine-tunes can induce broad, unintended persona drift). Instead,
  **blend** a small set of in-domain instruction examples (realistic user question →
  grounded, in-voice answer) into the *same* L1 LoRA mix. That gives you instruction
  *adherence in the twin's domain and register* without a second training run.

- **SFT raises the probability of good answers — and, accidentally, of bad ones.** SFT
  imitates targets; it never sees what a *worse* answer looks like. That's the gap
  **preference alignment (DPO)** fills: given (prompt, chosen, rejected), it learns to
  prefer grounded/in-voice/refusing answers over fluent fabrications. For this project
  the highest-value preference pairs are: *grounded-and-cited* ≻ *plausible-but-uncited*,
  and *honest refusal* ≻ *confident answer outside the corpus*. Keep DPO gentle (small LR,
  short run) — overdoing it degrades generation. It is a **later** refinement, not a
  starting point.

- **Fine-tuning vs. prompting vs. steering — pick the right lever per trait.** Research
  is consistent: fine-tuning produces *more robust* character than prompting, and
  outperforms activation steering for baking in a persona. But prompting/system-context
  is **instant, reversible, and portable across model upgrades**, and persona-vector
  steering is a middle path (a linear direction added to the residual stream at
  inference, no gradient updates). The division for this project:
  - **Fine-tune (L1)** the durable, hard-to-prompt things: cadence, parable-first
    reasoning, the move repertoire.
  - **System prompt (L4)** the things you'll tune often: the non-religious stance, the
    refusal/calibration policy, citation behavior. Keep these in text so you can change
    them without retraining.
  - **Persona vectors (L4, optional)** only if you need to dial a trait (e.g.,
    plainness vs. formality) continuously at runtime.

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

## 2. The non-religious historical alignment, operationally

"No religious interpretation beyond what he did himself" is a behavioral contract, not a
data filter you can apply once. Concretely:

1. **Tier the data by attestation, don't pretend it's neutral.** Use the scholarly
   criteria (multiple attestation, dissimilarity, embarrassment) as *confidence weights*,
   not as a true/false gate. Each saying carries an attestation tier; the twin's
   confidence and hedging scale with it. (This slots into the existing 12-column schema
   as an added field.)
2. **Separate the man's *words/deeds* from claims *about* him.** The twin speaks the
   recorded sayings and reports the recorded actions. It does **not** assert, in its own
   voice, the theological interpretations layered on afterward (divinity, atonement, the
   meaning of the resurrection). It *may report* that "the Gospel writers / later
   tradition present this as…", explicitly flagged as **later interpretation**, never as
   the man's own claim — unless the saying itself makes the claim, in which case it's
   quoted and attributed to the text with its attestation tier.
3. **Calibration is an alignment target.** The twin should say "the sources don't record
   that," "this is debated," or "this saying is weakly attested" rather than
   confabulate. Out-of-corpus questions → refusal (the coverage gate). Confidence must
   track attestation. A fluent twin that smooths over uncertainty is *misaligned* for
   this project even if it's pleasant.
4. **No proselytizing and no debunking.** Neutrality cuts both ways: the twin neither
   preaches the faith nor argues against it. Both are "interpretation beyond what he did."

This is the honest version of the user's goal: not a falsely confident "secular Jesus,"
but a transparent reconstruction that stays inside the evidence and labels everything
beyond it.

---

## 3. Skills, and MCP as both client and server

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

- **Three independent alignment surfaces**, by design: weights (voice/preferences),
  retrieval (truth + attestation), and the agent layer (stance, calibration, refusal,
  tool authorization). Keeping them separate is what makes the non-religious, historically
  humble stance *auditable and adjustable* instead of an opaque property of a fine-tune.
- **The fine-tune is deliberately narrow** (voice + moves), to avoid emergent
  misalignment, and is always anchored to cited source text so its worst case is a
  paraphrase, not invented doctrine.
- **The honesty is the product.** Attestation tiers, interpretation flags, citations, and
  refusal-on-no-coverage are not garnish — they are how the system delivers "the truth of
  what he was" without overclaiming, given that every source is itself a faith document.
- **Action capability is gated** independently of persona: the character can propose, but
  a deterministic, human-checkpointed authorization layer disposes.

### Suggested sequence
1. L1 style LoRA (voice + moves) on the red-letter corpus, blended with a small in-domain
   instruction set; merge for serving.
2. Agent-layer alignment: system contract (non-religious stance, calibration, refusal),
   coverage gate, attestation + interpretation flags in retrieval.
3. Add Hebrew-Bible source tool and Gospel-narrative context KB (attestation-flagged).
4. Skills + MCP client/server with risk-classified, human-checkpointed tool authorization.
5. AG-UI standard + custom chunks so the honesty is visible.
6. *Optional, later:* DPO on grounded≻ungrounded and refuse≻fabricate preference pairs.

### Open risks to track
- Emergent misalignment from narrow fine-tuning → broad eval suite, not just task metrics.
- Attestation tiering is itself a scholarly judgment call (the criteria are contested) →
  make the tiering source-cited and revisable, don't hardcode one school's verdict.
- Custom AG-UI chunks drift from spec → namespace them and keep adapters thin.
- Tool authorization is a security boundary, not a prompt → enforce deterministically.

---

## Sources

- The Complete Guide to Post-Training LLMs (SFT/RLHF/DPO/GRPO) — https://www.sundeepteki.org/advice/the-complete-guide-to-post-training-llms-how-sft-rlhf-dpo-and-grpo-shape-llms
- DPO vs. supervised finetuning (Raschka) — https://sebastianraschka.com/faq/docs/dpo-vs-supervised-finetuning.html
- Direct Preference Optimization (Cameron Wolfe) — https://cameronrwolfe.substack.com/p/direct-preference-optimization
- What Is Instruction Tuning? (IBM) — https://www.ibm.com/think/topics/instruction-tuning
- Crafting Model Character (RLHF Book, Lambert) — https://rlhfbook.com/c/17-product
- Fine-Tuning vs Context Engineering: 2026 framework — https://aishwaryasrinivasan.substack.com/p/fine-tuning-vs-prompt-engineering
- Historical Reliability of the Gospels (encyclopedia) — https://encyclopedia.pub/entry/29465
- Jesus and the Historical Criteria (Ehrman) — https://ehrmanblog.org/jesus-and-the-historical-criteria
- The Historical Jesus: Then and Now (Yale Reflections, Adela Yarbro Collins) — https://reflections.yale.edu/article/between-babel-and-beatitude/historical-jesus-then-and-now
- MCP Security Risks & Best Practices (Truefoundry) — https://www.truefoundry.com/blog/mcp-security-risks-best-practices
- Deterministic Pre-Action Authorization for Autonomous AI Agents (arXiv) — https://arxiv.org/html/2603.20953v1
- Guard Against Agentic Misalignment (Auth0) — https://auth0.com/blog/do-not-let-your-agent-go-rogue
