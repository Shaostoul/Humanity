//! The mix's effects for the electronic genres (rung E3, docs/design/music.md section 7g): a
//! TEMPO DELAY (its time a note value at the music's tempo, straight, dotted or triplet; its repeats
//! band-limited in the feedback loop as a tape echo's are, and optionally bouncing between the
//! speakers), a GATED REVERB (a small, dense room whose tail a gate opened by each hit cuts dead), and
//! the TAPE STOP (the whole score slowing to nothing, pitch and time together, read back from a
//! short buffer at a falling speed). The reverb's low cut lives in the score (a high-pass on the
//! send). All our own code: the delay is a recirculating delay line with filters in its loop, the
//! reverb a feedback delay network like the score's main one (dsp.rs `Fdn`), the tape stop a
//! variable-rate read with linear interpolation. What each is FOR, and the numbers, come from the
//! production sources the research read on 2026-11-13
//! (`docs/reference/research/2026-11-13-instrument-libraries/electronic-genres-and-synthesis.md`):
//! Sound On Sound's "Sound Design For Ambient Music" (Paul White, November 2018: "a tape-delay
//! emulation, sync'ed to the project tempo and with the lows rolled off at 600Hz and the highs at
//! 1200Hz"; "a 6 or 12 dB/octave low-cut filter at 300-400 Hz"), MusicRadar's "How to make an '80s-
//! style synth track in your DAW" (5 August 2020: "heavily compressing a non-linear reverb tail, then
//! placing a noise gate plugin across it to chop off the release"), Wikipedia's "Gated reverb" (9
//! October 2026: "combines strong reverb and a noise gate that cuts the tail of the reverb") and
//! MusicRadar's tape-stop how-to (30 August 2017: "emulating the effect of a tape deck slowing
//! down"). Nothing allocates after `new`; every buffer is made once.

use crate::dsp::{fast_tanh, Svf, SvfCoefs};

/// The delay's lines: 65,536 samples each (1.36 s at 48 kHz, 1.49 s at 44.1 kHz). A note value longer
/// than that at a slow tempo is held to it.
const DELAY_LEN: usize = 1 << 16;
const DELAY_MASK: usize = DELAY_LEN - 1;

/// A genre's tempo delay, ready to play (genre.rs `DelaySpec`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RDelay {
    /// The time in beats (a dotted eighth is 0.75).
    pub beats: f32,
    /// How much of each repeat comes round again, 0 .. 0.95.
    pub feedback: f32,
    /// The repeats band-limited: lows rolled off under `low_cut_hz`, highs over `high_cut_hz` (0: none).
    pub low_cut_hz: f32,
    pub high_cut_hz: f32,
    /// Each repeat on the other side.
    pub ping_pong: bool,
    /// Each role's send, 0 .. 1.
    pub sends: [f32; 8],
    /// The repeats' level, and their share sent on into the reverb.
    pub wet: f32,
    pub reverb: f32,
}

/// A recirculating stereo delay whose time follows the tempo, its repeats filtered in the loop (each
/// pass a little thinner and darker, as a tape echo's) and saturated softly so feedback can never run
/// away. A change of time (a new tempo) glides over about 60 ms, as a tape machine's would.
pub struct TempoDelay {
    l: Vec<f32>,
    r: Vec<f32>,
    w: usize,
    /// The delay now, samples (gliding toward the asked time).
    time: f32,
    hp: [Svf; 2],
    lp: [Svf; 2],
}

impl Default for TempoDelay {
    fn default() -> Self {
        Self::new()
    }
}

impl TempoDelay {
    /// Allocates its two lines here, once.
    pub fn new() -> Self {
        TempoDelay { l: vec![0.0; DELAY_LEN], r: vec![0.0; DELAY_LEN], w: 0, time: 0.0, hp: [Svf::default(); 2], lp: [Svf::default(); 2] }
    }

    /// Silence its lines (a tape stop's end).
    pub fn clear(&mut self) {
        self.l.fill(0.0);
        self.r.fill(0.0);
        self.hp = [Svf::default(); 2];
        self.lp = [Svf::default(); 2];
    }

    /// The longest delay it can hold, seconds, at `rate`.
    pub fn max_s(rate: f32) -> f32 {
        (DELAY_LEN - 4) as f32 / rate
    }

