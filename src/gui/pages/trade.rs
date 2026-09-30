//! P2P Trading page - live escrow trades against the connected relay
//! (v0.756, closure ladder rung 5). The relay stores the whole flow
//! (request, accept/reject, per-side item offers, dual confirm, cancel)
//! and delivers state through targeted `__trade_data__:` / `__trade_list__:`
//! private wrappers that lib.rs routes into GuiState.trades. This page
//! renders that live state and sends the typed WS requests back - the
//! round-trip is the truth, no local echo. (Replaced the hardcoded
//! Alice/Bob/Carol mock that shipped with the first page skeleton.)
//!
//! TRADES MOVE ITEMS (2026-09-29). An offer line names an item you carry (typed
//! by name or id, resolved against your backpack, refused otherwise) and carries
//! its items.csv id, wear and grade. Confirm waits until you carry everything you
//! offered. When the relay completes the trade, each player's own game hands over
//! its side: what you offered leaves your backpack, what they offered arrives
//! (`settle_completed`). The relay holds no inventories yet, so it cannot keep the
//! items in escrow; the confirm check is the guard, and a line typed as text (the
//! web page's offers) is shown but cannot move.

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiItemSlot, GuiState, GuiTrade, GuiTradeItem};
use crate::systems::inventory::TransferOp;
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

/// One line of my offer draft (2026-09-29): the carried item it names (its
/// items.csv id, display name, and the stack's wear and grade) and the quantity
/// text. An empty `item_id` is a line typed as text, which cannot move.
#[derive(Debug, Clone, Default, PartialEq)]
struct OfferLine {
    item_id: String,
    name: String,
    qty: String,
    wear: u32,
    quality: u8,
}

impl OfferLine {
    fn from_item(i: &GuiTradeItem) -> Self {
        Self {
            item_id: i.reference_id.clone().unwrap_or_default(),
            name: i.name.clone(),
            qty: i.quantity.to_string(),
            wear: i.wear,
            quality: i.quality,
        }
    }

    /// The relay's TradeItem shape; the id, wear and grade ride along.
    fn to_json(&self) -> serde_json::Value {
        let mut v = serde_json::json!({
            "item_type": "item",
            "name": self.name.trim(),
            "quantity": self.qty.trim().parse::<u32>().unwrap_or(1).max(1),
            "description": "",
        });
        if !self.item_id.is_empty() {
            v["reference_id"] = serde_json::json!(self.item_id);
            v["wear"] = serde_json::json!(self.wear);
            v["quality"] = serde_json::json!(self.quality);
        }
        v
    }
}

/// The carried stack a typed offer means: an item id or a name, ignoring case.
/// None when nothing in the backpack matches.
fn resolve_carried(inv: &[Option<GuiItemSlot>], typed: &str) -> Option<GuiItemSlot> {
    let t = typed.trim();
    if t.is_empty() {
        return None;
    }
    inv.iter().flatten().find(|s| s.item_id.eq_ignore_ascii_case(t) || s.name.eq_ignore_ascii_case(t)).cloned()
}

/// How many of an item the backpack holds, across all its stacks.
fn carried_count(inv: &[Option<GuiItemSlot>], item_id: &str) -> u32 {
    inv.iter().flatten().filter(|s| s.item_id == item_id).map(|s| s.quantity).sum()
}

/// Why this offer cannot be confirmed yet, or None when the backpack holds all
/// of it. Lines of the same item are added up before counting.
fn offer_shortfall(inv: &[Option<GuiItemSlot>], offer: &[GuiTradeItem]) -> Option<String> {
    let mut wanted: Vec<(&str, &str, u32)> = Vec::new();
    for i in offer {
        let Some(id) = i.reference_id.as_deref() else {
            return Some(format!("\"{}\" is text, not an item from your backpack: remove it and add the item by name.", i.name));
        };
        match wanted.iter_mut().find(|w| w.0 == id) {
            Some(w) => w.2 += i.quantity,
            None => wanted.push((id, &i.name, i.quantity)),
        }
    }
    wanted.into_iter().find_map(|(id, name, q)| {
        let have = carried_count(inv, id);
        (have < q).then(|| format!("You offer {q} x {name} but carry {have}."))
    })
}

