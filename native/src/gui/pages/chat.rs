//! In-game chat overlay (semi-transparent, bottom-left).
//! Messages are stored locally in GuiState. When a server connection
//! is available, messages will be sent via WebSocket (not yet wired).

use egui::{Color32, Frame, RichText, ScrollArea};
use crate::gui::GuiState;
use crate::gui::theme::Theme;

/// Maximum messages kept in the local chat buffer.
const MAX_MESSAGES: usize = 100;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Chat").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Offline").size(theme.font_size_small).color(theme.warning()));
                });
            });

            ui.add_space(theme.spacing_sm);

            // Message list
            ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    if state.chat_messages.is_empty() {
                        ui.label(RichText::new("Press Enter to chat").color(theme.text_muted()));
                    }
                    for msg in &state.chat_messages {
                        ui.label(RichText::new(msg).size(theme.font_size_small).color(theme.text_primary()));
                    }
                });

            // Input at bottom
            ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                ui.horizontal(|ui| {
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut state.chat_input)
                            .desired_width(ui.available_width() - 70.0)
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
        });
}
