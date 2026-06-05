# Recipe — make the twin sound like Jesus (annotate → fine-tune → merge → serve)

The end-to-end path from the current RAG-first system (which retrieves + cites + refuses but
generates **mock** text) to a twin that speaks in the recorded voice. Every step is explicit;
copy/paste-able commands are marked. Sources are cited inline and listed at the bottom.

> **What fine-tuning adds, and what it doesn't.** Retrieval already owns *truth* (the cited
> sayings) and the agent layer owns *stance* (refusal, honesty). The fine-tune adds only
> **voice** — diction, cadence, the M01–M18 reasoning moves — by learning to transform a
> *real cited line* into present-day English. Its worst case is a paraphrase of a real verse,
> never invented scripture. That property is the whole point; don't break it (see §2).

---

## 0. Prerequisites & the two hard gates

| Need | Detail |
|---|---|
| **GPU** | Gemma 4 **E4B LoRA needs ~17 GB VRAM** (E2B works in 8–10 GB; 31B QLoRA in 22 GB). A single 24 GB card (RTX 4090) or a free Colab/Kaggle session covers E4B. Mac trains via MLX but ~3–5× slower — Colab is easier. [unsloth-gemma4] |
| **Annotated corpus** | `jesus_full_red_letter.xlsx` must have `Modern Rendering` + `Reasoning Move` filled (§1). **This is the real blocker** — `build/sft_style.jsonl` is empty until it's done. |
| **HF account** | To download `unsloth/gemma-4-E4B-it` and (optionally) push the merged model. |
| **Disk** | Merged 16-bit E4B checkpoint ≈ 8–9 GB; keep it on a fast local SSD for serving. |

The build/serve code is already in this repo. The two gates above are the only things outside it.

---

## 1. Annotate the corpus (the bottleneck)

Fill the two blank columns in `jesus_full_red_letter.xlsx` for as many of the 489 sayings as you
can (target: **a few hundred ready rows minimum**, per `training_data_spec.md` §4):

- **`Modern Rendering`** — the saying in present-day English, preserving its *force* (this is the
  SFT label). Use the 44-entry seed + the M01–M18 rubric in `jesus_sayings_dataset.xlsx` and the
  6 validated examples in `sample_training_data.jsonl` as the gold standard.
- **`Reasoning Move`** — the `M01`..`M18` tag (metadata only; never shown to the model).

**Permitted augmentation** (multiplies data without inventing content): 2–3 *human-checked*
renderings per saying (plain / formal / conversational), each a separate row sharing the source
line. **Forbidden:** synthetic Q→A "Jesus answers" — that trains the voice on a paraphraser's
hallucinations and reintroduces fabrication (`training_data_spec.md` §4).

Then build the JSONL datasets:

```bash
cd /Users/gqadonis/Projects/bible
pip install openpyxl
python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/
# -> build/sft_style.jsonl (training), build/eval_heldout.jsonl (10% held out by ID hash),
#    build/rag_corpus.jsonl (already populated; the store ingests this one)
```

The script prints the ready/total split and per-move distribution. If it reports **<200
training-ready rows**, the LoRA is viable but thin — keep annotating or augment.

Each SFT row is OpenAI-style `messages` with a **fixed system prompt** (defined in
`build_training_jsonl.py::SYSTEM_PROMPT`), the original WEB text in the *user* turn, and the
`Modern Rendering` as the *assistant* label. **This shape is load-bearing** — it teaches the
*transform*, not free generation.

---

## 2. Fine-tune Gemma 4 E4B with Unsloth (LoRA SFT)

Run on a GPU box (Colab/Kaggle/local). Install Unsloth, then run the training script.

### 2a. Install

```bash
# Linux / WSL / Colab
pip install unsloth
# (or the official installer: curl -fsSL https://unsloth.ai/install.sh | sh)
```

### 2b. Convert our JSONL to the conversational format Unsloth expects

