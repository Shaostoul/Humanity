//! Tools -- the catalog of EXTERNAL things, as a library to look at like the Games page
//! (2026-10-19; the operator: "How can we beautify this tools page? What if we make the tools page
//! like the games page? Then what if through that interface users can install/launch/go to the
//! website(s) for the various non-game apps?"). Two kinds live here (v0.1063):
//!
//! - `software`: free programs you download and run yourself (Blender, GIMP...).
//! - `service`: real-world help websites and organizations (water, food, housing, legal aid...),
//!   which used to be the Library page's third face.
//!
//! Both come from the single `data/external/catalog.json` via `state.tools_catalog`, loaded at
//! startup (software/catalog.rs). The web mirror is `web/pages/tools.html` reading the same file.
//! Library is what you READ; this page is what you GO USE.
//!
//! Top to bottom:
//! - "Tools" and Refresh (looks again for what is installed); the intro line, where the programs
//!   come from, and how what is installed is found.
//! - The toolbar (tools_library.rs, on the shelf the Games page uses, widgets/shelf.rs): search; sort
//!   by Name, Installed first or Category; Grid or List; the kind pills (Everything, Software, Help
//!   and services, each with its count) and the category pills. The view, the sort and both filters
//!   are kept in the settings (config.json `tools_view`), so they survive a restart.
//! - The shelf: a tile per entry (a panel in its category's colour with, for an installed program,
//!   its own icon; its name, category and one line; Launch when installed, otherwise Download or
//!   Open website), or a row per entry with its facts in columns.
//! - An entry clicked opens its details at the right (tools_detail.rs): its description, licence,
//!   platforms, size, the file types its category opens, where it is installed, and Launch,
//!   Show in folder, Website, Download page and Install.
//!
//! What is installed is read by src/software/ on a worker thread when the page opens and on
//! Refresh, never every frame (Windows' own records and Steam's, read only). Launch goes through the
//! Games page's one launcher (games::launch), and only on a click. Install runs winget, Windows'
//! own package manager, only after the consent step in the details and the person's own "I agree,
//! install" (software/install.rs, on a worker; one install at a time), then looks again. A web page
//! opens in the person's browser only on a click. What may be done is set by the dated finding
//! docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md, which the page's Why
//! opens in the Library.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use egui::RichText;
use serde::{Deserialize, Serialize};

use crate::games::{self, Launch};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::software::{self, install, Found, Installed, ToolEntry, Via};

pub use crate::gui::widgets::shelf::Layout;

#[path = "tools_library.rs"]
mod library;
#[path = "tools_detail.rs"]
mod detail;

