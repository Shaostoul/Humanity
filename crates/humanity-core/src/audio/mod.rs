//! Audio manager powered by kira.
//!
//! Handles music, sound effects, ambient layers, and spatial audio.
//! All audio code is gated behind the `native` feature (kira doesn't support WASM).

pub mod spatial;

/// ITU-R BS.1770 loudness: the meter that says how loud a sound is (docs/design/loudness.md).
pub mod loudness;
/// "Even out loudness": the kinds of sound, their targets and the shipped sounds' measurements.
pub mod levels;
/// What volume a new video starts at (Settings > Audio, 2026-11-06).
pub mod start_volume;
/// The person's own level for each shipped sound (Settings > Audio > Sound levels, 2026-11-06).
pub mod sound_levels;

#[cfg(feature = "native")]
pub mod sounds;

#[cfg(feature = "native")]
use std::collections::HashMap;

/// Manages all audio playback via kira.
///
/// `B` is kira's backend: the sound card (`kira::DefaultBackend`, the default, what the app always
/// uses) or, for tests on a machine with no sound card, kira's mock backend
/// ([`AudioManager::without_device`]), which takes every sound and plays none, so a test can check
/// what a sound WOULD play at through the very functions the app calls.
#[cfg(feature = "native")]
pub struct AudioManager<B: kira::backend::Backend = kira::DefaultBackend> {
    manager: kira::AudioManager<B>,
    /// Decoded sound data by path, kept while reuse is on (the Dev page's switch: off, every play
    /// decodes its file again).
    sound_cache: HashMap<String, kira::sound::static_sound::StaticSoundData>,
    /// How loud each sound the measured data does not list was found to be when it loaded, kept
    /// while reuse is on (off: measured again at every play).
    measured_cache: HashMap<String, loudness::SoundLoudness>,
    /// "Even out loudness" (Settings > Audio, on by default): each sound played at its kind's
    /// loudness (`levels`).
    even_loudness: bool,
    /// The targets and measurements, read from `data_dir` the first time a sound plays (None after
    /// that only when the targets could not be read: nothing is levelled).
    levels: Option<levels::LoudnessLevels>,
    /// Each catalogued sound file's id (`sfx.button_click`) by [`levels::sound_key`], read with the
    /// levels: the person's own level for a sound ([`sound_levels`]) is kept by its id.
    ids: HashMap<String, String>,
    /// Whether `levels` and `ids` were read (with reuse off they are read again at every play).
    levels_read: bool,
    /// Where the data folder is (the app's "data" beside it; tests point at the repo's).
    data_dir: std::path::PathBuf,
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
    /// The sound Settings > Audio > Sound levels' Play started last, stopped when the next starts
    /// (so a long sound previewed twice does not play twice over itself).
    preview_handle: Option<kira::sound::static_sound::StaticSoundHandle>,
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
        let manager = kira::AudioManager::<kira::DefaultBackend>::new(
            kira::AudioManagerSettings::default(),
        )
        .map_err(|e| format!("audio device init failed: {e}"))?;
        Ok(Self::with_manager(manager))
    }
}

#[cfg(feature = "native")]
impl AudioManager<kira::backend::mock::MockBackend> {
    /// An audio manager on kira's mock backend: no sound card is opened and nothing is heard, but
    /// every sound is decoded, levelled and handed to the mixer exactly as the app's would be. For
    /// tests (they run on machines with no sound card, and must never make a sound): what
    /// `one_shot` and `preview` answer is what the app plays at. The app never builds one.
    pub fn without_device() -> Self {
        let manager = kira::AudioManager::<kira::backend::mock::MockBackend>::new(kira::AudioManagerSettings::default())
            .expect("kira's mock backend opens no device and cannot fail");
        Self::with_manager(manager)
    }
}

