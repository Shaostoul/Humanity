//! Inventory grid with item slots, equipment section, weight tracking,
//! item detail panel, and quick actions.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

const COLS: usize = 8;

// Equipment slot definitions are loaded from `data/inventory/equipment_slots.json`
// into `GuiState.equipment_slots` at startup (see `lib.rs`). The equipped Vec is
// populated lazily on the first draw where slots are available.

/// Page-local state for the inventory.
struct InventoryPageState {
    /// Equipped items (slot_name -> item name or empty).
    equipped: Vec<(String, Option<String>)>,
    /// Current carry weight.
    carry_weight: f32,
    /// Max carry weight.
    max_carry_weight: f32,
    /// Whether we have initialized sample data.
    initialized: bool,
}

impl Default for InventoryPageState {
    fn default() -> Self {
        Self {
            // Populated from gui_state.equipment_slots on first draw.
            equipped: Vec::new(),
            carry_weight: 0.0,
            max_carry_weight: 50.0,
            initialized: false,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut InventoryPageState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<InventoryPageState> = RefCell::new(InventoryPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

/// Get a color based on item category.
fn category_color(category: &str) -> Color32 {
    match category {
        "clothing" => Color32::from_rgb(100, 140, 200),
        "tool" => Color32::from_rgb(180, 150, 80),
        "weapon" => Color32::from_rgb(200, 80, 80),
        "furniture" => Color32::from_rgb(140, 120, 100),
        "food" => Color32::from_rgb(80, 180, 80),
        "material" => Color32::from_rgb(160, 160, 140),
        "machine" => Color32::from_rgb(140, 140, 180),
        "vehicle" => Color32::from_rgb(180, 100, 180),
        _ => Color32::from_rgb(120, 120, 120),
    }
}

/// Short, capitalized ore name for display: "iron_ore_0" -> "Iron".
fn ore_short(item_id: &str) -> String {
    let base = item_id.strip_suffix("_0").unwrap_or(item_id);
    let base = base.strip_suffix("_ore").unwrap_or(base);
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => base.to_string(),
    }
}

/// Parse item data from embedded CSV to get details for a given item_id.
fn lookup_item_details(item_id: &str) -> Option<ItemDetails> {
    let csv = crate::embedded_data::ITEMS_CSV;
    for line in csv.lines() {
        if line.starts_with('#') || line.starts_with("id,") || line.is_empty() {
            continue;
        }
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() >= 9 && fields[0] == item_id {
            return Some(ItemDetails {
                name: fields[1].to_string(),
                category: fields[2].to_string(),
                subcategory: fields[3].to_string(),
                base_material: fields[4].to_string(),
                weight_kg: fields[5].parse().unwrap_or(0.0),
                stack_size: fields[6].parse().unwrap_or(1),
                durability: fields[7].parse().unwrap_or(0),
                description: fields[8].to_string(),
            });
        }
    }
    None
}

#[derive(Debug, Clone)]
struct ItemDetails {
    name: String,
    category: String,
    subcategory: String,
    base_material: String,
    weight_kg: f32,
    stack_size: u32,
    durability: u32,
    description: String,
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let total_slots = state.inventory_max_slots.max(1);

    // Calculate carry weight from inventory items + populate equipped slots from
    // the loaded `data/inventory/equipment_slots.json` (lazily — guards against
    // the GUI rendering before lib.rs has wired the loaded data into GuiState).
    with_state(|ps| {
        if ps.equipped.is_empty() && !state.equipment_slots.is_empty() {
            ps.equipped = state.equipment_slots.iter()
                .map(|(id, _)| (id.clone(), None))
                .collect();
        }
        if !ps.initialized {
            let mut weight = 0.0f32;
            for slot in &state.inventory_items {
                if let Some(item) = slot {
                    if let Some(details) = lookup_item_details(&item.item_id) {
                        weight += details.weight_kg * item.quantity as f32;
                    }
                }
            }
            ps.carry_weight = weight;
            ps.initialized = true;
        }
    });

    // Right side panel: item detail
    let mut action_drop = false;
    let mut action_equip = false;
    // Set inside the panel/central closures (which borrow `state`); applied after
    // so we can mutate GuiState. action_eat/action_plant come from the detail panel;
    // the crop actions come from the Garden section in the central panel.
    let mut action_eat: Option<String> = None;
    let mut action_drink: Option<String> = None;
    let mut action_plant: Option<String> = None;
    let mut action_water_crop: Option<u64> = None;
    let mut action_harvest_crop: Option<u64> = None;
    let mut action_dev_grow = false;
    let mut action_commission_ore: Option<String> = None;
    let mut action_rest = false;
    let mut action_compost = false;
    let mut action_fertilize_crop: Option<u64> = None;

    if let Some(idx) = state.selected_slot {
        egui::SidePanel::right("inv_detail_panel")
            .min_width(220.0)
            .max_width(280.0)
            .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(10.0))
            .show(ctx, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    if let Some(Some(item)) = state.inventory_items.get(idx) {
                        ui.label(RichText::new(&item.name).size(theme.font_size_title).color(theme.accent()));
                        ui.add_space(theme.spacing_xs);

                        // Look up full item details
                        if let Some(details) = lookup_item_details(&item.item_id) {
                            // Category badge
                            let cat_col = category_color(&details.category);
                            egui::Frame::none()
                                .fill(cat_col)
                                .rounding(Rounding::same(3))
                                .inner_margin(Vec2::new(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(&details.category).size(theme.font_size_small).color(Color32::WHITE));
                                });

                            ui.add_space(theme.spacing_sm);

                            widgets::card(ui, theme, |ui| {
                                crate::gui::widgets::detail_row(ui, theme, "ID", &item.item_id);
                                crate::gui::widgets::detail_row(ui, theme, "Quantity", &item.quantity.to_string());
                                crate::gui::widgets::detail_row(ui, theme, "Category", &details.category);
                                crate::gui::widgets::detail_row(ui, theme, "Subcategory", &details.subcategory);
                                crate::gui::widgets::detail_row(ui, theme, "Weight", &format!("{:.2} kg", details.weight_kg));
                                crate::gui::widgets::detail_row(ui, theme, "Stack Size", &details.stack_size.to_string());
                                crate::gui::widgets::detail_row(ui, theme, "Material", &details.base_material);
                                if details.durability > 0 {
                                    crate::gui::widgets::detail_row(ui, theme, "Durability", &details.durability.to_string());
                                }
                            });

                            // Description
                            if !details.description.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(RichText::new(&details.description).color(theme.text_secondary()).size(theme.font_size_small));
                            }
                        } else {
                            widgets::card(ui, theme, |ui| {
                                crate::gui::widgets::detail_row(ui, theme, "ID", &item.item_id);
                                crate::gui::widgets::detail_row(ui, theme, "Quantity", &item.quantity.to_string());
                            });
                        }

                        ui.add_space(theme.spacing_md);

                        // Quick actions
                        ui.label(RichText::new("Actions").size(theme.font_size_body).color(theme.text_secondary()));
                        ui.add_space(theme.spacing_xs);
                        ui.horizontal(|ui| {
                            // Food items get a real "Eat" action (drives the nutrition
                            // loop), seeds get "Plant" (drives the gardening loop); all
                            // else keeps the placeholder "Use".
                            let details = lookup_item_details(&item.item_id);
                            let is_drink = details
                                .as_ref()
                                .map(|d| d.subcategory == "drink" || d.subcategory == "liquid")
                                .unwrap_or(false)
                                || item.item_id.starts_with("water_");
                            // Drinks are category "food" too, so check drink FIRST.
                            let is_food = details
                                .as_ref()
                                .map(|d| d.category == "food")
                                .unwrap_or(false)
                                && !is_drink;
                            let is_seed = details
                                .as_ref()
                                .map(|d| d.subcategory == "seed")
                                .unwrap_or(false)
                                || item.item_id.starts_with("seed_");
                            if is_drink {
                                if widgets::primary_button(ui, theme, "Drink") {
                                    action_drink = Some(item.item_id.clone());
                                }
                            } else if is_food {
                                if widgets::primary_button(ui, theme, "Eat") {
                                    action_eat = Some(item.item_id.clone());
                                }
                            } else if is_seed {
                                if widgets::primary_button(ui, theme, "Plant") {
                                    action_plant = Some(item.item_id.clone());
                                }
                            } else if widgets::primary_button(ui, theme, "Use") {
                                // Placeholder for non-food/non-seed/non-drink use.
                            }
                            if widgets::secondary_button(ui, theme, "Equip") {
                                action_equip = true;
                            }
                            if widgets::danger_button(ui, theme, "Drop") {
                                action_drop = true;
                            }
                        });
                    } else {
                        ui.label(RichText::new("Empty Slot").size(theme.font_size_heading).color(theme.text_muted()));
                        ui.add_space(theme.spacing_sm);
                        ui.label(RichText::new("Select an item to view details.").color(theme.text_muted()));
                    }
                });
            });
    }

    // Handle drop action
    if action_drop {
        if let Some(idx) = state.selected_slot {
            if idx < state.inventory_items.len() {
                state.inventory_items[idx] = None;
                state.selected_slot = None;
                with_state(|ps| ps.initialized = false); // recalc weight
            }
        }
    }

    // Handle equip action (placeholder: just note the equipped item)
    if action_equip {
        if let Some(idx) = state.selected_slot {
            if let Some(Some(item)) = state.inventory_items.get(idx) {
                let item_name = item.name.clone();
                with_state(|ps| {
                    // Try to equip to first empty slot
                    for slot in &mut ps.equipped {
                        if slot.1.is_none() {
                            slot.1 = Some(item_name.clone());
                            break;
                        }
                    }
                });
            }
        }
    }

    // Handle eat action — bridge to FoodSystem via GuiState (the main loop forwards
    // pending_consume_item into the consume_request DataStore channel before the tick).
    if let Some(item_id) = action_eat {
        state.pending_consume_item = Some(item_id);
    }
    // Handle drink action — bridge to FoodSystem (restores hydration).
    if let Some(item_id) = action_drink {
        state.pending_drink_item = Some(item_id);
    }
    // Handle plant action — bridge to FarmingSystem (consumes the seed, spawns a crop).
    if let Some(seed_id) = action_plant {
        state.pending_plant_seed = Some(seed_id);
    }

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            // Header with slot count
            let used = state.inventory_items.iter().filter(|s| s.is_some()).count();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Inventory").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}/{} slots", used, total_slots)).color(theme.text_muted()));
                });
            });
            ui.add_space(theme.spacing_xs);

            // Weight indicator
            let (carry_weight, max_weight) = with_state(|ps| (ps.carry_weight, ps.max_carry_weight));
            let weight_frac = if max_weight > 0.0 { carry_weight / max_weight } else { 0.0 };
            let weight_color = if weight_frac > 0.9 {
                theme.danger()
            } else if weight_frac > 0.7 {
                theme.warning()
            } else {
                theme.accent()
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new("Weight:").color(theme.text_secondary()));
                ui.label(RichText::new(format!("{:.1} / {:.1} kg", carry_weight, max_weight)).color(weight_color));
            });
            let bar = egui::ProgressBar::new(weight_frac.clamp(0.0, 1.0))
                .fill(weight_color);
            ui.add(bar);

            // ── Survival vitals: satiation / hydration + active status effects ──
            // (synced from the player's ECS Vitals + StatusEffects each frame).
            let (sat, sat_max, hyd, hyd_max, energy, energy_max) = (
                state.vitals.satiation,
                state.vitals.satiation_max,
                state.vitals.hydration,
                state.vitals.hydration_max,
                state.vitals.energy,
                state.vitals.energy_max,
            );
            if sat_max > 0.0 {
                let effects = state.vitals.effects.clone();
                let sat_frac = (sat / sat_max).clamp(0.0, 1.0);
                let hyd_frac = (hyd / hyd_max.max(1.0)).clamp(0.0, 1.0);
                let energy_frac = (energy / energy_max.max(1.0)).clamp(0.0, 1.0);
                let color_for = |frac: f32| {
                    if frac < 0.25 {
                        theme.danger()
                    } else if frac < 0.5 {
                        theme.warning()
                    } else {
                        theme.accent()
                    }
                };
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Satiation:").color(theme.text_secondary()));
                    ui.label(
                        RichText::new(format!("{:.0} / {:.0}", sat, sat_max)).color(color_for(sat_frac)),
                    );
                });
                ui.add(egui::ProgressBar::new(sat_frac).fill(color_for(sat_frac)));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Hydration:").color(theme.text_secondary()));
                    ui.label(
                        RichText::new(format!("{:.0} / {:.0}", hyd, hyd_max)).color(color_for(hyd_frac)),
                    );
                });
                ui.add(egui::ProgressBar::new(hyd_frac).fill(color_for(hyd_frac)));
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Energy:").color(theme.text_secondary()));
                    ui.label(
                        RichText::new(format!("{:.0} / {:.0}", energy, energy_max))
                            .color(color_for(energy_frac)),
                    );
                    // Rest refills energy + clears fatigue (instant for now).
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::secondary_button(ui, theme, "Rest") {
                            action_rest = true;
                        }
                    });
                });
                ui.add(egui::ProgressBar::new(energy_frac).fill(color_for(energy_frac)));
                // Oxygen + body temperature + seal status (survival #7b).
                let oxy = state.vitals.oxygen;
                let oxy_max = state.vitals.oxygen_max.max(1.0);
                let oxy_frac = (oxy / oxy_max).clamp(0.0, 1.0);
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Oxygen:").color(theme.text_secondary()));
                    ui.label(
                        RichText::new(format!("{:.0} / {:.0}", oxy, oxy_max)).color(color_for(oxy_frac)),
                    );
                });
                ui.add(egui::ProgressBar::new(oxy_frac).fill(color_for(oxy_frac)));
                let temp = state.vitals.body_temp_c;
                let temp_col = if temp < 35.0 || temp > 39.0 {
                    theme.danger()
                } else if temp < 36.0 || temp > 38.0 {
                    theme.warning()
                } else {
                    theme.accent()
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Body temp:").color(theme.text_secondary()));
                    ui.label(RichText::new(format!("{:.1}°C", temp)).color(temp_col));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if state.vitals.sealed {
                            ui.label(
                                RichText::new("Sealed")
                                    .size(theme.font_size_small)
                                    .color(theme.accent()),
                            );
                        } else {
                            ui.label(
                                RichText::new("EXPOSED — no air!")
                                    .size(theme.font_size_small)
                                    .color(theme.danger()),
                            );
                        }
                    });
                });
                // Waste / sanitation: rises over time + when eating; Compost → fertilizer.
                let waste = state.vitals.waste;
                let waste_max = state.vitals.waste_max.max(1.0);
                let waste_frac = (waste / waste_max).clamp(0.0, 1.0);
                // High waste is BAD here (inverted color vs the other vitals).
                let waste_col = if waste_frac > 0.75 {
                    theme.danger()
                } else if waste_frac > 0.5 {
                    theme.warning()
                } else {
                    theme.text_secondary()
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Waste:").color(theme.text_secondary()));
                    ui.label(
                        RichText::new(format!("{:.0} / {:.0}", waste, waste_max)).color(waste_col),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::secondary_button(ui, theme, "Compost") {
                            action_compost = true;
                        }
                    });
                });
                ui.add(egui::ProgressBar::new(waste_frac).fill(waste_col));
                if !effects.is_empty() {
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("Effects:").color(theme.text_secondary()));
                        for (name, remaining) in &effects {
                            let label = if *remaining >= 60.0 {
                                format!("{} ({:.0}m)", name, remaining / 60.0)
                            } else {
                                format!("{} ({:.0}s)", name, remaining)
                            };
                            egui::Frame::none()
                                .fill(theme.bg_secondary())
                                .rounding(Rounding::same(3))
                                .inner_margin(Vec2::new(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(label)
                                            .size(theme.font_size_small)
                                            .color(theme.text_primary()),
                                    );
                                });
                        }
                    });
                }
            }

            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical().show(ui, |ui| {
                // Equipment section
                ui.label(RichText::new("Equipment").size(theme.font_size_heading).color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);

                ui.horizontal_wrapped(|ui| {
                    with_state(|ps| {
                        for (slot_id, equipped_item) in &ps.equipped {
                            let label = state.equipment_slots.iter()
                                .find(|(id, _)| id == slot_id)
                                .map(|(_, name)| name.as_str())
                                .unwrap_or(slot_id.as_str());

                            let slot_size = 64.0;
                            ui.vertical(|ui| {
                                let (rect, _response) = ui.allocate_exact_size(
                                    Vec2::splat(slot_size),
                                    egui::Sense::click(),
                                );

                                let fill = theme.bg_secondary();
                                let stroke = Stroke::new(1.0, theme.border());
                                ui.painter().rect_filled(rect, Rounding::same(4), fill);
                                ui.painter().rect_stroke(rect, Rounding::same(4), stroke, egui::StrokeKind::Outside);

                                if let Some(item_name) = equipped_item {
                                    let icon = item_name.chars().next().unwrap_or('?').to_string();
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        &icon,
                                        egui::FontId::proportional(18.0),
                                        theme.text_primary(),
                                    );
                                } else {
                                    ui.painter().text(
                                        rect.center(),
                                        egui::Align2::CENTER_CENTER,
                                        "-",
                                        egui::FontId::proportional(14.0),
                                        theme.text_muted(),
                                    );
                                }

                                ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_muted()));
                            });
                        }
                    });
                });

                ui.add_space(theme.spacing_md);
                ui.separator();
                ui.add_space(theme.spacing_sm);

                // Main inventory grid
                ui.label(RichText::new("Backpack").size(theme.font_size_heading).color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);

                let slot_size = 52.0;
                egui::Grid::new("inv_grid")
                    .spacing(Vec2::splat(4.0))
                    .show(ui, |ui| {
                        for i in 0..total_slots {
                            let selected = state.selected_slot == Some(i);
                            let stroke = if selected {
                                Stroke::new(2.0, theme.accent())
                            } else {
                                Stroke::new(1.0, theme.border())
                            };

                            let (rect, response) = ui.allocate_exact_size(
                                Vec2::splat(slot_size),
                                egui::Sense::click(),
                            );

                            if response.clicked() {
                                state.selected_slot = if selected { None } else { Some(i) };
                            }

                            // Slot background with category-colored border if item present
                            let fill = if selected { theme.bg_card() } else { theme.bg_secondary() };
                            ui.painter().rect_filled(rect, Rounding::same(4), fill);
                            ui.painter().rect_stroke(rect, Rounding::same(4), stroke, egui::StrokeKind::Outside);

                            // Draw item if slot is occupied
                            if let Some(Some(item)) = state.inventory_items.get(i) {
                                // Category-colored square icon
                                let details = lookup_item_details(&item.item_id);
                                let cat_color = details.as_ref()
                                    .map(|d| category_color(&d.category))
                                    .unwrap_or(Color32::from_rgb(120, 120, 120));

                                let icon_size = 22.0;
                                let icon_rect = egui::Rect::from_center_size(
                                    rect.center() - Vec2::new(0.0, 4.0),
                                    Vec2::splat(icon_size),
                                );
                                ui.painter().rect_filled(icon_rect, Rounding::same(3), cat_color);

                                // Item initial on top of colored square
                                let icon = item.name.chars().next().unwrap_or('?').to_string();
                                ui.painter().text(
                                    icon_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    &icon,
                                    egui::FontId::proportional(12.0),
                                    Color32::WHITE,
                                );

                                // Item name below
                                let name_preview: String = item.name.chars().take(6).collect();
                                ui.painter().text(
                                    rect.center() + Vec2::new(0.0, 14.0),
                                    egui::Align2::CENTER_CENTER,
                                    &name_preview,
                                    egui::FontId::proportional(8.0),
                                    theme.text_muted(),
                                );

                                // Stack count in bottom-right
                                if item.quantity > 1 {
                                    ui.painter().text(
                                        rect.right_bottom() + Vec2::new(-4.0, -2.0),
                                        egui::Align2::RIGHT_BOTTOM,
                                        item.quantity.to_string(),
                                        egui::FontId::proportional(10.0),
                                        theme.text_primary(),
                                    );
                                }
                            }

                            if (i + 1) % COLS == 0 {
                                ui.end_row();
                            }
                        }
                    });

                // ── Garden: crops growing in the world (plant via a seed's "Plant"
                //    action; FarmingSystem advances growth from game time + water). ──
                ui.add_space(theme.spacing_md);
                ui.separator();
                ui.add_space(theme.spacing_sm);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Garden")
                            .size(theme.font_size_heading)
                            .color(theme.text_primary()),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Dev/testing affordance: skip the game-day wait for growth.
                        if widgets::secondary_button(ui, theme, "Dev: grow all") {
                            action_dev_grow = true;
                        }
                    });
                });
                ui.add_space(theme.spacing_xs);
                if state.crops.is_empty() {
                    ui.label(
                        RichText::new("No crops yet — plant a seed from your inventory.")
                            .color(theme.text_muted()),
                    );
                } else {
                    for crop in &state.crops {
                        widgets::card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                let (title, title_col) = if crop.dead {
                                    (format!("{} (dead)", crop.name), theme.danger())
                                } else if crop.mature {
                                    (format!("{} — ready to harvest", crop.name), theme.accent())
                                } else {
                                    (format!("{} — {}", crop.name, crop.stage), theme.text_primary())
                                };
                                ui.label(RichText::new(title).color(title_col));
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if crop.mature
                                            && widgets::primary_button(ui, theme, "Harvest")
                                        {
                                            action_harvest_crop = Some(crop.entity_bits);
                                        }
                                        if !crop.dead
                                            && widgets::secondary_button(ui, theme, "Water")
                                        {
                                            action_water_crop = Some(crop.entity_bits);
                                        }
                                        if !crop.dead
                                            && widgets::secondary_button(ui, theme, "Fertilize")
                                        {
                                            action_fertilize_crop = Some(crop.entity_bits);
                                        }
                                    },
                                );
                            });
                            ui.add(
                                egui::ProgressBar::new(crop.progress.clamp(0.0, 1.0))
                                    .fill(theme.accent())
                                    .text("growth"),
                            );
                            ui.horizontal(|ui| {
                                let wcol = if crop.water < 0.2 {
                                    theme.danger()
                                } else {
                                    theme.text_secondary()
                                };
                                ui.label(
                                    RichText::new(format!("Water {:.0}%", crop.water * 100.0))
                                        .size(theme.font_size_small)
                                        .color(wcol),
                                );
                                ui.label(
                                    RichText::new(format!("Health {:.0}%", crop.health))
                                        .size(theme.font_size_small)
                                        .color(theme.text_secondary()),
                                );
                            });
                        });
                        ui.add_space(theme.spacing_xs);
                    }
                }

                // ── Mining: commission drones to fetch ore from finite asteroids. ──
                ui.add_space(theme.spacing_md);
                ui.separator();
                ui.add_space(theme.spacing_sm);
                ui.label(
                    RichText::new("Mining")
                        .size(theme.font_size_heading)
                        .color(theme.text_primary()),
                );
                ui.add_space(theme.spacing_xs);
                if state.asteroids.is_empty() {
                    ui.label(RichText::new("No asteroids in range.").color(theme.text_muted()));
                } else {
                    // Distinct ores available across all asteroids (id -> total qty).
                    let mut ores: Vec<(String, f32)> = Vec::new();
                    for ast in &state.asteroids {
                        for (id, qty) in &ast.ores {
                            if *qty < 1.0 {
                                continue;
                            }
                            if let Some(slot) = ores.iter_mut().find(|(i, _)| i == id) {
                                slot.1 += *qty;
                            } else {
                                ores.push((id.clone(), *qty));
                            }
                        }
                    }
                    for ast in &state.asteroids {
                        let summary: Vec<String> = ast
                            .ores
                            .iter()
                            .filter(|(_, q)| *q >= 1.0)
                            .map(|(id, q)| format!("{} {:.0}", ore_short(id), q))
                            .collect();
                        ui.label(
                            RichText::new(format!(
                                "{} [{}] — {}",
                                ast.name,
                                ast.classification,
                                if summary.is_empty() {
                                    "depleted".to_string()
                                } else {
                                    summary.join(", ")
                                }
                            ))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                        );
                    }
                    ui.add_space(theme.spacing_xs);
                    // The number after each ore is how much is LEFT in the field; a
                    // drone hauls only up to 10 per round trip (say so, since the
                    // big number was misread as the per-trip amount).
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new("Commission drone (hauls up to 10 per trip):")
                                .color(theme.text_secondary()),
                        );
                        for (id, total) in &ores {
                            let label = format!("{} · {:.0} left", ore_short(id), total);
                            if widgets::secondary_button(ui, theme, &label) {
                                action_commission_ore = Some(id.clone());
                            }
                        }
                    });
                }
                ui.add_space(theme.spacing_xs);
                if state.drones.is_empty() {
                    ui.label(
                        RichText::new("No drones in flight.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                } else {
                    // A drone runs a 3-stage round trip; show which stage it's in +
                    // a bar of how far through that stage it is (the "ship is out" cue).
                    for drone in &state.drones {
                        let (stage, desc) = match drone.phase.as_str() {
                            "Outbound" => ("Stage 1/3", "outbound to the asteroid"),
                            "Mining" => ("Stage 2/3", "mining"),
                            "Returning" => ("Stage 3/3", "returning home"),
                            _ => ("Done", "delivering cargo"),
                        };
                        ui.label(
                            RichText::new(format!(
                                "Drone ({}) — {} · {} · cargo {}",
                                ore_short(&drone.ore_id),
                                stage,
                                desc,
                                drone.cargo
                            ))
                            .size(theme.font_size_small)
                            .color(theme.text_primary()),
                        );
                        widgets::progress_bar(ui, theme, drone.phase_progress, None);
                    }
                }
            });
        });

    // Apply the Garden actions (set inside the central panel) to GuiState; the main
    // loop bridges these into FarmingSystem's command channels before the next tick.
    if let Some(bits) = action_water_crop {
        state.pending_water_crop = Some(bits);
    }
    if let Some(bits) = action_harvest_crop {
        state.pending_harvest_crop = Some(bits);
    }
    if action_dev_grow {
        state.dev_grow_crops = true;
    }
    // Bridge a commissioned drone to DroneSystem (via the main loop).
    if let Some(ore_id) = action_commission_ore {
        state.pending_commission_ore = Some(ore_id);
    }
    // Bridge the Rest button to FoodSystem (refills energy).
    if action_rest {
        state.pending_rest = true;
    }
    // Bridge Compost (FoodSystem) + Fertilize (FarmingSystem).
    if action_compost {
        state.pending_compost = true;
    }
    if let Some(bits) = action_fertilize_crop {
        state.pending_fertilize_crop = Some(bits);
    }
}

// detail_row moved to crate::gui::widgets::detail_row
