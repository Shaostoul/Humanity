//! THE PIECES HEARD, kept across runs (docs/design/music-page.md section 9, rung 2; the operator,
//! 2026-11-20, on keeping them across restarts: "Yes", in his data folder, with a Clear button).
//!
//! The last 50 pieces that played (a composed piece's genre, mood, seed and tuning, or a written
//! piece's id), newest first, with when each began. It is the person's own listening, so it is a data
//! file of theirs (`music/heard.json` beside their favourites and config.json), never a cache: nothing
//! here is made again from anything else, and Clear empties it. Written only when the list changes (a
//! new piece begins, or Clear), and read once when the music first runs.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use humanity_music::favourite::SavedTune;
use humanity_music::MusicSettings;
use serde::{Deserialize, Serialize};

use crate::gui::pages::dev_music::MusicReadout;

/// The pieces heard kept at most (docs/design/music-page.md section 9).
pub const HEARD_MOST: usize = 50;

/// One piece heard: a composed piece's genre, mood, seed and tuning (as the favourites keep it), or a
/// written piece's id, and when it began (seconds since 1970, UTC).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Heard {
    pub genre: String,
    pub mood: String,
    #[serde(default)]
    pub seed: u64,
    #[serde(default)]
    pub tune: SavedTune,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub written: Option<String>,
    pub at: u64,
}

/// The file: its list, newest first.
#[derive(Default, Serialize, Deserialize)]
struct HeardFile {
    #[serde(default)]
    heard: Vec<Heard>,
}

/// Now, in seconds since 1970.
pub fn now_s() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// The person's pieces-heard file: `music/heard.json` beside their favourites.
pub fn heard_path() -> PathBuf {
    super::favourites_path().with_file_name("heard.json")
}

/// The same piece (a level, a mute or a solo changed is still the piece heard).
fn same_piece(a: &Heard, b: &Heard) -> bool {
    let plain = |x: &SavedTune| SavedTune { levels_db: Default::default(), muted: Vec::new(), soloed: Vec::new(), ..x.clone() };
    a.written == b.written && (a.written.is_some() || (a.genre == b.genre && a.seed == b.seed && plain(&a.tune) == plain(&b.tune)))
}

/// Note what plays now at the top of the pieces heard (once per piece; a piece heard again moves to the
/// top). True when the list changed (it is then written).
pub fn note_heard(heard: &mut Vec<Heard>, s: &MusicSettings, r: &MusicReadout, now: u64) -> bool {
    let entry = if let Some(id) = &r.written {
        Heard { genre: s.genre.clone(), mood: r.mood.clone(), seed: 0, tune: SavedTune::default(), written: Some(id.clone()), at: now }
    } else if let Some(p) = r.playing.as_ref().filter(|p| p.seed > 0) {
        let tune = p.choice.as_ref().map(|c| c.tune).unwrap_or(p.ctx.tune);
        let genre = super::playing_genre(p, &s.genre);
        let saved = p.lib.genre_index(&genre).map(|g| SavedTune::from_tune(&tune, &p.lib, g)).unwrap_or_default();
        Heard { genre, mood: r.mood.clone(), seed: p.seed, tune: saved, written: None, at: now }
    } else {
        return false;
    };
    if heard.first().is_some_and(|h| same_piece(h, &entry)) {
        return false;
    }
    heard.retain(|h| !same_piece(h, &entry));
    heard.insert(0, entry);
    heard.truncate(HEARD_MOST);
    true
}

/// Read the pieces heard (no file yet: none). An unreadable file is set aside (renamed with
/// `.unreadable`) so the next write does not lose it silently, and said in words.
pub fn load(path: &Path) -> Result<Vec<Heard>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    match serde_json::from_str::<HeardFile>(&text) {
        Ok(f) => {
            let mut list = f.heard;
            list.truncate(HEARD_MOST);
            Ok(list)
        }
        Err(e) => {
            let aside = path.with_extension("json.unreadable");
            let _ = std::fs::rename(path, &aside);
            Err(format!("{} could not be read ({e}); it was set aside as {}", path.display(), aside.display()))
        }
    }
}

/// Write the pieces heard (through a temporary file, so a crash mid-write never leaves half a file).
pub fn save(path: &Path, heard: &[Heard]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(&HeardFile { heard: heard.to_vec() }).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}
