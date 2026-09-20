# Wispr Flow vs dictate — useful feature gap

**dictate** is open-source, CLI/daemon-first, Linux/Wayland-focused. **Wispr Flow** is a commercial cross-platform dictation product with proprietary models and mobile apps.

Legend: **Partial** = similar idea, weaker or different UX. **Done** = shipped in dictate (local/Linux).

---

## Platforms

| Flow | dictate |
|------|---------|
| macOS / Windows native apps | Linux (Wayland); Windows via a hotkey agent from `dictate setup` (`RegisterHotKey`, see `docs/windows.md`); no macOS app |
| iOS / Android | — |
| Browser / web dictation | — |
| Linux / WSL / terminal | **Native** CLI + compositor shortcuts |

---

## Dictation UX

| Flow | dictate |
|------|---------|
| **Flow Bar** | Daemon (beeps / terminal feedback); Windows global hotkeys via `dictate setup` |
| **Scratchpad** | **Done:** `~/.local/share/dictate/scratchpad.md` |
| **Hands-free** | **Done:** one shortcut (`SUPER,R`; Windows `CTRL,ALT,R`) starts and stops dictation |
| Onboarding | `dictate setup`, `dictate doctor` |
| **Sync across devices** | — (local only) |
| Transcript **history** | **Done:** `history.jsonl` (`[history] enabled` in text.toml) |

---

## Intelligence & formatting

| Flow | dictate |
|------|---------|
| Polished writing | **Done:** `[polish]` on the whole take when you stop, via OpenCode Zen / Mistral / Ollama |
| **Flow Styles** | **Done:** `[polish].style` — casual, formal, concise, email, bullets |
| **Smart formatting & backtrack** | **Partial:** self-corrections (`no wait, it’s Wednesday`) and spoken layout in the polish pass; `scratch that` on a take that is only that command |
| Realtime formatting | **Done:** no live typing; insert once after polish so corrections and formatting see the full take |
| **Context awareness** | **Partial:** `CONTEXT_EDITING` + session buffer; no app OCR |
| IDE variable recognition | **Partial:** `--dictation-mode code-symbols` |
| **File tagging** | — |
| Command mode (AI on selection) | **Done:** `DICTATE_PROFILE=command` + `[command_mode] use_llm` |

---

## Snippets, dictionary, shortcuts

| Flow | dictate |
|------|---------|
| Snippets / dictionary GUI | **Partial:** `dictate words` GUI for preferred words + dictionary aliases; snippets still use `text.toml` |
| Hotkeys | One shortcut from `dictate setup` (bind `dictate`) |

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

CLI/stdout/`--pipe-to`, local Whisper, Groq, open `.env` + `text.toml`, GPL, developer dictation modes, `SIGUSR1` daemon.

See [`context-aware-editing.md`](context-aware-editing.md) for context-editing notes.
