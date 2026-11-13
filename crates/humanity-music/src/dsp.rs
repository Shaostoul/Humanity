//! The building blocks every voice and effect is made of: band-limited oscillators, a state
//! variable filter, envelopes, smoothers, rotating phasors for decaying partials, an 8-line feedback
//! delay network reverb and a soft limiter. Plain arithmetic, no allocation after construction
//! (the reverb's delay lines are sized once, for rates up to 96 kHz), so all of it may run on the
//! real-time audio thread (kira: "should not allocate or deallocate memory").
//!
//! The algorithms are published ones: polyBLEP band limiting (Valimaki and Huovilainen, 2007), the
//! topology-preserving state variable filter (Zavalishin, "The Art of VA Filter Design", 2012),
//! a Jot-genre feedback delay network with a Hadamard mix (Jot and Chaigne, 1991), the same as the
//! direction sketch scripts/music-sketch.js, which this crate's first rung was compared against.

use std::f32::consts::PI;

/// Decibels to an amplitude.
#[inline]
pub fn db_to_amp(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// An amplitude to decibels (silence is -200).
#[inline]
pub fn amp_to_db(amp: f32) -> f32 {
    if amp <= 1e-10 {
        -200.0
    } else {
        20.0 * amp.log10()
    }
}

/// Constant-power stereo gains for a pan in -1 (left) .. 1 (right).
#[inline]
pub fn pan_gains(p: f32) -> (f32, f32) {
    let a = (p.clamp(-1.0, 1.0) + 1.0) * PI / 4.0;
    (a.cos(), a.sin())
}

/// The polyBLEP correction for a naive sawtooth or square edge: phase `t` in [0, 1), step `dt`.
#[inline]
pub fn polyblep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let t = t / dt;
        t + t - t * t - 1.0
    } else if t > 1.0 - dt {
        let t = (t - 1.0) / dt;
        t * t + t + t + 1.0
    } else {
        0.0
    }
}

/// A one-pole smoother: approaches its target with a time constant, so a level, a cutoff or a gain
/// moves without a step (a doorway is a crossfade, never a click).
#[derive(Clone, Copy, Debug)]
pub struct Smoother {
    pub value: f32,
    coef: f32,
}

impl Smoother {
    /// Starting at `value`, reaching about 63% of a change in `time_s` at `rate` Hz.
    pub fn new(value: f32, time_s: f32, rate: f32) -> Self {
        let mut s = Smoother { value, coef: 0.0 };
        s.set_time(time_s, rate);
        s
    }

    /// A new time constant.
    pub fn set_time(&mut self, time_s: f32, rate: f32) {
        self.coef = if time_s <= 0.0 { 0.0 } else { (-1.0 / (time_s * rate)).exp() };
    }

    /// One step toward `target`.
    #[inline]
    pub fn next(&mut self, target: f32) -> f32 {
        self.value = target + (self.value - target) * self.coef;
        self.value
    }

    /// `n` steps toward `target` at once (a block-rate parameter).
    #[inline]
    pub fn advance(&mut self, target: f32, n: usize) -> f32 {
        self.value = target + (self.value - target) * self.coef.powi(n as i32);
        self.value
    }
}

/// Coefficients of a topology-preserving state variable filter at one cutoff and Q.
#[derive(Clone, Copy, Debug, Default)]
pub struct SvfCoefs {
    /// 1/Q: the band output times this has unity gain at the centre.
    pub k: f32,
    a1: f32,
    a2: f32,
    a3: f32,
}

impl SvfCoefs {
    /// Cutoff `fc` hertz (held under 45% of the rate), quality `q`, at `rate` Hz.
    pub fn new(fc: f32, q: f32, rate: f32) -> Self {
        let g = (PI * fc.clamp(5.0, rate * 0.45) / rate).tan();
        let k = 1.0 / q.max(0.05);
        let a1 = 1.0 / (1.0 + g * (g + k));
        SvfCoefs { k, a1, a2: g * a1, a3: g * g * a1 }
    }
}

