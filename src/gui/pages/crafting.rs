//! Crafting page — full crafting interface with sidebar categories, recipe list,
//! detail panel, and craft queue with progress bars.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke};
use crate::gui::{GuiState, GuiRecipe};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

// Crafting categories are loaded from `data/crafting/categories.json` into
// `GuiState.crafting_categories` at startup.

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
                // Data-driven collapsible group -> category tree (from
                // data/crafting/categories.json). Snapshot the current selection into
                // a local + defer the new selection, so no `state` borrow straddles
                // the nested collapsing-header closures.
                let current = state.craft_selected_category.clone();
                let groups = state.crafting_category_groups.clone();
                let mut newly_selected: Option<Option<String>> = None;

                if ui
                    .selectable_label(
                        current.is_none(),
                        RichText::new("All")
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    )
                    .clicked()
                {
                    newly_selected = Some(None);
                }
                ui.add_space(theme.spacing_xs);

                for group in &groups {
                    egui::CollapsingHeader::new(
                        RichText::new(&group.name)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    )
                    .id_salt(&group.name)
                    .default_open(true)
                    .show(ui, |ui| {
                        for cat in &group.categories {
                            let selected = current.as_deref() == Some(cat.as_str());
                            if ui
                                .selectable_label(
                                    selected,
                                    RichText::new(cat)
                                        .size(theme.font_size_body)
                                        .color(theme.text_primary()),
                                )
                                .clicked()
                            {
                                newly_selected = Some(Some(cat.clone()));
                            }
                        }
                    });
                }

                if let Some(sel) = newly_selected {
                    state.craft_selected_category = sel;
                    state.craft_selected = None;
                }
            });
        });

    // Owned filter/search captured before the panels so the immutable borrows of
    // `state` don't straddle the mutable closures below. `craft_selected_category`
    // = None means "All" (no category filter).
    let filter_cat: Option<String> = state.craft_selected_category.clone();
    let search_term = with_local(|local| local.search.to_lowercase());

    // ── Right panel: selected recipe detail (full height, resizable) ──
    // A SidePanel has a real bounded height, so the detail fills the screen instead
    // of collapsing to its content height (the old in-CentralPanel layout relied on
    // ui.available_height(), which came back tiny and cramped everything).
    egui::SidePanel::right("craft_detail_panel")
        .default_width(480.0)
        .min_width(300.0)
        .resizable(true)
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .id_salt("craft_detail")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if let Some(idx) = state.craft_selected {
                        if let Some(recipe) = state.craft_recipes.get(idx).cloned() {
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

    // ── Bottom strip: craft queue ──
    egui::TopBottomPanel::bottom("craft_queue")
        .resizable(false)
        .min_height(72.0)
        .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
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
            if !state.craft_status.is_empty() {
                ui.label(
                    RichText::new(&state.craft_status)
                        .size(theme.font_size_small)
                        .color(theme.success()),
                );
            }
        });

    // ── Center: recipe list (fills the remaining space) ──
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Crafting")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_sm);
            with_local(|local| {
                widgets::search_bar(ui, theme, &mut local.search, "Filter recipes...");
            });
            // Dev/creative provisioning (the "develop as if 100% unlocked" posture):
            // stock the player with one stack of every recipe input (raws AND
            // intermediates) so EVERY recipe is craftable in one click right now.
            // Gated/removed once progression lands.
            if widgets::primary_button(ui, theme, "Dev: stock all materials") {
                state.dev_stock_materials = true;
                state.craft_status = "Stocking all materials...".to_string();
            }
            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical()
                .id_salt("craft_recipe_list")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let filtered: Vec<(usize, GuiRecipe)> = state
                        .craft_recipes
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| {
                            filter_cat.as_deref().map_or(true, |f| category_matches(f, &r.category))
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
                            .rounding(Rounding::same(3))
                            .stroke(stroke)
                            .inner_margin(4.0);

                        // Compact two-line row (name + craft-time on line 1, ingredient
                        // have/need on line 2; the redundant category line is dropped —
                        // it's the sidebar selection). Whole frame is clickable + full
                        // width. Roughly half the old three-line height so far more
                        // recipes are visible at once.
                        let inner = frame.show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&recipe.name)
                                        .size(theme.font_size_body)
                                        .color(theme.text_primary()),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            RichText::new(format!("{}s", recipe.craft_time_sec))
                                                .size(theme.font_size_small)
                                                .color(theme.text_muted()),
                                        );
                                    },
                                );
                            });
                            let inputs_str: String = recipe
                                .inputs
                                .iter()
                                .map(|(id, qty)| {
                                    let have = count_in_inventory(state, id);
                                    format!("{} {}/{}", id, have, qty)
                                })
                                .collect::<Vec<_>>()
                                .join(", ");
                            if !inputs_str.is_empty() {
                                ui.label(
                                    RichText::new(inputs_str)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            }
                        });
                        if inner.response.interact(egui::Sense::click()).clicked() {
                            state.craft_selected = Some(*idx);
                        }
                        ui.add_space(1.0);
                    }
                });
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
            // Real craft: hand the recipe id to the ECS CraftingSystem (via the
            // main-loop bridge -> DataStore "craft_request"), which consumes inputs
            // and produces outputs on the player's actual inventory. The local queue
            // below is just a visual progress indicator.
            state.pending_craft_recipe = Some(recipe.id.clone());
            state.craft_status = format!("Crafting {}...", recipe.name);
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
