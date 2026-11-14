//! FAVOURITES (docs/design/music-page.md section 7): the pieces a person keeps. The operator,
//! 2026-11-13: "find the exact seeds we want using the genre we want and all that and set some as
//! permanent. Like maybe seed X has a particular sound I love and want to combat or just to listen to
//! in my real-life car."
//!
//! A favourite is a name, the genre, the mood, the seed and the tuning, saved by IDS (a genre's id, a
//! mood's id, a role's id, a lead voice's part or instrument id), never by list positions, so it
//! plays the same after a data file gains an entry. The file (`music/favourites.json` in the
//! person's data folder, beside config.json) is the person's own data, like a save: never a cache,
//! never sent anywhere. It is written to a temporary file and renamed, so a crash never tears it, and
//! a file that no longer reads is moved aside rather than written over.
//!
//! A favourite also travels as a SHARE CODE: "HOSM1-" and Crockford base32 of its seed, its tuning,
//! its genre and mood ids and its name, with a checksum, so a mistyped code is refused rather than
//! playing something else.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::library::MusicLibrary;
use crate::tune::{Tune, PIECE, ROLES, ROLE_IDS};

/// A favourite as the file keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Favourite {
    /// Its own number in the file (assignments name it, so a rename keeps them).
    pub id: u64,
    pub name: String,
    pub genre: String,
    pub mood: String,
    pub seed: u64,
    #[serde(default)]
    pub tune: SavedTune,
}

/// A tuning as the file keeps it: by ids, and only what was changed.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SavedTune {
    #[serde(default, skip_serializing_if = "is_zero_i8")]
    pub transpose: i8,
    /// Beats a minute (absent: the piece's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tempo_bpm: Option<f32>,
    /// 0.5 straight to 0.75 heavy (absent: the genre's own).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swing: Option<f32>,
    /// Role id to lead voice id (the entry's part, else its instrument).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub leads: BTreeMap<String, String>,
    /// Role id to its level, dB (only those not at 0).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub levels_db: BTreeMap<String, f32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub muted: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub soloed: Vec<String>,
}

fn is_zero_i8(x: &i8) -> bool {
    *x == 0
}

/// The person's favourites and what they are for.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Favourites {
    #[serde(default)]
    pub favourites: Vec<Favourite>,
    /// "My music": a mood's id to the favourite that plays in it (its `id`).
    #[serde(default)]
    pub for_mood: BTreeMap<String, u64>,
}

/// A lead entry's voice id: its part, else its instrument.
fn lead_id(r: &crate::mood::RoleSpec) -> &str {
    r.part.as_deref().unwrap_or(&r.instrument)
}

impl SavedTune {
    /// The tuning as the score takes it, in the genre `genre` of `lib` (a lead voice the genre no
    /// longer has is the piece's own again).
    pub fn to_tune(&self, lib: &MusicLibrary, genre: usize) -> Tune {
        let mut t = Tune { transpose: self.transpose.clamp(-6, 6), tempo_bpm: self.tempo_bpm.unwrap_or(0.0).max(0.0), swing: self.swing.unwrap_or(-1.0), ..Tune::default() };
        let role = |id: &str| ROLE_IDS.iter().position(|r| *r == id);
        if let Some(g) = lib.genres.get(genre) {
            for (r, voice) in &self.leads {
                if let Some(k) = role(r) {
                    if let Some(i) = g.leads.iter().position(|e| e.role.index() == k && lead_id(e) == voice) {
                        t.lead[k] = i as u16;
                    }
                }
            }
        }
        for (r, db) in &self.levels_db {
            if let Some(k) = role(r) {
                t.level_db[k] = db.clamp(-60.0, 12.0);
            }
        }
        for r in &self.muted {
            if let Some(k) = role(r) {
                t.mute |= 1 << k;
            }
        }
        for r in &self.soloed {
            if let Some(k) = role(r) {
                t.solo |= 1 << k;
            }
        }
        t
    }

    /// The tuning `t` (in the genre `genre` of `lib`) as the file keeps it.
    pub fn from_tune(t: &Tune, lib: &MusicLibrary, genre: usize) -> SavedTune {
        let mut s = SavedTune { transpose: t.transpose, tempo_bpm: (t.tempo_bpm > 0.0).then_some(t.tempo_bpm), swing: (t.swing >= 0.0).then_some(t.swing), ..Default::default() };
        let g = lib.genres.get(genre);
        for k in 0..ROLES {
            if t.lead[k] != PIECE {
                if let Some(e) = g.and_then(|g| g.leads.get(t.lead[k] as usize)) {
                    s.leads.insert(ROLE_IDS[k].to_string(), lead_id(e).to_string());
                }
            }
            if t.level_db[k] != 0.0 {
                s.levels_db.insert(ROLE_IDS[k].to_string(), t.level_db[k]);
            }
            if t.mute & (1 << k) != 0 {
                s.muted.push(ROLE_IDS[k].to_string());
            }
            if t.solo & (1 << k) != 0 {
                s.soloed.push(ROLE_IDS[k].to_string());
            }
        }
        s
    }
}

