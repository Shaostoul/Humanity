//! OPERATOR FM: the voice of the RETRO FM sound style (docs/design/music-page.md section 15a and
//! section 25, rung 3a; the operator, 2026-11-20, chose retro FM as the first style).
//!
//! The method is John Chowning's, "The Synthesis of Complex Audio Spectra by Means of Frequency
//! Modulation" (Journal of the Audio Engineering Society 21(7), 1973): a sine (the CARRIER) whose
//! phase another sine (the MODULATOR) moves, `sin(2 pi fc t + I sin(2 pi fm t))`, so one pair of
//! oscillators makes a whole spectrum of partials at `fc +- k fm`, its brightness set by the index
//! `I` and its harmony by the ratio `fm / fc`. An envelope on the index makes a note brighter at its
//! strike and mellower as it rings, the way a real instrument's tone changes.
//!
//! Up to four OPERATORS (sine oscillators with their own envelope), wired as data: each names the
//! earlier operators that modulate it and whether it is heard; the first may modulate itself
//! (FEEDBACK: its own output, averaged over its last two samples so it stays stable, moves its phase,
//! turning a sine into a saw-like and, pushed far, a noisy tone, which is how such a voice makes a
//! snare or a hi-hat). The early-1990s computer sound cards' FM chips worked this way, with two or four
//! operators, four waveforms (a sine, a half sine, a rectified sine and a pulsed quarter sine), short
//! envelopes, a fixed vibrato and tremolo, and key scaling (higher notes less bright); this is our own
//! voice in that spirit, every number in its data file, no chip emulated and nothing sampled.
//!
//! Every pitch from `tuning::note_hz` (the voice is given the note's frequency at the person's A),
//! so retro FM plays at A = 432 by default and at 440 when switched, like everything else. Nothing
//! here allocates: the state is a fixed array inside the voice.

use std::f32::consts::TAU;

use serde::{Deserialize, Serialize};

use crate::dsp::{fast_floor, fast_sin, Env};
use crate::instrument::EnvSpec;

/// Operators at most.
pub const MAX_OPS: usize = 4;

/// An operator's waveform: the sine, and the three shapes the early FM chips added by cutting it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpWave {
    #[default]
    Sine,
    /// The positive half of each cycle, silent for the rest (a hollow, reedy tone).
    HalfSine,
    /// The sine with its negative half turned up (an octave of brightness above a sine).
    AbsSine,
    /// The rising quarter of each half cycle, then silence (a buzzy pulse).
    QuarterSine,
}

/// One operator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FmOp {
    /// Its frequency over the note's (1 the note, 2 an octave up, 3.5 an inharmonic bell partial).
    pub ratio: f32,
    /// A fine detune, cents (a chorus between two carriers).
    #[serde(default)]
    pub detune_cents: f32,
    /// For a heard operator its amplitude (0..1); for a modulator its modulation index, Chowning's
    /// `I` in radians (0 none, about 1 warm, 5 and over bright and buzzy).
    pub level: f32,
    /// Its envelope (an index envelope, for a modulator: how the tone's brightness moves).
    pub envelope: EnvSpec,
    #[serde(default)]
    pub wave: OpWave,
    /// The earlier operators (by their place in the list, from 0) whose output moves this one's phase.
    #[serde(default)]
    pub mods: Vec<u8>,
    /// Heard: added to the voice's output (a carrier).
    #[serde(default)]
    pub out: bool,
    /// Key scaling: its level falls this many dB for each octave above middle C (a modulator so
    /// scaled keeps high notes from turning harsh).
    #[serde(default)]
    pub key_scale_db: f32,
    /// How much a soft note lowers it, 0 (not at all) .. 1 (in proportion to the velocity): on a
    /// modulator, a soft note is also a darker one.
    #[serde(default)]
    pub velocity: f32,
}

/// Operator FM (`InstrumentKind::FmOps`).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FmOpsSpec {
    /// One to four operators; a modulator comes before the operators it modulates.
    pub ops: Vec<FmOp>,
    /// The first operator's feedback onto itself, as a modulation index in radians (0 none; about 1
    /// a saw-like edge; 6 and over a noise for drums).
    #[serde(default)]
    pub feedback: f32,
    /// A vibrato: (rate Hz, depth cents).
    #[serde(default)]
    pub vibrato: Option<(f32, f32)>,
    /// A tremolo: (rate Hz, depth dB).
    #[serde(default)]
    pub tremolo: Option<(f32, f32)>,
    /// A drum's falling pitch: it starts this many semitones above the note and falls to it with this
    /// time constant (seconds).
    #[serde(default)]
    pub pitch_drop: Option<(f32, f32)>,
}

impl FmOpsSpec {
    /// Checked when the library loads: one to four operators, each modulated only by earlier ones,
    /// at least one heard, every number finite and in range.
    pub fn check(&self) -> Result<(), String> {
        if self.ops.is_empty() || self.ops.len() > MAX_OPS {
            return Err(format!("operator FM needs 1 to {MAX_OPS} operators, it has {}", self.ops.len()));
        }
        if !self.ops.iter().any(|o| o.out) {
            return Err("no operator is heard (`out: true`)".into());
        }
        for (k, o) in self.ops.iter().enumerate() {
            if let Some(m) = o.mods.iter().find(|&&m| m as usize >= k) {
                return Err(format!("operator {k} is modulated by operator {m}, which must come before it"));
            }
            if !(o.ratio > 0.0 && o.ratio <= 32.0) || !o.level.is_finite() || o.level < 0.0 || o.level > 32.0 {
                return Err(format!("operator {k}: its ratio must be above 0 and at most 32, its level 0 to 32"));
            }
        }
        if !self.feedback.is_finite() || !(0.0..=16.0).contains(&self.feedback) {
            return Err("the feedback must be 0 to 16".into());
        }
        Ok(())
    }