    /// Run `send` through the delay for `spec` at `beat_hz` beats a second, ADDING its repeats (times
    /// `spec.wet`) into `out` and (times `spec.reverb` too) into the reverb's send `rev`.
    #[allow(clippy::too_many_arguments)]
    pub fn process(&mut self, spec: &RDelay, beat_hz: f32, rate: f32, send_l: &[f32], send_r: &[f32], out_l: &mut [f32], out_r: &mut [f32], rev_l: &mut [f32], rev_r: &mut [f32]) {
        let n = send_l.len().min(send_r.len()).min(out_l.len()).min(out_r.len()).min(rev_l.len()).min(rev_r.len());
        let target = (spec.beats.max(0.01) * rate / beat_hz.max(0.05)).clamp(1.0, (DELAY_LEN - 4) as f32);
        if self.time <= 0.0 {
            self.time = target;
        }
        let glide = 1.0 - (-1.0 / (0.06 * rate)).exp();
        let hpc = (spec.low_cut_hz > 0.0).then(|| SvfCoefs::new(spec.low_cut_hz, 0.707, rate));
        let lpc = (spec.high_cut_hz > 0.0).then(|| SvfCoefs::new(spec.high_cut_hz, 0.707, rate));
        let fb = spec.feedback.clamp(0.0, 0.95);
        let (wet, rev) = (spec.wet, spec.wet * spec.reverb);
        for i in 0..n {
            self.time += (target - self.time) * glide;
            let rp = (self.w + DELAY_LEN) as f32 - self.time;
            let i0 = rp as usize;
            let fr = rp - i0 as f32;
            let (a, b) = (i0 & DELAY_MASK, (i0 + 1) & DELAY_MASK);
            let mut yl = self.l[a] + (self.l[b] - self.l[a]) * fr;
            let mut yr = self.r[a] + (self.r[b] - self.r[a]) * fr;
            if let Some(c) = &hpc {
                yl = self.hp[0].hp(c, yl);
                yr = self.hp[1].hp(c, yr);
            }
            if let Some(c) = &lpc {
                yl = self.lp[0].lp(c, yl);
                yr = self.lp[1].lp(c, yr);
            }
            let w = self.w & DELAY_MASK;
            if spec.ping_pong {
                // Both sides' sound enters on the left; each pass crosses to the other side.
                self.l[w] = fast_tanh(0.5 * (send_l[i] + send_r[i]) + fb * yr);
                self.r[w] = fast_tanh(fb * yl);
            } else {
                self.l[w] = fast_tanh(send_l[i] + fb * yl);
                self.r[w] = fast_tanh(send_r[i] + fb * yr);
            }
            self.w = (self.w + 1) & DELAY_MASK;
            out_l[i] += yl * wet;
            out_r[i] += yr * wet;
            rev_l[i] += yl * rev;
            rev_r[i] += yr * rev;
        }
    }
}

/// The reverb send's LOW CUT (genre.rs `Recipe::reverb_low_cut_hz`): a two-pole high-pass (12 dB an
/// octave) at `hz` over the send, in place, so the hall keeps out of the low end.
pub fn low_cut(st: &mut [Svf; 2], hz: f32, rate: f32, l: &mut [f32], r: &mut [f32]) {
    let c = SvfCoefs::new(hz, 0.707, rate);
    for (x, y) in l.iter_mut().zip(r.iter_mut()) {
        *x = st[0].hp(&c, *x);
        *y = st[1].hp(&c, *y);
    }
}

/// A genre's gated reverb, ready to play (genre.rs `GatedReverbSpec`).
#[derive(Clone, Copy, Debug, Default)]
pub struct RGated {
    /// The room's decay (long, so the tail stays level until the gate cuts it: the "non-linear"
    /// reverb's flat envelope), the gate's open time after each hit and its closing time.
    pub t60_s: f32,
    pub hold_s: f32,
    pub release_s: f32,
    pub wet: f32,
}

/// The gated room's four lines at 44.1 kHz (about 17 to 34 ms, no two in a simple ratio), and its
/// two diffusers'.
const GATE_BASE: [f32; 4] = [743.0, 997.0, 1231.0, 1499.0];
const GATE_AP: [f32; 2] = [142.0, 379.0];
const GATE_LINE: usize = 4096;
const GATE_MASK: usize = GATE_LINE - 1;

