//! How loud a sound IS, measured the way people hear it: ITU-R BS.1770 loudness (2026-11-06,
//! docs/design/loudness.md).
//!
//! The operator, 2026-11-06: "Are we able to normalize audio volumes between everything? Like
//! speaking is always at X volume. SFX are always X volume." Levelling a sound needs a number for
//! how loud it is, and the number the broadcast world agreed on is ITU-R BS.1770's (EBU R 128 and
//! every streaming service's loudness targets are written in it). The algorithm, from the
//! Recommendation itself (BS.1770-5, 11/2023, Annex 1), in four steps:
//!
//! 1. **K-weighting.** Each channel goes through two filters: a high shelf of about +4 dB above
//!    roughly 1.5 kHz (the head's effect on sound reaching the ears) and a gentle high-pass under
//!    about 40 Hz (the ear's weak bass). The Recommendation gives both as 48 kHz biquads (its
//!    Tables 1 and 2, copied below); for other rates it asks for "the same frequency response",
//!    which [`k_weighting`] gets by taking each 48 kHz filter back to the continuous-time filter it
//!    is a sampling of and sampling that again at the new rate (at 48 kHz this gives the table back
//!    exactly, a test below).
//! 2. **Mean square** of each filtered channel over a stretch of time.
//! 3. **Channel sum**, each channel weighted (left, right and centre 1.0, the surrounds 1.41),
//!    then `-0.691 + 10 log10(sum)`: LUFS ("LKFS" in the ITU's own word). The -0.691 cancels the
//!    filter's gain at 997 Hz.
//! 4. **Gating** (the integrated loudness of a whole programme): the stretch is cut into 400 ms
//!    blocks, each starting 100 ms after the last (75% overlap); blocks under -70 LUFS are dropped
//!    (silence), then blocks more than 10 LU under the loudness of what is left (pauses, quiet
//!    tails), and the loudness of the rest is the answer.
//!
//! EBU Tech 3341 ("EBU Mode", V4, 11/2023) adds the two windows a live meter shows: Momentary
//! (the last 400 ms) and Short-term (the last 3 s), neither gated, and a test set; the tests beside
//! this file synthesise its tones (cases 1 to 6, 9, 10, 12, 13 and the true-peak cases 15 to 19)
//! and check the readings within its tolerances (a stereo 1 kHz sine at -23 dBFS per channel reads
//! -23.0 LUFS: the -3 dB of a sine's mean square and the +3 dB of two channels cancel).
//!
//! **True peak** (BS.1770-5 Annex 2): the highest point of the waveform between the samples, found
//! by 4x oversampling with the Annex's own 48-tap interpolating filter. A level that pushes a sound
//! louder must stop below a true-peak ceiling, or the sound card clips it.
//!
//! Everything here is plain arithmetic, no audio device and no allocation per sample, so it is
//! ungated (the relay and the browser build compile it too). The meter takes 10 ms steps: the
//! Momentary and Short-term readings move every 10 ms (Tech 3341 asks a live meter for 10 Hz;
//! the finer step lets a file be measured within 0.06 LU of the best window placement).

/// Blocks quieter than this are silence, not part of a programme (BS.1770-5, the absolute gate).
pub const ABSOLUTE_GATE_LUFS: f64 = -70.0;
/// Blocks this far under the absolute-gated loudness are pauses (BS.1770-5, the relative gate).
pub const RELATIVE_GATE_LU: f64 = -10.0;
/// One measuring step: 10 ms. Momentary and Short-term readings move on it.
pub const STEP_S: f64 = 0.01;
/// The Momentary window and the gating block: 400 ms, 40 steps.
const MOMENTARY_STEPS: usize = 40;
/// The Short-term window: 3 s, 300 steps.
const SHORT_TERM_STEPS: usize = 300;
/// A gating block starts every 100 ms (75% overlap of 400 ms): 10 steps.
const BLOCK_HOP_STEPS: u64 = 10;
/// A sound this long or shorter is a one-shot (a click, a footstep, a door), measured by its
/// loudest Momentary window; a longer one (a loop, a song, a voice, a video) by its integrated
/// loudness. Our choice, not the standard's (BS.1770 is for programmes; a 0.2 s click has no
/// complete gating block at all): docs/design/loudness.md, "Measuring a short sound".
pub const ONE_SHOT_MAX_S: f64 = 3.0;

