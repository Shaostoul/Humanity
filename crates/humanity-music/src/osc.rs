//! A synth's oscillators (docs/design/music.md sections 6 and 7f): the classic waves, and since rung
//! E5 the PULSE with its width modulated, UNISON (seven saws detuned is the supersaw) and the
//! WAVETABLE. The voice (voice.rs) asks for a block's plan once ([`plan`]: the phase steps, the
//! pans, the tables, the morph and the width, each LFO advanced), then for the samples ([`fill`]),
//! oscillator by oscillator, summed into one buffer (mono, for a voice with the bass toolkit's chain)
//! or two (stereo), and filters them after. Nothing allocates.
//!
//! The band limiting: polyBLEP at every edge of the saw, square and pulse (Valimaki and
//! Huovilainen, 2007, as dsp.rs); the wavetable's tables hold only the harmonics under Nyquist at the
//! note (wavetable.rs). The unison's detune and mix curves are Adam Szabo's measurements of the
//! Roland JP-8000's super saw ("How to Emulate the Super Saw", KTH, Stockholm 2010, read 2026-11-14:
//! table 1's offsets, his 11th-degree polynomial for the detune knob, the centre's straight line and
//! the sides' parabola for the mix knob, the oscillators "strictly random" in phase at each note, and
//! a high-pass "at the fundamental harmonic").

use std::f32::consts::PI;

use crate::dsp::{lfo_value, pan_gains, polyblep, Lfo, Svf, SvfCoefs};
use crate::instrument::{SynthSpec, Wave};
use crate::rng::Rng;
use crate::voice::BlockParams;
use crate::wavetable::WaveTable;

/// Unison copies at most (the super saw's seven).
pub const MAX_UNISON: usize = 7;

/// The seven oscillators' frequencies relative to the centre at full detune (Szabo 2010, table 1:
/// "1 - 0.11002313" for the first, "1 + 0.10745242" for the seventh).
pub const SZABO_OFFSETS: [f32; MAX_UNISON] = [-0.110_023_13, -0.062_884_39, -0.019_523_56, 0.0, 0.019_912_21, 0.062_165_38, 0.107_452_42];

/// Which of the seven play for each count: the centre when the count is odd, then pairs from the
/// outside in.
const ORDER: [[usize; MAX_UNISON]; 8] = [
    [3, 0, 0, 0, 0, 0, 0],
    [3, 0, 0, 0, 0, 0, 0],
    [0, 6, 0, 0, 0, 0, 0],
    [3, 0, 6, 0, 0, 0, 0],
    [0, 6, 1, 5, 0, 0, 0],
    [3, 0, 6, 1, 5, 0, 0],
    [0, 6, 1, 5, 2, 4, 0],
    [3, 0, 6, 1, 5, 2, 4],
];

/// The detune knob (0 .. 1) to the amount the offsets are scaled by: Szabo's 11th-degree polynomial
/// fitted to the JP-8000's measured detune (2010, section 3.1), 0.003 at 0 and 0.99 at 1.
pub fn szabo_detune(x: f32) -> f32 {
    const C: [f64; 12] = [
        10028.731_289_163_4,
        -50818.865_204_592_4,
        111_363.480_872_936_8,
        -138_150.676_108_054_8,
        106_649.667_915_829_2,
        -53046.964_275_187_5,
        17019.951_858_008_0,
        -3425.083_659_131_8,
        404.270_393_838_8,
        -24.187_882_439_1,
        0.671_741_763_4,
        0.003_011_559_6,
    ];
    let x = x.clamp(0.0, 1.0) as f64;
    C.iter().fold(0.0, |acc, c| acc * x + c) as f32
}

/// The mix knob (0 .. 1) to the centre copy's level and each side copy's (Szabo 2010, section 3.2:
/// "y = -0.55366*x + 0.99785" and "y = -0.73764*x^2 + 1.2841*x + 0.044372").
pub fn szabo_mix(x: f32) -> (f32, f32) {
    let x = x.clamp(0.0, 1.0);
    (-0.55366 * x + 0.99785, -0.73764 * x * x + 1.2841 * x + 0.044372)
}

