# Context-aware realtime editing

## Status

**Parked** — implementation lives on branch `context-aware-editing` (tip ~v1.1.0). **Not merged** into `main` as of 2026-06.

## What the branch adds

- Rolling **transcript buffer** of what was typed in-session.
- **Intent detection** (`src/intent.rs`): e.g. delete last sentence/line, “scratch that”, reset context, implicit replacements.
- **Edit planner** (`src/editing.rs`) → keyboard/backspace operations via typing backend.
- Realtime streaming integration, daemon lock, toggle/stop, output mode from config.

## Why not merge the branch wholesale

`git diff main..context-aware-editing` removes or replaces large parts of current `main`:

- `setup_tui`, `config_cli` (partial), `developer_modes`, `text_processing`, `llm_polish`
- Different `main.rs` / config architecture (~5k line churn)

Merging would regress: guided setup, smart paste polish, and the shipped text pipeline unless carefully reconciled.

## Recommended path

1. **Spike on `main`:** add minimal `TranscriptBuffer` + rule-based intents for 2–3 phrases (“scratch that”, “delete last line”).
2. **Wire only into** `streaming` / realtime typing path behind a config flag (e.g. `CONTEXT_EDITING=true`).
3. **Cherry-pick** modules from `context-aware-editing` as needed (`intent.rs`, `editing.rs`, `transcript.rs`, `typing.rs`) — adapt to existing config and `text_processing`, do not delete polish/setup.
4. **Delete or archive** the long-lived branch after parity, or keep it read-only for reference.

## Original design notes

Detailed v1.0.5 vs v1.1.0 split and architecture bullets were in repo file `improvement.md` (removed in commit `f0e5f35`). Same themes as commit message on `72436f3` (“Plan context-aware dictation editing”).

## Decision log

| Date | Decision |
|------|----------|
| 2026-06-24 | Document branch; **do not** merge `context-aware-editing` until ported incrementally on `main`. |