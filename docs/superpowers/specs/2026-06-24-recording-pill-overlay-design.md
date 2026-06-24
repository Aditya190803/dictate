# Recording pill overlay (Wispr-style waveform)

## Status

**Implemented** (feature `overlay`, binary `dictate-overlay`). Off by default (`ENABLE_OVERLAY=false`). Uses **gtk4-layer-shell** (`Layer::Overlay`, bottom anchor, click-through).

## Goal

Small floating **pill** near cursor or bottom-center while dictating:

- Animated **waveform** from mic RMS (or VAD level)
- States: idle / listening / processing (smart paste)
- Minimal chrome (no full GUI app)

## Why separate from core

- Wispr Flow is a desktop app with always-on UI; dictate is signal/daemon + stdout.
- Overlay needs a **second process** or thread + Wayland layer-shell (or X11), not just Rust STT.

## Recommended approach (Linux / Wayland)

1. **`dictate-overlay`** (optional binary or `dictate --overlay` sidecar)
   - Subscribes to dictate via **Unix socket** or env `DICTATE_OVERLAY_SOCKET`
   - Daemon emits: `{ "state": "listening", "level": 0.0..1.0 }` each ~50ms from existing `AudioProcessor::calculate_rms` / VAD
2. **UI toolkit** (pick one):
   - **egui + winit** + `layer-shell` (lightweight, one repo)
   - **GTK4 + LayerShell** (better on GNOME)
   - **slint** (declarative pill + bars)
3. **Pill layout**: ~120×36px, rounded, semi-transparent; 12–24 vertical bars driven by `level` history (ring buffer)

## IPC sketch

```json
{"type":"level","v":0.42}
{"type":"state","s":"listening|processing|idle"}
```

Emit from `streaming.rs` / daemon when `ENABLE_OVERLAY=true`.

## Setup

- `dictate setup`: one Confirm — “Show recording pill? (experimental)” default **No**
- If yes: write `ENABLE_OVERLAY=true`, print `dictate-overlay &` in autostart hint

## Non-goals (v1 overlay)

- macOS/Windows pill (follow after Linux stable)
- Click-through settings in pill (use `dictate setup` only)

## Next step

Approve this spec → implement socket + minimal egui pill on `feat/recording-pill`.