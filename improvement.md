# Context-Aware Realtime STT Improvement Plan

## Summary

Yes, this is possible, but it is no longer just speech-to-text. It becomes a realtime dictation editor: speech recognition plus an intent layer that understands correction commands and edits already-inserted text.

Examples:

- User says: `sorry remove that last line`  
  App should delete the last dictated line/sentence from the target app.

- User says: `I will leave by 10 30 actually by 11`  
  App should output: `I will leave by 11`

This should be planned as a larger release, likely **v1.1.0**, because it adds new product behavior, state management, command parsing, and editing primitives. A smaller **v1.0.5** could ship only low-risk realtime typing polish.

## Goals

1. Keep realtime typing fast and natural.
2. Maintain a rolling context of dictated text.
3. Detect spoken correction/editing intent.
4. Convert correction intent into safe text edits.
5. Apply edits in the focused app using keyboard operations.
6. Avoid surprising destructive edits.
7. Make behavior configurable for users who want plain STT only.

## Non-goals

- Full document editing across arbitrary apps with perfect state sync.
- Reconstructing text the user manually edited outside Dictate.
- Replacing a full text editor or IDE integration.
- Guaranteed perfect semantic correction for every natural-language phrase.

## Proposed Release Split

### v1.0.5: Realtime typing polish

Scope:

- Improve spacing and punctuation handling.
- Reduce duplicate partial text.
- Add configuration for typing separator behavior.
- Improve logs and diagnostics for ydotool/wtype.
- Keep current direct realtime transcription architecture.

This is a patch release if no new editing features are added.

### v1.1.0: Context-aware dictation editor

Scope:

- Rolling transcript buffer.
- Spoken edit command detection.
- Correction/rewrite engine.
- Text edit application layer.
- Configurable safety modes.
- Integration tests for common correction phrases.

This is a minor release because it changes the product from raw STT into an editing assistant.

## Architecture

### 1. Transcript Buffer

Add an in-memory buffer that records what Dictate has typed during the current session.

It should track:

- Raw STT deltas.
- Finalized segments.
- Text actually typed into the target app.
- Sentence/line boundaries.
- Cursor assumptions.
- Timestamps.

Example structure:

```rust
struct TranscriptBuffer {
    text: String,
    segments: Vec<TranscriptSegment>,
}

struct TranscriptSegment {
    id: SegmentId,
    text: String,
    started_at: Instant,
    finalized_at: Option<Instant>,
    typed_range: Range<usize>,
}
```

This buffer is the app's best-effort model of the text it inserted. It should not claim to know about text typed manually by the user.

### 2. Intent Detection Layer

Every finalized segment should pass through an intent detector before it is typed.

Possible intents:

```rust
enum DictationIntent {
    InsertText(String),
    ReplaceRecent { from: String, to: String },
    DeleteLastSentence,
    DeleteLastLine,
    DeleteLastWords(usize),
    RewriteRecent { instruction: String },
    CommandRejected { reason: String },
}
```

Detection can start with deterministic rules and later add an LLM option.

Rule examples:

- `sorry remove that last line` → `DeleteLastLine`
- `delete last sentence` → `DeleteLastSentence`
- `actually by 11` after `I will leave by 10 30` → replace the most recent time phrase.
- `no I mean X` → rewrite recent phrase.
- `scratch that` → delete last sentence or clause.

### 3. Correction Engine

The correction engine decides how an intent changes the transcript buffer.

For simple commands:

- Delete last sentence.
- Delete last line.
- Delete last N words.
- Replace exact recent phrase.

For semantic commands:

- Build a small context window from recent dictated text.
- Ask an LLM or local rules to return a structured edit.
- Validate the edit before applying it.

The structured edit should be explicit, not free-form:

```json
{
  "operation": "replace_range",
  "old_text": "I will leave by 10 30",
  "new_text": "I will leave by 11",
  "confidence": 0.91
}
```

Only apply edits above a confidence threshold, unless unsafe commands are explicitly enabled.

### 4. Edit Application Layer

Dictate currently types text using commands like:

```bash
ydotool type --file -
```

For editing, it also needs keyboard operations:

- Backspace N characters.
- Select/delete previous sentence.
- Select/delete previous line.
- Replace recent text by deleting characters and typing replacement.

The safest generic approach is character-based backspace from the transcript buffer:

1. Compute the suffix to remove.
2. Send N backspaces.
3. Type replacement text.
4. Update transcript buffer.

This assumes the cursor is still after the text Dictate typed. If the user moves the cursor, edits may be wrong. Dictate should detect or warn about this limitation.

Longer term, support app-specific integrations:

