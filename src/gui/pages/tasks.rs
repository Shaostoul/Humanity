//! Tasks: the not-game surface. A three-column kanban (Todo/In Progress/Done)
//! for the person's own tasks, with SUGGESTED cards at the top of Todo.
//!
//! Features: project selector, kanban columns, task cards with priority badges
//! and labels, detail panel, new task form, filter bar with search/priority/
//! assignee filters, task count per column, suggested cards.
//!
//! SUGGESTED CARDS (2026-10-30). The learn-by-doing chains
//! (`data/onboarding/quests.json`: First Steps, Water Independence, ...) are
//! suggestions at the top of the Todo column, one hollow card each
//! (`widgets::card_suggested`): a muted "Suggested" label, the chain's title and
//! one-line purpose, "1 of 4", and its steps with their ticks. They replaced a
//! narrow, tall "Learn by doing" panel right of the board, at the operator's
//! request: "Shouldn't those be cards in like the Todo section instead of a weird
//! dedicated right bar separate from the tasklist?"
//!
//! - A step's box ticks it. The ticks are kept exactly where they always were:
//!   `GuiState::onboarding_quest_progress`, keyed "chain:step", written to
//!   config.json the moment a box changes (the website keeps the same keys in
//!   localStorage, `humanity_quest_progress`).
//! - A step's title opens it: its words, Go there (the page it names) and, when
//!   the step has a written how-to (`how_to` in the file), Open the how-to.
//! - Add to my tasks makes the chain an ordinary Todo task linked to it
//!   (`GuiTask::chain`), and the card goes away while that task exists. The
//!   task's card then shows "Steps: 1 of 4" and its detail panel the steps,
//!   tickable there. A task the server sends with a `chain:<id>` label (the
//!   website's link, [`task_from_server`]) is linked the same way.
//! - The header's Hide suggestions button hides them, for this run of the app.
//!
//! These cards are built to be fed by how-tos: increment 6 of
//! docs/design/how-to.md turns the chains into how-tos, and then this page lists
//! the how-tos in progress and the Getting started ones the same way.
//!
//! Web mirror: web/pages/tasks.html + tasks-app.js (renderSuggestions), the
//! same data and the same words.

use egui::{Color32, Frame, RichText, ScrollArea};
use crate::gui::{GuiPage, GuiState, TaskPriority, TaskStatus, GuiTask};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::i18n::{t, tf};
use crate::gui::widgets::{with_hint, Hint};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

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
    /// Suggested cards shown? Default true (they are the point for a new
    /// person); someone who only wants the board hides them with the header
    /// button. Page-local on purpose: a view preference for this run, not
    /// app state.
    show_suggestions: bool,
    /// The learn-by-doing chains, read from `data/onboarding/quests.json` the
    /// first time the page draws (`load_chains`).
    chains: Rc<Vec<Chain>>,
    /// Per chain, the step whose words are open (none until a title is clicked).
    open_step: HashMap<String, String>,
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
            show_suggestions: true,
            chains: Rc::new(load_chains(&crate::data_dir())),
            open_step: HashMap::new(),
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

// ── The suggested chains ──────────────────────────────────────────────────

/// One learn-by-doing chain as this page reads it from
/// `data/onboarding/quests.json`. Read here, not through
/// `onboarding::QuestChain`, because the card needs the `purpose` line and the
/// steps' `how_to`, which only this page shows.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct Chain {
    pub id: String,
    pub title: String,
    /// The one line the card shows under the title.
    pub purpose: String,
    pub steps: Vec<ChainStep>,
}

/// One step of a chain.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ChainStep {
    pub id: String,
    pub title: String,
    /// The words shown when the step is opened.
    pub description: String,
    /// The page (a web route such as "/chat", or a full link) where the step is done.
    #[serde(default)]
    pub link: Option<String>,
    /// The written how-to (an id in data/how_to/) that does this step.
    #[serde(default)]
    pub how_to: Option<String>,
}

#[derive(serde::Deserialize)]
struct ChainFile {
    chains: Vec<Chain>,
}

/// Read the chains from `<data>/onboarding/quests.json`. Empty, with a warning
/// in the log, when the file is missing or does not parse, so the board still
/// draws.
pub(crate) fn load_chains(data_dir: &Path) -> Vec<Chain> {
    let path = data_dir.join("onboarding").join("quests.json");
    let parsed = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|s| serde_json::from_str::<ChainFile>(&s).map_err(|e| e.to_string()));
    match parsed {
        Ok(f) => f.chains,
        Err(e) => {
            log::warn!("[tasks] no suggested cards: could not read {}: {e}", path.display());
            Vec::new()
        }
    }
}

/// A step's key in `onboarding_quest_progress` (and the website's localStorage).
fn step_key(chain: &Chain, step: &ChainStep) -> String {
    format!("{}:{}", chain.id, step.id)
}

/// How many of a chain's steps are ticked.
fn steps_done(chain: &Chain, progress: &HashMap<String, bool>) -> usize {
    chain.steps.iter().filter(|s| progress.get(&step_key(chain, s)).copied().unwrap_or(false)).count()
}

