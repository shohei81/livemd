#!/usr/bin/env bash
# kotomark install / update — works both locally (cloned repo) and via curl | bash.
# Rerun the same command to update; config is preserved unless --reset-config.
set -euo pipefail

REPO_URL="https://github.com/shohei81/kotomark"
RAW_URL="https://raw.githubusercontent.com/shohei81/kotomark/main"

TIER=""
RESET_CONFIG=0

print_usage() {
    cat <<EOF >&2
usage: install.sh {standard|high} [--reset-config]

  standard         Whisper small + Qwen2.5-7B   (~5 GB disk, ~7 GB RAM)
  high             Whisper large-v3-turbo + Qwen2.5-14B   (~10 GB disk, ~11 GB RAM)
  --reset-config   Overwrite ~/.config/kotomark/kotomark.toml with the tier default

Remote install / update:
  curl -fsSL ${RAW_URL}/install.sh | bash -s -- high
EOF
}

for arg in "$@"; do
    case "$arg" in
        standard|high) TIER="$arg" ;;
        --reset-config) RESET_CONFIG=1 ;;
        -h|--help) print_usage; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; print_usage; exit 1 ;;
    esac
done

if [[ -z "$TIER" ]]; then
    print_usage
    exit 1
fi

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo not found. Install Rust first: https://rustup.rs" >&2
    exit 1
fi
if ! command -v llama-server >/dev/null 2>&1; then
    echo "NOTE: llama-server not found on PATH — translation will be disabled."
    echo "      Install with: brew install llama.cpp"
fi

CONFIG_DIR="$HOME/.config/kotomark"
MODEL_DIR="$CONFIG_DIR/models"
mkdir -p "$MODEL_DIR"

echo "==> Installing / updating kotomark from ${REPO_URL}"
cargo install --git "$REPO_URL" --force kotomark

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

write_config() {
    local example_name="$1"
    local dest="$CONFIG_DIR/kotomark.toml"
    if [[ -f "$dest" && $RESET_CONFIG -eq 0 ]]; then
        echo "  keep existing: $dest (pass --reset-config to overwrite)"
        return
    fi
    echo "  writing: $dest"
    curl -fsSL -o "$dest" "${RAW_URL}/${example_name}"
}

if [[ "$TIER" == "standard" ]]; then
    echo "==> Fetching standard-tier models"
    fetch "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin" \
          "$MODEL_DIR/ggml-small.bin"
    fetch "https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf" \
          "$MODEL_DIR/Qwen2.5-7B-Instruct-Q4_K_M.gguf"
    echo "==> Config"
    write_config "kotomark.toml.standard.example"
else
    echo "==> Fetching high-tier models"
    fetch "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin" \
          "$MODEL_DIR/ggml-large-v3-turbo.bin"
    fetch "https://huggingface.co/bartowski/Qwen2.5-14B-Instruct-GGUF/resolve/main/Qwen2.5-14B-Instruct-Q4_K_M.gguf" \
          "$MODEL_DIR/Qwen2.5-14B-Instruct-Q4_K_M.gguf"
    echo "==> Config"
    write_config "kotomark.toml.high.example"
fi

echo ""
echo "==> Done"
echo "    Config: $CONFIG_DIR/kotomark.toml"
echo "    Models: $MODEL_DIR"
echo ""
echo "Run: kmark notes.md"
