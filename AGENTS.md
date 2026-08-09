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

## Windows

Supported target; **not** a fork — one binary, `#[cfg]` split. Guide: [`docs/windows.md`](docs/windows.md).

| Concern | Linux | Windows |
|---------|-------|---------|
| Output sinks (`type`/`paste`/`clipboard`) | `ydotool`, `wl-copy` subprocesses | in-process Win32 `SendInput` / clipboard |
| Daemon toggle | `SIGUSR1` | named pipe `\\.\pipe\dictate-{live,smart,main}` |
| Global shortcuts | compositor binds | `dictate hotkeys` agent (`RegisterHotKey`) |
| Autostart | systemd user units | `HKCU\...\CurrentVersion\Run` + `%APPDATA%\dictate\hotkey-agent.vbs` |
| Config dir | `~/.config/dictate` | `%APPDATA%\dictate` |
| TLS | rustls + ring | SChannel via `native-tls` (keep it — `ring` needs a C compiler) |

Code: `src/platform/` (`mod.rs` routes in-process sinks via the `@dictate` pseudo-command; `unix.rs` vs `windows/{input,clipboard,hotkeys,autostart}.rs`) and `src/control.rs` (toggle transport).

Not built on Windows: `words-ui` (GTK4) and `local` (whisper-rs).

**No C compiler is needed** — that is why the Windows TLS backend is SChannel, not `ring`. A linker still is: `x86_64-pc-windows-msvc` uses the VS Build Tools, and `x86_64-pc-windows-gnu` needs MinGW-w64 binutils on `PATH` (`dlltool` + `as`, for the `raw-dylib` import libraries tokio and `windows-sys` generate). rustup's bundled `dlltool` is **not** sufficient — it shells out to `as`, which rustup does not ship.

Default `SUPER,R` cannot register on Windows (`Win+R` is the Run dialog); use `CTRL,ALT,R` / `CTRL,ALT,SHIFT,R`.

## Optional

- `~/.config/dictate/text.toml` — dictionary, snippets, cleanup, `[polish]` for smart_paste.
- `dictate config wizard` — full interactive setup when flags are unknown.
- [`docs/wispr-flow-gap.md`](docs/wispr-flow-gap.md) — Wispr Flow vs dictate.
