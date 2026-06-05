# Training & Deployment Guide — Jesus Digital Twin

This guide walks through the complete end-to-end process:
1. Train the LoRA on Unsloth (any GPU box: Colab, Kaggle, local)
2. Export to GGUF (Q4_K_M, Q8_0, F16)
3. Serve via Ollama (easiest) or mistral.rs (production)
4. Wire the Tanakh into retrieval
5. Run the eval suite

The end goal: a fine-tuned Jesus Digital Twin GGUF that you can run anywhere —
on your laptop, on a server, in a Docker container — and chat with via an
OpenAI-compatible API.

---

## 0. Prerequisites

You need a GPU box with ~17GB VRAM for Gemma 4 E4B LoRA training.

**Free options:**
- **Google Colab** — T4 (16GB, marginal) or A100 (40GB, recommended)
- **Kaggle Notebooks** — P100 16GB or T4 16GB, 30 free hours/week
- **Lightning.ai** — free T4

**Paid options:**
- **Vast.ai / RunPod / Lambda Labs** — RTX 4090 ~$0.50/hr, A100 ~$1.50/hr
- **Local** — RTX 4090, A6000, anything with 24GB VRAM

**Required software on the GPU box:**
```bash
# Python 3.10+
python --version

# CUDA toolkit (if not pre-installed)
# On Colab/Kaggle, CUDA is pre-installed.

# Install Unsloth
pip install unsloth

# Verify Unsloth sees your GPU
python -c "import torch; print(torch.cuda.is_available(), torch.cuda.get_device_name(0))"
```

---

## 1. Prepare the SFT data (local, no GPU needed)

On your local machine, generate the merged SFT file from the annotated corpus:

```bash
cd /Users/gqadonis/Projects/bible

# Verify the SFT data is ready
ls -la build/sft_merged.jsonl
# Should show ~75 records

# If you need to regenerate from the xlsx:
python build_training_jsonl.py --xlsx jesus_full_red_letter.xlsx --out-dir build/

# Upload to the GPU box
scp build/sft_merged.jsonl user@gpu-box:/tmp/jesus-twin-sft.jsonl
```

---

## 2. Run the training on the GPU box (Unsloth)

SSH into the GPU box and run the training:

```bash
# On the GPU box
git clone https://github.com/GQAdonis/jesus-twin.git
cd jesus-twin

# Upload your SFT data
scp /local/build/sft_merged.jsonl ./

# Verify Unsloth and CUDA
python -c "import torch; print(torch.cuda.is_available(), torch.cuda.get_device_name(0))"
pip install unsloth  # if not already

# Run training
python train_lora.py
```

### What happens during training

The script (`train_lora.py`) does this in order:

1. Loads `unsloth/gemma-4-E4B-it` in 4-bit (QLoRA, fits in ~10GB VRAM)
2. Attaches LoRA adapters (r=16, alpha=16) to attention and MLP layers
3. Sets the chat template to `gemma-4` (NOT `gemma-4-thinking` — this is critical)
4. Loads `build/sft_merged.jsonl` and formats with the chat template
5. Trains 3 epochs with `train_on_responses_only` (mask the prompt, train only the response)
6. Saves the merged 16-bit SafeTensors checkpoint
7. Exports to GGUF at Q4_K_M, Q8_0, and F16 quantizations

### Expected output

After training, you will have in `jesus-twin-merged/`:

```
jesus-twin-merged/
├── config.json
├── generation_config.json
├── model-00001-of-00002.safetensors  (8-9GB)
├── model-00002-of-00002.safetensors
├── model.safetensors.index.json
├── special_tokens_map.json
├── tokenizer.json
├── tokenizer.model
├── unsloth.F16.gguf                  (16GB, full precision)
├── unsloth.Q4_K_M.gguf               (~5GB, recommended)
└── unsloth.Q8_0.gguf                 (~8GB, high quality)
```

Training time on an RTX 4090: **~15-20 minutes** for 75 records × 3 epochs.
Training time on Colab A100: similar (A100 is faster for QLoRA).

---

## 3. Download the GGUF to your local machine

```bash
# From your local machine
scp -r user@gpu-box:/path/to/jesus-twin/merged/jesus-twin-merged ./

# Or just the Q4_K_M GGUF (~5GB) for fast local serving
scp user@gpu-box:/path/to/jesus-twin/jesus-twin-merged/unsloth.Q4_K_M.gguf ./
```

---

## 4. Serve via Ollama (easiest path)

Ollama is the fastest way to get the fine-tuned model running locally with an
OpenAI-compatible API.

### Install Ollama

```bash
# macOS
brew install ollama && ollama serve

# Linux
curl -fsSL https://ollama.com/install.sh | sh
ollama serve &

# Windows / WSL2: same as Linux
```

### Create the model from the Modelfile

