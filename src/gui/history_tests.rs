// Child of gui/history.rs (#[path]): Back and Forward on the desktop (navigation rung 4,
// docs/design/navigation.md section 10), driven the way a person does: pages and the places inside
// them, the cap, a popup closed first, Alt+Left inside a text box, the mouse's side buttons on a
// page and in the world, Escape's rules, and the world never a step. The frames run headless, as
// lib.rs runs them: the history first (`frame`), then the page.
//
// RED FIRST (2026-11-04). All ten were seen failing in one run against a history broken nine ways
// on purpose (then put back), each on its own broken part, saying:
// - `record` keeping the steps ahead (no `truncate`) and never dropping the oldest:
//   back_and_forward_walk_pages_like_a_browser "going somewhere new drops the steps ahead, left:
//   [Chat, Files, Maps, Settings], right: [Chat, Files, Settings]";
//   the_history_keeps_the_last_hundred_steps "left: 150, right: 100".
// - `apply` ignoring a Settings section: a_settings_section_is_a_step "left: None, right:
//   Some(Graphics)" (nothing scrolled to the section chosen).
// - `observed_at` seeing nothing: a_chat_channel_is_a_step "a channel clicked is a step, left: 1,
//   right: 2"; a_files_place_is_a_step "a Files place chosen is a step, left: 1, right: 2".
// - `close_popup` always false: back_closes_an_open_popup_first "Back closes the popup first, the
//   page stays, left: Chat, right: Files".
// - `frame` taking Alt+Left whatever has the keyboard: alt_left_in_a_text_box_moves_its_cursor
//   "Alt+Left inside a text box is its word jump, not Back, left: Chat, right: Files".
// - `frame` reading the side buttons in the world too (its early return gone):
//   the_side_buttons_go_back_and_forward_on_a_page_and_do_nothing_in_the_world "in the world the
//   back button does nothing, left: Files, right: None".
// - `escape_target` ignoring `from` (any step behind a step out): escape_backs_out_of_a_page_
//   opened_from_another "Escape on a page reached from the menu goes into the world, not Back,
//   left: Chat, right: None".
// - `enter_world` recording a step: the_world_is_never_a_step "entering the world adds no step,
//   left: 3, right: 2".

use super::*;
use crate::gui::theme::Theme;
use egui::{Event, PointerButton};

/// A GuiState on `pages`, each opened from the menu in turn.
fn on(pages: &[GuiPage]) -> GuiState {
    let mut s = GuiState::default();
    for p in pages {
        s.go(*p, How::Beside);
    }
    s
}

fn pages(s: &GuiState) -> Vec<GuiPage> {
    s.history.places().map(|p| p.page).collect()
}

/// Frames headless, the history first, then what the test draws as the page.
struct App {
    ctx: egui::Context,
    theme: Theme,
    state: GuiState,
    t: f64,
}

impl App {
    fn new(state: GuiState) -> Self {
        let ctx = egui::Context::default();
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        ctx.all_styles_mut(|s| s.animation_time = 0.0);
        App { ctx, theme, state, t: 1.0 }
    }

    fn frame(&mut self, events: Vec<Event>, mut page: impl FnMut(&egui::Context, &Theme, &mut GuiState)) {
        self.t += 1.0 / 60.0;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 720.0))),
            time: Some(self.t),
            events,
            ..Default::default()
        };
        let (theme, state) = (&self.theme, &mut self.state);
        let _ = self.ctx.run(input, |ctx| {
            frame(ctx, state);
            page(ctx, theme, state);
        });
    }

    /// A press of one of the mouse's buttons, then its release, over an empty spot.
    fn button(&mut self, button: PointerButton) {
        let pos = egui::pos2(900.0, 600.0);
        let m = egui::Modifiers::default();
        self.frame(vec![Event::PointerMoved(pos), Event::PointerButton { pos, button, pressed: true, modifiers: m }], |_, _, _| {});
        self.frame(vec![Event::PointerButton { pos, button, pressed: false, modifiers: m }], |_, _, _| {});
    }
}

