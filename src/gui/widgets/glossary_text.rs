//! Words with glossary terms in them (the ELI5 mandate, "the glossary on every
//! page", 2026-10-22): the desktop twin of the website's `[data-term]` overlay
//! (`web/shared/glossary.js`).
//!
//! A word a newcomer may not know is marked in its string in `data/i18n/`:
//! `[[relay]]`, or `[[relays|relay]]` when the shown word differs from the
//! glossary entry. This draws the string as one wrapping run of text, the marked
//! words underlined in the accent colour, and resting the pointer on one shows
//! its definition from `data/glossary.json` (the term, its category, the plain
//! definition and where to read more), the same card the website shows on a
//! click. A mark whose term the glossary lacks draws as plain text, and
//! `scripts/tests/glossary-terms.test.js` fails on it.
//!
//! ```ignore
//! widgets::glossary_text(ui, theme, "chat.relay_note");   // body text by key
//! widgets::glossary_words(ui, theme, marked, size, color); // any marked string
//! ```

use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Response, Sense, Stroke, Ui};

use crate::i18n::{self, Segment};
use crate::gui::theme::Theme;

/// Body text by key, its glossary marks underlined and explained on hover.
pub fn glossary_text(ui: &mut Ui, theme: &Theme, key: &'static str) -> Response {
    glossary_words(ui, theme, i18n::t_marked(key), theme.font_size_body, theme.text_primary())
}

/// Small muted text by key (a hint or a note line), with its glossary marks.
pub fn glossary_note(ui: &mut Ui, theme: &Theme, key: &'static str) -> Response {
    glossary_words(ui, theme, i18n::t_marked(key), theme.font_size_small, theme.text_muted())
}

/// Where each marked term sits in the laid-out text, by character index.
struct TermSpan {
    start: usize,
    end: usize,
    term: String,
}

/// Draw a marked string (see the module doc) at `size` in `color`.
pub fn glossary_words(ui: &mut Ui, theme: &Theme, marked: &str, size: f32, color: Color32) -> Response {
    let gl = crate::gui::glossary::glossary();
    let font = FontId::proportional(size);
    let mut job = LayoutJob::default();
    job.wrap.max_width = ui.available_width().max(40.0);
    let mut spans: Vec<TermSpan> = Vec::new();
    let mut at = 0usize;
    let mut plain = String::new();
    for seg in i18n::segments(marked) {
        match seg {
            Segment::Text(s) => {
                job.append(s, 0.0, TextFormat { font_id: font.clone(), color, ..Default::default() });
                at += s.chars().count();
                plain.push_str(s);
            }
            Segment::Term { shown, term } => {
                let n = shown.chars().count();
                let known = gl.lookup(term).is_some();
                let format = if known {
                    TextFormat {
                        font_id: font.clone(),
                        color: theme.accent(),
                        underline: Stroke::new(1.0, theme.accent()),
                        ..Default::default()
                    }
                } else {
                    TextFormat { font_id: font.clone(), color, ..Default::default() }
                };
                job.append(shown, 0.0, format);
                if known {
                    spans.push(TermSpan { start: at, end: at + n, term: term.to_string() });
                }
                at += n;
                plain.push_str(shown);
            }
        }
    }
    let galley = ui.fonts(|f| f.layout_job(job));
    let (rect, resp) = ui.allocate_exact_size(galley.size(), Sense::hover());
    // A screen reader reads the words, without the marks.
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, plain.clone()));
    if ui.is_rect_visible(rect) {
        ui.painter().galley(rect.min, galley.clone(), color);
    }
    if let Some(pos) = resp.hover_pos() {
        let rel = pos - rect.min;
        let cursor = galley.cursor_from_pos(rel);
        let left = galley.pos_from_cursor(&cursor).min.x;
        let mut idx = cursor.ccursor.index;
        if rel.x < left && idx > 0 {
            idx -= 1;
        }
        if let Some(span) = spans.iter().find(|s| idx >= s.start && idx < s.end) {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Help);
            if let Some(entry) = gl.lookup(&span.term) {
                let category = gl.category_name(&entry.category).unwrap_or("").to_string();
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), resp.id.with(("term", span.start)), |ui| {
                    definition_card(ui, theme, &entry.term, &category, &entry.definition, &entry.link);
                });
            }
        }
    }
    resp
}

/// The definition card: the term, its category, the definition, and where to
/// read more. Shared with anything else that shows a glossary entry.
pub fn definition_card(ui: &mut Ui, theme: &Theme, term: &str, category: &str, definition: &str, link: &str) {
    ui.set_max_width(360.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(term).strong().color(theme.accent()));
        if !category.is_empty() {
            ui.label(egui::RichText::new(category).size(theme.font_size_small).color(theme.text_muted()));
        }
    });
    ui.add_space(theme.spacing_xs);
    ui.label(egui::RichText::new(definition).color(theme.text_primary()));
    if !link.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(
            egui::RichText::new(i18n::tf("glossary.learn_more_at", &[("link", &link)]))
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(ctx: &egui::Context, pos: Option<egui::Pos2>, marked: &str, out: &mut egui::Rect) {
        let theme = crate::gui::theme::load_theme();
        let mut input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(600.0, 200.0))),
            ..Default::default()
        };
        if let Some(p) = pos {
            input.events.push(egui::Event::PointerMoved(p));
        }
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                *out = glossary_words(ui, &theme, marked, 14.0, Color32::WHITE).rect;
            });
        });
    }

    fn tooltip_up(ctx: &egui::Context) -> bool {
        ctx.memory(|m| m.areas().visible_layer_ids().iter().any(|l| l.order == egui::Order::Tooltip))
    }

    /// Resting on the marked word shows its definition; resting on plain words
    /// beside it shows nothing. "relay" is in the shipped glossary.
    #[test]
    fn a_marked_term_shows_its_definition_and_plain_words_do_not() {
        let marked = "Messages go to a [[relay]] first";
        let ctx = egui::Context::default();
        ctx.style_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
            s.interaction.show_tooltips_only_when_still = false;
        });
        let mut rect = egui::Rect::NOTHING;
        run(&ctx, None, marked, &mut rect);
        // Where "relay" sits: lay the prefix out to measure it.
        let prefix_w = ctx.fonts(|f| f.layout_no_wrap("Messages go to a ".into(), FontId::proportional(14.0), Color32::WHITE).size().x);
        let on_term = egui::pos2(rect.min.x + prefix_w + 8.0, rect.center().y);
        for _ in 0..3 {
            run(&ctx, Some(on_term), marked, &mut rect);
        }
        assert!(tooltip_up(&ctx), "the definition shows on the marked word");

        let ctx2 = egui::Context::default();
        ctx2.style_mut(|s| {
            s.interaction.tooltip_delay = 0.0;
            s.interaction.show_tooltips_only_when_still = false;
        });
        let on_plain = egui::pos2(rect.min.x + 6.0, rect.center().y);
        for _ in 0..3 {
            run(&ctx2, Some(on_plain), marked, &mut rect);
        }
        assert!(!tooltip_up(&ctx2), "plain words show nothing");
    }
}
