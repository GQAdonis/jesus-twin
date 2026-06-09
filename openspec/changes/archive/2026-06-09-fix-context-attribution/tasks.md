# Tasks: fix-context-attribution

Ordered. `[ ]` todo · `[/]` in progress · `[x]` done. Code edits are out of scope for the
plan phase — this list is what the **execute** phase will run.

## 1. System prompt — declare context provenance
- [x] Add one clause to `jesus-twin/crates/jesus-twin-core/src/prompt.rs::SYSTEM_PROMPT`:
      passages provided are the mentor's own attested teachings for grounding; the person
      asking has not presented them — speak from them directly, do not refer to them as
      something the user gave you.
- [x] Mirror the identical change to all parity copies (train/inference invariant):
      `build_training_jsonl.py` SYSTEM_PROMPT, `ollama/Modelfile.jesus-twin` SYSTEM,
      `PROMPTS.md` canonical block. Verify byte-identical.

## 2. Context block — provenance-labeled instruction
- [x] Implement the framing in `prompt::assemble_context` (currently a `join("\n\n")` stub
      whose doc-comment defers exactly this). Prepend the validated instruction line:
      `[Draw your answer from these attested passages you have in mind; speak directly to the
      person as their mentor. They have not seen these references.]` then the `ref: text`
      passage lines.
- [x] Confirm `orchestrator.rs::context_lines()` still supplies `ref: text_original` lines;
      framing belongs in `assemble_context`, not scattered.

## 3. User-turn assembly — question first, passages last
- [x] Change `jesus-twin/crates/jesus-twin-inference/src/mistral.rs:92` from
      `format!("{}\n\n{}", req.context, req.user)` to question-first, labeled-context-last
      order (end-of-prompt high-attention placement per Lost-in-the-Middle).
- [x] Keep the `mock` engine behavior consistent enough that existing tests pass; update the
      mock's grounding marker only if the assembled-order assertion requires it.

## 4. Regression test (the gap that let this ship)
- [x] Add a test asserting the assembled grounding block (a) opens with the provenance
      instruction label and (b) keeps passages after it (`prompt::tests`). Question-before-
      passages order is enforced in `mistral.rs` assembly; the empty-context guard is tested too.
- [x] `cargo test -p jesus-twin-core -p jesus-twin-inference` green (16 tests, incl. 3 new).

## 5. Verify end-to-end
- [x] Rebuild the RAG-only release; re-run *"what are the greatest commandments in the law?"*.
      **(operator-confirmed at archive time, 2026-06-09 — ran outside this environment.)**
- [x] Confirm the answer no longer says "you have presented / you have shown me" and instead
      speaks as the mentor who recalled the passages. **(operator-confirmed at archive time.)**
- [x] `cargo fmt` + `cargo clippy -- -D warnings` clean.
- [x] `cargo check -p jesus-twin-inference --features mistralrs` clean (feature-gated path compiles).

## 6. OpenSpec close-out
- [x] `openspec validate fix-context-attribution` → "Change 'fix-context-attribution' is valid".
- [x] After the user's live verification (§5) + merge: archive via
      `/opsx:archive fix-context-attribution`; sync `grounded-generation` to `openspec/specs/`.
      **(2026-06-09: grounded-generation synced to openspec/specs/; archived.)**