/// A state variable filter's state: low, band and high pass from one structure, stable under fast
/// modulation (the pads' breathing cutoff, the choir's vowel glide, a wobble bass's sweep).
#[derive(Clone, Copy, Debug, Default)]
pub struct Svf {
    ic1: f32,
    ic2: f32,
}

impl Svf {
    /// One sample: (low pass, band pass, high pass).
    #[inline]
    pub fn tick(&mut self, c: &SvfCoefs, v0: f32) -> (f32, f32, f32) {
        let v3 = v0 - self.ic2;
        let v1 = c.a1 * self.ic1 + c.a2 * v3;
        let v2 = self.ic2 + c.a2 * self.ic1 + c.a3 * v3;
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        (v2, v1, v0 - c.k * v1 - v2)
    }

    /// One sample, low pass only.
    #[inline]
    pub fn lp(&mut self, c: &SvfCoefs, v0: f32) -> f32 {
        self.tick(c, v0).0
    }

    /// One sample, band pass normalised to unity at the centre.
    #[inline]
    pub fn bp(&mut self, c: &SvfCoefs, v0: f32) -> f32 {
        self.tick(c, v0).1 * c.k
    }

    /// One sample, high pass only.
    #[inline]
    pub fn hp(&mut self, c: &SvfCoefs, v0: f32) -> f32 {
        self.tick(c, v0).2
    }
}

/// An envelope: a raised-cosine attack (no click), an exponential decay to a sustain level, and an
/// exponential release to silence (-80 dB over the release time). A sustain of 0 makes a struck
/// note that dies away by itself.
#[derive(Clone, Copy, Debug, Default)]
pub struct Env {
    attack_n: u32,
    n: u32,
    level: f32,
    sustain: f32,
    decay_coef: f32,
    release_coef: f32,
    state: u8, // 0 attack, 1 decay/sustain, 2 release, 3 done
}

impl Env {
    /// Times in seconds at `rate` Hz; `sustain` 0..1.
    pub fn new(attack_s: f32, decay_s: f32, sustain: f32, release_s: f32, rate: f32) -> Self {
        let per = |t: f32| if t <= 0.0 { 0.0 } else { (1e-4f32.ln() / (t * rate)).exp() };
        Env {
            attack_n: ((attack_s * rate) as u32).max(1),
            n: 0,
            level: 0.0,
            sustain: sustain.clamp(0.0, 1.0),
            decay_coef: per(decay_s),
            release_coef: per(release_s),
            state: 0,
        }
    }

    /// The next level.
    #[inline]
    pub fn next(&mut self) -> f32 {
        match self.state {
            0 => {
                self.n += 1;
                if self.n >= self.attack_n {
                    self.state = 1;
                    self.level = 1.0;
                } else {
                    self.level = 0.5 - 0.5 * (PI * self.n as f32 / self.attack_n as f32).cos();
                }
            }
            1 => {
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coef;
                if self.sustain <= 0.0 && self.level < 1e-4 {
                    self.state = 3;
                    self.level = 0.0;
                }
            }
            2 => {
                self.level *= self.release_coef;
                // Done at -60 dB: under a score at -18 LUFS a voice that quiet is far below hearing,
                // and ending it a quarter sooner frees its share of the audio thread.
                if self.level < 1e-3 {
                    self.state = 3;
                    self.level = 0.0;
                }
            }
            _ => {}
        }
        self.level
    }

    /// Start the release (a held note let go).
    pub fn release(&mut self) {
        if self.state < 2 {
            self.state = 2;
        }
    }

    /// Whether the release has begun (or finished).
    pub fn releasing(&self) -> bool {
        self.state >= 2
    }

    /// Whether it has died away.
    pub fn done(&self) -> bool {
        self.state == 3
    }

    /// The current level.
    pub fn level(&self) -> f32 {
        self.level
    }
}