/// A small dense room (four delay lines mixed by a Hadamard matrix, after two all-pass diffusers)
/// whose output passes a GATE: opened by each gated hit (the composer's own event, exact, with no
/// detector listening), held open `hold_s`, then closed in a straight line over `release_s`.
pub struct GatedReverb {
    lines: Vec<[f32; GATE_LINE]>,
    w: usize,
    len: [usize; 4],
    gain: [f32; 4],
    ap: Vec<[f32; 1024]>,
    ap_len: [usize; 2],
    ap_w: [usize; 2],
    damp: [f32; 4],
    t60: f32,
    rate: f32,
    /// Seconds since the gate last opened (f64::MAX: closed since the start), and whether the room is
    /// empty (emptied as the gate shut).
    since: f64,
    empty: bool,
}

impl GatedReverb {
    /// Allocates its lines here, once.
    pub fn new(rate: f32) -> Self {
        let mut g = GatedReverb { lines: vec![[0.0; GATE_LINE]; 4], w: 0, len: [1; 4], gain: [0.0; 4], ap: vec![[0.0; 1024]; 2], ap_len: [1; 2], ap_w: [0; 2], damp: [0.0; 4], t60: 2.5, rate, since: f64::MAX, empty: true };
        g.set(2.5, rate);
        g
    }

    /// Its decay and rate (no allocation; up to 96 kHz).
    pub fn set(&mut self, t60: f32, rate: f32) {
        let rate = rate.clamp(8000.0, 96000.0);
        self.rate = rate;
        self.t60 = t60.max(0.1);
        let s = rate / 44100.0;
        for k in 0..4 {
            self.len[k] = ((GATE_BASE[k] * s) as usize).clamp(1, GATE_LINE - 1);
            self.gain[k] = 10f32.powf(-3.0 * self.len[k] as f32 / (self.t60 * rate));
        }
        for k in 0..2 {
            self.ap_len[k] = ((GATE_AP[k] * s) as usize).clamp(1, 1023);
        }
    }

    /// The decay now.
    pub fn t60(&self) -> f32 {
        self.t60
    }

    /// A gated hit: the gate opens now.
    pub fn trigger(&mut self) {
        self.since = 0.0;
    }

    /// Silence the room and close the gate (a tape stop's end).
    pub fn clear(&mut self) {
        for l in self.lines.iter_mut() {
            l.fill(0.0);
        }
        for a in self.ap.iter_mut() {
            a.fill(0.0);
        }
        self.damp = [0.0; 4];
        self.since = f64::MAX;
        self.empty = true;
    }

    /// The gate's gain `t` seconds after it opened.
    pub fn gate_at(spec: &RGated, t: f64) -> f32 {
        let hold = spec.hold_s.max(0.0) as f64;
        if t < hold {
            1.0
        } else {
            (1.0 - (t - hold) / spec.release_s.max(0.001) as f64).max(0.0) as f32
        }
    }

