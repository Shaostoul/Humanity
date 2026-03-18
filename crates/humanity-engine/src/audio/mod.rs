//! Audio manager — powered by kira.
//!
//! Audio settings loaded from `config/audio.toml`.

pub mod spatial;

/// Manages audio playback via kira.
pub struct AudioManager {
    // TODO: kira::manager::AudioManager
}

impl AudioManager {
    pub fn new() -> Self {
        Self {}
    }
}
