//! The state behind Settings > Audio > Instruments, and the frame step that acts on its buttons:
//! the star catalog's request-and-thread pattern (the section sets a request, [`tick`], called once
//! a frame by the engine, which owns the data folder, starts the work).
//!
//! The state lives here, in [`shared`], rather than on `GuiState`: `src/gui/mod.rs` sits at its
//! line budget (`tests/file_size_ratchet.rs`), and only two callers touch it, both on the main
//! thread and never at once (the Settings section while it draws, `tick` after the pages).
//!
//! What it reads (the catalog, the parts, which packs are installed, the instruments available) is
//! read again after every install and removal; with the Dev page's "turn off all reuse" switch off
//! it is read again every frame the section is on screen, so nothing it shows can be stale. The
//! memory each set takes (super::memory, 2026-11-14) reads thousands of file headers, so it is
//! counted on its own thread: when the section first shows, after each install or removal (tagged
//! with the install generation), and with the switch off, each time the section opens again.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use super::catalog::{packs_dir, PackCatalog, Parts};
use super::install::{self, InstalledPack, ProgressHandle, Stage};
use super::library::InstrumentLibrary;

/// Overrides the catalog's download address, for testing packs served from this computer before
/// they are uploaded (`HUMANITY_INSTRUMENT_PACKS_URL=http://127.0.0.1:8000/`). A dev aid only.
pub const URL_OVERRIDE_ENV: &str = "HUMANITY_INSTRUMENT_PACKS_URL";

/// Everything the section shows and asks for.
#[derive(Debug, Default)]
pub struct PacksUi {
    /// Read at least once.
    pub loaded: bool,
    /// The section drew this frame (so `tick` knows the lists are wanted).
    pub shown: bool,
    pub catalog: Option<PackCatalog>,
    pub parts: Option<Parts>,
    /// Why the catalog or the parts could not be read.
    pub load_error: Option<String>,
    pub installed: BTreeMap<String, InstalledPack>,
    pub library: InstrumentLibrary,
    /// The parts the music plays (its moods' and styles' roles and palettes name them,
    /// `score::named_parts`): a part outside it is shown as not used yet (BUG-266). None when the
    /// music's data did not load.
    pub used_parts: Option<BTreeSet<String>>,
    /// The pack downloading now, and its progress (kept after a failure until the next try).
    pub download: Option<(String, ProgressHandle)>,
    /// Requests from the section, acted on by `tick`.
    pub request_download: Option<String>,
    pub request_remove: Option<String>,
    /// The last removal that failed: (pack, why).
    pub remove_error: Option<(String, String)>,
    /// The memory each set's instruments take while its parts play (super::memory), by set ("core"
    /// or a pack's id), and the install generation it was counted for.
    pub memory: BTreeMap<String, super::memory::SetMemory>,
    pub memory_for: Option<u64>,
    /// Counting it now, on its own thread.
    pub memory_job: Option<std::thread::JoinHandle<BTreeMap<String, super::memory::SetMemory>>>,
    /// The section drew last frame (so `tick` knows when it opens again).
    pub was_shown: bool,
    /// The first read, on its own thread (`tick`).
    pub read_job: Option<std::thread::JoinHandle<PacksRead>>,
}

/// What the section shows, read from the data folder in one go: on the frame after an install or a
/// removal ([`PacksUi::reload`]), on its own thread the first time ([`tick`]).
#[derive(Debug)]
pub struct PacksRead {
    catalog: Result<PackCatalog, String>,
    parts: Result<Parts, String>,
    installed: BTreeMap<String, InstalledPack>,
    library: InstrumentLibrary,
    used_parts: Option<BTreeSet<String>>,
}

impl PacksRead {
    /// Read the catalog, the parts, the installed packs, the instruments and the parts the music uses.
    pub fn from_disk(data_dir: &Path) -> PacksRead {
        PacksRead {
            catalog: PackCatalog::load(data_dir),
            parts: Parts::load(data_dir),
            installed: install::installed_packs(&packs_dir(data_dir)),
            library: InstrumentLibrary::load(data_dir),
            used_parts: humanity_music::MusicLibrary::load(data_dir).ok().map(|m| super::score::named_parts(&m).into_iter().map(|(p, _, _)| p).collect()),
        }
    }
}

impl PacksUi {
    /// Read the catalog, the parts, the installed packs and the instruments from `data_dir`.
    pub fn reload(&mut self, data_dir: &Path) {
        self.apply(PacksRead::from_disk(data_dir));
    }

    /// Take what was read.
    fn apply(&mut self, read: PacksRead) {
        let mut errors = Vec::new();
        self.catalog = read.catalog.map_err(|e| errors.push(e)).ok();
        self.parts = read.parts.map_err(|e| errors.push(e)).ok();
        self.load_error = if errors.is_empty() { None } else { Some(errors.join("; ")) };
        self.installed = read.installed;
        self.library = read.library;
        for p in &self.library.problems {
            log::warn!("[Instruments] {p}");
        }
        self.used_parts = read.used_parts;
        self.loaded = true;
    }

    /// Is a download moving now (not finished, not failed)?
    pub fn downloading(&self) -> bool {
        self.download.as_ref().and_then(|(_, p)| p.lock().ok().map(|g| !matches!(g.stage, Stage::Done | Stage::Failed(_)))).unwrap_or(false)
    }

