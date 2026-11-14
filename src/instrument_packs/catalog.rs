//! `data/music/packs.ron`, written by `just instrument-packs`: the core set and each downloadable
//! pack with its file, size, SHA-256, licence and instruments. And `data/music/parts.ron`, written
//! by hand: the parts of the score and the synthesised voice that plays each one when no recorded
//! instrument for it is installed.

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The catalog of instrument packs.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PackCatalog {
    /// The GitHub release the packs are assets of (a tag that is not `v*`, so uploading triggers no
    /// build, the star catalog's way).
    pub release: String,
    /// Where the pack files are downloaded from; each pack's `file` is appended.
    pub base_url: String,
    /// What ships inside the app.
    pub core: CoreSet,
    pub packs: Vec<PackEntry>,
}

/// The instruments that ship inside the app, always present.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct CoreSet {
    /// Its name, by key.
    pub words: String,
    pub instruments: Vec<InstrumentRef>,
    /// Its wave sets (2026-11-13): single cycles for the synthesiser, in `data/music/waves/<id>/`.
    #[serde(default)]
    pub waves: Vec<WaveSetRef>,
    pub bytes: u64,
}

/// An instrument a set holds, and the part of the score it plays (so the list can say what a pack
/// would add before it is downloaded: its definitions are inside the archive).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct InstrumentRef {
    pub id: String,
    pub part: String,
    /// What it holds, written by the pipeline from the definition it built (2026-11-13, the full
    /// packs): the most velocity layers and round robins any of its keys has, its samples, and the
    /// seconds of recording. 0 in a hand-written test catalog.
    #[serde(default)]
    pub layers: u32,
    #[serde(default)]
    pub round_robins: u32,
    #[serde(default)]
    pub zones: u32,
    #[serde(default)]
    pub seconds: f32,
    /// The core instrument this one is the whole recording of: a full pack keeps every velocity
    /// layer and round robin the library has, where the core set keeps a small cut of the same
    /// recording (a test holds the pack's to more).
    #[serde(default)]
    pub full_of: Option<String>,
}

/// A wave set a set holds (2026-11-13, AKWF): single-cycle waves for the synthesiser's wavetable
/// oscillator, kept as plain PCM, and how many. Nothing in the music plays them yet (the electronic
/// rung E5, docs/design/music.md); Settings says so. Its index is `waves/<id>/waves.ron`
/// (`super::waves::WaveSet`).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WaveSetRef {
    pub id: String,
    pub count: u32,
}

/// One downloadable pack.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct PackEntry {
    pub id: String,
    /// Bumped when the pack's contents change; an installed pack of an older version is offered
    /// again.
    pub version: u32,
    /// keys, strings, mallets, guitars, percussion.
    pub family: String,
    /// Its name, by key.
    pub words: String,
    /// What it adds, in a sentence, by key (Settings shows it under the name; 2026-11-13).
    #[serde(default)]
    pub about: Option<String>,
    /// The archive's file name on the release.
    pub file: String,
    /// The archive's size and SHA-256 (lower-case hex). Empty until the pipeline has built it.
    pub bytes: u64,
    pub sha256: String,
    /// The licence in plain words, by key.
    pub licence: String,
    /// The rows of `data/credits.ron` its licence obliges the app to show (a CC BY pack: the choir,
    /// the electric pianos; 2026-11-13). A test holds each to a row that is marked required and
    /// shown on the Credits page.
    #[serde(default)]
    pub credits: Vec<String>,
    pub instruments: Vec<InstrumentRef>,
    #[serde(default)]
    pub waves: Vec<WaveSetRef>,
}

impl PackEntry {
    /// The download address, with `base` (the catalog's, or a test server's) in front of the file.
    pub fn url(&self, base: &str) -> String {
        format!("{}{}", base, self.file)
    }

    /// Built by the pipeline (it has a size and a checksum to check a download against).
    pub fn is_built(&self) -> bool {
        self.bytes > 0 && self.sha256.len() == 64
    }
}

impl PackCatalog {
    /// Read `<data>/music/packs.ron`.
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("music").join("packs.ron");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        ron::from_str(text).map_err(|e| e.to_string())
    }

    pub fn pack(&self, id: &str) -> Option<&PackEntry> {
        self.packs.iter().find(|p| p.id == id)
    }

    /// The pack an instrument comes in (None for the core set's, or an unknown id).
    pub fn pack_of(&self, instrument: &str) -> Option<&PackEntry> {
        self.packs.iter().find(|p| p.instruments.iter().any(|i| i.id == instrument))
    }
}

/// The parts of the score: what the music asks for ("a piano", "string pads"), whichever
/// instrument plays it.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Parts {
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Part {
    pub id: String,
    /// Its name, by key.
    pub words: String,
    /// The synthesised voice that plays this part when no recorded instrument for it is installed.
    pub synth: SynthFallback,
    /// Another part that plays this one when no recording of it is installed, and plays the notes
    /// its recording lacks (the handpan's nine notes, the rest on the steel pan): what that part
    /// plays now, a recording or its synthesised voice. None: the role's own synthesised voice.
    #[serde(default)]
    pub fallback_part: Option<String>,
    /// No mood or genre plays this part yet (its instruments install and wait): Settings > Audio >
    /// Instruments says so, and a data test fails when a part no music names is not marked, or a
    /// marked one is named (data_tests.rs).
    #[serde(default)]
    pub not_used_yet: bool,
    /// Its notes are held (string sections, a bowed vibraphone, a glass): a recording of it with no
    /// loop points is held past its end by a crossfaded loop (`SampleZone::loop_xfade`).
    #[serde(default)]
    pub sustained: bool,
}

/// One of music.md's synthesised voices (section 6: `synth`, `choir`, `modal`, `plucked`) and the
/// preset of it the music engine plays.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SynthFallback {
    pub kind: String,
    pub preset: String,
}

impl Parts {
    /// Read `<data>/music/parts.ron`.
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join("music").join("parts.ron");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn part(&self, id: &str) -> Option<&Part> {
        self.parts.iter().find(|p| p.id == id)
    }
}

/// Where installed packs live: `<data>/music/packs/<id>/`, beside the core set in `<data>/music/`
/// (the star catalog's tiers live beside `stars.bin` the same way).
pub fn packs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("music").join("packs")
}
