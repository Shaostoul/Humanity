//! The notice bell (rung 2, 2026-10-31, docs/design/notifications.md section 3): a square button
//! with a bell and, when some notices are not opened yet, their count in a small pill at its
//! top-right corner. The header's in-app list opens from it (`gui/notice_panel.rs`).
//!
//! The website's twin is the `.notice-bell` / `.notice-count` classes (web/chat/layout.css), drawn
//! by web/chat/chat-notify.js in the chat page's header. Every colour is a theme token: the button
//! as the help button beside it (`bg_secondary`, `border`, `accent` while its panel is open), the
//! pill in `danger` with `Theme::ink_on` for its digits, as the Chat tab's unread dot.
//!
//! Per frame it paints a few shapes and one short number; it reads the count it is given (the
//! list keeps it, `notifications::list::List::unseen`), never the list.

use egui::{Color32, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets::Hint;

/// What the pill shows for `unseen` notices: nothing for none, the number up to 99, then "99+".
pub fn count_text(unseen: usize) -> Option<String> {
    match unseen {
        0 => None,
        1..=99 => Some(unseen.to_string()),
        _ => Some("99+".to_string()),
    }
}

/// Draw the bell, `size` square, with `unseen` in its pill and `open` while its panel shows.
/// The response is the button's, carrying its plain-language hint: click to open or close the
/// panel. `label` is what a screen reader says (the bell's name and, with notices waiting, how
/// many).
pub fn notice_bell(ui: &mut Ui, theme: &Theme, size: f32, unseen: usize, open: bool, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let resp = resp.hint("notifications.bell_hint");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if !ui.is_rect_visible(rect) {
        return resp;
    }
    let (fill, ink) = if open {
        (theme.accent(), theme.text_on_accent())
    } else if resp.hovered() {
        (theme.bg_secondary(), theme.text_primary())
    } else {
        (theme.bg_secondary(), theme.text_muted())
    };
    let p = ui.painter();
    p.rect(rect, egui::Rounding::same(theme.border_radius as u8), fill, Stroke::new(1.0, theme.border()), egui::StrokeKind::Inside);
    crate::gui::widgets::icons::paint_bell(p, rect.shrink(size * 0.22), ink);
    if let Some(text) = count_text(unseen) {
        paint_count(ui, theme, rect, &text);
    }
    resp
}

/// The count's pill over the bell's top-right corner, in the unread colour.
fn paint_count(ui: &Ui, theme: &Theme, bell: Rect, text: &str) {
    let fill: Color32 = theme.danger();
    let font = egui::FontId::proportional(theme.font_size_small * 0.85);
    let galley = ui.painter().layout_no_wrap(text.to_string(), font, Theme::ink_on(fill));
    let h = galley.size().y.max(11.0);
    let w = (galley.size().x + 6.0).max(h);
    let pill = Rect::from_center_size(egui::pos2(bell.right() - w * 0.25, bell.top() + h * 0.25), Vec2::new(w, h));
    ui.painter().rect_filled(pill, egui::Rounding::same((h * 0.5) as u8), fill);
    ui.painter().galley(pill.center() - galley.size() * 0.5, galley, Theme::ink_on(fill));
}

/// The words a screen reader gets for the bell: its name, and how many are waiting.
pub fn label_for(unseen: usize) -> String {
    let name = crate::i18n::t("notifications.bell");
    match count_text(unseen) {
        None => name.to_string(),
        Some(n) => format!("{name}: {}", crate::i18n::tf("notifications.bell_count", &[("count", &n)])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The pill shows nothing for none, the number to 99, then "99+" (a pill never grows past
    /// three characters). Seen red 2026-10-31 with the 99 bound off by one: 100 showed "100".
    #[test]
    fn the_count_pill_reads_nothing_a_number_or_99_plus() {
        assert_eq!(count_text(0), None);
        assert_eq!(count_text(1).as_deref(), Some("1"));
        assert_eq!(count_text(99).as_deref(), Some("99"));
        assert_eq!(count_text(100).as_deref(), Some("99+"));
    }
}
