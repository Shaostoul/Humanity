//! The in-world notice banner (rung 2, part 2, 2026-10-31, docs/design/notifications.md section 3
//! item 4): a short banner at the top of the screen while the person plays, because a toast may
//! sit behind a full-screen game and a window in front gets no toast at all.
//!
//! Paint only: it is drawn inside the HUD's non-interactive layer (`pages/hud.rs`), under the
//! compass and the movement line, so it never takes a click from the world. It shows the notice's
//! own words (`notifications::decide::words`, as "What a notice shows" allows): the title, and the
//! message only when the person chose to see it. It stays [`crate::notifications::clicks::BANNER_SECS`]
//! seconds and fades over its last. Every colour is a theme token: the card behind it
//! (`bg_card`), its edge in `accent`, the bell in `accent`, the words in `text_primary` and
//! `text_secondary`.
//!
//! The website has no world, so it has no twin.

use egui::{Color32, FontId, Painter, Pos2, Rect, Rounding, Stroke, Vec2};

use crate::gui::theme::Theme;
use crate::notifications::clicks::Banner;

/// How wide the banner may grow, and how far below the top of the screen it sits (under the
/// compass at 20 and the movement line at 40, `pages/hud.rs`).
const MAX_WIDTH: f32 = 440.0;
const TOP: f32 = 64.0;

/// Paint `banner` at `opacity` (0 paints nothing), centred at the top of `screen`.
pub fn paint(painter: &Painter, theme: &Theme, screen: Rect, banner: &Banner, opacity: f32) {
    if opacity <= 0.0 || banner.title.is_empty() {
        return;
    }
    let fade = |c: Color32| c.gamma_multiply(opacity);
    let width = MAX_WIDTH.min(screen.width() - 32.0).max(120.0);
    let pad = 10.0;
    let bell = 18.0;
    let text_w = width - pad * 3.0 - bell;
    let title = painter.layout(banner.title.clone(), FontId::proportional(theme.font_size_body), fade(theme.text_primary()), text_w);
    let body = (!banner.body.is_empty()).then(|| painter.layout(banner.body.clone(), FontId::proportional(theme.font_size_small), fade(theme.text_secondary()), text_w));
    let text_h = title.size().y + body.as_ref().map_or(0.0, |b| b.size().y + 2.0);
    let height = text_h.max(bell) + pad * 2.0;
    let rect = Rect::from_min_size(Pos2::new(screen.center().x - width * 0.5, screen.top() + TOP), Vec2::new(width, height));
    let round = Rounding::same(theme.border_radius as u8);
    painter.rect(rect, round, fade(theme.bg_card()), Stroke::new(1.0, fade(theme.accent())), egui::StrokeKind::Inside);
    let bell_rect = Rect::from_min_size(Pos2::new(rect.left() + pad, rect.center().y - bell * 0.5), Vec2::splat(bell));
    crate::gui::widgets::icons::paint_bell(painter, bell_rect, fade(theme.accent()));
    let x = bell_rect.right() + pad;
    let mut y = rect.center().y - text_h * 0.5;
    let title_h = title.size().y;
    painter.galley(Pos2::new(x, y), title, fade(theme.text_primary()));
    y += title_h + 2.0;
    if let Some(b) = body {
        painter.galley(Pos2::new(x, y), b, fade(theme.text_secondary()));
    }
}
