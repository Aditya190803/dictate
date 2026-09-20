# dictate

Speech-to-text for Linux (Wayland) and Windows: one shortcut, daemon-friendly, stdout-first. Words appear as you speak, pauses polish the last phrase, and voice edits (`scratch that`, `no I mean …`) rewrite what was just typed.

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
irm https://dictate.adityamer.dev/install.ps1 | iex
dictate setup
dictate doctor
```

No ydotool/wl-clipboard/PipeWire — typing, paste, and clipboard are in-process Win32. Full guide: **[docs/windows.md](docs/windows.md)**.

## Shortcut

One key starts and stops dictation (`dictate`):

| Platform | Default |
|----------|---------|
| **Linux** | Super+R |
| **Windows** | Ctrl+Alt+R (`Win+R` is the Run dialog and cannot be registered) |

GNOME and Windows: `dictate setup` installs the key. Hyprland/Niri: bind the key to `dictate`.

Run `dictate setup` once.

## Features

- **STT:** Mistral (default), Groq, or local Whisper (`--features local`)
- **Polish:** Independent of STT — `POLISH_PROVIDER=auto` uses **OpenCode Zen** (`big-pickle`) if `OPENCODE_API_KEY` is set, else Mistral if `MISTRAL_API_KEY` is set, else **Ollama** (`ollama pull gemma-4`). Groq/local STT + Ollama polish works.
- **Dictionary:** `dictate words` — GUI for names, jargon, and misspelling fixes (`words-ui` build)
- **text.toml:** Dictionary, snippets, cleanup, `[polish]` style/model, command mode
- **Output:** stdout, clipboard, type, or paste — `SHORTCUT_OUTPUT` (Linux: `ydotool`/`wl-copy`; Windows: in-process `SendInput`/Win32 clipboard)

## Quick config

`~/.config/dictate/.env` — Windows: `%APPDATA%\dictate\.env` (see `.env.example`):

```bash
TRANSCRIPTION_PROVIDER=mistral
MISTRAL_API_KEY=...
DICTATE_PROFILE=segmented
SHORTCUT_OUTPUT=type
SHORTCUT_KEY_LIVE=SUPER,R
POLISH_PROVIDER=auto               # auto | opencode | mistral | ollama
OPENCODE_API_KEY=...               # text polish only — OpenCode Zen does not do STT
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

CLI: `dictate` · `dictate setup` · `dictate doctor` · `dictate config get|set|edit` · `dictate words`

## Common commands

```bash
dictate setup
dictate doctor
dictate                 # start or stop dictation
dictate words           # dictionary
dictate config set KEY value
```

Bind `dictate` to a shortcut. Clipboard commands run on that same press when you copied text first.

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