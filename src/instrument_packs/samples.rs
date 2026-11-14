//! Decoded sample audio, and a voice that plays one sample at a rate.
//!
//! Decoding an Ogg Vorbis file takes a few milliseconds, so the bank keeps each one decoded while
//! the Dev page's "turn off all reuse" switch is on. It is THE path the live score's recordings are
//! decoded through (engine/music.rs's build, on its worker thread, BUG-267): with reuse on, a
//! recording decoded for one score is handed to the next (a new piece, an install) without decoding
//! it again; the music empties the bank when an install or removal changes what is installed (a pack
//! updated in place writes new files at the same paths) and when no music has played for a while.
//! With the switch off the bank keeps nothing: every build decodes every file again, the way the
//! audio manager's sounds do since BUG-257's fix (CLAUDE.md, the reuse rule).
//!
//! **Long recordings are streamed** (2026-11-14, docs/design/music.md section 23, "Memory"): a
//! recording longer than `data/music/streaming.ron`'s `stream_longer_than_s` that a note plays
//! straight through (no loop, a part that does not sustain) is decoded only as far as its attack
//! ([`SampleBank::get_attack`]), and the rest is read from the file while a note plays, through the
//! bank's [`humanity_music::stream::StreamPool`] and its worker thread ([`SampleBank::stream_for`]).
//! The attacks obey the reuse switch exactly as whole recordings do; the rings hold no recording
//! from one note to the next, so they are not reuse.
//!
//! [`SampleVoice::render`] allocates nothing (the audio thread's rule, music.md section 4.2): the
//! decoded frames are shared through an `Arc` made before the note starts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use humanity_music::stream::{FileOpener, SourceOpener, StreamPool, StreamSettings, ZoneStream};

use super::definition::Zone;

/// One sample, decoded to 16-bit stereo frames (`humanity_music::instrument::SAMPLE_SCALE`; shared:
/// a score's zone holds the same frames, never a copy).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSample {
    pub sample_rate: u32,
    pub frames: Arc<Vec<[i16; 2]>>,
    /// False: `frames` is the attack of a longer recording, whose rest is streamed.
    pub whole: bool,
}

/// Decode an audio file (Ogg Vorbis, FLAC, WAV: whatever kira and symphonia read).
pub fn decode_file(path: &Path) -> Result<DecodedSample, String> {
    let (sample_rate, frames) = humanity_music::decode::decode_file(path)?;
    Ok(DecodedSample { sample_rate, frames: Arc::new(frames), whole: true })
}

/// A recording to be streamed: whole when it is no longer than `settings.stream_longer_than_s`,
/// else only its first `settings.attack_s`.
pub fn decode_attack(path: &Path, settings: &StreamSettings) -> Result<DecodedSample, String> {
    let (sample_rate, frames, whole) = humanity_music::decode::decode_attack(path, settings.stream_longer_than_s, settings.attack_s)?;
    Ok(DecodedSample { sample_rate, frames: Arc::new(frames), whole })
}

/// `data/music/streaming.ron`, read now (every build: nothing about it is kept). Its default, with
/// the reason, when it is missing or does not parse.
pub fn stream_settings(data_dir: &Path) -> (StreamSettings, Option<String>) {
    let path = data_dir.join("music").join("streaming.ron");
    match std::fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| StreamSettings::parse(&t)) {
        Ok(s) => (s, None),
        Err(e) => (StreamSettings::default(), Some(format!("{}: {e}; long recordings stream as the default says", path.display()))),
    }
}

/// The streaming the bank's notes read through: the rings, their worker, and the files they open.
struct Streamer {
    pool: Arc<StreamPool>,
    files: Arc<FileOpener>,
}

/// Decoded samples by path, kept while reuse is on.
#[derive(Default)]
pub struct SampleBank {
    /// reuse: site: decoded recordings kept between score builds; with the switch off the bank
    /// keeps nothing, so every build decodes every recording again.
    kept: HashMap<PathBuf, Arc<DecodedSample>>,
    /// Attacks (or whole short recordings) for streaming, by path, kept while reuse is on.
    kept_attacks: HashMap<PathBuf, Arc<DecodedSample>>,
    /// Tests set this instead of flipping the process-wide switch under other tests running at the
    /// same time (the garden mirror's way). None in the app: the Dev page's switch decides.
    pub(crate) reuse_override: Option<bool>,
    /// Decoded for the build in progress, several files at once ([`SampleBank::prefetch`]), each
    /// taken by the build's own request for it and the rest let go at [`SampleBank::end_build`]:
    /// never kept from one build to the next, so it is not reuse (BUG-297). Keyed by path and
    /// whether it is an attack.
    prefetched: HashMap<(PathBuf, bool), Arc<DecodedSample>>,
    /// How long recordings stream (`data/music/streaming.ron`, given each build by
    /// [`SampleBank::configure`]).
    settings: StreamSettings,
    streamer: Option<Streamer>,
}

