//! SOUND STYLES as data (docs/design/music-page.md section 15a, rung 3a): `data/music/styles/*.ron`,
//! one file per style (infinite-of-X: a new style is a file, never code).
//!
//! A style is a palette of our own instruments that any written or imported piece can be played
//! through: the piece keeps its notes, only its sound changes. The operator, 2026-11-20, asked
//! whether there is "a genre like mode for playing any music piece in midi like the original Elite
//! Frontier 2 game plays music"; this is that (the game is his reference for the feel only: nothing
//! of it is named, sampled or copied). The first styles are his choice of the same day: RETRO FM
//! (crate::fm_ops) and SOLO PIANO, then the GENERAL-MIDI-LIKE table for imported files.
//!
//! What plays each staff is decided once, on the music's build thread (never the audio thread):
//! 1. a staff of an imported MIDI file (it carries its General MIDI program, or plays on channel 10)
//!    takes the style's `programs` row for its program, or for a drum track the `drums` map key by
//!    key; a program or a drum no row names plays on the style's stand-in and is SAID IN WORDS
//!    ([`StyleNote`]), never dropped in silence;
//! 2. otherwise the first `parts` rule naming the staff's part (`timpani`, `strings_high`), else the
//!    first `roles` rule naming its role (melody, bass, pad...), else `default`; a style with no
//!    `default` leaves such a staff on its own instruments.
//!
//! SOLO PIANO's `reduction` turns a full score into a PIANO REDUCTION, the rules written down here
//! because the plan left them open (music-page.md section 25): every note is folded by octaves into
//! the piano's keys; notes struck together on the same key by several staves (a unison doubling)
//! sound once, at the loudest of their velocities, credited to the most prominent staff (melody
//! first, then the bass, then score order); at most `max_notes` keys go down together (two hands),
//! keeping the highest and the lowest and then the notes that add a pitch class not yet held, from
//! the top down, so the harmony survives and only doublings are lost; and the chord is VOICED as a
//! pianist voices it, the top note full, the bass a little under, the inner notes softer.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::scores::{DrumHit, ScoreEntry, StaffSpec, WrittenPiece};
use crate::written::{TimedNote, Timeline};

/// What a staff (or a drum key) plays in a style.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StyleVoice {
    /// A synthesised instrument of data/music/instruments (what plays when `part` has no recording).
    pub instrument: String,
    /// A part of data/music/parts.ron whose best installed recording plays it instead.
    #[serde(default)]
    pub part: Option<String>,
    /// dB on top of the staff's own level.
    #[serde(default)]
    pub level_db: f32,
    /// A pan in place of the staff's own (-1 .. 1; None: the staff's).
    #[serde(default)]
    pub pan: Option<f32>,
}

/// A rule by a staff's part id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartRule {
    pub part: String,
    pub voice: StyleVoice,
}

/// A rule by a staff's role (one of `tune::ROLE_IDS`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoleRule {
    pub role: String,
    pub voice: StyleVoice,
}

/// General MIDI programs `from..=to` (0 to 127, as the file counts them) and what plays them.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProgramRule {
    pub from: u8,
    pub to: u8,
    pub voice: StyleVoice,
}

/// General MIDI drum keys `from..=to` (35 to 81 in the drum map) and what plays them, at `pitch` (a
/// note name; None: the key itself, which a recorded hit ignores).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DrumRule {
    pub from: u8,
    pub to: u8,
    pub voice: StyleVoice,
    #[serde(default)]
    pub pitch: Option<String>,
}

/// How a full score becomes a piano reduction (the module's comment has the rules and why).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reduction {
    /// Keys down together at most (two hands).
    #[serde(default = "ten")]
    pub max_notes: u8,
    /// The piano's lowest and highest keys, as note names.
    #[serde(default = "a0")]
    pub low: String,
    #[serde(default = "c8")]
    pub high: String,
    /// Notes starting within this many milliseconds are struck together.
    #[serde(default = "together")]
    pub together_ms: f32,
    /// The voicing: the velocity of the top note, the bass and the inner notes, as a share of written.
    #[serde(default = "one")]
    pub top: f32,
    #[serde(default = "bottom")]
    pub bottom: f32,
    #[serde(default = "inner")]
    pub inner: f32,
}