/// The page's heading.
pub const TITLE: &str = "Tools";
/// The page's first line (kept from before it was a library).
pub const INTRO: &str = "Free software you can install, and real-world help services. Everything here is external: we point, you choose.";
/// Where the programs come from.
pub const WHERE_FROM: &str = "Each program comes from its maker: Download and Website open the maker's own pages in your browser, and Install uses the maker's own installer through winget, Windows' own package manager, after you agree to its licence.";
/// The safety line (the 2026-10-19 finding, section 6 and judgement call 7: Microsoft, the FTC and
/// the makers say the same).
pub const SAFETY_LINE: &str = "Get programs from their makers, never from an advert, even a top search result. The programs here are free from their makers' own sites; some makers also sell store copies to fund their work, and anyone else asking you to pay for one is not the maker.";
/// How what is installed is found, and what the pictures are.
pub const FOUND_LINE: &str = "Which programs are installed is read from Windows' own list of installed programs and from Steam, on this computer only, and never sent anywhere; Windows has no sure way to find every program, so one not found may still be there. An installed program shows the icon in its own file; no logo is fetched from any website, because each maker sets its own rules for its logo.";
/// Said once, quietly (the finding's judgement call 6).
pub const NOT_AFFILIATED: &str = "HumanityOS is not made by, or affiliated with, the makers of these programs.";
/// Said for a program not found (never "not installed": Windows has no sure way to find every one).
pub const NOT_FOUND: &str = "Not found on this computer";
/// Opens the finding in the Library.
pub const WHY: &str = "Why";
/// The finding's Library slug, and where it is in the project's files.
pub const FINDING_SLUG: &str = "2026-10-19-tools-page-install-launch-and-icons";
pub const FINDING_PATH: &str = "docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md";
/// The consent step's button: it says what it does.
pub const AGREE_INSTALL: &str = "I agree, install";
pub const CANCEL: &str = "Cancel";
/// Said where finding what is installed does not work.
pub const WINDOWS_ONLY: &str = "Finding which programs are installed works on Windows for now.";
pub const REFRESH: &str = "Refresh";
pub const READING: &str = "Looking for the programs installed on this computer...";
pub const LAUNCH: &str = "Launch";
pub const DOWNLOAD: &str = "Download";
pub const OPEN_WEBSITE: &str = "Open website";
pub const WEBSITE: &str = "Website";
pub const DOWNLOAD_PAGE: &str = "Download page";
pub const INSTALL: &str = "Install";
pub const SHOW_IN_FOLDER: &str = "Show in folder";
pub const CLOSE: &str = "Close";
pub const INSTALLED: &str = "Installed";
/// The search field's hint.
pub const SEARCH_HINT: &str = "Search tools and services";
/// The kind pill for everything, and the category pill for every category.
pub const EVERYTHING: &str = "Everything";
pub const ALL: &str = "All";
/// Said for an installed program whose program file was not found.
pub const NO_PROGRAM: &str = "Windows has no record of where its program is, so start it from its own shortcut.";
/// A tile's few words for the same.
pub const NO_PROGRAM_SHORT: &str = "Start it from its shortcut";
/// A second Launch on the same program this soon after the first is the same press.
const AGAIN_AFTER: Duration = Duration::from_secs(2);
/// A page not drawn for longer than this was closed: drawing it again is opening it.
const OPEN_GAP: Duration = Duration::from_secs(2);

/// The order the shelf is shown in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortBy {
    /// By name, A to Z.
    #[default]
    Name,
    /// The installed programs first, each part by name.
    InstalledFirst,
    /// By category, A to Z, then by name (the Grid shows each category under its name).
    Category,
}

impl SortBy {
    pub const ALL: [SortBy; 3] = [SortBy::Name, SortBy::InstalledFirst, SortBy::Category];

    /// The words on its pill.
    pub fn words(self) -> &'static str {
        match self {
            SortBy::Name => "Name",
            SortBy::InstalledFirst => "Installed first",
            SortBy::Category => "Category",
        }
    }

    /// Its pill's hover words.
    pub fn tip(self) -> &'static str {
        match self {
            SortBy::Name => "A to Z.",
            SortBy::InstalledFirst => "The programs on this computer first.",
            SortBy::Category => "By category, each under its name.",
        }
    }
}

/// How the shelf is shown, kept in the settings (config.json `tools_view`): Grid or List, the
/// order, and the kind and category it is narrowed to (`None`: every one). A config written before
/// it existed shows every entry in a Grid by name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct View {
    pub layout: Layout,
    pub sort: SortBy,
    /// A kind id ("software", "service").
    pub kind: Option<String>,
    /// A category id ("3d-modeling").
    pub category: Option<String>,
}

/// Where the page reads what is installed from.
#[derive(Debug, Clone, PartialEq)]
pub enum Where {
    /// This computer (`software::Sources::this_computer`, read on the worker).
    ThisComputer,
    /// These records (a test's, or the snapshot's).
    These(software::Sources),
}

impl Default for Where {
    fn default() -> Self {
        // A test build never reads the real computer's records: its pages find nothing installed
        // unless a test hands them records of its own.
        #[cfg(test)]
        {
            Where::These(software::Sources::default())
        }
        #[cfg(not(test))]
        {
            Where::ThisComputer
        }
    }
}

