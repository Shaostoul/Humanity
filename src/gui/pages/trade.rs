//! P2P Trading page - live escrow trades against the connected relay
//! (v0.756, closure ladder rung 5). The relay stores the whole flow
//! (request, accept/reject, per-side item offers, dual confirm, cancel)
//! and delivers state through targeted `__trade_data__:` / `__trade_list__:`
//! private wrappers that lib.rs routes into GuiState.trades. This page
//! renders that live state and sends the typed WS requests back - the
//! round-trip is the truth, no local echo. (Replaced the hardcoded
//! Alice/Bob/Carol mock that shipped with the first page skeleton.)

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiState, GuiTrade};
use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};

/// Colour for a relay trade status string.
fn status_color(theme: &Theme, status: &str) -> Color32 {
    match status {
        "pending" => theme.warning(),
        "active" => Theme::c32(&theme.info),
        "completed" => theme.success(),
        _ => theme.danger(), // cancelled / rejected
    }
}

/// Short display for the other party's key.
fn key_label(key: &str) -> String {
    if key.len() > 10 {
        format!("{}...", &key[..10])
    } else {
        key.to_string()
    }
}

/// Page-local UI state: selection + form drafts. The live trade data itself
/// lives in GuiState (bridged from the relay), never here.
struct TradePageState {
    /// Selected trade by ID (broadcasts reorder the vector).
    selected: Option<String>,
    show_new: bool,
    new_recipient: String,
    new_message: String,
    /// My-side offer draft rows (name, quantity text), seeded from the
    /// selected trade's current items; `draft_for` says which trade.
    draft_items: Vec<(String, String)>,
    draft_for: String,
    new_item_name: String,
    new_item_qty: String,
}

