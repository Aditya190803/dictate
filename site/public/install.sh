#!/bin/sh
# dictate — One-command installer
# Copyright (C) 2025 Artur Roszczyk
# License: GPL-3.0-or-later
#
# Usage:
#   curl -fsSL https://dictate.adityamer.dev/install.sh | sh
#
# Environment overrides:
#   DICTATE_BUILD_FROM_SOURCE=yes   Force building from source
#   DICTATE_BUILD_FEATURES=local    Build with local Whisper support
#   DICTATE_INSTALL_DIR=/usr/local/bin  Custom install location

set -e

# ─── Colors ──────────────────────────────────────────────────────────
if [ -t 1 ]; then
  GREEN='\033[0;32m'; BOLD='\033[1m'; YELLOW='\033[1;33m'
  RED='\033[0;31m'; CYAN='\033[0;36m'; NC='\033[0m'
else
  GREEN=''; BOLD=''; YELLOW=''; RED=''; CYAN=''; NC=''
fi

info()  { printf "${GREEN}✓${NC} %s\n" "$1"; }
warn()  { printf "${YELLOW}⚠${NC} %s\n" "$1"; }
error() { printf "${RED}✗${NC} %s\n" "$1"; }
step()  { printf "\n${BOLD}── %s ──${NC}\n" "$1"; }
code()  { printf "  ${CYAN}%s${NC}\n" "$1"; }

# ─── Usage ────────────────────────────────────────────────────────────
usage() {
  cat <<EOF
Usage: curl -fsSL https://dictate.adityamer.dev/install.sh | sh

Environment variables:
  DICTATE_BUILD_FROM_SOURCE  Force build from source (default: no)
  DICTATE_BUILD_FEATURES     Cargo features (default: "", use "local" for local Whisper)
  DICTATE_INSTALL_DIR        Install directory (default: ~/.local/bin)
EOF
  exit 0
}

[ "$1" = "--help" ] || [ "$1" = "-h" ] && usage

# ─── Detect OS & Architecture ─────────────────────────────────────────
OS="$(uname -s)"
ARCH="$(uname -m)"
case "$OS" in
  Linux) ;;
  *) error "Unsupported OS: $OS (only Linux is supported)"; exit 1 ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH_TARGET="x86_64" ;;
  aarch64|arm64) ARCH_TARGET="aarch64" ;;
  *) error "Unsupported architecture: $ARCH"; exit 1 ;;
esac

info "Detected ${OS} / ${ARCH_TARGET}"

# ─── Detect Distro ────────────────────────────────────────────────────
detect_distro() {
  if command -v pacman >/dev/null 2>&1; then
    echo "arch"
  elif command -v apt-get >/dev/null 2>&1; then
    echo "debian"
  elif command -v dnf >/dev/null 2>&1; then
    echo "fedora"
  elif command -v zypper >/dev/null 2>&1; then
    echo "opensuse"
  elif command -v apk >/dev/null 2>&1; then
    echo "alpine"
  elif command -v xbps-install >/dev/null 2>&1; then
    echo "void"
  elif command -v emerge >/dev/null 2>&1; then
    echo "gentoo"
  else
    echo "unknown"
  fi
}

DISTRO="$(detect_distro)"
info "Distro: ${DISTRO}"

# ─── Check / Install Dependencies ─────────────────────────────────────
step "Dependencies"

