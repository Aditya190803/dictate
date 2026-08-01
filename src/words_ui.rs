//! GTK dictionary manager (Wispr Flow–style vocabulary + misspelling fixes).

use crate::word_store::{load_word_book, save_word_book, DictionaryRow, WordBook, MAX_WORD_LEN};
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, Entry, Label,
    ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow, Separator,
};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

fn subtitle(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    label.add_css_class("dim-label");
    label
}

fn row_matches_filter(row: &DictionaryRow, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    if row.word.to_lowercase().contains(&q) {
        return true;
    }
    if let Some(m) = &row.misspelling {
        if m.to_lowercase().contains(&q) {
            return true;
        }
    }
    false
}

fn rebuild_list(
    book: &WordBook,
    list: &ListBox,
    filter: &str,
    on_star: Rc<dyn Fn(&str)>,
    on_delete: Rc<dyn Fn(&str)>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    for row in book.rows() {
        if !row_matches_filter(&row, filter) {
            continue;
        }
        let list_row = ListBoxRow::new();
        list_row.set_widget_name(&row.word);

        let h = GtkBox::new(Orientation::Horizontal, 8);
        h.set_margin_top(6);
        h.set_margin_bottom(6);
        h.set_margin_start(10);
        h.set_margin_end(10);

        let star = Button::from_icon_name(if row.starred {
            "star-filled-symbolic"
        } else {
            "starred-symbolic"
        });
        star.add_css_class("flat");
        star.set_tooltip_text(Some(if row.starred {
            "Unstar word"
        } else {
            "Star important words (sorted first)"
        }));
        let word_for_star = row.word.clone();
        let on_star = Rc::clone(&on_star);
        star.connect_clicked(move |_| on_star(&word_for_star));

        let text = if let Some(m) = &row.misspelling {
            format!("{}  ·  fixes “{}”", row.word, m)
        } else {
            row.word.clone()
        };
        let label = Label::new(Some(&text));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        label.set_wrap(true);

        let delete = Button::from_icon_name("user-trash-symbolic");
        delete.add_css_class("flat");
        delete.set_tooltip_text(Some("Remove from dictionary"));
        let word_for_del = row.word.clone();
        let on_delete = Rc::clone(&on_delete);
        delete.connect_clicked(move |_| on_delete(&word_for_del));

        h.append(&star);
        h.append(&label);
        h.append(&delete);
        list_row.set_child(Some(&h));
        list.append(&list_row);
    }
}

fn save_and_status(path: &Path, book: &WordBook, status: &Label) {
    match save_word_book(path, book) {
        Ok(()) => status.set_text("Saved — takes effect on the next phrase (no restart)."),
        Err(e) => status.set_text(&format!("Could not save: {e}")),
    }
}

pub fn run(path: &Path) -> anyhow::Result<()> {
    gtk4::init().map_err(|e| anyhow::anyhow!("GTK init failed: {e}"))?;

    let path = path.to_path_buf();
    let initial_book = load_word_book(&path)?;
    let app = Application::builder()
        .application_id("dev.dictate.words")
        .build();

    app.connect_activate(move |app| build_window(app, path.clone(), initial_book.clone()));
    app.run();
    Ok(())
}

