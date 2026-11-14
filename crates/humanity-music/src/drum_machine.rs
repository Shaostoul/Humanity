//! A drum machine's voices (docs/design/music.md section 7f, rung E4), synthesised in the style of
//! the analogue machines (instrument.rs `DrumMachineSpec` says which published circuit descriptions
//! each part follows): tones whose pitch falls as they ring, a beater's click, filtered noise struck
//! once or in a clap's quick bursts, and six square waves tuned inharmonically for the metal of a
//! hi-hat. Each part is optional, so a kick, a snare, a clap, a hat and a tom are each a data file of
//! one kind. Plain arithmetic per sample, no allocation, as every voice (voice.rs).

use crate::dsp::{fast_sin, fast_tanh, polyblep, Svf, SvfCoefs};
use crate::instrument::{DrumMachineSpec, FilterKind};
use crate::rng::Rng;

const MAX_TONES: usize = 2;
const MAX_METAL: usize = 6;

/// A level falling 60 dB over `t` seconds at `rate`: the factor each sample.
fn per_sample_60db(t: f32, rate: f32) -> f32 {
    (1e-3f32.ln() / (t.max(0.001) * rate)).exp()
}

/// One drum hit's state.
#[derive(Clone, Copy, Default)]
pub(crate) struct DrumSt {
    // The tones: phase, step at their own pitch, level and its fall, how far sharp (falling to 0).
    tph: [f32; MAX_TONES],
    tdt: [f32; MAX_TONES],
    tamp: [f32; MAX_TONES],
    tdec: [f32; MAX_TONES],
    tsharp: [f32; MAX_TONES],
    tsharp_c: [f32; MAX_TONES],
    nt: u8,
    drive_k: f32,
    drive_norm: f32,
    // The click: its level and fall, and its low-pass.
    click: f32,
    click_c: f32,
    click_svf: Svf,
    click_f: SvfCoefs,
    // The noise: level, fall, filters; a clap's bursts.
    nenv: f32,
    ncoef: f32,
    nsvf: Svf,
    nhp: Svf,
    nf: SvfCoefs,
    nhf: SvfCoefs,
    has_hp: bool,
    bursts_left: u8,
    burst_n: u32,
    burst_ctr: u32,
    burst_c: f32,
    tail_c: f32,
    // The metal: six squares, their envelope, a band-pass and a high-pass.
    mph: [f32; MAX_METAL],
    mdt: [f32; MAX_METAL],
    nm: u8,
    menv: f32,
    mcoef: f32,
    mbp: Svf,
    mhp: Svf,
    mbpf: SvfCoefs,
    mhpf: SvfCoefs,
    /// A choke's fade (1: none), and its fall each sample once choked.
    choke: f32,
    choke_c: f32,
}

/// A hit of `spec` at the note's frequency `f`.
pub(crate) fn start(spec: &DrumMachineSpec, f: f32, rate: f32, rng: &mut Rng) -> DrumSt {
    let mut st = DrumSt { choke: 1.0, choke_c: 1.0, ..Default::default() };
    for (j, t) in spec.tones.iter().take(MAX_TONES).enumerate() {
        st.tdt[j] = (f * t.ratio / rate).min(0.45);
        st.tamp[j] = t.level;
        st.tdec[j] = per_sample_60db(t.decay_s, rate);
        st.tsharp[j] = (t.pitch - 1.0).max(-0.9);
        st.tsharp_c[j] = (-1.0 / (t.pitch_s.max(0.0005) * rate)).exp();
        st.tph[j] = 0.0;
        st.nt = j as u8 + 1;
    }
    let k = 1.0 + spec.drive.clamp(0.0, 1.0) * 5.0;
    st.drive_k = k;
    st.drive_norm = 1.0 / fast_tanh(k);
    st.click = spec.click;
    st.click_c = (-1.0 / (spec.click_ms.max(0.1) * 0.001 * rate)).exp();
    st.click_f = SvfCoefs::new(5000.0, 0.7, rate);
    if let Some(nz) = &spec.noise {
        st.nf = SvfCoefs::new(nz.hz, nz.q, rate);
        st.has_hp = nz.hp_hz > 0.0;
        st.nhf = SvfCoefs::new(nz.hp_hz.max(20.0), 0.707, rate);
        st.tail_c = per_sample_60db(nz.decay_s, rate);
        if nz.bursts > 0 {
            // A clap: each burst falls about 35 dB before the next, then the tail.
            st.bursts_left = nz.bursts;
            st.burst_n = ((nz.burst_ms.max(1.0) * 0.001 * rate) as u32).max(1);
            st.burst_c = (-4.0 / st.burst_n as f32).exp();
            st.nenv = nz.level;
            st.ncoef = st.burst_c;
        } else {
            st.nenv = nz.level;
            st.ncoef = st.tail_c;
        }
    }
    if let Some(m) = &spec.metal {
        for (j, hz) in m.hz.iter().take(MAX_METAL).enumerate() {
            st.mdt[j] = (hz * m.tune / rate).min(0.45);
            st.mph[j] = rng.f32();
            st.nm = j as u8 + 1;
        }
        st.menv = m.level;
        st.mcoef = per_sample_60db(m.decay_s, rate);
        st.mbpf = SvfCoefs::new(m.bp_hz, m.bp_q, rate);
        st.mhpf = SvfCoefs::new(m.hp_hz, 0.707, rate);
    }
    st
}

