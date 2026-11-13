//! The library as the audio thread reads it: every name turned into an index, every note name into
//! a MIDI number, every dB into an amplitude, and every drum pattern into velocities, once, on the
//! game thread when a score is made. The composer then never compares a string, hashes a name or
//! allocates while it plays.

use std::collections::HashMap;
use std::sync::Arc;

use crate::dsp::db_to_amp;
use crate::instrument::{Instrument, InstrumentKind, SampleBank, SampleLibrary};
use crate::library::MusicLibrary;
use crate::mood::{Motif, Pulse, RoleKind, RoleSpec};
use crate::genre::{hit_velocity, Comping, Groove, HarmonyRule, Recipe};
use crate::tuning::{parse_note, pitch_class, Mode};

/// A pattern written as text, as velocities per grid step (`|` and spaces are for the eye only).
fn pattern(p: &str) -> Vec<f32> {
    p.chars().filter(|c| !c.is_whitespace() && *c != '|').map(hit_velocity).collect()
}

/// A bass pattern: its characters per step (`x` root, `5` fifth, `o` octave, `3` third, `.` rest).
fn bass_pattern(p: &str) -> Vec<u8> {
    p.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'|').collect()
}

/// A role ready to play.
#[derive(Clone, Debug)]
pub struct RRole {
    pub inst: u16,
    pub low: i32,
    pub high: i32,
    pub voices: u8,
    pub density: f32,
    pub level: f32,
    pub min_intensity: f32,
    pub pulse: Option<Pulse>,
}

/// A chord ready to play.
#[derive(Clone, Debug)]
pub struct RChord {
    pub root: i32,
    pub notes: Vec<i32>,
}

/// Key centres and a chord graph ready to play: a mood's, or a genre's own progressions.
#[derive(Clone, Debug)]
pub struct RHarmony {
    pub keys: Vec<(i32, Mode, f32)>,
    pub chords: Vec<RChord>,
    /// Per chord: the chords it moves to, with weights.
    pub moves: Vec<Vec<(usize, f32)>>,
    pub cadence: Vec<bool>,
    pub start: usize,
}

/// Resolve key centres and a chord graph (a mood's or a genre's), checking every name.
fn harmony_of(keys_in: &[crate::mood::KeyCentre], chords: &[crate::mood::ChordDef], moves_in: &[(String, String, f32)], cadences: &[String], start: &str, owner: &str) -> Result<RHarmony, String> {
    let mut keys = Vec::new();
    for k in keys_in {
        keys.push((pitch_class(&k.root).ok_or_else(|| format!("{owner}: key {:?} is not a note letter", k.root))?, k.mode, k.weight));
    }
    if keys.is_empty() {
        return Err(format!("{owner}: no keys"));
    }
    if chords.is_empty() {
        return Err(format!("{owner}: no chords"));
    }
    let chord = |name: &str| chords.iter().position(|c| c.name == name).ok_or_else(|| format!("{owner}: chord {name:?} is not one of its chords"));
    let mut moves = vec![Vec::new(); chords.len()];
    for (a, b, w) in moves_in {
        moves[chord(a)?].push((chord(b)?, *w));
    }
    for (i, mv) in moves.iter().enumerate() {
        if mv.is_empty() {
            return Err(format!("{owner}: chord {:?} moves nowhere (every chord needs a move out)", chords[i].name));
        }
    }
    let mut cadence = vec![false; chords.len()];
    for c in cadences {
        cadence[chord(c)?] = true;
    }
    Ok(RHarmony { keys, chords: chords.iter().map(|c| RChord { root: c.root, notes: c.notes.clone() }).collect(), moves, cadence, start: chord(start)? })
}