fn build_window(app: &Application, path: PathBuf, initial_book: WordBook) {
    let book = Rc::new(RefCell::new(initial_book));
    let editing_word: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let search_query = Rc::new(RefCell::new(String::new()));

    let window = ApplicationWindow::builder()
        .application(app)
        .title("Dictionary")
        .default_width(520)
        .default_height(680)
        .resizable(true)
        .build();

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(18);
    root.set_margin_bottom(18);
    root.set_margin_start(18);
    root.set_margin_end(18);

    let title = Label::new(Some("Dictionary"));
    title.set_xalign(0.0);
    title.add_css_class("title-1");
    root.append(&title);
    root.append(&subtitle(
        "Teach Dictate names, jargon, and products. Vocabulary boosts recognition; optional misspelling rules swap what the model keeps hearing for the spelling you want.",
    ));

    let search = Entry::new();
    search.set_placeholder_text(Some("Search words and corrections"));
    root.append(&search);

    let list = ListBox::new();
    list.set_selection_mode(gtk4::SelectionMode::Single);
    let list_scroll = ScrolledWindow::builder()
        .min_content_height(220)
        .vexpand(true)
        .hscrollbar_policy(PolicyType::Never)
        .child(&list)
        .build();
    root.append(&list_scroll);

    root.append(&Separator::new(Orientation::Horizontal));

    let form_title = Label::new(Some("Add new"));
    form_title.set_xalign(0.0);
    form_title.add_css_class("heading");
    root.append(&form_title);

    let word_entry = Entry::new();
    word_entry.set_placeholder_text(Some("Word or phrase to recognize"));
    let correct_misspelling = CheckButton::with_label("Correct a misspelling");
    let wrong_entry = Entry::new();
    wrong_entry.set_placeholder_text(Some("Wrong spelling Dictate keeps producing"));
    wrong_entry.set_sensitive(false);

    correct_misspelling.connect_toggled({
        let wrong_entry = wrong_entry.clone();
        move |cb| wrong_entry.set_sensitive(cb.is_active())
    });

    let form = GtkBox::new(Orientation::Vertical, 8);
    form.append(&word_entry);
    form.append(&correct_misspelling);
    form.append(&wrong_entry);

    let actions = GtkBox::new(Orientation::Horizontal, 8);
    let save_btn = Button::with_label("Save");
    save_btn.add_css_class("suggested-action");
    let cancel_edit = Button::with_label("Cancel edit");
    cancel_edit.set_visible(false);
    actions.append(&save_btn);
    actions.append(&cancel_edit);
    actions.set_halign(Align::End);
    form.append(&actions);
    root.append(&form);

    let status = Label::new(Some(&format!("{}", path.display())));
    status.set_xalign(0.0);
    status.set_wrap(true);
    status.add_css_class("dim-label");
    root.append(&status);

    let refresh_slot: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));

    {
        let book = Rc::clone(&book);
        let list = list.clone();
        let search_query = Rc::clone(&search_query);
        let path = path.clone();
        let status = status.clone();
        let editing_word = Rc::clone(&editing_word);
        let word_entry = word_entry.clone();
        let wrong_entry = wrong_entry.clone();
        let correct_misspelling = correct_misspelling.clone();
        let cancel_edit = cancel_edit.clone();
        let form_title = form_title.clone();
        let refresh_slot_for_closure = Rc::clone(&refresh_slot);

        let do_refresh: Rc<dyn Fn()> = Rc::new(move || {
            let filter = search_query.borrow().clone();
            let book_ref = book.borrow();
            rebuild_list(
                &book_ref,
                &list,
                &filter,
                {
                    let book = Rc::clone(&book);
                    let path = path.clone();
                    let status = status.clone();
                    let refresh_slot = Rc::clone(&refresh_slot_for_closure);
                    Rc::new(move |word: &str| {
                        book.borrow_mut().toggle_star(word);
                        save_and_status(&path, &book.borrow(), &status);
                        if let Some(f) = refresh_slot.borrow().as_ref() {
                            f();
                        }
                    }) as Rc<dyn Fn(&str)>
                },
                {
                    let book = Rc::clone(&book);
                    let path = path.clone();
                    let status = status.clone();
                    let editing_word = Rc::clone(&editing_word);
                    let word_entry = word_entry.clone();
                    let wrong_entry = wrong_entry.clone();
                    let correct_misspelling = correct_misspelling.clone();
                    let cancel_edit = cancel_edit.clone();
                    let form_title = form_title.clone();
                    let refresh_slot = Rc::clone(&refresh_slot_for_closure);
                    Rc::new(move |word: &str| {
                        book.borrow_mut().remove_preferred_word(word);
                        save_and_status(&path, &book.borrow(), &status);
                        if editing_word
                            .borrow()
                            .as_ref()
                            .is_some_and(|w| w.eq_ignore_ascii_case(word))
                        {
                            *editing_word.borrow_mut() = None;
                            word_entry.set_text("");
                            wrong_entry.set_text("");
                            correct_misspelling.set_active(false);
                            cancel_edit.set_visible(false);
                            form_title.set_text("Add new");
                        }
                        if let Some(f) = refresh_slot.borrow().as_ref() {
                            f();
                        }
                    }) as Rc<dyn Fn(&str)>
                },
            );
        });
        *refresh_slot.borrow_mut() = Some(Rc::clone(&do_refresh));
        do_refresh();
    }

    let refresh_fn = refresh_slot.borrow().clone().unwrap();

    search.connect_changed({
        let search_query = Rc::clone(&search_query);
        let refresh_fn = Rc::clone(&refresh_fn);
        move |e| {
            *search_query.borrow_mut() = e.text().to_string();
            refresh_fn();
        }
    });

    list.connect_row_activated({
        let book = Rc::clone(&book);
        let editing_word = Rc::clone(&editing_word);
        let word_entry = word_entry.clone();
        let wrong_entry = wrong_entry.clone();
        let correct_misspelling = correct_misspelling.clone();
        let cancel_edit = cancel_edit.clone();
        let form_title = form_title.clone();
        move |_, row| {
            let word = row.widget_name().to_string();
            if word.is_empty() {
                return;
            }
            let book_ref = book.borrow();
            let Some(entry) = book_ref.rows().into_iter().find(|r| r.word == word) else {
                return;
            };
            *editing_word.borrow_mut() = Some(entry.word.clone());
            word_entry.set_text(&entry.word);
            if let Some(m) = &entry.misspelling {
                correct_misspelling.set_active(true);
                wrong_entry.set_text(m);
            } else {
                correct_misspelling.set_active(false);
                wrong_entry.set_text("");
            }
            form_title.set_text("Edit entry");
            cancel_edit.set_visible(true);
        }
    });

    cancel_edit.connect_clicked({
        let editing_word = Rc::clone(&editing_word);
        let word_entry = word_entry.clone();
        let wrong_entry = wrong_entry.clone();
        let correct_misspelling = correct_misspelling.clone();
        let cancel_edit = cancel_edit.clone();
        let form_title = form_title.clone();
        move |_| {
            *editing_word.borrow_mut() = None;
            word_entry.set_text("");
            wrong_entry.set_text("");
            correct_misspelling.set_active(false);
            cancel_edit.set_visible(false);
            form_title.set_text("Add new");
        }
    });

    save_btn.connect_clicked({
        let book = Rc::clone(&book);
        let path = path.clone();
        let status = status.clone();
        let editing_word = Rc::clone(&editing_word);
        let word_entry = word_entry.clone();
        let wrong_entry = wrong_entry.clone();
        let correct_misspelling = correct_misspelling.clone();
        let cancel_edit = cancel_edit.clone();
        let form_title = form_title.clone();
        let refresh_fn = Rc::clone(&refresh_fn);
        move |_| {
            let word = word_entry.text().trim().to_string();
            let prev = editing_word.borrow().clone();
            if let Err(msg) = book.borrow().validate_new_word(&word, prev.as_deref()) {
                status.set_text(&msg);
                return;
            }
            let miss = if correct_misspelling.is_active() {
                let w = wrong_entry.text().trim().to_string();
                if w.is_empty() {
                    status.set_text(
                        "Enter the misspelling Dictate produces, or turn the option off.",
                    );
                    return;
                }
                if crate::word_store::word_char_len(&w) > MAX_WORD_LEN {
                    status.set_text(&format!("Use at most {MAX_WORD_LEN} characters."));
                    return;
                }
                Some(w)
            } else {
                None
            };
            {
                let mut b = book.borrow_mut();
                b.upsert_row(&word, miss.as_deref(), prev.as_deref());
                save_and_status(&path, &b, &status);
            }
            *editing_word.borrow_mut() = None;
            word_entry.set_text("");
            wrong_entry.set_text("");
            correct_misspelling.set_active(false);
            cancel_edit.set_visible(false);
            form_title.set_text("Add new");
            refresh_fn();
        }
    });

    let css = gtk4::CssProvider::new();
    css.load_from_data(
        r#"
        window { background: #fbfaf7; }
        entry { min-height: 38px; }
        listbox { background: #ffffff; border: 1px solid rgba(0,0,0,0.10); border-radius: 10px; }
        button { min-height: 34px; }
        .heading { font-weight: 700; }
        "#,
    );
    gtk4::style_context_add_provider_for_display(
        &gtk4::prelude::RootExt::display(&window),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let key_controller = gtk4::EventControllerKey::new();
    let save_btn_for_enter = save_btn.clone();
    let word_entry_for_keys = word_entry.clone();
    key_controller.connect_key_pressed(move |_, key, _, modifiers| {
        let ctrl = modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
        if key == gtk4::gdk::Key::Return || key == gtk4::gdk::Key::KP_Enter {
            save_btn_for_enter.emit_clicked();
            return glib::Propagation::Stop;
        }
        if ctrl && key == gtk4::gdk::Key::n {
            word_entry_for_keys.grab_focus();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    window.add_controller(key_controller);

    window.set_child(Some(&root));
    window.present();
}