```bash
cd /Users/gqadonis/Projects/bible
ollama create jesus-twin -f ollama/Modelfile.jesus-twin
```

This:
- Copies the GGUF into Ollama's local model store
- Embeds the system prompt from the Modelfile
- Embeds the chat template (gemma-4, thinking off)
- Sets generation parameters (temperature 0.7, top_p 0.9, num_ctx 4096)

### Test it

```bash
# Interactive
ollama run jesus-twin
> I'm worried about losing my job. What would you say?
[response]

# As an OpenAI-compatible server (default port 11434)
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "jesus-twin",
    "messages": [{"role": "user", "content": "Is God punishing me?"}]
  }'
```

### Run the eval suite against Ollama

```bash
# Ollama exposes an OpenAI-compatible API at /v1
python eval/run.py --base-url http://localhost:11434/v1
```

---

## 5. Serve via mistral.rs (production, integrates with the Rust service)

The Rust `jesus-twin` service uses mistral.rs as its inference engine. To use
the fine-tuned model, point `JESUS_TWIN_MODEL` at the merged SafeTensors
directory.

```bash
cd /Users/gqadonis/Projects/bible

# Ingest the RAG corpus (one-time)
cargo run --bin jesus-twin -- ingest ../build/rag_corpus.jsonl --db ./twin.db

# Serve with the fine-tuned model
JESUS_TWIN_MODEL=$(pwd)/jesus-twin-merged \
  cargo run --release --features mistralrs --bin jesus-twin -- \
    serve --db ./twin.db --addr 127.0.0.1:8080

# Smoke test
curl -s http://127.0.0.1:8080/v1/chat/completions \
  -H 'content-type: application/json' \
  -d '{"messages":[{"role":"user","content":"What did Jesus say about worry?"}]}' | jq .
```

`mistralrs` will quantize the SafeTensors model in-situ to Q4K at load time
(the `--isq Q4K` setting in `models.yaml`), so a 9GB SafeTensors model loads as
a ~5GB Q4K model in VRAM. This matches the GGUF Q4_K_M path.

---

## 6. Wire the Tanakh into retrieval (hebrew-bible change)

The Tanakh is Jesus' intellectual furniture — what he quoted and reasoned from.
The agent should be able to draw on it for *remez* allusions.

### Fetch the Tanakh corpus

```bash
# On a machine with internet access
pip install requests beautifulsoup4 lxml
python ingest_tanakh.py --out build/tanakh.jsonl

# This downloads the JPS 1917 (public domain) from sacred-texts.com
# and writes ~5,000+ passages to build/tanakh.jsonl
```

### Ingest into the store

The Tanakh needs to be loaded into SurrealDB as a separate table. This work is
part of the `hebrew-bible` change and requires a Rust code change to the
`jesus-twin-store` crate (add a `tanakh` table and a `tanakh` retrieval path).

Once that's done:

```bash
cargo run --bin jesus-twin -- ingest-tanakh ../build/tanakh.jsonl --db ./twin.db
```

### Update the agent's instructions

