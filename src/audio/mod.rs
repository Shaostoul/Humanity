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
    /// Interface-sound bus gain (0.0 - 1.0). Sounds whose catalog `bus` is
    /// "ui" (button clicks / UI feedback) route through this instead of
    /// `sfx_volume`, so a user can quiet clicks without silencing footsteps
    /// and machines. Default 1.0.
    ui_volume: f64,
}

#[cfg(feature = "native")]
impl AudioManager {
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create audio manager")
    }

    /// Fallible construction (v0.960): a machine with no audio device
    /// (headless rig, some VMs) must degrade to silence, not crash at
    /// startup - the engine stores Option<AudioManager> and skips play
    /// calls when None.
    pub fn try_new() -> Result<Self, String> {
        let manager = kira::manager::AudioManager::<kira::manager::backend::DefaultBackend>::new(
            kira::manager::AudioManagerSettings::default(),
        )
        .map_err(|e| format!("audio device init failed: {e}"))?;

        Ok(Self {
            manager,
            sound_cache: HashMap::new(),
            music_handle: None,
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 1.0,
            ui_volume: 1.0,
        })
    }

    /// Play a one-shot sound effect.
    pub fn play_sound(&mut self, path: &str) -> Result<(), String> {
        self.play_sound_vol(path, 1.0)
    }

    /// Play a one-shot at a per-sound volume (v0.996): the catalog's
    /// `volume` field finally applies - before this every effect played at
    /// full master*sfx amplitude and the operator's "footsteps are kind of
    /// loud and jarring" was structural, not a tuning miss.
    ///
    /// Routes through the sfx bus. For interface sounds, call
    /// [`play_sound_bus`](Self::play_sound_bus) with `"ui"` so the separate
    /// interface-volume control governs them.
    pub fn play_sound_vol(&mut self, path: &str, vol: f64) -> Result<(), String> {
        self.play_sound_bus(path, vol, "sfx")
    }

    /// Play a one-shot routed through a named audio bus (v0.1112). The "ui"
    /// bus carries its own volume so a user can quiet interface clicks without
    /// touching footsteps and machines; every other bus routes through sfx.
    /// This is what finally makes the `bus` field in data/sounds.toml
    /// load-bearing - before this it was parsed and ignored.
    pub fn play_sound_bus(&mut self, path: &str, vol: f64, bus: &str) -> Result<(), String> {
        let bus_gain = if bus == "ui" {
            self.ui_volume
        } else {
            self.sfx_volume
        };
        let data = self.load_sound(path)?;
        let settings = kira::sound::static_sound::StaticSoundSettings::default().volume(
            kira::Volume::Amplitude(self.master_volume * bus_gain * vol.clamp(0.0, 2.0)),
        );
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

    /// Set interface-sound volume (0.0 - 1.0). Governs sounds on the "ui" bus.
    pub fn set_ui_volume(&mut self, vol: f64) {
        self.ui_volume = vol.clamp(0.0, 1.0);
    }

    /// The current master volume (0.0 - 1.0). A long-lived sound whose
    /// volume is set per frame (a clip on an in-world screen) reads this so
    /// a Settings change reaches it live; the one-shot paths snapshot it at
    /// play time instead.
    pub fn master_volume(&self) -> f64 {
        self.master_volume
    }

    /// The current sound-effects bus volume (0.0 - 1.0). World sounds that
    /// are not one-shots (a clip playing on a screen) sit on this bus, the
    /// same one footsteps and machines use.
    pub fn sfx_volume(&self) -> f64 {
        self.sfx_volume
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

    /// Play a streaming sound whose samples come from a caller-provided
    /// decoder (the media player's Opus track, `src/media/audio.rs`). kira
    /// pulls chunks from the decoder on its own decode thread; the returned
    /// handle reports the playback position and takes pause / resume / seek,
    /// which is what lets the video clock follow the sound card.
    ///
    /// `amplitude` and `panning` are ABSOLUTE and apply from the very first
    /// sample: `amplitude` is the final gain (1.0 = as decoded, 0.0 = silent;
    /// the caller composes master x bus x placement itself, which is why
    /// `master_volume()` and `sfx_volume()` are readable), `panning` is
    /// kira's 0..1 (0 hard left, 0.5 centre, 1 hard right). Unlike the
    /// one-shot paths above, master is NOT multiplied in here: a long-lived
    /// stream re-sends its full mix every frame through its handle, and
    /// that value is absolute too, so the attach and the updates must agree
    /// or the first frames play at the wrong level (they did: a stream
    /// attached at "master" and then tweened to master x sfx x falloff spent
    /// its first 60 ms up to 33 times too loud with sfx at 10 percent).
    pub fn play_stream<E: Send + 'static>(
        &mut self,
        data: kira::sound::streaming::StreamingSoundData<E>,
        amplitude: f64,
        panning: f64,
    ) -> Result<kira::sound::streaming::StreamingSoundHandle<E>, String> {
        let data = data
            .volume(kira::Volume::Amplitude(amplitude.clamp(0.0, 2.0)))
            .panning(panning.clamp(0.0, 1.0));
        self.manager
            .play(data)
            .map_err(|e| format!("Stream play error: {e}"))
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