/// The backpack moves a completed trade makes for me: what I offered goes out,
/// what they offered comes in, with the offered wear and grade. A line with no
/// item id, or naming an item this game does not know (`known` says no), cannot
/// move and is counted instead.
fn trade_transfers(t: &GuiTrade, my_key: &str, known: impl Fn(&str) -> bool) -> (Vec<TransferOp>, usize) {
    let (give, get) = if t.initiator_key == my_key {
        (&t.initiator_items, &t.recipient_items)
    } else if t.recipient_key == my_key {
        (&t.recipient_items, &t.initiator_items)
    } else {
        return (Vec::new(), 0);
    };
    let mut ops = Vec::new();
    let mut unmovable = 0;
    for (items, add) in [(give, false), (get, true)] {
        for i in items {
            match i.reference_id.as_deref() {
                Some(id) if i.quantity > 0 && (!add || known(id)) => ops.push(TransferOp {
                    item_id: id.to_string(),
                    qty: i.quantity,
                    add,
                    wear: i.wear,
                    quality: i.quality,
                }),
                _ => unmovable += 1,
            }
        }
    }
    (ops, unmovable)
}

/// Settle a completed trade in this game, once per trade (2026-09-29): queue the
/// backpack moves (the main loop hands them to the InventorySystem; what does
/// not fit goes to Home storage and says so) and return the line the page shows.
pub(crate) fn settle_completed(gs: &mut GuiState, trade_id: &str, known: impl Fn(&str) -> bool) -> String {
    if !with_state(|ts| ts.settled.insert(trade_id.to_string())) {
        return "Trade completed.".to_string();
    }
    let Some(t) = gs.trades.iter().find(|t| t.id == trade_id) else {
        return "Trade completed, but its items were not loaded here, so nothing moved.".to_string();
    };
    let (ops, unmovable) = trade_transfers(t, &gs.profile_public_key, known);
    let count = |n: u32| if n == 1 { "1 item".to_string() } else { format!("{n} items") };
    let gave = count(ops.iter().filter(|o| !o.add).map(|o| o.qty).sum());
    let got = count(ops.iter().filter(|o| o.add).map(|o| o.qty).sum());
    gs.pending_inventory_transfers.extend(ops);
    let mut msg = format!("Trade completed: {gave} left your backpack and {got} arrived.");
    if unmovable > 0 {
        msg += &format!(" {unmovable} line(s) could not move (typed as text, or an item this game does not know).");
    }
    msg
}