install_deps() {
  case "$DISTRO" in
    arch)
      if ! command -v pipewire >/dev/null 2>&1; then
        info "Installing PipeWire..."
        sudo pacman -Sy --noconfirm pipewire pipewire-pulse wireplumber 2>/dev/null || true
      else
        info "PipeWire already installed"
      fi
      if ! command -v ydotool >/dev/null 2>&1; then
        warn "ydotool not installed (optional, for direct typing)"
        echo "  Install with: sudo pacman -S ydotool"
      fi
      ;;
    debian)
      if ! dpkg -l pipewire >/dev/null 2>&1; then
        info "Installing PipeWire..."
        sudo apt-get update -qq 2>/dev/null || true
        sudo apt-get install -y -qq pipewire pipewire-pulse wireplumber 2>/dev/null || true
      else
        info "PipeWire already installed"
      fi
      if ! command -v ydotool >/dev/null 2>&1; then
        warn "ydotool not installed (optional, for direct typing)"
        echo "  Install with: sudo apt install ydotool"
      fi
      ;;
    fedora)
      if ! rpm -q pipewire >/dev/null 2>&1; then
        info "Installing PipeWire..."
        sudo dnf install -y pipewire pipewire-pulseaudio wireplumber 2>/dev/null || true
      else
        info "PipeWire already installed"
      fi
      if ! command -v ydotool >/dev/null 2>&1; then
        warn "ydotool not installed (optional, for direct typing)"
        echo "  Install with: sudo dnf install ydotool"
      fi
      ;;
    opensuse)
      if ! rpm -q pipewire >/dev/null 2>&1; then
        info "Installing PipeWire..."
        sudo zypper install -y pipewire pipewire-pulseaudio wireplumber 2>/dev/null || true
      else
        info "PipeWire already installed"
      fi
      ;;
    alpine)
      if ! command -v pipewire >/dev/null 2>&1; then
        info "Installing PipeWire..."
        sudo apk add pipewire pipewire-pulse wireplumber 2>/dev/null || true
      else
        info "PipeWire already installed"
      fi
      ;;
    void)
      if ! command -v pipewire >/dev/null 2>&1; then
        info "Installing PipeWire..."
        sudo xbps-install -S pipewire pipewire-pulse wireplumber 2>/dev/null || true
      else
        info "PipeWire already installed"
      fi
      ;;
    *)
      warn "Unknown distro — please install PipeWire manually if needed"
      ;;
  esac

  # Check that pipewire-pulse is running
  if command -v systemctl >/dev/null 2>&1; then
    if systemctl --user is-active pipewire >/dev/null 2>&1; then
      info "PipeWire is running"
    else
      warn "PipeWire may not be running. Start with: systemctl --user start pipewire"
    fi
  fi
}

install_deps

# ─── Determine Install Dir ────────────────────────────────────────────
INSTALL_DIR="${DICTATE_INSTALL_DIR:-$HOME/.local/bin}"
mkdir -p "$INSTALL_DIR"

# ─── Download or Build ────────────────────────────────────────────────
step "Installing dictate"

BIN_PATH="$INSTALL_DIR/dictate"

install_from_source() {
  info "Building from source..."
  
  # Check for Rust
  if ! command -v cargo >/dev/null 2>&1; then
    info "Installing Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
  fi
  
  # Install system build deps
  case "$DISTRO" in
    arch) sudo pacman -Sy --noconfirm base-devel 2>/dev/null || true ;;
    debian) sudo apt-get install -y -qq build-essential pkg-config libasound2-dev 2>/dev/null || true ;;
    fedora) sudo dnf install -y gcc pkg-config alsa-lib-devel 2>/dev/null || true ;;
    *) warn "Make sure build-essential, pkg-config, and ALSA dev libs are installed" ;;
  esac

  # Build
  if [ ! -d /tmp/dictate-build ]; then
    git clone --depth 1 https://github.com/Aditya190803/dictate.git /tmp/dictate-build
  fi
  cd /tmp/dictate-build

  FEATURES="${DICTATE_BUILD_FEATURES:-}"
  if [ -n "$FEATURES" ]; then
    info "Building with features: ${FEATURES}"
    cargo build --release --features "$FEATURES"
  else
    cargo build --release
  fi

  cp target/release/dictate "$BIN_PATH"
  chmod +x "$BIN_PATH"
  info "Built dictate and installed to ${BIN_PATH}"
}

# Try binary download first
DOWNLOAD_URL="https://github.com/Aditya190803/dictate/releases/latest/download/dictate-linux-${ARCH_TARGET}"

