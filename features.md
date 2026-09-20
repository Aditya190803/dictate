# Open-source Feature Roadmap

Open-source, local-first, CLI-native. Ignore billing, teams, enterprise compliance, and hosted admin from commercial dictation apps.

**Commercial comparison:** [Wispr Flow gap list](docs/wispr-flow-gap.md).

## Shipped on `main` (v1.0.5+)

| # | Feature | Config / CLI |
|---|---------|----------------|
| 1 | **Local snippets** | `text.toml` → `[[snippets]]` |
| 2 | **Personal dictionary** | `text.toml` → `[dictionary]` |
| 3 | **Text cleanup** | `text.toml` → `[cleanup]` |
| 4 | **Command mode** | `DICTATE_PROFILE=command`, or the same shortcut when clipboard already has text |
| 5 | **Developer dictation modes** | `--dictation-mode` (markdown, git, terminal, code symbols, paths) |
| — | **Default dictation** | One shortcut: speak, stop, one polished insert (corrections + formatting) |
| — | **LLM polish** | `text.toml` → `[polish]` (per utterance; also whole-clip on one-shot) |
| — | **Setup & health** | `dictate setup`, `dictate doctor`, `dictate config get|set|edit` |

**Pipeline (implemented):**

```text
audio → transcription → dictionary → inline fixes → snippets → cleanup
  → developer mode → [LLM polish on stop] → type / paste / clipboard / stdout
```

**Initial MVP** (snippets, dictionary, order, tests): done in `src/text_processing.rs`.

## Next (recommended order)

### Shipped (single default path)

- One dictation product: speak, stop; polish applies self-corrections and formatting to the whole take.
- One shortcut in `dictate setup`. Press the key or run `dictate`.
- `DICTATE_PROFILE=live_typing|smart_paste` are aliases of that product.

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