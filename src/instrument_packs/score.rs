//! The bridge from the instrument library into the live score (crates/humanity-music; docs/design/
//! music.md, rung 6 and section 23): for every PART a mood's roles name (`part: Some("deep_drum")`,
//! the ids of `data/music/parts.ron`), the best recorded instrument installed
//! ([`InstrumentLibrary::choose`]) is decoded into the engine's sample bank and plays that role,
//! levelled to the synthesised voice it replaces so the mood's balance holds; with none installed,
//! the role plays its own synthesised instrument. So the music always plays, and installing a pack
//! (Settings > Audio > Instruments) changes what plays from the next piece on.
//!
//! The recordings are decoded here, on the game thread, when a score starts, and held by that score
//! only: nothing is kept between scores (the reuse rule; a new score decodes again), and a voice on
//! the audio thread reads them through an `Arc`, never copying or freeing them there.

use std::sync::Arc;

use humanity_music::instrument::{EnvSpec, Instrument as MusicInstrument, InstrumentKind, SampleBank as _, SampleSet, SampleZone, SamplerSpec};
use humanity_music::render::note_rms;
use humanity_music::{MusicLibrary, Resolved, SampleLibrary};

use super::catalog::Parts;
use super::definition::note_hz;
use super::library::{Choice, InstrumentLibrary};
use super::samples::decode_file;

/// What plays one part of the score.
#[derive(Clone, Debug, PartialEq)]
pub enum PartPlays {
    /// A recorded instrument (its definition's id), as the score's instrument `recorded:<id>`.
    Recorded { id: String },
    /// No recording for the part is installed: the role's synthesised instrument.
    Synth { instrument: String },
}

/// What [`attach`] did.
#[derive(Default)]
pub struct Attached {
    /// The decoded recordings, by the score's instrument id: what `Resolved::new` takes.
    pub samples: SampleLibrary,
    /// Each part the moods name, and what plays it.
    pub plays: Vec<(String, PartPlays)>,
    /// Recordings that could not be decoded (their part falls back to its synthesised voice).
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

/// Attach the best installed recording of every part `music`'s moods and styles name: decode it
/// into a `SampleSet`, add it to `music` as a `Sampler` instrument whose fallback is the role's own
/// synthesised instrument, point those roles at it, and level it to that synthesised voice (RMS of
/// one note at velocity 1, at A = `a4`).
pub fn attach(music: &mut MusicLibrary, instruments: &InstrumentLibrary, parts: &Parts, a4: f32) -> Attached {
    let mut out = Attached::default();
    // Every part a role names, with the first role's synthesised instrument and register.
    let mut named: Vec<(String, String, i32)> = Vec::new();
    let roles = music.moods.iter().flat_map(|m| m.roles.iter()).chain(music.styles.iter().flat_map(|s| s.roles_add.iter()));
    for r in roles {
        if let Some(p) = &r.part {
            if !named.iter().any(|(n, _, _)| n == p) {
                let lo = humanity_music::tuning::parse_note(&r.low).unwrap_or(48);
                let hi = humanity_music::tuning::parse_note(&r.high).unwrap_or(lo);
                named.push((p.clone(), r.instrument.clone(), (lo + hi) / 2));
            }
        }
    }
    // The synthesised voices, resolved once, to level each recording against.
    let synth = Resolved::new(music, &SampleLibrary::new()).ok();
    for (part, fallback, mid) in named {
        let recorded = match instruments.choose(&part, parts) {
            Some(Choice::Sampled(a)) => a,
            _ => {
                out.plays.push((part, PartPlays::Synth { instrument: fallback }));
                continue;
            }
        };
        let s = recorded.sampler();
        let mut zones = Vec::new();
        for z in &s.zones {
            match decode_file(&recorded.zone_path(&z.file)) {
                Ok(d) => zones.push(SampleZone {
                    frames: Arc::new(d.frames),
                    rate: d.sample_rate as f32,
                    recorded_hz: if s.pitched { z.recorded_hz.unwrap_or_else(|| note_hz(z.key as f64, s.source_a4_hz as f64) as f32) } else { 0.0 },
                    low_note: z.lo_key as i32,
                    high_note: z.hi_key as i32,
                    velocity: (z.lo_vel as f32 + z.hi_vel as f32) * 0.5 / 127.0,
                    loop_range: match (z.loop_start, z.loop_end) {
                        (Some(a), Some(b)) => Some((a as usize, b as usize)),
                        _ => None,
                    },
                }),
                Err(e) => out.problems.push(format!("{}: {e}", recorded.def.id)),
            }
        }
        if zones.is_empty() {
            out.plays.push((part, PartPlays::Synth { instrument: fallback }));
            continue;
        }
        let set = SampleSet { zones, exact_keys: s.exact_keys };
        // Level: the recording's RMS over its first second (the zone for the role's middle note at
        // a firm velocity) against one note of the synthesised voice it replaces.
        let sample_rms = set
            .pick(mid, 0.8, 0)
            .map(|i| {
                let f = &set.zones[i].frames;
                let n = f.len().min(set.zones[i].rate as usize).max(1);
                ((f[..n].iter().map(|x| (x[0] as f64).powi(2) + (x[1] as f64).powi(2)).sum::<f64>()) / (2 * n) as f64).sqrt() as f32
            })
            .unwrap_or(0.0);
        let synth_rms = synth
            .as_ref()
            .and_then(|res| music.instrument_index(&fallback).map(|i| note_rms(res, i as u16, mid as f32, a4, 1.0, 48000.0)))
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
                fallback: fallback.clone(),
                envelope: EnvSpec { attack_s: 0.002, decay_s: 0.0, sustain: 1.0, release_s: s.release_s.max(0.05) },
                fallback_index: None,
            }),
            level_db,
            reverb_send: recorded.def.reverb_send,
            pan_spread: 0.0,
        });
        for r in music.moods.iter_mut().flat_map(|m| m.roles.iter_mut()).chain(music.styles.iter_mut().flat_map(|s| s.roles_add.iter_mut())) {
            if r.part.as_deref() == Some(part.as_str()) {
                r.instrument = id.clone();
            }
        }
        out.samples.insert(id, Arc::new(set));
        out.plays.push((part, PartPlays::Recorded { id: recorded.def.id.clone() }));
    }
    out
}
