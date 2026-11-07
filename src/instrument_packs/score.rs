//! The bridge from the instrument library into the live score (crates/humanity-music; docs/design/
//! music.md, rung 6 and section 23): for every PART the moods' roles and the styles' roles and
//! palettes name (`part: Some("strings_high")`, the ids of `data/music/parts.ron`), the best recorded
//! instrument installed ([`InstrumentLibrary::choose`]) is decoded into the engine's sample bank and
//! plays that role, levelled to the synthesised voice it replaces so the mood's balance holds; with
//! none installed, the part plays what its `fallback_part` plays (the handpan: the steel pan), else
//! the role's own synthesised instrument. So the music always plays.
//!
//! Installing or removing a pack (Settings > Audio > Instruments) reaches the music playing at its
//! next phrase: the install bumps [`super::ui::installed_generation`], the music builds a new score on
//! its worker thread and the new score takes over at the next phrase boundary (engine/music.rs,
//! BUG-266). Before, it reached the music only when a new score started, after 30 s off the music.
//!
//! The recordings are decoded on that worker thread through the music's [`SampleBank`] (kept between
//! builds while the Dev page's "turn off all reuse" switch is on, every file decoded again while it
//! is off), and a score holds them through an `Arc`: a voice on the audio thread reads them, never
//! copying or freeing them there.

use std::collections::HashMap;
use std::sync::Arc;

use humanity_music::instrument::{EnvSpec, Instrument as MusicInstrument, InstrumentKind, SampleBank as _, SampleSet, SampleZone, SamplerSpec};
use humanity_music::render::{note_rms, note_rms_between};
use humanity_music::{MusicLibrary, Resolved, SampleLibrary};

use super::catalog::Parts;
use super::definition::note_hz;
use super::library::{Choice, InstrumentLibrary};
use super::samples::SampleBank;

/// What plays one part of the score.
#[derive(Clone, Debug, PartialEq)]
pub enum PartPlays {
    /// A recorded instrument (its definition's id), as the score's instrument `recorded:<id>`; for a
    /// part with no recording of its own, its fallback part's recording.
    Recorded { id: String },
    /// No recording for the part (or its fallback part) is installed: a synthesised instrument.
    Synth { instrument: String },
}

/// What [`attach`] did.
#[derive(Default)]
pub struct Attached {
    /// The decoded recordings, by the score's instrument id: what `Resolved::new` takes.
    pub samples: SampleLibrary,
    /// Each part the music names, and what plays it.
    pub plays: Vec<(String, PartPlays)>,
    /// Recordings that could not be decoded (their part falls back).
    pub problems: Vec<String>,
}

impl Attached {
    /// What plays `part`.
    pub fn plays(&self, part: &str) -> Option<&PartPlays> {
        self.plays.iter().find(|(p, _)| p == part).map(|(_, x)| x)
    }
}

/// The score's instrument id for a recorded instrument.
pub fn recorded_id(definition_id: &str) -> String {
    format!("recorded:{definition_id}")
}

/// Every part `music` names: its moods' roles, its styles' added roles and their palettes, each
/// once, with the synthesised instrument the first naming gives it and the middle of its register
/// (MIDI; a palette entry with no register of its own, middle C). The parts Settings > Audio >
/// Instruments can truthfully say the music plays.
pub fn named_parts(music: &MusicLibrary) -> Vec<(String, String, i32)> {
    let mut named: Vec<(String, String, i32)> = Vec::new();
    let mid = |lo: &str, hi: &str| {
        let lo = humanity_music::tuning::parse_note(lo).unwrap_or(48);
        (lo + humanity_music::tuning::parse_note(hi).unwrap_or(lo)) / 2
    };
    let roles = music.moods.iter().flat_map(|m| m.roles.iter()).chain(music.styles.iter().flat_map(|s| s.roles_add.iter()));
    for r in roles {
        if let Some(p) = &r.part {
            if !named.iter().any(|(n, _, _)| n == p) {
                named.push((p.clone(), r.instrument.clone(), mid(&r.low, &r.high)));
            }
        }
    }
    for e in music.styles.iter().flat_map(|s| s.palette.iter()) {
        if let Some(p) = &e.part {
            if !named.iter().any(|(n, _, _)| n == p) {
                let m = match (&e.low, &e.high) {
                    (Some(lo), Some(hi)) => mid(lo, hi),
                    _ => 60,
                };
                named.push((p.clone(), e.instrument.clone(), m));
            }
        }
    }
    named
}

