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

/// Quick-action outputs from the inline item card. The card only borrows an item
/// SNAPSHOT (so it can't touch GuiState mid-render), so it records what the player
/// asked for here and the caller applies it to GuiState after the tree renders.
#[derive(Default)]
struct ItemCardActions {
    eat: Option<String>,
    drink: Option<String>,
    plant: Option<String>,
    equip: bool,
    drop: bool,
}

/// The EXPAND-IN-PLACE card for one inventory item (operator 2026-06-08: "click an
/// item row to expand in place — picture/3d + full details over rows — instead of a
/// popup/top detail"). Rendered under the selected row by the places/backpack tree's
/// inline renderer: a colored placeholder image tile (the universal widget, swatch by
/// item id) + category badge + a details grid + description + quick actions. Records
/// the chosen action into `acts`; the caller bridges it into GuiState.
fn draw_item_card(
    ui: &mut egui::Ui,
    theme: &Theme,
    item: &crate::gui::GuiItemSlot,
    acts: &mut ItemCardActions,
) {
    let details = lookup_item_details(&item.item_id);
    ui.add_space(theme.spacing_xs);
    ui.horizontal_top(|ui| {
        // Colored placeholder image / 3D-model stand-in (stable colour per item id).
        let glyph = item
            .name
            .chars()
            .next()
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_default();
        widgets::placeholder_tile(ui, theme, widgets::swatch_color(&item.item_id), 64.0, &glyph);
        ui.add_space(theme.spacing_sm);
        ui.vertical(|ui| {
            ui.label(
                RichText::new(&item.name)
                    .size(theme.font_size_heading)
                    .strong()
                    .color(theme.accent()),
            );
            if let Some(d) = &details {
                // Category badge.
                egui::Frame::none()
                    .fill(category_color(&d.category))
                    .rounding(Rounding::same(3))
                    .inner_margin(Vec2::new(6.0, 2.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(&d.category)
                                .size(theme.font_size_small)
                                .color(Color32::WHITE),
                        );
                    });
            }
        });
    });
    ui.add_space(theme.spacing_xs);
    // Details grid.
    widgets::card(ui, theme, |ui| {
        widgets::detail_row(ui, theme, "ID", &item.item_id);
        widgets::detail_row(ui, theme, "Quantity", &item.quantity.to_string());
        if let Some(d) = &details {
            widgets::detail_row(ui, theme, "Category", &d.category);
            widgets::detail_row(ui, theme, "Subcategory", &d.subcategory);
            widgets::detail_row(ui, theme, "Weight", &format!("{:.2} kg", d.weight_kg));
            widgets::detail_row(ui, theme, "Stack Size", &d.stack_size.to_string());
            widgets::detail_row(ui, theme, "Material", &d.base_material);
            if d.durability > 0 {
                widgets::detail_row(ui, theme, "Durability", &d.durability.to_string());
            }
        }
    });
    if let Some(d) = &details {
        if !d.description.is_empty() {
            ui.add_space(theme.spacing_xs);
            ui.label(
                RichText::new(&d.description)
                    .color(theme.text_secondary())
                    .size(theme.font_size_small),
            );
        }
    }
    ui.add_space(theme.spacing_sm);
    // Quick actions — Eat (food) / Drink (liquid) / Plant (seed) / Use, then Equip +
    // Drop. Compact buttons so the row reads cleanly under the item.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let is_drink = details
            .as_ref()
            .map(|d| d.subcategory == "drink" || d.subcategory == "liquid")
            .unwrap_or(false)
            || item.item_id.starts_with("water_");
        let is_food = details.as_ref().map(|d| d.category == "food").unwrap_or(false) && !is_drink;
        let is_seed = details.as_ref().map(|d| d.subcategory == "seed").unwrap_or(false)
            || item.item_id.starts_with("seed_");
        if is_drink {
            if widgets::compact_button(ui, theme, "Drink", widgets::ButtonVariant::Primary) {
                acts.drink = Some(item.item_id.clone());
            }
        } else if is_food {
            if widgets::compact_button(ui, theme, "Eat", widgets::ButtonVariant::Primary) {
                acts.eat = Some(item.item_id.clone());
            }
        } else if is_seed {
            if widgets::compact_button(ui, theme, "Plant", widgets::ButtonVariant::Primary) {
                acts.plant = Some(item.item_id.clone());
            }
        } else {
            let _ = widgets::compact_button(ui, theme, "Use", widgets::ButtonVariant::Secondary);
        }
        if widgets::compact_button(ui, theme, "Equip", widgets::ButtonVariant::Secondary) {
            acts.equip = true;
        }
        if widgets::compact_button(ui, theme, "Drop", widgets::ButtonVariant::Danger) {
            acts.drop = true;
        }
    });
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

    // The selected item's inline expand-in-place card records its quick action here
    // (Eat/Drink/Plant/Equip/Drop); applied to GuiState after the panel closes (the
    // card only borrows an inventory snapshot, never GuiState mid-render).
    let mut item_acts = ItemCardActions::default();
    // Crop actions come from the Garden section in the central panel; applied after.
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

            // Collapse/Expand/Start-collapsed controls (operator 2026-06-08): ONE
            // control set driving every collapsible SECTION below + their nested
            // trees. Rendered up top so `tree_force` is set before any section reads
            // it (the buttons mutate it THIS frame; sections below pick it up).
            let mut tree_force: Option<bool> = None;
            ui.horizontal(|ui| {
                if widgets::secondary_button(ui, theme, "Collapse all") {
                    tree_force = Some(false);
                }
                if widgets::secondary_button(ui, theme, "Expand all") {
                    tree_force = Some(true);
                }
                // "Start collapsed" as a proper bordered button — the whole area is
                // clickable; .active() renders the ON state like a pressed button.
                if widgets::Button::new("Start collapsed")
                    .active(state.trees_start_collapsed)
                    .show(ui, theme)
                {
                    state.trees_start_collapsed = !state.trees_start_collapsed;
                    tree_force = Some(!state.trees_start_collapsed);
                }
            });
            let tree_default_open = !state.trees_start_collapsed;
            ui.add_space(theme.spacing_sm);

            // ── Status (collapsible) — the live player vitals. Body renders only
            //    when the section is open; closes just before the Equipment divider. ──
            if widgets::section_disclosure(ui, theme, ("inv_sec", "status"), "Status", tree_force) {

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

            } // ── end Status ──

            widgets::rgb_section_divider(ui, theme);

                // ── Equipment (collapsible) — closes before the You & places divider ──
                if widgets::section_disclosure(ui, theme, ("inv_sec", "equipment"), "Equipment", tree_force) {
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

                } // ── end Equipment ──

                widgets::rgb_section_divider(ui, theme);

                // Your entity / container tree — top-level entities (You, your
                // home, a vehicle, …), each a container with its own contents,
                // colour-coded by kind so "what is where" reads at a glance. The
                // spine is data-driven (data/places/seed.json → state.places); the
                // live backpack contents inject at the node marked kind:"backpack".
                // No place data → flat backpack fallback.
                // ── You & places (collapsible) — closes before the Garden divider ──
                let header = if state.places.is_empty() { "Backpack" } else { "You & your places" };
                if widgets::section_disclosure(ui, theme, ("inv_sec", "places"), header, tree_force) {
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

                // Inline expand-in-place card for the SELECTED item, rendered by the
                // tree directly under that item's row (operator 2026-06-08: replaces
                // the old top-of-panel detail). The card only reads an inventory
                // SNAPSHOT, recording the chosen action into `item_acts` (applied to
                // GuiState after the panel) so it never borrows GuiState mid-render.
                let inv_snapshot = state.inventory_items.clone();
                let mut inline_card = |ui: &mut egui::Ui, id: &str| {
                    if let Ok(idx) = id.parse::<usize>() {
                        if let Some(Some(it)) = inv_snapshot.get(idx) {
                            draw_item_card(ui, theme, it, &mut item_acts);
                        }
                    }
                };

                // Entity tree when we have the spine; else flat backpack.
                let clicked = if !state.places.is_empty() {
                    let entities = state.places.clone();
                    let trees: Vec<widgets::TreeNode> = entities
                        .iter()
                        .map(|e| place_to_tree(theme, e, &item_nodes))
                        .collect();
                    widgets::tree_list_ex(ui, theme, &trees, &selected_str, tree_default_open, tree_force, &mut inline_card)
                } else if item_nodes.is_empty() {
                    ui.label(
                        RichText::new("Empty, mine, craft, or dev-stock to fill it.")
                            .color(theme.text_muted()),
                    );
                    None
                } else {
                    widgets::tree_list_ex(ui, theme, &item_nodes, &selected_str, tree_default_open, tree_force, &mut inline_card)
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

                } // ── end You & places ──

                // ── GARDEN (collapsible) — numbered SLOT rows per tower; click a
                //    slot to expand its multi-row plant card. The Dev seed/grow
                //    buttons sit in the body. Closes before the Mining divider. ──
                widgets::rgb_section_divider(ui, theme);
                if widgets::section_disclosure(ui, theme, ("inv_sec", "garden"), "Garden", tree_force) {
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
                        // The tower's CONFIG (None for the "Other crops" group, which
                        // holds loose seed-planted crops with no fixed-slot design).
                        let cfg = tid
                            .as_ref()
                            .and_then(|id| state.tower_configs.iter().find(|t| &t.id == id));
                        // Per-slot planned spec (plant id, role, note), flattened from the
                        // plantings EXACTLY as the farming handler assigns tower_slot, so
                        // slot index lines up with crop.tower_slot. Empty for "Other crops".
                        let slot_specs: Vec<(String, String, String)> = cfg
                            .map(|t| {
                                t.plantings
                                    .iter()
                                    .flat_map(|p| {
                                        std::iter::repeat((
                                            p.plant.clone(),
                                            p.role.clone(),
                                            p.note.clone(),
                                        ))
                                        .take(p.slots.max(1) as usize)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        // Slot rows to render: the design's planned slots for a real tower,
                        // else one row per loose crop (the "Other crops" fallback).
                        let slot_count = if cfg.is_some() { slot_specs.len() } else { crops.len() };
                        let count_txt = if slot_count == 0 {
                            "no slots".to_string()
                        } else if crops.is_empty() {
                            format!("{} slots · not planted", slot_count)
                        } else {
                            format!("{}/{} ready · {} slots", ready, crops.len(), slot_count)
                        };
                        // Make/model/version subtitle for the tower title row (operator
                        // 2026-06-08: "aeroponic tower make model version"). Data-driven.
                        let subtitle = cfg
                            .map(|t| {
                                let mut parts: Vec<String> = Vec::new();
                                if !t.make.is_empty() {
                                    parts.push(t.make.clone());
                                }
                                if !t.model.is_empty() {
                                    parts.push(t.model.clone());
                                }
                                if !t.version.is_empty() {
                                    parts.push(format!("v{}", t.version));
                                }
                                parts.join(" ")
                            })
                            .unwrap_or_default();
                        // Resolve the tower's plant-id list up front (so the Plant button
                        // doesn't borrow state inside the body closure).
                        let plant_ids: Option<(String, Vec<String>)> = cfg.map(|t| {
                            let ids: Vec<String> = t
                                .plantings
                                .iter()
                                .flat_map(|p| {
                                    std::iter::repeat(p.plant.clone()).take(p.slots.max(1) as usize)
                                })
                                .collect();
                            (t.id.clone(), ids)
                        });
                        let planted_label = if crops.is_empty() { "Plant this tower" } else { "Plant again" };
                        widgets::expandable_row(
                            ui,
                            ("garden_grp", gi),
                            tree_default_open,
                            tree_force,
                            |ui| {
                                // Tower title row: name + make/model/version + ready
                                // count + the Plant button (widgets in the title row).
                                ui.label(RichText::new(title).strong().color(theme.accent()));
                                if !subtitle.is_empty() {
                                    ui.label(RichText::new(format!("· {subtitle}")).size(theme.font_size_small).color(theme.text_muted()));
                                }
                                ui.label(RichText::new(format!("· {count_txt}")).size(theme.font_size_small).color(theme.text_secondary()));
                                if let Some((tid2, ids)) = &plant_ids {
                                    if widgets::compact_button(ui, theme, planted_label, widgets::ButtonVariant::Secondary) {
                                        action_plant_tower = Some((tid2.clone(), ids.clone()));
                                    }
                                }
                            },
                            |ui| {
                                if slot_count == 0 {
                                    ui.label(
                                        RichText::new("This tower has no plantings yet.")
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                    return;
                                }
                                // Prettify a plant id ("cherry_tomato" -> "Cherry Tomato")
                                // for slots not yet grown (no GuiCrop to read a name from).
                                let prettify = |id: &str| -> String {
                                    id.split(['_', '-', ' '])
                                        .filter(|w| !w.is_empty())
                                        .map(|w| {
                                            let mut ch = w.chars();
                                            match ch.next() {
                                                Some(f) => {
                                                    f.to_uppercase().collect::<String>() + ch.as_str()
                                                }
                                                None => String::new(),
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                };
                                // One SLOT row per slot (operator 2026-06-08: "rows of like
                                // slot 1, 2, 3 ... click on the specific plant can expand
                                // that single row to a multirow card"). Each slot is itself
                                // an expandable_row whose body is the plant CARD.
                                for s in 0..slot_count {
                                    let spec = slot_specs.get(s);
                                    // Live crop in this slot: real tower -> by tower_slot;
                                    // loose-crops fallback -> positional.
                                    let crop: Option<&crate::gui::GuiCrop> = if cfg.is_some() {
                                        crops.iter().copied().find(|c| c.tower_slot == Some(s as u32))
                                    } else {
                                        crops.get(s).copied()
                                    };
                                    let plant_id = spec
                                        .map(|sp| sp.0.clone())
                                        .or_else(|| crop.map(|c| c.name.clone()))
                                        .unwrap_or_default();
                                    let role = spec.map(|sp| sp.1.clone()).unwrap_or_default();
                                    let note = spec.map(|sp| sp.2.clone()).unwrap_or_default();
                                    let name = crop.map(|c| c.name.clone()).unwrap_or_else(|| {
                                        if plant_id.is_empty() {
                                            format!("Slot {}", s + 1)
                                        } else {
                                            prettify(&plant_id)
                                        }
                                    });
                                    let name_body = name.clone();
                                    let swatch = widgets::swatch_color(if plant_id.is_empty() {
                                        &name
                                    } else {
                                        &plant_id
                                    });
                                    let glyph = name
                                        .chars()
                                        .next()
                                        .map(|c| c.to_uppercase().to_string())
                                        .unwrap_or_default();
                                    widgets::expandable_row(
                                        ui,
                                        ("garden_slot", gi, s),
                                        false,
                                        tree_force,
                                        |ui| {
                                            widgets::row_cell(ui, theme.cell_narrow_width, |ui| {
                                                ui.label(
                                                    RichText::new(format!("Slot {}", s + 1))
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_muted()),
                                                );
                                            });
                                            widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                                let col = match crop {
                                                    Some(c) if c.dead => theme.danger(),
                                                    Some(c) if c.mature => theme.accent(),
                                                    Some(_) => theme.text_primary(),
                                                    None => theme.text_muted(),
                                                };
                                                ui.label(RichText::new(&name).color(col));
                                            });
                                            widgets::row_cell(ui, theme.cell_short_width, |ui| {
                                                let (txt, col) = match crop {
                                                    Some(c) if c.dead => {
                                                        ("dead".to_string(), theme.danger())
                                                    }
                                                    Some(c) if c.mature => {
                                                        ("ready".to_string(), theme.accent())
                                                    }
                                                    Some(c) => {
                                                        (c.stage.clone(), theme.text_secondary())
                                                    }
                                                    None => {
                                                        ("planned".to_string(), theme.text_muted())
                                                    }
                                                };
                                                ui.label(
                                                    RichText::new(txt)
                                                        .size(theme.font_size_small)
                                                        .color(col),
                                                );
                                            });
                                            if let Some(c) = crop {
                                                ui.add(
                                                    egui::ProgressBar::new(c.progress.clamp(0.0, 1.0))
                                                        .fill(theme.accent())
                                                        .desired_width(theme.status_bar_width)
                                                        .desired_height(theme.status_bar_height),
                                                );
                                            }
                                        },
                                        |ui| {
                                            // THE CARD — colored placeholder tile + name +
                                            // role + description (the planting note) + the
                                            // live details/actions when a crop occupies it.
                                            ui.add_space(theme.spacing_xs);
                                            ui.horizontal_top(|ui| {
                                                widgets::placeholder_tile(
                                                    ui, theme, swatch, 72.0, &glyph,
                                                );
                                                ui.add_space(theme.spacing_sm);
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        RichText::new(&name_body)
                                                            .size(theme.font_size_heading)
                                                            .strong()
                                                            .color(theme.accent()),
                                                    );
                                                    if !role.is_empty() {
                                                        ui.label(
                                                            RichText::new(&role)
                                                                .size(theme.font_size_small)
                                                                .italics()
                                                                .color(theme.text_muted()),
                                                        );
                                                    }
                                                    if !note.is_empty() {
                                                        ui.label(
                                                            RichText::new(&note)
                                                                .color(theme.text_secondary()),
                                                        );
                                                    }
                                                    ui.add_space(theme.spacing_xs);
                                                    match crop {
                                                        Some(c) => {
                                                            egui::Grid::new(("slot_card", gi, s))
                                                                .spacing([10.0, 2.0])
                                                                .show(ui, |ui| {
                                                                    let mut stat =
                                                                        |ui: &mut egui::Ui,
                                                                         k: &str,
                                                                         v: String| {
                                                                            ui.label(
                                                                                RichText::new(k)
                                                                                    .size(theme.font_size_small)
                                                                                    .color(theme.text_muted()),
                                                                            );
                                                                            ui.label(
                                                                                RichText::new(v)
                                                                                    .size(theme.font_size_small)
                                                                                    .color(theme.text_secondary()),
                                                                            );
                                                                            ui.end_row();
                                                                        };
                                                                    let stage = if c.dead {
                                                                        "dead".to_string()
                                                                    } else if c.mature {
                                                                        "ready".to_string()
                                                                    } else {
                                                                        c.stage.clone()
                                                                    };
                                                                    stat(ui, "Stage", stage);
                                                                    stat(ui, "Growth", format!("{:.0}%", c.progress * 100.0));
                                                                    stat(ui, "N·P·K", format!("{:.2} · {:.2} · {:.2}", c.n, c.p, c.k));
                                                                    stat(ui, "Water/day", format!("{:.1} L", c.water_per_day));
                                                                    stat(ui, "Temp window", format!("{:.0}-{:.0} °C", c.temp_min, c.temp_max));
                                                                    stat(ui, "Reservoir", format!("{:.0}%", c.water * 100.0));
                                                                    stat(ui, "Health", format!("{:.0}%", c.health));
                                                                });
                                                            ui.add_space(theme.spacing_xs);
                                                            ui.horizontal(|ui| {
                                                                ui.spacing_mut().item_spacing.x = 4.0;
                                                                if c.mature && widgets::compact_button(ui, theme, "Harvest", widgets::ButtonVariant::Primary) {
                                                                    action_harvest_crop = Some(c.entity_bits);
                                                                }
                                                                if !c.dead && widgets::compact_button(ui, theme, "Water", widgets::ButtonVariant::Secondary) {
                                                                    action_water_crop = Some(c.entity_bits);
                                                                }
                                                                if !c.dead && widgets::compact_button(ui, theme, "Fertilize", widgets::ButtonVariant::Secondary) {
                                                                    action_fertilize_crop = Some(c.entity_bits);
                                                                }
                                                            });
                                                        }
                                                        None => {
                                                            ui.label(
                                                                RichText::new("Not yet planted. Use the Plant button above to fill empty slots.")
                                                                    .size(theme.font_size_small)
                                                                    .color(theme.text_muted()),
                                                            );
                                                        }
                                                    }
                                                });
                                            });
                                            ui.add_space(theme.spacing_xs);
                                        },
                                    );
                                }
                            },
                        );
                    }
                }
                } // ── end Garden ──

                widgets::rgb_section_divider(ui, theme);
                // ── Mining (collapsible) — commission drones to fetch ore from
                //    finite asteroids. Closes before the panel's scroll area. ──
                if widgets::section_disclosure(ui, theme, ("inv_sec", "mining"), "Mining", tree_force) {
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
                    // Asteroids as aligned EXPANDABLE rows (v0.414, the universal
                    // widget): Asteroid | Type | ore summary in the title row;
                    // expand for the per-ore composition, each with a thin bar of
                    // its share of the asteroid's richest deposit.
                    for (ai, ast) in state.asteroids.iter().enumerate() {
                        let summary: Vec<String> = ast
                            .ores
                            .iter()
                            .filter(|(_, q)| *q >= 1.0)
                            .map(|(id, q)| format!("{} {:.0}", ore_short(id), q))
                            .collect();
                        let max_q = ast.ores.iter().map(|(_, q)| *q).fold(0.0f32, f32::max).max(1.0);
                        widgets::expandable_row(
                            ui,
                            ("mining_ast", ai),
                            false,
                            tree_force,
                            |ui| {
                                widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                    ui.label(RichText::new(&ast.name).size(theme.font_size_small).color(theme.text_primary()));
                                });
                                widgets::row_cell(ui, theme.cell_short_width, |ui| {
                                    ui.label(RichText::new(&ast.classification).size(theme.font_size_small).color(theme.text_secondary()));
                                });
                                ui.label(RichText::new(if summary.is_empty() { "depleted".to_string() } else { summary.join(", ") }).size(theme.font_size_small).color(theme.text_secondary()));
                            },
                            |ui| {
                                for (id, qty) in ast.ores.iter().filter(|(_, q)| *q >= 1.0) {
                                    ui.horizontal(|ui| {
                                        widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                            ui.label(RichText::new(ore_short(id)).size(theme.font_size_small).color(theme.text_secondary()));
                                        });
                                        widgets::row_cell(ui, theme.cell_short_width, |ui| {
                                            ui.label(RichText::new(format!("{:.0}", qty)).size(theme.font_size_small).color(theme.text_muted()));
                                        });
                                        ui.add(
                                            egui::ProgressBar::new((qty / max_q).clamp(0.0, 1.0))
                                                .fill(theme.accent())
                                                .desired_width(theme.status_bar_width)
                                                .desired_height(theme.status_bar_height),
                                        );
                                    });
                                }
                            },
                        );
                    }
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
                        // Per-ore allocation as aligned EXPANDABLE rows (v0.414):
                        // Ore | Available | [-] qty [+] in the title row; expand
                        // for which asteroids hold that ore.
                        for (id, avail) in &ores {
                            let cur = state
                                .drone_manifest_draft
                                .iter()
                                .find(|(o, _)| o == id)
                                .map(|(_, u)| *u)
                                .unwrap_or(0);
                            widgets::expandable_row(
                                ui,
                                ("mining_ore", id.as_str()),
                                false,
                                tree_force,
                                |ui| {
                                    widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                        ui.label(RichText::new(ore_short(id)).size(theme.font_size_small).color(theme.text_secondary()));
                                    });
                                    widgets::row_cell(ui, theme.cell_short_width, |ui| {
                                        ui.label(RichText::new(format!("{:.0} left", avail)).size(theme.font_size_small).color(theme.text_muted()));
                                    });
                                    ui.spacing_mut().item_spacing.x = 4.0;
                                    if widgets::stepper_button(ui, theme, "-", cur > 0, false) {
                                        action_manifest_delta = Some((id.clone(), -1));
                                    }
                                    ui.label(RichText::new(format!("{cur}")).color(theme.text_primary()));
                                    if widgets::stepper_button(ui, theme, "+", total < cap, true) {
                                        action_manifest_delta = Some((id.clone(), 1));
                                    }
                                },
                                |ui| {
                                    for ast in &state.asteroids {
                                        if let Some((_, q)) = ast.ores.iter().find(|(o, q)| o == id && *q >= 1.0) {
                                            ui.horizontal(|ui| {
                                                widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                                    ui.label(RichText::new(&ast.name).size(theme.font_size_small).color(theme.text_secondary()));
                                                });
                                                ui.label(RichText::new(format!("{:.0} available", q)).size(theme.font_size_small).color(theme.text_muted()));
                                            });
                                        }
                                    }
                                },
                            );
                        }
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
                    // The active drone (one per player) as an aligned EXPANDABLE row
                    // (v0.414): Stage | status | progress bar in the title row;
                    // expand for the manifest it's fetching + cargo on board.
                    for (di, drone) in state.drones.iter().enumerate() {
                        let (stage, desc) = match drone.phase.as_str() {
                            "Outbound" => ("1/3", "outbound"),
                            "Mining" => ("2/3", "mining"),
                            "Returning" => ("3/3", "returning"),
                            _ => ("done", "delivering"),
                        };
                        widgets::expandable_row(
                            ui,
                            ("mining_drone", di),
                            false,
                            tree_force,
                            |ui| {
                                widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                    ui.label(RichText::new(format!("Drone, stage {stage}")).size(theme.font_size_small).color(theme.text_primary()));
                                });
                                widgets::row_cell(ui, theme.cell_short_width, |ui| {
                                    ui.label(RichText::new(desc).size(theme.font_size_small).color(theme.text_secondary()));
                                });
                                ui.add(egui::ProgressBar::new(drone.phase_progress.clamp(0.0, 1.0)).fill(theme.accent()).desired_width(theme.status_bar_width).desired_height(theme.status_bar_height));
                            },
                            |ui| {
                                let fetching: Vec<String> = drone
                                    .manifest
                                    .iter()
                                    .map(|(o, u)| format!("{}x {}", u, ore_short(o)))
                                    .collect();
                                ui.horizontal(|ui| {
                                    widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                        ui.label(RichText::new("Fetching").size(theme.font_size_small).color(theme.text_muted()));
                                    });
                                    ui.label(RichText::new(fetching.join(", ")).size(theme.font_size_small).color(theme.text_secondary()));
                                });
                                ui.horizontal(|ui| {
                                    widgets::row_cell(ui, theme.cell_name_width, |ui| {
                                        ui.label(RichText::new("Cargo").size(theme.font_size_small).color(theme.text_muted()));
                                    });
                                    ui.label(RichText::new(format!("{} units", drone.cargo_total)).size(theme.font_size_small).color(theme.text_secondary()));
                                });
                            },
                        );
                    }
                }
                } // ── end Mining ──
        }); // close the single-panel ScrollArea
        }); // close the single CentralPanel

    // Apply the inline item card's quick action (set under the selected item row,
    // inside the panel) to GuiState now that the panel — and the snapshot-borrowing
    // card closure — are done. Drop clears the slot; Equip fills the first free
    // equipment slot; Eat/Drink/Plant bridge into the Food/Farming systems.
    if item_acts.drop {
        if let Some(idx) = state.selected_slot {
            if idx < state.inventory_items.len() {
                state.inventory_items[idx] = None;
                state.selected_slot = None;
                with_state(|ps| ps.initialized = false); // recalc weight
            }
        }
    }
    if item_acts.equip {
        if let Some(idx) = state.selected_slot {
            if let Some(Some(item)) = state.inventory_items.get(idx) {
                let item_name = item.name.clone();
                with_state(|ps| {
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
    if let Some(item_id) = item_acts.eat {
        state.pending_consume_item = Some(item_id);
    }
    if let Some(item_id) = item_acts.drink {
        state.pending_drink_item = Some(item_id);
    }
    if let Some(seed_id) = item_acts.plant {
        state.pending_plant_seed = Some(seed_id);
    }

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