fn ten() -> u8 {
    10
}
fn a0() -> String {
    "A0".into()
}
fn c8() -> String {
    "C8".into()
}
fn together() -> f32 {
    30.0
}
fn one() -> f32 {
    1.0
}
fn bottom() -> f32 {
    0.9
}
fn inner() -> f32 {
    0.75
}

/// One sound style (its RON names the struct `Style`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename = "Style")]
pub struct SoundStyle {
    /// Stable id: what the person's settings keep.
    pub id: String,
    /// Its name and its one plain sentence (what changes when it is chosen), by key; a community file
    /// may give plain `name` and `plain` instead.
    #[serde(default)]
    pub words: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub plain_words: String,
    #[serde(default)]
    pub plain: String,
    /// dB on top of the piece's own level (measured, so the style sounds at the music target).
    #[serde(default)]
    pub level_db: f32,
    #[serde(default)]
    pub parts: Vec<PartRule>,
    #[serde(default)]
    pub roles: Vec<RoleRule>,
    /// What plays a staff no rule names (None: the staff's own instruments).
    #[serde(default)]
    pub default: Option<StyleVoice>,
    /// Imported files: what plays each General MIDI program, the drum map, and what plays a drum key
    /// no row names (None: a drum track is left out, said in words).
    #[serde(default)]
    pub programs: Vec<ProgramRule>,
    #[serde(default)]
    pub drums: Vec<DrumRule>,
    #[serde(default)]
    pub drum_default: Option<StyleVoice>,
    /// What plays a program no row names (None: `default`, else the staff's own).
    #[serde(default)]
    pub program_default: Option<StyleVoice>,
    /// Solo piano's reduction of a full score.
    #[serde(default)]
    pub reduction: Option<Reduction>,
}

/// Something a person should be told about how a style plays a piece (the page puts it in words).
#[derive(Clone, Debug, PartialEq)]
pub enum StyleNote {
    /// Staff `staff` asks for General MIDI program `program`, which no instrument of ours plays yet:
    /// it plays on `plays_on` (a part's or an instrument's id).
    MissingProgram { staff: u16, program: u8, plays_on: String },
    /// Drum key `key` of staff `staff` has no instrument of ours yet: it plays on `plays_on`.
    MissingDrum { staff: u16, key: u8, plays_on: String },
    /// Staff `staff` (a drum track) is left out: the style plays no drums.
    LeftOut { staff: u16 },
}

/// What a style decided for one staff.
#[derive(Clone, Debug, PartialEq)]
pub struct StaffPlan {
    pub staff: u16,
    /// The voice it plays (None: its own instruments, or left out when `left_out`).
    pub voice: Option<StyleVoice>,
    /// Its General MIDI program, when the file names one.
    pub program: Option<u8>,
    pub left_out: bool,
}

/// The styles folder under the data folder.
pub fn styles_dir(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("music").join("styles")
}

/// Read every style (no folder: none). Errors name the file.
pub fn load_styles(data_dir: &Path) -> Result<Vec<SoundStyle>, String> {
    let dir = styles_dir(data_dir);
    let mut paths: Vec<_> = match std::fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "ron")).collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", dir.display())),
    };
    paths.sort();
    let mut out = Vec::new();
    for p in paths {
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let s: SoundStyle = ron::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        out.push(s);
    }
    Ok(out)
}

impl SoundStyle {
    /// Every voice the style names (to check each instrument exists, and to list its parts).
    pub fn voices(&self) -> impl Iterator<Item = &StyleVoice> {
        self.parts
            .iter()
            .map(|r| &r.voice)
            .chain(self.roles.iter().map(|r| &r.voice))
            .chain(self.default.iter())
            .chain(self.programs.iter().map(|r| &r.voice))
            .chain(self.drums.iter().map(|r| &r.voice))
            .chain(self.drum_default.iter())
            .chain(self.program_default.iter())
    }