/// A second-order filter: `y = b0 x + b1 x1 + b2 x2 - a1 y1 - a2 y2` (a0 = 1), the form both of
/// BS.1770's tables are written in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Biquad {
    pub b: [f64; 3],
    pub a: [f64; 2],
}

/// BS.1770-5 Table 1: the K-weighting's first stage (the head's high shelf) at 48 kHz.
pub const STAGE1_48K: Biquad = Biquad {
    b: [1.53512485958697, -2.69169618940638, 1.19839281085285],
    a: [-1.69065929318241, 0.73248077421585],
};
/// BS.1770-5 Table 2: the K-weighting's second stage (the RLB high-pass) at 48 kHz.
pub const STAGE2_48K: Biquad = Biquad { b: [1.0, -2.0, 1.0], a: [-1.99004745483398, 0.99007225036621] };

/// Where each K-weighting stage is moved between rates without bending: the middle of its slope
/// (stage 1's shelf is half way up, +2 dB, at about 1.5 kHz; stage 2 turns down at about 40 Hz).
/// Our choice: BS.1770 asks only for "the same frequency response".
const STAGE1_EXACT_HZ: f64 = 1500.0;
const STAGE2_EXACT_HZ: f64 = 40.0;

impl Biquad {
    /// The same filter for `to_rate`, from this one at `from_rate`: back through the bilinear
    /// transform to the continuous-time filter (s-domain polynomials), then forward again at the new
    /// rate. At `to_rate == from_rate` it is this filter, to rounding. The bilinear transform bends
    /// the frequency scale (tan), differently at each rate; both transforms are scaled to bend it the
    /// same at `exact_hz` (the textbook "pre-warping"), so the two rates agree exactly there and
    /// closely around it: within 0.01 dB from 20 Hz to 10 kHz at 32, 44.1 and 96 kHz for K-weighting
    /// (a test measures it; plain bilinear was 0.016 dB off at 1.5 kHz at 32 kHz).
    pub fn at_rate(self, from_rate: f64, to_rate: f64, exact_hz: f64) -> Biquad {
        let [b0, b1, b2] = self.b;
        let [a1, a2] = self.a;
        let warp = |rate: f64| {
            let w = std::f64::consts::PI * exact_hz / rate;
            std::f64::consts::TAU * exact_hz / w.tan()
        };
        let k = warp(from_rate);
        // z^-1 = (k - s) / (k + s), both sides times (k + s)^2: N(s) = n0 + n1 s + n2 s^2.
        let n = [k * k * (b0 + b1 + b2), 2.0 * k * (b0 - b2), b0 - b1 + b2];
        let d = [k * k * (1.0 + a1 + a2), 2.0 * k * (1.0 - a2), 1.0 - a1 + a2];
        // s = K (1 - w) / (1 + w), w = z^-1, both sides times (1 + w)^2.
        let kk = warp(to_rate);
        let fwd = |p: [f64; 3]| [p[0] + p[1] * kk + p[2] * kk * kk, 2.0 * p[0] - 2.0 * p[2] * kk * kk, p[0] - p[1] * kk + p[2] * kk * kk];
        let (nb, da) = (fwd(n), fwd(d));
        let a0 = da[0];
        Biquad { b: [nb[0] / a0, nb[1] / a0, nb[2] / a0], a: [da[1] / a0, da[2] / a0] }
    }

    /// The filter's gain at `freq` Hz for `rate`, in dB (a test compares rates with it).
    pub fn gain_db(self, freq: f64, rate: f64) -> f64 {
        let w = std::f64::consts::TAU * freq / rate;
        let (c1, s1, c2, s2) = (w.cos(), -w.sin(), (2.0 * w).cos(), -(2.0 * w).sin());
        let num = (self.b[0] + self.b[1] * c1 + self.b[2] * c2, self.b[1] * s1 + self.b[2] * s2);
        let den = (1.0 + self.a[0] * c1 + self.a[1] * c2, self.a[0] * s1 + self.a[1] * s2);
        10.0 * ((num.0 * num.0 + num.1 * num.1) / (den.0 * den.0 + den.1 * den.1)).log10()
    }
}

/// The two K-weighting stages for a sample rate (BS.1770-5's 48 kHz tables, moved to `rate`).
pub fn k_weighting(rate: f64) -> [Biquad; 2] {
    [STAGE1_48K.at_rate(48_000.0, rate, STAGE1_EXACT_HZ), STAGE2_48K.at_rate(48_000.0, rate, STAGE2_EXACT_HZ)]
}

