//! The Apps page's games, its red part (the Games library, 2026-10-18; the operator,
//! docs/PRIORITIES.md 2026-10-13: "A Games library: list and launch every installed game (Steam,
//! Epic, GOG, anything installed elsewhere), the stores still doing downloads and updates behind
//! it"; a library to look at since 2026-10-19; part of the Apps page since the same day).
//!
//! What may be done is set by the dated finding
//! docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md, and the part says so,
//! with a Why that opens that finding in the Library (CLAUDE.md, "Saying what we will not build,
//! and why": downloads and updates are kept by the stores' terms, a platform holder's rule); its
//! pictures, store names, play records and links by the second,
//! docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md, with its own Why. The
//! readers are src/games/ and only read the stores' records. Every start goes through
//! `games::launch`, and only on a click; a web page opens in the person's browser only on a click,
//! and only on a host `games::links` lists.
//!
//! The stores are read on a worker thread when the page opens and when Refresh is pressed, never
//! every frame and never on a timer. A game's details are read with it, on the same worker; the
//! page only draws what was read.
//!
//! ADDED BY YOU, the page's yellow part (2026-10-19, the operator: "Y" for what is left once help
//! and services take cyan and magenta), is here too: "Add a program" adds anything by its own
//! program (the app's file picker), a game or not, named by its file name and renamed with Rename,
//! kept in the settings (config.json `apps_added`). It is listed as a `Game` from `Store::Added`,
//! so it starts through the same launcher, as its own program in its own folder.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::games::{self, AddedApp, Found, Game, Launch, Sources, State, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult, FilePickerState};
use crate::gui::{GuiPage, GuiState};

use super::Picked;

/// The line about the stores (a platform holder's rule, said plainly).
pub const STORES_LINE: &str = "HumanityOS starts your games through each store's own app. Downloads and updates stay with the store, because the stores' terms keep those for their own software. That is a platform holder's rule, not a law.";
/// What the page never does by itself.
pub const NOTHING_BY_ITSELF: &str = "Nothing here signs in to a store or does anything by itself: a store is asked to start a game only when you press Play.";
/// The finding's Library slug (data/library/catalog.json, "Research Findings").
pub const FINDING_SLUG: &str = "2026-10-13-game-stores-and-a-games-library";
/// Where the finding is in the project's files, said when the Library does not have it.
pub const FINDING_PATH: &str = "docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md";
/// What the pictures are, why the stores' logos are not shown, and what stays on this computer
/// (the 2026-10-19 finding, said on the screen as the project's rule asks: a platform holder's
/// rule, named).
pub const PICTURES_LINE: &str = "Covers are the pictures each store's own app keeps on this computer. Store names are shown without their logos, because the stores have not licensed their logos to us: a platform holder's rule. Nothing about your games leaves this computer, except when you open one of a game's web links or turn on Epic's covers below.";
/// Said once, quietly.
pub const NOT_AFFILIATED: &str = "HumanityOS is not affiliated with Valve, Epic Games or GOG.";
/// The second finding's Library slug, and where it is in the project's files.
pub const PICTURES_FINDING_SLUG: &str = "2026-10-19-games-page-art-stats-and-links";
pub const PICTURES_FINDING_PATH: &str = "docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md";
/// Add anything by its own program: a game or any other program (Added by you).
pub const ADD: &str = "Add a program";
pub const PICKER_TITLE: &str = "Add a program: choose it";
/// Launch, on what was added: a game or any other program, started as its own program.
pub const LAUNCH: &str = "Launch";
pub const READING: &str = "Looking for the games on this computer...";
pub const NO_STORES: &str = "No game store was found on this computer. Steam, the Epic Games Store and GOG are looked for.";
pub const ADDED_NONE: &str = "Any game or program on this computer, such as one downloaded from its maker's own website, can be added here by its program: press Add a program.";
pub const PLAY: &str = "Play";
pub const RENAME: &str = "Rename";
pub const SAVE: &str = "Save";
pub const REMOVE: &str = "Remove";
pub const FOLDER_GONE: &str = "Its folder is not there: it may have been moved or deleted, or be on a drive that is not plugged in.";
pub const PROGRAM_GONE: &str = "Its program is not in its folder, so it cannot be started from here.";
/// At most this many things can be added by hand.
pub const MAX_ADDED: usize = 500;
/// A second Play on the same game this soon after the first is the same press (a double-click).
const AGAIN_AFTER: Duration = Duration::from_secs(2);

