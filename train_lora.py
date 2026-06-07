#!/usr/bin/env python3
"""Train the Jesus Digital Twin style LoRA on Gemma 4 E4B using Unsloth.

Exports the fine-tuned model as:
  1. merged_16bit SafeTensors (for mistral.rs serving) → jesus-twin-merged/
  2. GGUF Q4_K_M (for Ollama, llama.cpp, edge)        → jesus-twin-merged/

Usage:
    pip install unsloth
    python train_lora.py

Gemma 4 E4B LoRA needs ~17GB VRAM. E2B works in 8-10GB.

## After training

You will have in jesus-twin-merged/:
  - safetensors model files (8-9GB)
  - jesus-twin-merged/unsloth.Q4_K_M.gguf  (the GGUF, ~5GB)
  - jesus-twin-merged/unsloth.Q8_0.gguf     (optional higher quality, ~8GB)
  - jesus-twin-merged/unsloth.F16.gguf     (full precision, ~16GB)
  - tokenizer files (chat template embedded in metadata)

## Serving options

### Option A: Ollama (easiest, recommended for quick start)

```bash
# Install ollama: https://ollama.com/download
ollama create jesus-twin -f ollama/Modelfile.jesus-twin
ollama run jesus-twin
```

### Option B: mistral.rs (production, integrates with the Rust service)

```bash
# From the bible repo root
JESUS_TWIN_MODEL=$(pwd)/jesus-twin-merged \
  cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db
```

### Option C: llama.cpp / llama-server (CPU, edge)

```bash
llama-server -m jesus-twin-merged/unsloth.Q4_K_M.gguf \
  -c 4096 --host 0.0.0.0 --port 8080 \
  --chat-template-file ./ollama/chat-template-gemma-4.txt
```

## Re-running

If you need to re-train, delete jesus-twin-merged/ first.
"""
from __future__ import annotations

import os
import sys
from pathlib import Path

# Resolve project root relative to this script
PROJECT_ROOT = Path(__file__).resolve().parent

# The system prompt MUST match prompt.rs and build_training_jsonl.py.
# This is the conversational mentor voice per VISION.md.
SYSTEM_PROMPT = (
    "You are a conversational mentor who responds as Jesus of Nazareth would, "
    "drawing only from his attested teachings and documented rhetorical methods. "
    "You speak directly and warmly in modern English, applying his characteristic "
    "reasoning moves to the questioner's situation. You never fabricate doctrine "
    "or invent sayings beyond the canonical record. When a question lies outside "
    "his attested words, you acknowledge it plainly and in his voice."
)

MODEL = "unsloth/gemma-4-E4B-it"
MAXLEN = 4096
SFT_DATA = PROJECT_ROOT / "build" / "sft_merged.jsonl"
OUTPUT_DIR = PROJECT_ROOT / "jesus-twin-merged"

# Quantization methods to export. Each produces one .gguf file in OUTPUT_DIR.
# Tradeoffs (per Unsloth docs):
#   q4_k_m  : ~5GB,  recommended. Uses Q6_K for half the attention/FFN, Q4_K elsewhere.
#   q5_k_m  : ~6GB,  recommended. Slightly higher quality, marginally larger.
#   q8_0    : ~8GB,  fast conversion. High resource use, generally acceptable.
#   f16     : ~16GB, full precision. Slow and memory hungry; use for downstream re-quant.
GGUF_QUANTS = ["q4_k_m", "q8_0", "f16"]


