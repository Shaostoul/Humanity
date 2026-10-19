//! The Games library (2026-10-18; the operator, docs/PRIORITIES.md 2026-10-13: "A Games library:
//! list and launch every installed game (Steam, Epic, GOG, anything installed elsewhere), the
//! stores still doing downloads and updates behind it").
//!
//! DESKTOP ONLY. The website has no Games page: a web page cannot read the games installed on the
//! computer it runs on, or start them.
//!
//! What the page may do is set by the dated finding
//! docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md, and the page says so at
//! its top, with a Why that opens that finding in the Library (CLAUDE.md, "Saying what we will not
//! build, and why": downloads and updates are kept by the stores' terms, a platform holder's
//! rule). The readers are src/games/ and only read the stores' records. Every start goes through
//! `games::launch`, and only on a click.
//!
//! Top to bottom:
//! - "Games", with "Add a game" (the app's file picker: any game by its own program, named by its
//!   file name and renamed with Rename) and "Refresh" (reads the stores again).
//! - The line about the stores, with Why.
//! - One section per store found on this computer (Steam, Epic Games Store, GOG; a store not
//!   installed has none), then "Added by you". Each game: Play (when it can start now), its name,
//!   where it came from, the store's own Open button where the store documents one, its folder with
//!   Show in folder, and, when it cannot start, why (being installed by the store, its folder or
//!   its program gone).
//!
//! The stores are read on a worker thread when the page opens and when Refresh is pressed, never
//! every frame and never on a timer.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use egui::RichText;

use crate::games::{self, AddedGame, Found, Game, Launch, Sources, State, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult, FilePickerState};
use crate::gui::widgets::tint_section::{self, Tint};
use crate::gui::{GuiPage, GuiState};

/// The page's heading.
pub const TITLE: &str = "Games";
/// The line about the stores at the top of the page (a platform holder's rule, said plainly).
pub const STORES_LINE: &str = "HumanityOS starts your games through each store's own app. Downloads and updates stay with the store, because the stores' terms keep those for their own software. That is a platform holder's rule, not a law.";
/// What the page never does by itself.
pub const NOTHING_BY_ITSELF: &str = "Nothing here signs in to a store or does anything by itself: a store is asked to start a game only when you press Play.";
/// Opens the finding in the Library.
pub const WHY: &str = "Why";
/// The finding's Library slug (data/library/catalog.json, "Research Findings").
pub const FINDING_SLUG: &str = "2026-10-13-game-stores-and-a-games-library";
/// Where the finding is in the project's files, said when the Library does not have it.
pub const FINDING_PATH: &str = "docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md";
/// Add any game by its own program.
pub const ADD_A_GAME: &str = "Add a game";
pub const PICKER_TITLE: &str = "Add a game: choose its program";
/// Read the stores again.
pub const REFRESH: &str = "Refresh";
pub const READING: &str = "Looking for the games on this computer...";
pub const NO_STORES: &str = "No game store was found on this computer. Steam, the Epic Games Store and GOG are looked for.";
pub const ADDED_NONE: &str = "A game from anywhere else, such as one downloaded from its maker's own website, can be added here by its program: press Add a game.";
pub const PLAY: &str = "Play";
pub const SHOW_IN_FOLDER: &str = "Show in folder";
pub const RENAME: &str = "Rename";
pub const SAVE: &str = "Save";
pub const CANCEL: &str = "Cancel";
pub const REMOVE: &str = "Remove";
pub const FOLDER_GONE: &str = "Its folder is not there: it may have been moved or deleted, or be on a drive that is not plugged in.";
pub const PROGRAM_GONE: &str = "Its program is not in its folder, so it cannot be started from here.";
/// At most this many games can be added by hand.
pub const MAX_ADDED: usize = 500;
/// A page not drawn for longer than this was closed: drawing it again is opening it.
const OPEN_GAP: Duration = Duration::from_secs(2);
/// A second Play on the same game this soon after the first is the same press (a double-click).
const AGAIN_AFTER: Duration = Duration::from_secs(2);

