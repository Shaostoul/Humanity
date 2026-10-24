//! Characters past a field's limit, shown as they are typed: the part within the limit as the box
//! draws it, the rest in the danger colour on the danger background (theme tokens `danger` and
//! `danger_bg`), and a "312 / 280" count. A universal widget (2026-10-24): the chat's message box
//! (pages/chat/line_limit.rs) and the Profile page's bio use it, and the website's twin is
//! web/shared/over-limit.js and .css.
//!
//! Operator, 2026-10-24: "highlight the character spots that go beyond the character limit have
//! their background highlight dark red and the text change from white to red. We need to improve
//! the visual ques for some errors." A soft limit, not egui's `char_limit`: that stops typing at the
//! limit and cuts a longer paste without a word, where this shows what is over and lets the person
//! choose what to cut. Counted in characters, as the servers count them.

use std::sync::Arc;

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId};

use crate::gui::theme::Theme;

/// Where the characters past `limit` begin, as a byte index into `text`, or None when it is within
/// the limit. Counted in characters, as the relay counts them (`content.chars().count()`).
pub fn over_from(text: &str, limit: usize) -> Option<usize> {
    text.char_indices().nth(limit).map(|(i, _)| i)
}

/// A box's words laid out with every character past `limit` in the danger colour on the danger
/// background, the rest as the box draws them.
pub fn layout_job(text: &str, limit: usize, font_id: FontId, color: Color32, theme: &Theme, wrap_width: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    let cut = over_from(text, limit).unwrap_or(text.len());
    job.append(&text[..cut], 0.0, TextFormat::simple(font_id.clone(), color));
    if cut < text.len() {
        job.append(
            &text[cut..],
            0.0,
            TextFormat { font_id, color: theme.danger(), background: theme.danger_bg(), ..Default::default() },
        );
    }
    job.wrap.max_width = wrap_width;
    job
}

/// A `TextEdit` layouter that shows the characters past `limit` (`layout_job`), with the font and
/// colour the box would use by itself. `one_line`: a single-line box, which never wraps, as egui's
/// own single-line layout does not.
pub fn layouter(theme: &Theme, limit: usize, one_line: bool) -> impl FnMut(&egui::Ui, &str, f32) -> Arc<egui::Galley> + '_ {
    move |ui: &egui::Ui, text: &str, wrap_width: f32| {
        let font_id = egui::FontSelection::default().resolve(ui.style());
        let color = ui.visuals().override_text_color.unwrap_or_else(|| ui.visuals().widgets.inactive.text_color());
        let mut job = layout_job(text, limit, font_id, color, theme, if one_line { f32::INFINITY } else { wrap_width });
        job.break_on_newline = !one_line;
        ui.fonts(|f| f.layout_job(job))
    }
}

/// The count beside the box, "312 / 280", and its colour: the danger colour past the limit, the
/// warning colour within `warn_within` of it, muted otherwise.
pub fn count(len: usize, limit: usize, warn_within: usize, theme: &Theme) -> (String, Color32) {
    let color = if len > limit {
        theme.danger()
    } else if len + warn_within > limit {
        theme.warning()
    } else {
        theme.text_muted()
    };
    (format!("{len} / {limit}"), color)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_characters_past_the_limit_are_counted_as_the_server_counts_them() {
        // Characters, never bytes: an emoji is one character to the relay.
        assert_eq!(over_from("ab\u{1F600}cd", 3), Some("ab\u{1F600}".len()));
        assert_eq!(over_from("abc", 3), None, "exactly at the limit is within it");
        assert_eq!(over_from("", 0), None);
    }

    #[test]
    fn the_part_past_the_limit_is_drawn_in_red_on_dark_red_and_the_rest_as_usual() {
        let theme = crate::gui::theme::load_theme();
        let font = FontId::proportional(14.0);
        let job = layout_job("hello world", 5, font, Color32::WHITE, &theme, 300.0);
        assert_eq!(job.sections.len(), 2);
        let (within, past) = (&job.sections[0], &job.sections[1]);
        assert_eq!(&job.text[within.byte_range.clone()], "hello");
        assert_eq!(within.format.color, Color32::WHITE);
        assert_eq!(within.format.background, Color32::TRANSPARENT);
        assert_eq!(&job.text[past.byte_range.clone()], " world");
        assert_eq!(past.format.color, theme.danger());
        assert_eq!(past.format.background, theme.danger_bg());
        assert_eq!(job.wrap.max_width, 300.0);
        // Within the limit: one plain section, nothing red.
        let plain = layout_job("hi", 5, FontId::proportional(14.0), Color32::WHITE, &theme, 300.0);
        assert_eq!(plain.sections.len(), 1);
        assert_eq!(plain.sections[0].format.background, Color32::TRANSPARENT);
    }

    #[test]
    fn a_one_line_box_stays_on_one_line_and_a_page_box_wraps() {
        let theme = crate::gui::theme::load_theme();
        let ctx = egui::Context::default();
        let text = "a line long enough to wrap in a narrow box, and past the limit";
        let mut rows = (0, 0);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                rows.0 = layouter(&theme, 10, true)(ui, text, 60.0).rows.len();
                rows.1 = layouter(&theme, 10, false)(ui, text, 60.0).rows.len();
            });
        });
        assert_eq!(rows.0, 1, "a single-line box never wraps");
        assert!(rows.1 > 1, "a page's box wraps at its width ({} rows)", rows.1);
    }

    #[test]
    fn the_count_turns_amber_near_the_limit_and_red_past_it() {
        let theme = crate::gui::theme::load_theme();
        assert_eq!(count(100, 280, 40, &theme), ("100 / 280".to_string(), theme.text_muted()));
        assert_eq!(count(250, 280, 40, &theme).1, theme.warning());
        assert_eq!(count(292, 280, 40, &theme), ("292 / 280".to_string(), theme.danger()));
    }
}
