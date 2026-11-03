// Child of pages/tasks.rs (#[path]): the page's controls clicked headlessly, the way a person
// clicks them (memory feedback_ui_interactability: a control that is drawn is not yet a control
// that works). No window and no server: my own tasks are kept in memory, the board is a list set
// by the test. Seen red once on purpose; what it saw is at the test.

use super::*;
use crate::gui::screen_surface::find_text_in_shapes;
use crate::gui::mine_tasks::MineTasks;

fn frame(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, events: Vec<egui::Event>) -> egui::FullOutput {
    let time = Some(ctx.input(|i| i.time) + 0.5);
    let size = egui::vec2(1400.0, 1100.0);
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), time, events, ..Default::default() };
    ctx.run(input, |ctx| draw(ctx, theme, state))
}

fn settled(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) -> egui::FullOutput {
    frame(ctx, theme, state, Vec::new());
    frame(ctx, theme, state, Vec::new())
}

fn drawn(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) -> bool {
    find_text_in_shapes(&settled(ctx, theme, state).shapes, text).is_some_and(|f| f.text == text)
}

/// Move the pointer onto the first `text` drawn, press and release there.
fn click(ctx: &egui::Context, theme: &Theme, state: &mut GuiState, text: &str) {
    let out = settled(ctx, theme, state);
    let pos = find_text_in_shapes(&out.shapes, text).unwrap_or_else(|| panic!("{text:?} is drawn")).rect.center();
    let m = egui::Modifiers::default();
    frame(ctx, theme, state, vec![egui::Event::PointerMoved(pos)]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: m }]);
    frame(ctx, theme, state, vec![egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: m }]);
}

fn mine_count(state: &GuiState) -> usize {
    state.mine_tasks.as_ref().map_or(0, |m| m.tasks().len())
}

/// Add to my tasks, the switch between My tasks and the board, New task and Create, Hide
/// suggestions, View and Delete, each clicked: each does what it says, and the board shows its
/// own words and no New task. Seen red 2026-11-02 with the Add button's action dropped from `apply_chain_actions`:
/// "Add to my tasks made one of my own" failed (0 tasks).
#[test]
fn the_pages_controls_do_what_they_say() {
    set_for_snapshot(Kind::Mine, None, &[]);
    let mut state = GuiState::default();
    state.profile_public_key = "me".into();
    state.mine_tasks = Some(MineTasks::in_memory("me"));
    state.board_tasks = vec![task_from_server(&serde_json::json!({ "id": 5, "title": "Translate the Library", "status": "todo" }))];
    let (ctx, theme) = crate::gui::pages::apps::tests::ctx_and_theme();

    assert!(drawn(&ctx, &theme, &mut state, t("tasks.kind_mine_who")), "My tasks says who can see them");
    click(&ctx, &theme, &mut state, t("tasks.suggested_add"));
    assert_eq!(mine_count(&state), 1, "Add to my tasks made one of my own");
    assert!(state.mine_tasks.as_ref().unwrap().tasks()[0].chain.is_some(), "linked to its chain");
    assert!(state.board_tasks.len() == 1, "and put nothing on the board");

    click(&ctx, &theme, &mut state, t("tasks.kind_board"));
    assert!(drawn(&ctx, &theme, &mut state, t("tasks.kind_board_who")), "the board says who can see it");
    assert!(drawn(&ctx, &theme, &mut state, "Translate the Library"), "and shows the server's tasks");
    assert!(!drawn(&ctx, &theme, &mut state, t("tasks.new_task")), "the board is read-only here");
    assert!(!drawn(&ctx, &theme, &mut state, t("tasks.suggested_add")), "and has no suggestions");

    click(&ctx, &theme, &mut state, t("tasks.kind_mine"));
    click(&ctx, &theme, &mut state, t("tasks.new_task"));
    assert!(state.task_show_new_form, "New task opens the form");
    state.task_new_title = "Fix the gate".into();
    click(&ctx, &theme, &mut state, t("tasks.create"));
    assert_eq!(mine_count(&state), 2, "Create keeps it as one of mine");
    assert!(!state.task_show_new_form, "and closes the form");

    // The five suggestions left fill the Todo column above my two tasks: hide them first.
    click(&ctx, &theme, &mut state, t("tasks.hide_suggestions"));
    assert!(!drawn(&ctx, &theme, &mut state, t("tasks.suggested_add")), "Hide suggestions hides them");
    click(&ctx, &theme, &mut state, t("tasks.view"));
    click(&ctx, &theme, &mut state, t("common.delete"));
    assert_eq!(mine_count(&state), 1, "Delete takes it away");
    set_for_snapshot(Kind::Mine, None, &[]);
}

/// THE PAGE SAYS WHEN MY SAVED TASKS DID NOT OPEN (the review of rung 1, 2026-11-02): a file kept
/// aside, and one that could not be read, each shows its own words on My tasks. Seen red
/// 2026-11-02 with the notice left out of `draw` (in the same run as the store's keep-aside
/// break): "the page says the file was kept aside" failed.
#[test]
fn the_page_says_when_my_saved_tasks_did_not_open() {
    set_for_snapshot(Kind::Mine, None, &[]);
    let (ctx, theme) = crate::gui::pages::apps::tests::ctx_and_theme();
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let damaged = std::env::temp_dir().join(format!("hum-tasks-test-notice-{nanos}.mine"));
    std::fs::write(&damaged, b"not a sealed file").unwrap();
    let mut state = GuiState::default();
    state.profile_public_key = "me".into();
    state.mine_tasks = Some(MineTasks::at_path_for_test(&[9u8; 32], "me", damaged.clone()));
    assert!(drawn(&ctx, &theme, &mut state, t("tasks.mine_kept_aside")), "the page says the file was kept aside");
    if let Some(crate::gui::mine_tasks::OpenProblem::KeptAside(aside)) = state.mine_tasks.as_ref().and_then(|m| m.problem()).cloned() {
        let _ = std::fs::remove_file(aside);
    }
    let folder = std::env::temp_dir().join(format!("hum-tasks-test-notice-dir-{nanos}.mine"));
    std::fs::create_dir_all(&folder).unwrap();
    state.mine_tasks = Some(MineTasks::at_path_for_test(&[9u8; 32], "me", folder.clone()));
    assert!(drawn(&ctx, &theme, &mut state, t("tasks.mine_unreadable")), "and that it could not be read");
    assert!(!drawn(&ctx, &theme, &mut state, t("tasks.mine_kept_aside")), "each with its own words");
    let _ = std::fs::remove_dir_all(&folder);
}
