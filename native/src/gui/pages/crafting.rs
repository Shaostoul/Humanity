//! Crafting page — recipe browser with ingredient check against inventory.

use egui::{RichText, Rounding, Stroke, Vec2};
use crate::gui::{GuiPage, GuiRecipe, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Categories for filtering recipes.
const CATEGORIES: &[&str] = &[
    "All", "smelting", "refining", "crafting", "cooking",
    "construction", "electronics", "assembly", "textile", "chemistry",
];

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Crafting")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(640.0, 480.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Crafting")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);

            // ── Category filter ──
            ui.horizontal(|ui| {
                for (i, cat) in CATEGORIES.iter().enumerate() {
                    let is_active = state.craft_category == i;
                    let text = if is_active {
                        RichText::new(*cat).size(theme.font_size_small).color(theme.text_on_accent())
                    } else {
                        RichText::new(*cat).size(theme.font_size_small).color(theme.text_secondary())
                    };
                    let fill = if is_active { theme.accent() } else { egui::Color32::TRANSPARENT };
                    let btn = egui::Button::new(text).fill(fill);
                    if ui.add(btn).clicked() {
                        state.craft_category = i;
                        state.craft_selected = None;
                    }
                }
            });

            ui.add_space(theme.spacing_sm);

            ui.horizontal(|ui| {
                // ── Left: recipe list ──
                ui.vertical(|ui| {
                    ui.set_min_width(220.0);
                    ui.set_max_width(220.0);

                    // Filter recipes by category
                    let filter = if state.craft_category == 0 {
                        None
                    } else {
                        Some(CATEGORIES[state.craft_category])
                    };

                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        let filtered: Vec<_> = state
                            .craft_recipes
                            .iter()
                            .enumerate()
                            .filter(|(_, r)| {
                                filter.map_or(true, |f| r.category == f)
                            })
                            .collect();

                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("No recipes found")
                                    .color(theme.text_muted()),
                            );
                        }

                        for (idx, recipe) in filtered {
                            let is_selected = state.craft_selected == Some(idx);
                            let fill = if is_selected { theme.bg_card() } else { egui::Color32::TRANSPARENT };
                            let stroke = if is_selected {
                                Stroke::new(1.0, theme.accent())
                            } else {
                                Stroke::NONE
                            };

                            let frame = egui::Frame::none()
                                .fill(fill)
                                .rounding(Rounding::same(4))
                                .stroke(stroke)
                                .inner_margin(6.0);

                            frame.show(ui, |ui| {
                                let resp = ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&recipe.name)
                                            .color(theme.text_primary()),
                                    );
                                    ui.label(
                                        RichText::new(&recipe.category)
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                }).response;
                                if resp.interact(egui::Sense::click()).clicked() {
                                    state.craft_selected = Some(idx);
                                }
                            });
                        }
                    });
                });

                ui.separator();

                // ── Right: recipe detail ──
                ui.vertical(|ui| {
                    if let Some(idx) = state.craft_selected {
                        if let Some(recipe) = state.craft_recipes.get(idx) {
                            let recipe = recipe.clone();

                            ui.label(
                                RichText::new(&recipe.name)
                                    .size(theme.font_size_heading)
                                    .color(theme.accent()),
                            );
                            ui.label(
                                RichText::new(&recipe.description)
                                    .color(theme.text_secondary()),
                            );
                            ui.add_space(theme.spacing_sm);

                            // Station
                            let station = if recipe.station_required.is_empty() {
                                "Hand-craftable"
                            } else {
                                &recipe.station_required
                            };
                            ui.label(
                                RichText::new(format!("Station: {}", station))
                                    .color(theme.text_muted()),
                            );
                            ui.label(
                                RichText::new(format!("Craft time: {}s", recipe.craft_time_sec))
                                    .color(theme.text_muted()),
                            );

                            ui.add_space(theme.spacing_md);

                            // Inputs
                            widgets::card_with_header(ui, theme, "Ingredients", |ui| {
                                for (item_id, qty) in &recipe.inputs {
                                    let have = count_in_inventory(state, item_id);
                                    let color = if have >= *qty {
                                        theme.success()
                                    } else {
                                        theme.danger()
                                    };
                                    ui.label(
                                        RichText::new(format!(
                                            "{} x{} (have: {})",
                                            item_id, qty, have
                                        ))
                                        .color(color),
                                    );
                                }
                            });

                            ui.add_space(theme.spacing_sm);

                            // Outputs
                            widgets::card_with_header(ui, theme, "Produces", |ui| {
                                for (item_id, qty) in &recipe.outputs {
                                    ui.label(
                                        RichText::new(format!("{} x{}", item_id, qty))
                                            .color(theme.text_primary()),
                                    );
                                }
                            });

                            ui.add_space(theme.spacing_md);

                            // Craft button — enabled only if all inputs are met
                            let can_craft = recipe.inputs.iter().all(|(item_id, qty)| {
                                count_in_inventory(state, item_id) >= *qty
                            });
                            ui.add_enabled_ui(can_craft, |ui| {
                                if widgets::primary_button(ui, theme, "Craft") {
                                    // Placeholder: would deduct items and start crafting timer
                                    state.craft_status = format!("Crafting {}...", recipe.name);
                                }
                            });
                            if !state.craft_status.is_empty() {
                                ui.label(
                                    RichText::new(&state.craft_status)
                                        .size(theme.font_size_small)
                                        .color(theme.success()),
                                );
                            }
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Select a recipe")
                                    .size(theme.font_size_body)
                                    .color(theme.text_muted()),
                            );
                        });
                    }
                });
            });

            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Close") {
                state.active_page = GuiPage::EscapeMenu;
            }
        });
}

/// Count how many of an item_id the player has in their inventory.
fn count_in_inventory(state: &GuiState, item_id: &str) -> u32 {
    state
        .inventory_items
        .iter()
        .filter_map(|slot| slot.as_ref())
        .filter(|item| item.item_id == item_id)
        .map(|item| item.quantity)
        .sum()
}
