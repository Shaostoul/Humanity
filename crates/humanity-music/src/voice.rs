//! The voice pool: 64 voices made once, each a note being played by one instrument kind. A note-on
//! takes a free voice (or the quietest, steals it), a note-off releases it, and each block every
//! active voice adds its sound into the dry and reverb-send buffers. Nothing here allocates after
//! [`VoicePool::new`]: every kind's state lives inside its voice, and the plucked strings' delay
//! lines are one buffer cut into 64 pieces.

use std::f32::consts::PI;
use std::sync::Arc;

use crate::dsp::{db_to_amp, pan_gains, polyblep, Env, Lfo, Phasor, Svf, SvfCoefs};
use crate::instrument::{FilterKind, FilterSpec, Instrument, InstrumentKind, SampleBank, Wave};
use crate::rng::Rng;
use crate::tuning::note_hz;

/// Voices at most (docs/design/music.md, section 14).
pub const MAX_VOICES: usize = 64;
/// One plucked string's delay line: enough for the lowest note at 96 kHz.
const KS_LEN: usize = 4096;
const MAX_PARTIALS: usize = 12;
const MAX_MODES: usize = 8;

/// What every voice needs to know for one block.
pub struct BlockParams<'a> {
    pub rate: f32,
    /// A pitch multiplier over every note (a genre's tape wow and flutter; 1 = none).
    pub pitch_mul: f32,
    /// Beats per second (a filter swept in time with the music).
    pub beat_hz: f32,
    /// The choir's five formants this block: coefficients and gains.
    pub vowel: &'a [(SvfCoefs, f32); 5],
}

/// A note to start.
#[derive(Clone, Copy, Debug)]
pub struct NoteOn {
    /// The instrument's index in the library (already resolved past a missing sample pack).
    pub inst: u16,
    /// The role it plays (its index in `RoleKind::ALL`).
    pub role: u8,
    pub midi: f32,
    pub a4: f32,
    /// 0..1.
    pub vel: f32,
    /// -1 .. 1, before the instrument's own spread.
    pub pan: f32,
    /// How long it is held, seconds (0 = until a note-off by its tag).
    pub hold_s: f32,
    /// Identity for the note-off.
    pub tag: u32,
    /// The role's level as an amplitude.
    pub level: f32,
}

#[derive(Clone, Copy, Default)]
struct SynthSt {
    ph: [f32; 4],
    svf_l: Svf,
    svf_r: Svf,
    vib: Lfo,
    trem: Lfo,
    flt: Lfo,
}

#[derive(Clone, Copy, Default)]
struct Singer {
    ph: f32,
    det: f32,
    vib: Lfo,
    vib_rate: f32,
    drift: f32,
    drift_v: f32,
    lp: f32,
    forms: [Svf; 5],
    gl: f32,
    gr: f32,
}

#[derive(Clone, Copy, Default)]
struct ChoirSt {
    s: [Singer; 3],
    n: u8,
}

#[derive(Clone, Copy, Default)]
struct ModalSt {
    p: [Phasor; MAX_PARTIALS],
    start_amp: f32,
    n: u8,
    attack_n: u32,
    strike: f32,
    strike_coef: f32,
    strike_svf: Svf,
    strike_c: SvfCoefs,
}

#[derive(Clone, Copy, Default)]
struct MembraneSt {
    p: [Phasor; MAX_MODES],
    ratio: [f32; MAX_MODES],
    t60: [f32; MAX_MODES],
    n: u8,
    start_amp: f32,
    attack_n: u32,
    noise_env: f32,
    noise_coef: f32,
    noise_svf: Svf,
    noise_c: SvfCoefs,
}

#[derive(Clone, Copy, Default)]
struct FmSt {
    car: f32,
    md: f32,
    index: f32,
    index_coef: f32,
    tine: Phasor,
}

#[derive(Clone, Copy, Default)]
struct PluckSt {
    delay: f32,
    w: usize,
    prev: f32,
    gain: f32,
    body: Svf,
    body_c: SvfCoefs,
    peak: f32,
}

#[derive(Clone, Copy, Default)]
struct NoiseSt {
    svf: Svf,
    c: SvfCoefs,
    tone: Phasor,
    pop: f32,
    pop_coef: f32,
    hiss_lp: f32,
}

#[derive(Clone, Copy, Default)]
struct SamplerSt {
    zone: u32,
    pos: f64,
    step: f64,
}

