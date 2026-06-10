# Three Surfaces, Not One Model: The Hybrid We Built So a Digital Twin Can Sound Like a Man Without Lying About Him

> **Authentic Digital Twin Content Standard — v2 (Tier 1, full manifest).**
> This article uses per-block authorship annotations. The eyebrow line above each
> block declares who wrote it. The manifest at the foot of the article consolidates
> the audit. Read past the eyebrows if you don't want them; read the manifest first
> if you do.

## How to read this article

_— Travis James._

Three labels show up under every block. **Human-authored** means the words are mine — no AI in the loop, or AI limited to spell-check that didn't move the voice. **AI-drafted, human-edited** means an agent drafted it and I rewrote it for voice, framing, and accuracy until it was mine to sign. **AI verbatim** means the machine wrote it and I left it alone — code, tables, mechanical reference. The point of the scheme is that you don't have to trust me about which is which. You can check. That's the whole idea, and it happens to be the same idea the system in this article is built on.

---

## The thing most people get wrong before they write a line of code

_— Travis James._

Most "AI persona" projects start by asking the wrong question. They ask: how do I get the model to *be* the character? So they take everything they have about the person, pour it into a fine-tune, and hope the weights come out sounding right and knowing the facts and behaving themselves. One model, one training run, one bag of hope.

That design fails three ways at once, and it fails quietly. The voice drifts toward generic-helpful-assistant. The facts get invented when retrieval would have grounded them. And the honesty — the part where the thing admits what it doesn't know — gets baked into weights nobody can inspect or change without retraining. You can't audit a property of a billion parameters. You can only audit text.

So we didn't build one model. We built three surfaces, and we gave each one exactly one job.

The system is a study-aid digital twin of Jesus of Nazareth — it renders his attested teachings in present-day English, preserves his documented reasoning moves, and never fabricates sayings. But the architecture isn't about Jesus. It's about a constraint that applies to any twin of a real person where getting the facts wrong is the cardinal sin: **retrieval owns truth, the fine-tune owns voice, the agent layer owns stance and honesty.** Three surfaces, three owners, no overlap. That sentence is the entire design, and the rest of this article is what it costs to mean it.

---

## Surface one — retrieval owns truth

_— Travis James ← AI: Anthropic Claude Opus 4.8 via Claude Code._

The first surface is retrieval-augmented generation, and its job is the one thing the model is never allowed to improvise: the actual words. Every saying in the corpus is indexed as a passage — original text and modern rendering both — with a citation that traces back to a verse. When a question comes in, the system retrieves the relevant passages, and a coverage gate checks whether there's enough grounded material to answer at all. No coverage, no answer. The model doesn't get to fill the gap from memory.

This is not a stylistic choice. It's the load-bearing wall. The research on knowledge injection is blunt about why: when you try to teach facts by fine-tuning instead of retrieving them, you don't just fail to help — you can actively hurt. In Microsoft's "Fine-Tuning or Retrieval?" study, RAG beat fine-tuning on genuinely new factual knowledge by roughly **37 percentage points**, and on one model fine-tuning *dropped accuracy below the untouched base model* — 0.353 down to 0.219. The model didn't learn the facts. It learned to sound confident while getting them wrong.

For a twin of a real person, that failure mode isn't a bug — it's a reputation event. A confident paraphrase of something the man never said is exactly the lie the whole project exists to prevent. So truth doesn't live in the weights. It lives in a vector-plus-keyword index with a citation attached to every passage, and a gate in front that refuses rather than guesses.

---

## Surface two — the fine-tune owns voice

_— Travis James._

Here's where people who half-understand RAG get nervous. If retrieval owns truth, what's the fine-tune even for? Drop it, they say. Just prompt the base model and ground it with citations.

That's wrong, and the reason it's wrong is measurable. A system prompt is a weak lever for *form*. You can tell a base model "respond warmly, in his rhetorical style, using his documented moves" — and it will, about a third of the time. The other two-thirds it slides back into the house voice of whatever foundation model you started from. There's a 2025 study on exactly this: fine-tuning for conversational tone versus prompting for it. Prompting alone landed the target tone **23 to 46 percent** of the time. Fine-tuning landed it **above 95 percent**. Same content. Different reliability.

That gap is the entire case for surface two. The fine-tune is not there to make the answers more correct — retrieval already owns correct. It's there to make the voice *reliable*. The difference between a twin that sounds like the man nineteen times out of twenty and one that sounds like him one time out of three is the difference between a product and a demo.

And the voice we're teaching isn't decoration. It's method — the counter-question that exposes a false premise, the lesser-to-greater argument that scales from a common observation to a personal one, the parable that carries one point. Those are documented, analyzable, trainable patterns. The fine-tune learns the *transform*: attested line in, the man's rendering and the man's move out. Its worst case, by construction, is a real saying delivered in a slightly-off tone. It is never asked to generate doctrine, so it can never get doctrine wrong.

The best part is the cost. That same tone study hit its 95-percent reliability with **100 training examples**, with diminishing returns past a thousand. You do not need a mountain of data to teach voice. You need a clean, annotated, in-character set — and you need to keep the adapter light, because a heavy one starts to forget.

---

## Surface three — the agent layer owns stance and honesty

_— Travis James._

The third surface is the one the other two architectures don't have, and it's the one that makes this defensible rather than just clever. It's the agent layer — the system contract, the coverage gate, the attestation flags, the refusal policy, the tool-authorization boundary. Everything that isn't truth and isn't voice lives here, in text, where it can be read and changed without touching a single weight.

