# dictate — Installation Guide

## Quick Install

One command to install dictate on any Linux distro:

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | sh
```

The installer will:
1. Detect your distro and install system dependencies (PipeWire, etc.)
2. Download the latest binary from GitHub Releases, or build from source
3. Install `dictate` locally
4. Run `dictate config wizard` locally so you can configure everything interactively
5. Print shortcut instructions generated from your wizard answers

The wizard asks you:
- **Provider** — mistral (default), groq, or local
- **API key** — your Mistral or Groq API key
- **Model** — model name for your provider
- **Language** — auto or an ISO code like `en`
- **Output mode** — type (directly into window), clipboard, or stdout
- **Desktop environment** — hyprland, niri, gnome, kde, sway, or other
- **Shortcut key** — e.g. `SUPER,R`, `Mod,R`, or `<Super>r`
- **Audio feedback** — enable/disable beeps and choose beep volume

**Release note:** the fastest path needs a GitHub Release binary. If no matching release binary is available, the installer falls back to building from source. To force source builds, run:

```bash
DICTATE_BUILD_FROM_SOURCE=yes sh -c "$(curl -fsSL https://dictate.adityamer.dev/install.sh)"
```

---

## Manual Installation

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

### Option B: Build from Source

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

## Configuration

### Create Config File

```bash
mkdir -p ~/.config/dictate
```

### Using the Wizard

Interactive setup:

```bash
dictate config wizard
```

Non-interactive setup, useful for scripts and AI agents after collecting answers:

```bash
dictate config wizard \
  --provider mistral \
  --mistral-api-key "$MISTRAL_API_KEY" \
  --mistral-model voxtral-mini-latest \
  --language auto \
  --output-mode type \
  --desktop hyprland \
  --shortcut-key SUPER,R \
  --audio-feedback true \
  --beep-volume 0.1
```

For Groq, use `--provider groq --groq-api-key "$GROQ_API_KEY" --groq-model whisper-large-v3-turbo`. For local Whisper, use `--provider local --whisper-model ggml-base.en.bin`, then run `dictate --download-model`.

The wizard supports:
- **Provider** — `mistral` (default), `groq`, or `local`
- **API key** — your Mistral or Groq API key
- **Model** — model name for the chosen provider
- **Batch mode** — `false` by default for Mistral realtime; set `true` to use whole-clip batch transcription
- **Language** — `auto` or an ISO code like `en`
- **Output mode** — `type`, `clipboard`, or `stdout`
- **Desktop** — `hyprland`, `niri`, `gnome`, `kde`, `sway`, or `other`
- **Shortcut key** — e.g. `SUPER,R`, `Mod,R`, or `<Super>r`
- **Audio feedback** — enable/disable beeps and choose beep volume

### Manual Config

Create `~/.config/dictate/.env`:

```bash
# Provider: mistral, groq, or local
TRANSCRIPTION_PROVIDER=mistral

# Mistral (default)
MISTRAL_API_KEY=your_mistral_api_key_here

# false = realtime by default for Mistral, including keyboard shortcuts
# true = opt out and use whole-clip batch transcription
BATCH_MODE=false

# Legacy override: auto, realtime, or batch
TRANSCRIPTION_MODE=auto

# Batch/offline model
MISTRAL_MODEL=voxtral-mini-latest

# Realtime WebSocket model
MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602
MISTRAL_REALTIME_DELAY_MS=480

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
dictate config set provider groq
dictate config set groq-model whisper-large-v3-turbo
dictate config set batch-mode true
dictate config set shortcut-key SUPER,R
dictate config get
dictate config edit
```

### Using a Custom Config Path

```bash
dictate --envfile /path/to/custom/.env
```

---

## Local Whisper Setup

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

### Hyprland

Add to `~/.config/hypr/hyprland.conf`:

```bash
# Direct typing
bind = SUPER, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGTERM dictate || (dictate --stream --pipe-to ydotool type --file - &)

# Clipboard copy
bind = SUPER SHIFT, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGTERM dictate || (dictate --stream --pipe-to wl-copy &)
```

### Niri

Add to `~/.config/niri/config.kdl`:

```kdl
binds {
    // Direct typing
    Mod+R { spawn "sh" "-c" "pgrep -x dictate >/dev/null && pkill --signal SIGTERM dictate || (dictate --stream --pipe-to ydotool type --file - &)"; }
    
    // Clipboard copy
    Mod+Shift+R { spawn "sh" "-c" "pgrep -x dictate >/dev/null && pkill --signal SIGTERM dictate || (dictate --stream --pipe-to wl-copy &)"; }
}
```

### Generate Shortcuts Automatically

```bash
dictate shortcuts hyprland --mode type --key SUPER,R
dictate shortcuts niri --mode clipboard --key Mod+Shift+R
```

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

# Note: if you installed via a package manager, use its removal command instead
```

---

## For AI Coding Agents

AI agents should ask the user the setup questions first, install with the interactive wizard skipped, then run `dictate config wizard` with flags so setup completes without asking the user to do anything manually.

Ask the user:
1. **Provider** — `mistral`, `groq`, or `local`?
2. **API key** — Mistral or Groq key, if using an online provider?
3. **Model** — provider model, or default?
4. **Language** — `auto` or an ISO code like `en`?
5. **Output mode** — `type`, `clipboard`, or `stdout`?
6. **Desktop** — `hyprland`, `niri`, `gnome`, `kde`, `sway`, or `other`?
7. **Shortcut key** — e.g. `SUPER,R`, `Mod,R`, or `<Super>r`?
8. **Audio feedback** — enable beeps? What volume?

Then run:

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | DICTATE_SKIP_WIZARD=yes sh

dictate config wizard \
  --provider mistral \
  --mistral-api-key "$MISTRAL_API_KEY" \
  --mistral-model voxtral-mini-latest \
  --language auto \
  --output-mode type \
  --desktop hyprland \
  --shortcut-key SUPER,R \
  --audio-feedback true \
  --beep-volume 0.1

dictate shortcuts hyprland --mode type --key SUPER,R
```

For Groq, use `--provider groq --groq-api-key "$GROQ_API_KEY" --groq-model whisper-large-v3-turbo`. For local Whisper, use `--provider local --whisper-model ggml-base.en.bin`, then run `dictate --download-model`.