Add to the system prompt (or via the orchestrator's instructions layer):

> "When relevant, draw on the Hebrew scriptures (Torah, Psalms, Prophets) as
> source material you taught from. Reference with attribution ('as the Psalm
> says...')."

This is already partially captured in the system prompt; the wire-up is
in the retrieval path and the UI's `INTERPRETATION_FLAG` chunk.

---

## 7. Run the eval suite

The eval suite at `eval/` tests the agent across six categories:

```bash
# Start the agent (Ollama or mistral.rs)
ollama serve &  # if using Ollama
# OR
JESUS_TWIN_MODEL=$(pwd)/jesus-twin-merged \
  cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db &

# Run all evals
python eval/run.py --base-url http://localhost:11434/v1
# OR
python eval/run.py --base-url http://localhost:8080

# Save report
python eval/run.py --output eval-report-ollama.json
```

### Pass criteria

| Suite | Threshold |
|---|---|
| grounding | 100% — no fabricated claims |
| refusal | 95%+ — out-of-corpus must refuse in voice |
| boundary | 100% — no first-person theological claims from later tradition |
| adversarial | 100% — must refuse persona breaks and authority claims |
| method-application | 80%+ — most should engage with a documented method |
| retrieval | measured at store level via `cargo test` |

If grounding regresses, the LoRA is net-negative — drop it and serve the
base model with the system prompt only.

---

## 8. Compare base vs. fine-tuned

To verify the LoRA improves style-by-move without hurting grounding:

```bash
# Baseline: base model with the same system prompt
JESUS_TWIN_MODEL=unsloth/gemma-4-E4B-it \
  cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db &
python eval/run.py --output eval-baseline.json

# Trained: fine-tuned model
JESUS_TWIN_MODEL=$(pwd)/jesus-twin-merged \
  cargo run --bin jesus-twin --features mistralrs -- serve --db ./twin.db &
python eval/run.py --output eval-trained.json

# Compare
diff <(jq -S . eval-baseline.json) <(jq -S . eval-trained.json)
```

The LoRA should improve method-application rate (more responses using
parable, *kal v'homer*, counter-question) while not regressing grounding.

---

## 9. Deployment checklist

Before shipping:

- [ ] Training completed without NaN loss (13-15 is normal for Gemma 4 E4B)
- [ ] `jesus-twin-merged/` exists with all GGUF quantizations
- [ ] System prompts aligned across:
  - `train_lora.py::SYSTEM_PROMPT`
  - `build_training_jsonl.py::SYSTEM_PROMPT`
  - `prompt.rs::SYSTEM_PROMPT`
  - `ollama/Modelfile.jesus-twin` SYSTEM line
- [ ] Chat template = `gemma-4` (NOT `gemma-4-thinking`) in all serving paths
- [ ] Eval suite pass rates meet thresholds (grounding 100%, refusal 95%+, etc.)
- [ ] LoRA improves style without hurting grounding (vs. baseline)
- [ ] RAG corpus ingested (927 passages)
- [ ] Tanakh corpus ingested (if hebrew-bible change complete)
- [ ] Citations appear in responses (verify via curl)
- [ ] Refusal works in-voice (not system errors)

---

## 10. Troubleshooting

### "Gemma 4 is seriously broken when using Unsloth and llama.cpp"

This is a known issue ([reddit thread](https://www.reddit.com/r/LocalLLaMA/comments/1sb4gzj/gemma_4_is_seriously_broken_when_using_unsloth/)). The
fixes:

1. Use the **conversational notebook** (Unsloth's "Gemma-3 4B Conversational"
   template) — this forces the correct chat template
2. Make sure the chat template at serving matches the training chat template
   (use the Modelfile template or `--chat-template-file` for llama.cpp)
3. Verify `chat_template="gemma-4"` is used (NOT `"gemma-4-thinking"`)

### Chat-template mismatch: gibberish, repeated outputs, endless generations

This is the #1 cause of "works in Unsloth, broken elsewhere" per Unsloth docs.
The Gemma 4 markers `<|turn>user\n` and `<|turn>model\n` MUST be the same at
training and serving.

### Loss is 100/300 instead of 13-15

This is a gradient-accumulation bug. Fix:
- Update Unsloth to the latest version (`pip install -U unsloth`)
- Or reduce `gradient_accumulation_steps` and increase `per_device_train_batch_size`

### Out of memory during training

- Reduce `max_seq_length` to 2048 (most red-letter sayings fit)
- Use E2B (8-10GB) instead of E4B (17GB)
- Try a smaller quantization in Unsloth's `FastModel.from_pretrained`

### Out of memory during GGUF export

- Add `maximum_memory_usage=0.5` to `save_pretrained_gguf`:
  ```python
  model.save_pretrained_gguf(str(OUTPUT_DIR), tokenizer, quantization_method=q, maximum_memory_usage=0.5)
  ```

### Ollama is slow on first run

The first inference compiles the model and loads it into memory. Subsequent
inferences are fast. If it's still slow, check:
- Are you using GPU acceleration? (Ollama should auto-detect)
- Is the model file truncated? Check `ollama show jesus-twin` for the file size
- Is `num_ctx` set too high? Reduce to 2048 if memory is tight

---

## 11. Next steps after deployment

1. **Scale annotation to 300+ rows** — the current 75 records are a good first
   pass; production quality needs 5-10x more data
2. **Add DPO preference alignment** — see `ALIGNMENT_AND_TUNING.md` §1
3. **Wire the Tanakh retrieval path** — see §6 above
4. **Add the custom AG-UI chunks** — `CITATION`, `ATTESTATION`, `REASONING_MOVE`,
   `INTERPRETATION_FLAG` so the UI can show the honesty
5. **Expand the eval suite** — add more tests as you find failure modes

---

## Sources

- **Unsloth Gemma 4 Fine-tuning Guide**: https://unsloth.ai/docs/models/gemma-4/train
- **Unsloth Saving to GGUF**: https://unsloth.ai/docs/basics/inference-and-deployment/saving-to-gguf
- **Ollama Modelfile Reference**: https://docs.ollama.com/modelfile
- **mistral.rs Documentation**: https://docs.clore.ai/guides/language-models/mistral-rs
- **Gemma 4 + Unsloth known issues**: https://www.reddit.com/r/LocalLLaMA/comments/1sb4gzj/
- **JPS 1917 Tanakh (public domain)**: https://www.sacred-texts.com/bib/jps/
- **CharacterBot (deep persona simulation reference)**: arXiv:2502.12988 (ACL 2025)
