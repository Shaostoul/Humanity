//! A cover slot and the small pieces of a library shelf (2026-10-19, first used by the Games
//! page, pages/games.rs):
//!
//! - [`paint_cover`]: a picture filling a rounded slot, cropped to fill it, when there is one;
//!   otherwise a plain panel in the theme's own colours, tinted toward a hue of the thing's own
//!   ([`super::swatch_color`] of its id, so a shelf of them is not one grey wall), with its name
//!   on it and, behind, its first letter large and faint or a small picture of its own (a
//!   program's icon). The same slot takes the picture when there is one: nothing else about the
//!   tile changes.
//! - [`thin_bar`]: a slim progress line (achievements on a tile).
//! - [`choice_pills`]: a row of pills, one of them chosen (a view, a sort, a filter).
//! - [`fixed_cell`]: a table cell of exactly its width, so a list's columns line up.
//!
//! Every colour and size comes from the theme. Native only for now: the Games page is
//! desktop-only (a web page cannot read the games on the computer it runs on), so there is no web
//! page to mirror these on yet; the shapes are plain enough to be a CSS class each when one is.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Rect, RichText, Stroke, TextureId, Ui, Vec2};

use crate::gui::theme::Theme;

/// How far a plain cover's panel is tinted toward its own hue (0: the theme's colour alone).
const TINT: f32 = 0.34;

/// The UV rectangle that crops a picture of `size` to fill a slot shaped like `slot` (centred).
pub fn fill_uv(size: Vec2, slot: Vec2) -> Rect {
    if size.x <= 0.0 || size.y <= 0.0 || slot.x <= 0.0 || slot.y <= 0.0 {
        return Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    }
    let (pic, want) = (size.x / size.y, slot.x / slot.y);
    if pic > want {
        // Wider than the slot: keep the middle, the sides cut.
        let w = want / pic;
        Rect::from_min_max(egui::pos2(0.5 - w / 2.0, 0.0), egui::pos2(0.5 + w / 2.0, 1.0))
    } else {
        let h = pic / want;
        Rect::from_min_max(egui::pos2(0.0, 0.5 - h / 2.0), egui::pos2(1.0, 0.5 + h / 2.0))
    }
}

/// The first letter or digit of `title`, upper-cased: the faint mark behind a plain cover.
fn monogram(title: &str) -> String {
    title.chars().find(|c| c.is_alphanumeric()).map(|c| c.to_uppercase().collect()).unwrap_or_default()
}

/// Paint a cover in `rect`: `picture` (its texture and its size) cropped to fill, or the plain
/// panel with `title` (`seed` picks its hue) and, when it has one, `mark` (a small picture of its
/// own, a program's icon, say) where its first letter would be. `title_size` is the name's font
/// size on the panel; `radius` its corners. Clipped to `rect`.
#[allow(clippy::too_many_arguments)]
pub fn paint_cover(painter: &Painter, theme: &Theme, rect: Rect, picture: Option<(TextureId, Vec2)>, mark: Option<(TextureId, Vec2)>, title: &str, seed: &str, title_size: f32, radius: f32) {
    let painter = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
    let corners = CornerRadius::same(radius.round().clamp(0.0, 255.0) as u8);
    if let Some((tex, size)) = picture {
        let uv = fill_uv(size, rect.size());
        painter.add(egui::epaint::RectShape::filled(rect, corners, Color32::WHITE).with_texture(tex, uv));
        painter.rect_stroke(rect, corners, Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
        return;
    }
    let hue = super::swatch_color(seed);
    let base = theme.bg_tertiary().lerp_to_gamma(hue, TINT);
    painter.rect_filled(rect, corners, base);
    // A lighter wash over the top half, so the panel reads as lit from above.
    let top = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.center().y));
    let top_corners = CornerRadius { nw: corners.nw, ne: corners.ne, sw: 0, se: 0 };
    painter.rect_filled(top, top_corners, theme.text_primary().gamma_multiply(0.035));
    let pad = (rect.height() * 0.09).clamp(theme.spacing_xs, theme.spacing_md);
    match mark {
        // Its own small picture, at the top right, no larger than it was made (a blurred icon is
        // worse than a small one).
        Some((tex, size)) => {
            let side = (rect.height() * 0.42).min(size.x.max(size.y)).max(1.0);
            let k = side / size.x.max(size.y).max(1.0);
            let at = Rect::from_min_size(egui::pos2(rect.right() - pad - size.x * k, rect.top() + pad), size * k);
            painter.image(tex, at, Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
        }
        // The first letter, large and faint, at the right.
        None => {
            let letter = monogram(title);
            if !letter.is_empty() {
                let size = rect.height() * 0.95;
                painter.text(egui::pos2(rect.right() - rect.height() * 0.08, rect.center().y), Align2::RIGHT_CENTER, letter, FontId::proportional(size), theme.text_primary().gamma_multiply(0.09));
            }
        }
    }
    // A darker band along the bottom holding the name.
    let galley = painter.layout(title.to_string(), FontId::proportional(title_size), theme.text_strong(), rect.width() - 2.0 * pad);
    let lines = galley.rows.len().min(2) as f32;
    let row_h = galley.size().y / galley.rows.len().max(1) as f32;
    let band_h = (lines * row_h + 2.0 * pad).min(rect.height());
    let band = Rect::from_min_max(egui::pos2(rect.min.x, rect.max.y - band_h), rect.max);
    let band_corners = CornerRadius { nw: 0, ne: 0, sw: corners.sw, se: corners.se };
    painter.rect_filled(band, band_corners, theme.bg_card().gamma_multiply(0.6));
    // At most two lines of the name: the band's clip cuts a third.
    painter.with_clip_rect(band.intersect(painter.clip_rect())).galley(egui::pos2(band.min.x + pad, band.min.y + pad), galley, theme.text_strong());
    painter.rect_stroke(rect, corners, Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
}

/// A slim progress line `width` wide: `fraction` (0 to 1) of it in the accent colour.
pub fn thin_bar(ui: &mut Ui, theme: &Theme, fraction: f32, width: f32) {
    let height = (theme.spacing_xs).max(3.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let corners = CornerRadius::same((height / 2.0).round() as u8);
    ui.painter().rect_filled(rect, corners, theme.slider_track());
    let f = fraction.clamp(0.0, 1.0);
    if f > 0.0 {
        let filled = Rect::from_min_size(rect.min, Vec2::new((rect.width() * f).max(height), height));
        ui.painter().rect_filled(filled, corners, theme.accent());
    }
}

/// One cell of a table row, exactly `width` by `height` whatever it holds (its contents centred
/// on the row's line and clipped to the cell), so a list's columns line up from row to row.
/// (`widgets::row_cell` takes only the room its contents use, which lets columns drift.)
pub fn fixed_cell(ui: &mut Ui, width: f32, height: f32, add: impl FnOnce(&mut Ui)) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::hover());
    let mut cell = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Center)));
    cell.set_clip_rect(rect.intersect(ui.clip_rect()));
    add(&mut cell);
}

