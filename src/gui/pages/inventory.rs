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
                            if widgets::primary_button(ui, theme, "Use") {
                                // Placeholder
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
            });
        });
}

// detail_row moved to crate::gui::widgets::detail_row
