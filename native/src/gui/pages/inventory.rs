//! Inventory page — grid of item slots with detail panel.

use egui::{Align2, Area, Color32, Frame, Margin, Rounding, Stroke, StrokeKind};
use crate::gui::{GuiPage, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Number of columns in the inventory grid.
const GRID_COLUMNS: usize = 6;
/// Total number of inventory slots.
const TOTAL_SLOTS: usize = 36;
/// Size of each inventory slot in pixels.
const SLOT_SIZE: f32 = 56.0;

/// Placeholder inventory item for demo purposes.
struct DemoItem {
    name: &'static str,
    icon: &'static str,
    count: u32,
    description: &'static str,
}

/// Demo items to populate a few slots.
fn demo_items() -> Vec<Option<DemoItem>> {
    let mut items: Vec<Option<DemoItem>> = Vec::with_capacity(TOTAL_SLOTS);
    for _ in 0..TOTAL_SLOTS {
        items.push(None);
    }
    items[0] = Some(DemoItem { name: "Iron Ore", icon: "Fe", count: 64, description: "Raw iron ore mined from asteroids." });
    items[1] = Some(DemoItem { name: "Steel Plate", icon: "St", count: 12, description: "Refined steel plating for construction." });
    items[2] = Some(DemoItem { name: "Copper Wire", icon: "Cu", count: 30, description: "Conductive wiring for electronics." });
    items[5] = Some(DemoItem { name: "Seeds", icon: "Sd", count: 8, description: "Crop seeds for farming." });
    items[6] = Some(DemoItem { name: "Water", icon: "H2", count: 5, description: "Purified water canister." });
    items[12] = Some(DemoItem { name: "Fuel Cell", icon: "Fc", count: 3, description: "Hydrogen fuel cell for vehicles." });
    items
}

/// Draw the inventory overlay.
pub fn draw(ctx: &egui::Context, theme: &Theme, gui_state: &mut GuiState) {
    let items = demo_items();

    Area::new(egui::Id::new("inventory_panel"))
        .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme.panel_bg)
                .rounding(theme.rounding)
                .inner_margin(Margin::same(24))
                .show(ui, |ui| {
                    // Header
                    ui.horizontal(|ui| {
                        ui.label(theme.heading("Inventory"));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if widgets::secondary_button(ui, theme, "Close") {
                                gui_state.active_page = GuiPage::None;
                            }
                        });
                    });
                    ui.separator();
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        // Grid on the left
                        ui.vertical(|ui| {
                            draw_grid(ui, theme, gui_state, &items);
                        });

                        ui.add_space(16.0);

                        // Detail panel on the right
                        ui.vertical(|ui| {
                            draw_detail(ui, theme, gui_state, &items);
                        });
                    });
                });
        });
}

fn draw_grid(
    ui: &mut egui::Ui,
    theme: &Theme,
    gui_state: &mut GuiState,
    items: &[Option<DemoItem>],
) {
    let rows = (TOTAL_SLOTS + GRID_COLUMNS - 1) / GRID_COLUMNS;

    for row in 0..rows {
        ui.horizontal(|ui| {
            for col in 0..GRID_COLUMNS {
                let idx = row * GRID_COLUMNS + col;
                if idx >= TOTAL_SLOTS {
                    break;
                }

                let selected = gui_state.selected_slot == Some(idx);
                let has_item = items.get(idx).map_or(false, |i| i.is_some());

                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(SLOT_SIZE, SLOT_SIZE),
                    egui::Sense::click(),
                );

                // Slot background
                let bg = if selected {
                    theme.primary.linear_multiply(0.3)
                } else if response.hovered() && has_item {
                    Color32::from_rgba_premultiplied(60, 60, 80, 200)
                } else {
                    Color32::from_rgba_premultiplied(35, 35, 50, 200)
                };

                let stroke = if selected {
                    Stroke::new(2.0, theme.primary)
                } else {
                    Stroke::new(1.0, Color32::from_rgba_premultiplied(60, 60, 80, 150))
                };

                ui.painter().rect(rect, Rounding::same(4), bg, stroke, StrokeKind::Outside);

                // Item content
                if let Some(Some(item)) = items.get(idx) {
                    // Icon text (placeholder)
                    ui.painter().text(
                        rect.center() - egui::vec2(0.0, 6.0),
                        egui::Align2::CENTER_CENTER,
                        item.icon,
                        egui::FontId::new(16.0, egui::FontFamily::Monospace),
                        theme.text,
                    );

                    // Stack count in bottom-right corner
                    if item.count > 1 {
                        ui.painter().text(
                            rect.right_bottom() - egui::vec2(4.0, 4.0),
                            egui::Align2::RIGHT_BOTTOM,
                            format!("{}", item.count),
                            egui::FontId::new(10.0, egui::FontFamily::Proportional),
                            theme.text_dim,
                        );
                    }
                }

                if response.clicked() && has_item {
                    gui_state.selected_slot = Some(idx);
                }
            }
        });
    }
}

fn draw_detail(
    ui: &mut egui::Ui,
    theme: &Theme,
    gui_state: &GuiState,
    items: &[Option<DemoItem>],
) {
    ui.set_min_width(180.0);

    widgets::card(ui, theme, "Item Details", |ui| {
        if let Some(idx) = gui_state.selected_slot {
            if let Some(Some(item)) = items.get(idx) {
                ui.label(theme.subheading(item.name));
                ui.add_space(4.0);
                ui.label(theme.body(item.description));
                ui.add_space(8.0);
                ui.label(theme.dimmed(&format!("Quantity: {}", item.count)));
            } else {
                ui.label(theme.dimmed("Empty slot"));
            }
        } else {
            ui.label(theme.dimmed("Select an item to view details"));
        }
    });
}