/// Where the page reads the stores from.
#[derive(Debug, Clone, PartialEq)]
pub enum Where {
    /// This computer's stores (`Sources::this_computer`).
    ThisComputer,
    /// These folders (a test's fixtures, or the snapshot's).
    These(Sources),
}

impl Default for Where {
    fn default() -> Self {
        // A test build never reads the real computer's stores: its pages find nothing unless a
        // test points them at fixtures.
        #[cfg(test)]
        {
            Where::These(Sources::default())
        }
        #[cfg(not(test))]
        {
            Where::ThisComputer
        }
    }
}

/// The page's state, kept in `GuiState::games`.
#[derive(Default)]
pub struct GamesState {
    /// Where the stores are read from.
    pub sources: Where,
    /// What the stores had when last read; `None` until the first read finishes.
    pub found: Option<Found>,
    /// A read under way, on a worker thread.
    reading: Option<Receiver<Found>>,
    /// How many reads have been started (the tests count them: never one per frame).
    pub reads_started: u32,
    /// The added games as last checked, and the list they were checked from.
    added: Vec<Game>,
    added_from: Option<Vec<AddedGame>>,
    /// When the page was last drawn: a gap of more than `OPEN_GAP` means it was just opened.
    pub(crate) last_drawn: Option<Instant>,
    /// "Add a game"'s file picker, while open.
    pub picker: Option<FilePickerState>,
    /// The added game being renamed (its program) and the name typed so far.
    pub renaming: Option<(PathBuf, String)>,
    /// One line about the last thing done or refused, and whether it is a problem.
    pub status: Option<(String, bool)>,
    /// The last game Play started (its store and id) and when, for the double-click guard.
    last_play: Option<((Store, String), Instant)>,
}

impl GamesState {
    fn say(&mut self, words: impl Into<String>, problem: bool) {
        self.status = Some((words.into(), problem));
    }
}

/// Start reading the stores on a worker thread, unless a read is already under way. The added
/// games are checked again too (two file checks each, here).
pub(crate) fn start_reading(state: &mut GuiState) {
    recheck_added(state);
    if state.games.reading.is_some() {
        return;
    }
    let sources = state.games.sources.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("games-library-read".into()).spawn(move || {
        let sources = match sources {
            Where::ThisComputer => Sources::this_computer(),
            Where::These(s) => s,
        };
        let _ = tx.send(games::read(&sources));
    });
    match spawned {
        Ok(_) => {
            state.games.reading = Some(rx);
            state.games.reads_started += 1;
        }
        Err(e) => state.games.say(format!("The games could not be looked for: {e}."), true),
    }
}

/// Test builds: read the stores now, on this thread, and count the page as open, so a snapshot's
/// three frames show what was read (a worker's answer could arrive after them). Reads only a
/// test's own folders: `ThisComputer` reads nothing here.
#[cfg(test)]
pub(crate) fn read_now(state: &mut GuiState) {
    let sources = match &state.games.sources {
        Where::These(s) => s.clone(),
        Where::ThisComputer => Sources::default(),
    };
    state.games.found = Some(games::read(&sources));
    recheck_added(state);
    state.games.last_drawn = Some(Instant::now());
}

/// Take the worker's answer when it is in.
fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let Some(rx) = &state.games.reading else { return };
    match rx.try_recv() {
        Ok(found) => {
            state.games.found = Some(found);
            state.games.reading = None;
        }
        Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        Err(TryRecvError::Disconnected) => {
            state.games.reading = None;
            state.games.found.get_or_insert_with(Found::default);
        }
    }
}

/// Check the added games again (whether each program and its folder are there).
fn recheck_added(state: &mut GuiState) {
    state.games.added = games::other::games(&state.settings.added_games);
    state.games.added_from = Some(state.settings.added_games.clone());
}