    /// Checked when the library loads: an id, rules in range, roles that exist, note names that read.
    pub fn check(&self) -> Result<(), String> {
        let at = |what: String| format!("styles/{}.ron: {what}", self.id);
        if self.id.is_empty() {
            return Err("a style has no id".into());
        }
        for r in &self.roles {
            if !crate::tune::ROLE_IDS.contains(&r.role.as_str()) {
                return Err(at(format!("role {:?} is not one of {:?}", r.role, crate::tune::ROLE_IDS)));
            }
        }
        for r in &self.programs {
            if r.from > r.to || r.to > 127 {
                return Err(at(format!("programs {}..{} must rise within 0 to 127", r.from, r.to)));
            }
        }
        for r in &self.drums {
            if r.from > r.to || r.to > 127 || r.pitch.as_deref().is_some_and(|p| crate::tuning::parse_note(p).is_none()) {
                return Err(at(format!("drum keys {}..{} must rise within 0 to 127, and a pitch must be a note name", r.from, r.to)));
            }
        }
        if let Some(rd) = &self.reduction {
            let (lo, hi) = (crate::tuning::parse_note(&rd.low), crate::tuning::parse_note(&rd.high));
            if !matches!((lo, hi), (Some(l), Some(h)) if h - l >= 12) || rd.max_notes == 0 {
                return Err(at("the reduction's low and high must be note names at least an octave apart, and max_notes above 0".into()));
            }
        }
        if self.voices().any(|v| v.pan.is_some_and(|p| !(-1.0..=1.0).contains(&p))) {
            return Err(at("a pan must be -1 to 1".into()));
        }
        Ok(())
    }

    /// The program row for `program`.
    pub fn program_voice(&self, program: u8) -> Option<&StyleVoice> {
        self.programs.iter().find(|r| (r.from..=r.to).contains(&program)).map(|r| &r.voice)
    }

    /// The drum row for `key`, and the pitch it plays at.
    pub fn drum_voice(&self, key: u8) -> Option<(&StyleVoice, f32)> {
        self.drums.iter().find(|r| (r.from..=r.to).contains(&key)).map(|r| (&r.voice, r.pitch.as_deref().and_then(crate::tuning::parse_note).map(|m| m as f32).unwrap_or(key as f32)))
    }

    /// What plays each staff of `entry` (`programs[i]`: staff i's General MIDI program when the file
    /// names one and the entry does not), and what to tell the person.
    pub fn plan(&self, entry: &ScoreEntry, programs: &[(u16, u8)]) -> (Vec<StaffPlan>, Vec<StyleNote>) {
        let mut plans = Vec::new();
        let mut notes = Vec::new();
        let imported = !self.programs.is_empty() || !self.drums.is_empty();
        for s in &entry.staves {
            let program = s.program.or_else(|| programs.iter().find(|(st, _)| *st == s.staff).map(|(_, p)| *p));
            let fallback = || self.program_default.clone().or_else(|| self.default.clone());
            let (voice, left_out) = if s.drums {
                if imported && (!self.drums.is_empty() || self.drum_default.is_some()) {
                    (None, false)
                } else {
                    notes.push(StyleNote::LeftOut { staff: s.staff });
                    (None, true)
                }
            } else if let (true, Some(p)) = (imported, program) {
                match self.program_voice(p) {
                    Some(v) => (Some(v.clone()), false),
                    None => {
                        let fb = fallback();
                        let plays_on = fb.as_ref().map(|v| v.part.clone().unwrap_or_else(|| v.instrument.clone())).unwrap_or_else(|| s.part.clone().unwrap_or_else(|| s.instrument.clone()));
                        notes.push(StyleNote::MissingProgram { staff: s.staff, program: p, plays_on });
                        (fb, false)
                    }
                }
            } else {
                let by_part = s.part.as_ref().and_then(|p| self.parts.iter().find(|r| r.part == *p)).map(|r| r.voice.clone());
                let by_role = || self.roles.iter().find(|r| r.role == s.role).map(|r| r.voice.clone());
                (by_part.or_else(by_role).or_else(|| self.default.clone()), false)
            };
            plans.push(StaffPlan { staff: s.staff, voice, program, left_out });
        }
        (plans, notes)
    }

