//! Styles as data (docs/design/music.md, section 7a): `data/music/styles/<id>.ron`, laid over a mood.
//!
//! The operator, 2026-11-06: "Your procedural idea is actually really cool. I'm curious how we could
//! tailor it to different tastes. Like what if someone likes dubstep or electroswing or whatever."
//! A style says HOW the music sounds: its tempo range, its groove (swing, the grid, syncopation,
//! drum patterns written as text), its harmony rules, which instrument plays each role (its
//! palette), its sound-design recipe (tape wow, a low-pass, saturation), and its level. Every field
//! is optional over the mood: "Mothership calm" changes nothing, so the mood plays as written.
//!
//! A person picks a style in Settings > Music, and may BLEND two (a slider between them: tempo,
//! swing and the recipe move smoothly, each role takes the second style's instrument with a chance
//! equal to the blend, and both styles' drums play, each at its share). Places may suggest a livelier
//! style (a gym, a workshop) only if the person allows it. A style is a file, so anyone can make one
//! and share it (a mod): drop it in data/music/styles/ and it is in the list.
//!
//! Honest limits (said in Settings too): genres built on RECORDED timbres (swing-era brass and
//! clarinet, vocal chops) need samples, and the app only uses recordings free to share (CC0); it
//! never samples copyrighted recordings, because copyright law forbids it. Public-domain recordings
//! (old 78s) would need a dated legal finding first; none has been written.

use serde::{Deserialize, Serialize};

use crate::mood::RoleKind;

/// The groove: how the beat is divided and pushed.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Groove {
    /// Where the second of each pair of grid steps lands, as a fraction of the pair: 0.5 is
    /// straight, 0.667 a triplet swing.
    #[serde(default = "straight")]
    pub swing: f32,
    /// Grid steps per beat: 2 is eighths, 4 sixteenths.
    #[serde(default = "two")]
    pub subdivision: u32,
    /// The chance a drum hit moves to its neighbouring off-step, or a ghost note joins, 0..1.
    #[serde(default)]
    pub syncopation: f32,
    /// Random timing per hit, milliseconds (a player, not a machine).
    #[serde(default)]
    pub humanize_ms: f32,
}

fn straight() -> f32 {
    0.5
}
fn two() -> u32 {
    2
}

impl Default for Groove {
    fn default() -> Self {
        Groove { swing: 0.5, subdivision: 2, syncopation: 0.0, humanize_ms: 0.0 }
    }
}

/// A drum part: an instrument and its patterns, one bar each, written as text over the grid:
/// `x` a hit, `o` a softer hit, `g` a ghost note, `.` a rest. A phrase picks one pattern.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrumPart {
    pub instrument: String,
    pub patterns: Vec<String>,
    /// Its velocity, 0..1.
    #[serde(default = "one")]
    pub velocity: f32,
    /// The note it is tuned to (a membrane drum's pitch), a name such as "D2".
    #[serde(default = "default_drum_note")]
    pub note: String,
    /// It plays only at or above this intensity (the taiko's doubled ostinato at a danger peak).
    #[serde(default)]
    pub min_intensity: f32,
}

fn one() -> f32 {
    1.0
}
fn default_drum_note() -> String {
    "D2".to_string()
}

/// Harmony rules the style lays over the mood's chords.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct HarmonyRule {
    /// Every chord gets its diatonic seventh if it lacks one (jazz and lo-fi colour).
    #[serde(default)]
    pub sevenths: bool,
    /// Tensions above the octave dropped (simpler, rootsier chords).
    #[serde(default)]
    pub plain: bool,
    /// Chord length range in bars, over the mood's.
    #[serde(default)]
    pub chord_bars: Option<(u32, u32)>,
}

