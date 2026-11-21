//! THE MUSIC PAGE (docs/design/music-page.md; the operator, 2026-11-13: "a dedicated music page to
//! try infinite seeds and variations and genres and what not ... set some as permanent ... it'd be
//! cool if we could see the song in some sort of music sheet kind of thing and have a visualizer
//! that moves around to the beat").
//!
//! VERSION 2 (2026-11-20, docs/design/music-page.md sections 5 to 11; the operator: "we have a lot of
//! real estate for the UI. We could condense things"): the Now playing strip along the top, always in
//! view (pause, previous and next piece, what plays, the genre, mood and seed chips, and Like, which
//! keeps the piece in one click: `strip.rs`); then tabs: Listen (What plays, Tune, the piece in words
//! and the visualizer), Explore (a card for each genre and mood in plain words, the seed, the pieces
//! heard: `explore.rs`), Written music (the searchable catalogue: `written.rs`) and Your music
//! (favourites, "my music" for a mood, share codes, export). On a wide page the piano roll (its
//! length slider, the overview, follow the music: `roll.rs`) is docked along the bottom on every tab;
//! on a narrow one (a small window, the phone) it sits in Listen.
//!
//! ONE PLACE CHOOSES THE MUSIC (2026-11-14, section 4; the operator: "When I go to the settings it
//! plays one music and when I go to the music page it goes to another. We should have one place
//! ideally"). This page is it, and what it chose keeps playing after you leave it, until you choose
//! again. The choice is the person's own settings (`MusicSettings`: `genre`, `game_chooses`,
//! `mood`, `seed`, `tune`, saved in config.json, never a cache), the one state the score
//! (engine/music.rs, through [`chosen`]), this page and Settings > Music (through
//! [`now_playing_words`]) read. The page itself plays nothing: the game's one score plays the
//! choice, taking over at the next bar. The engine reports back what plays
//! (`MusicReadout::playing`), and the piano roll writes that piece down with the composer run dry
//! (humanity_music::sheet), kept in memory tagged by what it was made from and made again every
//! frame with "turn off all reuse" off.
//!
//! The favourites are the person's own file, `music/favourites.json` beside config.json, read once
//! when the music first runs (so "my music" plays without the page ever being opened) and written
//! whenever they change.

mod controls;
mod explore;
pub mod files;
pub mod history;
mod keys;
mod mixer;
mod roll;
mod strip;
pub mod styles;
#[cfg(test)]
pub(crate) mod tests;
#[cfg(test)]
mod tests_rung2;
#[cfg(test)]
mod tests_style;
#[cfg(test)]
mod tests_v2;
#[cfg(test)]
mod tests_arc;
mod view;
mod written;

pub use history::Heard;
pub use written::{WrittenQuery, WrittenSort};

use std::path::PathBuf;
use std::sync::Arc;

use egui::{Color32, Frame, RichText, ScrollArea};
use humanity_core::i18n::{t, t_dyn};
use humanity_music::favourite::{explore_intensity, Favourite, Favourites, SavedTune};
use humanity_music::sheet::Sheet;
use humanity_music::tune::Tune;
use humanity_music::{MusicLibrary, MusicSettings};

use crate::gui::pages::dev_music::{MusicChoice, MusicReadout};
use crate::gui::theme::Theme;
use crate::gui::GuiState;

/// The export lengths offered, minutes (docs/design/music-page.md section 8).
pub const EXPORT_MINUTES: [f32; 6] = [1.0, 3.0, 5.0, 10.0, 30.0, 60.0];

/// The largest seed Random draws (seeds under a million are easy to read out and to type).
pub const RANDOM_SEED_MAX: u64 = 999_999;

/// How long a card's Listen plays, seconds of audio (docs/design/music-page.md section 9; the operator,
/// 2026-11-20: "Agree"), before what played before comes back.
pub const AUDITION_S: f32 = 20.0;

/// An audition asked for: the piece it plays and the card's words (the page says what is auditioned).
#[derive(Clone, Debug, PartialEq)]
pub struct Audition {
    pub choice: MusicChoice,
    pub words: String,
}

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

/// The page's tabs (docs/design/music-page.md section 5), in their order; the Orchestra joins in
/// rung 3.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Listen,
    Explore,
    Written,
    Yours,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Listen, Tab::Explore, Tab::Written, Tab::Yours];
    /// Its words key and its hint's key.
    pub fn keys(self) -> (&'static str, &'static str) {
        match self {
            Tab::Listen => ("music_page.tab_listen", "music_page.tab_listen_hint"),
            Tab::Explore => ("music_page.tab_explore", "music_page.tab_explore_hint"),
            Tab::Written => ("music_page.tab_written", "music_page.tab_written_hint"),
            Tab::Yours => ("music_page.tab_yours", "music_page.tab_yours_hint"),
        }
    }
}