/// Where the stores are read from.
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

/// Which "Show all" a game's details have open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShowAll {
    pub achievements: bool,
    pub screenshots: bool,
    pub mods: bool,
}

/// The games' state, kept in `AppsState::games`.
#[derive(Default)]
pub struct GamesPart {
    /// Where the stores are read from.
    pub sources: Where,
    /// What the stores had when last read; `None` until the first read finishes.
    pub found: Option<Found>,
    /// A read under way, on a worker thread.
    pub(super) reading: Option<Receiver<Found>>,
    /// How many reads have been started (the tests count them: never one per frame).
    pub reads_started: u32,
    /// The added games as last checked, and the list they were checked from.
    pub(super) added: Vec<Game>,
    pub(super) added_from: Option<Vec<AddedApp>>,
    /// Every game the page shows: the stores' (as last read) and the added ones, rebuilt only when
    /// either changes. `generation` counts the rebuilds.
    pub(crate) all: Vec<Game>,
    pub(super) generation: u64,
    /// "Add a program"'s file picker, while open.
    pub picker: Option<FilePickerState>,
    /// The added game being renamed (its program) and the name typed so far.
    pub renaming: Option<(PathBuf, String)>,
    /// The last game Play started (its store and id) and when, for the double-click guard.
    last_play: Option<((Store, String), Instant)>,
    /// The store the games are narrowed to (`None`: every store).
    pub store: Option<Store>,
    /// Which of the open game's screenshots is shown large (an index into its list).
    pub viewer: Option<usize>,
    /// Which of its lists show everything.
    pub show_all: ShowAll,
    /// The programs' icons (Epic and GOG games, games/art.rs) as textures, made once each from
    /// what the worker read, in memory only; let go when the stores are read again.
    pub(crate) icons: std::collections::HashMap<(Store, String), egui::TextureHandle>,
    /// The list's `generation` the icons were made for.
    icons_made: Option<u64>,
}

impl GamesPart {
    /// A game's program icon as a texture, with its size, when it has one.
    pub(crate) fn icon_of(&self, game: &Game) -> Option<(egui::TextureId, egui::Vec2)> {
        self.icons.get(&(game.store, game.id.clone())).map(|t| (t.id(), t.size_vec2()))
    }

    /// The game `store`, `id`, when it is on the list.
    pub(crate) fn find(&self, store: Store, id: &str) -> Option<&Game> {
        self.all.iter().find(|g| g.store == store && g.id == id)
    }
}

/// Rebuild the list of every game from what was read and what was added (on a read coming in and
/// on the added games changing, never every frame). The open details close if their game is gone.
fn rebuild_all(state: &mut GuiState) {
    let page = &mut state.apps.games;
    let mut all = Vec::new();
    if let Some(found) = &page.found {
        for list in [&found.steam, &found.epic, &found.gog].into_iter().flatten() {
            all.extend(list.iter().cloned());
        }
    }
    all.extend(page.added.iter().cloned());
    page.all = all;
    page.generation += 1;
    page.icons.clear();
    if let Some(Picked::Game(store, id)) = state.apps.selected.clone() {
        if state.apps.games.find(store, &id).is_none() {
            state.apps.selected = None;
            state.apps.games.viewer = None;
        }
    }
}

/// The order of `games` from `store` (`None`: the three stores', not what was added; Added by you
/// asks for `Store::Added`) whose names hold every word of `search` (letters' case ignored),
/// sorted by `sort`. A game with no number to sort by (never played, no size recorded) goes after
/// every game with one, by name. The sorts that are not a game's (installed first, category) are
/// by name.
pub(crate) fn order_of(games: &[Game], sort: super::SortBy, search: &str, store: Option<Store>) -> Vec<usize> {
    use super::SortBy;
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut out: Vec<usize> = games
        .iter()
        .enumerate()
        .filter(|(_, g)| match store {
            None => g.store != Store::Added,
            Some(s) => g.store == s,
        })
        .filter(|(_, g)| {
            let name = g.name.to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    let by_name = |a: &Game, b: &Game| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id));
    let number = |g: &Game| match sort {
        SortBy::Name | SortBy::InstalledFirst | SortBy::Category => None,
        SortBy::RecentlyPlayed => g.details.last_played,
        SortBy::MostPlayed => g.details.played_minutes.filter(|m| *m > 0),
        SortBy::Size => g.details.size,
    };
    out.sort_by(|&a, &b| {
        let (ga, gb) = (&games[a], &games[b]);
        // Largest first; `None` is less than any `Some`, so a game without one goes last.
        number(gb).cmp(&number(ga)).then_with(|| by_name(ga, gb))
    });
    out
}

