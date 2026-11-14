//! Wave sets (2026-11-13, the operator's approval of AKWF): single-cycle waves for the synthesiser's
//! wavetable oscillator, kept as plain PCM WAV (a lossy codec smears a cycle of 600 samples), each
//! set a folder `waves/<id>/` beside `instruments/` and `samples/`, in the core set
//! (`data/music/waves/`) or in a pack. `waves.ron` is the set's index, written by
//! `scripts/instrument-packs.js`; `LICENSE.txt` beside it is the library's licence text.
//!
//! The music's wavetable oscillator reads them (rung E5, 2026-11-14: crates/humanity-music's
//! `wavetable.rs`, which finds a wave in the core set or an installed pack itself; docs/design/music.md
//! section 7f). Here they are data the core set carries and a pack can add, counted in Settings >
//! Audio > Instruments, and held by the data tests to their licence and their format.

use super::definition::{Licence, SourceInfo};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// One set's index.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WaveSet {
    pub id: String,
    /// Its name, by key.
    pub words: String,
    pub licence: Licence,
    pub source: SourceInfo,
    pub credit: String,
    pub waves: Vec<Wave>,
}

/// One single cycle: its file (under the set's folder), its length and format.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Wave {
    pub file: String,
    pub frames: u32,
    pub rate: u32,
    pub channels: u16,
    pub bits: u16,
}

impl WaveSet {
    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    /// Read `<root>/waves/<id>/waves.ron`.
    pub fn load(root: &Path, id: &str) -> Result<Self, String> {
        let path = Self::dir(root, id).join("waves.ron");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The set's folder under a root (`data/music` for the core set, a pack's folder otherwise).
    pub fn dir(root: &Path, id: &str) -> PathBuf {
        root.join("waves").join(id)
    }
}
