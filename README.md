# livemd

Live bilingual voice transcription TUI in Rust.

- **ASR**: `cpal` → `webrtc-vad` → `whisper.cpp` (Metal) via `whisper-rs`
- **Translation** (optional): Qwen2.5 via `llama-server` subprocess (Metal), queried over HTTP
- **UI**: `ratatui` two-column display (English ↔ 日本語)
- **Output**: timestamped Markdown table

## Requirements

- Rust (stable, 2021 edition)
- CMake + a C/C++ toolchain (needed to build whisper.cpp)
- `llama-server` on PATH → `brew install llama.cpp`
- Apple Silicon (for Metal) recommended

## Model tiers

Pick the tier that matches your machine.

| Tier | Whisper | Translator | Disk | RAM |
|------|---------|------------|------|-----|
| **standard** | `small` (500 MB) | Qwen2.5-7B Q4_K_M (4.5 GB) | ~5 GB | ~7 GB |
| **high** | `large-v3-turbo` (1.6 GB) | Qwen2.5-14B Q4_K_M (8.5 GB) | ~10 GB | ~11 GB |

- **standard** is comfortable on any 16 GB M-series Mac.
- **high** shines on 32 GB Macs and produces noticeably better JA ↔ EN translation
  and ASR accuracy.

## Install

One command does everything (binary install + model downloads + config):

```sh
./setup.sh standard   # 16 GB Macs
# or
./setup.sh high       # 32 GB+ Macs
```

The script is idempotent — already-present models are skipped, so re-running
is safe. It installs `livemd` into `~/.cargo/bin/`, drops models into
`~/.config/livemd/models/`, and writes `~/.config/livemd/livemd.toml` from
the matching example. To switch tiers later, just re-run with the other
argument (models for the other tier will be added alongside; edit the
config by hand if you want to free disk).

### Manual setup

If you prefer to drive it yourself, `livemd.toml.standard.example` and
`livemd.toml.high.example` list the exact model paths expected. Download
the corresponding models into `~/.config/livemd/models/` and copy the
example to `~/.config/livemd/livemd.toml`. Relative paths in the config
resolve against the config file's directory, so `models/foo` means
`~/.config/livemd/models/foo`.

To disable translation, delete or comment out the `[translator]` section.
The transcript-only path keeps working.

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

`▶` marks the source-language side (the one the speaker actually used).
The opposite column shows the Qwen-translated version (or `…` while pending).

### Markdown output

```markdown
| time | English | 日本語 |
|------|---------|--------|
| 10:31:03 | Hello, how are you? | こんにちは、お元気ですか？ |
| 10:31:10 | I'm fine, thanks. | 元気です、ありがとう。 |
```

## Memory & performance (M-series Mac)

On a 32 GB M4 MacBook Air with the recommended stack:
- Whisper large-v3-turbo: transcribes faster than realtime on Metal
- Qwen2.5-14B Q4_K_M: ~15–25 tok/s → a 20-word translation in ~2 s
- Peak RSS: ~12 GB (plenty of headroom on 32 GB)

For lighter setups: use `ggml-small.bin` + Qwen2.5-7B-Instruct-Q4_K_M.

## Roadmap

- [x] Phase 1: cpal → VAD → whisper.cpp → ratatui UI → Markdown export
- [x] Phase 1.5: hallucination filters (silence fallbacks, YouTube tails, repetition loops)
- [x] Phase 2: bilingual side-by-side (Qwen via llama.cpp)
- [ ] Phase 3: configurable shortcuts, device picker, live draft buffer

## Logs

Diagnostic logs are written to `livemd.log` (keeps the TUI clean). Set
`RUST_LOG=debug` for verbose output.
