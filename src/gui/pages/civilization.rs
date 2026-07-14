//! Civilization dashboard - the community's REAL numbers from the connected
//! relay (GET /api/civilization, v0.757 closure ladder rung 10). Members,
//! messages, market, tasks, follows: every stat is what the server counts,
//! with the week's growth as the trend. Replaced the fabricated static
//! metrics (tech level, food/water/happiness percentages, hardcoded "+12"
//! trends) that no system ever fed - those return as REAL sims wire in.
//!
//! v0.197.0 note kept: this page commits to the Real (community) framing;
//! a colony/sim variant would be a separate in-world surface.

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiCivStats, GuiState};
use egui::{Frame, RichText, ScrollArea, Vec2};

/// Background fetch of the aggregated stats (public endpoint; same
/// worker-thread + mpsc pattern as the Guilds page).
fn spawn_civ_fetch(state: &mut GuiState) {
    let base = state.server_url.trim_end_matches('/').to_string();
    let (tx, rx) = std::sync::mpsc::channel();
    state.civ_stats_rx = Some(rx);
    state.civ_stats_loaded = true;
    std::thread::spawn(move || {
        let fetch = || -> Result<GuiCivStats, String> {
            let body = ureq::get(&format!("{base}/api/civilization"))
                .call()
                .map_err(|e| format!("stats: {e}"))?
                .into_string()
                .map_err(|e| format!("read: {e}"))?;
            let val: serde_json::Value =
                serde_json::from_str(&body).map_err(|e| format!("parse: {e}"))?;
            Ok(GuiCivStats::from_relay_json(&val))
        };
        let _ = tx.send(fetch());
    });
}

/// Drain a finished fetch and kick a new one off when none is in flight.
///
/// pub(crate) because the Humanity tab's Mission Dashboard embeds this same
/// live view (v0.851 rescue: the page was orphaned, only its `draw_stat_card`
/// helper was being reused). Both surfaces share ONE fetch, ONE cache, and one
/// set of `GuiState.civ_*` fields, so opening either keeps the other current
/// and neither double-fetches.
pub(crate) fn poll_stats(ctx: &egui::Context, state: &mut GuiState) {
    if let Some(rx) = &state.civ_stats_rx {
        match rx.try_recv() {
            Ok(Ok(stats)) => {
                state.civ_stats = Some(stats);
                state.civ_status.clear();
                state.civ_stats_rx = None;
            }
            Ok(Err(e)) => {
                state.civ_status = e;
                state.civ_stats_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                ctx.request_repaint_after(std::time::Duration::from_millis(300));
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.civ_stats_rx = None;
            }
        }
    }
    if !state.civ_stats_loaded && state.civ_stats_rx.is_none() {
        spawn_civ_fetch(state);
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    poll_stats(ctx, state);

    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("Community Dashboard")
                        .size(theme.font_size_title)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Refresh") {
                        state.civ_stats_loaded = false;
                    }
                });
            });
            ui.add_space(theme.spacing_md);

            ScrollArea::vertical().show(ui, |ui| {
                draw_live_body(ui, theme, state);
            });
        });
}

