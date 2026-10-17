//! A file's type, always shown beside its name (2026-10-17): a drawing, a word and a colour, so
//! that someone who reads little or nothing still knows what kind of file they are looking at (the
//! operator: "people inexperienced with technology and possibly illiterate people"). Green for
//! what is fine to look at (a picture, a video, a sound, a document, a 3D model), amber for what
//! needs care (an archive, a document that can carry macros, a web page, a Blender file), red for
//! what can run things on the computer or pretends to be something it is not, grey when the type
//! is not known. The type comes from `net::file_risk::file_type`; the drawings are the list's own
//! (`data/safety/file_risks.json` `icons`), so both apps draw the same ones.
//!
//! Web mirror: `HosFileRisk.badgeHtml` in web/shared/file-risk.js (class `file-badge level-<level>`).

use crate::gui::theme::Theme;
use crate::net::file_risk::{self, FileType, Level, Shape};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// The colour of a level: the theme's success, warning, danger and muted text.
pub fn level_color(theme: &Theme, level: Level) -> Color32 {
    match level {
        Level::View => theme.success(),
        Level::Care => theme.warning(),
        Level::Danger => theme.danger(),
        Level::Unknown => theme.text_muted(),
    }
}

/// The badge's hover words: its type, and for a disguised file what it is doing.
pub fn hover_words(t: &FileType) -> String {
    let what = if t.ending.is_empty() { t.label.clone() } else { format!("{} ({})", t.label, t.ending) };
    if t.disguised {
        format!("{what}: this file pretends to be something else.")
    } else {
        what
    }
}

/// Draw the badge for `t`: a rounded chip holding its drawing, its word and its ending.
pub fn show(ui: &mut Ui, theme: &Theme, t: &FileType) -> egui::Response {
    let color = level_color(theme, t.level);
    let font = egui::FontId::proportional(theme.font_size_small);
    let label = ui.painter().layout_no_wrap(t.label.clone(), egui::FontId::new(theme.font_size_small, egui::FontFamily::Proportional), color);
    let ending = (!t.ending.is_empty()).then(|| ui.painter().layout_no_wrap(t.ending.clone(), font, theme.text_secondary()));
    let icon = (theme.font_size_small + 3.0).round();
    let (pad, gap) = (6.0, 4.0);
    let text_h = label.size().y;
    let h = icon.max(text_h) + 4.0;
    let w = pad + icon + gap + label.size().x + ending.as_ref().map_or(0.0, |e| gap + e.size().x) + pad;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border = if t.disguised { 2.0 } else { 1.0 };
        painter.rect(rect, h / 2.0, color.gamma_multiply(0.15), Stroke::new(border, color), egui::StrokeKind::Inside);
        let icon_rect = Rect::from_min_size(Pos2::new(rect.left() + pad, rect.center().y - icon / 2.0), Vec2::splat(icon));
        paint_icon(painter, icon_rect, &t.icon, color);
        let mut x = icon_rect.right() + gap;
        let lw = label.size().x;
        painter.galley(Pos2::new(x, rect.center().y - text_h / 2.0), label, color);
        x += lw;
        if let Some(e) = ending {
            let eh = e.size().y;
            painter.galley(Pos2::new(x + gap, rect.center().y - eh / 2.0), e, theme.text_secondary());
        }
    }
    resp.on_hover_text(hover_words(t))
}

/// Paint the drawing named `icon` (the list's 48 by 48 shapes) into `rect`, in `color`.
pub fn paint_icon(painter: &egui::Painter, rect: Rect, icon: &str, color: Color32) {
    let scale = rect.width() / 48.0;
    let at = |x: f32, y: f32| Pos2::new(rect.left() + x * scale, rect.top() + y * scale);
    let stroke = Stroke::new((4.0 * scale).max(1.0), color);
    for s in file_risk::icon(icon) {
        paint_shape(painter, s, &at, stroke, color, scale);
    }
}

