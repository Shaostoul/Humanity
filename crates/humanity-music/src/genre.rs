//! Genres as data (docs/design/music.md, section 7a): `data/music/genres/<id>.ron`, laid over a mood.
//!
//! The operator, 2026-11-06: "Your procedural idea is actually really cool. I'm curious how we could
//! tailor it to different tastes. Like what if someone likes dubstep or electroswing or whatever."
//! A genre says HOW the music sounds: its tempo range, its groove (swing, the grid, syncopation,
//! drum patterns written as text), its harmony rules, which instrument plays each role (its
//! palette), its sound-design recipe (tape wow, a low-pass, saturation), and its level. Every field
//! is optional over the mood: "Mothership calm" changes nothing, so the mood plays as written.
//!
//! A person picks a genre in Settings > Music, and may BLEND two (a slider between them: tempo,
//! swing and the recipe move smoothly, each role takes the second genre's instrument with a chance
//! equal to the blend, and both genres' drums play, each at its share). Places may suggest a livelier
//! genre (a gym, a workshop) only if the person allows it. A genre is a file, so anyone can make one
//! and share it (a mod): drop it in data/music/genres/ and it is in the list.
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

/// Harmony rules the genre lays over the mood's chords.
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
    /// The part of the score it plays (data/music/parts.ron), as a mood's role names one: when a
    /// recorded instrument for that part is installed it plays instead of `instrument`
    /// (src/instrument_packs/score.rs), and `instrument` is its synthesised fallback.
    #[serde(default)]
    pub part: Option<String>,
    /// A register of its own (note names), for an instrument that sits elsewhere than the mood's
    /// role (a guitar playing the bells' motifs an octave or two down, a handpan's nine notes).
    #[serde(default)]
    pub low: Option<String>,
    #[serde(default)]
    pub high: Option<String>,
}

/// A genre's chord vocabulary and progressions: its key centres and a weighted chord graph, the
/// shape a mood writes them in (mood.rs): chords as a root in semitones above the key centre and
/// notes above that root, moves between named chords with weights, the chords a phrase may end on.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Progressions {
    pub keys: Vec<crate::mood::KeyCentre>,
    pub chords: Vec<crate::mood::ChordDef>,
    pub moves: Vec<(String, String, f32)>,
    pub cadences: Vec<String>,
    pub start: String,
    /// The harmonic grammar a piece's sections are generated from (docs/design/music.md section
    /// 7c); None: each section is a walk of the chord graph above.
    #[serde(default)]
    pub grammar: Option<Grammar>,
}

/// A chord's harmonic function, for the grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Function {
    Tonic,
    Subdominant,
    Dominant,
}

/// A harmonic grammar: each named chord's function; SHAPES, runs of functions written as letters
/// ("T S D T"), by weight; and the chances of the style's embellishments.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Grammar {
    pub functions: Vec<(String, Function)>,
    pub shapes: Vec<(String, f32)>,
    /// Before a chord, its secondary dominant (the seventh chord a fifth above it).
    #[serde(default)]
    pub secondary_dominants: f32,
    /// A dominant played as the seventh chord a tritone away.
    #[serde(default)]
    pub tritone_substitutes: f32,
    /// Before a chord, the diminished seventh chord a half step under it.
    #[serde(default)]
    pub passing_diminished: f32,
}

/// One form a piece may take: its sections' letters in order ("AABA"), and its weight.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FormSpec {
    pub form: String,
    #[serde(default = "one_f32")]
    pub weight: f32,
}

fn one_f32() -> f32 {
    1.0
}

/// How a piece makes its own riffs (crate::piece::make_motifs).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MotifRules {
    /// How many riffs a piece makes, and how many notes each.
    pub count: usize,
    pub notes: (usize, usize),
    /// Rhythm cells, note lengths in beats, strung together.
    pub cells: Vec<Vec<f32>>,
    /// The contours a riff may follow, by weight.
    pub contours: Vec<(crate::piece::Contour, f32)>,
    /// The chance a step is a leap (three or four scale steps).
    #[serde(default)]
    pub leap: f32,
    /// How far a riff may go from its first note, scale steps down and up.
    pub range: (i32, i32),
    /// How much of each note sounds, a range.
    pub hold: (f32, f32),
}

/// How a struck pad sounds.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Comping {
    /// Milliseconds between one string and the next, lowest first (0: all at once).
    #[serde(default)]
    pub strum_ms: f32,
    /// How long each strike rings, beats (0: until the next strike); a gypsy-jazz guitar's chop is
    /// short.
    #[serde(default)]
    pub hold_beats: f32,
}