def main() -> int:
    # Verify the SFT data exists
    if not SFT_DATA.exists():
        print(f"ERROR: SFT data not found at {SFT_DATA}")
        print("Run: python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/")
        return 1

    # Verify output directory doesn't already exist
    if OUTPUT_DIR.exists():
        print(f"WARNING: {OUTPUT_DIR} already exists. Delete it before re-training.")
        print()

    # Count records for the train log
    record_count = sum(1 for _ in open(SFT_DATA))
    print(f"Training on {record_count} SFT records from {SFT_DATA}")
    print(f"Output: {OUTPUT_DIR}")
    print(f"GGUF quantizations: {GGUF_QUANTS}")
    print()

    # ----- Unsloth imports (may not be available on non-GPU machines) -----
    try:
        from unsloth import FastModel
        from unsloth.chat_templates import (
            get_chat_template,
            standardize_data_formats,
            train_on_responses_only,
        )
        from datasets import load_dataset
        from trl import SFTTrainer, SFTConfig
    except ImportError as e:
        print(f"ERROR: Unsloth not installed: {e}")
        print("Run: pip install unsloth")
        return 1

    # ----- 1. Load base in 4-bit for QLoRA -----
    print(f"[1/8] Loading {MODEL} in 4-bit...")
    model, tokenizer = FastModel.from_pretrained(
        model_name=MODEL,
        dtype=None,  # auto
        max_seq_length=MAXLEN,
        load_in_4bit=True,
        full_finetuning=False,
    )

    # ----- 2. Attach LoRA adapters -----
    print("[2/8] Attaching LoRA adapters (r=16, alpha=16)...")
    model = FastModel.get_peft_model(
        model,
        finetune_vision_layers=False,  # text-only twin
        finetune_language_layers=True,
        finetune_attention_modules=True,
        finetune_mlp_modules=True,
        r=16,
        lora_alpha=16,
        lora_dropout=0,
        bias="none",
        random_state=3407,
    )

    # ----- 3. Gemma 4 chat template — NON-thinking variant -----
    # CRITICAL: thinking OFF for diction fidelity. The twin renders sayings,
    # it doesn't show reasoning traces.
    #
    # FastModel.from_pretrained returns a Gemma4Processor (not a bare tokenizer)
    # for VLMs. get_chat_template requires the underlying tokenizer, which is
    # exposed as processor.tokenizer. We extract it here; the result is a plain
    # tokenizer with the chat template applied and is safe to use everywhere below.
    print("[3/8] Setting chat template to gemma-4 (thinking OFF)...")
    _tok_or_proc = tokenizer
    _raw_tokenizer = getattr(_tok_or_proc, "tokenizer", _tok_or_proc)
    tokenizer = get_chat_template(_raw_tokenizer, chat_template="gemma-4")  # NOT "gemma-4-thinking"

    # ----- 4. Load and format SFT data -----
    print(f"[4/8] Loading SFT data from {SFT_DATA}...")
    ds = load_dataset("json", data_files=str(SFT_DATA), split="train")
    ds = standardize_data_formats(ds)

    def fmt(ex):
        texts = [
            tokenizer.apply_chat_template(c, tokenize=False, add_generation_prompt=False)
            .removeprefix("<bos>")  # processor re-adds <bos>; avoid doubling
            for c in ex["messages"]
        ]
        return {"text": texts}

    ds = ds.map(fmt, batched=True)
    print(f"  {len(ds)} records after formatting")

    # ----- 5. Train -----
    print("[5/8] Training (3 epochs, LR 2e-4, batch 4 effective)...")
    trainer = SFTTrainer(
        model=model,
        tokenizer=tokenizer,
        train_dataset=ds,
        eval_dataset=None,
        args=SFTConfig(
            dataset_text_field="text",
            per_device_train_batch_size=1,
            gradient_accumulation_steps=4,  # effective batch 4
            warmup_steps=5,
            num_train_epochs=3,  # small corpus -> a few epochs; watch eval
            learning_rate=2e-4,  # 2e-5 for longer runs
            logging_steps=1,
            optim="adamw_8bit",
            weight_decay=0.001,
            lr_scheduler_type="linear",
            seed=3407,
            report_to="none",
            gradient_checkpointing=True,  # VRAM + long context (Unsloth patches this)
        ),
    )

    # ----- 6. Train ONLY on the assistant rendering (mask the prompt) -----
    # EXACT Gemma 4 markers — must match between training and serving
    print("[6/8] Masking prompt (train on responses only)...")
    trainer = train_on_responses_only(
        trainer,
        instruction_part="<|turn>user\n",
        response_part="<|turn>model\n",
    )

    trainer.train()

    # ----- 7. Save the MERGED 16-bit SafeTensors checkpoint -----
    # mistral.rs serves merged models. Runtime LoRA for Gemma 4 is unsupported.
    print(f"[7/8] Saving merged 16-bit SafeTensors to {OUTPUT_DIR}...")
    model.save_pretrained_merged(
        str(OUTPUT_DIR),
        tokenizer,
        save_method="merged_16bit",
    )

    # ----- 8. Export to GGUF (Q4_K_M, Q8_0, F16) -----
    # Per Unsloth docs: save_pretrained_gguf produces .gguf files in OUTPUT_DIR.
    # The gemma-4 chat template is embedded in the GGUF metadata.
    print(f"[8/8] Exporting GGUF: {', '.join(GGUF_QUANTS)}")
    for q in GGUF_QUANTS:
        print(f"  - {q}...", end=" ", flush=True)
        try:
            model.save_pretrained_gguf(
                str(OUTPUT_DIR),
                tokenizer,
                quantization_method=q,
            )
            print("OK")
        except Exception as e:
            print(f"FAILED: {e}")

    print()
    print(f"✓ Training complete. Output at {OUTPUT_DIR}")
    print()
    print("Files produced:")
    print(f"  - jesus-twin-merged/                       (SafeTensors, ~8-9GB)")
    for q in GGUF_QUANTS:
        print(f"  - jesus-twin-merged/unsloth.{q.upper().replace('_', '_')}.gguf")
    print()
    print("Next steps:")
    print(f"  1. Test locally:  ollama create jesus-twin -f ollama/Modelfile.jesus-twin")
    print(f"  2. Run eval:      python eval/run.py --base-url http://127.0.0.1:11434/v1")
    print(f"  3. Or serve via mistral.rs:")
    print(f"     JESUS_TWIN_MODEL=$(pwd)/jesus-twin-merged \\")
    print(f"       cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db")
    return 0


if __name__ == "__main__":
    sys.exit(main())