/// The oscillators' state in one voice.
#[derive(Clone, Copy, Default)]
pub(crate) struct OscBank {
    /// Each oscillator's phase.
    pub ph: [f32; 4],
    /// Each unison copy's phase, per oscillator.
    uph: [[f32; MAX_UNISON]; 4],
    /// The unison's high-pass at the note, per oscillator, left and right.
    uhp: [[Svf; 2]; 4],
    /// The width modulation's free LFO, its restarted phase and its last offset.
    pwm_lfo: Lfo,
    pwm_ph: f32,
    pwm_off: f32,
    /// The morph's free LFO, its restarted phase and its last position.
    wt_lfo: Lfo,
    wt_ph: f32,
    wt_pos: f32,
    /// A block has been planned (the width and the morph start where they are, with no ramp).
    started: bool,
}

impl OscBank {
    /// A note's oscillators: every phase random (free running). Only what the synth uses draws from
    /// `rng`, so an instrument without the rung's oscillators plays exactly as before it.
    pub fn new(s: &SynthSpec, rng: &mut Rng) -> Self {
        let mut b = OscBank::default();
        for p in b.ph.iter_mut() {
            *p = rng.f32();
        }
        for (k, o) in s.oscillators.iter().take(4).enumerate() {
            if o.unison.is_some() {
                for p in b.uph[k].iter_mut() {
                    *p = rng.f32();
                }
            }
        }
        if s.pwm.is_some() {
            b.pwm_lfo = Lfo::new(rng.f32());
        }
        if s.wavetable.is_some() {
            b.wt_lfo = Lfo::new(rng.f32());
        }
        b
    }
}

/// One oscillator's plan for a block.
#[derive(Clone, Copy)]
struct OscPlan {
    wave: Wave,
    dt: f32,
    lv: f32,
    pl: f32,
    pr: f32,
    /// A pulse's width at the block's start and end.
    w0: f32,
    w1: f32,
    /// The wavetable's table for this pitch.
    level: usize,
    /// Unison: copies (0: none), each one's phase step and gains (mono, left, right), and the
    /// high-pass at the note.
    uni: usize,
    udt: [f32; MAX_UNISON],
    ug: [f32; MAX_UNISON],
    ugl: [f32; MAX_UNISON],
    ugr: [f32; MAX_UNISON],
    hp: Option<SvfCoefs>,
}

/// A block's plan for every oscillator.
pub(crate) struct Plan {
    o: [OscPlan; 4],
    no: usize,
    /// The morph's position at the block's start and end.
    pos0: f32,
    pos1: f32,
}

/// A tempo LFO's value at the block's start, as voice.rs's `beat_lfo`, from a [`BeatLfo`], or a free
/// one at `hz` (0 when neither is set).
fn lfo_now(l: &crate::instrument::BeatLfo, hz: f32, free: &mut Lfo, ph: &mut f32, p: &BlockParams, n: usize) -> f32 {
    if l.per_beat > 0.0 || !l.rates.is_empty() {
        let r = if l.rates.is_empty() { l.per_beat } else { l.rates[(p.bar % l.rates.len() as u64) as usize] };
        if l.restart {
            let c = crate::dsp::fast_floor(*ph);
            let y = lfo_value(l.shape, *ph - c, c as u64);
            *ph += r * p.beat_hz * n as f32 / p.rate;
            y
        } else {
            let x = p.beats * r as f64;
            lfo_value(l.shape, x.fract() as f32, x.floor() as u64)
        }
    } else if hz > 0.0 {
        free.advance(hz, n, p.rate)
    } else {
        0.0
    }
}

