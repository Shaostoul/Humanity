//! THE WRITTEN PIECES as data (docs/design/score-player.md section 14): `data/music/scores/index.ron`
//! lists them, each an MEI file beside it (crate::mei reads it, crate::written times and plays it).
//! Infinite-of-x: a new piece is a file and a line in the index, never code.
//!
//! The index says what the file cannot: the words a person reads for the piece (by key), the tempo
//! when the file marks none, which of our instruments plays each staff, the piece's level (measured,
//! so it sounds at the music's loudness target) and the rest between times through.
//!
//! Read on the game thread (the list, with the rest of the music's data) and on the music's build
//! thread (a piece's file, when it is chosen); never on the audio thread, and never kept between runs.

use std::path::Path;
use std::sync::Arc;

use crate::mei::MeiScore;
use crate::written::{StaffVoice, Timeline, WrittenPlayer};

/// One staff of a piece and what plays it.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct StaffSpec {
    /// The staff's number in the file (`staffDef@n`).
    pub staff: u16,
    /// An instrument's id in data/music/instruments (a recorded one falls back to its synthesised
    /// voice when its pack is not installed, as everywhere).
    pub instrument: String,
    /// The role it counts as on the Music page (its meter and colour): one of `tune::ROLE_IDS`.
    #[serde(default = "default_role")]
    pub role: String,
    #[serde(default)]
    pub level_db: f32,
    /// -1 (left) .. 1 (right), before the instrument's own spread.
    #[serde(default)]
    pub pan: f32,
    /// The part it is (an id in data/music/parts.ron: `piano`, `solo_violin`, `strings_low` ...):
    /// the best installed recording of that part plays the staff (score-player.md section 9, rung 2),
    /// `instrument` when none is installed. None: `instrument` always.
    #[serde(default)]
    pub part: Option<String>,
}

fn default_role() -> String {
    "melody".into()
}

fn default_rest() -> f32 {
    4.0
}

fn default_qpm() -> f64 {
    crate::written::DEFAULT_QPM
}

/// One written piece in the index.
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
pub struct ScoreEntry {
    /// Stable id: what the person's settings keep (`MusicSettings::written`).
    pub id: String,
    /// The MEI file, beside the index (a plain file name).
    pub file: String,
    /// The piece's title and its composer, by key in data/i18n/en.json (a community file may give
    /// plain words in `name` and `composer` instead).
    #[serde(default)]
    pub words: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub composer_words: String,
    #[serde(default)]
    pub composer: String,
    /// Quarter notes a minute before the file's first tempo mark.
    #[serde(default = "default_qpm")]
    pub qpm: f64,
    /// The piece's level, dB (measured: crate tests hold the shipped pieces to the music target).
    #[serde(default)]
    pub level_db: f32,
    /// Seconds of rest after each time through, before it plays again.
    #[serde(default = "default_rest")]
    pub rest_s: f32,
    pub staves: Vec<StaffSpec>,
    /// Where the file came from (docs/reference/findings/2026-11-20-public-domain-scores.md): the
    /// printed edition it follows, the encoding it was converted from (a site and its piece), the
    /// licence it is under, and the credit a person reads beside it on the Music page (a words key;
    /// empty: none).
    #[serde(default)]
    pub edition: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub licence: String,
    #[serde(default)]
    pub credit_words: String,
}

/// The index file (its RON names the struct `Scores`).
#[derive(serde::Deserialize)]
struct Scores {
    pieces: Vec<ScoreEntry>,
}

/// The index file's place under the data folder.
pub fn index_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("music").join("scores").join("index.ron")
}

