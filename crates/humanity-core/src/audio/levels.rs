//! "Even out loudness": every sound played at the loudness of its kind (loudness rung 1,
//! 2026-11-06; docs/design/loudness.md).
//!
//! Two data files, both under `data/audio/`:
//!
//! - **`loudness.toml`**, written by hand: the kinds of sound (speech, sound effects, music,
//!   video, interface, notifications, ambience...), each with the loudness it plays at in LUFS,
//!   which bus of `data/sounds.toml` belongs to which kind, and the two limits on a raise (never
//!   past a true-peak ceiling, never more than so many dB). A new kind is a new table there, not
//!   code (infinite-of-X).
//! - **`measured_loudness.toml`**, GENERATED: how loud each shipped sound file is, measured once
//!   by [`super::loudness`] from the repo's own files (`just loudness` writes it; a lib test,
//!   `the_measured_loudness_data_matches_the_shipped_sounds`, fails when a sound file changed and the
//!   data did not). It is data made from the repo, the way the Library is made from the Accord, not
//!   a cache kept across runs: nothing at run time writes it.
//!
//! A sound's level is then one number: `target - measured` in dB, as an amplitude, with the raise
//! held under both limits. It is folded into the sound's volume when it starts to play
//! (`AudioManager::play_sound_bus`), so the audio thread does no extra work at all. On top of it
//! sit the sound's own authored volume from `sounds.toml` (a footstep is meant to be softer than a
//! crash: with the setting on, the volume says how far under its kind's loudness the sound sits,
//! instead of making up for how loud its file happened to be recorded) and the person's sliders.
//!
//! A file the measured data does not list (a mod's new sound) is measured when it is first loaded.
//! With the Dev page's "turn off all reuse" switch off, every sound is measured from its decoded
//! samples every time it plays and the data file is not trusted (`AudioManager::level_for`), so a
//! stale table is ruled in or out with one click.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use super::loudness::{Measure, SoundLoudness};

/// The hand-written targets, under the data folder.
pub const TARGETS_FILE: &str = "audio/loudness.toml";
/// The generated measurements, under the data folder.
pub const MEASURED_FILE: &str = "audio/measured_loudness.toml";

/// `loudness.toml`: what each kind of sound plays at, and the limits on a raise.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct LoudnessTargets {
    /// No raise pushes a sound's true peak above this (dBTP).
    pub ceiling_dbtp: f64,
    /// No sound is raised by more than this (dB): a near-silent file stays quiet rather than
    /// having its hiss brought up to full loudness.
    pub max_boost_db: f64,
    /// The kind a sound of an unknown kind or bus counts as.
    pub default_category: String,
    /// Every kind, by name.
    pub categories: BTreeMap<String, CategoryTarget>,
    /// Which kind each bus of `data/sounds.toml` is, for sounds that do not name one.
    #[serde(default)]
    pub buses: BTreeMap<String, String>,
}

/// One kind of sound.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct CategoryTarget {
    /// The loudness a sound of this kind plays at, before the person's own volume (LUFS: integrated
    /// loudness for a long sound, the loudest 400 ms for a one-shot).
    pub target_lufs: f64,
}

impl LoudnessTargets {
    /// Read `loudness.toml`'s text, checking that the default kind and every bus's kind exist.
    pub fn parse(text: &str) -> Result<Self, String> {
        let t: LoudnessTargets = toml::from_str(text).map_err(|e| format!("loudness.toml: {e}"))?;
        if !t.categories.contains_key(&t.default_category) {
            return Err(format!("loudness.toml: the default kind {:?} is not one of its kinds", t.default_category));
        }
        for (bus, kind) in &t.buses {
            if !t.categories.contains_key(kind) {
                return Err(format!("loudness.toml: bus {bus:?} names the kind {kind:?}, which is not one of its kinds"));
            }
        }
        Ok(t)
    }

    /// Read it from the data folder.
    pub fn load(data_dir: &Path) -> Result<Self, String> {
        let path = data_dir.join(TARGETS_FILE);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text)
    }

    /// The kind a name stands for: itself when it is a kind, the kind of the bus it names, else the
    /// default kind.
    pub fn resolve<'a>(&'a self, kind_or_bus: &str) -> &'a str {
        if let Some((k, _)) = self.categories.get_key_value(kind_or_bus) {
            return k;
        }
        match self.buses.get(kind_or_bus) {
            Some(k) => k,
            None => &self.default_category,
        }
    }

    /// The loudness a sound of this kind (or bus) plays at, LUFS.
    pub fn target_lufs(&self, kind_or_bus: &str) -> f64 {
        self.categories.get(self.resolve(kind_or_bus)).map_or(-23.0, |c| c.target_lufs)
    }
}

