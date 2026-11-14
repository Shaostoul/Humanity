//! THE MUSIC PAGE (docs/design/music-page.md; the operator, 2026-11-13: "a dedicated music page to
//! try infinite seeds and variations and genres and what not ... set some as permanent ... it'd be
//! cool if we could see the song in some sort of music sheet kind of thing and have a visualizer
//! that moves around to the beat").
//!
//! One scroll in the order a person uses it: Now playing (the visualizer, the piece in words, Play
//! here), Explore (genre, mood, seed), Tune (key, tempo, lead voice, each part's level and mute),
//! Keep (favourites, "my music" for a mood, share codes, export to WAV) and See (the piano roll with
//! chord symbols and section letters).
//!
//! The page plays nothing itself: while it is open with Play here on, the game's one score plays its
//! choice (engine/music.rs reads [`MusicPage::choice`]), taking over at the next bar. The engine
//! reports back what plays (`MusicReadout::playing`: the seed, the context, the instruments), and the
//! piano roll writes that piece down with the composer run dry (humanity_music::sheet), kept in memory
//! tagged by what it was made from and made again every frame with "turn off all reuse" off.
//!
//! The favourites are the person's own file, `music/favourites.json` beside config.json, read once
//! when the music first runs (so "my music" plays without the page ever being opened) and written
//! whenever they change.

mod controls;
mod view;

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, Frame, RichText, ScrollArea};
use humanity_core::i18n::{t, t_dyn};
use humanity_music::favourite::{explore_intensity, Favourite, Favourites};
use humanity_music::sheet::Sheet;
use humanity_music::tune::Tune;
use humanity_music::MusicLibrary;

use crate::gui::pages::dev_music::MusicChoice;
use crate::gui::theme::Theme;
use crate::gui::GuiState;

/// The export lengths offered, minutes (docs/design/music-page.md section 8).
pub const EXPORT_MINUTES: [f32; 6] = [1.0, 3.0, 5.0, 10.0, 30.0, 60.0];

/// The largest seed Random draws (seeds under a million are easy to read out and to type).
pub const RANDOM_SEED_MAX: u64 = 999_999;

/// An export the page asked for: the engine renders it on its own thread (engine/music.rs).
#[derive(Clone, Debug, PartialEq)]
pub struct ExportJob {
    pub favourite: Favourite,
    pub minutes: f32,
    pub path: PathBuf,
}

/// Where an export stands.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum ExportStatus {
    #[default]
    Idle,
    Working { name: String },
    Done { path: PathBuf },
    Failed { why: String },
}

/// The piano roll's transcription, tagged by what it was made from.
#[derive(Clone)]
pub struct SheetCache {
    pub tag: SheetTag,
    pub sheet: Sheet,
}

/// What a transcription was made from: the piece (seed and composing context), the instruments it
/// plays (the playing score's copy, by address) and how far it reaches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetTag {
    pub seed: u64,
    pub res: usize,
    pub steer: [u32; 6],
    pub tune: Tune,
    pub seconds: u32,
}

/// The Music page's state.
#[derive(Clone)]
pub struct MusicPage {
    /// The genre and mood explored (ids; empty: not chosen yet, the first draw fills them), the seed
    /// and its box, and the tuning.
    pub genre: String,
    pub mood: String,
    pub seed: u64,
    pub seed_text: String,
    pub tune: Tune,
    /// The game's music plays the page's choice while the page is open.
    pub play_here: bool,
    /// The person's favourites, whether they were read (and why not), and the file's place.
    pub favourites: Arc<Favourites>,
    pub fav_loaded: bool,
    pub fav_error: Option<String>,
    pub fav_path: Option<PathBuf>,
    /// The name box for a new favourite, the share box, and what the last paste said.
    pub name_text: String,
    pub share_text: String,
    pub share_msg: Option<String>,
    /// The export length chosen, the export asked for (taken by the engine) and where it stands.
    pub export_minutes: f32,
    pub export_request: Option<ExportJob>,
    pub export_status: ExportStatus,
    /// The piano roll's transcription.
    pub sheet: Option<SheetCache>,
    /// The export folder shown and used instead of export_dir(), for the page snapshot only, so a
    /// committed picture never carries the machine's own home folder (None: export_dir()).
    pub export_dir_override: Option<PathBuf>,
}

