//! The music's data, read from the data folder (on the game thread, never the audio thread):
//!
//! - `data/music/instruments/*.ron`: one instrument per file (instrument.rs)
//! - `data/music/moods/*.ron`: one mood per file (mood.rs)
//! - `data/music/genres/*.ron`: one genre per file, a community genre a dropped-in file (genre.rs)
//! - `data/music/places.ron`: which mood plays where and when (places.rs)
//! - `data/audio/hum_sources.ron`: where the ship hums from (bed.rs)
//! - `data/music/tones.ron`: the named tones of the optional tones layer
//! - `data/music/intensity.ron`: the intensity bands, the stinger and the riser (intensity.rs)
//!
//! Every file is read every time a score starts (no copy kept across runs: CLAUDE.md, the no-caches
//! rule), and [`MusicLibrary::check`] resolves every name, so a typo in a file is an error naming
//! the file, not a silent gap. The lib test `the_shipped_music_data_parses` (in
//! `just validate-data`) holds the shipped files to it.

use std::path::Path;

use crate::bed::HumSources;
use crate::instrument::{Instrument, SampleLibrary};
use crate::mood::Mood;
use crate::places::Places;
use crate::resolved::Resolved;
use crate::genre::Genre;

/// Every music data file.
#[derive(Clone, Debug)]
pub struct MusicLibrary {
    pub instruments: Vec<Instrument>,
    pub moods: Vec<Mood>,
    pub genres: Vec<Genre>,
    pub places: Places,
    pub hum: HumSources,
    /// The named tones Settings offers for the optional tones layer, hertz (data/music/tones.ron:
    /// the "solfeggio" set as data; any other frequency can be typed). Offered as a matter of taste.
    pub tones: Vec<f32>,
    /// How the game's intensity becomes danger and action music (data/music/intensity.ron).
    pub intensity: crate::intensity::IntensityMap,
}

/// data/music/tones.ron.
#[derive(Clone, Debug, serde::Deserialize)]
struct Tones {
    named_hz: Vec<f32>,
}

/// The genre a person hears unless they choose another.
pub const DEFAULT_GENRE: &str = "mothership_calm";
/// The mood where nothing else is asked for.
pub const DEFAULT_MOOD: &str = "mothership_calm";

/// A recorded instrument's definition (`kind: Sampler((...))`, written by `just instrument-packs`):
/// it shares data/music/instruments/ with the synthesised ones, but the instrument library loads it
/// (src/instrument_packs) and attaches it to the score by its part, so this loader leaves it alone.
pub fn is_recorded_definition(text: &str) -> bool {
    text.contains("Sampler((")
}

fn read_dir_ron<T: serde::de::DeserializeOwned>(dir: &Path) -> Result<Vec<(String, T)>, String> {
    let mut out = Vec::new();
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "ron"))
        .collect();
    paths.sort();
    for p in paths {
        let text = std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        if is_recorded_definition(&text) {
            continue;
        }
        let v = ron::from_str::<T>(&text).map_err(|e| format!("{}: {e}", p.display()))?;
        out.push((p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default(), v));
    }
    Ok(out)
}