/// The page is laid out in columns, with the piano roll docked along the bottom, from this width
/// (points); narrower (a small window, the phone), one column with the roll in Listen.
pub const WIDE_POINTS: f32 = 1100.0;

/// The piano roll's bars shown when the page first opens (0 would be the whole piece).
pub const ROLL_BARS_DEFAULT: usize = 8;

/// The Music page's own state (what plays is the person's settings, not this).
#[derive(Clone)]
pub struct MusicPage {
    /// The tab shown (kept for the session only: a tab is not a choice worth a file).
    pub tab: Tab,
    /// The seed box's text, and the seed it last showed (the box keeps what is being typed until it
    /// is a seed).
    pub seed_text: String,
    pub seed_shown: u64,
    /// The person's favourites, whether they were read (and why not), and the file's place.
    pub favourites: Arc<Favourites>,
    pub fav_loaded: bool,
    pub fav_error: Option<String>,
    pub fav_path: Option<PathBuf>,
    /// The favourite whose name is being changed (its id and the box's text), the one Like marked on
    /// Your music, and what the last Like said.
    pub renaming: Option<(u64, String)>,
    pub marked: Option<u64>,
    pub like_msg: Option<String>,
    /// The piano roll: how many bars it shows (0: the whole piece), whether it follows the music,
    /// and the first bar shown while it does not.
    pub roll_bars: usize,
    pub roll_follow: bool,
    pub roll_first: usize,
    /// The pieces heard, newest first (`history::HEARD_MOST` at most), kept in the person's file
    /// (None: not written, as in a test), and why it could not be read or written.
    pub heard: Vec<Heard>,
    pub heard_path: Option<PathBuf>,
    pub heard_error: Option<String>,
    /// A seek in the written piece playing: a number raised per click, and the bar (engine/music.rs
    /// sends both in the score's context).
    pub seek: (u32, u32),
    /// The audition playing or asked for (a card's Listen), until it has played `AUDITION_S`.
    pub audition: Option<Audition>,
    /// The person's own written pieces (MIDI files they imported: humanity_music::own), their folder,
    /// and what the last import, edit or removal said.
    pub own: Vec<humanity_music::scores::ScoreEntry>,
    pub own_dir: Option<PathBuf>,
    pub own_msg: Option<String>,
    /// The open file picker of an import, or of the folder an export goes to.
    pub import_picker: Option<crate::gui::widgets::file_browser::FilePickerState>,
    pub export_picker: Option<crate::gui::widgets::file_browser::FilePickerState>,
    /// The music-file export (MEI or MIDI): its name box, its folder, and what it last said.
    pub file_name: String,
    pub file_dir: Option<PathBuf>,
    pub file_msg: Option<String>,
    /// A piece of the person's own whose details are being changed: its id and the boxes' text.
    pub editing: Option<files::OwnEdit>,
    /// The Written music list's search, sort and filters, and the piece whose details show.
    pub written_query: WrittenQuery,
    pub written_selected: Option<String>,
    /// The share box, and what the last paste said.
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
            tab: Tab::default(),
            seed_text: String::new(),
            seed_shown: 0,
            favourites: Arc::default(),
            fav_loaded: false,
            fav_error: None,
            fav_path: None,
            renaming: None,
            marked: None,
            like_msg: None,
            roll_bars: ROLL_BARS_DEFAULT,
            roll_follow: true,
            roll_first: 0,
            heard: Vec::new(),
            heard_path: None,
            heard_error: None,
            seek: (0, 0),
            audition: None,
            own: Vec::new(),
            own_dir: None,
            own_msg: None,
            import_picker: None,
            export_picker: None,
            file_name: String::new(),
            file_dir: None,
            file_msg: None,
            editing: None,
            written_query: WrittenQuery::default(),
            written_selected: None,
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

/// The person's own written pieces' folder: `music/scores` beside their favourites.
pub fn own_scores_dir() -> PathBuf {
    favourites_path().with_file_name("scores")
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
        // The pieces heard (history.rs) and the person's own written pieces (files.rs) beside them.
        let heard = history::heard_path();
        match history::load(&heard) {
            Ok(list) => self.heard = list,
            Err(e) => {
                log::warn!("[Music] {e}");
                self.heard_error = Some(e);
            }
        }
        self.heard_path = Some(heard);
        let own = own_scores_dir();
        match humanity_music::own::load(&own) {
            Ok(list) => self.own = list,
            Err(e) => {
                log::warn!("[Music] {e}");
                self.own_msg = Some(e);
            }
        }
        self.own_dir = Some(own);
    }