impl Default for MusicPage {
    fn default() -> Self {
        MusicPage {
            genre: String::new(),
            mood: String::new(),
            seed: 0,
            seed_text: String::new(),
            tune: Tune::default(),
            play_here: true,
            favourites: Arc::default(),
            fav_loaded: false,
            fav_error: None,
            fav_path: None,
            name_text: String::new(),
            share_text: String::new(),
            share_msg: None,
            export_minutes: 3.0,
            export_request: None,
            export_status: ExportStatus::Idle,
            sheet: None,
            export_dir_override: None,
        }
    }
}

/// The person's favourites file: `music/favourites.json` beside config.json (the data folder,
/// `HUMANITY_DATA_DIR` and portable mode included).
pub fn favourites_path() -> PathBuf {
    let cfg = crate::config::AppConfig::config_path();
    cfg.parent().map(|p| p.to_path_buf()).unwrap_or_default().join("music").join("favourites.json")
}

/// Where exports go: `Music\HumanityOS` in the person's home folder (the design's question 2), else
/// beside the favourites.
pub fn export_dir() -> PathBuf {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).map(PathBuf::from);
    match home {
        Some(h) if h.join("Music").is_dir() => h.join("Music").join("HumanityOS"),
        _ => favourites_path().parent().map(|p| p.join("exports")).unwrap_or_else(|| PathBuf::from("exports")),
    }
}

/// A file name for an export: the favourite's name made safe for every file system, its seed and
/// its length.
pub fn export_file_name(f: &Favourite, minutes: f32) -> String {
    let safe: String = f.name.chars().map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' }).collect();
    let safe = safe.trim();
    let safe = if safe.is_empty() { "HumanityOS music" } else { safe };
    format!("{safe} (seed {}, {} min).wav", f.seed, minutes as u32)
}

impl MusicPage {
    /// Read the favourites file once (the engine's first music frame; never in a snapshot).
    pub fn load_favourites(&mut self) {
        if self.fav_loaded {
            return;
        }
        self.fav_loaded = true;
        let path = favourites_path();
        match Favourites::load(&path) {
            Ok(f) => self.favourites = Arc::new(f),
            Err(e) => {
                log::warn!("[Music] {e}");
                self.fav_error = Some(e);
            }
        }
        self.fav_path = Some(path);
    }

    /// Write the favourites (after any change), saying so on the page when it fails.
    pub fn save_favourites(&mut self) {
        let Some(path) = self.fav_path.clone() else { return };
        match self.favourites.save(&path) {
            Ok(()) => self.fav_error = None,
            Err(e) => {
                log::warn!("[Music] {e}");
                self.fav_error = Some(e);
            }
        }
    }

    /// The genre explored: the page's, else `fallback` (Settings' genre).
    pub fn genre_or<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.genre.is_empty() {
            fallback
        } else {
            &self.genre
        }
    }

    /// What the game's music plays while the page is open with Play here on.
    pub fn choice(&self, lib: Option<&MusicLibrary>, settings_genre: &str) -> Option<MusicChoice> {
        if !self.play_here || self.seed == 0 {
            return None;
        }
        let mood = if self.mood.is_empty() { humanity_music::library::DEFAULT_MOOD.to_string() } else { self.mood.clone() };
        let intensity = lib.map(|l| explore_intensity(l, &mood)).unwrap_or(0.3);
        Some(MusicChoice { genre: self.genre_or(settings_genre).to_string(), mood: Some(mood), intensity: Some(intensity), seed: self.seed, tune: self.tune })
    }

    /// Set the seed (and its box).
    pub fn set_seed(&mut self, seed: u64) {
        self.seed = seed.max(1);
        self.seed_text = self.seed.to_string();
    }

    /// Load a favourite into the page (Play on its row, or a pasted code).
    pub fn open_favourite(&mut self, f: &Favourite, lib: Option<&MusicLibrary>) {
        self.genre = f.genre.clone();
        self.mood = f.mood.clone();
        self.set_seed(f.seed);
        self.tune = lib.and_then(|l| l.genre_index(&f.genre).map(|g| f.tune.to_tune(l, g))).unwrap_or_default();
        self.name_text = f.name.clone();
        self.play_here = true;
    }
}

