//! The reaction buttons' emoji and the pack's credit (2026-11-20, docs/design/emoji.md rungs 3 and 4).
//!
//! egui's own button lays out its text inside itself, where `inline::layout` cannot reach, so a
//! reaction (the quick row beside the Þ and the ∞ panel, `pages::chat::rows`) is this button: framed
//! and coloured exactly as `egui::Button` is (the style's widget visuals for its hover and press),
//! with its emoji laid out through `inline::layout`, so it shows the pack's picture when one is
//! installed and the font's glyph otherwise. The website's twin is `web/chat/view/pillPopup.js`.

use egui::{CornerRadius, FontId, Response, Sense, Ui, Vec2};
use humanity_core::i18n::t;

use crate::gui::theme::Theme;

use super::inline::{layout_singleline, paint};

/// A button showing one emoji in `font`, at least `min` points big, rounded `corner` (None: as the
/// style rounds every button). Its label for screen readers is the emoji itself.
pub fn button(ui: &mut Ui, text: &str, font: FontId, min: Vec2, corner: Option<CornerRadius>) -> Response {
    let color = ui.visuals().text_color();
    let (galley, pictures) = layout_singleline(ui.ctx(), text.to_string(), font, color);
    let size = (galley.size() + 2.0 * ui.spacing().button_padding).max(min);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click()); // hint-exempt: a reaction is its own explanation (the emoji), and the website's reaction buttons show no hint either (pillPopup.js `button`)
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), text));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact(&response);
        ui.painter().rect(rect.expand(visuals.expansion), corner.unwrap_or(visuals.corner_radius), visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        let origin = rect.center() - galley.size() / 2.0;
        ui.painter().galley(origin, galley.clone(), visuals.text_color());
        paint(ui.painter(), origin, &galley, &pictures);
    }
    response
}

/// One piece of the credit line: words, or the name of a link (`{{openmoji}}`, `{{licence}}`).
#[derive(Debug, PartialEq, Eq)]
pub enum CreditPart<'a> {
    Words(&'a str),
    Link(&'a str),
}

/// `words` cut at each `{{name}}` (as the website's `emojiCredit` splits `emoji.credit`).
pub fn credit_parts(words: &str) -> Vec<CreditPart<'_>> {
    let mut out = Vec::new();
    let mut rest = words;
    while let Some(open) = rest.find("{{") {
        let Some(close) = rest[open..].find("}}").map(|c| open + c) else { break };
        let name = &rest[open + 2..close];
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            break;
        }
        if open > 0 {
            out.push(CreditPart::Words(&rest[..open]));
        }
        out.push(CreditPart::Link(name));
        rest = &rest[close + 2..];
    }
    if !rest.is_empty() {
        out.push(CreditPart::Words(rest));
    }
    out
}

/// Where the credit's two links go (the website's `EMOJI_CREDIT_LINKS`, web/chat/view/pillPopup.js).
const OPENMOJI_URL: &str = "https://openmoji.org/";
const LICENCE_URL: &str = "https://creativecommons.org/licenses/by-sa/4.0/#";

/// OpenMoji's credit, small and muted, at the foot of the ∞ panel where its pictures are picked: the
/// attribution OpenMoji asks for, word for word, from `emoji.credit` with its two links
/// (docs/reference/findings/2026-11-20-openmoji.md). Drawn only while the pack's pictures are
/// installed (`inline::pack_installed`): system-font emoji are not OpenMoji's and owe no credit.
pub fn credit(ui: &mut Ui, theme: &Theme) {
    let size = theme.font_size_small;
    let muted = theme.text_muted();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for part in credit_parts(t("emoji.credit")) {
            match part {
                CreditPart::Words(w) => {
                    ui.label(egui::RichText::new(w).size(size).color(muted));
                }
                CreditPart::Link("openmoji") => {
                    ui.hyperlink_to(egui::RichText::new(t("emoji.credit_openmoji")).size(size), OPENMOJI_URL).on_hover_text(t("emoji.credit_openmoji_hint"));
                }
                CreditPart::Link("licence") => {
                    ui.hyperlink_to(egui::RichText::new(t("emoji.credit_licence")).size(size), LICENCE_URL).on_hover_text(t("emoji.credit_licence_hint"));
                }
                CreditPart::Link(other) => {
                    ui.label(egui::RichText::new(other).size(size).color(muted));
                }
            }
        }
    });
}
