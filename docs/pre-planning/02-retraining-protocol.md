# 02 — The Retraining Protocol (Gaps #3, #4, #9)

> Prerequisite: ≥300 training-ready rows ([01-annotation-program.md](01-annotation-program.md)).
> Do not start any step in this document before that gate is met — the previous attempt
> (75 rows) collapsed, and the literature says it will again (00-theory §4).

## 1. Theory: the three dials that caused the collapse, and where to set them

Fine-tuning is gradient descent: each batch nudges the weights toward reproducing your
examples. Three dials control how violent the nudging is.

1. **Learning rate (LR)** — the step size of each nudge. Too high on small data =
   each step overshoots toward a handful of examples, bulldozing pre-trained structure.
   The collapse used `2e-4`. The protocol uses **`2e-5`** (10× gentler). Why this number:
   it's the standard "longer/safer run" rate the original `train_lora.py` comment already
   suggested, and the data-scarce literature (arXiv:2511.00130) shows lowering LR directly
   mitigates catastrophic forgetting.
2. **Epochs** — full passes over the dataset. Each extra pass on tiny data re-burns the
   same examples in. The collapse used 3. The protocol uses **1**.
3. **LoRA rank (r)** — the capacity of the adapter. Keep **r=16, alpha=16** (unchanged):
   the constraint of a low-rank adapter is itself protective against forgetting (LoRA's
   "constrained nature inherently mitigates catastrophic forgetting" — same paper). Do
   not raise r to "fix" underfitting before you have evidence underfitting is the problem.

Everything else stays as `train_lora.py` already has it: QLoRA 4-bit load,
`train_on_responses_only` (the loss is computed only on the assistant turn, so the model
learns to *produce renderings*, not to parrot prompts), effective batch 4, thinking mode
OFF (diction fidelity), seed 3407.

## 2. The gates (write these down before training; they are pass/fail, not vibes)

| Gate | Instrument | Pass condition |
|---|---|---|
| G1: data | `build_training_jsonl.py` printout | ≥300 ready rows; no move with 0 eval examples |
| G2: training health | loss curve | smooth decrease; no cliff to ~0 (cliff = memorization) |
| G3: style gain | eval suite (`eval/run.py`, 145 tests + extensions per [04](04-resources-roles-evaluation.md) §3) | style-by-move score improves vs base model |
| G4: grounding unchanged | same suite, grounding/citation facets | **no regression** vs base — this is the hybrid's non-negotiable (00-theory §3: FT+RAG can hurt facts) |
| G5: no broad damage | a general-capability spot-check (e.g., 20 generic instruction prompts diffed base vs tuned) | no degenerate output, no off-task rambling |

An adapter that passes G3 but fails G4 is **rejected** — that is the exact trade the
architecture forbids ("keep the LoRA only if it improves style-by-move without hurting
grounding," ARCHITECTURE.md step 8).

## 3. The dual-base experiment (gap #4): Gemma 4 E4B vs Qwen3-4B

**Why both:** the 2026-06-09 assessment found Gemma 4 caused most toolchain pain — it is
a VLM class (`Gemma4ForConditionalGeneration`) that required the `MultimodalModelBuilder`
workaround in mistral.rs, its mmproj GGUF conversion is broken (bypassed via
`llama-quantize`), runtime LoRA is unsupported (merge-only), and the community reports it
"seriously broken" with Unsloth+llama.cpp. Qwen3-4B has first-class Unsloth support,
official GGUFs, a text-only causal architecture, and runtime-LoRA support in serving
stacks. **But the collapse was recipe-caused, not base-caused** — so the honest move is
an experiment, not a switch on faith.

**Protocol:**
1. Train the identical dataset with the identical recipe on both bases:
   - `unsloth/gemma-4-E4B-it` (current `train_lora.py` default)
   - `unsloth/Qwen3-4B-Instruct` (set via `--model`; **run Qwen first** — its tooling
     maturity means failures will be *your* failures, easier to debug)
2. Qwen3-specific care: it is a *hybrid-thinking* model — disable thinking at train and
   serve time (`enable_thinking=False` / the non-thinking chat template), the same
   discipline as Gemma's `gemma-4` (not `gemma-4-thinking`) template. **Chat-template
   mismatch between train and serve is the #1 cause of gibberish** (training guide §10).
3. Run both through gates G2–G5. The eval suite decides; preregister the decision rule:
   *highest G3 subject to G4-pass; ties broken by toolchain cost (favoring Qwen)*.
4. Record the result in `docs/FINDINGS.md` either way — a negative result (neither base
   passes) sends you back to data quality, not to recipe fiddling.

**Serving note:** the shipped CUDA build serves Gemma 4. If Qwen wins, the serving change
is contained: `models.yaml` + `MistralConfig` + (if needed) builder selection in
`jesus-twin-inference/src/mistral.rs`. Qwen3 is text-only, so `TextModelBuilder` applies —
*simpler* than the current path. Budget half a day, plus re-verifying the chat template
end-to-end.

## 4. Pre-training checklist (mechanical hygiene)

### 4.1 Fix the dead SYSTEM_PROMPT in `train_lora.py` (gap #9)

`train_lora.py` lines ~64-71 define a long SYSTEM_PROMPT constant that is **unused** (the
script trains on the system message already baked into `sft_merged.jsonl`) and
**contradicts** the canonical short prompt. Before any retrain: delete the dead constant
or replace its value with an import/copy of the canonical text, and update the "MUST
match" comment to point at `PROMPTS.md`. Why it matters: the next person to edit the file
will assume the constant is live, change it, and silently break the train/inference
parity invariant (00-theory §5).

### 4.2 Regenerate and split the data

```bash
python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/
python build_training_jsonl.py --eval-frac 0.10
# CHECK: ready count >= 300; per-move table has no zero rows in eval
```

Merge in the (reviewed) L2 records the same way the current `sft_merged.jsonl` was built;
confirm the merged file's system messages all carry the current prompt (spot-check 3 rows
with `head -3 build/sft_merged.jsonl | python3 -m json.tool | grep -A1 system`).

