//! Chat overlay — semi-transparent message log in the bottom-left corner.

use egui::{Align2, Area, Color32, Frame, Margin, Rounding};
use crate::gui::GuiState;
use crate::gui::theme::Theme;

/// Maximum number of messages shown in the chat overlay.
const MAX_VISIBLE_MESSAGES: usize = 10;

/// Draw the chat overlay in the bottom-left corner.
/// Semi-transparent so the game scene is visible behind it.
pub fn draw(ctx: &egui::Context, theme: &Theme, gui_state: &mut GuiState) {
    let screen = ctx.screen_rect();
    let chat_width = 380.0;
    let chat_height = 280.0;
    let margin = 16.0;

    Area::new(egui::Id::new("chat_overlay"))
        .fixed_pos(egui::pos2(margin, screen.height() - chat_height - margin))
        .show(ctx, |ui| {
            Frame::none()
                .fill(theme.overlay_bg)
                .rounding(Rounding::same(8))
                .inner_margin(Margin::same(10))
                .show(ui, |ui| {
                    ui.set_min_width(chat_width);
                    ui.set_max_height(chat_height);

                    // Message history
                    let messages_height = chat_height - 40.0;
                    egui::ScrollArea::vertical()
                        .max_height(messages_height)
                        .stick_to_bottom(true)
                        .show(ui, |ui| {
                            if gui_state.chat_messages.is_empty() {
                                ui.label(theme.dimmed("No messages yet. Press Enter to chat."));
                            } else {
                                let start = gui_state.chat_messages.len()
                                    .saturating_sub(MAX_VISIBLE_MESSAGES);
                                for (time, sender, msg) in &gui_state.chat_messages[start..] {
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            egui::RichText::new(time)
                                                .size(10.0)
                                                .color(theme.text_dim),
                                        );
                                        ui.label(
                                            egui::RichText::new(sender)
                                                .size(12.0)
                                                .color(theme.accent),
                                        );
                                        ui.label(
                                            egui::RichText::new(msg)
                                                .size(12.0)
                                                .color(theme.text),
                                        );
                                    });
                                }
                            }
                        });

                    ui.separator();

                    // Text input
                    let response = ui.add(
                        egui::TextEdit::singleline(&mut gui_state.chat_input)
                            .desired_width(chat_width - 20.0)
                            .hint_text(
                                egui::RichText::new("Type a message...")
                                    .color(Color32::from_rgba_premultiplied(100, 100, 120, 150)),
                            )
                            .text_color(theme.text),
                    );

                    // Submit on Enter
                    if response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && !gui_state.chat_input.trim().is_empty()
                    {
                        let msg = gui_state.chat_input.trim().to_string();
                        let time = "now".to_string();
                        let sender = "You".to_string();
                        gui_state.chat_messages.push((time, sender, msg));
                        gui_state.chat_input.clear();
                        response.request_focus();
                    }
                });
        });
}
