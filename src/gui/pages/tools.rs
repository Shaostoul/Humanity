//! Tools -- the catalog of EXTERNAL things, searchable, with kind + category
//! filters. Two kinds live here (v0.1063):
//!
//! - `software`: free programs you download and run yourself (Blender, GIMP...).
//! - `service`: real-world help websites and organizations (water, food,
//!   housing, legal aid...), which used to be the Library page's third face.
//!
//! Both come from the single `data/external/catalog.json` via
//! `state.tools_catalog`, loaded at startup. The web mirror is
//! `web/pages/tools.html` reading the same file, so the two stay in step.
//! Library is what you READ; this page is what you GO USE.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

/// Local page state.
pub struct ToolsPageState {
    pub search: String,
    pub active_category: Option<String>,
    /// Kind id filter: None = everything, else "software" or "service".
    pub active_kind: Option<String>,
}

impl Default for ToolsPageState {
    fn default() -> Self {
        Self {
            search: String::new(),
            active_category: None,
            active_kind: None,
        }
    }
}

/// Display label for a kind id (the ids themselves are data, these are the
/// human words for the two we ship).
fn kind_label(kind: &str) -> &str {
    match kind {
        "software" => "Software",
        "service" => "Help and services",
        other => other,
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
                RichText::new("Tools")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new("Free software you can install, and real-world help services. Everything here is external: we point, you choose.")
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_xs);

            // Search bar
            with_state(|ts| {
                widgets::search_bar(ui, theme, &mut ts.search, "Search tools and services...");
            });
            ui.add_space(theme.spacing_xs);

            // Kind filter (software / services), in first-seen catalog order.
            let mut kinds: Vec<String> = Vec::new();
            for t in &state.tools_catalog {
                if !t.kind.is_empty() && !kinds.contains(&t.kind) {
                    kinds.push(t.kind.clone());
                }
            }
            with_state(|ts| {
                ui.horizontal_wrapped(|ui| {
                    if widgets::Button::secondary("Everything")
                        .active(ts.active_kind.is_none())
                        .show(ui, theme)
                    {
                        ts.active_kind = None;
                        ts.active_category = None;
                    }
                    for k in &kinds {
                        let is_active = ts.active_kind.as_deref() == Some(k.as_str());
                        if widgets::Button::secondary(kind_label(k)).active(is_active).show(ui, theme) {
                            ts.active_kind = if is_active { None } else { Some(k.clone()) };
                            // Categories are kind-scoped, so a stale category
                            // filter would silently show nothing.
                            ts.active_category = None;
                        }
                    }
                });
            });
            ui.add_space(theme.spacing_xs);

            // Category list, scoped to the active kind so the filter row only
            // ever offers categories that can actually match.
            let active_kind = with_state(|ts| ts.active_kind.clone());
            let mut categories: Vec<String> = state
                .tools_catalog
                .iter()
                .filter(|t| active_kind.as_deref().map_or(true, |k| t.kind == k))
                .map(|t| t.category.clone())
                .collect();
            categories.sort();
            categories.dedup();

            // Category filter buttons
            with_state(|ts| {
                ui.horizontal_wrapped(|ui| {
                    // "All" filter — Secondary that flips to accent via .active().
                    if widgets::Button::secondary("All")
                        .active(ts.active_category.is_none())
                        .show(ui, theme)
                    {
                        ts.active_category = None;
                    }

                    for cat in &categories {
                        let is_active = ts.active_category.as_deref() == Some(cat.as_str());
                        if widgets::Button::secondary(cat)
                            .active(is_active)
                            .show(ui, theme)
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
                                let matches_kind = ts
                                    .active_kind
                                    .as_deref()
                                    .map_or(true, |k| t.kind == k);
                                let matches_cat = ts
                                    .active_category
                                    .as_deref()
                                    .map_or(true, |c| t.category == c);
                                let matches_search = search_lower.is_empty()
                                    || t.name.to_lowercase().contains(&search_lower)
                                    || t.description.to_lowercase().contains(&search_lower)
                                    || t.category.to_lowercase().contains(&search_lower);
                                matches_kind && matches_cat && matches_search
                            })
                            .collect();

                        if filtered.is_empty() {
                            ui.label(
                                RichText::new("Nothing matches your search.")
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
                                            // Category badge, plus the license
                                            // when there is one (software only).
                                            ui.horizontal(|ui| {
                                                widgets::badge(ui, theme, &tool.category, Theme::c32(&theme.info));
                                                if !tool.license.is_empty() {
                                                    ui.label(
                                                        RichText::new(&tool.license)
                                                            .size(theme.font_size_small)
                                                            .color(theme.text_muted()),
                                                    );
                                                }
                                            });
                                            // Description
                                            ui.label(
                                                RichText::new(&tool.description)
                                                    .size(theme.font_size_small)
                                                    .color(theme.text_secondary()),
                                            );
                                            // Platforms + size, software only. A
                                            // help service has neither, so the row
                                            // is skipped rather than left blank.
                                            if !tool.platforms.is_empty() {
                                                let mut meta = tool.platforms.join(", ");
                                                if !tool.size.is_empty() {
                                                    meta.push_str(" · ");
                                                    meta.push_str(&tool.size);
                                                }
                                                ui.label(
                                                    RichText::new(meta)
                                                        .size(theme.font_size_small)
                                                        .color(theme.text_muted()),
                                                );
                                            }
                                            // The action names what actually
                                            // happens: software downloads, a
                                            // service just opens in the browser.
                                            let action = if tool.kind == "service" { "Open website" } else { "Download" };
                                            if widgets::primary_button(ui, theme, action) {
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
