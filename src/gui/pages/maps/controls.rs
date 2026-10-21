//! The Maps camera's controls (Maps increment 1): the mouse and the Home key,
//! the follow row over the canvas (Back to me, Stop following, Open where I
//! left it), and the per-frame advance of a flight and of the page's own
//! clock when it is off Live.
//!
//! Order matters, and it is why these run at the END of the System view's
//! draw: the engine renders the world layer before the page draws
//! (`engine/map_view.rs`), from the camera as the previous frame left it.
//! The page draws its symbols from that same camera, then applies this
//! frame's input, so the symbols and the world under them always come from
//! one pose and input shows up in both on the next frame.

use egui::{Pos2, Rect, RichText, Vec2};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

use super::camera::{MapAnchor, MapScene};
use super::time_bar::current_real_time_seconds_since_j2000;

/// Radians of turn per point of drag.
const TURN_PER_PT: f64 = 0.005;
/// Zoom per point of scroll: about 1.08x for a 50-point wheel tick.
const ZOOM_PER_PT: f64 = 1.0015;

/// Mouse and keys: drag turns around the anchor (keeps following), shift or
/// middle drag pans (ends following), scroll zooms toward the anchor, Home
/// flies back to you. Any of them takes hold of a flight in progress.
pub(super) fn handle_input(ui: &egui::Ui, response: &egui::Response, rect: Rect, state: &mut GuiState) {
    let me = state.maps.me.clone();
    let scene = MapScene::new(me.as_ref(), state.maps.sim_time_seconds, state.maps.live);
    let cam = &mut state.maps.camera;
    if response.dragged() {
        let d = response.drag_delta();
        let pan = ui.input(|i| i.modifiers.shift) || response.dragged_by(egui::PointerButton::Middle);
        if pan {
            cam.pan_by(d.x as f64, d.y as f64, rect.height() as f64, &scene);
        } else {
            cam.rotate_by(d.x as f64 * TURN_PER_PT, -(d.y as f64) * TURN_PER_PT, &scene);
        }
    }
    let scroll = ui.input(|i| i.smooth_scroll_delta.y);
    if scroll != 0.0 && ui.rect_contains_pointer(rect) {
        cam.zoom_by(ZOOM_PER_PT.powf(-scroll as f64), &scene);
    }
    if ui.input(|i| i.key_pressed(egui::Key::Home)) {
        cam.back_to_me(&scene);
    }
}

/// The follow row in the canvas's top-left corner.
pub(super) fn draw_follow_row(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, rect: Rect) {
    let row = Rect::from_min_size(rect.min + Vec2::new(8.0, 8.0), Vec2::new((rect.width() - 300.0).max(200.0), 24.0));
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(row).layout(egui::Layout::left_to_right(egui::Align::Center)));
    let me = state.maps.me.clone();
    let scene = MapScene::new(me.as_ref(), state.maps.sim_time_seconds, state.maps.live);
    if me.is_some() {
        if state.maps.camera.follows_me() {
            ui.label(RichText::new("Following you").size(theme.font_size_small).color(theme.accent()).strong());
            if widgets::Button::secondary("Stop following")
                .tooltip("Leave the camera where it is while your ship moves on.")
                .show(&mut ui, theme)
            {
                state.maps.camera.unfollow(&scene);
            }
        } else if widgets::Button::primary("Back to me")
            .tooltip("Fly back to you and follow you again (the Home key does this too).")
            .show(&mut ui, theme)
        {
            state.maps.camera.back_to_me(&scene);
        }
    }
    if widgets::toggle(&mut ui, theme, "Open where I left it", &mut state.settings.maps_open_where_left) {
        state.settings_dirty = true;
    }
}

/// Advance a flight and, off Live, the page's own clock, by the time since
/// the last draw. While Live the feed sets the clock from the engine's.
pub(super) fn advance(state: &mut GuiState) {
    let now = std::time::Instant::now();
    let dt = state.maps.last_real_instant.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(0.0);
    state.maps.last_real_instant = Some(now);
    state.maps.camera.tick(dt);
    if state.maps.turn_dps != 0.0 {
        // The rig's dev turn (`MapVerb::turn_dps`): a steady drag round the anchor.
        let me = state.maps.me.clone();
        let scene = MapScene::new(me.as_ref(), state.maps.sim_time_seconds, state.maps.live);
        state.maps.camera.rotate_by(state.maps.turn_dps.to_radians() * dt, 0.0, &scene);
    }
    if state.maps.live {
        if state.maps.me.is_none() {
            state.maps.sim_time_seconds = current_real_time_seconds_since_j2000();
        }
    } else {
        state.maps.sim_time_seconds += dt * state.maps.sim_speed;
    }
    // A tracked body stays tracked only while the camera is on it, or on its
    // way to it (the card's Track asks for the flight next frame).
    if let Some(id) = state.maps.tracked_body.clone() {
        let pending = state.maps.focus_request.as_deref() == Some(id.as_str());
        if !pending && state.maps.camera.fly.is_none() && state.maps.camera.anchor != MapAnchor::Body(id) {
            state.maps.tracked_body = None;
        }
    }
    state.maps.drawn_seq = state.maps.frame_seq;
}

/// Record where the canvas is on the window, for the engine's world layer
/// (its size) and the rig's report (where its centre is).
pub(super) fn note_canvas(ui: &egui::Ui, rect: Rect, state: &mut GuiState) {
    let ppp = ui.ctx().pixels_per_point();
    state.maps.pixels_per_point = ppp;
    state.maps.canvas_points = Some([rect.width(), rect.height()]);
    let px = |p: Pos2| [p.x * ppp, p.y * ppp];
    let (a, b) = (px(rect.min), px(rect.max));
    state.maps.canvas_px = Some([a[0], a[1], b[0], b[1]]);
}
