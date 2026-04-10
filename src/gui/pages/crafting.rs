//! Crafting page — full crafting interface with sidebar categories, recipe list,
//! detail panel, and craft queue with progress bars.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke};
use crate::gui::{GuiState, GuiRecipe};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// Categories for filtering recipes.
const CATEGORIES: &[&str] = &[
    "All", "Smelting", "Refining", "Tools", "Weapons", "Furniture",
    "Cooking", "Textile", "Electronics", "Machines", "Vehicles", "Medical",
];

/// Map display categories to data categories for matching.
fn category_matches(filter: &str, recipe_cat: &str) -> bool {
    filter.eq_ignore_ascii_case(recipe_cat)
}

/// An item in the craft queue.
struct CraftQueueItem {
    recipe_name: String,
    craft_time: f32,
    elapsed: f32,
}

/// Page-local state for the crafting UI.
struct CraftPageState {
    search: String,
    queue: Vec<CraftQueueItem>,
    last_frame_time: f64,
}

impl Default for CraftPageState {
    fn default() -> Self {
        Self {
            search: String::new(),
            queue: Vec::new(),
            last_frame_time: 0.0,
        }
    }
}

thread_local! {
    static LOCAL: RefCell<CraftPageState> = RefCell::new(CraftPageState::default());
}

fn with_local<R>(f: impl FnOnce(&mut CraftPageState) -> R) -> R {
    LOCAL.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Advance queue timers
    let now = ctx.input(|i| i.time);
    with_local(|local| {
        let dt = if local.last_frame_time > 0.0 {
            (now - local.last_frame_time) as f32
        } else {
            0.0
        };
        local.last_frame_time = now;
        for item in &mut local.queue {
            item.elapsed += dt;
        }
        local.queue.retain(|item| item.elapsed < item.craft_time);
    });

    // Left sidebar: recipe categories
    egui::SidePanel::left("craft_categories")
        .default_width(160.0)
        .resizable(false)
        .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Categories")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical().show(ui, |ui| {
                if let Some(new_idx) = widgets::sidebar_nav(ui, theme, CATEGORIES, state.craft_category) {
                    state.craft_category = new_idx;
                    state.craft_selected = None;
                }
            });
        });

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Crafting")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);

            // Search bar
            with_local(|local| {
                widgets::search_bar(ui, theme, &mut local.search, "Filter recipes...");
            });
            ui.add_space(theme.spacing_sm);

            let available_h = ui.available_height();
            // Main content: recipe list (left) + detail (right), queue at bottom
            let main_h = (available_h - 120.0).max(200.0);

            ui.horizontal(|ui| {
                // Center: scrollable recipe list
                ui.vertical(|ui| {
                    ui.set_min_width(260.0);
                    ui.set_max_width(260.0);

                    let filter_cat = if state.craft_category == 0 {
                        None
                    } else {
                        Some(CATEGORIES[state.craft_category])
                    };

                    let search_term = with_local(|local| local.search.to_lowercase());

                    ScrollArea::vertical()
                        .id_salt("craft_recipe_list")
                        .max_height(main_h)
                        .show(ui, |ui| {
                            let filtered: Vec<(usize, GuiRecipe)> = state
                                .craft_recipes
                                .iter()
                                .enumerate()
                                .filter(|(_, r)| {
                                    filter_cat.map_or(true, |f| category_matches(f, &r.category))
                                })
                                .filter(|(_, r)| {
                                    search_term.is_empty()
                                        || r.name.to_lowercase().contains(&search_term)
                                        || r.id.to_lowercase().contains(&search_term)
                                })
                                .map(|(i, r)| (i, r.clone()))
                                .collect();

                            if filtered.is_empty() {
                                ui.add_space(theme.spacing_md);
                                ui.label(
                                    RichText::new("No recipes match your filter")
                                        .color(theme.text_muted()),
                                );
                            }

                            for (idx, recipe) in &filtered {
                                let is_selected = state.craft_selected == Some(*idx);
                                let fill = if is_selected { theme.bg_card() } else { Color32::TRANSPARENT };
                                let stroke = if is_selected {
                                    Stroke::new(1.0, theme.accent())
                                } else {
                                    Stroke::NONE
                                };

                                let frame = egui::Frame::none()
                                    .fill(fill)
                                    .rounding(Rounding::same(4))
                                    .stroke(stroke)
                                    .inner_margin(theme.panel_margin);

                                frame.show(ui, |ui| {
                                    let resp = ui.vertical(|ui| {
                                        ui.label(
                                            RichText::new(&recipe.name)
                                                .size(theme.font_size_body)
                                                .color(theme.text_primary()),
                                        );
                                        // Inputs summary
                                        let inputs_str: String = recipe
                                            .inputs
                                            .iter()
                                            .map(|(id, qty)| {
                                                let have = count_in_inventory(state, id);
                                                format!("{} {}/{}", id, have, qty)
                                            })
                                            .collect::<Vec<_>>()
                                            .join(", ");
                                        ui.label(
                                            RichText::new(inputs_str)
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                        ui.horizontal(|ui| {
                                            ui.label(
                                                RichText::new(&recipe.category)
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_muted()),
                                            );
                                            ui.label(
                                                RichText::new(format!("{}s", recipe.craft_time_sec))
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_muted()),
                                            );
                                        });
                                    }).response;
                                    if resp.interact(egui::Sense::click()).clicked() {
                                        state.craft_selected = Some(*idx);
                                    }
                                });
                                ui.add_space(theme.row_gap);
                            }
                        });
                });

                ui.separator();

                // Right panel: selected recipe detail
                ui.vertical(|ui| {
                    ScrollArea::vertical()
                        .id_salt("craft_detail")
                        .max_height(main_h)
                        .show(ui, |ui| {
                            if let Some(idx) = state.craft_selected {
                                if let Some(recipe) = state.craft_recipes.get(idx) {
                                    let recipe = recipe.clone();
                                    draw_recipe_detail(ui, theme, state, &recipe);
                                } else {
                                    state.craft_selected = None;
                                }
                            } else {
                                ui.add_space(theme.spacing_xl);
                                ui.centered_and_justified(|ui| {
                                    ui.label(
                                        RichText::new("Select a recipe to view details")
                                            .size(theme.font_size_body)
                                            .color(theme.text_muted()),
                                    );
                                });
                            }
                        });
                });
            });

            ui.add_space(theme.spacing_sm);
            ui.separator();
            ui.add_space(theme.spacing_xs);

            // Craft queue at bottom
            ui.label(
                RichText::new("Craft Queue")
                    .size(theme.font_size_heading)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_xs);

            with_local(|local| {
                if local.queue.is_empty() {
                    ui.label(
                        RichText::new("No active crafts")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                } else {
                    for item in &local.queue {
                        ui.horizontal(|ui| {
                            ui.set_min_width(200.0);
                            ui.label(
                                RichText::new(&item.recipe_name)
                                    .size(theme.font_size_small)
                                    .color(theme.text_primary()),
                            );
                            let progress = (item.elapsed / item.craft_time).clamp(0.0, 1.0);
                            let remaining = (item.craft_time - item.elapsed).max(0.0);
                            ui.add(
                                egui::ProgressBar::new(progress)
                                    .fill(theme.accent())
                                    .text(format!("{:.1}s remaining", remaining)),
                            );
                        });
                    }
                }
            });

            // Status message
            if !state.craft_status.is_empty() {
                ui.label(
                    RichText::new(&state.craft_status)
                        .size(theme.font_size_small)
                        .color(theme.success()),
                );
            }
        });

    // Request repaint while queue has items
    with_local(|local| {
        if !local.queue.is_empty() {
            ctx.request_repaint();
        }
    });
}