/// The page's state, kept in `GuiState::tools`.
#[derive(Default)]
pub struct ToolsState {
    /// Where what is installed is read from.
    pub sources: Where,
    /// What was found when last read; `None` until the first read finishes.
    pub found: Option<Found>,
    /// A read under way, on a worker thread.
    reading: Option<Receiver<Found>>,
    /// How many reads have been started (the tests count them: never one per frame).
    pub reads_started: u32,
    /// Counts the reads that came in (the order and the icons are worked out again after each).
    generation: u64,
    /// The order the shelf is drawn in (indexes into the catalog), and what it was worked out from.
    order: Option<(OrderKey, Vec<usize>)>,
    /// When the page was last drawn: a gap of more than `OPEN_GAP` means it was just opened.
    pub(crate) last_drawn: Option<Instant>,
    /// What the search field holds.
    pub search: String,
    /// The entry whose details are open (its place in the catalog).
    pub selected: Option<usize>,
    /// One line about the last thing done or refused, and whether it is a problem.
    pub status: Option<(String, bool)>,
    /// An install under way, one at a time: reading what winget says, asking the person, or
    /// winget installing (software/install.rs).
    pub(crate) install: Option<Installing>,
    /// The last install's outcome, shown in that entry's details: (entry, words, a problem).
    pub install_note: Option<(usize, String, bool)>,
    /// An entry winget just installed: when the next read is in, the page says whether it was found.
    after_install: Option<usize>,
    /// Read again as soon as the read under way is in (it began before an install ended).
    read_again: bool,
    /// The last program Launch started and when, for the double-click guard.
    last_launch: Option<(usize, Instant)>,
    /// The installed programs' icons as textures, made once each from what the worker read.
    pub(crate) icons: HashMap<usize, egui::TextureHandle>,
    icons_made: Option<u64>,
}

/// An install under way (one at a time), by its entry's place in the catalog.
pub(crate) enum Installing {
    /// Asking winget what it knows about the package (`winget show`, on a worker).
    Reading { entry: usize, rx: Receiver<install::Event> },
    /// The consent step, in the entry's details, until "I agree, install" or Cancel.
    Asking { entry: usize, package: install::Package },
    /// winget installing it (on a worker), its latest line.
    Running { entry: usize, rx: Receiver<install::Event>, line: String },
}

impl Installing {
    pub(crate) fn entry(&self) -> usize {
        match self {
            Installing::Reading { entry, .. } | Installing::Asking { entry, .. } | Installing::Running { entry, .. } => *entry,
        }
    }
}

/// What the shelf's order was worked out from.
#[derive(Debug, Clone, PartialEq)]
struct OrderKey {
    generation: u64,
    entries: usize,
    search: String,
    view: View,
}

impl ToolsState {
    fn say(&mut self, words: impl Into<String>, problem: bool) {
        self.status = Some((words.into(), problem));
    }

    /// How catalog entry `i` is installed, when it is.
    pub(crate) fn installed(&self, i: usize) -> Option<&Installed> {
        self.found.as_ref().and_then(|f| f.get(i))
    }

    /// Entry `i`'s program icon as a texture, with its size, when it has one.
    pub(crate) fn icon_of(&self, i: usize) -> Option<(egui::TextureId, egui::Vec2)> {
        self.icons.get(&i).map(|t| (t.id(), t.size_vec2()))
    }
}