impl std::fmt::Debug for SampleBank {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SampleBank({} kept, {} attacks kept)", self.kept.len(), self.kept_attacks.len())
    }
}

impl SampleBank {
    pub fn new() -> Self {
        Self::default()
    }

    fn reuse(&self) -> bool {
        self.reuse_override.unwrap_or_else(humanity_core::reuse::enabled)
    }

    /// How long recordings stream, as the build read it. A change to the attack lets go of the
    /// attacks kept; a change to the rings makes new ones (the old go with the last score using
    /// them).
    pub fn configure(&mut self, settings: StreamSettings) {
        if settings == self.settings {
            return;
        }
        if (settings.attack_s, settings.stream_longer_than_s) != (self.settings.attack_s, self.settings.stream_longer_than_s) {
            self.kept_attacks.clear();
        }
        let rings = |s: &StreamSettings| (s.streams, s.ring_s.to_bits(), s.fade_ms.to_bits(), s.poll_ms.to_bits());
        if rings(&settings) != rings(&self.settings) {
            self.streamer = None;
        }
        self.settings = settings;
    }

    /// How long recordings stream now.
    pub fn settings(&self) -> &StreamSettings {
        &self.settings
    }

    /// The decoded sample at `path`, whole. With reuse on, decoded once and kept; with it off, the
    /// bank lets go of everything and decodes the file again.
    pub fn get(&mut self, path: &Path) -> Result<Arc<DecodedSample>, String> {
        self.fetch(path, false)
    }

    /// The sample at `path` for a note that plays it straight through: whole when it is short (or
    /// streaming is off), else its attack, the rest to be streamed ([`SampleBank::stream_for`]).
    /// Kept under the reuse switch as [`SampleBank::get`] keeps whole ones.
    pub fn get_attack(&mut self, path: &Path) -> Result<Arc<DecodedSample>, String> {
        if !self.settings.enabled {
            return self.get(path);
        }
        self.fetch(path, true)
    }

    fn fetch(&mut self, path: &Path, attack: bool) -> Result<Arc<DecodedSample>, String> {
        let reuse = self.reuse();
        if !reuse {
            self.kept.clear();
            self.kept_attacks.clear();
        } else if let Some(s) = (if attack { &self.kept_attacks } else { &self.kept }).get(path) {
            return Ok(s.clone());
        }
        let s = match self.prefetched.remove(&(path.to_path_buf(), attack)) {
            Some(s) => s,
            None if attack => Arc::new(decode_attack(path, &self.settings)?),
            None => Arc::new(decode_file(path)?),
        };
        if reuse {
            let kept = if attack { &mut self.kept_attacks } else { &mut self.kept };
            kept.insert(path.to_path_buf(), s.clone());
        }
        Ok(s)
    }

    /// The stream a note of the recording at `path` reads past its attack: the bank's rings (made,
    /// with their worker thread, the first time) and the file registered there. None when the
    /// worker could not start (the caller decodes the recording whole instead).
    pub fn stream_for(&mut self, path: &Path, frames: Option<u64>) -> Option<ZoneStream> {
        if self.streamer.is_none() {
            let pool = StreamPool::new(&self.settings);
            let files = Arc::new(FileOpener::default());
            if let Err(e) = humanity_music::stream::start_worker(&pool, files.clone() as Arc<dyn SourceOpener>) {
                log::warn!("[Music] the streaming thread did not start, so long recordings are decoded whole: {e}");
                return None;
            }
            self.streamer = Some(Streamer { pool, files });
        }
        let s = self.streamer.as_ref()?;
        Some(ZoneStream { pool: s.pool.clone(), source: s.files.register(path), frames })
    }

    /// The rings' memory, bytes (0 before the first streamed recording).
    pub fn stream_memory(&self) -> usize {
        self.streamer.as_ref().map_or(0, |s| s.pool.memory())
    }

    /// How many notes stream now, and how many rings there are (0, 0 before the first).
    pub fn streams(&self) -> (usize, usize) {
        self.streamer.as_ref().map_or((0, 0), |s| (s.pool.in_use(), s.pool.streams()))
    }

    /// How many samples are kept decoded now.
    pub fn kept(&self) -> usize {
        self.kept.len() + self.kept_attacks.len()
    }

    /// The memory the kept samples hold, bytes (16-bit stereo frames).
    pub fn bytes(&self) -> usize {
        self.kept.values().chain(self.kept_attacks.values()).map(|s| s.frames.len() * 4).sum()
    }

