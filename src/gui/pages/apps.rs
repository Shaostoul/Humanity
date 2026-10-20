//! Apps (2026-10-19): the Games library and the Tools page made one page (the operator: "What do
//! you think about changing the name of ... games to apps. Then merge tools with games/apps. Like
//! an offline recommended list that also tracks installs, launches, and what not. Red section
//! could be games. Green could be humanityOS tools. Blue could be third party tools."; and, the
//! same day, "What do you think extending the RGB style to CMY for beyond 3 options. In this case
//! what if help is C and services is M and then we still have Y.").
//!
//! SIX PARTS on one page, each under a band in its colour AND its name (a colour is never the only
//! way to tell them apart): the red, green and blue the chat sidebar and the Media page's Edit form
//! use, then cyan, magenta and yellow (widgets/tint_section.rs, theme tokens):
//! - RED, Games (apps/games.rs): the games installed from Steam, the Epic Games Store and GOG, each
//!   started through its store's own app; its lines, with a Why each, say what may be done and why
//!   (the dated findings 2026-10-13-game-stores-and-a-games-library.md and
//!   2026-10-19-games-page-art-stats-and-links.md). Epic's covers can be fetched from Epic's
//!   servers, only when the person turns that on (apps/covers.rs).
//! - GREEN, HumanityOS tools (apps/own_tools.rs): the app's own pages a person uses (Maps, Tasks,
//!   Crafting...), from data/gui/humanityos_tools.json, each opening its page here.
//! - BLUE, Other programs (apps/programs.rs): the free programs of the external catalog
//!   (data/external/catalog.json, "software"): which are installed (Windows' records, the Start
//!   menu's shortcuts and Steam's, read only), Launch, Show in folder, and Install through winget
//!   after a consent step; the dated finding 2026-10-19-tools-page-install-launch-and-icons.md,
//!   its Why.
//! - CYAN, Help, and MAGENTA, Services: the catalog's real-world help websites ("service"), split
//!   by each category's "section" in the data: urgent and protective needs (emergency, health,
//!   mental health, legal) in Help, everyday ones (water, food, housing, money, work, learning) in
//!   Services.
//! - YELLOW, Added by you: whatever the person added by its program, a game or any other program
//!   (config.json `apps_added`), started as its own program; Rename and Remove in its details.
//!
//! HOW THIS PAGE WORKS (2026-10-20): the parts' explanations (what the stores' terms keep for the
//! stores, what the covers are and why there are no logos, what is never done by itself, where the
//! other programs come from and how what is installed is found, Services' line), each sentence and
//! each Why word for word, are folded behind one small toggle by the heading (widgets/
//! how_it_works.rs; apps/library.rs `how_it_works`), closed until the person opens it, remembered
//! in config.json `apps_how_open`. Outside it stays only what a person must see before acting: the
//! other programs' safety line, Help's line (in danger, call the local emergency number first),
//! Epic's covers switch with its words, and the two short not-affiliated lines.
//!
//! The toolbar (apps/library.rs) searches, sorts, shows a Grid or a List, and narrows to one part
//! (a chip each) or to what is on this computer, across the parts; the choices are kept in the
//! settings (config.json `apps_view`). A tile is short: its cover (the name on it when there is no
//! picture, or one small name line under a picture), one line of where it is from and one fact,
//! and its button. An item clicked opens its details at the right (apps/game_detail.rs,
//! apps/entry_detail.rs, apps/detail.rs).
//!
//! The stores and the installed programs are read on worker threads when the page opens and on
//! Refresh, never every frame. Everything that starts or shows something goes through
//! `games::launch`, only on a click.
//!
//! DESKTOP ONLY as a whole: a web page cannot read or start what is installed on its computer. The
//! website's Tools page shows the catalog.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui::RichText;
use serde::{Deserialize, Serialize};

use crate::games::{Game, Launch, Store};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::tint_section::Tint;
use crate::gui::{GuiPage, GuiState};

