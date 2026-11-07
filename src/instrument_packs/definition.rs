//! An instrument definition: `data/music/instruments/<id>.ron` (music.md section 6's shape), or
//! `instruments/<id>.ron` inside an installed pack. Written by `just instrument-packs`.
//!
//! Only sampled instruments (`kind: Sampler((...))`) are read here; a definition of another kind
//! (the music engine's `Synth`, `Choir`, `Modal`, `Plucked`) is recognised and left to it, so both
//! can share the folder.

use serde::de::{Deserializer, EnumAccess, IgnoredAny, VariantAccess, Visitor};
use serde::Deserialize;

/// One instrument.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Instrument {
    pub id: String,
    /// Its name, by key.
    pub words: String,
    /// The part of the score it plays (`data/music/parts.ron`).
    #[serde(default)]
    pub part: String,
    /// Higher wins when several installed instruments play the same part.
    #[serde(default)]
    pub rank: i32,
    pub kind: Kind,
    #[serde(default)]
    pub range: NoteRange,
    #[serde(default)]
    pub reverb_send: f32,
    #[serde(default)]
    pub licence: Licence,
    #[serde(default)]
    pub source: SourceInfo,
    /// Who made the recordings, even where the licence needs no credit.
    #[serde(default)]
    pub credit: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct NoteRange {
    pub low: String,
    pub high: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Licence {
    /// "CC0 1.0".
    pub name: String,
    /// The licence in plain words, by key.
    pub words: String,
    /// The licence's text, in the instrument's sample folder.
    pub file: String,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct SourceInfo {
    pub library: String,
    pub url: String,
    pub version: String,
    /// When the raw library was downloaded (docs/reference/instrument-sources.md).
    pub fetched: String,
}

/// What kind of instrument: a sampled one this module plays, or another kind it leaves alone.
#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Sampler(Sampler),
    /// Another kind, by its name ("Synth").
    Other(String),
}

/// An enum variant's name, read as an identifier (RON writes `Sampler`, not `"Sampler"`).
struct VariantName(String);

impl<'de> Deserialize<'de> for VariantName {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = VariantName;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a kind's name")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<VariantName, E> {
                Ok(VariantName(v.to_string()))
            }
        }
        d.deserialize_identifier(V)
    }
}

impl<'de> Deserialize<'de> for Kind {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Kind;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an instrument kind, such as Sampler((...))")
            }
            fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Kind, A::Error> {
                let (VariantName(name), variant): (VariantName, _) = data.variant()?;
                if name == "Sampler" {
                    Ok(Kind::Sampler(variant.newtype_variant()?))
                } else {
                    variant.newtype_variant::<IgnoredAny>()?;
                    Ok(Kind::Other(name))
                }
            }
        }
        d.deserialize_enum("Kind", &["Sampler"], V)
    }
}

fn yes() -> bool {
    true
}
fn one() -> u8 {
    1
}

/// A sampled instrument: its recordings and how they map onto keys and velocities.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Sampler {
    /// The sample folder, under `samples/`.
    pub folder: String,
    /// False for drums and other hits with no pitch to tune: played at their recorded rate.
    #[serde(default = "yes")]
    pub pitched: bool,
    /// The library's own A4, measured by the pipeline (VCSL and VSCO are near 440; some libraries
    /// are not). Every zone's `recorded_hz` already includes it.
    pub source_a4_hz: f32,
    /// How the source tuning was found, in words.
    #[serde(default)]
    pub tuning_note: String,
    #[serde(default)]
    pub release_s: f32,
    /// The library normalised every file, so loudness by velocity is the engine's job.
    #[serde(default)]
    pub normalized: bool,
    pub zones: Vec<Zone>,
}

/// One sample and the keys and velocities it plays.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Zone {
    pub file: String,
    /// The key the sample sounds at.
    pub key: u8,
    pub lo_key: u8,
    pub hi_key: u8,
    pub lo_vel: u8,
    pub hi_vel: u8,
    /// Its turn among the zone's round robins, from 1.
    #[serde(default = "one")]
    pub rr: u8,
    /// The frequency the sample was recorded at (its key at the library's tuning). None when
    /// unpitched.
    #[serde(default)]
    pub recorded_hz: Option<f32>,
    /// What the pipeline measured, for the record.
    #[serde(default)]
    pub measured_hz: Option<f32>,
    #[serde(default)]
    pub loop_start: Option<u32>,
    #[serde(default)]
    pub loop_end: Option<u32>,
}

/// The frequency of MIDI note `midi` with A4 at `a4_hz`, equal temperament: music.md section 9's
/// one function, `f = a4 x 2^((midi - 69) / 12)`.
pub fn note_hz(midi: f64, a4_hz: f64) -> f64 {
    a4_hz * 2f64.powf((midi - 69.0) / 12.0)
}

impl Sampler {
    /// The zone that plays `key` at `velocity` (1..=127), taking turn `turn` among its round
    /// robins. A key or velocity outside every zone takes the nearest one, so a note is never lost.
    pub fn zone_for(&self, key: u8, velocity: u8, turn: usize) -> Option<&Zone> {
        let key_dist = |z: &Zone| {
            if key < z.lo_key {
                z.lo_key - key
            } else if key > z.hi_key {
                key - z.hi_key
            } else {
                0
            }
        };
        let nearest_key = self.zones.iter().map(key_dist).min()?;
        let at_key: Vec<&Zone> = self.zones.iter().filter(|&z| key_dist(z) == nearest_key).collect();
        let vel_dist = |z: &Zone| {
            if velocity < z.lo_vel {
                z.lo_vel - velocity
            } else if velocity > z.hi_vel {
                velocity - z.hi_vel
            } else {
                0
            }
        };
        let nearest_vel = at_key.iter().map(|&z| vel_dist(z)).min()?;
        let mut turns: Vec<&Zone> = at_key.into_iter().filter(|&z| vel_dist(z) == nearest_vel).collect();
        turns.sort_by_key(|z| (z.key, z.rr));
        // Several sample keys can share a zone's key range only in a hand-made file; prefer the
        // closest key, then rotate the round robins of that key.
        let best_key = turns.iter().map(|z| (z.key as i32 - key as i32).abs()).min()?;
        let turns: Vec<&Zone> = turns.into_iter().filter(|z| (z.key as i32 - key as i32).abs() == best_key).collect();
        Some(turns[turn % turns.len()])
    }

    /// The playback rate that makes `zone` sound at `key` with A4 = `a4_hz` (432 by default, the
    /// operator's choice; 440 the other): wanted frequency over recorded frequency. Nothing on disk
    /// is retuned. An unpitched zone plays at its recorded rate (1.0).
    pub fn playback_rate(&self, zone: &Zone, key: u8, a4_hz: f64) -> f64 {
        if !self.pitched {
            return 1.0;
        }
        let recorded = zone
            .recorded_hz
            .map(f64::from)
            .unwrap_or_else(|| note_hz(zone.key as f64, self.source_a4_hz as f64));
        note_hz(key as f64, a4_hz) / recorded
    }
}

impl Instrument {
    /// Parse a definition's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn sampler(&self) -> Option<&Sampler> {
        match &self.kind {
            Kind::Sampler(s) => Some(s),
            Kind::Other(_) => None,
        }
    }
}
