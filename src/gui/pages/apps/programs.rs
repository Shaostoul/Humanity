//! The Apps page's other programs, its blue part, and its help services, cyan and magenta (the
//! Tools page until 2026-10-19; the operator: "How can we beautify this tools page? What if we
//! make the tools page like the games page? Then what if through that interface users can
//! install/launch/go to the website(s) for the various non-game apps?"; part of the Apps page since
//! the same day: "Blue could be third party tools").
//!
//! Both come from the single external catalog, data/external/catalog.json, via
//! `state.external_catalog`, loaded at startup (software/catalog.rs): `software`, free programs you
//! download and run yourself (Blender, GIMP...), here; `service`, real-world help websites and
//! organizations, in the Help and Services parts (each category's "section" in the data). The
//! website's Tools page (web/pages/tools.html)
//! reads the same file.
//!
//! What is installed is read by src/software/ on a worker thread when the page opens and on
//! Refresh, never every frame (Windows' own records, the Start menu's shortcuts and Steam's, read
//! only). Launch goes through the one launcher (games::launch), and only on a click. Install runs
//! winget, Windows' own package manager, only after the consent step in the details and the
//! person's own "I agree, install" (software/install.rs, on a worker; one install at a time), then
//! looks again. A web page opens in the person's browser only on a click. What may be done is set
//! by the dated finding docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md,
//! which the part's Why opens in the Library.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use crate::games;
use crate::gui::GuiState;
use crate::software::{self, install, Found, Installed, CatalogEntry, Via};

/// Where the programs come from.
pub const WHERE_FROM: &str = "Each program comes from its maker: Download and Website open the maker's own pages in your browser, and Install uses the maker's own installer through winget, Windows' own package manager, after you agree to its licence.";
/// The safety line (the 2026-10-19 finding, section 6 and judgement call 7: Microsoft, the FTC and
/// the makers say the same).
pub const SAFETY_LINE: &str = "Get programs from their makers, never from an advert, even a top search result. The programs here are free from their makers' own sites; some makers also sell store copies to fund their work, and anyone else asking you to pay for one is not the maker.";
/// How what is installed is found, and what the pictures are.
pub const FOUND_LINE: &str = "Which programs are installed is read from Windows' own list of installed programs, its Start menu and Steam, on this computer only, and never sent anywhere; Windows has no sure way to find every program, so one not found may still be there. An installed program shows the icon in its own file; no logo is fetched from any website, because each maker sets its own rules for its logo.";
/// Said once, quietly (the finding's judgement call 6).
pub const NOT_AFFILIATED: &str = "HumanityOS is not made by, or affiliated with, the makers of these programs.";
/// Help's line (the cyan part): urgent and protective needs.
pub const HELP_LINE: &str = "Help when it is urgent or you need protecting: crisis lines, health, emergencies and legal aid. Each opens its own website in your browser; in danger, call your local emergency number first.";
/// Services' line (the magenta part): everyday needs.
pub const SERVICES_LINE: &str = "Services for everyday needs: water, food, housing, money, work and learning. Each opens its own website in your browser. Everything here is external: we point, you choose.";
/// Said for a program not found (never "not installed": Windows has no sure way to find every one).
pub const NOT_FOUND: &str = "Not found on this computer";
/// The finding's Library slug, and where it is in the project's files.
pub const FINDING_SLUG: &str = "2026-10-19-tools-page-install-launch-and-icons";
pub const FINDING_PATH: &str = "docs/reference/findings/2026-10-19-tools-page-install-launch-and-icons.md";
/// The consent step's button: it says what it does.
pub const AGREE_INSTALL: &str = "I agree, install";
/// Said where finding what is installed does not work.
pub const WINDOWS_ONLY: &str = "Finding which programs are installed works on Windows for now.";
pub const READING: &str = "Looking for the programs installed on this computer...";
pub const LAUNCH: &str = "Launch";
pub const DOWNLOAD: &str = "Download";
pub const OPEN_WEBSITE: &str = "Open website";
pub const WEBSITE: &str = "Website";
pub const DOWNLOAD_PAGE: &str = "Download page";
pub const INSTALL: &str = "Install";
pub const INSTALLED: &str = "Installed";
/// Said for an installed program whose program file was not found.
pub const NO_PROGRAM: &str = "Windows has no record of where its program is, and no Start-menu shortcut to it was found, so start it from wherever you usually do.";
/// A tile's few words for the same.
pub const NO_PROGRAM_SHORT: &str = "Start it from its shortcut";
/// A second Launch on the same program this soon after the first is the same press.
const AGAIN_AFTER: Duration = Duration::from_secs(2);

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

