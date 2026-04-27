//! Bug Reporter page — submit bug reports with severity and category.

use egui::{Color32, Frame, RichText, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;

// Severity and category lists are loaded from `data/bugs/taxonomy.json` into
// `GuiState.bug_severities` / `GuiState.bug_categories` at startup.

/// A submitted bug report.
#[derive(Debug, Clone)]
pub struct BugReport {
    pub title: String,
    pub description: String,
    pub severity: String,
    pub category: String,
    pub version: String,
    pub status: &'static str,
}

/// Local state for the bug reporter page.
pub struct BugReporterState {
    pub title: String,
    pub description: String,
    pub severity_idx: usize,
    pub category_idx: usize,
    pub reports: Vec<BugReport>,
    pub status_message: String,
}

impl Default for BugReporterState {
    fn default() -> Self {
        Self {
            title: String::new(),
            description: String::new(),
            severity_idx: 0,
            category_idx: 0,
            reports: vec![
                BugReport {
                    title: "Chat messages flicker on scroll".into(),
                    description: "When scrolling fast, messages flicker briefly.".into(),
                    severity: "Low".into(),
                    category: "UI".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    status: "Open",
                },
                BugReport {
                    title: "Inventory duplication on reconnect".into(),
                    description: "Items appear duplicated after reconnecting.".into(),
                    severity: "High".into(),
                    category: "Network".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    status: "Investigating",
                },
            ],
            status_message: String::new(),
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut BugReporterState) -> R) -> R {
    use std::cell::RefCell;
    thread_local! {
        static STATE: RefCell<BugReporterState> = RefCell::new(BugReporterState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

fn status_color(status: &str, theme: &Theme) -> Color32 {
    match status {
        "Open" => theme.warning(),
        "Investigating" => Theme::c32(&theme.info),
        "Fixed" => theme.success(),
        "Closed" => theme.text_muted(),
        _ => theme.text_secondary(),
    }
}

fn severity_color(severity: &str, theme: &Theme) -> Color32 {
    match severity {
        "Critical" => theme.danger(),
        "High" => Theme::c32(&theme.badge_live),
        "Medium" => theme.warning(),
        "Low" => theme.text_secondary(),
        _ => theme.text_muted(),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Report a Bug")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
            ui.label(
                RichText::new(format!("Version: v{}", env!("CARGO_PKG_VERSION")))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.add_space(theme.spacing_sm);

            ScrollArea::vertical().show(ui, |ui| {
                with_state(|bs| {
                    // Form
                    widgets::card(ui, theme, |ui| {
                        // Title
                        ui.label(RichText::new("Title").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::singleline(&mut bs.title)
                                .desired_width(f32::INFINITY)
                                .hint_text("Brief summary of the bug"),
                        );
                        ui.add_space(theme.spacing_xs);

                        // Description
                        ui.label(RichText::new("Description").color(theme.text_secondary()));
                        ui.add(
                            egui::TextEdit::multiline(&mut bs.description)
                                .desired_width(f32::INFINITY)
                                .desired_rows(4)
                                .hint_text("Steps to reproduce, expected vs actual behavior..."),
                        );
                        ui.add_space(theme.spacing_xs);

                        // Severity + Category dropdowns (lists from data/bugs/taxonomy.json)
                        let severities = &state.bug_severities;
                        let categories = &state.bug_categories;
                        let sev_label = severities.get(bs.severity_idx).map(String::as_str).unwrap_or("");
                        let cat_label = categories.get(bs.category_idx).map(String::as_str).unwrap_or("");
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Severity:").color(theme.text_secondary()));
                            egui::ComboBox::from_id_salt("severity")
                                .selected_text(sev_label)
                                .show_ui(ui, |ui| {
                                    for (i, sev) in severities.iter().enumerate() {
                                        ui.selectable_value(&mut bs.severity_idx, i, sev.as_str());
                                    }
                                });

                            ui.add_space(theme.spacing_md);

                            ui.label(RichText::new("Category:").color(theme.text_secondary()));
                            egui::ComboBox::from_id_salt("category")
                                .selected_text(cat_label)
                                .show_ui(ui, |ui| {
                                    for (i, cat) in categories.iter().enumerate() {
                                        ui.selectable_value(&mut bs.category_idx, i, cat.as_str());
                                    }
                                });
                        });
                        ui.add_space(theme.spacing_sm);

                        // Submit
                        ui.horizontal(|ui| {
                            if widgets::primary_button(ui, theme, "Submit Report") {
                                if !bs.title.trim().is_empty() {
                                    bs.reports.insert(
                                        0,
                                        BugReport {
                                            title: bs.title.trim().to_string(),
                                            description: bs.description.trim().to_string(),
                                            severity: severities.get(bs.severity_idx).cloned().unwrap_or_default(),
                                            category: categories.get(bs.category_idx).cloned().unwrap_or_default(),
                                            version: env!("CARGO_PKG_VERSION").to_string(),
                                            status: "Open",
                                        },
                                    );
                                    bs.title.clear();
                                    bs.description.clear();
                                    bs.severity_idx = 0;
                                    bs.category_idx = 0;
                                    bs.status_message = "Bug report submitted.".into();
                                } else {
                                    bs.status_message = "Title is required.".into();
                                }
                            }
                            if !bs.status_message.is_empty() {
                                ui.label(
                                    RichText::new(&bs.status_message)
                                        .color(theme.text_muted())
                                        .size(theme.font_size_small),
                                );
                            }
                        });
                    });

                    ui.add_space(theme.spacing_md);

                    // Recent reports list
                    ui.label(
                        RichText::new("Recent Reports")
                            .size(theme.font_size_body)
                            .color(theme.text_secondary()),
                    );
                    if bs.reports.is_empty() {
                        ui.label(
                            RichText::new("No reports yet.")
                                .color(theme.text_muted()),
                        );
                    }
                    for report in &bs.reports {
                        widgets::card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&report.title)
                                        .color(theme.text_primary())
                                        .strong(),
                                );
                                // Severity badge
                                widgets::badge(ui, theme, &report.severity, severity_color(&report.severity, theme));
                                // Status badge
                                widgets::badge(ui, theme, report.status, status_color(report.status, theme));
                            });
                            ui.label(
                                RichText::new(format!("{} | v{}", report.category, report.version))
                                    .size(theme.font_size_small)
                                    .color(theme.text_muted()),
                            );
                        });
                        ui.add_space(theme.row_gap);
                    }
                });
            });
        });
}
