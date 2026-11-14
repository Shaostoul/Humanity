//! Moods as data (docs/design/music.md, section 7): `data/music/moods/<id>.ron`. A mood is what the
//! composer improvises in: its keys, its chords and the weighted moves between them, how long its
//! phrases and chords last, its tempo, which instruments play which roles, and its motifs.
//!
//! A GENRE (genre.rs) is laid over a mood: the mood says where you are and what the music is about
//! (calm aboard, a garden morning, the night watch), the genre says how it sounds (the ship's own
//! calm, lo-fi, and one day electroswing or dubstep).

use serde::{Deserialize, Serialize};

use crate::tuning::Mode;

/// What a part of the score does. Each role behaves its own way in the composer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoleKind {
    /// The voice-led chord, held (or restruck on a genre's comp pattern).
    Pad,
    /// The chord's root low down, held (or on a genre's bass pattern).
    Bass,
    /// The key's root with its fifth, held while the key lasts.
    Drone,
    /// A wordless choir on some chords, upper chord tones.
    Choir,
    /// Motifs in a high register (celesta-like bells).
    Bells,
    /// A second motif voice (the steel pan's gentle colour).
    Melody,
    /// Slow pulses (the deep drums' heartbeat), never busy.
    Percussion,
    /// A steady layer that runs under everything (vinyl crackle).
    Texture,
}

impl RoleKind {
    /// Every role, in mixing order (each role's index in the voice pool's role table).
    pub const ALL: [RoleKind; 8] =
        [RoleKind::Pad, RoleKind::Bass, RoleKind::Drone, RoleKind::Choir, RoleKind::Bells, RoleKind::Melody, RoleKind::Percussion, RoleKind::Texture];

    /// The role's index in [`RoleKind::ALL`].
    pub fn index(self) -> usize {
        RoleKind::ALL.iter().position(|r| *r == self).unwrap_or(0)
    }
}

/// A key centre: a pitch class by name ("D", "F#", "Bb") and a mode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KeyCentre {
    pub root: String,
    pub mode: Mode,
    /// How often the composer is in this key, relative to the others (the first is home).
    #[serde(default = "one")]
    pub weight: f32,
}

fn one() -> f32 {
    1.0
}

/// A chord: its name (what the graph names), its root as semitones above the key centre, and its
/// notes as semitones above its root (a maj9 is [0, 4, 7, 11, 14]).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChordDef {
    pub name: String,
    pub root: i32,
    pub notes: Vec<i32>,
}

/// One part of the score: which role, which instrument, where it sits and how much it plays.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoleSpec {
    pub role: RoleKind,
    /// An instrument id (data/music/instruments/<id>.ron): the synthesised voice it plays. A genre's
    /// palette may swap it.
    pub instrument: String,
    /// The part of the score it plays (data/music/parts.ron, the instrument library's): when a
    /// recorded instrument for that part is installed, it plays this role instead of `instrument`
    /// (src/instrument_packs/score.rs attaches it), and `instrument` is its fallback.
    #[serde(default)]
    pub part: Option<String>,
    /// The register, as note names ("D3", "A5").
    pub low: String,
    pub high: String,
    /// How many notes at once (a pad's voicing; the others play one at a time).
    #[serde(default = "one_u8")]
    pub voices: u8,
    /// How much it plays, 0..1 (a motif role: the chance of a motif on each chord; the choir: the
    /// chance it sings on a chord; percussion: the chance a phrase has its pulse).
    #[serde(default = "one")]
    pub density: f32,
    /// Its level, dB.
    #[serde(default)]
    pub level_db: f32,
    /// It joins only at or above this intensity (layers in and out), 0..1.
    #[serde(default)]
    pub min_intensity: f32,
    /// Percussion only: hits every `every_bars` bars, each (beat offset in the bar, velocity).
    #[serde(default)]
    pub pulse: Option<Pulse>,
    /// A genre's lead voices only: how often a piece picks this one, by weight (1 when not given).
    #[serde(default = "one")]
    pub weight: f32,
}

