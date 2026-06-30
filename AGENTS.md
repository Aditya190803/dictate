# dictate — agent setup (local)

Follow **INSTALL.md → “For AI Coding Agents”**. This file is the checklist for this machine.

## Install (already on PATH)

```bash
cargo build --release --features words-ui
cp target/release/dictate ~/.local/bin/
```

Verify: `dictate --version` (expect **1.1.0-dev** while on dev builds).

## Non-interactive config

```bash
# Default profile is segmented (no DICTATE_PROFILE needed). Legacy: live_typing | smart_paste | batch_clip
dictate config set SHORTCUT_DESKTOP gnome          # match $XDG_CURRENT_DESKTOP
dictate config set SHORTCUT_OUTPUT type            # type | paste | clipboard | stdout
dictate config set SHORTCUT_KEY_LIVE 'SUPER,R'
dictate config set SHORTCUT_KEY_SMART 'SUPER,SHIFT,R'
# API keys: dictate config set MISTRAL_API_KEY '...'  (never commit)
```

## Shortcuts

```bash
dictate shortcuts gnome --install   # writes both keys from SHORTCUT_KEY_LIVE / SMART
dictate doctor
```

**GNOME:** `dictate shortcuts gnome --install` (commands: `dictate toggle live` / `smart`).  
**Hyprland/Niri:** paste into compositor config (see INSTALL.md).

## Run

- **Live typing:** `dictate --daemon --mode live` (or shortcut that starts daemon + SIGUSR1 toggle).
- **Dictionary GUI:** `dictate words` (requires `words-ui` build).

## Optional

- `~/.config/dictate/text.toml` — dictionary, snippets, cleanup, `[polish]` for smart_paste.
- `dictate config wizard` — full interactive setup when flags are unknown.
- [`docs/wispr-flow-gap.md`](docs/wispr-flow-gap.md) — Wispr Flow vs dictate.
