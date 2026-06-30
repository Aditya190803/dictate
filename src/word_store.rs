use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MAX_WORD_LEN: usize = 60;

pub fn word_char_len(word: &str) -> usize {
    word.chars().count()
}

fn word_too_long(word: &str) -> bool {
    word_char_len(word) > MAX_WORD_LEN
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WordBook {
    pub preferred_words: Vec<String>,
    pub dictionary: BTreeMap<String, String>,
    pub starred_words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryRow {
    pub word: String,
    /// What Dictate often hears instead of `word`.
    pub misspelling: Option<String>,
    pub starred: bool,
}

impl WordBook {
    pub fn add_preferred_word(&mut self, word: &str) {
        let word = word.trim();
        if word.is_empty() || word_too_long(word) {
            return;
        }
        if !self
            .preferred_words
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(word))
        {
            self.preferred_words.push(word.to_string());
            self.preferred_words.sort_by_key(|s| s.to_lowercase());
        }
    }

    pub fn remove_preferred_word(&mut self, word: &str) {
        let needle = word.trim();
        self.preferred_words
            .retain(|existing| !existing.eq_ignore_ascii_case(needle));
        self.starred_words
            .retain(|existing| !existing.eq_ignore_ascii_case(needle));
        self.dictionary
            .retain(|_, preferred| !preferred.eq_ignore_ascii_case(needle));
    }

    pub fn set_misspelling(&mut self, correct: &str, wrong: Option<&str>) {
        let correct = correct.trim();
        if correct.is_empty() {
            return;
        }
        self.dictionary
            .retain(|_, preferred| !preferred.eq_ignore_ascii_case(correct));
        if let Some(wrong) = wrong.map(str::trim).filter(|s| !s.is_empty()) {
            if !word_too_long(wrong) && !word_too_long(correct) {
                self.dictionary
                    .insert(wrong.to_string(), correct.to_string());
            }
        }
        self.add_preferred_word(correct);
    }

    pub fn add_alias(&mut self, heard: &str, preferred: &str) {
        self.set_misspelling(preferred, Some(heard));
    }

    pub fn remove_alias(&mut self, heard: &str) {
        self.dictionary.remove(heard.trim());
    }

    pub fn toggle_star(&mut self, word: &str) {
        let word = word.trim();
        if word.is_empty() {
            return;
        }
        if self
            .starred_words
            .iter()
            .any(|w| w.eq_ignore_ascii_case(word))
        {
            self.starred_words
                .retain(|w| !w.eq_ignore_ascii_case(word));
        } else {
            self.starred_words.push(word.to_string());
            self.starred_words.sort_by_key(|s| s.to_lowercase());
        }
    }

    pub fn rows(&self) -> Vec<DictionaryRow> {
        let mut out: Vec<DictionaryRow> = self
            .preferred_words
            .iter()
            .map(|word| DictionaryRow {
                word: word.clone(),
                misspelling: self.misspelling_for(word),
                starred: self
                    .starred_words
                    .iter()
                    .any(|s| s.eq_ignore_ascii_case(word)),
            })
            .collect();
        out.sort_by(|a, b| {
            b.starred
                .cmp(&a.starred)
                .then_with(|| a.word.to_lowercase().cmp(&b.word.to_lowercase()))
        });
        out
    }

    fn misspelling_for(&self, correct: &str) -> Option<String> {
        self.dictionary
            .iter()
            .find(|(_, preferred)| preferred.eq_ignore_ascii_case(correct))
            .map(|(heard, _)| heard.clone())
    }

    fn replace_word_casing(&mut self, from: &str, to: &str) {
        for existing in &mut self.preferred_words {
            if existing.eq_ignore_ascii_case(from) {
                *existing = to.to_string();
            }
        }
        for existing in &mut self.starred_words {
            if existing.eq_ignore_ascii_case(from) {
                *existing = to.to_string();
            }
        }
    }

    pub fn upsert_row(&mut self, word: &str, misspelling: Option<&str>, previous_word: Option<&str>) {
        let word = word.trim();
        if let Some(prev) = previous_word.map(str::trim).filter(|s| !s.is_empty()) {
            if !prev.eq_ignore_ascii_case(word) {
                self.remove_preferred_word(prev);
            } else if prev != word {
                self.replace_word_casing(prev, word);
            } else {
                self.set_misspelling(prev, None);
            }
        }
        self.add_preferred_word(word);
        self.set_misspelling(word, misspelling);
    }

    /// Returns `Err` message for duplicate vocabulary (case-insensitive), excluding `except_word`.
    pub fn validate_new_word(&self, word: &str, except_word: Option<&str>) -> Result<(), String> {
        let word = word.trim();
        if word.is_empty() {
            return Err("Enter a word or phrase.".into());
        }
        if word_too_long(word) {
            return Err(format!("Use at most {MAX_WORD_LEN} characters."));
        }
        let clashes = self.preferred_words.iter().any(|existing| {
            existing.eq_ignore_ascii_case(word)
                && except_word.is_none_or(|e| !existing.eq_ignore_ascii_case(e))
        });
        if clashes {
            return Err("That word is already in your dictionary.".into());
        }
        Ok(())
    }
}

pub fn default_text_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| std::env::var("HOME").map_or_else(|_| PathBuf::from("."), PathBuf::from))
        .join("dictate")
        .join("text.toml")
}