pub use crate::gui::widgets::shelf::{Layout, GRID, LIST};

pub mod covers;
mod detail;
mod entry_detail;
mod game_detail;
pub mod games;
mod library;
pub mod own_tools;
pub mod programs;

pub use games::ShowAll;

/// The page's heading.
pub const TITLE: &str = "Apps";
/// The page's first line.
pub const INTRO: &str = "Your games, HumanityOS's own tools, other programs, and real-world help, in one place.";
/// Read the stores and the installed programs again.
pub const REFRESH: &str = "Refresh";
/// Opens a finding in the Library.
pub const WHY: &str = "Why";
pub const CLOSE: &str = "Close";
pub const CANCEL: &str = "Cancel";
pub const SHOW_IN_FOLDER: &str = "Show in folder";
/// The search field's hint.
pub const SEARCH_HINT: &str = "Search your apps and help";
/// The chip for every part, and the one that keeps only what is on this computer.
pub const ALL: &str = "All";
pub const INSTALLED_ONLY: &str = "Only what is on this computer";
/// A page not drawn for longer than this was closed: drawing it again is opening it.
const OPEN_GAP: Duration = Duration::from_secs(2);

/// The page's six parts, in the order shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    Games,
    Tools,
    Programs,
    Help,
    Services,
    Added,
}

impl Section {
    pub const ALL: [Section; 6] = [Section::Games, Section::Tools, Section::Programs, Section::Help, Section::Services, Section::Added];

    /// Its band's title and its chip's words: it always carries its name, never its colour alone.
    pub fn words(self) -> &'static str {
        match self {
            Section::Games => "Games",
            Section::Tools => "HumanityOS tools",
            Section::Programs => "Other programs",
            Section::Help => "Help",
            Section::Services => "Services",
            Section::Added => "Added by you",
        }
    }

    /// Its colour: red, green, blue, cyan, magenta, yellow.
    pub fn tint(self) -> Tint {
        match self {
            Section::Games => Tint::Red,
            Section::Tools => Tint::Green,
            Section::Programs => Tint::Blue,
            Section::Help => Tint::Cyan,
            Section::Services => Tint::Magenta,
            Section::Added => Tint::Yellow,
        }
    }

    /// Its chip's hover words.
    pub fn tip(self) -> &'static str {
        match self {
            Section::Games => "Only the games from Steam, the Epic Games Store and GOG.",
            Section::Tools => "Only HumanityOS's own tools: each opens its page here.",
            Section::Programs => "Only the other programs: free programs from their makers.",
            Section::Help => "Only help for urgent and protective needs: crisis lines, health, emergencies, legal aid.",
            Section::Services => "Only services for everyday needs: water, food, housing, money, work, learning.",
            Section::Added => "Only what you added yourself, games or any other program.",
        }
    }

    /// Its catalog entries, for the parts that show the external catalog: their kind, and for a
    /// help service its "section".
    pub fn catalog(self) -> Option<(&'static str, Option<&'static str>)> {
        match self {
            Section::Programs => Some(("software", None)),
            Section::Help => Some(("service", Some("help"))),
            Section::Services => Some(("service", Some("services"))),
            Section::Games | Section::Tools | Section::Added => None,
        }
    }
}

/// The order the parts are shown in, kept in the settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// By name, A to Z.
    #[default]
    Name,
    /// The most recently played game first (games).
    RecentlyPlayed,
    /// The most played game first (games).
    MostPlayed,
    /// The largest game on the disk first (games).
    Size,
    /// The programs on this computer first (other programs).
    InstalledFirst,
    /// By category (other programs, help, services).
    Category,
}

impl SortBy {
    pub const ALL: [SortBy; 6] = [SortBy::Name, SortBy::RecentlyPlayed, SortBy::MostPlayed, SortBy::Size, SortBy::InstalledFirst, SortBy::Category];