fn one_u8() -> u8 {
    1
}

/// A slow pulse: the deep drums' heartbeat ("lub-dub" every two bars).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pulse {
    pub every_bars: u32,
    pub hits: Vec<(f32, f32)>,
}

/// A motif: scale steps from an anchor (a chord tone), each with a length in beats. The composer
/// repeats it with variation (transposed to the chord, a note moved by a step, the rhythm loosened).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Motif {
    pub steps: Vec<i32>,
    pub beats: Vec<f32>,
    /// How much of its beats each note sounds, 0..1 (a swing riff's short, punched notes); empty:
    /// each note nearly its whole length.
    #[serde(default)]
    pub hold: Vec<f32>,
}

/// A mood.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mood {
    pub id: String,
    /// Its name, by key.
    #[serde(default)]
    pub words: String,
    pub keys: Vec<KeyCentre>,
    pub chords: Vec<ChordDef>,
    /// The chord graph: (from, to, weight) by chord name.
    pub moves: Vec<(String, String, f32)>,
    /// Chords a phrase may end on (its cadences).
    pub cadences: Vec<String>,
    /// The chord it starts on.
    pub start: String,
    /// Phrase lengths it chooses from, in bars (8 or 16).
    pub phrase_bars: Vec<u32>,
    /// How long a chord lasts, bars (inclusive range).
    pub chord_bars: (u32, u32),
    /// Tempo range, beats per minute, and whether a beat is felt at all.
    pub tempo_bpm: (f32, f32),
    #[serde(default)]
    pub beat_felt: bool,
    #[serde(default = "four")]
    pub beats_per_bar: u32,
    pub roles: Vec<RoleSpec>,
    #[serde(default)]
    pub motifs: Vec<Motif>,
    /// The output level, dB: set so this mood renders at the music category's loudness
    /// (data/audio/loudness.toml, -18 LUFS at volume 1), measured by the tests.
    #[serde(default)]
    pub level_db: f32,
    /// How long, seconds, before a phrase may come back note for note (the composer re-plans any
    /// phrase it played within this window).
    #[serde(default = "default_no_repeat")]
    pub no_repeat_s: f32,
    /// Reverb decay, seconds, and how wet.
    #[serde(default = "default_t60")]
    pub reverb_t60_s: f32,
    #[serde(default = "default_wet")]
    pub reverb_wet: f32,
    /// A danger or action mood (intensity.rs): it plays as written whatever the person's genre (its
    /// tempo, groove, drums, patterns and instruments); the genre's recipe (tape colour) still applies.
    #[serde(default)]
    pub urgent: bool,
    /// The tempo follows the intensity across the mood's band (low end at the band's start, high at
    /// its end) instead of being chosen at random in the range.
    #[serde(default)]
    pub tempo_follows_intensity: bool,
    /// The mood's own groove and drums (an urgent mood's taiko ostinatos), its pad struck on a pattern
    /// (brass stabs) and its bass on a pattern (a pulsing ostinato). A calm mood leaves them empty and
    /// a genre supplies them.
    #[serde(default)]
    pub groove: Option<crate::genre::Groove>,
    #[serde(default)]
    pub drums: Vec<crate::genre::DrumPart>,
    #[serde(default)]
    pub pad_patterns: Vec<String>,
    #[serde(default)]
    pub bass_patterns: Vec<String>,
}

fn four() -> u32 {
    4
}
fn default_no_repeat() -> f32 {
    600.0
}
fn default_t60() -> f32 {
    4.8
}
fn default_wet() -> f32 {
    0.55
}

impl Mood {
    /// The index of the chord named `name`.
    pub fn chord_index(&self, name: &str) -> Option<usize> {
        self.chords.iter().position(|c| c.name == name)
    }

    /// The role spec for `role`, if this mood has one.
    pub fn role(&self, role: RoleKind) -> Option<&RoleSpec> {
        self.roles.iter().find(|r| r.role == role)
    }
}