    /// The longest release of a heard operator, seconds.
    pub fn release_s(&self) -> f32 {
        self.ops.iter().filter(|o| o.out).map(|o| o.envelope.release_s).fold(0.0, f32::max)
    }
}

/// A voice's state.
#[derive(Clone, Copy, Debug, Default)]
pub struct FmOpsSt {
    ph: [f32; MAX_OPS],
    env: [Env; MAX_OPS],
    /// Each operator's last raw sample (its wave times its envelope), what its modulatees read.
    raw: [f32; MAX_OPS],
    /// The first operator's two last raw samples (its feedback).
    fb: [f32; 2],
    /// Each operator's level as a modulator (radians over 2 pi: a phase in cycles) and as heard.
    md: [f32; MAX_OPS],
    car: [f32; MAX_OPS],
    /// Each operator's frequency over the note's (its ratio and detune).
    mul: [f32; MAX_OPS],
    /// Each operator's modulators, one bit each.
    mods: [u8; MAX_OPS],
    n: u8,
    /// The pitch drop still to fall, as a frequency multiple above 1, and what is left of it after a sample.
    drop: f32,
    drop_coef: f32,
    vib: f32,
    trem: f32,
}

#[inline]
fn wave(w: OpWave, ph: f32) -> f32 {
    let p = ph - fast_floor(ph);
    match w {
        OpWave::Sine => fast_sin(p),
        OpWave::HalfSine => fast_sin(p).max(0.0),
        OpWave::AbsSine => fast_sin(p).abs(),
        OpWave::QuarterSine => {
            if (p * 2.0).fract() < 0.25 {
                fast_sin(p).abs()
            } else {
                0.0
            }
        }
    }
}

/// Start a note (MIDI number `midi`) at velocity `vel` (0..1); `phase0` starts its vibrato and tremolo.
pub fn start(spec: &FmOpsSpec, midi: f32, vel: f32, rate: f32, phase0: f32) -> FmOpsSt {
    let mut st = FmOpsSt { n: spec.ops.len().min(MAX_OPS) as u8, drop: 0.0, drop_coef: 0.0, vib: phase0, trem: phase0, ..Default::default() };
    // Octaves above middle C, by note number: key scaling is a tone colour, never a pitch.
    let octaves = ((midi - 60.0) / 12.0).max(0.0);
    for (k, o) in spec.ops.iter().take(MAX_OPS).enumerate() {
        st.env[k] = Env::new(o.envelope.attack_s, o.envelope.decay_s, o.envelope.sustain, o.envelope.release_s, rate);
        let scale = 10f32.powf(-o.key_scale_db.max(0.0) * octaves / 20.0) * (1.0 - o.velocity.clamp(0.0, 1.0) * (1.0 - vel.clamp(0.0, 1.0)));
        st.md[k] = o.level * scale / TAU;
        st.car[k] = if o.out { o.level * scale } else { 0.0 };
        st.mul[k] = o.ratio * 2f32.powf(o.detune_cents / 1200.0);
        st.mods[k] = o.mods.iter().filter(|&&m| (m as usize) < k).fold(0u8, |a, &m| a | (1 << m));
    }
    if let Some((semis, secs)) = spec.pitch_drop {
        st.drop = 2f32.powf(semis.max(0.0) / 12.0) - 1.0;
        st.drop_coef = (-1.0 / (secs.max(0.0005) * rate)).exp();
    }
    st
}

/// Let it go: every operator begins its release.
pub fn release(st: &mut FmOpsSt) {
    for e in st.env.iter_mut() {
        e.release();
    }
}

/// Render `n` samples of a note of `hz` (after the block's pitch bend) at amplitude `amp` through
/// `out(i, sample)`. False once every heard operator has died away.
pub fn render(spec: &FmOpsSpec, st: &mut FmOpsSt, hz: f32, amp: f32, rate: f32, n: usize, mut out: impl FnMut(usize, f32)) -> bool {
    let k_n = st.n as usize;
    // The vibrato and tremolo move once a block (2.7 ms at 48 kHz: far quicker than either).
    let mut f = hz;
    if let Some((r, cents)) = spec.vibrato {
        st.vib = (st.vib + r * n as f32 / rate).fract();
        f *= 2f32.powf(cents * fast_sin(st.vib) / 1200.0);
    }
    let mut gain = amp;
    if let Some((r, db)) = spec.tremolo {
        st.trem = (st.trem + r * n as f32 / rate).fract();
        gain *= 10f32.powf(-db.abs() * (0.5 + 0.5 * fast_sin(st.trem)) / 20.0);
    }
    let base = f / rate;
    let fb_amt = spec.feedback / TAU;
    for i in 0..n {
        let mul = 1.0 + st.drop;
        st.drop *= st.drop_coef;
        let mut y = 0.0;
        for k in 0..k_n {
            let e = st.env[k].next();
            let mut m = 0.0;
            if k == 0 && fb_amt > 0.0 {
                m += fb_amt * 0.5 * (st.fb[0] + st.fb[1]);
            }
            let bits = st.mods[k];
            if bits != 0 {
                for j in 0..k {
                    if bits & (1 << j) != 0 {
                        m += st.raw[j] * st.md[j];
                    }
                }
            }
            let s = wave(spec.ops[k].wave, st.ph[k] + m) * e;
            st.raw[k] = s;
            y += s * st.car[k];
            st.ph[k] += base * st.mul[k] * mul;
            st.ph[k] -= fast_floor(st.ph[k]);
        }
        st.fb[1] = st.fb[0];
        st.fb[0] = st.raw[0];
        out(i, y * gain);
    }
    (0..k_n).any(|k| spec.ops[k].out && !st.env[k].done())
}