/// Whether two paths are one file (letter case aside, as Windows has it).
fn same_file(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().replace('/', "\\").to_lowercase() == b.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// Add the game whose program is `exe`, named by its file name, kept in the settings.
pub(crate) fn add_game(state: &mut GuiState, exe: &Path) {
    let name = games::other::name_from_file(exe);
    if state.settings.added_games.iter().any(|a| same_file(&a.exe, exe)) {
        state.games.say(format!("{name} is already on the list."), false);
        return;
    }
    if state.settings.added_games.len() >= MAX_ADDED {
        state.games.say(format!("At most {MAX_ADDED} games can be added here. Remove one first."), true);
        return;
    }
    let mut list = state.settings.added_games.clone();
    list.push(AddedGame { name: name.clone(), exe: exe.to_path_buf() });
    state.settings.added_games = list;
    state.settings_dirty = true;
    state.games.say(format!("Added {name}."), false);
}

/// Take the added game whose program is `exe` off the list. The game itself is not touched.
pub(crate) fn remove_game(state: &mut GuiState, exe: &Path) {
    let mut list = state.settings.added_games.clone();
    list.retain(|a| !same_file(&a.exe, exe));
    state.settings.added_games = list;
    state.settings_dirty = true;
    if state.games.renaming.as_ref().is_some_and(|(p, _)| same_file(p, exe)) {
        state.games.renaming = None;
    }
}

/// Give the added game whose program is `exe` the name `name` (an empty name keeps the old one).
pub(crate) fn rename_game(state: &mut GuiState, exe: &Path, name: &str) {
    let name: String = name.trim().chars().take(200).collect();
    if !name.is_empty() {
        let mut list = state.settings.added_games.clone();
        for a in list.iter_mut().filter(|a| same_file(&a.exe, exe)) {
            a.name = name.clone();
        }
        state.settings.added_games = list;
        state.settings_dirty = true;
    }
    state.games.renaming = None;
}

/// What the status line says once Play has asked for `game`.
fn started_words(game: &Game) -> String {
    match game.store {
        Store::Steam => format!("Asked Steam to start {}.", game.name),
        Store::Epic => format!("Asked the Epic Games Store to start {}.", game.name),
        Store::Gog | Store::Added => format!("Started {}.", game.name),
    }
}

/// Play: start `game` through `games::launch`, once per press.
pub(crate) fn play(state: &mut GuiState, game: &Game) {
    let Some(what) = game.play.clone() else { return };
    let key = (game.store, game.id.clone());
    if state.games.last_play.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < AGAIN_AFTER) {
        return;
    }
    state.games.last_play = Some((key, Instant::now()));
    match games::launch(&what) {
        Ok(()) => state.games.say(started_words(game), false),
        Err(e) => state.games.say(format!("{} did not start: {e}.", game.name), true),
    }
}

/// Send a store button's link, or Show in folder.
fn open(state: &mut GuiState, what: &Launch) {
    if let Err(e) = games::launch(what) {
        state.games.say(format!("That did not open: {e}."), true);
    }
}

/// Why: the finding, in the Library.
pub(crate) fn open_why(state: &mut GuiState) {
    if crate::gui::pages::library::show_doc(state, FINDING_SLUG) {
        state.push_nav_to(GuiPage::Library);
    } else {
        state.games.say(format!("The Library on this computer does not have that document. It is {FINDING_PATH} in the project's files."), true);
    }
}

/// The words for a game that cannot start now.
fn why_not_words(game: &Game) -> Option<String> {
    let store = match game.store {
        Store::Epic => "the Epic Games Store".to_string(),
        other => other.name().to_string(),
    };
    match game.state {
        State::Ready if game.play.is_none() => Some(format!("Start it from {store}'s own app.")),
        State::Ready => None,
        State::BeingInstalled => Some(format!("Being installed by {store}")),
        State::BeingRemoved => Some(format!("Being removed by {store}")),
        State::FolderGone => Some(FOLDER_GONE.to_string()),
        State::ProgramGone => Some(PROGRAM_GONE.to_string()),
    }
}

