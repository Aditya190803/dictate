# dictate — One-command installer for Windows
# Copyright (C) 2025 Artur Roszczyk
# License: GPL-3.0-or-later
#
# Usage (PowerShell):
#   irm https://dictate.adityamer.dev/install.ps1 | iex
#
# Environment overrides:
#   $env:DICTATE_BUILD_FROM_SOURCE = "yes"   Force building from source
#   $env:DICTATE_INSTALL_DIR = "C:\path\to\bin"  Custom install location
#   $env:DICTATE_SKIP_WIZARD = "yes"         Skip interactive setup
#
# Mirrors install.sh: tries the latest GitHub Release binary first,
# falls back to building from source.

$ErrorActionPreference = "Stop"

function Info($msg) { Write-Host "✓ $msg" -ForegroundColor Green }
function Warn($msg) { Write-Host "⚠ $msg" -ForegroundColor Yellow }
function Fail($msg) { Write-Host "✗ $msg" -ForegroundColor Red }
function Step($msg) { Write-Host "`n── $msg ──" -ForegroundColor White }
function Code($msg) { Write-Host "  $msg" -ForegroundColor Cyan }

Step "Installing dictate"

# ─── Install location ──────────────────────────────────────────────
$InstallDir = $env:DICTATE_INSTALL_DIR
if ([string]::IsNullOrWhiteSpace($InstallDir)) {
  $InstallDir = Join-Path $env:USERPROFILE "bin"
}
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
$BinPath = Join-Path $InstallDir "dictate.exe"
Info "Install dir: $InstallDir"

# ─── Download or build ─────────────────────────────────────────────
$DownloadUrl = "https://github.com/Aditya190803/dictate/releases/latest/download/dictate-windows-x86_64.exe"

function Install-FromSource {
  Info "Building from source..."

  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Info "Installing Rust (rustup)..."
    if (Get-Command winget -ErrorAction SilentlyContinue) {
      winget install --silent --accept-source-agreements --accept-package-agreements Rustlang.Rustup
      $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
      if (Test-Path $cargoBin) { $env:PATH = "$cargoBin;$env:PATH" }
    } else {
      $rustupInit = Join-Path $env:TEMP "rustup-init.exe"
      Invoke-WebRequest -Uri "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe" -OutFile $rustupInit -UseBasicParsing
      & $rustupInit -y --default-toolchain stable
      $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    }
  }
  if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Fail "cargo still not found after rustup install. Restart PowerShell and re-run the installer."
    exit 1
  }

  if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
    Fail "git is required to build from source. Install it (winget install Git.Git) and re-run."
    exit 1
  }

  $BuildDir = Join-Path $env:TEMP "dictate-build"
  if (-not (Test-Path (Join-Path $BuildDir ".git"))) {
    if (Test-Path $BuildDir) { Remove-Item -Recurse -Force $BuildDir }
    git clone --depth 1 https://github.com/Aditya190803/dictate.git $BuildDir
  }
  Push-Location $BuildDir
  try {
    # words-ui (GTK4) and local (whisper-rs) are not supported on Windows.
    cargo build --release
    Copy-Item -Force (Join-Path "target\release\dictate.exe") $BinPath
  } finally {
    Pop-Location
  }
  Info "Built dictate and installed to $BinPath"
}

$installed = $false
if ($env:DICTATE_BUILD_FROM_SOURCE -eq "yes") {
  Warn "DICTATE_BUILD_FROM_SOURCE=yes — building from source"
  Install-FromSource
  $installed = $true
} else {
  Info "Downloading dictate..."
  try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile "$BinPath.tmp" -UseBasicParsing
    Move-Item -Force "$BinPath.tmp" $BinPath
    Info "Downloaded dictate to $BinPath"
    $installed = $true
  } catch {
    Warn "Binary download failed ($($_.Exception.Message)) — building from source"
    if (Test-Path "$BinPath.tmp") { Remove-Item -Force "$BinPath.tmp" -ErrorAction SilentlyContinue }
    Install-FromSource
    $installed = $true
  }
}