/// The shelf's order: `catalog` narrowed to `view`'s kind and category and to entries whose name,
/// category or description hold every word of `search` (letters' case ignored), sorted by `view`'s
/// sort. Installed first needs `found`; without it (still reading) it is by name.
pub(crate) fn order_of(catalog: &[ToolEntry], found: Option<&Found>, view: &View, search: &str) -> Vec<usize> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut out: Vec<usize> = catalog
        .iter()
        .enumerate()
        .filter(|(_, e)| view.kind.as_deref().is_none_or(|k| e.kind == k))
        .filter(|(_, e)| view.category.as_deref().is_none_or(|c| e.category_id == c))
        .filter(|(_, e)| {
            let text = format!("{}\n{}\n{}", e.name, e.category, e.description).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    let by_name = |a: usize, b: usize| catalog[a].name.to_lowercase().cmp(&catalog[b].name.to_lowercase()).then_with(|| catalog[a].category.cmp(&catalog[b].category)).then_with(|| a.cmp(&b));
    let installed = |i: usize| found.and_then(|f| f.get(i)).is_some();
    match view.sort {
        SortBy::Name => out.sort_by(|&a, &b| by_name(a, b)),
        SortBy::InstalledFirst => out.sort_by(|&a, &b| installed(b).cmp(&installed(a)).then_with(|| by_name(a, b))),
        SortBy::Category => out.sort_by(|&a, &b| catalog[a].category.to_lowercase().cmp(&catalog[b].category.to_lowercase()).then_with(|| by_name(a, b))),
    }
    out
}

/// Work the shelf's order out again when what it depends on changed (not every frame).
fn ensure_order(state: &mut GuiState) {
    let key = OrderKey { generation: state.tools.generation, entries: state.tools_catalog.len(), search: state.tools.search.trim().to_lowercase(), view: state.settings.tools_view.clone() };
    if state.tools.order.as_ref().is_some_and(|(k, _)| *k == key) {
        return;
    }
    let order = order_of(&state.tools_catalog, state.tools.found.as_ref(), &key.view, &key.search);
    state.tools.order = Some((key, order));
}

/// Make a texture of each program icon the worker read and that has none yet (once per read:
/// nothing is read from the disk here, the pixels are already in memory).
fn ensure_icons(ctx: &egui::Context, page: &mut ToolsState) {
    if page.icons_made == Some(page.generation) {
        return;
    }
    page.icons_made = Some(page.generation);
    let Some(found) = &page.found else { return };
    for (i, installed) in found.installed.iter().enumerate() {
        let Some(icon) = installed.as_ref().and_then(|f| f.icon.as_ref()) else { continue };
        if page.icons.contains_key(&i) {
            continue;
        }
        let image = egui::ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
        let texture = ctx.load_texture(format!("tool-icon:{i}"), image, egui::TextureOptions::LINEAR);
        page.icons.insert(i, texture);
    }
}

/// Start looking for what is installed on a worker thread, unless a read is already under way.
pub(crate) fn start_reading(state: &mut GuiState) {
    if state.tools.reading.is_some() {
        return;
    }
    let sources = state.tools.sources.clone();
    let catalog = state.tools_catalog.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("tools-installed-read".into()).spawn(move || {
        let sources = match sources {
            Where::ThisComputer => software::Sources::this_computer(),
            Where::These(s) => s,
        };
        let _ = tx.send(software::read(&sources, &catalog));
    });
    match spawned {
        Ok(_) => {
            state.tools.reading = Some(rx);
            state.tools.reads_started += 1;
        }
        Err(e) => state.tools.say(format!("The installed programs could not be looked for: {e}."), true),
    }
}

/// Take the worker's answer when it is in.
fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let Some(rx) = &state.tools.reading else { return };
    match rx.try_recv() {
        Ok(found) => took(state, found),
        Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        Err(TryRecvError::Disconnected) => took(state, Found::default()),
    }
}

fn took(state: &mut GuiState, found: Found) {
    let page = &mut state.tools;
    page.found = Some(found);
    page.reading = None;
    page.generation += 1;
    page.icons.clear();
    if std::mem::take(&mut page.read_again) {
        start_reading(state);
        return;
    }
    // After an install: say whether the program is found now.
    if let Some(i) = state.tools.after_install.take() {
        let name = state.tools_catalog.get(i).map(|e| e.name.clone()).unwrap_or_default();
        let words = if state.tools.installed(i).is_some() {
            format!("{name} is installed.")
        } else {
            format!("winget finished, but {name} was not found on this computer yet. Refresh looks again.")
        };
        state.tools.install_note = Some((i, words.clone(), false));
        state.tools.say(words, false);
    }
}

/// Test builds: find what is installed now, on this thread, and count the page as open, so a
/// snapshot's frames show it (a worker's answer could arrive after them). Reads only a test's own
/// records: `ThisComputer` reads nothing here.
#[cfg(test)]
pub(crate) fn read_now(state: &mut GuiState) {
    let sources = match &state.tools.sources {
        Where::These(s) => s.clone(),
        Where::ThisComputer => software::Sources::default(),
    };
    let found = software::read(&sources, &state.tools_catalog);
    took(state, found);
    state.tools.last_drawn = Some(Instant::now());
}