/// What a click on a row asked for, done once the row is drawn.
enum Act {
    Play(Game),
    Open(Launch),
    Rename(PathBuf, String),
    StartRename(PathBuf, String),
    CancelRename,
    Remove(PathBuf),
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let now = Instant::now();
    let opened = state.games.last_drawn.is_none_or(|t| now.duration_since(t) > OPEN_GAP);
    state.games.last_drawn = Some(now);
    if opened {
        start_reading(state);
    } else if state.games.added_from.as_ref() != Some(&state.settings.added_games) {
        recheck_added(state);
    }
    poll(ctx, state);

    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.panel_margin))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("games_page").auto_shrink([false, false]).show(ui, |ui| {
                draw_body(ui, theme, state);
            });
        });
    draw_picker(ctx, theme, state);
}

fn draw_body(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (mut add, mut refresh, mut why) = (false, false, false);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(TITLE).size(theme.font_size_title).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        add = widgets::Button::primary(ADD_A_GAME).tooltip("Add any game on this computer by its program, such as one downloaded from its maker's own website.").show(ui, theme);
        refresh = widgets::Button::ghost(REFRESH).tooltip("Look again for the games the stores have installed.").show(ui, theme);
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(STORES_LINE).size(theme.font_size_small).color(theme.text_secondary()));
        why = widgets::Button::secondary(WHY).size(widgets::ButtonSize::Small).tooltip("What Steam's, Epic's and GOG's own terms say, as researched on 13 October 2026, in the Library.").show(ui, theme);
    });
    widgets::body_hint(ui, theme, NOTHING_BY_ITSELF);
    if let Some((words, problem)) = &state.games.status {
        let color = if *problem { theme.warning() } else { theme.text_secondary() };
        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    }
    ui.add_space(theme.spacing_sm);

    // Drawn from the state as it is (nothing is changed while drawing); what was clicked is done
    // afterwards.
    let mut act = None;
    let page = &state.games;
    match &page.found {
        None => {
            ui.label(RichText::new(READING).size(theme.font_size_body).color(theme.text_muted()));
        }
        Some(found) => {
            if found.steam.is_none() && found.epic.is_none() && found.gog.is_none() {
                ui.label(RichText::new(NO_STORES).size(theme.font_size_small).color(theme.text_muted()));
                ui.add_space(theme.spacing_sm);
            }
            let sections = [
                (Store::Steam, found.steam.as_ref(), "No Steam games are installed on this computer."),
                (Store::Epic, found.epic.as_ref(), "No games from the Epic Games Store are installed on this computer."),
                (Store::Gog, found.gog.as_ref(), "No GOG games are installed on this computer."),
            ];
            for (store, list, none) in sections {
                let Some(list) = list else { continue };
                tint_section::show(ui, theme, Tint::Blue, store.name(), |ui| {
                    if list.is_empty() {
                        ui.label(RichText::new(none).size(theme.font_size_small).color(theme.text_muted()));
                    }
                    for game in list {
                        if let Some(a) = draw_row(ui, theme, game, None) {
                            act = Some(a);
                        }
                    }
                });
                ui.add_space(theme.spacing_sm);
            }
        }
    }
    tint_section::show(ui, theme, Tint::Green, Store::Added.name(), |ui| {
        if page.added.is_empty() {
            ui.label(RichText::new(ADDED_NONE).size(theme.font_size_small).color(theme.text_muted()));
        }
        for game in &page.added {
            let editing = page.renaming.as_ref().filter(|(p, _)| same_file(p, &game.show)).map(|(_, n)| n.clone());
            if let Some(a) = draw_row(ui, theme, game, Some(editing)) {
                act = Some(a);
            }
        }
    });

    if add {
        state.games.picker = Some(FilePickerState::new(EXE_EXTS, 0).with_pick_verb("Add"));
    }
    if refresh {
        state.games.status = None;
        start_reading(state);
    }
    if why {
        open_why(state);
    }
    match act {
        Some(Act::Play(g)) => play(state, &g),
        Some(Act::Open(what)) => open(state, &what),
        Some(Act::StartRename(p, n)) => state.games.renaming = Some((p, n)),
        Some(Act::Rename(p, n)) => rename_game(state, &p, &n),
        Some(Act::CancelRename) => state.games.renaming = None,
        Some(Act::Remove(p)) => remove_game(state, &p),
        None => {}
    }
}

