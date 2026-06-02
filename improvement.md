# Dictate v1.1.0 Context-Aware Dictation Editor Plan

## Executive Summary

Dictate v1.1.0 should turn realtime speech-to-text into a conservative, context-aware dictation editor. The goal is not to build a full text editor or perfectly sync with every application. The goal is to let users naturally correct recent dictated text with spoken commands while keeping ordinary dictation fast, predictable, and safe.

Examples:

- User dictates: `I will leave by 10 30 actually by 11`  
  Desired output: `I will leave by 11`

- User dictates a sentence, then says: `scratch that`  
  Desired behavior: delete the last dictated sentence.

- User says: `sorry remove that last line`  
  Desired behavior: delete the last line Dictate inserted.

This release should be treated as a **minor release** because it changes Dictate from raw STT output into a dictation assistant with state, intent detection, editing primitives, and safety policy.

---

## Current Foundation Already Implemented

The project now has the beginning of the v1.1.0 foundation:

- `TypingBackend` abstraction in `src/typing.rs`.
- `OutputBackend` for current stdout/pipe behavior.
- `MockTypingBackend` for tests.
- `TranscriptBuffer` in `src/transcript.rs`.
- Conservative intent detection in `src/intent.rs`.
- Basic edit planning/application in `src/editing.rs`.
- Config fields:
  - `REALTIME_OUTPUT_MODE=delta|stable`
  - `CONTEXT_EDITING=false|true`
  - `CONTEXT_EDITING_MODE=conservative`
- Realtime stable segments can route through context editing.
- VAD/batch stream segments can route through context editing.
- Current supported spoken commands:
  - `scratch that`
  - `forget that`
  - `delete last sentence`
  - `remove last sentence`
  - `delete last line`
  - `remove last line`
  - `delete last word`
  - `delete last two words`
  - `delete last three words`

This is a good foundation, but it is not yet a complete v1.1.0 context-aware release.

---

## v1.1.0 Product Goal

v1.1.0 should support **safe correction of recently dictated text** while preserving Dictate's core strengths:

1. Fast realtime dictation.
2. Predictable output.
3. Low surprise.
4. Minimal desktop assumptions.
5. Easy fallback to plain STT.
6. Clear limits when cursor/application state is unknown.

The release should default to safe behavior and require explicit opt-in for context-aware editing.

Recommended default:

```env
REALTIME_OUTPUT_MODE=delta
CONTEXT_EDITING=false
CONTEXT_EDITING_MODE=conservative
```

Recommended context-aware setup:

```env
REALTIME_OUTPUT_MODE=stable
CONTEXT_EDITING=true
CONTEXT_EDITING_MODE=conservative
```

---

## Non-Goals for v1.1.0

v1.1.0 should not attempt:

- Full document understanding across arbitrary apps.
- Perfect sync with text manually edited by the user.
- Arbitrary middle-of-document edits.
- Large destructive edits like `delete everything`.
- Always-on LLM rewriting by default.
- Application-specific editor plugins.
- Accessibility-tree integration.
- Clipboard-based range replacement unless the simple backend is already solid.

Those can come later.

---

## Release Scope

## Must Ship in v1.1.0

### 1. Reliable stable output mode

`REALTIME_OUTPUT_MODE=stable` must be reliable enough to be the recommended mode for context-aware editing.

Requirements:

- Do not type unstable realtime deltas in stable mode.
- Type finalized segments only once.
- Preserve natural spacing between finalized segments.
- Avoid jamming words together.
- Avoid double spaces around punctuation.
- Keep perceived latency acceptable, ideally under 500–800ms after the provider finalizes text.

### 2. Stronger transcript buffer

The transcript buffer must become the session source of truth for text Dictate inserted.

It should track:

- Full emitted text.
- Segment boundaries.
- Whether a segment was inserted, deleted, or rejected.
- Timestamps.
- Finalized STT segment text.
- Typed range for each segment.