/// A mood ready to play.
#[derive(Clone, Debug)]
pub struct RMood {
    pub harmony: RHarmony,
    pub phrase_bars: Vec<u32>,
    pub chord_bars: (u32, u32),
    pub tempo: (f32, f32),
    pub beat_felt: bool,
    pub beats_per_bar: u32,
    pub roles: [Option<RRole>; 8],
    pub motifs: Vec<Motif>,
    pub level_db: f32,
    pub no_repeat_s: f32,
    pub reverb_t60_s: f32,
    pub reverb_wet: f32,
    pub urgent: bool,
    pub tempo_follows_intensity: bool,
    pub groove: Option<Groove>,
    pub drums: Vec<RDrum>,
    pub pad_patterns: Vec<Vec<f32>>,
    pub bass_patterns: Vec<Vec<u8>>,
}

/// The intensity bands ready to play (intensity.rs).
#[derive(Clone, Debug, Default)]
pub struct RIntensity {
    pub froms: Vec<f32>,
    /// Each band's mood (None: the place's).
    pub moods: Vec<Option<usize>>,
    pub hysteresis: f32,
    /// (instrument, note, velocity, beat) of each stinger note.
    pub stinger: Vec<(u16, i32, f32, f32)>,
    pub riser: Option<(u16, i32, f32)>,
    pub fall_bars: u32,
}

/// A drum part ready to play.
#[derive(Clone, Debug)]
pub struct RDrum {
    pub inst: u16,
    /// Each pattern: a velocity per grid step (0 = rest).
    pub patterns: Vec<Vec<f32>>,
    pub velocity: f32,
    pub midi: i32,
    pub min_intensity: f32,
}

/// A genre ready to play.
#[derive(Clone, Debug)]
pub struct RGenre {
    pub tempo: Option<(f32, f32)>,
    pub beat_felt: Option<bool>,
    pub groove: Groove,
    pub drums: Vec<RDrum>,
    pub pad_patterns: Vec<Vec<f32>>,
    pub comping: Comping,
    pub bass_patterns: Vec<Vec<u8>>,
    pub stab_patterns: Vec<Vec<f32>>,
    pub stab_hold_beats: f32,
    pub beats_per_bar: Option<u32>,
    /// Its own key centres and chord graph, played in calm moods in place of the mood's.
    pub progressions: Option<RHarmony>,
    pub motifs: Vec<Motif>,
    pub motif_scale: Option<Mode>,
    pub call_and_response: bool,
    pub dynamics: Option<(f32, f32)>,
    pub harmony: HarmonyRule,
    /// Per role: the palette's instrument, its level change (dB) and its own register (MIDI low,
    /// high), if it has one.
    pub palette: [Option<(u16, f32, Option<(i32, i32)>)>; 8],
    pub roles_off: [bool; 8],
    pub roles_add: [Option<RRole>; 8],
    pub recipe: Recipe,
    pub level_db: f32,
}

/// Everything a score plays from.
pub struct Resolved {
    pub instruments: Vec<Instrument>,
    /// Instrument i's installed recordings (None: not a sampler, or its pack is not installed).
    pub banks: Vec<Option<Arc<dyn SampleBank>>>,
    pub moods: Vec<RMood>,
    pub genres: Vec<RGenre>,
    pub intensity: RIntensity,
}