    /// Play the written piece from bar `bar` (0 = the first): a new seek number, sent to the score.
    pub fn seek_to(&mut self, bar: usize) {
        self.seek = (self.seek.0.wrapping_add(1).max(1), bar as u32);
    }

    /// Empty the pieces heard, and the file (Explore's Clear).
    pub fn clear_heard(&mut self) {
        self.heard.clear();
        self.write_heard();
    }

    /// Write the pieces heard to the person's file (when there is one), saying so when it fails.
    fn write_heard(&mut self) {
        let Some(path) = self.heard_path.clone() else { return };
        match history::save(&path, &self.heard) {
            Ok(()) => self.heard_error = None,
            Err(e) => {
                log::warn!("[Music] {e}");
                self.heard_error = Some(e);
            }
        }
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
}

/// A mood id, or the default mood for none.
fn mood_or_default(id: &str) -> String {
    if id.is_empty() {
        humanity_music::library::DEFAULT_MOOD.to_string()
    } else {
        id.to_string()
    }
}

/// What the score plays in place of new pieces because the person chose it: their genre, mood, seed
/// and tuning while `game_chooses` is off (None while the game chooses). Read by the score every
/// frame, page open or not (engine/music.rs `choice_now`). Without the music's data yet (the first
/// moment of a run, before any score can play) the tuning is the piece's own.
pub fn chosen(s: &MusicSettings, lib: Option<&MusicLibrary>) -> Option<MusicChoice> {
    if !s.written.is_empty() {
        return Some(written_choice(s, lib));
    }
    chosen_composed(s, lib)
}

/// [`chosen`] leaving a written piece out (the engine, when the piece could not be read).
pub fn chosen_composed(s: &MusicSettings, lib: Option<&MusicLibrary>) -> Option<MusicChoice> {
    if s.game_chooses {
        return None;
    }
    let mood = mood_or_default(&s.mood);
    let tune = lib.and_then(|l| l.genre_index(&s.genre).map(|g| s.tune.to_tune(l, g))).unwrap_or_default();
    // The page's Intensity when the person set one (rung 3a), else the mood's own.
    let intensity = if tune.intensity >= 0.0 { tune.intensity.min(1.0) } else { lib.map(|l| explore_intensity(l, &mood)).unwrap_or(0.3) };
    Some(MusicChoice { genre: s.genre.clone(), mood: Some(mood), intensity: Some(intensity), seed: s.seed.max(1), tune, written: None, style: None })
}

/// The written piece the person chose (`MusicSettings::written`, docs/design/score-player.md section
/// 10), heard in the calm mood's hall (the person's mood when they chose one) whoever chooses the rest:
/// it plays everywhere until Stop clears it.
pub fn written_choice(s: &MusicSettings, lib: Option<&MusicLibrary>) -> MusicChoice {
    let mood = if s.game_chooses { humanity_music::library::DEFAULT_MOOD.to_string() } else { mood_or_default(&s.mood) };
    let intensity = lib.map(|l| explore_intensity(l, &mood)).unwrap_or(0.3);
    let mut tune = Tune::default();
    if let Some(full) = lib.and_then(|l| l.genre_index(&s.genre).map(|g| s.tune.to_tune(l, g))) {
        tune.level_db = full.level_db;
        tune.mute = full.mute;
        tune.solo = full.solo;
    }
    // The sound style it plays in (rung 3a): the person's choice for it or for all written music, else
    // an imported file's General-MIDI-like style. Its id names the build the engine asks for.
    let imported = lib.and_then(|l| l.score(&s.written)).map(humanity_music::style::is_imported).unwrap_or(s.written.starts_with(humanity_music::own::ID_PREFIX));
    MusicChoice { genre: s.genre.clone(), mood: Some(mood), intensity: Some(intensity), seed: s.seed.max(1), tune, written: Some(s.written.clone()), style: s.style_for(&s.written, imported) }
}

/// A written piece's title and composer for a person: their words by key, else the plain words a
/// community index gives, else the id.
/// The credit a written piece's index line gives (its `credit_words`, by key), None when it gives
/// none or the key has no words.
pub fn credit_words(e: &humanity_music::scores::ScoreEntry) -> Option<String> {
    if e.credit_words.is_empty() {
        return None;
    }
    let w = t_dyn(&e.credit_words);
    (w != e.credit_words).then(|| w.to_string())
}

pub fn score_words(e: &humanity_music::scores::ScoreEntry) -> (String, String) {
    let pick = |key: &str, plain: &str, fallback: &str| -> String {
        if !key.is_empty() {
            let w = t_dyn(key);
            if w != key {
                return w.to_string();
            }
        }
        if plain.is_empty() {
            fallback.to_string()
        } else {
            plain.to_string()
        }
    };
    (pick(&e.words, &e.name, &e.id.replace('_', " ")), pick(&e.composer_words, &e.composer, ""))
}

/// What the page shows and its Explore and Tune controls change: the person's choice, or while the
/// game chooses, what plays now (its seed, the game's mood, its tuning) in the person's genre, so
/// Next goes on from the piece that is heard.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub genre: String,
    pub mood: String,
    pub seed: u64,
    pub tune: Tune,
}

