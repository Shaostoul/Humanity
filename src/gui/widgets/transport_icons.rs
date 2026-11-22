//! A MEDIA PLAYER'S CONTROLS AS DRAWN ICONS (2026-11-21, docs/design/music-page.md section 28; the
//! operator: "Let's add icons for the buttons too. Like play, fastword, etc.").
//!
//! Each icon is drawn with the painter as vector shapes in the theme's colours: no icon font, no
//! picture file, no licence, nothing downloaded, and no glyph that egui's fonts would draw as a box
//! (`tests/icon_glyph_lint.rs`). They read at the theme's control height on the operator's 65-inch
//! screen and at a phone's, crisp at any scale because they are geometry.
//!
//! [`IconButton`] carries one: a square button whose NAME (words by key) is its accessible name, so a
//! screen reader hears "Next", and whose hint says what it does, shown when the pointer rests on it or
//! the keyboard reaches it, as every button's is. The website draws the same shapes as inline SVG of
//! its own (`web/shared/transport-icons.js`), each button with an `aria-label`.

use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Shape, Stroke, Vec2};

use crate::gui::theme::Theme;

/// The icons of a media player's controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportIcon {
    Play,
    Pause,
    Previous,
    Next,
    Shuffle,
    Repeat,
    RepeatOne,
    UpNext,
    Heart,
    HeartFilled,
    Speaker,
    SpeakerMuted,
}

impl TransportIcon {
    pub const ALL: [TransportIcon; 12] = [
        TransportIcon::Play,
        TransportIcon::Pause,
        TransportIcon::Previous,
        TransportIcon::Next,
        TransportIcon::Shuffle,
        TransportIcon::Repeat,
        TransportIcon::RepeatOne,
        TransportIcon::UpNext,
        TransportIcon::Heart,
        TransportIcon::HeartFilled,
        TransportIcon::Speaker,
        TransportIcon::SpeakerMuted,
    ];
}

/// A filled triangle pointing right (`dir` = 1) or left (-1), centred in `r`.
fn triangle(painter: &egui::Painter, r: Rect, dir: f32, color: Color32) {
    let (l, rr, t, b) = (r.left(), r.right(), r.top(), r.bottom());
    let pts = if dir > 0.0 { vec![Pos2::new(l, t), Pos2::new(rr, r.center().y), Pos2::new(l, b)] } else { vec![Pos2::new(rr, t), Pos2::new(l, r.center().y), Pos2::new(rr, b)] };
    painter.add(Shape::convex_polygon(pts, color, Stroke::NONE));
}

/// An arrowhead at `tip` pointing along `dir` (a unit vector), `size` long.
fn arrowhead(painter: &egui::Painter, tip: Pos2, dir: Vec2, size: f32, color: Color32) {
    let back = tip - dir * size;
    let side = Vec2::new(-dir.y, dir.x) * size * 0.6;
    painter.add(Shape::convex_polygon(vec![tip, back + side, back - side], color, Stroke::NONE));
}