#[derive(Clone, Copy, Default)]
enum St {
    #[default]
    Idle,
    Synth(SynthSt),
    Choir(ChoirSt),
    Modal(ModalSt),
    Membrane(MembraneSt),
    Fm(FmSt),
    Plucked(PluckSt),
    Noise(NoiseSt),
    Sampler(SamplerSt),
}

/// One voice.
#[derive(Clone, Copy)]
pub struct Voice {
    pub active: bool,
    pub inst: u16,
    pub role: u8,
    pub tag: u32,
    pub midi: f32,
    freq: f32,
    amp: f32,
    gl: f32,
    gr: f32,
    send: f32,
    /// Samples until it is let go; negative: held until its note-off.
    hold_left: i64,
    released: bool,
    age: u64,
    env: Env,
    st: St,
    rng: Rng,
}

impl Voice {
    fn idle(seed: u64) -> Self {
        Voice {
            active: false,
            inst: 0,
            role: 0,
            tag: 0,
            midi: 0.0,
            freq: 0.0,
            amp: 0.0,
            gl: 0.0,
            gr: 0.0,
            send: 0.0,
            hold_left: -1,
            released: false,
            age: 0,
            env: Env::default(),
            st: St::Idle,
            rng: Rng::new(seed),
        }
    }

    /// Whether it has been let go.
    pub fn released(&self) -> bool {
        self.released
    }
}

/// The pool.
pub struct VoicePool {
    pub voices: Vec<Voice>,
    ks: Vec<f32>,
    /// Notes started (a sampler's round robin).
    started: u32,
}

impl VoicePool {
    /// 64 idle voices (allocates here, once).
    pub fn new(seed: u64) -> Self {
        let mut r = Rng::new(seed);
        VoicePool { voices: (0..MAX_VOICES).map(|_| Voice::idle(r.next_u64())).collect(), ks: vec![0.0; MAX_VOICES * KS_LEN], started: 0 }
    }

    /// How many voices are sounding.
    pub fn active(&self) -> usize {
        self.voices.iter().filter(|v| v.active).count()
    }

    /// Silence every voice at once.
    pub fn clear(&mut self) {
        for v in self.voices.iter_mut() {
            v.active = false;
        }
    }

    /// Let go every voice holding `tag`.
    pub fn note_off(&mut self, tag: u32, insts: &[Instrument], rate: f32) {
        for v in self.voices.iter_mut() {
            if v.active && v.tag == tag && !v.released {
                release(v, insts, rate);
            }
        }
    }

    /// Let go every voice of role `role`.
    pub fn release_role(&mut self, role: u8, insts: &[Instrument], rate: f32) {
        for v in self.voices.iter_mut() {
            if v.active && v.role == role && !v.released {
                release(v, insts, rate);
            }
        }
    }