    /// The words on its pill.
    pub fn words(self) -> &'static str {
        match self {
            SortBy::Name => "Name",
            SortBy::RecentlyPlayed => "Recently played",
            SortBy::MostPlayed => "Most played",
            SortBy::Size => "Size",
            SortBy::InstalledFirst => "Installed first",
            SortBy::Category => "Category",
        }
    }

    /// Its pill's hover words.
    pub fn tip(self) -> &'static str {
        match self {
            SortBy::Name => "A to Z.",
            SortBy::RecentlyPlayed => "The game you played last first.",
            SortBy::MostPlayed => "The game you have played longest first.",
            SortBy::Size => "The largest game on the disk first.",
            SortBy::InstalledFirst => "The programs on this computer first.",
            SortBy::Category => "By category, each under its name.",
        }
    }

    /// Whether it orders `section` (the others are by name there).
    pub fn orders(self, section: Section) -> bool {
        match self {
            SortBy::Name => true,
            SortBy::RecentlyPlayed | SortBy::MostPlayed | SortBy::Size => section == Section::Games,
            SortBy::InstalledFirst => section == Section::Programs,
            SortBy::Category => section.catalog().is_some(),
        }
    }
}

/// How the page is shown, kept in the settings (config.json `apps_view`, 2026-10-19): Grid or
/// List, the order, the part narrowed to, only what is on this computer, and the category each
/// catalog part is narrowed to (`None`: every one).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    pub layout: Layout,
    pub sort: SortBy,
    /// The one part shown (`None`: all six).
    pub section: Option<Section>,
    /// Only what is on this computer: the help parts are left out, and of the other programs only
    /// those found (games, HumanityOS's own tools and what you added always are).
    pub installed_only: bool,
    /// The other programs' category, by id ("3d-modeling").
    pub programs_category: Option<String>,
    /// Help's category, by id ("health").
    pub help_category: Option<String>,
    /// Services' category, by id ("water").
    pub services_category: Option<String>,
}

impl View {
    /// Whether `section` is shown.
    pub fn shows(&self, section: Section) -> bool {
        let installed = !(self.installed_only && matches!(section, Section::Help | Section::Services));
        installed && self.section.is_none_or(|s| s == section)
    }

    /// The category a catalog part is narrowed to.
    pub fn category(&self, section: Section) -> Option<&str> {
        match section {
            Section::Programs => self.programs_category.as_deref(),
            Section::Help => self.help_category.as_deref(),
            Section::Services => self.services_category.as_deref(),
            _ => None,
        }
    }

    /// This view with a catalog part's category set to `category`.
    pub fn with_category(&self, section: Section, category: Option<String>) -> View {
        let mut v = self.clone();
        match section {
            Section::Programs => v.programs_category = category,
            Section::Help => v.help_category = category,
            Section::Services => v.services_category = category,
            _ => {}
        }
        v
    }
}

/// What a click picked: a game or something added (its store and id), a catalog entry (a program
/// or a help service, by its place in the catalog) or one of HumanityOS's own tools (by its place
/// in their list).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Picked {
    Game(Store, String),
    Entry(usize),
    Tool(usize),
}

/// The page's state, kept in `GuiState::apps`.
#[derive(Default)]
pub struct AppsState {
    /// The games (Steam, Epic, GOG) and what the person added.
    pub games: games::GamesPart,
    /// The other programs' installs, launches and installing.
    pub programs: programs::ProgramsPart,
    /// HumanityOS's own tools, read from data/gui/humanityos_tools.json at startup.
    pub own_tools: Vec<own_tools::OwnTool>,
    /// Epic's covers, fetched only when the person turns that on.
    pub covers: covers::EpicCovers,
    /// What the search field holds.
    pub search: String,
    /// What is open in the details at the right.
    pub selected: Option<Picked>,
    /// One line about the last thing done or refused, and whether it is a problem.
    pub status: Option<(String, bool)>,
    /// When the page was last drawn: a gap of more than `OPEN_GAP` means it was just opened.
    pub(crate) last_drawn: Option<Instant>,
    /// The parts' orders, worked out when what they depend on changes (apps/library.rs).
    pub(crate) orders: library::Orders,
}

