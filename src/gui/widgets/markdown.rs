//! Minimal markdown reader shared by the in-app doc surfaces: the Humanity Accord
//! viewer modal (`pages/humanity.rs`) and the Library doc pane (`pages/library.rs`).
//!
//! Renders headings (`#`, `##`, `###`), bullets (`-`, `*`), horizontal rules
//! (`---`), and paragraphs; inline emphasis markers (`**`, `*`, `` ` ``) are
//! stripped for plain, readable text styled by the theme. Not a full parser, just
//! enough to read a document cleanly. The bullet glyph is U+00B7 ("·"), a
//! confirmed-rendering symbol in the bundled font.

use egui::RichText;
use crate::gui::theme::Theme;

/// Render `md` as themed, readable text into `ui`.
pub fn render_markdown(ui: &mut egui::Ui, theme: &Theme, md: &str) {
    render_markdown_impl(ui, theme, md, false, &mut None);
}

/// Render `md` with DEFINE MODE on (v0.989, operator: "clicking a button to
/// the click on a word to bring up the definition"): body and bullet text
/// lay out word by word, every word is clickable, and words the dictionary
/// knows are underlined in the accent color. A click stores
/// `(word, Some(entry))` on a hit or `(word, None)` on a miss into
/// `clicked` - the caller owns the popup. Headings stay plain labels (they
/// are titles, not prose). Costs more layout than the plain path, which is
/// why it only runs while the reader has the toggle on.
pub fn render_markdown_defining(
    ui: &mut egui::Ui,
    theme: &Theme,
    md: &str,
    clicked: &mut Option<String>,
) {
    let mut c = Some(clicked);
    render_markdown_impl(ui, theme, md, true, &mut c);
}

fn render_markdown_impl(
    ui: &mut egui::Ui,
    theme: &Theme,
    md: &str,
    define: bool,
    clicked: &mut Option<&mut Option<String>>,
) {
    for raw in md.lines() {
        let trimmed = raw.trim_start();
        if trimmed.is_empty() {
            ui.add_space(theme.spacing_sm);
            continue;
        }
        if trimmed.starts_with("---") && trimmed.chars().all(|c| c == '-') {
            ui.separator();
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            ui.add_space(theme.spacing_xs);
            ui.label(RichText::new(strip_md(rest)).size(theme.font_size_body).strong().color(theme.accent()));
        } else if let Some(rest) = trimmed.strip_prefix("## ") {
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(strip_md(rest)).size(theme.font_size_heading).strong().color(theme.text_primary()));
        } else if let Some(rest) = trimmed.strip_prefix("# ") {
            ui.add_space(theme.spacing_sm);
            ui.label(RichText::new(strip_md(rest)).size(theme.font_size_title).strong().color(theme.text_primary()));
        } else if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            ui.horizontal_top(|ui| {
                ui.add_space(theme.spacing_sm);
                ui.label(RichText::new("\u{00b7}").color(theme.accent()));
                if define {
                    defining_words(ui, theme, &strip_md(rest), theme.font_size_small, clicked);
                } else {
                    ui.label(RichText::new(strip_md(rest)).size(theme.font_size_small).color(theme.text_secondary()));
                }
            });
        } else if define {
            defining_words(ui, theme, &strip_md(trimmed), theme.font_size_small, clicked);
        } else {
            ui.label(RichText::new(strip_md(trimmed)).size(theme.font_size_small).color(theme.text_secondary()));
        }
    }
}

/// One prose line as clickable words, wrapping like normal text. Dictionary
/// hits are underlined accent; misses stay body-colored but still click
/// (the popup then says the word isn't defined yet - honest, not silent).
fn defining_words(
    ui: &mut egui::Ui,
    theme: &Theme,
    text: &str,
    size: f32,
    clicked: &mut Option<&mut Option<String>>,
) {
    let gl = crate::gui::glossary::glossary();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for word in text.split_whitespace() {
            let known = gl.lookup_word(word).is_some();
            let rt = if known {
                RichText::new(word).size(size).color(theme.accent()).underline()
            } else {
                RichText::new(word).size(size).color(theme.text_secondary())
            };
            let resp = ui.add(egui::Label::new(rt).sense(egui::Sense::click()));
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                if let Some(slot) = clicked.as_deref_mut() {
                    *slot = Some(word.to_string());
                }
            }
        }
    });
}

/// Strip the common inline markdown markers so text reads cleanly as plain text.
pub fn strip_md(s: &str) -> String {
    s.replace("**", "").replace('`', "").replace('*', "")
}