    /// Start a note. `banks[i]` is instrument i's installed recordings (a `Sampler` whose bank is
    /// missing is resolved to its fallback before it gets here).
    pub fn note_on(&mut self, n: NoteOn, insts: &[Instrument], banks: &[Option<Arc<dyn SampleBank>>], rate: f32) {
        let Some(inst) = insts.get(n.inst as usize) else { return };
        // A recorded instrument that has no recording for this note (exact keys, or no bank) plays
        // it on its fallback, so no note is lost: a recorded fallback that has the note (the Hang's
        // missing notes on the recorded steel pan, data/music/parts.ron's `fallback_part`), else the
        // first synthesised one along the chain. Never more than two steps, so no chain can loop.
        if let InstrumentKind::Sampler(s) = &inst.kind {
            let midi = n.midi.round() as i32;
            let rr = self.started.wrapping_add(1);
            let has = |i: u16| banks.get(i as usize).and_then(|b| b.as_ref()).is_some_and(|b| b.pick(midi, n.vel, rr).is_some());
            if !has(n.inst) {
                let synthesised = |i: u16| !matches!(insts.get(i as usize).map(|x| &x.kind), Some(InstrumentKind::Sampler(_)));
                let fb = s.fallback_index.filter(|&fb| fb != n.inst).and_then(|fb| match insts.get(fb as usize).map(|x| &x.kind) {
                    Some(InstrumentKind::Sampler(_)) if has(fb) => Some(fb),
                    Some(InstrumentKind::Sampler(f)) => f.fallback_index.filter(|&x| x != fb && x != n.inst && synthesised(x)),
                    Some(_) => Some(fb),
                    None => None,
                });
                if let Some(fb) = fb {
                    self.note_on(NoteOn { inst: fb, ..n }, insts, banks, rate);
                }
                return;
            }
        }
        // A free voice, else the quietest: a released one before a held one, then the oldest.
        let slot = match self.voices.iter().position(|v| !v.active) {
            Some(i) => i,
            None => {
                let mut best = 0;
                let mut best_key = (false, 0u64);
                for (i, v) in self.voices.iter().enumerate() {
                    let key = (v.released, v.age);
                    if key > best_key {
                        best_key = key;
                        best = i;
                    }
                }
                best
            }
        };
        self.started = self.started.wrapping_add(1);
        let rr = self.started;
        let ks = &mut self.ks[slot * KS_LEN..(slot + 1) * KS_LEN];
        let v = &mut self.voices[slot];
        let f = note_hz(n.midi, n.a4);
        let spread = inst.pan_spread * v.rng.bipolar();
        let (gl, gr) = pan_gains((n.pan + spread).clamp(-1.0, 1.0));
        v.active = true;
        v.inst = n.inst;
        v.role = n.role;
        v.tag = n.tag;
        v.midi = n.midi;
        v.freq = f;
        v.amp = n.vel.clamp(0.0, 1.0) * db_to_amp(inst.level_db) * n.level;
        v.gl = gl;
        v.gr = gr;
        v.send = inst.reverb_send.clamp(0.0, 1.0);
        v.hold_left = if n.hold_s > 0.0 { (n.hold_s * rate) as i64 } else { -1 };
        v.released = false;
        v.age = 0;
        let env = |e: &crate::instrument::EnvSpec| Env::new(e.attack_s, e.decay_s, e.sustain, e.release_s, rate);
        v.st = match &inst.kind {
            InstrumentKind::Synth(s) => {
                v.env = env(&s.envelope);
                let mut st = SynthSt { vib: Lfo::new(v.rng.f32()), trem: Lfo::new(v.rng.f32()), flt: Lfo::new(v.rng.f32()), ..Default::default() };
                for p in st.ph.iter_mut() {
                    *p = v.rng.f32();
                }
                St::Synth(st)
            }
            InstrumentKind::Choir(c) => {
                v.env = env(&c.envelope);
                let n_s = c.singers.clamp(1, 3);
                let mut st = ChoirSt { n: n_s, ..Default::default() };
                for (j, s) in st.s.iter_mut().take(n_s as usize).enumerate() {
                    let side = if n_s == 1 { 0.0 } else { j as f32 / (n_s - 1) as f32 * 2.0 - 1.0 };
                    let det = side * 5.0 + v.rng.bipolar() * 2.0;
                    let (gl, gr) = pan_gains(side * 0.35 + v.rng.bipolar() * 0.15);
                    *s = Singer {
                        ph: v.rng.f32(),
                        det: 2f32.powf(det / 1200.0),
                        vib: Lfo::new(v.rng.f32()),
                        vib_rate: c.vibrato.0 + v.rng.f32() * 0.6,
                        gl,
                        gr,
                        ..Default::default()
                    };
                }
                St::Choir(st)
            }
            InstrumentKind::Modal(m) => {
                v.env = Env::new(0.0, 0.0, 1.0, 0.1, rate);
                let mut st = ModalSt { attack_n: ((m.attack_s * rate) as u32).max(1), ..Default::default() };
                let scale = if m.decay_pitch_scale != 0.0 { (f / 440.0).powf(-m.decay_pitch_scale) } else { 1.0 };
                let mut k = 0;
                for &(ratio, a, t60) in m.partials.iter() {
                    let fr = f * ratio;
                    if fr >= rate * 0.45 || k >= MAX_PARTIALS {
                        continue;
                    }
                    st.p[k] = Phasor::new(fr, a, t60 * scale, rate);
                    st.start_amp = st.start_amp.max(a);
                    k += 1;
                }
                st.n = k as u8;
                st.strike = m.strike_noise;
                st.strike_coef = (-1.0 / (m.strike_ms.max(0.5) * 0.001 * rate)).exp();
                st.strike_c = SvfCoefs::new((f * 3.0).min(8000.0), 0.8, rate);
                St::Modal(st)
            }
            InstrumentKind::Membrane(m) => {
                v.env = Env::new(0.0, 0.0, 1.0, 0.1, rate);
                let mut st = MembraneSt { attack_n: ((m.attack_s * rate) as u32).max(1), ..Default::default() };
                let mut k = 0;
                for &(ratio, a, t60) in m.modes.iter() {
                    if f * ratio * (1.0 + m.pitch_drop) >= rate * 0.45 || k >= MAX_MODES {
                        continue;
                    }
                    st.p[k] = Phasor::new(f * ratio * (1.0 + m.pitch_drop), a, t60, rate);
                    st.ratio[k] = ratio;
                    st.t60[k] = t60;
                    st.start_amp = st.start_amp.max(a);
                    k += 1;
                }
                st.n = k as u8;
                st.noise_env = m.noise;
                st.noise_coef = (-1.0 / (m.noise_decay_s.max(0.001) * rate)).exp();
                st.noise_c = SvfCoefs::new(m.noise_hz, 0.7, rate);
                St::Membrane(st)
            }
            InstrumentKind::Fm(fm) => {
                v.env = env(&fm.envelope);
                let tine = fm.tine.map(|(r, a, t)| Phasor::new((f * r).min(rate * 0.45), a, t, rate)).unwrap_or_default();
                St::Fm(FmSt { car: 0.0, md: 0.0, index: fm.index, index_coef: (-1.0 / (fm.index_decay_s.max(0.001) * rate)).exp(), tine })
            }
            InstrumentKind::Plucked(p) => {
                v.env = Env::new(0.0, 0.0, 1.0, 0.1, rate);
                let delay = (rate / f - 0.5).clamp(2.0, (KS_LEN - 2) as f32);
                let len = delay.ceil() as usize + 1;
                // The pluck: noise, low-passed by how bright it is, its average taken out.
                let b = 0.08 + 0.9 * p.brightness.clamp(0.0, 1.0);
                let mut y = 0.0;
                let mut mean = 0.0;
                for s in ks.iter_mut().take(len) {
                    y += b * (v.rng.bipolar() - y);
                    *s = y;
                    mean += y;
                }
                mean /= len as f32;
                let mut peak = 0f32;
                for s in ks.iter_mut().take(len) {
                    *s -= mean;
                    peak = peak.max(s.abs());
                }
                if peak > 0.0 {
                    for s in ks.iter_mut().take(len) {
                        *s /= peak;
                    }
                }
                for s in ks.iter_mut().skip(len) {
                    *s = 0.0;
                }
                St::Plucked(PluckSt {
                    delay,
                    w: len % KS_LEN,
                    prev: 0.0,
                    gain: 10f32.powf(-3.0 / (f * p.decay_s.max(0.05))),
                    body: Svf::default(),
                    body_c: SvfCoefs::new(p.body_hz.max(40.0), 1.2, rate),
                    peak: 1.0,
                })
            }
            InstrumentKind::Noise(nz) => {
                v.env = env(&nz.envelope);
                St::Noise(NoiseSt {
                    svf: Svf::default(),
                    c: filter_coefs(&nz.filter, f, 0.0, 0.0, rate),
                    tone: if nz.tone_hz > 0.0 { Phasor::new(nz.tone_hz, nz.tone_level, nz.tone_decay_s, rate) } else { Phasor::default() },
                    pop: 0.0,
                    pop_coef: (-1.0 / (0.0007 * rate)).exp(),
                    hiss_lp: 0.0,
                })
            }
            InstrumentKind::Sampler(s) => {
                v.env = env(&s.envelope);
                let bank = banks.get(n.inst as usize).and_then(|b| b.as_ref());
                match bank.and_then(|b| b.pick(n.midi.round() as i32, n.vel, rr).map(|z| (z, b.zone(z)))) {
                    Some((zi, zone)) if !zone.frames.is_empty() => {
                        // Retuned by playback rate: the note wanted over the note recorded, and the
                        // recording's own rate over the output's. An unpitched hit (recorded_hz 0)
                        // plays at its recorded rate.
                        let pitch = if zone.recorded_hz > 0.0 { f as f64 / zone.recorded_hz as f64 } else { 1.0 };
                        let step = pitch * (zone.rate as f64 / rate as f64);
                        St::Sampler(SamplerSt { zone: zi as u32, pos: 0.0, step })
                    }
                    _ => {
                        v.active = false;
                        St::Idle
                    }
                }
            }
        };
    }

