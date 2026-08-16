//! Tasks: the not-game surface. A three-column kanban (Todo/In Progress/Done)
//! for the user's own tasks, plus the learn-by-doing guide panel.
//!
//! Features: project selector, kanban columns, task cards with priority badges
//! and labels, detail panel, new task form, filter bar with search/priority/
//! assignee filters, task count per column.
//!
//! The guide panel renders the shared onboarding quest chains
//! (`data/onboarding/quests.json`, via `onboarding::draw_quests`). It moved
//! here from the Quests page in v0.1145, per the operator's split: Quests is
//! gameplay, Tasks is real life, and the tutorial for the app itself is a Tasks
//! thing. It sits right of the board on a wide window, drops below it when
//! narrow, and the header's Guides button collapses it either way.

use egui::{Color32, Frame, RichText, ScrollArea};
use crate::gui::{GuiState, TaskPriority, TaskStatus, GuiTask};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use super::onboarding;
use std::cell::RefCell;

/// Available width at or above which the guide panel rides beside the board
/// instead of dropping below it.
const WIDE_LAYOUT_MIN_WIDTH: f32 = 1200.0;
/// Fixed width of the side-by-side guide panel. The board takes the rest.
const GUIDE_PANEL_WIDTH: f32 = 340.0;
/// Fraction of the panel height the board keeps when the guides stack below it.
/// Without a cap the kanban columns' own ScrollAreas eat the full height and
/// the guides section is unreachable off the bottom.
const STACKED_BOARD_HEIGHT_FRACTION: f32 = 0.6;

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
    /// Guide panel visible? Default true (the tutorials are the point for a new
    /// user); board-only users collapse it with the header toggle. Page-local
    /// on purpose, this is a view preference, not app state.
    show_guides: bool,
}

impl Default for TaskPageState {
    fn default() -> Self {
        Self {
            selected_task: None,
            project_filter: String::new(),
            // Loaded from data/tasks/default_projects.json (see load_default_projects()).
            // Falls back to empty Vec if the file is missing — user can add projects via UI.
            projects: load_default_projects(),
            new_labels_input: String::new(),
            new_project: String::new(),
            editing: false,
            edit_status: TaskStatus::Todo,
            edit_priority: TaskPriority::Medium,
            show_guides: true,
        }
    }
}

