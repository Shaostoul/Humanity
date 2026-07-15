//! Watch page (v0.857) — the in-app stream viewer.
//!
//! The receiving mirror of Studio: Studio publishes, this watches. It lists what is
//! live from `GET /api/live`, and when you pick a stream it opens a `LiveViewer`
//! (net::live_viewer) that decodes the MJPEG frames on a background thread and hands
//! the newest one here to upload as an egui texture and paint. Mirrors the web
//! `/watch` page so a viewer gets the same thing in the native app.

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use egui::{RichText, ScrollArea};

/// How often to re-poll the directory of live streams, in seconds.
const DIRECTORY_REFRESH_SECS: f64 = 5.0;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Drain a finished directory fetch, and kick a new one on the refresh cadence.
    poll_directory(ctx, state);

    // Pull the newest decoded frame (if any) and upload it as a texture. Done every
    // frame so playback is smooth; take_latest returns None when nothing is new, so
    // this is cheap when idle.
    if let Some(viewer) = &state.watch_viewer {
        if let Some(frame) = viewer.take_latest() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [frame.width as usize, frame.height as usize],
                &frame.rgba,
            );
            state.watch_texture =
                Some(ctx.load_texture("watch_frame", image, egui::TextureOptions::LINEAR));
        }
        // Keep repainting while watching so new frames show without mouse movement.
        ctx.request_repaint();
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ScrollArea::vertical().id_salt("watch_scroll").show(ui, |ui| {
            ui.add_space(theme.panel_margin);
            ui.heading("Watch");
            widgets::body_hint(
                ui,
                theme,
                "Live streams broadcast from the Studio page, on this server. Pick one below to \
                 watch it here in the app. Anyone can also watch on the web at /watch.",
            );
            ui.add_space(theme.section_gap);

            if state.watch_viewer.is_some() {
                draw_player(ui, theme, state);
                ui.add_space(theme.section_gap);
            }

            draw_directory(ui, theme, state);
        });
    });
}

/// The video surface + controls for the stream currently being watched.
fn draw_player(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (connected, status, frames, stream_id) = {
        let v = state.watch_viewer.as_ref().unwrap();
        (v.is_connected(), v.status(), v.frames(), v.stream_id().to_string())
    };

    ui.horizontal(|ui| {
        if connected {
            ui.label(RichText::new("LIVE").color(theme.danger()).strong());
        } else if status.is_empty() {
            ui.label(RichText::new("Connecting...").color(theme.text_muted()));
        } else {
            ui.label(RichText::new(status.clone()).color(theme.warning()));
        }
        ui.label(RichText::new(stream_id).color(theme.text_secondary()).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::Button::danger("Stop watching")
                .tooltip("Disconnect from this stream and return to the list.")
                .show(ui, theme)
            {
                state.watch_viewer = None;
                state.watch_texture = None;
            }
        });
    });
    ui.add_space(theme.panel_margin);

    // The video surface: a 16:9 box that shows the current frame, or a placeholder
    // until the first one lands.
    let avail_w = ui.available_width().min(960.0);
    let box_h = avail_w * 9.0 / 16.0;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(avail_w, box_h), egui::Sense::hover());
    ui.painter().rect_filled(rect, theme.border_radius_lg, egui::Color32::BLACK);

    if let Some(tex) = &state.watch_texture {
        // Fit the frame inside the box, preserving aspect ratio (letterbox).
        let ts = tex.size_vec2();
        let scale = (rect.width() / ts.x).min(rect.height() / ts.y);
        let draw = egui::vec2(ts.x * scale, ts.y * scale);
        let img_rect = egui::Rect::from_center_size(rect.center(), draw);
        ui.painter().image(
            tex.id(),
            img_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            if status.is_empty() { "Waiting for video..." } else { &status },
            egui::FontId::proportional(theme.font_size_body),
            theme.text_muted(),
        );
    }

    ui.add_space(theme.panel_margin);
    ui.label(
        RichText::new(format!("{frames} frames received"))
            .size(theme.font_size_small)
            .color(theme.text_muted()),
    );
}