#[cfg(feature = "native")]
impl<B: kira::backend::Backend> AudioManager<B> {
    fn with_manager(manager: kira::AudioManager<B>) -> Self {
        Self {
            manager,
            sound_cache: HashMap::new(),
            measured_cache: HashMap::new(),
            even_loudness: true,
            levels: None,
            ids: HashMap::new(),
            levels_read: false,
            data_dir: std::path::PathBuf::from("data"),
            music_handle: None,
            master_volume: 1.0,
            music_volume: 0.7,
            sfx_volume: 1.0,
            ui_volume: 1.0,
            preview_handle: None,
        }
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
        let (data, amplitude) = self.one_shot(path, vol, bus)?;
        let settings = kira::sound::static_sound::StaticSoundSettings::default().volume(decibels(amplitude));
        let data_with_settings = data.with_settings(settings);
        self.manager
            .play(data_with_settings)
            .map_err(|e| format!("Play error: {}", e))?;
        Ok(())
    }

    /// The decoded sound at `path` and the amplitude a one-shot of it plays at on `bus` with its
    /// authored volume `vol`: master x bus x volume x its loudness level ([`Self::level_for`], 1.0
    /// with "Even out loudness" off) x the person's own level for it ([`Self::persons_level`],
    /// Settings > Audio > Sound levels). One function, so what is played and what a test checks
    /// are the same number. The "ui" bus is the interface volume, "music" the music volume, any
    /// other the sound-effects volume.
    pub fn one_shot(&mut self, path: &str, vol: f64, bus: &str) -> Result<(kira::sound::static_sound::StaticSoundData, f64), String> {
        let bus_gain = match bus {
            "ui" => self.ui_volume,
            "music" => self.music_volume,
            _ => self.sfx_volume,
        };
        let data = self.load_sound(path)?;
        self.read_data();
        let vol = vol.clamp(0.0, 2.0);
        let level = self.level_for(path, &data, vol);
        let persons = self.persons_level(path);
        Ok((data, self.master_volume * bus_gain * vol * level * persons))
    }

    /// Play the catalogued sound at `path` once, the way Settings > Audio > Sound levels' Play
    /// does: the one-shot it is in the game ([`Self::one_shot`]: its authored volume `vol`, its
    /// `bus`, its loudness level and the person's level for it as it is NOW, so a slider just
    /// moved is heard at once), after stopping the sound the last Play started. The amplitude it
    /// plays at, for the tests.
    pub fn preview(&mut self, path: &str, vol: f64, bus: &str) -> Result<f64, String> {
        if let Some(mut last) = self.preview_handle.take() {
            last.stop(kira::Tween::default());
        }
        let (data, amplitude) = self.one_shot(path, vol, bus)?;
        let settings = kira::sound::static_sound::StaticSoundSettings::default().volume(decibels(amplitude));
        let handle = self.manager.play(data.with_settings(settings)).map_err(|e| format!("Play error: {e}"))?;
        self.preview_handle = Some(handle);
        Ok(amplitude)
    }

    /// The person's own level for the sound at `path` as an amplitude (Settings > Audio > Sound
    /// levels, [`sound_levels`]): the catalogue says which sound id the file is, and a file no
    /// catalogue entry names (a mod's, a video's) has no level, 1.0.
    fn persons_level(&self, path: &str) -> f64 {
        self.ids.get(&levels::sound_key(path)).map_or(1.0, |id| sound_levels::gain(id))
    }

    /// Read the sound catalogue and the loudness data from `data_dir`: each sound file's kind and
    /// id, the targets and the measurements. Once, the first time a sound plays; with the Dev
    /// page's "turn off all reuse" switch off, at every play, so a data file changed on disk is
    /// heard at once (CLAUDE.md, the reuse rule).
    fn read_data(&mut self) {
        if self.levels_read && crate::reuse::enabled() {
            return;
        }
        self.levels_read = true;
        let catalog = sounds::SoundCatalog::load(&self.data_dir);
        self.ids = catalog.ids_by_sound_key();
        match levels::LoudnessLevels::load(&self.data_dir, catalog.loudness_sources()) {
            Ok(l) => self.levels = Some(l),
            Err(e) => {
                self.levels = None;
                log::warn!("[Audio] Even out loudness is off for this run: {e}");
            }
        }
    }