/// Draw `icon` inside `rect` in `color`.
pub fn paint(painter: &egui::Painter, rect: Rect, icon: TransportIcon, color: Color32) {
    let s = rect.width().min(rect.height());
    let r = Rect::from_center_size(rect.center(), Vec2::splat(s));
    let p = |x: f32, y: f32| Pos2::new(r.left() + s * x, r.top() + s * y);
    let thick = Stroke::new((s * 0.11).max(1.5), color);
    match icon {
        TransportIcon::Play => triangle(painter, Rect::from_min_max(p(0.22, 0.12), p(0.86, 0.88)), 1.0, color),
        TransportIcon::Pause => {
            painter.rect_filled(Rect::from_min_max(p(0.2, 0.14), p(0.4, 0.86)), CornerRadius::same(1), color);
            painter.rect_filled(Rect::from_min_max(p(0.6, 0.14), p(0.8, 0.86)), CornerRadius::same(1), color);
        }
        TransportIcon::Previous => {
            painter.rect_filled(Rect::from_min_max(p(0.14, 0.18), p(0.26, 0.82)), CornerRadius::same(1), color);
            triangle(painter, Rect::from_min_max(p(0.28, 0.18), p(0.86, 0.82)), -1.0, color);
        }
        TransportIcon::Next => {
            triangle(painter, Rect::from_min_max(p(0.14, 0.18), p(0.72, 0.82)), 1.0, color);
            painter.rect_filled(Rect::from_min_max(p(0.74, 0.18), p(0.86, 0.82)), CornerRadius::same(1), color);
        }
        TransportIcon::Shuffle => {
            // Two paths crossing from left to right, each ending in an arrowhead.
            let head = s * 0.2;
            painter.add(Shape::line(vec![p(0.08, 0.28), p(0.3, 0.28), p(0.62, 0.72), p(0.78, 0.72)], thick));
            painter.add(Shape::line(vec![p(0.08, 0.72), p(0.3, 0.72), p(0.62, 0.28), p(0.78, 0.28)], thick));
            arrowhead(painter, p(0.94, 0.28), Vec2::X, head, color);
            arrowhead(painter, p(0.94, 0.72), Vec2::X, head, color);
        }
        TransportIcon::Repeat | TransportIcon::RepeatOne => {
            // A loop: the top run going right, the bottom run going left, each with its arrowhead.
            // Big heads and open corners, so it reads as a loop and not a box at a button's size.
            let head = s * 0.26;
            painter.add(Shape::line(vec![p(0.16, 0.6), p(0.16, 0.3), p(0.66, 0.3)], thick));
            painter.add(Shape::line(vec![p(0.84, 0.4), p(0.84, 0.7), p(0.34, 0.7)], thick));
            arrowhead(painter, p(0.92, 0.3), Vec2::X, head, color);
            arrowhead(painter, p(0.08, 0.7), -Vec2::X, head, color);
            if icon == TransportIcon::RepeatOne {
                // A small 1 inside the loop, drawn as strokes: its stem and its flag.
                let one = Stroke::new((s * 0.08).max(1.2), color);
                painter.line_segment([p(0.52, 0.4), p(0.52, 0.6)], one);
                painter.line_segment([p(0.52, 0.4), p(0.45, 0.46)], one);
            }
        }
        TransportIcon::UpNext => {
            // A list: three lines, a small play mark before the first (the item playing).
            triangle(painter, Rect::from_min_max(p(0.1, 0.18), p(0.28, 0.38)), 1.0, color);
            painter.line_segment([p(0.36, 0.28), p(0.9, 0.28)], thick);
            painter.line_segment([p(0.12, 0.52), p(0.9, 0.52)], thick);
            painter.line_segment([p(0.12, 0.76), p(0.9, 0.76)], thick);
        }
        TransportIcon::Heart | TransportIcon::HeartFilled => {
            // Two lobes and a point: an outline when not liked, filled when liked.
            let n = 28;
            let pts: Vec<Pos2> = (0..n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    // The classic heart curve, scaled into the square.
                    let x = 16.0 * t.sin().powi(3);
                    let y = 13.0 * t.cos() - 5.0 * (2.0 * t).cos() - 2.0 * (3.0 * t).cos() - (4.0 * t).cos();
                    Pos2::new(r.center().x + x / 34.0 * s, r.center().y + 0.04 * s - y / 34.0 * s)
                })
                .collect();
            if icon == TransportIcon::HeartFilled {
                // Not convex: filled as a fan of triangles from its centre, each convex.
                let c = Pos2::new(r.center().x, r.center().y + 0.02 * s);
                for i in 0..n {
                    let (a, b) = (pts[i], pts[(i + 1) % n]);
                    painter.add(Shape::convex_polygon(vec![c, a, b], color, Stroke::new(0.5_f32, color)));
                }
            } else {
                painter.add(Shape::closed_line(pts, Stroke::new((s * 0.09).max(1.3), color)));
            }
        }
        TransportIcon::Speaker | TransportIcon::SpeakerMuted => {
            painter.rect_filled(Rect::from_min_max(p(0.1, 0.38), p(0.26, 0.62)), CornerRadius::ZERO, color);
            painter.add(Shape::convex_polygon(vec![p(0.24, 0.38), p(0.5, 0.16), p(0.5, 0.84), p(0.24, 0.62)], color, Stroke::NONE));
            if icon == TransportIcon::SpeakerMuted {
                painter.line_segment([p(0.62, 0.36), p(0.9, 0.64)], thick);
                painter.line_segment([p(0.62, 0.64), p(0.9, 0.36)], thick);
            } else {
                for (k, rad) in [(0, 0.16f32), (1, 0.3)] {
                    let c = p(0.52, 0.5);
                    let m = 8;
                    let pts: Vec<Pos2> = (0..=m)
                        .map(|i| {
                            let a = (-50.0 + 100.0 * i as f32 / m as f32).to_radians();
                            c + Vec2::new(a.cos(), a.sin()) * (s * rad)
                        })
                        .collect();
                    let _ = k;
                    painter.add(Shape::line(pts, thick));
                }
            }
        }
    }
}

