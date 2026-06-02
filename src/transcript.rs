use std::ops::Range;
use std::time::Instant;

pub type SegmentId = u64;

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentKind {
    Inserted,
    Deleted,
    Command,
    RejectedCommand,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub id: SegmentId,
    pub text: String,
    pub kind: SegmentKind,
    pub typed_range: Range<usize>,
    pub started_at: Option<Instant>,
    pub finalized_at: Option<Instant>,
}

#[derive(Debug, Clone, Default)]
pub struct TranscriptBuffer {
    text: String,
    segments: Vec<TranscriptSegment>,
    next_id: SegmentId,
}

impl TranscriptBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    #[cfg(test)]
    pub fn segments(&self) -> &[TranscriptSegment] {
        &self.segments
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.segments.clear();
    }

    #[allow(dead_code)]
    pub fn recent_window(&self, max_chars: usize) -> &str {
        if self.text.chars().count() <= max_chars {
            return &self.text;
        }
        let start = self
            .text
            .char_indices()
            .rev()
            .nth(max_chars.saturating_sub(1))
            .map_or(0, |(idx, _)| idx);
        &self.text[start..]
    }

    pub fn append_typed(&mut self, text: &str) -> Range<usize> {
        let range = self.append_text(text);
        self.push_segment(text, SegmentKind::Inserted, range.clone());
        range
    }

    #[allow(dead_code)]
    pub fn record_command(&mut self, text: &str) {
        self.push_segment(text, SegmentKind::Command, self.text.len()..self.text.len());
    }

    pub fn record_rejected_command(&mut self, text: &str) {
        self.push_segment(
            text,
            SegmentKind::RejectedCommand,
            self.text.len()..self.text.len(),
        );
    }

    pub fn delete_range(&mut self, range: Range<usize>) {
        let deleted = self.text[range.clone()].to_string();
        self.text.replace_range(range.clone(), "");
        self.push_segment(&deleted, SegmentKind::Deleted, range);
    }

    pub fn replace_range(&mut self, range: Range<usize>, replacement: &str) -> Range<usize> {
        let old = self.text[range.clone()].to_string();
        self.text.replace_range(range.clone(), replacement);
        self.push_segment(&old, SegmentKind::Deleted, range.clone());
        let new_range = range.start..range.start + replacement.len();
        self.push_segment(replacement, SegmentKind::Inserted, new_range.clone());
        new_range
    }

    #[cfg(test)]
    pub fn last_word_range(&self) -> Option<Range<usize>> {
        self.last_words_range(1)
    }

    pub fn last_words_range(&self, count: usize) -> Option<Range<usize>> {
        if count == 0 || self.text.trim_end().is_empty() {
            return None;
        }

        let end = trim_end_index(&self.text);
        let prefix = &self.text[..end];
        let mut ranges = Vec::new();
        let mut in_word = false;
        let mut word_end = end;

        for (idx, ch) in prefix.char_indices().rev() {
            if ch.is_whitespace() {
                if in_word {
                    ranges.push(idx + ch.len_utf8()..word_end);
                    in_word = false;
                    if ranges.len() == count {
                        break;
                    }
                }
            } else if !in_word {
                in_word = true;
                word_end = idx + ch.len_utf8();
            }
        }

        if in_word && ranges.len() < count {
            ranges.push(0..word_end);
        }

        if ranges.len() < count {
            return None;
        }

        let start = ranges.last().unwrap().start;
        Some(start..end)
    }

    pub fn last_line_range(&self) -> Option<Range<usize>> {
        let end = trim_end_index(&self.text);
        if end == 0 {
            return None;
        }
        let start = self.text[..end].rfind('\n').map_or(0, |idx| idx + 1);
        Some(start..end)
    }

    pub fn last_sentence_range(&self) -> Option<Range<usize>> {
        let end = trim_end_index(&self.text);
        if end == 0 {
            return None;
        }
        let prefix = &self.text[..end];
        let mut boundary = 0;
        for (idx, ch) in prefix.char_indices().rev() {
            if matches!(ch, '.' | '!' | '?' | '\n') && idx + ch.len_utf8() < end {
                boundary = idx + ch.len_utf8();
                break;
            }
        }
        let start = self.text[boundary..end]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
            .map_or(boundary, |(idx, _)| boundary + idx);
        Some(start..end)
    }

    pub fn last_exact_suffix_match(&self, needle: &str) -> Option<Range<usize>> {
        let end = trim_end_index(&self.text);
        let haystack = &self.text[..end];
        let needle = needle.trim();
        if needle.is_empty() {
            return None;
        }
        let lower_haystack = haystack.to_lowercase();
        let lower_needle = needle.to_lowercase();
        let start = lower_haystack.rfind(&lower_needle)?;
        let range = start..start + lower_needle.len();
        if range.end == end {
            Some(range)
        } else {
            None
        }
    }

    fn append_text(&mut self, text: &str) -> Range<usize> {
        let start = self.text.len();
        self.text.push_str(text);
        start..self.text.len()
    }

    fn push_segment(&mut self, text: &str, kind: SegmentKind, typed_range: Range<usize>) {
        let id = self.next_id;
        self.next_id += 1;
        self.segments.push(TranscriptSegment {
            id,
            text: text.to_string(),
            kind,
            typed_range,
            started_at: None,
            finalized_at: Some(Instant::now()),
        });
    }
}

fn trim_end_index(text: &str) -> usize {
    text.trim_end_matches(char::is_whitespace).len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_last_word() {
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("hello world  ");
        assert_eq!(buffer.last_word_range(), Some(6..11));
    }

    #[test]
    fn finds_last_sentence() {
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("Hello world. This is wrong.  ");
        let range = buffer.last_sentence_range().unwrap();
        assert_eq!(&buffer.text()[range], "This is wrong.");
    }

    #[test]
    fn finds_last_line() {
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("one\ntwo three\n");
        let range = buffer.last_line_range().unwrap();
        assert_eq!(&buffer.text()[range], "two three");
    }

    #[test]
    fn tracks_segments() {
        let mut buffer = TranscriptBuffer::new();
        buffer.append_typed("hello");
        buffer.delete_range(0..5);
        assert_eq!(buffer.segments().len(), 2);
        assert_eq!(buffer.segments()[1].kind, SegmentKind::Deleted);
    }
}
