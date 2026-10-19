//! The Games library (2026-10-18; the operator, docs/PRIORITIES.md 2026-10-13: "A Games library:
//! list and launch every installed game (Steam, Epic, GOG, anything installed elsewhere), the
//! stores still doing downloads and updates behind it").
//!
//! 2026-10-19, a library to look at as well as a list (the operator, after trying it: "how can we
//! beautify the games page? Can we include achievements, mods, screenshots, other metrics, sites
//! like steam, GOG, epic."): a Grid of tiles or a List, search, sort and a filter by store, and a
//! game's details beside the library with its numbers, its achievements, the person's screenshots,
//! its Workshop mods and its links.
//!
//! DESKTOP ONLY. The website has no Games page: a web page cannot read the games installed on the
//! computer it runs on, or start them.
//!
//! What the page may do is set by the dated finding
//! docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md, and the page says so at
//! its top, with a Why that opens that finding in the Library (CLAUDE.md, "Saying what we will not
//! build, and why": downloads and updates are kept by the stores' terms, a platform holder's
//! rule). The readers are src/games/ and only read the stores' records. Every start goes through
//! `games::launch`, and only on a click; a web page opens in the person's browser only on a click,
//! and only on a host `games::links` lists.
//!
//! Top to bottom:
//! - "Games", with "Add a game" (the app's file picker: any game by its own program, named by its
//!   file name and renamed with Rename) and "Refresh" (reads the stores again).
//! - The line about the stores, with Why.
//! - The toolbar (games_library.rs): search by name; sort by Name, Recently played, Most played or
//!   Size; Grid or List (both kept in the settings, so they survive a restart); one pill per store
//!   found on this computer with its count (a store not installed has none), and "Added by you".
//! - The library: a tile per game (its cover slot, its name, its store and one number, its
//!   achievements, Play) or a row per game (Play, its name, store, time played, last played, size,
//!   achievements, the store's own button). A game that cannot start says why in Play's place.
//! - A game clicked opens its details at the right (games_detail.rs).
//!
//! The stores are read on a worker thread when the page opens and when Refresh is pressed, never
//! every frame and never on a timer. A game's details are read with it, on the same worker; the
//! page only draws what was read. Screenshots are shrunk for drawing on widgets/thumbnails.rs's
//! own workers.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use egui::RichText;
use serde::{Deserialize, Serialize};

use crate::games::{self, AddedGame, Found, Game, Launch, Sources, State, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult, FilePickerState};
use crate::gui::{GuiPage, GuiState};

#[path = "games_library.rs"]
mod library;
#[path = "games_detail.rs"]
mod detail;

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
/// What the pictures are, why the stores' logos are not shown, and what stays on this computer
/// (the 2026-10-19 finding, said on the screen as the project's rule asks: a platform holder's
/// rule, named).
pub const PICTURES_LINE: &str = "Covers are the pictures each store's own app keeps on this computer. Store names are shown without their logos, because the stores have not licensed their logos to us: a platform holder's rule. Nothing about your games leaves this computer, except when you open one of a game's web links.";
/// Said once, quietly.
pub const NOT_AFFILIATED: &str = "HumanityOS is not affiliated with Valve, Epic Games or GOG.";
/// The second finding's Library slug, and where it is in the project's files.
pub const PICTURES_FINDING_SLUG: &str = "2026-10-19-games-page-art-stats-and-links";
pub const PICTURES_FINDING_PATH: &str = "docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md";
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
pub const CLOSE: &str = "Close";
pub const FOLDER_GONE: &str = "Its folder is not there: it may have been moved or deleted, or be on a drive that is not plugged in.";
pub const PROGRAM_GONE: &str = "Its program is not in its folder, so it cannot be started from here.";
/// The search field's hint.
pub const SEARCH_HINT: &str = "Search by name";
/// The pill for every store.
pub const ALL: &str = "All";
/// The views' names.
pub const GRID: &str = "Grid";
pub const LIST: &str = "List";
/// At most this many games can be added by hand.
pub const MAX_ADDED: usize = 500;
/// A page not drawn for longer than this was closed: drawing it again is opening it.
const OPEN_GAP: Duration = Duration::from_secs(2);
/// A second Play on the same game this soon after the first is the same press (a double-click).
const AGAIN_AFTER: Duration = Duration::from_secs(2);

