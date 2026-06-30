# dictate → FluidVoice-class parity (extension plan)

This document lists what **dictate** would need to feel **comparable to [FluidVoice](https://github.com/altic-dev/FluidVoice)** for daily dictation on Linux (and optionally Windows)—without porting the macOS Swift app.

**How to use it:** Each row is a trackable gap. Phases order work by user-visible value vs engineering risk. See also [`docs/wispr-flow-gap.md`](docs/wispr-flow-gap.md) (commercial Flow comparison).

**Reference (FluidVoice):** macOS menu-bar app, global hotkeys, live overlay/notch, multi ASR engines (CoreML Parakeet/Nemotron, Apple Speech, Whisper), smart typing into any app, optional cloud + **Fluid Intelligence** (private on-device LLM), Command Mode agent, Write/Rewrite modes, history/stats, per-app prompts, custom dictionary/vocabulary boost, meeting transcription, local API, auto-updates.

**dictate today:** ~10k LOC Rust, Wayland-first CLI/daemon, Mistral realtime live typing, Groq/Mistral clip + local Whisper, polish/styles, command mode, overlay pill, history, scratchpad, compositor shortcuts, `text.toml` dictionary/snippets.

---

## Legend

| Status | Meaning |
|--------|---------|
| **Done** | Shipped in dictate (may differ in UX) |
| **Partial** | Same intent, weaker or Linux-only |
| **Gap** | Not implemented |
| **N/A** | FluidVoice/mac-specific; optional substitute only |

---

## Feature matrix

### Platforms & distribution

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| macOS app | Linux Wayland CLI | Partial | **Windows port** (cpal, global hotkey, clipboard typing); keep CLI-first |
| Homebrew / signed .dmg | curl install, AUR, releases | Partial | **Flatpak** manifest; **winget** / MSI for Windows |
| Auto-updates | Manual / package manager | Gap | `dictate self-update` or release-check in `doctor` |
| Login at startup | User systemd / compositor autostart | Partial | Document + `dictate install --user-service` (systemd unit) |

### Activation & hotkeys

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Global hotkey (hold / tap / multi-mode) | Compositor binds + `SIGUSR1` toggle | Partial | **In-process global hotkey** (optional) via evdev/portal where possible; **hold-to-talk** vs toggle in daemon |
| Separate shortcuts: dictation, command, rewrite, prompt slots | live / smart / segmented profiles | Partial | **Named modes** in config matching Fluid: `dictate toggle rewrite`, `dictate toggle command` |
| Hotkey capture UI in settings | `dictate config wizard` / TUI | Partial | Same; optional GUI later |

### Live UX & overlay

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Notch / live preview with partial text | `dictate-overlay` (layer shell) | Partial | Stream **partial transcript** to overlay IPC; audio level meter |
| Multiple overlay sizes | Single pill | Gap | Configurable HUD size / position |
| Mode label (dictation / command / write) | Minimal | Gap | Overlay shows active profile + model |
| Sounds on start/stop | Beeps | Done | Match Fluid SFX optional assets |

### Speech recognition (ASR)

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Parakeet / Nemotron (CoreML, low latency) | Mistral realtime (cloud) | Partial | **CPU local fast path:** whisper.cpp streaming and/or **ONNX** Parakeet-class when viable |
| Apple Speech | — | N/A | — |
| Whisper (many sizes) | `local` feature + Groq | Partial | **Model manager:** `dictate models list|download|use`; disk budget |
| Cohere Transcribe | — | Gap | Optional provider if ONNX/API path exists |
| Language picker per engine | `transcription_language` | Partial | Per-provider language tables in doctor/setup |
| Custom vocabulary / word boost (Parakeet) | `text.toml` dictionary | Partial | **Phrase list** export for future local ASR; fuzzy boost hooks |
| Word boost status in UI | — | Gap | Log/doctor line: “dictionary: N terms” |

### Text insertion (“smart typing”)

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Paste with clipboard restore | `wl-copy` + ydotool paste | Partial | **Save/restore clipboard** around paste (like Fluid `TypingService`) |
| Keystroke typing with layout awareness | `ydotool type` | Partial | **Insertion strategy chain:** paste → type → retry; document Tier-1 apps |
| Focus / field verification | — | Gap | Post-insert check (selection length, clipboard echo) with timeout |
| Per-app insertion mode | — | Gap | `[apps."org.mozilla.firefox"]` insertion = paste in `text.toml` |
| Accessibility permission flow | ydotool group + doctor | Partial | Expand **doctor** for AT-SPI, portal, ydotool socket |

### AI enhancement (post-dictation)

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Fluid Intelligence (private local LLM) | — | N/A | **Sidecar adapter** if win/linux runtime ever exists; until then Ollama/OpenAI-compatible |
| OpenAI / Groq / custom providers | Groq + Mistral polish | Partial | **Provider table** in config: base URL, model, key env vars |
| Enhancement on every dictation | smart/segmented + `[polish]` | Partial | **Always-enhance** toggle per profile |
| Context from active app (name, title) | — | Gap | **Active window context** (Wayland wlr-foreign-toplevel / hyprctl / GNOME Shell D-Bus) injected into polish prompt |

### Modes: Write / Rewrite / Edit

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Write mode (dictate into field) | live + pipe-to type | Done | — |
| Rewrite selection | command mode + clipboard | Partial | **Rewrite profile:** read selection via AT-SPI/primary selection → replace in field |
| Edit / prompt profiles (multiple) | polish styles | Partial | **Named prompts** in `text.toml` (`[[prompts]]` id, system, user template) |
| Per-app prompt binding | — | Gap | `[app_prompts."code-oss"]` → prompt id |

### Command Mode

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Voice agent + tools (terminal, apps) | `command` profile + LLM transform | Partial | **Tool loop:** shell exec (whitelist), `xdg-open`, read file; safety prompts |
| Conversation history + UI | — | Gap | `dictate command repl` or TUI; persist `command_sessions.jsonl` |
| Streaming thinking / steps in overlay | — | Gap | Stream command output to overlay IPC |
| mac Shortcuts integration | — | N/A | **Linux:** scripts dir + `dictate run-script` |

### History, stats, scratchpad

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Transcription history + export | `history.jsonl` | Partial | **Search, export ZIP**, optional audio retention toggle |
| Today usage stats | — | Gap | Daily aggregates in history index; `dictate stats today` |
| Scratchpad | scratchpad.md | Done | Optional GUI view later |
| Audio history / replay | — | Gap | Optional WAV segments on disk with budget cap |

### Settings & onboarding

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Full settings GUI | TUI wizard + `dictate config` | Partial | Optional **Tauri tray app** (settings only) reading same `.env` / `text.toml` |
| Onboarding: permissions + try dictation | `setup`, `doctor` | Partial | Guided **first recording** + insertion test target window |
| Adaptive light/dark theme | — | Gap | Only if GUI; overlay GTK theme follow |

### Integrations & API

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Local HTTP API | — | Gap | **localhost API:** start/stop, last transcript (opt-in) for automation |
| Menu bar status | — | Partial | **Tray icon** (GTK/libappindicator) or Tauri |
| PostHog analytics | — | Gap | Opt-in telemetry or skip (privacy default: off) |

### Meeting / long-form

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Meeting transcription mode | — | Gap | Long-record profile + speaker-less transcript file |
| System audio capture | Mic only | Gap | PipeWire monitor source option (document legal/UX) |

### Security & storage

| FluidVoice | dictate today | Status | Extension |
|------------|---------------|--------|-----------|
| Keychain for API keys | `.env` plaintext | Partial | **Secret Service** (libsecret) optional backend for keys |
| Backup / restore settings | Manual copy | Gap | `dictate config export|import` archive |

---

## Phased roadmap

### Phase 0 — Parity hygiene (1–2 weeks)

- [ ] Keep `extend.md` / matrix in sync with releases.
- [ ] `dictate doctor`: clipboard restore test, ydotool, Mistral/Groq/Whisper reachability, overlay binary.
- [ ] Document **Tier-1 Linux** (GNOME Wayland, KDE, Hyprland) vs best-effort elsewhere.

### Phase 1 — Daily dictation feels “product-grade” (4–8 weeks)

- [ ] Clipboard save/restore on smart paste path.
- [ ] Partial transcript + level meter in `dictate-overlay` (extend `overlay_ipc`).
- [ ] `dictate models` for local Whisper (download dir, size hints).
- [ ] Provider config: Ollama/OpenAI-compatible URL for polish + command mode.
- [ ] `dictate stats today` + simple history search.
- [ ] Active window title in polish context (best-effort per compositor).

### Phase 2 — Fluid-like modes (6–10 weeks)

- [ ] Rewrite profile: selection capture + replace pipeline.
- [ ] Named prompts + per-app bindings in `text.toml`.
- [ ] Multiple global shortcuts: `dictate toggle` for live / smart / command / rewrite.
- [ ] Command mode: persistent session log + optional TUI.
- [ ] Insertion strategy chain + per-app overrides; expand doctor app matrix.

### Phase 3 — Local ASR depth (8–12 weeks, CPU-first)

- [ ] Streaming local Whisper (lower latency than full clip).
- [ ] Evaluate ONNX Parakeet/Nemotron weights on CPU; ship behind feature flag.
- [ ] Phrase list / dictionary integration for local engines.
- [ ] Optional audio snippets in history (disk budget).

### Phase 4 — Platform & packaging (parallel)

- [ ] **Windows:** cpal, hotkey crate, clipboard paste typing, build in CI.
- [ ] Flatpak + PipeWire permissions.
- [ ] Optional Tauri **settings + tray** (no duplicate daemon logic).
- [ ] `dictate install --user-service` for systemd user daemon.

### Phase 5 — Optional parity extras

- [ ] Local HTTP API.
- [ ] Meeting / long-record profile.
- [ ] Fluid Intelligence sidecar (only if official win/linux binary + API).
- [ ] Command agent tools (whitelisted shell, files) with confirmation gates.

---

## Non-goals (unless requirements change)

- Porting FluidVoice Swift UI or DynamicNotchKit notch geometry on non-Mac hardware.
- macOS build of dictate (FluidVoice already covers mac for GUI users).
- Feature parity with **private Fluid Intelligence** without a supported cross-platform runtime.
- Guaranteed dictation into every Linux app on every compositor without Tier-1 testing.

---

## Success criteria (“comparable to FluidVoice”)

For **personal daily use**, dictate is comparable when:

1. **One shortcut** starts hands-free live dictation with visible feedback (overlay + partial text).
2. **Second shortcut** does polished paste with clipboard safety and reliable Tier-1 app insertion.
3. **Rewrite + command** work on selection without manual copy in common apps.
4. **Local path exists** (Whisper minimum; faster local engine if Phase 3 lands).
5. **History + stats + config** are discoverable without reading the full README.
6. **Windows** (if in scope) matches Linux feature set for dictation modes, not mac-only ASR.

FluidVoice will still win on **mac-native polish**, **CoreML latency**, and **Fluid Intelligence** until equivalent runtimes exist on Linux/Windows.

---

## Tracking

| Phase | Target | Owner |
|-------|--------|-------|
| 0 | doctor + docs | |
| 1 | overlay + clipboard + models + stats | |
| 2 | rewrite + prompts + command sessions | |
| 3 | local streaming / ONNX | |
| 4 | Windows + Flatpak + tray | |
| 5 | API / meeting / FI sidecar | |

Update this table as items ship; link PRs/issues in the repo when opened.