//! Wallet page — SOL balance, address, send form, transaction history.

use egui::{Color32, Frame, RichText, ScrollArea};
use crate::gui::{GuiState, WalletNetwork, WalletTransaction};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Wallet")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Network selector
                    let networks = [WalletNetwork::Mainnet, WalletNetwork::Devnet, WalletNetwork::Testnet];
                    for net in networks.iter().rev() {
                        let is_active = state.wallet_network == *net;
                        let text = if is_active {
                            RichText::new(net.label()).color(theme.text_on_accent()).size(theme.font_size_small)
                        } else {
                            RichText::new(net.label()).color(theme.text_secondary()).size(theme.font_size_small)
                        };
                        let fill = if is_active { theme.accent() } else { theme.bg_card() };
                        let btn = egui::Button::new(text).fill(fill);
                        if ui.add(btn).clicked() {
                            state.wallet_network = *net;
                        }
                    }
                });
            });

            ui.add_space(theme.spacing_md);

            ScrollArea::vertical().show(ui, |ui| {
                // Balance display
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Balance")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!("{:.4} SOL", state.wallet_balance))
                                .size(theme.font_size_title)
                                .color(theme.accent()),
                        );
                        ui.label(
                            RichText::new(format!("~${:.2} USD", state.wallet_balance * state.wallet_sol_price))
                                .size(theme.font_size_body)
                                .color(theme.text_muted()),
                        );
                    });
                });

                ui.add_space(theme.spacing_sm);

                // Address display
                widgets::card(ui, theme, |ui| {
                    ui.label(
                        RichText::new("Address")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                    ui.horizontal(|ui| {
                        let addr = &state.wallet_address;
                        let display = if addr.len() > 12 {
                            format!("{}...{}", &addr[..6], &addr[addr.len() - 6..])
                        } else {
                            addr.clone()
                        };
                        ui.label(
                            RichText::new(&display)
                                .size(theme.font_size_body)
                                .color(theme.text_primary())
                                .monospace(),
                        );
                        if widgets::secondary_button(ui, theme, "Copy") {
                            ui.output_mut(|o| o.copied_text = state.wallet_address.clone());
                        }
                    });
                });

                ui.add_space(theme.spacing_md);

                // Send form
                widgets::card_with_header(ui, theme, "Send SOL", |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("To:").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::singleline(&mut state.wallet_send_to)
                                .desired_width(300.0)
                                .hint_text("Recipient address"),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Amount:").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::singleline(&mut state.wallet_send_amount)
                                .desired_width(120.0)
                                .hint_text("0.0"),
                        );
                        ui.label(RichText::new("SOL").color(theme.text_muted()));
                    });
                    ui.add_space(theme.spacing_xs);
                    let can_send = !state.wallet_send_to.is_empty()
                        && state.wallet_send_amount.parse::<f64>().unwrap_or(0.0) > 0.0;
                    ui.add_enabled_ui(can_send, |ui| {
                        if widgets::primary_button(ui, theme, "Send") {
                            let amount_str = state.wallet_send_amount.clone();
                            let to = state.wallet_send_to.clone();
                            state.wallet_transactions.insert(0, WalletTransaction {
                                signature: format!("tx_{}", state.wallet_transactions.len()),
                                direction: "Sent".to_string(),
                                amount: amount_str.parse().unwrap_or(0.0),
                                counterparty: to,
                                timestamp: "Just now".to_string(),
                            });
                            state.wallet_send_to.clear();
                            state.wallet_send_amount.clear();
                        }
                    });
                });

                ui.add_space(theme.spacing_md);

                // Transaction history
                widgets::card_with_header(ui, theme, "Transaction History", |ui| {
                    if state.wallet_transactions.is_empty() {
                        ui.label(
                            RichText::new("No transactions yet")
                                .color(theme.text_muted()),
                        );
                    } else {
                        for tx in &state.wallet_transactions {
                            ui.horizontal(|ui| {
                                let dir_color = if tx.direction == "Sent" {
                                    theme.danger()
                                } else {
                                    theme.success()
                                };
                                ui.label(
                                    RichText::new(&tx.direction)
                                        .size(theme.font_size_small)
                                        .color(dir_color),
                                );
                                ui.label(
                                    RichText::new(format!("{:.4} SOL", tx.amount))
                                        .size(theme.font_size_small)
                                        .color(theme.text_primary()),
                                );
                                let cp = if tx.counterparty.len() > 12 {
                                    format!("{}...", &tx.counterparty[..8])
                                } else {
                                    tx.counterparty.clone()
                                };
                                ui.label(
                                    RichText::new(cp)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted())
                                        .monospace(),
                                );
                                ui.label(
                                    RichText::new(&tx.timestamp)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });
                        }
                    }
                });
            });
        });
}