/// Work out a block: `f` is the note's frequency now (vibrato, glide and wow in it), `env` the
/// envelope's level at the block's start, `n` the block's length.
pub(crate) fn plan(s: &SynthSpec, st: &mut OscBank, f: f32, env: f32, p: &BlockParams, n: usize) -> Plan {
    let rate = p.rate;
    let blank = OscPlan { wave: Wave::Sine, dt: 0.0, lv: 0.0, pl: 0.0, pr: 0.0, w0: 0.5, w1: 0.5, level: 0, uni: 0, udt: [0.0; MAX_UNISON], ug: [0.0; MAX_UNISON], ugl: [0.0; MAX_UNISON], ugr: [0.0; MAX_UNISON], hp: None };
    let mut plan = Plan { o: [blank; 4], no: s.oscillators.len().min(4), pos0: 0.0, pos1: 0.0 };
    // The width modulation's offset now (shared by every pulse).
    let pwm = s.pwm.as_ref().map(|pw| pw.depth * lfo_now(&pw.lfo, pw.hz, &mut st.pwm_lfo, &mut st.pwm_ph, p, n));
    if let Some(off) = pwm {
        if !st.started {
            st.pwm_off = off;
        }
    }
    let table = s.wavetable.as_ref().and_then(|w| w.table.as_deref());
    if let Some(w) = &s.wavetable {
        let sweep = 0.5 + 0.5 * lfo_now(&w.lfo, w.lfo_hz, &mut st.wt_lfo, &mut st.wt_ph, p, n);
        let has_lfo = w.lfo.per_beat > 0.0 || !w.lfo.rates.is_empty() || w.lfo_hz > 0.0;
        let pos = (w.position + if has_lfo { w.depth * sweep } else { 0.0 } + w.env * env).clamp(0.0, 1.0);
        plan.pos0 = if st.started { st.wt_pos } else { pos };
        plan.pos1 = pos;
        st.wt_pos = pos;
    }
    for (k, o) in s.oscillators.iter().take(4).enumerate() {
        let dt = (f * 2f32.powf((o.semitones * 100.0 + o.detune_cents) / 1200.0) / rate).min(0.45);
        let (pl, pr) = pan_gains(o.pan);
        let q = &mut plan.o[k];
        q.wave = o.wave;
        q.dt = dt;
        q.lv = o.level;
        q.pl = pl;
        q.pr = pr;
        if o.wave == Wave::Pulse {
            // Never narrower than two samples either side, so the two edges' corrections never meet.
            let lo = 0.02f32.max(2.0 * dt);
            let hi = (1.0 - lo).max(lo);
            let (a, b) = match pwm {
                Some(off) => (st.pwm_off, off),
                None => (0.0, 0.0),
            };
            q.w0 = (o.width + a).clamp(lo, hi);
            q.w1 = (o.width + b).clamp(lo, hi);
        }
        let mut top_dt = dt;
        if let Some(u) = &o.unison {
            let nv = (u.voices as usize).clamp(1, MAX_UNISON);
            let amount = szabo_detune(u.detune);
            let (centre, side) = szabo_mix(u.mix);
            q.uni = nv;
            for j in 0..nv {
                let idx = ORDER[nv][j];
                let off = SZABO_OFFSETS[idx];
                q.udt[j] = (dt * (1.0 + off * amount)).min(0.45);
                top_dt = top_dt.max(q.udt[j]);
                let g = if idx == 3 { centre } else { side };
                let (a, b) = pan_gains((o.pan + u.spread.clamp(0.0, 1.0) * off / 0.11).clamp(-1.0, 1.0));
                q.ug[j] = g;
                q.ugl[j] = g * a;
                q.ugr[j] = g * b;
            }
            q.hp = u.high_pass.then(|| SvfCoefs::new(dt * rate, 0.707, rate));
        }
        if o.wave == Wave::Table {
            q.level = table.map(|t| t.level_for(top_dt)).unwrap_or(0);
        }
    }
    if let Some(off) = pwm {
        st.pwm_off = off;
    }
    st.started = true;
    plan
}

