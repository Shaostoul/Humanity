//! Task Board — three-column kanban layout with task cards.
//!
//! Features: project selector, kanban columns (Todo/In Progress/Done),
//! task cards with priority badges and labels, detail panel, new task form,
//! filter bar with search/priority/assignee filters, task count per column.

use egui::{Color32, Frame, RichText, Rounding, ScrollArea, Vec2};
use crate::gui::{GuiState, TaskPriority, TaskStatus, GuiTask};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use std::cell::RefCell;

/// Page-local state for the task board.
struct TaskPageState {
    selected_task: Option<usize>,
    project_filter: String,
    projects: Vec<String>,
    new_labels_input: String,
    new_project: String,
    editing: bool,
    edit_status: TaskStatus,
    edit_priority: TaskPriority,
}

impl Default for TaskPageState {
    fn default() -> Self {
        Self {
            selected_task: None,
            project_filter: String::new(),
            projects: vec!["Frontend".into(), "Backend".into(), "Game Engine".into(), "Infrastructure".into()],
            new_labels_input: String::new(),
            new_project: String::new(),
            editing: false,
            edit_status: TaskStatus::Todo,
            edit_priority: TaskPriority::Medium,
        }
    }
}

fn with_state<R>(f: impl FnOnce(&mut TaskPageState) -> R) -> R {
    thread_local! {
        static STATE: RefCell<TaskPageState> = RefCell::new(TaskPageState::default());
    }
    STATE.with(|s| f(&mut s.borrow_mut()))
}

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
    // Track which task to select (set by card "View" buttons)
    let mut select_task: Option<usize> = None;
    let mut close_detail = false;
    let mut apply_edit: Option<(usize, TaskStatus, TaskPriority)> = None;

    // Draw detail side panel if a task is selected
    with_state(|ps| {
        if let Some(sel_idx) = ps.selected_task {
            if let Some(task) = state.tasks.get(sel_idx) {
                egui::SidePanel::right("task_detail_panel")
                    .min_width(280.0)
                    .max_width(360.0)
                    .frame(Frame::none().fill(Color32::from_rgb(25, 25, 32)).inner_margin(12.0))
                    .show(ctx, |ui| {
                        ScrollArea::vertical().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Task Detail").size(theme.font_size_heading).color(theme.text_primary()));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if widgets::secondary_button(ui, theme, "X") {
                                        close_detail = true;
                                    }
                                });
                            });
                            ui.add_space(theme.spacing_sm);

                            // Title
                            ui.label(RichText::new(&task.title).size(theme.font_size_title).color(theme.accent()));
                            ui.add_space(theme.spacing_xs);

                            // Priority badge
                            let pc = priority_color(theme, task.priority);
                            egui::Frame::none()
                                .fill(pc)
                                .rounding(Rounding::same(3))
                                .inner_margin(Vec2::new(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(priority_label(task.priority)).size(theme.font_size_small).color(Color32::WHITE));
                                });

                            ui.add_space(theme.spacing_sm);

                            // Status
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Status:").color(theme.text_secondary()));
                                if ps.editing {
                                    egui::ComboBox::from_id_salt("edit_status")
                                        .selected_text(status_label(ps.edit_status))
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for s in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done] {
                                                if ui.selectable_label(ps.edit_status == s, status_label(s)).clicked() {
                                                    ps.edit_status = s;
                                                }
                                            }
                                        });
                                } else {
                                    ui.label(RichText::new(status_label(task.status)).color(theme.text_primary()));
                                }
                            });

                            // Priority (editable)
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Priority:").color(theme.text_secondary()));
                                if ps.editing {
                                    egui::ComboBox::from_id_salt("edit_priority")
                                        .selected_text(priority_label(ps.edit_priority))
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for p in [TaskPriority::Low, TaskPriority::Medium, TaskPriority::High, TaskPriority::Critical] {
                                                if ui.selectable_label(ps.edit_priority == p, priority_label(p)).clicked() {
                                                    ps.edit_priority = p;
                                                }
                                            }
                                        });
                                } else {
                                    ui.label(RichText::new(priority_label(task.priority)).color(theme.text_primary()));
                                }
                            });

                            // Assignee
                            if !task.assignee.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Assignee:").color(theme.text_secondary()));
                                    ui.label(RichText::new(&task.assignee).color(theme.text_primary()));
                                });
                            }

                            ui.add_space(theme.spacing_sm);

                            // Full description
                            if !task.description.is_empty() {
                                ui.label(RichText::new("Description").size(theme.font_size_body).color(theme.text_secondary()));
                                widgets::card(ui, theme, |ui| {
                                    ui.label(RichText::new(&task.description).color(theme.text_primary()));
                                });
                            }

                            ui.add_space(theme.spacing_xs);

                            // Labels
                            if !task.labels.is_empty() {
                                ui.label(RichText::new("Labels").size(theme.font_size_body).color(theme.text_secondary()));
                                ui.horizontal_wrapped(|ui| {
                                    for label in &task.labels {
                                        egui::Frame::none()
                                            .fill(Theme::c32(&theme.info))
                                            .rounding(Rounding::same(3))
                                            .inner_margin(Vec2::new(5.0, 2.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(label).size(theme.font_size_small).color(Color32::WHITE));
                                            });
                                    }
                                });
                            }

                            ui.add_space(theme.spacing_sm);

                            // Task ID
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Task ID:").color(theme.text_muted()));
                                ui.label(RichText::new(format!("#{}", task.id)).color(theme.text_muted()));
                            });

                            ui.add_space(theme.spacing_md);

                            // Comments section
                            ui.label(RichText::new("Comments").size(theme.font_size_body).color(theme.text_secondary()));
                            widgets::card(ui, theme, |ui| {
                                ui.label(RichText::new("No comments yet.").color(theme.text_muted()));
                            });

                            ui.add_space(theme.spacing_md);

                            // Edit / Save buttons
                            ui.horizontal(|ui| {
                                if ps.editing {
                                    if widgets::primary_button(ui, theme, "Save") {
                                        apply_edit = Some((sel_idx, ps.edit_status, ps.edit_priority));
                                        ps.editing = false;
                                    }
                                    if widgets::secondary_button(ui, theme, "Cancel") {
                                        ps.editing = false;
                                    }
                                } else if widgets::primary_button(ui, theme, "Edit") {
                                    ps.edit_status = task.status;
                                    ps.edit_priority = task.priority;
                                    ps.editing = true;
                                }
                            });
                        });
                    });
            } else {
                ps.selected_task = None;
            }
        }
    });

    // Apply edits outside borrow
    if let Some((idx, new_status, new_priority)) = apply_edit {
        if let Some(task) = state.tasks.get_mut(idx) {
            task.status = new_status;
            task.priority = new_priority;
        }
    }
    if close_detail {
        with_state(|ps| ps.selected_task = None);
    }

    egui::CentralPanel::default()
        .frame(Frame::none().fill(Color32::from_rgb(20, 20, 25)).inner_margin(16.0))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                ui.label(RichText::new("Task Board").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, theme, "+ New Task") {
                        state.task_show_new_form = !state.task_show_new_form;
                    }
                });
            });

            ui.add_space(theme.spacing_xs);

            // Project selector bar
            with_state(|ps| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Project:").color(theme.text_secondary()));
                    let current_project = if ps.project_filter.is_empty() { "All Projects" } else { &ps.project_filter };
                    egui::ComboBox::from_id_salt("project_selector")
                        .selected_text(current_project)
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(ps.project_filter.is_empty(), "All Projects").clicked() {
                                ps.project_filter.clear();
                            }
                            for proj in &ps.projects {
                                if ui.selectable_label(ps.project_filter == *proj, proj).clicked() {
                                    ps.project_filter = proj.clone();
                                }
                            }
                        });
                });
            });

            ui.add_space(theme.spacing_xs);

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
                    with_state(|ps| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Labels:").color(theme.text_secondary()));
                            ui.add(egui::TextEdit::singleline(&mut ps.new_labels_input)
                                .desired_width(200.0)
                                .hint_text("comma-separated"));
                            ui.add_space(theme.spacing_md);
                            ui.label(RichText::new("Project:").color(theme.text_secondary()));
                            let projects_clone = ps.projects.clone();
                            egui::ComboBox::from_id_salt("new_task_project")
                                .selected_text(if ps.new_project.is_empty() { "None" } else { &ps.new_project })
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(ps.new_project.is_empty(), "None").clicked() {
                                        ps.new_project.clear();
                                    }
                                    for proj in &projects_clone {
                                        if ui.selectable_label(ps.new_project == *proj, proj).clicked() {
                                            ps.new_project = proj.clone();
                                        }
                                    }
                                });
                        });
                    });
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        if widgets::primary_button(ui, theme, "Create") && !state.task_new_title.is_empty() {
                            let labels: Vec<String> = with_state(|ps| {
                                let mut all_labels: Vec<String> = ps.new_labels_input.split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect();
                                if !ps.new_project.is_empty() {
                                    all_labels.push(ps.new_project.clone());
                                }
                                ps.new_labels_input.clear();
                                ps.new_project.clear();
                                all_labels
                            });
                            let task = GuiTask {
                                id: state.task_next_id,
                                title: state.task_new_title.clone(),
                                description: state.task_new_description.clone(),
                                priority: state.task_new_priority,
                                status: TaskStatus::Todo,
                                assignee: state.task_new_assignee.clone(),
                                labels,
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
            let project_filter = with_state(|ps| ps.project_filter.clone());
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
                    if !project_filter.is_empty() {
                        if !t.labels.iter().any(|l| l == &project_filter) { return false; }
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
                        let col_tasks: Vec<usize> = filtered.iter()
                            .copied()
                            .filter(|&i| state.tasks[i].status == *col_status)
                            .collect();

                        // Column header with count
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(*col_name).size(theme.font_size_heading).color(theme.text_primary()));
                            egui::Frame::none()
                                .fill(theme.bg_secondary())
                                .rounding(Rounding::same(8))
                                .inner_margin(Vec2::new(6.0, 2.0))
                                .show(ui, |ui| {
                                    ui.label(RichText::new(col_tasks.len().to_string()).size(theme.font_size_small).color(theme.text_muted()));
                                });
                        });
                        ui.add_space(theme.spacing_xs);
                        ui.separator();
                        ui.add_space(theme.spacing_xs);

                        if col_tasks.is_empty() {
                            ui.label(RichText::new("No tasks").color(theme.text_muted()));
                        }

                        ScrollArea::vertical().show(ui, |ui| {
                            for &idx in &col_tasks {
                                let task = &state.tasks[idx];
                                let pc = priority_color(theme, task.priority);
                                widgets::card(ui, theme, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&task.title).size(theme.font_size_body).color(theme.text_primary()));
                                        egui::Frame::none()
                                            .fill(pc)
                                            .rounding(Rounding::same(3))
                                            .inner_margin(Vec2::new(4.0, 1.0))
                                            .show(ui, |ui| {
                                                ui.label(RichText::new(priority_label(task.priority)).size(theme.font_size_small).color(Color32::WHITE));
                                            });
                                    });
                                    // Description preview (first 80 chars)
                                    if !task.description.is_empty() {
                                        let preview: String = task.description.chars().take(80).collect();
                                        let suffix = if task.description.chars().count() > 80 { "..." } else { "" };
                                        ui.label(RichText::new(format!("{}{}", preview, suffix)).color(theme.text_muted()).size(theme.font_size_small));
                                    }
                                    if !task.assignee.is_empty() {
                                        ui.label(RichText::new(format!("Assignee: {}", task.assignee)).color(theme.text_secondary()).size(theme.font_size_small));
                                    }
                                    // Labels as small badges
                                    if !task.labels.is_empty() {
                                        ui.horizontal_wrapped(|ui| {
                                            for label in &task.labels {
                                                egui::Frame::none()
                                                    .fill(Theme::c32(&theme.info))
                                                    .rounding(Rounding::same(3))
                                                    .inner_margin(Vec2::new(4.0, 1.0))
                                                    .show(ui, |ui| {
                                                        ui.label(RichText::new(label).size(theme.font_size_small).color(Color32::WHITE));
                                                    });
                                            }
                                        });
                                    }
                                    // Task ID
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(format!("#{}", task.id)).color(theme.text_muted()).size(theme.font_size_small));
                                        if widgets::secondary_button(ui, theme, "View") {
                                            select_task = Some(idx);
                                        }
                                    });
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

    // Apply card selection
    if let Some(idx) = select_task {
        with_state(|ps| {
            ps.selected_task = Some(idx);
            ps.editing = false;
        });
    }
}
