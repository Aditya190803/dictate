# Open-source Feature Roadmap

This project should stay open-source, local-first, and CLI-native. Ignore billing, team management, enterprise compliance, and hosted admin features from Wispr Flow. The most useful additions are the features that improve daily dictation quality without turning `dictate` into a closed SaaS app.

## Recommended build order

### 1. Local snippets

Add voice shortcuts that expand into reusable text.

Examples:

```text
"calendar link" -> "Book a time here: https://cal.com/adi"
"email signature" -> full email signature
"meeting intro" -> reusable meeting intro paragraph
```

Why first:

- High daily value.
- Easy to keep fully local.
- Does not require a new AI model.
- Fits the existing CLI pipeline well.

Possible config shape:

```toml
[[snippets]]
trigger = "calendar link"
text = "Book a time here: https://cal.com/adi"

[[snippets]]
trigger = "email signature"
text = "Best,\nAditya"
```

### 2. Personal dictionary / custom vocabulary

Let users define corrections for names, acronyms, project terms, and technical vocabulary.

Examples:

```toml
[dictionary]
"whisper flow" = "Wispr Flow"
"high per land" = "Hyprland"
"vox stroll" = "Voxtral"
"adi" = "Aditya"
```

Why second:

- Fixes one of the most annoying dictation problems.
- Especially valuable for developers, Linux users, and people with custom terminology.
- Can start as simple post-processing before becoming smarter later.

### 3. Text cleanup pipeline

Add optional post-processing after transcription and before output.

Useful cleanup options:

- Remove filler words.
- Fix spacing.
- Fix capitalization.
- Improve punctuation.
- Convert spoken lists into bullets or numbered lists.
- Clean repeated words.

Example input:

```text
um so first thing is install pipe wire second thing is set the key bind
```

Possible output:

```text
First:
- Install PipeWire.
- Set the keybind.
```

Why third:

- Makes output feel much more polished.
- Works with all transcription providers.
- Can be implemented as a configurable local pipeline.

### 4. Command mode

Add a mode where speech is treated as an instruction instead of inserted text.

Examples:

```text
make this more concise
turn this into bullet points
rewrite casually
fix grammar
summarize this paragraph
```

This likely needs clipboard or selection integration so `dictate` can transform existing text.

Why fourth:

- Big productivity feature.
- Similar to Wispr Flow Pro's command/editing mode.
- More complex than snippets or dictionary, so it should come after the processing pipeline exists.

### 5. Developer-focused dictation modes

Lean into this project's CLI/Linux/developer identity instead of copying commercial apps exactly.

Possible modes:

- Markdown mode.
- Git commit mode.
- Terminal command mode.
- Code symbol mode.
- File/path dictation helpers.
- Developer vocabulary presets.

Examples:

```text
cargo build release features local
```

Could become:

```bash
cargo build --release --features local
```

```text
open paren user id colon string close paren arrow result
```

Could become:

```text
(user_id: String) -> Result
```

## Suggested architecture

Add a text-processing pipeline between transcription and output:

```text
audio -> transcription -> dictionary -> snippets -> cleanup / commands -> stdout / clipboard / type
```

This keeps the core philosophy intact:

- No required GUI.
- No hosted account system.
- Works with stdout and shell pipelines.
- Local-first by default where possible.
- Compatible with existing Mistral, Groq, and local Whisper providers.

## Initial MVP scope

The first milestone should be small and practical:

1. Add a local config file section for snippets.
2. Expand exact snippet triggers after transcription.
3. Add a local dictionary replacement map.
4. Apply dictionary replacements before snippets.
5. Add tests for replacement order and edge cases.

Avoid starting with command mode or AI rewriting. Those are valuable, but snippets and dictionary create the foundation with much less complexity.