/// The live relay dashboard itself: fetch status, the stat grid, the activity
/// summary. No panel and no ScrollArea of its own, so a host page can drop it
/// anywhere (the Civilization page wraps it in a CentralPanel + ScrollArea; the
/// Humanity tab's Mission Dashboard embeds it inside its existing scroll).
///
/// The caller MUST have run `poll_stats` this frame, otherwise nothing ever
/// fetches and this only ever shows the loading hint.
pub(crate) fn draw_live_body(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if !state.civ_status.is_empty() {
        ui.label(
            RichText::new(&state.civ_status)
                .color(theme.text_secondary())
                .size(theme.font_size_small),
        );
    }

    let Some(s) = state.civ_stats.clone() else {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.spacing_xl);
            let hint = if state.civ_stats_rx.is_some() {
                "Loading community stats from the server..."
            } else {
                "Could not reach the server - Refresh to retry."
            };
            ui.label(
                RichText::new(hint)
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
        });
        return;
    };

    // Real stats, real trends: the week's growth is the trend.
    let task_progress = if s.total_tasks > 0 {
        s.tasks_completed as f32 / s.total_tasks as f32
    } else {
        0.0
    };
    let new_week = if s.new_this_week > 0 {
        format!("+{} this week", s.new_this_week)
    } else {
        String::new()
    };
    let msgs_today = if s.messages_today > 0 {
        format!("+{} today", s.messages_today)
    } else {
        String::new()
    };
    let stats: Vec<(&str, String, String, f32)> = vec![
        ("Members", s.total_members.to_string(), new_week, 0.0),
        ("Online Now", s.online_now.to_string(), String::new(), 0.0),
        ("Messages", s.total_messages.to_string(), msgs_today, 0.0),
        ("Channels", format!("{} (+{} voice)", s.channels, s.voice_channels), String::new(), 0.0),
        ("Projects", s.projects.to_string(), String::new(), 0.0),
        ("Market Listings", s.active_listings.to_string(), String::new(), 0.0),
        ("Trades Completed", s.total_trades.to_string(), String::new(), 0.0),
        (
            "Tasks Done",
            format!("{} of {}", s.tasks_completed, s.total_tasks),
            String::new(),
            task_progress,
        ),
        ("Follows", s.total_follows.to_string(), String::new(), 0.0),
    ];

    egui::Grid::new("civ_stats_grid_3col")
        .num_columns(3)
        .spacing(Vec2::new(theme.spacing_sm, theme.spacing_sm))
        .show(ui, |ui| {
            for (i, (label, value, trend, progress)) in stats.iter().enumerate() {
                draw_stat_card(ui, theme, label, value, trend, *progress);
                if (i + 1) % 3 == 0 {
                    ui.end_row();
                }
            }
        });

    ui.add_space(theme.spacing_md);

    // Activity summary.
    widgets::card(ui, theme, |ui| {
        ui.label(
            RichText::new("Activity")
                .size(theme.font_size_heading)
                .color(theme.text_primary()),
        );
        ui.add_space(theme.spacing_xs);
        let rows: Vec<(&str, String)> = vec![
            (
                "Most active channel",
                if s.most_active_channel.is_empty() {
                    "-".to_string()
                } else {
                    format!("#{}", s.most_active_channel)
                },
            ),
            ("Peak online", s.peak_online.to_string()),
            ("Tasks in progress", s.tasks_in_progress.to_string()),
            ("Tasks open", s.tasks_open.to_string()),
            ("Listing reviews", s.total_reviews.to_string()),
            ("Direct message threads", s.total_dms.to_string()),
        ];
        for (label, value) in rows {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(label)
                        .size(theme.font_size_body)
                        .color(theme.text_secondary()),
                );
                ui.label(
                    RichText::new(value)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
            });
        }
    });
}

/// Draw a stat card with large number, trend arrow, and optional progress bar.
/// pub(crate): the Humanity Mission Dashboard reuses it for its scoreboard
/// (v0.662) so the two pages' stat tiles stay one visual language.
pub(crate) fn draw_stat_card(ui: &mut egui::Ui, theme: &Theme, label: &str, value: &str, trend: &str, progress: f32) {
    widgets::card(ui, theme, |ui| {
        ui.set_min_width(180.0);
        // Explicit vertical layout (v0.684): the card INHERITED the parent's
        // layout direction, so inside a left-to-right container (a Grid cell,
        // a horizontal row) the label and value landed side by side at drifting
        // heights -- the stair-stepped scoreboard the operator screenshotted
        // 2026-07-04. A widget must own its internal layout.
        ui.vertical(|ui| {
        ui.label(
            RichText::new(label)
                .size(theme.font_size_small)
                .color(theme.text_muted()),
        );
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(value)
                    .size(theme.font_size_title)
                    .color(theme.accent()),
            );
            if !trend.is_empty() {
                let (arrow, color) = if trend.starts_with('+') {
                    ("^", theme.success())
                } else if trend.starts_with('-') {
                    ("v", theme.danger())
                } else {
                    ("-", theme.text_muted())
                };
                ui.label(
                    RichText::new(format!("{} {}", arrow, trend))
                        .size(theme.font_size_small)
                        .color(color),
                );
            }
        });
        if progress > 0.0 {
            widgets::progress_bar(ui, theme, progress, None);
        }
        }); // end ui.vertical
    });
}
