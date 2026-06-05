#!/usr/bin/env python3
"""Train the Jesus Digital Twin style LoRA on Gemma 4 E4B using Unsloth.

This is the runnable version of the training recipe documented in
jesus-twin/RECIPE.md. Run on a GPU box (Colab/Kaggle/local).

Usage:
    pip install unsloth
    python train_lora.py

    # Then on a serving box:
    cp -r jesus-twin-merged/ /path/to/serving/
    JESUS_TWIN_MODEL=/path/to/serving/jesus-twin-merged \\
      cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db

Gemma 4 E4B LoRA needs ~17GB VRAM. E2B works in 8-10GB.
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
SAVE_METHOD = "merged_16bit"  # Runtime LoRA for Gemma 4 is unsupported; always merge


def main() -> int:
    # Verify the SFT data exists
    if not SFT_DATA.exists():
        print(f"ERROR: SFT data not found at {SFT_DATA}")
        print("Run: python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/")
        return 1

    # Verify output directory doesn't already exist
    if OUTPUT_DIR.exists():
        print(f"WARNING: {OUTPUT_DIR} already exists. Delete it before re-training.")

    # Count records for the train log
    record_count = sum(1 for _ in open(SFT_DATA))
    print(f"Training on {record_count} SFT records from {SFT_DATA}")
    print(f"Output: {OUTPUT_DIR} ({SAVE_METHOD})")
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
    print(f"[1/7] Loading {MODEL} in 4-bit...")
    model, tokenizer = FastModel.from_pretrained(
        model_name=MODEL,
        dtype=None,  # auto
        max_seq_length=MAXLEN,
        load_in_4bit=True,
        full_finetuning=False,
    )

    # ----- 2. Attach LoRA adapters -----
    print("[2/7] Attaching LoRA adapters (r=16, alpha=16)...")
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
    print("[3/7] Setting chat template to gemma-4 (thinking OFF)...")
    tokenizer = get_chat_template(tokenizer, chat_template="gemma-4")  # NOT "gemma-4-thinking"

    # ----- 4. Load and format SFT data -----
    print(f"[4/7] Loading SFT data from {SFT_DATA}...")
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
    print("[5/7] Training (3 epochs, LR 2e-4, batch 4 effective)...")
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
            use_gradient_checkpointing="unsloth",  # VRAM + long context
        ),
    )

    # ----- 6. Train ONLY on the assistant rendering (mask the prompt) -----
    # EXACT Gemma 4 markers
    print("[6/7] Masking prompt (train on responses only)...")
    trainer = train_on_responses_only(
        trainer,
        instruction_part="<|turn>user\n",
        response_part="<|turn>model\n",
    )

    trainer.train()

    # ----- 7. Save the MERGED 16-bit checkpoint -----
    # mistral.rs serves a merged model — runtime LoRA for Gemma 4 is unsupported
    print(f"[7/7] Saving merged checkpoint to {OUTPUT_DIR} ({SAVE_METHOD})...")
    model.save_pretrained_merged(
        str(OUTPUT_DIR),
        tokenizer,
        save_method=SAVE_METHOD,
    )

    print()
    print(f"✓ Training complete. Merged checkpoint at {OUTPUT_DIR}")
    print()
    print("Next steps:")
    print(f"  1. Copy {OUTPUT_DIR}/ to your serving box")
    print(f"  2. Point JESUS_TWIN_MODEL at it")
    print(f"  3. Serve: cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db")
    print(f"  4. Evaluate: python eval/run.py --base-url http://127.0.0.1:8080")
    return 0


if __name__ == "__main__":
    sys.exit(main())