Target structure:

```rust
pub struct TranscriptBuffer {
    text: String,
    segments: Vec<TranscriptSegment>,
}

pub struct TranscriptSegment {
    id: SegmentId,
    text: String,
    kind: SegmentKind,
    typed_range: Range<usize>,
    started_at: Option<Instant>,
    finalized_at: Option<Instant>,
}

pub enum SegmentKind {
    Inserted,
    Deleted,
    Command,
    RejectedCommand,
}
```

The buffer must explicitly document that it only knows what Dictate typed during the current session.

### 3. Conservative spoken command detection

Rule-based commands should be robust before any semantic/LLM work.

Supported command groups:

#### Delete last sentence

Examples:

- `scratch that`
- `forget that`
- `delete that`
- `remove that`
- `delete last sentence`
- `remove last sentence`
- `remove that sentence`

Behavior:

- Delete only the most recent dictated sentence.
- Reject if no sentence exists.
- Do not delete more than configured safety limit.

#### Delete last line

Examples:

- `delete last line`
- `remove last line`
- `sorry remove that last line`
- `scratch that line`

Behavior:

- Delete only the most recent dictated line.
- Reject if no line exists.

#### Delete last words

Examples:

- `delete last word`
- `remove last word`
- `delete last two words`
- `delete last three words`
- `delete last 5 words`

Behavior:

- Support numeric words and digits.
- Enforce a maximum, for example 10 words.
- Reject unsupported or excessive counts.

#### Simple replacement commands

Examples:

- `actually eleven`
- `actually by eleven`
- `no six`
- `no I mean six`
- `I mean Friday`
- `change five to six`
- `replace five with six`

Behavior:

- Prefer exact recent phrase replacement when `from` is explicit.
- For implicit corrections like `actually by eleven`, only target the most recent clause or phrase.
- Reject if no safe target can be found.
- Do not perform broad semantic rewrites in conservative mode.

### 4. Structured edit model

All correction behavior must pass through an explicit edit plan.

Target model:

```rust
pub enum TextEdit {
    Insert {
        text: String,
    },
    DeleteSuffix {
        range: Range<usize>,
    },
    ReplaceSuffix {
        range: Range<usize>,
        replacement: String,
    },
    Reject {
        reason: String,
    },
}
```

v1.1.0 should prefer suffix-only edits because they are safest with generic keyboard backspacing.

A planned edit must be validated before applying.

Validation rules:

- Edit range must exist in the transcript buffer.
- Edit target must be at the current suffix, unless a future backend supports selection/clipboard replacement safely.
- Delete length must be within `CONTEXT_EDITING_MAX_DELETE_CHARS`.
- Replacement text must be non-empty for replacement operations.
- Rejected commands must not be typed into the target app unless configured otherwise.

### 5. Safe edit application

The edit applier should update the target app and transcript buffer consistently.

For suffix deletion:

1. Compute character count, not byte count.
2. Send backspace count through `TypingBackend`.
3. Update transcript buffer only after backend succeeds.

For suffix replacement:

1. Compute range.
2. Backspace target range.
3. Type replacement text.
4. Update transcript buffer only after both backend operations succeed.

Important Unicode requirement:

- Use character counts for backspaces.
- Tests must include Unicode words and punctuation.

### 6. Backend abstraction hardening

`TypingBackend` should be expanded enough for v1.1.0:

```rust
#[async_trait]
pub trait TypingBackend: Send + Sync {
    async fn type_text(&self, text: &str) -> Result<()>;
    async fn backspace(&self, count: usize) -> Result<()>;
}
```

Optional helper:

```rust
async fn replace_suffix(&self, delete_chars: usize, replacement: &str) -> Result<()> {
    self.backspace(delete_chars).await?;
    self.type_text(replacement).await
}
```

Backends:

- `OutputBackend`: current stdout/pipe behavior.
- `MockTypingBackend`: test-only backend.
- Future but not required for v1.1.0:
  - `YdotoolBackend`
  - `WtypeBackend`
  - `ClipboardBackend`

### 7. Configuration and safety controls

Add these config values:

```env
CONTEXT_EDITING=false
CONTEXT_EDITING_MODE=conservative
CONTEXT_EDITING_MAX_DELETE_CHARS=300
CONTEXT_EDITING_MAX_DELETE_WORDS=10
CONTEXT_EDITING_TYPE_REJECTED_COMMANDS=false
REALTIME_OUTPUT_MODE=delta
```

Meanings:

- `CONTEXT_EDITING=false`: plain STT behavior.
- `CONTEXT_EDITING_MODE=conservative`: rule-based edits only.
- `CONTEXT_EDITING_MAX_DELETE_CHARS`: protects against large accidental deletions.
- `CONTEXT_EDITING_MAX_DELETE_WORDS`: protects word-deletion commands.
- `CONTEXT_EDITING_TYPE_REJECTED_COMMANDS=false`: rejected commands are not inserted by default.
- `REALTIME_OUTPUT_MODE=stable`: recommended when context editing is enabled.

Validation:

- Reject unsupported modes.
- Reject zero max delete chars/words.
- Warn if `CONTEXT_EDITING=true` and `REALTIME_OUTPUT_MODE=delta` because command words may already be typed.

### 8. User-facing diagnostics

Context editing must be explainable.

Log examples:

```text
🧠 Intent: DeleteLastSentence
✂️  Deleted last sentence: 24 chars
⚠️  Context edit rejected: no previous sentence to delete
⚠️  Context editing works best with REALTIME_OUTPUT_MODE=stable
```

Logs should not expose API keys or sensitive config.

### 9. Documentation

Update:

- `README.md`
- `INSTALL.md`
- `.env.example`
- shortcut docs if relevant

Docs must explain:

- What context editing does.
- How to enable it.
- Why stable mode is recommended.
- Supported commands.
- Safety limitations.
- Cursor drift limitation.
- How to disable context editing.

---

## Should Ship if Time Allows

### 1. Simple implicit replacement

Support corrections like:

```text
the meeting is at five no six
```

Expected:

```text
the meeting is at six
```

Conservative implementation:

- Detect `no <replacement>` only when the previous text ends with a short replaceable phrase.
- Replace the last word or short phrase.
- Reject if replacement target is unclear.

### 2. Explicit phrase replacement

Support:

```text
replace Tuesday with Thursday
change five PM to six PM
```

Rules:

- Search only recent transcript window.
- Prefer the last exact match.
- Only apply if match is within suffix-editable region.
- Reject if multiple ambiguous matches exist outside the suffix.

### 3. Session reset command

Support:

```text
reset dictate context
clear dictate context
```

Behavior:

- Clear transcript buffer.
- Do not edit target app.
- Useful after user manually edits or moves cursor.

---

## Explicitly Defer Past v1.1.0

### LLM semantic rewrite assistant

Defer to v1.2.0 or later unless the rule-based version is fully stable.

Potential future mode:

```env
CONTEXT_EDITING_MODE=assistant
CONTEXT_EDITING_MIN_CONFIDENCE=0.80
```

Future examples:

- `make that more formal`
- `rewrite the last sentence to sound friendlier`
- `change that to say I am unavailable tomorrow`

If added later, the LLM must return structured JSON only:

```json
{
  "operation": "replace_suffix",
  "old_text": "I can come tomorrow",
  "new_text": "I am unavailable tomorrow",
  "confidence": 0.91
}
```

The app should reject invalid JSON, low confidence, unrelated text, or non-suffix edits.

### App-specific integrations

Defer:

- editor plugins
- browser extensions
- accessibility tree integrations
- compositor-specific selection APIs
- clipboard-based arbitrary range replacement

---

## Detailed Implementation Plan

