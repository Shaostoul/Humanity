//! Every sampled instrument available now, the core set's and the installed packs', and the best
//! one for each part of the score, with the part's synthesised voice when there is none: the music
//! always plays.
//!
//! The music engine (crates/humanity-music, music.md section 4.3) asks [`InstrumentLibrary::choose`]
//! for each part a mood names, plays a `Choice::Sampled` through its sampler voice
//! ([`super::samples`]) and a `Choice::Synth` through its own synthesised voice.

use std::path::{Path, PathBuf};

use super::catalog::{packs_dir, Parts, SynthFallback};
use super::definition::{Instrument, Sampler};

/// An instrument that can play now: its definition, where its samples are, and which pack it came
/// in (None for the core set).
#[derive(Debug, Clone, PartialEq)]
pub struct Available {
    pub def: Instrument,
    pub samples_dir: PathBuf,
    pub pack: Option<String>,
}

impl Available {
    pub fn sampler(&self) -> &Sampler {
        self.def.sampler().expect("only sampled instruments are Available")
    }

    /// The file of a zone.
    pub fn zone_path(&self, file: &str) -> PathBuf {
        self.samples_dir.join(file)
    }
}

/// What plays a part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Choice<'a> {
    /// A recorded instrument.
    Sampled(&'a Available),
    /// No recorded instrument for this part is installed: its synthesised voice.
    Synth(&'a SynthFallback),
}

/// The instruments found, and anything that could not be read (shown on the Settings section and
/// logged, never fatal: a broken instrument is skipped and its part falls back).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InstrumentLibrary {
    pub instruments: Vec<Available>,
    pub problems: Vec<String>,
}

impl InstrumentLibrary {
    /// The core set in `<data>/music/` and every installed pack in `<data>/music/packs/<id>/`.
    pub fn load(data_dir: &Path) -> Self {
        let mut roots = vec![(data_dir.join("music"), None)];
        let packs = packs_dir(data_dir);
        for (id, _) in super::install::installed_packs(&packs) {
            roots.push((packs.join(&id), Some(id)));
        }
        Self::load_roots(&roots)
    }

    /// The sampled instruments under each root's `instruments/` folder, with their samples under
    /// the root's `samples/<folder>/`. An instrument whose definition does not parse, or names a
    /// sample file that is not there, is left out with the reason in `problems`.
    pub fn load_roots(roots: &[(PathBuf, Option<String>)]) -> Self {
        let mut lib = Self::default();
        for (root, pack) in roots {
            let dir = root.join("instruments");
            let Ok(entries) = std::fs::read_dir(&dir) else { continue };
            let mut files: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ron")).collect();
            files.sort();
            for file in files {
                let def = match std::fs::read_to_string(&file).map_err(|e| e.to_string()).and_then(|t| Instrument::parse(&t)) {
                    Ok(d) => d,
                    Err(e) => {
                        lib.problems.push(format!("{}: {e}", file.display()));
                        continue;
                    }
                };
                let Some(sampler) = def.sampler() else { continue };
                let samples_dir = root.join("samples").join(&sampler.folder);
                if let Some(missing) = sampler.zones.iter().find(|z| !samples_dir.join(&z.file).is_file()) {
                    lib.problems.push(format!("{}: its sample {} is missing", def.id, samples_dir.join(&missing.file).display()));
                    continue;
                }
                if sampler.zones.is_empty() {
                    lib.problems.push(format!("{}: it has no samples", def.id));
                    continue;
                }
                lib.instruments.push(Available { def, samples_dir, pack: pack.clone() });
            }
        }
        lib
    }

    pub fn get(&self, id: &str) -> Option<&Available> {
        self.instruments.iter().find(|a| a.def.id == id)
    }

    /// The best recorded instrument for `part`: the highest rank; on a tie, one from a pack over
    /// the core set's, then the first id.
    pub fn best_for(&self, part: &str) -> Option<&Available> {
        self.instruments
            .iter()
            .filter(|a| a.def.part == part)
            .max_by(|a, b| {
                a.def
                    .rank
                    .cmp(&b.def.rank)
                    .then(a.pack.is_some().cmp(&b.pack.is_some()))
                    .then(b.def.id.cmp(&a.def.id))
            })
    }

    /// What plays `part`: the best recorded instrument installed, else the part's synthesised voice.
    /// None only for a part `parts` does not know.
    pub fn choose<'a>(&'a self, part: &str, parts: &'a Parts) -> Option<Choice<'a>> {
        if let Some(a) = self.best_for(part) {
            return Some(Choice::Sampled(a));
        }
        parts.part(part).map(|p| Choice::Synth(&p.synth))
    }
}
