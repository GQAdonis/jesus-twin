# Change: fix-context-attribution

**Phase:** build-agent · **Backend:** OpenSpec · **Effort:** ~2 hours · **Agent:** general

## Why

The shipped RAG-only build leaks the provenance of retrieved passages. Live run, query
*"what are the greatest commandments in the law?"*: retrieval was correct (Mark 12:29-31,
Matthew 22:37-40, …), but the model answered *"The scriptures **you have presented** show a
clear pattern…"* — attributing the passages to the **user**, who never presented them. The
retrieval system found them.

### Root cause (verified, read-only)

The retrieved passages are fused into the **user turn** with no provenance label and no
handling instruction:

- `orchestrator.rs::context_lines()` emits bare `ref: text` lines (no framing).
- `jesus-twin-inference/src/mistral.rs:92` concatenates `format!("{}\n\n{}", req.context, req.user)`
  — context **before** the question, all inside the `User` message.

So the model's user turn literally reads as the passages *followed by* the question, in the
user's voice. A well-behaved model correctly attributes them to whoever spoke that turn —
which the chat template labels `user`. This is a textbook RAG framing defect, not a
retrieval or model bug.

### Validation (web search, required by the request)

- **Instruction-after-context with explicit provenance is established RAG practice.**
  Production grounded-RAG prompts instruct the model "do not refer to the provided context
  like someone handed it to you" (arXiv:2603.09999; DoRA RAG assessment, ResearchGate 404021813).
- **End-of-prompt instruction placement is correct.** "Lost in the Middle" (Liu et al.,
  TACL 2023, arXiv:2307.03172) shows models attend most strongly to the **beginning and end**
  of the input and lose the middle. The user's chosen placement — instruction at the **end**,
  immediately before the passages the model must use — is the high-attention position.

Both confirm the requested approach. No reframing needed (Heuristic 7 — no silent pivot).

## What changes

Three small, surgical prompt-assembly edits. **No** change to retrieval, the coverage gate,
the store, the model, or the architecture. The fix completes a function whose own doc-comment
already defers this work (`prompt::assemble_context` is a stub).

1. **System prompt** (`prompt.rs::SYSTEM_PROMPT`, mirrored to `build_training_jsonl.py`,
   `ollama/Modelfile.jesus-twin`, `PROMPTS.md`): add one clause declaring that grounding
   passages are the mentor's **own attested recall**, not something the user submitted.
2. **Context block framing** (`prompt::assemble_context` / `orchestrator.rs::context_lines`):
   wrap passages with the provenance-labeled instruction line, exactly as requested:
   ```
   [Draw your answer from these attested passages you have in mind; speak directly to the
   person as their mentor. They have not seen these references.]
   Mark 12:29-31: …
   ```
3. **User-turn assembly order** (`mistral.rs:92`): put the **question first**, the labeled
   passage block **last** (end-of-prompt = high attention, per Lost-in-the-Middle), instead
   of the current context-first order.

## Non-goals

- Voice warmth/mentor-tone *training* — that is the deferred `lora-train` / `production-lora`
  fine-tune (see `assessment.md`). This change fixes the framing-driven portion of the voice
  problem only; it does not substitute for the LoRA.
- No change to retrieval ranking, RRF, the gate thresholds, or citation emission.

## Impact

- **Specs:** `grounded-generation` (new capability spec — how retrieved context is framed to
  the model).
- **Code:** `jesus-twin-core/src/prompt.rs`, `jesus-twin-core/src/orchestrator.rs`,
  `jesus-twin-inference/src/mistral.rs`; prompt mirrors in `build_training_jsonl.py`,
  `ollama/Modelfile.jesus-twin`, `PROMPTS.md`.
- **Tests:** new regression test asserting the assembled user turn carries the provenance
  label and question-last order. (The `mock` engine does not exercise `mistral.rs:92`, which
  is why the 51 passing tests missed this — the test must assert on the assembled string.)
- **Risk:** Low. Prompt-only. Reversible. The one cross-cutting risk is the SYSTEM_PROMPT
  **train/inference parity invariant** — the prompt is duplicated across 4+ files by design
  (`prompt.rs` doc-comment) and all copies must change together or the served behavior drifts
  from what any future LoRA learned.
