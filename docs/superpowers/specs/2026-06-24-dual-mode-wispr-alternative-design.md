# Dual-mode dictate (Wispr Flow–style open alternative)

## Goal

Open-source, local-first dictation: any STT provider (Mistral, Groq, local Whisper), starting on Linux/Wayland with a path to other OSes later.

## Two user-facing modes (two shortcuts)

| Mode | Shortcut env | Behavior | Context editing |
|------|----------------|----------|-----------------|
| **Live** | `SHORTCUT_KEY_LIVE` (default `SUPER,R`) | Mistral realtime WebSocket, words as you speak | Off (raw deltas) |
| **Smart** | `SHORTCUT_KEY_SMART` (default `SUPER,SHIFT,R`) | Record → stop → transcribe → dictionary/snippets/cleanup → LLM polish → paste once | Session buffer for voice corrections on finalized segments (VAD/stream); polish for formatting |

## Setup

- `dictate setup`: minimal prompts, **Confirm** yes/no with sensible defaults.
- Writes API key, `DICTATE_MODE` default live, both shortcut keys, desktop, `SHORTCUT_OUTPUT`, beeps on.
- Prints **two** compositor bind lines.

## Implementation base

**`main`**, not merging `context-aware-editing` wholesale. Port `intent`, `transcript`, `editing`, `typing` modules and wire:

- Live realtime: unchanged delta typing.
- Smart: `smart_paste` profile + existing polish.
- Optional `CONTEXT_EDITING=true`: stable segment + `handle_final_text` on VAD `process_segment` (any provider).

## Long-term (not this PR)

macOS/Windows backends, arbitrary STT plugin API, LLM command mode, full Wispr parity.