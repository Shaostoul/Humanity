//! AI subscription quota tracker + usage log (native, v0.121.0+).
//! Mirrors web/pages/ai-usage.html — interactive form + persistence to
//! `%APPDATA%/HumanityOS/ai_usage.json` (or platform equivalent).

use egui::{RichText, ScrollArea, Vec2};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize};
use crate::gui::GuiState;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiUsageStore {
    pub quotas: Vec<Quota>,
    pub events: Vec<UsageEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quota {
    pub provider: String,
    pub window: String,    // hourly / 5h / daily / weekly / monthly
    pub used: u64,
    pub limit: u64,
    pub resets: String,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageEvent {
    pub t: u64,            // unix ms
    pub provider: String,
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub notes: String,
}

fn ai_usage_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata).join("HumanityOS").join("ai_usage.json");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library").join("Application Support")
                .join("HumanityOS").join("ai_usage.json");
        }
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
            return PathBuf::from(xdg).join("HumanityOS").join("ai_usage.json");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local").join("share")
                .join("HumanityOS").join("ai_usage.json");
        }
    }
    PathBuf::from("ai_usage.json")
}

fn load_store() -> AiUsageStore {
    let path = ai_usage_path();
    if let Ok(text) = std::fs::read_to_string(&path) {
        serde_json::from_str(&text).unwrap_or_default()
    } else {
        AiUsageStore::default()
    }
}

fn save_store(store: &AiUsageStore) {
    let path = ai_usage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(store) {
        let _ = std::fs::write(&path, text);
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

const PROVIDERS: &[&str] = &["claude", "claude-code", "gpt", "gemini", "grok", "other"];
const WINDOWS: &[&str] = &["hourly", "5h", "daily", "weekly", "monthly"];

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let mut store = load_store();
    let mut store_dirty = false;

    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "AI Usage Tracker");
                ui.label(
                    RichText::new(
                        "Manual log of AI subscription usage across providers. Persisted to \
                         your local AppData (single JSON file). Mirrors the web page \
                         /ai-usage which uses browser localStorage.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                // ── Active quota cards ──
                widgets::section_header(ui, theme, "Active quotas");
                if store.quotas.is_empty() {
                    ui.label(
                        RichText::new("No quotas set yet. Add one below.")
                            .color(theme.text_muted())
                            .size(theme.font_size_small),
                    );
                } else {
                    let to_remove: Vec<usize> = store.quotas.iter().enumerate().filter_map(|(i, q)| {
                        let pct = ((q.used as f32 / q.limit as f32) * 100.0).min(100.0);
                        let removed = quota_card(ui, theme, q, pct);
                        if removed { Some(i) } else { None }
                    }).collect();
                    for i in to_remove.into_iter().rev() {
                        store.quotas.remove(i);
                        store_dirty = true;
                    }
                }

                ui.add_space(theme.spacing_md);

                // ── Add quota form ──
                widgets::card_with_header(ui, theme, "Set / update a quota", |ui| {
                    egui::Grid::new("quota_form_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                        ui.label("Provider");
                        provider_combo(ui, "qprov", &mut state.ai_usage_quota_provider);
                        ui.end_row();

                        ui.label("Window");
                        window_combo(ui, "qwin", &mut state.ai_usage_quota_window);
                        ui.end_row();

                        ui.label("Used");
                        ui.add(egui::TextEdit::singleline(&mut state.ai_usage_quota_used).desired_width(140.0));
                        ui.end_row();

                        ui.label("Limit");
                        ui.add(egui::TextEdit::singleline(&mut state.ai_usage_quota_limit).desired_width(140.0));
                        ui.end_row();

                        ui.label("Resets");
                        ui.add(egui::TextEdit::singleline(&mut state.ai_usage_quota_resets).desired_width(140.0).hint_text("2h, 4d, 30m"));
                        ui.end_row();
                    });
                    ui.add_space(theme.spacing_sm);
                    if Button::primary("Save quota").show(ui, theme) {
                        let used: u64 = state.ai_usage_quota_used.parse().unwrap_or(0);
                        let limit: u64 = state.ai_usage_quota_limit.parse().unwrap_or(0);
                        if limit > 0 {
                            // Replace existing (provider, window) pair
                            store.quotas.retain(|q| !(q.provider == state.ai_usage_quota_provider && q.window == state.ai_usage_quota_window));
                            store.quotas.push(Quota {
                                provider: state.ai_usage_quota_provider.clone(),
                                window: state.ai_usage_quota_window.clone(),
                                used,
                                limit,
                                resets: state.ai_usage_quota_resets.clone(),
                                updated_at: now_ms(),
                            });
                            store_dirty = true;
                        }
                    }
                });

                ui.add_space(theme.spacing_md);

                // ── Log event form ──
                widgets::card_with_header(ui, theme, "Log a usage event", |ui| {
                    egui::Grid::new("event_form_grid").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                        ui.label("Provider");
                        provider_combo(ui, "eprov", &mut state.ai_usage_event_provider);
                        ui.end_row();

                        ui.label("Model");
                        ui.add(egui::TextEdit::singleline(&mut state.ai_usage_event_model).desired_width(200.0).hint_text("e.g. opus-4.7"));
                        ui.end_row();

                        ui.label("Input tokens");
                        ui.add(egui::TextEdit::singleline(&mut state.ai_usage_event_input).desired_width(140.0));
                        ui.end_row();

                        ui.label("Output tokens");
                        ui.add(egui::TextEdit::singleline(&mut state.ai_usage_event_output).desired_width(140.0));
                        ui.end_row();

                        ui.label("Notes");
                        ui.add(egui::TextEdit::multiline(&mut state.ai_usage_event_notes).desired_rows(2).desired_width(360.0));
                        ui.end_row();
                    });
                    ui.add_space(theme.spacing_sm);
                    if Button::primary("Log event").show(ui, theme) {
                        let input: u64 = state.ai_usage_event_input.parse().unwrap_or(0);
                        let output: u64 = state.ai_usage_event_output.parse().unwrap_or(0);
                        store.events.push(UsageEvent {
                            t: now_ms(),
                            provider: state.ai_usage_event_provider.clone(),
                            model: state.ai_usage_event_model.clone(),
                            input,
                            output,
                            notes: state.ai_usage_event_notes.clone(),
                        });
                        // Cap at 500 events to keep file small
                        if store.events.len() > 500 {
                            let excess = store.events.len() - 500;
                            store.events.drain(0..excess);
                        }
                        state.ai_usage_event_input.clear();
                        state.ai_usage_event_output.clear();
                        state.ai_usage_event_notes.clear();
                        store_dirty = true;
                    }
                });

                ui.add_space(theme.spacing_md);

                // ── Recent events ──
                widgets::section_header(ui, theme, "Recent events (last 50)");
                if store.events.is_empty() {
                    ui.label(
                        RichText::new("No events logged yet.")
                            .color(theme.text_muted())
                            .size(theme.font_size_small),
                    );
                } else {
                    let recent: Vec<&UsageEvent> = store.events.iter().rev().take(50).collect();
                    egui::Grid::new("events_grid")
                        .striped(true)
                        .num_columns(6)
                        .spacing([12.0, 4.0])
                        .show(ui, |ui| {
                            ui.label(RichText::new("When").color(theme.accent()).strong().size(theme.font_size_small));
                            ui.label(RichText::new("Provider").color(theme.accent()).strong().size(theme.font_size_small));
                            ui.label(RichText::new("Model").color(theme.accent()).strong().size(theme.font_size_small));
                            ui.label(RichText::new("In").color(theme.accent()).strong().size(theme.font_size_small));
                            ui.label(RichText::new("Out").color(theme.accent()).strong().size(theme.font_size_small));
                            ui.label(RichText::new("Notes").color(theme.accent()).strong().size(theme.font_size_small));
                            ui.end_row();
                            for ev in recent {
                                ui.label(RichText::new(fmt_when(ev.t)).color(theme.text_muted()).size(theme.font_size_small));
                                ui.label(RichText::new(&ev.provider).color(theme.text_secondary()).size(theme.font_size_small));
                                ui.label(RichText::new(&ev.model).color(theme.text_secondary()).size(theme.font_size_small));
                                ui.label(RichText::new(format!("{}", ev.input)).color(theme.text_secondary()).size(theme.font_size_small));
                                ui.label(RichText::new(format!("{}", ev.output)).color(theme.text_secondary()).size(theme.font_size_small));
                                ui.label(RichText::new(&ev.notes).color(theme.text_muted()).size(theme.font_size_small));
                                ui.end_row();
                            }
                        });
                }

                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new(format!("Storage: {}", ai_usage_path().display()))
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
                ui.label(
                    RichText::new("Web equivalent: united-humanity.us/ai-usage (separate \u{2014} browser localStorage).")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });

    if store_dirty {
        save_store(&store);
    }
}

