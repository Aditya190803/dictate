# dictate — Installation Guide

Linux (Wayland) and Windows. For Windows jump to **[Windows](#windows)** — or the full guide, [docs/windows.md](docs/windows.md).

## Quick Install (Linux)

One command to install dictate on any Linux distro:

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | sh
```

The installer will:
1. Detect your distro and install system dependencies (PipeWire, etc.)
2. Download the latest binary from GitHub Releases, or build from source
3. Install `dictate` locally
4. Run **`dictate setup`** to configure interactively
5. Print **one** shortcut for `dictate --daemon`

**Recommended:** **`dictate setup`** then **`dictate doctor`**.

**Default behavior:** speak in phrases; after each pause, dictate inserts **polished, context-aware** text (voice commands like “scratch that” work on what was already typed). Run **`dictate --daemon`** and toggle recording with your shortcut.

Setup asks:
- **Provider** — mistral (recommended for realtime + polish), groq, or local
- **API key** — Mistral or Groq
- **Shortcut output** (`SHORTCUT_OUTPUT`) — `type`, `paste` (clipboard + Ctrl+V), `clipboard`, or `stdout`
- **Desktop** — hyprland, niri, gnome, kde, sway, other
- **Shortcut key** — e.g. `SUPER,R`
- **Audio feedback** — beeps on/off and volume

Verify with `dictate doctor`.

**Release note:** the fastest path needs a GitHub Release binary. If no matching release binary is available, the installer falls back to building from source. To force source builds, run:

```bash
DICTATE_BUILD_FROM_SOURCE=yes sh -c "$(curl -fsSL https://dictate.adityamer.dev/install.sh)"
```

---

## Manual Installation (Linux)

### Prerequisites

- **Wayland desktop** (Hyprland, Niri, GNOME, KDE, Sway, etc.)
- **PipeWire** (for audio capture)
- **An API key** for Mistral or Groq (or use local Whisper)

#### System Dependencies

```bash
# Arch Linux
sudo pacman -S pipewire pipewire-pulse

# Ubuntu / Debian
sudo apt install pipewire pipewire-pulse wireplumber

# Fedora
sudo dnf install pipewire pipewire-pulseaudio wireplumber

# openSUSE
sudo zypper install pipewire pipewire-pulseaudio wireplumber

# Alpine
sudo apk add pipewire pipewire-pulse wireplumber

# Void Linux
sudo xbps-install -S pipewire pipewire-pulse wireplumber
```

Optional for direct typing into focused windows:

```bash
# Arch Linux
sudo pacman -S ydotool

# Ubuntu / Debian
sudo apt install ydotool

# Fedora
sudo dnf install ydotool

# Setup ydotool permissions
sudo usermod -a -G input $USER
sudo systemctl enable --now ydotool.service
echo 'export YDOTOOL_SOCKET=/tmp/.ydotool_socket' >> ~/.bashrc
```

### Option A: Download Binary

```bash
# Download the latest release
wget https://github.com/Aditya190803/dictate/releases/latest/download/dictate-linux-x86_64

# Install to ~/.local/bin
mkdir -p ~/.local/bin
mv dictate-linux-x86_64 ~/.local/bin/dictate
chmod +x ~/.local/bin/dictate

# Ensure ~/.local/bin is in your PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

### Option B: AUR (Arch Linux)

```bash
yay -S dictate-bin
# or
paru -S dictate-bin
```

### Option C: Build from Source

```bash
# Clone the repository
git clone https://github.com/Aditya190803/dictate.git
cd dictate

# Build with default features (Mistral + Groq online providers)
cargo build --release

# Or build with local Whisper support
cargo build --release --features local

# Install to ~/.local/bin
mkdir -p ~/.local/bin
cp target/release/dictate ~/.local/bin/

# Ensure ~/.local/bin is in your PATH
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
```

---

## Windows

Same binary and same config keys as Linux. The platform glue differs: output sinks are in-process Win32 calls (no `ydotool` / `wl-clipboard`), audio is WASAPI (no PipeWire), and daemon control is a named pipe (no `SIGUSR1`). Full detail: **[docs/windows.md](docs/windows.md)**.

### Toolchain

| Item | Status |
|------|--------|
| C compiler | **Not needed.** TLS is per-target: Unix uses rustls+ring, Windows uses SChannel via `native-tls` |
| `x86_64-pc-windows-msvc` | Works with the Visual Studio Build Tools linker |
| `x86_64-pc-windows-gnu` | Works with MinGW-w64 binutils on `PATH`. rustup's bundled `dlltool` is not enough — it invokes `as`, which rustup does not ship |
| Audio | cpal → WASAPI, no extra setup |
| `--features words-ui` | Not supported on Windows (GTK4) |
| `--features local` | Not supported/tested on Windows (whisper-rs needs cmake + clang) |

### Build

Pick one toolchain:

```powershell
# Option A — MSVC (the Rust default on Windows)
winget install Microsoft.VisualStudio.2022.BuildTools    # "Desktop development with C++"
rustup default stable-x86_64-pc-windows-msvc

# Option B — GNU (no Visual Studio)
winget install BrechtSanders.WinLibs.POSIX.UCRT
rustup default stable-x86_64-pc-windows-gnu
$env:PATH = "$env:LOCALAPPDATA\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.UCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin;$env:PATH"
```

```powershell
git clone https://github.com/Aditya190803/dictate.git
cd dictate
cargo build --release
copy target\release\dictate.exe %USERPROFILE%\bin\dictate.exe   # any dir on PATH
```

### Configure

```powershell
dictate setup      # same wizard; reports Windows-specific checks
dictate doctor
```

| Path | Contents |
|------|----------|
| `%APPDATA%\dictate\.env` | Config — same keys as Linux |
| `%APPDATA%\dictate\text.toml` | Dictionary, snippets, cleanup, `[polish]` |
| `%APPDATA%\dictate\` | Models, history, scratchpad |

`SHORTCUT_OUTPUT` on Windows: `type` = Win32 `SendInput` (Unicode) into the focused window · `paste` = clipboard + Ctrl+V · `clipboard` = Win32 clipboard · `stdout` = print. Command mode (`dictate --command`) reads the Win32 clipboard directly.

### Shortcuts

**`Win+R` is reserved by Windows (Run dialog), so the default `SUPER,R` will not register.** Change both keys:

```powershell
dictate config set SHORTCUT_KEY_LIVE  "CTRL,ALT,R"
dictate config set SHORTCUT_KEY_SMART "CTRL,ALT,SHIFT,R"
```

Shortcut strings accept `SUPER,R`, `Meta+Shift+R`, and `<Super>r` styles; a modifier is required.

```powershell
dictate hotkeys            # background agent: RegisterHotKey → dictate toggle live | smart
dictate hotkeys --warm     # also pre-starts idle daemons so the first press is instant
dictate shortcuts windows  # printed setup guidance
```

### Autostart

```powershell
dictate autostart install   # Run-key entry + starts the agent now
dictate autostart status
dictate autostart remove
```

Registers the hotkey agent under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` via a hidden `wscript` launcher at `%APPDATA%\dictate\hotkey-agent.vbs`. (`dictate shortcuts windows --install` is an alias.) On Linux the same command manages systemd user services.

### Daemon control

No `SIGUSR1`. Daemons listen on `\\.\pipe\dictate-live`, `\\.\pipe\dictate-smart`, `\\.\pipe\dictate-main`; `dictate toggle live|smart` connects and sends a toggle. Ctrl+C stops a daemon.

### Windows troubleshooting

| Symptom | Fix |
|---------|-----|
| Hotkey never fires | Combo is taken by Windows or another app (classically `Win+R`) — pick another and restart `dictate hotkeys` |
| Nothing typed into an app running as Administrator | `SendInput` cannot reach elevated windows unless dictate is elevated too; run dictate as Administrator or use `SHORTCUT_OUTPUT=clipboard` |
| `dictate words` opens Notepad instead of the GUI | Expected — the `words-ui` GTK build is Linux-only; set `$EDITOR` to change the editor |
| First key press is slow | `dictate hotkeys --warm`, or `dictate autostart install` |
| `toggle` reports no daemon | Start `dictate --daemon --mode live`, or use `--warm` / autostart |

---

## Configuration

### Create Config File

```bash
mkdir -p ~/.config/dictate     # Windows: %APPDATA%\dictate
```

### Interactive setup

```bash
dictate setup              # guided menus (recommended)
dictate setup --quick      # fewer questions (defaults to mistral)
dictate config wizard      # scriptable flags for CI/agents
dictate doctor             # check API keys, profile, wl-copy/ydotool
```

Non-interactive wizard example:

```bash
dictate config wizard \
  --provider mistral \
  --profile segmented \
  --mistral-api-key "$MISTRAL_API_KEY" \
  --language auto \
  --output-mode type \
  --desktop hyprland \
  --shortcut-key SUPER,R \
  --audio-feedback true
```

For legacy **smart paste** (one shot at end), use `--profile smart_paste`. For Groq: `--provider groq --groq-api-key "$GROQ_API_KEY"`. For local: `--provider local --whisper-model ggml-base.en.bin`, then `dictate --download-model`.

Wizard / setup sets:
- **`DICTATE_PROFILE`** — default `segmented` (omit for new installs); legacy: `live_typing`, `smart_paste`, `batch_clip`
- **`SHORTCUT_OUTPUT`** — default pipe when your shortcut runs plain `dictate` (also used to generate bind lines)
- Provider, API key, language, desktop, shortcut key, beeps

Legacy: `BATCH_MODE=true` still maps to batch-style behavior; prefer `DICTATE_PROFILE=batch_clip`.

### Manual Config

Create `~/.config/dictate/.env`:

```bash
# Provider: mistral, groq, or local
TRANSCRIPTION_PROVIDER=mistral

MISTRAL_API_KEY=your_mistral_api_key_here

# Omit for default segmented dictation, or set explicitly:
DICTATE_PROFILE=segmented

# Default output when shortcut omits --pipe-to: type | paste | clipboard | stdout
SHORTCUT_OUTPUT=type
SHORTCUT_KEY=SUPER,R
SHORTCUT_DESKTOP=hyprland

MISTRAL_MODEL=voxtral-mini-latest
MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602
MISTRAL_REALTIME_DELAY_MS=480

# Optional legacy STT toggles (advanced)
# BATCH_MODE=false
# TRANSCRIPTION_MODE=auto

# Groq
# GROQ_API_KEY=your_groq_api_key_here
# GROQ_MODEL=whisper-large-v3-turbo
# Groq stream mode uses VAD chunking; it does not support Mistral realtime WebSockets.

# Language (auto or ISO code like en)
TRANSCRIPTION_LANGUAGE=auto

# Audio feedback
ENABLE_AUDIO_FEEDBACK=true
BEEP_VOLUME=0.1
```

### Using Config Commands

```bash
dictate config set profile smart_paste
dictate config set shortcut-output paste
dictate config set provider mistral
dictate config set mistral-api-key "$MISTRAL_API_KEY"
dictate config get
dictate config edit
```

### Polish & `text.toml`

Default **`segmented`** dictation polishes **each pause-bound segment** with Mistral chat when `MISTRAL_API_KEY` is set. Without a key, segments still get local dictionary/snippets/cleanup and context edits.

Create `~/.config/dictate/text.toml` (optional):

```toml
[polish]
enabled = true
model = "mistral-small-latest"
on_failure = "fallback"   # fallback = insert locally cleaned text; error = insert nothing

# Same file can hold dictionary, snippets, cleanup — see README
```

Uses the same **`MISTRAL_API_KEY`** as speech-to-text. If polish fails after retries, `on_failure=fallback` pastes the locally processed transcript and prints a warning.

### Using a Custom Config Path

```bash
dictate --envfile /path/to/custom/.env
```

---

## Local Whisper Setup

Linux only — the `local` feature is not supported on Windows.

If you want offline transcription (your audio never leaves your machine):

1. **Build with local feature:**
   ```bash
   cargo build --release --features local
   ```

2. **Configure local provider:**
   ```bash
   dictate config set provider local
   dictate config set whisper-model ggml-base.en.bin
   ```

3. **Download a model:**
   ```bash
   dictate --download-model
   ```

   Models are stored in `~/.local/share/dictate/models/`.

**Available Models:**
| Model | Size | Speed | Accuracy |
|-------|------|-------|----------|
| `ggml-tiny.en.bin` | 39 MB | Fastest | Low |
| `ggml-base.en.bin` | 142 MB | Fast | Good |
| `ggml-small.en.bin` | 466 MB | Moderate | Better |
| `ggml-medium.en.bin` | 1.5 GB | Slow | Great |
| `ggml-large-v3.bin` | 2.9 GB | Slowest | Best |

---

## Composer Shortcuts

Wayland compositors. On Windows use `dictate hotkeys` / `dictate autostart install` — see [Windows](#windows).

### Hyprland

Add to `~/.config/hypr/hyprland.conf`:

```bash
# Direct typing
bind = SUPER, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to ydotool type --file - &)

# Clipboard copy
bind = SUPER SHIFT, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to wl-copy &)
```

### Niri

Add to `~/.config/niri/config.kdl`:

```kdl
binds {
    // Direct typing
    Mod+R { spawn "sh" "-c" "pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to ydotool type --file - &)"; }
    
    // Clipboard copy
    Mod+Shift+R { spawn "sh" "-c" "pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to wl-copy &)"; }
}
```

### Generate Shortcuts Automatically

```bash
dictate shortcuts hyprland --profile segmented --mode type --key SUPER,R
```

Default **`segmented`** snippets include **`--daemon`** so the process stays warm between key presses.

---

## Audio Feedback

dictate plays musical beeps to confirm actions:
- **Recording start** — ascending ding-dong (C4→E4)
- **Recording stop** — descending dong-ding (E4→C4)
- **Success** — double ding (E4, gap, E4)
- **Error** — low warbling tone

Configure in `.env`:
```bash
ENABLE_AUDIO_FEEDBACK=true
BEEP_VOLUME=0.1
```

Set `ENABLE_AUDIO_FEEDBACK=false` to disable all beeps.

---

## Troubleshooting

### "No default input device"
- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check microphone permissions
- Verify mic is not muted

### "MISTRAL_API_KEY is required"
- Create `~/.config/dictate/.env` with your API key
- Or use `dictate config wizard` to set it up

### "Failed to download model"
- Check internet connection
- The model file may be large (up to 3 GB) — be patient
- Ensure you have enough disk space in `~/.local/share/dictate/models/`

### Audio beeps not playing
- Ensure audio output device is connected and working
- Try increasing `BEEP_VOLUME` in config
- Check if `ENABLE_AUDIO_FEEDBACK` is set to `true`

---

## Uninstall

```bash
# Remove the binary
rm -f ~/.local/bin/dictate

# Remove config and data (optional)
rm -rf ~/.config/dictate
rm -rf ~/.local/share/dictate

# Remove AUR package (if installed via AUR)
yay -R dictate-bin
# or
paru -R dictate-bin
```

Windows:

```powershell
dictate autostart remove          # drop the Run-key entry + hotkey agent
del %USERPROFILE%\bin\dictate.exe
rmdir /s /q %APPDATA%\dictate     # config, models, history, scratchpad
```

---

## For AI Coding Agents

AI agents should ask the user the setup questions first, install with the interactive wizard skipped, then run `dictate config wizard` with flags so setup completes without asking the user to do anything manually.

Ask the user:
1. **Provider** — `mistral` (recommended), `groq`, or `local`?
2. **API key** — Mistral or Groq if online (polish needs Mistral key)
3. **API key** — Mistral or Groq if online
4. **Shortcut output** — `type`, `paste`, `clipboard`, or `stdout`?
5. **Desktop** — `hyprland`, `niri`, etc.
6. **Shortcut key** — e.g. `SUPER,R`
7. **Audio feedback** — beeps on/off, volume

Then run:

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | DICTATE_SKIP_WIZARD=yes sh

dictate config wizard \
  --provider mistral \
  --profile segmented \
  --mistral-api-key "$MISTRAL_API_KEY" \
  --language auto \
  --output-mode type \
  --desktop hyprland \
  --shortcut-key SUPER,R

dictate shortcuts hyprland --profile segmented --mode type --key SUPER,R
dictate doctor
```

For Groq, use `--provider groq --groq-api-key "$GROQ_API_KEY" --groq-model whisper-large-v3-turbo`. For local Whisper, use `--provider local --whisper-model ggml-base.en.bin`, then run `dictate --download-model`.