    /// Is `id` installed at the catalog's version?
    pub fn is_current(&self, id: &str) -> bool {
        match (self.installed.get(id), self.catalog.as_ref().and_then(|c| c.pack(id))) {
            (Some(i), Some(p)) => i.version == p.version,
            _ => false,
        }
    }
}

/// Bumped each time an install or a removal finishes: the live music compares it with the one its
/// score was built from and builds again, taking over at the next phrase (engine/music.rs, BUG-266).
static GENERATION: AtomicU64 = AtomicU64::new(0);

/// How many installs and removals have finished this run.
pub fn installed_generation() -> u64 {
    GENERATION.load(Ordering::Relaxed)
}

/// An install or a removal finished: what is installed changed.
pub fn installed_changed() {
    GENERATION.fetch_add(1, Ordering::Relaxed);
}

/// The section's state for this run. Locked by the Settings section while it draws and by `tick`
/// once a frame; a poisoned lock (a panic while drawing) is taken over rather than failing forever.
pub fn shared() -> std::sync::MutexGuard<'static, PacksUi> {
    static STATE: std::sync::OnceLock<std::sync::Mutex<PacksUi>> = std::sync::OnceLock::new();
    STATE.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The base address packs download from: the override when set (a local test server), else the
/// catalog's.
pub fn base_url(catalog: &PackCatalog) -> String {
    std::env::var(URL_OVERRIDE_ENV).ok().filter(|v| !v.is_empty()).unwrap_or_else(|| catalog.base_url.clone())
}

/// The frame step: read what the section shows when needed, start a requested download on its own
/// thread, remove a requested pack, and notice a finished download.
pub fn tick(state: &mut PacksUi, data_dir: &Path) {
    let shown = std::mem::take(&mut state.shown);
    let opened = shown && !state.was_shown;
    state.was_shown = shown;
    // The first read on its own thread (2026-11-21): every instrument's definition, a look for each
    // of its recordings on disk and the music's whole library, which held the boot frame 85 to 92 ms
    // on a quiet PC and was part of a 3,037 ms boot frame beside other agents' compiles (the long-gap
    // lines of `just verify-phone-start`, engine/world_prep.rs). The section shows its introduction
    // alone until it lands. With the reuse switch off it is read again each frame the section shows,
    // on the frame, as before.
    if state.read_job.as_ref().is_some_and(|j| j.is_finished()) {
        match state.read_job.take().map(|j| j.join()) {
            Some(Ok(read)) => state.apply(read),
            _ => {
                state.load_error = Some("the instruments' reading thread stopped".to_string());
                state.loaded = true;
            }
        }
    }
    if !state.loaded && state.read_job.is_none() {
        let dir = data_dir.to_path_buf();
        match std::thread::Builder::new().name("instrument-packs-read".into()).spawn(move || PacksRead::from_disk(&dir)) {
            Ok(j) => state.read_job = Some(j),
            Err(e) => {
                log::warn!("[Instruments] could not read the instruments on their own thread ({e}); reading them now");
                state.reload(data_dir);
            }
        }
    } else if state.loaded && shown && !humanity_core::reuse::enabled() {
        state.reload(data_dir);
    }
    // The memory each set takes (super::memory): counted on its own thread when the section first
    // shows and after each install or removal; with reuse off, again each time the section opens.
    if state.memory_job.as_ref().is_some_and(|j| j.is_finished()) {
        if let Some(Ok(m)) = state.memory_job.take().map(|j| j.join()) {
            state.memory = m;
        }
    }
    let stale = state.memory_for != Some(installed_generation()) || (opened && !humanity_core::reuse::enabled());
    if shown && stale && state.memory_job.is_none() {
        let dir = data_dir.to_path_buf();
        state.memory_for = Some(installed_generation());
        match std::thread::Builder::new().name("instrument-memory".into()).spawn(move || super::memory::by_set(&dir)) {
            Ok(j) => state.memory_job = Some(j),
            Err(e) => log::warn!("[Instruments] could not count the instruments' memory: {e}"),
        }
    }
    if let Some(id) = state.request_download.take() {
        if !state.downloading() {
            if let Some(entry) = state.catalog.as_ref().and_then(|c| c.pack(&id)).cloned() {
                let progress = install::new_progress();
                state.download = Some((id.clone(), progress.clone()));
                let url = entry.url(&base_url(state.catalog.as_ref().expect("the catalog was read")));
                let dir = packs_dir(data_dir);
                log::info!("[Instruments] downloading the {} pack from {url}", entry.id);
                std::thread::spawn(move || {
                    match install::download_and_install(&url, &entry, &dir, &progress) {
                        Ok(()) => log::info!("[Instruments] the {} pack is installed", entry.id),
                        Err(f) => log::warn!("[Instruments] the {} pack did not install: {f:?}", entry.id),
                    }
                });
            }
        }
    }
    if let Some(id) = state.request_remove.take() {
        match install::remove_pack(&packs_dir(data_dir), &id) {
            Ok(()) => {
                state.remove_error = None;
                log::info!("[Instruments] the {id} pack was removed; its parts fall back to the next instrument or to synthesis");
            }
            Err(e) => state.remove_error = Some((id, e)),
        }
        installed_changed();
        state.reload(data_dir);
    }
    let finished = state.download.as_ref().is_some_and(|(_, p)| p.lock().map(|g| g.stage == Stage::Done).unwrap_or(false));
    if finished {
        state.download = None;
        installed_changed();
        state.reload(data_dir);
    }
}