/// The directory of live streams, plus a manual "watch by name" entry.
fn draw_directory(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(RichText::new("Live now").size(theme.font_size_heading).strong());
    ui.add_space(theme.panel_margin);

    if state.watch_streams.is_empty() {
        ui.label(
            RichText::new("Nobody is streaming right now.")
                .color(theme.text_muted()),
        );
    } else {
        // Clone the list so we are not borrowing state while we may mutate it.
        let streams = state.watch_streams.clone();
        for (id, title, viewers) in streams {
            let currently = state
                .watch_viewer
                .as_ref()
                .map(|v| v.stream_id() == id)
                .unwrap_or(false);
            ui.horizontal(|ui| {
                let label = if title.is_empty() { id.clone() } else { title.clone() };
                ui.label(RichText::new(label).strong());
                ui.label(
                    RichText::new(format!("{viewers} watching"))
                        .size(theme.font_size_small)
                        .color(theme.text_muted()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let btn = if currently {
                        widgets::Button::secondary("Watching").disabled(true)
                    } else {
                        widgets::Button::primary("Watch")
                    };
                    if btn.show(ui, theme) && !currently {
                        start_watching(state, &id);
                    }
                });
            });
            ui.add_space(4.0);
        }
    }

    ui.add_space(theme.section_gap);
    ui.separator();
    ui.add_space(theme.panel_margin);

    // Watch by name: a stream is published under the operator's registered name, so
    // you can watch a known streamer even before the directory refreshes.
    ui.label(RichText::new("Watch by name").size(theme.font_size_small).color(theme.text_secondary()));
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut state.watch_input)
                .desired_width(200.0)
                .hint_text("stream name"),
        );
        let name = state.watch_input.trim().to_lowercase();
        if widgets::Button::secondary("Watch")
            .disabled(name.is_empty())
            .show(ui, theme)
            && !name.is_empty()
        {
            start_watching(state, &name);
        }
    });
}

/// Open a viewer for `stream_id`, replacing any current one.
fn start_watching(state: &mut GuiState, stream_id: &str) {
    let server = state.server_url.trim_end_matches('/').to_string();
    state.watch_texture = None;
    state.watch_viewer = Some(crate::net::live_viewer::LiveViewer::start(&server, stream_id));
}

/// Drain a finished directory fetch and start a new one on the refresh cadence.
fn poll_directory(ctx: &egui::Context, state: &mut GuiState) {
    if let Some(rx) = &state.watch_streams_rx {
        if let Ok(list) = rx.try_recv() {
            state.watch_streams = list;
            state.watch_streams_rx = None;
        }
    }
    let now = ctx.input(|i| i.time);
    if state.watch_streams_rx.is_none() && now - state.watch_last_fetch > DIRECTORY_REFRESH_SECS {
        state.watch_last_fetch = now;
        let base = state.server_url.trim_end_matches('/').to_string();
        let (tx, rx) = std::sync::mpsc::channel();
        state.watch_streams_rx = Some(rx);
        std::thread::spawn(move || {
            let list = fetch_directory(&base).unwrap_or_default();
            let _ = tx.send(list);
        });
    }
}

/// Fetch and parse `GET /api/live` into (id, title, viewers) rows.
fn fetch_directory(base: &str) -> Option<Vec<(String, String, u64)>> {
    let body = ureq::get(&format!("{base}/api/live"))
        .timeout(std::time::Duration::from_secs(4))
        .call()
        .ok()?
        .into_string()
        .ok()?;
    let v: serde_json::Value = serde_json::from_str(&body).ok()?;
    let arr = v.get("streams")?.as_array()?;
    Some(
        arr.iter()
            .map(|s| {
                (
                    s.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    s.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                    s.get("viewers").and_then(|x| x.as_u64()).unwrap_or(0),
                )
            })
            .filter(|(id, _, _)| !id.is_empty())
            .collect(),
    )
}
