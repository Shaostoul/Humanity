//! Task Board — three-column kanban layout with task cards.
//!
//! Columns: Todo, In Progress, Done. Each card shows title, description
//! preview, priority badge, assignee, and labels. Filter bar at top.

use egui::{Color32, RichText, Rounding, ScrollArea, Stroke, Vec2};
use crate::gui::{GuiPage, GuiState, TaskPriority, TaskStatus, GuiTask};
use crate::gui::theme::Theme;
use crate::gui::widgets;

fn priority_color(theme: &Theme, priority: TaskPriority) -> Color32 {
    match priority {
        TaskPriority::Low => theme.text_muted(),
        TaskPriority::Medium => Theme::c32(&theme.info),
        TaskPriority::High => theme.warning(),
        TaskPriority::Critical => theme.danger(),
    }
}

fn priority_label(priority: TaskPriority) -> &'static str {
    match priority {
        TaskPriority::Low => "Low",
        TaskPriority::Medium => "Medium",
        TaskPriority::High => "High",
        TaskPriority::Critical => "Critical",
    }
}

fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => "Todo",
        TaskStatus::InProgress => "In Progress",
        TaskStatus::Done => "Done",
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let screen = ctx.screen_rect();
    let painter = ctx.layer_painter(egui::LayerId::background());
    painter.rect_filled(screen, 0.0, Color32::from_rgba_unmultiplied(0, 0, 0, 200));

    egui::Window::new("Task Board")
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size(Vec2::new(900.0, 600.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Task Board").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::secondary_button(ui, theme, "Back") {
                        state.active_page = GuiPage::EscapeMenu;
                    }
                    if widgets::primary_button(ui, theme, "+ New Task") {
                        state.task_show_new_form = !state.task_show_new_form;
                    }
                });
            });

            ui.add_space(theme.spacing_sm);

            // Filter bar
            ui.horizontal(|ui| {
                ui.label(RichText::new("Search:").color(theme.text_secondary()));
                ui.add(egui::TextEdit::singleline(&mut state.task_search)
                    .desired_width(200.0)
                    .hint_text("Filter tasks..."));
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new("Priority:").color(theme.text_secondary()));
                let current_label = state.task_filter_priority.map_or("All", priority_label);
                egui::ComboBox::from_id_salt("priority_filter")
                    .selected_text(current_label)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(state.task_filter_priority.is_none(), "All").clicked() {
                            state.task_filter_priority = None;
                        }
                        for p in [TaskPriority::Low, TaskPriority::Medium, TaskPriority::High, TaskPriority::Critical] {
                            if ui.selectable_label(state.task_filter_priority == Some(p), priority_label(p)).clicked() {
                                state.task_filter_priority = Some(p);
                            }
                        }
                    });
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new("Assignee:").color(theme.text_secondary()));
                ui.add(egui::TextEdit::singleline(&mut state.task_filter_assignee)
                    .desired_width(120.0)
                    .hint_text("Filter by assignee"));
            });

            ui.add_space(theme.spacing_sm);

            // New task form (inline)
            if state.task_show_new_form {
                widgets::card(ui, theme, |ui| {
                    ui.label(RichText::new("New Task").size(theme.font_size_heading).color(theme.accent()));
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Title:").color(theme.text_secondary()));
                        ui.add(egui::TextEdit::singleline(&mut state.task_new_title).desired_width(300.0));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Description:").color(theme.text_secondary()));
                        ui.add(egui::TextEdit::multiline(&mut state.task_new_description)
                            .desired_width(300.0)
                            .desired_rows(2));
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Priority:").color(theme.text_secondary()));
                        egui::ComboBox::from_id_salt("new_task_priority")
                            .selected_text(priority_label(state.task_new_priority))
                            .show_ui(ui, |ui| {
                                for p in [TaskPriority::Low, TaskPriority::Medium, TaskPriority::High, TaskPriority::Critical] {
                                    if ui.selectable_label(state.task_new_priority == p, priority_label(p)).clicked() {
                                        state.task_new_priority = p;
                                    }
                                }
                            });
                        ui.add_space(theme.spacing_md);
                        ui.label(RichText::new("Assignee:").color(theme.text_secondary()));
                        ui.add(egui::TextEdit::singleline(&mut state.task_new_assignee).desired_width(120.0));
                    });
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, theme, "Create") && !state.task_new_title.is_empty() {
                            let task = GuiTask {
                                id: state.task_next_id,
                                title: state.task_new_title.clone(),
                                description: state.task_new_description.clone(),
                                priority: state.task_new_priority,
                                status: TaskStatus::Todo,
                                assignee: state.task_new_assignee.clone(),
                                labels: Vec::new(),
                            };
                            state.task_next_id += 1;
                            state.tasks.push(task);
                            state.task_new_title.clear();
                            state.task_new_description.clear();
                            state.task_new_priority = TaskPriority::Medium;
                            state.task_new_assignee.clear();
                            state.task_show_new_form = false;
                        }
                        if widgets::secondary_button(ui, theme, "Cancel") {
                            state.task_show_new_form = false;
                        }
                    });
                });
                ui.add_space(theme.spacing_sm);
            }

            // Build filtered indices
            let search_lower = state.task_search.to_lowercase();
            let assignee_lower = state.task_filter_assignee.to_lowercase();
            let filtered: Vec<usize> = state.tasks.iter().enumerate()
                .filter(|(_, t)| {
                    if !search_lower.is_empty() && !t.title.to_lowercase().contains(&search_lower)
                        && !t.description.to_lowercase().contains(&search_lower) {
                        return false;
                    }
                    if let Some(pf) = state.task_filter_priority {
                        if t.priority != pf { return false; }
                    }
                    if !assignee_lower.is_empty() && !t.assignee.to_lowercase().contains(&assignee_lower) {
                        return false;
                    }
                    true
                })
                .map(|(i, _)| i)
                .collect();

            // Three-column kanban
            let columns = [
                ("Todo", TaskStatus::Todo),
                ("In Progress", TaskStatus::InProgress),
                ("Done", TaskStatus::Done),
            ];

            ui.columns(3, |cols| {
                for (col_idx, (col_name, col_status)) in columns.iter().enumerate() {
                    cols[col_idx].vertical(|ui| {
                        ui.label(RichText::new(*col_name).size(theme.font_size_heading).color(theme.text_primary()));
                        ui.add_space(theme.spacing_xs);
                        ui.separator();
                        ui.add_space(theme.spacing_xs);

                        let col_tasks: Vec<usize> = filtered.iter()
                            .copied()
                            .filter(|&i| state.tasks[i].status == *col_status)
                            .collect();

                        if col_tasks.is_empty() {
                            ui.label(RichText::new("No tasks").color(theme.text_muted()));
                        }

                        ScrollArea::vertical().max_height(380.0).show(ui, |ui| {
                            for &idx in &col_tasks {
                                let task = &state.tasks[idx];
                                let pc = priority_color(theme, task.priority);
                                widgets::card(ui, theme, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&task.title).size(theme.font_size_body).color(theme.text_primary()));
                                        // Priority badge
                                        egui::Frame::none()
                                            .fill(pc)
                                            .rounding(Rounding::same(3))
                                            .inner_margin(Vec2::new(4.0, 1.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(priority_label(task.priority)).size(theme.font_size_small).color(Color32::WHITE));
                                            });
                                    });
                                    if !task.description.is_empty() {
                                        let preview: String = task.description.chars().take(60).collect();
                                        let suffix = if task.description.len() > 60 { "..." } else { "" };
                                        ui.label(RichText::new(format!("{}{}", preview, suffix)).color(theme.text_muted()).size(theme.font_size_small));
                                    }
                                    if !task.assignee.is_empty() {
                                        ui.label(RichText::new(format!("Assignee: {}", task.assignee)).color(theme.text_secondary()).size(theme.font_size_small));
                                    }
                                    for label in &task.labels {
                                        ui.label(RichText::new(label).color(theme.text_muted()).size(theme.font_size_small));
                                    }
                                });
                                ui.add_space(theme.spacing_xs);
                            }
                        });
                    });
                }
            });

            // Empty state
            if state.tasks.is_empty() {
                ui.add_space(theme.spacing_lg);
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("No tasks yet").size(theme.font_size_heading).color(theme.text_muted()));
                    ui.label(RichText::new("Click '+ New Task' to create one.").color(theme.text_secondary()));
                });
            }
        });
}