/// The chains to suggest: those no task was made from yet and, while a search
/// is typed, whose title, purpose or a step's title holds it (`search` is
/// already lower case).
fn suggested<'a>(chains: &'a [Chain], tasks: &[GuiTask], search: &str) -> Vec<&'a Chain> {
    chains
        .iter()
        .filter(|c| !tasks.iter().any(|t| t.chain.as_deref() == Some(c.id.as_str())))
        .filter(|c| {
            search.is_empty()
                || c.title.to_lowercase().contains(search)
                || c.purpose.to_lowercase().contains(search)
                || c.steps.iter().any(|s| s.title.to_lowercase().contains(search))
        })
        .collect()
}

/// The prefix of the label that links a task to its learn-by-doing chain (`chain:water`). The
/// website writes it (web/pages/tasks-app.js submitTask, getTaskChain); it is a link the pages
/// read, not a tag a person wrote.
const CHAIN_LABEL: &str = "chain:";

/// A task as the server's task list sends it (`task_list` and `task_list_response`,
/// engine/frame_ws_poll.rs), made into the page's own.
///
/// The server keeps a task's labels as one string: a JSON array when the website wrote it
/// (`["water","scope:city","chain:water"]`), else words separated by commas. A `chain:<id>`
/// label becomes `GuiTask::chain` (2026-10-30), so a chain added on the website is the same
/// chain here: its task shows its steps and its suggested card goes away.
pub(crate) fn task_from_server(task: &serde_json::Value) -> GuiTask {
    // A field of the server's JSON (not words for the screen: these are the task's own).
    let field = |k: &str| task.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let status = match task.get("status").and_then(|v| v.as_str()).unwrap_or("todo") {
        "in_progress" => TaskStatus::InProgress,
        "done" => TaskStatus::Done,
        _ => TaskStatus::Todo,
    };
    let priority = match task.get("priority").and_then(|v| v.as_str()).unwrap_or("medium") {
        "low" => TaskPriority::Low,
        "high" => TaskPriority::High,
        "critical" => TaskPriority::Critical,
        _ => TaskPriority::Medium,
    };
    let raw = field("labels");
    let all: Vec<String> = match serde_json::from_str::<Vec<String>>(raw.trim()) {
        Ok(list) => list,
        Err(_) => raw.split(',').map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect(),
    };
    let chain = all.iter().find_map(|l| l.strip_prefix(CHAIN_LABEL)).filter(|id| !id.is_empty()).map(str::to_string);
    let labels: Vec<String> = all.into_iter().filter(|l| !l.starts_with(CHAIN_LABEL)).collect();
    GuiTask {
        id: task.get("id").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        title: field("title"),
        description: field("description"),
        priority,
        status,
        assignee: field("assignee"),
        labels,
        chain,
    }
}

/// The labels a task's card and detail panel show: the person's own tags. The `scope:` label the
/// website files a task under is a setting of that board, not a tag (the website hides it too,
/// tasks-app.js getNonScopeLabels), so it stays in `labels` but is not drawn.
fn shown_labels(task: &GuiTask) -> Vec<&str> {
    task.labels.iter().map(String::as_str).filter(|l| !l.starts_with("scope:")).collect()
}

/// Where a step's link leads in this app: a page of its own, a full web link
/// (opened in the person's browser), or nowhere (a website-only route such as
/// "/download").
enum Dest {
    Page(GuiPage),
    Web(String),
}

fn dest_of(link: &str) -> Option<Dest> {
    if link.starts_with("https://") || link.starts_with("http://") {
        return Some(Dest::Web(link.to_string()));
    }
    crate::gui::dispatch::page_from_id(link.trim_start_matches('/')).map(Dest::Page)
}

/// What a person did on a suggested card, or on a linked task's steps, this
/// frame. Gathered while drawing and done afterwards, when `GuiState` is free to
/// change.
enum ChainAction {
    /// Tick or untick a step (its progress key).
    Tick(String),
    /// Open a step's words (chain id, step id), or fold them (None).
    Open(String, Option<String>),
    Go(GuiPage),
    Web(String),
    HowTo(String),
    /// Make a chain one of the person's own tasks.
    Add(String),
}

/// A chain's steps: a tick box and a title each; the open step also shows its
/// words and where to do it. Shared by the suggested card and the detail panel.
fn draw_steps(
    ui: &mut egui::Ui,
    theme: &Theme,
    chain: &Chain,
    progress: &HashMap<String, bool>,
    open: Option<&str>,
    actions: &mut Vec<ChainAction>,
) {
    for step in &chain.steps {
        let key = step_key(chain, step);
        let mut done = progress.get(&key).copied().unwrap_or(false);
        let is_open = open == Some(step.id.as_str());
        ui.horizontal_top(|ui| {
            // The box is named by its step, so a screen reader says what it ticks.
            let ticked = widgets::hint::with_name(ui, &step.title, |ui| {
                with_hint(ui, "tasks.suggested_tick_hint", |ui| widgets::custom_checkbox(ui, theme, &mut done))
            });
            if ticked {
                actions.push(ChainAction::Tick(key.clone()));
            }
            // Fixed width for the words, so they WRAP inside the card rather
            // than widening it past its column (the 2026-09-27 lesson below).
            let words_w = ui.available_width().max(60.0);
            ui.vertical(|ui| {
                ui.set_max_width(words_w);
                let title = RichText::new(&step.title)
                    .size(theme.font_size_small)
                    .strong()
                    .color(if done { theme.text_muted() } else { theme.text_primary() });
                if ui.add(egui::Label::new(title).wrap().sense(egui::Sense::click())).hint("tasks.suggested_step_hint").clicked() {
                    let next = if is_open { None } else { Some(step.id.clone()) };
                    actions.push(ChainAction::Open(chain.id.clone(), next));
                }
                if is_open {
                    ui.add(
                        egui::Label::new(
                            RichText::new(&step.description)
                                .size(theme.font_size_small)
                                .color(theme.text_secondary()),
                        )
                        .wrap(),
                    );
                    let dest = step.link.as_deref().and_then(dest_of);
                    if dest.is_some() || step.how_to.is_some() {
                        ui.horizontal_wrapped(|ui| {
                            if let Some(d) = dest {
                                let go = widgets::Button::secondary(t("tasks.suggested_go"))
                                    .size(widgets::ButtonSize::Small)
                                    .hint("tasks.suggested_go_hint")
                                    .show(ui, theme);
                                if go {
                                    actions.push(match d {
                                        Dest::Page(p) => ChainAction::Go(p),
                                        Dest::Web(u) => ChainAction::Web(u),
                                    });
                                }
                            }
                            if let Some(h) = &step.how_to {
                                let open_how_to = widgets::Button::secondary(t("tasks.suggested_how_to"))
                                    .size(widgets::ButtonSize::Small)
                                    .hint("tasks.suggested_how_to_hint")
                                    .show(ui, theme);
                                if open_how_to {
                                    actions.push(ChainAction::HowTo(h.clone()));
                                }
                            }
                        });
                    }
                }
            });
        });
        ui.add_space(theme.spacing_xs);
    }
}

