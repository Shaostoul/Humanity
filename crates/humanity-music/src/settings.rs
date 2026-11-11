//! The person's music settings (Settings > Music, docs/design/music.md section 15), saved in the
//! app's config.json as one `music` object. Every field has a default, so a config written before a
//! field existed reads with that field at its default.

use serde::{Deserialize, Serialize};

use crate::tuning::Tuning;

/// Settings > Music.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MusicSettings {
    /// The game's own music, in the world aboard the ship.
    pub music_on: bool,
    /// Also on the app's everyday pages (Chat, Tasks, the Library): off by default, because those
    /// pages are real life and people often play their own music there.
    pub music_on_pages: bool,
    /// The music from the moment HumanityOS opens (2026-11-11, the operator: "we should have the
    /// music start playing when HumanityOS starts"): the score plays on the title screen and the
    /// pages you open, from the app's start until you first go into the world; after that the pages
    /// follow [`Self::music_on_pages`]. On by default; it follows the volumes and `music_on`, and an
    /// instance a script started in the background has no sound at all until a person clicks into it
    /// (engine/launch_focus.rs), so a rig stays silent.
    pub music_at_start: bool,
    /// The tuning every note follows.
    pub tuning: Tuning,
    /// The style (a file's id in data/music/styles).
    pub style: String,
    /// A second style to blend toward ("" = none) and how far (0..1).
    pub blend_with: String,
    pub blend: f32,
    /// Let places (a gym, a workshop) pick a livelier style while you are there.
    pub places_pick_style: bool,
    /// Background sounds of places (the ship's hum), separate from the music, 0..1.
    pub ambience_volume: f32,
    /// The ship's hum aboard, and its level among the background sounds: the hum follows the
    /// Ambience Volume and this, never the Music Volume (0.35 puts it about 18 LU under the music
    /// with both volumes at the same setting).
    pub ship_hum: bool,
    pub ship_hum_level: f32,
    /// The optional tones layer: off by default; it plays only while the music is off.
    pub tones_on: bool,
    pub tone_hz: f32,
    pub tone_volume: f32,
}

impl Default for MusicSettings {
    fn default() -> Self {
        MusicSettings {
            music_on: true,
            music_on_pages: false,
            music_at_start: true,
            tuning: Tuning::A432,
            style: crate::library::DEFAULT_STYLE.to_string(),
            blend_with: String::new(),
            blend: 0.0,
            places_pick_style: false,
            ambience_volume: 0.6,
            ship_hum: true,
            ship_hum_level: 0.35,
            tones_on: false,
            tone_hz: 528.0,
            tone_volume: 0.3,
        }
    }
}