/// A rotating complex phasor: a sine at a fixed frequency that decays (or not) with no `sin` call
/// per sample, four multiplies. The partials of a bell, a steel pan, a drum skin.
#[derive(Clone, Copy, Debug, Default)]
pub struct Phasor {
    pub re: f32,
    pub im: f32,
    cr: f32,
    ci: f32,
}

impl Phasor {
    /// Frequency `hz` at `rate`, starting amplitude `amp`, falling 60 dB over `t60_s` (0 = never).
    pub fn new(hz: f32, amp: f32, t60_s: f32, rate: f32) -> Self {
        let w = 2.0 * PI * hz / rate;
        let radius = if t60_s > 0.0 { (1e-3f32.ln() / (t60_s * rate)).exp() } else { 1.0 };
        Phasor { re: 0.0, im: amp, cr: w.cos() * radius, ci: w.sin() * radius }
    }

    /// A change of frequency keeping the phase and amplitude (a drum's pitch falling as it rings).
    pub fn retune(&mut self, hz: f32, t60_s: f32, rate: f32) {
        let w = 2.0 * PI * hz / rate;
        let radius = if t60_s > 0.0 { (1e-3f32.ln() / (t60_s * rate)).exp() } else { 1.0 };
        self.cr = w.cos() * radius;
        self.ci = w.sin() * radius;
    }

    /// One sample.
    #[inline]
    pub fn tick(&mut self) -> f32 {
        let re = self.re * self.cr - self.im * self.ci;
        self.im = self.re * self.ci + self.im * self.cr;
        self.re = re;
        self.im
    }

    /// Bring the amplitude back to `amp` (rounding drifts it over minutes).
    pub fn renormalise(&mut self, amp: f32) {
        let m = (self.re * self.re + self.im * self.im).sqrt();
        if m > 1e-20 {
            self.re *= amp / m;
            self.im *= amp / m;
        }
    }

    /// The amplitude now.
    pub fn amplitude(&self) -> f32 {
        (self.re * self.re + self.im * self.im).sqrt()
    }
}

/// A slow sine for modulation (vibrato, breathing, wow), advanced per block or per sample.
#[derive(Clone, Copy, Debug, Default)]
pub struct Lfo {
    phase: f32,
}

impl Lfo {
    /// Starting at `phase` (0..1).
    pub fn new(phase: f32) -> Self {
        Lfo { phase: phase.fract() }
    }

    /// Advance by `hz` over `n` samples at `rate`, and return the sine.
    #[inline]
    pub fn advance(&mut self, hz: f32, n: usize, rate: f32) -> f32 {
        self.phase = (self.phase + hz * n as f32 / rate).fract();
        (2.0 * PI * self.phase).sin()
    }

    /// The sine now.
    pub fn value(&self) -> f32 {
        (2.0 * PI * self.phase).sin()
    }
}

/// The longest delay line any reverb rate needs (8 lines, the longest 4943 samples at 44.1 kHz,
/// scaled to 96 kHz plus the modulation: about 10,800).
const FDN_LINE: usize = 16384;
const FDN_MASK: usize = FDN_LINE - 1;
/// The lines' lengths at 44.1 kHz (mutually prime-ish, as the sketch's).
const FDN_BASE: [f32; 8] = [1657.0, 2129.0, 2441.0, 2887.0, 3301.0, 3769.0, 4391.0, 4943.0];

/// An 8-line feedback delay network reverb: a pre-delay, four allpass diffusers, lines mixed by a
/// normalised Hadamard matrix, each damped (high frequencies die sooner) and slowly modulated in
/// length (so the tail never rings metallic). The direction sketch's reverb, in Rust. Its lines are
/// fixed-size arrays indexed through a mask (no bounds checks in the loop), and the slow modulation
/// moves once every 16 samples (a 0.1 Hz sway changes a line's length by about a thousandth of a
/// sample in that time), which made it a third cheaper (measured with `just music-render`).
pub struct Fdn {
    lines: Vec<[f32; FDN_LINE]>,
    w: usize,
    len: [f32; 8],
    gain: [f32; 8],
    lp: [f32; 8],
    lfo: [Phasor; 8],
    delay: [f32; 8],
    sub: usize,
    pre: Vec<[f32; 8192]>,
    pre_w: usize,
    pre_d: usize,
    ap: Vec<[f32; 2048]>,
    ap_len: [usize; 4],
    ap_w: [usize; 4],
    damp: f32,
    t60: f32,
    rate: f32,
}

