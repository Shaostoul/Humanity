//! Diagnostics dev-HUD overlays (v0.482). A small developer heads-up display,
//! toggled by F-keys and stacked in the top-right corner:
//!   F2 = Performance (FPS, frame time, a frame-time sparkline, entity count)
//!   F3 = Network     (connection, server, members online, messages received)
//!   F4 = System      (process RAM, uptime, build version)
//! All read from GuiState fields sampled in lib.rs (see show_*_overlay). The F1
//! keymap lists the toggles so they are discoverable. Display-only.
//!
//! PAINTED, taking no click (2026-11-15): the readouts are drawn with the tooltips (gui/layers.rs
//! layer 1), over every popup, and as the egui Area they were they took the click of whatever
//! stood under them (the BUG-316 class: egui gives every Area a widget rect, `interactable(false)`
//! or not). They are a card of words painted with `widgets::painted_card`, the same lines in the
//! same corner, and `tests/layer_lint.rs` refuses an Area on the tooltip layer. Dev tooling is
//! permanent (CLAUDE.md, forever development): made click-through, never removed.

use egui::{Color32, Context, FontId, Painter, Rect, Vec2};
use humanity_core::i18n::{t, tf};

use crate::gui::layers::Layer;
use crate::gui::theme::Theme;
use crate::gui::widgets::painted_card::{PaintedCard, Place, Words};
use crate::gui::GuiState;

/// The id the readouts are painted under (`painted_card::rect` finds them).
pub const ID: &str = "diagnostics_overlay";

/// Draw whichever overlays are toggled on, stacked in the top-right corner.
pub fn draw(ctx: &Context, theme: &Theme, state: &GuiState) {
    if !state.show_perf_overlay && !state.show_network_overlay && !state.show_system_overlay {
        return;
    }
    let frame = egui::Frame::popup(&ctx.style()).fill(theme.bg_panel()).inner_margin(10.0);
    let mut card = PaintedCard::new(frame).width(168.0);
    if state.show_perf_overlay {
        card = perf(card, theme, state);
    }
    if state.show_network_overlay {
        if state.show_perf_overlay {
            card = card.space(theme.spacing_sm);
        }
        card = network(card, theme, state);
    }
    if state.show_system_overlay {
        if state.show_perf_overlay || state.show_network_overlay {
            card = card.space(theme.spacing_sm);
        }
        card = system(card, theme, state);
    }
    card.paint(ctx, egui::Id::new(ID), Layer::Tooltip, Place::Anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 44.0)));
}

/// Small words in `color`.
fn small(theme: &Theme, text: impl Into<String>, color: Color32) -> Words {
    Words::new(text, FontId::proportional(theme.font_size_small), color)
}

/// A readout's heading on the left, its F-key on the right.
fn header<'a>(card: PaintedCard<'a>, theme: &Theme, text: &str, key: &str) -> PaintedCard<'a> {
    card.pair(small(theme, text, theme.accent()), small(theme, key, theme.text_muted()))
}

/// A label on the left, its value on the right.
fn row<'a>(card: PaintedCard<'a>, theme: &Theme, label: &str, value: String) -> PaintedCard<'a> {
    card.pair(small(theme, label, theme.text_secondary()), small(theme, value, theme.text_primary()))
}

fn perf<'a>(card: PaintedCard<'a>, theme: &'a Theme, state: &'a GuiState) -> PaintedCard<'a> {
    let fps = state.fps;
    let frame_ms = if fps > 0.0 { 1000.0 / fps } else { 0.0 };
    let mut card = header(card, theme, t("diagnostics.performance"), "F2");
    card = row(card, theme, t("diagnostics.fps"), format!("{fps:.0}"));
    card = row(card, theme, t("diagnostics.frame"), format!("{frame_ms:.1} ms"));
    card = card.block(Vec2::new(148.0, 34.0), move |p, rect| paint_frame_graph(p, rect, theme, &state.frame_times));
    card = row(card, theme, t("diagnostics.entities"), state.diag_entity_count.to_string());
    // Live GPU light count (v0.782): lights are uncapped now, so this + FPS is
    // how you find your GPU's real ceiling empirically.
    card = row(card, theme, t("diagnostics.lights"), state.diag_light_count.to_string());
    // Gravity actually applied this frame (v0.1120, artificial-planet arc):
    // makes gravity_curve tuning visible live while walking a world. Only
    // shown in a surface band; the ship interior and free flight have their
    // own rules.
    if let Some(g) = state.surface_gravity_now {
        card = row(card, theme, t("diagnostics.gravity"), format!("{g:.2} m/s\u{00b2} ({:.2} g)", g / 9.81));
    }
    card
}

