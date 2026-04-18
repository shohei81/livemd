# kotoma

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

## Install / update

One command from anywhere — no clone needed:

```sh
curl -fsSL https://raw.githubusercontent.com/shohei81/kotoma/main/install.sh | bash -s -- high
# or
curl -fsSL https://raw.githubusercontent.com/shohei81/kotoma/main/install.sh | bash -s -- standard
```

The installer:
- `cargo install --git … --force kotoma` — builds from `main`, replaces the
  existing binary.
- Downloads tier-appropriate models into `~/.config/kotoma/models/`
  (skips anything already present).
- Writes `~/.config/kotoma/kotoma.toml` from the tier example **only if
  the file doesn't exist yet**. Pass `--reset-config` at the end to force
  overwrite.

### Update

Run the exact same command. `cargo install --force` re-fetches `main`,
rebuilds, and swaps the binary. Existing models and your edited config
are preserved.

### From a cloned repo (dev)

```sh
./install.sh high                  # same flow, cargo install --git
# or, to build the currently checked-out code:
./setup.sh high                    # cargo install --path .
```

### Manual setup

If you prefer to drive it yourself, `kotoma.toml.standard.example` and
`kotoma.toml.high.example` list the exact model paths expected. Download
the corresponding models into `~/.config/kotoma/models/` and copy the
example to `~/.config/kotoma/kotoma.toml`. Relative paths resolve against
the config file's directory, so `models/foo` → `~/.config/kotoma/models/foo`.

To disable translation, delete or comment out the `[translator]` section.
The transcript-only path keeps working.

## Run

```sh
# from anywhere
kotoma notes.md

# or use the default output path from config
kotoma

# append a new session to an existing file
kotoma --resume notes.md

# override language at launch
kotoma meeting.md -l auto

# explicit config file
kotoma notes.md -c ./project-specific.toml
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
2. `./kotoma.toml` in the current directory
3. `~/.config/kotoma/kotoma.toml`

Legacy `livemd.toml` paths (from previous versions) are still picked up as a
fallback if no `kotoma.toml` is present.

### Log location

- App log: `~/.config/kotoma/logs/kotoma.log`
- llama-server log: `~/.config/kotoma/logs/llama-server.log`
- Override: `KOTOMA_LOG_DIR=/some/path kotoma notes.md`

## Development

```sh
cargo run --release          # uses ./kotoma.toml
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
┌ kotoma · REC · lang=en · in=MacBook Pro Mic · model=ggml-small.bin · tr=ready ┐
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

## Logs

Diagnostic logs are written to `kotoma.log` (keeps the TUI clean). Set
`RUST_LOG=debug` for verbose output.
