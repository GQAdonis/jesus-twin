#!/usr/bin/env bash
# Download the models the jesus-twin release runs on, into local dirs the binary loads
# offline at startup. See jesus-twin/README.md ("Running the release") for how they're used.
#
#   - google/gemma-4-E4B-it      -> jesus-twin-base/            (generation; ungated)
#   - google/embeddinggemma-300m -> jesus-twin-embeddinggemma/  (retrieval embedder; GATED)
#
# The generation model is served 4-bit via in-situ quantization (ISQ Q4K) at load — you do
# NOT download a quantized file; the BF16 safetensors are quantized in-process on the GPU.
#
# Requirements:
#   - the Hugging Face CLI `hf` (pip install -U "huggingface_hub[cli]")
#   - HF_TOKEN exported (a read token). embeddinggemma is gated by Google: accept its license
#     at https://huggingface.co/google/embeddinggemma-300m FIRST, or the download 403s.
#
# Usage:
#   HF_TOKEN=hf_xxx scripts/download-models.sh            # both models
#   HF_TOKEN=hf_xxx scripts/download-models.sh base       # just the base generation model
#   HF_TOKEN=hf_xxx scripts/download-models.sh embed      # just the embedder
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WHAT="${1:-all}"

if ! command -v hf >/dev/null 2>&1; then
  echo "ERROR: 'hf' CLI not found. Install: pip install -U 'huggingface_hub[cli]'" >&2
  exit 1
fi
if [[ -z "${HF_TOKEN:-}" ]]; then
  echo "ERROR: HF_TOKEN is not set. Export a Hugging Face read token first." >&2
  exit 1
fi

download() {  # repo, dest
  echo ">>> downloading $1 -> $2"
  hf download "$1" --local-dir "$2"
}

if [[ "$WHAT" == "all" || "$WHAT" == "base" ]]; then
  download "google/gemma-4-E4B-it" "$ROOT/jesus-twin-base"
fi

if [[ "$WHAT" == "all" || "$WHAT" == "embed" ]]; then
  # Gated: requires accepting the license on the model page while logged in.
  download "google/embeddinggemma-300m" "$ROOT/jesus-twin-embeddinggemma" || {
    echo "" >&2
    echo "embeddinggemma download failed. If it was 'awaiting review' / 403, accept the" >&2
    echo "license at https://huggingface.co/google/embeddinggemma-300m and retry." >&2
    exit 1
  }
fi

echo ">>> done. Set these when running the binary:"
echo "    JESUS_TWIN_MODEL=$ROOT/jesus-twin-base"
echo "    JESUS_TWIN_EMBED_MODEL=$ROOT/jesus-twin-embeddinggemma"