/// The level that brings `sound` to `target_lufs`, as an amplitude to multiply its volume by: down
/// as far as it takes, up by at most `max_boost_db` and never so far that its true peak AS PLAYED
/// (at its authored `volume`, the multiplier applied with the level) passes `ceiling_dbtp`; a sound
/// already peaking above the ceiling is not raised at all. A silent sound, or one with no reading,
/// is left as it is (1.0). The ceiling counts the authored volume because most shipped files peak
/// near full scale while playing 6 to 14 dB under it: without it, a quiet footstep at volume 0.25
/// could not be raised by even 1 dB, when as played it has 8 dB of room.
pub fn levelling_gain(sound: &SoundLoudness, target_lufs: f64, ceiling_dbtp: f64, max_boost_db: f64, volume: f64) -> f64 {
    if !sound.lufs.is_finite() || !target_lufs.is_finite() {
        return 1.0;
    }
    let mut db = target_lufs - sound.lufs;
    if db > 0.0 {
        let played_peak = sound.true_peak_dbtp + if volume > 0.0 { 20.0 * volume.log10() } else { f64::NEG_INFINITY };
        let headroom = if played_peak.is_finite() { (ceiling_dbtp - played_peak).max(0.0) } else { max_boost_db };
        db = db.min(max_boost_db).min(headroom);
    }
    10f64.powf(db / 20.0)
}

/// The level the sound at `path`, played at authored `volume`, plays at: 1.0 with "Even out
/// loudness" off (`even`) or with no targets; else the gain to its kind's loudness, from the
/// measured data when `trust_data` (reuse on) and the data lists it, otherwise from `measure_now`
/// (the decoded sound measured on the spot). `AudioManager` calls this for every sound it starts;
/// the tests call it on the shipped files.
pub fn level_for(levels: Option<&LoudnessLevels>, even: bool, path: &str, volume: f64, trust_data: bool, measure_now: impl FnOnce() -> Option<SoundLoudness>) -> f64 {
    if !even {
        return 1.0;
    }
    let Some(l) = levels else { return 1.0 };
    let sound = if trust_data { l.measured(path).copied() } else { None }.or_else(measure_now);
    sound.map_or(1.0, |s| l.gain_for(path, &s, volume))
}

/// The key a sound file is known by in the data: its path from the `assets` folder with forward
/// slashes (`audio/sfx/crash.ogg`), however the caller spelled it (`assets/audio/...`, backslashes).
pub fn sound_key(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = p.trim_start_matches("./");
    match p.find("assets/") {
        Some(i) => p[i + "assets/".len()..].to_string(),
        None => p.to_string(),
    }
}

/// `measured_loudness.toml`: how loud each shipped sound file is.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeasuredSounds {
    /// By [`sound_key`].
    pub sounds: BTreeMap<String, SoundLoudness>,
}

/// One row of the file as TOML has it.
#[derive(serde::Deserialize)]
struct MeasuredRow {
    loudness_lufs: f64,
    measure: String,
    true_peak_dbtp: f64,
    seconds: f64,
}

#[derive(serde::Deserialize)]
struct MeasuredFile {
    #[serde(default)]
    sounds: BTreeMap<String, MeasuredRow>,
}

impl MeasuredSounds {
    /// Read the file's text.
    pub fn parse(text: &str) -> Result<Self, String> {
        let f: MeasuredFile = toml::from_str(text).map_err(|e| format!("measured_loudness.toml: {e}"))?;
        let mut sounds = BTreeMap::new();
        for (key, row) in f.sounds {
            let measure = Measure::parse(&row.measure).ok_or_else(|| format!("measured_loudness.toml: {key}: measure {:?} is not one this app knows", row.measure))?;
            sounds.insert(key, SoundLoudness { lufs: row.loudness_lufs, measure, true_peak_dbtp: row.true_peak_dbtp, seconds: row.seconds });
        }
        Ok(MeasuredSounds { sounds })
    }