# ─── PATH ──────────────────────────────────────────────────────────
# Persist for future shells (HKCU) and fix the current session.
$currentSessionHasIt = ($env:PATH -split ";" | Where-Object { $_ -eq $InstallDir }) -ne $null -and @($env:PATH -split ";" | Where-Object { $_ -eq $InstallDir }).Count -gt 0
if (-not $currentSessionHasIt) {
  $env:PATH = "$InstallDir;$env:PATH"
}
try {
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if ($null -eq $userPath) { $userPath = "" }
  $parts = $userPath -split ";" | Where-Object { $_ -ne "" }
  if ($parts -notcontains $InstallDir) {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
    Info "Added $InstallDir to user PATH (restart shell to pick it up everywhere)"
  }
} catch {
  Warn "Could not persist PATH automatically. Add $InstallDir to PATH manually."
}

try {
  $version = & $BinPath --version 2>$null
  Info "dictate $version installed successfully!"
} catch {
  Warn "Installed to $BinPath but --version failed. Make sure $InstallDir is on PATH."
}

# ─── Configuration ─────────────────────────────────────────────────
Step "Configuration"

function Write-DefaultConfig {
  $ConfigDir = Join-Path $env:APPDATA "dictate"
  $ConfigFile = Join-Path $ConfigDir ".env"
  New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
  if (-not (Test-Path $ConfigFile)) {
    @(
      "# dictate configuration",
      '# Generated by install.ps1 — finish setup with `dictate setup`',
      "TRANSCRIPTION_PROVIDER=mistral",
      "DICTATE_PROFILE=segmented",
      "TRANSCRIPTION_MODE=auto",
      "MISTRAL_MODEL=voxtral-mini-latest",
      "MISTRAL_REALTIME_MODEL=voxtral-mini-transcribe-realtime-2602",
      "MISTRAL_REALTIME_DELAY_MS=480",
      "GROQ_MODEL=whisper-large-v3-turbo",
      "TRANSCRIPTION_LANGUAGE=auto",
      "TRANSCRIPTION_TIMEOUT_SECONDS=60",
      "TRANSCRIPTION_MAX_RETRIES=3",
      "ENABLE_AUDIO_FEEDBACK=true",
      "BEEP_VOLUME=0.1",
      "# Win+R is reserved by Windows — defaults use Ctrl+Alt+R",
      "SHORTCUT_KEY_LIVE=CTRL,ALT,R",
      "SHORTCUT_KEY_SMART=CTRL,ALT,SHIFT,R",
      "SHORTCUT_OUTPUT=type"
    ) -join "`r`n" | Set-Content -Path $ConfigFile -Encoding utf8
    Info "Created default config at $ConfigFile"
  } else {
    Info "Config already exists at $ConfigFile"
  }
}

if ($env:DICTATE_SKIP_WIZARD -eq "yes") {
  Warn "DICTATE_SKIP_WIZARD=yes — skipping interactive setup."
  Warn "Configure with: dictate setup"
  Write-DefaultConfig
} else {
  try {
    & $BinPath setup
    Info "Configuration complete"
  } catch {
    Warn "Setup did not complete ($($_.Exception.Message))"
    Warn "Run this after install: dictate setup"
    Write-DefaultConfig
  }
}

# ─── Quick start ───────────────────────────────────────────────────
Step "Quick Start"

Write-Host ""
Write-Host "Try it now:" -ForegroundColor White
Code "dictate setup"
Code "dictate doctor"
Code "dictate"
Write-Host ""
Write-Host "Hotkeys (Win+R is reserved — defaults use Ctrl+Alt+R):" -ForegroundColor White
Code "dictate setup    # installs the hotkey agent at login"
Write-Host ""
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
Write-Host "  dictate is ready!" -ForegroundColor Green
Write-Host "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━" -ForegroundColor Green
