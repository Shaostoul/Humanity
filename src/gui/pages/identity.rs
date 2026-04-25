//! Identity hub page: DID, Verifiable Credentials, trust score, AI status.
//! Mirrors web/pages/identity.html.
//!
//! For the v1 native shipment, this is a read-only viewer. Editing/credential
//! issuance flows live in their respective screens (Profile for self-issued
//! VCs, Settings for Recovery, etc.). This page consolidates the read view.

use egui::{RichText, ScrollArea};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize};
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "Identity & Credentials");
                ui.label(
                    RichText::new(
                        "Look up any DID. See its current key, trust score breakdown, every \
                         Verifiable Credential issued to it, and its AI-status declaration. \
                         Inputs always exposed \u{2014} no black-box reputation.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                // DID lookup row
                ui.horizontal(|ui| {
                    ui.label(RichText::new("DID:").color(theme.text_secondary()));
                    ui.add_sized(
                        [ui.available_width() - 120.0, theme.input_height],
                        egui::TextEdit::singleline(&mut state.identity_lookup_did)
                            .hint_text("did:hum:..."),
                    );
                    if Button::primary("Look up").show(ui, theme) {
                        state.identity_lookup_pending = true;
                    }
                });

                ui.add_space(theme.spacing_md);

                if state.identity_lookup_did.is_empty() {
                    ui.label(
                        RichText::new(
                            "Enter a DID above to see its identity card. \
                             A DID looks like did:hum:<22-char base58>.",
                        )
                        .color(theme.text_muted()),
                    );
                    return;
                }

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("DID Resolution").color(theme.accent()).strong().size(theme.font_size_heading),
                    );
                    ui.add_space(theme.spacing_xs);
                    widgets::detail_row(ui, theme, "DID", &state.identity_lookup_did);
                    widgets::detail_row(ui, theme, "Crypto suite", "ml-dsa-65");
                    widgets::detail_row(
                        ui,
                        theme,
                        "Server endpoint",
                        "GET /api/v2/did/{did}",
                    );
                });

                ui.add_space(theme.spacing_sm);

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Trust Score").color(theme.accent()).strong().size(theme.font_size_heading),
                    );
                    ui.label(
                        RichText::new(
                            "Aggregate of: VCs received, vouching graph entropy, activity \
                             diversity (last 90d), account age, economic stake, legacy reputation. \
                             Capped at 0.95 for governance vote weighting (Accord power-asymmetry).",
                        )
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                    );
                    ui.add_space(theme.spacing_xs);
                    widgets::detail_row(
                        ui,
                        theme,
                        "Server endpoint",
                        "GET /api/v2/trust/{did}",
                    );
                });

                ui.add_space(theme.spacing_sm);

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Verifiable Credentials").color(theme.accent()).strong().size(theme.font_size_heading),
                    );
                    ui.label(
                        RichText::new(
                            "Every VC about this DID across the federated network. Issuer-auth-checked \
                             revocation. Subject-auth-checked withdrawal (Accord consent).",
                        )
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                    );
                    ui.add_space(theme.spacing_xs);
                    widgets::detail_row(
                        ui,
                        theme,
                        "Server endpoint",
                        "GET /api/v2/credentials?subject={did}",
                    );
                });

                ui.add_space(theme.spacing_sm);

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("AI / Subject Class").color(theme.accent()).strong().size(theme.font_size_heading),
                    );
                    ui.label(
                        RichText::new(
                            "human / ai_agent / institution. AI agents must declare via subject_class_v1 \
                             AND post a controlled_by_v1 binding to a human operator. AI excluded from \
                             governance voting per Accord (votes require sentient consent).",
                        )
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                    );
                    ui.add_space(theme.spacing_xs);
                    widgets::detail_row(
                        ui,
                        theme,
                        "Server endpoint",
                        "GET /api/v2/ai-status/{did}",
                    );
                });

                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Same view on the web: united-humanity.us/identity")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });
}