/// The intensity a mood is heard at when it is chosen on the Music page: a mood an intensity band
/// names (tension, action) a little into that band, so its stinger, riser and danger layers play as
/// in the game; any other mood the calm default (0.3).
pub fn explore_intensity(lib: &MusicLibrary, mood: &str) -> f32 {
    lib.intensity.bands.iter().find(|b| b.mood.as_deref() == Some(mood)).map(|b| (b.from + 0.1).min(1.0)).unwrap_or(0.3)
}

impl Favourite {
    /// The score's context for this favourite as it was kept (its genre, its mood heard as the
    /// Music page plays it, its tuning) at A4 = `a4` Hz. Err: a genre or mood this data lacks (a
    /// community genre removed, a code from a friend with one more genre).
    pub fn context(&self, lib: &MusicLibrary, a4: f32) -> Result<crate::MusicContext, String> {
        let genre = lib.genre_index(&self.genre).ok_or_else(|| format!("the genre {:?} is not installed", self.genre))?;
        let mood = lib.mood_index(&self.mood).ok_or_else(|| format!("the mood {:?} is not in this music's data", self.mood))?;
        Ok(crate::MusicContext {
            gain: 1.0,
            a4_hz: a4,
            mood: mood as u16,
            genre_a: genre as u16,
            genre_b: genre as u16,
            blend: 0.0,
            intensity: explore_intensity(lib, &self.mood),
            duck_db: 0.0,
            tune: self.tune.to_tune(lib, genre),
            analyse: false,
        })
    }
}