/// A kind id's words ("software" is "Software").
pub(crate) fn kind_label(kind: &str) -> &str {
    match kind {
        "software" => "Software",
        "service" => "Help and services",
        other => other,
    }
}

/// A platform id's words ("windows" is "Windows").
pub(crate) fn platform_words(p: &str) -> String {
    match p {
        "windows" => "Windows".to_string(),
        "mac" => "Mac".to_string(),
        "linux" => "Linux".to_string(),
        "web" => "Web browser".to_string(),
        other => other.to_string(),
    }
}

/// An entry's platforms in words ("Windows, Mac, Linux").
pub(crate) fn platforms_words(e: &ToolEntry) -> String {
    e.platforms.iter().map(|p| platform_words(p)).collect::<Vec<_>>().join(", ")
}

/// What the main button says for an entry not installed (or not startable): Download for a program
/// to download, Open website for one that runs in the browser and for a help service.
pub(crate) fn web_button(e: &ToolEntry) -> &'static str {
    if e.is_software() && !e.web_only() {
        DOWNLOAD
    } else {
        OPEN_WEBSITE
    }
}

/// What a click asked for, done once the page is drawn.
pub(crate) enum Act {
    Launch(usize),
    Open(Launch),
    Web(String),
    Select(usize),
    CloseDetails,
    /// Install, pressed (the consent step follows).
    Install(usize),
    /// "I agree, install", pressed in the consent step.
    Agree(usize),
    /// Cancel, in the consent step.
    CancelInstall,
    View(View),
}

/// Launch: start entry `i`'s program through the one launcher, once per press.
pub(crate) fn launch(state: &mut GuiState, i: usize) {
    let Some(entry) = state.tools_catalog.get(i).cloned() else { return };
    let Some(installed) = state.tools.installed(i).cloned() else { return };
    let Some(what) = installed.launch.clone() else { return };
    if state.tools.last_launch.is_some_and(|(k, at)| k == i && at.elapsed() < AGAIN_AFTER) {
        return;
    }
    state.tools.last_launch = Some((i, Instant::now()));
    match games::launch(&what) {
        Ok(()) if installed.via == Via::Steam => state.tools.say(format!("Asked Steam to start {}.", entry.name), false),
        Ok(()) => state.tools.say(format!("Started {}.", entry.name), false),
        Err(e) => state.tools.say(format!("{} did not start: {e}.", entry.name), true),
    }
}

/// Open a web page from the catalog in the person's browser (only a web address).
fn open_web(ctx: &egui::Context, state: &mut GuiState, url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    } else {
        state.tools.say("That is not a web address this page opens.", true);
    }
}

/// Run `plan` (winget) on a worker thread; its lines and its end come back on the receiver. Test
/// builds run it here (install::run only records there, and answers at once), so the test's
/// recorder, which is per thread, sees it.
fn start_job(plan: install::Plan) -> Receiver<install::Event> {
    let (tx, rx) = std::sync::mpsc::channel();
    #[cfg(test)]
    install::run(&plan, &tx);
    #[cfg(not(test))]
    {
        let failed = tx.clone();
        if let Err(e) = std::thread::Builder::new().name("tools-winget".into()).spawn(move || install::run(&plan, &tx)) {
            let _ = failed.send(install::Event::Done(Err(install::Failure { code: None, words: format!("winget could not be started ({e}).") })));
        }
    }
    rx
}

