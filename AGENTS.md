# dictate — agent setup (local)

Follow **INSTALL.md → “For AI Coding Agents”**. This file is the checklist for this machine.

## Install (already on PATH)

```bash
cargo build --release --features overlay
cp target/release/dictate target/release/dictate-overlay ~/.local/bin/
```

Verify: `dictate --version` (expect **1.1.0+**).

## Non-interactive config

```bash
dictate config set DICTATE_PROFILE live_typing    # or smart_paste | batch_clip
dictate config set SHORTCUT_DESKTOP gnome          # match $XDG_CURRENT_DESKTOP
dictate config set SHORTCUT_OUTPUT type            # type | paste | clipboard | stdout
dictate config set SHORTCUT_KEY_LIVE 'SUPER,R'
dictate config set SHORTCUT_KEY_SMART 'SUPER,SHIFT,R'
dictate config set ENABLE_OVERLAY true             # optional pill; needs overlay build
# API keys: dictate config set MISTRAL_API_KEY '...'  (never commit)
```

## Shortcuts

```bash
dictate shortcuts gnome --profile live_typing --mode type --key SUPER,R
dictate doctor
```

**GNOME:** Settings → Keyboard → Custom Shortcuts → paste the printed command.  
**Hyprland/Niri:** paste into compositor config (see INSTALL.md).

## Run

- **Live typing:** `dictate --daemon` (or shortcut that starts daemon + SIGUSR1 toggle).
- **Overlay:** `ENABLE_OVERLAY=true` in `~/.config/dictate/.env` + daemon spawns `dictate-overlay`.

## Optional

- `~/.config/dictate/text.toml` — dictionary, snippets, cleanup, `[polish]` for smart_paste.
- `dictate config wizard` — full interactive setup when flags are unknown.