/// The [`Shown`] for these settings and what the engine reports.
pub fn shown(s: &MusicSettings, r: &MusicReadout) -> Shown {
    let lib = r.lib.as_deref();
    if !s.game_chooses {
        let tune = lib.and_then(|l| l.genre_index(&s.genre).map(|g| s.tune.to_tune(l, g))).unwrap_or_default();
        return Shown { genre: s.genre.clone(), mood: mood_or_default(&s.mood), seed: s.seed.max(1), tune };
    }
    let playing = r.playing.as_ref();
    let seed = playing.map(|p| p.seed).filter(|&x| x > 0).unwrap_or(s.seed.max(1));
    let tune = playing.and_then(|p| p.choice.as_ref()).filter(|c| c.genre == s.genre).map(|c| c.tune).unwrap_or_default();
    Shown { genre: s.genre.clone(), mood: mood_or_default(&r.mood), seed, tune }
}

/// Write what the page's controls changed into the person's settings: a genre is the genre whoever
/// chooses (its lead voices are its own, so a new genre starts from the piece's); a mood, a seed or
/// a change of tuning is the person's choice, which then plays everywhere. True when anything
/// changed (the settings are then saved).
pub fn apply(s: &mut MusicSettings, lib: Option<&MusicLibrary>, before: &Shown, after: &Shown) -> bool {
    let mut changed = false;
    if after.genre != before.genre {
        s.genre = after.genre.clone();
        s.tune.leads.clear();
        changed = true;
    }
    if after.mood != before.mood || after.seed != before.seed || after.tune != before.tune {
        s.game_chooses = false;
        s.mood = after.mood.clone();
        s.seed = after.seed.max(1);
        if let Some((l, g)) = lib.and_then(|l| l.genre_index(&after.genre).map(|g| (l, g))) {
            s.tune = SavedTune::from_tune(&after.tune, l, g);
        }
        changed = true;
    }
    changed
}

/// Make a favourite (opened from Keep, or a pasted code) the person's choice.
pub fn open_favourite(s: &mut MusicSettings, f: &Favourite) {
    s.genre = f.genre.clone();
    s.mood = f.mood.clone();
    s.seed = f.seed.max(1);
    s.tune = f.tune.clone();
    s.game_chooses = false;
}

/// A favourite as "my music" plays it: its genre, seed and tuning in the game's own mood.
pub fn my_music_choice(f: &Favourite, lib: &MusicLibrary) -> Option<MusicChoice> {
    let g = lib.genre_index(&f.genre)?;
    Some(MusicChoice { genre: f.genre.clone(), mood: None, intensity: None, seed: f.seed, tune: f.tune.to_tune(lib, g), written: None, style: None })
}

/// A genre's name for a person: its words by key, else the plain name a community file gives, else
/// its id.
pub fn genre_label(id: &str, words: &str, name: &str) -> String {
    if !words.is_empty() {
        let w = t_dyn(words);
        if w != words {
            return w.to_string();
        }
    }
    if !name.is_empty() {
        name.to_string()
    } else {
        id.to_string()
    }
}

/// A genre's words from what the engine reports: the genres offered, else the data, else its id.
fn reported_genre_words(r: &MusicReadout, id: &str) -> String {
    if let Some(g) = r.genres.iter().find(|g| g.id == id) {
        return genre_label(&g.id, &g.words, &g.name);
    }
    match r.lib.as_deref().and_then(|l| l.genres.iter().find(|g| g.id == id)) {
        Some(g) => genre_label(&g.id, &g.words, &g.name),
        None => id.replace('_', " "),
    }
}