/// The parts that can SOUND with `styles` playing (indices into `music.styles`: the person's style,
/// a blend's second, a place's suggestion): each calm mood's roles as those styles lay them out (an
/// added role in place of the mood's, a role a style silences left out, a palette entry's part in
/// place of the role's), every urgent mood's roles as written (danger plays as written in every
/// style), and the styles' added roles. What [`attach`] decodes, so recordings no playing style can
/// reach are never held in memory (music.md section 14's 64 MB: the piano is decoded only while
/// lo-fi plays, the strings only while a style that keeps the pads does). Each once, as
/// [`named_parts`] gives them.
pub fn sounding_parts(music: &MusicLibrary, styles: &[usize]) -> Vec<(String, String, i32)> {
    let mut named: Vec<(String, String, i32)> = Vec::new();
    let mut add = |part: &Option<String>, inst: &str, lo: &str, hi: &str| {
        if let Some(p) = part {
            if !named.iter().any(|(n, _, _)| n == p) {
                let lo = humanity_music::tuning::parse_note(lo).unwrap_or(48);
                let mid = (lo + humanity_music::tuning::parse_note(hi).unwrap_or(lo)) / 2;
                named.push((p.clone(), inst.to_string(), mid));
            }
        }
    };
    for m in &music.moods {
        for r in &m.roles {
            if m.urgent {
                add(&r.part, &r.instrument, &r.low, &r.high);
                continue;
            }
            for s in styles.iter().filter_map(|&i| music.styles.get(i)) {
                if s.roles_add.iter().any(|a| a.role == r.role) || s.roles_off.contains(&r.role) {
                    continue;
                }
                match s.palette_for(r.role) {
                    Some(e) => add(&e.part, &e.instrument, e.low.as_deref().unwrap_or(&r.low), e.high.as_deref().unwrap_or(&r.high)),
                    None => add(&r.part, &r.instrument, &r.low, &r.high),
                }
            }
        }
    }
    for s in styles.iter().filter_map(|&i| music.styles.get(i)) {
        for a in &s.roles_add {
            add(&a.part, &a.instrument, &a.low, &a.high);
        }
    }
    named
}

/// Point every role and palette entry of `music` that names `part` at the score's instrument `id`.
fn point(music: &mut MusicLibrary, part: &str, id: &str) {
    for r in music.moods.iter_mut().flat_map(|m| m.roles.iter_mut()).chain(music.styles.iter_mut().flat_map(|s| s.roles_add.iter_mut())) {
        if r.part.as_deref() == Some(part) {
            r.instrument = id.to_string();
        }
    }
    for e in music.styles.iter_mut().flat_map(|s| s.palette.iter_mut()) {
        if e.part.as_deref() == Some(part) {
            e.instrument = id.to_string();
        }
    }
}

/// The crossfaded loop a sustained recording with no loop points of its own is held by: from 40% to
/// 85% of it (past the bow's attack, before the fade the pipeline gives a note it cut), faded over
/// 0.3 s or a third of the loop, whichever is shorter. None for a recording too short to loop.
pub fn soft_loop(len: usize, rate: u32) -> Option<(usize, usize, usize)> {
    let (a, b) = (len * 2 / 5, len * 17 / 20);
    let x = ((0.3 * rate as f64) as usize).min((b.saturating_sub(a)) / 3);
    (x > 64 && a >= x && b > a + x).then_some((a, b, x))
}

/// A sustained part is levelled over this much of a note, seconds: the length of a calm chord.
const SUSTAIN_WINDOW_S: f32 = 8.0;

/// How many fallback parts `part` leads through (a part comes after the part it falls back to).
fn depth(part: &str, parts: &Parts) -> usize {
    let mut cur = part.to_string();
    let mut n = 0;
    while let Some(next) = parts.part(&cur).and_then(|p| p.fallback_part.clone()) {
        n += 1;
        if n > 4 {
            break;
        }
        cur = next;
    }
    n
}