/// A thin progress bar: `done` of `total`, in the accent colour.
fn thin_bar(ui: &mut egui::Ui, theme: &Theme, done: usize, total: usize) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 4.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let r = egui::Rounding::same(2);
        // The track in the card's border colour, so an empty bar still shows
        // (bg_tertiary vanished against the hollow card).
        ui.painter().rect_filled(rect, r, theme.border());
        if total > 0 && done > 0 {
            let mut fill = rect;
            fill.set_width(rect.width() * done as f32 / total as f32);
            ui.painter().rect_filled(fill, r, theme.accent());
        }
    }
}

/// One suggested card: the "Suggested" label and the count, the title, the
/// purpose, the bar, the steps, then Add to my tasks.
fn draw_suggestion(
    ui: &mut egui::Ui,
    theme: &Theme,
    chain: &Chain,
    progress: &HashMap<String, bool>,
    open: Option<&str>,
    actions: &mut Vec<ChainAction>,
) {
    let total = chain.steps.len();
    let done = steps_done(chain, progress);
    widgets::card_suggested(ui, theme, |ui| {
        ui.horizontal(|ui| {
            widgets::card_suggested_label(ui, theme, t("tasks.suggested"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (count, color) = if done == total {
                    (tf("tasks.suggested_all_done", &[("total", &total)]), theme.success())
                } else {
                    (tf("tasks.suggested_progress", &[("done", &done), ("total", &total)]), theme.text_muted())
                };
                ui.label(RichText::new(count).size(theme.font_size_small).color(color));
            });
        });
        ui.add(
            egui::Label::new(
                RichText::new(&chain.title).size(theme.font_size_body).strong().color(theme.text_primary()),
            )
            .wrap(),
        );
        ui.add(
            egui::Label::new(
                RichText::new(&chain.purpose).size(theme.font_size_small).color(theme.text_secondary()),
            )
            .wrap(),
        );
        ui.add_space(theme.spacing_xs);
        thin_bar(ui, theme, done, total);
        ui.add_space(theme.spacing_sm);
        draw_steps(ui, theme, chain, progress, open, actions);
        ui.add_space(theme.spacing_xs);
        let add = widgets::Button::secondary(t("tasks.suggested_add"))
            .hint("tasks.suggested_add_hint")
            .show(ui, theme);
        if add {
            actions.push(ChainAction::Add(chain.id.clone()));
        }
    });
}

/// Do what the person did on the cards this frame.
fn apply_chain_actions(ctx: &egui::Context, state: &mut GuiState, chains: &[Chain], actions: Vec<ChainAction>) {
    for action in actions {
        match action {
            ChainAction::Tick(key) => {
                let now = !state.onboarding_quest_progress.get(&key).copied().unwrap_or(false);
                state.onboarding_quest_progress.insert(key, now);
                // Written at once, as the guide panel did since v0.1066: a tick
                // kept only in memory was blank again on the next launch.
                crate::config::AppConfig::from_gui_state(state).save();
            }
            ChainAction::Open(chain, step) => with_state(|ps| match step {
                Some(s) => {
                    ps.open_step.insert(chain, s);
                }
                None => {
                    ps.open_step.remove(&chain);
                }
            }),
            // "Create your first task" names this very page: open the form.
            ChainAction::Go(GuiPage::Tasks) => state.task_show_new_form = true,
            ChainAction::Go(page) => state.push_nav_to(page),
            ChainAction::Web(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
            ChainAction::HowTo(id) => {
                state.how_to.open(&id);
                state.push_nav_to(GuiPage::HowTo);
            }
            ChainAction::Add(id) => {
                let Some(chain) = chains.iter().find(|c| c.id == id) else { continue };
                // Never twice: a second click in the same frame, or a task made
                // already, adds nothing.
                if state.tasks.iter().any(|t| t.chain.as_deref() == Some(id.as_str())) {
                    continue;
                }
                state.tasks.push(GuiTask {
                    id: state.task_next_id,
                    title: chain.title.clone(),
                    description: chain.purpose.clone(),
                    priority: TaskPriority::Medium,
                    status: TaskStatus::Todo,
                    assignee: String::new(),
                    labels: Vec::new(),
                    chain: Some(chain.id.clone()),
                });
                state.task_next_id += 1;
            }
        }
    }
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
        TaskPriority::Low => t("tasks.priority_low"),
        TaskPriority::Medium => t("tasks.priority_medium"),
        TaskPriority::High => t("tasks.priority_high"),
        TaskPriority::Critical => t("tasks.priority_critical"),
    }
}

