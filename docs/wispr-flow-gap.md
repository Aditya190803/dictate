# Wispr Flow vs dictate — feature gap

**dictate** is open-source, CLI/daemon-first, Wayland/Linux-focused. **Wispr Flow** is a commercial cross-platform desktop/mobile product with hosted accounts and proprietary models.

Legend: **Partial** = similar idea, weaker or different UX.

---

## Platforms & distribution

| Wispr Flow | dictate |
|------------|---------|
| macOS native app | Linux (Wayland); no macOS/Windows app |
| Windows native app | — |
| iOS (keyboard, Action Button, Shortcuts) | — |
| Android (beta, bubble, work profiles) | — |
| Browser / web dictation | — |
| Linux / WSL / terminal (supported by Wispr) | **Native** CLI + compositor shortcuts |
| App Store / signed installers | curl / GitHub release / AUR / cargo build |
| Auto-update channel | Manual / package manager |

---

## Account, billing, teams

| Wispr Flow | dictate |
|------------|---------|
| Wispr account + login | No account (API keys in `~/.config/dictate/.env`) |
| Free / Pro / Team / Enterprise plans | No billing |
| Weekly word limits (free tier) | Limited only by provider quotas you pay for |
| Student / referral / discounts | — |
| Admin portal (users, billing) | — |
| SCIM user provisioning | — |
| SSO (SAML/OIDC-style enterprise) | — |
| Domain capture / org enrollment | — |
| Invoice download | — |
| Hosted Wispr transcription API (REST + API key) | Bring your own Mistral / Groq / local Whisper |

---

## UX & “always-on” UI

| Wispr Flow | dictate |
|------------|---------|
| **Flow Bar** (desktop) — persistent control surface | Signal/daemon + optional **recording pill** (`dictate-overlay`) |
| **Flow Bubble** (Android) | — |
| **Dictation bubble** snooze / hide | Overlay on/off via `ENABLE_OVERLAY` only |
| **Scratchpad** — save & edit notes in-app | stdout / pipe / clipboard only |
| **Flow keyboard** (iOS system keyboard) | — |
| **Hands-free** mode (docs) | Hold-to-talk via double keybind toggle only |
| In-app onboarding tour | `dictate setup`, `dictate config wizard`, `dictate doctor` |
| **Sync across devices** (cloud) | No cloud sync |
| **Privacy mode** vs cloud sync toggle | Local-first; keys and audio go to providers you choose |
| Transcript **history** + delete history | No built-in history store |
| Report / flag / support ticketing in app | GitHub issues |

---

## Intelligence & formatting

| Wispr Flow | dictate |
|------------|---------|
| Proprietary “polished writing” model (all apps) | **Partial:** `[polish]` Mistral chat on **smart_paste** only |
| **Flow Styles** (tone/format presets) | **Partial:** command mode keywords + developer modes |
| **Smart formatting & backtrack** (product feature) | **Partial:** local cleanup + inline “replace X with Y”; no full backtrack UX |
| **Context awareness** (screen/app-aware prompts) | **Partial:** `CONTEXT_EDITING` on stream segments (branch/planned); no app OCR |
| **Variable recognition** in IDEs (backticks for symbols) | **Partial:** `--dictation-mode code-symbols` (word-based, not IDE-integrated) |
| **File tagging** (spoken tags → file metadata) | — |
| Multi-language switching in product UI | `TRANSCRIPTION_LANGUAGE` / provider auto |
| Language-specific abbreviations teaching | Dictionary in `text.toml` only |
| Bulk import dictionary & snippets (admin UI) | Manual `text.toml` edit |
| Teaching vocabulary over time (ML personalization) | Static dictionary replacements |
| Command mode (AI edit selection) | **Partial:** `dictate --command` local rules + clipboard; no arbitrary LLM command UI |
| Realtime deltas in every app with formatting | **Partial:** realtime on **live_typing**; delta post-processing limited |
| Missing-first-words mitigation (product fixes) | VAD/trim in `audio_processing` only |

---

## Snippets, dictionary, shortcuts

| Wispr Flow | dictate |
|------------|---------|
| Snippets (GUI create/edit) | `text.toml` `[[snippets]]` |
| Personal dictionary (GUI) | `text.toml` `[dictionary]` |
| **Hotkey** per platform (rich set) | Compositor custom shortcuts + generated snippets |
| iOS Shortcuts integration | — |
| Supported/unsupported hotkey matrix (docs) | User-defined keys in `.env` |
| Outlook / IDE-specific guides | README + `dictate shortcuts` for Hyprland/Niri/GNOME/KDE/Sway |

---

## Audio & hardware

| Wispr Flow | dictate |
|------------|---------|
| External mic setup wizard | `dictate doctor` + PipeWire checks |
| Discreet mic / hardware guide | — |
| AirPods / iOS audio quirks (known issues docs) | — |
| Banking app detection (Android) | — |
| Remote desktop (Citrix, RDP, VDI) | — |

---

## Security & compliance (enterprise)

| Wispr Flow | dictate |
|------------|---------|
| Security & compliance FAQ (SOC2-style positioning) | GPL app; you own keys and data path |
| Enterprise sign-up flow | — |
| VPN / security tool blocking troubleshooting | — |

---

## What dictate has that Wispr Flow doesn’t emphasize

- **UNIX philosophy:** stdout, `--pipe-to`, shell pipelines
- **No mandatory GUI** for core dictation
- **Local Whisper** (`--features local`, offline STT)
- **Groq** provider option
- **Open config** (`.env` + `text.toml`, versionable)
- **Self-hosted** overlay IPC (Unix socket)
- **GPL source** — fork and patch
- **Developer modes:** git-commit, terminal flags, file-path compaction
- **Dual profiles:** `live_typing` vs `smart_paste` vs `batch_clip` in one binary
- **Signal-driven** daemon (`SIGUSR1`) for minimal WM integration

---

## Summary count (rough)

- **Not in dictate:** ~45+ distinct Wispr product areas (mobile, enterprise, cloud sync, scratchpad, styles UI, file tagging, hands-free product mode, history, accounts, etc.).
- **Partial overlap:** ~15 areas (polish, snippets, dictionary, IDE dictation, context, command mode, overlay vs bubble).
- **dictate-only strengths:** ~10 areas (CLI, local whisper, pipe-to, open config, signals, no subscription).

See [`features.md`](../features.md) for dictate roadmap.