/// A biquad's two remembered values (transposed direct form II, in f64: a 30 Hz pole at 48 kHz sits
/// 0.004 from the unit circle, where f32 would drift).
#[derive(Clone, Copy, Debug, Default)]
struct BiquadState {
    z1: f64,
    z2: f64,
}

impl BiquadState {
    #[inline]
    fn run(&mut self, c: &Biquad, x: f64) -> f64 {
        let y = c.b[0] * x + self.z1;
        self.z1 = c.b[1] * x - c.a[0] * y + self.z2;
        self.z2 = c.b[2] * x - c.a[1] * y;
        y
    }
}

/// Loudness from a channel-weighted mean square: `-0.691 + 10 log10(z)` (BS.1770-5, equation 2).
/// Silence is minus infinity.
pub fn lufs(mean_square: f64) -> f64 {
    if mean_square > 0.0 {
        -0.691 + 10.0 * mean_square.log10()
    } else {
        f64::NEG_INFINITY
    }
}

/// The mean square that reads `lufs` (the inverse of [`lufs`]).
fn mean_square_of(lufs: f64) -> f64 {
    10f64.powf((lufs + 0.691) / 10.0)
}

/// BS.1770-5 Annex 2: the 48-tap, 4-phase interpolating filter for true-peak metering, as printed
/// (phase k, tap m). Each phase makes one of the four points per input sample.
const TRUE_PEAK_PHASES: [[f64; 12]; 4] = [
    [
        0.0017089843750, 0.0109863281250, -0.0196533203125, 0.0332031250000, -0.0594482421875, 0.1373291015625,
        0.9721679687500, -0.1022949218750, 0.0476074218750, -0.0266113281250, 0.0148925781250, -0.0083007812500,
    ],
    [
        -0.0291748046875, 0.0292968750000, -0.0517578125000, 0.0891113281250, -0.1665039062500, 0.4650878906250,
        0.7797851562500, -0.2003173828125, 0.1015625000000, -0.0582275390625, 0.0330810546875, -0.0189208984375,
    ],
    [
        -0.0189208984375, 0.0330810546875, -0.0582275390625, 0.1015625000000, -0.2003173828125, 0.7797851562500,
        0.4650878906250, -0.1665039062500, 0.0891113281250, -0.0517578125000, 0.0292968750000, -0.0291748046875,
    ],
    [
        -0.0083007812500, 0.0148925781250, -0.0266113281250, 0.0476074218750, -0.1022949218750, 0.9721679687500,
        0.1373291015625, -0.0594482421875, 0.0332031250000, -0.0196533203125, 0.0109863281250, 0.0017089843750,
    ],
];

/// One channel's last 12 samples, for the interpolating filter.
#[derive(Clone, Copy, Debug, Default)]
struct PeakState {
    hist: [f64; 12],
    at: usize,
}

impl PeakState {
    /// Take one sample; the largest of its own size and the four points interpolated around it.
    #[inline]
    fn run(&mut self, x: f64) -> f64 {
        self.at = (self.at + 11) % 12;
        self.hist[self.at] = x;
        let mut most = x.abs();
        for phase in &TRUE_PEAK_PHASES {
            let mut y = 0.0;
            for (m, h) in phase.iter().enumerate() {
                y += h * self.hist[(self.at + m) % 12];
            }
            most = most.max(y.abs());
        }
        most
    }
}

/// A BS.1770 / EBU Mode meter: push samples in, read Momentary, Short-term, Integrated and the
/// true peak out. One per sound (or per playing source); it keeps the last 3 s of 10 ms steps and
/// one number per 100 ms of everything it has heard (for the integrated loudness's gating).
#[derive(Clone, Debug)]
pub struct LoudnessMeter {
    rate: u32,
    weights: Vec<f64>,
    k: [Biquad; 2],
    filters: Vec<[BiquadState; 2]>,
    peaks: Vec<PeakState>,
    /// Samples per channel in one 10 ms step.
    step_len: usize,
    /// Samples per channel into the step being filled, and its channel-weighted sum of squares.
    step_fill: usize,
    step_sum: f64,
    /// The last (up to 300) steps' sums, oldest first.
    recent: std::collections::VecDeque<f64>,
    steps_done: u64,
    /// The mean square of every complete 400 ms gating block so far.
    blocks: Vec<f64>,
    momentary: Option<f64>,
    short_term: Option<f64>,
    max_momentary: Option<f64>,
    max_short_term: Option<f64>,
    true_peak: f64,
}

