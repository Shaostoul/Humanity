//! The recovery phrase as it is being typed, word by word and numbered, under the box it is typed
//! into (2026-11-22). The operator, restoring his identity on his phone: "when I'm entering my 24
//! word recovery phrase ... I can't scroll low enough when the keyboard opens to see the text box
//! preventing me from spelling things correctly." On a phone the 24 words wrap into one long block
//! in a narrow box, where a misspelled or missing word is hard to find. Here each word typed gets
//! its number, as on the paper it was written on, a word that is not one of the recovery phrase's
//! words is marked (in the danger colour, and named by its number in words, so the colour is never
//! the only sign), and the count says how many of the 24 are in.
//!
//! The words are only drawn: nothing here keeps or writes them anywhere. Used by the first-run
//! restore (pages/main_menu.rs) and by Settings' restore (pages/settings.rs).

use crate::gui::theme::Theme;
use egui::{RichText, Ui};
use humanity_core::i18n::{t, tf, tn};

/// The words in `typed` (split at spaces and line breaks, as the phrase is read) with whether each
/// is one of the recovery phrase's words, in order.
pub fn words(typed: &str) -> Vec<(String, bool)> {
    let list = bip39::Language::English;
    typed
        .split_whitespace()
        .map(|w| {
            let lower = w.to_lowercase();
            let known = list.find_word(&lower).is_some();
            (lower, known)
        })
        .collect()
}

/// The typed words numbered under the box (the module notes): the count, then the words in rows
/// (four on a wide window, three on a phone's width), then the numbers of any word that is not a
/// recovery-phrase word. Nothing while the box is empty. `id_salt` names this place.
pub fn numbered(ui: &mut Ui, theme: &Theme, id_salt: &str, typed: &str) {
    let words = words(typed);
    if words.is_empty() {
        return;
    }
    let total = crate::net::identity::PHRASE_WORDS;
    let count_color = if words.len() == total { theme.success() } else { theme.text_secondary() };
    ui.label(RichText::new(tn("phrase_typed.count", words.len() as i64, &[("total", &total)])).size(theme.font_size_small).color(count_color))
        .on_hover_text(t("phrase_typed.hint"));
    let columns = if ui.available_width() < 420.0 { 3 } else { 4 };
    egui::Grid::new(egui::Id::new(id_salt).with("phrase_typed_grid"))
        .num_columns(columns)
        .spacing([theme.spacing_md, theme.spacing_xs])
        .show(ui, |ui| {
            for (i, (word, known)) in words.iter().enumerate() {
                let color = if *known { theme.text_primary() } else { theme.danger() };
                ui.label(RichText::new(format!("{:>2}. {word}", i + 1)).monospace().size(theme.font_size_body).color(color));
                if (i + 1) % columns == 0 {
                    ui.end_row();
                }
            }
        });
    let unknown: Vec<String> = words.iter().enumerate().filter(|(_, (_, known))| !known).map(|(i, _)| (i + 1).to_string()).collect();
    if !unknown.is_empty() {
        let list = unknown.join(", ");
        ui.add(egui::Label::new(RichText::new(tf("phrase_typed.unknown", &[("list", &list)])).size(theme.font_size_small).color(theme.danger())).wrap());
    }
}
