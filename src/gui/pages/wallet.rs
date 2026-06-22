//! Wallet page — SOL balance, address with copy, network selector,
//! send form with MAX button, transaction history, token list.
//!
//! Layout: a responsive two-column split (v0.499) — balance / address / send on
//! the left, transaction history + tokens on the right when the panel is wide
//! enough, stacked on a narrow window — so the page uses the horizontal space
//! instead of hugging the left edge.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiState, WalletNetwork, WalletTransaction};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Below this panel width the two columns stack into one.
const TWO_COL_MIN: f32 = 820.0;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
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
                        let btn = egui::Button::new(text)
                            .fill(fill)
                            .rounding(Rounding::same(4));
                        if ui.add(btn).clicked() {
                            state.wallet_network = *net;
                        }
                    }
                });
            });

            ui.add_space(theme.spacing_md);

            // Private-section lock — the wallet (crypto keys + balance) stays
            // collapsed + LOCKED until the vault passphrase is entered (the
            // operator's "crypto coin always locked when not actively in use").
            // Re-lock with the Lock button. Verification re-decrypts the seed
            // vault with the typed passphrase (decrypt_private_key → AES-GCM
            // auth); nothing is persisted, so a restart re-locks. No vault on
            // file (fresh / imported key) → nothing to gate against → shows open.
            let enc = state.encrypted_private_key.clone();
            let salt = state.key_salt.clone();
            let iters = state.key_iterations;
            let has_vault = !enc.is_empty() && !salt.is_empty();
            let unlocked = if has_vault {
                let lock = state.section_locks.entry("wallet".to_string()).or_default();
                widgets::lockable_gate(ui, theme, lock, "Private wallet data", |pass| {
                    crate::config::decrypt_private_key(&enc, &salt, pass, iters).is_ok()
                })
            } else {
                true
            };
            if !unlocked {
                return;
            }

            ScrollArea::vertical().show(ui, |ui| {
                // Responsive two-column: balance / address / send on the left,
                // history + tokens on the right when wide; stacked when narrow.
                if ui.available_width() >= TWO_COL_MIN {
                    ui.columns(2, |cols| {
                        pane_balance_send(&mut cols[0], theme, state);
                        pane_history_tokens(&mut cols[1], theme, state);
                    });
                } else {
                    pane_balance_send(ui, theme, state);
                    ui.add_space(theme.spacing_md);
                    pane_history_tokens(ui, theme, state);
                }
            });
        });
}

/// Left pane: balance, address + QR, and the send form (mutates wallet state).
fn pane_balance_send(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    // Balance card
    widgets::card(ui, theme, |ui| {
        ui.label(
            RichText::new("Balance")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{:.4} SOL", state.wallet_balance))
                    .size(theme.title_size)
                    .color(theme.accent()),
            );
        });
        ui.horizontal(|ui| {
            let usd = state.wallet_balance * state.wallet_sol_price;
            ui.label(
                RichText::new(format!("~${:.2} USD", usd))
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_md);
            // 24h change placeholder
            ui.label(
                RichText::new("24h: --")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });
    });

    ui.add_space(theme.spacing_sm);

    // Address section
    widgets::card(ui, theme, |ui| {
        ui.label(
            RichText::new("Address")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.horizontal(|ui| {
            let addr = &state.wallet_address;
            let display = if addr.len() > 16 {
                format!("{}...{}", &addr[..8], &addr[addr.len() - 8..])
            } else if addr.is_empty() {
                "No address set".to_string()
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
        ui.add_space(theme.spacing_xs);
        // QR placeholder
        let (qr_rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 100.0), egui::Sense::hover());
        ui.painter().rect_filled(qr_rect, Rounding::same(4), Color32::from_rgb(30, 30, 38)); // theme-exempt: QR-code placeholder fill until real QR rendering
        ui.painter().rect_stroke(qr_rect, Rounding::same(4), Stroke::new(1.0, theme.border()), egui::StrokeKind::Outside);
        ui.painter().text(
            qr_rect.center(),
            egui::Align2::CENTER_CENTER,
            "QR Code",
            egui::FontId::proportional(theme.font_size_small),
            theme.text_muted(),
        );
    });

    ui.add_space(theme.spacing_md);

    // Send form
    widgets::card_with_header(ui, theme, "Send SOL", |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("To:").color(theme.text_secondary()).size(theme.font_size_body));
            ui.add(
                egui::TextEdit::singleline(&mut state.wallet_send_to)
                    .desired_width(320.0)
                    .hint_text("Recipient address"),
            );
        });
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Amount:").color(theme.text_secondary()).size(theme.font_size_body));
            ui.add(
                egui::TextEdit::singleline(&mut state.wallet_send_amount)
                    .desired_width(140.0)
                    .hint_text("0.0"),
            );
            ui.label(RichText::new("SOL").color(theme.text_muted()));
            // MAX button
            if widgets::secondary_button(ui, theme, "MAX") {
                state.wallet_send_amount = format!("{:.4}", state.wallet_balance);
            }
        });
        ui.add_space(theme.spacing_sm);
        let amount = state.wallet_send_amount.parse::<f64>().unwrap_or(0.0);
        let can_send = !state.wallet_send_to.is_empty()
            && amount > 0.0
            && amount <= state.wallet_balance;
        ui.add_enabled_ui(can_send, |ui| {
            if widgets::primary_button(ui, theme, "Send") {
                let amount_val = state.wallet_send_amount.parse().unwrap_or(0.0);
                let to = state.wallet_send_to.clone();
                state.wallet_transactions.insert(0, WalletTransaction {
                    signature: format!("tx_{}", state.wallet_transactions.len() + 1),
                    direction: "Sent".to_string(),
                    amount: amount_val,
                    counterparty: to,
                    timestamp: "Just now".to_string(),
                });
                state.wallet_balance -= amount_val;
                state.wallet_send_to.clear();
                state.wallet_send_amount.clear();
            }
        });
        if !can_send && amount > state.wallet_balance && amount > 0.0 {
            ui.label(
                RichText::new("Insufficient balance")
                    .size(theme.font_size_small)
                    .color(theme.danger()),
            );
        }
    });
}

