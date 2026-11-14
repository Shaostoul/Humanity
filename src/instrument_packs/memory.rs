//! The memory each installed set of instruments takes while its parts play, for Settings > Audio >
//! Instruments (2026-11-14, docs/design/music.md section 23, "Memory"): the same honesty CLAUDE.md
//! asks of a graphics setting ("say what it actually changes and what it costs"), for music.
//!
//! For every part the music plays (a part marked `not_used_yet` never sounds), the instrument it
//! would choose ([`InstrumentLibrary::choose`]) is counted against its set: each of its recordings at the length its file's header gives (nothing
//! decoded), held whole or, when a note plays it straight through and it is long, only its attack
//! (the rest streams, `data/music/streaming.ron`), as [`super::score::attach`] holds it. The rings the
//! streams read through are shared by every set, so the Dev page's music line counts them, not a set.
//!
//! Reading every header takes a while for the full packs (several thousand files), so it runs on its
//! own thread ([`super::ui::tick`]): when the section first shows, after each install or removal,
//! and, with the Dev page's "turn off all reuse" switch off, every time the section opens again.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use humanity_music::stream::StreamSettings;

use super::catalog::Parts;
use super::library::{Choice, InstrumentLibrary};

/// What one set's chosen instruments hold while they play.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SetMemory {
    /// As the music holds them (long recordings streamed), bytes.
    pub held: u64,
    /// Were every recording decoded whole, bytes.
    pub whole: u64,
    /// Instruments counted (one per part the set plays).
    pub instruments: usize,
    /// Recordings whose length could not be read (left out of both).
    pub unread: usize,
}

/// One recording to count: its path, and whether a note plays it straight through.
struct Job {
    set: String,
    path: PathBuf,
    streamable: bool,
}

/// Each set's memory (the core set as "core", each pack by its id), from `data_dir` as it is now.
pub fn by_set(data_dir: &Path) -> BTreeMap<String, SetMemory> {
    let library = InstrumentLibrary::load(data_dir);
    let Ok(parts) = Parts::load(data_dir) else { return BTreeMap::new() };
    let (settings, _) = super::samples::stream_settings(data_dir);
    count(&library, &parts, &settings)
}

/// Each set's memory for `library`'s choices of `parts` under `settings`.
pub fn count(library: &InstrumentLibrary, parts: &Parts, settings: &StreamSettings) -> BTreeMap<String, SetMemory> {
    let mut out: BTreeMap<String, SetMemory> = BTreeMap::new();
    let mut jobs = Vec::new();
    let mut seen: Vec<&str> = Vec::new();
    // A part marked not used yet never sounds, so its instrument holds nothing.
    for part in parts.parts.iter().filter(|p| !p.not_used_yet) {
        let Some(Choice::Sampled(a)) = library.choose(&part.id, parts) else { continue };
        if seen.contains(&a.def.id.as_str()) {
            continue;
        }
        seen.push(&a.def.id);
        let set = a.pack.clone().unwrap_or_else(|| "core".to_string());
        out.entry(set.clone()).or_default().instruments += 1;
        for z in &a.sampler().zones {
            jobs.push(Job { set: set.clone(), path: a.zone_path(&z.file), streamable: super::score::streamable(z, part.sustained) });
        }
    }
    let lengths = humanity_music::decode::each(&jobs, humanity_music::decode::decode_threads(), |j| humanity_music::decode::frame_count(&j.path));
    for (j, len) in jobs.iter().zip(lengths) {
        let m = out.entry(j.set.clone()).or_default();
        match len {
            Ok((rate, Some(frames))) => {
                let (held, whole) = held_frames(frames, rate, j.streamable, settings);
                m.held += held * 4;
                m.whole += whole * 4;
            }
            _ => m.unread += 1,
        }
    }
    out
}

/// A recording of `frames` at `rate`: the frames the music holds of it, and the frames it has. A note
/// that plays it straight through holds only its attack when it is longer than streaming's
/// threshold; any other is held whole.
pub fn held_frames(frames: u64, rate: u32, streamable: bool, settings: &StreamSettings) -> (u64, u64) {
    let limit = (settings.stream_longer_than_s.max(0.0) * rate as f32) as u64;
    let attack = ((settings.attack_s.max(0.01) * rate as f32) as u64).clamp(1, limit.max(1));
    if settings.enabled && streamable && frames > limit {
        (attack, frames)
    } else {
        (frames, frames)
    }
}
