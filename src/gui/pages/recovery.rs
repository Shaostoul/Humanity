//! Social key recovery page: guardian setup, share storage, active requests.
//! Mirrors web/pages/recovery.html.

use egui::{RichText, ScrollArea};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize};
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "Social Key Recovery");
                ui.label(
                    RichText::new(
                        "Lose your device, recover your identity. Your BIP39 seed splits via Shamir \
                         secret sharing across trusted guardians, encrypted to each guardian's Kyber768 \
                         key. The relay stores only opaque ciphertext.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                widgets::card_with_header(ui, theme, "How it works", |ui| {
                    let steps = [
                        "1. Pick N trusted guardians and a threshold M (e.g., 3 of 5).",
                        "2. Your client splits your seed into N Shamir shares, encrypts each to a guardian's Kyber768 pubkey, posts recovery_share_v1 signed_objects.",
                        "3. If you lose your device, generate a new Dilithium3 keypair and post recovery_request_v1 signed by the new key.",
                        "4. Guardians review the request and post recovery_approval_v1. When M approvals arrive, request status flips to ready.",
                        "5. Your client collects the M decrypted shares (out-of-band), reassembles the seed via Shamir, posts a key_rotation_v1.",
                    ];
                    for step in steps {
                        ui.label(RichText::new(step).color(theme.text_secondary()).size(theme.font_size_small));
                        ui.add_space(2.0);
                    }
                });

                ui.add_space(theme.spacing_md);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Look up holder DID:").color(theme.text_secondary()));
                    ui.add_sized(
                        [ui.available_width() - 120.0, theme.input_height],
                        egui::TextEdit::singleline(&mut state.recovery_lookup_did)
                            .hint_text("did:hum:..."),
                    );
                    if Button::primary("Look up").show(ui, theme) {
                        state.recovery_lookup_pending = true;
                    }
                });

                ui.add_space(theme.spacing_sm);

                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Recovery setup").color(theme.accent()).strong().size(theme.font_size_heading),
                    );
                    ui.label(
                        RichText::new(
                            "GET /api/v2/recovery/setup/{holder_did} returns the threshold + guardian list.",
                        )
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                    );
                });

                ui.add_space(theme.spacing_md);
                ui.separator();
                ui.add_space(theme.spacing_md);

                ui.label(
                    RichText::new("Are you a guardian?")
                        .color(theme.text_primary())
                        .strong()
                        .size(theme.font_size_heading),
                );
                ui.label(
                    RichText::new("Look up shares stored on this server for your DID.")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_sm);

                ui.horizontal(|ui| {
                    ui.label(RichText::new("Guardian DID:").color(theme.text_secondary()));
                    ui.add_sized(
                        [ui.available_width() - 120.0, theme.input_height],
                        egui::TextEdit::singleline(&mut state.recovery_guardian_did)
                            .hint_text("did:hum:..."),
                    );
                    if Button::primary("Show shares").show(ui, theme) {
                        state.recovery_guardian_pending = true;
                    }
                });

                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Same view on the web: united-humanity.us/recovery")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });
}
