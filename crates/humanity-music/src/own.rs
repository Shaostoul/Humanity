//! THE PERSON'S OWN WRITTEN PIECES (docs/design/music-page.md section 15a (b), rung 2): a MIDI file
//! they open becomes a written piece they own, kept in their data folder (`music/scores/` beside
//! their favourites: an MEI file and a line in their own `index.ron`, the same shape as the shipped
//! index), listed in the Written music catalogue beside the shipped pieces, with its details theirs to
//! change. It is the person's own data, never a cache: nothing here is made again from anything else.
//!
//! The import reads the MIDI file through midly (crate::smf), notates it as MEI (crate::mei_write) and
//! reads that back with the crate's own reader before keeping it, so a piece that is kept is one the
//! player plays. What it cannot read it refuses, in words. Until sound styles arrive (rung 3: the
//! General-MIDI-like table), every part plays on the piano part and a drum track on one drum.

use std::path::{Path, PathBuf};

use crate::scores::{ScoreEntry, StaffSpec};

/// The person's own index file in their scores folder.
pub fn index_path(dir: &Path) -> PathBuf {
    dir.join("index.ron")
}

/// The person's own pieces in `dir` (no index yet: none), each marked with the folder its file sits in.
pub fn load(dir: &Path) -> Result<Vec<ScoreEntry>, String> {
    let path = index_path(dir);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("{}: {e}", path.display())),
    };
    let mut pieces = crate::scores::parse_index(&text, &path)?;
    for p in &mut pieces {
        p.folder = Some(dir.to_path_buf());
    }
    Ok(pieces)
}

/// Write the person's own index (after an import, an edit or a removal).
pub fn save(dir: &Path, pieces: &[ScoreEntry]) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let text = crate::scores::index_text(pieces)?;
    let path = index_path(dir);
    let tmp = path.with_extension("ron.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))
}

/// A MIDI file read and notated, not yet kept.
#[derive(Clone, Debug)]
pub struct Imported {
    pub entry: ScoreEntry,
    pub mei: String,
    /// How many notes and parts it holds (the page says so).
    pub notes: usize,
    pub parts: usize,
}

/// A safe id from words: lower-case letters and digits joined by underscores.
fn slug(words: &str) -> String {
    let mut s = String::new();
    for c in words.chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c.to_ascii_lowercase());
        } else if !s.ends_with('_') && !s.is_empty() {
            s.push('_');
        }
    }
    let s = s.trim_end_matches('_');
    let s: String = s.chars().take(40).collect();
    if s.is_empty() {
        "piece".into()
    } else {
        s
    }
}

