//! Epic covers on the Apps page, OFF unless the person turns them on (2026-10-19; the operator:
//! "can we not find an image for fortnite so it's not just simple text?").
//!
//! An Epic game's details carry the ADDRESSES of its covers on Epic's servers, read from the
//! launcher's own catalog on this computer (games/epic_covers.rs). With the switch on (it is off
//! by default, and kept in the settings, config.json `apps_epic_covers`), the pictures of the Epic
//! games installed on this computer are fetched from Epic's own picture servers, each address once
//! a session, on a worker thread, through `games::epic_covers::fetch`, decoded, and made textures
//! for this session only. NOTHING IS WRITTEN anywhere: no cache, no copy (the dated finding
//! docs/reference/findings/2026-10-19-games-page-art-stats-and-links.md, judgement call 1 and the
//! dated note at its end; CLAUDE.md's no-caches rule). Turned off, the textures are let go and no
//! more is asked. The switch's own words say what a request tells Epic.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, TryRecvError};

use crate::games::{epic_covers, Game, Store};
use crate::gui::GuiState;

/// The switch, in plain words: what it does and what it tells Epic.
pub const SWITCH: &str = "Get covers for Epic games from Epic's servers (this tells Epic which of your games' pictures were looked up)";
/// Its hover words.
pub const SWITCH_TIP: &str = "Off unless you turn it on. When on, the covers of the Epic games installed here are fetched from Epic's own picture servers, kept in memory while HumanityOS is open, and never saved.";
/// The largest side a fetched cover is kept at (it is drawn far smaller).
const KEEP_SIDE: u32 = 1280;

/// A cover that came back, decoded.
struct Picture {
    width: usize,
    height: usize,
    rgba: Vec<u8>,
}

/// The covers fetched this session, kept in `AppsState::covers`.
#[derive(Default)]
pub struct EpicCovers {
    /// What came back, as textures, by address: in memory for this session only.
    pub(crate) textures: HashMap<String, egui::TextureHandle>,
    /// The addresses asked for this session (each once).
    asked: HashSet<String>,
    /// The worker fetching, and what it sends back.
    rx: Option<Receiver<(String, Result<Picture, String>)>>,
    /// How many could not be fetched or read.
    pub failed: usize,
    /// How many fetch jobs were started (the tests count them).
    pub jobs_started: u32,
}

impl EpicCovers {
    /// `game`'s tile cover, when it came back.
    pub(crate) fn tile_of(&self, game: &Game) -> Option<(egui::TextureId, egui::Vec2)> {
        let url = game.details.art.epic_covers.as_ref()?.tile.as_ref()?;
        self.textures.get(url).map(|t| (t.id(), t.size_vec2()))
    }

    /// `game`'s banner, when it came back (its tile's picture when it has no banner of its own).
    pub(crate) fn banner_of(&self, game: &Game) -> Option<(egui::TextureId, egui::Vec2)> {
        let covers = game.details.art.epic_covers.as_ref()?;
        covers.banner.as_ref().or(covers.tile.as_ref()).and_then(|url| self.textures.get(url)).map(|t| (t.id(), t.size_vec2()))
    }

    /// Whether `game` has covers to fetch (an Epic game the catalog lists).
    pub(crate) fn has_covers(game: &Game) -> bool {
        game.details.art.epic_covers.as_ref().is_some_and(|c| !c.urls().is_empty())
    }
}

/// The addresses wanted: each installed Epic game's covers, on Epic's own hosts, each once.
pub(crate) fn wanted(games: &[Game]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for g in games.iter().filter(|g| g.store == Store::Epic) {
        for url in g.details.art.epic_covers.iter().flat_map(|c| c.urls()) {
            if epic_covers::host_allowed(&url) && !out.contains(&url) {
                out.push(url);
            }
        }
    }
    out
}

/// Each frame: with the switch off, let go of everything and ask nothing; with it on, take what
/// came back and ask for the covers not asked yet (one job at a time).
pub(super) fn update(ctx: &egui::Context, state: &mut GuiState) {
    let covers = &mut state.apps.covers;
    if !state.settings.apps_epic_covers {
        if !covers.textures.is_empty() || !covers.asked.is_empty() || covers.rx.is_some() {
            *covers = EpicCovers { jobs_started: covers.jobs_started, ..EpicCovers::default() };
        }
        return;
    }
    if let Some(rx) = &covers.rx {
        loop {
            match rx.try_recv() {
                Ok((url, Ok(p))) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied([p.width, p.height], &p.rgba);
                    let texture = ctx.load_texture(format!("epic-cover:{url}"), image, egui::TextureOptions::LINEAR);
                    covers.textures.insert(url, texture);
                }
                Ok((_, Err(e))) => {
                    covers.failed += 1;
                    log::info!("apps: an Epic cover was not fetched: {e}");
                }
                Err(TryRecvError::Empty) => {
                    ctx.request_repaint_after(std::time::Duration::from_millis(150));
                    break;
                }
                Err(TryRecvError::Disconnected) => {
                    covers.rx = None;
                    break;
                }
            }
        }
    }
    if covers.rx.is_some() {
        return;
    }
    let new: Vec<String> = wanted(&state.apps.games.all).into_iter().filter(|u| !covers.asked.contains(u)).collect();
    if new.is_empty() {
        return;
    }
    covers.asked.extend(new.iter().cloned());
    covers.jobs_started += 1;
    covers.rx = Some(start(new));
}

/// Fetch and decode `urls` on a worker thread. Test builds do it here, on this thread, so the
/// fetch recorder (per thread) sees every address asked.
fn start(urls: Vec<String>) -> Receiver<(String, Result<Picture, String>)> {
    let (tx, rx) = std::sync::mpsc::channel();
    #[cfg(test)]
    job(urls, tx);
    #[cfg(not(test))]
    {
        let _ = std::thread::Builder::new().name("apps-epic-covers".into()).spawn(move || job(urls, tx));
    }
    rx
}

fn job(urls: Vec<String>, tx: std::sync::mpsc::Sender<(String, Result<Picture, String>)>) {
    for url in urls {
        let got = epic_covers::fetch(&url).and_then(|bytes| decode(&bytes));
        if tx.send((url, got)).is_err() {
            return;
        }
    }
}

/// A fetched picture decoded into memory, at most [`KEEP_SIDE`] on its longer side.
fn decode(bytes: &[u8]) -> Result<Picture, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("it is not a picture this app reads ({e})"))?;
    let img = if img.width() > KEEP_SIDE || img.height() > KEEP_SIDE { img.thumbnail(KEEP_SIDE, KEEP_SIDE) } else { img };
    let rgba = img.to_rgba8();
    Ok(Picture { width: rgba.width() as usize, height: rgba.height() as usize, rgba: rgba.into_raw() })
}

/// The switch, turned to `on`: kept in the settings (saved), and the page says what it does.
pub(super) fn set(state: &mut GuiState, on: bool) {
    if state.settings.apps_epic_covers != on {
        state.settings.apps_epic_covers = on;
        state.settings_dirty = true;
    }
}

/// Test builds: whether this decodes, for the by-hand check of a real cover (see apps/tests.rs).
#[cfg(test)]
pub(crate) fn decodes(bytes: &[u8]) -> Option<(usize, usize)> {
    decode(bytes).ok().map(|p| (p.width, p.height))
}
