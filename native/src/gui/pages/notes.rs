//! Notes page — sidebar list of notes with text editor panel.

use egui::{RichText, Rounding, Stroke, Vec2};
use crate::gui::{GuiNote, GuiPage, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::Window::new("Notes")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(640.0, 460.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // ── Left sidebar: note list ──
                ui.vertical(|ui| {
                    ui.set_min_width(180.0);
                    ui.set_max_width(180.0);

                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new("Notes")
                                .size(theme.font_size_heading)
                                .color(theme.text_primary()),
                        );
                    });
                    ui.add_space(theme.spacing_xs);

                    if widgets::primary_button(ui, theme, "+ New Note") {
                        let id = state.notes_next_id;
                        state.notes_next_id += 1;
                        let now = current_timestamp();
                        state.notes.push(GuiNote {
                            id,
                            title: "Untitled".to_string(),
                            content: String::new(),
                            modified: now,
                        });
                        state.notes_selected = Some(id);
                    }

                    ui.add_space(theme.spacing_sm);

                    egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                        let mut select_id = None;
                        for note in &state.notes {
                            let is_selected = state.notes_selected == Some(note.id);
                            let fill = if is_selected {
                                theme.bg_card()
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let stroke = if is_selected {
                                Stroke::new(1.0, theme.accent())
                            } else {
                                Stroke::NONE
                            };

                            let frame = egui::Frame::none()
                                .fill(fill)
                                .rounding(Rounding::same(4))
                                .stroke(stroke)
                                .inner_margin(6.0);

                            frame.show(ui, |ui| {
                                let resp = ui.vertical(|ui| {
                                    ui.label(
                                        RichText::new(&note.title)
                                            .size(theme.font_size_body)
                                            .color(theme.text_primary()),
                                    );
                                    ui.label(
                                        RichText::new(format_timestamp(note.modified))
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                }).response;
                                if resp.interact(egui::Sense::click()).clicked() {
                                    select_id = Some(note.id);
                                }
                            });
                        }
                        if let Some(id) = select_id {
                            state.notes_selected = Some(id);
                        }
                    });
                });

                ui.separator();

                // ── Right panel: editor ──
                ui.vertical(|ui| {
                    if let Some(sel_id) = state.notes_selected {
                        if let Some(note) = state.notes.iter_mut().find(|n| n.id == sel_id) {
                            // Title
                            ui.horizontal(|ui| {
                                let title_resp = ui.add(
                                    egui::TextEdit::singleline(&mut note.title)
                                        .font(egui::FontId::proportional(theme.font_size_heading))
                                        .desired_width(300.0)
                                        .hint_text("Note title"),
                                );
                                if title_resp.changed() {
                                    note.modified = current_timestamp();
                                }
                            });

                            // Auto-save indicator
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(format!("Last saved: {}", format_timestamp(note.modified)))
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });

                            ui.add_space(theme.spacing_xs);

                            // Content editor
                            let content_resp = ui.add(
                                egui::TextEdit::multiline(&mut note.content)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(18)
                                    .hint_text("Start writing..."),
                            );
                            if content_resp.changed() {
                                note.modified = current_timestamp();
                            }

                            ui.add_space(theme.spacing_sm);

                            // Delete button
                            let delete_id = sel_id;
                            if widgets::danger_button(ui, theme, "Delete Note") {
                                state.notes.retain(|n| n.id != delete_id);
                                state.notes_selected = state.notes.first().map(|n| n.id);
                            }
                        }
                    } else {
                        ui.centered_and_justified(|ui| {
                            ui.label(
                                RichText::new("Select or create a note")
                                    .size(theme.font_size_body)
                                    .color(theme.text_muted()),
                            );
                        });
                    }
                });
            });

            ui.add_space(theme.spacing_sm);
            if widgets::secondary_button(ui, theme, "Close") {
                state.active_page = GuiPage::EscapeMenu;
            }
        });
}

fn current_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn format_timestamp(ts: u64) -> String {
    if ts == 0 {
        return "Never".to_string();
    }
    let now = current_timestamp();
    let diff = now.saturating_sub(ts);
    if diff < 60 {
        "Just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}