if [ "$DICTATE_BUILD_FROM_SOURCE" = "yes" ]; then
  warn "DICTATE_BUILD_FROM_SOURCE=yes — building from source"
  install_from_source
else
  info "Downloading dictate..."
  HTTP_CODE=$(curl -fsSL -w '%{http_code}' -o /tmp/dictate "$DOWNLOAD_URL" 2>/dev/null || echo "failed")
  
  if [ "$HTTP_CODE" = "200" ] || [ "$HTTP_CODE" = "302" ]; then
    mv /tmp/dictate "$BIN_PATH"
    chmod +x "$BIN_PATH"
    info "Downloaded dictate to ${BIN_PATH}"
  else
    warn "Binary download failed (HTTP ${HTTP_CODE})"
    install_from_source
  fi
fi

# Verify
if command -v dictate >/dev/null 2>&1 || [ -x "$BIN_PATH" ]; then
  # Make sure it's in PATH
  if ! command -v dictate >/dev/null 2>&1; then
    # Add to current shell's PATH
    export PATH="$INSTALL_DIR:$PATH"
    case "$SHELL" in
      */zsh) shell_rc="$HOME/.zshrc" ;;
      */bash) shell_rc="$HOME/.bashrc" ;;
      *) shell_rc="$HOME/.profile" ;;
    esac
    if ! grep -q "export PATH=\"\$HOME/.local/bin:\$PATH\"" "$shell_rc" 2>/dev/null; then
      echo "" >> "$shell_rc"
      echo "# dictate" >> "$shell_rc"
      echo "export PATH=\"\$HOME/.local/bin:\$PATH\"" >> "$shell_rc"
      info "Added ~/.local/bin to PATH in ${shell_rc}"
    fi
  fi
  
  VERSION=$(dictate --version 2>/dev/null || echo "unknown")
  info "dictate ${VERSION} installed successfully!"
else
  error "Installation failed — dictate binary not found at ${BIN_PATH}"
  exit 1
fi

# ─── Configuration ────────────────────────────────────────────────────
step "Configuration"

CONFIG_DIR="$HOME/.config/dictate"
CONFIG_FILE="$CONFIG_DIR/.env"

if [ ! -f "$CONFIG_FILE" ]; then
  mkdir -p "$CONFIG_DIR"
  cat > "$CONFIG_FILE" << 'ENVEOF'
# dictate configuration
# Generated by install.sh — edit this file or use `dictate config wizard`
TRANSCRIPTION_PROVIDER=mistral
MISTRAL_MODEL=voxtral-mini-latest
GROQ_MODEL=whisper-large-v3-turbo
TRANSCRIPTION_LANGUAGE=auto
TRANSCRIPTION_TIMEOUT_SECONDS=60
TRANSCRIPTION_MAX_RETRIES=3
ENABLE_AUDIO_FEEDBACK=true
BEEP_VOLUME=0.1
ENVEOF
  info "Created default config at ${CONFIG_FILE}"
fi

# Prompt for API key if not set
CURRENT_API_KEY=""
if [ -f "$CONFIG_FILE" ]; then
  CURRENT_API_KEY=$(grep -E '^MISTRAL_API_KEY=' "$CONFIG_FILE" 2>/dev/null | cut -d= -f2- | tr -d '"')
  if [ -z "$CURRENT_API_KEY" ] || [ "$CURRENT_API_KEY" = "your_mistral_api_key_here" ]; then
    CURRENT_API_KEY=""
  fi
fi