## Phase 1: Harden stable output mode

### Tasks

- Audit Mistral realtime event handling.
- Confirm which events are final and which are partial.
- Ensure `REALTIME_OUTPUT_MODE=stable` only emits final segments.
- Add spacing normalization between stable segments.
- Add tests for stable segment joining.

### Acceptance Criteria

- Stable mode does not emit deltas.
- Stable mode does not duplicate final text.
- Consecutive segments produce natural spacing.
- Delta mode remains unchanged for users who want maximum immediacy.

---

## Phase 2: Upgrade transcript buffer

### Tasks

- Add `TranscriptSegment` metadata.
- Track inserted and deleted ranges.
- Add helper methods:
  - `append_segment`
  - `last_sentence_range`
  - `last_line_range`
  - `last_words_range`
  - `recent_window`
  - `clear`
- Add Unicode-aware range tests.

### Acceptance Criteria

- Buffer correctly tracks text after insertions and deletions.
- Sentence, line, and word detection work with punctuation and whitespace.
- Unicode words do not break range calculations.

---

## Phase 3: Expand intent detection

### Tasks

- Replace simple substring matching with command-pattern rules.
- Normalize filler words like `sorry`, `please`, `um`, `uh`.
- Add number parsing for word counts:
  - `one`, `two`, `three`, etc.
  - digits like `5`
- Add explicit replacement intents:
  - `replace X with Y`
  - `change X to Y`
- Add implicit replacement intents:
  - `actually X`
  - `no I mean X`
  - `no X`
- Add command rejection reasons.

Target intent model:

```rust
pub enum DictationIntent {
    InsertText(String),
    DeleteLastSentence,
    DeleteLastLine,
    DeleteLastWords(usize),
    ReplaceRecentExact { from: String, to: String },
    ReplaceRecentImplicit { replacement: String },
    ResetContext,
    CommandRejected { reason: String },
}
```

### Acceptance Criteria

- Common correction phrases are detected.
- Ordinary dictation is not misclassified.
- Ambiguous commands are rejected safely.
- Unit tests cover all supported command phrases.

---

## Phase 4: Implement replacement planning

### Tasks

- Add `ReplaceSuffix` edit variant.
- Implement exact recent replacement.
- Implement last-word implicit replacement.
- Implement short recent-phrase implicit replacement only when safe.
- Enforce max edit size.
- Reject non-suffix matches.

### Acceptance Criteria

- `replace five with six` works when `five` is recent and suffix-editable.
- `change Tuesday to Thursday` works for recent suffix text.
- `actually by eleven` can replace a recent matching phrase when safely identifiable.
- Ambiguous replacements are rejected.

---

## Phase 5: Strengthen edit application

### Tasks

- Apply replacement as backspace + type.
- Update transcript only after backend success.
- Add operation-level error handling.
- Add logs for applied/rejected edits.
- Add tests with `MockTypingBackend`.

### Acceptance Criteria

- Failed backend operations do not corrupt transcript state.
- Backspace count is character-based.
- Replacement operation order is correct.
- Rejections do not type command text by default.

---

## Phase 6: Add safety config

### Tasks

- Add config fields:
  - `context_editing_max_delete_chars`
  - `context_editing_max_delete_words`
  - `context_editing_type_rejected_commands`
- Add env parsing.
- Add validation.
- Add README and `.env.example` docs.
- Print warning for context editing + delta mode.

### Acceptance Criteria

- Invalid safety config fails validation.
- Defaults are conservative.
- Users can fully disable context editing.
- Docs include clear examples.

---

## Phase 7: Integration tests

### Required Tests

Use `MockTypingBackend` and direct calls to intent/edit modules.

#### Delete sentence

Input buffer:

```text
Hello world. This is wrong.
```

Command:

```text
scratch that
```

Expected buffer:

```text
Hello world. 
```

Expected backend:

```text
Backspace(14)
```

#### Delete line