pub fn load_word_book(path: &Path) -> Result<WordBook> {
    if !path.exists() {
        return Ok(WordBook::default());
    }

    let contents = std::fs::read_to_string(path)?;
    if contents.trim().is_empty() {
        return Ok(WordBook::default());
    }

    let value: toml::Value = toml::from_str(&contents)?;
    let mut book = WordBook::default();

    if let Some(words) = value.get("preferred_words").and_then(|v| v.as_array()) {
        for word in words.iter().filter_map(|v| v.as_str()) {
            book.add_preferred_word(word);
        }
    }

    if let Some(starred) = value.get("starred_words").and_then(|v| v.as_array()) {
        for word in starred.iter().filter_map(|v| v.as_str()) {
            let word = word.trim();
            if word.is_empty() {
                continue;
            }
            if !book
                .starred_words
                .iter()
                .any(|w| w.eq_ignore_ascii_case(word))
            {
                book.starred_words.push(word.to_string());
            }
        }
        book.starred_words.sort_by_key(|s| s.to_lowercase());
    }

    if let Some(dictionary) = value.get("dictionary").and_then(|v| v.as_table()) {
        for (heard, preferred) in dictionary {
            if let Some(preferred) = preferred.as_str() {
                book.add_alias(heard, preferred);
            }
        }
    }

    Ok(book)
}

pub fn save_word_book(path: &Path, book: &WordBook) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut root = if path.exists() {
        let contents = std::fs::read_to_string(path)?;
        if contents.trim().is_empty() {
            toml::Value::Table(Default::default())
        } else {
            toml::from_str::<toml::Value>(&contents)?
        }
    } else {
        toml::Value::Table(Default::default())
    };

    let Some(table) = root.as_table_mut() else {
        return Err(anyhow!("{} must contain a TOML table", path.display()));
    };

    table.insert(
        "preferred_words".to_string(),
        toml::Value::Array(
            book.preferred_words
                .iter()
                .map(|word| toml::Value::String(word.clone()))
                .collect(),
        ),
    );

    if book.starred_words.is_empty() {
        table.remove("starred_words");
    } else {
        table.insert(
            "starred_words".to_string(),
            toml::Value::Array(
                book.starred_words
                    .iter()
                    .map(|word| toml::Value::String(word.clone()))
                    .collect(),
            ),
        );
    }

    let dictionary = book
        .dictionary
        .iter()
        .map(|(heard, preferred)| (heard.clone(), toml::Value::String(preferred.clone())))
        .collect();
    table.insert("dictionary".to_string(), toml::Value::Table(dictionary));

    let serialized = toml::to_string_pretty(&root)?;
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, serialized)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_words_without_losing_other_sections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.toml");
        std::fs::write(
            &path,
            r#"
[[snippets]]
trigger = "sig"
text = "Best"

[polish]
style = "concise"
"#,
        )
        .unwrap();

        let mut book = load_word_book(&path).unwrap();
        book.add_preferred_word("Supabase");
        book.add_alias("super base", "Supabase");
        save_word_book(&path, &book).unwrap();

        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("preferred_words"));
        assert!(saved.contains("super base"));
        assert!(saved.contains("[[snippets]]"));
        assert!(saved.contains("[polish]"));
    }

    #[test]
    fn one_misspelling_per_correct_word() {
        let mut book = WordBook::default();
        book.set_misspelling("Draft", Some("Draught"));
        book.set_misspelling("Draft", Some("Draf"));
        assert_eq!(book.dictionary.len(), 1);
        assert_eq!(
            book.dictionary.get("Draf"),
            Some(&"Draft".to_string())
        );
    }
}