impl LoudnessMeter {
    /// A meter for `channels` channels at `rate` Hz, weighted as BS.1770-5 Table 3 says for the
    /// usual layouts: mono and stereo all 1.0; five channels in L, R, C, Ls, Rs order with the
    /// surrounds at 1.41; any other count all 1.0 (an LFE channel is the caller's to leave out).
    pub fn new(rate: u32, channels: usize) -> Self {
        let weights = if channels == 5 { vec![1.0, 1.0, 1.0, 1.41, 1.41] } else { vec![1.0; channels.max(1)] };
        Self::with_weights(rate, &weights)
    }

    /// A meter with these channel weights, one per channel in interleaved order.
    pub fn with_weights(rate: u32, weights: &[f64]) -> Self {
        let rate = rate.max(1);
        let n = weights.len().max(1);
        LoudnessMeter {
            rate,
            weights: if weights.is_empty() { vec![1.0] } else { weights.to_vec() },
            k: k_weighting(rate as f64),
            filters: vec![[BiquadState::default(); 2]; n],
            peaks: vec![PeakState::default(); n],
            step_len: ((rate as f64 * STEP_S).round() as usize).max(1),
            step_fill: 0,
            step_sum: 0.0,
            recent: std::collections::VecDeque::with_capacity(SHORT_TERM_STEPS + 1),
            steps_done: 0,
            blocks: Vec::new(),
            momentary: None,
            short_term: None,
            max_momentary: None,
            max_short_term: None,
            true_peak: 0.0,
        }
    }

    /// The channel count it measures.
    pub fn channels(&self) -> usize {
        self.weights.len()
    }

    /// The sample rate it measures.
    pub fn rate(&self) -> u32 {
        self.rate
    }

    /// One frame: one sample per channel (missing channels count as silence, extra ones are
    /// ignored).
    #[inline]
    pub fn push_frame(&mut self, frame: &[f32]) {
        let mut sum = 0.0;
        for i in 0..self.weights.len() {
            let x = frame.get(i).copied().unwrap_or(0.0) as f64;
            let [s1, s2] = &mut self.filters[i];
            let y = s2.run(&self.k[1], s1.run(&self.k[0], x));
            sum += self.weights[i] * y * y;
            self.true_peak = self.true_peak.max(self.peaks[i].run(x));
        }
        self.step_sum += sum;
        self.step_fill += 1;
        if self.step_fill == self.step_len {
            self.end_step();
        }
    }

    /// Interleaved samples, whole frames (a trailing part frame is ignored).
    pub fn push_interleaved(&mut self, samples: &[f32]) {
        let n = self.weights.len();
        for frame in samples.chunks_exact(n) {
            self.push_frame(frame);
        }
    }

    /// `frames` frames of silence (a one-shot's padding to a full window).
    pub fn push_silence(&mut self, frames: usize) {
        let zeros = vec![0.0f32; self.weights.len()];
        for _ in 0..frames {
            self.push_frame(&zeros);
        }
    }

    /// The end of a sound: its last part step filled with silence, and silence after it until at
    /// least one 400 ms window is complete, so a click shorter than a window still has a Momentary
    /// reading (the rest of its window is the quiet after it).
    pub fn finish(&mut self) {
        if self.step_fill > 0 {
            self.push_silence(self.step_len - self.step_fill);
        }
        while self.steps_done < MOMENTARY_STEPS as u64 {
            self.push_silence(self.step_len);
        }
    }

    fn end_step(&mut self) {
        self.recent.push_back(self.step_sum);
        if self.recent.len() > SHORT_TERM_STEPS {
            self.recent.pop_front();
        }
        self.steps_done += 1;
        self.step_sum = 0.0;
        self.step_fill = 0;
        let per = self.step_len as f64;
        if self.recent.len() >= MOMENTARY_STEPS {
            let z = self.recent.iter().rev().take(MOMENTARY_STEPS).sum::<f64>() / (MOMENTARY_STEPS as f64 * per);
            self.momentary = Some(z);
            self.max_momentary = Some(self.max_momentary.map_or(z, |m| m.max(z)));
            if (self.steps_done - MOMENTARY_STEPS as u64) % BLOCK_HOP_STEPS == 0 {
                self.blocks.push(z);
            }
        }
        if self.recent.len() >= SHORT_TERM_STEPS {
            let z = self.recent.iter().sum::<f64>() / (SHORT_TERM_STEPS as f64 * per);
            self.short_term = Some(z);
            self.max_short_term = Some(self.max_short_term.map_or(z, |m| m.max(z)));
        }
    }