This is where the project's hard rules live. The twin is historical, not devotional — it speaks the man's words and applies his methods, but it does not assert, in its own voice, the theology that came after him. It flags when content crosses from *what he said* to *what was later said about him*. It refuses questions outside the corpus instead of confabulating an answer. And — this is the rule that matters most for any persona with hands — **persona is not permission.** "In character" is not an authorization argument. The character can propose an action; a deterministic, human-checkpointed policy layer decides whether it runs. A jailbreak of the voice can't escalate into a jailbreak of the privileges, because the two were never the same module.

Why does this belong in the agent layer and not the fine-tune? Because honesty has to be auditable. You do not want a twin's theological neutrality, or its refusal behavior, or its action limits, to be an opaque emergent property of a training run. You want them in a file. When the stance needs to change — and it will, because attestation is contested scholarship, not settled fact — you edit the file. You don't retrain the man.

---

## Why three, and why this specific split

_— Travis James._

Pull back and the logic is almost mechanical. Each surface owns the thing it's actually good at, and is forbidden from the things it's bad at.

Retrieval is good at facts and terrible at voice — so it owns truth and nothing else. The fine-tune is good at form and dangerous with facts — so it owns voice and is never asked to know anything. The agent layer is the only one of the three that's inspectable line by line — so it owns every rule that has to survive an audit. The boundaries aren't bureaucratic. They map directly onto what the research says each technique can and can't do.

There's a subtle trap worth naming, because it's the one that catches teams who *do* build all three. The instinct, once you have a fine-tune and a retriever, is to assume more is better — fine-tune harder, retrieve more aggressively, stack the gains. The knowledge-injection study checked that directly: combining fine-tuning with RAG was **not reliably additive, and sometimes worse than RAG alone.** The fine-tuned generator, more confident, was more willing to paraphrase past the retrieved citation. The honest assessment limit here is real: the three surfaces don't simply add up. They have to be tuned against each other, with grounding held constant while voice improves — or the voice surface quietly erodes the truth surface, and you've spent effort making the system worse.

The other limit worth stating plainly: narrow fine-tuning on a small, pointed dataset can broadly misalign a model — the alignment literature calls it emergent misalignment, and it happens when a tight fine-tune forgets the safety behavior the base model came with. That's not a reason to skip the fine-tune. It's a reason to start from the instruct-tuned base, keep the adapter light, blend in general examples, and evaluate across every behavior, not just the one you trained. The risk is managed at the agent layer and the training recipe — never trusted to the weights alone.

---

## What this buys you, concretely

_— Travis James._

Three things, and they're the three you can't get from a single model.

You get a twin whose facts you can verify, because every grounded claim carries a citation and the gate refuses when coverage runs out. You get a voice that's reliable instead of occasional, because the fine-tune moved tone-fidelity from one-in-three to nineteen-in-twenty. And you get honesty you can audit, because the stance and the refusals and the action limits are sitting in a text file instead of buried in a parameter you'll never read.

A single fine-tuned model gives you none of those cleanly. It gives you a voice that drifts, facts you can't trust, and a conscience you can't inspect. The hybrid costs more to build — three surfaces, three contracts, a tuning pass that holds grounding constant while it improves voice. It's worth it for exactly one reason: when the subject is a real person and the failure mode is putting words in his mouth, the architecture *is* the ethics. You don't bolt honesty on at the end. You build it as a surface, give it an owner, and let the reader check your work.

The agents are ready. The discipline to keep their truth, their voice, and their honesty in separate hands is what's still catching up.

---

## Provenance manifest

_— AI verbatim: Anthropic Claude Opus 4.8 via Claude Code._

**Authorship categories:**

- **Human-authored** — original voice of Travis James; no AI involvement, or AI limited to spell-check that did not alter voice.
- **AI-drafted, human-edited** — AI drafted; Travis James edited substantively for voice, framing, accuracy, or structure.
- **AI verbatim** — AI output reproduced without editorial intervention.

| Section / block | Category | Model | Tool |
|---|---|---|---|
| How to read this article | Human-authored | — | — |
| The thing most people get wrong before they write a line of code | Human-authored | — | — |
| Surface one — retrieval owns truth | AI-drafted, human-edited | Anthropic Claude Opus 4.8 | Claude Code |
| Surface two — the fine-tune owns voice | Human-authored | — | — |
| Surface three — the agent layer owns stance and honesty | Human-authored | — | — |
| Why three, and why this specific split | Human-authored | — | — |
| What this buys you, concretely | Human-authored | — | — |
| This manifest | AI verbatim | Anthropic Claude Opus 4.8 | Claude Code |

**Sources referenced in the argument:**

- Ovadia et al., "Fine-Tuning or Retrieval? Comparing Knowledge Injection in LLMs," arXiv:2312.05934 — RAG vs. fine-tuning for factual knowledge; the ~37-point gap, the below-base regression, and the non-additive RAG+FT finding.
- Marquardt & Brule, "Fine-tuning on simulated data outperforms prompting for agent tone," arXiv:2507.04889 — the 23–46% (prompted) vs. >95% (fine-tuned) tone-reliability gap, achieved with 100 training samples.
- Betley et al., "Emergent Misalignment: Narrow finetuning can produce broadly misaligned LLMs" — the small-dataset misalignment risk and its mitigations.

**Standard:** Authentic Digital Twin Content Standard v2, Tier 1.