/// How the library is shown: kept in the settings (config.json `games_layout`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// A tile per game.
    #[default]
    Grid,
    /// A row per game, its numbers in columns.
    List,
}

/// The order the library is shown in: kept in the settings (config.json `games_sort`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// By name, A to Z.
    #[default]
    Name,
    /// The most recently played first.
    RecentlyPlayed,
    /// The most played first.
    MostPlayed,
    /// The largest on the disk first.
    Size,
}

impl SortBy {
    pub const ALL: [SortBy; 4] = [SortBy::Name, SortBy::RecentlyPlayed, SortBy::MostPlayed, SortBy::Size];

    /// The words on its pill.
    pub fn words(self) -> &'static str {
        match self {
            SortBy::Name => "Name",
            SortBy::RecentlyPlayed => "Recently played",
            SortBy::MostPlayed => "Most played",
            SortBy::Size => "Size",
        }
    }
}

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

/// Which "Show all" a game's details have open.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ShowAll {
    pub achievements: bool,
    pub screenshots: bool,
    pub mods: bool,
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
    /// Every game the page shows: the stores' (as last read) and the added ones, rebuilt only when
    /// either changes. `generation` counts the rebuilds.
    pub(crate) all: Vec<Game>,
    generation: u64,
    /// The order the library is drawn in (indexes into `all`), and what it was worked out from.
    order: Option<(OrderKey, Vec<usize>)>,
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
    /// What the search field holds.
    pub search: String,
    /// The store the library is narrowed to (`None`: every store).
    pub filter: Option<Store>,
    /// The game whose details are open (its store and id).
    pub selected: Option<(Store, String)>,
    /// Which of its screenshots is shown large (an index into its list).
    pub viewer: Option<usize>,
    /// Which of its lists show everything.
    pub show_all: ShowAll,
    /// The programs' icons (Epic and GOG games, games/art.rs) as textures, made once each from
    /// what the worker read, in memory only; let go when the stores are read again.
    pub(crate) icons: std::collections::HashMap<(Store, String), egui::TextureHandle>,
    /// The list's `generation` the icons were made for.
    icons_made: Option<u64>,
}

/// What the library's order was worked out from.
#[derive(Debug, Clone, PartialEq)]
struct OrderKey {
    generation: u64,
    search: String,
    sort: SortBy,
    filter: Option<Store>,
}

impl GamesState {
    fn say(&mut self, words: impl Into<String>, problem: bool) {
        self.status = Some((words.into(), problem));
    }

    /// The game whose details are open, when it is still on the list.
    pub(crate) fn selected_game(&self) -> Option<&Game> {
        let (store, id) = self.selected.as_ref()?;
        self.all.iter().find(|g| g.store == *store && &g.id == id)
    }
}

/// Rebuild the list of every game from what was read and what was added (on a read coming in and
/// on the added games changing, never every frame). The open details close if their game is gone.
fn rebuild_all(page: &mut GamesState) {
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
    if page.selected.is_some() && page.selected_game().is_none() {
        page.selected = None;
        page.viewer = None;
    }
}