impl AppsState {
    pub(crate) fn say(&mut self, words: impl Into<String>, problem: bool) {
        self.status = Some((words.into(), problem));
    }

    /// The game (or the thing added) whose details are open, when it is still on the list.
    pub(crate) fn selected_game(&self) -> Option<&Game> {
        match &self.selected {
            Some(Picked::Game(store, id)) => self.games.find(*store, id),
            _ => None,
        }
    }
}

/// Open `picked`'s details (from the start of its lists).
pub(crate) fn select(page: &mut AppsState, picked: Picked) {
    if page.selected.as_ref() != Some(&picked) {
        page.games.viewer = None;
        page.games.show_all = ShowAll::default();
        page.games.renaming = None;
    }
    page.selected = Some(picked);
}

/// What a click asked for, done once the page is drawn.
pub(crate) enum Act {
    /// A game's Play, or a thing added's Launch.
    Play(Game),
    /// A store's link, a Show in folder: through the one launcher.
    Open(Launch),
    /// A game's web link: only on a host `games::links` lists.
    OpenWeb(String),
    /// A catalog entry's web page (its website, its download page, its licence).
    Web(String),
    Select(Picked),
    CloseDetails,
    Rename(PathBuf, String),
    StartRename(PathBuf, String),
    CancelRename,
    Remove(PathBuf),
    ShowShot(usize),
    CloseShot,
    ShowAll(ShowAll),
    /// A program's Launch.
    Launch(usize),
    /// Install, pressed (the consent step follows).
    Install(usize),
    /// Install, pressed on a tile: its details open with the consent step (2026-10-20).
    InstallFromTile(usize),
    /// "I agree, install", pressed in the consent step.
    Agree(usize),
    /// Cancel, in the consent step.
    CancelInstall,
    /// One of HumanityOS's own tools: go to its page.
    OpenPage(GuiPage),
    /// A new view (kept in the settings).
    View(View),
    /// The games narrowed to one store (`None`: every store).
    Store(Option<Store>),
    /// "Add a program": the file picker.
    Add,
    /// Epic's covers switch, turned on or off.
    EpicCovers(bool),
    /// A Why: a finding's Library slug and its path in the project's files.
    Why(&'static str, &'static str),
}

fn apply(ctx: &egui::Context, state: &mut GuiState, act: Act) {
    match act {
        Act::Play(g) => games::play(state, &g),
        Act::Open(what) => games::open(state, &what),
        Act::OpenWeb(url) => games::open_web(ctx, state, &url),
        Act::Web(url) => programs::open_web(ctx, state, &url),
        Act::Select(p) => select(&mut state.apps, p),
        Act::CloseDetails => {
            state.apps.selected = None;
            state.apps.games.viewer = None;
        }
        Act::StartRename(p, n) => state.apps.games.renaming = Some((p, n)),
        Act::Rename(p, n) => games::rename_added(state, &p, &n),
        Act::CancelRename => state.apps.games.renaming = None,
        Act::Remove(p) => games::remove_added(state, &p),
        Act::ShowShot(i) => state.apps.games.viewer = Some(i),
        Act::CloseShot => state.apps.games.viewer = None,
        Act::ShowAll(all) => state.apps.games.show_all = all,
        Act::Launch(i) => programs::launch(state, i),
        Act::Install(i) => programs::install_pressed(state, i),
        Act::InstallFromTile(i) => programs::install_from_tile(state, i),
        Act::Agree(i) => programs::agree(state, i),
        Act::CancelInstall => programs::cancel_install(state),
        Act::OpenPage(page) => state.push_nav_to(page),
        Act::View(view) => keep_view(state, view),
        Act::Store(store) => state.apps.games.store = store,
        Act::Add => games::open_picker(state),
        Act::EpicCovers(on) => covers::set(state, on),
        Act::Why(slug, path) => games::open_why(state, slug, path),
    }
}

