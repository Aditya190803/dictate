# Text Post-processing Foundation Design

## Scope

Implement the `features.md` initial MVP:

- Load local snippets from `text.toml`.
- Load a personal dictionary from `text.toml`.
- Apply dictionary replacements before snippet expansion.
- Run processing after transcription and before stdout or `--pipe-to` output.
- Add tests for ordering and matching edge cases.

Out of scope for this milestone:

- Text cleanup heuristics.
- Command/edit mode.
- Developer-focused modes.
- Mistral realtime delta post-processing.

## Configuration

`dictate` keeps `.env` for provider/runtime settings. Text transformations live beside it in `text.toml`.

Default path:

```text
~/.config/dictate/text.toml
```

For `--envfile /path/to/.env`, `dictate` reads `/path/to/text.toml`.

Example:

```toml
[dictionary]
"whisper flow" = "Wispr Flow"
"high per land" = "Hyprland"

[[snippets]]
trigger = "calendar link"
text = "Book a time here: https://cal.com/adi"

[[snippets]]
trigger = "email signature"
text = "Best,\nAditya"
```

If `text.toml` is missing, processing is a no-op.

## Architecture

Add `src/text_processing.rs`:

- `Snippet { trigger, text }`
- `TextProcessingConfig { dictionary, snippets }`
- `process_text(input, config) -> String`
- dictionary replacement helper
- snippet expansion helper

Extend `Config` with:

- `text_processing: TextProcessingConfig` (dictionary, snippets, cleanup loaded from `text.toml`)

`Config` loads `.env` first, then attempts to load `text.toml` from the `.env` directory.

## Behavior

Processing order:

```text
transcription -> dictionary -> snippets -> stdout / pipe-to
```

Dictionary behavior:

- Case-insensitive phrase matching.
- Replaces whole phrases without matching inside larger words.
- Preserves configured replacement casing.

Snippet behavior:

- Exact trigger match after trimming processed text.
- Trigger comparison is case-insensitive.
- Output becomes the configured snippet text.
- If no trigger matches, returns dictionary-processed text unchanged.

Realtime behavior:

- Mistral realtime delta output is unchanged in this milestone because text arrives in fragments.
- Buffered realtime final-segment processing should be handled in a later milestone.

## Integration points

Apply post-processing in:

- normal clip mode
- daemon clip mode
- VAD stream segment mode

Do not apply in:

- Mistral realtime delta streaming

## Testing

Add unit tests for:

- no-op empty config
- dictionary before snippets
- exact snippet trigger expansion
- case-insensitive dictionary replacement
- no partial-word dictionary replacement
- loading missing `text.toml` as no-op
- loading valid `text.toml`
