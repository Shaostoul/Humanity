//! The recovery phrase, and the device-link code that carries the same key, covered until the
//! person shows them (operator, 2026-10-16: "livestreamers may accidentally reveal their seed
//! phrase while showing people what it looks like on first run", and the same for anyone signing
//! up in public).
//!
//! Wherever the 24 words or the code appear, they start covered: in their place the numbers and
//! dots (the code: a blank square), a line saying when it is safe to show them, and one button.
//! While covered the words are not drawn at all, so nothing of them reaches a screen capture. They
//! cover themselves again when the window stops being the one in front (the person switched to
//! their streaming or call app) and whenever they come back on screen after being away, so a
//! screen reopened later never opens uncovered. The website does the same
//! (web/shared/phrase-cover.js), in the same words.

use crate::gui::theme::Theme;
use crate::gui::widgets;
use egui::{RichText, Ui};

/// The line beside covered words.
pub const WARNING: &str = "Only show these when nobody can see your screen: not while streaming, recording or sharing your screen, and not where people can look over your shoulder.";
/// The button that shows the words: held for `HOLD_SECONDS`, so a stray click never shows them
/// (operator, 2026-10-16: "a 3 second hold", like the app's other hold-to-confirm buttons).
pub const SHOW: &str = "Hold to show the words";
/// What a show button says while it is held.
pub const HOLDING: &str = "Keep holding...";
/// How long a show button is held before it shows.
pub const HOLD_SECONDS: f32 = 3.0;
/// The show buttons' tooltip.
pub const HOLD_TIP: &str = "Press and hold for 3 seconds, so a stray click never shows them.";
/// The button that covers them again.
pub const HIDE: &str = "Hide the words";
/// The line under shown words or a shown code.
pub const COVERS_ITSELF: &str = "They cover themselves again when you switch to another window.";
/// The line beside a covered device-link code.
pub const CODE_WARNING: &str = "Only show this code when nobody can see your screen or record it: anyone who scans it, even from a recording, gets your account.";
/// The button that shows the code, held like `SHOW`.
pub const SHOW_CODE: &str = "Hold to show the code";
/// The button that covers it again.
pub const HIDE_CODE: &str = "Hide the code";
/// What a covered word shows: as wide as the longest word on the list (8 letters).
pub const COVERED_WORD: &str = "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}";
/// What a covered code shows.
pub const CODE_HIDDEN: &str = "Code hidden";

/// Whether the cover `id` is lifted on this pass. It goes down again when the window is not the
/// one in front, and when it was not drawn on the pass before (its screen was left and is back).
fn lifted(ui: &Ui, id: egui::Id) -> bool {
    let pass = ui.ctx().cumulative_pass_nr();
    let (up, last) = ui.ctx().data(|d| d.get_temp::<(bool, u64)>(id)).unwrap_or((false, 0));
    up && ui.input(|i| i.focused) && last + 1 >= pass
}

/// A show button: press and hold for `HOLD_SECONDS` (the ring fills; letting go early starts
/// over). True on the pass the hold completes. The same hold-to-confirm button the app's other
/// guarded actions use (chat.rs `hold_to_confirm_button`), kept to a button's width.
fn hold_to_show(ui: &mut Ui, theme: &Theme, id: egui::Id, label: &str) -> bool {
    ui.scope(|ui| {
        ui.set_max_width(280.0);
        crate::gui::pages::chat::hold_to_confirm_button(ui, theme, id.with("hold"), label, HOLDING, HOLD_SECONDS, HOLD_TIP)
    })
    .inner
}

/// Keep the cover's state for the next pass.
fn keep(ui: &Ui, id: egui::Id, up: bool) {
    let pass = ui.ctx().cumulative_pass_nr();
    ui.ctx().data_mut(|d| d.insert_temp(id, (up, pass)));
}

/// The 24 words of `phrase`, numbered in four columns, covered until the person presses Show the
/// words (see the module). `id_salt` names this place, so each place covers on its own. True
/// while they are shown: a Copy button beside them is drawn only then (BUG-214), so a stray click
/// on covered words cannot put them on the clipboard.
pub fn words(ui: &mut Ui, theme: &Theme, id_salt: &str, phrase: &str) -> bool {
    let id = egui::Id::new(id_salt).with("phrase_cover");
    let mut up = lifted(ui, id);
    egui::Grid::new(egui::Id::new(id_salt).with("phrase_cover_grid"))
        .num_columns(4)
        .spacing([18.0, 4.0])
        .show(ui, |ui| {
            for (i, word) in phrase.split_whitespace().enumerate() {
                let (shown, color) = if up { (word, theme.text_primary()) } else { (COVERED_WORD, theme.text_muted()) };
                ui.label(RichText::new(format!("{:>2}. {shown}", i + 1)).monospace().size(theme.font_size_body).color(color));
                if (i + 1) % 4 == 0 {
                    ui.end_row();
                }
            }
        });
    ui.add_space(theme.spacing_xs);
    if up {
        if widgets::secondary_button(ui, theme, HIDE) {
            up = false;
        }
        ui.label(RichText::new(COVERS_ITSELF).size(theme.font_size_small).color(theme.text_muted()));
    } else {
        ui.label(RichText::new(WARNING).size(theme.font_size_small).color(theme.warning()));
        ui.add_space(theme.spacing_xs);
        if hold_to_show(ui, theme, id, SHOW) {
            up = true;
        }
    }
    keep(ui, id, up);
    up
}

/// A code that carries the account (the device-link QR), drawn by `draw`, covered until the person
/// presses Show the code: in its place a blank square `side` points across.
pub fn code(ui: &mut Ui, theme: &Theme, id_salt: &str, side: f32, draw: impl FnOnce(&mut Ui)) {
    let id = egui::Id::new(id_salt).with("code_cover");
    let mut up = lifted(ui, id);
    if up {
        draw(ui);
        ui.add_space(theme.spacing_xs);
        if widgets::secondary_button(ui, theme, HIDE_CODE) {
            up = false;
        }
        ui.label(RichText::new(COVERS_ITSELF).size(theme.font_size_small).color(theme.text_muted()));
    } else {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
        ui.painter().rect_filled(rect, egui::Rounding::same(theme.border_radius as u8), theme.bg_tertiary());
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, CODE_HIDDEN, egui::FontId::proportional(theme.font_size_small), theme.text_muted());
        ui.add_space(theme.spacing_xs);
        ui.label(RichText::new(CODE_WARNING).size(theme.font_size_small).color(theme.warning()));
        ui.add_space(theme.spacing_xs);
        if hold_to_show(ui, theme, id, SHOW_CODE) {
            up = true;
        }
    }
    keep(ui, id, up);
}

#[cfg(test)]
#[path = "phrase_cover_tests.rs"]
mod tests;
