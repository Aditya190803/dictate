# dictate

Speech-to-text for Linux (Wayland) and Windows: global shortcuts, daemon-friendly, stdout-first. Speak in phrases and get **polished text** in the focused app (default **segmented** profile), or use **live typing** / **smart paste** on a second key.

Not [Wispr Flow](https://wisprflow.ai/) — see [docs/wispr-flow-gap.md](docs/wispr-flow-gap.md).

## Install

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | sh
dictate setup
dictate doctor
```

Manual build, dependencies, and compositor shortcuts: **[INSTALL.md](INSTALL.md)**.

```bash
# From source (dictionary GUI)
cargo build --release --features words-ui
```

### Windows

```powershell
cargo build --release          # no C compiler needed; MSVC or MinGW linker required
dictate setup
dictate autostart install      # background hotkey agent, runs at login
```

No ydotool/wl-clipboard/PipeWire — typing, paste, and clipboard are in-process Win32. Full guide: **[docs/windows.md](docs/windows.md)**.

## Recommended shortcuts

| Key | Behavior |
|-----|----------|
| **Super+R** | Live typing — Mistral realtime deltas as you speak (`dictate toggle live`) |
| **Super+Shift+R** | Smart paste — record, stop, polish once, paste (`dictate toggle smart`) |

GNOME: `dictate shortcuts gnome --install`. Hyprland/Niri: `dictate shortcuts hyprland` / `niri` (paste into config).

**Windows:** `Win+R` is reserved by the OS, so the defaults will not register — use `SHORTCUT_KEY_LIVE=CTRL,ALT,R` and `SHORTCUT_KEY_SMART=CTRL,ALT,SHIFT,R`, then `dictate hotkeys` (or `dictate autostart install`).

Default **segmented** dictation (`DICTATE_PROFILE=segmented` or `dictate --daemon`): pause-bound segments, dictionary/snippets/cleanup, optional **LLM polish** per segment, voice edits (“scratch that”). Run `dictate setup` once.

## Features

- **STT:** Mistral (default), Groq, or local Whisper (`--features local`)
- **Polish:** Independent of STT — `POLISH_PROVIDER=auto` uses Mistral if `MISTRAL_API_KEY` is set, else **Ollama** (`ollama pull gemma-4`). Groq/local STT + Ollama polish works.
- **Dictionary:** `dictate words` — GUI for names, jargon, and misspelling fixes (`words-ui` build)
- **text.toml:** Dictionary, snippets, cleanup, `[polish]` style/model, command mode
- **Extras:** `dictate history`, `dictate scratchpad`, clipboard-aware command mode
- **Output:** stdout, clipboard, type, or paste — `SHORTCUT_OUTPUT` / `--pipe-to` (Linux: `ydotool`/`wl-copy`; Windows: in-process `SendInput`/Win32 clipboard)

## Quick config

`~/.config/dictate/.env` — Windows: `%APPDATA%\dictate\.env` (see `.env.example`):

```bash
TRANSCRIPTION_PROVIDER=mistral
MISTRAL_API_KEY=...
DICTATE_PROFILE=segmented          # or live_typing, smart_paste, batch_clip
SHORTCUT_OUTPUT=type
SHORTCUT_KEY_LIVE=SUPER,R
SHORTCUT_KEY_SMART=SUPER,SHIFT,R
POLISH_PROVIDER=auto               # auto | mistral | ollama
OLLAMA_BASE_URL=http://127.0.0.1:11434
OLLAMA_POLISH_MODEL=gemma-4
```

Optional `~/.config/dictate/text.toml` (Windows: `%APPDATA%\dictate\text.toml`):

```toml
preferred_words = ["Hyprland", "Supabase"]

[dictionary]
"super base" = "Supabase"

[polish]
enabled = true
style = "concise"    # casual, formal, email, bullets — see polish_styles
on_failure = "fallback"
```

CLI: `dictate config wizard` · `dictate config get|set|edit` · `dictate words`

## Common commands

```bash
dictate --daemon                   # segmented (default product)
dictate --daemon --mode live       # realtime typing daemon
dictate --daemon --mode smart      # smart paste daemon
dictate --pipe-to wl-copy          # one-shot clip → clipboard
dictate --command                  # transform clipboard from voice instruction
dictate --download-model           # local Whisper GGML into ~/.local/share/dictate/models
dictate --dictation-mode terminal  # developer formatting modes
```

Signal toggle while a daemon runs: `pkill -SIGUSR1 dictate` (shortcuts usually wrap `dictate toggle live|smart`). Windows has no SIGUSR1 — daemons listen on a named pipe and `dictate toggle live|smart` is the only path.

## Profiles (power users)

| `DICTATE_PROFILE` | What you get |
|-------------------|--------------|
| `segmented` | Default — phrases, per-segment polish, session context |
| `live_typing` | Raw realtime deltas, minimal latency |
| `smart_paste` | One recording, polish whole clip, paste at end |
| `batch_clip` | One clip, local cleanup only, no LLM polish |
| `command` | Voice instruction applies to clipboard text |

## Requirements

- Mistral or Groq key **or** local model
- **Linux:** Wayland, PipeWire; **ydotool** + user in `input` group for `SHORTCUT_OUTPUT=type` / `paste`
- **Windows:** nothing extra (WASAPI + Win32). `words-ui` and `local` features are Linux-only — see [docs/windows.md](docs/windows.md)

## Development

```bash
cargo test
cargo build --release --features words-ui
RUST_LOG=debug cargo run -- --envfile .env
```

## License

GPL-3.0-or-later — [LICENSE](LICENSE) · [GitHub](https://github.com/Aditya190803/dictate)