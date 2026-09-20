//! `dictate words` without the GTK UI: list and add often-spoken names.

use crate::config_cli;
use anyhow::Result;
use dictate::word_store::{load_word_book, save_word_book, WordBook};
use inquire::Text;
use std::io::IsTerminal;
use std::path::Path;

pub fn run(path: &Path, add: &[String]) -> Result<()> {
    if let Some(parent) = path.parent() {
        config_cli::ensure_private_dir(parent)?;
    }

    let mut book = load_word_book(path)?;
    let mut changed = false;

    for word in add {
        match book.validate_new_word(word, None) {
            Ok(()) => {
                book.add_preferred_word(word);
                changed = true;
            }
            Err(msg) => eprintln!("{msg}"),
        }
    }

    if !add.is_empty() {
        if changed {
            save_and_lock(path, &book)?;
        }
        print_words(&book);
        return Ok(());
    }

    print_words(&book);
    if !std::io::stdin().is_terminal() {
        if book.preferred_words.is_empty() {
            eprintln!("Add a word: dictate words Hyprland");
        }
        return Ok(());
    }

    loop {
        let Some(word) = Text::new("Add a word (empty to finish)").prompt_skippable()? else {
            break;
        };
        let word = word.trim().to_string();
        if word.is_empty() {
            break;
        }
        match book.validate_new_word(&word, None) {
            Ok(()) => {
                book.add_preferred_word(&word);
                changed = true;
            }
            Err(msg) => eprintln!("{msg}"),
        }
    }

    if changed {
        save_and_lock(path, &book)?;
    }
    Ok(())
}

fn save_and_lock(path: &Path, book: &WordBook) -> Result<()> {
    save_word_book(path, book)?;
    config_cli::restrict_perms(path);
    Ok(())
}

fn print_words(book: &WordBook) {
    if book.preferred_words.is_empty() {
        eprintln!("Dictionary is empty.");
        return;
    }
    eprintln!("Dictionary:");
    for row in book.rows() {
        match &row.misspelling {
            Some(heard) => eprintln!("  {}  (heard as {heard})", row.word),
            None => eprintln!("  {}", row.word),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_words_to_text_toml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.toml");
        run(&path, &["Hyprland".into(), "Supabase".into()]).unwrap();
        let book = load_word_book(&path).unwrap();
        assert_eq!(book.preferred_words, vec!["Hyprland", "Supabase"]);
    }

    #[test]
    fn skips_duplicates_without_failing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.toml");
        run(&path, &["Hyprland".into()]).unwrap();
        run(&path, &["hyprland".into()]).unwrap();
        let book = load_word_book(&path).unwrap();
        assert_eq!(book.preferred_words, vec!["Hyprland"]);
    }
}