    /// "Even out loudness" on or off (Settings > Audio; the engine sends it every frame). Applies to
    /// sounds that start from now on.
    pub fn set_even_loudness(&mut self, on: bool) {
        self.even_loudness = on;
    }

    /// Whether "Even out loudness" is on.
    pub fn even_loudness(&self) -> bool {
        self.even_loudness
    }

    /// Read the loudness data from `dir` instead of the app's `data` folder (tests: the repo's).
    pub fn set_data_dir(&mut self, dir: &std::path::Path) {
        self.data_dir = dir.to_path_buf();
        self.levels = None;
        self.levels_read = false;
    }

    /// The loudness level of the sound at `path` (decoded as `data`, played at authored `volume`):
    /// 1.0 with "Even out loudness" off; else the gain to its kind's loudness (`levels`), from the
    /// measured data, or, for a file the data does not list or with reuse off, from measuring `data`
    /// now. The caller has read the data ([`Self::read_data`]).
    fn level_for(&mut self, path: &str, data: &kira::sound::static_sound::StaticSoundData, volume: f64) -> f64 {
        if !self.even_loudness {
            return 1.0;
        }
        let reuse = crate::reuse::enabled();
        if !reuse {
            self.measured_cache.clear();
        }
        let key = levels::sound_key(path);
        let cache = &mut self.measured_cache;
        levels::level_for(self.levels.as_ref(), true, path, volume, reuse, || {
            if let Some(s) = cache.get(&key) {
                return Some(*s);
            }
            let s = measure_static(data);
            if reuse {
                cache.insert(key.clone(), s);
            }
            Some(s)
        })
    }

