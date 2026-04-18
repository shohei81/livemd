#!/usr/bin/env bash
# kotoma setup — installs the binary, downloads models, writes the global config.
# Usage: ./setup.sh {standard|high}
set -euo pipefail

TIER="${1:-}"
case "$TIER" in
    standard|high) ;;
    *)
        echo "usage: $0 {standard|high}" >&2
        echo "" >&2
        echo "  standard  small.bin + Qwen2.5-7B  (~5 GB disk, ~7 GB RAM)" >&2
        echo "  high      large-v3-turbo + Qwen2.5-14B  (~10 GB disk, ~11 GB RAM)" >&2
        exit 1
        ;;
esac

CONFIG_DIR="$HOME/.config/kotoma"
MODEL_DIR="$CONFIG_DIR/models"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

mkdir -p "$MODEL_DIR"

fetch() {
    local url="$1"
    local dest="$2"
    if [[ -f "$dest" && -s "$dest" ]]; then
        echo "  skip (already present): $(basename "$dest")"
    else
        echo "  downloading: $(basename "$dest")"
        curl -fL --progress-bar -o "$dest" "$url"
    fi
}

echo "==> Installing kotoma binary"
(cd "$SCRIPT_DIR" && cargo install --path .)

if [[ "$TIER" == "standard" ]]; then
    echo "==> Fetching Whisper small (500 MB)"
    fetch \
      "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin" \
      "$MODEL_DIR/ggml-small.bin"

    echo "==> Fetching Qwen2.5-7B Q4_K_M (4.5 GB)"
    fetch \
      "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf" \
      "$MODEL_DIR/Qwen2.5-7B-Instruct-Q4_K_M.gguf"

    cp "$SCRIPT_DIR/kotoma.toml.standard.example" "$CONFIG_DIR/kotoma.toml"
else
    echo "==> Fetching Whisper large-v3-turbo (1.6 GB)"
    fetch \
      "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin" \
      "$MODEL_DIR/ggml-large-v3-turbo.bin"

    echo "==> Fetching Qwen2.5-14B Q4_K_M (8.5 GB)"
    fetch \
      "https://huggingface.co/bartowski/Qwen2.5-14B-Instruct-GGUF/resolve/main/Qwen2.5-14B-Instruct-Q4_K_M.gguf" \
      "$MODEL_DIR/Qwen2.5-14B-Instruct-Q4_K_M.gguf"

    cp "$SCRIPT_DIR/kotoma.toml.high.example" "$CONFIG_DIR/kotoma.toml"
fi

echo ""
echo "==> Done. Config → $CONFIG_DIR/kotoma.toml"
echo "    Models → $MODEL_DIR"
echo ""
echo "Try: kotoma notes.md"
