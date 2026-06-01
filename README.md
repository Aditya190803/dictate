# dictate - Wayland Speech-to-Text Tool

Press a keybind, speak, and get instant text output. A speech-to-text tool that transcribes audio using Mistral, Groq, or local Whisper and outputs to stdout.

## Features

- **Signal-driven**: Press keybind → speak → get text (no GUI needed)
- **UNIX philosophy**: Outputs transcribed text to stdout for piping to other tools
- **On-demand operation**: Starts when called, processes audio, then exits
- **Audio feedback**: Beeps confirm recording start/stop and success
- **Wayland native**: Works with modern Linux desktops (Hyprland, Niri, etc.)
- **Optional local transcription**: Run Whisper locally using whisper-rs

## Requirements

- **Wayland desktop** (Hyprland, Niri, GNOME, KDE, etc.)
- **Mistral or Groq API key** (for online transcription)
- **System packages**:

```bash
# Arch Linux
sudo pacman -S pipewire

# Ubuntu/Debian  
sudo apt install pipewire-pulse

# Fedora
sudo dnf install pipewire-pulseaudio
```

**Optional (for direct typing keybindings):**
```bash
# Arch Linux
sudo pacman -S ydotool

# Ubuntu/Debian  
sudo apt install ydotool

# Fedora
sudo dnf install ydotool

# Setup ydotool permissions and service:
sudo usermod -a -G input $USER

# Enable and start ydotool daemon service
sudo systemctl enable --now ydotool.service

# Set socket environment variable (add to ~/.bashrc or ~/.zshrc)
echo 'export YDOTOOL_SOCKET=/tmp/.ydotool_socket' >> ~/.bashrc

# Log out and back in (or source ~/.bashrc)
```

## Installation

### One-command installer

```bash
curl -fsSL https://dictate.adityamer.dev/install.sh | sh
```

The installer detects common Linux package managers, installs/checks system dependencies, downloads the latest release binary when available, falls back to building from source, writes `~/.config/dictate/.env`, and prints shortcut snippets. See [`INSTALL.md`](INSTALL.md) for manual steps and a coding-agent prompt.

### From AUR (Arch Linux)

```bash
# Using your preferred AUR helper
yay -S dictate-bin
# or
paru -S dictate-bin
```

### Download Binary

