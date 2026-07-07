//! Vendor modal (v0.747, closure ladder rung 3): the trading post's buy/sell
//! window. Opened from the trading post machine's walk-up card; prices come
//! from data/trade_goods.ron (player pays 1.25x base, receives 0.5x base).
//! Transactions settle in lib.rs's frame bridge via economy::vendor_buy/sell
//! against the ECS inventory + Wallet, so this page only reads GuiState and
//! records intents (the same pattern as every other page).

use egui::{Align2, RichText, ScrollArea};
use std::cell::RefCell;

use crate::gui::theme::Theme;
use crate::gui::{widgets, GuiState};

thread_local! {
    /// false = Buy tab, true = Sell tab.
    static SELL_TAB: RefCell<bool> = const { RefCell::new(false) };
}

pub fn draw_vendor_modal(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.vendor_open {
        return;
    }
    let mut open_flag = true;
    let mut sell_tab = SELL_TAB.with(|t| *t.borrow());
    egui::Window::new("Trading post")
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(egui::Vec2::new(460.0, 420.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .open(&mut open_flag)
        .show(ctx, |ui| {
            // Wallet + status header.
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} CR", state.wallet_credits))
                        .size(theme.font_size_heading)
                        .strong()
                        .color(theme.accent()),
                );
                ui.label(
                    RichText::new("buy 125% of base · sell 50% of base")
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
            });
            if !state.vendor_status.is_empty() {
                ui.label(
                    RichText::new(&state.vendor_status)
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                if ui.selectable_label(!sell_tab, "Buy").clicked() {
                    sell_tab = false;
                }
                if ui.selectable_label(sell_tab, "Sell").clicked() {
                    sell_tab = true;
                }
            });
            ui.separator();

            if sell_tab {
                // SELL: backpack items the vendor trades, at the receive price.
                let sellable: Vec<(String, String, u32, i64)> = state
                    .inventory_items
                    .iter()
                    .flatten()
                    .filter_map(|it| {
                        state
                            .vendor_goods
                            .iter()
                            .find(|g| g.id == it.item_id)
                            .map(|g| (it.item_id.clone(), it.name.clone(), it.quantity, g.sell_price))
                    })
                    .collect();
                ScrollArea::vertical().id_salt("vendor_sell").show(ui, |ui| {
                    if sellable.is_empty() {
                        ui.label(
                            RichText::new("Nothing in your pack the vendor trades.")
                                .color(theme.text_muted()),
                        );
                    }
                    for (id, name, qty, price) in &sellable {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{name} x{qty}"))
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::compact_button(ui, theme, "Sell all", widgets::ButtonVariant::Secondary) {
                                    state.pending_vendor_sell = Some((id.clone(), *qty));
                                }
                                if widgets::compact_button(ui, theme, "Sell 1", widgets::ButtonVariant::Secondary) {
                                    state.pending_vendor_sell = Some((id.clone(), 1));
                                }
                                ui.label(
                                    RichText::new(format!("{price} CR each"))
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                            });
                        });
                    }
                });
            } else {
                // BUY: the vendor catalog at the pay price.
                let goods = state.vendor_goods.clone();
                ScrollArea::vertical().id_salt("vendor_buy").show(ui, |ui| {
                    if goods.is_empty() {
                        ui.label(
                            RichText::new("The vendor has nothing in stock (trade_goods.ron).")
                                .color(theme.text_muted()),
                        );
                    }
                    let mut last_cat = String::new();
                    for g in &goods {
                        if g.category != last_cat {
                            last_cat = g.category.clone();
                            ui.add_space(theme.spacing_xs);
                            ui.label(
                                RichText::new(&g.category)
                                    .size(theme.font_size_small)
                                    .strong()
                                    .color(theme.text_muted()),
                            );
                        }
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&g.name)
                                    .size(theme.font_size_body)
                                    .color(theme.text_primary()),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::compact_button(ui, theme, "Buy", widgets::ButtonVariant::Primary) {
                                    state.pending_vendor_buy = Some((g.id.clone(), 1));
                                }
                                ui.label(
                                    RichText::new(format!("{} CR", g.buy_price))
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                            });
                        });
                    }
                });
            }
        });
    SELL_TAB.with(|t| *t.borrow_mut() = sell_tab);
    if !open_flag {
        state.vendor_open = false;
        state.vendor_status.clear();
    }
}
