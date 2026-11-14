//! Decoding recordings for the score, several files at once (BUG-297, docs/design/music.md
//! section 12a).
//!
//! A genre change builds a new score whose recordings must be decoded before it can play. One file
//! after another, electro swing's 51 small Ogg files took the build's worker most of its time; the
//! files are independent, so they are decoded on a few threads at once. Nothing is kept here: the
//! caller (the engine's `SampleBank`) decides what, if anything, outlives the build, under the Dev
//! page's "turn off all reuse" switch (CLAUDE.md, the no-caches rule).

use std::path::{Path, PathBuf};

/// One decoded file: its sample rate and its frames as 16-bit stereo
/// ([`crate::instrument::SAMPLE_SCALE`]).
pub type Decoded = (u32, Vec<[i16; 2]>);

/// Decode one audio file (Ogg Vorbis, FLAC, WAV: whatever kira and symphonia read).
pub fn decode_file(path: &Path) -> Result<Decoded, String> {
    let data = kira::sound::static_sound::StaticSoundData::from_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((data.sample_rate, crate::instrument::frames16(data.frames.iter().map(|f| [f.left, f.right]))))
}

/// How many threads a batch of decodes uses: all but one of the machine's cores, at most four (the
/// game's own threads keep running beside it; four already brings electro swing's set from 120 ms
/// to about 40 in an optimised build, and more threads gain little on files this small).
pub fn decode_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get().saturating_sub(1)).unwrap_or(1).clamp(1, 4)
}

/// Decode every file in `paths` on up to `threads` threads; the results in the same order. The
/// files are shared out by an atomic counter, so a long file does not hold up the others.
pub fn decode_files(paths: &[PathBuf], threads: usize) -> Vec<Result<Decoded, String>> {
    let threads = threads.clamp(1, paths.len().max(1));
    if threads <= 1 {
        return paths.iter().map(|p| decode_file(p)).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut out: Vec<Option<Result<Decoded, String>>> = (0..paths.len()).map(|_| None).collect();
    let parts: Vec<Vec<(usize, Result<Decoded, String>)>> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = Vec::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(p) = paths.get(i) else { break };
                        mine.push((i, decode_file(p)));
                    }
                    mine
                })
            })
            .collect();
        workers.into_iter().map(|w| w.join().unwrap_or_default()).collect()
    });
    for (i, r) in parts.into_iter().flatten() {
        out[i] = Some(r);
    }
    out.into_iter().enumerate().map(|(i, r)| r.unwrap_or_else(|| Err(format!("{}: the decode thread stopped", paths[i].display())))).collect()
}
