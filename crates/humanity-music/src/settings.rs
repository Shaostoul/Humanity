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
    /// The genre (a file's id in data/music/genres), chosen on the Music page: new pieces are in it
    /// while the game chooses, and it is the genre of the person's own choice otherwise.
    pub genre: String,
    /// WHAT PLAYS (2026-11-14, docs/design/music-page.md section 4; the operator: "We should have one
    /// place ideally or at least have the synced so users don't get different music on different
    /// pages"). Chosen on the Music page only, and kept here so the score, the Music page and
    /// Settings > Music read the one choice, page open or not. On (the default): the game chooses:
    /// new pieces of `genre` in the mood of the place and the moment (calm, danger), a mood's "my
    /// music" favourite in that mood, a new piece every `piece_minutes`, places and a blend as set.
    /// Off: the person's own choice, `genre` with `mood`, `seed` and `tune`, plays everywhere until
    /// they choose again.
    pub game_chooses: bool,
    /// The person's own choice, heard while `game_chooses` is off: the mood it is heard in (an id;
    /// a danger mood brings its danger layers), its seed, and its changes (by ids, as a favourite
    /// keeps them, so a genre file that gains a voice changes nothing).
    pub mood: String,
    pub seed: u64,
    pub tune: crate::favourite::SavedTune,
    /// A WRITTEN piece playing in place of everything above (docs/design/score-player.md section 10):
    /// its id in data/music/scores/index.ron, chosen on the Music page's list; "" = none. Stop clears
    /// it, and what played before plays again (the game's choice or the person's own is untouched).
    pub written: String,
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
    /// How often a new piece begins, minutes (0: never; the choices are data/music/pieces.ron).
    pub piece_minutes: f32,
}

impl Default for MusicSettings {
    fn default() -> Self {
        MusicSettings {
            music_on: true,
            music_on_pages: true,
            tuning: Tuning::A432,
            genre: crate::library::DEFAULT_GENRE.to_string(),
            game_chooses: true,
            mood: crate::library::DEFAULT_MOOD.to_string(),
            seed: 1,
            tune: crate::favourite::SavedTune::default(),
            written: String::new(),
            blend_with: String::new(),
            blend: 0.0,
            places_pick_genre: false,
            ambience_volume: 0.6,
            ship_hum: true,
            ship_hum_level: 0.35,
            tones_on: false,
            tone_hz: 528.0,
            tone_volume: 0.3,
            piece_minutes: 4.0,
        }
    }
}