/// WHAT IS PLAYING NOW, IN WORDS, and who chose it: the person, the game, or the person's music for
/// the game's mood; or, with nothing playing, what will. ONE function for the Music page and
/// Settings > Music, so the two pages can never say different things
/// (`settings_music_says_what_the_music_page_says`).
pub fn now_playing_words(s: &MusicSettings, r: &MusicReadout) -> String {
    // A written piece the person chose comes first (unless it could not be read: then what would
    // have played without it plays, and is said below).
    let unreadable = r.written_error.as_ref().is_some_and(|(id, _)| *id == s.written);
    if !s.written.is_empty() && !unreadable {
        let title = r.lib.as_deref().and_then(|l| l.score(&s.written)).map(|e| score_words(e).0).or_else(|| r.written_piece.as_ref().filter(|p| p.entry.id == s.written).map(|p| score_words(&p.entry).0)).unwrap_or_else(|| s.written.replace('_', " "));
        return if !s.music_on {
            t("music_page.now_off").replace("{{what}}", &t("music_page.what_written").replace("{{title}}", &title))
        } else if r.playing.is_some() {
            t("music_page.now_written").replace("{{title}}", &title)
        } else {
            t("music_page.now_quiet_written").replace("{{title}}", &title)
        };
    }
    let what = if s.game_chooses {
        t("music_page.what_game").replace("{{genre}}", &reported_genre_words(r, &s.genre))
    } else {
        t("music_page.what_yours").replace("{{genre}}", &reported_genre_words(r, &s.genre)).replace("{{seed}}", &s.seed.max(1).to_string())
    };
    if !s.music_on {
        return t("music_page.now_off").replace("{{what}}", &what);
    }
    match &r.playing {
        Some(p) => {
            // The genre heard: the choice's, else the score's leading genre (a blend past halfway
            // leads with its second).
            let id = match &p.choice {
                Some(c) => c.genre.clone(),
                None => {
                    let k = if p.ctx.blend >= 0.5 { p.ctx.genre_b } else { p.ctx.genre_a };
                    p.lib.genres.get(k as usize).map(|g| g.id.clone()).unwrap_or_else(|| s.genre.clone())
                }
            };
            let key = match (&p.choice, s.game_chooses) {
                (Some(_), false) => "music_page.now_yours",
                (Some(_), true) => "music_page.now_my_music",
                (None, _) => "music_page.now_game",
            };
            t_dyn(key).replace("{{genre}}", &reported_genre_words(r, &id)).replace("{{seed}}", &p.seed.to_string()).replace("{{mood}}", &mood_words(r.lib.as_deref(), &r.mood))
        }
        None if s.game_chooses => t("music_page.now_quiet_game").replace("{{genre}}", &reported_genre_words(r, &s.genre)),
        None => t("music_page.now_quiet_yours").replace("{{genre}}", &reported_genre_words(r, &s.genre)).replace("{{seed}}", &s.seed.max(1).to_string()),
    }
}