    /// Run the gated send through the room and the gate, ADDING the result (times `spec.wet`) into out.
    /// While the gate is shut nothing is heard, so the room is emptied once as it shuts and then
    /// skipped until the next hit opens it (it costs nothing three quarters of the time; the next hit
    /// rings in an empty room, where a running one would still hold the last hit's tail 20 dB down).
    pub fn process(&mut self, spec: &RGated, send_l: &[f32], send_r: &[f32], out_l: &mut [f32], out_r: &mut [f32]) {
        let n = send_l.len().min(send_r.len()).min(out_l.len()).min(out_r.len());
        let dt = 1.0 / self.rate as f64;
        let open_s = (spec.hold_s.max(0.0) + spec.release_s.max(0.001)) as f64;
        if self.since >= open_s {
            if !self.empty {
                for l in self.lines.iter_mut() {
                    l.fill(0.0);
                }
                for a in self.ap.iter_mut() {
                    a.fill(0.0);
                }
                self.damp = [0.0; 4];
                self.empty = true;
            }
            return;
        }
        self.empty = false;
        let norm = 0.5f32;
        for i in 0..n {
            let mut s = 0.5 * (send_l[i] + send_r[i]);
            for a in 0..2 {
                let w = self.ap_w[a];
                let line = &mut self.ap[a];
                let z = line[w];
                let y = -0.65 * s + z;
                line[w] = s + 0.65 * y;
                self.ap_w[a] = if w + 1 >= self.ap_len[a] { 0 } else { w + 1 };
                s = y;
            }
            let mut x = [0f32; 4];
            for k in 0..4 {
                let v = self.lines[k][(self.w + GATE_LINE - self.len[k]) & GATE_MASK];
                // A gentle damping: the highs die a little sooner.
                self.damp[k] = v * 0.8 + self.damp[k] * 0.2;
                x[k] = self.damp[k] * self.gain[k];
            }
            let (a0, a1, a2, a3) = (x[0] + x[1], x[0] - x[1], x[2] + x[3], x[2] - x[3]);
            let y = [a0 + a2, a1 + a3, a0 - a2, a1 - a3];
            let w = self.w & GATE_MASK;
            for k in 0..4 {
                self.lines[k][w] = y[k] * norm + s * 0.5;
            }
            self.w = (self.w + 1) & GATE_MASK;
            let g = if self.since < 10.0 { Self::gate_at(spec, self.since) } else { 0.0 };
            if self.since < 10.0 {
                self.since += dt;
            }
            if g > 0.0 {
                out_l[i] += (y[0] + y[2]) * norm * g * spec.wet;
                out_r[i] += (y[1] + y[3]) * norm * g * spec.wet;
            }
        }
    }
}

/// Room for the tape stop's read buffer: a chunk read at any speed up to 1 needs at most a chunk and
/// two frames more.
pub const TAPE_CAP: usize = 2 * 64 + 8;

/// The tape stop's read-back (rung E3): while a stop runs, the score renders into this short buffer
/// and reads it back at the falling speed, interpolating between frames, so pitch and time fall
/// together exactly as a slowing tape's; the level falls with the speed too (a playback head's
/// voltage follows the tape's speed, Faraday's law). Outside a stop it is bypassed and empty.
pub struct TapeRead {
    buf: [[f32; 2]; TAPE_CAP],
    len: usize,
    pos: f64,
}

impl Default for TapeRead {
    fn default() -> Self {
        TapeRead { buf: [[0.0; 2]; TAPE_CAP], len: 0, pos: 0.0 }
    }
}

impl TapeRead {
    /// Empty it (a stop's start or end).
    pub fn reset(&mut self) {
        self.len = 0;
        self.pos = 0.0;
    }

    /// Frames still to be rendered into the buffer before `n` frames can be read at speeds that sum
    /// to `advance` (each speed at most 1).
    pub fn needed(&self, advance: f64) -> usize {
        let want = (self.pos + advance).floor() as usize + 2;
        want.saturating_sub(self.len).min(TAPE_CAP - self.len)
    }

    /// Append rendered frames.
    pub fn push(&mut self, l: &[f32], r: &[f32]) {
        for (a, b) in l.iter().zip(r.iter()) {
            if self.len < TAPE_CAP {
                self.buf[self.len] = [*a, *b];
                self.len += 1;
            }
        }
    }

    /// Read one frame at the current position, then move on by `speed` frames; the level follows the
    /// speed.
    pub fn read(&mut self, speed: f32) -> (f32, f32) {
        let i0 = (self.pos.floor() as usize).min(self.len.saturating_sub(1));
        let i1 = (i0 + 1).min(self.len.saturating_sub(1));
        let fr = (self.pos - i0 as f64).clamp(0.0, 1.0) as f32;
        let (a, b) = (self.buf[i0], self.buf[i1]);
        let g = speed.clamp(0.0, 1.0);
        self.pos += speed.clamp(0.0, 1.0) as f64;
        ((a[0] + (b[0] - a[0]) * fr) * g, (a[1] + (b[1] - a[1]) * fr) * g)
    }

    /// Drop the frames already read past (keeping the one the next read starts from).
    pub fn compact(&mut self) {
        let k = (self.pos.floor() as usize).min(self.len);
        if k > 0 {
            self.buf.copy_within(k..self.len, 0);
            self.len -= k;
            self.pos -= k as f64;
        }
    }
}