    /// Add every active voice's next `n` samples into the buffers.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        insts: &[Instrument],
        banks: &[Option<Arc<dyn SampleBank>>],
        p: &BlockParams,
        dry_l: &mut [f32],
        dry_r: &mut [f32],
        send_l: &mut [f32],
        send_r: &mut [f32],
        n: usize,
    ) {
        for (slot, v) in self.voices.iter_mut().enumerate() {
            if !v.active {
                continue;
            }
            let Some(inst) = insts.get(v.inst as usize) else {
                v.active = false;
                continue;
            };
            if v.hold_left >= 0 {
                v.hold_left -= n as i64;
                if v.hold_left < 0 && !v.released {
                    release(v, insts, p.rate);
                    v.hold_left = -1;
                }
            }
            let ks = &mut self.ks[slot * KS_LEN..(slot + 1) * KS_LEN];
            let bank = banks.get(v.inst as usize).and_then(|b| b.as_deref());
            render_voice(v, inst, bank, ks, p, &mut dry_l[..n], &mut dry_r[..n], &mut send_l[..n], &mut send_r[..n]);
            v.age += n as u64;
        }
    }
}

fn filter_coefs(f: &FilterSpec, note_hz: f32, lfo: f32, env: f32, rate: f32) -> SvfCoefs {
    let fc = f.cutoff_hz + f.key_track * note_hz + f.lfo_depth_hz * (0.5 + 0.5 * lfo) + f.env_hz * env;
    SvfCoefs::new(fc.max(20.0), f.q, rate)
}