/// A row of pills, `chosen` the one picked; returns the one clicked, when it is another.
/// `tips` gives each pill's hover words (as many as there are pills, or none).
pub fn choice_pills(ui: &mut Ui, theme: &Theme, options: &[String], chosen: usize, tips: &[&str]) -> Option<usize> {
    let mut clicked = None;
    for (i, label) in options.iter().enumerate() {
        let on = i == chosen;
        let (fill, text) = if on { (theme.accent(), theme.text_on_accent()) } else { (theme.bg_secondary(), theme.text_secondary()) };
        let button = egui::Button::new(RichText::new(label).size(theme.font_size_small).color(text))
            .fill(fill)
            .stroke(Stroke::new(1.0, if on { theme.accent() } else { theme.border() }))
            .corner_radius(CornerRadius::same(theme.pill_radius.round().clamp(0.0, 255.0) as u8));
        let mut resp = ui.add(button);
        if let Some(tip) = tips.get(i) {
            resp = resp.on_hover_text(*tip);
        }
        if resp.clicked() && !on {
            clicked = Some(i);
        }
    }
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A PICTURE IS CROPPED TO FILL ITS SLOT, never stretched: a wide picture in a narrower slot
    /// keeps its middle, a tall one keeps its middle band, one of the slot's own shape is whole.
    ///
    /// Seen red 2026-10-19 with the two cases swapped (`pic < want`): "a wide picture keeps its
    /// middle" got [[0.0 -0.5] - [1.0 1.5]], a UV taller than the picture.
    #[test]
    fn a_picture_is_cropped_to_fill() {
        let wide = fill_uv(Vec2::new(400.0, 100.0), Vec2::new(200.0, 100.0));
        assert!((wide.min.x - 0.25).abs() < 1e-4 && (wide.max.x - 0.75).abs() < 1e-4 && wide.min.y == 0.0 && wide.max.y == 1.0, "a wide picture keeps its middle: {wide:?}");
        let tall = fill_uv(Vec2::new(100.0, 400.0), Vec2::new(100.0, 100.0));
        assert!((tall.min.y - 0.375).abs() < 1e-4 && (tall.max.y - 0.625).abs() < 1e-4, "a tall one keeps its middle band: {tall:?}");
        let same = fill_uv(Vec2::new(460.0, 215.0), Vec2::new(230.0, 107.5));
        assert!(same.min.x.abs() < 1e-4 && same.min.y.abs() < 1e-4 && (same.max.x - 1.0).abs() < 1e-4 && (same.max.y - 1.0).abs() < 1e-4, "one of the slot's shape is whole: {same:?}");
        assert_eq!(monogram("  the long dark"), "T");
    }
}
