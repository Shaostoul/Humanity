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
                ui.label(RichText::new(strip_md(rest)).size(theme.font_size_small).color(theme.text_secondary()));
            });
        } else {
            ui.label(RichText::new(strip_md(trimmed)).size(theme.font_size_small).color(theme.text_secondary()));
        }
    }
}

/// Strip the common inline markdown markers so text reads cleanly as plain text.
pub fn strip_md(s: &str) -> String {
    s.replace("**", "").replace('`', "").replace('*', "")
}