/// Read the index (no index: no written pieces, not an error). Errors name the file and the piece.
pub fn load_index(data_dir: &Path) -> Result<Vec<ScoreEntry>, String> {
    let path = index_path(data_dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let index: Scores = ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut seen = std::collections::HashSet::new();
    for p in &index.pieces {
        let at = |what: String| format!("{}: piece {:?}: {what}", path.display(), p.id);
        if p.id.is_empty() || !seen.insert(p.id.clone()) {
            return Err(at("its id is empty or used twice".into()));
        }
        if p.file.is_empty() || p.file.contains(['/', '\\']) || p.file.contains("..") || !p.file.ends_with(".mei") {
            return Err(at(format!("its file {:?} must be a plain .mei file name beside the index", p.file)));
        }
        if p.staves.is_empty() {
            return Err(at("it names no staff to play".into()));
        }
        for s in &p.staves {
            if !crate::tune::ROLE_IDS.contains(&s.role.as_str()) {
                return Err(at(format!("staff {}'s role {:?} is not one of {:?}", s.staff, s.role, crate::tune::ROLE_IDS)));
            }
        }
        if !(5.0..=1000.0).contains(&p.qpm) || !(0.0..=60.0).contains(&p.rest_s) {
            return Err(at("its qpm must be 5 to 1000 and its rest 0 to 60 seconds".into()));
        }
    }
    Ok(index.pieces)
}

/// A piece read and timed, ready to play.
#[derive(Clone, Debug)]
pub struct WrittenPiece {
    pub entry: ScoreEntry,
    pub score: MeiScore,
    pub timeline: Arc<Timeline>,
}

/// Read and time one piece of the index.
pub fn load_piece(data_dir: &Path, entry: &ScoreEntry) -> Result<WrittenPiece, String> {
    let path = index_path(data_dir).with_file_name(&entry.file);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let score = crate::mei::read(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if score.notes.is_empty() {
        return Err(format!("{}: no notes to play", path.display()));
    }
    let timeline = Arc::new(Timeline::from_mei(&score, entry.qpm));
    Ok(WrittenPiece { entry: entry.clone(), score, timeline })
}

impl WrittenPiece {
    /// Each staff the index names and its role on the Music page (an index into `tune::ROLE_IDS`; 255:
    /// none): what the piano roll colours its notes by.
    pub fn staff_roles(&self) -> Vec<(u16, u8)> {
        self.entry.staves.iter().map(|s| (s.staff, crate::tune::ROLE_IDS.iter().position(|r| *r == s.role).map(|i| i as u8).unwrap_or(255))).collect()
    }

    /// Its player over the resolved library `res` at `rate` Hz and A4 = `a4`: each staff the index
    /// names on its instrument (an id the library lacks plays the first instrument: the library's
    /// check refuses such an index, so only a hand-made test reaches it).
    pub fn player(&self, res: &crate::Resolved, rate: f32, a4: f32) -> WrittenPlayer {
        let voices = self
            .entry
            .staves
            .iter()
            .map(|s| StaffVoice {
                staff: s.staff,
                inst: res.instrument_index(&s.instrument).unwrap_or(0),
                role: crate::tune::ROLE_IDS.iter().position(|r| *r == s.role).map(|i| i as u8).unwrap_or(255),
                level: crate::dsp::db_to_amp(s.level_db),
                pan: s.pan.clamp(-1.0, 1.0),
            })
            .collect();
        WrittenPlayer::new(self.timeline.clone(), voices, rate, a4, self.entry.rest_s, self.entry.level_db)
    }
}

/// The parts a written piece's staves name (score-player.md section 9, rung 2), each once with the
/// staff's own synthesised instrument (what plays it with no recording installed) and the middle of
/// the notes it plays (MIDI; middle C for a staff with none): what the instrument library attaches
/// recordings for while the piece plays, beside the parts the genres name.
pub fn written_parts(entry: &ScoreEntry, score: &MeiScore) -> Vec<(String, String, i32)> {
    let mut out: Vec<(String, String, i32)> = Vec::new();
    for s in &entry.staves {
        let Some(part) = &s.part else { continue };
        if out.iter().any(|(p, _, _)| p == part) {
            continue;
        }
        let (lo, hi) = score.notes.iter().filter(|n| n.staff == s.staff).fold((f32::MAX, f32::MIN), |(lo, hi), n| (lo.min(n.midi), hi.max(n.midi)));
        let mid = if lo <= hi { ((lo + hi) * 0.5).round() as i32 } else { 60 };
        out.push((part.clone(), s.instrument.clone(), mid));
    }
    out
}

/// Point each staff of `entry` that names a part with a recording attached (`recorded`: the part
/// and the score's instrument id that plays it) at that recording.
pub fn point_staves(entry: &mut ScoreEntry, recorded: &[(String, String)]) {
    for s in &mut entry.staves {
        if let Some(id) = s.part.as_ref().and_then(|p| recorded.iter().find(|(q, _)| q == p)).map(|(_, id)| id.clone()) {
            s.instrument = id;
        }
    }
}