fn status_label(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Todo => t("tasks.status_todo"),
        TaskStatus::InProgress => t("tasks.status_in_progress"),
        TaskStatus::Done => t("tasks.status_done"),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    // Track which task to select (set by card "View" buttons)
    let mut select_task: Option<usize> = None;
    let mut close_detail = false;
    let mut apply_edit: Option<(usize, TaskStatus, TaskPriority)> = None;
    let mut actions: Vec<ChainAction> = Vec::new();
    let chains = with_state(|ps| ps.chains.clone());

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
                                ui.label(RichText::new(t("tasks.detail_title")).size(theme.font_size_heading).color(theme.text_primary()));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if with_hint(ui, "tasks.close_detail_hint", |ui| widgets::secondary_button(ui, theme, "X")) {
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
                                ui.label(RichText::new(t("tasks.status_label")).color(theme.text_secondary()));
                                if ps.editing {
                                    egui::ComboBox::from_id_salt("edit_status")
                                        .selected_text(status_label(ps.edit_status))
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for s in [TaskStatus::Todo, TaskStatus::InProgress, TaskStatus::Done] {
                                                if ui.selectable_label(ps.edit_status == s, status_label(s)).hint("tasks.status_option_hint").clicked() {
                                                    ps.edit_status = s;
                                                }
                                            }
                                        })
                                        .response
                                        .hint("tasks.edit_status_hint");
                                } else {
                                    ui.label(RichText::new(status_label(task.status)).color(theme.text_primary()));
                                }
                            });

                            // Priority (editable)
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(t("tasks.priority_label")).color(theme.text_secondary()));
                                if ps.editing {
                                    egui::ComboBox::from_id_salt("edit_priority")
                                        .selected_text(priority_label(ps.edit_priority))
                                        .width(120.0)
                                        .show_ui(ui, |ui| {
                                            for p in [TaskPriority::Low, TaskPriority::Medium, TaskPriority::High, TaskPriority::Critical] {
                                                if ui.selectable_label(ps.edit_priority == p, priority_label(p)).hint("tasks.priority_option_hint").clicked() {
                                                    ps.edit_priority = p;
                                                }
                                            }
                                        })
                                        .response
                                        .hint("tasks.edit_priority_hint");
                                } else {
                                    ui.label(RichText::new(priority_label(task.priority)).color(theme.text_primary()));
                                }
                            });

                            // Assignee
                            if !task.assignee.is_empty() {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(t("tasks.assignee_label")).color(theme.text_secondary()));
                                    ui.label(RichText::new(&task.assignee).color(theme.text_primary()));
                                });
                            }

                            ui.add_space(theme.spacing_sm);

                            // Full description
                            if !task.description.is_empty() {
                                ui.label(RichText::new(t("tasks.description")).size(theme.font_size_body).color(theme.text_secondary()));
                                widgets::card(ui, theme, |ui| {
                                    ui.label(RichText::new(&task.description).color(theme.text_primary()));
                                });
                            }

                            // A task made from a suggested card: its chain's
                            // steps, tickable here as on the card.
                            if let Some(chain) = task.chain.as_deref().and_then(|id| chains.iter().find(|c| c.id == id)) {
                                ui.add_space(theme.spacing_sm);
                                ui.label(RichText::new(t("tasks.detail_steps")).size(theme.font_size_body).color(theme.text_secondary()));
                                ui.add_space(theme.spacing_xs);
                                let open = ps.open_step.get(&chain.id).map(String::as_str);
                                draw_steps(ui, theme, chain, &state.onboarding_quest_progress, open, &mut actions);
                            }

                            ui.add_space(theme.spacing_xs);

                            // Labels
                            let shown = shown_labels(task);
                            if !shown.is_empty() {
                                ui.label(RichText::new(t("tasks.labels")).size(theme.font_size_body).color(theme.text_secondary()));
                                ui.horizontal_wrapped(|ui| {
                                    for label in shown {
                                        widgets::badge_sm(ui, theme, label, Theme::c32(&theme.info));
                                    }
                                });
                            }

                            ui.add_space(theme.spacing_sm);

                            // Task ID
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(t("tasks.task_id_label")).color(theme.text_muted()));
                                ui.label(RichText::new(format!("#{}", task.id)).color(theme.text_muted()));
                            });

                            ui.add_space(theme.spacing_md);

                            // Comments section
                            ui.label(RichText::new(t("tasks.comments")).size(theme.font_size_body).color(theme.text_secondary()));
                            widgets::card(ui, theme, |ui| {
                                ui.label(RichText::new(t("tasks.no_comments")).color(theme.text_muted()));
                            });

                            ui.add_space(theme.spacing_md);

                            // Edit / Save buttons
                            ui.horizontal(|ui| {
                                if ps.editing {
                                    if widgets::Button::primary(t("common.save")).hint("tasks.save_edit_hint").show(ui, theme) {
                                        apply_edit = Some((sel_idx, ps.edit_status, ps.edit_priority));
                                        ps.editing = false;
                                    }
                                    if widgets::Button::secondary(t("common.cancel")).hint("tasks.cancel_edit_hint").show(ui, theme) {
                                        ps.editing = false;
                                    }
                                } else if widgets::Button::primary(t("common.edit")).hint("tasks.edit_hint").show(ui, theme) {
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
                ui.label(RichText::new(t("tasks.title")).size(theme.font_size_title).color(theme.text_primary()));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if widgets::Button::primary(t("tasks.new_task")).hint("tasks.new_task_hint").show(ui, theme) {
                        state.task_show_new_form = !state.task_show_new_form;
                    }
                    // Suggested-cards toggle. Right-to-left layout puts this
                    // just left of the New Task button.
                    with_state(|ps| {
                        let label = if ps.show_suggestions { t("tasks.hide_suggestions") } else { t("tasks.show_suggestions") };
                        if with_hint(ui, "tasks.suggestions_hint", |ui| widgets::secondary_button(ui, theme, label)) {
                            ps.show_suggestions = !ps.show_suggestions;
                        }
                    });
                });
            });

            ui.add_space(theme.spacing_xs);

            // Project selector bar
            with_state(|ps| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(t("tasks.project_label")).color(theme.text_secondary()));
                    let current_project = if ps.project_filter.is_empty() { t("tasks.all_projects") } else { &ps.project_filter };
                    egui::ComboBox::from_id_salt("project_selector")
                        .selected_text(current_project)
                        .width(160.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(ps.project_filter.is_empty(), t("tasks.all_projects")).hint("tasks.all_projects_hint").clicked() {
                                ps.project_filter.clear();
                            }
                            for proj in &ps.projects {
                                if ui.selectable_label(ps.project_filter == *proj, proj).hint("tasks.project_option_hint").clicked() {
                                    ps.project_filter = proj.clone();
                                }
                            }
                        })
                        .response
                        .hint("tasks.project_filter_hint");
                });
            });

            ui.add_space(theme.spacing_xs);

            // Filter bar
            with_hint(ui, "tasks.filter_tasks_hint", |ui| widgets::search_bar(ui, theme, &mut state.task_search, t("tasks.filter_tasks")));
            ui.horizontal(|ui| {
                ui.label(RichText::new(t("tasks.priority_label")).color(theme.text_secondary()));
                let current_label = state.task_filter_priority.map_or(t("tasks.all"), priority_label);
                egui::ComboBox::from_id_salt("priority_filter")
                    .selected_text(current_label)
                    .width(100.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(state.task_filter_priority.is_none(), t("tasks.all")).hint("tasks.priority_all_hint").clicked() {
                            state.task_filter_priority = None;
                        }
                        for p in [TaskPriority::Low, TaskPriority::Medium, TaskPriority::High, TaskPriority::Critical] {
                            if ui.selectable_label(state.task_filter_priority == Some(p), priority_label(p)).hint("tasks.priority_filter_option_hint").clicked() {
                                state.task_filter_priority = Some(p);
                            }
                        }
                    })
                    .response
                    .hint("tasks.priority_filter_hint");
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new(t("tasks.assignee_label")).color(theme.text_secondary()));
                ui.add(egui::TextEdit::singleline(&mut state.task_filter_assignee)
                    .desired_width(120.0)
                    .hint_text(t("tasks.filter_assignee")))
                    .hint("tasks.filter_assignee_hint");
            });

            ui.add_space(theme.spacing_sm);

            // New task form (inline)
            if state.task_show_new_form {
                widgets::card(ui, theme, |ui| {
                    ui.label(RichText::new(t("tasks.new_task_heading")).size(theme.font_size_heading).color(theme.accent()));
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("tasks.title_label")).color(theme.text_secondary()));
                        ui.add(egui::TextEdit::singleline(&mut state.task_new_title).desired_width(300.0))
                            .hint("tasks.new_title_hint");
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("tasks.description_label")).color(theme.text_secondary()));
                        ui.add(egui::TextEdit::multiline(&mut state.task_new_description)
                            .desired_width(300.0)
                            .desired_rows(2))
                            .hint("tasks.new_description_hint");
                    });
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(t("tasks.priority_label")).color(theme.text_secondary()));
                        egui::ComboBox::from_id_salt("new_task_priority")
                            .selected_text(priority_label(state.task_new_priority))
                            .show_ui(ui, |ui| {
                                for p in [TaskPriority::Low, TaskPriority::Medium, TaskPriority::High, TaskPriority::Critical] {
                                    if ui.selectable_label(state.task_new_priority == p, priority_label(p)).hint("tasks.new_priority_option_hint").clicked() {
                                        state.task_new_priority = p;
                                    }
                                }
                            })
                            .response
                            .hint("tasks.new_priority_hint");
                        ui.add_space(theme.spacing_md);
                        ui.label(RichText::new(t("tasks.assignee_label")).color(theme.text_secondary()));
                        ui.add(egui::TextEdit::singleline(&mut state.task_new_assignee).desired_width(120.0))
                            .hint("tasks.new_assignee_hint");
                    });
                    with_state(|ps| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(t("tasks.labels_label")).color(theme.text_secondary()));
                            ui.add(egui::TextEdit::singleline(&mut ps.new_labels_input)
                                .desired_width(200.0)
                                .hint_text(t("tasks.labels_placeholder")))
                                .hint("tasks.new_labels_hint");
                            ui.add_space(theme.spacing_md);
                            ui.label(RichText::new(t("tasks.project_label")).color(theme.text_secondary()));
                            let projects_clone = ps.projects.clone();
                            egui::ComboBox::from_id_salt("new_task_project")
                                .selected_text(if ps.new_project.is_empty() { t("tasks.project_none") } else { &ps.new_project })
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(ps.new_project.is_empty(), t("tasks.project_none")).hint("tasks.new_project_none_hint").clicked() {
                                        ps.new_project.clear();
                                    }
                                    for proj in &projects_clone {
                                        if ui.selectable_label(ps.new_project == *proj, proj).hint("tasks.new_project_option_hint").clicked() {
                                            ps.new_project = proj.clone();
                                        }
                                    }
                                })
                                .response
                                .hint("tasks.new_project_hint");
                        });
                    });
                    ui.add_space(theme.spacing_xs);
                    ui.horizontal(|ui| {
                        if widgets::Button::primary(t("tasks.create")).hint("tasks.create_hint").show(ui, theme) && !state.task_new_title.is_empty() {
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
                                chain: None,
                            };
                            state.task_next_id += 1;
                            state.tasks.push(task);
                            state.task_new_title.clear();
                            state.task_new_description.clear();
                            state.task_new_priority = TaskPriority::Medium;
                            state.task_new_assignee.clear();
                            state.task_show_new_form = false;
                        }
                        if widgets::Button::secondary(t("common.cancel")).hint("tasks.cancel_new_hint").show(ui, theme) {
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
                .filter(|(_, task)| {
                    if !search_lower.is_empty() && !task.title.to_lowercase().contains(&search_lower)
                        && !task.description.to_lowercase().contains(&search_lower) {
                        return false;
                    }
                    if let Some(pf) = state.task_filter_priority {
                        if task.priority != pf { return false; }
                    }
                    if !assignee_lower.is_empty() && !task.assignee.to_lowercase().contains(&assignee_lower) {
                        return false;
                    }
                    if !project_filter.is_empty() {
                        if !task.labels.iter().any(|l| l == &project_filter) { return false; }
                    }
                    true
                })
                .map(|(i, _)| i)
                .collect();

            // The suggestions: hidden by the header button, and while a
            // priority, assignee or project filter is set (a suggestion has
            // none of those, so it can never match one). The search box
            // filters them like tasks.
            let show = with_state(|ps| ps.show_suggestions);
            let filtering = state.task_filter_priority.is_some() || !assignee_lower.is_empty() || !project_filter.is_empty();
            let suggestions = if show && !filtering {
                suggested(&chains, &state.tasks, &search_lower)
            } else {
                Vec::new()
            };
            let open = with_state(|ps| ps.open_step.clone());
            let board = Board {
                filtered: &filtered,
                chains: &chains,
                suggestions: &suggestions,
                open: &open,
            };
            draw_board(ui, theme, state, &board, &mut select_task, &mut actions);
        });

    apply_chain_actions(ctx, state, &chains, actions);

    // Apply card selection
    if let Some(idx) = select_task {
        with_state(|ps| {
            ps.selected_task = Some(idx);
            ps.editing = false;
        });
    }
}