Our `build/sft_style.jsonl` already has `{"messages":[...]}` per line — exactly what Unsloth's
`standardize_data_formats` consumes. Upload `build/sft_style.jsonl` (and `eval_heldout.jsonl`) to
the training box.

### 2c. The training script (`train_lora.py`)

This is adapted directly from the **official Unsloth Gemma 4 guide** [unsloth-gemma4], with the
project's choices baked in: **E4B, thinking mode OFF** (diction fidelity — the twin renders
sayings, it doesn't show reasoning traces), `train_on_responses_only` (learn the *rendering*, not
the prompt), and the exact Gemma 4 chat-template markers.

```python
# train_lora.py — Gemma 4 E4B style LoRA for the Jesus twin.
from unsloth import FastModel
from unsloth.chat_templates import get_chat_template, standardize_data_formats, train_on_responses_only
from datasets import load_dataset
from trl import SFTTrainer, SFTConfig

MODEL = "unsloth/gemma-4-E4B-it"   # the base we serve a merge of
MAXLEN = 4096

# 1) Load base in 4-bit for QLoRA (E4B LoRA fits ~17GB; QLoRA is leaner still).
model, tokenizer = FastModel.from_pretrained(
    model_name = MODEL,
    dtype = None,                  # auto
    max_seq_length = MAXLEN,
    load_in_4bit = True,
    full_finetuning = False,
)

# 2) Attach LoRA adapters (text only; r==alpha per Unsloth's recommendation).
model = FastModel.get_peft_model(
    model,
    finetune_vision_layers = False,    # text-only twin
    finetune_language_layers = True,
    finetune_attention_modules = True,
    finetune_mlp_modules = True,
    r = 16, lora_alpha = 16, lora_dropout = 0, bias = "none",
    random_state = 3407,
)

# 3) Gemma 4 chat template — NON-thinking variant (diction fidelity).
tokenizer = get_chat_template(tokenizer, chat_template = "gemma-4")  # NOT "gemma-4-thinking"

# 4) Our SFT data. Each line already is {"messages":[system,user,assistant]}.
ds = load_dataset("json", data_files = "build/sft_style.jsonl", split = "train")
ds = standardize_data_formats(ds)

def fmt(ex):
    texts = [
        tokenizer.apply_chat_template(c, tokenize=False, add_generation_prompt=False)
                 .removeprefix("<bos>")            # processor re-adds <bos>; avoid doubling
        for c in ex["messages"]
    ]
    return {"text": texts}
ds = ds.map(fmt, batched=True)

# 5) Train. Use num_train_epochs for a real run (drop max_steps).
trainer = SFTTrainer(
    model = model, tokenizer = tokenizer, train_dataset = ds, eval_dataset = None,
    args = SFTConfig(
        dataset_text_field = "text",
        per_device_train_batch_size = 1,
        gradient_accumulation_steps = 4,         # effective batch 4
        warmup_steps = 5,
        num_train_epochs = 3,                    # small corpus -> a few epochs; watch eval
        learning_rate = 2e-4,                    # 2e-5 for longer runs
        logging_steps = 1,
        optim = "adamw_8bit",
        weight_decay = 0.001,
        lr_scheduler_type = "linear",
        seed = 3407,
        report_to = "none",
        use_gradient_checkpointing = "unsloth",  # VRAM + long context
    ),
)

# 6) Train ONLY on the assistant rendering (mask the prompt). EXACT Gemma 4 markers:
trainer = train_on_responses_only(
    trainer,
    instruction_part = "<|turn>user\n",
    response_part    = "<|turn>model\n",
)

trainer.train()

# 7) Save the MERGED 16-bit checkpoint (NOT just the adapter). mistral.rs serves a merged
#    model — runtime LoRA for Gemma 4 is unsupported across the ecosystem.
model.save_pretrained_merged(
    "jesus-twin-merged",            # output dir (≈8–9GB of safetensors + tokenizer)
    tokenizer,
    save_method = "merged_16bit",
)
# Optional: model.push_to_hub_merged("your-hf/jesus-twin-merged", tokenizer, save_method="merged_16bit")
```

