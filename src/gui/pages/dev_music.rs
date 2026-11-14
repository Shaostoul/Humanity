//! The Dev page's Music card (docs/design/music.md, section 16; permanent dev tooling): the danger
//! knob (an intensity slider, 0 calm to 1 the height of a meteor storm, and the hull-breach alarm)
//! and what the music is doing (the mood and the place's mood, the danger band's effect, the key,
//! the tempo, the voices, the audio thread's cost per block, the hum's level and alarm, the seed). A
//! rig reaches the same knob with the `music_intensity` and `music_breach` showcase verbs
//! (engine/music.rs, docs/ai/mcp.md).
//!
//! The engine (engine/music.rs) fills `MusicDev::readout` each frame; this card only reads it and
//! writes the knob.

use egui::RichText;
use humanity_core::i18n::t;
use humanity_music::{BedStatus, MusicStatus};

use crate::gui::theme::Theme;
use crate::gui::widgets::with_hint;
use crate::gui::{widgets, GuiState};

/// A genre as Settings > Music offers it.
#[derive(Clone, Debug, Default)]
pub struct GenreChoice {
    pub id: String,
    /// Its name and its one sentence by key; a community genre with no key gives plain words.
    pub words: String,
    pub name: String,
    pub about_words: String,
    pub about: String,
    /// The parts that can sound with it playing (a tuba, a brushed snare) that no installed recording
    /// of their own plays: stand-ins play them, and Settings says so, naming the pack that would
    /// bring each one's recording.
    pub stand_ins: Vec<StandIn>,
}

/// One part played by a stand-in: its name and the name of the pack whose recording would play
/// it (None: no pack holds one yet), both by words key.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StandIn {
    pub part: String,
    pub pack: Option<String>,
}

/// What the music plays in place of new pieces: the Music page's choice while it is open, or a
/// favourite that is a mood's "my music" (docs/design/music-page.md sections 4 and 7).
#[derive(Clone, Debug, PartialEq)]
pub struct MusicChoice {
    /// The genre's id, and the mood heard (None: the game's own mood, as for "my music").
    pub genre: String,
    pub mood: Option<String>,
    /// The intensity it is heard at (None: the game's own).
    pub intensity: Option<f32>,
    pub seed: u64,
    pub tune: humanity_music::tune::Tune,
}

impl MusicChoice {
    /// Whether `other` composes the same piece (a level or a mute is heard at once; anything else
    /// starts the piece again with the change, at the next bar).
    pub fn composes_like(&self, other: &MusicChoice) -> bool {
        self.genre == other.genre && self.mood == other.mood && self.intensity.map(f32::to_bits) == other.intensity.map(f32::to_bits) && self.seed == other.seed && self.tune.composes_like(&other.tune)
    }
}

/// The piece the score plays, as the Music page needs it to write it down and name it: what it was
/// built for, its seed, its context when it began, and the data and instruments it plays from.
#[derive(Clone)]
pub struct Playing {
    /// The choice it plays (None: a new piece of the person's genre).
    pub choice: Option<MusicChoice>,
    pub seed: u64,
    pub ctx: humanity_music::MusicContext,
    pub res: std::sync::Arc<humanity_music::Resolved>,
    pub lib: std::sync::Arc<humanity_music::MusicLibrary>,
}

impl std::fmt::Debug for Playing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playing").field("choice", &self.choice).field("seed", &self.seed).finish_non_exhaustive()
    }
}

/// What the engine reports about the music this frame.
#[derive(Clone, Debug, Default)]
pub struct MusicReadout {
    /// An audio device is open (a script's boot opens none: launch_focus.rs).
    pub device: bool,
    pub score: Option<MusicStatus>,
    pub bed: Option<BedStatus>,
    /// The mood playing, and the one the place asks for.
    pub mood: String,
    pub place_mood: String,
    pub zone: String,
    pub hum_level_db: f32,
    pub seed: u64,
    /// The genres Settings offers, each with its words and its stand-ins.
    pub genres: Vec<GenreChoice>,
    /// The named tones Settings offers, hertz.
    pub tones: Vec<f32>,
    pub load_error: Option<String>,
    /// How long the last build took on its own thread, milliseconds (engine/music.rs: the data read,
    /// the recordings decoded and resolved; the game thread never waits for it).
    pub build_ms: Option<f32>,
    /// How many scores have started this run (a new one takes over at a phrase after an install, and
    /// every phrase with reuse off).
    pub pieces: u64,
    /// How far under itself the score sits now, dB (a chat video playing, the world behind a page).
    pub duck_db: f32,
    /// Seconds until the next piece begins (None: Never, or nothing playing), and Settings > Music's
    /// "A new piece" choices, minutes (data/music/pieces.ron; 0 = Never).
    pub next_piece_s: Option<f32>,
    pub piece_minutes: Vec<f32>,
    /// The piece playing (None: nothing plays).
    pub playing: Option<Playing>,
    /// The music's data as last read (the Music page lists genres, moods and lead voices from it).
    pub lib: Option<std::sync::Arc<humanity_music::MusicLibrary>>,
}

