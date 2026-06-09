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

/// A segmented capacity bar for the drone manifest: a track sized to `cap`, with one
/// coloured segment per ore in the draft (width ∝ its allocated units). Lets the
/// player see the allocation fill the hold as they build it.
fn manifest_bar(ui: &mut egui::Ui, theme: &Theme, draft: &[(String, u32)], cap: u32) {
    let w = ui.available_width().max(80.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(w, 12.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, egui::Rounding::same(2), theme.border());
    let palette = [
        theme.accent(),
        theme.warning(),
        theme.danger(),
        theme.text_secondary(),
    ];
    let capf = cap.max(1) as f32;
    let mut x = rect.left();
    for (i, (_ore, units)) in draft.iter().enumerate() {
        let seg_w = (*units as f32 / capf) * w;
        let seg = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(seg_w, 12.0));
        painter.rect_filled(seg, egui::Rounding::same(2), palette[i % palette.len()]);
        x += seg_w;
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

/// Convert a [`crate::gui::Place`] hierarchy into renderable [`widgets::TreeNode`]s,
/// injecting the live backpack `items` at the node marked `kind: "backpack"`.
/// Place nodes are non-selectable (empty id); only the injected item leaves are
/// clickable. Each node gets a colour swatch by kind (so You / your home / a
/// vehicle / containers / items read at a glance) and shows its location and/or
/// coordinate as right-aligned detail.
fn place_to_tree(theme: &Theme, place: &crate::gui::Place, items: &[widgets::TreeNode]) -> widgets::TreeNode {
    let mut children: Vec<widgets::TreeNode> =
        place.children.iter().map(|c| place_to_tree(theme, c, items)).collect();
    if place.kind == "backpack" {
        children.extend(items.iter().cloned());
    }
    // Location inline in the label, in parentheses (operator 2026-06-08: not
    // right-aligned to the page edge) — e.g. "Home (Silverdale, WA · 47.6°, -122.7°)".
    let mut loc = place.location.clone().unwrap_or_default();
    if let Some([lat, lon]) = place.coordinate {
        let coord = format!("{lat:.4}°, {lon:.4}°");
        loc = if loc.is_empty() { coord } else { format!("{loc} · {coord}") };
    }
    let label = if loc.is_empty() {
        place.label.clone()
    } else {
        format!("{} ({})", place.label, loc)
    };
    widgets::TreeNode {
        id: String::new(),
        label,
        detail: String::new(),
        color: Some(kind_color(theme, &place.kind)),
        children,
    }
}

/// Colour for a place/entity node by its `kind` — drives the tree swatches so the
/// structure is scannable ("what is where"). All theme tokens, no literals.
fn kind_color(theme: &Theme, kind: &str) -> egui::Color32 {
    match kind {
        "person" => theme.success(),
        "vehicle" => theme.warning(),
        "building" | "property" => theme.accent(),
        "planet" | "region" | "locale" => theme.info(),
        "item" => theme.text_muted(),
        // rooms, floors, packs, duffels, bags, pouches, generic containers
        _ => theme.text_secondary(),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
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
    // "Dev: stock seeds" — the seed item ids of the starter set to grant.
    let mut action_stock_seeds: Option<Vec<String>> = None;
    // Plant a whole tower (v0.386): (tower id, plant ids) to spawn as crops, set by
    // the Garden "Plant a tower" buttons, applied to GuiState after the panel.
    let mut action_plant_tower: Option<(String, Vec<String>)> = None;
    // Drone manifest builder: a stepper click (ore, +1/-1), launch, or clear.
    let mut action_manifest_delta: Option<(String, i32)> = None;
    let mut action_launch_manifest = false;
    let mut action_clear_manifest = false;
    let mut action_rest = false;
    let mut action_compost = false;
    let mut action_fertilize_crop: Option<u64> = None;
    // ── ONE PANEL (operator 2026-06-08: back to a single panel; the 3-panel split
    //    read as stair-stepped). Everything in one CentralPanel + a vertical scroll,
    //    widgets aligned, the status bars capped at theme.status_bar_width. The
    //    detail block shows only when something is selected. ──
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
        ScrollArea::vertical().show(ui, |ui| {
            // Selected inventory item -> its detail card + actions (crop/tower
            // detail is gone; the Garden TABLE below shows that data inline).
            if let Some(idx) = state.selected_slot {
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
            ui.label(RichText::new("Inventory").size(theme.font_size_title).color(theme.text_primary()));
            ui.add_space(theme.spacing_xs);

            // Creative / survival mode (default Creative during early dev): in
            // Creative, planting + crafting don't need or consume resources, so the
            // seed/material economy can be built out before it actually bites.
            widgets::toggle(ui, theme, "Creative mode", &mut state.creative_mode);
            ui.label(
                RichText::new(if state.creative_mode {
                    "Creative: plant + craft freely, nothing consumed."
                } else {
                    "Survival: planting + crafting consume seeds + materials."
                })
                .size(theme.font_size_small)
                .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

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
            widgets::stat_row(
                ui, theme, "Weight",
                &format!("{:.1} / {:.1} kg", carry_weight, max_weight),
                weight_color, weight_frac, weight_color,
            );

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
                // Compact stat table — one thin row per vital (name · value · bar);
                // the columns align because every row shares widgets::stat_row's
                // fixed name/value widths.
                widgets::stat_row(
                    ui, theme, "Satiation",
                    &format!("{:.0} / {:.0}", sat, sat_max),
                    color_for(sat_frac), sat_frac, color_for(sat_frac),
                );
                widgets::stat_row(
                    ui, theme, "Hydration",
                    &format!("{:.0} / {:.0}", hyd, hyd_max),
                    color_for(hyd_frac), hyd_frac, color_for(hyd_frac),
                );
                widgets::stat_row(
                    ui, theme, "Energy",
                    &format!("{:.0} / {:.0}", energy, energy_max),
                    color_for(energy_frac), energy_frac, color_for(energy_frac),
                );
                let oxy = state.vitals.oxygen;
                let oxy_max = state.vitals.oxygen_max.max(1.0);
                let oxy_frac = (oxy / oxy_max).clamp(0.0, 1.0);
                widgets::stat_row(
                    ui, theme, "Oxygen",
                    &format!("{:.0} / {:.0}", oxy, oxy_max),
                    color_for(oxy_frac), oxy_frac, color_for(oxy_frac),
                );
                let waste = state.vitals.waste;
                let waste_max = state.vitals.waste_max.max(1.0);
                let waste_frac = (waste / waste_max).clamp(0.0, 1.0);
                // High waste is BAD (inverted colour vs the other vitals).
                let waste_col = if waste_frac > 0.75 {
                    theme.danger()
                } else if waste_frac > 0.5 {
                    theme.warning()
                } else {
                    theme.text_secondary()
                };
                widgets::stat_row(
                    ui, theme, "Waste",
                    &format!("{:.0} / {:.0}", waste, waste_max),
                    waste_col, waste_frac, waste_col,
                );
                // Body temperature is a readout (not a 0..100 bar) + the seal status,
                // on the same name/value columns so it lines up with the bars above.
                let temp = state.vitals.body_temp_c;
                let temp_col = if temp < 35.0 || temp > 39.0 {
                    theme.danger()
                } else if temp < 36.0 || temp > 38.0 {
                    theme.warning()
                } else {
                    theme.accent()
                };
                ui.horizontal(|ui| {
                    let h = theme.font_size_body + 2.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(theme.stat_name_width, h),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            ui.label(RichText::new("Body temp").color(theme.text_secondary()).size(theme.font_size_small));
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(theme.stat_value_width, h),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.label(RichText::new(format!("{:.1}°C", temp)).color(temp_col).size(theme.font_size_small));
                        },
                    );
                    if state.vitals.sealed {
                        ui.label(RichText::new("Sealed").size(theme.font_size_small).color(theme.accent()));
                    } else {
                        ui.label(RichText::new("EXPOSED, no air!").size(theme.font_size_small).color(theme.danger()));
                    }
                });
                // Survival actions (decoupled from the bars so each bar reads cleanly).
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    if widgets::secondary_button(ui, theme, "Rest") {
                        action_rest = true;
                    }
                    if widgets::secondary_button(ui, theme, "Compost") {
                        action_compost = true;
                    }
                });
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

                // Tree controls (operator 2026-06-08): collapse/expand ALL branches +
                // a "Start collapsed" default, driving BOTH the places tree and the
                // Garden tree below (one control set for the inventory's nested lists).
                let mut tree_force: Option<bool> = None;
                ui.horizontal(|ui| {
                    if widgets::secondary_button(ui, theme, "Collapse all") {
                        tree_force = Some(false);
                    }
                    if widgets::secondary_button(ui, theme, "Expand all") {
                        tree_force = Some(true);
                    }
                    // "Start collapsed" as a proper bordered button (operator
                    // 2026-06-08: the whole button area is clickable, not just a tiny
                    // checkbox); .active() renders the ON state like a pressed button.
                    if widgets::Button::new("Start collapsed")
                        .active(state.trees_start_collapsed)
                        .show(ui, theme)
                    {
                        state.trees_start_collapsed = !state.trees_start_collapsed;
                        // Apply the new default to the already-rendered trees this frame.
                        tree_force = Some(!state.trees_start_collapsed);
                    }
                });
                let tree_default_open = !state.trees_start_collapsed;
                ui.add_space(theme.spacing_xs);

                // Your entity / container tree — top-level entities (You, your
                // home, a vehicle, …), each a container with its own contents,
                // colour-coded by kind so "what is where" reads at a glance. The
                // spine is data-driven (data/places/seed.json → state.places); the
                // live backpack contents inject at the node marked kind:"backpack".
                // No place data → flat backpack fallback.
                let header = if state.places.is_empty() { "Backpack" } else { "You & your places" };
                ui.label(RichText::new(header).size(theme.font_size_heading).color(theme.text_primary()));
                ui.add_space(theme.spacing_xs);

                // Live backpack contents as selectable leaves (id = slot index → the
                // right detail panel shows the item + its actions).
                let item_color = kind_color(theme, "item");
                let item_nodes: Vec<widgets::TreeNode> = state
                    .inventory_items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, slot)| {
                        slot.as_ref().map(|item| {
                            widgets::TreeNode::selectable(
                                i.to_string(),
                                item.name.clone(),
                                format!("x{}", item.quantity),
                            )
                            .with_color(item_color)
                        })
                    })
                    .collect();

                let selected_str =
                    state.selected_slot.map(|i| i.to_string()).unwrap_or_default();

                // Entity tree when we have the spine; else flat backpack.
                let clicked = if !state.places.is_empty() {
                    let entities = state.places.clone();
                    let trees: Vec<widgets::TreeNode> = entities
                        .iter()
                        .map(|e| place_to_tree(theme, e, &item_nodes))
                        .collect();
                    widgets::tree_list_ex(ui, theme, &trees, &selected_str, tree_default_open, tree_force)
                } else if item_nodes.is_empty() {
                    ui.label(
                        RichText::new("Empty, mine, craft, or dev-stock to fill it.")
                            .color(theme.text_muted()),
                    );
                    None
                } else {
                    widgets::tree_list_ex(ui, theme, &item_nodes, &selected_str, tree_default_open, tree_force)
                };

                if let Some(clicked) = clicked {
                    if let Ok(idx) = clicked.parse::<usize>() {
                        state.selected_slot = if state.selected_slot == Some(idx) {
                            None
                        } else {
                            Some(idx)
                        };
                        state.garden_selection = None; // item + garden are exclusive
                    }
                }

                // ── GARDEN: an aligned TABLE grouped by tower (operator 2026-06-08:
                //    one panel, rows/columns spreadsheet design). Each tower is a
                //    collapsible group with a Plant button + a crop table; the
                //    "Dev: stock seeds" + "Dev: grow all" buttons sit in the header. ──
                ui.add_space(theme.spacing_md);
                ui.separator();
                ui.add_space(theme.spacing_sm);
                ui.label(RichText::new("Garden").size(theme.font_size_heading).color(theme.text_primary()));
                ui.horizontal_wrapped(|ui| {
                    // Dev: grant the starter seed set (one of each tower variety), so
                    // survival-mode planting is testable now. Creative ignores seeds.
                    if widgets::secondary_button(ui, theme, "Dev: stock seeds") {
                        let mut seeds: Vec<String> = Vec::new();
                        for t in &state.tower_configs {
                            for p in &t.plantings {
                                let sid = format!("seed_{}_0", p.plant);
                                if !seeds.contains(&sid) {
                                    seeds.push(sid);
                                }
                            }
                        }
                        if !seeds.is_empty() {
                            action_stock_seeds = Some(seeds);
                        }
                    }
                    if !state.crops.is_empty() && widgets::secondary_button(ui, theme, "Dev: grow all") {
                        action_dev_grow = true;
                    }
                });
                ui.add_space(theme.spacing_xs);
                // Garden as an aligned TABLE grouped by tower (operator 2026-06-08:
                // rows/columns spreadsheet design, not a tree+detail). One collapsible
                // group per tower (a Plant button in its body), each a compact egui::Grid
                // with fixed columns; the growth bar is capped at theme.status_bar_width.
                // Seed-planted crops fall under "Other crops".
                if state.tower_configs.is_empty() && state.crops.is_empty() {
                    ui.label(
                        RichText::new("No garden plots yet. Add an aeroponic tower design at Home.")
                            .color(theme.text_muted()),
                    );
                } else {
                    // Group order: one per configured tower, then "Other crops" if any
                    // seed-planted (tower-less) crops exist.
                    let mut groups: Vec<(Option<String>, String)> = state
                        .tower_configs
                        .iter()
                        .map(|t| (Some(t.id.clone()), t.name.clone()))
                        .collect();
                    if state.crops.iter().any(|c| c.tower_id.is_none()) {
                        groups.push((None, "Other crops".to_string()));
                    }
                    for (gi, (tid, title)) in groups.iter().enumerate() {
                        let crops: Vec<&crate::gui::GuiCrop> =
                            state.crops.iter().filter(|c| &c.tower_id == tid).collect();
                        let ready = crops.iter().filter(|c| c.mature).count();
                        let header = if crops.is_empty() {
                            format!("{}  (not planted)", title)
                        } else {
                            format!("{}  ({}/{} ready)", title, ready, crops.len())
                        };
                        // Resolve the tower's plant-id list up front (so the Plant button
                        // doesn't borrow state inside the body closure).
                        let plant_ids: Option<(String, Vec<String>)> = tid.as_ref().and_then(|id| {
                            state.tower_configs.iter().find(|t| &t.id == id).map(|t| {
                                let ids: Vec<String> = t
                                    .plantings
                                    .iter()
                                    .flat_map(|p| {
                                        std::iter::repeat(p.plant.clone()).take(p.slots.max(1) as usize)
                                    })
                                    .collect();
                                (t.id.clone(), ids)
                            })
                        });
                        let planted_label = if crops.is_empty() { "Plant this tower" } else { "Plant again" };
                        egui::CollapsingHeader::new(RichText::new(header).strong().color(theme.accent()))
                            .id_salt(("garden_grp", gi))
                            .default_open(tree_default_open)
                            .open(tree_force)
                            .show(ui, |ui| {
                                if let Some((tid, ids)) = &plant_ids {
                                    if widgets::secondary_button(ui, theme, planted_label) {
                                        action_plant_tower = Some((tid.clone(), ids.clone()));
                                    }
                                }
                                if crops.is_empty() {
                                    return;
                                }
                                egui::Grid::new(("garden_tbl", gi))
                                    .striped(true)
                                    .spacing([10.0, 3.0])
                                    .show(ui, |ui| {
                                        for h in ["Plant", "Stage", "Growth", "N·P·K", "Water/day", "Temp", "State", "Actions"] {
                                            ui.label(RichText::new(h).size(theme.font_size_small).strong().color(theme.text_secondary()));
                                        }
                                        ui.end_row();
                                        for crop in &crops {
                                            let name_col = if crop.dead { theme.danger() } else if crop.mature { theme.accent() } else { theme.text_primary() };
                                            ui.label(RichText::new(&crop.name).color(name_col));
                                            let stage_txt = if crop.dead { "dead" } else if crop.mature { "ready" } else { crop.stage.as_str() };
                                            ui.label(RichText::new(stage_txt).size(theme.font_size_small).color(theme.text_secondary()));
                                            ui.add(egui::ProgressBar::new(crop.progress.clamp(0.0, 1.0)).fill(theme.accent()).desired_width(theme.status_bar_width).desired_height(theme.status_bar_height));
                                            ui.label(RichText::new(format!("{:.2}·{:.2}·{:.2}", crop.n, crop.p, crop.k)).size(theme.font_size_small).color(theme.text_secondary()));
                                            ui.label(RichText::new(format!("{:.1} L", crop.water_per_day)).size(theme.font_size_small).color(theme.text_secondary()));
                                            ui.label(RichText::new(format!("{:.0}-{:.0}°C", crop.temp_min, crop.temp_max)).size(theme.font_size_small).color(theme.text_secondary()));
                                            let wcol = if crop.water < 0.2 { theme.danger() } else { theme.text_secondary() };
                                            ui.label(RichText::new(format!("{:.0}% / {:.0}%", crop.water * 100.0, crop.health)).size(theme.font_size_small).color(wcol));
                                            ui.horizontal(|ui| {
                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                if crop.mature && widgets::compact_button(ui, theme, "Harvest", widgets::ButtonVariant::Primary) {
                                                    action_harvest_crop = Some(crop.entity_bits);
                                                }
                                                if !crop.dead && widgets::compact_button(ui, theme, "Water", widgets::ButtonVariant::Secondary) {
                                                    action_water_crop = Some(crop.entity_bits);
                                                }
                                                if !crop.dead && widgets::compact_button(ui, theme, "Fertilize", widgets::ButtonVariant::Secondary) {
                                                    action_fertilize_crop = Some(crop.entity_bits);
                                                }
                                            });
                                            ui.end_row();
                                        }
                                    });
                            });
                    }
                }
                // ── Mining: commission drones to fetch ore from finite asteroids. ──
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
                    // Asteroids as an aligned table: Asteroid | Type | Ores.
                    egui::Grid::new("mining_asteroids")
                        .striped(true)
                        .spacing([12.0, 3.0])
                        .show(ui, |ui| {
                            for h in ["Asteroid", "Type", "Ores"] {
                                ui.label(RichText::new(h).size(theme.font_size_small).strong().color(theme.text_secondary()));
                            }
                            ui.end_row();
                            for ast in &state.asteroids {
                                let summary: Vec<String> = ast
                                    .ores
                                    .iter()
                                    .filter(|(_, q)| *q >= 1.0)
                                    .map(|(id, q)| format!("{} {:.0}", ore_short(id), q))
                                    .collect();
                                ui.label(RichText::new(&ast.name).size(theme.font_size_small).color(theme.text_primary()));
                                ui.label(RichText::new(&ast.classification).size(theme.font_size_small).color(theme.text_secondary()));
                                ui.label(RichText::new(if summary.is_empty() { "depleted".to_string() } else { summary.join(", ") }).size(theme.font_size_small).color(theme.text_secondary()));
                                ui.end_row();
                            }
                        });
                    ui.add_space(theme.spacing_xs);
                    // ── Drone manifest builder: allocate the fixed hold across ores
                    //    (+/- per ore; the segmented bar shows the split). One drone per
                    //    player, so this is hidden while a drone is already out.
                    if !state.drone_active {
                        let cap = crate::systems::mining::DRONE_CAPACITY;
                        let total: u32 = state.drone_manifest_draft.iter().map(|(_, u)| u).sum();
                        ui.label(
                            RichText::new(format!("Drone manifest, {total}/{cap} units"))
                                .color(theme.text_secondary()),
                        );
                        manifest_bar(ui, theme, &state.drone_manifest_draft, cap);
                        ui.add_space(theme.spacing_xs);
                        // Per-ore allocation as an aligned table: Ore | Available | [-] qty [+].
                        egui::Grid::new("mining_manifest")
                            .striped(true)
                            .spacing([10.0, 3.0])
                            .show(ui, |ui| {
                                for h in ["Ore", "Available", "In manifest"] {
                                    ui.label(RichText::new(h).size(theme.font_size_small).strong().color(theme.text_secondary()));
                                }
                                ui.end_row();
                                for (id, avail) in &ores {
                                    let cur = state
                                        .drone_manifest_draft
                                        .iter()
                                        .find(|(o, _)| o == id)
                                        .map(|(_, u)| *u)
                                        .unwrap_or(0);
                                    ui.label(RichText::new(ore_short(id)).size(theme.font_size_small).color(theme.text_secondary()));
                                    ui.label(RichText::new(format!("{:.0} left", avail)).size(theme.font_size_small).color(theme.text_muted()));
                                    ui.horizontal(|ui| {
                                        ui.spacing_mut().item_spacing.x = 4.0;
                                        if widgets::stepper_button(ui, theme, "-", cur > 0, false) {
                                            action_manifest_delta = Some((id.clone(), -1));
                                        }
                                        ui.label(RichText::new(format!("{cur}")).color(theme.text_primary()));
                                        if widgets::stepper_button(ui, theme, "+", total < cap, true) {
                                            action_manifest_delta = Some((id.clone(), 1));
                                        }
                                    });
                                    ui.end_row();
                                }
                            });
                        ui.add_space(theme.spacing_xs);
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(total >= 1, |ui| {
                                if widgets::primary_button(ui, theme, "Launch drone") {
                                    action_launch_manifest = true;
                                }
                            });
                            if total > 0 && widgets::secondary_button(ui, theme, "Clear") {
                                action_clear_manifest = true;
                            }
                        });
                    }
                }
                ui.add_space(theme.spacing_xs);
                if !state.drones.is_empty() {
                    // The active drone (one per player): its manifest + which of the 3
                    // round-trip stages it's in + a bar of progress through that stage.
                    // Active drone(s) as an aligned table: Stage | Status | Fetching |
                    // Cargo, then a thin progress bar per drone (one drone per player).
                    egui::Grid::new("mining_drones")
                        .striped(true)
                        .spacing([12.0, 3.0])
                        .show(ui, |ui| {
                            for h in ["Drone", "Status", "Fetching", "Cargo"] {
                                ui.label(RichText::new(h).size(theme.font_size_small).strong().color(theme.text_secondary()));
                            }
                            ui.end_row();
                            for drone in &state.drones {
                                let (stage, desc) = match drone.phase.as_str() {
                                    "Outbound" => ("1/3", "outbound"),
                                    "Mining" => ("2/3", "mining"),
                                    "Returning" => ("3/3", "returning"),
                                    _ => ("done", "delivering"),
                                };
                                let fetching: Vec<String> = drone
                                    .manifest
                                    .iter()
                                    .map(|(o, u)| format!("{}x {}", u, ore_short(o)))
                                    .collect();
                                ui.label(RichText::new(format!("Stage {stage}")).size(theme.font_size_small).color(theme.text_primary()));
                                ui.label(RichText::new(desc).size(theme.font_size_small).color(theme.text_secondary()));
                                ui.label(RichText::new(fetching.join(", ")).size(theme.font_size_small).color(theme.text_secondary()));
                                ui.label(RichText::new(drone.cargo_total.to_string()).size(theme.font_size_small).color(theme.text_secondary()));
                                ui.end_row();
                            }
                        });
                    for drone in &state.drones {
                        ui.add(egui::ProgressBar::new(drone.phase_progress.clamp(0.0, 1.0)).fill(theme.accent()).desired_width(theme.status_bar_width).desired_height(theme.status_bar_height));
                    }
                }
        }); // close the single-panel ScrollArea
        }); // close the single CentralPanel

    // Apply the Garden actions (set inside the central panel) to GuiState; the main
    // loop bridges these into FarmingSystem's command channels before the next tick.
    if let Some(ids) = action_plant_tower {
        state.pending_plant_tower = Some(ids);
    }
    if let Some(seeds) = action_stock_seeds {
        state.pending_stock_seeds = Some(seeds);
    }
    if let Some(bits) = action_water_crop {
        state.pending_water_crop = Some(bits);
    }
    if let Some(bits) = action_harvest_crop {
        state.pending_harvest_crop = Some(bits);
    }
    if action_dev_grow {
        state.dev_grow_crops = true;
    }
    // Apply the drone-manifest builder's actions (the panel only reads state).
    if let Some((ore, delta)) = action_manifest_delta {
        let cap = crate::systems::mining::DRONE_CAPACITY;
        let total: u32 = state.drone_manifest_draft.iter().map(|(_, u)| u).sum();
        if let Some(slot) = state.drone_manifest_draft.iter_mut().find(|(o, _)| *o == ore) {
            if delta > 0 && total < cap {
                slot.1 += 1;
            } else if delta < 0 && slot.1 > 0 {
                slot.1 -= 1;
            }
        } else if delta > 0 && total < cap {
            state.drone_manifest_draft.push((ore, 1));
        }
        state.drone_manifest_draft.retain(|(_, u)| *u > 0);
    }
    if action_clear_manifest {
        state.drone_manifest_draft.clear();
    }
    if action_launch_manifest {
        let manifest: Vec<(String, u32)> = state
            .drone_manifest_draft
            .iter()
            .filter(|(_, u)| *u > 0)
            .cloned()
            .collect();
        if !manifest.is_empty() {
            state.pending_drone_manifest = Some(manifest);
            state.drone_manifest_draft.clear();
        }
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

// (garden_tree_nodes + crop_leaf removed in v0.402 — the garden is an aligned
// TABLE now, not a tree, so the tree-node builders are no longer needed.)

// detail_row moved to crate::gui::widgets::detail_row
