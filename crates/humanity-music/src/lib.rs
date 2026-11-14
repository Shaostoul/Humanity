//! HumanityOS's live music and the ship's hum (docs/design/music.md; the crate card is
//! docs/crates/humanity-music.md).
//!
//! The operator, 2026-11-06: "Can you make music for HumanityOS? Like chill orchestral sci-fi operatic
//! soft metal? ... Designed for a peaceful chill environment hanging out on the mothership. In
//! 432Hz." Then, hearing the direction sketch: "Could we deep drums and steel drums. Let's just keep
//! focusing of 432 Hz." and "I'm curious how we could tailor it to different tastes."
//!
//! So the app composes its own music live, the way a musician improvises within rules: a composer
//! (composer.rs) walks a mood's chord graph with voice leading, plans phrases with motifs it varies,
//! and never repeats a phrase within ten minutes; a pool of synthesised voices (voice.rs: pads, a
//! formant choir, modal bells and STEEL PAN, membrane DEEP DRUMS, FM keys, plucked strings, noise and
//! the sampler seam for CC0 recordings) plays it; a reverb, a genre's sound-design recipe and a
//! limiter finish it (score.rs). Moods, genres, instruments, places and hum sources are data files
//! (data/music/, data/audio/hum_sources.ron). Every pitch comes from one function at A = 432 Hz by
//! default (tuning.rs). The ship's hum and the optional tones are a second sound (bed.rs).
//!
//! Pure: no audio device, no GPU, no I/O on the hot path. The engines render into a plain
//! `&mut [[f32; 2]]`, so tests run headless faster than real time and `just music-render` renders
//! offline. The `kira` feature adds the adapter that plays them on the sound card (kira_sound.rs).
//! Nothing here is ever networked: each player's game composes its own music.

pub mod bed;
pub mod composer;
pub mod dsp;
pub mod favourite;
pub mod instrument;
pub mod intensity;
pub mod library;
pub mod mood;
pub mod piece;
pub mod places;
pub mod render;
pub mod resolved;
pub mod rng;
pub mod score;
pub mod settings;
pub mod sheet;
pub mod genre;
pub mod tune;
pub mod tuning;
pub mod voice;

#[cfg(feature = "kira")]
pub mod decode;
#[cfg(feature = "kira")]
pub mod kira_sound;

pub use bed::{hum_place, BedContext, BedEngine, BedStatus, HumPlace, HumSources};
pub use instrument::{SampleBank, SampleLibrary, SampleSet, SampleZone};
pub use library::MusicLibrary;
pub use resolved::Resolved;
pub use score::{MusicContext, MusicStatus, ScoreEngine};
pub use settings::MusicSettings;
pub use tuning::{note_hz, Tuning};

/// The seed a session composes from: `HUMANITY_MUSIC_SEED` when set (rigs and tests reach the same
/// music every time), else a new one each run.
pub fn session_seed() -> u64 {
    if let Some(s) = std::env::var("HUMANITY_MUSIC_SEED").ok().and_then(|v| v.trim().parse::<u64>().ok()) {
        return s;
    }
    let t = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0x1234_5678);
    rng::Rng::new(t ^ std::process::id() as u64).next_u64()
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_audio;
#[cfg(test)]
mod tests_piece;
#[cfg(test)]
mod tests_page;
#[cfg(test)]
mod tests_electronic;
