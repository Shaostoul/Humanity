//! P2P Trading page — view, create, and manage trades with other players.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Status of a trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeStatus {
    Pending,
    Active,
    Completed,
    Cancelled,
}

impl TradeStatus {
    pub fn label(&self) -> &str {
        match self {
            Self::Pending => "Pending",
            Self::Active => "Active",
            Self::Completed => "Completed",
            Self::Cancelled => "Cancelled",
        }
    }

    pub fn color(&self, theme: &Theme) -> Color32 {
        match self {
            Self::Pending => theme.warning(),
            Self::Active => Theme::c32(&theme.info),
            Self::Completed => theme.success(),
            Self::Cancelled => theme.danger(),
        }
    }
}

/// A single item in a trade offer.
#[derive(Debug, Clone)]
pub struct TradeItem {
    pub name: String,
    pub quantity: u32,
}

/// A trade between two players.
#[derive(Debug, Clone)]
pub struct Trade {
    pub id: u64,
    pub partner_name: String,
    pub status: TradeStatus,
    pub your_items: Vec<TradeItem>,
    pub their_items: Vec<TradeItem>,
}

/// Local state for the trade page UI.
pub struct TradePageState {
    pub trades: Vec<Trade>,
    pub selected_trade: Option<usize>,
    pub show_new_trade: bool,
    pub new_trade_recipient: String,
    pub new_item_name: String,
    pub new_item_qty: u32,
}

impl Default for TradePageState {
    fn default() -> Self {
        Self {
            trades: vec![
                Trade {
                    id: 1,
                    partner_name: "Alice".into(),
                    status: TradeStatus::Active,
                    your_items: vec![TradeItem { name: "Iron Ore".into(), quantity: 10 }],
                    their_items: vec![TradeItem { name: "Wood Plank".into(), quantity: 20 }],
                },
                Trade {
                    id: 2,
                    partner_name: "Bob".into(),
                    status: TradeStatus::Pending,
                    your_items: vec![],
                    their_items: vec![TradeItem { name: "Stone".into(), quantity: 5 }],
                },
                Trade {
                    id: 3,
                    partner_name: "Carol".into(),
                    status: TradeStatus::Completed,
                    your_items: vec![TradeItem { name: "Copper Wire".into(), quantity: 3 }],
                    their_items: vec![TradeItem { name: "Glass".into(), quantity: 8 }],
                },
            ],
            selected_trade: None,
            show_new_trade: false,
            new_trade_recipient: String::new(),
            new_item_name: String::new(),
            new_item_qty: 1,
        }
    }
}