/// One sample of a wave at phase `ph`, phase step `d` (a pulse of width `w`; the table's wave
/// `pos` at table `level`).
#[inline(always)]
fn sample(wave: Wave, ph: f32, d: f32, w: f32, table: Option<&WaveTable>, level: usize, pos: f32) -> f32 {
    match wave {
        Wave::Saw => 2.0 * ph - 1.0 - polyblep(ph, d),
        Wave::Square => {
            let a = if ph < 0.5 { 1.0 } else { -1.0 };
            a + polyblep(ph, d) - polyblep((ph + 0.5).fract(), d)
        }
        Wave::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
        Wave::Sine => (2.0 * PI * ph).sin(),
        Wave::Pulse => {
            // High for the first `w` of the cycle: the rising edge at 0, the falling at `w`, each
            // corrected; the offset (2w - 1) taken out so the sound stays centred as the width moves.
            let a = if ph < w { 1.0 } else { -1.0 };
            let q = ph - w;
            let q = if q < 0.0 { q + 1.0 } else { q };
            a + polyblep(ph, d) - polyblep(q, d) - (2.0 * w - 1.0)
        }
        Wave::Table => match table {
            Some(t) => t.morph(pos, level, ph),
            None => 0.0,
        },
    }
}

/// The oscillators' samples `i0 .. i0 + bl.len()` of a block of `total`, ADDED into `bl` (and `br`
/// when `stereo`; mono sums ignore the pans). The classic waves without unison are worked exactly as
/// they were before the rung (the same operations in the same order, so the same sound).
pub(crate) fn fill(s: &SynthSpec, st: &mut OscBank, plan: &Plan, i0: usize, total: usize, stereo: bool, bl: &mut [f32], br: &mut [f32]) {
    let m = bl.len();
    let table = s.wavetable.as_ref().and_then(|w| w.table.as_deref());
    let inv = 1.0 / total.max(1) as f32;
    for k in 0..plan.no {
        let q = &plan.o[k];
        let ramp = |a: f32, b: f32, i: usize| a + (b - a) * (i0 + i) as f32 * inv;
        if q.uni == 0 {
            let d = q.dt;
            let mut ph = st.ph[k];
            for i in 0..m {
                let w = if q.wave == Wave::Pulse { ramp(q.w0, q.w1, i) } else { 0.5 };
                let pos = if q.wave == Wave::Table { ramp(plan.pos0, plan.pos1, i) } else { 0.0 };
                let y = sample(q.wave, ph, d, w, table, q.level, pos);
                let mut np = ph + d;
                if np >= 1.0 {
                    np -= 1.0;
                }
                ph = np;
                if stereo {
                    bl[i] += y * q.lv * q.pl;
                    br[i] += y * q.lv * q.pr;
                } else {
                    bl[i] += y * q.lv;
                }
            }
            st.ph[k] = ph;
            continue;
        }
        // Unison: every copy at its own step and pan, the sum high-passed at the note.
        let nv = q.uni;
        let mut ph = st.uph[k];
        let (hl, hr) = st.uhp[k].split_at_mut(1);
        let (hl, hr) = (&mut hl[0], &mut hr[0]);
        for i in 0..m {
            let w = if q.wave == Wave::Pulse { ramp(q.w0, q.w1, i) } else { 0.5 };
            let pos = if q.wave == Wave::Table { ramp(plan.pos0, plan.pos1, i) } else { 0.0 };
            let (mut sl, mut sr) = (0.0, 0.0);
            for j in 0..nv {
                let y = sample(q.wave, ph[j], q.udt[j], w, table, q.level, pos);
                let mut np = ph[j] + q.udt[j];
                if np >= 1.0 {
                    np -= 1.0;
                }
                ph[j] = np;
                if stereo {
                    sl += y * q.ugl[j];
                    sr += y * q.ugr[j];
                } else {
                    sl += y * q.ug[j];
                }
            }
            if let Some(c) = &q.hp {
                sl = hl.hp(c, sl);
                if stereo {
                    sr = hr.hp(c, sr);
                }
            }
            bl[i] += sl * q.lv;
            if stereo {
                br[i] += sr * q.lv;
            }
        }
        st.uph[k] = ph;
    }
}
