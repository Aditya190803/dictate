# Suggested PR split (current working tree)

Work is intertwined in a few files (`config.rs`, `main.rs`, `streaming.rs`, `Cargo.toml`, `INSTALL.md`). Use **stacked branches** or `git add -p` on those files. Order matters: land **overlay removal** before polish/docs that assume no pill.

Default Ollama polish model: **`gemma-4`** (`OLLAMA_POLISH_MODEL` overrides).

---

## PR 1 — `chore: remove recording pill overlay`

**Goal:** Delete Wayland layer-shell pill; no `ENABLE_OVERLAY`, no `dictate-overlay`.

| Include |
|--------|
| Remove `src/overlay_ipc.rs`, `src/overlay_ui.rs`, `src/bin/dictate-overlay.rs` |
| `Cargo.toml` / `Cargo.lock` — drop `overlay` feature, gtk4-layer-shell, overlay bin |
| `src/lib.rs` — no `overlay_ipc` / `overlay_ui` |
| `src/streaming.rs`, `src/segment_output.rs`, `src/main.rs` — **only** overlay IPC calls / `Commands::Overlay` / spawn |
| `src/config.rs` — remove `enable_overlay`; keep `prune_retired_env_keys` (`ENABLE_OVERLAY`) |
| `src/config_cli.rs`, `src/setup_tui.rs` — overlay doctor/setup/commands |
| `Makefile` — `words-ui` only, single binary |
| `AGENTS.md`, `docs/wispr-flow-gap.md`, `INSTALL.md` (pill bullets only) |

**Verify:** `cargo build --release --features words-ui && cargo test`

---

## PR 2 — `feat: dictionary GUI (dictate words)`

**Goal:** Wispr-style vocabulary + misspelling rules in GTK; `words-ui` feature.

| Include |
|--------|
| `src/word_store.rs`, `src/words_ui.rs` (new) |
| `src/lib.rs` — `#[cfg(feature = "words-ui")] pub mod words_ui` |
| `src/main.rs` — `Commands::Words` |
| `Cargo.toml` — `words-ui = ["dep:gtk4"]` |
| `INSTALL.md` / `README.md` — `dictate words` |
| `src/config_cli.rs` — doctor “Words UI” line (if not in PR 1) |

**Verify:** `cargo build --release --features words-ui && dictate words` (manual)

---

## PR 3 — `feat: polish independent of STT (Mistral + Ollama)`

**Goal:** Segmented / smart_paste / command LLM polish works with **groq** or **local** STT; polish via Mistral key or Ollama (`gemma-4` default).

| Include |
|--------|
| `src/config.rs` — `PolishBackend`, `POLISH_PROVIDER`, `OLLAMA_*`, `polish_available()`, tests |
| `src/llm_polish.rs` — Mistral + Ollama `/api/chat` |
| `src/segment_output.rs` — `polish_available()` gate |
| `src/clip_pipeline.rs`, `src/command_mode.rs` — if only polish-related hunks |
| `src/main.rs` — remove “smart requires mistral” |
| `src/config_cli.rs` — wizard (no smart→mistral), doctor polish backend |
| `README.md`, `INSTALL.md` — polish / Ollama / `gemma-4` |

**Verify:** `cargo test`; manual: `TRANSCRIPTION_PROVIDER=local POLISH_PROVIDER=ollama` + `ollama pull gemma-4`

---

## PR 4+ — Remaining product work (separate PRs)

Split by feature so each PR stays reviewable:

| PR | Suggested title | Files (indicative) |
|----|-----------------|-------------------|
| 4 | `feat: transcript history` | `src/history.rs`, wiring in segment/streaming/main |
| 5 | `feat: scratchpad` | `src/scratchpad.rs`, CLI |
| 6 | `feat: polish styles` | `src/polish_styles.rs`, `text_processing` `[polish].style` |
| 7 | `feat: clipboard command intent` | `src/clipboard_intent.rs`, clip_pipeline, segment_output |
| 8 | `feat: autostart / setup / doctor` | `config_cli.rs` autostart, `setup_tui.rs` (non-overlay) |
| 9 | `docs: flow diagram & extend notes` | `docs/flow.html`, `extend.md` |
| 10 | `chore: remove features.md` | deleted `features.md` |

---

## Git workflow (example)

```bash
# From clean main
git checkout -b chore/remove-overlay
# stage PR1 files only → commit → push → open PR

git checkout main && git pull
git checkout -b feat/dictionary-gui
# cherry-pick or re-apply PR2 → PR

git checkout main
git checkout -b feat/polish-ollama
# PR3 on top of main after PR1+2 merged (or stack on feat/dictionary-gui)
```

**Pre-push (each PR):** `cargo test` and `cargo build --release --features words-ui`.

---

## This session defaults

- Ollama polish model: **`gemma-4`** (`src/llm_polish.rs`, doctor fallback in `config_cli.rs`, `INSTALL.md` pull hint).
- Env: `POLISH_PROVIDER=auto|mistral|ollama`, `OLLAMA_BASE_URL`, `OLLAMA_POLISH_MODEL=gemma-4`.