/// A lead voice's words: its part's (`instrument_packs.part.<id>`), else its id made readable.
pub fn voice_words(id: &str) -> String {
    let key = format!("instrument_packs.part.{id}");
    let w = t_dyn(&key);
    if w != key {
        return w.to_string();
    }
    // A synthesised instrument of ours (a sound style's retro FM voices, rung 3a).
    let key = format!("music.instrument.{id}");
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

/// The genre id a playing piece is heard in: its choice's, else the score's leading genre (a blend
/// past halfway leads with its second), else the person's.
pub fn playing_genre(p: &crate::gui::pages::dev_music::Playing, fallback: &str) -> String {
    match &p.choice {
        Some(c) => c.genre.clone(),
        None => {
            let k = if p.ctx.blend >= 0.5 { p.ctx.genre_b } else { p.ctx.genre_a };
            p.lib.genres.get(k as usize).map(|g| g.id.clone()).unwrap_or_else(|| fallback.to_string())
        }
    }
}

impl MusicPage {
    /// Note the piece playing now in the pieces heard (called every frame by the engine's music,
    /// page open or not, so a piece heard elsewhere is in the list too; explore.rs draws it).
    pub fn note_heard(&mut self, s: &MusicSettings, r: &MusicReadout) {
        if history::note_heard(&mut self.heard, s, r, history::now_s()) {
            self.write_heard();
        }
    }
}

/// Draw the Music page: the title and the Now playing strip, the tabs, the tab's content, and on a
/// wide page the piano roll docked along the bottom (docs/design/music-page.md section 5).
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let accent = theme.nav_cat_apps();
    // The visualizer, the chord and section words and a written piece's roll move while music
    // plays: a phone draws only when asked (engine/redraw.rs), so the page asks, about thirty times
    // a second, while there is music, and stops asking when it stops.
    if state.music_dev.readout.score.is_some() || state.music_dev.readout.written_piece.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
    egui::CentralPanel::default().frame(Frame::NONE.fill(theme.bg_panel()).inner_margin(16.0)).show(ctx, |ui| {
        view::ensure_sheet(state);
        // The choice as the page shows it; the strip, Explore and Tune change `sh`, written back below.
        let before = shown(&state.settings.music, &state.music_dev.readout);
        let mut sh = before.clone();
        // The page's keyboard shortcuts (never while a text box has the keyboard: keys.rs).
        keys::handle(ctx, state, &mut sh);
        let wide = ui.available_width() >= WIDE_POINTS;
        egui::TopBottomPanel::top("music_page_top").frame(Frame::NONE).show_separator_line(false).show_inside(ui, |ui| {
            ui.label(RichText::new(t("music_page.title")).size(theme.font_size_title).color(theme.text_primary()));
            strip::strip(ui, theme, state, &mut sh, accent);
            keys::help(ui, theme);
            ui.add_space(theme.spacing_xs);
            ui.horizontal_wrapped(|ui| {
                for tab in Tab::ALL {
                    let (words, hint) = tab.keys();
                    let active = state.music_page.tab == tab;
                    if crate::gui::widgets::Button::tab(t(words), active).hint(hint).show(ui, theme) && !active {
                        state.music_page.tab = tab;
                    }
                }
            });
            ui.add_space(theme.spacing_xs);
        });
        if wide {
            let h = (ui.available_height() * 0.3).clamp(240.0, 460.0);
            egui::TopBottomPanel::bottom("music_page_roll")
                .frame(Frame::NONE.inner_margin(egui::Margin { top: 8, ..Default::default() }))
                .show_separator_line(false)
                .resizable(false)
                .exact_height(h)
                .show_inside(ui, |ui| roll::panel(ui, theme, state, accent));
        }
        ScrollArea::vertical().id_salt("music_page_scroll").auto_shrink([false, false]).show(ui, |ui| match state.music_page.tab {
            Tab::Listen => listen(ui, theme, state, &before, &mut sh, accent, wide),
            Tab::Explore => explore::explore(ui, theme, state, &mut sh, accent, wide),
            Tab::Written => written::written(ui, theme, state, accent, wide),
            Tab::Yours => {
                controls::keep(ui, theme, state, accent);
                ui.add_space(theme.spacing_md);
                files::files_card(ui, theme, state, accent);
            }
        });
        let lib = state.music_dev.readout.lib.clone();
        if apply(&mut state.settings.music, lib.as_deref(), &before, &sh) {
            state.settings_dirty = true;
        }
    });
    // An import's file picker or an export's folder picker, over the page.
    files::pickers(ctx, theme, state);
}

/// Listen: What plays beside the piece in words, the visualizer and Tune (wide), or one column with
/// the piano roll in it (narrow).
fn listen(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, before: &Shown, sh: &mut Shown, accent: Color32, wide: bool) {
    if !wide {
        view::now_playing(ui, theme, state, accent);
        roll::panel(ui, theme, state, accent);
        mixer::mixer(ui, theme, state, &mut sh.tune, accent, false);
        controls::what_plays(ui, theme, state, before, accent);
        controls::tune(ui, theme, state, sh, accent);
        return;
    }
    let gap = theme.spacing_md;
    let left_w = (ui.available_width() * 0.36).max(380.0);
    let right_w = (ui.available_width() - left_w - gap).max(320.0);
    // No height: each column grows with what it holds (the page scrolls; a scroll area's own height
    // is unbounded).
    let h = 0.0;
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(left_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(left_w);
            controls::what_plays(ui, theme, state, before, accent);
        });
        ui.add_space(gap);
        ui.allocate_ui_with_layout(egui::vec2(right_w, h), egui::Layout::top_down(egui::Align::Min), |ui| {
            ui.set_width(right_w);
            view::now_playing(ui, theme, state, accent);
            mixer::mixer(ui, theme, state, &mut sh.tune, accent, true);
            controls::tune(ui, theme, state, sh, accent);
        });
    });
}
