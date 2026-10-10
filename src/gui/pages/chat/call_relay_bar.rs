//! Calls through the server (step E of docs/design/blocking-and-safe-mode.md, section 10f,
//! 2026-10-09): what the call UI says when a call or voice room cannot go through the server.
//! The 1:1 call bar (modals.rs `draw_call_bar`) shows the line under its controls; a voice room
//! has no bar of its own, so while one cannot connect a small bar like the call bar carries the
//! line, with a Dismiss. The state behind both is `GuiState::call_relay`
//! (src/net/call_relay.rs), kept by src/engine/call_relay.rs.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use crate::net::call_relay::CallScope;

/// The line under a call's or room's controls, in the warning colour.
pub(super) fn relay_line(ui: &mut egui::Ui, theme: &Theme, line: &str) {
    ui.label(RichText::new(line).size(theme.font_size_small).color(theme.warning()));
}

/// While the voice room we are in cannot go through the server: a bar at the top, like the
/// call bar, saying so, until the person dismisses it or leaves the room.
pub(super) fn draw_room_bar(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(room) = state.voice_active_room.clone() else { return };
    let Some(line) = state.call_relay.line_for(&CallScope::Room(room.clone())) else { return };
    if state.call_relay.dismissed() {
        return;
    }
    let name = state
        .chat_channels
        .iter()
        .find(|c| c.id == room)
        .map(|c| c.name.clone())
        .unwrap_or(room);
    let mut dismiss = false;
    egui::Window::new("voice_room_relay_bar")
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 8.0])
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("Voice in {name}: not connected"))
                        .size(theme.font_size_body)
                        .color(theme.text_primary())
                        .strong(),
                );
                if widgets::Button::secondary("Dismiss").show(ui, theme) {
                    dismiss = true;
                }
            });
            relay_line(ui, theme, line);
        });
    if dismiss {
        state.call_relay.dismiss();
    }
}