/// Cut a hit short (another of its choke group struck): it fades over 4 ms.
pub(crate) fn choke(st: &mut DrumSt, rate: f32) {
    st.choke_c = (-1.0 / (0.004 * rate)).exp();
}

/// Render `n` samples of a hit, each handed to `out` (index, mono sample); false once it has died
/// away (every part 60 dB down).
pub(crate) fn render(spec: &DrumMachineSpec, st: &mut DrumSt, amp: f32, n: usize, rng: &mut Rng, mut out: impl FnMut(usize, f32)) -> bool {
    let nt = st.nt as usize;
    let nm = st.nm as usize;
    let drive = st.drive_k > 1.0;
    let noise = spec.noise.as_ref();
    let metal_level = spec.metal.as_ref().map(|m| m.level).unwrap_or(0.0);
    for i in 0..n {
        let mut y = 0.0;
        if nt > 0 {
            let mut t = 0.0;
            for j in 0..nt {
                t += fast_sin(st.tph[j]) * st.tamp[j];
                let mut ph = st.tph[j] + st.tdt[j] * (1.0 + st.tsharp[j]);
                if ph >= 1.0 {
                    ph -= 1.0;
                }
                st.tph[j] = ph;
                st.tamp[j] *= st.tdec[j];
                st.tsharp[j] *= st.tsharp_c[j];
            }
            y = if drive { fast_tanh(t * st.drive_k) * st.drive_norm } else { t };
        }
        if st.click > 1e-5 {
            y += st.click_svf.lp(&st.click_f, rng.bipolar()) * st.click * 2.0;
            st.click *= st.click_c;
        }
        if let Some(nz) = noise {
            if st.nenv > 1e-6 || st.bursts_left > 0 {
                let mut x = rng.bipolar();
                x = match nz.filter {
                    FilterKind::LowPass => st.nsvf.lp(&st.nf, x),
                    FilterKind::BandPass => st.nsvf.bp(&st.nf, x),
                    FilterKind::HighPass => st.nsvf.hp(&st.nf, x),
                };
                if st.has_hp {
                    x = st.nhp.hp(&st.nhf, x);
                }
                y += x * st.nenv;
                st.nenv *= st.ncoef;
                if st.bursts_left > 0 {
                    st.burst_ctr += 1;
                    if st.burst_ctr >= st.burst_n {
                        st.burst_ctr = 0;
                        st.bursts_left -= 1;
                        if st.bursts_left > 0 {
                            st.nenv = nz.level;
                        } else {
                            // The last strike rings on as the tail.
                            st.nenv = nz.level * nz.tail;
                            st.ncoef = st.tail_c;
                        }
                    }
                }
            }
        }
        if nm > 0 && st.menv > 1e-6 {
            let mut x = 0.0;
            for j in 0..nm {
                let ph = st.mph[j];
                let d = st.mdt[j];
                let a = if ph < 0.5 { 1.0 } else { -1.0 };
                let q = if ph < 0.5 { ph + 0.5 } else { ph - 0.5 };
                x += a + polyblep(ph, d) - polyblep(q, d);
                let mut np = ph + d;
                if np >= 1.0 {
                    np -= 1.0;
                }
                st.mph[j] = np;
            }
            x = st.mhp.hp(&st.mhpf, st.mbp.bp(&st.mbpf, x * (1.0 / MAX_METAL as f32)));
            y += x * st.menv;
            st.menv *= st.mcoef;
        }
        out(i, y * amp * st.choke);
        st.choke *= st.choke_c;
    }
    let tones_left = st.tamp.iter().take(nt).fold(0.0f32, |a, &b| a.max(b));
    let alive = tones_left > 1e-3 * spec.tones.iter().map(|t| t.level).fold(0.0, f32::max).max(1e-6)
        || st.click > 1e-4
        || st.bursts_left > 0
        || (noise.is_some() && st.nenv > 1e-3 * noise.map(|z| z.level).unwrap_or(1.0).max(1e-6))
        || (nm > 0 && st.menv > 1e-3 * metal_level.max(1e-6));
    alive && st.choke > 1e-3
}
