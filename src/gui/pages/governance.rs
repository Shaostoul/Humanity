//! Governance page: proposals + votes + tally.
//! Mirrors web/pages/governance.html.

use egui::{RichText, ScrollArea};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize};
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "Civic Participation");
                ui.label(
                    RichText::new(
                        "Local-scope proposals run on this server. Civilization-scope proposals \
                         (Accord amendments, federation floor policy) federate to all servers and \
                         need federation-wide quorum to pass.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                widgets::card_with_header(ui, theme, "Accord power-asymmetry mitigation", |ui| {
                    ui.label(
                        RichText::new(
                            "Vote weight equals your trust score at vote time, capped at 0.95. \
                             No single high-trust voter can dominate. AI agents are excluded from \
                             voting per Accord \u{2014} votes require sentient consent.",
                        )
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                    );
                });

                ui.add_space(theme.spacing_md);

                // Scope tabs (using the unified tab_bar that shares Button styling)
                let scopes = ["All scopes", "Local (this server)", "Civilization-wide"];
                widgets::tab_bar(ui, theme, &scopes, &mut state.governance_scope_tab);

                ui.add_space(theme.spacing_sm);

                // Open / All filter pills
                let filters = ["Open", "All"];
                widgets::tab_bar(ui, theme, &filters, &mut state.governance_filter_tab);

                ui.add_space(theme.spacing_md);

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Proposal feed")
                            .color(theme.accent())
                            .strong()
                            .size(theme.font_size_heading),
                    );
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new(
                            "Live proposals fetched from this server's GET /api/v2/proposals \
                             feed. Civilization-scope items federate via SignedObjectGossip. \
                             Cast a vote by posting a vote_v1 signed_object referencing the \
                             proposal_object_id.",
                        )
                        .color(theme.text_muted()),
                    );
                });

                ui.add_space(theme.spacing_md);

                ui.label(
                    RichText::new("Same view on the web: united-humanity.us/governance \u{2014} API: GET /api/v2/proposals")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });
}