/// A favourite as "my music" plays it: its genre, seed and tuning in the game's own mood.
pub fn my_music_choice(f: &Favourite, lib: &MusicLibrary) -> Option<MusicChoice> {
    let g = lib.genre_index(&f.genre)?;
    Some(MusicChoice { genre: f.genre.clone(), mood: None, intensity: None, seed: f.seed, tune: f.tune.to_tune(lib, g) })
}

/// A lead voice's words: its part's (`instrument_packs.part.<id>`), else its id made readable.
pub fn voice_words(id: &str) -> String {
    let key = format!("instrument_packs.part.{id}");
    let w = t_dyn(&key);
    if w != key {
        w.to_string()
    } else {
        id.replace('_', " ")
    }
}

/// A mood's words (`music.mood.<id>` where the data names it), else its id made readable.
pub fn mood_words(lib: Option<&MusicLibrary>, id: &str) -> String {
    if let Some(m) = lib.and_then(|l| l.moods.iter().find(|m| m.id == id)) {
        if !m.words.is_empty() {
            let w = t_dyn(&m.words);
            if w != m.words {
                return w.to_string();
            }
        }
    }
    id.replace('_', " ")
}

/// A role's colour (theme tokens, one a role).
pub fn role_colour(theme: &Theme, role: u8) -> Color32 {
    match role {
        0 => theme.music_role_pad(),
        1 => theme.music_role_bass(),
        2 => theme.music_role_drone(),
        3 => theme.music_role_choir(),
        4 => theme.music_role_bells(),
        5 => theme.music_role_melody(),
        6 => theme.music_role_percussion(),
        7 => theme.music_role_texture(),
        _ => theme.text_muted(),
    }
}

/// A role's words (`music_page.role.<id>`).
pub fn role_words(role: usize) -> &'static str {
    humanity_music::tune::ROLE_IDS.get(role).map(|id| t_dyn(&format!("music_page.role.{id}"))).unwrap_or("")
}

/// Draw the Music page.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let accent = theme.nav_cat_apps();
    // The first look: what plays now (a person opening the page to ask "what is this?"), else the
    // first seed of the person's genre.
    if state.music_page.seed == 0 {
        let playing = state.music_dev.readout.playing.as_ref().map(|p| (p.seed, p.choice.as_ref().map(|c| c.genre.clone())));
        let seed = playing.as_ref().map(|p| p.0).filter(|&s| s > 0).unwrap_or(1);
        state.music_page.set_seed(seed);
        if let Some((_, Some(g))) = playing {
            state.music_page.genre = g;
        }
    }
    egui::CentralPanel::default().frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(16.0)).show(ctx, |ui| {
        ScrollArea::vertical().id_salt("music_page_scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new(t("music_page.title")).size(theme.font_size_title).color(theme.text_primary()));
            ui.label(RichText::new(t("music_page.intro")).color(theme.text_secondary()));
            ui.add_space(theme.spacing_sm);
            view::now_playing(ui, theme, state, accent);
            controls::explore(ui, theme, state, accent);
            controls::tune(ui, theme, state, accent);
            controls::keep(ui, theme, state, accent);
            view::see(ui, theme, state, accent);
        });
    });
}
