//! Curated Resources page — context-aware (Real/Sim) resource directory.
//!
//! Categories load from `data/resources/catalog.json` into
//! `GuiState.resource_categories` at startup. To add or change a resource,
//! edit the JSON — no code change required.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Local page state.
pub struct ResourcesPageState {
    pub selected_category: usize,
}

impl Default for ResourcesPageState {
    fn default() -> Self {
        Self {
            selected_category: 0,
        }
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
    let is_real = state.context_real;

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Resources")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let mode_label = if is_real { "Real" } else { "Sim" };
                    let mode_color = if is_real { theme.success() } else { Theme::c32(&theme.info) };
                    egui::Frame::none()
                        .fill(mode_color)
                        .rounding(Rounding::same(3))
                        .inner_margin(Vec2::new(8.0, 3.0))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(mode_label)
                                    .size(theme.font_size_small)
                                    .color(Color32::WHITE),
                            );
                        });
                });
            });
            ui.separator();

            ui.columns(2, |cols| {
                // Left: category list
                cols[0].label(
                    RichText::new("Categories")
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                cols[0].add_space(theme.spacing_xs);

                with_state(|rs| {
                    // Clamp selection in case the loaded catalog has fewer categories
                    // than a previously persisted index.
                    if rs.selected_category >= state.resource_categories.len() {
                        rs.selected_category = 0;
                    }
                    for (i, cat) in state.resource_categories.iter().enumerate() {
                        let selected = rs.selected_category == i;
                        let fill = if selected {
                            theme.bg_card()
                        } else {
                            Color32::TRANSPARENT
                        };
                        egui::Frame::none()
                            .fill(fill)
                            .rounding(Rounding::same(theme.border_radius as u8))
                            .inner_margin(Vec2::new(12.0, 6.0))
                            .show(&mut cols[0], |ui| {
                                let text_color = if selected {
                                    theme.accent()
                                } else {
                                    theme.text_primary()
                                };
                                let resp = ui.selectable_label(
                                    false,
                                    RichText::new(cat.name.as_str()).color(text_color),
                                );
                                if resp.clicked() {
                                    rs.selected_category = i;
                                }
                            });
                    }
                });

                // Right: resource cards
                with_state(|rs| {
                    let Some(cat) = state.resource_categories.get(rs.selected_category) else {
                        cols[1].label(RichText::new("No resources loaded.").color(theme.text_muted()));
                        return;
                    };
                    let resources: &[crate::gui::ResourceEntry] = if is_real {
                        &cat.real_resources
                    } else {
                        &cat.sim_resources
                    };

                    cols[1].label(
                        RichText::new(cat.name.as_str())
                            .size(theme.font_size_body)
                            .color(theme.accent()),
                    );
                    cols[1].add_space(theme.spacing_xs);

                    ScrollArea::vertical()
                        .id_salt("resource_cards")
                        .show(&mut cols[1], |ui| {
                            for res in resources {
                                widgets::card(ui, theme, |ui| {
                                    ui.label(
                                        RichText::new(res.title.as_str())
                                            .size(theme.font_size_body)
                                            .color(theme.text_primary())
                                            .strong(),
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
                                ui.add_space(4.0);
                            }
                        });
                });
            });
        });
}
