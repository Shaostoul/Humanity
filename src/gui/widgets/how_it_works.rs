//! "How this page works" (2026-10-20): a page's explanations folded behind one small toggle, so a
//! page shows its things first and its reasons one click away. The Apps page and the Files page use
//! it, each remembering in the settings whether it is open (config.json `apps_how_open`,
//! `files_how_open`), closed until the person opens it.
//!
//! CLAUDE.md's "say why" rule is kept, not traded away: every sentence and every Why button that
//! was on the page is inside the fold, word for word, on the same page where the person meets the
//! limit; only what a person must see BEFORE acting stays outside it (the page decides which).
//!
//! Two parts: [`toggle`], the small button with a triangle that points right when closed and down
//! when open, and [`panel`], the box the explanations are drawn in, under a heading per part of the
//! page ([`part_heading`]). Theme tokens only.
//!
//! Web twin: none yet (the website's Apps and Files pages show their lines in place).

use egui::{Rect, RichText, Sense, Stroke, Ui, Vec2};

use super::icons;
use crate::gui::theme::Theme;

/// The toggle's words.
pub const LABEL: &str = "How this page works";
/// Its hover words.
pub const TIP: &str = "What this page does and does not do, and why, with the research behind each reason.";

/// The toggle: its triangle and its words, small. True when it was pressed this frame.
pub fn toggle(ui: &mut Ui, theme: &Theme, open: bool) -> bool {
    let font = egui::FontId::proportional(theme.font_size_small);
    let text_colour = if open { theme.text_primary() } else { theme.text_secondary() };
    let galley = ui.painter().layout_no_wrap(LABEL.to_string(), font, text_colour);
    let mark = theme.font_size_small;
    let pad = Vec2::new(theme.spacing_sm, theme.button_pad_y.max(2.0));
    let size = Vec2::new(pad.x * 2.0 + mark + theme.spacing_xs + galley.size().x, pad.y * 2.0 + galley.size().y.max(mark));
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if ui.is_rect_visible(rect) {
        let corners = egui::CornerRadius::same(theme.pill_radius.round().clamp(0.0, 255.0) as u8);
        let fill = if resp.hovered() || open { theme.bg_secondary() } else { theme.bg_panel() };
        let edge = if resp.hovered() { theme.accent() } else { theme.border() };
        ui.painter().rect_filled(rect, corners, fill);
        ui.painter().rect_stroke(rect, corners, Stroke::new(1.0, edge), egui::StrokeKind::Inside);
        let mark_rect = Rect::from_min_size(egui::pos2(rect.left() + pad.x, rect.center().y - mark / 2.0), Vec2::splat(mark));
        if open {
            icons::paint_triangle_down(ui.painter(), mark_rect, text_colour);
        } else {
            icons::paint_triangle_right(ui.painter(), mark_rect, text_colour);
        }
        let at = egui::pos2(mark_rect.right() + theme.spacing_xs, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(at, galley, text_colour);
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    resp.on_hover_text(TIP).clicked()
}

/// The box the explanations are drawn in, the page's full width.
pub fn panel<R>(ui: &mut Ui, theme: &Theme, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(theme.spacing_xs);
    let r = egui::Frame::NONE
        .fill(theme.bg_secondary())
        .stroke(Stroke::new(1.0, theme.border()))
        .corner_radius(egui::CornerRadius::same(theme.border_radius.round().clamp(0.0, 255.0) as u8))
        .inner_margin(egui::Margin::same(theme.spacing_md.round().clamp(0.0, 127.0) as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner;
    ui.add_space(theme.spacing_xs);
    r
}

/// A part's heading inside the box ("Games", "Other programs"), with a dot in its colour when it
/// has one (never the colour alone: the name is always there).
pub fn part_heading(ui: &mut Ui, theme: &Theme, name: &str, colour: Option<egui::Color32>) {
    ui.add_space(theme.spacing_xs);
    ui.horizontal(|ui| {
        if let Some(c) = colour {
            let (dot, _) = ui.allocate_exact_size(Vec2::splat(theme.font_size_small), Sense::hover());
            ui.painter().circle_filled(dot.center(), dot.width() * 0.35, c);
        }
        ui.label(RichText::new(name).size(theme.font_size_body).strong().color(theme.text_primary()));
    });
}

/// One sentence of explanation, in the box.
pub fn line(ui: &mut Ui, theme: &Theme, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).size(theme.font_size_small).color(theme.text_secondary())).wrap());
}