impl Favourites {
    /// Read the favourites at `path` (no file: none yet).
    pub fn load(path: &Path) -> Result<Favourites, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{} does not read: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Favourites::default()),
            Err(e) => Err(format!("{} could not be opened: {e}", path.display())),
        }
    }

    /// Write the favourites to `path`: to a temporary file beside it, then renamed over it. A file
    /// there that no longer reads (mended by hand and broken) is moved aside to
    /// `<name>.unreadable.json` first, never written over.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{} could not be made: {e}", dir.display()))?;
        }
        if path.exists() && Favourites::load(path).is_err() {
            let aside = path.with_extension("unreadable.json");
            std::fs::rename(path, &aside).map_err(|e| format!("{} could not be moved aside: {e}", path.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text).map_err(|e| format!("{} could not be written: {e}", tmp.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("{} could not be written: {e}", path.display()))
    }

    /// Keep `fav` as a new favourite (its id: one past the highest), returning the id.
    pub fn add(&mut self, mut fav: Favourite) -> u64 {
        let id = self.favourites.iter().map(|f| f.id).max().unwrap_or(0) + 1;
        fav.id = id;
        self.favourites.push(fav);
        id
    }

    /// Remove favourite `id` and any mood it was the music for.
    pub fn remove(&mut self, id: u64) {
        self.favourites.retain(|f| f.id != id);
        self.for_mood.retain(|_, v| *v != id);
    }

    /// The favourite `id`.
    pub fn get(&self, id: u64) -> Option<&Favourite> {
        self.favourites.iter().find(|f| f.id == id)
    }

    /// The favourite that plays in mood `mood` (its id), if one is assigned and still kept.
    pub fn for_mood(&self, mood: &str) -> Option<&Favourite> {
        self.for_mood.get(mood).and_then(|id| self.get(*id))
    }

    /// Make favourite `id` the music for `mood` (None: the mood plays new pieces again).
    pub fn assign(&mut self, mood: &str, id: Option<u64>) {
        match id {
            Some(id) => {
                self.for_mood.insert(mood.to_string(), id);
            }
            None => {
                self.for_mood.remove(mood);
            }
        }
    }
}

// ---- share codes ----

/// What every share code starts with: the app and the code's version.
pub const SHARE_PREFIX: &str = "HOSM1-";
/// The longest name a share code carries (bytes, cut at a character boundary).
pub const SHARE_NAME_MAX: usize = 24;

const B32: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

fn b32_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((acc >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(B32[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

fn b32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.chars() {
        let c = c.to_ascii_uppercase();
        if c == '-' || c.is_whitespace() {
            continue;
        }
        // Crockford's reading of the letters people confuse: I and L are one, O is zero.
        let c = match c {
            'I' | 'L' => '1',
            'O' => '0',
            c => c,
        };
        let v = B32.iter().position(|&x| x as char == c)? as u32;
        acc = (acc << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

fn check16(bytes: &[u8]) -> u16 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h ^ (h >> 16)) as u16
}

fn push_str(out: &mut Vec<u8>, s: &str, max: usize) {
    let mut end = s.len().min(max).min(255);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    out.push(end as u8);
    out.extend_from_slice(&s.as_bytes()[..end]);
}

/// A favourite as a share code (its id stays behind: the friend's file numbers it).
pub fn share_code(f: &Favourite) -> String {
    let t = &f.tune;
    let mut b = Vec::with_capacity(64);
    // The seed as a variable-length number (seven bits a byte): the page's seeds are typed or drawn
    // under a million, three bytes instead of eight.
    let mut s = f.seed;
    loop {
        let byte = (s & 0x7f) as u8;
        s >>= 7;
        if s == 0 {
            b.push(byte);
            break;
        }
        b.push(byte | 0x80);
    }
    b.push(t.transpose as u8);
    b.extend_from_slice(&((t.tempo_bpm.unwrap_or(0.0).clamp(0.0, 6000.0) * 10.0).round() as u16).to_le_bytes());
    b.push(t.swing.map(|s| (s.clamp(0.5, 0.75) * 200.0).round() as u8).unwrap_or(0));
    let bits = |list: &[String]| list.iter().filter_map(|r| ROLE_IDS.iter().position(|x| x == r)).fold(0u8, |a, k| a | 1 << k);
    b.push(bits(&t.muted));
    b.push(bits(&t.soloed));
    let levels: Vec<(usize, f32)> = t.levels_db.iter().filter_map(|(r, db)| ROLE_IDS.iter().position(|x| x == r).map(|k| (k, *db))).collect();
    b.push(levels.len() as u8);
    for (k, db) in levels {
        b.push(k as u8);
        b.push(((db.clamp(-60.0, 12.0)) * 2.0).round() as i8 as u8);
    }
    let leads: Vec<(usize, &String)> = t.leads.iter().filter_map(|(r, v)| ROLE_IDS.iter().position(|x| x == r).map(|k| (k, v))).collect();
    b.push(leads.len() as u8);
    for (k, v) in leads {
        b.push(k as u8);
        push_str(&mut b, v, 64);
    }
    push_str(&mut b, &f.genre, 64);
    push_str(&mut b, &f.mood, 64);
    push_str(&mut b, &f.name, SHARE_NAME_MAX);
    let c = check16(&b);
    b.extend_from_slice(&c.to_le_bytes());
    format!("{SHARE_PREFIX}{}", b32_encode(&b))
}

/// Why a pasted code was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShareError {
    /// It does not start with the prefix (another app's code, or not a code at all).
    NotACode,
    /// A letter that is not in the code's alphabet, or the code ends too soon.
    Garbled,
    /// The checksum does not match: a letter was mistyped or lost.
    Mistyped,
}

struct Reader<'a> {
    b: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let s = self.b.get(self.i..self.i + n)?;
        self.i += n;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn string(&mut self) -> Option<String> {
        let n = self.u8()? as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
}

/// Read a pasted share code (spaces, dashes and case ignored).
pub fn from_share_code(code: &str) -> Result<Favourite, ShareError> {
    let code = code.trim();
    let body = code.get(..SHARE_PREFIX.len()).filter(|p| p.eq_ignore_ascii_case(SHARE_PREFIX)).map(|_| &code[SHARE_PREFIX.len()..]).ok_or(ShareError::NotACode)?;
    let bytes = b32_decode(body).ok_or(ShareError::Garbled)?;
    if bytes.len() < 2 + 10 {
        return Err(ShareError::Garbled);
    }
    let (data, sum) = bytes.split_at(bytes.len() - 2);
    if check16(data) != u16::from_le_bytes([sum[0], sum[1]]) {
        return Err(ShareError::Mistyped);
    }
    let mut r = Reader { b: data, i: 0 };
    let read = |r: &mut Reader| -> Option<Favourite> {
        let mut seed = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = r.u8()?;
            seed |= ((byte & 0x7f) as u64) << shift;
            if byte & 0x80 == 0 {
                break;
            }
        }
        let mut t = SavedTune { transpose: r.u8()? as i8, ..Default::default() };
        let tempo = u16::from_le_bytes(r.take(2)?.try_into().ok()?);
        t.tempo_bpm = (tempo > 0).then(|| tempo as f32 / 10.0);
        let swing = r.u8()?;
        t.swing = (swing > 0).then(|| swing as f32 / 200.0);
        let roles = |bits: u8| (0..ROLES).filter(|k| bits & (1 << k) != 0).map(|k| ROLE_IDS[k].to_string()).collect::<Vec<_>>();
        t.muted = roles(r.u8()?);
        t.soloed = roles(r.u8()?);
        for _ in 0..r.u8()? {
            let k = r.u8()? as usize;
            let db = r.u8()? as i8 as f32 / 2.0;
            t.levels_db.insert(ROLE_IDS.get(k)?.to_string(), db);
        }
        for _ in 0..r.u8()? {
            let k = r.u8()? as usize;
            let v = r.string()?;
            t.leads.insert(ROLE_IDS.get(k)?.to_string(), v);
        }
        let genre = r.string()?;
        let mood = r.string()?;
        let name = r.string()?;
        Some(Favourite { id: 0, name, genre, mood, seed, tune: t })
    };
    read(&mut r).ok_or(ShareError::Garbled)
}