/// Install, pressed: first ask winget what it knows about the package (its publisher, licence and
/// agreements), for the consent step. Nothing is installed here. One install at a time.
pub(crate) fn install_pressed(state: &mut GuiState, i: usize) {
    let Some(entry) = state.tools_catalog.get(i).cloned() else { return };
    if !entry.installable() || state.tools.installed(i).is_some() {
        return;
    }
    if let Some(busy) = &state.tools.install {
        let name = state.tools_catalog.get(busy.entry()).map(|e| e.name.clone()).unwrap_or_default();
        state.tools.say(format!("One install at a time: {name} first."), false);
        return;
    }
    let Some(plan) = entry.winget.as_deref().and_then(install::show_plan) else {
        state.tools.install_note = Some((i, "Its package id in the catalog is not one winget can use.".to_string(), true));
        return;
    };
    state.tools.install_note = None;
    state.tools.install = Some(Installing::Reading { entry: i, rx: start_job(plan) });
}

/// "I agree, install", pressed in the consent step for entry `i`: only now is winget asked to
/// install it, with the person's agreement passed on for that one package.
pub(crate) fn agree(state: &mut GuiState, i: usize) {
    let Some(Installing::Asking { entry, .. }) = &state.tools.install else { return };
    if *entry != i {
        return;
    }
    let Some(e) = state.tools_catalog.get(i).cloned() else { return };
    let Some(id) = e.winget.clone() else { return };
    let agreed = install::Agreed { id, interactive: e.install_interactive };
    let Some(plan) = install::install_plan(&agreed) else { return };
    state.tools.install = Some(Installing::Running { entry: i, rx: start_job(plan), line: String::new() });
    state.tools.say(format!("Installing {} with winget.", e.name), false);
}

/// Cancel in the consent step: nothing is installed.
fn cancel_install(state: &mut GuiState) {
    if let Some(Installing::Asking { entry, .. } | Installing::Reading { entry, .. }) = &state.tools.install {
        state.tools.install_note = Some((*entry, "Cancelled. Nothing was installed.".to_string(), false));
        state.tools.install = None;
    }
}

/// Take what winget sent since the last frame: the package read (the consent step opens), a line
/// of progress, or its end (look for the program again, or say why it did not install).
fn poll_install(ctx: &egui::Context, state: &mut GuiState) {
    let Some(step) = state.tools.install.as_mut() else { return };
    let (entry, rx) = match step {
        Installing::Reading { entry, rx } | Installing::Running { entry, rx, .. } => (*entry, rx),
        Installing::Asking { .. } => return,
    };
    let mut done = None;
    let mut latest = None;
    loop {
        match rx.try_recv() {
            Ok(install::Event::Line(l)) => latest = Some(l),
            Ok(install::Event::Done(d)) => {
                done = Some(d);
                break;
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
                break;
            }
            Err(TryRecvError::Disconnected) => {
                done = Some(Err(install::Failure { code: None, words: "winget stopped without saying how it ended.".to_string() }));
                break;
            }
        }
    }
    if let (Some(l), Installing::Running { line, .. }) = (latest, &mut *step) {
        *line = l;
    }
    let Some(done) = done else { return };
    let reading = matches!(step, Installing::Reading { .. });
    let name = state.tools_catalog.get(entry).map(|e| e.name.clone()).unwrap_or_default();
    let id = state.tools_catalog.get(entry).and_then(|e| e.winget.clone()).unwrap_or_default();
    state.tools.install = None;
    match (reading, done) {
        (true, Ok(text)) => match install::parse_show(&id, &text) {
            Some(package) => state.tools.install = Some(Installing::Asking { entry, package }),
            None => state.tools.install_note = Some((entry, format!("winget did not find the package {id}. Download opens the maker's own page."), true)),
        },
        (true, Err(f)) => state.tools.install_note = Some((entry, f.words, true)),
        (false, Ok(_)) => installed_now(state, entry, format!("winget installed {name}. Looking for it on this computer...")),
        (false, Err(f)) if f.code == Some(install::ALREADY_INSTALLED) => installed_now(state, entry, f.words),
        (false, Err(f)) => {
            state.tools.install_note = Some((entry, f.words.clone(), true));
            state.tools.say(format!("{name} did not install: {}", f.words), true);
        }
    }
}

/// winget says entry `i` is on the computer now: look for it again, so its tile turns to Launch.
fn installed_now(state: &mut GuiState, i: usize, words: String) {
    state.tools.install_note = Some((i, words.clone(), false));
    state.tools.say(words, false);
    state.tools.after_install = Some(i);
    if state.tools.reading.is_none() {
        start_reading(state);
    } else {
        // A read under way began before the install ended: read again once it is in.
        state.tools.read_again = true;
    }
}