#[inline]
fn filt(kind: FilterKind, s: &mut Svf, c: &SvfCoefs, x: f32) -> f32 {
    match kind {
        FilterKind::LowPass => s.lp(c, x),
        FilterKind::BandPass => s.bp(c, x),
        FilterKind::HighPass => s.hp(c, x),
    }
}

/// Let a voice go: its envelope releases, a struck bar or a string is damped.
fn release(v: &mut Voice, insts: &[Instrument], rate: f32) {
    v.released = true;
    v.env.release();
    let Some(inst) = insts.get(v.inst as usize) else { return };
    match (&inst.kind, &mut v.st) {
        (InstrumentKind::Modal(m), St::Modal(st)) if m.damping_on_release > 1.0 => {
            let scale = if m.decay_pitch_scale != 0.0 { (v.freq / 440.0).powf(-m.decay_pitch_scale) } else { 1.0 };
            let mut k = 0;
            for &(ratio, _, t60) in m.partials.iter() {
                if k >= st.n as usize {
                    break;
                }
                if v.freq * ratio >= rate * 0.45 {
                    continue;
                }
                st.p[k].retune(v.freq * ratio, t60 * scale / m.damping_on_release, rate);
                k += 1;
            }
        }
        (InstrumentKind::Plucked(p), St::Plucked(st)) => {
            st.gain = 10f32.powf(-3.0 * p.damping_on_release.max(1.0) / (v.freq * p.decay_s.max(0.05)));
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn render_voice(
    v: &mut Voice,
    inst: &Instrument,
    bank: Option<&dyn SampleBank>,
    ks: &mut [f32],
    p: &BlockParams,
    dl: &mut [f32],
    dr: &mut [f32],
    sl: &mut [f32],
    sr: &mut [f32],
) {
    let n = dl.len();
    let rate = p.rate;
    let (gl, gr, send) = (v.gl, v.gr, v.send);
    let mut out = |i: usize, l: f32, r: f32| {
        dl[i] += l * gl;
        dr[i] += r * gr;
        sl[i] += l * gl * send;
        sr[i] += r * gr * send;
    };
    match (&inst.kind, &mut v.st) {
        (InstrumentKind::Synth(s), St::Synth(st)) => {
            let vib = s.vibrato.map(|(r, d)| st.vib.advance(r, n, rate) * d).unwrap_or(0.0);
            let f = v.freq * p.pitch_mul * 2f32.powf(vib / 1200.0);
            let mut dt = [0f32; 4];
            let mut lv = [0f32; 4];
            let mut pl = [0f32; 4];
            let mut pr = [0f32; 4];
            let mut wave = [Wave::Sine; 4];
            let no = s.oscillators.len().min(4);
            for (k, o) in s.oscillators.iter().take(4).enumerate() {
                dt[k] = (f * 2f32.powf((o.semitones * 100.0 + o.detune_cents) / 1200.0) / rate).min(0.45);
                lv[k] = o.level;
                let (a, b) = pan_gains(o.pan);
                pl[k] = a;
                pr[k] = b;
                wave[k] = o.wave;
            }
            let coefs = s.filter.map(|fl| {
                let lfo = if fl.lfo_per_beat > 0.0 {
                    st.flt.advance(fl.lfo_per_beat * p.beat_hz, n, rate)
                } else if fl.lfo_hz > 0.0 {
                    st.flt.advance(fl.lfo_hz, n, rate)
                } else {
                    0.0
                };
                (fl.kind, filter_coefs(&fl, f, lfo, v.env.level(), rate))
            });
            let trem = s.tremolo.map(|(r, d)| 1.0 - d * (0.5 + 0.5 * st.trem.advance(r, n, rate))).unwrap_or(1.0);
            let g = v.amp * trem;
            let drive = s.drive;
            for i in 0..n {
                let e = v.env.next();
                let (mut xl, mut xr) = (0.0, 0.0);
                for k in 0..no {
                    let ph = st.ph[k];
                    let d = dt[k];
                    let y = match wave[k] {
                        Wave::Saw => 2.0 * ph - 1.0 - polyblep(ph, d),
                        Wave::Square => {
                            let a = if ph < 0.5 { 1.0 } else { -1.0 };
                            a + polyblep(ph, d) - polyblep((ph + 0.5).fract(), d)
                        }
                        Wave::Triangle => 1.0 - 4.0 * (ph - 0.5).abs(),
                        Wave::Sine => (2.0 * PI * ph).sin(),
                    };
                    let mut np = ph + d;
                    if np >= 1.0 {
                        np -= 1.0;
                    }
                    st.ph[k] = np;
                    xl += y * lv[k] * pl[k];
                    xr += y * lv[k] * pr[k];
                }
                if let Some((kind, c)) = &coefs {
                    xl = filt(*kind, &mut st.svf_l, c, xl);
                    xr = filt(*kind, &mut st.svf_r, c, xr);
                }
                if drive > 0.0 {
                    let k = 1.0 + drive * 3.0;
                    xl = (xl * k).tanh() / k.sqrt();
                    xr = (xr * k).tanh() / k.sqrt();
                }
                out(i, xl * e * g, xr * e * g);
            }
            if v.env.done() {
                v.active = false;
            }
        }
        (InstrumentKind::Choir(c), St::Choir(st)) => {
            let lp_a = (-2.0 * PI * 1300.0 / rate).exp();
            let f = v.freq * p.pitch_mul;
            // Vibrato once a block per singer (a 5 Hz wobble sampled every 1.3 ms is smooth) and
            // faded in over the first 1.5 s: no sine per sample.
            let vib_depth = (c.vibrato.1 / 1200.0) * 0.69 * (v.age as f32 / (1.5 * rate)).min(1.0);
            let mut vib = [0f32; 3];
            for (j, s) in st.s.iter_mut().take(st.n as usize).enumerate() {
                vib[j] = s.vib.advance(s.vib_rate, n, rate) * vib_depth;
            }
            for i in 0..n {
                let e = v.env.next();
                let age = v.age + i as u64;
                let (mut yl, mut yr) = (0.0, 0.0);
                for (j, s) in st.s.iter_mut().take(st.n as usize).enumerate() {
                    let vib = vib[j];
                    if age & 255 == 0 {
                        s.drift_v += v.rng.bipolar() * 0.0002 - s.drift * 0.02;
                        s.drift_v *= 0.9;
                    }
                    s.drift += s.drift_v / 256.0;
                    let d = (f * s.det * (1.0 + vib + s.drift) / rate).min(0.45);
                    let mut src = 2.0 * s.ph - 1.0 - polyblep(s.ph, d);
                    s.ph += d;
                    if s.ph >= 1.0 {
                        s.ph -= 1.0;
                    }
                    src += v.rng.bipolar() * c.breath * 0.5;
                    s.lp = s.lp * lp_a + src * (1.0 - lp_a);
                    let mut y = 0.0;
                    for k in 0..5 {
                        let (ref cf, g) = p.vowel[k];
                        // A formant 40 dB down (the fifth, at -50 dB in both vowels) adds nothing
                        // anyone hears: skipped.
                        if g > 0.01 {
                            y += s.forms[k].bp(cf, s.lp) * g;
                        }
                    }
                    yl += y * s.gl;
                    yr += y * s.gr;
                }
                let g = e * v.amp;
                out(i, yl * g, yr * g);
            }
            if v.env.done() {
                v.active = false;
            }
        }
        (InstrumentKind::Modal(_), St::Modal(st)) => {
            let m = st.n as usize;
            for i in 0..n {
                let mut y = 0.0;
                for ph in st.p.iter_mut().take(m) {
                    y += ph.tick();
                }
                let age = v.age as u32 + i as u32;
                let a = if age < st.attack_n { age as f32 / st.attack_n as f32 } else { 1.0 };
                if st.strike > 1e-5 {
                    y += st.strike_svf.bp(&st.strike_c, v.rng.bipolar()) * st.strike;
                    st.strike *= st.strike_coef;
                }
                // No envelope: a struck bar rings by its partials' own decays (a note let go is
                // damped by retuning them shorter, `release`).
                let s = y * a * v.amp;
                out(i, s, s);
            }
            let left = st.p.iter().take(m).map(|q| q.amplitude()).fold(0.0, f32::max);
            // Done 60 dB down (a bell at a soft velocity is then near -80 dBFS).
            if left < st.start_amp * 1e-3 || m == 0 {
                v.active = false;
            }
        }
        (InstrumentKind::Membrane(mb), St::Membrane(st)) => {
            // The pitch falls back from sharp as the skin's tension relaxes: retuned once a block.
            let t = v.age as f32 / rate;
            let m = st.n as usize;
            let tau = mb.pitch_drop_s.max(0.001);
            // Retuned only while the drop is still moving (about ten time constants): after that the
            // modes ring at their own pitch and a block costs only their ticks.
            if t < 10.0 * tau + n as f32 / rate {
                let mul = 1.0 + mb.pitch_drop * (-t / tau).exp();
                for k in 0..m {
                    st.p[k].retune(v.freq * st.ratio[k] * mul, st.t60[k], rate);
                }
            }
            for i in 0..n {
                let mut y = 0.0;
                for ph in st.p.iter_mut().take(m) {
                    y += ph.tick();
                }
                if st.noise_env > 1e-5 {
                    y += st.noise_svf.lp(&st.noise_c, v.rng.bipolar()) * st.noise_env;
                    st.noise_env *= st.noise_coef;
                }
                let age = v.age as u32 + i as u32;
                let a = if age < st.attack_n { 0.5 - 0.5 * (PI * age as f32 / st.attack_n as f32).cos() } else { 1.0 };
                let s = y * a * v.amp;
                out(i, s, s);
            }
            let left = st.p.iter().take(m).map(|q| q.amplitude()).fold(0.0, f32::max);
            if left < st.start_amp * 1e-3 || m == 0 {
                v.active = false;
            }
        }
        (InstrumentKind::Fm(fm), St::Fm(st)) => {
            let f = v.freq * p.pitch_mul;
            let dc = f / rate;
            let dm = f * fm.ratio / rate;
            let has_tine = fm.tine.is_some();
            for i in 0..n {
                let e = v.env.next();
                st.index = fm.index_sustain + (st.index - fm.index_sustain) * st.index_coef;
                let y = (2.0 * PI * st.car + st.index * (2.0 * PI * st.md).sin()).sin();
                st.car = (st.car + dc).fract();
                st.md = (st.md + dm).fract();
                let t = if has_tine { st.tine.tick() } else { 0.0 };
                let s = (y + t) * e * v.amp;
                out(i, s, s);
            }
            if v.env.done() {
                v.active = false;
            }
        }
        (InstrumentKind::Plucked(pk), St::Plucked(st)) => {
            let mut peak = 0f32;
            let d = st.delay;
            for i in 0..n {
                let mut rp = st.w as f32 - d;
                if rp < 0.0 {
                    rp += KS_LEN as f32;
                }
                let i0 = rp as usize;
                let fr = rp - i0 as f32;
                let a = ks[i0 % KS_LEN];
                let b = ks[(i0 + 1) % KS_LEN];
                let x = a + (b - a) * fr;
                let y = 0.5 * (x + st.prev) * st.gain;
                st.prev = x;
                ks[st.w] = y;
                st.w = (st.w + 1) % KS_LEN;
                let mut s = y;
                if pk.body_level > 0.0 {
                    s += st.body.bp(&st.body_c, y) * pk.body_level;
                }
                peak = peak.max(s.abs());
                let s = s * v.amp;
                out(i, s, s);
            }
            st.peak = peak;
            if v.age > (0.05 * rate) as u64 && peak < 2e-4 {
                v.active = false;
            }
        }
        (InstrumentKind::Noise(nz), St::Noise(st)) => {
            let has_tone = nz.tone_hz > 0.0;
            let pop_p = nz.crackle_per_s / rate;
            if nz.filter.env_hz != 0.0 {
                // A sweep that follows the envelope (a riser opening as it builds): once a block.
                st.c = filter_coefs(&nz.filter, v.freq, 0.0, v.env.level(), rate);
            }
            for i in 0..n {
                let e = v.env.next();
                let mut y = filt(nz.filter.kind, &mut st.svf, &st.c, v.rng.bipolar()) * nz.noise_level;
                if has_tone {
                    y += st.tone.tick();
                }
                if pop_p > 0.0 {
                    if v.rng.f32() < pop_p {
                        st.pop = v.rng.range(0.25, 1.0) * if v.rng.chance(0.5) { 1.0 } else { -1.0 };
                    }
                    y += st.pop;
                    st.pop *= st.pop_coef;
                }
                if nz.hiss > 0.0 {
                    st.hiss_lp += 0.3 * (v.rng.bipolar() - st.hiss_lp);
                    y += st.hiss_lp * nz.hiss;
                }
                let s = y * e * v.amp;
                out(i, s, s);
            }
            if v.env.done() {
                v.active = false;
            }
        }
        (InstrumentKind::Sampler(_), St::Sampler(st)) => {
            let Some(zone) = bank.map(|b| b.zone(st.zone as usize)) else {
                v.active = false;
                return;
            };
            let len = zone.frames.len();
            let step = st.step * p.pitch_mul as f64;
            // A crossfaded loop (SampleZone::loop_xfade) while the note is held: (start, end, the
            // fade's length), all in frames.
            let soft = match zone.loop_range {
                Some((a, b)) if zone.loop_xfade > 0 && !v.released && a >= zone.loop_xfade && b > a + zone.loop_xfade && b <= len => Some((a as f64, b as f64, zone.loop_xfade as f64)),
                _ => None,
            };
            let sc = crate::instrument::SAMPLE_SCALE;
            let at = |pos: f64| -> (f32, f32) {
                let idx = (pos.max(0.0) as usize).min(len - 1);
                let fr = (pos - idx as f64).clamp(0.0, 1.0) as f32;
                let [al, ar] = zone.frames[idx];
                let [bl, br] = zone.frames[(idx + 1).min(len - 1)];
                let (al, ar, bl, br) = (al as f32 * sc, ar as f32 * sc, bl as f32 * sc, br as f32 * sc);
                (al + (bl - al) * fr, ar + (br - ar) * fr)
            };
            if let Some((a, b, x)) = soft {
                for i in 0..n {
                    let e = v.env.next();
                    if st.pos >= b {
                        st.pos -= b - a;
                    }
                    let (mut l, mut r) = at(st.pos);
                    let fade_from = b - x;
                    if st.pos >= fade_from {
                        // Into the frames just before the loop's start, which play on from there.
                        let t = ((st.pos - fade_from) / x) as f32;
                        let (l2, r2) = at(st.pos - (b - a));
                        let (g1, g2) = ((1.0 - t).max(0.0).sqrt(), t.min(1.0).sqrt());
                        l = l * g1 + l2 * g2;
                        r = r * g1 + r2 * g2;
                    }
                    st.pos += step;
                    let g = e * v.amp;
                    out(i, l * g, r * g);
                }
                if v.env.done() {
                    v.active = false;
                }
                return;
            }
            for i in 0..n {
                let e = v.env.next();
                if st.pos as usize + 1 >= len {
                    match zone.loop_range {
                        Some((a, b)) if !v.released && b > a + 1 && b <= len => {
                            st.pos -= (b - a) as f64;
                        }
                        _ => {
                            v.active = false;
                            return;
                        }
                    }
                }
                let (l, r) = at(st.pos);
                st.pos += step;
                let g = e * v.amp;
                out(i, l * g, r * g);
            }
            if v.env.done() {
                v.active = false;
            }
        }
        _ => {
            v.active = false;
        }
    }
}