/// A small frame-time sparkline. Bars scale to a fixed 33 ms ceiling (30 FPS);
/// a bar at the dashed line means a 30 FPS frame, lower is better. Bars are
/// tinted by the success/warning tokens at the 60/30 FPS thresholds.
fn paint_frame_graph(painter: &Painter, rect: Rect, theme: &Theme, times: &[f32]) {
    painter.rect_filled(rect, 2.0, theme.bg_card());
    let ceil_ms = 33.3_f32; // 30 FPS reference
    // 60 FPS reference line (16.7 ms) so you can eyeball headroom.
    let y60 = rect.bottom() - (16.7 / ceil_ms).clamp(0.0, 1.0) * rect.height();
    painter.hline(
        rect.left()..=rect.right(),
        y60,
        egui::Stroke::new(1.0_f32, theme.text_muted()),
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

fn network<'a>(card: PaintedCard<'a>, theme: &Theme, state: &GuiState) -> PaintedCard<'a> {
    let mut card = header(card, theme, t("diagnostics.network"), "F3");
    let connected = state.ws_client.as_ref().map_or(false, |c| c.is_connected());
    let (txt, col) = if connected {
        (t("diagnostics.connected"), theme.success())
    } else {
        (t("diagnostics.disconnected"), theme.danger())
    };
    card = card.pair(small(theme, t("diagnostics.status"), theme.text_secondary()), small(theme, txt, col));
    // Server host only (the full URL is long); strip the scheme for brevity.
    let host = state
        .server_url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_end_matches('/');
    let host_short = if host.len() > 22 { format!("{}...", humanity_core::text::prefix(&host, 22)) } else { host.to_string() };
    card = row(card, theme, t("diagnostics.server"), if host_short.is_empty() { t("diagnostics.no_server").to_string() } else { host_short });
    card = row(card, theme, t("diagnostics.members"), state.chat_users.len().to_string());
    card = row(card, theme, t("diagnostics.msgs_in"), state.ws_msgs_in.to_string());
    // Background connections (multi-connection stage 3+): one row per
    // parked/background server so the overlay shows the WHOLE mesh.
    for conn in &state.connections {
        let short = {
            let h = conn
                .url
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            if h.len() > 16 { format!("{}...", humanity_core::text::prefix(&h, 16)) } else { h.to_string() }
        };
        let link = if conn.identified {
            t("diagnostics.link_up")
        } else if conn.ws.is_some() {
            t("diagnostics.link_connecting")
        } else {
            t("diagnostics.link_down")
        };
        card = row(
            card,
            theme,
            &tf("diagnostics.background", &[("host", &short)]),
            tf("diagnostics.link_line", &[("link", &link), ("n", &conn.msgs_in)]),
        );
    }
    card
}

fn system<'a>(card: PaintedCard<'a>, theme: &Theme, state: &GuiState) -> PaintedCard<'a> {
    let mut card = header(card, theme, t("diagnostics.system"), "F4");
    card = row(card, theme, t("diagnostics.ram"), format!("{:.0} MB", state.diag_mem_mb));
    card = row(card, theme, t("diagnostics.uptime"), fmt_uptime(state.diag_uptime_secs));
    row(card, theme, t("diagnostics.version"), format!("v{}", env!("CARGO_PKG_VERSION")))
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
