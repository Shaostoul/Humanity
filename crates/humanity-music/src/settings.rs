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
    /// Also on the app's pages (the title screen, Chat, Tasks, the Library), from the moment
    /// HumanityOS opens and all session, a trip into the world and back included. ON by default
    /// (2026-11-11, the operator: "we should have the music start playing when HumanityOS starts",
    /// then "Play on the app pages as well."); it plays alongside the person's own music player, each
    /// at its own volume in Windows, and follows `music_on` and the volumes. An instance a script
    /// started in the background has no sound at all until a person clicks into it
    /// (engine/launch_focus.rs), so a rig stays silent. (This one switch replaced a second, "Play the
    /// music when HumanityOS opens", the same day; an old config's `music_at_start` is ignored.)
    pub music_on_pages: bool,
    /// The tuning every note follows.
    pub tuning: Tuning,
    /// The genre (a file's id in data/music/genres).
    pub genre: String,
    /// A second genre to blend toward ("" = none) and how far (0..1).
    pub blend_with: String,
    pub blend: f32,
    /// Let places (a gym, a workshop) pick a livelier genre while you are there.
    pub places_pick_genre: bool,
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
            music_on_pages: true,
            tuning: Tuning::A432,
            genre: crate::library::DEFAULT_GENRE.to_string(),
            blend_with: String::new(),
            blend: 0.0,
            places_pick_genre: false,
            ambience_volume: 0.6,
            ship_hum: true,
            ship_hum_level: 0.35,
            tones_on: false,
            tone_hz: 528.0,
            tone_volume: 0.3,
        }
    }
}
