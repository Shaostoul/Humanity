//! A box for a secret the person types themselves (a passphrase, a PIN, a token): its text hidden
//! behind dots by default, with a Show / Hide button beside it, a drawn eye (2026-11-22).
//!
//! The operator, on his phone, in the window that asks for a passphrase to encrypt his key: "I
//! also have no way of previewing the passphrase in the encrypt your private key window. It's just
//! the dots with encrypt or skip." On a phone's keyboard a passphrase is easy to mistype and, with
//! only dots, impossible to check, and a mistyped passphrase locks the person out of their own key.
//!
//! - **Hidden by default**, and hidden again whenever the box was not drawn on the pass before (the
//!   window it is in closed, or its page was left) and while the app is not the window in front
//!   (the person switched to another app, a screen-sharing or streaming one among them). The same
//!   rule as the covered recovery phrase (phrase_cover.rs).
//! - **The button** is a drawn eye (an eye to show, an eye struck through to hide; egui's font
//!   draws eye glyphs as boxes), named Show or Hide for a screen reader, with its hint by key.
//! - **Nothing is kept**: whether it is shown lives in egui's memory for this window only; the text
//!   is the caller's, as before.
//!
//! The website's twin is web/shared/secret-field.js (the same words by key), so a passphrase box
//! there has the same button.

use egui::{Context, Id, Response, Sense, Ui, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets::hint::Hint;
use humanity_core::i18n::t;

/// The id under which a secret's shown state is kept: (shown, the pass it was last drawn on).
fn shown_id(id: Id) -> Id {
    id.with("secret_field_shown")
}

/// Whether the secret in the box `id` is shown on this pass: only after the person pressed Show,
/// only while the box has been drawn on every pass since, and only while the app is the window in
/// front (the module notes).
pub fn is_shown(ctx: &Context, id: Id) -> bool {
    let pass = ctx.cumulative_pass_nr();
    let (shown, last) = ctx.data(|d| d.get_temp::<(bool, u64)>(shown_id(id))).unwrap_or((false, 0));
    shown && last + 1 >= pass && ctx.input(|i| i.focused)
}

/// The id of the Show / Hide button beside the box `id` (for the tests, and for a screen reader's
/// walk of the window).
pub fn toggle_id(id: Id) -> Id {
    id.with("secret_field_toggle")
}

/// A box for a secret (the module notes). Build it, then [`SecretField::show`] it where the
/// `egui::TextEdit::singleline(..).password(true)` was.
pub struct SecretField<'t> {
    text: &'t mut String,
    id: Id,
    hint_text: Option<String>,
    width: Option<f32>,
    char_limit: Option<usize>,
    rows: Option<usize>,
}

impl<'t> SecretField<'t> {
    /// A box for `text`, named by `id_salt` (each box its own, so showing one shows only it).
    pub fn new(id_salt: impl std::hash::Hash, text: &'t mut String) -> Self {
        Self { text, id: Id::new(id_salt).with("secret_field"), hint_text: None, width: None, char_limit: None, rows: None }
    }

    /// The words shown in the empty box.
    pub fn hint_text(mut self, words: impl Into<String>) -> Self {
        self.hint_text = Some(words.into());
        self
    }

    /// At most `n` characters (a PIN's longest).
    pub fn char_limit(mut self, n: usize) -> Self {
        self.char_limit = Some(n);
        self
    }

    /// A box of `rows` lines (the recovery phrase typed to unlock), not one.
    pub fn multiline(mut self, rows: usize) -> Self {
        self.rows = Some(rows);
        self
    }

    /// The width of the box and its button together (the row's width when not given).
    pub fn desired_width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// The text box's id, for a caller that requests focus on it.
    pub fn id(&self) -> Id {
        self.id
    }

    /// Draw the box and its button; the box's response (changed, lost focus, Enter), as the plain
    /// text box gave.
    pub fn show(self, ui: &mut Ui, theme: &Theme) -> Response {
        let SecretField { text, id, hint_text, width, char_limit, rows } = self;
        let shown = is_shown(ui.ctx(), id);
        let total = width.unwrap_or_else(|| ui.available_width());
        let side = theme.input_height.max(ui.spacing().interact_size.y);
        let inner = ui
            .horizontal(|ui| {
                let box_w = (total - side - ui.spacing().item_spacing.x).max(60.0);
                let edit = match rows {
                    Some(rows) => egui::TextEdit::multiline(text).desired_rows(rows), // hint-exempt: added below with secret_field.box_hint
                    None => egui::TextEdit::singleline(text), // hint-exempt: added below with secret_field.box_hint
                };
                let mut edit = edit.id(id).password(!shown).desired_width(box_w);
                if let Some(words) = hint_text {
                    edit = edit.hint_text(words);
                }
                if let Some(n) = char_limit {
                    edit = edit.char_limit(n);
                }
                let resp = ui.add(edit).hint("secret_field.box_hint");
                // Above a phone's keyboard once it opens (gui/visible_area.rs).
                crate::gui::visible_area::keep_in_view(ui, &resp);
                let toggled = eye_button(ui, theme, toggle_id(id), shown, side).clicked();
                (resp, toggled)
            })
            .inner;
        let (resp, toggled) = inner;
        let now_shown = shown != toggled;
        let pass = ui.ctx().cumulative_pass_nr();
        ui.ctx().data_mut(|d| d.insert_temp(shown_id(id), (now_shown, pass)));
        resp
    }
}

/// The Show / Hide button: a drawn eye while the text is hidden (press to show it), the eye struck
/// through while it is shown (press to hide it). `side` square, named for a screen reader.
fn eye_button(ui: &mut Ui, theme: &Theme, id: Id, shown: bool, side: f32) -> Response {
    // The space alone (no widget of its own), then the one clickable widget under its known id.
    let (_, rect) = ui.allocate_space(Vec2::splat(side));
    let resp = ui.interact(rect, id, Sense::click());
    let (name, hint) = if shown { ("secret_field.hide", "secret_field.hide_hint") } else { ("secret_field.show", "secret_field.show_hint") };
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t(name)));
    let color = if resp.hovered() { theme.accent() } else { theme.text_secondary() };
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        if resp.hovered() {
            painter.rect_filled(rect, theme.border_radius, theme.bg_secondary());
        }
        let icon = egui::Rect::from_center_size(rect.center(), Vec2::splat(side * 0.62));
        crate::gui::widgets::icons::paint_eye(painter, icon, color);
        if shown {
            // Struck through: the text is shown, press to hide it.
            let w = icon.width();
            painter.line_segment(
                [icon.left_bottom() + egui::vec2(w * 0.12, -w * 0.12), icon.right_top() + egui::vec2(-w * 0.12, w * 0.12)],
                egui::Stroke::new((w * 0.1).max(1.5), color),
            );
        }
    }
    resp.hint(hint)
}

#[cfg(test)]
#[path = "secret_field_tests.rs"]
mod tests;