/// The programs' state, kept in `AppsState::programs`.
#[derive(Default)]
pub struct ProgramsPart {
    /// Where what is installed is read from.
    pub sources: Where,
    /// What was found when last read; `None` until the first read finishes.
    pub found: Option<Found>,
    /// A read under way, on a worker thread.
    pub(super) reading: Option<Receiver<Found>>,
    /// How many reads have been started (the tests count them: never one per frame).
    pub reads_started: u32,
    /// Counts the reads that came in (the order and the icons are worked out again after each).
    pub(super) generation: u64,
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
    pub(crate) last_launch: Option<(usize, Instant)>,
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

impl ProgramsPart {
    /// How catalog entry `i` is installed, when it is.
    pub(crate) fn installed(&self, i: usize) -> Option<&Installed> {
        self.found.as_ref().and_then(|f| f.get(i))
    }

    /// Entry `i`'s program icon as a texture, with its size, when it has one.
    pub(crate) fn icon_of(&self, i: usize) -> Option<(egui::TextureId, egui::Vec2)> {
        self.icons.get(&i).map(|t| (t.id(), t.size_vec2()))
    }

    /// Whether winget is installing entry `i` now.
    pub(crate) fn installing(&self, i: usize) -> bool {
        matches!(&self.install, Some(Installing::Running { entry, .. }) if *entry == i)
    }
}

/// The order of `catalog`'s entries of `kind` ("software" or "service") and, for a help service,
/// of the page's `part` ("help" or "services": its category's `section` in the data), narrowed to
/// `category` (`None`: every one), to the installed ones when `installed_only`, and to those whose
/// name, category or description hold every word of `search` (letters' case ignored); sorted by
/// `sort`. Installed first needs `found`; without it (still reading) it is by name. The sorts that
/// are a game's (played, size) are by name here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn order_of(catalog: &[CatalogEntry], found: Option<&Found>, kind: &str, part: Option<&str>, category: Option<&str>, installed_only: bool, sort: super::SortBy, search: &str) -> Vec<usize> {
    use super::SortBy;
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let installed = |i: usize| found.and_then(|f| f.get(i)).is_some();
    let mut out: Vec<usize> = catalog
        .iter()
        .enumerate()
        .filter(|(_, e)| e.kind == kind)
        .filter(|(_, e)| part.is_none_or(|p| e.section == p))
        .filter(|(_, e)| category.is_none_or(|c| e.category_id == c))
        .filter(|(i, _)| !installed_only || installed(*i))
        .filter(|(_, e)| {
            let text = format!("{}\n{}\n{}", e.name, e.category, e.description).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .map(|(i, _)| i)
        .collect();
    let by_name = |a: usize, b: usize| catalog[a].name.to_lowercase().cmp(&catalog[b].name.to_lowercase()).then_with(|| catalog[a].category.cmp(&catalog[b].category)).then_with(|| a.cmp(&b));
    match sort {
        SortBy::InstalledFirst => out.sort_by(|&a, &b| installed(b).cmp(&installed(a)).then_with(|| by_name(a, b))),
        SortBy::Category => out.sort_by(|&a, &b| catalog[a].category.to_lowercase().cmp(&catalog[b].category.to_lowercase()).then_with(|| by_name(a, b))),
        SortBy::Name | SortBy::RecentlyPlayed | SortBy::MostPlayed | SortBy::Size => out.sort_by(|&a, &b| by_name(a, b)),
    }
    out
}

/// Make a texture of each program icon the worker read and that has none yet (once per read:
/// nothing is read from the disk here, the pixels are already in memory).
pub(super) fn ensure_icons(ctx: &egui::Context, page: &mut ProgramsPart) {
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
    if state.apps.programs.reading.is_some() {
        return;
    }
    let sources = state.apps.programs.sources.clone();
    let catalog = state.external_catalog.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    let spawned = std::thread::Builder::new().name("apps-programs-read".into()).spawn(move || {
        let sources = match sources {
            Where::ThisComputer => software::Sources::this_computer(),
            Where::These(s) => s,
        };
        let _ = tx.send(software::read(&sources, &catalog));
    });
    match spawned {
        Ok(_) => {
            state.apps.programs.reading = Some(rx);
            state.apps.programs.reads_started += 1;
        }
        Err(e) => state.apps.say(format!("The installed programs could not be looked for: {e}."), true),
    }
}

/// Take the worker's answer when it is in.
pub(super) fn poll(ctx: &egui::Context, state: &mut GuiState) {
    let Some(rx) = &state.apps.programs.reading else { return };
    match rx.try_recv() {
        Ok(found) => took(state, found),
        Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(100)),
        Err(TryRecvError::Disconnected) => took(state, Found::default()),
    }
}

fn took(state: &mut GuiState, found: Found) {
    let page = &mut state.apps.programs;
    page.found = Some(found);
    page.reading = None;
    page.generation += 1;
    page.icons.clear();
    if std::mem::take(&mut page.read_again) {
        start_reading(state);
        return;
    }
    // After an install: say whether the program is found now.
    if let Some(i) = state.apps.programs.after_install.take() {
        let name = state.external_catalog.get(i).map(|e| e.name.clone()).unwrap_or_default();
        let words = if state.apps.programs.installed(i).is_some() {
            format!("{name} is installed.")
        } else {
            format!("winget finished, but {name} was not found on this computer yet. Refresh looks again.")
        };
        state.apps.programs.install_note = Some((i, words.clone(), false));
        state.apps.say(words, false);
    }
}

/// Test builds: find what is installed now, on this thread, so a snapshot's frames show it (a
/// worker's answer could arrive after them). Reads only a test's own records: `ThisComputer` reads
/// nothing here.
#[cfg(test)]
pub(crate) fn read_now(state: &mut GuiState) {
    let sources = match &state.apps.programs.sources {
        Where::These(s) => s.clone(),
        Where::ThisComputer => software::Sources::default(),
    };
    let found = software::read(&sources, &state.external_catalog);
    took(state, found);
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
pub(crate) fn platforms_words(e: &CatalogEntry) -> String {
    e.platforms.iter().map(|p| platform_words(p)).collect::<Vec<_>>().join(", ")
}

/// What the main button says for an entry not installed (or not startable): Download for a program
/// to download, Open website for one that runs in the browser and for a help service.
pub(crate) fn web_button(e: &CatalogEntry) -> &'static str {
    if e.is_software() && !e.web_only() {
        DOWNLOAD
    } else {
        OPEN_WEBSITE
    }
}

/// Launch's hover words for `found`.
pub(crate) fn launch_tip(found: &Installed) -> &'static str {
    match (found.via, &found.launch) {
        (Via::Steam, _) => "Asks Steam to start it.",
        (_, Some(games::Launch::Shortcut(_))) => "Opens its Start-menu shortcut, as a click on it in the Start menu does.",
        _ => "Starts its program.",
    }
}