impl Default for TradePageState {
    fn default() -> Self {
        Self {
            selected: None,
            show_new: false,
            new_recipient: String::new(),
            new_message: String::new(),
            draft_items: Vec::new(),
            draft_for: String::new(),
            new_item_name: String::new(),
            new_item_qty: "1".to_string(),
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut TradePageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<TradePageState> = RefCell::new(TradePageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Live sync (same lifecycle as the Market page): first view after
    // connect pulls my trade list; the private-wrapper bridge keeps it
    // current; a disconnect clears the flag so a reconnect re-syncs.
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    if connected && !state.trades_synced {
        if let Some(ws) = &state.ws_client {
            ws.send(&serde_json::json!({"type": "trade_list_request"}).to_string());
        }
        state.trades_synced = true;
    }
    if !connected {
        state.trades_synced = false;
    }
    let my_key = state.profile_public_key.clone();

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("P2P Trading")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    with_state(|ts| {
                        if widgets::primary_button(ui, theme, "New Trade") {
                            ts.show_new = !ts.show_new;
                        }
                    });
                    if connected && widgets::secondary_button(ui, theme, "Refresh") {
                        if let Some(ws) = &state.ws_client {
                            ws.send(&serde_json::json!({"type": "trade_list_request"}).to_string());
                        }
                    }
                });
            });
            if !connected {
                ui.label(
                    RichText::new("Offline - connect to a server (Chat page) to trade.")
                        .color(theme.warning())
                        .size(theme.font_size_small),
                );
            }
            if !state.trade_status.is_empty() {
                ui.label(
                    RichText::new(&state.trade_status)
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                );
            }
            ui.separator();

            // New trade form: ask another player (by public key) to trade.
            with_state(|ts| {
                if ts.show_new {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("Start New Trade")
                                .size(theme.font_size_body)
                                .color(theme.accent()),
                        );
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Their key:").color(theme.text_secondary()));
                            ui.add(
                                egui::TextEdit::singleline(&mut ts.new_recipient)
                                    .hint_text("paste the other player's public key")
                                    .desired_width(320.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Message:").color(theme.text_secondary()));
                            ui.add(
                                egui::TextEdit::singleline(&mut ts.new_message)
                                    .hint_text("optional greeting / what you are after")
                                    .desired_width(320.0),
                            );
                        });
                        ui.horizontal(|ui| {
                            if widgets::primary_button(ui, theme, "Send request")
                                && !ts.new_recipient.trim().is_empty()
                            {
                                if connected {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(
                                            &serde_json::json!({
                                                "type": "trade_request",
                                                "target_key": ts.new_recipient.trim(),
                                                "message": ts.new_message.trim(),
                                            })
                                            .to_string(),
                                        );
                                    }
                                    state.trade_status = "Trade request sent.".to_string();
                                    ts.new_recipient.clear();
                                    ts.new_message.clear();
                                    ts.show_new = false;
                                } else {
                                    state.trade_status =
                                        "Connect to a server to send trade requests.".to_string();
                                }
                            }
                            if widgets::secondary_button(ui, theme, "Cancel") {
                                ts.show_new = false;
                            }
                        });
                    });
                    ui.add_space(theme.spacing_sm);
                }
            });

            if state.trades.is_empty() {
                ui.add_space(theme.spacing_xl);
                ui.vertical_centered(|ui| {
                    ui.label(
                        RichText::new("No trades")
                            .size(theme.font_size_heading)
                            .color(theme.text_muted()),
                    );
                    let hint = if connected {
                        "Start one with New Trade - you need the other player's public key."
                    } else {
                        "Connect to a server to see your trades."
                    };
                    ui.label(RichText::new(hint).color(theme.text_secondary()));
                });
                return;
            }

            // List + detail
            ui.columns(2, |cols| {
                // Left: my trades, newest first (relay order).
                cols[0].label(
                    RichText::new("Trades")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                let rows: Vec<(String, String, String, usize, usize, bool)> = state
                    .trades
                    .iter()
                    .map(|t| {
                        let partner = if t.initiator_key == my_key {
                            &t.recipient_key
                        } else {
                            &t.initiator_key
                        };
                        (
                            t.id.clone(),
                            key_label(partner),
                            t.status.clone(),
                            t.initiator_items.len(),
                            t.recipient_items.len(),
                            t.initiator_key == my_key,
                        )
                    })
                    .collect();
                ScrollArea::vertical().id_salt("trade_list").show(&mut cols[0], |ui| {
                    with_state(|ts| {
                        for (id, partner, status, init_n, recv_n, i_started) in &rows {
                            let selected = ts.selected.as_deref() == Some(id.as_str());
                            let fill = if selected { theme.bg_card() } else { Color32::TRANSPARENT };
                            egui::Frame::none()
                                .fill(fill)
                                .rounding(Rounding::same(theme.border_radius as u8))
                                .inner_margin(8.0)
                                .show(ui, |ui| {
                                    let resp = ui.horizontal(|ui| {
                                        ui.label(RichText::new(partner).color(theme.text_primary()));
                                        egui::Frame::none()
                                            .fill(status_color(theme, status))
                                            .rounding(Rounding::same(3))
                                            .inner_margin(Vec2::new(6.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(
                                                    RichText::new(status.as_str())
                                                        .size(theme.font_size_small)
                                                        .color(Color32::WHITE),
                                                );
                                            });
                                        let (mine, theirs) = if *i_started {
                                            (init_n, recv_n)
                                        } else {
                                            (recv_n, init_n)
                                        };
                                        ui.label(
                                            RichText::new(format!("{mine} for {theirs} items"))
                                                .color(theme.text_muted())
                                                .size(theme.font_size_small),
                                        );
                                    });
                                    if resp.response.interact(egui::Sense::click()).clicked() {
                                        ts.selected = Some(id.clone());
                                    }
                                });
                        }
                    });
                });

                // Right: selected trade detail + actions.
                let sel_id = with_state(|ts| ts.selected.clone()).unwrap_or_default();
                let trade: Option<GuiTrade> =
                    state.trades.iter().find(|t| t.id == sel_id).cloned();
                if let Some(t) = trade {
                    let i_am_initiator = t.initiator_key == my_key;
                    let (my_items, their_items) = if i_am_initiator {
                        (&t.initiator_items, &t.recipient_items)
                    } else {
                        (&t.recipient_items, &t.initiator_items)
                    };
                    let (my_confirmed, their_confirmed) = if i_am_initiator {
                        (t.initiator_confirmed, t.recipient_confirmed)
                    } else {
                        (t.recipient_confirmed, t.initiator_confirmed)
                    };
                    let partner = if i_am_initiator { &t.recipient_key } else { &t.initiator_key };

                    // Seed / reseed the offer draft when the selection changes.
                    with_state(|ts| {
                        if ts.draft_for != t.id {
                            ts.draft_for = t.id.clone();
                            ts.draft_items = my_items
                                .iter()
                                .map(|i| (i.name.clone(), i.quantity.to_string()))
                                .collect();
                            ts.new_item_name.clear();
                            ts.new_item_qty = "1".to_string();
                        }
                    });

                    let ui = &mut cols[1];
                    ui.label(
                        RichText::new(format!("Trade with {}", key_label(partner)))
                            .size(theme.font_size_body)
                            .color(theme.accent()),
                    );
                    if !t.message.is_empty() {
                        ui.label(
                            RichText::new(format!("\"{}\"", t.message))
                                .color(theme.text_muted())
                                .size(theme.font_size_small),
                        );
                    }
                    ui.add_space(theme.spacing_sm);

                    // Their offer (read-only).
                    ui.label(RichText::new("Their offer:").color(theme.text_secondary()));
                    if their_items.is_empty() {
                        ui.label(RichText::new("  (nothing yet)").color(theme.text_muted()));
                    }
                    for item in their_items {
                        ui.label(
                            RichText::new(format!("  {} x{}", item.name, item.quantity))
                                .color(theme.text_primary()),
                        );
                    }
                    ui.add_space(theme.spacing_sm);

                    // My offer: editable while the trade is active.
                    ui.label(RichText::new("Your offer:").color(theme.text_secondary()));
                    if t.status == "active" {
                        with_state(|ts| {
                            let mut remove: Option<usize> = None;
                            for (i, (name, qty)) in ts.draft_items.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.add(egui::TextEdit::singleline(name).desired_width(160.0));
                                    ui.add(egui::TextEdit::singleline(qty).desired_width(40.0));
                                    if widgets::secondary_button(ui, theme, "x") {
                                        remove = Some(i);
                                    }
                                });
                            }
                            if let Some(i) = remove {
                                ts.draft_items.remove(i);
                            }
                            ui.horizontal(|ui| {
                                ui.add(
                                    egui::TextEdit::singleline(&mut ts.new_item_name)
                                        .hint_text("item name")
                                        .desired_width(160.0),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut ts.new_item_qty)
                                        .desired_width(40.0),
                                );
                                if widgets::secondary_button(ui, theme, "Add")
                                    && !ts.new_item_name.trim().is_empty()
                                {
                                    ts.draft_items.push((
                                        ts.new_item_name.trim().to_string(),
                                        ts.new_item_qty.trim().to_string(),
                                    ));
                                    ts.new_item_name.clear();
                                    ts.new_item_qty = "1".to_string();
                                }
                            });
                            if widgets::secondary_button(ui, theme, "Update offer") {
                                let items: Vec<serde_json::Value> = ts
                                    .draft_items
                                    .iter()
                                    .filter(|(n, _)| !n.trim().is_empty())
                                    .map(|(n, q)| {
                                        serde_json::json!({
                                            "item_type": "item",
                                            "name": n.trim(),
                                            "quantity": q.trim().parse::<u32>().unwrap_or(1).max(1),
                                            "description": "",
                                        })
                                    })
                                    .collect();
                                if let Some(ws) = &state.ws_client {
                                    ws.send(
                                        &serde_json::json!({
                                            "type": "trade_update_items",
                                            "trade_id": t.id,
                                            "items": items,
                                        })
                                        .to_string(),
                                    );
                                }
                                state.trade_status = "Offer updated.".to_string();
                            }
                        });
                    } else {
                        if my_items.is_empty() {
                            ui.label(RichText::new("  (nothing yet)").color(theme.text_muted()));
                        }
                        for item in my_items {
                            ui.label(
                                RichText::new(format!("  {} x{}", item.name, item.quantity))
                                    .color(theme.text_primary()),
                            );
                        }
                    }
                    ui.add_space(theme.spacing_sm);

                    // Confirmations + actions per status.
                    match t.status.as_str() {
                        "pending" => {
                            if i_am_initiator {
                                ui.label(
                                    RichText::new("Waiting for them to accept.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                                if widgets::secondary_button(ui, theme, "Cancel request") {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&serde_json::json!({"type": "trade_cancel", "trade_id": t.id}).to_string());
                                    }
                                }
                            } else {
                                ui.horizontal(|ui| {
                                    if widgets::primary_button(ui, theme, "Accept") {
                                        if let Some(ws) = &state.ws_client {
                                            ws.send(&serde_json::json!({"type": "trade_response", "trade_id": t.id, "accepted": true}).to_string());
                                        }
                                    }
                                    if widgets::secondary_button(ui, theme, "Reject") {
                                        if let Some(ws) = &state.ws_client {
                                            ws.send(&serde_json::json!({"type": "trade_response", "trade_id": t.id, "accepted": false}).to_string());
                                        }
                                    }
                                });
                            }
                        }
                        "active" => {
                            let conf = format!(
                                "You: {}   Them: {}",
                                if my_confirmed { "confirmed ✓" } else { "not confirmed" },
                                if their_confirmed { "confirmed ✓" } else { "not confirmed" },
                            );
                            ui.label(
                                RichText::new(conf)
                                    .color(theme.text_secondary())
                                    .size(theme.font_size_small),
                            );
                            ui.horizontal(|ui| {
                                if !my_confirmed && widgets::primary_button(ui, theme, "Confirm trade") {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&serde_json::json!({"type": "trade_confirm", "trade_id": t.id}).to_string());
                                    }
                                }
                                if widgets::secondary_button(ui, theme, "Cancel trade") {
                                    if let Some(ws) = &state.ws_client {
                                        ws.send(&serde_json::json!({"type": "trade_cancel", "trade_id": t.id}).to_string());
                                    }
                                }
                            });
                            if my_confirmed && !their_confirmed {
                                ui.label(
                                    RichText::new("Confirmed - waiting on them. Changing items resets confirmations.")
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                        }
                        other => {
                            ui.label(
                                RichText::new(format!("This trade is {other}."))
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                            );
                        }
                    }
                } else {
                    cols[1].label(
                        RichText::new("Select a trade to view it.")
                            .color(theme.text_muted()),
                    );
                }
            });
        });
}
