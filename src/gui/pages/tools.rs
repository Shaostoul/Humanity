//! Open Source Tools Catalog -- searchable grid of tools with category filters.
//! Reads from `state.tools_catalog` which is loaded from data/tools/catalog.json at startup.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Local page state.
pub struct ToolsPageState {
    pub search: String,
    pub active_category: Option<String>,
}

impl Default for ToolsPageState {
    fn default() -> Self {
        Self {
            search: String::new(),
            active_category: None,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut ToolsPageState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<ToolsPageState> = RefCell::new(ToolsPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Open Source Tools")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.add_space(theme.spacing_xs);

            // Search bar
            with_state(|ts| {
                widgets::search_bar(ui, theme, &mut ts.search, "Filter tools...");
            });
            ui.add_space(theme.spacing_xs);

            // Build sorted/deduped category list from loaded catalog
            let mut categories: Vec<String> = state
                .tools_catalog
                .iter()
                .map(|t| t.category.clone())
                .collect();
            categories.sort();
            categories.dedup();

            // Category filter buttons
            with_state(|ts| {
                ui.horizontal_wrapped(|ui| {
                    // "All" button
                    let all_active = ts.active_category.is_none();
                    let all_text = if all_active {
                        RichText::new("All").color(theme.text_on_accent()).size(theme.font_size_small)
                    } else {
                        RichText::new("All").color(theme.text_secondary()).size(theme.font_size_small)
                    };
                    let all_fill = if all_active { theme.accent() } else { Color32::TRANSPARENT };
                    if ui
                        .add(
                            egui::Button::new(all_text)
                                .fill(all_fill)
                                .rounding(Rounding::same(theme.border_radius as u8)),
                        )
                        .clicked()
                    {
                        ts.active_category = None;
                    }

                    for cat in &categories {
                        let is_active = ts.active_category.as_deref() == Some(cat.as_str());
                        let text = if is_active {
                            RichText::new(cat).color(theme.text_on_accent()).size(theme.font_size_small)
                        } else {
                            RichText::new(cat).color(theme.text_secondary()).size(theme.font_size_small)
                        };
                        let fill = if is_active { theme.accent() } else { Color32::TRANSPARENT };
                        if ui
                            .add(
                                egui::Button::new(text)
                                    .fill(fill)
                                    .rounding(Rounding::same(theme.border_radius as u8)),
                            )
                            .clicked()
                        {
                            ts.active_category = if is_active { None } else { Some(cat.clone()) };
                        }
                    }
                });
            });

            ui.separator();

            // Tool cards grid
            ScrollArea::vertical()
                .id_salt("tools_grid")
                .show(ui, |ui| {
                    with_state(|ts| {
                        let search_lower = ts.search.to_lowercase();
                        let filtered: Vec<_> = state
                            .tools_catalog
                            .iter()
                            .filter(|t| {
                                let matches_cat = ts
                                    .active_category
                                    .as_deref()
                                    .map_or(true, |c| t.category == c);
                                let matches_search = search_lower.is_empty()
                                    || t.name.to_lowercase().contains(&search_lower)
                                    || t.description.to_lowercase().contains(&search_lower)
                                    || t.category.to_lowercase().contains(&search_lower);
                                matches_cat && matches_search
                            })
                            .collect();

                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("No tools match your search.")
                                    .color(theme.text_muted()),
                            );
                        }

                        // Two-column grid layout
                        let cols = 2;
                        egui::Grid::new("tools_card_grid")
                            .num_columns(cols)
                            .spacing(Vec2::new(theme.spacing_sm, theme.spacing_sm))
                            .show(ui, |ui| {
                                for (i, tool) in filtered.iter().enumerate() {
                                    widgets::card(ui, theme, |ui| {
                                            ui.set_min_width(260.0);
                                            // Name
                                            ui.label(
                                                RichText::new(&tool.name)
                                                    .size(theme.font_size_body)
                                                    .color(theme.text_primary())
                                                    .strong(),
                                            );
                                            // Category badge
                                            ui.horizontal(|ui| {
                                                widgets::badge(ui, theme, &tool.category, Theme::c32(&theme.info));
                                                ui.label(
                                                    RichText::new(&tool.license)
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_muted()),
                                                );
                                            });
                                            // Description
                                            ui.label(
                                                RichText::new(&tool.description)
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_secondary()),
                                            );
                                            // Platforms
                                            ui.label(
                                                RichText::new(tool.platforms.join(", "))
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_muted()),
                                            );
                                            // Download link
                                            if widgets::primary_button(ui, theme, "Download") {
                                                ui.ctx().open_url(egui::OpenUrl::new_tab(&tool.url));
                                            }
                                        });

                                    if (i + 1) % cols == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
                });
        });
}
