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
    /// The styles Settings offers: (id, words key, plain name for a community style).
    pub styles: Vec<(String, String, String)>,
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
}

/// The Music card's state: the knob and the readout.
#[derive(Clone, Debug, Default)]
pub struct MusicDev {
    /// The danger intensity, 0..1 (game systems will raise it too: a breach, a storm, combat).
    pub intensity: f32,
    /// The hull-breach alarm in the ship's hum.
    pub breach: bool,
    /// Read the music's files again and start it afresh (after editing a mood or a style).
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