    /// The last 400 ms, LUFS (None before 400 ms have been heard).
    pub fn momentary(&self) -> Option<f64> {
        self.momentary.map(lufs)
    }

    /// The last 3 s, LUFS (None before 3 s have been heard).
    pub fn short_term(&self) -> Option<f64> {
        self.short_term.map(lufs)
    }

    /// The loudest 400 ms so far, LUFS.
    pub fn max_momentary(&self) -> Option<f64> {
        self.max_momentary.map(lufs)
    }

    /// The loudest 3 s so far, LUFS.
    pub fn max_short_term(&self) -> Option<f64> {
        self.max_short_term.map(lufs)
    }

    /// Everything heard so far, gated (BS.1770-5 equations 5 to 7), LUFS. None before the first
    /// 400 ms block, or when every block is silence.
    pub fn integrated(&self) -> Option<f64> {
        let abs_gate = mean_square_of(ABSOLUTE_GATE_LUFS);
        let loud: Vec<f64> = self.blocks.iter().copied().filter(|&z| z > abs_gate).collect();
        if loud.is_empty() {
            return None;
        }
        let rel_gate = mean_square_of(lufs(loud.iter().sum::<f64>() / loud.len() as f64) + RELATIVE_GATE_LU);
        let kept: Vec<f64> = loud.into_iter().filter(|&z| z > rel_gate).collect();
        if kept.is_empty() {
            return None;
        }
        Some(lufs(kept.iter().sum::<f64>() / kept.len() as f64))
    }

    /// The highest true peak so far, dBTP (minus infinity for silence).
    pub fn true_peak_dbtp(&self) -> f64 {
        if self.true_peak > 0.0 {
            20.0 * self.true_peak.log10()
        } else {
            f64::NEG_INFINITY
        }
    }
}

/// Which number a sound's loudness is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// Its loudest 400 ms (a one-shot, [`ONE_SHOT_MAX_S`] or shorter).
    MaxMomentary,
    /// Its integrated loudness, gated (a longer sound).
    Integrated,
}

impl Measure {
    /// The word the measured-loudness data file writes.
    pub fn as_str(self) -> &'static str {
        match self {
            Measure::MaxMomentary => "max_momentary",
            Measure::Integrated => "integrated",
        }
    }

    /// Read back from the data file.
    pub fn parse(s: &str) -> Option<Measure> {
        match s {
            "max_momentary" => Some(Measure::MaxMomentary),
            "integrated" => Some(Measure::Integrated),
            _ => None,
        }
    }
}

/// How loud one sound is: the number that levels it, which number that is, its true peak (the
/// ceiling a raise must stay under) and its length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundLoudness {
    /// LUFS; minus infinity for a silent sound (which is never raised).
    pub lufs: f64,
    pub measure: Measure,
    pub true_peak_dbtp: f64,
    pub seconds: f64,
}

/// Measure a whole sound: `samples` interleaved, `channels` per frame, at `rate`. A one-shot
/// ([`ONE_SHOT_MAX_S`] or shorter) by its loudest 400 ms, padded with silence to one full window;
/// a longer sound by its integrated loudness. Measure the samples as they PLAY (a mono file the
/// mixer sends to both speakers is two channels here), so every sound is weighed the same way.
pub fn measure_sound(rate: u32, channels: usize, samples: &[f32]) -> SoundLoudness {
    let channels = channels.max(1);
    let seconds = (samples.len() / channels) as f64 / rate.max(1) as f64;
    let mut meter = LoudnessMeter::new(rate, channels);
    meter.push_interleaved(samples);
    meter.finish();
    let (lufs, measure) = if seconds <= ONE_SHOT_MAX_S {
        (meter.max_momentary(), Measure::MaxMomentary)
    } else {
        (meter.integrated(), Measure::Integrated)
    };
    SoundLoudness { lufs: lufs.unwrap_or(f64::NEG_INFINITY), measure, true_peak_dbtp: meter.true_peak_dbtp(), seconds }
}

#[cfg(test)]
#[path = "loudness_tests.rs"]
mod tests;