fn quarter() -> f32 {
    0.25
}

/// A genre.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Genre {
    pub id: String,
    /// Its name by key; a community genre with no key in en.json gives plain words in `name`.
    #[serde(default)]
    pub words: String,
    #[serde(default)]
    pub name: String,
    /// One sentence on what it sounds like, by key (or plain words in `about` for a community genre).
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
    /// Beats in a bar over the mood's (its meter: 4 for 4/4, 3 for a waltz).
    #[serde(default)]
    pub beats_per_bar: Option<u32>,
    /// The pad restruck on one of these patterns instead of held (an electric piano comping, a
    /// guitar's "la pompe"); `x` strikes, `o` softer. A phrase picks one.
    #[serde(default)]
    pub pad_patterns: Vec<String>,
    /// How the pad's strikes sound: a strum (low string to high) and how long each chord rings.
    #[serde(default)]
    pub comping: Comping,
    /// The bass on one of these patterns instead of held: `x` the root, `5` the fifth, `o` the
    /// octave, `3` the third, `w` a WALKING step (the root on a chord's first beat, chord tones
    /// and scale steps toward the next chord, a half-step approach on the beat before it). A phrase
    /// picks one (a two-feel verse, a walking chorus).
    #[serde(default)]
    pub bass_patterns: Vec<String>,
    /// The choir role's voicing (the upper chord tones, voice-led) struck SHORT on one of these
    /// patterns instead of held: a horn section's stabs. A phrase picks one; it plays on the chords
    /// the plan gives the choir role.
    #[serde(default)]
    pub stab_patterns: Vec<String>,
    /// How long each stab rings, beats.
    #[serde(default = "quarter")]
    pub stab_hold_beats: f32,
    /// The genre's own key centres and chord graph (its chord vocabulary and progressions), in
    /// place of the mood's in every calm mood; a danger mood keeps its own.
    #[serde(default)]
    pub progressions: Option<Progressions>,
    /// Its own motif bank, in place of the mood's in calm moods (empty: the mood's).
    #[serde(default)]
    pub motifs: Vec<crate::mood::Motif>,
    /// The scale its motifs walk (the minor blues for swing riffs); else the key's pentatonic.
    #[serde(default)]
    pub motif_scale: Option<crate::tuning::Mode>,
    /// Call and response: the melody role plays a motif (the call) and on the next chord the bells
    /// role answers it, the same rhythm from another chord tone, resolving down.
    #[serde(default)]
    pub call_and_response: bool,
    /// Each phrase plays at a dynamic level in this range (velocity times 0..1): a softer breakdown,
    /// a louder chorus. None: every phrase at full.
    #[serde(default)]
    pub dynamics: Option<(f32, f32)>,
    #[serde(default)]
    pub harmony: HarmonyRule,
    #[serde(default)]
    pub palette: Vec<PaletteEntry>,
    /// Roles the genre silences (lo-fi has no choir), and roles it adds over the mood's.
    #[serde(default)]
    pub roles_off: Vec<RoleKind>,
    #[serde(default)]
    pub roles_add: Vec<crate::mood::RoleSpec>,
    #[serde(default)]
    pub recipe: Recipe,
    /// Level over the mood's, dB, so the genre still renders at the music category's loudness.
    #[serde(default)]
    pub level_db: f32,
    /// Instruments it would rather have recorded (it plays synthesised stand-ins until a CC0 pack
    /// supplies them): shown honestly in Settings.
    #[serde(default)]
    pub needs_recordings: Vec<String>,
    /// A piece's section lengths in bars, over the mood's phrase lengths (one is drawn per section).
    #[serde(default)]
    pub phrase_bars: Option<Vec<u32>>,
    /// The forms a piece may take, by weight (empty: no form, every phrase walked afresh).
    #[serde(default)]
    pub forms: Vec<FormSpec>,
    /// How a piece makes its own riffs (None: the `motifs` bank above).
    #[serde(default)]
    pub motif_rules: Option<MotifRules>,
    /// Voices a piece chooses among for a role, by each one's `weight` (the melody's call, the
    /// answer): each in place of the genre's added role for that role, never two roles on one voice.
    #[serde(default)]
    pub leads: Vec<crate::mood::RoleSpec>,
}

impl Genre {
    /// The palette's instrument for `role`, if the genre swaps it.
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