fn quota_card(ui: &mut egui::Ui, theme: &Theme, q: &Quota, pct: f32) -> bool {
    let mut remove = false;
    widgets::card(ui, theme, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} \u{00B7} {}", q.provider, q.window))
                    .strong()
                    .color(theme.text_primary()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if Button::ghost("\u{2715}").size(ButtonSize::Small).tooltip("Remove").show(ui, theme) {
                    remove = true;
                }
            });
        });
        let color = if pct >= 90.0 { theme.danger() }
                    else if pct >= 70.0 { theme.warning() }
                    else { theme.accent() };
        ui.label(
            RichText::new(format!("{:.0}%", pct))
                .size(theme.font_size_heading)
                .color(color)
                .strong(),
        );
        // Meter
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().min(280.0), 6.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 3.0, theme.bg_panel());
        let mut filled = rect;
        filled.set_width(rect.width() * (pct / 100.0));
        ui.painter().rect_filled(filled, 3.0, color);
        ui.label(
            RichText::new(format!(
                "{} / {}{}",
                q.used,
                q.limit,
                if q.resets.is_empty() { String::new() } else { format!(" \u{00B7} resets {}", q.resets) },
            ))
            .color(theme.text_muted())
            .size(theme.font_size_small),
        );
    });
    remove
}

fn provider_combo(ui: &mut egui::Ui, id: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for opt in PROVIDERS {
                ui.selectable_value(value, opt.to_string(), *opt);
            }
        });
}

fn window_combo(ui: &mut egui::Ui, id: &str, value: &mut String) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(value.clone())
        .show_ui(ui, |ui| {
            for opt in WINDOWS {
                ui.selectable_value(value, opt.to_string(), *opt);
            }
        });
}

fn fmt_when(ms: u64) -> String {
    // Cheap "X ago" format: relative to now
    let now = now_ms();
    if ms == 0 || ms > now { return "?".into(); }
    let s = (now - ms) / 1000;
    if s < 60 { format!("{}s ago", s) }
    else if s < 3600 { format!("{}m ago", s / 60) }
    else if s < 86400 { format!("{}h ago", s / 3600) }
    else { format!("{}d ago", s / 86400) }
}