/// What the board draws besides `GuiState`: the tasks that pass the filters,
/// the chains (for a linked task's steps line) and the suggestions to show.
struct Board<'a> {
    filtered: &'a [usize],
    chains: &'a [Chain],
    suggestions: &'a [&'a Chain],
    open: &'a HashMap<String, String>,
}

/// The three-column kanban itself, with the suggested cards at the top of Todo.
fn draw_board(
    ui: &mut egui::Ui,
    theme: &Theme,
    state: &GuiState,
    board: &Board,
    select_task: &mut Option<usize>,
    actions: &mut Vec<ChainAction>,
) {
    // (scroll id, status): the shown name comes from `status_label`, by key,
    // so the id stays the same whatever language the words are in.
    let columns = [
        ("task_col_todo", TaskStatus::Todo),
        ("task_col_in_progress", TaskStatus::InProgress),
        ("task_col_done", TaskStatus::Done),
    ];
    let progress = &state.onboarding_quest_progress;

    ui.columns(3, |cols| {
        for (col_idx, (col_id, col_status)) in columns.iter().enumerate() {
            cols[col_idx].vertical(|ui| {
                let col_tasks: Vec<usize> = board.filtered.iter()
                    .copied()
                    .filter(|&i| state.tasks[i].status == *col_status)
                    .collect();

                // Column header with count (the person's own tasks only)
                ui.horizontal(|ui| {
                    ui.label(RichText::new(status_label(*col_status)).size(theme.font_size_heading).color(theme.text_primary()));
                    widgets::badge(ui, theme, &col_tasks.len().to_string(), theme.bg_secondary());
                });
                ui.add_space(theme.spacing_xs);
                ui.separator();
                ui.add_space(theme.spacing_xs);

                // Each kanban column needs its OWN ScrollArea id — they
                // live inside ui.columns(3,..) which gives each the same
                // auto-id, so egui flags a duplicate-id clash and the
                // columns can stop scrolling independently. Salt by column.
                let scrolled = ScrollArea::vertical().id_salt(*col_id).show(ui, |ui| {
                    // Suggestions first, above the person's own tasks.
                    if *col_status == TaskStatus::Todo {
                        for chain in board.suggestions {
                            let open = board.open.get(&chain.id).map(String::as_str);
                            draw_suggestion(ui, theme, chain, progress, open, actions);
                            ui.add_space(theme.spacing_xs);
                        }
                    }
                    if col_tasks.is_empty() {
                        ui.label(RichText::new(t("tasks.no_tasks_column")).color(theme.text_muted()));
                    }
                    for &idx in &col_tasks {
                        let task = &state.tasks[idx];
                        let pc = priority_color(theme, task.priority);
                        widgets::card(ui, theme, |ui| {
                            // The title WRAPS in the room left beside the
                            // priority badge, which keeps the top-right corner.
                            // As a plain label in a horizontal row it could not
                            // wrap: the card grew past its column and the
                            // "Medium" badge sat in the Done column (2026-09-27
                            // snapshot review).
                            ui.horizontal_top(|ui| {
                                let prio = priority_label(task.priority);
                                let badge_w = ui
                                    .fonts(|f| {
                                        f.layout_no_wrap(
                                            prio.to_string(),
                                            egui::FontId::proportional(theme.small_size),
                                            theme.text_primary(),
                                        )
                                    })
                                    .size()
                                    .x
                                    + 8.0 // badge_sm's inner margin, 4 px a side
                                    + ui.spacing().item_spacing.x;
                                let title_w = (ui.available_width() - badge_w).max(60.0);
                                ui.scope(|ui| {
                                    ui.set_max_width(title_w);
                                    ui.add(
                                        egui::Label::new(
                                            RichText::new(&task.title)
                                                .size(theme.font_size_body)
                                                .color(theme.text_primary()),
                                        )
                                        .wrap(),
                                    );
                                });
                                widgets::badge_sm(ui, theme, prio, pc);
                            });
                            // Description preview (first 80 chars)
                            if !task.description.is_empty() {
                                let preview: String = task.description.chars().take(80).collect();
                                let suffix = if task.description.chars().count() > 80 { "..." } else { "" };
                                ui.label(RichText::new(format!("{}{}", preview, suffix)).color(theme.text_muted()).size(theme.font_size_small));
                            }
                            // A task made from a suggested card: how far along its steps are.
                            if let Some(chain) = task.chain.as_deref().and_then(|id| board.chains.iter().find(|c| c.id == id)) {
                                let done = steps_done(chain, progress);
                                let line = tf("tasks.card_steps", &[("done", &done), ("total", &chain.steps.len())]);
                                ui.label(RichText::new(line).color(theme.text_secondary()).size(theme.font_size_small));
                            }
                            if !task.assignee.is_empty() {
                                ui.label(RichText::new(tf("tasks.card_assignee", &[("name", &task.assignee)])).color(theme.text_secondary()).size(theme.font_size_small));
                            }
                            // Labels as small badges
                            let shown = shown_labels(task);
                            if !shown.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    for label in shown {
                                        widgets::badge_sm(ui, theme, label, Theme::c32(&theme.info));
                                    }
                                });
                            }
                            // Task ID
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("#{}", task.id)).color(theme.text_muted()).size(theme.font_size_small));
                                if widgets::Button::secondary(t("tasks.view")).hint("tasks.view_hint").show(ui, theme) {
                                    *select_task = Some(idx);
                                }
                            });
                        });
                        ui.add_space(theme.spacing_xs);
                    }
                });
                // A column taller than the window scrolls, and egui then makes
                // its scroll bar a Tab stop with no name: give it one.
                widgets::hint::name_scroll_area(ui, scrolled.id, t("tasks.column_scroll"), "tasks.column_scroll_hint");
            });
        }
    });

    // Empty state. Not while suggestions fill the Todo column: they are the
    // empty state then, and this line would sit under them, off the screen.
    if state.tasks.is_empty() && board.suggestions.is_empty() {
        ui.add_space(theme.spacing_lg);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(t("tasks.empty_title")).size(theme.font_size_heading).color(theme.text_muted()));
            ui.label(RichText::new(t("tasks.empty_body")).color(theme.text_secondary()));
        });
    }
}