fn draw_recipe_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, recipe: &GuiRecipe) {
    ui.label(
        RichText::new(&recipe.name)
            .size(theme.font_size_heading)
            .color(theme.accent()),
    );
    if !recipe.description.is_empty() {
        ui.label(
            RichText::new(&recipe.description)
                .color(theme.text_secondary()),
        );
    }
    ui.add_space(theme.spacing_sm);

    // Station & time
    let station = if recipe.station_required.is_empty() {
        "Hand-craftable"
    } else {
        &recipe.station_required
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new("Station:").size(theme.font_size_small).color(theme.text_muted()));
        ui.label(RichText::new(station).size(theme.font_size_small).color(theme.text_primary()));
    });
    ui.horizontal(|ui| {
        ui.label(RichText::new("Craft time:").size(theme.font_size_small).color(theme.text_muted()));
        ui.label(RichText::new(format!("{}s", recipe.craft_time_sec)).size(theme.font_size_small).color(theme.text_primary()));
    });

    ui.add_space(theme.spacing_md);

    // Ingredients
    widgets::card_with_header(ui, theme, "Ingredients", |ui| {
        for (item_id, qty) in &recipe.inputs {
            let have = count_in_inventory(state, item_id);
            let enough = have >= *qty;
            let have_color = if enough { theme.success() } else { theme.danger() };
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(item_id)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
                ui.label(
                    RichText::new(format!("{}", have))
                        .size(theme.font_size_body)
                        .color(have_color),
                );
                ui.label(
                    RichText::new(format!("/ {}", qty))
                        .size(theme.font_size_body)
                        .color(theme.text_muted()),
                );
            });
        }
    });

    ui.add_space(theme.spacing_sm);

    // Outputs
    widgets::card_with_header(ui, theme, "Produces", |ui| {
        for (item_id, qty) in &recipe.outputs {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} x{}", item_id, qty))
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
            });
        }
    });

    ui.add_space(theme.spacing_md);

    // Craft button
    let can_craft = recipe.inputs.iter().all(|(item_id, qty)| {
        count_in_inventory(state, item_id) >= *qty
    });

    ui.add_enabled_ui(can_craft, |ui| {
        if widgets::primary_button(ui, theme, "Craft") {
            state.craft_status = format!("Started crafting {}", recipe.name);
            with_local(|local| {
                local.queue.push(CraftQueueItem {
                    recipe_name: recipe.name.clone(),
                    craft_time: recipe.craft_time_sec.max(1.0),
                    elapsed: 0.0,
                });
            });
        }
    });

    if !can_craft {
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new("Missing ingredients")
                .size(theme.font_size_small)
                .color(theme.danger()),
        );
    }
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