/// A page with one closable dialog on it (widgets::dialog), open while `open` is.
fn popup(open: &mut bool) -> impl FnMut(&egui::Context, &Theme, &mut GuiState) + '_ {
    move |ctx, theme, _| {
        crate::gui::widgets::dialog(ctx, theme, "history_test_popup", "A popup", open, |ui| {
            ui.label("Inside");
        });
    }
}

/// A page with one text box, given the keyboard on the first frame `focus` is set.
fn text_box<'a>(text: &'a mut String, focus: &'a mut bool, id: egui::Id) -> impl FnMut(&egui::Context, &Theme, &mut GuiState) + 'a {
    move |ctx, _, _| {
        egui::CentralPanel::default().show(ctx, |ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut *text).id(id));
            if std::mem::take(&mut *focus) {
                r.request_focus();
            }
        });
    }
}

fn alt_key(key: egui::Key) -> Vec<Event> {
    let m = egui::Modifiers::ALT;
    vec![
        Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: m },
        Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: m },
    ]
}

/// Browser semantics (section 10.2): Back and Forward move along the steps; going somewhere new
/// drops the steps ahead; the place you are on adds nothing.
#[test]
fn back_and_forward_walk_pages_like_a_browser() {
    let mut s = on(&[GuiPage::Chat, GuiPage::Files, GuiPage::Maps]);
    assert_eq!(pages(&s), vec![GuiPage::Chat, GuiPage::Files, GuiPage::Maps]);
    assert!(s.back() && s.active_page == GuiPage::Files);
    assert!(s.back() && s.active_page == GuiPage::Chat);
    assert!(!s.back(), "nothing before the first step");
    assert!(s.forward() && s.active_page == GuiPage::Files);
    s.go(GuiPage::Settings, How::Beside);
    assert_eq!(pages(&s), vec![GuiPage::Chat, GuiPage::Files, GuiPage::Settings], "going somewhere new drops the steps ahead");
    assert!(!s.forward(), "and there is nothing to go forward to");
    s.go(GuiPage::Settings, How::Beside);
    assert_eq!(s.history.len(), 3, "the place you are already on adds nothing");
}

/// A Settings section chosen is a step; Back scrolls to the one before. Scrolling, which moves
/// the open section, is not a step.
#[test]
fn a_settings_section_is_a_step() {
    let mut s = on(&[GuiPage::Chat]);
    s.go_to(Place::settings(SettingsCategory::Audio), How::Beside);
    s.go_to(Place::settings(SettingsCategory::Graphics), How::Beside);
    assert_eq!(s.settings.scroll_to_section, Some(SettingsCategory::Graphics));
    s.settings.scroll_to_section = None; // the page scrolled there
    s.settings.category = SettingsCategory::Privacy; // and the person scrolled on
    s.note_place();
    assert_eq!(s.history.len(), 3, "scrolling is not a step");
    assert!(s.back());
    assert_eq!((s.active_page, s.settings.scroll_to_section), (GuiPage::Settings, Some(SettingsCategory::Audio)));
    assert!(s.back() && s.active_page == GuiPage::Chat);
}

/// A channel clicked on the Chat page is a step, seen by the history; Back opens the channel
/// before the way its row does (the room's messages asked for again).
#[test]
fn a_chat_channel_is_a_step() {
    let mut s = GuiState::default();
    s.chat_active_channel = "global".into();
    s.go(GuiPage::Chat, How::Beside);
    s.note_place();
    s.chat_active_channel = "feedback".into(); // a row clicked
    s.history_fetched = true;
    s.note_place();
    assert_eq!(s.history.len(), 2, "a channel clicked is a step");
    assert!(s.back());
    assert_eq!(s.chat_active_channel, "global");
    assert!(!s.history_fetched, "the room's messages are asked for again");
    s.note_place();
    assert_eq!(s.history.len(), 2, "going back is not a new step");
    assert!(s.forward() && s.chat_active_channel == "feedback");
}