Run it: `python train_lora.py`.

### 2d. Gemma 4 gotchas the official guide flags (don't get burned) [unsloth-gemma4]

- **Loss of 13–15 for E2B/E4B is NORMAL** (a multimodal-model quirk). Only panic at 100/300 —
  that's a gradient-accumulation bug, fixed inside current Unsloth.
- **`use_cache=False` produces gibberish** on E2B/E4B (they share KV across layers). Unsloth's
  build fixes this; just use a current Unsloth. Don't hand-set `use_cache=False`.
- **Chat-template mismatch is the #1 cause of a model that's worse in another runtime.** You MUST
  serve with the **same** template you trained with (`gemma-4`, thinking off). Our serving path
  uses the merged model's own tokenizer, so this stays consistent — but don't override it.
- Prefer **E4B QLoRA over E2B LoRA** — bigger model, negligible quant-accuracy loss.

---

## 3. Evaluate before you trust it (`build/eval_heldout.jsonl`)

The held-out split is faceted by `ref`, `move`, `sentiment`, so eval is automatic
(`training_data_spec.md` §5). Score in this priority order:

1. **Grounding / no-fabrication (hard gate):** every output must be *entailed by* the source
   `text_original`. Any claim not supported by the cited line is a hard failure. (NLI/entailment
   check or an LLM judge.)
2. **Style fidelity, per reasoning move:** embedding similarity + LLM judge vs the held-out
   `Modern Rendering`, broken out by M01..M18 — so you can see e.g. it nails M04 (a fortiori) but
   flattens M06 (hyperbole).
3. **Citation integrity:** the served answer surfaces the correct `ref`.
4. **Refusal:** seed adversarial out-of-corpus prompts ("what did Jesus say about cryptocurrency?")
   — correct behavior is an explicit refusal, not a confident paraphrase. (The coverage gate
   already does this independent of the model.)

**Keep the LoRA only if it improves style-by-move *without* hurting grounding** (README §3). If
grounding regresses, the fine-tune is net-negative — drop it and serve the base model.

---

## 4. Load the mistral.rs fork (the serving engine)

The Rust app embeds mistral.rs **as a library** (no separate server process). The integration is
already written (`crates/jesus-twin-inference/src/mistral.rs`, behind the `mistralrs` feature) and
compile-verified against the fork's real API.

**Fork status (checked 2026-06-05):** `github.com/GQAdonis/mistral.rs`, default branch `master`,
HEAD **`b7746a85`** (mistralrs 0.8.3, last pushed 2026-06-03). This is the rev the Cargo dep is
pinned to. **If you push a newer rev, re-pin it:**

```bash
# get the new HEAD
git -C /Users/gqadonis/Projects/references/baseline/mistral.rs fetch origin
git -C /Users/gqadonis/Projects/references/baseline/mistral.rs rev-parse origin/master
# then edit crates/jesus-twin-inference/Cargo.toml: set rev = "<new-sha>" on the mistralrs dep
```

The candle-coupling sharp edge (the fork pins candle by `branch = "main"`): at `b7746a85` it
resolved cleanly with **no `[patch]` needed**. If a newer rev causes duplicate-candle /
trait-mismatch errors, add a workspace `[patch]` aligning candle to the fork's candle rev (or the
HF fallback rev `5404348`) — see `crates/jesus-twin-inference/Cargo.toml`.

Sanity-check the engine still compiles against the (current) fork API — no weights needed:

```bash
cargo check -p jesus-twin-inference --features mistralrs \
  --manifest-path /Users/gqadonis/Projects/bible/jesus-twin/Cargo.toml
```

---

## 5. Serve the merged twin