/// Read `bytes` (a MIDI file the person chose, named `file_name`) as a piece of their own: its title
/// (the file's own, else its name), its parts with their roles (the highest the melody, the lowest the
/// bass, a drum track the percussion), its tempo and length, an id none of `taken` uses. Refused in
/// words: a file that is not MIDI, one timed in film frames, one with no notes, one whose notes the
/// MEI reader then cannot read back.
pub fn import_midi(bytes: &[u8], file_name: &str, taken: &[String]) -> Result<Imported, String> {
    let mut n = crate::smf::read_smf(bytes)?;
    let stem = Path::new(file_name).file_stem().and_then(|s| s.to_str()).unwrap_or("piece").trim().to_string();
    if n.title.trim().is_empty() {
        n.title = stem.clone();
    }
    // The roles: by each part's middle pitch, the drums apart.
    let avg = |p: &crate::smf::NotatedPart| p.notes.iter().map(|k| k.key as f64).sum::<f64>() / p.notes.len().max(1) as f64;
    let mut order: Vec<usize> = (0..n.parts.len()).filter(|&i| !n.parts[i].drums).collect();
    order.sort_by(|&a, &b| avg(&n.parts[b]).total_cmp(&avg(&n.parts[a])));
    let mut roles = vec!["texture"; n.parts.len()];
    for (k, &i) in order.iter().enumerate() {
        roles[i] = if k == 0 {
            "melody"
        } else if k + 1 == order.len() {
            "bass"
        } else {
            "texture"
        };
    }
    for (i, p) in n.parts.iter_mut().enumerate() {
        if p.drums {
            roles[i] = "percussion";
        }
        if p.name.is_empty() {
            p.name = format!("{} {}", if p.drums { "Drums" } else { "Part" }, i + 1);
        }
    }
    let availability = format!("Imported into HumanityOS from the MIDI file {file_name:?}. Its rights are those of the file it came from: keep and share it only as that file allows.");
    let mei = crate::mei_write::write_mei(&n, &availability);
    let score = crate::mei::read(&mei).map_err(|e| format!("its notes could not be written down as music the player reads ({e})"))?;
    if score.notes.is_empty() {
        return Err("none of its notes could be written down as music the player reads".into());
    }
    let qpm = n.tempos.first().map(|t| t.1).unwrap_or(120.0).clamp(5.0, 1000.0);
    let timeline = crate::written::Timeline::from_mei(&score, qpm);
    let base = format!("own_{}", slug(&n.title));
    let mut id = base.clone();
    let mut k = 2;
    while taken.iter().any(|t| *t == id) {
        id = format!("{base}_{k}");
        k += 1;
    }
    let count = n.parts.len().max(1) as f32;
    let staves = n
        .parts
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let pan = if count > 1.0 { -0.3 + 0.6 * i as f32 / (count - 1.0) } else { 0.0 };
            if p.drums {
                StaffSpec { staff: i as u16 + 1, instrument: "deep_drum".into(), role: roles[i].into(), level_db: -6.0, pan, part: None }
            } else {
                StaffSpec { staff: i as u16 + 1, instrument: "electric_piano".into(), role: roles[i].into(), level_db: 0.0, pan, part: Some("piano".into()) }
            }
        })
        .collect();
    let key = n.key.map(|(sf, minor)| key_words(sf, minor)).unwrap_or_default();
    let entry = ScoreEntry {
        id: id.clone(),
        file: format!("{id}.mei"),
        words: String::new(),
        name: n.title.clone(),
        composer_words: String::new(),
        composer: n.composer.clone(),
        qpm,
        level_db: -3.0,
        rest_s: 4.0,
        staves,
        edition: String::new(),
        source: file_name.to_string(),
        licence: String::new(),
        credit_words: String::new(),
        era_words: String::new(),
        era: String::new(),
        year: 0,
        composer_years: String::new(),
        key_root: key.0,
        key_mode: key.1,
        length_s: timeline.length_s as f32,
        source_url: String::new(),
        typesetter: String::new(),
        tempo_words: String::new(),
        tempo_finding: String::new(),
        folder: None,
    };
    Ok(Imported { entry, mei, notes: score.notes.len(), parts: n.parts.len() })
}

/// A key signature as an index entry's `key_root` and `key_mode` ("E flat", "major").
fn key_words(fifths: i8, minor: bool) -> (String, String) {
    const MAJOR: [&str; 15] = ["C flat", "G flat", "D flat", "A flat", "E flat", "B flat", "F", "C", "G", "D", "A", "E", "B", "F sharp", "C sharp"];
    const MINOR: [&str; 15] = ["A flat", "E flat", "B flat", "F", "C", "G", "D", "A", "E", "B", "F sharp", "C sharp", "G sharp", "D sharp", "A sharp"];
    let i = (fifths.clamp(-7, 7) + 7) as usize;
    if minor {
        (MINOR[i].into(), "minor".into())
    } else {
        (MAJOR[i].into(), "major".into())
    }
}

/// Keep an imported piece in `dir`: its MEI file, then its line in the person's index (`pieces`, the
/// list as it stands, which gains it). The entry kept, marked with its folder.
pub fn keep(dir: &Path, pieces: &mut Vec<ScoreEntry>, imported: Imported) -> Result<ScoreEntry, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = dir.join(&imported.entry.file);
    std::fs::write(&file, &imported.mei).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut entry = imported.entry;
    entry.folder = Some(dir.to_path_buf());
    pieces.retain(|p| p.id != entry.id);
    pieces.push(entry.clone());
    save(dir, pieces)?;
    Ok(entry)
}

/// Remove a piece of the person's own: its line in the index and its file.
pub fn remove(dir: &Path, pieces: &mut Vec<ScoreEntry>, id: &str) -> Result<(), String> {
    let Some(at) = pieces.iter().position(|p| p.id == id) else { return Ok(()) };
    let p = pieces.remove(at);
    save(dir, pieces)?;
    let file = dir.join(&p.file);
    match std::fs::remove_file(&file) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{}: {e}", file.display())),
    }
}