/// Launch: start entry `i`'s program through the one launcher, once per press.
pub(crate) fn launch(state: &mut GuiState, i: usize) {
    let Some(entry) = state.external_catalog.get(i).cloned() else { return };
    let Some(installed) = state.apps.programs.installed(i).cloned() else { return };
    let Some(what) = installed.launch.clone() else { return };
    if state.apps.programs.last_launch.is_some_and(|(k, at)| k == i && at.elapsed() < AGAIN_AFTER) {
        return;
    }
    state.apps.programs.last_launch = Some((i, Instant::now()));
    match games::launch(&what) {
        Ok(()) if installed.via == Via::Steam => state.apps.say(format!("Asked Steam to start {}.", entry.name), false),
        Ok(()) => state.apps.say(format!("Started {}.", entry.name), false),
        Err(e) => state.apps.say(format!("{} did not start: {e}.", entry.name), true),
    }
}

/// Open a web page from the catalog in the person's browser (only a web address).
pub(super) fn open_web(ctx: &egui::Context, state: &mut GuiState, url: &str) {
    if url.starts_with("https://") || url.starts_with("http://") {
        ctx.open_url(egui::OpenUrl::new_tab(url));
    } else {
        state.apps.say("That is not a web address this page opens.", true);
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
        if let Err(e) = std::thread::Builder::new().name("apps-winget".into()).spawn(move || install::run(&plan, &tx)) {
            let _ = failed.send(install::Event::Done(Err(install::Failure { code: None, words: format!("winget could not be started ({e}).") })));
        }
    }
    rx
}

