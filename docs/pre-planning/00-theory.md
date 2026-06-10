# 00 — Theory: Why the System Is Built This Way

> Audience: junior AI developer. Terms are defined the first time they appear. Citations
> are to published work you can (and should) read.

## 1. The problem, stated as an engineering problem

We want an AI agent a person can talk to as if speaking with Jesus of Nazareth — mentor,
teacher, counselor — in modern English. The hard constraint that makes this an unusual
engineering problem: **the agent must never fabricate a saying or invent doctrine.** The
entire primary corpus is ~489 sayings (927 verse-rows) from the public-domain World
English Bible, plus the Hebrew Bible he quoted, plus narrative context written decades
later by faith communities.

Compare this to a normal chatbot persona: if a "pirate assistant" invents a phrase no
pirate ever said, nothing is harmed. Here, a confident paraphrase of something the man
never said is precisely the failure the project exists to prevent. So the design question
is not "how do we make the model act like Jesus?" but **"how do we get his voice and his
reasoning patterns without ever asking the model to know or invent his content?"**

## 2. Background concepts (read if any term is unfamiliar)

- **LLM (large language model):** a neural network trained to predict the next token
  (word piece) of text. Everything it "knows" is encoded in billions of numeric weights.
- **Fine-tuning / SFT (supervised fine-tuning):** continuing training on your own
  examples (input → desired output pairs) so the model's behavior shifts toward them.
- **LoRA (Low-Rank Adaptation):** a cheap fine-tuning method. Instead of updating all
  weights, you train two small matrices per layer whose product approximates the change.
  Result: a small "adapter" you can merge into the model. Trains on one consumer GPU.
- **RAG (retrieval-augmented generation):** instead of asking the model to recall facts
  from its weights, you *search a database* for relevant passages and paste them into the
  prompt. The model paraphrases what is in front of it rather than recalling.
- **Embedding:** a vector (list of numbers) representing a text's meaning, so "similar
  meaning" becomes "nearby vector." Used for the semantic half of retrieval.
- **BM25:** a classic keyword-relevance scoring formula. Used for the lexical half of
  retrieval. Our store fuses both rankings (vector + BM25) with RRF (reciprocal rank
  fusion) — a simple formula that rewards items ranked high by either method.
- **System prompt:** standing instructions prepended to every conversation. Cheap to
  change, weakly enforced — the model drifts from it, especially on style.
- **Catastrophic forgetting:** when fine-tuning on a narrow task erases broader abilities
  the model had before. The smaller and more aggressive the training, the worse it is.

## 3. The three-surface theory

The architecture assigns each property of the product to the *one mechanism that is
actually good at it*:

| Property | Mechanism (surface) | Why this assignment |
|---|---|---|
| **Truth** — what he actually said | RAG over 927 cited passages | Facts in weights are unauditable and unreliable; facts in a database are checkable, citable, and updatable |
| **Voice** — diction, cadence, his rhetorical moves | A light LoRA fine-tune | Style is exactly what fine-tuning is good at, and exactly what system prompts are bad at |
| **Stance & honesty** — the refusal policy, non-denominational neutrality, "this is a role, not an identity claim" | The agent layer: system prompt + a deterministic coverage gate in Rust | Stance must be *inspectable and changeable without retraining*; an honesty property buried in weights cannot be audited |

### The published evidence for each assignment

**Truth → retrieval.** Ovadia et al., *"Fine-Tuning or Retrieval? Comparing Knowledge
Injection in LLMs"* (arXiv:2312.05934, Microsoft): on genuinely new factual content, RAG
beat fine-tuning by **~37 percentage points**, and fine-tuning sometimes pushed accuracy
*below the untrained base model* (Llama-2: 0.353 → 0.219). Models "struggle to learn new
factual information through unsupervised fine-tuning." Worse for us: **combining**
fine-tuning with RAG was *not* reliably additive — sometimes below RAG alone — because a
more confident fine-tuned model paraphrases past the retrieved citation. Conclusion: never
expect the fine-tune to improve correctness; protect retrieval *from* the fine-tune.

**Voice → fine-tuning.** Marquardt & Brule, *"Fine-tuning on simulated data outperforms
prompting for agent tone"* (arXiv:2507.04889): prompting a base model into a target
conversational tone succeeded **23–46%** of the time; a LoRA fine-tune hit **>95%** — and
reached >90% with only **100 training examples**, with diminishing returns past ~1000.
This is why the annotation program targets hundreds, not tens of thousands, of rows.

**Stance → agent layer.** Two reasons. (a) *Auditability:* attestation tiering (how
well-attested a saying is) is contested scholarship; it must be revisable and
source-cited, which weights cannot be. (b) *Safety:* Betley et al., *"Emergent
Misalignment"* — narrow fine-tuning on small datasets can broadly misalign a model by
catastrophically forgetting its safety training. You do not want your refusal behavior to
live in the same weights you are perturbing. The Rust `CoverageGate` refuses
out-of-corpus questions *before the model runs* — deterministic code, not model judgment.

