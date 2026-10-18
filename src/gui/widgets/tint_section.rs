//! One part of a window on a darker shade of the chat sidebar's section colours (the DMs' red, the
//! groups' green, the servers' blue), edged in the colour itself, with a small title, so a window's
//! parts read apart at a glance (operator, 2026-10-17: "the RGB aesthetic for the different
//! sections of the interface like how the DMs, Groups, and servers in chat are but darker RGB in
//! the pop up window"). Used by the windows where people attach files: the file picker, Add to
//! Media, a Media item's Edit, and the chat's "Send a file". The colours are the theme's own tokens,
//! so the Settings theme editor changes them here too.
//!
//! Web mirror: the `tint-section` class (`tint-red`, `tint-green`, `tint-blue`).

use crate::gui::theme::Theme;
use egui::RichText;

/// Which of the three colours a section takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tint {
    /// The DMs' red (`dm_bg`).
    Red,
    /// The groups' green (`group_bg`).
    Green,
    /// The servers' blue (`server_bg`).
    Blue,
}

/// How much of a section colour's strength its fill keeps: darker than the chat sidebar's headers.
pub const SHADE: f32 = 0.7;

/// The theme colour of `tint`.
pub fn color(theme: &Theme, tint: Tint) -> egui::Color32 {
    match tint {
        Tint::Red => theme.dm_bg(),
        Tint::Green => theme.group_bg(),
        Tint::Blue => theme.server_bg(),
    }
}

/// Draw a section titled `title` in `tint` around what `add` draws, the full width available.
pub fn show<R>(ui: &mut egui::Ui, theme: &Theme, tint: Tint, title: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let c = color(theme, tint);
    egui::Frame::NONE
        .fill(c.gamma_multiply(SHADE))
        .stroke(egui::Stroke::new(1.0, c))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(egui::Margin::same(theme.spacing_sm as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(theme.font_size_small).color(theme.text_secondary()).strong());
            ui.add_space(theme.spacing_xs);
            add(ui)
        })
        .inner
}

/// The nav bar's active border around `rect` (an Upload button): the same colour and animation the
/// theme gives the current page's tab (RGB cycle by default; pulse, solid or off as set), redrawn
/// while it moves.
pub fn rgb_border(ui: &egui::Ui, theme: &Theme, rect: egui::Rect) {
    let time = ui.input(|i| i.time) as f32;
    let color = crate::gui::pages::escape_menu::channeling_color(theme, time, false, theme.accent());
    let width = theme.nav_active_border_width.max(1.5);
    ui.painter().rect_stroke(rect.expand(width), egui::CornerRadius::same(theme.border_radius as u8 + 2), egui::Stroke::new(width, color), egui::StrokeKind::Middle);
    let moving = theme.animations_enabled && matches!(theme.nav_active_border_animation, crate::gui::theme::anim::RGB_CYCLE | crate::gui::theme::anim::PULSE);
    if moving {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
}

/// Whether a frame painted a section's fill in `tint` (its colour at [`SHADE`]), for the tests of
/// the windows that use these sections.
#[cfg(test)]
pub fn painted(out: &egui::FullOutput, theme: &Theme, tint: Tint) -> bool {
    let want = color(theme, tint).gamma_multiply(SHADE);
    fn walk(s: &egui::Shape, want: egui::Color32) -> bool {
        match s {
            egui::Shape::Rect(r) => r.fill == want,
            egui::Shape::Vec(v) => v.iter().any(|x| walk(x, want)),
            _ => false,
        }
    }
    out.shapes.iter().any(|c| walk(&c.shape, want))
}

/// Every place the text exactly `text` is drawn, in paint order (bottom first), for the same tests.
#[cfg(test)]
pub fn texts_at(out: &egui::FullOutput, text: &str) -> Vec<egui::Rect> {
    fn walk(s: &egui::Shape, text: &str, found: &mut Vec<egui::Rect>) {
        match s {
            egui::Shape::Text(t) if t.galley.text() == text => found.push(t.galley.rect.translate(t.pos.to_vec2())),
            egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, text, found)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, text, &mut found));
    found
}

/// Where the first letter of the text starting with `prefix` is drawn (x), for the same tests: a
/// label in a wrapping row has a box that starts at the row's left edge, its text after whatever
/// came before it on that row, so the box alone cannot say what is to the left of the words.
#[cfg(test)]
pub fn text_starts_at(out: &egui::FullOutput, prefix: &str) -> Option<f32> {
    fn walk(s: &egui::Shape, prefix: &str) -> Option<f32> {
        match s {
            egui::Shape::Text(t) if t.galley.text().starts_with(prefix) => {
                let first = t.galley.rows.first()?.glyphs.first()?;
                Some(t.pos.x + first.pos.x)
            }
            egui::Shape::Vec(v) => v.iter().find_map(|x| walk(x, prefix)),
            _ => None,
        }
    }
    out.shapes.iter().find_map(|c| walk(&c.shape, prefix))
}
