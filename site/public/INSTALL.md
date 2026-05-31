# dictate — Installation Guide

## Quick Install

One command to install dictate on any Linux distro:

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | sh
```

The installer will:
1. Detect your distro and install system dependencies (PipeWire, etc.)
2. Download the latest binary from GitHub Releases
3. Fall back to building from source if no binary is available
4. Create `~/.config/dictate/.env` with defaults
5. Prompt for your API key and preferred provider
6. Print shortcut snippets for your compositor

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

## Configuration

### Create Config File

```bash
mkdir -p ~/.config/dictate
```

### Using the Wizard

```bash
dictate config wizard
```

The wizard asks for:
- **Provider** — `mistral` (default), `groq`, or `local`
- **API key** — your Mistral or Groq API key
- **Model** — model name for the chosen provider
- **Language** — `auto` or an ISO code like `en`
- **Output mode** — `type`, `clipboard`, or `stdout`
- **Desktop** — `hyprland` or `niri`
- **Shortcut key** — e.g. `SUPER,R` or `Mod+R`

### Manual Config

Create `~/.config/dictate/.env`:

```bash
# Provider: mistral, groq, or local
TRANSCRIPTION_PROVIDER=mistral

# Mistral (default)
MISTRAL_API_KEY=your_mistral_api_key_here
MISTRAL_MODEL=voxtral-mini-latest

# Groq
# GROQ_API_KEY=your_groq_api_key_here
# GROQ_MODEL=whisper-large-v3-turbo

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

# Remove AUR package (if installed via AUR)
yay -R dictate-bin
# or
paru -R dictate-bin
```

---

## For AI Coding Agents

This guide can be used as context for AI coding agents to install and configure dictate automatically. Provide the agent with the URL to this file and ask it to:

1. Read this INSTALL.md
2. Ask you setup questions (provider, API key, desktop, etc.)
3. Execute the installation and configuration steps

```text
Read https://dictate.adityamer.dev/INSTALL.md and follow it step by step 
to install and configure dictate on this machine. Ask me the setup questions 
first, then execute everything non-interactively using 'dictate config set'.
```
