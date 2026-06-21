//! Laws page (v0.496): the rules that apply where you live, nested from Humanity
//! down to your town. Two kinds: the HumanityOS base set (our framework,
//! distilled from the Humanity Accord) and real laws (plain-language summaries
//! with a source). Data-driven from `data/laws/laws.json` via `gui::laws`.
//! See docs/design/laws.md.

use egui::{RichText, ScrollArea};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

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
                    widgets::card(ui, theme, |ui| {
                        ui.horizontal(|ui| {
                            let (badge, col) = if r.is_base() {
                                ("BASE", theme.accent())
                            } else {
                                ("REAL", theme.success())
                            };
                            ui.label(
                                RichText::new(badge)
                                    .strong()
                                    .color(col)
                                    .size(theme.font_size_small),
                            );
                            ui.label(RichText::new(&r.title).strong().color(theme.text_primary()));
                        });
                        if !r.category.is_empty() {
                            ui.label(
                                RichText::new(&r.category)
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                            );
                        }
                        ui.label(RichText::new(&r.summary).color(theme.text_secondary()));
                        if !r.source.is_empty() {
                            ui.label(
                                RichText::new(format!("Source: {}", r.source))
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                            );
                        }
                    });
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