    /// Play looping background music (stops previous track).
    pub fn play_music(&mut self, path: &str, volume: f64) -> Result<(), String> {
        self.stop_music();
        let data = self.load_sound(path)?;
        self.read_data();
        let level = self.level_for(path, &data, volume);
        let effective_vol = self.master_volume * self.music_volume * volume * level * self.persons_level(path);
        let settings = kira::sound::static_sound::StaticSoundSettings::new()
            .volume(decibels(effective_vol))
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
            handle.stop(kira::Tween {
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
        let data = self.load_sound(path)?;
        self.read_data();
        let vol = self.master_volume * self.sfx_volume * falloff * self.level_for(path, &data, 1.0) * self.persons_level(path);

        // Simple stereo panning from X offset
        let pan = (dx / max_distance).clamp(-1.0, 1.0) as f64;

        let settings = kira::sound::static_sound::StaticSoundSettings::default()
            .volume(decibels(vol))
            .panning(panning(0.5 + pan * 0.5)); // ours: 0.0=left, 0.5=center, 1.0=right
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
    /// 0..1 (0 hard left, 0.5 centre, 1 hard right; `panning` below turns it into kira's
    /// -1..1). Unlike the
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
            .volume(decibels(amplitude.clamp(0.0, 2.0)))
            .panning(self::panning(panning));
        self.manager
            .play(data)
            .map_err(|e| format!("Stream play error: {e}"))
    }

    /// Load a sound file, kept decoded by path while reuse is on. With the Dev page's "turn off all
    /// reuse" switch off (CLAUDE.md, the reuse rule: every reuse obeys it), the kept copies go and
    /// each play decodes its file again, so a sound file changed on disk plays as it is now.
    fn load_sound(
        &mut self,
        path: &str,
    ) -> Result<kira::sound::static_sound::StaticSoundData, String> {
        let reuse = crate::reuse::enabled();
        if !reuse {
            self.sound_cache.clear();
        } else if let Some(data) = self.sound_cache.get(path) {
            return Ok(data.clone());
        }
        let data = kira::sound::static_sound::StaticSoundData::from_file(path)
            .map_err(|e| format!("Failed to load '{}': {}", path, e))?;
        if reuse {
            self.sound_cache.insert(path.to_string(), data.clone());
        }
        Ok(data)
    }
}

/// How loud a decoded sound is, measured as it plays: kira's frames are always two channels (a mono
/// file is sent to both speakers), so it is measured as two. A few hundred microseconds for a
/// one-second one-shot in a release build.
#[cfg(feature = "native")]
pub fn measure_static(data: &kira::sound::static_sound::StaticSoundData) -> loudness::SoundLoudness {
    let mut samples = Vec::with_capacity(data.frames.len() * 2);
    for f in data.frames.iter() {
        samples.push(f.left);
        samples.push(f.right);
    }
    loudness::measure_sound(data.sample_rate, 2, &samples)
}

/// Decode the sound file at `path` and measure it ([`measure_static`]).
#[cfg(feature = "native")]
pub fn measure_file(path: &std::path::Path) -> Result<loudness::SoundLoudness, String> {
    let data = kira::sound::static_sound::StaticSoundData::from_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(measure_static(&data))
}

/// Measure every sound file the catalog under `repo/data` names that exists under `repo/assets`:
/// what `data/audio/measured_loudness.toml` holds (`just loudness` writes it, a lib test checks it).
#[cfg(feature = "native")]
pub fn measure_shipped(repo: &std::path::Path) -> Result<levels::MeasuredSounds, String> {
    let catalog = sounds::SoundCatalog::load(&repo.join("data"));
    let mut out = levels::MeasuredSounds::default();
    for (path, _) in catalog.loudness_sources() {
        let file = repo.join("assets").join(&path);
        if file.exists() {
            out.sounds.insert(levels::sound_key(&path), measure_file(&file)?);
        }
    }
    Ok(out)
}

/// An amplitude (1.0 = as recorded, 0.0 = silent, up to 2.0) as the decibels kira takes.
///
/// kira 0.9 took a volume as an amplitude; kira 0.10 on (0.12 since 2026-11-06, the security
/// cadence: 0.9's ringbuf 0.3 had RUSTSEC-2026-0293) takes decibels. Every caller here, and the
/// media player and the sound view through `play_stream` and their handles, works in amplitude,
/// so this converts at the edge: 20 x log10(amplitude). kira plays -60 dB and below as silence
/// (`Decibels::SILENCE`, an amplitude of 0.001), so anything at or under that floor, 0 and a NaN
/// included, is sent as the floor.
#[cfg(feature = "native")]
pub fn decibels(amplitude: f64) -> kira::Decibels {
    let floor = f64::from(kira::Decibels::SILENCE.0);
    if !(amplitude > 0.0) {
        return kira::Decibels::SILENCE;
    }
    kira::Decibels((20.0 * amplitude.log10()).max(floor) as f32)
}

/// Our panning (0 = hard left, 0.5 = centre, 1 = hard right, what kira 0.9 took) as kira 0.12's
/// (-1 .. 0 .. 1). Every caller keeps the 0..1 it has; out-of-range values are clamped.
#[cfg(feature = "native")]
pub fn panning(pan: f64) -> kira::Panning {
    kira::Panning((pan.clamp(0.0, 1.0) * 2.0 - 1.0) as f32)
}

#[cfg(all(test, feature = "native"))]
mod kira_edge_tests {
    use super::*;

    /// The kira 0.9 -> 0.12 conversion keeps every level the game sends: an amplitude round-trips
    /// through kira's own decibels-to-amplitude within a hair, silence stays silent, and the
    /// 2.0 boost is +6 dB. RED first (2026-11-06): with the naive `Decibels(amplitude as f32)`
    /// this failed at its first line, "1.0 is unchanged" (left: Decibels(1.0), right:
    /// Decibels(0.0)): every level would have been sent as a tiny boost.
    #[test]
    fn an_amplitude_reaches_kira_as_the_same_level() {
        assert_eq!(decibels(1.0), kira::Decibels::IDENTITY, "1.0 is unchanged");
        for amp in [0.002, 0.01, 0.1, 0.25, 0.5, 0.7, 0.9, 1.0, 1.5, 2.0] {
            let back = f64::from(decibels(amp).as_amplitude());
            assert!((back - amp).abs() < amp * 1e-4, "{amp} came back as {back}");
        }
        assert!((decibels(2.0).0 - 6.0206).abs() < 1e-3, "2.0 is +6 dB: {:?}", decibels(2.0));
        for silent in [0.0, -1.0, 0.0005, f64::NAN] {
            assert_eq!(decibels(silent).as_amplitude(), 0.0, "{silent} is silent");
        }
    }

    /// Our 0..1 panning lands on kira 0.12's -1..1. RED first: passed straight through, hard
    /// left (0.0) arrived as kira's centre (left: Panning(0.0), right: Panning(-1.0)), and every
    /// screen's sound would have leaned right.
    #[test]
    fn our_panning_lands_on_kiras_scale() {
        assert_eq!(panning(0.0), kira::Panning::LEFT);
        assert_eq!(panning(0.5), kira::Panning::CENTER);
        assert_eq!(panning(1.0), kira::Panning::RIGHT);
        assert_eq!(panning(0.75), kira::Panning(0.5));
        assert_eq!(panning(-3.0), kira::Panning::LEFT, "clamped");
        assert_eq!(panning(9.0), kira::Panning::RIGHT, "clamped");
    }

    /// The one-shot, spatial and music paths on a REAL device through kira 0.12 (the media
    /// player's and the sound view's device tests cover the streaming path): a shipped sound
    /// plays on each bus, panned, and as looping music that is still playing a moment later and
    /// then stops. Silent (master volume 0). A smoke check run by hand, not a gate: a script's
    /// boot of the game opens no audio device (launch_focus.rs), so nothing else plays these on
    /// a device. Seen failing first (2026-11-06) pointed at a file that is not there: "ui one-shot:
    /// Failed to load 'assets/audio/not-there.ogg': The system cannot find the path specified."
    #[test]
    #[ignore = "needs an audio output device; run: node scripts/lib-tests.js --crate humanity-core one_shots_and_music --ignored --nocapture"]
    fn one_shots_and_music_play_on_a_device() {
        let mut audio = match AudioManager::try_new() {
            Ok(a) => a,
            Err(e) => {
                println!("skipped: no audio device ({e})");
                return;
            }
        };
        audio.set_master_volume(0.0);
        let repo = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
        let cat = sounds::SoundCatalog::load(&repo.join("data"));
        let file = cat
            .entries()
            .values()
            .map(|e| repo.join("assets").join(&e.path))
            .filter(|p| p.exists())
            .min()
            .expect("a shipped sound file");
        let path = file.to_str().unwrap();
        audio.play_sound_bus(path, 1.0, "ui").expect("ui one-shot");
        audio.play_sound_vol(path, 0.5).expect("sfx one-shot");
        audio.play_spatial(path, [10.0, 0.0, 0.0], [0.0, 0.0, 0.0]).expect("spatial one-shot");
        audio.play_music(path, 1.0).expect("music");
        std::thread::sleep(std::time::Duration::from_millis(150));
        let state = audio.music_handle.as_ref().map(|h| h.state());
        assert_eq!(state, Some(kira::sound::PlaybackState::Playing), "the music loops on");
        audio.stop_music();
        assert!(audio.music_handle.is_none());
    }

    /// "EVEN OUT LOUDNESS" REACHES WHAT A ONE-SHOT PLAYS AT, on a real device (2026-11-06): the
    /// quietest shipped footstep's amplitude with the setting on is its amplitude off times exactly
    /// the level the shipped data gives it (read by the manager itself from the data folder); with
    /// reuse off the manager measures the file on the spot and lands within 0.01 dB of the table;
    /// and the sound then plays (silently, master 0) both ways. A smoke check run by hand (the
    /// pure parts are lib tests in levels_tests.rs). Reuse is a process-wide switch, so run it on its
    /// own, as the command says. Seen failing first (2026-11-06) with `one_shot` leaving the level
    /// out: "on is off times the level (left: 1, right: 1.72)".
    #[test]
    #[ignore = "needs an audio output device; run: node scripts/lib-tests.js --crate humanity-core even_out_loudness_reaches --ignored --nocapture"]
    fn even_out_loudness_reaches_a_one_shots_volume_on_a_device() {
        let mut audio = match AudioManager::try_new() {
            Ok(a) => a,
            Err(e) => {
                println!("skipped: no audio device ({e})");
                return;
            }
        };
        let repo = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
        audio.set_data_dir(&repo.join("data"));
        audio.set_master_volume(0.5);
        let file = repo.join("assets/audio/sfx/footstep_grass.ogg");
        let path = file.to_str().unwrap();
        audio.set_even_loudness(false);
        let (_, off) = audio.one_shot(path, 0.25, "sfx").expect("off");
        audio.set_even_loudness(true);
        let (_, on) = audio.one_shot(path, 0.25, "sfx").expect("on");
        let levels = audio.levels.clone().expect("the manager read the loudness data");
        let want = levels.gain_for(path, levels.measured(path).expect("the footstep is measured"), 0.25);
        assert!(want > 1.2, "the grass footstep is a quiet file, raised: {want}");
        assert!(((on / off) - want).abs() < 1e-9, "on is off times the level (left: {}, right: {want:.2})", on / off);
        crate::reuse::set_enabled(false);
        let (_, measured_now) = audio.one_shot(path, 0.25, "sfx").expect("reuse off");
        crate::reuse::set_enabled(true);
        assert!((20.0 * (measured_now / on).log10()).abs() < 0.01, "measured on the spot: {measured_now} against the table's {on}");
        audio.set_master_volume(0.0);
        audio.play_sound_vol(path, 0.25).expect("plays with the setting on");
        audio.set_even_loudness(false);
        audio.play_sound_vol(path, 0.25).expect("plays with it off");
        println!("grass footstep at volume 0.25, master 0.5: off {off:.4}, on {on:.4} (x{want:.3}, {:+.2} dB)", 20.0 * want.log10());
    }
}

/// The person's level for a sound (Settings > Audio > Sound levels) reaching what the mixer plays,
/// through the app's own `one_shot` and `preview` on kira's mock backend: no sound card, no sound,
/// the same numbers the app hands kira. Each test on its own thread's levels.
#[cfg(all(test, feature = "native"))]
mod sound_level_tests {
    use super::*;
    use std::collections::BTreeMap;

    fn repo() -> &'static std::path::Path {
        std::path::Path::new(env!("HUMANITY_REPO_ROOT"))
    }

    fn manager() -> AudioManager<kira::backend::mock::MockBackend> {
        let mut audio = AudioManager::without_device();
        audio.set_data_dir(&repo().join("data"));
        audio.set_master_volume(0.5);
        audio.set_ui_volume(0.8);
        audio
    }

    fn file(rel: &str) -> String {
        repo().join("assets").join(rel).to_str().unwrap().to_string()
    }

    fn db(ratio: f64) -> f64 {
        20.0 * ratio.log10()
    }

    /// A LEVEL CHANGES WHAT A SOUND PLAYS AT BY EXACTLY ITS dB, on top of "Even out loudness" and
    /// with it off: the button click (the sound the operator asked about, `sfx.button_click`,
    /// found from its file by the catalogue) at -6 dB plays at its amplitude without a level times
    /// 10^(-6/20), at +12 dB times 10^(12/20); the crash, given no level, plays exactly as before
    /// (a sound missing from the levels plays at its default); and Reset puts the click back to
    /// exactly its levelled amplitude. RED first (2026-11-06): with `one_shot` leaving the person's
    /// level out, "even out true: -6 dB moves the click by -6 dB (0.000 dB)".
    #[test]
    fn a_level_moves_what_a_sound_plays_at_by_exactly_its_db() {
        sound_levels::use_on_this_thread(BTreeMap::new());
        let mut audio = manager();
        let (click, crash) = (file("audio/ui/button_click.ogg"), file("audio/sfx/crash.ogg"));
        for even in [true, false] {
            audio.set_even_loudness(even);
            let (_, base) = audio.one_shot(&click, 0.2, "ui").expect("the click decodes");
            let (_, crash_base) = audio.one_shot(&crash, 0.85, "sfx").expect("the crash decodes");
            sound_levels::set_level("sfx.button_click", -6.0);
            let (_, down) = audio.one_shot(&click, 0.2, "ui").unwrap();
            assert!((db(down / base) + 6.0).abs() < 1e-9, "even out {even}: -6 dB moves the click by -6 dB ({:.3} dB)", db(down / base));
            let (_, crash_now) = audio.one_shot(&crash, 0.85, "sfx").unwrap();
            assert_eq!(crash_now, crash_base, "even out {even}: the crash, with no level, plays at its default");
            sound_levels::set_level("sfx.button_click", 12.0);
            let (_, up) = audio.one_shot(&click, 0.2, "ui").unwrap();
            assert!((db(up / base) - 12.0).abs() < 1e-9, "even out {even}: +12 dB moves the click by +12 dB ({:.3} dB)", db(up / base));
            sound_levels::reset("sfx.button_click");
            let (_, back) = audio.one_shot(&click, 0.2, "ui").unwrap();
            assert_eq!(back, base, "even out {even}: Reset puts the click back to exactly its levelled amplitude");
        }
        // With levelling on the click's file (one of the quietest interface sounds) is raised, and
        // the person's level sits on top of that raise, not instead of it.
        audio.set_even_loudness(false);
        let (_, raw) = audio.one_shot(&click, 0.2, "ui").unwrap();
        audio.set_even_loudness(true);
        let (_, levelled) = audio.one_shot(&click, 0.2, "ui").unwrap();
        assert!(db(levelled / raw) > 3.0, "the click is raised by levelling: {:+.2} dB", db(levelled / raw));
    }

    /// PLAY PREVIEWS AT THE SLIDER'S LEVEL: `preview` (what Settings' Play calls) plays a sound at
    /// exactly what the game's own one-shot of it plays at, with the level just set, on its bus
    /// (the interface volume for the click, the music volume for a music-bus sound). RED first
    /// (2026-11-06): with `preview` playing the file at its authored volume alone, "the preview
    /// plays at the slider's level" failed (left: 0.2, right: 0.0250...).
    #[test]
    fn play_previews_at_the_sliders_level() {
        sound_levels::use_on_this_thread(BTreeMap::new());
        let mut audio = manager();
        let click = file("audio/ui/button_click.ogg");
        sound_levels::set_level("sfx.button_click", -9.5);
        let (_, game) = audio.one_shot(&click, 0.2, "ui").unwrap();
        let heard = audio.preview(&click, 0.2, "ui").expect("the preview plays");
        assert_eq!(heard, game, "the preview plays at the slider's level");
        sound_levels::set_level("sfx.button_click", 0.0);
        let at_zero = audio.preview(&click, 0.2, "ui").unwrap();
        assert!((db(heard / at_zero) + 9.5).abs() < 1e-9, "moving the slider moves the next Play: {:.3} dB", db(heard / at_zero));
        audio.set_music_volume(0.25);
        let on_music = audio.preview(&click, 0.2, "music").unwrap();
        assert!((on_music / at_zero - 0.25 / 0.8).abs() < 1e-12, "a music-bus sound previews at the music volume");
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