/// The programs "Add a game" offers: on Windows, programs (`.exe`); elsewhere a program has no
/// mark of its own, so any file.
#[cfg(windows)]
const EXE_EXTS: &[&str] = &["exe"];
#[cfg(not(windows))]
const EXE_EXTS: &[&str] = &[];

/// One game's row. `rename` is `Some` for an added game (`Some(Some(name))` while it is being
/// renamed, with the name typed so far). Returns what was clicked.
fn draw_row(ui: &mut egui::Ui, theme: &Theme, game: &Game, rename: Option<Option<String>>) -> Option<Act> {
    let mut act = None;
    ui.horizontal_wrapped(|ui| {
        if game.play.is_some() {
            let tip = match game.store {
                Store::Steam => "Asks Steam to start it.",
                Store::Epic => "Asks the Epic Games Store to start it.",
                Store::Gog => "Starts its own program, as its desktop shortcut does.",
                Store::Added => "Starts its program.",
            };
            if widgets::Button::primary(PLAY).tooltip(tip).show(ui, theme) {
                act = Some(Act::Play(game.clone()));
            }
        }
        match &rename {
            Some(Some(typed)) => {
                let mut typed = typed.clone();
                let edit = ui.add(egui::TextEdit::singleline(&mut typed).desired_width(260.0).hint_text("Its name"));
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if edit.changed() {
                    act = Some(Act::StartRename(game.show.clone(), typed.clone()));
                }
                if widgets::Button::primary(SAVE).size(widgets::ButtonSize::Small).show(ui, theme) || enter {
                    act = Some(Act::Rename(game.show.clone(), typed.clone()));
                }
                if widgets::Button::secondary(CANCEL).size(widgets::ButtonSize::Small).show(ui, theme) {
                    act = Some(Act::CancelRename);
                }
            }
            _ => {
                ui.label(RichText::new(&game.name).size(theme.font_size_body).strong().color(theme.text_primary()));
            }
        }
        ui.label(RichText::new(game.store.name()).size(theme.font_size_small).color(theme.text_muted()));
        if let Some(b) = game.store_button() {
            if widgets::Button::secondary(b.label).size(widgets::ButtonSize::Small).tooltip(b.tip).show(ui, theme) {
                act = Some(Act::Open(b.open));
            }
        }
        if let Some(None) = &rename {
            if widgets::Button::ghost(RENAME).size(widgets::ButtonSize::Small).tooltip("Change the name it shows here.").show(ui, theme) {
                act = Some(Act::StartRename(game.show.clone(), game.name.clone()));
            }
            if widgets::Button::ghost(REMOVE).size(widgets::ButtonSize::Small).tooltip("Take it off this list. The game itself is not touched.").show(ui, theme) {
                act = Some(Act::Remove(game.show.clone()));
            }
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(game.folder.display().to_string()).size(theme.font_size_small).color(theme.text_muted()));
        // With its program gone, the folder itself is what is there to show.
        let shown = if game.state == State::ProgramGone { game.folder.clone() } else { game.show.clone() };
        if game.folder_here && widgets::Button::ghost(SHOW_IN_FOLDER).size(widgets::ButtonSize::Small).tooltip("Show it in the computer's file manager.").show(ui, theme) {
            act = Some(Act::Open(Launch::ShowInFolder(shown)));
        }
    });
    if let Some(words) = why_not_words(game) {
        ui.label(RichText::new(words).size(theme.font_size_small).color(theme.warning()));
    }
    ui.add_space(theme.spacing_xs);
    act
}

/// "Add a game"'s file picker, while open (drawn over the page).
fn draw_picker(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut picker) = state.games.picker.take() else { return };
    match file_picker_modal(ctx, theme, &mut picker, PICKER_TITLE) {
        FilePickerResult::Open => state.games.picker = Some(picker),
        FilePickerResult::Cancelled => {}
        FilePickerResult::Picked(paths) => {
            if let Some(exe) = paths.first() {
                add_game(state, exe);
            }
        }
    }
}

#[cfg(test)]
#[path = "games_tests.rs"]
mod tests;