/// Attach the best installed recording of every part that can sound with `styles` playing
/// ([`sounding_parts`]; None: every part `music` names, [`named_parts`]): decode it
/// through `bank` into a `SampleSet`, add it to `music` as a `Sampler` instrument whose fallback is
/// what plays the part without it (its fallback part's instrument, else the role's synthesised one),
/// point those roles and palette entries at it, and level it to the synthesised voice it replaces
/// (RMS of one note at velocity 1, at A = `a4`). A part with no recording plays its fallback part's
/// instrument when it has one.
pub fn attach(music: &mut MusicLibrary, instruments: &InstrumentLibrary, parts: &Parts, a4: f32, bank: &mut SampleBank, styles: Option<&[usize]>) -> Attached {
    let mut out = Attached::default();
    let mut named = match styles {
        Some(s) => sounding_parts(music, s),
        None => named_parts(music),
    };
    named.sort_by_key(|(p, _, _)| depth(p, parts));
    // The synthesised voices, resolved once, to level each recording against.
    let synth = Resolved::new(music, &SampleLibrary::new()).ok();
    // What each part attached so far plays: the score's instrument id.
    let mut plays_as: HashMap<String, (String, PartPlays)> = HashMap::new();
    for (part, synth_inst, mid) in named {
        let fallback = parts
            .part(&part)
            .and_then(|p| p.fallback_part.as_ref())
            .and_then(|f| plays_as.get(f).cloned())
            .unwrap_or_else(|| (synth_inst.clone(), PartPlays::Synth { instrument: synth_inst.clone() }));
        let mut fall_back = |music: &mut MusicLibrary, out: &mut Attached| {
            if fallback.0 != synth_inst {
                point(music, &part, &fallback.0);
            }
            out.plays.push((part.clone(), fallback.1.clone()));
            plays_as.insert(part.clone(), fallback.clone());
        };
        let recorded = match instruments.choose(&part, parts) {
            Some(Choice::Sampled(a)) => a,
            _ => {
                fall_back(music, &mut out);
                continue;
            }
        };
        let s = recorded.sampler();
        let sustained = parts.part(&part).is_some_and(|p| p.sustained);
        let mut zones = Vec::new();
        for z in &s.zones {
            match bank.get(&recorded.zone_path(&z.file)) {
                Ok(d) => zones.push(SampleZone {
                    loop_xfade: match (z.loop_start, z.loop_end) {
                        (Some(_), Some(_)) => 0,
                        _ if sustained => soft_loop(d.frames.len(), d.sample_rate).map_or(0, |(_, _, x)| x),
                        _ => 0,
                    },
                    frames: d.frames.clone(),
                    rate: d.sample_rate as f32,
                    recorded_hz: if s.pitched { z.recorded_hz.unwrap_or_else(|| note_hz(z.key as f64, s.source_a4_hz as f64) as f32) } else { 0.0 },
                    low_note: z.lo_key as i32,
                    high_note: z.hi_key as i32,
                    velocity: (z.lo_vel as f32 + z.hi_vel as f32) * 0.5 / 127.0,
                    loop_range: match (z.loop_start, z.loop_end) {
                        (Some(a), Some(b)) => Some((a as usize, b as usize)),
                        _ if sustained => soft_loop(d.frames.len(), d.sample_rate).map(|(a, b, _)| (a, b)),
                        _ => None,
                    },
                }),
                Err(e) => out.problems.push(format!("{}: {e}", recorded.def.id)),
            }
        }
        if zones.is_empty() {
            fall_back(music, &mut out);
            continue;
        }
        let set = SampleSet { zones, exact_keys: s.exact_keys };
        // Level: the recording against one note of the synthesised voice it replaces, the zone for
        // the role's middle note at a firm velocity. A struck or plucked part: both over their first
        // second. A sustained part: both over the length of a calm chord (8 s), the synthesised pad's
        // slow attack (3.5 to 6 s) included, as it is heard (BUG-266: over the first second alone a
        // pad read 5 to 8 dB under itself, and the strings that replaced it played that much too
        // quiet).
        let sample_rms = set
            .pick(mid, 0.8, 0)
            .map(|i| {
                let z = &set.zones[i];
                let f = &z.frames;
                let window_s = if sustained { SUSTAIN_WINDOW_S } else { 1.0 };
                let (a, b) = (0, f.len().min((z.rate * window_s) as usize).max(1));
                let sc = humanity_music::instrument::SAMPLE_SCALE as f64;
                ((f[a..b].iter().map(|x| (x[0] as f64 * sc).powi(2) + (x[1] as f64 * sc).powi(2)).sum::<f64>()) / (2 * (b - a).max(1)) as f64).sqrt() as f32
            })
            .unwrap_or(0.0);
        let synth_rms = synth
            .as_ref()
            .and_then(|res| {
                music.instrument_index(&synth_inst).map(|i| {
                    if sustained {
                        note_rms_between(res, i as u16, mid as f32, a4, 0.0, SUSTAIN_WINDOW_S, 48000.0)
                    } else {
                        note_rms(res, i as u16, mid as f32, a4, 1.0, 48000.0)
                    }
                })
            })
            .unwrap_or(0.0);
        let level_db = if sample_rms > 1e-6 && synth_rms > 1e-6 { (20.0 * (synth_rms / sample_rms).log10()).clamp(-30.0, 30.0) } else { 0.0 };
        let id = recorded_id(&recorded.def.id);
        music.instruments.retain(|i| i.id != id);
        music.instruments.push(MusicInstrument {
            id: id.clone(),
            words: recorded.def.words.clone(),
            kind: InstrumentKind::Sampler(SamplerSpec {
                pack: recorded.pack.clone().unwrap_or_else(|| "core".to_string()),
                folder: s.folder.clone(),
                fallback: fallback.0.clone(),
                envelope: EnvSpec { attack_s: 0.002, decay_s: 0.0, sustain: 1.0, release_s: s.release_s.max(0.05) },
                fallback_index: None,
            }),
            level_db,
            reverb_send: recorded.def.reverb_send,
            pan_spread: 0.0,
        });
        point(music, &part, &id);
        out.samples.insert(id.clone(), Arc::new(set));
        let plays = PartPlays::Recorded { id: recorded.def.id.clone() };
        out.plays.push((part.clone(), plays.clone()));
        plays_as.insert(part, (id, plays));
    }
    out
}
