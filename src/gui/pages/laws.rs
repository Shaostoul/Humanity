//! Laws page (v0.496): the rules that apply where you live, nested from Humanity
//! down to your town. Two kinds: the HumanityOS base set (our framework,
//! distilled from the Humanity Accord) and real laws (plain-language summaries
//! with a source). Data-driven from `data/laws/laws.json` via `gui::laws`.
//! See docs/design/laws.md.

use egui::{Frame, Margin, RichText, Rounding, ScrollArea, Stroke};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// A small bordered chip (BASE / REAL kind badge). Outline + colored text so it
/// reads as a real chip (operator ask, 2026-07-01) without any literal colors.
fn kind_chip(ui: &mut egui::Ui, theme: &Theme, label: &str, color: egui::Color32) {
    Frame::none()
        .rounding(Rounding::same(6))
        .inner_margin(Margin::symmetric(6, 1))
        .stroke(Stroke::new(1.0, color))
        .show(ui, |ui| {
            ui.label(RichText::new(label).strong().color(color).size(theme.font_size_small));
        });
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let laws = crate::gui::laws::install();
    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "Laws and rights");
                ui.label(
                    RichText::new(
                        "The rules that apply where you live, nested from Humanity down to your \
                         town. Two kinds: the HumanityOS base set (our framework, from the Humanity \
                         Accord) and real laws (plain-language summaries with a source to verify).",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_sm);

                if !laws.disclaimer.is_empty() {
                    widgets::card(ui, theme, |ui| {
                        ui.label(
                            RichText::new(&laws.disclaimer)
                                .color(theme.text_muted())
                                .size(theme.font_size_small),
                        );
                    });
                }

                // Where you are: pick a jurisdiction; the breadcrumb shows the nest.
                widgets::card_with_header(ui, theme, "Where you are", |ui| {
                    if state.laws_location.is_empty() {
                        state.laws_location = laws
                            .jurisdictions
                            .last()
                            .map(|j| j.id.clone())
                            .unwrap_or_default();
                    }
                    let cur = laws.jurisdiction_name(&state.laws_location).to_string();
                    egui::ComboBox::from_id_salt("laws_location")
                        .selected_text(cur)
                        .show_ui(ui, |ui| {
                            for j in &laws.jurisdictions {
                                ui.selectable_value(
                                    &mut state.laws_location,
                                    j.id.clone(),
                                    format!("{} ({})", j.name, j.level),
                                );
                            }
                        });
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new(laws.location_breadcrumb(&state.laws_location))
                            .color(theme.text_secondary())
                            .size(theme.font_size_small),
                    );
                });

                ui.add_space(theme.spacing_sm);
                let kinds = ["All", "HumanityOS base", "Real laws"];
                widgets::tab_bar(ui, theme, &kinds, &mut state.laws_filter_tab);
                // Category chips from the data file's own `categories` list
                // (loaded since v0.496 but never surfaced until 2026-07-01).
                // Click to filter to one category; click again to clear.
                if !laws.categories.is_empty() {
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal_wrapped(|ui| {
                        if widgets::Button::tab("All categories", state.laws_category.is_empty())
                            .show(ui, theme)
                        {
                            state.laws_category.clear();
                        }
                        for cat in &laws.categories {
                            let active = state.laws_category == *cat;
                            if widgets::Button::tab(cat, active).show(ui, theme) {
                                state.laws_category = if active { String::new() } else { cat.clone() };
                            }
                        }
                    });
                }
                ui.add_space(theme.spacing_xs);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Search")
                            .color(theme.text_secondary())
                            .size(theme.font_size_small),
                    );
                    ui.text_edit_singleline(&mut state.laws_search);
                });
                ui.add_space(theme.spacing_sm);

                // Applicable rules, broadest (Humanity) first down to local,
                // grouped under each jurisdiction.
                let rules = laws.applicable_rules(&state.laws_location);
                let q = state.laws_search.trim().to_lowercase();
                let mut last_jur = String::new();
                let mut shown = 0usize;
                for r in rules {
                    if state.laws_filter_tab == 1 && !r.is_base() {
                        continue;
                    }
                    if state.laws_filter_tab == 2 && r.is_base() {
                        continue;
                    }
                    if !state.laws_category.is_empty() && r.category != state.laws_category {
                        continue;
                    }
                    if !q.is_empty() {
                        let hay = format!(
                            "{} {} {} {}",
                            r.title,
                            r.summary,
                            r.category,
                            r.tags.join(" ")
                        )
                        .to_lowercase();
                        if !hay.contains(&q) {
                            continue;
                        }
                    }
                    if r.jurisdiction != last_jur {
                        last_jur = r.jurisdiction.clone();
                        ui.add_space(theme.spacing_xs);
                        ui.label(
                            RichText::new(laws.jurisdiction_name(&r.jurisdiction))
                                .strong()
                                .color(theme.text_primary()),
                        );
                    }
                    // One compact row per rule for fast scanning of the bulk:
                    // [BASE/REAL] [category] [title] then the summary fills the
                    // remaining width (one line, ellipsized). Click to expand the
                    // row IN PLACE (no modal) into the full summary + source + tags,
                    // pushing the rows below it down.
                    let (badge, badge_col) = if r.is_base() {
                        ("BASE", theme.accent())
                    } else {
                        ("REAL", theme.success())
                    };
                    widgets::expandable_row(
                        ui,
                        ("law", r.id.as_str()),
                        false,
                        None,
                        |ui| {
                            // All left-aligned natural flow: a small BASE/REAL chip,
                            // the category, the title (bold, never clipped), then the
                            // summary taking whatever's left on the line, ellipsized.
                            // No fixed-width cells -- those clipped long titles and left
                            // gaps before short ones.
                            kind_chip(ui, theme, badge, badge_col);
                            if !r.category.is_empty() {
                                ui.label(
                                    RichText::new(&r.category)
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&r.title).strong().color(theme.text_primary()),
                                )
                                .wrap_mode(egui::TextWrapMode::Extend),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&r.summary)
                                        .color(theme.text_secondary())
                                        .size(theme.font_size_small),
                                )
                                .truncate(),
                            );
                        },
                        |ui| {
                            ui.add_space(theme.spacing_xs);
                            ui.label(RichText::new(&r.summary).color(theme.text_secondary()));
                            if !r.source.is_empty() {
                                ui.add_space(theme.spacing_xs);
                                ui.label(
                                    RichText::new(format!("Source: {}", r.source))
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                            if !r.tags.is_empty() {
                                ui.label(
                                    RichText::new(format!("Tags: {}", r.tags.join(", ")))
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                            ui.add_space(theme.spacing_xs);
                        },
                    );
                    shown += 1;
                }
                if shown == 0 {
                    ui.add_space(theme.spacing_md);
                    ui.label(
                        RichText::new("No rules match. Try the All filter or clear the search.")
                            .color(theme.text_muted()),
                    );
                }
            });
        });
}