/// The library's order: `games` narrowed to `filter`'s store and to names holding every word of
/// `search` (letters' case ignored), sorted by `sort`. A game with no number to sort by (never
/// played, no size recorded) goes after every game with one, by name.
pub(crate) fn order_of(games: &[Game], sort: SortBy, search: &str, filter: Option<Store>) -> Vec<usize> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut out: Vec<usize> = games
        .iter()
        .enumerate()
        .filter(|(_, g)| filter.is_none_or(|s| g.store == s))
        .filter(|(_, g)| {
            let name = g.name.to_lowercase();
            words.iter().all(|w| name.contains(w.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    let by_name = |a: &Game, b: &Game| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then_with(|| a.id.cmp(&b.id));
    let number = |g: &Game| match sort {
        SortBy::Name => None,
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

/// Work the library's order out again when what it depends on changed (not every frame).
fn ensure_order(page: &mut GamesState, sort: SortBy) {
    let key = OrderKey { generation: page.generation, search: page.search.trim().to_lowercase(), sort, filter: page.filter };
    if page.order.as_ref().is_some_and(|(k, _)| *k == key) {
        return;
    }
    let order = order_of(&page.all, sort, &key.search, key.filter);
    page.order = Some((key, order));
}

/// Make a texture of each program icon the worker read and that has none yet (once per game per
/// read: nothing is read from the disk here, the pixels are already in memory).
fn ensure_icons(ctx: &egui::Context, page: &mut GamesState) {
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

impl GamesState {
    /// A game's program icon as a texture, with its size, when it has one.
    pub(crate) fn icon_of(&self, game: &Game) -> Option<(egui::TextureId, egui::Vec2)> {
        self.icons.get(&(game.store, game.id.clone())).map(|t| (t.id(), t.size_vec2()))
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
            rebuild_all(&mut state.games);
        }
        Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        Err(TryRecvError::Disconnected) => {
            state.games.reading = None;
            state.games.found.get_or_insert_with(Found::default);
            rebuild_all(&mut state.games);
        }
    }
}

/// Check the added games again (whether each program and its folder are there).
fn recheck_added(state: &mut GuiState) {
    state.games.added = games::other::games(&state.settings.added_games);
    state.games.added_from = Some(state.settings.added_games.clone());
    rebuild_all(&mut state.games);
}

/// Whether two paths are one file (letter case aside, as Windows has it).
fn same_file(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().replace('/', "\\").to_lowercase() == b.to_string_lossy().replace('/', "\\").to_lowercase()
}

/// Add the game whose program is `exe`, named by its file name, kept in the settings. Its details
/// open, so the person sees where it went.
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
    recheck_added(state);
    select(&mut state.games, Store::Added, exe.display().to_string());
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
    if state.games.last_play.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < AGAIN_AFTER) {
        return;
    }
    state.games.last_play = Some((key, Instant::now()));
    match games::launch(&what) {
        Ok(()) => state.games.say(started_words(game), false),
        Err(e) => state.games.say(format!("{} did not start: {e}.", game.name), true),
    }
}

/// Send a store's link, or Show in folder.
fn open(state: &mut GuiState, what: &Launch) {
    if let Err(e) = games::launch(what) {
        state.games.say(format!("That did not open: {e}."), true);
    }
}

/// Open a web page in the person's browser, only on a host `games::links` lists.
fn open_web(ctx: &egui::Context, state: &mut GuiState, url: &str) {
    if games::links::web_allowed(url) {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    } else {
        state.games.say("That page is not one this page opens.", true);
    }
}

/// Why: a finding (its Library slug, and its path in the project's files), in the Library.
pub(crate) fn open_why(state: &mut GuiState, slug: &str, path: &str) {
    if crate::gui::pages::library::show_doc(state, slug) {
        state.push_nav_to(GuiPage::Library);
    } else {
        state.games.say(format!("The Library on this computer does not have that document. It is {path} in the project's files."), true);
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
    crate::gui::pages::media::api::size_words(bytes.min(i64::MAX as u64) as i64)
}

/// "3 of 5 achievements", or "5 achievements" when which are unlocked is not known.
pub(crate) fn achievements_words(a: &games::achievements::Achievements) -> String {
    match a.unlocked() {
        Some(n) => format!("{n} of {} achievements", a.total()),
        None if a.total() == 1 => "1 achievement".to_string(),
        None => format!("{} achievements", a.total()),
    }
}

/// What a click asked for, done once the page is drawn.
pub(crate) enum Act {
    Play(Game),
    Open(Launch),
    OpenWeb(String),
    Select(Store, String),
    CloseDetails,
    Rename(PathBuf, String),
    StartRename(PathBuf, String),
    CancelRename,
    Remove(PathBuf),
    ShowShot(usize),
    CloseShot,
    ShowAll(ShowAll),
    Sort(SortBy),
}

/// Open game `id`'s details (from the start of its lists).
fn select(page: &mut GamesState, store: Store, id: String) {
    if page.selected.as_ref() != Some(&(store, id.clone())) {
        page.viewer = None;
        page.show_all = ShowAll::default();
        page.renaming = None;
    }
    page.selected = Some((store, id));
}

fn apply(ctx: &egui::Context, state: &mut GuiState, act: Act) {
    match act {
        Act::Play(g) => play(state, &g),
        Act::Open(what) => open(state, &what),
        Act::OpenWeb(url) => open_web(ctx, state, &url),
        Act::Select(store, id) => select(&mut state.games, store, id),
        Act::CloseDetails => {
            state.games.selected = None;
            state.games.viewer = None;
        }
        Act::StartRename(p, n) => state.games.renaming = Some((p, n)),
        Act::Rename(p, n) => rename_game(state, &p, &n),
        Act::CancelRename => state.games.renaming = None,
        Act::Remove(p) => remove_game(state, &p),
        Act::ShowShot(i) => state.games.viewer = Some(i),
        Act::CloseShot => state.games.viewer = None,
        Act::ShowAll(all) => state.games.show_all = all,
        Act::Sort(sort) => {
            if state.settings.games_sort != sort {
                state.settings.games_sort = sort;
                state.settings_dirty = true;
            }
        }
    }
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
    ensure_icons(ctx, &mut state.games);
    widgets::thumbnails::next_frame();

    // The details first, so the library takes the room that is left.
    let mut acts = detail::panel(ctx, theme, state);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("games_page").auto_shrink([false, false]).show(ui, |ui| {
                draw_head(ui, theme, state);
                library::toolbar(ui, theme, state);
                ensure_order(&mut state.games, state.settings.games_sort);
                acts.extend(library::library(ui, theme, state));
            });
        });
    acts.extend(detail::viewer(ctx, theme, state));
    for act in acts {
        apply(ctx, state, act);
    }
    draw_picker(ctx, theme, state);
}

/// The heading, Add a game and Refresh, the lines about the stores and the pictures (each with
/// its Why), and the status line.
fn draw_head(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let (mut add, mut refresh, mut why, mut why_pictures) = (false, false, false, false);
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
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(PICTURES_LINE).size(theme.font_size_small).color(theme.text_secondary()));
        why_pictures = widgets::Button::secondary(WHY).size(widgets::ButtonSize::Small).tooltip("What the stores and the law say about game pictures, store names and logos, play records and links, as researched on 19 October 2026, in the Library.").show(ui, theme);
    });
    widgets::body_hint(ui, theme, NOTHING_BY_ITSELF);
    ui.label(RichText::new(NOT_AFFILIATED).size(theme.font_size_small).color(theme.text_muted()));
    if let Some((words, problem)) = &state.games.status {
        let color = if *problem { theme.warning() } else { theme.text_secondary() };
        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    }
    ui.add_space(theme.spacing_sm);
    if add {
        state.games.picker = Some(FilePickerState::new(EXE_EXTS, 0).with_pick_verb("Add"));
    }
    if refresh {
        state.games.status = None;
        start_reading(state);
    }
    if why {
        open_why(state, FINDING_SLUG, FINDING_PATH);
    }
    if why_pictures {
        open_why(state, PICTURES_FINDING_SLUG, PICTURES_FINDING_PATH);
    }
}

/// The programs "Add a game" offers: on Windows, programs (`.exe`); elsewhere a program has no
/// mark of its own, so any file.
#[cfg(windows)]
const EXE_EXTS: &[&str] = &["exe"];
#[cfg(not(windows))]
const EXE_EXTS: &[&str] = &[];

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

#[cfg(test)]
#[path = "games_detail_tests.rs"]
mod detail_tests;