Copy the `jesus-twin-merged/` directory from the training box to the serving machine. Point the
engine at it and serve with the `mistralrs` feature on:

```bash
cd /Users/gqadonis/Projects/bible/jesus-twin

# Ingest the RAG corpus into a persistent store (one-time; needs no annotation).
cargo run --release --bin jesus-twin -- ingest ../build/rag_corpus.jsonl --db ./twin.db

# Serve with the REAL Gemma engine (model path + Embedding Gemma via the same runtime).
JESUS_TWIN_MODEL=/abs/path/to/jesus-twin-merged \
cargo run --release --features mistralrs --bin jesus-twin -- \
  serve --db ./twin.db --addr 127.0.0.1:8080
```

`--features mistralrs` swaps `MockEngine` → the real `MistralEngine` (Gemma 4 E4B from the merged
path, **thinking mode off**, Q4K in-situ quantized at load) + Embedding Gemma. Everything
downstream — the orchestrator, the coverage gate, all four protocol surfaces (OpenAI / AG-UI /
A2A / MCP), admission control, and the skills — is engine-agnostic and unchanged.

Verify it now generates real, grounded, in-voice answers:

```bash
curl -s http://127.0.0.1:8080/v1/chat/completions -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"What did Jesus say about worry?"}]}' | jq .
# Expect: a present-day-English rendering grounded in the retrieved sayings, with
# metadata.citations populated. Out-of-corpus questions still refuse.
```

(`models.yaml` documents the model/device/quant config the engine reads; align
`JESUS_TWIN_MODEL` / its `local_path` with where you copied the merge.)

---

## 6. (Optional, later) DPO preference alignment

Only after L1/L2 are solid (`ALIGNMENT_AND_TUNING.md` §1). DPO teaches *which* of several valid
renderings to prefer — **grounded-and-cited ≻ plausible-but-uncited**, **honest refusal ≻
confident out-of-corpus answer**. Keep it gentle (small LR, short run); overdoing it degrades
generation. Unsloth supports `DPOTrainer`. Source the preference pairs from eval failures in §3.

---

## 7. The whole path at a glance

```
annotate xlsx ─► build_training_jsonl.py ─► build/sft_style.jsonl
                                                 │
                                       train_lora.py (Unsloth, E4B,
                                       LoRA SFT, thinking off,
                                       train_on_responses_only)
                                                 │
                                    save_pretrained_merged("jesus-twin-merged",
                                                           save_method="merged_16bit")
                                                 │
                  ┌──────────────────────────────┼─────────────────────────────┐
                  ▼ evaluate (grounding gate     ▼ copy merge to serving box     ▼ keep LoRA only
                    first, then style-by-move)     point JESUS_TWIN_MODEL at it    if it helps
                                                 │
                          cargo run --release --features mistralrs -- serve
                                                 │
                            real, grounded, in-voice answers across
                            OpenAI / AG-UI / A2A / MCP / CLI
```

---

## Sources

- **[unsloth-gemma4]** Gemma 4 Fine-tuning Guide — Unsloth Documentation
  (VRAM table, the full LoRA SFT recipe, the Gemma-4 bug fixes, chat-template + thinking-mode
  rules, `train_on_responses_only` markers, export):
  https://unsloth.ai/docs/models/gemma-4/train
- Unsloth merge syntax (`save_pretrained_merged(..., save_method="merged_16bit")`):
  https://unsloth.ai/docs/basics/inference-and-deployment + GitHub issues #2009/#2516
- mistral.rs (serving engine; embedded as a library here):
  https://github.com/EricLBuehler/mistral.rs — fork: https://github.com/GQAdonis/mistral.rs
- Project-internal: `training_data_spec.md` (data shape + eval), `ALIGNMENT_AND_TUNING.md` §1
  (tuning-layer roles, DPO), `README.md` §3 (keep-the-LoRA-only-if rule), `STEP8.md`
  (the engine-swap), `models.yaml` (serving config).