fn paint_shape(painter: &egui::Painter, s: &Shape, at: &dyn Fn(f32, f32) -> Pos2, stroke: Stroke, color: Color32, scale: f32) {
    if let Some(r) = s.rect.as_deref().filter(|r| r.len() == 4) {
        let rect = Rect::from_min_max(at(r[0], r[1]), at(r[0] + r[2], r[1] + r[3]));
        if s.fill {
            painter.rect_filled(rect, 0.0, color);
        } else {
            painter.rect_stroke(rect, 0.0, stroke, egui::StrokeKind::Middle);
        }
    } else if let Some(l) = s.line.as_deref().filter(|l| l.len() == 4) {
        painter.line_segment([at(l[0], l[1]), at(l[2], l[3])], stroke);
    } else if let Some(c) = s.circle.as_deref().filter(|c| c.len() == 3) {
        if s.fill {
            painter.circle_filled(at(c[0], c[1]), c[2] * scale, color);
        } else {
            painter.circle_stroke(at(c[0], c[1]), c[2] * scale, stroke);
        }
    } else if let Some(p) = s.poly.as_deref().filter(|p| p.len() >= 4) {
        let points: Vec<Pos2> = p.chunks_exact(2).map(|xy| at(xy[0], xy[1])).collect();
        let shape = if s.closed && s.fill {
            egui::epaint::PathShape::convex_polygon(points, color, stroke)
        } else if s.closed {
            egui::epaint::PathShape::closed_line(points, stroke)
        } else {
            egui::epaint::PathShape::line(points, stroke)
        };
        painter.add(shape);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE BADGE SAYS THE TYPE, IN WORDS AND IN A COLOUR (2026-10-17): drawn headless, a video's badge
    /// holds the word "Video" and its ending in the theme's success colour, a program's "Program" in
    /// the danger colour, a disguised one says so when hovered, and every kind's drawing paints
    /// something. Seen red 2026-10-17 with `level_color` giving every level the muted colour: "a
    /// program's word is red".
    #[test]
    fn the_badge_says_the_type_in_words_and_in_a_colour() {
        let theme = crate::gui::theme::load_theme();
        let ctx = egui::Context::default();
        let texts = |t: &FileType| {
            let out = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    show(ui, &theme, t);
                });
            });
            let mut found = Vec::new();
            let mut shapes = 0;
            fn walk(s: &egui::Shape, found: &mut Vec<(String, Color32)>, shapes: &mut usize) {
                match s {
                    egui::Shape::Text(t) => found.push((t.galley.job.text.clone(), t.fallback_color)),
                    egui::Shape::Vec(v) => v.iter().for_each(|x| walk(x, found, shapes)),
                    _ => *shapes += 1,
                }
            }
            out.shapes.iter().for_each(|c| walk(&c.shape, &mut found, &mut shapes));
            (found, shapes)
        };
        let video = file_risk::file_type("clip.mp4", None);
        let (found, shapes) = texts(&video);
        assert!(found.iter().any(|(t, c)| t == "Video" && *c == theme.success()), "a video's word is green: {found:?}");
        assert!(found.iter().any(|(t, _)| t == ".mp4"), "and its ending: {found:?}");
        assert!(shapes >= 3, "its chip and its drawing are painted ({shapes} shapes)");
        let program = file_risk::file_type("setup.exe", None);
        assert!(texts(&program).0.iter().any(|(t, c)| t == "Program" && *c == theme.danger()), "a program's word is red");
        let fake = file_risk::file_type("cat.png", Some(&[0x4d, 0x5a, 0x90, 0x00]));
        assert!(fake.disguised && hover_words(&fake).contains("pretends to be something else"));
        for name in ["photo.jpg", "song.mp3", "notes.pdf", "a.docx", "a.txt", "a.stl", "a.zip", "a.docm", "a.ps1", "a.lnk", "a.html", "a.blend", "a.dat"] {
            let t = file_risk::file_type(name, None);
            assert!(!file_risk::icon(&t.icon).is_empty(), "{name}: its drawing has shapes");
        }
    }
}