### Validation against the alternatives (2026-06-09 assessment, sycophancy-checked)

The role-play literature (R-CHAR, EMNLP 2025; *Two Tales of Persona*, EMNLP 2024) divides
persona methods into: (B) prompt-persona on a frontier model — best fluency today, but
truth and stance live in a provider's opaque weights and drift with provider updates;
(C) deep persona fine-tuning (CharacterLLM/CharacterBot) — strongest immersion, but moves
truth *into* weights, and its flagship case study (Lu Xun) had 17 essay collections of
signal where we have 927 verse-rows; (D) agentic memory/metacognition personas — no
weight changes, psychologically deep, but unproven on an ancient citation-required corpus.

The hybrid wins **for this project's constraint set** (never fabricate, cite, audit,
local-first). It is not universally optimal — under "maximum warmth, minimum effort," (B)
wins. The differentiator we have that (B) structurally cannot offer is the **visible
honesty architecture**: citations, attestation tiers, refusal-on-no-coverage. That is why
gap #6 (surfacing honesty in the UI) is a product gap, not a cosmetic one. And (D)
contributes one real idea worth adopting — **episodic relationship memory** — specified
in [03-memory-and-honesty.md](03-memory-and-honesty.md).

## 4. Why the last fine-tune collapsed (post-mortem as theory)

The attempt: 75 examples × learning rate 2e-4 × 3 epochs on Gemma 4 E4B (QLoRA, r=16).
The model collapsed (degenerate output). Three compounding causes, each a known
phenomenon you should be able to name:

1. **Overfitting:** 75 examples seen 3 times each at a high learning rate ≈ memorization.
   The model reproduces training rows instead of generalizing the *transform*
   (ancient → modern rendering). Practitioner rule of thumb: below ~100–500 high-quality
   examples, SFT memorizes (cf. arXiv:2511.00130, where small-data SFT "rapidly masters
   the skill but suffers complete catastrophic forgetting," NQ accuracy → 0).
2. **Learning rate too high:** lr 2e-4 is a common default for *larger* datasets. On tiny
   data it takes huge optimization steps toward a handful of examples, destroying
   pre-trained structure. The same paper found lowering LR directly mitigates forgetting.
3. **Too many epochs:** 3 passes over 75 rows = 225 gradient updates aimed at the same
   tiny target. One epoch was enough signal at this scale.

The fix is therefore **data first** (≥300 annotated rows — gap #1), **recipe second**
(lr ~2e-5, 1 epoch — [02-retraining-protocol.md](02-retraining-protocol.md)), and
**gates third** (the eval suite must show style gain *without* grounding loss before any
adapter ships).

## 5. Why prompts are assembled the way they are (the attribution lesson)

A live failure you should learn from: the RAG context (retrieved passages) used to be
concatenated *before* the user's question, unlabeled, inside the user's chat turn. The
model — correctly reading its input — attributed the passages to the user: *"the
scriptures you have presented…"*. Nobody "presented" them; retrieval found them.

Two findings from the literature drove the fix. *Lost in the Middle* (Liu et al., TACL
2023): models attend most strongly to the **beginning and end** of input; the middle
sags. And production RAG practice labels context provenance explicitly ("do not refer to
the provided context like someone handed it to you"). Hence the current assembly, which
you must preserve in all future work:

```
[system]  fixed contract incl. "passages provided are your own attested teachings;
          the person asking has not presented them"
[user]    <the user's question>                      ← start = high attention
          [Draw your answer from these attested passages you have in mind; speak
          directly to the person as their mentor. They have not seen these references.]
          Mark 12:29-31: "Hear, Israel..."           ← end = high attention
```

The invariant: **SYSTEM_PROMPT is duplicated by design** across `prompt.rs`,
`build_training_jsonl.py`, `ollama/Modelfile.jesus-twin`, and `PROMPTS.md` — and baked
into the SFT JSONL. All copies must change together (byte-identical at runtime) or the
served model drifts from what the LoRA learned. Every document in this spec that touches
a prompt repeats this warning on purpose.

## 6. What "authentic" means operationally

Throughout the annotation program, "authentic" has a precise meaning with a bright line:

- **Permitted:** multiple human-checked modern renderings of a *real* recorded line;
  labeling a real line with its rhetorical move; pairing a real situation with a response
  composed *only* from real lines and documented moves (with citations).
- **Forbidden:** any synthetic Q→A pair in which a model *invents* what Jesus answers;
  any rendering that adds content the original does not contain; any training example
  whose assistant turn cannot be traced to cited verses.

The worst case of a correctly-built system is *a real saying in a slightly-off tone*.
The worst case of a violated bright line is *a fabricated doctrine in a convincing
voice* — which is the one failure this project may never produce.
