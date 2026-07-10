//! Diagnostics dev-HUD overlays (v0.482). A small developer heads-up display,
//! toggled by F-keys and stacked in the top-right corner:
//!   F2 = Performance (FPS, frame time, a frame-time sparkline, entity count)
//!   F3 = Network     (connection, server, members online, messages received)
//!   F4 = System      (process RAM, uptime, build version)
//! All read from GuiState fields sampled in lib.rs (see show_*_overlay). The F1
//! keymap lists the toggles so they are discoverable. Display-only.

use egui::{Context, RichText, Sense, Vec2};

use crate::gui::theme::Theme;
use crate::gui::GuiState;

/// Draw whichever overlays are toggled on, stacked in the top-right corner.
pub fn draw(ctx: &Context, theme: &Theme, state: &GuiState) {
    if !state.show_perf_overlay && !state.show_network_overlay && !state.show_system_overlay {
        return;
    }
    egui::Area::new(egui::Id::new("diagnostics_overlay"))
        .interactable(false)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 44.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme.bg_panel())
                .inner_margin(10.0)
                .show(ui, |ui| {
                    ui.set_width(168.0);
                    if state.show_perf_overlay {
                        draw_perf(ui, theme, state);
                    }
                    if state.show_network_overlay {
                        if state.show_perf_overlay {
                            ui.add_space(theme.spacing_sm);
                        }
                        draw_network(ui, theme, state);
                    }
                    if state.show_system_overlay {
                        if state.show_perf_overlay || state.show_network_overlay {
                            ui.add_space(theme.spacing_sm);
                        }
                        draw_system(ui, theme, state);
                    }
                });
        });
}

fn header(ui: &mut egui::Ui, theme: &Theme, text: &str, key: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(text).strong().size(theme.font_size_small).color(theme.accent()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(key).size(theme.font_size_small).color(theme.text_muted()));
        });
    });
}

fn row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: String) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(theme.font_size_small).strong().color(theme.text_primary()));
        });
    });
}

fn draw_perf(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    header(ui, theme, "Performance", "F2");
    let fps = state.fps;
    let frame_ms = if fps > 0.0 { 1000.0 / fps } else { 0.0 };
    row(ui, theme, "FPS", format!("{fps:.0}"));
    row(ui, theme, "Frame", format!("{frame_ms:.1} ms"));
    draw_frame_graph(ui, theme, &state.frame_times);
    row(ui, theme, "Entities", state.diag_entity_count.to_string());
    // Live GPU light count (v0.782): lights are uncapped now, so this + FPS is
    // how you find your GPU's real ceiling empirically.
    row(ui, theme, "Lights", state.diag_light_count.to_string());
}

/// A small frame-time sparkline. Bars scale to a fixed 33 ms ceiling (30 FPS);
/// a bar at the dashed line means a 30 FPS frame, lower is better. Bars are
/// tinted by the success/warning tokens at the 60/30 FPS thresholds.
fn draw_frame_graph(ui: &mut egui::Ui, theme: &Theme, times: &[f32]) {
    let (rect, _resp) = ui.allocate_exact_size(Vec2::new(148.0, 34.0), Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, theme.bg_card());
    let ceil_ms = 33.3_f32; // 30 FPS reference
    // 60 FPS reference line (16.7 ms) so you can eyeball headroom.
    let y60 = rect.bottom() - (16.7 / ceil_ms).clamp(0.0, 1.0) * rect.height();
    painter.hline(
        rect.left()..=rect.right(),
        y60,
        egui::Stroke::new(1.0, theme.text_muted()),
    );
    if times.is_empty() {
        return;
    }
    let n = times.len().min(120);
    let start = times.len().saturating_sub(n);
    let bw = rect.width() / n as f32;
    for (i, ms) in times[start..].iter().enumerate() {
        let h = (ms / ceil_ms).clamp(0.0, 1.0) * rect.height();
        let x = rect.left() + i as f32 * bw;
        let bar = egui::Rect::from_min_max(
            egui::pos2(x, rect.bottom() - h),
            egui::pos2(x + bw.max(1.0), rect.bottom()),
        );
        // Green under 16.7 ms (60 FPS), warning up to 33 ms, danger above.
        let col = if *ms <= 16.7 {
            theme.success()
        } else if *ms <= 33.3 {
            theme.warning()
        } else {
            theme.danger()
        };
        painter.rect_filled(bar, 0.0, col);
    }
}

fn draw_network(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    header(ui, theme, "Network", "F3");
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    ui.horizontal(|ui| {
        ui.label(RichText::new("Status").size(theme.font_size_small).color(theme.text_secondary()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let (txt, col) = if connected {
                ("Connected", theme.success())
            } else {
                ("Disconnected", theme.danger())
            };
            ui.label(RichText::new(txt).size(theme.font_size_small).strong().color(col));
        });
    });
    // Server host only (the full URL is long); strip the scheme for brevity.
    let host = state
        .server_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_end_matches('/');
    let host_short = if host.len() > 22 { format!("{}...", &host[..22]) } else { host.to_string() };
    row(ui, theme, "Server", if host_short.is_empty() { "(none)".to_string() } else { host_short });
    row(ui, theme, "Members", state.chat_users.len().to_string());
    row(ui, theme, "Msgs in", state.ws_msgs_in.to_string());
}

fn draw_system(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    header(ui, theme, "System", "F4");
    row(ui, theme, "RAM", format!("{:.0} MB", state.diag_mem_mb));
    row(ui, theme, "Uptime", fmt_uptime(state.diag_uptime_secs));
    row(ui, theme, "Version", format!("v{}", env!("CARGO_PKG_VERSION")));
}

/// Seconds -> "1h 02m", "5m 03s", or "12s".
fn fmt_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}
