# Wispr Flow vs dictate — useful feature gap

**dictate** is open-source, CLI/daemon-first, Linux/Wayland-focused. **Wispr Flow** is a commercial cross-platform dictation product with proprietary models and mobile apps.

Legend: **Partial** = similar idea, weaker or different UX. **Done** = shipped in dictate (local/Linux).

---

## Platforms

| Flow | dictate |
|------|---------|
| macOS / Windows native apps | Linux (Wayland); no macOS/Windows app |
| iOS / Android | — |
| Browser / web dictation | — |
| Linux / WSL / terminal | **Native** CLI + compositor shortcuts |

---

## Dictation UX

| Flow | dictate |
|------|---------|
| **Flow Bar** | Daemon (beeps / terminal feedback) |
| **Scratchpad** | **Done:** `dictate scratchpad` → `~/.local/share/dictate/scratchpad.md` |
| **Hands-free** | **Done:** live typing daemon on `SUPER,R`; smart paste available on a second shortcut |
| Onboarding | `dictate setup`, `dictate config wizard`, `dictate doctor` |
| **Sync across devices** | — (local only) |
| Transcript **history** | **Done:** `dictate history list|clear` → `history.jsonl` (`[history] enabled` in text.toml) |

---

## Intelligence & formatting

| Flow | dictate |
|------|---------|
| Polished writing | **Partial:** `[polish]` Mistral on **smart_paste** + **segmented**; live typing skips polish for latency |
| **Flow Styles** | **Done:** `[polish].style` — casual, formal, concise, email, bullets |
| **Smart formatting & backtrack** | **Partial:** cleanup + replace X with Y + voice deletes; `undo that` → last sentence |
| **Context awareness** | **Partial:** `CONTEXT_EDITING` + session buffer; no app OCR |
| IDE variable recognition | **Partial:** `--dictation-mode code-symbols` |
| **File tagging** | — |
| Command mode (AI on selection) | **Done:** `DICTATE_PROFILE=command` + `[command_mode] use_llm` |
| Realtime formatting | **Partial:** raw deltas on `live_typing`; segmented/smart modes format after a pause or stop |

---

## Snippets, dictionary, shortcuts

| Flow | dictate |
|------|---------|
| Snippets / dictionary GUI | **Partial:** `dictate words` GUI for preferred words + dictionary aliases; snippets still use `text.toml` |
| Hotkeys | `dictate shortcuts` (Hyprland/Niri/GNOME/KDE/Sway), with exact daemon matching per profile |

---

## Audio & environment

| Flow | dictate |
|------|---------|
| Mic setup | `dictate doctor` + PipeWire |
| Remote desktop (VDI) | — |

---

## Still out of scope (by design)

Mobile apps, macOS/Windows clients, cloud sync, in-app styles GUI, file tagging, IDE integration, OCR context, remote-desktop guarantees.

---

## dictate strengths

CLI/stdout/`--pipe-to`, local Whisper, Groq, open `.env` + `text.toml`, GPL, developer dictation modes, segmented/live/smart/batch profiles, `SIGUSR1` daemon.

See [`context-aware-editing.md`](context-aware-editing.md) for context-editing notes.