/// Read the default project list from `data/tasks/default_projects.json`.
/// Used by `TaskPageState::default()` on first lazy init.
fn load_default_projects() -> Vec<String> {
    #[derive(serde::Deserialize)]
    struct File { projects: Vec<String> }
    std::fs::read_to_string("data/tasks/default_projects.json")
        .ok()
        .and_then(|s| serde_json::from_str::<File>(&s).ok())
        .map(|f| f.projects)
        .unwrap_or_default()
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
                    .frame(Frame::none().fill(theme.bg_sidebar()).inner_margin(12.0))
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
                            widgets::badge(ui, theme, priority_label(task.priority), priority_color(theme, task.priority));

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
                                        widgets::badge_sm(ui, theme, label, Theme::c32(&theme.info));
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
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            // Header
            ui.horizontal(|ui| {
                // Heading matches the nav button ("Tasks"): one page, one name.
                ui.label(RichText::new("Tasks").size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::primary_button(ui, theme, "+ New Task") {
                        state.task_show_new_form = !state.task_show_new_form;
                    }
                    // Guide-panel toggle. Right-to-left layout puts this just
                    // left of the New Task button.
                    with_state(|ps| {
                        let label = if ps.show_guides { "Hide guides" } else { "Guides" };
                        if widgets::secondary_button(ui, theme, label) {
                            ps.show_guides = !ps.show_guides;
                        }
                    });
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
            widgets::search_bar(ui, theme, &mut state.task_search, "Filter tasks...");
            ui.horizontal(|ui| {
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

            // Board + guides layout. Wide: guides ride the right edge of the
            // board. Narrow: the board keeps a capped height and the guides
            // stack below it. Either way the header toggle hides them.
            let show_guides = with_state(|ps| ps.show_guides);
            let avail_w = ui.available_width();
            let avail_h = ui.available_height();

            if !show_guides {
                draw_board(ui, theme, state, &filtered, &mut select_task);
            } else if avail_w >= WIDE_LAYOUT_MIN_WIDTH {
                ui.horizontal_top(|ui| {
                    let board_w = (avail_w - GUIDE_PANEL_WIDTH - theme.spacing_lg).max(480.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(board_w, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            // set_width so the board actually fills its share;
                            // allocate_ui_with_layout otherwise advances by the
                            // content's own (smaller) width.
                            ui.set_width(board_w);
                            draw_board(ui, theme, state, &filtered, &mut select_task);
                        },
                    );
                    ui.add_space(theme.spacing_lg);
                    ui.allocate_ui_with_layout(
                        egui::vec2(GUIDE_PANEL_WIDTH, avail_h),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.set_width(GUIDE_PANEL_WIDTH);
                            // Own scroll id: the kanban columns already salt
                            // theirs, and a bare auto-id would clash.
                            ScrollArea::vertical()
                                .id_salt("task_guides_side")
                                .show(ui, |ui| {
                                    draw_guides(ui, theme, state);
                                });
                        },
                    );
                });
            } else {
                let board_h = (avail_h * STACKED_BOARD_HEIGHT_FRACTION).max(220.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(avail_w, board_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_width(avail_w);
                        draw_board(ui, theme, state, &filtered, &mut select_task);
                    },
                );
                ui.add_space(theme.spacing_sm);
                ui.separator();
                ui.add_space(theme.spacing_sm);
                ScrollArea::vertical()
                    .id_salt("task_guides_below")
                    .show(ui, |ui| {
                        draw_guides(ui, theme, state);
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

/// The three-column kanban itself. Split out of `draw` in v0.1145 so both the
/// side-by-side and the stacked layout can render it into a sized child ui.
fn draw_board(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &GuiState,
    filtered: &[usize],
    select_task: &mut Option<usize>,
) {
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
                    widgets::badge(ui, theme, &col_tasks.len().to_string(), theme.bg_secondary());
                });
                ui.add_space(theme.spacing_xs);
                ui.separator();
                ui.add_space(theme.spacing_xs);

                if col_tasks.is_empty() {
                    ui.label(RichText::new("No tasks").color(theme.text_muted()));
                }

                // Each kanban column needs its OWN ScrollArea id — they
                // live inside ui.columns(3,..) which gives each the same
                // auto-id, so egui flags a duplicate-id clash and the
                // columns can stop scrolling independently. Salt by name.
                ScrollArea::vertical().id_salt(*col_name).show(ui, |ui| {
                    for &idx in &col_tasks {
                        let task = &state.tasks[idx];
                        let pc = priority_color(theme, task.priority);
                        widgets::card(ui, theme, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(&task.title).size(theme.font_size_body).color(theme.text_primary()));
                                widgets::badge_sm(ui, theme, priority_label(task.priority), pc);
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
                                        widgets::badge_sm(ui, theme, label, Theme::c32(&theme.info));
                                    }
                                });
                            }
                            // Task ID
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("#{}", task.id)).color(theme.text_muted()).size(theme.font_size_small));
                                if widgets::secondary_button(ui, theme, "View") {
                                    *select_task = Some(idx);
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
}

/// The learn-by-doing guide panel: the app's own tutorial, from first setup
/// through self-sufficiency. Content is the shared onboarding quest chains
/// (`data/onboarding/quests.json`), so adding a guide never needs a recompile.
fn draw_guides(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    ui.label(
        RichText::new("LEARN BY DOING")
            .size(theme.font_size_small)
            .color(theme.accent())
            .strong(),
    );
    ui.add_space(theme.spacing_xs);
    ui.label(
        RichText::new(
            "Guided tutorials from setup to self-sufficiency. Check steps off as you do them. Progress saved locally.",
        )
        .size(theme.font_size_small)
        .color(theme.text_secondary()),
    );
    ui.add_space(theme.spacing_sm);
    onboarding::draw_quests(ui, theme, state);
}