impl MusicLibrary {
    /// Read every music file under `data_dir` (the app's "data" folder; tests pass the repo's).
    pub fn load(data_dir: &Path) -> Result<MusicLibrary, String> {
        let music = data_dir.join("music");
        let instruments: Vec<Instrument> = read_dir_ron::<Instrument>(&music.join("instruments"))?.into_iter().map(|(_, i)| i).collect();
        let moods: Vec<Mood> = read_dir_ron::<Mood>(&music.join("moods"))?.into_iter().map(|(_, m)| m).collect();
        let genres: Vec<Genre> = read_dir_ron::<Genre>(&music.join("genres"))?.into_iter().map(|(_, s)| s).collect();
        let places_path = music.join("places.ron");
        let places: Places = ron::from_str(&std::fs::read_to_string(&places_path).map_err(|e| format!("{}: {e}", places_path.display()))?)
            .map_err(|e| format!("{}: {e}", places_path.display()))?;
        let hum_path = data_dir.join("audio").join("hum_sources.ron");
        let hum: HumSources = ron::from_str(&std::fs::read_to_string(&hum_path).map_err(|e| format!("{}: {e}", hum_path.display()))?)
            .map_err(|e| format!("{}: {e}", hum_path.display()))?;
        let tones_path = music.join("tones.ron");
        let tones: Tones = ron::from_str(&std::fs::read_to_string(&tones_path).map_err(|e| format!("{}: {e}", tones_path.display()))?)
            .map_err(|e| format!("{}: {e}", tones_path.display()))?;
        let intensity_path = music.join("intensity.ron");
        let intensity: crate::intensity::IntensityMap = ron::from_str(&std::fs::read_to_string(&intensity_path).map_err(|e| format!("{}: {e}", intensity_path.display()))?)
            .map_err(|e| format!("{}: {e}", intensity_path.display()))?;
        let mut lib = MusicLibrary { instruments, moods, genres, places, hum, tones: tones.named_hz, intensity };
        lib.order();
        lib.check()?;
        Ok(lib)
    }

    /// Put the defaults first (index 0 is what a context of zeros plays) and the rest by id.
    fn order(&mut self) {
        self.moods.sort_by(|a, b| (a.id != DEFAULT_MOOD, &a.id).cmp(&(b.id != DEFAULT_MOOD, &b.id)));
        self.genres.sort_by(|a, b| (a.id != DEFAULT_GENRE, &a.id).cmp(&(b.id != DEFAULT_GENRE, &b.id)));
        self.instruments.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// Every name resolved and every id unique: the error names the file's id and the field.
    pub fn check(&self) -> Result<(), String> {
        fn unique<'a>(what: &str, ids: impl Iterator<Item = &'a str>) -> Result<(), String> {
            let mut seen = std::collections::HashSet::new();
            for id in ids {
                if id.is_empty() {
                    return Err(format!("a {what} has an empty id"));
                }
                if !seen.insert(id) {
                    return Err(format!("two {what}s have the id {id:?}"));
                }
            }
            Ok(())
        }
        unique("instrument", self.instruments.iter().map(|i| i.id.as_str()))?;
        unique("mood", self.moods.iter().map(|i| i.id.as_str()))?;
        unique("genre", self.genres.iter().map(|i| i.id.as_str()))?;
        if self.moods.is_empty() {
            return Err("data/music/moods has no mood".into());
        }
        if self.genres.is_empty() {
            return Err("data/music/genres has no genre".into());
        }
        for r in &self.places.rules {
            if self.mood_index(&r.mood).is_none() {
                return Err(format!("places.ron: mood {:?} is not a mood", r.mood));
            }
        }
        if self.mood_index(&self.places.default_mood).is_none() {
            return Err(format!("places.ron: default_mood {:?} is not a mood", self.places.default_mood));
        }
        for s in &self.places.genre_suggestions {
            if self.genre_index(&s.genre).is_none() {
                return Err(format!("places.ron: genre {:?} is not a genre", s.genre));
            }
        }
        for b in &self.intensity.bands {
            if let Some(m) = &b.mood {
                if self.mood_index(m).is_none() {
                    return Err(format!("intensity.ron: mood {m:?} is not a mood"));
                }
            }
        }
        if self.intensity.bands.windows(2).any(|w| w[1].from <= w[0].from) {
            return Err("intensity.ron: the bands' starts must rise".into());
        }
        // Resolving with no sample packs checks every instrument, note, chord and fallback name.
        Resolved::new(self, &SampleLibrary::new()).map(|_| ())
    }

    /// A mood's index by id.
    pub fn mood_index(&self, id: &str) -> Option<usize> {
        self.moods.iter().position(|m| m.id == id)
    }

    /// A genre's index by id.
    pub fn genre_index(&self, id: &str) -> Option<usize> {
        self.genres.iter().position(|s| s.id == id)
    }

    /// An instrument's index by id.
    pub fn instrument_index(&self, id: &str) -> Option<usize> {
        self.instruments.iter().position(|s| s.id == id)
    }
}