    /// Let go of every kept sample (what is installed changed, or no music has played for a while).
    pub fn clear(&mut self) {
        self.kept.clear();
        self.kept_attacks.clear();
        self.prefetched.clear();
        // The rings too: their worker thread ends once the last playing note gives its ring back,
        // instead of waking every few milliseconds with the music off (the batch critic,
        // 2026-11-14). A later streamed note makes a fresh pool.
        self.streamer = None;
    }

    /// Decode the files a build is about to ask for, on a few threads at once (a genre change's
    /// cold decode, BUG-297: electro swing's 51 files one after another took about four times as
    /// long), skipping what is kept (reuse on): `whole` as [`SampleBank::get`] gives them, `attacks`
    /// as [`SampleBank::get_attack`] does. A file that fails is left for the request to decode
    /// again and report.
    pub fn prefetch(&mut self, whole: &[PathBuf], attacks: &[PathBuf]) {
        let reuse = self.reuse();
        let attacks_on = self.settings.enabled;
        let mut want: Vec<(PathBuf, bool)> = Vec::new();
        for (p, attack) in whole.iter().map(|p| (p, false)).chain(attacks.iter().map(|p| (p, attacks_on))) {
            let kept = if attack { &self.kept_attacks } else { &self.kept };
            let key = (p.clone(), attack);
            if !(reuse && kept.contains_key(p)) && !self.prefetched.contains_key(&key) && !want.contains(&key) {
                want.push(key);
            }
        }
        let settings = self.settings.clone();
        let decoded = humanity_music::decode::each(&want, humanity_music::decode::decode_threads(), |(p, attack)| if *attack { decode_attack(p, &settings) } else { decode_file(p) });
        for (key, r) in want.into_iter().zip(decoded) {
            if let Ok(s) = r {
                self.prefetched.insert(key, Arc::new(s));
            }
        }
    }

    /// The build is done: anything prefetched and not asked for goes.
    pub fn end_build(&mut self) {
        self.prefetched.clear();
    }
}

// One note of a sampled instrument: a sample read at a rate (its pitch ratio times its sample rate
/// over the output's), with linear interpolation, looping between the zone's loop points if it has
/// them, released with a linear fade.
#[derive(Debug, Clone)]
pub struct SampleVoice {
    sample: Arc<DecodedSample>,
    pos: f64,
    step: f64,
    gain: f32,
    looping: Option<(f64, f64)>,
    release_left: Option<u32>,
    release_len: u32,
    finished: bool,
}

impl SampleVoice {
    /// Start `zone`'s `sample` at `rate` (from `Sampler::playback_rate`) for an output at
    /// `output_rate` frames per second.
    pub fn start(sample: Arc<DecodedSample>, zone: &Zone, rate: f64, output_rate: u32, gain: f32) -> Self {
        let step = rate * sample.sample_rate as f64 / output_rate as f64;
        let looping = match (zone.loop_start, zone.loop_end) {
            (Some(s), Some(e)) if e > s && (e as usize) < sample.frames.len() => Some((s as f64, e as f64)),
            _ => None,
        };
        Self { sample, pos: 0.0, step, gain, looping, release_left: None, release_len: 0, finished: false }
    }

    /// Let go of the note: it fades out over `seconds`.
    pub fn release(&mut self, seconds: f32, output_rate: u32) {
        let len = ((seconds.max(0.0) * output_rate as f32) as u32).max(1);
        self.release_len = len;
        self.release_left = Some(len);
    }

    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Add this voice into `out`.
    pub fn render(&mut self, out: &mut [[f32; 2]]) {
        let frames = &self.sample.frames;
        for o in out.iter_mut() {
            if self.finished {
                return;
            }
            let i = self.pos as usize;
            if i + 1 >= frames.len() {
                self.finished = true;
                return;
            }
            let t = (self.pos - i as f64) as f32;
            let sc = humanity_music::instrument::SAMPLE_SCALE;
            let a = [frames[i][0] as f32 * sc, frames[i][1] as f32 * sc];
            let b = [frames[i + 1][0] as f32 * sc, frames[i + 1][1] as f32 * sc];
            let mut g = self.gain;
            if let Some(left) = self.release_left.as_mut() {
                if *left == 0 {
                    self.finished = true;
                    return;
                }
                g *= *left as f32 / self.release_len as f32;
                *left -= 1;
            }
            o[0] += (a[0] + (b[0] - a[0]) * t) * g;
            o[1] += (a[1] + (b[1] - a[1]) * t) * g;
            self.pos += self.step;
            if let Some((s, e)) = self.looping {
                if self.release_left.is_none() && self.pos >= e {
                    self.pos = s + (self.pos - e);
                }
            }
        }
    }
}