### 4.3 Hardware

One 24 GB GPU (L4, RTX 4090) suffices for 4-bit QLoRA at 4B–8B scale. ~300–500 rows × 1
epoch is minutes of training, not hours. If you OOM, lower `max_seq_length` (4096 → 2048)
before touching anything else — the corpus rows are short.

## 5. The run (explicit commands)

```bash
# 1) Qwen first
python train_lora.py --data build/sft_merged.jsonl \
  --model unsloth/Qwen3-4B-Instruct \
  --lr 2e-5 --epochs 1 --out qwen3-twin-merged/
# watch: loss decreasing smoothly (G2); a sudden dive toward 0 = stop, memorizing

# 2) Gemma second (same data, same recipe)
python train_lora.py --data build/sft_merged.jsonl \
  --model unsloth/gemma-4-E4B-it \
  --lr 2e-5 --epochs 1 --out gemma4-twin-merged/

# 3) Evaluate BOTH against the base model
python eval/run.py --model qwen3-twin-merged/  --report eval/out/qwen3.json
python eval/run.py --model gemma4-twin-merged/ --report eval/out/gemma4.json
python eval/run.py --model <base>              --report eval/out/base.json
# compare per the preregistered rule (G3 subject to G4)
```

(If `train_lora.py` lacks `--model/--lr/--epochs` flags, add them as thin argparse
passthroughs to the existing constants — a 10-line change — rather than editing constants
per run; runs must be reproducible from the command line.)

## 6. Shipping the winner

1. Export merged 16-bit (`save_pretrained_merged`, already in the script), then GGUF
   `q4_k_m` for the Ollama path if used.
2. Point the service at it: `JESUS_TWIN_MODEL=$(pwd)/<winner>-twin-merged cargo run ...
   --features mistralrs`. **No Rust code change is needed for a Gemma win**; a Qwen win
   needs §3's serving note.
3. Re-run the live attribution check (the greatest-commandments question) — the answer
   must still cite, still speak as the mentor, and never say "you have presented."
4. Keep the base-model RAG-only build deployable as the instant-rollback path. The
   adapter is an *enhancement*; the grounded engine is the product floor.

## 7. If it fails again (decision tree, so failure is information)

- **G2 cliff (loss → 0):** memorization → more data diversity (more renderings per
  saying, §01-4), not more epochs.
- **G3 flat (no style gain):** check the data actually varies in register (if every
  rendering is "plain," there's no warmth signal to learn); check the chat template
  matched at train/serve; only then consider r=32.
- **G4 regression (grounding hurt):** the adapter is too strong — halve LR to 1e-5 or
  blend ~10% generic instruction data (the standard emergent-misalignment mitigation);
  if it persists, ship RAG-only and revisit after more data.
- **G5 damage (general capability broken):** same mitigations as G4; also verify you
  trained the Instruct variant, not a base/pretrain checkpoint.
