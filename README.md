# livemd

Live bilingual voice transcription TUI in Rust.

- **ASR**: `cpal` → `webrtc-vad` → `whisper.cpp` (Metal) via `whisper-rs`
- **Translation** (optional): Qwen2.5-7B-Instruct via `llama-server` subprocess (Metal), queried over HTTP
- **Diarization** (optional): `sherpa-onnx` speaker-embedding ONNX model via `sherpa-rs` (CPU), online centroid clustering
- **UI**: `ratatui` two-column display (English ↔ 日本語) with `Sn:` speaker prefix
- **Output**: timestamped Markdown table

## Requirements

- Rust (stable, 2021 edition)
- CMake + a C/C++ toolchain (needed to build whisper.cpp and llama.cpp)
- Apple Silicon (for Metal) recommended but not required
- ~8 GB free disk for the two models

### 1. Whisper model

```sh
mkdir -p models
curl -L -o models/ggml-large-v3-turbo.bin \
  https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin
```

`large-v3-turbo` (~1.6 GB) is the recommended default — near-`large-v3`
quality with `small`-like speed on M-series Metal. `small` (~500 MB) is a
lighter fallback; `large-v3` (~3 GB) is the highest quality but slower.

### 2. llama-server + translator model (optional)

`livemd` drives `llama.cpp`'s `llama-server` as a subprocess to avoid linking
conflicts between whisper.cpp and llama.cpp in the same binary.

```sh
brew install llama.cpp   # provides `llama-server` on PATH
```

Then download the Qwen model (bartowski's single-file build, no auth required):

```sh
curl -L -o models/Qwen2.5-14B-Instruct-Q4_K_M.gguf \
  https://huggingface.co/bartowski/Qwen2.5-14B-Instruct-GGUF/resolve/main/Qwen2.5-14B-Instruct-Q4_K_M.gguf
```

Size: ~8.5 GB. Peak RAM during inference: ~9 GB. 14B noticeably outperforms
7B on JA↔EN conversational translation on a 32 GB M-series machine. For
lighter setups use `Qwen2.5-7B-Instruct-Q4_K_M.gguf` (~4.5 GB).

`livemd` will spawn `llama-server --model ... --port 8787 --n-gpu-layers 999`
on start and kill it on exit. Server logs go to `llama-server.log`.

To run without translation, delete or comment out the `[translator]` section in `livemd.toml`.

### 3. Diarizer model (optional)

```sh
curl -L -o models/3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx \
  https://github.com/k2-fsa/sherpa-onnx/releases/download/speaker-recongition-models/3dspeaker_speech_eres2net_base_sv_zh-cn_3dspeaker_16k.onnx
```

Size: ~40 MB. Embedding dim: 512. Language-agnostic (trained on zh but
generalises well to JA/EN voices). Runs on CPU only but is very fast on
M-series (~tens of ms per segment).

Tune `threshold` in `[diarizer]` (0.45–0.55 typical for mixed JA/EN) if
speakers are being merged or split.

To run without diarization, delete or comment out the `[diarizer]` section.

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

# append a new session to an existing file
livemd --resume notes.md

# override language at launch
livemd meeting.md -l auto

# explicit config file
livemd notes.md -c ./project-specific.toml
```

### Output modes

- **Default (overwrite)**: writes `---` frontmatter + `## start – end` session
  header + transcript table. Existing file is replaced.
- **`--resume` / `-r`**: existing file content is preserved verbatim, a new
  `## start – end` session block is appended below it. Multiple `s`
  (save-now) presses during a session rewrite the same block, never
  duplicate.

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

On a 32 GB M4 MacBook Air with the recommended stack:
- Whisper large-v3-turbo: transcribes faster than realtime on Metal
- Qwen2.5-14B Q4_K_M: ~15–25 tok/s → a 20-word translation in ~2 s
- Diarizer (sherpa-onnx ERes2Net): tens of ms per segment on CPU
- Peak RSS: ~12 GB (plenty of headroom on 32 GB)

For lighter setups: use `ggml-small.bin` + Qwen2.5-7B-Instruct-Q4_K_M.

## Roadmap

- [x] Phase 1: cpal → VAD → whisper.cpp → ratatui UI → Markdown export
- [x] Phase 1.5: hallucination filters (silence fallbacks, YouTube tails, repetition loops)
- [x] Phase 2: bilingual side-by-side (Qwen via llama.cpp)
- [ ] Phase 3: speaker diarization (sherpa-onnx)
- [ ] Phase 4: configurable shortcuts, device picker, live draft buffer

## Logs

Diagnostic logs are written to `livemd.log` (keeps the TUI clean). Set
`RUST_LOG=debug` for verbose output.