    /// Read it from the data folder; empty (every sound then measured as it loads) when it is not
    /// there or cannot be read, which is said once in the log.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(MEASURED_FILE);
        match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| Self::parse(&t)) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("[Audio] no measured loudness ({}: {e}); each sound is measured as it loads", path.display());
                MeasuredSounds::default()
            }
        }
    }

    /// The file's text: a header saying it is generated, then one table per sound in key order,
    /// numbers to 0.01 (a hundredth of a dB is under anything a person hears). The same sounds
    /// always give the same text, so a regenerated file only differs where a sound did.
    pub fn to_toml(&self) -> String {
        let num = |v: f64| if v.is_finite() { format!("{v:.2}") } else if v > 0.0 { "inf".to_string() } else { "-inf".to_string() };
        let mut out = String::new();
        out.push_str("# GENERATED by `just loudness` from the sound files under assets/: do not edit by hand.\n");
        out.push_str("# How loud each shipped sound is (ITU-R BS.1770, crates/humanity-core/src/audio/loudness.rs):\n");
        out.push_str("# loudness_lufs is the loudest 400 ms of a one-shot (3 s or shorter) or the integrated\n");
        out.push_str("# loudness of a longer sound, measured as it plays (two channels); true_peak_dbtp is its\n");
        out.push_str("# highest point between the samples. data/audio/loudness.toml says what each kind plays at;\n");
        out.push_str("# docs/design/loudness.md explains both. A lib test fails when a sound changes and this does not.\n");
        for (key, s) in &self.sounds {
            out.push_str(&format!("\n[sounds.\"{key}\"]\n"));
            out.push_str(&format!("loudness_lufs = {}\n", num(s.lufs)));
            out.push_str(&format!("measure = \"{}\"\n", s.measure.as_str()));
            out.push_str(&format!("true_peak_dbtp = {}\n", num(s.true_peak_dbtp)));
            out.push_str(&format!("seconds = {}\n", num(s.seconds)));
        }
        out
    }
}

/// Everything "Even out loudness" needs: the targets, the measurements and each known sound's kind.
#[derive(Clone, Debug)]
pub struct LoudnessLevels {
    targets: LoudnessTargets,
    measured: MeasuredSounds,
    /// Each catalogued sound file's kind, or its bus when it names no kind (resolved through the
    /// targets), by [`sound_key`].
    kinds: HashMap<String, String>,
}

impl LoudnessLevels {
    /// From the targets, the measurements and `sounds`: each catalogued sound file's path and its
    /// kind (or bus).
    pub fn new(targets: LoudnessTargets, measured: MeasuredSounds, sounds: impl IntoIterator<Item = (String, String)>) -> Self {
        let kinds = sounds.into_iter().map(|(path, kind)| (sound_key(&path), kind)).collect();
        LoudnessLevels { targets, measured, kinds }
    }

    /// Read both files from the data folder. An error when the targets are missing or wrong (then
    /// nothing is levelled, which the caller logs).
    pub fn load(data_dir: &Path, sounds: impl IntoIterator<Item = (String, String)>) -> Result<Self, String> {
        Ok(Self::new(LoudnessTargets::load(data_dir)?, MeasuredSounds::load(data_dir), sounds))
    }

    /// The targets.
    pub fn targets(&self) -> &LoudnessTargets {
        &self.targets
    }

    /// The kind of the sound at `path` (the default kind for a sound no catalogue entry names).
    pub fn kind_of(&self, path: &str) -> &str {
        match self.kinds.get(&sound_key(path)) {
            Some(k) => self.targets.resolve(k),
            None => &self.targets.default_category,
        }
    }

    /// The loudness the sound at `path` plays at with "Even out loudness" on, LUFS.
    pub fn target_for(&self, path: &str) -> f64 {
        self.targets.target_lufs(self.kind_of(path))
    }

    /// What the measured data says of the sound at `path`.
    pub fn measured(&self, path: &str) -> Option<&SoundLoudness> {
        self.measured.sounds.get(&sound_key(path))
    }

    /// The level for the sound at `path`, measured as `sound` (from the data, or just now), played at
    /// authored `volume`.
    pub fn gain_for(&self, path: &str, sound: &SoundLoudness, volume: f64) -> f64 {
        levelling_gain(sound, self.target_for(path), self.targets.ceiling_dbtp, self.targets.max_boost_db, volume)
    }
}

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;
