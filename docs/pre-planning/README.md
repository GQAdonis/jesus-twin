# Pre-Planning: Gap-Closure Functional Specification

**Status:** pre-planning (not yet decomposed into KBD/OpenSpec changes)
**Date:** 2026-06-09
**Audience:** a junior AI developer (advanced undergraduate / early Ph.D. level). Every
document explains the theory first, then gives step-by-step instructions explicit enough
for a beginner computer-science student to execute — and to understand *why* each step
exists.

## What this is

The project ships today as a **RAG-first grounded answer engine**: questions are answered
from 927 cited passages of the recorded sayings of Jesus, with a coverage gate that
refuses out-of-corpus questions. That release works. What remains is the distance between
"a correct answer engine" and the actual goal stated in `VISION.md`: a **conversational
mentor that sounds like him, thinks like him, and never invents doctrine**.

The 2026-06-09 assessment (`.kbd-orchestrator/phases/build-agent/assessment.md`,
addendum) validated the project's hybrid architecture against three competing methods and
identified the remaining gaps. This directory is the full functional specification for
closing them.

## The gaps, and where each is specified

| # | Gap | Why it matters | Document |
|---|---|---|---|
| 1 | **0 of 927 corpus rows annotated** (the style fine-tune is data-starved; the last attempt collapsed) | The voice/method layer is the product differentiator the user perceives most | [01-annotation-program.md](01-annotation-program.md) |
| 2 | **Annotation guide is wrong** (covers M01–M09 only, with names diverging from the canonical rubric) | Annotators following it would mislabel every row; garbage labels → garbage fine-tune | [01-annotation-program.md](01-annotation-program.md) §2 |
| 3 | **No retrain protocol** after the 75-row collapse (lr 2e-4 × 3 epochs overfit and broke the model) | Without a written recipe + gates, the next attempt repeats the failure | [02-retraining-protocol.md](02-retraining-protocol.md) |
| 4 | **Base-model question unresolved** (Gemma 4 toolchain pain vs Qwen3-4B maturity) | The wrong base costs weeks of toolchain fights | [02-retraining-protocol.md](02-retraining-protocol.md) §5 |
| 5 | **No episodic user-relationship memory** (the one capability the agentic-persona literature offers that we lack) | A mentor that forgets every prior conversation is not a mentor | [03-memory-and-honesty.md](03-memory-and-honesty.md) §1–3 |
| 6 | **The honesty architecture is invisible** (citations, attestation, refusal exist in the event stream but no UI surfaces them) | Honesty-you-can-see is the differentiator vs. prompt-persona apps | [03-memory-and-honesty.md](03-memory-and-honesty.md) §4–5 |
| 7 | **Hebrew Bible source tool unbuilt** (his actual intellectual world; the *remez* method needs it) | Without the Tanakh, a documented reasoning move (M09, *remez*) cannot ground | [01-annotation-program.md](01-annotation-program.md) §7; [04-resources-roles-evaluation.md](04-resources-roles-evaluation.md) §4 |
| 8 | **Eval suite must become the gate** for every contested choice (LoRA acceptance, base model, prompt changes) | Without a measuring instrument, every decision is vibes | [04-resources-roles-evaluation.md](04-resources-roles-evaluation.md) §3 |
| 9 | `train_lora.py` carries a dead, contradictory SYSTEM_PROMPT constant | A retrain that touches it would break the train/inference parity invariant | [02-retraining-protocol.md](02-retraining-protocol.md) §4.1 |

## Reading order

1. **[00-theory.md](00-theory.md)** — the three-surface theory and the published evidence
   behind it. Read this first; every later instruction traces back to it.
2. **[01-annotation-program.md](01-annotation-program.md)** — the annotation program: the
   single highest-value workstream. Contains worked examples of every annotation variation.
3. **[02-retraining-protocol.md](02-retraining-protocol.md)** — how to run the fine-tune
   so it does not collapse again, and how to decide Gemma vs Qwen empirically.
4. **[03-memory-and-honesty.md](03-memory-and-honesty.md)** — episodic memory and making
   the honesty visible.
5. **[04-resources-roles-evaluation.md](04-resources-roles-evaluation.md)** — hardware,
   people (and why humans are non-optional), evaluation, and sequencing.

## The one principle that governs everything

> **Retrieval owns truth. The fine-tune owns voice. The agent layer owns stance and
> honesty.**

If an instruction in any document ever seems to move *truth* into the model's weights, or
bake the *stance* into the fine-tune, the instruction is wrong — stop and re-read
[00-theory.md](00-theory.md) §3. The architecture is the ethics.
