//! Pitch: the ONE place a frequency is made (docs/design/music.md, section 9).
//!
//! `f = a4 x 2^((midi - 69) / 12)`: equal temperament, A4 = 432 Hz by default (the operator's
//! choice, 2026-11-06: "Let's just keep focusing of 432 Hz"). Every voice asks [`note_hz`], so the
//! tuning reaches every note, the hum's resonances and the sampler's playback rate alike. A = 440
//! Hz is the other setting and costs exactly one number here; nothing is rendered or tuned
//! specially for it. Nothing in this module claims 432 does anything for health (the finding,
//! docs/reference/findings/2026-11-06-music-sources.md, Part E1).

use serde::{Deserialize, Serialize};

/// A4 in hertz at the default tuning.
pub const A4_DEFAULT_HZ: f32 = 432.0;

/// The tuning every note follows (Settings > Music > Tuning).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tuning {
    /// A4 = 432 Hz: the default.
    #[default]
    A432,
    /// A4 = 440 Hz: the usual concert pitch.
    A440,
}

impl Tuning {
    /// Both, in the order Settings offers them.
    pub const ALL: [Tuning; 2] = [Tuning::A432, Tuning::A440];

    /// A4 in hertz.
    pub fn a4_hz(self) -> f32 {
        match self {
            Tuning::A432 => 432.0,
            Tuning::A440 => 440.0,
        }
    }

    /// The words for it in Settings (data/i18n/en.json).
    pub fn words_key(self) -> &'static str {
        match self {
            Tuning::A432 => "settings.music.tuning_432",
            Tuning::A440 => "settings.music.tuning_440",
        }
    }
}

/// The frequency of MIDI note `midi` (69 = A4; fractional notes glide) at A4 = `a4` hertz.
/// Computed in f64 so a table of every note keeps its last digit.
#[inline]
pub fn note_hz(midi: f32, a4: f32) -> f32 {
    note_hz_f64(midi as f64, a4 as f64) as f32
}

/// [`note_hz`] in f64 (the tests' and the hum's resonances).
#[inline]
pub fn note_hz_f64(midi: f64, a4: f64) -> f64 {
    a4 * 2f64.powf((midi - 69.0) / 12.0)
}

/// The note names in a 12-note octave, sharps (what [`note_name`] writes).
pub const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// A pitch class from its letter name: "D" is 2, "F#" 6, "Bb" 10. None for anything else.
pub fn pitch_class(name: &str) -> Option<i32> {
    let mut chars = name.chars();
    let base = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut pc: i32 = base;
    for c in chars {
        match c {
            '#' => pc += 1,
            'b' => pc -= 1,
            _ => return None,
        }
    }
    Some(pc.rem_euclid(12))
}

/// A MIDI note from its name with octave, scientific pitch: "A4" is 69, "D2" 38, "F#3" 54, "Bb1"
/// 34, "C-1" 0. None for anything else.
pub fn parse_note(name: &str) -> Option<i32> {
    let name = name.trim();
    let split = name.find(|c: char| c.is_ascii_digit() || c == '-')?;
    let (letters, octave) = name.split_at(split);
    pitch_class(letters)?;
    let octave: i32 = octave.parse().ok()?;
    // The letter's own octave: "Cb4" is B3 and "B#3" is C4, so the accidental moves the number.
    let base = match letters.chars().next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        _ => 11,
    };
    let accidental: i32 = letters.chars().skip(1).map(|c| if c == '#' { 1 } else { -1 }).sum();
    let midi = (octave + 1) * 12 + base + accidental;
    (0..=127).contains(&midi).then_some(midi)
}

/// A MIDI note's name with octave ("A4", "D2", "F#3").
pub fn note_name(midi: i32) -> String {
    format!("{}{}", NOTE_NAMES[midi.rem_euclid(12) as usize], midi.div_euclid(12) - 1)
}

/// A mode or scale the composer improvises in: the steps of its notes above the key centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    Ionian,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Aeolian,
    Locrian,
    MajorPentatonic,
    MinorPentatonic,
    /// The harmonic minor (the raised seventh a minor key's V7 chord leans on; the electro swing
    /// genre's walking bass).
    HarmonicMinor,
    /// The minor blues scale: the minor pentatonic with its flat fifth, the swing riff's colour.
    Blues,
}

impl Mode {
    /// The semitones of each scale note above the key centre.
    pub fn steps(self) -> &'static [i32] {
        match self {
            Mode::Ionian => &[0, 2, 4, 5, 7, 9, 11],
            Mode::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Mode::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            Mode::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            Mode::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            Mode::Aeolian => &[0, 2, 3, 5, 7, 8, 10],
            Mode::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            Mode::MajorPentatonic => &[0, 2, 4, 7, 9],
            Mode::MinorPentatonic => &[0, 3, 5, 7, 10],
            Mode::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            Mode::Blues => &[0, 3, 5, 6, 7, 10],
        }
    }

    /// The pentatonic subset a melody leans on in this mode (no half steps, so a motif placed over
    /// any chord of the mode rarely clashes): major-ish modes the major pentatonic, the rest minor.
    pub fn pentatonic(self) -> Mode {
        match self {
            Mode::Ionian | Mode::Lydian | Mode::Mixolydian | Mode::MajorPentatonic => Mode::MajorPentatonic,
            Mode::Blues => Mode::Blues,
            _ => Mode::MinorPentatonic,
        }
    }

    /// Whether `pc` (0..12, relative to the key centre) is a note of this mode.
    pub fn contains(self, pc: i32) -> bool {
        self.steps().contains(&pc.rem_euclid(12))
    }
}

/// The MIDI note `steps` scale steps from `from` in the scale of `mode` on `key_pc` (a pitch class).
/// A `from` off the scale first snaps up to the next scale note. Used by the motifs ("two steps up
/// from a chord tone").
pub fn scale_step(from: i32, steps: i32, key_pc: i32, mode: Mode) -> i32 {
    let in_scale = |m: i32| mode.contains(m - key_pc);
    let mut m = from;
    let mut guard = 0;
    while !in_scale(m) && guard < 12 {
        m += 1;
        guard += 1;
    }
    let mut s = steps;
    while s > 0 {
        m += 1;
        if in_scale(m) {
            s -= 1;
        }
    }
    while s < 0 {
        m -= 1;
        if in_scale(m) {
            s += 1;
        }
    }
    m
}