    /// The drum map a drum staff plays through, key by key (each key once, from 0 to 127), and the
    /// keys no row names (each played on `drum_default`, said in words).
    pub fn drum_map(&self, staff: u16, keys_used: &BTreeSet<u8>, notes: &mut Vec<StyleNote>) -> Vec<DrumHit> {
        let mut out = Vec::new();
        for &k in keys_used {
            match self.drum_voice(k) {
                Some((v, pitch)) => out.push(DrumHit { key: k, instrument: v.instrument.clone(), part: v.part.clone(), level_db: v.level_db, midi: pitch }),
                None => {
                    if let Some(v) = &self.drum_default {
                        notes.push(StyleNote::MissingDrum { staff, key: k, plays_on: v.part.clone().unwrap_or_else(|| v.instrument.clone()) });
                        out.push(DrumHit { key: k, instrument: v.instrument.clone(), part: v.part.clone(), level_db: v.level_db, midi: k as f32 });
                    }
                }
            }
        }
        out
    }
}

/// A piece played through a style: the piece (its staves pointed at the style's instruments, and its
/// timeline reduced for the piano when the style says so) and what to tell the person.
#[derive(Clone, Debug)]
pub struct Styled {
    pub piece: WrittenPiece,
    pub notes: Vec<StyleNote>,
}

/// Each staff's program as the MEI file names it (`instrDef@midi.instrnum`), for an entry written
/// before the index kept it.
pub fn file_programs(piece: &WrittenPiece) -> Vec<(u16, u8)> {
    piece.score.staves.iter().filter_map(|s| s.midi_program.map(|p| (s.n, p))).collect()
}

/// Play `piece` through `style`. On the build thread.
pub fn apply(style: &SoundStyle, piece: &WrittenPiece) -> Styled {
    let (plans, mut notes) = style.plan(&piece.entry, &file_programs(piece));
    let mut entry = piece.entry.clone();
    entry.level_db += style.level_db;
    let mut kept: Vec<StaffSpec> = Vec::new();
    for (s, plan) in entry.staves.iter().zip(plans.iter()) {
        if plan.left_out {
            continue;
        }
        let mut s = s.clone();
        s.program = plan.program;
        if let Some(v) = &plan.voice {
            s.instrument = v.instrument.clone();
            s.part = v.part.clone();
            s.level_db += v.level_db;
            if let Some(p) = v.pan {
                s.pan = p;
            }
        }
        if s.drums {
            let keys: BTreeSet<u8> = piece.timeline.notes.iter().filter(|n| n.staff == s.staff).map(|n| n.midi.round().clamp(0.0, 127.0) as u8).collect();
            s.drum_map = style.drum_map(s.staff, &keys, &mut notes);
        }
        kept.push(s);
    }
    entry.staves = kept;
    let timeline = match &style.reduction {
        Some(rd) => Arc::new(piano_reduction(&piece.timeline, &entry, rd)),
        None => piece.timeline.clone(),
    };
    Styled { piece: WrittenPiece { entry, score: piece.score.clone(), timeline }, notes }
}

/// How prominent a staff's role is in a reduction (lower first): the melody, then the bass, then the
/// rest in score order.
fn prominence(entry: &ScoreEntry, staff: u16) -> (u8, u16) {
    let role = entry.staves.iter().find(|s| s.staff == staff).map(|s| s.role.as_str()).unwrap_or("");
    (
        match role {
            "melody" => 0,
            "bass" => 1,
            _ => 2,
        },
        staff,
    )
}