/// Thread-local state for the trade page.
fn with_state<R>(f: impl FnOnce(&mut TradePageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<TradePageState> = RefCell::new(TradePageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
                            ts.show_new_trade = !ts.show_new_trade;
                        }
                    });
                });
            });
            ui.separator();

            // New trade form
            with_state(|ts| {
                if ts.show_new_trade {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new("Start New Trade")
                                .size(theme.font_size_body)
                                .color(theme.accent()),
                        );
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Recipient:").color(theme.text_secondary()));
                            ui.text_edit_singleline(&mut ts.new_trade_recipient);
                        });
                        ui.horizontal(|ui| {
                            if widgets::primary_button(ui, theme, "Create") {
                                if !ts.new_trade_recipient.trim().is_empty() {
                                    let next_id = ts.trades.len() as u64 + 1;
                                    ts.trades.push(Trade {
                                        id: next_id,
                                        partner_name: ts.new_trade_recipient.trim().to_string(),
                                        status: TradeStatus::Pending,
                                        your_items: Vec::new(),
                                        their_items: Vec::new(),
                                    });
                                    ts.new_trade_recipient.clear();
                                    ts.show_new_trade = false;
                                }
                            }
                            if widgets::secondary_button(ui, theme, "Cancel") {
                                ts.show_new_trade = false;
                            }
                        });
                    });
                    ui.add_space(theme.spacing_sm);
                }
            });

            // Main content: trade list + detail
            ui.columns(2, |cols| {
                // Left column: trade list
                cols[0].label(
                    RichText::new("Trades")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                ScrollArea::vertical()
                    .id_salt("trade_list")
                    .show(&mut cols[0], |ui| {
                        with_state(|ts| {
                            for (i, trade) in ts.trades.iter().enumerate() {
                                let selected = ts.selected_trade == Some(i);
                                let fill = if selected {
                                    theme.bg_card()
                                } else {
                                    Color32::TRANSPARENT
                                };
                                egui::Frame::none()
                                    .fill(fill)
                                    .rounding(Rounding::same(theme.border_radius as u8))
                                    .inner_margin(8.0)
                                    .show(ui, |ui| {
                                        let resp = ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&trade.partner_name)
                                                    .color(theme.text_primary()),
                                            );
                                            // Status badge
                                            let badge_color = trade.status.color(theme);
                                            egui::Frame::none()
                                                .fill(badge_color)
                                                .rounding(Rounding::same(3))
                                                .inner_margin(Vec2::new(6.0, 2.0))
                                                .show(ui, |ui| {
                                                    ui.label(
                                                        RichText::new(trade.status.label())
                                                            .size(theme.font_size_small)
                                                            .color(Color32::WHITE),
                                                    );
                                                });
                                        });
                                        if resp.response.interact(egui::Sense::click()).clicked() {
                                            ts.selected_trade = Some(i);
                                        }
                                    });
                            }
                        });
                    });

                // Right column: trade detail
                with_state(|ts| {
                    if let Some(idx) = ts.selected_trade {
                        if let Some(trade) = ts.trades.get(idx).cloned() {
                            cols[1].label(
                                RichText::new(format!("Trade with {}", trade.partner_name))
                                    .size(theme.font_size_body)
                                    .color(theme.accent()),
                            );
                            cols[1].add_space(theme.spacing_sm);

                            // Your items
                            cols[1].label(
                                RichText::new("Your Items:")
                                    .color(theme.text_secondary()),
                            );
                            if trade.your_items.is_empty() {
                                cols[1].label(
                                    RichText::new("  (none)")
                                        .color(theme.text_muted()),
                                );
                            }
                            for item in &trade.your_items {
                                cols[1].label(
                                    RichText::new(format!("  {} x{}", item.name, item.quantity))
                                        .color(theme.text_primary()),
                                );
                            }

                            cols[1].add_space(theme.spacing_sm);

                            // Their items
                            cols[1].label(
                                RichText::new("Their Items:")
                                    .color(theme.text_secondary()),
                            );
                            if trade.their_items.is_empty() {
                                cols[1].label(
                                    RichText::new("  (none)")
                                        .color(theme.text_muted()),
                                );
                            }
                            for item in &trade.their_items {
                                cols[1].label(
                                    RichText::new(format!("  {} x{}", item.name, item.quantity))
                                        .color(theme.text_primary()),
                                );
                            }

                            cols[1].add_space(theme.spacing_sm);

                            // Add item to your side
                            if trade.status == TradeStatus::Active || trade.status == TradeStatus::Pending {
                                cols[1].label(
                                    RichText::new("Add Item:")
                                        .color(theme.text_secondary()),
                                );
                                cols[1].horizontal(|ui| {
                                    ui.add(
                                        egui::TextEdit::singleline(&mut ts.new_item_name)
                                            .desired_width(100.0)
                                            .hint_text("Item name"),
                                    );
                                    ui.add(
                                        egui::DragValue::new(&mut ts.new_item_qty)
                                            .range(1..=999)
                                            .prefix("x"),
                                    );
                                    if widgets::primary_button(ui, theme, "Add") {
                                        if !ts.new_item_name.trim().is_empty() {
                                            if let Some(t) = ts.trades.get_mut(idx) {
                                                t.your_items.push(TradeItem {
                                                    name: ts.new_item_name.trim().to_string(),
                                                    quantity: ts.new_item_qty,
                                                });
                                            }
                                            ts.new_item_name.clear();
                                            ts.new_item_qty = 1;
                                        }
                                    }
                                });

                                // Remove items
                                let mut remove_idx = None;
                                if let Some(t) = ts.trades.get(idx) {
                                    for (item_i, item) in t.your_items.iter().enumerate() {
                                        cols[1].horizontal(|ui| {
                                            ui.label(
                                                RichText::new(format!("{} x{}", item.name, item.quantity))
                                                    .color(theme.text_primary()),
                                            );
                                            if widgets::danger_button(ui, theme, "X") {
                                                remove_idx = Some(item_i);
                                            }
                                        });
                                    }
                                }
                                if let Some(ri) = remove_idx {
                                    if let Some(t) = ts.trades.get_mut(idx) {
                                        t.your_items.remove(ri);
                                    }
                                }

                                cols[1].add_space(theme.spacing_md);

                                // Confirm / Cancel
                                cols[1].horizontal(|ui| {
                                    if widgets::primary_button(ui, theme, "Confirm") {
                                        if let Some(t) = ts.trades.get_mut(idx) {
                                            t.status = TradeStatus::Completed;
                                        }
                                    }
                                    if widgets::danger_button(ui, theme, "Cancel Trade") {
                                        if let Some(t) = ts.trades.get_mut(idx) {
                                            t.status = TradeStatus::Cancelled;
                                        }
                                    }
                                });
                            }
                        }
                    } else {
                        cols[1].centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Select a trade to view details")
                                    .color(theme.text_muted()),
                            );
                        });
                    }
                });
            });
        });
}
