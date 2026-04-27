//! AI subscription quota tracker + usage log (native).
//! Mirrors web/pages/ai-usage.html.
//!
//! Native version is documentation-first: walks the user through the schema
//! and points them at the web page (where the localStorage form lives).
//! Future: persist quotas + events to a local SQLite table so the native app
//! can render them too.

use egui::{RichText, ScrollArea};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self};
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, _state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "AI Usage Tracker");
                ui.label(
                    RichText::new(
                        "Manual log of AI subscription usage across providers (Claude, Claude Code, \
                         GPT, Gemini, Grok, ...). Helps you stay under quotas without surprise hits.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                widgets::card_with_header(ui, theme, "What it tracks", |ui| {
                    let rows = [
                        "Quotas per (provider, window): hourly / 5h / daily / weekly / monthly.",
                        "Usage events: provider, model, input/output tokens, free-text notes.",
                        "Color-coded meter: green <70%, amber 70\u{2013}90%, red \u{2265}90%.",
                        "Last-50 events table for recent activity.",
                    ];
                    for r in rows {
                        ui.label(RichText::new(format!("\u{2022} {}", r)).color(theme.text_secondary()).size(theme.font_size_small));
                    }
                });

                ui.add_space(theme.spacing_sm);

                widgets::card_with_header(ui, theme, "Why manual?", |ui| {
                    ui.label(
                        RichText::new(
                            "Claude Code's context-window panel only surfaces its own state, and \
                             external providers don't offer a unified usage API. Logging is manual \
                             because the data lives in many disconnected dashboards. Future: an \
                             optional API webhook so providers that do expose usage can post here.",
                        )
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                    );
                });

                ui.add_space(theme.spacing_sm);

                widgets::card_with_header(ui, theme, "Storage scope", |ui| {
                    ui.label(
                        RichText::new(
                            "Web: localStorage in your browser. Native: not yet persisted \u{2014} \
                             the web page is the canonical entry point. A native SQLite-backed \
                             store is a follow-up if you want desktop-only tracking.",
                        )
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                    );
                });

                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Same view (with the actual form) on the web: united-humanity.us/ai-usage")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });
}