/// The reverb's modulation moves once every this many samples.
const FDN_MOD_STEP: usize = 16;

impl Fdn {
    /// A reverb with a decay of `t60` seconds at `rate` Hz (allocates its lines here, once).
    pub fn new(t60: f32, rate: f32) -> Self {
        let mut f = Fdn {
            lines: vec![[0.0; FDN_LINE]; 8],
            w: 0,
            len: [0.0; 8],
            gain: [0.0; 8],
            lp: [0.0; 8],
            lfo: [Phasor::default(); 8],
            delay: [0.0; 8],
            sub: 0,
            pre: vec![[0.0; 8192]; 1],
            pre_w: 0,
            pre_d: 0,
            ap: vec![[0.0; 2048]; 4],
            ap_len: [0; 4],
            ap_w: [0; 4],
            damp: 0.25,
            t60,
            rate,
        };
        f.set_rate(rate);
        f
    }

    /// Recompute the lengths and decays for a new rate (no allocation; up to 96 kHz).
    pub fn set_rate(&mut self, rate: f32) {
        let rate = rate.clamp(8000.0, 96000.0);
        self.rate = rate;
        let s = rate / 44100.0;
        for k in 0..8 {
            self.len[k] = FDN_BASE[k] * s;
            self.gain[k] = 10f32.powf(-3.0 * self.len[k] / (self.t60 * rate));
            self.lfo[k] = Phasor::new(0.07 + 0.05 * k as f32 / 7.0, 1.0, 0.0, rate / FDN_MOD_STEP as f32);
            self.delay[k] = self.len[k];
        }
        self.pre_d = ((0.028 * rate) as usize).min(8191);
        for (i, d) in [225.0f32, 341.0, 441.0, 556.0].iter().enumerate() {
            self.ap_len[i] = ((d * s) as usize).clamp(1, 2047);
        }
    }

    /// A new decay time (a new mood's hall): only the line gains change, nothing is allocated.
    pub fn set_t60(&mut self, t60: f32) {
        self.t60 = t60.max(0.1);
        for k in 0..8 {
            self.gain[k] = 10f32.powf(-3.0 * self.len[k] / (self.t60 * self.rate));
        }
    }

    /// Clear the tail.
    pub fn clear(&mut self) {
        for l in self.lines.iter_mut() {
            l.fill(0.0);
        }
        self.pre[0].fill(0.0);
        for a in self.ap.iter_mut() {
            a.fill(0.0);
        }
        self.lp = [0.0; 8];
    }