/// A full score as a piano reduction (the module's comment has the rules). Only the staves `entry`
/// still plays are kept.
pub fn piano_reduction(t: &Timeline, entry: &ScoreEntry, rd: &Reduction) -> Timeline {
    let lo = crate::tuning::parse_note(&rd.low).unwrap_or(21) as f32;
    let hi = crate::tuning::parse_note(&rd.high).unwrap_or(108) as f32;
    let together_s = (rd.together_ms.max(0.0) / 1000.0) as f64;
    let fold = |mut m: f32| {
        while m < lo {
            m += 12.0;
        }
        while m > hi {
            m -= 12.0;
        }
        m
    };
    let playing: Vec<u16> = entry.staves.iter().map(|s| s.staff).collect();
    let mut notes: Vec<TimedNote> = t.notes.iter().filter(|n| playing.contains(&n.staff)).map(|n| TimedNote { midi: fold(n.midi), ..*n }).collect();
    notes.sort_by(|a, b| a.at_s.total_cmp(&b.at_s));
    let mut out: Vec<TimedNote> = Vec::with_capacity(notes.len());
    let mut i = 0;
    while i < notes.len() {
        // The notes struck together.
        let t0 = notes[i].at_s;
        let mut j = i;
        while j < notes.len() && notes[j].at_s - t0 <= together_s {
            j += 1;
        }
        let mut group: Vec<TimedNote> = notes[i..j].to_vec();
        // A unison doubling sounds once: per key, the loudest velocity, the longest hold, the most
        // prominent staff.
        group.sort_by(|a, b| a.midi.total_cmp(&b.midi).then(prominence(entry, a.staff).cmp(&prominence(entry, b.staff))));
        let mut keys: Vec<TimedNote> = Vec::with_capacity(group.len());
        for n in group {
            match keys.last_mut() {
                Some(k) if (k.midi - n.midi).abs() < 0.5 => {
                    k.vel = k.vel.max(n.vel);
                    k.hold_s = k.hold_s.max(n.hold_s);
                    k.dur_s = k.dur_s.max(n.dur_s);
                }
                _ => keys.push(n),
            }
        }
        // Two hands: the highest and the lowest, then a pitch class not yet held, from the top down,
        // then the rest from the top down, up to `max_notes`.
        let max = rd.max_notes.max(1) as usize;
        if keys.len() > max {
            let mut chosen: Vec<usize> = vec![keys.len() - 1, 0];
            let mut classes: BTreeSet<i32> = chosen.iter().map(|&k| (keys[k].midi.round() as i32).rem_euclid(12)).collect();
            for k in (1..keys.len() - 1).rev() {
                if chosen.len() >= max {
                    break;
                }
                let pc = (keys[k].midi.round() as i32).rem_euclid(12);
                if classes.insert(pc) {
                    chosen.push(k);
                }
            }
            for k in (1..keys.len() - 1).rev() {
                if chosen.len() >= max {
                    break;
                }
                if !chosen.contains(&k) {
                    chosen.push(k);
                }
            }
            chosen.sort_unstable();
            keys = chosen.into_iter().map(|k| keys[k]).collect();
        }
        // Voicing: the top note full, the bass a little under, the inner notes softer.
        let n_keys = keys.len();
        for (k, n) in keys.iter_mut().enumerate() {
            let share = if n_keys == 1 || k + 1 == n_keys {
                rd.top
            } else if k == 0 {
                rd.bottom
            } else {
                rd.inner
            };
            n.vel = (n.vel * share).clamp(0.0, 1.0);
        }
        out.extend(keys);
        i = j;
    }
    out.sort_by(|a, b| a.at_s.total_cmp(&b.at_s));
    // The syllables point into the notes by index; a reduction keeps no lyrics (a piano sings none).
    Timeline { notes: out, syllables: Vec::new(), bars: t.bars.clone(), length_s: t.length_s }
}

/// The style an imported MIDI file plays in unless the person chooses another: by the instruments it
/// asks for.
pub const IMPORTED_STYLE: &str = "general_midi";

/// Whether a written piece came from a MIDI file the person imported (its staves carry their General
/// MIDI programs or a drum channel, or it lives in their own folder).
pub fn is_imported(entry: &ScoreEntry) -> bool {
    entry.folder.is_some() || entry.staves.iter().any(|s| s.program.is_some() || s.drums)
}

/// A style of `styles` by its id.
pub fn find<'a>(styles: &'a [SoundStyle], id: Option<&str>) -> Option<&'a SoundStyle> {
    id.and_then(|id| styles.iter().find(|s| s.id == id))
}