/// A square button carrying a drawn icon.
pub struct IconButton<'a> {
    icon: TransportIcon,
    id: egui::Id,
    name: &'a str,
    hint: Option<&'static str>,
    size: f32,
    primary: bool,
    active: bool,
    enabled: bool,
}

impl<'a> IconButton<'a> {
    /// A button drawing `icon`, named `name` (its words, said to assistive technology and shown on
    /// hover), its egui id from `id` (stable, so a test reads where it was drawn: `Context::read_response`).
    pub fn new(icon: TransportIcon, id: impl std::hash::Hash, name: &'a str) -> Self {
        IconButton { icon, id: egui::Id::new(("icon_button", id)), name, hint: None, size: 40.0, primary: false, active: false, enabled: true }
    }
    /// The words key of what it does, shown with its name.
    pub fn hint(mut self, key: &'static str) -> Self {
        self.hint = Some(key);
        self
    }
    /// Its side, points.
    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
    /// The main control (Play or Pause): filled in the accent.
    pub fn primary(mut self, primary: bool) -> Self {
        self.primary = primary;
        self
    }
    /// A toggle that is on (shuffle, repeat, the list open): outlined and drawn in the accent.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Draw it; its response (clicked when pressed with the pointer or the keyboard).
    pub fn show(self, ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::hover());
        let sense = if self.enabled { Sense::click() } else { Sense::hover() }; // hint-exempt: the caller gives the hint by key (`IconButton::hint`)
        let resp = ui.interact(rect, self.id, sense);
        let (enabled, name) = (self.enabled, self.name.to_string());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name.clone()));
        let hovered = resp.hovered() && self.enabled;
        let (fill, stroke, ink) = if self.primary {
            (if hovered { theme.accent_hover() } else { theme.accent() }, Stroke::NONE, theme.text_on_accent())
        } else if self.active {
            (theme.bg_tertiary(), Stroke::new(1.5_f32, theme.accent()), theme.accent())
        } else if !self.enabled {
            (Color32::TRANSPARENT, Stroke::NONE, theme.text_muted())
        } else {
            (if hovered { theme.bg_tertiary() } else { Color32::TRANSPARENT }, Stroke::new(1.0_f32, theme.border()), theme.text_primary())
        };
        let painter = ui.painter_at(rect.expand(2.0));
        let corner = if self.primary { CornerRadius::same((self.size * 0.5) as u8) } else { CornerRadius::same(theme.border_radius as u8) };
        painter.rect(rect, corner, fill, stroke, egui::StrokeKind::Inside);
        paint(&painter, rect.shrink(self.size * 0.26), self.icon, ink);
        // Its name, and what it does while hints are on, on hover and when the keyboard reaches it.
        let hint_words = self.hint.filter(|_| super::hint::shown(ui.ctx())).map(humanity_core::i18n::t_dyn);
        let tip = match hint_words {
            Some(h) => format!("{}\n{h}", self.name),
            None => self.name.to_string(),
        };
        let resp = resp.on_hover_text(tip.clone()).on_disabled_hover_text(tip.clone());
        super::hint::show_on_focus(&resp, &tip);
        super::focus_ring(ui, theme, &resp);
        resp
    }
}