/// Make a texture of each program icon the worker read and that has none yet (once per game per
/// read: nothing is read from the disk here, the pixels are already in memory).
pub(super) fn ensure_icons(ctx: &egui::Context, page: &mut GamesPart) {
    if page.icons_made == Some(page.generation) {
        return;
    }
    page.icons_made = Some(page.generation);
    for game in &page.all {
        let Some(icon) = &game.details.art.icon else { continue };
        let key = (game.store, game.id.clone());
        if page.icons.contains_key(&key) {
            continue;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
        let texture = ctx.load_texture(format!("game-icon:{:?}:{}", game.store, game.id), image, egui::TextureOptions::LINEAR);
        page.icons.insert(key, texture);
    }
}

/// Start reading the stores on a worker thread, unless a read is already under way. The added
/// games are checked again too (two file checks each, here).
pub(crate) fn start_reading(state: &mut GuiState) {
    recheck_added(state);
    if state.apps.games.reading.is_some() {
        return;
    }
    let sources = state.apps.games.sources.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("apps-games-read".into()).spawn(move || {
        let sources = match sources {
            Where::ThisComputer => Sources::this_computer(),
            Where::These(s) => s,
        };
        let _ = tx.send(games::read(&sources));
    });
    match spawned {
        Ok(_) => {
            state.apps.games.reading = Some(rx);
            state.apps.games.reads_started += 1;
        }
        Err(e) => state.apps.say(format!("The games could not be looked for: {e}."), true),
    }
}

/// Test builds: read the stores now, on this thread, so a snapshot's frames show what was read (a
/// worker's answer could arrive after them). Reads only a test's own folders: `ThisComputer` reads
/// nothing here.
#[cfg(test)]
pub(crate) fn read_now(state: &mut GuiState) {
    let sources = match &state.apps.games.sources {
        Where::These(s) => s.clone(),
        Where::ThisComputer => Sources::default(),
    };
    state.apps.games.found = Some(games::read(&sources));
    recheck_added(state);
}

/// Take the worker's answer when it is in.
pub(super) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let Some(rx) = &state.apps.games.reading else { return };
    match rx.try_recv() {
        Ok(found) => {
            state.apps.games.found = Some(found);
            state.apps.games.reading = None;
            rebuild_all(state);
        }
        Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        Err(TryRecvError::Disconnected) => {
            state.apps.games.reading = None;
            state.apps.games.found.get_or_insert_with(Found::default);
            rebuild_all(state);
        }
    }
}

/// Check the added games again (whether each program and its folder are there).
pub(super) fn recheck_added(state: &mut GuiState) {
    state.apps.games.added = games::other::games(&state.settings.apps_added);
    state.apps.games.added_from = Some(state.settings.apps_added.clone());
    rebuild_all(state);
}