    /// Run the send buffers through the reverb and ADD the wet signal (times `wet`) into out.
    pub fn process(&mut self, send_l: &[f32], send_r: &[f32], out_l: &mut [f32], out_r: &mut [f32], wet: f32) {
        let norm = 1.0 / 8f32.sqrt();
        let n = send_l.len().min(send_r.len()).min(out_l.len()).min(out_r.len());
        let (damp, undamp) = (self.damp, 1.0 - self.damp);
        let pre = &mut self.pre[0];
        for i in 0..n {
            if self.sub == 0 {
                for k in 0..8 {
                    self.delay[k] = self.len[k] + 6.0 * self.lfo[k].tick();
                }
            }
            self.sub = (self.sub + 1) % FDN_MOD_STEP;
            let (inl, inr) = (send_l[i], send_r[i]);
            // pre-delay
            pre[self.pre_w & 8191] = (inl + inr) * 0.5;
            let mut s = pre[(self.pre_w + 8192 - self.pre_d) & 8191];
            self.pre_w = (self.pre_w + 1) & 8191;
            // diffusers
            for a in 0..4 {
                let w = self.ap_w[a] & 2047;
                let line = &mut self.ap[a];
                let z = line[w];
                let y = -0.6 * s + z;
                line[w] = s + 0.6 * y;
                self.ap_w[a] = if w + 1 >= self.ap_len[a] { 0 } else { w + 1 };
                s = y;
            }
            let side = (inl - inr) * 0.3;
            let mut x = [0f32; 8];
            let wf = self.w as f32 + FDN_LINE as f32;
            for k in 0..8 {
                let rp = wf - self.delay[k];
                let i0 = rp as usize;
                let fr = rp - i0 as f32;
                let line = &self.lines[k];
                let a = line[i0 & FDN_MASK];
                let b = line[(i0 + 1) & FDN_MASK];
                let v = a + (b - a) * fr;
                self.lp[k] = v * undamp + self.lp[k] * damp;
                x[k] = self.lp[k] * self.gain[k];
            }
            // An 8-point fast Walsh-Hadamard transform, written out.
            let (a0, a1, a2, a3) = (x[0] + x[1], x[0] - x[1], x[2] + x[3], x[2] - x[3]);
            let (a4, a5, a6, a7) = (x[4] + x[5], x[4] - x[5], x[6] + x[7], x[6] - x[7]);
            let (b0, b1, b2, b3) = (a0 + a2, a1 + a3, a0 - a2, a1 - a3);
            let (b4, b5, b6, b7) = (a4 + a6, a5 + a7, a4 - a6, a5 - a7);
            let y = [b0 + b4, b1 + b5, b2 + b6, b3 + b7, b0 - b4, b1 - b5, b2 - b6, b3 - b7];
            let w = self.w & FDN_MASK;
            let feed = s * 0.5;
            for k in 0..8 {
                let sgn = if k < 4 { side } else { -side };
                self.lines[k][w] = y[k] * norm + feed + sgn * 0.25;
            }
            self.w = (self.w + 1) & FDN_MASK;
            out_l[i] += (y[0] - y[2] + y[4] - y[6]) * norm * wet;
            out_r[i] += (y[1] - y[3] + y[5] - y[7]) * norm * wet;
        }
        // The modulators drift in amplitude by rounding; hold them at 1.
        for l in self.lfo.iter_mut() {
            l.renormalise(1.0);
        }
    }
}

/// A safety-net limiter (docs/design/music.md, section 13: "A soft limiter at -1 dBTP is a safety
/// net, never the sound"): an instant-attack, slow-release gain computer on the sample peak, held
/// under `ceiling` with 1.5 dB of room for the peaks between samples, then a gentle tanh knee for
/// whatever slips past. The score is levelled well under it, so it rarely acts.
#[derive(Clone, Copy, Debug)]
pub struct Limiter {
    env: f32,
    release: f32,
    ceiling: f32,
    /// How many samples it pulled down (the Dev page's "limiter active" and a test).
    pub engaged: u64,
}

impl Limiter {
    /// A ceiling in dBFS (true peak; the sample ceiling sits 1.5 dB under it) at `rate`.
    pub fn new(ceiling_db: f32, rate: f32) -> Self {
        Limiter { env: 0.0, release: (-1.0 / (0.25 * rate)).exp(), ceiling: db_to_amp(ceiling_db - 1.5), engaged: 0 }
    }

    /// One stereo sample, limited.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let p = l.abs().max(r.abs());
        self.env = if p > self.env { p } else { p + (self.env - p) * self.release };
        let g = if self.env > self.ceiling {
            self.engaged += 1;
            self.ceiling / self.env
        } else {
            1.0
        };
        let c = self.ceiling;
        let knee = |x: f32| {
            let a = x.abs();
            if a <= 0.9 * c {
                x
            } else {
                // tanh above 90% of the ceiling, continuous in value and slope, never past it
                let over = (a - 0.9 * c) / (0.1 * c);
                x.signum() * (0.9 * c + 0.1 * c * over.tanh())
            }
        };
        (knee(l * g), knee(r * g))
    }
}