Input buffer:

```text
first line
second line
```

Command:

```text
remove last line
```

Expected:

```text
first line
```

#### Delete words

Input buffer:

```text
one two three four
```

Command:

```text
delete last two words
```

Expected:

```text
one two 
```

#### Exact replacement

Input buffer:

```text
meeting at five
```

Command:

```text
replace five with six
```

Expected:

```text
meeting at six
```

#### Rejected ambiguous command

Input buffer:

```text
five plus five
```

Command:

```text
replace five with six
```

Expected:

- reject if not suffix-safe or if ambiguity policy requires rejection.

#### Unicode backspace

Input buffer:

```text
café tomorrow
```

Command:

```text
delete last word
```

Expected:

- backspace count equals character count of `tomorrow`.

---

## Phase 8: Manual desktop validation

Test with:

- terminal stdout mode
- `ydotool type --file -`
- browser text fields
- chat apps
- plain text editor
- GNOME session
- Hyprland session if available

Manual scenarios:

1. Plain realtime dictation in delta mode.
2. Stable mode normal dictation.
3. Stable mode with `scratch that`.
4. Stable mode with `remove last line`.
5. Stable mode with `delete last two words`.
6. User moves cursor, then speaks correction command.
7. User manually edits text, then speaks correction command.
8. Long dictation session with many commands.
9. Rapid start/stop daemon usage.

Expected limitation:

- Cursor movement cannot be fully detected with generic typing backends. Docs must tell users to run `reset dictate context` or restart dictation after manual edits/cursor movement.

---

## Risk Register

## 1. Cursor drift

Risk:

- User moves cursor after Dictate types text. Backspace edits may affect the wrong location.

Mitigation:

- Only suffix edits.
- Conservative docs.
- Optional context reset command.
- Future cursor-aware integrations.

## 2. Realtime delta instability

Risk:

- Provider emits partial text that changes later.

Mitigation:

- Recommend stable mode for context editing.
- Warn if context editing is enabled in delta mode.

## 3. Accidental destructive commands

Risk:

- Normal speech misclassified as edit command.

Mitigation:

- Conservative command grammar.
- Max delete limits.
- Rejection over guessing.
- Context editing off by default.

## 4. Whitespace and punctuation edge cases

Risk:

- Deletes leave awkward spacing.

Mitigation:

- Normalize suffix ranges carefully.
- Add punctuation/spacing tests.

## 5. Desktop backend differences

Risk:

- Backspace or pipe behavior differs across environments.

Mitigation:

- Keep backend interface small.
- Test with mock backend and real desktop flows.
- Document supported setups.

---

## v1.1.0 Release Checklist

Before tagging v1.1.0:

- [ ] `REALTIME_OUTPUT_MODE=stable` is reliable.
- [ ] Transcript buffer tracks segments and edits.
- [ ] Delete sentence/line/word commands work.
- [ ] Exact recent replacements work.
- [ ] Implicit replacements are safe or deferred.
- [ ] Safety config is implemented and documented.
- [ ] Rejected commands are not typed by default.
- [ ] Unicode backspace tests pass.
- [ ] Integration tests pass with `MockTypingBackend`.
- [ ] Manual desktop tests completed.
- [ ] README updated.
- [ ] INSTALL updated if setup/config changed.
- [ ] `.env.example` updated.
- [ ] Release notes clearly mention limitations.

---

## Recommended v1.1.0 Definition of Done

v1.1.0 is ready when a user can enable:

```env
REALTIME_OUTPUT_MODE=stable
CONTEXT_EDITING=true
CONTEXT_EDITING_MODE=conservative
```

and reliably use:

```text
scratch that
delete last sentence
remove last line
delete last two words
replace five with six
change Monday to Tuesday
```

without Dictate unexpectedly editing old text or making large destructive changes.

The LLM-powered assistant should remain out of scope until the conservative editing layer is stable and well-tested.