/// Snapshot helper: choose (or clear) the task whose detail panel is open, and
/// open a chain's step, so a still can show the linked steps
/// (`ui_snapshots::snapshot_tasks_suggested`).
#[cfg(test)]
pub(crate) fn set_for_snapshot(selected: Option<usize>, open: &[(&str, &str)]) {
    with_state(|ps| {
        ps.selected_task = selected;
        ps.editing = false;
        ps.show_suggestions = true;
        ps.open_step = open.iter().map(|(c, s)| (c.to_string(), s.to_string())).collect();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chains() -> Vec<Chain> {
        load_chains(Path::new(env!("CARGO_MANIFEST_DIR")).join("data").as_path())
    }

    fn task(chain: Option<&str>) -> GuiTask {
        GuiTask {
            id: 1,
            title: "x".into(),
            description: String::new(),
            priority: TaskPriority::Medium,
            status: TaskStatus::Todo,
            assignee: String::new(),
            labels: Vec::new(),
            chain: chain.map(str::to_string),
        }
    }

    /// The shipped chains parse as suggestions: every chain has its one-line
    /// purpose and at least one step. A missing `purpose` would empty the
    /// whole Todo column's suggestions with only a log line to say why.
    #[test]
    fn shipped_chains_load_as_suggestions() {
        let c = chains();
        assert!(c.len() >= 6, "expected the six shipped chains, got {}", c.len());
        for chain in &c {
            assert!(!chain.purpose.trim().is_empty(), "{} has no purpose line", chain.id);
            assert!(!chain.steps.is_empty(), "{} has no steps", chain.id);
        }
        assert_eq!(c[0].id, "first-steps", "First Steps leads the column");
    }

    /// Every step's `how_to` names a WRITTEN how-to, so Open the how-to never
    /// lands on a planned one or a typo. Proven able to fail by the bad id.
    #[test]
    fn step_how_tos_are_written_how_tos() {
        let index: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("data/how_to/index.json")).unwrap(),
        )
        .unwrap();
        let written: std::collections::HashSet<String> = index["how_tos"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|h| h["id"].as_str().map(str::to_string))
            .collect();
        assert!(!written.contains("no_such_how_to"), "the check can fail");
        let mut linked = 0;
        for chain in chains() {
            for step in &chain.steps {
                if let Some(h) = &step.how_to {
                    assert!(written.contains(h), "{}:{} names {h}, which is not a written how-to", chain.id, step.id);
                    linked += 1;
                }
            }
        }
        assert!(linked >= 2, "the recovery-phrase and boiled-water steps link their how-tos");
    }

    /// A chain made into a task is no longer suggested; the others still are.
    #[test]
    fn an_added_chain_is_not_suggested() {
        let c = chains();
        let all = suggested(&c, &[], "");
        assert_eq!(all.len(), c.len());
        let after = suggested(&c, &[task(Some("water"))], "");
        assert_eq!(after.len(), c.len() - 1);
        assert!(after.iter().all(|x| x.id != "water"));
        // An ordinary task hides nothing.
        assert_eq!(suggested(&c, &[task(None)], "").len(), c.len());
    }

    /// A chain added on the website is the same chain here (2026-10-30). The website links its
    /// task to the chain by a `chain:<id>` label, and the server keeps a task's labels as one
    /// JSON-array string; the desktop read that string by splitting on commas, so the task
    /// showed raw tags (`["water"`, `"chain:water"]`) and the card for its chain was still
    /// suggested. Plain comma labels (an older or hand-written task) still read as before.
    ///
    /// Seen red 2026-10-30: the labels came out as `["[\"water\""...` and `chain` as None.
    #[test]
    fn a_chain_added_on_the_website_is_known_here() {
        let from_web = serde_json::json!({
            "id": 7, "title": "Water Independence", "status": "todo", "priority": "medium",
            "labels": "[\"water\",\"scope:city\",\"chain:water\"]",
        });
        let t = task_from_server(&from_web);
        assert_eq!(t.chain.as_deref(), Some("water"), "the chain label links the task");
        assert_eq!(t.labels, vec!["water".to_string(), "scope:city".to_string()], "the link is not a tag");
        assert_eq!(shown_labels(&t), vec!["water"], "and neither is the scope the website keeps");
        let c = chains();
        assert!(suggested(&c, &[t], "").iter().all(|x| x.id != "water"), "its card is no longer suggested");
        // Comma labels, and none at all.
        let plain = task_from_server(&serde_json::json!({ "id": 8, "title": "x", "labels": "garden, food" }));
        assert_eq!(plain.labels, vec!["garden".to_string(), "food".to_string()]);
        assert!(plain.chain.is_none());
        assert!(task_from_server(&serde_json::json!({ "id": 9, "title": "y", "labels": "[]" })).labels.is_empty());
        assert!(task_from_server(&serde_json::json!({ "id": 10, "title": "z" })).labels.is_empty());
    }

    /// The search box filters suggestions by title, purpose and step titles.
    #[test]
    fn search_filters_suggestions() {
        let c = chains();
        let water: Vec<&str> = suggested(&c, &[], "water").iter().map(|x| x.id.as_str()).collect();
        assert!(water.contains(&"water"), "{water:?}");
        assert!(suggested(&c, &[], "recovery phrase").iter().any(|x| x.id == "first-steps"));
        assert!(suggested(&c, &[], "zzzz no such words").is_empty());
    }

    /// Progress counts only the ticked steps of that chain, by the same
    /// "chain:step" keys the guide panel always used.
    #[test]
    fn steps_done_reads_the_old_keys() {
        let c = chains();
        let water = c.iter().find(|x| x.id == "water").unwrap();
        let mut p = HashMap::new();
        assert_eq!(steps_done(water, &p), 0);
        p.insert("water:filtration".to_string(), true);
        p.insert("water:storage".to_string(), false);
        p.insert("food:share-surplus".to_string(), true);
        assert_eq!(steps_done(water, &p), 1);
    }

    /// Step links resolve: app routes to pages, full links to the browser, and
    /// a website-only route to nothing.
    #[test]
    fn step_links_resolve() {
        assert!(matches!(dest_of("/chat"), Some(Dest::Page(GuiPage::Chat))));
        assert!(matches!(dest_of("/settings"), Some(Dest::Page(GuiPage::Settings))));
        assert!(matches!(dest_of("/tasks"), Some(Dest::Page(GuiPage::Tasks))));
        assert!(matches!(dest_of("https://example.org/x"), Some(Dest::Web(_))));
        assert!(dest_of("/download").is_none());
        // Every link in the shipped chains is one of the three, never a typo'd page.
        for chain in chains() {
            for step in &chain.steps {
                if let Some(l) = &step.link {
                    if dest_of(l).is_none() {
                        assert_eq!(l, "/download", "{}:{} links {l}, which no page answers", chain.id, step.id);
                    }
                }
            }
        }
    }
}