/// Right pane: transaction history + token list (read-only display).
fn pane_history_tokens(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    // Transaction history
    widgets::card_with_header(ui, theme, "Transaction History", |ui| {
        if state.wallet_transactions.is_empty() {
            ui.label(
                RichText::new("No transactions yet")
                    .color(theme.text_muted()),
            );
        } else {
            // Table header
            ui.horizontal(|ui| {
                ui.set_min_width(500.0);
                let header_style = |t: &str| RichText::new(t).size(theme.font_size_small).color(theme.text_muted());
                ui.label(header_style("Date"));
                ui.add_space(40.0);
                ui.label(header_style("Type"));
                ui.add_space(20.0);
                ui.label(header_style("Amount"));
                ui.add_space(30.0);
                ui.label(header_style("Status"));
                ui.add_space(20.0);
                ui.label(header_style("Tx Hash"));
            });
            ui.separator();

            ScrollArea::vertical()
                .id_salt("wallet_tx_list")
                .max_height(200.0)
                .show(ui, |ui| {
                    for tx in &state.wallet_transactions {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&tx.timestamp)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                            ui.add_space(20.0);
                            let dir_color = if tx.direction == "Sent" {
                                theme.danger()
                            } else {
                                theme.success()
                            };
                            let dir_prefix = if tx.direction == "Sent" { "-" } else { "+" };
                            ui.label(
                                RichText::new(&tx.direction)
                                    .size(theme.font_size_small)
                                    .color(dir_color),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(format!("{}{:.4} SOL", dir_prefix, tx.amount))
                                    .size(theme.font_size_small)
                                    .color(dir_color),
                            );
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new("Confirmed")
                                    .size(theme.font_size_small)
                                    .color(theme.success()),
                            );
                            ui.add_space(10.0);
                            let hash_display = if tx.signature.len() > 12 {
                                format!("{}...", &tx.signature[..10])
                            } else {
                                tx.signature.clone()
                            };
                            ui.label(
                                RichText::new(hash_display)
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted())
                                    .monospace(),
                            );
                        });
                    }
                });
        }
    });

    ui.add_space(theme.spacing_md);

    // Token list
    widgets::card_with_header(ui, theme, "Tokens", |ui| {
        // SOL always shown
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("SOL")
                    .size(theme.font_size_body)
                    .color(theme.text_primary()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:.4}", state.wallet_balance))
                        .size(theme.font_size_body)
                        .color(theme.accent()),
                );
            });
        });
        ui.separator();
        // Placeholder for SPL tokens
        ui.label(
            RichText::new("No other tokens found")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    });
}
