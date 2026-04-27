//! Multi-AI agent coordination dashboard (native).
//! Mirrors web/pages/agents.html.
//!
//! Reads data/coordination/agent_registry.ron + sessions/*.json directly off
//! disk so the page works even when the relay isn't running. The web version
//! goes through the API for live runtime claims; the native version just
//! shows the static + audit state.

use egui::{RichText, ScrollArea};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, ButtonSize};
use crate::gui::GuiState;

pub fn draw(ctx: &egui::Context, theme: &Theme, _state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(widgets::page_frame(theme))
        .show(ctx, |ui| {
            ScrollArea::vertical().show(ui, |ui| {
                widgets::section_header(ui, theme, "Agent Dashboard");
                ui.label(
                    RichText::new(
                        "Live spreadsheet of every multi-AI scope, status, recommendation, \
                         runtime claim, and user override. Backed by data/coordination/.",
                    )
                    .color(theme.text_muted())
                    .size(theme.font_size_small),
                );
                ui.add_space(theme.spacing_md);

                widgets::card_with_header(ui, theme, "Two views available", |ui| {
                    ui.label(
                        RichText::new(
                            "1. Web: full interactive dashboard with override dropdowns at \
                             united-humanity.us/agents (rendered with live API data).\n\n\
                             2. Local files: read data/coordination/agent_registry.ron + \
                             sessions/*.json directly. Run `node scripts/agent-status.js` from \
                             the repo root for a markdown summary.",
                        )
                        .color(theme.text_secondary())
                        .size(theme.font_size_small),
                    );
                });

                ui.add_space(theme.spacing_sm);

                widgets::card_with_header(ui, theme, "Quick scope summary", |ui| {
                    let scopes = read_registry_scope_ids();
                    if scopes.is_empty() {
                        ui.label(
                            RichText::new("Could not read data/coordination/agent_registry.ron")
                                .color(theme.text_muted())
                                .size(theme.font_size_small),
                        );
                    } else {
                        ui.label(
                            RichText::new(format!("{} declared scopes:", scopes.len()))
                                .color(theme.text_primary())
                                .size(theme.font_size_small),
                        );
                        for scope_id in scopes {
                            let audit_state = read_session_status(&scope_id);
                            let line = match audit_state.as_deref() {
                                Some(s) => format!("  \u{2022} {}  \u{2014}  {}", scope_id, s),
                                None => format!("  \u{2022} {}  \u{2014}  unaudited", scope_id),
                            };
                            ui.label(
                                RichText::new(line)
                                    .color(theme.text_muted())
                                    .size(theme.font_size_small),
                            );
                        }
                    }
                });

                ui.add_space(theme.spacing_md);

                widgets::card_with_header(ui, theme, "Lifecycle protocol", |ui| {
                    let steps = [
                        "1. New AI session reads data/coordination/orchestrator_state.json first.",
                        "2. Runs `node scripts/agent-status.js` for current per-scope status.",
                        "3. Picks an unclaimed active scope from agent_registry.ron.",
                        "4. Calls agent_claim_scope() to record the claim in agent_sessions table.",
                        "5. Heartbeats every 5\u{2013}10 minutes via agent_heartbeat().",
                        "6. On exit, calls agent_release_scope() with state = paused / completed / blocked.",
                        "7. Agent overrides via the dashboard fire announcements on the #announcements channel.",
                    ];
                    for s in steps {
                        ui.label(RichText::new(s).color(theme.text_secondary()).size(theme.font_size_small));
                    }
                });

                ui.add_space(theme.spacing_md);
                ui.label(
                    RichText::new("Same view on the web: united-humanity.us/agents")
                        .color(theme.text_muted())
                        .size(theme.font_size_small),
                );
            });
        });
}

fn registry_path() -> std::path::PathBuf {
    std::path::PathBuf::from("data/coordination/agent_registry.ron")
}

fn sessions_dir() -> std::path::PathBuf {
    std::path::PathBuf::from("data/coordination/sessions")
}

fn read_registry_scope_ids() -> Vec<String> {
    let text = std::fs::read_to_string(registry_path()).unwrap_or_default();
    let mut ids = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("id:") {
            if let Some(start) = rest.find('"') {
                let after = &rest[start + 1..];
                if let Some(end) = after.find('"') {
                    ids.push(after[..end].to_string());
                }
            }
        }
    }
    ids
}

fn read_session_status(scope_id: &str) -> Option<String> {
    let mut path = sessions_dir();
    path.push(format!("{}.json", scope_id));
    let text = std::fs::read_to_string(path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;
    let status = json.get("implementation_status").and_then(|v| v.as_str())?;
    let rec = json
        .get("recommended_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    Some(format!("{} ({})", status, rec))
}