/// The Music card's state: the knob and the readout.
#[derive(Clone, Debug, Default)]
pub struct MusicDev {
    /// The danger intensity, 0..1 (game systems will raise it too: a breach, a storm, combat).
    pub intensity: f32,
    /// The hull-breach alarm in the ship's hum.
    pub breach: bool,
    /// Read the music's files again and start it afresh (after editing a mood or a genre).
    pub restart_requested: bool,
    pub readout: MusicReadout,
}

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Draw the card.
pub fn draw_card(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev.music.title")).size(theme.font_size_body).strong().color(theme.text_primary()));
        widgets::setting_hint(ui, theme, hint, t("dev.music.about"));
        let d = &mut state.music_dev;
        with_hint(ui, "dev.music.intensity_hint", |ui| widgets::labeled_slider(ui, theme, t("dev.music.intensity"), &mut d.intensity, 0.0..=1.0));
        widgets::setting_hint(ui, theme, hint, t("dev.music.intensity_hint"));
        with_hint(ui, "dev.music.breach_hint", |ui| widgets::toggle(ui, theme, t("dev.music.breach"), &mut d.breach));
        widgets::setting_hint(ui, theme, hint, t("dev.music.breach_hint"));
        if with_hint(ui, "dev.music.restart_hint", |ui| widgets::Button::ghost(t("dev.music.restart")).show(ui, theme)) {
            d.restart_requested = true;
        }
        let r = &d.readout;
        let line = |ui: &mut egui::Ui, label: &str, value: String| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary()));
                ui.label(RichText::new(value).size(theme.font_size_small).color(theme.text_primary()));
            });
        };
        if let Some(e) = &r.load_error {
            ui.label(RichText::new(format!("{} {e}", t("dev.music.load_error"))).size(theme.font_size_small).color(theme.warning()));
        }
        if !r.device {
            ui.label(RichText::new(t("dev.music.no_device")).size(theme.font_size_small).color(theme.text_muted()));
        }
        match r.score {
            Some(s) => {
                line(ui, t("dev.music.mood"), format!("{} ({} {})", r.mood, t("dev.music.place_asks"), r.place_mood));
                line(ui, t("dev.music.key"), format!("{}  {:.1} bpm  {} {}  {} {}", NOTE_NAMES[(s.key_pc as i32).rem_euclid(12) as usize], s.tempo_bpm, t("dev.music.phrase"), s.phrase, t("dev.music.bar"), s.bar + 1));
                line(ui, t("dev.music.voices"), format!("{}  {} {:.3} ms  {} {:.3} ms", s.voices, t("dev.music.mean"), s.block_us_mean / 1000.0, t("dev.music.worst"), s.block_us_worst / 1000.0));
                // The piece (docs/design/music.md section 7c): its seed (HUMANITY_MUSIC_SEED sets
                // the first one for a rig), the section of its form and the time through it, and
                // when the next piece begins.
                let section = if s.section == 0 { "-".to_string() } else { ((b'A' + s.section - 1) as char).to_string() };
                let next = r.next_piece_s.map_or("-".to_string(), |x| format!("{}:{:02}", (x as u32) / 60, (x as u32) % 60));
                line(ui, t("dev.music.piece"), format!("{} {}  {} {}  {} {}  {} {next}", t("dev.music.piece_seed"), s.piece_seed, t("dev.music.piece_section"), section, t("dev.music.piece_chorus"), s.chorus + 1, t("dev.music.piece_next")));
            }
            None => line(ui, t("dev.music.mood"), t("dev.music.silent").to_string()),
        }
        if let Some(b) = r.bed {
            line(ui, t("dev.music.hum"), format!("{:+.1} dB  {:.0} Hz  {} {:.2}  {:.3} ms", r.hum_level_db, b.cutoff_hz, t("dev.music.alarm"), b.alarm_level, b.block_us_mean / 1000.0));
        }
        line(ui, t("dev.music.build"), format!("{}  {} {}  {} {:+.0} dB", r.build_ms.map_or("-".to_string(), |ms| format!("{ms:.0} ms")), t("dev.music.pieces"), r.pieces, t("dev.music.duck"), r.duck_db));
        line(ui, t("dev.music.zone"), if r.zone.is_empty() { "-".to_string() } else { r.zone.clone() });
        line(ui, t("dev.music.seed"), r.seed.to_string());
    });
}
