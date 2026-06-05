# Modelfile for the Jesus Digital Twin (Ollama)

This Modelfile packages the fine-tuned GGUF with the correct chat template
and system prompt for Ollama.

## Build the model

```bash
# From the project root, with jesus-twin-merged/ containing the GGUF files
ollama create jesus-twin -f ollama/Modelfile.jesus-twin

# Verify
ollama show jesus-twin --modelfile
```

## Run

```bash
# Interactive
ollama run jesus-twin

# As an OpenAI-compatible server (port 11434)
ollama serve
# In another terminal:
ollama run jesus-twin "I'm worried about losing my job."

# Via HTTP API
curl http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "jesus-twin",
    "messages": [
      {"role": "user", "content": "I'm worried about money. What would you say?"}
    ]
  }'
```

## The system prompt

The system prompt here is the canonical "conversational mentor" voice from
`build_training_jsonl.py::SYSTEM_PROMPT` and `prompt.rs::SYSTEM_PROMPT`. It MUST
be the same string in all three places (Python script, Rust prompt, Modelfile)
to avoid training/serving drift.

## The chat template

The gemma-4 chat template (NOT `gemma-4-thinking`) is what Unsloth uses for
Gemma 4 E4B fine-tuning. The template markers are:
- `<|turn>user\n` (instruction part)
- `<|turn>model\n` (response part)

If the template is mismatched between training and serving, the model
produces gibberish, endless generations, or repeated outputs. This is the
#1 cause of "works in Unsloth, broken elsewhere" per Unsloth's docs.

## Ollama template variables

Ollama's Modelfile template syntax is Go templates. The available variables are:
- `{{ .System }}` — the system message
- `{{ .Prompt }}` — the user message
- `{{ .Response }}` — the model's response (stop token)

## Notes

- For Linux/Mac GPU, install `ollama` from https://ollama.com/download
- For Windows/WSL2, same install path
- For inference on a different machine than training, just copy the GGUF file and the Modelfile, then `ollama create`
- To push to a registry: `ollama push jesus-twin` (after `ollama login`)
- To export the model package: `ollama save jesus-twin > jesus-twin.tar`