/// Install, pressed: first ask winget what it knows about the package (its publisher, licence and
/// agreements), for the consent step. Nothing is installed here. One install at a time.
pub(crate) fn install_pressed(state: &mut GuiState, i: usize) {
    let Some(entry) = state.external_catalog.get(i).cloned() else { return };
    if !entry.installable() || state.apps.programs.installed(i).is_some() {
        return;
    }
    if let Some(busy) = &state.apps.programs.install {
        let name = state.external_catalog.get(busy.entry()).map(|e| e.name.clone()).unwrap_or_default();
        state.apps.say(format!("One install at a time: {name} first."), false);
        return;
    }
    let Some(plan) = entry.winget.as_deref().and_then(install::show_plan) else {
        state.apps.programs.install_note = Some((i, "Its package id in the catalog is not one winget can use.".to_string(), true));
        return;
    };
    state.apps.programs.install_note = None;
    state.apps.programs.install = Some(Installing::Reading { entry: i, rx: start_job(plan) });
}

/// "I agree, install", pressed in the consent step for entry `i`: only now is winget asked to
/// install it, with the person's agreement passed on for that one package.
pub(crate) fn agree(state: &mut GuiState, i: usize) {
    let Some(Installing::Asking { entry, .. }) = &state.apps.programs.install else { return };
    if *entry != i {
        return;
    }
    let Some(e) = state.external_catalog.get(i).cloned() else { return };
    let Some(id) = e.winget.clone() else { return };
    let agreed = install::Agreed { id, interactive: e.install_interactive };
    let Some(plan) = install::install_plan(&agreed) else { return };
    state.apps.programs.install = Some(Installing::Running { entry: i, rx: start_job(plan), line: String::new() });
    state.apps.say(format!("Installing {} with winget.", e.name), false);
}

/// Cancel in the consent step: nothing is installed.
pub(super) fn cancel_install(state: &mut GuiState) {
    if let Some(Installing::Asking { entry, .. } | Installing::Reading { entry, .. }) = &state.apps.programs.install {
        state.apps.programs.install_note = Some((*entry, "Cancelled. Nothing was installed.".to_string(), false));
        state.apps.programs.install = None;
    }
}

/// Take what winget sent since the last frame: the package read (the consent step opens), a line
/// of progress, or its end (look for the program again, or say why it did not install).
pub(super) fn poll_install(ctx: &egui::Context, state: &mut GuiState) {
    let Some(step) = state.apps.programs.install.as_mut() else { return };
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
    let name = state.external_catalog.get(entry).map(|e| e.name.clone()).unwrap_or_default();
    let id = state.external_catalog.get(entry).and_then(|e| e.winget.clone()).unwrap_or_default();
    state.apps.programs.install = None;
    match (reading, done) {
        (true, Ok(text)) => match install::parse_show(&id, &text) {
            Some(package) => state.apps.programs.install = Some(Installing::Asking { entry, package }),
            None => state.apps.programs.install_note = Some((entry, format!("winget did not find the package {id}. Download opens the maker's own page."), true)),
        },
        (true, Err(f)) => state.apps.programs.install_note = Some((entry, f.words, true)),
        (false, Ok(_)) => installed_now(state, entry, format!("winget installed {name}. Looking for it on this computer...")),
        (false, Err(f)) if f.code == Some(install::ALREADY_INSTALLED) => installed_now(state, entry, f.words),
        (false, Err(f)) => {
            state.apps.programs.install_note = Some((entry, f.words.clone(), true));
            state.apps.say(format!("{name} did not install: {}", f.words), true);
        }
    }
}

/// winget says entry `i` is on the computer now: look for it again, so its tile turns to Launch.
fn installed_now(state: &mut GuiState, i: usize, words: String) {
    state.apps.programs.install_note = Some((i, words.clone(), false));
    state.apps.say(words, false);
    state.apps.programs.after_install = Some(i);
    if state.apps.programs.reading.is_none() {
        start_reading(state);
    } else {
        // A read under way began before the install ended: read again once it is in.
        state.apps.programs.read_again = true;
    }
}
