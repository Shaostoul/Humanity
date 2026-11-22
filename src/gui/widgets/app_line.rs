//! The app's own voice in the chat (the owner branch's seventh review, H1 (a), 2026-11-21).
//!
//! A line the app itself wrote (`ChatMessage::origin` the App origin, set only by local code)
//! is drawn HERE, with the app's mark: an accent bar down the row's left edge and a painted diamond
//! where a member's letter avatar would sit, the words in the muted italic of the app's notes, and
//! no sender name at all. Every row from the network goes through `widgets::row::message_row`
//! instead (a letter avatar and the name the frame claimed), so a frame that names itself "System"
//! or "HumanityOS" is drawn as someone's words, never like this. Nothing here is a glyph or a name a
//! frame could imitate: the mark is geometry.
//!
//! The website's twin is the `.message.system.app-line` class (web/chat/messages.css), drawn by
//! app.js `addSystemMessage`; the server's own lines are `.server-line` there and the sender
//! "From this server" here.

use egui::{Color32, CornerRadius, Sense, Shape, Stroke, Vec2};

use crate::gui::theme::Theme;

/// Draw one app line, `text`, across the available width on `bg`. Returns the row's response.
pub fn draw(ui: &mut egui::Ui, theme: &Theme, text: &str, bg: Color32) -> egui::Response {
    let full_width = ui.available_width();
    let content_left = theme.avatar_size + theme.avatar_gap;
    let content_width = (full_width - content_left - 4.0).max(100.0);
    let mut job = egui::text::LayoutJob::default();
    job.wrap.max_width = content_width;
    job.append(
        text,
        0.0,
        egui::TextFormat { font_id: egui::FontId::proportional(theme.body_size), color: theme.text_secondary(), italics: true, ..Default::default() },
    );
    let galley = ui.painter().layout_job(job);
    let row_h = (galley.size().y + 6.0).max(theme.avatar_size * 0.75);
    // Words only, nothing to press: the row senses hover alone.
    let (rect, response) = ui.allocate_exact_size(Vec2::new(full_width, row_h), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(4), bg);
    // The app's mark: an accent bar on the left edge ...
    let bar = egui::Rect::from_min_size(rect.min, Vec2::new(3.0, rect.height()));
    painter.rect_filled(bar, CornerRadius::ZERO, theme.accent());
    // ... and a diamond where a member's letter avatar sits, drawn as geometry.
    let c = egui::pos2(rect.min.x + theme.avatar_size * 0.5, rect.min.y + row_h.min(theme.avatar_size) * 0.5);
    let r = theme.avatar_size * 0.22;
    let diamond = vec![egui::pos2(c.x, c.y - r), egui::pos2(c.x + r, c.y), egui::pos2(c.x, c.y + r), egui::pos2(c.x - r, c.y)];
    painter.add(Shape::convex_polygon(diamond, theme.accent(), Stroke::NONE));
    painter.galley(egui::pos2(rect.min.x + content_left, rect.min.y + 3.0), galley, theme.text_secondary());
    response
}
