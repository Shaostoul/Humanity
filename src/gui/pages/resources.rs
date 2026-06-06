//! Curated Resources page: a directory of everything HumanityOS points you to.
//!
//! Categories load from `data/resources/catalog.json` into
//! `GuiState.resource_categories` at startup. To add or change a resource,
//! edit the JSON, no code change required.
//!
//! Layout: a narrow category rail on the left (All + each category) and a dense,
//! wrapping grid of link cards on the right. "All" is the default so the whole
//! directory is visible at a glance (operator 2026-06-05: the old design "showed
//! tons of links, easy to see everything we link to"; one category at a time
//! read as barely populated).

use egui::{Align, Color32, Frame, Layout, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiState, ResourceEntry};
use crate::gui::theme::Theme;

/// Local page state. `selected_category`: 0 = All, 1..=N = the (i-1)th category.
pub struct ResourcesPageState {
    pub selected_category: usize,
}

impl Default for ResourcesPageState {
    fn default() -> Self {
        Self { selected_category: 0 }
    }
}

fn with_state<R>(f: impl FnOnce(&mut ResourcesPageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<ResourcesPageState> = RefCell::new(ResourcesPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Resources")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("Everything we point you to, in one place. Browse All, or filter by category.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.separator();

            let n = state.resource_categories.len();
            let rail_w = 160.0;
            let content_w = (ui.available_width() - rail_w - 24.0).max(280.0);
            let body_h = ui.available_height();

            ui.horizontal_top(|ui| {
                // Left rail: All + each category.
                ui.allocate_ui_with_layout(
                    Vec2::new(rail_w, body_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        ui.label(
                            RichText::new("Categories")
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                        ui.add_space(theme.spacing_xs);
                        with_state(|rs| {
                            if rs.selected_category > n {
                                rs.selected_category = 0;
                            }
                            if rail_item(ui, theme, "All", rs.selected_category == 0) {
                                rs.selected_category = 0;
                            }
                            for (i, cat) in state.resource_categories.iter().enumerate() {
                                if rail_item(ui, theme, cat.name.as_str(), rs.selected_category == i + 1) {
                                    rs.selected_category = i + 1;
                                }
                            }
                        });
                    },
                );

                ui.separator();

                // Right: dense, wrapping link grid.
                ui.allocate_ui_with_layout(
                    Vec2::new(content_w, body_h),
                    Layout::top_down(Align::Min),
                    |ui| {
                        with_state(|rs| {
                            ScrollArea::vertical()
                                .id_salt("resource_content")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    if state.resource_categories.is_empty() {
                                        ui.label(
                                            RichText::new("No resources loaded.")
                                                .color(theme.text_muted()),
                                        );
                                        return;
                                    }
                                    if rs.selected_category == 0 {
                                        // All: every category, grouped.
                                        for cat in &state.resource_categories {
                                            category_block(ui, theme, cat.name.as_str(), &cat.real_resources);
                                        }
                                    } else if let Some(cat) =
                                        state.resource_categories.get(rs.selected_category - 1)
                                    {
                                        category_block(ui, theme, cat.name.as_str(), &cat.real_resources);
                                    }
                                });
                        });
                    },
                );
            });
        });
}

/// One left-rail entry. Returns true if it was clicked this frame.
fn rail_item(ui: &mut egui::Ui, theme: &Theme, label: &str, selected: bool) -> bool {
    let fill = if selected { theme.bg_card() } else { Color32::TRANSPARENT };
    let mut clicked = false;
    Frame::none()
        .fill(fill)
        .rounding(Rounding::same(theme.border_radius as u8))
        .inner_margin(Vec2::new(10.0, 6.0))
        .show(ui, |ui| {
            let w = ui.available_width();
            ui.set_width(w);
            let color = if selected { theme.accent() } else { theme.text_primary() };
            if ui
                .selectable_label(false, RichText::new(label).color(color))
                .clicked()
            {
                clicked = true;
            }
        });
    clicked
}

/// A category header followed by its links as a dense, wrapping grid of cards.
fn category_block(ui: &mut egui::Ui, theme: &Theme, name: &str, resources: &[ResourceEntry]) {
    ui.label(
        RichText::new(name)
            .size(theme.font_size_body)
            .strong()
            .color(theme.accent()),
    );
    ui.add_space(theme.spacing_xs);
    if resources.is_empty() {
        ui.label(
            RichText::new("Nothing here yet.")
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
    } else {
        ui.horizontal_wrapped(|ui| {
            for res in resources {
                resource_card(ui, theme, res);
            }
        });
    }
    ui.add_space(theme.spacing_md);
}

/// One fixed-width link card (title, description, url).
fn resource_card(ui: &mut egui::Ui, theme: &Theme, res: &ResourceEntry) {
    Frame::none()
        .fill(theme.bg_card())
        .rounding(Rounding::same(theme.border_radius as u8))
        .stroke(Stroke::new(1.0, theme.border()))
        .inner_margin(Vec2::new(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(240.0);
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(res.title.as_str())
                        .size(theme.font_size_body)
                        .strong()
                        .color(theme.text_primary()),
                );
                ui.label(
                    RichText::new(res.description.as_str())
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
                ui.label(
                    RichText::new(res.url.as_str())
                        .size(theme.font_size_small)
                        .color(Theme::c32(&theme.info)),
                );
            });
        });
    ui.add_space(8.0);
}
