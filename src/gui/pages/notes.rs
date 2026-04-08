//! Notes page — full notes app with sidebar list, text editor,
//! toolbar with bold/italic markers, word count, auto-save indicator,
//! delete with confirmation.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiNote, GuiState};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// Page-local state for delete confirmation and other transient UI.
struct NotesPageState {
    confirm_delete: Option<u64>,
    search: String,
}

impl Default for NotesPageState {
    fn default() -> Self {
        Self {
            confirm_delete: None,
            search: String::new(),
        }
    }
}

thread_local! {
    static LOCAL: RefCell<NotesPageState> = RefCell::new(NotesPageState::default());
}

fn with_local<R>(f: impl FnOnce(&mut NotesPageState) -> R) -> R {
    LOCAL.with(|s| f(&mut s.borrow_mut()))
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Left sidebar
    egui::SidePanel::left("notes_sidebar")
        .default_width(220.0)
        .resizable(true)
        .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(8.0))
        .show(ctx, |ui| {
            ui.label(
                RichText::new("Notes")
                    .size(theme.font_size_title)
                    .color(theme.text_primary()),
            );
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
                with_local(|local| local.confirm_delete = None);
            }

            ui.add_space(theme.spacing_xs);

            // Search notes
            with_local(|local| {
                ui.add(
                    egui::TextEdit::singleline(&mut local.search)
                        .desired_width(ui.available_width())
                        .hint_text("Search notes..."),
                );
            });

            ui.add_space(theme.spacing_sm);

            // Note list
            ScrollArea::vertical()
                .id_salt("notes_sidebar_list")
                .show(ui, |ui| {
                    let search_term = with_local(|local| local.search.to_lowercase());
                    let mut select_id = None;

                    // Sort by modified time, newest first
                    let mut sorted_indices: Vec<usize> = (0..state.notes.len()).collect();
                    sorted_indices.sort_by(|a, b| {
                        state.notes[*b].modified.cmp(&state.notes[*a].modified)
                    });

                    for idx in sorted_indices {
                        let note = &state.notes[idx];
                        // Filter by search
                        if !search_term.is_empty()
                            && !note.title.to_lowercase().contains(&search_term)
                            && !note.content.to_lowercase().contains(&search_term)
                        {
                            continue;
                        }

                        let is_selected = state.notes_selected == Some(note.id);
                        let fill = if is_selected { theme.bg_card() } else { Color32::TRANSPARENT };
                        let stroke = if is_selected {
                            Stroke::new(1.0, theme.accent())
                        } else {
                            Stroke::NONE
                        };

                        let frame = egui::Frame::none()
                            .fill(fill)
                            .rounding(Rounding::same(4))
                            .stroke(stroke)
                            .inner_margin(8.0);

                        let note_id = note.id;
                        let note_title = note.title.clone();
                        let note_modified = note.modified;
                        let preview: String = note.content.lines().next().unwrap_or("").chars().take(40).collect();

                        frame.show(ui, |ui| {
                            let resp = ui.vertical(|ui| {
                                ui.label(
                                    RichText::new(&note_title)
                                        .size(theme.font_size_body)
                                        .color(theme.text_primary()),
                                );
                                ui.label(
                                    RichText::new(format_timestamp(note_modified))
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                                if !preview.is_empty() {
                                    ui.label(
                                        RichText::new(&preview)
                                            .size(theme.font_size_small)
                                            .color(theme.text_muted()),
                                    );
                                }
                            }).response;
                            if resp.interact(egui::Sense::click()).clicked() {
                                select_id = Some(note_id);
                            }
                        });
                        ui.add_space(2.0);
                    }

                    if let Some(id) = select_id {
                        state.notes_selected = Some(id);
                        with_local(|local| local.confirm_delete = None);
                    }
                });

            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new(format!("{} notes", state.notes.len()))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
        });

    // Main editor panel
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(16.0))
        .show(ctx, |ui| {
            if let Some(sel_id) = state.notes_selected {
                if let Some(note) = state.notes.iter_mut().find(|n| n.id == sel_id) {
                    // Toolbar
                    ui.horizontal(|ui| {
                        // Title editor
                        let title_resp = ui.add(
                            egui::TextEdit::singleline(&mut note.title)
                                .font(egui::FontId::proportional(theme.font_size_heading))
                                .desired_width(300.0)
                                .hint_text("Note title"),
                        );
                        if title_resp.changed() {
                            note.modified = current_timestamp();
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Auto-save indicator
                            let ts = note.modified;
                            let save_text = if current_timestamp().saturating_sub(ts) < 2 {
                                "Unsaved changes"
                            } else {
                                "Saved"
                            };
                            let save_color = if save_text == "Saved" { theme.success() } else { theme.warning() };
                            ui.label(
                                RichText::new(save_text)
                                    .size(theme.font_size_small)
                                    .color(save_color),
                            );
                        });
                    });

                    ui.add_space(theme.spacing_xs);

                    // Formatting toolbar
                    ui.horizontal(|ui| {
                        // Bold/Italic markers
                        let bold_btn = egui::Button::new(
                            RichText::new("B").size(theme.font_size_body).color(theme.text_primary()).strong(),
                        )
                        .fill(theme.bg_card())
                        .min_size(Vec2::new(28.0, 28.0));
                        if ui.add(bold_btn).clicked() {
                            note.content.push_str("**");
                            note.modified = current_timestamp();
                        }

                        let italic_btn = egui::Button::new(
                            RichText::new("I").size(theme.font_size_body).color(theme.text_primary()).italics(),
                        )
                        .fill(theme.bg_card())
                        .min_size(Vec2::new(28.0, 28.0));
                        if ui.add(italic_btn).clicked() {
                            note.content.push_str("_");
                            note.modified = current_timestamp();
                        }

                        ui.separator();

                        // Word count
                        let word_count = note.content.split_whitespace().count();
                        let char_count = note.content.len();
                        ui.label(
                            RichText::new(format!("{} words | {} chars", word_count, char_count))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );

                        ui.separator();

                        ui.label(
                            RichText::new(format!("Modified: {}", format_timestamp(note.modified)))
                                .size(theme.font_size_small)
                                .color(theme.text_muted()),
                        );
                    });

                    ui.add_space(theme.spacing_xs);

                    // Content editor - fills available space
                    let available = ui.available_height() - 50.0;
                    let rows = (available / 18.0).max(10.0) as usize;
                    let content_resp = ui.add(
                        egui::TextEdit::multiline(&mut note.content)
                            .desired_width(f32::INFINITY)
                            .desired_rows(rows)
                            .hint_text("Start writing...")
                            .font(egui::FontId::proportional(theme.font_size_body)),
                    );
                    if content_resp.changed() {
                        note.modified = current_timestamp();
                    }

                    ui.add_space(theme.spacing_sm);

                    // Delete button with confirmation
                    let delete_id = sel_id;
                    let confirming = with_local(|local| local.confirm_delete == Some(delete_id));
                    if confirming {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("Delete this note?")
                                    .color(theme.danger()),
                            );
                            if widgets::danger_button(ui, theme, "Yes, Delete") {
                                state.notes.retain(|n| n.id != delete_id);
                                state.notes_selected = state.notes.first().map(|n| n.id);
                                with_local(|local| local.confirm_delete = None);
                            }
                            if widgets::secondary_button(ui, theme, "Cancel") {
                                with_local(|local| local.confirm_delete = None);
                            }
                        });
                    } else if widgets::danger_button(ui, theme, "Delete Note") {
                        with_local(|local| local.confirm_delete = Some(delete_id));
                    }
                } else {
                    state.notes_selected = None;
                }
            } else {
                ui.centered_and_justified(|ui| {
                    ui.label(
                        RichText::new("Select or create a note")
                            .size(theme.font_size_heading)
                            .color(theme.text_muted()),
                    );
                });
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