/// Whether two paths are one file (letter case aside, as Windows has it).
pub(super) fn same_file(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().replace('/', "\\").to_lowercase() == b.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// Add the game whose program is `exe`, named by its file name, kept in the settings. Its details
/// open, so the person sees where it went.
pub(crate) fn add_added(state: &mut GuiState, exe: &Path) {
    let name = games::other::name_from_file(exe);
    if state.settings.apps_added.iter().any(|a| same_file(&a.exe, exe)) {
        state.apps.say(format!("{name} is already on the list."), false);
        return;
    }
    if state.settings.apps_added.len() >= MAX_ADDED {
        state.apps.say(format!("At most {MAX_ADDED} programs can be added here. Remove one first."), true);
        return;
    }
    let mut list = state.settings.apps_added.clone();
    list.push(AddedApp { name: name.clone(), exe: exe.to_path_buf() });
    state.settings.apps_added = list;
    state.settings_dirty = true;
    state.apps.say(format!("Added {name}."), false);
    recheck_added(state);
    super::select(&mut state.apps, Picked::Game(Store::Added, exe.display().to_string()));
}

/// Take the added game whose program is `exe` off the list. The game itself is not touched.
pub(crate) fn remove_added(state: &mut GuiState, exe: &Path) {
    let mut list = state.settings.apps_added.clone();
    list.retain(|a| !same_file(&a.exe, exe));
    state.settings.apps_added = list;
    state.settings_dirty = true;
    if state.apps.games.renaming.as_ref().is_some_and(|(p, _)| same_file(p, exe)) {
        state.apps.games.renaming = None;
    }
}

/// Give the added game whose program is `exe` the name `name` (an empty name keeps the old one).
pub(crate) fn rename_added(state: &mut GuiState, exe: &Path, name: &str) {
    let name: String = name.trim().chars().take(200).collect();
    if !name.is_empty() {
        let mut list = state.settings.apps_added.clone();
        for a in list.iter_mut().filter(|a| same_file(&a.exe, exe)) {
            a.name = name.clone();
        }
        state.settings.apps_added = list;
        state.settings_dirty = true;
    }
    state.apps.games.renaming = None;
}

/// What the status line says once Play has asked for `game`.
fn started_words(game: &Game) -> String {
    match game.store {
        Store::Steam => format!("Asked Steam to start {}.", game.name),
        Store::Epic => format!("Asked the Epic Games Store to start {}.", game.name),
        Store::Gog | Store::Added => format!("Started {}.", game.name),
    }
}

/// The words on the start button: Play for a store's game, Launch for what was added (a game or
/// any other program).
pub(crate) fn play_words(store: Store) -> &'static str {
    if store == Store::Added {
        LAUNCH
    } else {
        PLAY
    }
}

/// Play's hover words.
pub(crate) fn play_tip(store: Store) -> &'static str {
    match store {
        Store::Steam => "Asks Steam to start it.",
        Store::Epic => "Asks the Epic Games Store to start it.",
        Store::Gog => "Starts its own program, as its desktop shortcut does.",
        Store::Added => "Starts its program.",
    }
}

/// Play: start `game` through `games::launch`, once per press.
pub(crate) fn play(state: &mut GuiState, game: &Game) {
    let Some(what) = game.play.clone() else { return };
    let key = (game.store, game.id.clone());
    if state.apps.games.last_play.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < AGAIN_AFTER) {
        return;
    }
    state.apps.games.last_play = Some((key, Instant::now()));
    match games::launch(&what) {
        Ok(()) => state.apps.say(started_words(game), false),
        Err(e) => state.apps.say(format!("{} did not start: {e}.", game.name), true),
    }
}

/// Open a web page in the person's browser, only on a host `games::links` lists.
pub(super) fn open_web(ctx: &egui::Context, state: &mut GuiState, url: &str) {
    if games::links::web_allowed(url) {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    } else {
        state.apps.say("That page is not one this page opens.", true);
    }
}

/// A store's name in a sentence ("the Epic Games Store").
fn store_in_words(store: Store) -> String {
    match store {
        Store::Epic => "the Epic Games Store".to_string(),
        other => other.name().to_string(),
    }
}

/// The words for a game that cannot start now.
pub(crate) fn why_not_words(game: &Game) -> Option<String> {
    let store = store_in_words(game.store);
    match game.state {
        State::Ready if game.play.is_none() => Some(format!("Start it from {store}'s own app.")),
        State::Ready => None,
        State::BeingInstalled => Some(format!("Being installed by {store}")),
        State::BeingRemoved => Some(format!("Being removed by {store}")),
        State::FolderGone => Some(FOLDER_GONE.to_string()),
        State::ProgramGone => Some(PROGRAM_GONE.to_string()),
    }
}

/// The few words for it on a tile, where there is no room for the sentence (its hover words).
pub(crate) fn why_not_short(game: &Game) -> Option<String> {
    let store = match game.store {
        Store::Epic => "Epic",
        other => other.name(),
    };
    match game.state {
        State::Ready if game.play.is_none() => Some(format!("Start it from {store}")),
        State::Ready => None,
        State::BeingInstalled => Some(format!("Being installed by {store}")),
        State::BeingRemoved => Some(format!("Being removed by {store}")),
        State::FolderGone => Some("Its folder is not there".to_string()),
        State::ProgramGone => Some("Its program is not there".to_string()),
    }
}