1. Download from [GitHub Releases](https://github.com/Aditya190803/dictate/releases)
2. Install:

```bash
wget https://github.com/Aditya190803/dictate/releases/latest/download/dictate-linux-x86_64
mkdir -p ~/.local/bin
mv dictate-linux-x86_64 ~/.local/bin/dictate
chmod +x ~/.local/bin/dictate

# Add to PATH (add to ~/.bashrc or ~/.zshrc)
export PATH="$HOME/.local/bin:$PATH"
```

## Quick Start

1. **Setup configuration:**
```bash
# Create config directory and file
mkdir -p ~/.config/dictate
echo "MISTRAL_API_KEY=your_api_key_here" > ~/.config/dictate/.env
```

2. **Test the application:**
```bash
# Run dictate and pipe output to see it working
dictate | tee /tmp/dictate-output.txt
```

3. **Use with signals:**
```bash
# Transcribe and output to stdout
pkill --signal SIGUSR1 dictate
```

## Quick Reference

### Common Commands

```bash
# Download local model and exit
dictate --download-model

# Start dictate and save output to file
dictate > output.txt

# Start dictate and copy output to clipboard
dictate --pipe-to wl-copy

# Start dictate and type output directly
dictate --pipe-to ydotool type --file -

# Trigger transcription (if dictate is running)
pkill --signal SIGUSR1 dictate
```

### Keybinding Pattern

Most keybindings follow this pattern:
```bash
pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate [OPTIONS] &)
```

This means: "If dictate is running, send signal to transcribe. Otherwise, start dictate with specified options."

## Keyboard Shortcuts Setup

### Hyprland

Add to your `~/.config/hypr/hyprland.conf`:

```bash
# dictate - Speech to Text (direct typing)
bind = SUPER, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to ydotool type --file - &)

# dictate - Speech to Text (clipboard copy)  
bind = SUPER SHIFT, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to wl-copy &)
```

### Niri

Add to your `~/.config/niri/config.kdl`:

```kdl
binds {
    // dictate - Speech to Text (direct typing)
    Mod+R { spawn "sh" "-c" "pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to ydotool type --file - &)"; }
    
    // dictate - Speech to Text (clipboard copy)
    Mod+Shift+R { spawn "sh" "-c" "pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to wl-copy &)"; }
}
```

**Keybinding Functions:**
- **Super+R** (Hyprland) / **Mod+R** (Niri): Direct typing via ydotool
- **Super+Shift+R** (Hyprland) / **Mod+Shift+R** (Niri): Copy to clipboard

## Usage Examples

dictate starts on-demand, records audio, transcribes it, outputs to stdout, then exits:

### Basic Usage (stdout)

```bash
# Terminal 1: Start dictate with output to file
dictate > transcription.txt

# Terminal 2: Trigger transcription (or use keyboard shortcut)
pkill --signal SIGUSR1 dictate
```

### Using --pipe-to Option

The `--pipe-to` option allows you to pipe transcribed text directly to another command:

```bash
# Copy transcription to clipboard
dictate --pipe-to wl-copy
pkill --signal SIGUSR1 dictate

# Type transcription directly into focused window
dictate --pipe-to ydotool type --file -
pkill --signal SIGUSR1 dictate

# Process transcription with sed and copy to clipboard
dictate --pipe-to sh -c "sed 's/hello/hi/g' | wl-copy"
pkill --signal SIGUSR1 dictate

# Save to file with timestamp
dictate --pipe-to sh -c "echo \"$(date): $(cat)\" >> speech-log.txt"
pkill --signal SIGUSR1 dictate
```


## Configuration

Configuration is read from `~/.config/dictate/.env` by default. You can override this location using the `--envfile` flag:

```bash
dictate --envfile /path/to/custom/.env
```

You can edit config from the CLI:

```bash
dictate config wizard
dictate config get
dictate config set provider groq
dictate config set groq-model whisper-large-v3-turbo
dictate config set shortcut-key SUPER,R
dictate config edit
```

Generate compositor shortcuts:

```bash
dictate shortcuts hyprland --mode type --key SUPER,R
dictate shortcuts niri --mode clipboard --key Mod+Shift+R
```

dictate supports three transcription providers: **Mistral** (default), **Groq**, and **Local Whisper**.

Mistral uses true realtime STT by default, including from normal keyboard shortcuts. Set `BATCH_MODE=true` to opt out and use whole-clip batch transcription with the normal audio endpoint.

### Mistral (Default)

**Required:** Create `~/.config/dictate/.env` with your Mistral API key:

```bash
MISTRAL_API_KEY=your_api_key_here
```

**Optional Mistral settings:**
```bash
TRANSCRIPTION_PROVIDER=mistral

# false = realtime by default for Mistral, including keyboard shortcuts
# true = opt out and use whole-clip batch transcription
BATCH_MODE=false

# Legacy override: auto, realtime, or batch
TRANSCRIPTION_MODE=auto

# Batch/offline transcription model
MISTRAL_MODEL=voxtral-mini-latest
#MISTRAL_BASE_URL=https://api.mistral.ai/v1

# Realtime WebSocket transcription model
MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602
MISTRAL_REALTIME_DELAY_MS=480
#MISTRAL_REALTIME_BASE_URL=wss://api.mistral.ai
```

### Groq

```bash
TRANSCRIPTION_PROVIDER=groq
GROQ_API_KEY=your_api_key_here
GROQ_MODEL=whisper-large-v3-turbo
#GROQ_BASE_URL=https://api.groq.com/openai/v1

# Groq does not support Mistral realtime WebSockets,
# so it stays on provider batch/VAD behavior.
BATCH_MODE=true
```

### Shared Online Settings

```bash
# Force specific language, or auto-detect
TRANSCRIPTION_LANGUAGE=auto

# API timeout in seconds
TRANSCRIPTION_TIMEOUT_SECONDS=60

# Max retry attempts
TRANSCRIPTION_MAX_RETRIES=3
```

### Local Whisper (whisper-rs)

Run transcription locally without sending audio to external APIs. Models are downloaded from [Hugging Face](https://huggingface.co/ggerganov/whisper.cpp) in GGML format. Local stream mode uses VAD chunks and defaults away from realtime because it keeps CPU/GPU work local.

Local Whisper is optional at build time. Install it with:

```bash
DICTATE_BUILD_FROM_SOURCE=yes DICTATE_BUILD_FEATURES=local curl -fsSL https://dictate.adityamer.dev/install.sh | sh
```

Or build manually with `cargo build --release --features local`.

```bash
# Switch to local provider
TRANSCRIPTION_PROVIDER=local

# Model file name stored in ~/.local/share/applications/dictate/models/
WHISPER_MODEL=ggml-base.en.bin

# Download the model and exit
dictate --download-model
```

**Available Models (GGML format):**
- `ggml-tiny.bin` - Fastest, least accurate (39 MB)
- `ggml-tiny.en.bin` - English-only tiny model (39 MB)
- `ggml-base.bin` - Small size, good performance (142 MB)
- `ggml-base.en.bin` - English-only base model (142 MB)
- `ggml-small.bin` - Better accuracy than base (466 MB)
- `ggml-small.en.bin` - English-only small model (466 MB)
- `ggml-medium.bin` - Good accuracy/speed balance (1.5 GB)
- `ggml-medium.en.bin` - English-only medium model (1.5 GB)
- `ggml-large.bin` - Best accuracy, slower (2.9 GB)
- `ggml-large-v1.bin` - Large model v1 (2.9 GB)
- `ggml-large-v2.bin` - Large model v2 (2.9 GB)
- `ggml-large-v3.bin` - Latest large model (2.9 GB)

**Recommendations:**
- **For English only**: Use `.en.bin` models for better performance
- **For speed**: `ggml-tiny.en.bin` or `ggml-base.en.bin`
- **For accuracy**: `ggml-large-v3.bin` or `ggml-medium.en.bin`
- **For balance**: `ggml-base.en.bin` (default)

If the configured model is missing, the application will exit with an error. Mistral remains the default provider.

### General Settings

**Audio and system settings:**
```bash
# Disable audio beeps
ENABLE_AUDIO_FEEDBACK=false

# Adjust beep volume (0.0 to 1.0)
BEEP_VOLUME=0.1

# Debug logging
RUST_LOG=debug
```


## Troubleshooting

### Audio Issues

If audio recording fails:
- Ensure PipeWire is running: `systemctl --user status pipewire`
- Check microphone permissions
- Verify microphone is not muted


### API Issues

**Mistral Provider:**
- Verify `MISTRAL_API_KEY` is valid and has sufficient credits
- Check internet connectivity
- Review logs for specific error messages

**Groq Provider:**
- Verify `GROQ_API_KEY` is valid and has sufficient credits
- Check internet connectivity
- Review logs for specific error messages

## Development

### Running Tests

```bash
cargo test
```

### Running with Debug Output

```bash
# Using default config location (~/.config/dictate/.env)
RUST_LOG=debug cargo run

# Or using project-local .env file for development
RUST_LOG=debug cargo run -- --envfile .env
```

## Building from Source

```bash
git clone https://github.com/Aditya190803/dictate.git
cd dictate

# Create config directory and copy example configuration
mkdir -p ~/.config/dictate
cp .env.example ~/.config/dictate/.env
# Edit ~/.config/dictate/.env with your API key

# Build the project
cargo build --release

# Install to local bin
mkdir -p ~/.local/bin
cp ./target/release/dictate ~/.local/bin/
```

## License

Licensed under GPL v3.0 or later. Source code: https://github.com/Aditya190803/dictate

See [LICENSE](LICENSE) for full terms.