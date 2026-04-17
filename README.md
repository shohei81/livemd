# livemd

Live bilingual voice transcription TUI in Rust.

- **ASR**: `cpal` → `webrtc-vad` → `whisper.cpp` (Metal) via `whisper-rs`
- **Translation** (optional): Qwen2.5-7B-Instruct via `llama-server` subprocess (Metal), queried over HTTP
- **UI**: `ratatui` two-column display (English ↔ 日本語)
- **Output**: timestamped Markdown table

## Requirements

- Rust (stable, 2021 edition)
- CMake + a C/C++ toolchain (needed to build whisper.cpp and llama.cpp)
- Apple Silicon (for Metal) recommended but not required
- ~8 GB free disk for the two models

### 1. Whisper model

```sh
mkdir -p models
curl -L -o models/ggml-small.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin
```

`small` (~500 MB) is the default; `base` is lighter, `medium` is more accurate.

### 2. llama-server + translator model (optional)

`livemd` drives `llama.cpp`'s `llama-server` as a subprocess to avoid linking
conflicts between whisper.cpp and llama.cpp in the same binary.

```sh
brew install llama.cpp   # provides `llama-server` on PATH
```

Then download the Qwen model (bartowski's single-file build, no auth required):

```sh
curl -L -o models/Qwen2.5-7B-Instruct-Q4_K_M.gguf \
  https://huggingface.co/bartowski/Qwen2.5-7B-Instruct-GGUF/resolve/main/Qwen2.5-7B-Instruct-Q4_K_M.gguf
```

Size: ~4.7 GB. Peak RAM during inference: ~5 GB.

`livemd` will spawn `llama-server --model ... --port 8787 --n-gpu-layers 999`
on start and kill it on exit. Server logs go to `llama-server.log`.

To run without translation, delete or comment out the `[translator]` section in `livemd.toml`.

## Install globally

```sh
cargo install --path .
# -> installs `livemd` into ~/.cargo/bin/
```

Create the global config directory and drop models into it:

```sh
mkdir -p ~/.config/livemd/models
cp livemd.toml.example ~/.config/livemd/livemd.toml

# move (or re-download) models into the config dir
mv models/ggml-small.bin ~/.config/livemd/models/
mv models/Qwen2.5-7B-Instruct-Q4_K_M.gguf ~/.config/livemd/models/
```

Edit `~/.config/livemd/livemd.toml` — the default relative paths (`models/...`)
resolve against the config file's directory, so once models are in
`~/.config/livemd/models/` the sample config works unchanged. You can also use
absolute paths or `~/` in any path field.

## Run

```sh
# from anywhere
livemd notes.md

# or use the default output path from config
livemd

# override language at launch
livemd meeting.md -l auto

# explicit config file
livemd notes.md -c ./project-specific.toml
```

### Config search order

1. `-c / --config` CLI flag (if given)
2. `./livemd.toml` in the current directory
3. `~/.config/livemd/livemd.toml`

### Log location

- App log: `~/.config/livemd/logs/livemd.log`
- llama-server log: `~/.config/livemd/logs/llama-server.log`
- Override: `LIVEMD_LOG_DIR=/some/path livemd notes.md`

## Development

```sh
cargo run --release          # uses ./livemd.toml
cargo run --release -- notes.md
```

### Keybindings

| Key           | Action                          |
|---------------|---------------------------------|
| `q` / `Ctrl+C`| Save transcript and quit        |
| `s`           | Save transcript now             |
| `l`           | Cycle Whisper language (en → ja → auto) |
| `space`       | Pause / resume UI               |

### UI

```
┌ livemd · REC · lang=en · in=MacBook Pro Mic · model=ggml-small.bin · tr=ready ┐
│ ██████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░                              │
├─ English ────────────────────────────┬─ 日本語 ──────────────────────────────┤
│ [10:31:03] ▶ Hello, how are you?     │ [10:31:03]   こんにちは、お元気ですか？│
│ [10:31:10]   I'm fine, thanks.        │ [10:31:10] ▶ 元気です、ありがとう。    │
└──────────────────────────────────────┴───────────────────────────────────────┘
 q quit&save · s save · l cycle lang · space pause
```

`▶` marks the source language (the side the speaker actually used).
The opposite column shows the Qwen-translated version (or `…` while pending).

### Markdown output

```markdown
| time | speaker | English | 日本語 |
|------|---------|---------|--------|
| 10:31:03 |  | Hello, how are you? | こんにちは、お元気ですか？ |
| 10:31:10 |  | I'm fine, thanks. | 元気です、ありがとう。 |
```

## Memory & performance (M-series Mac)

On a 32 GB M4 MacBook Air with Qwen 7B Q4_K_M:
- Whisper small: transcribes in ~30% real-time (near-instant per segment)
- Qwen2.5-7B: ~30–50 tok/s on Metal → a 20-word translation lands in ~1 s
- Peak RSS: ~6 GB

For higher quality, swap in Qwen2.5-14B Q4_K_M (~9 GB, ~20 tok/s).

## Roadmap

- [x] Phase 1: cpal → VAD → whisper.cpp → ratatui UI → Markdown export
- [x] Phase 1.5: hallucination filters (silence fallbacks, YouTube tails, repetition loops)
- [x] Phase 2: bilingual side-by-side (Qwen via llama.cpp)
- [ ] Phase 3: speaker diarization (sherpa-onnx)
- [ ] Phase 4: configurable shortcuts, device picker, live draft buffer

## Logs

Diagnostic logs are written to `livemd.log` (keeps the TUI clean). Set
`RUST_LOG=debug` for verbose output.
