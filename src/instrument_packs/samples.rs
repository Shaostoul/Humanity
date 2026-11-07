//! Decoded sample audio, and a voice that plays one sample at a rate.
//!
//! Decoding an Ogg Vorbis file takes a few milliseconds, so the bank keeps each one decoded while
//! the Dev page's "turn off all reuse" switch is on (CLAUDE.md, the reuse rule: kept data cannot
//! go stale here, because an installed sample file never changes; removing its pack removes the
//! instrument). With the switch off the bank keeps nothing: every request decodes its file again,
//! the way the audio manager's sounds do since BUG-257's fix.
//!
//! [`SampleVoice::render`] allocates nothing (the audio thread's rule, music.md section 4.2): the
//! decoded frames are shared through an `Arc` made before the note starts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::definition::Zone;

/// One sample, decoded to stereo frames.
#[derive(Debug, Clone, PartialEq)]
pub struct DecodedSample {
    pub sample_rate: u32,
    pub frames: Vec<[f32; 2]>,
}

/// Decode an audio file (Ogg Vorbis, FLAC, WAV: whatever kira and symphonia read).
pub fn decode_file(path: &Path) -> Result<DecodedSample, String> {
    let data = kira::sound::static_sound::StaticSoundData::from_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(DecodedSample { sample_rate: data.sample_rate, frames: data.frames.iter().map(|f| [f.left, f.right]).collect() })
}

/// Decoded samples by path, kept while reuse is on.
#[derive(Debug, Default)]
pub struct SampleBank {
    kept: HashMap<PathBuf, Arc<DecodedSample>>,
    /// Tests set this instead of flipping the process-wide switch under other tests running at the
    /// same time (the garden mirror's way). None in the app: the Dev page's switch decides.
    pub(crate) reuse_override: Option<bool>,
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
            let a = frames[i];
            let b = frames[i + 1];
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
