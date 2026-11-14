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
//! [`SampleVoice::render`] allocates nothing (the audio thread's rule, music.md section 4.2): the
//! decoded frames are shared through an `Arc` made before the note starts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::definition::Zone;

/// One sample, decoded to 16-bit stereo frames (`humanity_music::instrument::SAMPLE_SCALE`; shared:
/// a score's zone holds the same frames, never a copy).
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSample {
    pub sample_rate: u32,
    pub frames: Arc<Vec<[i16; 2]>>,
}

/// Decode an audio file (Ogg Vorbis, FLAC, WAV: whatever kira and symphonia read).
pub fn decode_file(path: &Path) -> Result<DecodedSample, String> {
    let (sample_rate, frames) = humanity_music::decode::decode_file(path)?;
    Ok(DecodedSample { sample_rate, frames: Arc::new(frames) })
}

/// Decoded samples by path, kept while reuse is on.
#[derive(Debug, Default)]
pub struct SampleBank {
    kept: HashMap<PathBuf, Arc<DecodedSample>>,
    /// Tests set this instead of flipping the process-wide switch under other tests running at the
    /// same time (the garden mirror's way). None in the app: the Dev page's switch decides.
    pub(crate) reuse_override: Option<bool>,
    /// Decoded for the build in progress, several files at once ([`SampleBank::prefetch`]), each
    /// taken by the build's own request for it and the rest let go at [`SampleBank::end_build`]:
    /// never kept from one build to the next, so it is not reuse (BUG-297).
    prefetched: HashMap<PathBuf, Arc<DecodedSample>>,
}

impl SampleBank {
    pub fn new() -> Self {
        Self::default()
    }

    /// The decoded sample at `path`. With reuse on, decoded once and kept; with it off, the bank
    /// lets go of everything and decodes the file again.
    pub fn get(&mut self, path: &Path) -> Result<Arc<DecodedSample>, String> {
        let reuse = self.reuse_override.unwrap_or_else(humanity_core::reuse::enabled);
        if !reuse {
            self.kept.clear();
        } else if let Some(s) = self.kept.get(path) {
            return Ok(s.clone());
        }
        if let Some(s) = self.prefetched.remove(path) {
            if reuse {
                self.kept.insert(path.to_path_buf(), s.clone());
            }
            return Ok(s);
        }
        let s = Arc::new(decode_file(path)?);
        if reuse {
            self.kept.insert(path.to_path_buf(), s.clone());
        }
        Ok(s)
    }

    /// How many samples are kept decoded now.
    pub fn kept(&self) -> usize {
        self.kept.len()
    }

    /// The memory the kept samples hold, bytes (16-bit stereo frames).
    pub fn bytes(&self) -> usize {
        self.kept.values().map(|s| s.frames.len() * 4).sum()
    }

    /// Let go of every kept sample (what is installed changed, or no music has played for a while).
    pub fn clear(&mut self) {
        self.kept.clear();
        self.prefetched.clear();
    }

    /// Decode the files a build is about to ask for, on a few threads at once (a genre change's
    /// cold decode, BUG-297: electro swing's 51 files one after another took about four times as
    /// long), skipping what is kept (reuse on). A file that fails is left for [`SampleBank::get`]
    /// to decode again and report.
    pub fn prefetch(&mut self, paths: &[PathBuf]) {
        let reuse = self.reuse_override.unwrap_or_else(humanity_core::reuse::enabled);
        let mut want: Vec<PathBuf> = Vec::new();
        for p in paths {
            if !(reuse && self.kept.contains_key(p)) && !self.prefetched.contains_key(p) && !want.contains(p) {
                want.push(p.clone());
            }
        }
        let decoded = humanity_music::decode::decode_files(&want, humanity_music::decode::decode_threads());
        for (p, r) in want.into_iter().zip(decoded) {
            if let Ok((sample_rate, frames)) = r {
                self.prefetched.insert(p, Arc::new(DecodedSample { sample_rate, frames: Arc::new(frames) }));
            }
        }
    }

    /// The build is done: anything prefetched and not asked for goes.
    pub fn end_build(&mut self) {
        self.prefetched.clear();
    }
}

/// One note of a sampled instrument: a sample read at a rate (its pitch ratio times its sample rate
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
