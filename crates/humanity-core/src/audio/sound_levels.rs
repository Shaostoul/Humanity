//! The person's own level for each sound the game ships: Settings > Audio > "Sound levels"
//! (2026-11-06; docs/design/loudness.md, "Sound levels").
//!
//! The operator, 2026-11-06, told that "Even out loudness" made the button click about 9 dB louder
//! and that fixing it is "a one-number change in the sound data": "Can we edit them in the settings
//! menu?" So every shipped sound gets a level of its own, in dB, that the person sets in Settings:
//!
//! - **0 dB is the sound as the app plays it now**: with "Even out loudness" on, at the loudness of
//!   its kind (`levels`) times its authored volume from `data/sounds.toml`; with it off, its file as
//!   recorded times that volume. A level moves the sound up or down from there, by exactly its dB,
//!   and the person's Master and bus sliders still apply on top.
//! - **The shipped data is never written.** `data/sounds.toml` stays the shipped default; the
//!   person's levels are overrides kept in their config (`AppConfig::sound_levels`), by the sound's
//!   catalogue id (`sfx.button_click`), so a sound that points at a new file keeps its level.
//! - **A sound with no level plays at 0 dB.** A reset takes the sound's entry out rather than
//!   keeping a 0, so the config holds only what the person changed.
//!
//! Held here, process-wide, the way `start_volume` holds the video start volume: the config gives
//! the levels at load and reads them at save, Settings edits them, and `AudioManager` asks
//! [`gain`] for each sound as it starts (one lock and one map lookup per sound started; nothing on
//! the audio thread). Not a cache: this IS the person's setting, the one copy of it while the app
//! runs.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::RwLock;

/// The lowest level a sound can be set to, dB: 24 dB under is about a sixteenth of its loudness,
/// close to silent beside everything else (the sound's own bus slider silences it outright).
pub const MIN_DB: f32 = -24.0;
/// The highest, dB: 12 dB up is the same limit "Even out loudness" puts on its own raise
/// (`max_boost_db` in data/audio/loudness.toml); past it a quiet file's hiss comes up with it.
pub const MAX_DB: f32 = 12.0;
/// Levels are kept to this step, dB: half the 1 dB difference a person can just hear, so a level
/// reads back as the number Settings showed.
pub const STEP_DB: f32 = 0.5;

static LEVELS: RwLock<BTreeMap<String, f32>> = RwLock::new(BTreeMap::new());

thread_local! {
    /// A test's own levels, on its own thread only, so tests running side by side do not see each
    /// other's ([`use_on_this_thread`]).
    static THIS_THREAD: RefCell<Option<BTreeMap<String, f32>>> = const { RefCell::new(None) };
}

fn read<R>(f: impl FnOnce(&BTreeMap<String, f32>) -> R) -> R {
    let mut f = Some(f);
    if let Some(r) = THIS_THREAD.with(|c| c.borrow().as_ref().map(|m| (f.take().unwrap())(m))) {
        return r;
    }
    let levels = LEVELS.read().unwrap_or_else(|e| e.into_inner());
    (f.take().unwrap())(&levels)
}

fn write<R>(f: impl FnOnce(&mut BTreeMap<String, f32>) -> R) -> R {
    let mut f = Some(f);
    if let Some(r) = THIS_THREAD.with(|c| c.borrow_mut().as_mut().map(|m| (f.take().unwrap())(m))) {
        return r;
    }
    let mut levels = LEVELS.write().unwrap_or_else(|e| e.into_inner());
    (f.take().unwrap())(&mut levels)
}

/// A level as it is kept: inside [`MIN_DB`]..=[`MAX_DB`], on the [`STEP_DB`] grid. None for a
/// number that is not one (NaN, infinite): never kept.
pub fn kept_db(db: f32) -> Option<f32> {
    if !db.is_finite() {
        return None;
    }
    let stepped = (db / STEP_DB).round() * STEP_DB;
    // `+ 0.0` turns a -0.0 (a small negative rounded up) into 0.0.
    Some(stepped.clamp(MIN_DB, MAX_DB) + 0.0)
}

/// A level in dB as the amplitude it multiplies a sound by: 10^(dB / 20) (0 dB is 1.0, -6 dB about
/// half, +12 dB about four times).
pub fn db_to_gain(db: f32) -> f64 {
    10f64.powf(db as f64 / 20.0)
}

/// The person's level for the sound `id`, dB (0 for a sound they have not set).
pub fn level_db(id: &str) -> f32 {
    read(|m| m.get(id).copied().unwrap_or(0.0))
}

/// The amplitude the person's level multiplies the sound `id` by ([`db_to_gain`] of
/// [`level_db`]; 1.0 for a sound they have not set). `AudioManager` asks it as each sound starts.
pub fn gain(id: &str) -> f64 {
    db_to_gain(level_db(id))
}

/// Set the sound `id` to `db` (kept as [`kept_db`] keeps it; 0 takes its entry out). True when
/// the level changed (the caller saves the settings).
pub fn set_level(id: &str, db: f32) -> bool {
    let Some(db) = kept_db(db) else { return false };
    write(|m| {
        let before = m.get(id).copied().unwrap_or(0.0);
        if db == 0.0 {
            m.remove(id);
        } else {
            m.insert(id.to_string(), db);
        }
        before != db
    })
}

/// Put the sound `id` back to 0 dB, as the app plays it without a level. True when it had one.
pub fn reset(id: &str) -> bool {
    write(|m| m.remove(id).is_some())
}

/// Put every sound back to 0 dB. True when any had a level.
pub fn reset_all() -> bool {
    write(|m| {
        let any = !m.is_empty();
        m.clear();
        any
    })
}

/// Every level the person set, by sound id (what the config saves).
pub fn all() -> BTreeMap<String, f32> {
    read(|m| m.clone())
}

/// Whether the person has set any level (Settings greys out "Reset all" when not).
pub fn any() -> bool {
    read(|m| !m.is_empty())
}

/// Put the levels back as the config kept them (at load), replacing any held now. Each is kept as
/// [`kept_db`] keeps it; a 0 or a number that is not one is dropped.
pub fn restore(levels: &BTreeMap<String, f32>) {
    let clean: BTreeMap<String, f32> = levels
        .iter()
        .filter_map(|(id, db)| kept_db(*db).filter(|d| *d != 0.0).map(|d| (id.clone(), d)))
        .collect();
    write(|m| *m = clean);
}

/// Give this thread levels of its own (tests): every function here then reads and writes those,
/// not the app's, until the thread ends. The app never calls it.
pub fn use_on_this_thread(levels: BTreeMap<String, f32>) {
    THIS_THREAD.with(|c| *c.borrow_mut() = Some(levels));
}

#[cfg(test)]
#[path = "sound_levels_tests.rs"]
mod tests;
