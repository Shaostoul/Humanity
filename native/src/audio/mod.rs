//! Audio manager powered by kira.
//!
//! Handles music, sound effects, ambient layers, and spatial audio.
//! All audio code is gated behind the `native` feature (kira doesn't support WASM).

pub mod spatial;

#[cfg(feature = "native")]
pub mod sounds;

#[cfg(feature = "native")]
use std::collections::HashMap;

/// Manages all audio playback via kira.
#[cfg(feature = "native")]
pub struct AudioManager {
    manager: kira::manager::AudioManager,
    /// Cached sound data by path.
    sound_cache: HashMap<String, kira::sound::static_sound::StaticSoundData>,
    /// Current music track handle (if playing).
    music_handle: Option<kira::sound::static_sound::StaticSoundHandle>,
    /// Volume settings (0.0 - 1.0).
    master_volume: f64,
    music_volume: f64,
    sfx_volume: f64,
}

#[cfg(feature = "native")]
impl AudioManager {
    pub fn new() -> Self {
        let manager = kira::manager::AudioManager::<kira::manager::backend::DefaultBackend>::new(
            kira::manager::AudioManagerSettings::default(),
        )
        .expect("Failed to create audio manager");

        Self {
            manager,
            sound_cache: HashMap::new(),
            music_handle: None,
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 1.0,
        }
    }

    /// Play a one-shot sound effect.
    pub fn play_sound(&mut self, path: &str) -> Result<(), String> {
        let data = self.load_sound(path)?;
        let settings = kira::sound::static_sound::StaticSoundSettings::default()
            .volume(kira::Volume::Amplitude(self.master_volume * self.sfx_volume));
        let data_with_settings = data.with_settings(settings);
        self.manager
            .play(data_with_settings)
            .map_err(|e| format!("Play error: {}", e))?;
        Ok(())
    }

    /// Play looping background music (stops previous track).
    pub fn play_music(&mut self, path: &str, volume: f64) -> Result<(), String> {
        self.stop_music();
        let data = self.load_sound(path)?;
        let effective_vol = self.master_volume * self.music_volume * volume;
        let settings = kira::sound::static_sound::StaticSoundSettings::new()
            .volume(kira::Volume::Amplitude(effective_vol))
            .loop_region(..);
        let data_with_settings = data.with_settings(settings);
        let handle = self
            .manager
            .play(data_with_settings)
            .map_err(|e| format!("Music play error: {}", e))?;
        self.music_handle = Some(handle);
        Ok(())
    }

    /// Stop currently playing music.
    pub fn stop_music(&mut self) {
        if let Some(ref mut handle) = self.music_handle {
            handle.stop(kira::tween::Tween {
                duration: std::time::Duration::from_millis(500),
                ..Default::default()
            });
        }
        self.music_handle = None;
    }

    /// Set master volume (0.0 - 1.0).
    pub fn set_master_volume(&mut self, vol: f64) {
        self.master_volume = vol.clamp(0.0, 1.0);
    }

    /// Set music volume (0.0 - 1.0).
    pub fn set_music_volume(&mut self, vol: f64) {
        self.music_volume = vol.clamp(0.0, 1.0);
    }

    /// Set SFX volume (0.0 - 1.0).
    pub fn set_sfx_volume(&mut self, vol: f64) {
        self.sfx_volume = vol.clamp(0.0, 1.0);
    }

    /// Play a sound with distance-based volume falloff (simple spatial audio).
    pub fn play_spatial(
        &mut self,
        path: &str,
        source_pos: [f32; 3],
        listener_pos: [f32; 3],
    ) -> Result<(), String> {
        let dx = source_pos[0] - listener_pos[0];
        let dy = source_pos[1] - listener_pos[1];
        let dz = source_pos[2] - listener_pos[2];
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        // Inverse distance falloff, clamped
        let max_distance = 50.0_f32;
        if distance > max_distance {
            return Ok(()); // Too far, don't play
        }
        let falloff = (1.0 - distance / max_distance).max(0.0) as f64;
        let vol = self.master_volume * self.sfx_volume * falloff;

        // Simple stereo panning from X offset
        let pan = (dx / max_distance).clamp(-1.0, 1.0) as f64;

        let data = self.load_sound(path)?;
        let settings = kira::sound::static_sound::StaticSoundSettings::default()
            .volume(kira::Volume::Amplitude(vol))
            .panning(0.5 + pan * 0.5); // kira panning: 0.0=left, 0.5=center, 1.0=right
        let data_with_settings = data.with_settings(settings);
        self.manager
            .play(data_with_settings)
            .map_err(|e| format!("Spatial play error: {}", e))?;
        Ok(())
    }

    /// Load and cache sound data from a file path.
    fn load_sound(
        &mut self,
        path: &str,
    ) -> Result<kira::sound::static_sound::StaticSoundData, String> {
        if let Some(data) = self.sound_cache.get(path) {
            return Ok(data.clone());
        }
        let data = kira::sound::static_sound::StaticSoundData::from_file(path)
            .map_err(|e| format!("Failed to load '{}': {}", path, e))?;
        self.sound_cache.insert(path.to_string(), data.clone());
        Ok(data)
    }
}

/// Stub AudioManager for non-native builds.
#[cfg(not(feature = "native"))]
pub struct AudioManager;

#[cfg(not(feature = "native"))]
impl AudioManager {
    pub fn new() -> Self {
        Self
    }
}