/// One or two words for it in a list row's Play cell (the sentence is their hover words).
pub(crate) fn why_not_tag(game: &Game) -> Option<&'static str> {
    match game.state {
        State::Ready if game.play.is_none() => Some("In its store"),
        State::Ready => None,
        State::BeingInstalled => Some("Installing"),
        State::BeingRemoved => Some("Removing"),
        State::FolderGone => Some("Folder missing"),
        State::ProgramGone => Some("Program missing"),
    }
}

/// Time played in words: "45 minutes", "12.6 hours", "233 hours".
pub(crate) fn played_words(minutes: u64) -> String {
    if minutes < 60 {
        return if minutes == 1 { "1 minute".to_string() } else { format!("{minutes} minutes") };
    }
    let tenths = (minutes * 10 + 30) / 60;
    if tenths == 10 {
        "1 hour".to_string()
    } else if minutes < 100 * 60 && tenths % 10 != 0 {
        format!("{}.{} hours", tenths / 10, tenths % 10)
    } else {
        format!("{} hours", (minutes + 30) / 60)
    }
}

const MONTHS: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// A Unix time as a date, "21 September 2026" (`short`: "21 Sep 2026"). The stores record these
/// in UTC, and the app writes its other dates in UTC too.
pub(crate) fn day_words(unix: u64, short: bool) -> String {
    let (y, m, d) = crate::persistence::civil_from_days((unix / 86_400) as i64);
    let month = MONTHS[(m as usize).clamp(1, 12) - 1];
    if short {
        format!("{d} {} {y}", &month[..3])
    } else {
        format!("{d} {month} {y}")
    }
}

/// A size in the words the app uses elsewhere ("412 MB", "31.2 GB").
pub(crate) fn size_words(bytes: u64) -> String {
    crate::gui::pages::files::api::size_words(bytes.min(i64::MAX as u64) as i64)
}

/// "3 of 5 achievements", or "5 achievements" when which are unlocked is not known.
pub(crate) fn achievements_words(a: &games::achievements::Achievements) -> String {
    match a.unlocked() {
        Some(n) => format!("{n} of {} achievements", a.total()),
        None if a.total() == 1 => "1 achievement".to_string(),
        None => format!("{} achievements", a.total()),
    }
}

/// Send a store's link, or Show in folder.
pub(super) fn open(state: &mut GuiState, what: &Launch) {
    if let Err(e) = games::launch(what) {
        state.apps.say(format!("That did not open: {e}."), true);
    }
}

/// The programs "Add a program" offers: on Windows, programs (`.exe`); elsewhere a program has no
/// mark of its own, so any file.
#[cfg(windows)]
pub(super) const EXE_EXTS: &[&str] = &["exe"];
#[cfg(not(windows))]
pub(super) const EXE_EXTS: &[&str] = &[];

/// Open "Add a program"'s file picker.
pub(super) fn open_picker(state: &mut GuiState) {
    state.apps.games.picker = Some(FilePickerState::new(EXE_EXTS, 0).with_pick_verb("Add"));
}

/// "Add a program"'s file picker, while open (drawn over the page).
pub(super) fn draw_picker(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut picker) = state.apps.games.picker.take() else { return };
    match file_picker_modal(ctx, theme, &mut picker, PICKER_TITLE) {
        FilePickerResult::Open => state.apps.games.picker = Some(picker),
        FilePickerResult::Cancelled => {}
        FilePickerResult::Picked(paths) => {
            if let Some(exe) = paths.first() {
                add_added(state, exe);
            }
        }
    }
}

/// Why: a finding (its Library slug, and its path in the project's files), in the Library.
pub(crate) fn open_why(state: &mut GuiState, slug: &str, path: &str) {
    if crate::gui::pages::library::show_doc(state, slug) {
        state.push_nav_to(GuiPage::Library);
    } else {
        state.apps.say(format!("The Library on this computer does not have that document. It is {path} in the project's files."), true);
    }
}