impl Resolved {
    /// Resolve a library against the installed recordings. Errors name the file and the field.
    pub fn new(lib: &MusicLibrary, samples: &SampleLibrary) -> Result<Resolved, String> {
        let index: HashMap<&str, usize> = lib.instruments.iter().enumerate().map(|(i, x)| (x.id.as_str(), i)).collect();
        // A sampler with no installed pack plays its fallback (followed a few steps, in case a
        // fallback is itself a sampler).
        let mut map = vec![0u16; lib.instruments.len()];
        for (i, inst) in lib.instruments.iter().enumerate() {
            let mut cur = i;
            for _ in 0..4 {
                match &lib.instruments[cur].kind {
                    InstrumentKind::Sampler(s) if !samples.contains_key(&lib.instruments[cur].id) => {
                        cur = *index.get(s.fallback.as_str()).ok_or_else(|| format!("instrument {}: fallback {:?} is not an instrument", inst.id, s.fallback))?;
                    }
                    _ => break,
                }
            }
            map[i] = cur as u16;
        }
        let inst_of = |id: &str, owner: &str| -> Result<u16, String> {
            let i = index.get(id).ok_or_else(|| format!("{owner}: instrument {id:?} is not in data/music/instruments"))?;
            Ok(map[*i])
        };
        let note = |n: &str, owner: &str| parse_note(n).ok_or_else(|| format!("{owner}: {n:?} is not a note name (like \"D3\" or \"F#4\")"));
        let role = |r: &RoleSpec, owner: &str| -> Result<RRole, String> {
            Ok(RRole {
                inst: inst_of(&r.instrument, owner)?,
                low: note(&r.low, owner)?,
                high: note(&r.high, owner)?,
                voices: r.voices.clamp(1, 8),
                density: r.density.clamp(0.0, 1.0),
                level: db_to_amp(r.level_db),
                min_intensity: r.min_intensity,
                pulse: r.pulse.clone(),
            })
        };
        let drums_of = |parts: &[crate::genre::DrumPart], owner: &str| -> Result<Vec<RDrum>, String> {
            let mut out = Vec::new();
            for d in parts {
                out.push(RDrum {
                    inst: inst_of(&d.instrument, owner)?,
                    patterns: d.patterns.iter().map(|p| pattern(p)).collect(),
                    velocity: d.velocity,
                    midi: note(&d.note, owner)?,
                    min_intensity: d.min_intensity,
                });
            }
            Ok(out)
        };
        let mut moods = Vec::new();
        for m in &lib.moods {
            let owner = format!("mood {}", m.id);
            let harmony = harmony_of(&m.keys, &m.chords, &m.moves, &m.cadences, &m.start, &owner)?;
            let mut roles: [Option<RRole>; 8] = Default::default();
            for r in &m.roles {
                roles[r.role.index()] = Some(role(r, &owner)?);
            }
            if m.phrase_bars.is_empty() || m.chord_bars.0 == 0 || m.chord_bars.1 < m.chord_bars.0 {
                return Err(format!("{owner}: phrase_bars needs at least one length and chord_bars a range of at least 1 bar"));
            }
            for (i, mo) in m.motifs.iter().enumerate() {
                if mo.steps.is_empty() || mo.steps.len() != mo.beats.len() {
                    return Err(format!("{owner}: motif {i} needs as many beats as steps"));
                }
            }
            moods.push(RMood {
                harmony,
                phrase_bars: m.phrase_bars.clone(),
                chord_bars: m.chord_bars,
                tempo: m.tempo_bpm,
                beat_felt: m.beat_felt,
                beats_per_bar: m.beats_per_bar.clamp(2, 12),
                roles,
                motifs: m.motifs.clone(),
                level_db: m.level_db,
                no_repeat_s: m.no_repeat_s,
                reverb_t60_s: m.reverb_t60_s,
                reverb_wet: m.reverb_wet,
                urgent: m.urgent,
                tempo_follows_intensity: m.tempo_follows_intensity,
                groove: m.groove,
                drums: drums_of(&m.drums, &owner)?,
                pad_patterns: m.pad_patterns.iter().map(|p| pattern(p)).collect(),
                bass_patterns: m.bass_patterns.iter().map(|p| bass_pattern(p)).collect(),
            });
        }
        let mut genres = Vec::new();
        for s in &lib.genres {
            let owner = format!("genre {}", s.id);
            let drums = drums_of(&s.drums, &owner)?;
            for (i, mo) in s.motifs.iter().enumerate() {
                if mo.steps.is_empty() || mo.steps.len() != mo.beats.len() || (!mo.hold.is_empty() && mo.hold.len() != mo.steps.len()) {
                    return Err(format!("{owner}: motif {i} needs as many beats (and holds, if any) as steps"));
                }
            }
            let mut palette: [Option<(u16, f32, Option<(i32, i32)>)>; 8] = Default::default();
            for p in &s.palette {
                let register = match (&p.low, &p.high) {
                    (Some(lo), Some(hi)) => Some((note(lo, &owner)?, note(hi, &owner)?)),
                    (None, None) => None,
                    _ => return Err(format!("{owner}: a palette entry's register needs both low and high")),
                };
                palette[p.role.index()] = Some((inst_of(&p.instrument, &owner)?, p.level_db, register));
            }
            let mut roles_off = [false; 8];
            for r in &s.roles_off {
                roles_off[r.index()] = true;
            }
            let mut roles_add: [Option<RRole>; 8] = Default::default();
            for r in &s.roles_add {
                roles_add[r.role.index()] = Some(role(r, &owner)?);
            }
            genres.push(RGenre {
                tempo: s.tempo_bpm,
                beat_felt: s.beat_felt,
                groove: s.groove.unwrap_or_default(),
                drums,
                pad_patterns: s.pad_patterns.iter().map(|p| pattern(p)).collect(),
                comping: s.comping,
                bass_patterns: s.bass_patterns.iter().map(|p| bass_pattern(p)).collect(),
                stab_patterns: s.stab_patterns.iter().map(|p| pattern(p)).collect(),
                stab_hold_beats: s.stab_hold_beats.max(0.02),
                beats_per_bar: s.beats_per_bar.map(|b| b.clamp(2, 12)),
                progressions: match &s.progressions {
                    Some(pr) => Some(harmony_of(&pr.keys, &pr.chords, &pr.moves, &pr.cadences, &pr.start, &owner)?),
                    None => None,
                },
                motifs: s.motifs.clone(),
                motif_scale: s.motif_scale,
                call_and_response: s.call_and_response,
                dynamics: s.dynamics.map(|(a, b)| (a.clamp(0.0, 1.0), b.clamp(a.clamp(0.0, 1.0), 1.0))),
                harmony: s.harmony,
                palette,
                roles_off,
                roles_add,
                recipe: s.recipe,
                level_db: s.level_db,
            });
        }
        let im = &lib.intensity;
        let mut intensity = RIntensity { froms: im.bands.iter().map(|b| b.from).collect(), hysteresis: im.hysteresis.max(0.0), fall_bars: im.fall_bars.max(1), ..Default::default() };
        for b in &im.bands {
            intensity.moods.push(match &b.mood {
                Some(id) => Some(lib.mood_index(id).ok_or_else(|| format!("intensity.ron: mood {id:?} is not a mood"))?),
                None => None,
            });
        }
        for g in &im.stinger {
            intensity.stinger.push((inst_of(&g.instrument, "intensity.ron stinger")?, note(&g.note, "intensity.ron stinger")?, g.velocity, g.beat));
        }
        if let Some(g) = &im.riser {
            intensity.riser = Some((inst_of(&g.instrument, "intensity.ron riser")?, note(&g.note, "intensity.ron riser")?, g.velocity));
        }
        let banks = lib.instruments.iter().map(|i| samples.get(&i.id).cloned()).collect();
        let mut instruments = lib.instruments.clone();
        for inst in instruments.iter_mut() {
            if let InstrumentKind::Sampler(s) = &mut inst.kind {
                s.fallback_index = index.get(s.fallback.as_str()).map(|&i| map[i]);
            }
        }
        Ok(Resolved { instruments, banks, moods, genres, intensity })
    }

    /// The mood band `b` plays (None: the place's own).
    pub fn band_mood(&self, b: usize) -> Option<usize> {
        self.intensity.moods.get(b).copied().flatten()
    }

    /// The role table index for a role kind.
    pub fn role_index(r: RoleKind) -> usize {
        r.index()
    }
}