- Clipboard-based replacement.
- Text editor plugins.
- Accessibility APIs where available.
- Compositor/input-method protocols if practical.

### 5. Typing Backend Abstraction

Create a typing backend trait instead of directly calling command execution everywhere.

```rust
trait TypingBackend {
    async fn type_text(&self, text: &str) -> Result<()>;
    async fn backspace(&self, count: usize) -> Result<()>;
    async fn delete_last_line(&self) -> Result<()>;
}
```

Implementations:

- `YdotoolBackend`
- `CommandPipeBackend`
- Future: `WtypeBackend`
- Future: `ClipboardBackend`

This makes context-aware editing testable without requiring a live desktop.

### 6. Realtime Output Policy

Realtime STT often emits partial deltas that later get corrected. There are two possible modes:

#### Low-latency mode

Type deltas immediately.

Pros:

- Feels realtime.

Cons:

- Harder to correct partial hypotheses.
- May need backspaces when provider changes its mind.

#### Stable-segment mode

Only type finalized segments.

Pros:

- Cleaner output.
- Easier correction logic.

Cons:

- Slightly less realtime.

Recommended default for v1.1.0:

- Type stable chunks quickly, not every tiny unstable token.
- Keep latency target under 500–800ms.
- Allow `REALTIME_OUTPUT_MODE=delta|stable`.

### 7. Safety Modes

Add config:

```env
CONTEXT_EDITING=true
CONTEXT_EDITING_MODE=conservative
CONTEXT_EDITING_MIN_CONFIDENCE=0.80
CONTEXT_EDITING_CONFIRM_DESTRUCTIVE=false
REALTIME_OUTPUT_MODE=stable
```

Modes:

- `off`: plain STT only.
- `conservative`: only obvious commands like delete last sentence/line and exact replacements.
- `assistant`: allow semantic rewrites with LLM validation.

### 8. LLM/Provider Support

Semantic editing can use an LLM, but it must return structured JSON edits.

Provider options:

- Mistral chat model using existing Mistral API key.
- Groq chat model if configured.
- Local model later.

Prompt requirements:

- Input: recent transcript, new utterance, cursor assumption.
- Output: strict JSON edit operation.
- Never invent unrelated text.
- Reject ambiguous commands.

### 9. Testing Plan

Unit tests:

- Intent parsing rules.
- Transcript buffer updates.
- Replace/delete range calculations.
- Whitespace and punctuation normalization.
- Confidence threshold behavior.

Integration tests with mock typing backend:

- `I will leave by 10 30 actually by 11` → types final corrected sentence.
- `write hello world sorry remove that last line` → deletes line.
- `the meeting is at five no six` → replaces `five` with `six`.
- Manual cursor moved simulation → command rejected or warning.

Manual desktop tests:

- GNOME + ydotool.
- Hyprland + ydotool/wtype if available.
- Long dictation session.
- Rapid start/stop with Super+R.

## Implementation Phases

### Phase 1: Cleanup current realtime path

- Keep whitespace-preserving delta output.
- Add a reusable typing abstraction.
- Reduce noisy command logs in realtime mode.
- Add config for output mode.

### Phase 2: Transcript buffer

- Track all emitted text.
- Segment into sentences and lines.
- Add tests for buffer operations.

### Phase 3: Rule-based command detection

- Detect common correction commands.
- Support delete last line/sentence/word.
- Support simple `actually X` replacements for recent clauses.

### Phase 4: Edit backend

- Implement backspace/delete/type primitives for ydotool.
- Add mock backend tests.
- Apply edits to both target app and transcript buffer.

### Phase 5: Semantic rewrite assistant

- Add optional LLM structured edit provider.
- Add confidence thresholds and rejection paths.
- Add config flags and documentation.

### Phase 6: Release v1.1.0

- Update README and INSTALL docs.
- Add examples and troubleshooting.
- Tag release.
- Publish binary/package updates.

## Main Risks

1. **Cursor drift**: Dictate cannot always know if the user moved the cursor.
2. **Provider instability**: realtime deltas may change before finalization.
3. **Unsafe edits**: natural-language commands can be ambiguous.
4. **Desktop differences**: ydotool behavior may vary by compositor/session.
5. **Latency vs correctness**: instant typing conflicts with semantic correction.

## Recommendation

Ship the current realtime daemon and spacing fixes as **v1.0.4**.

Plan context-aware editing as **v1.1.0** because it introduces a new editing assistant layer, not just a bug fix.

If a smaller step is desired first, ship **v1.0.5** with:

- Better spacing/punctuation.
- Stable segment output mode.
- Typing backend abstraction.
- Transcript buffer only, without semantic commands.

Then ship full context-aware commands in **v1.1.0**.
