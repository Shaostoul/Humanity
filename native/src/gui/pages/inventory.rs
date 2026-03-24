//! Inventory grid with item slots — reads from player's ECS Inventory component.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

const COLS: usize = 6;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let total_slots = state.inventory_max_slots.max(1);

    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header with slot count
            let used = state.inventory_items.iter().filter(|s| s.is_some()).count();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Inventory").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(format!("{}/{} slots", used, total_slots)).color(theme.text_muted()));
                });
            });
            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical().show(ui, |ui| {
                // Item grid
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

                            let fill = if selected { theme.bg_card() } else { theme.bg_secondary() };
                            ui.painter().rect_filled(rect, Rounding::same(4), fill);
                            ui.painter().rect_stroke(rect, Rounding::same(4), stroke, egui::StrokeKind::Outside);

                            // Draw item if slot is occupied
                            if let Some(Some(item)) = state.inventory_items.get(i) {
                                let icon = item.name.chars().next().unwrap_or('?').to_string();
                                let center = rect.center();
                                ui.painter().text(
                                    center,
                                    egui::Align2::CENTER_CENTER,
                                    &icon,
                                    egui::FontId::proportional(18.0),
                                    theme.text_primary(),
                                );
                                ui.painter().text(
                                    rect.right_bottom() + Vec2::new(-4.0, -2.0),
                                    egui::Align2::RIGHT_BOTTOM,
                                    item.quantity.to_string(),
                                    egui::FontId::proportional(10.0),
                                    theme.text_muted(),
                                );
                            }

                            if (i + 1) % COLS == 0 {
                                ui.end_row();
                            }
                        }
                    });

                // Detail panel for selected slot
                if let Some(idx) = state.selected_slot {
                    ui.add_space(theme.spacing_md);
                    if let Some(Some(item)) = state.inventory_items.get(idx) {
                        widgets::card(ui, theme, |ui| {
                            ui.label(RichText::new(&item.name).size(theme.font_size_heading).color(theme.accent()));
                            ui.label(RichText::new(format!("ID: {}", item.item_id)).color(theme.text_muted()));
                            ui.label(RichText::new(format!("Quantity: {}", item.quantity)).color(theme.text_secondary()));
                        });
                    } else {
                        ui.label(RichText::new("Empty slot").color(theme.text_muted()));
                    }
                }
            });
        });
}