/// Page-local UI state: selection + form drafts. The live trade data itself
/// lives in GuiState (bridged from the relay), never here.
struct TradePageState {
    /// Selected trade by ID (broadcasts reorder the vector).
    selected: Option<String>,
    show_new: bool,
    new_recipient: String,
    new_message: String,
    /// My-side offer draft rows, seeded from the selected trade's current
    /// items; `draft_for` says which trade.
    draft_items: Vec<OfferLine>,
    draft_for: String,
    new_item_name: String,
    new_item_qty: String,
    /// Trades whose items this game already moved (a completion is settled
    /// once, however many times its notice arrives).
    settled: std::collections::HashSet<String>,
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
            settled: std::collections::HashSet::new(),
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
                // Heading matches the Profile sidebar label ("Trade").
                ui.label(
                    RichText::new("Trade")
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
                            ts.draft_items = my_items.iter().map(OfferLine::from_item).collect();
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
                            for (i, line) in ts.draft_items.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    // A text line (from before offers named items)
                                    // shows in the warning colour: it cannot move.
                                    let color = if line.item_id.is_empty() { theme.warning() } else { theme.text_primary() };
                                    ui.add_sized([160.0, 18.0], egui::Label::new(RichText::new(&line.name).color(color)));
                                    ui.add(egui::TextEdit::singleline(&mut line.qty).desired_width(40.0));
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
                                        .hint_text("an item you carry")
                                        .desired_width(160.0),
                                );
                                ui.add(
                                    egui::TextEdit::singleline(&mut ts.new_item_qty)
                                        .desired_width(40.0),
                                );
                                if widgets::secondary_button(ui, theme, "Add")
                                    && !ts.new_item_name.trim().is_empty()
                                {
                                    match resolve_carried(&state.inventory_items, &ts.new_item_name) {
                                        Some(slot) => {
                                            ts.draft_items.push(OfferLine {
                                                item_id: slot.item_id,
                                                name: slot.name,
                                                qty: ts.new_item_qty.trim().to_string(),
                                                wear: slot.wear,
                                                quality: slot.quality,
                                            });
                                            ts.new_item_name.clear();
                                            ts.new_item_qty = "1".to_string();
                                        }
                                        None => {
                                            state.trade_status = format!(
                                                "You aren't carrying anything called \"{}\".",
                                                ts.new_item_name.trim()
                                            );
                                        }
                                    }
                                }
                            });
                            if widgets::secondary_button(ui, theme, "Update offer") {
                                let items: Vec<serde_json::Value> = ts
                                    .draft_items
                                    .iter()
                                    .filter(|l| !l.name.trim().is_empty())
                                    .map(OfferLine::to_json)
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
                            let shortfall = offer_shortfall(&state.inventory_items, my_items);
                            ui.horizontal(|ui| {
                                if !my_confirmed
                                    && shortfall.is_none()
                                    && widgets::primary_button(ui, theme, "Confirm trade")
                                {
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
                            if let Some(why) = &shortfall {
                                ui.label(RichText::new(why).color(theme.warning()).size(theme.font_size_small));
                            }
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

/// TRADES MOVE ITEMS (2026-09-29). Red checks, run: settling twice moves the
/// items twice without the `settled` guard; an offer the backpack cannot cover
/// confirms without `offer_shortfall`.
#[cfg(all(test, feature = "native"))]
mod trade_moves_tests {
    use super::*;

    fn slot(id: &str, name: &str, qty: u32) -> Option<GuiItemSlot> {
        Some(GuiItemSlot { item_id: id.into(), name: name.into(), quantity: qty, ..Default::default() })
    }
    fn offered(id: Option<&str>, name: &str, qty: u32) -> GuiTradeItem {
        GuiTradeItem { name: name.into(), quantity: qty, reference_id: id.map(str::to_string), ..Default::default() }
    }

    #[test]
    fn an_offer_names_a_carried_item_and_must_be_covered() {
        let inv = vec![slot("rope_0", "Rope", 2), None, slot("rope_0", "Rope", 1)];
        // By name or id, any case; something not carried is refused.
        assert_eq!(resolve_carried(&inv, " rope ").map(|s| s.item_id), Some("rope_0".into()));
        assert_eq!(resolve_carried(&inv, "ROPE_0").map(|s| s.name), Some("Rope".into()));
        assert!(resolve_carried(&inv, "Hammer").is_none());
        // Three ropes across two stacks cover two lines of 1 and 2, not 4.
        assert_eq!(offer_shortfall(&inv, &[offered(Some("rope_0"), "Rope", 1), offered(Some("rope_0"), "Rope", 2)]), None);
        let short = offer_shortfall(&inv, &[offered(Some("rope_0"), "Rope", 4)]).expect("4 ropes are not carried");
        assert!(short.contains("carry 3"), "{short}");
        // A text line cannot be confirmed.
        assert!(offer_shortfall(&inv, &[offered(None, "Help building", 1)]).is_some());
        // The line the relay stores keeps the id, wear and grade.
        let line = OfferLine { item_id: "hammer_0".into(), name: "Hammer".into(), qty: "1".into(), wear: 150, quality: 2 };
        let back = GuiTrade::from_relay_json(&serde_json::json!({"initiator_items": [line.to_json()]}));
        assert_eq!(OfferLine::from_item(&back.initiator_items[0]), line);
    }

    #[test]
    fn a_completed_trade_moves_each_side_once() {
        let mut gs = GuiState::default();
        gs.profile_public_key = "bob".into();
        gs.trades.push(GuiTrade {
            id: "t-1".into(),
            initiator_key: "alice".into(),
            recipient_key: "bob".into(),
            status: "completed".into(),
            initiator_items: vec![
                GuiTradeItem { wear: 150, ..offered(Some("hammer_0"), "Hammer", 1) },
                offered(None, "A hug", 1),
                offered(Some("modded_thing"), "Modded", 1),
            ],
            recipient_items: vec![offered(Some("wheat_0"), "Wheat", 5)],
            ..Default::default()
        });
        let known = |id: &str| id != "modded_thing";
        let msg = settle_completed(&mut gs, "t-1", known);
        let ops = &gs.pending_inventory_transfers;
        assert!(ops.contains(&TransferOp { item_id: "wheat_0".into(), qty: 5, add: false, wear: 0, quality: 0 }), "Bob gives his wheat: {ops:?}");
        assert!(ops.contains(&TransferOp { item_id: "hammer_0".into(), qty: 1, add: true, wear: 150, quality: 0 }), "the hammer arrives worn: {ops:?}");
        assert_eq!(ops.len(), 2, "the hug and the unknown item do not move: {ops:?}");
        assert!(msg.contains("5 items left") && msg.contains("1 item arrived") && msg.contains("2 line(s)"), "{msg}");
        // A second notice for the same trade moves nothing more.
        settle_completed(&mut gs, "t-1", known);
        assert_eq!(gs.pending_inventory_transfers.len(), 2);
        // Someone outside the trade moves nothing.
        assert_eq!(trade_transfers(&gs.trades[0], "carol", known).0.len(), 0);
    }
}
