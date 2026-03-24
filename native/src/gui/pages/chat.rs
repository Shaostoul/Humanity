//! In-game chat overlay (semi-transparent, bottom-left).
//! Messages are stored locally in GuiState. When a server connection
//! is available, messages will be sent via WebSocket (not yet wired).

use egui::{Align2, Color32, Frame, RichText, Rounding, Vec2};
use crate::gui::GuiState;
use crate::gui::theme::Theme;

/// Maximum messages kept in the local chat buffer.
const MAX_MESSAGES: usize = 100;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let screen = ctx.screen_rect();
    let chat_width = 400.0_f32.min(screen.width() * 0.4);
    let chat_height = 250.0;

    egui::Window::new("chat_overlay")
        .title_bar(false)
        .resizable(false)
        .fixed_pos([8.0, screen.height() - chat_height - 8.0])
        .fixed_size(Vec2::new(chat_width, chat_height))
        .frame(Frame::none().fill(Color32::from_black_alpha(160)).rounding(Rounding::same(theme.border_radius as u8)))
        .show(ctx, |ui| {
            // Offline mode indicator
            ui.horizontal(|ui| {
                ui.label(RichText::new("Local Chat").size(theme.font_size_small).color(theme.text_muted()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Offline").size(theme.font_size_small).color(theme.warning()));
                });
            });

            // Message list
            let msg_height = chat_height - 60.0;
            egui::ScrollArea::vertical()
                .max_height(msg_height)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if state.chat_messages.is_empty() {
                        ui.label(RichText::new("Press Enter to chat").color(theme.text_muted()));
                    }
                    for msg in &state.chat_messages {
                        ui.label(RichText::new(msg).size(theme.font_size_small).color(theme.text_primary()));
                    }
                });

            // Input
            ui.horizontal(|ui| {
                let response = ui.add(
                    egui::TextEdit::singleline(&mut state.chat_input)
                        .desired_width(chat_width - 70.0)
                        .hint_text("Type a message...")
                );
                if (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                    || ui.button("Send").clicked()
                {
                    if !state.chat_input.trim().is_empty() {
                        let msg = format!("You: {}", state.chat_input.trim());
                        state.chat_messages.push(msg);
                        state.chat_input.clear();
                        // Keep message buffer bounded
                        while state.chat_messages.len() > MAX_MESSAGES {
                            state.chat_messages.remove(0);
                        }
                    }
                    response.request_focus();
                }
            });
        });
}