if [ -z "$CURRENT_API_KEY" ]; then
  printf "\nSelect transcription provider [mistral/groq/local] (default: mistral): "
  read -r PROVIDER; PROVIDER="${PROVIDER:-mistral}"
  
  case "$PROVIDER" in
    groq)
      printf "Enter your Groq API key: "
      read -r API_KEY
      dictate config set provider groq 2>/dev/null || true
      dictate config set groq-api-key "$API_KEY" 2>/dev/null || true
      # Also write directly in case dictate isn't fully configured
      if grep -q "^GROQ_API_KEY=" "$CONFIG_FILE"; then
        sed -i "s/^GROQ_API_KEY=.*/GROQ_API_KEY=$API_KEY/" "$CONFIG_FILE"
      else
        echo "GROQ_API_KEY=$API_KEY" >> "$CONFIG_FILE"
      fi
      if grep -q "^TRANSCRIPTION_PROVIDER=" "$CONFIG_FILE"; then
        sed -i "s/^TRANSCRIPTION_PROVIDER=.*/TRANSCRIPTION_PROVIDER=groq/" "$CONFIG_FILE"
      else
        echo "TRANSCRIPTION_PROVIDER=groq" >> "$CONFIG_FILE"
      fi
      info "Configured Groq provider"
      ;;
    local)
      dictate config set provider local 2>/dev/null || true
      dictate config set whisper-model ggml-base.en.bin 2>/dev/null || true
      if grep -q "^TRANSCRIPTION_PROVIDER=" "$CONFIG_FILE"; then
        sed -i "s/^TRANSCRIPTION_PROVIDER=.*/TRANSCRIPTION_PROVIDER=local/" "$CONFIG_FILE"
      fi
      info "Configured local Whisper provider"
      warn "Local Whisper requires a model download: dictate --download-model"
      ;;
    mistral|*)
      printf "Enter your Mistral API key: "
      read -r API_KEY
      dictate config set provider mistral 2>/dev/null || true
      dictate config set mistral-api-key "$API_KEY" 2>/dev/null || true
      if grep -q "^MISTRAL_API_KEY=" "$CONFIG_FILE"; then
        sed -i "s/^MISTRAL_API_KEY=.*/MISTRAL_API_KEY=$API_KEY/" "$CONFIG_FILE"
      else
        echo "MISTRAL_API_KEY=$API_KEY" >> "$CONFIG_FILE"
      fi
      if grep -q "^TRANSCRIPTION_PROVIDER=" "$CONFIG_FILE"; then
        sed -i "s/^TRANSCRIPTION_PROVIDER=.*/TRANSCRIPTION_PROVIDER=mistral/" "$CONFIG_FILE"
      else
        echo "TRANSCRIPTION_PROVIDER=mistral" >> "$CONFIG_FILE"
      fi
      info "Configured Mistral provider"
      ;;
  esac
else
  info "API key already configured"
fi

# ─── Shortcut Snippets ────────────────────────────────────────────────
step "Quick Start"

printf "\n${BOLD}Try it now:${NC}\n"
code "dictate > /tmp/speech.txt"
code "pkill --signal SIGUSR1 dictate"
code "cat /tmp/speech.txt"

printf "\n${BOLD}Hyprland keybind (add to ~/.config/hypr/hyprland.conf):${NC}\n"
printf "  ${CYAN}bind = SUPER, R, exec, pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to ydotool type --file - &)${NC}\n"

printf "\n${BOLD}Niri keybind (add to ~/.config/niri/config.kdl):${NC}\n"
printf "  ${CYAN}Mod+R { spawn \"sh\" \"-c\" \"pgrep -x dictate >/dev/null && pkill --signal SIGUSR1 dictate || (dictate --pipe-to ydotool type --file - &)\"; }${NC}\n"

printf "\n${BOLD}Generate shortcuts for your compositor:${NC}\n"
code "dictate shortcuts hyprland --mode type --key SUPER,R"
code "dictate shortcuts niri --mode clipboard --key Mod+Shift+R"

printf "\n${BOLD}Configuration commands:${NC}\n"
code "dictate config wizard"
code "dictate config get"
code "dictate config set provider groq"
code "dictate config set shortcut-key SUPER,R"

printf "\n${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
printf "${GREEN}${BOLD}  dictate is ready!${NC}\n"
printf "${GREEN}  Press ${BOLD}Super+R${NC}${GREEN} to speak and get text.${NC}\n"
printf "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}\n"