/// Open entry `i`'s details.
fn select(page: &mut ToolsState, i: usize) {
    page.selected = Some(i);
}

fn apply(ctx: &egui::Context, state: &mut GuiState, act: Act) {
    match act {
        Act::Launch(i) => launch(state, i),
        Act::Open(what) => {
            if let Err(e) = games::launch(&what) {
                state.tools.say(format!("That did not open: {e}."), true);
            }
        }
        Act::Web(url) => open_web(ctx, state, &url),
        Act::Select(i) => select(&mut state.tools, i),
        Act::CloseDetails => state.tools.selected = None,
        Act::Install(i) => install_pressed(state, i),
        Act::Agree(i) => agree(state, i),
        Act::CancelInstall => cancel_install(state),
        Act::View(view) => library::keep_view(state, view),
    }
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let now = Instant::now();
    let opened = state.tools.last_drawn.is_none_or(|t| now.duration_since(t) > OPEN_GAP);
    state.tools.last_drawn = Some(now);
    if opened {
        start_reading(state);
    }
    poll(ctx, state);
    poll_install(ctx, state);
    ensure_icons(ctx, &mut state.tools);
    if state.tools.selected.is_some_and(|i| i >= state.tools_catalog.len()) {
        state.tools.selected = None;
    }

    // The details first, so the shelf takes the room that is left.
    let mut acts = detail::panel(ctx, theme, state);
    egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("tools_page").auto_shrink([false, false]).show(ui, |ui| {
                draw_head(ui, theme, state);
                library::toolbar(ui, theme, state);
                ensure_order(state);
                acts.extend(library::shelf(ui, theme, state));
            });
        });
    for act in acts {
        apply(ctx, state, act);
    }
}

/// The heading and Refresh, the lines about the page, and the status line.
fn draw_head(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let mut refresh = false;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(TITLE).size(theme.font_size_title).color(theme.text_primary()));
        ui.add_space(theme.spacing_md);
        refresh = widgets::Button::ghost(REFRESH).tooltip("Look again for the programs installed on this computer.").show(ui, theme);
    });
    let mut why = false;
    ui.label(RichText::new(INTRO).size(theme.font_size_small).color(theme.text_secondary()));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(WHERE_FROM).size(theme.font_size_small).color(theme.text_secondary()));
        why = widgets::Button::secondary(WHY).size(widgets::ButtonSize::Small).tooltip("What Microsoft, the makers and the law say about finding, installing and starting programs, their licences and their logos, as researched on 19 October 2026, in the Library.").show(ui, theme);
    });
    ui.label(RichText::new(SAFETY_LINE).size(theme.font_size_small).color(theme.text_primary()));
    if cfg!(windows) {
        widgets::body_hint(ui, theme, FOUND_LINE);
    } else {
        widgets::body_hint(ui, theme, WINDOWS_ONLY);
    }
    ui.label(RichText::new(NOT_AFFILIATED).size(theme.font_size_small).color(theme.text_muted()));
    if state.tools.found.is_none() {
        ui.label(RichText::new(READING).size(theme.font_size_small).color(theme.text_muted()));
    }
    if let Some((words, problem)) = &state.tools.status {
        let color = if *problem { theme.warning() } else { theme.text_secondary() };
        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    }
    ui.add_space(theme.spacing_sm);
    if refresh {
        state.tools.status = None;
        start_reading(state);
    }
    if why {
        if crate::gui::pages::library::show_doc(state, FINDING_SLUG) {
            state.push_nav_to(crate::gui::GuiPage::Library);
        } else {
            state.tools.say(format!("The Library on this computer does not have that document. It is {FINDING_PATH} in the project's files."), true);
        }
    }
}

// pub(crate): the Tools snapshots (gui/ui_snapshots.rs) build on its fixture.
#[cfg(test)]
#[path = "tools_tests.rs"]
pub(crate) mod tests;