/// A Files place chosen is a step.
#[test]
fn a_files_place_is_a_step() {
    let mut s = on(&[GuiPage::Files]);
    s.note_place();
    let first = s.files.place;
    let other = if first == FilesPlace::YourFolders { FilesPlace::Shared } else { FilesPlace::YourFolders };
    s.files.place = other; // its tab clicked
    s.note_place();
    assert_eq!(s.history.len(), 2, "a Files place chosen is a step");
    assert!(s.back() && s.files.place == first);
    assert!(s.forward() && s.files.place == other);
}

/// At most 100 steps; the oldest go first.
#[test]
fn the_history_keeps_the_last_hundred_steps() {
    let mut s = GuiState::default();
    for i in 0..150 {
        s.go(if i % 2 == 0 { GuiPage::Chat } else { GuiPage::Files }, How::Beside);
    }
    assert_eq!(s.history.len(), MAX_STEPS);
    let mut backs = 0;
    while s.back() {
        backs += 1;
    }
    assert_eq!(backs, MAX_STEPS - 1, "Back walks every step kept, and no further");
}

/// With a popup open, Back closes the popup first and the page stays; the next Back goes back.
/// Escape asks the same `close_popup` (lib.rs).
#[test]
fn back_closes_an_open_popup_first() {
    let mut app = App::new(on(&[GuiPage::Chat, GuiPage::Files]));
    let mut open = true;
    let draw = popup;
    app.frame(Vec::new(), draw(&mut open));
    assert!(open);
    let pos = egui::pos2(900.0, 600.0);
    let m = egui::Modifiers::default();
    app.frame(vec![Event::PointerButton { pos, button: PointerButton::Extra1, pressed: true, modifiers: m }], draw(&mut open));
    assert_eq!(app.state.active_page, GuiPage::Files, "Back closes the popup first, the page stays");
    assert!(!open, "the popup is closed");
    app.frame(vec![Event::PointerButton { pos, button: PointerButton::Extra1, pressed: false, modifiers: m }], draw(&mut open));
    app.button(PointerButton::Extra1);
    assert_eq!(app.state.active_page, GuiPage::Chat, "the next Back goes back");
    // Escape's first rule is the same function.
    let mut open = true;
    app.frame(Vec::new(), draw(&mut open));
    assert!(close_popup(&app.ctx), "a popup was open");
    app.frame(Vec::new(), draw(&mut open));
    assert!(!open, "and Escape's close closed it");
    assert!(!close_popup(&app.ctx), "with none open, Escape goes on to its other rules");
}

/// egui's text box moves its cursor a word on Alt+Left, so with a text box holding the keyboard
/// Alt+Left is that, never Back; with none, it is Back, and Alt+Right Forward.
#[test]
fn alt_left_in_a_text_box_moves_its_cursor() {
    let mut app = App::new(on(&[GuiPage::Chat, GuiPage::Files]));
    let id = egui::Id::new("history_test_box");
    let mut text = String::from("hello world");
    let mut focus = true;
    app.frame(Vec::new(), text_box(&mut text, &mut focus, id));
    app.frame(Vec::new(), text_box(&mut text, &mut focus, id));
    let at = |ctx: &egui::Context| egui::text_edit::TextEditState::load(ctx, id).and_then(|s| s.cursor.char_range()).map(|r| r.primary.index);
    let before = at(&app.ctx);
    app.frame(alt_key(egui::Key::ArrowLeft), text_box(&mut text, &mut focus, id));
    assert_eq!(app.state.active_page, GuiPage::Files, "Alt+Left inside a text box is its word jump, not Back");
    assert_ne!(at(&app.ctx), before, "the cursor moved (from {before:?})");
    // Without a text box: Back, then Forward.
    let mut app = App::new(on(&[GuiPage::Chat, GuiPage::Files]));
    app.frame(alt_key(egui::Key::ArrowLeft), |_, _, _| {});
    assert_eq!(app.state.active_page, GuiPage::Chat, "Alt+Left with no text box is Back");
    app.frame(alt_key(egui::Key::ArrowRight), |_, _, _| {});
    assert_eq!(app.state.active_page, GuiPage::Files, "Alt+Right is Forward");
}

