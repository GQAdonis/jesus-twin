# HANDOFF — Wave 3 (the human-gated / UI work)

**For:** taking the twin from "answers any life question via RAG, zero human hours" (end of Wave 2)
to "all the way correct." **Authored:** 2026-06-21, after Waves 1–2 shipped (PR #2).

Wave 3 is **not pure dev** — it is gated on human annotation and on a UI that doesn't exist yet.
Do the items in order; each has an explicit gate. Read the governing specs:
- `docs/pre-planning/01-annotation-program.md` — the annotation program (the long pole).
- `docs/pre-planning/02-retraining-protocol.md` — the retrain recipe + gates G1–G5 (authoritative).
- `docs/annotation-guide.md` — the annotator's guide (fixed in Wave 1: all 18 canonical moves).
- `docs/FINDINGS.md` — what Waves 1–2 shipped + the deferred follow-ups each wave surfaced.

## What is already done for you (dev prep, this handoff)

`train_lora.py` is **ready to run** — no edits needed before a retrain:
- The dead `SYSTEM_PROMPT` constant (gap #9) is removed; the system message comes from
  `build/sft_merged.jsonl` (baked by `build_training_jsonl.py` from the canonical `PROMPTS.md`).
- CLI flags added: `--data --model --out --lr --epochs --max-seq-length`. **Defaults are the GENTLE
  recipe** (`--lr 2e-5 --epochs 1`) — the 2e-4×3 recipe that collapsed on 75 rows is gone.

Everything else in Waves 1–2 ships behind a one-command corpus run (see `docs/FINDINGS.md` handoffs).

---

## 3.1 annotation-300 (HUMAN — the long pole, unblocks everything)

**Gate to start the rest of Wave 3.** Per `01-annotation-program.md`.

1. Annotators fill `jesus_full_red_letter.xlsx` (`Modern Rendering` + `Reasoning Move`) for **≥300
   rows**, using the fixed `docs/annotation-guide.md` (18 moves). Stratify across the change-11
   life-domain taxonomy so every domain gets L2 coverage. QC per 01 §6 (blind re-derivation of 5
   labels; garbage labels are worse than none).
2. Regenerate + split, and **check the G1 gate**:
   ```bash
   python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/
   python build_training_jsonl.py --eval-frac 0.10
   # G1: ready count >= 300; per-move table has NO move with 0 eval examples
   ```
3. Re-ingest + **recalibrate the gate** (the ~300-row milestone trigger — modern legs go live as
   real human renderings replace machine drafts):
   ```bash
   cd jesus-twin
   cargo run --release --features cuda -p jesus-twin-cli -- ingest ../build/rag_corpus.jsonl --db ./twin.db
   cargo run --release --features cuda -p jesus-twin-cli -- gate calibrate --eval-dir ../eval --db ./twin.db --out ../eval/out/gate-calibration.jsonl
   ```
   Under leg-agreement this should be a **no-op confirmation**; a tier shift is itself a signal —
   record it in `docs/FINDINGS.md`.

## 3.2 Review the machine tags (HUMAN — promotes Wave-2 drafts row-by-row)

- **11r principle-index review:** verify/correct the machine `domains`/`principles` facets; on
  approval set `machine_tagged = false` (re-`apply-principle-tags` a reviewed sidecar).
- **modern-legs / annotation promotion:** human `Modern Rendering`s overwrite machine drafts
  (`machine_draft → false`) as rows are annotated — this is automatic via 3.1's re-ingest.
- **13r attestation scholarly pass:** see 3.4.

## 3.3 retrain-dual-base (DEV — gated on G1 ≥300 rows; script is ready)

Per `02-retraining-protocol.md`. **Do NOT start before G1 passes** (75 rows collapsed; it will again).

```bash
# Qwen FIRST (better tooling -> failures are easier to debug), then Gemma — identical data + recipe.
python train_lora.py --data build/sft_merged.jsonl --model unsloth/Qwen3-4B-Instruct --out qwen3-twin-merged/
python train_lora.py --data build/sft_merged.jsonl --model unsloth/gemma-4-E4B-it   --out gemma4-twin-merged/
# (gentle recipe is the default: --lr 2e-5 --epochs 1)
```
Gate each through **G2–G5** (training-health / style-gain / grounding-unchanged / no-broad-damage).
**Preregistered decision rule:** highest G3 *subject to G4-pass*; ties → toolchain cost (favor
Qwen). An adapter that gains style (G3) but hurts grounding (G4) is **rejected** — RAG-only stays
the product floor and the instant-rollback path. Qwen care: disable thinking at train + serve
(chat-template mismatch is the #1 gibberish cause). Record the result (incl. a negative one) in
`docs/FINDINGS.md`. Ship the winner: a Gemma win needs no Rust change (`JESUS_TWIN_MODEL=...`); a
Qwen win is contained to `models.yaml` + `MistralConfig` + builder selection (Qwen3 is text-only →
`TextModelBuilder`, *simpler* than today's path).

## 3.4 gospel attestation data (HUMAN/DATA — unblocks the Wave-2 deferral)

gospel-context-kb shipped attestation-flagged but `attestation = single` for all rows: **there is
no synoptic-parallel data** (the `parallels` graph is unpopulated). To compute multiply-vs-single
mechanically, produce a synoptic-parallel mapping (pericope alignment across the four Gospels —
e.g. from a published harmony), then a small dev pass recomputes `attestation`/`witnesses` and
populates the `parallels` graph edges. Same data also enables synoptic-attestation on the
red-letter corpus.

## 3.5 honesty-surface-ui (DEV — BLOCKED on the frontend not existing)

No React app exists yet (CLAUDE.md mandates React 19 + Vite, shadcn-on-base-ui, Zustand+Immer,
served by a crate `build.rs`). When the frontend is scaffolded, render the canonical chunks the
backend already emits: `x-jesus-twin/citation` (chips), `x-jesus-twin/low-confidence` (the Tier-2
badge + its `principles`), `x-jesus-twin/refusal` (in-voice styling). DoD (pre-planning 03 §4–5):
the two screenshots + a Tier-2 answer screenshot showing the transparent frame.

## Cross-wave dev follow-ups (surfaced in Waves 1–2; do any time)

1. **Refusal eval → LLM-judge.** Keyword matching can't measure honest-decline vs confabulation
   (`docs/FINDINGS.md` gate Reflect). Replace `run_refusal`'s keyword check with a judge.
2. **Option B — discriminating signal for HARD refusal.** Leg-agreement can't refuse adjacent-topic
   out-of-corpus questions (eval-life-questions: T3 0/9). Add per-leg rank-depth or a relevance
   judge so "what's the date the world ends" actually refuses.
3. **Theme-expansion retrieval boost** (principle-index-v1 deferral): embed question → nearest
   domain → boost domain-tagged passages as a 5th RRF leg.
4. ~~**Orchestrator source/narrative blocks.**~~ **DONE** — the orchestrator now retrieves Tanakh +
   Gospel narrative each turn and injects them as distinct, labeled context blocks
   (`x-jesus-twin/source-text` + `x-jesus-twin/narrative-context` chunks). See `docs/FINDINGS.md`
   "Orchestrator wiring — the two source/narrative blocks are now live" (also fixed the latent
   `Arc<SurrealStore>` trait-default no-op that was silently disabling episodic-memory in `serve`).
5. **Memory v2** (episodic-memory deferrals): reflection synthesis, relevance-ranked recall,
   preference auto-extraction.

## The bright line (holds across every Wave-3 step)

Machine-generated text may steer *which* passages retrieve — never *what is said or trained*.
Display uses `text_original`; SFT uses human-verified xlsx rows only; each corpus stays labeled
distinctly; memory never crosses into the persona. Wave 3 promotes machine drafts to human-verified
content row-by-row — it never relaxes this line.
