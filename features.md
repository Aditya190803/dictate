# Open-source Feature Roadmap

Open-source, local-first, CLI-native. Ignore billing, teams, enterprise compliance, and hosted admin from commercial dictation apps.

## Shipped on `main` (v1.0.5+)

| # | Feature | Config / CLI |
|---|---------|----------------|
| 1 | **Local snippets** | `text.toml` → `[[snippets]]` |
| 2 | **Personal dictionary** | `text.toml` → `[dictionary]` |
| 3 | **Text cleanup** | `text.toml` → `[cleanup]` |
| 4 | **Command mode** | `dictate --command` (clipboard + local transforms) |
| 5 | **Developer dictation modes** | `--dictation-mode` (markdown, git, terminal, code symbols, paths) |
| — | **Profiles** | `DICTATE_PROFILE`: `live_typing`, `smart_paste`, `batch_clip` |
| — | **Smart paste polish** | `text.toml` → `[polish]` (Mistral, smart_paste profile) |
| — | **Setup & health** | `dictate setup`, `dictate doctor`, `dictate config wizard` |

**Pipeline (implemented):**

```text
audio → transcription → dictionary → inline fixes → snippets → cleanup
  → developer mode → [LLM polish if smart_paste] → stdout / clipboard / type / paste
```

**Initial MVP** (snippets, dictionary, order, tests): done in `src/text_processing.rs`.

**CLI simplification:** profile drives `batch-mode` / `transcription-mode` in config wizard (PR #2).

## Next (recommended order)

### In progress (`feat/dual-mode-context`)

- **Live** shortcut (`--mode live`): realtime Mistral WebSocket, no context edits on deltas.
- **Smart** shortcut (`--mode smart`): smart paste daemon + LLM polish + `CONTEXT_EDITING` on VAD/stream segments.
- **`dictate setup`**: yes/no defaults, two bind lines via `print_dual_shortcuts`.

### A. Context-aware realtime editing (v1.1.0 candidate)

Spoken corrections while live typing: “scratch that”, delete last line/sentence, implicit rewrites (“actually by 11”).

- **Branch:** `context-aware-editing` (large refactor; not merged).
- **Plan:** see [`docs/context-aware-editing.md`](docs/context-aware-editing.md) — prefer porting intent + edit layer onto current `main` rather than merging the branch as-is.

### B. Command mode — LLM + selection

Today: keyword rules on clipboard text. Next: optional Mistral rewrite for arbitrary instructions; optional primary-selection / clipboard hooks.

### C. Developer vocabulary in `text.toml`

Built-in dev vocab exists in code; add user-defined presets in `text.toml` (same spirit as `[dictionary]`).

### D. Docs & release

- Keep `features.md` in sync when shipping A–C.
- Tag release after v1.1.0 scope is chosen (context editing vs patch-only).

## Principles (unchanged)

- No required GUI for core dictation.
- No hosted account system.
- stdout and shell pipelines first.
- Local-first where possible.
- Mistral, Groq, and local Whisper remain supported.