/// The mouse's back and forward buttons (egui's Extra1 and Extra2) are Back and Forward on a
/// page; in the world they do nothing (unbound game buttons, the operator's answer of 2026-11-03).
#[test]
fn the_side_buttons_go_back_and_forward_on_a_page_and_do_nothing_in_the_world() {
    let mut app = App::new(on(&[GuiPage::Chat, GuiPage::Files]));
    app.button(PointerButton::Extra1);
    assert_eq!(app.state.active_page, GuiPage::Chat, "the back button goes back");
    app.button(PointerButton::Extra2);
    assert_eq!(app.state.active_page, GuiPage::Files, "the forward button goes forward");
    app.button(PointerButton::Extra1);
    app.state.enter_world();
    let (len, pos) = (app.state.history.len(), app.state.history.position());
    app.button(PointerButton::Extra1);
    assert_eq!(app.state.active_page, GuiPage::None, "in the world the back button does nothing");
    app.button(PointerButton::Extra2);
    assert_eq!(app.state.active_page, GuiPage::None, "nor the forward button");
    assert_eq!((app.state.history.len(), app.state.history.position()), (len, pos), "and the history is as it was");
}

/// Escape (section 10.4): out of a page opened from another, back to it, a visit's inside steps
/// included; on a page reached from the menu, into the world; in the world, the page left; on
/// the title screen, nothing.
#[test]
fn escape_backs_out_of_a_page_opened_from_another() {
    let mut s = on(&[GuiPage::Chat, GuiPage::Files]);
    s.escape();
    assert_eq!(s.active_page, GuiPage::None, "Escape on a page reached from the menu goes into the world, not Back");
    let mut s = on(&[GuiPage::Chat]);
    s.go(GuiPage::ServerSettings, How::From); // the chat's cog
    s.go(GuiPage::Settings, How::From); // a link inside Server settings
    s.escape();
    assert_eq!(s.active_page, GuiPage::ServerSettings);
    s.escape();
    assert_eq!(s.active_page, GuiPage::Chat);
    s.escape();
    assert_eq!(s.active_page, GuiPage::None, "Escape on a page reached from the menu goes into the world");
    s.escape();
    assert_eq!(s.active_page, GuiPage::Chat, "Escape in the world brings back the page left");
    // A visit opened from another page, with a step inside it: Escape leaves the whole visit.
    let mut s = on(&[GuiPage::Settings]);
    s.chat_active_channel = "global".into();
    s.go(GuiPage::Chat, How::From);
    s.note_place(); // the frame after: the page settled on #global
    s.chat_active_channel = "feedback".into();
    s.note_place();
    s.escape();
    assert_eq!(s.active_page, GuiPage::Settings, "the page the visit was opened from");
    s.show_title();
    s.escape();
    assert_eq!(s.active_page, GuiPage::MainMenu, "the title screen stays");
}

/// Entering the world is never a step: Play leaves the history where it is, leaving the world
/// returns to the step you were on, and Back after that goes to the page before it.
#[test]
fn the_world_is_never_a_step() {
    let mut s = on(&[GuiPage::Chat, GuiPage::Files]);
    crate::gui::pages::play::enter_world(&mut s, false);
    assert_eq!(s.active_page, GuiPage::None);
    assert_eq!(s.history.len(), 2, "entering the world adds no step");
    s.leave_world();
    assert_eq!(s.active_page, GuiPage::Files, "leaving it returns to the step you were on");
    assert_eq!(s.history.len(), 2);
    assert!(s.back() && s.active_page == GuiPage::Chat, "and Back goes to the page before it");
}