/// Sound-design recipe for the whole mix.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Recipe {
    /// Tape wow: depth (cents) and rate (Hz); flutter: depth and rate.
    #[serde(default)]
    pub wow_cents: f32,
    #[serde(default)]
    pub wow_hz: f32,
    #[serde(default)]
    pub flutter_cents: f32,
    #[serde(default)]
    pub flutter_hz: f32,
    /// A low-pass over the whole mix (a worn tape or a small radio), Hz (0 = none).
    #[serde(default)]
    pub lowpass_hz: f32,
    /// Soft saturation over the mix, 0..1.
    #[serde(default)]
    pub saturation: f32,
    /// Reverb wetness times this (lo-fi is drier than the ship's halls).
    #[serde(default = "one")]
    pub reverb_scale: f32,
}

/// A role's instrument swapped (the palette).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaletteEntry {
    pub role: RoleKind,
    pub instrument: String,
    /// Its level, dB, over the mood's for that role.
    #[serde(default)]
    pub level_db: f32,
}

/// A style.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Style {
    pub id: String,
    /// Its name by key; a community style with no key in en.json gives plain words in `name`.
    #[serde(default)]
    pub words: String,
    #[serde(default)]
    pub name: String,
    /// One sentence on what it sounds like, by key (or plain words in `about` for a community style).
    #[serde(default)]
    pub about_words: String,
    #[serde(default)]
    pub about: String,
    /// Tempo range over the mood's, bpm; a felt beat over the mood's.
    #[serde(default)]
    pub tempo_bpm: Option<(f32, f32)>,
    #[serde(default)]
    pub beat_felt: Option<bool>,
    #[serde(default)]
    pub groove: Option<Groove>,
    #[serde(default)]
    pub drums: Vec<DrumPart>,
    /// The pad restruck on this pattern instead of held (an electric piano comping); `x` strikes.
    #[serde(default)]
    pub pad_pattern: Option<String>,
    /// The bass on this pattern instead of held: `x` the root, `5` the fifth, `o` the octave.
    #[serde(default)]
    pub bass_pattern: Option<String>,
    #[serde(default)]
    pub harmony: HarmonyRule,
    #[serde(default)]
    pub palette: Vec<PaletteEntry>,
    /// Roles the style silences (lo-fi has no choir), and roles it adds over the mood's.
    #[serde(default)]
    pub roles_off: Vec<RoleKind>,
    #[serde(default)]
    pub roles_add: Vec<crate::mood::RoleSpec>,
    #[serde(default)]
    pub recipe: Recipe,
    /// Level over the mood's, dB, so the style still renders at the music category's loudness.
    #[serde(default)]
    pub level_db: f32,
    /// Instruments it would rather have recorded (it plays synthesised stand-ins until a CC0 pack
    /// supplies them): shown honestly in Settings.
    #[serde(default)]
    pub needs_recordings: Vec<String>,
}

impl Style {
    /// The palette's instrument for `role`, if the style swaps it.
    pub fn palette_for(&self, role: RoleKind) -> Option<&PaletteEntry> {
        self.palette.iter().find(|p| p.role == role)
    }
}

/// Which hits a drum pattern's character asks for, as a velocity (0 = rest).
pub fn hit_velocity(c: char) -> f32 {
    match c {
        'x' | 'X' => 1.0,
        'o' | 'O' => 0.6,
        'g' | 'G' => 0.3,
        _ => 0.0,
    }
}

/// The time of grid step `k` (0..subdivision) within a beat, as a fraction of the beat, with swing:
/// steps come in pairs, and the second of each pair lands at `swing` of the pair (0.5 straight).
pub fn step_offset_in_beat(k: u32, subdivision: u32, swing: f32) -> f32 {
    let sub = subdivision.max(1);
    if sub == 1 {
        return 0.0;
    }
    let pair_len = 2.0 / sub as f32;
    let pair = (k / 2) as f32;
    let base = pair * pair_len;
    if k % 2 == 1 {
        base + swing.clamp(0.3, 0.8) * pair_len
    } else {
        base
    }
}