/// Keep a new view in the settings (marked for saving).
pub(crate) fn keep_view(state: &mut GuiState, view: View) {
    if state.settings.apps_view != view {
        state.settings.apps_view = view;
        state.settings_dirty = true;
    }
}

/// Close details whose thing is gone (a game no longer listed, a catalog shorter than before).
fn fix_selection(state: &mut GuiState) {
    let gone = match &state.apps.selected {
        Some(Picked::Game(store, id)) => state.apps.games.find(*store, id).is_none(),
        Some(Picked::Entry(i)) => *i >= state.external_catalog.len(),
        Some(Picked::Tool(i)) => *i >= state.apps.own_tools.len(),
        None => false,
    };
    if gone {
        state.apps.selected = None;
        state.apps.games.viewer = None;
    }
}

/// Test builds: read the stores and find the installed programs now, on this thread, and count the
/// page as open, so a snapshot's frames show what was read.
#[cfg(test)]
pub(crate) fn read_now(state: &mut GuiState) {
    games::read_now(state);
    programs::read_now(state);
    state.apps.last_drawn = Some(Instant::now());
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let now = Instant::now();
    let opened = state.apps.last_drawn.is_none_or(|t| now.duration_since(t) > OPEN_GAP);
    state.apps.last_drawn = Some(now);
    if opened {
        games::start_reading(state);
        programs::start_reading(state);
    } else if state.apps.games.added_from.as_ref() != Some(&state.settings.apps_added) {
        games::recheck_added(state);
    }
    games::poll(ctx, state);
    programs::poll(ctx, state);
    programs::poll_install(ctx, state);
    games::ensure_icons(ctx, &mut state.apps.games);
    programs::ensure_icons(ctx, &mut state.apps.programs);
    covers::update(ctx, state);
    widgets::thumbnails::next_frame();
    fix_selection(state);

    // The details first, so the parts take the room that is left.
    let mut acts = detail::panel(ctx, theme, state);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("apps_page").auto_shrink([false, false]).show(ui, |ui| {
                acts.extend(draw_head(ui, theme, state));
                acts.extend(library::toolbar(ui, theme, state));
                library::ensure_orders(state);
                acts.extend(library::parts(ui, theme, state));
            });
        });
    acts.extend(game_detail::viewer(ctx, theme, state));
    for act in acts {
        apply(ctx, state, act);
    }
    games::draw_picker(ctx, theme, state);
}

/// The heading, Refresh and "How this page works" (2026-10-20: the parts' explanations, with their
/// Whys, folded behind it, closed until the person opens it, remembered in config.json
/// `apps_how_open`), the page's line, the fold when open, and the status line.
fn draw_head(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> Vec<Act> {
    let mut refresh = false;
    let mut how = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(TITLE).size(theme.font_size_title).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        refresh = widgets::Button::ghost(REFRESH).tooltip("Look again for the games and programs installed on this computer.").show(ui, theme);
        ui.add_space(theme.spacing_sm);
        how = widgets::how_it_works::toggle(ui, theme, state.settings.apps_how_open);
    });
    ui.label(RichText::new(INTRO).size(theme.font_size_small).color(theme.text_secondary()));
    if how {
        state.settings.apps_how_open = !state.settings.apps_how_open;
        state.settings_dirty = true;
    }
    let acts = if state.settings.apps_how_open { library::how_it_works(ui, theme, state) } else { Vec::new() };
    if let Some((words, problem)) = &state.apps.status {
        let color = if *problem { theme.warning() } else { theme.text_secondary() };
        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    }
    ui.add_space(theme.spacing_xs);
    if refresh {
        state.apps.status = None;
        games::start_reading(state);
        programs::start_reading(state);
    }
    acts
}

#[cfg(test)]
pub(crate) mod tests;

#[cfg(test)]
mod games_tests;

#[cfg(test)]
mod game_detail_tests;

#[cfg(test)]
pub(crate) mod programs_tests;
