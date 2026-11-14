//! Rungs E5 and E4 (docs/design/music.md section 7f, 2026-11-14): the WAVETABLE oscillator that plays
//! the core set's AKWF single cycles, band-limited by octave and morphing; UNISON (the supersaw) and
//! the PULSE with its width modulated; and the DRUM MACHINE's voices (808 and 909 style). Every test
//! measures the rendered sound, never what a voice was told, and each was seen failing first: the red
//! is in its comment.

use crate::composer::Composer;
use crate::dsp::SvfCoefs;
use crate::instrument::{Instrument, InstrumentKind, Wave};
use crate::resolved::Resolved;
use crate::tests::{data_dir, run, shipped, steer, Recorder};
use crate::tests_electronic::power;
use crate::tuning::note_hz;
use crate::voice::{BlockParams, NoteOn, VoicePool};
use crate::SampleLibrary;

const RATE: f32 = 48000.0;
const A4: f32 = 432.0;

/// An instrument from RON with its wavetable read from the repo's data folder.
fn inst(def: &str) -> Instrument {
    let mut v = vec![ron::from_str::<Instrument>(def).expect("the instrument parses")];
    crate::library::load_wavetables(&data_dir(), &mut v).expect("its waves load");
    v.pop().unwrap()
}

/// Instruments rendered through the voice pool for `seconds` at `bpm` (their LFOs given the bar and
/// beat a score would), mono: each note (instrument index, midi, start s, held s; 0 = until the end).
fn render(insts: &[Instrument], notes: &[(u16, f32, f64, f32)], bpm: f64, seconds: f64) -> Vec<f32> {
    let banks: Vec<_> = insts.iter().map(|_| None).collect();
    let mut pool = VoicePool::new(3);
    let vowel = [(SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    let total = (seconds * RATE as f64) as usize;
    let mut out = Vec::with_capacity(total);
    let mut next = 0;
    let (mut l, mut r, mut sl, mut sr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
    while out.len() < total {
        let t = out.len() as f64 / RATE as f64;
        while next < notes.len() && notes[next].2 <= t {
            let (i, midi, _, hold) = notes[next];
            pool.note_on(NoteOn { inst: i, role: 1, midi, a4: A4, vel: 1.0, pan: 0.0, hold_s: hold, tag: next as u32 + 1, level: 1.0 }, insts, &banks, RATE);
            next += 1;
        }
        let beats = t * bpm / 60.0;
        let mut p = BlockParams::plain(RATE, (bpm / 60.0) as f32, &vowel);
        p.bar = (beats / 4.0).floor() as u64;
        p.beats = beats;
        l.fill(0.0);
        r.fill(0.0);
        pool.render(insts, &banks, &p, &mut l, &mut r, &mut sl, &mut sr, 64);
        out.extend(l.iter().zip(r.iter()).map(|(a, b)| a + b));
    }
    out.truncate(total);
    out
}

/// One held note of one instrument.
fn held(i: &Instrument, midi: f32, bpm: f64, seconds: f64) -> Vec<f32> {
    render(std::slice::from_ref(i), &[(0, midi, 0.0, 0.0)], bpm, seconds)
}

fn db(x: f64) -> f64 {
    10.0 * x.max(1e-30).log10()
}

/// The power of harmonic `h` of `f` in `x[a..b]` (seconds), against harmonic 1's, dB.
fn harmonic_db(x: &[f32], f: f32, h: u32, a: f64, b: f64) -> f64 {
    let s = &x[(a * RATE as f64) as usize..(b * RATE as f64) as usize];
    db(power(s, f as f64 * h as f64, RATE as f64) / power(s, f as f64, RATE as f64))
}

/// The cosine and sine amplitudes of a cycle's harmonics 1 ..= n (a plain DFT, the test's own).
fn dft_mags(cyc: &[f32], n: usize) -> Vec<f64> {
    let len = cyc.len() as f64;
    (1..=n)
        .map(|h| {
            let (mut a, mut b) = (0.0, 0.0);
            for (k, &x) in cyc.iter().enumerate() {
                let w = 2.0 * std::f64::consts::PI * h as f64 * k as f64 / len;
                a += x as f64 * w.cos();
                b += x as f64 * w.sin();
            }
            (a * a + b * b).sqrt() * 2.0 / len
        })
        .collect()
}

fn read_cycle(name: &str) -> Vec<f32> {
    let path = crate::wavetable::find_wave(&data_dir(), name).expect("the wave is in the core set");
    crate::wavetable::read_wav_cycle(&std::fs::read(path).unwrap()).unwrap()
}

fn table_inst(waves: &[&str], extra: &str) -> Instrument {
    let list: Vec<String> = waves.iter().map(|w| format!("{w:?}")).collect();
    inst(&format!(
        r#"Instrument(id: "t", kind: Synth((oscillators: [(wave: Table)], wavetable: Some((waves: [{}]{extra})),
            envelope: (attack_s: 0.005, decay_s: 0.0, sustain: 1.0, release_s: 0.1))), level_db: 0.0)"#,
        list.join(", ")
    ))
}

const GUITAR: &str = "akwf_core/AKWF_aguitar/AKWF_aguitar_0001.wav";
const SAW: &str = "akwf_core/AKWF_bw_perfectwaves/AKWF_saw.wav";
const SINE: &str = "akwf_core/AKWF_bw_perfectwaves/AKWF_sin.wav";

/// THE WAVETABLE PLAYS ITS WAVE'S SPECTRUM: one held A2 (108 Hz at A = 432) of a wavetable holding the
/// core set's AKWF acoustic guitar cycle, its first 24 harmonics measured in the rendered sound
/// (Goertzel, 0.2 to 1.0 s) each within 1 dB of the cycle's own (the test's own DFT of the file),
/// relative to the fundamental, wherever the wave's harmonic is no more than 40 dB under it. RED first
/// (2026-11-14) with the oscillator's `Table` playing a polyBLEP saw in its place: "harmonic 2: the
/// sound -6.0 dB, the wave 12.6 dB" (this guitar cycle's second harmonic stands over its first).
#[test]
fn the_wavetable_plays_its_waves_spectrum() {
    let i = table_inst(&[GUITAR], "");
    let midi = 45.0;
    let f = note_hz(midi, A4);
    let x = held(&i, midi, 120.0, 1.0);
    let want = dft_mags(&read_cycle(GUITAR), 24);
    let mut checked = 0;
    for h in 2..=24u32 {
        let w = 20.0 * (want[h as usize - 1] / want[0]).log10();
        if w < -40.0 {
            continue;
        }
        let got = harmonic_db(&x, f, h, 0.2, 1.0);
        assert!((got - w).abs() <= 1.0, "harmonic {h}: the sound {got:.1} dB, the wave {w:.1} dB");
        checked += 1;
    }
    println!("the wavetable's guitar: {checked} harmonics within 1 dB of the wave's own");
    assert!(checked >= 10, "only {checked} harmonics were loud enough to compare");
}

/// NOTHING FOLDS BACK ABOVE THE BAND LIMIT AT A HIGH NOTE: a held C7 (2055 Hz at A = 432; 11 harmonics
/// fit under 24 kHz) of the AKWF sawtooth cycle (300 harmonics in its 600 samples). Every frequency a
/// harmonic above Nyquist would fold back to (harmonics 12 to 60, each |m * 48000 - h * f|) measures at
/// least 60 dB under the fundamental, and the harmonics that do fit are there (the 11th within 3 dB of
/// a saw's 1/11). RED first (2026-11-14) with `level_for` always choosing the top table (every
/// harmonic, as a plain table oscillator would): "harmonic 12 folds back to 23341 Hz at -21.6 dB (the
/// bound -60)"; and with a polyBLEP saw in the table's place: "-29.9 dB" (polyBLEP alone still folds
/// back at a high note; the band-limited tables do not, -141.7 dB measured).
#[test]
fn nothing_folds_back_above_the_band_limit_at_a_high_note() {
    let i = table_inst(&[SAW], "");
    let midi = 96.0;
    let f = note_hz(midi, A4) as f64;
    let x = held(&i, midi as f32, 120.0, 1.0);
    let s = &x[(0.2 * RATE) as usize..];
    let p1 = power(s, f, RATE as f64);
    let nyq = RATE as f64 / 2.0;
    let mut worst = (-300.0, 0u32, 0.0);
    for h in 12..=60u32 {
        let mut a = h as f64 * f % RATE as f64;
        if a > nyq {
            a = RATE as f64 - a;
        }
        if a < 30.0 || (a / f - (a / f).round()).abs() * f < 15.0 {
            continue; // on a true harmonic or at DC: not an alias we can tell apart
        }
        let d = db(power(s, a, RATE as f64) / p1);
        if d > worst.0 {
            worst = (d, h, a);
        }
    }
    println!("the wavetable saw at C7: the worst fold-back {:.1} dB (harmonic {} at {:.0} Hz)", worst.0, worst.1, worst.2);
    assert!(worst.0 <= -60.0, "harmonic {} folds back to {:.0} Hz at {:.1} dB (the bound -60)", worst.1, worst.2, worst.0);
    let h11 = harmonic_db(&x, f as f32, 11, 0.2, 1.0);
    assert!((h11 - 20.0 * (1.0f64 / 11.0).log10()).abs() < 3.0, "the 11th harmonic, under Nyquist, plays: {h11:.1} dB");
}

/// THE MORPH POSITION MOVES THE SPECTRUM: a wavetable of the core set's sine and saw cycles, held at A3.
/// At position 0 the second harmonic is more than 40 dB under the fundamental (the sine); at 1 it is a
/// saw's -6 dB (within 1.5); at 0.5 between them. With the morph swept by a triangle in time with the
/// music (one cycle a bar at 120 bpm, 2 s), the second harmonic measured over 100 ms windows rises and
/// falls more than 20 dB. RED first (2026-11-14) with the oscillator's `Table` playing a polyBLEP saw
/// whatever the morph: "position 0: harmonic 2 at -6.0 dB, a sine has none".
#[test]
fn the_morph_position_moves_the_spectrum() {
    let midi = 57.0;
    let f = note_hz(midi, A4);
    let h2 = |pos: f32| {
        let i = table_inst(&[SINE, SAW], &format!(", position: {pos}"));
        harmonic_db(&held(&i, midi, 120.0, 0.8), f, 2, 0.2, 0.8)
    };
    let (a, m, b) = (h2(0.0), h2(0.5), h2(1.0));
    println!("the morph from sine to saw: harmonic 2 at {a:.1}, {m:.1}, {b:.1} dB (positions 0, 0.5, 1)");
    assert!(a < -40.0, "position 0: harmonic 2 at {a:.1} dB, a sine has none");
    assert!((b + 6.02).abs() < 1.5, "position 1: harmonic 2 at {b:.1} dB, a saw's is -6");
    assert!(m > a + 10.0 && m < b - 2.0, "position 0.5: harmonic 2 at {m:.1} dB, between {a:.1} and {b:.1}");
    let i = table_inst(&[SINE, SAW], ", lfo: (per_beat: 0.25, shape: Triangle), depth: 1.0");
    let x = held(&i, midi, 120.0, 4.0);
    let w: Vec<f64> = (0..36).map(|k| 0.2 + k as f64 * 0.1).map(|t| harmonic_db(&x, f, 2, t, t + 0.1)).collect();
    let (hi, lo) = (w.iter().copied().fold(f64::MIN, f64::max), w.iter().copied().fold(f64::MAX, f64::min));
    println!("the morph swept a bar a cycle: harmonic 2 from {lo:.1} to {hi:.1} dB");
    assert!(hi - lo > 20.0, "the swept morph moves harmonic 2 only {:.1} dB", hi - lo);
}

/// THE SUPERSAW'S DETUNE SPREADS THE FUNDAMENTAL, BY SZABO'S OFFSETS: seven saws at A3 with the high-pass
/// off, measured over 4 s. With the detune knob at 0 the power 0.89 times the note (where the lowest
/// copy sits at full detune) is more than 40 dB under the note's; at 1 the strongest peak between 0.85
/// and 0.91 of the note (under the next copy, at 0.937) lies within 0.3% of 1 - 0.11002313 x 0.99 (Szabo's offset times his curve at
/// 1), within 12 dB of the centre; at 0.5 the lowest copy moves in by the curve's value there (0.098: 1.08%), its peak
/// within 0.001 of the place. RED first
/// (2026-11-14) with the unison's detune amount fixed at 0: "detune 1: the lowest copy's peak at 0.9080
/// of the note, Szabo puts it at 0.8900" (with every copy at the note, the search found only a
/// sidelobe).
#[test]
fn the_supersaw_detune_spreads_the_fundamental() {
    let midi = 57.0;
    let f = note_hz(midi, A4) as f64;
    let saw = |detune: f32| inst(&format!(r#"Instrument(id: "s", kind: Synth((oscillators: [(wave: Saw, unison: Some((voices: 7, detune: {detune}, mix: 0.6, high_pass: false)))],
        envelope: (attack_s: 0.005, decay_s: 0.0, sustain: 1.0, release_s: 0.1))), level_db: 0.0)"#));
    let peak = |x: &[f32], lo: f64, hi: f64| {
        let mut best = (0.0, 0.0);
        let mut r = lo;
        while r <= hi {
            let p = power(x, f * r, RATE as f64);
            if p > best.1 {
                best = (r, p);
            }
            r += 0.0005;
        }
        best
    };
    let x0 = held(&saw(0.0), midi as f32, 120.0, 4.2);
    let s0 = &x0[(0.2 * RATE) as usize..];
    let off0 = db(power(s0, f * 0.8907, RATE as f64) / power(s0, f, RATE as f64));
    let x1 = held(&saw(1.0), midi as f32, 120.0, 4.2);
    let s1 = &x1[(0.2 * RATE) as usize..];
    let want1 = 1.0 - 0.110_023_13 * crate::osc::szabo_detune(1.0) as f64;
    let (r1, p1) = peak(s1, 0.85, 0.91);
    let rel1 = db(p1 / peak(s1, 0.995, 1.005).1);
    let x5 = held(&saw(0.5), midi as f32, 120.0, 4.2);
    let s5 = &x5[(0.2 * RATE) as usize..];
    let want5 = 1.0 - 0.110_023_13 * crate::osc::szabo_detune(0.5) as f64;
    let (r5, _) = peak(s5, want5 - 0.002, want5 + 0.002);
    println!("the supersaw: at detune 0 the place of the lowest copy {off0:.1} dB; at 1 its peak at {r1:.4} (Szabo {want1:.4}), {rel1:.1} dB against the centre; at 0.5 at {r5:.4} (Szabo {want5:.4})");
    assert!(off0 < -40.0, "detune 0: {off0:.1} dB at 0.8907 of the note, the copies should all be at the note");
    assert!((r1 - want1).abs() < 0.003, "detune 1: the lowest copy's peak at {r1:.4} of the note, Szabo puts it at {want1:.4}");
    assert!(rel1 > -12.0, "detune 1: the lowest copy {rel1:.1} dB under the centre");
    assert!((r5 - want5).abs() < 0.001, "detune 0.5: the lowest copy's peak at {r5:.4} of the note, Szabo puts it at {want5:.4}");
}

/// The fraction of each window's samples above zero (a pulse with its offset taken out is above zero
/// for exactly its width), windows of `w` seconds from `a`.
fn duty(x: &[f32], a: f64, w: f64, count: usize) -> Vec<f64> {
    (0..count)
        .map(|k| {
            let i0 = ((a + k as f64 * w) * RATE as f64) as usize;
            let s = &x[i0..i0 + (w * RATE as f64) as usize];
            s.iter().filter(|&&v| v > 0.0).count() as f64 / s.len() as f64
        })
        .collect()
}

/// PULSE-WIDTH MODULATION MOVES THE PULSE'S DUTY IN THE RENDERED SOUND: a pulse at A3, no filter. At
/// width 0.5 it is above zero half the time and its second harmonic more than 40 dB down (a square has
/// none); at width 0.2 it is above zero 20% of the time (within 0.02) and its second harmonic strong
/// (within 2 dB of a 20% pulse's -1.9 dB, |sin 2 pi w / 2 sin pi w|). With PWM (width 0.5, depth 0.3, a
/// triangle one cycle a bar at 120 bpm) the duty measured over 50 ms windows sweeps from under 0.25 to
/// over 0.75, its widest and narrowest a bar apart. RED first (2026-11-14) with the PWM offset ignored:
/// "the duty under PWM swept only 0.49 to 0.51".
#[test]
fn pwm_moves_the_pulse_duty_in_the_rendered_sound() {
    let midi = 57.0;
    let f = note_hz(midi, A4);
    let pulse = |width: f32, pwm: &str| inst(&format!(r#"Instrument(id: "p", kind: Synth((oscillators: [(wave: Pulse, width: {width})]{pwm},
        envelope: (attack_s: 0.002, decay_s: 0.0, sustain: 1.0, release_s: 0.1))), level_db: 0.0)"#));
    let x = held(&pulse(0.5, ""), midi, 120.0, 1.0);
    let d5 = duty(&x, 0.2, 0.8, 1)[0];
    let h2_5 = harmonic_db(&x, f, 2, 0.2, 1.0);
    let x = held(&pulse(0.2, ""), midi, 120.0, 1.0);
    let d2 = duty(&x, 0.2, 0.8, 1)[0];
    let h2_2 = harmonic_db(&x, f, 2, 0.2, 1.0);
    let want = 20.0 * ((2.0 * std::f64::consts::PI * 0.2).sin() / (2.0 * (std::f64::consts::PI * 0.2).sin())).abs().log10();
    println!("the pulse: width 0.5 above zero {d5:.3} of the time, harmonic 2 {h2_5:.1} dB; width 0.2 {d2:.3}, harmonic 2 {h2_2:.1} dB (a 20% pulse's {want:.1})");
    assert!((d5 - 0.5).abs() < 0.02 && h2_5 < -40.0, "width 0.5: duty {d5:.3}, harmonic 2 {h2_5:.1} dB");
    assert!((d2 - 0.2).abs() < 0.02 && (h2_2 - want).abs() < 2.0, "width 0.2: duty {d2:.3}, harmonic 2 {h2_2:.1} dB (want {want:.1})");
    let x = held(&pulse(0.5, ", pwm: Some((lfo: (per_beat: 0.25, shape: Triangle), depth: 0.3))"), midi, 120.0, 4.2);
    let d = duty(&x, 0.2, 0.05, 80);
    let (imax, imin) = (0..d.len()).fold((0, 0), |(a, b), i| (if d[i] > d[a] { i } else { a }, if d[i] < d[b] { i } else { b }));
    println!("the duty under PWM swept {:.2} to {:.2}, its widest and narrowest {:.2} s apart", d[imin], d[imax], (imax as f64 - imin as f64).abs() * 0.05);
    assert!(d[imin] < 0.25 && d[imax] > 0.75, "the duty under PWM swept only {:.2} to {:.2}", d[imin], d[imax]);
    assert!(((imax as f64 - imin as f64).abs() * 0.05 - 1.0).abs() < 0.15, "the widest and narrowest are half the LFO's 2 s cycle apart");
}

/// The rendered sound's frequency in a window, from its upward zero crossings (low-passed at `lp` Hz
/// first, so a click's noise does not count): (crossings - 1) over the time between the first and last.
fn crossing_hz(x: &[f32], a: f64, b: f64, lp: f32) -> f64 {
    let c = SvfCoefs::new(lp, 0.707, RATE);
    let (mut s1, mut s2) = (crate::dsp::Svf::default(), crate::dsp::Svf::default());
    let y: Vec<f32> = x.iter().map(|&v| s2.lp(&c, s1.lp(&c, v))).collect();
    let (i0, i1) = ((a * RATE as f64) as usize, (b * RATE as f64) as usize);
    let mut ups = Vec::new();
    for i in i0.max(1)..i1.min(y.len()) {
        if y[i - 1] < 0.0 && y[i] >= 0.0 {
            ups.push(i as f64 - 1.0 + (-y[i - 1] / (y[i] - y[i - 1])) as f64);
        }
    }
    if ups.len() < 2 {
        return 0.0;
    }
    (ups.len() - 1) as f64 / ((ups[ups.len() - 1] - ups[0]) / RATE as f64)
}

/// RMS over 1 ms windows.
fn rms_ms(x: &[f32]) -> Vec<f32> {
    x.chunks(48).map(|c| (c.iter().map(|v| v * v).sum::<f32>() / c.len() as f32).sqrt()).collect()
}

/// How long until the 1 ms RMS falls `fall_db` under its peak for good, seconds.
fn fall_time(x: &[f32], fall_db: f32) -> f64 {
    let e = rms_ms(x);
    let peak = e.iter().copied().fold(0.0, f32::max);
    let floor = peak * 10f32.powf(-fall_db / 20.0);
    let last = e.iter().rposition(|&v| v > floor).unwrap_or(0);
    (last + 1) as f64 / 1000.0
}

/// The share of a window's energy above `hz` (a two-pole high-pass).
fn share_above(x: &[f32], hz: f32) -> f64 {
    let c = SvfCoefs::new(hz, 0.707, RATE);
    let (mut s1, mut s2) = (crate::dsp::Svf::default(), crate::dsp::Svf::default());
    let (mut hi, mut all) = (0.0, 0.0);
    for &v in x {
        let h = s2.hp(&c, s1.hp(&c, v));
        hi += (h * h) as f64;
        all += (v * v) as f64;
    }
    hi / all.max(1e-30)
}

/// One hit of a shipped drum at `midi`, rendered for `seconds`.
fn hit(lib: &crate::MusicLibrary, id: &str, midi: f32, seconds: f64) -> Vec<f32> {
    let i = lib.instruments.iter().find(|i| i.id == id).unwrap_or_else(|| panic!("{id} is shipped")).clone();
    render(&[i], &[(0, midi, 0.0, 0.1)], 120.0, seconds)
}

/// EACH DRUM-MACHINE VOICE'S ENVELOPE AND PITCH DROP, MEASURED IN THE SOUND (rung E4), every shipped
/// voice struck once: the 909 kick starts more than twice its settled pitch (the first two cycles after
/// 3 ms against 0.15 to 0.3 s, which is within 10% of its note) and falls 30 dB within 0.6 s; the 808
/// kick starts only a little sharp (1.03 to 1.4 times) and rings longer than the 909's; each tom starts
/// over 1.08 times its settled pitch; each snare's two tones are there (the note and 1.83 times it each
/// stand 10 dB over 1.4 times it in 20 to 120 ms) with a fifth of its energy above 3 kHz (the snares);
/// each clap is struck at least three times in its first 40 ms (the 1 ms RMS rising twofold from a dip)
/// and its tail rings past 100 ms; each closed hat falls 20 dB within 80 ms and each open hat still
/// sounds within 20 dB at 150 ms, both with 80% of their energy above 5 kHz; and a closed hat struck
/// 100 ms after an open one cuts it short (its energy 150 to 300 ms at least 15 dB under the open hat
/// alone). RED first (2026-11-14) with the tones' pitch envelope ignored, the clap's bursts and the
/// choke skipped at once (the first failure stops the test): "kick_909: the first cycles at 1.00 times
/// its settled pitch (want 2 to 4)".
#[test]
fn each_drum_machine_voice_has_its_envelope_and_pitch_drop() {
    let lib = shipped();
    for (id, drop_lo, drop_hi) in [("kick_909", 2.0, 4.0), ("kick_808", 1.03, 1.4), ("tom_808", 1.08, 2.0), ("tom_909", 1.08, 2.0)] {
        let midi = if id.starts_with("kick") { 33.0 } else { 45.0 };
        let x = hit(&lib, id, midi, 1.5);
        let f = note_hz(midi, A4) as f64;
        // The first two cycles after the click (from 3 ms), and the settled pitch.
        let early = {
            let lp = (f * 6.0) as f32;
            let c = SvfCoefs::new(lp, 0.707, RATE);
            let (mut s1, mut s2) = (crate::dsp::Svf::default(), crate::dsp::Svf::default());
            let y: Vec<f32> = x.iter().map(|&v| s2.lp(&c, s1.lp(&c, v))).collect();
            let mut ups = Vec::new();
            for i in (0.003 * RATE) as usize..y.len() {
                if y[i - 1] < 0.0 && y[i] >= 0.0 {
                    ups.push(i as f64 / RATE as f64);
                    if ups.len() == 3 {
                        break;
                    }
                }
            }
            2.0 / (ups[2] - ups[0])
        };
        let late = crossing_hz(&x, 0.15, 0.3, (f * 3.0) as f32);
        let ratio = early / late;
        println!("{id}: the first cycles at {early:.1} Hz, settled {late:.1} Hz (the note {f:.1}): {ratio:.2} times; 30 dB down at {:.2} s", fall_time(&x, 30.0));
        assert!((late / f - 1.0).abs() < 0.1, "{id}: settled at {late:.1} Hz, the note is {f:.1}");
        assert!(ratio > drop_lo && ratio < drop_hi, "{id}: the first cycles at {ratio:.2} times its settled pitch (want {drop_lo} to {drop_hi})");
    }
    let (k9, k8) = (fall_time(&hit(&lib, "kick_909", 33.0, 2.0), 30.0), fall_time(&hit(&lib, "kick_808", 33.0, 2.0), 30.0));
    assert!(k9 < 0.6 && k8 > k9, "the 909 kick falls 30 dB in {k9:.2} s, the 808's in {k8:.2} s");
    for id in ["snare_808", "snare_909"] {
        let midi = 50.0;
        let f = note_hz(midi, A4) as f64;
        let x = hit(&lib, id, midi, 0.6);
        let s = &x[(0.02 * RATE) as usize..(0.12 * RATE) as usize];
        let (p1, p2, gap) = (power(s, f, RATE as f64), power(s, f * 1.83, RATE as f64), power(s, f * 1.4, RATE as f64));
        let noise = share_above(&x[..(0.15 * RATE) as usize], 3000.0);
        println!("{id}: its tones {:.1} and {:.1} dB over the gap between them; {:.0}% of its energy above 3 kHz", db(p1 / gap), db(p2 / gap), noise * 100.0);
        assert!(db(p1 / gap) > 10.0 && db(p2 / gap) > 10.0, "{id}: its two tones stand only {:.1} and {:.1} dB over the gap", db(p1 / gap), db(p2 / gap));
        assert!(noise > 0.2, "{id}: {:.0}% of its energy above 3 kHz, the snares' noise", noise * 100.0);
    }
    for id in ["clap_808", "clap_909"] {
        let x = hit(&lib, id, 50.0, 0.8);
        let e = rms_ms(&x);
        // A strike: the envelope rising to twice its lowest since the last strike (and a tenth of the
        // loudest); the next counts once it has fallen to half of this one's peak.
        let peak40 = e[..40].iter().copied().fold(0.0, f32::max);
        let (mut strikes, mut low, mut high, mut armed) = (0, f32::MAX, 0.0f32, true);
        for &v in &e[..40] {
            if armed {
                low = low.min(v);
                if v > 2.0 * low && v > 0.1 * peak40 {
                    strikes += 1;
                    armed = false;
                    high = v;
                }
            } else {
                high = high.max(v);
                if v < 0.5 * high {
                    armed = true;
                    low = v;
                }
            }
        }
        let tail = fall_time(&x, 30.0);
        println!("{id}: {strikes} strikes in its first 40 ms, 30 dB down at {tail:.2} s");
        assert!(strikes >= 3, "{id}: {strikes} strikes in its first 40 ms");
        assert!(tail > 0.1, "{id}: its tail is gone by {tail:.2} s");
    }
    for (id, open) in [("hat_808_closed", false), ("hat_808_open", true), ("hat_909_closed", false), ("hat_909_open", true)] {
        let x = hit(&lib, id, 74.0, 1.0);
        let e = rms_ms(&x);
        let peak = e.iter().copied().fold(0.0, f32::max);
        let at = |ms: usize| 20.0 * (e[ms..ms + 20].iter().copied().fold(0.0, f32::max) / peak).log10();
        let bright = share_above(&x[..(0.1 * RATE) as usize], 5000.0);
        println!("{id}: {:.1} dB at 60 ms, {:.1} dB at 150 ms; {:.0}% of its energy above 5 kHz", at(60), at(150), bright * 100.0);
        assert!(bright > 0.8, "{id}: only {:.0}% of its energy above 5 kHz", bright * 100.0);
        if open {
            assert!(at(150) > -20.0, "{id}: {:.1} dB at 150 ms, an open hat rings on", at(150));
        } else {
            assert!(at(60) < -20.0, "{id}: {:.1} dB at 60 ms, a closed hat is short", at(60));
        }
    }
    let find = |id: &str| lib.instruments.iter().find(|i| i.id == id).unwrap().clone();
    let pair = [find("hat_808_open"), find("hat_808_closed")];
    let alone = render(&pair, &[(0, 74.0, 0.0, 0.1)], 120.0, 0.4);
    let choked = render(&pair, &[(0, 74.0, 0.0, 0.1), (1, 74.0, 0.1, 0.1)], 120.0, 0.4);
    let energy = |x: &[f32]| x[(0.15 * RATE) as usize..(0.3 * RATE) as usize].iter().map(|v| (v * v) as f64).sum::<f64>();
    let cut = db(energy(&choked) / energy(&alone));
    println!("the choked open hat: {cut:.1} dB against the open hat alone");
    assert!(cut < -15.0, "the choked open hat: {cut:.1} dB against the open hat alone");
}

/// A WAVE MISSING FROM THE DATA: with a `fallback` its instrument resolves to the fallback (a pack's
/// waves not installed play a synthesised stand-in, never silence); without one, loading names the
/// instrument and the wave. RED first (2026-11-14) with `load_wavetables` ignoring the fallback: "the
/// instrument wt_test: instrument wt_test: wave \"no_such_set/x.wav\" is in neither data/music/waves
/// nor an installed pack".
#[test]
fn a_missing_wave_plays_the_fallback_or_names_itself() {
    let mut lib = shipped();
    let def = |fb: &str| format!(r#"Instrument(id: "wt_test", kind: Synth((oscillators: [(wave: Table)], wavetable: Some((waves: ["no_such_set/x.wav"], fallback: "{fb}")),
        envelope: (attack_s: 0.01, release_s: 0.1))), level_db: 0.0)"#);
    let mut v = vec![ron::from_str::<Instrument>(&def("")).unwrap()];
    let e = crate::library::load_wavetables(&data_dir(), &mut v).expect_err("a missing wave without a fallback is an error");
    assert!(e.contains("wt_test") && e.contains("no_such_set/x.wav"), "the error names the instrument and the wave: {e}");
    let mut v = vec![ron::from_str::<Instrument>(&def("soft_lead")).unwrap()];
    crate::library::load_wavetables(&data_dir(), &mut v).unwrap_or_else(|e| panic!("the instrument wt_test: {e}"));
    lib.instruments.extend(v);
    lib.genres[0].roles_add.push(crate::mood::RoleSpec { role: crate::mood::RoleKind::Texture, instrument: "wt_test".into(), part: None, low: "C4".into(), high: "C5".into(), voices: 1, density: 1.0, level_db: 0.0, min_intensity: 0.0, pulse: None, weight: 1.0, rests: Vec::new() });
    let res = Resolved::new(&lib, &SampleLibrary::new()).expect("resolves");
    let fb = lib.instrument_index("soft_lead").unwrap() as u16;
    let g = &res.genres[0];
    assert!(g.roles_add.iter().flatten().any(|r| r.inst == fb), "the missing wavetable's role plays its fallback, soft_lead");
    // Every SHIPPED wavetable names a fallback, so a data folder without the waves still plays (RED
    // first, 2026-11-14, in the engine's runtime tests, which copy the music's data without its waves
    // folder: seven failed with "not reached in 600 frames", the library refusing to load).
    for i in shipped().instruments.iter() {
        if let InstrumentKind::Synth(s) = &i.kind {
            if let Some(w) = &s.wavetable {
                assert!(!w.fallback.is_empty() && lib.instrument_index(&w.fallback).is_some(), "{}: its wavetable names no fallback instrument ({:?})", i.id, w.fallback);
            }
        }
    }
}

/// THE DRUM MACHINE IS A KIT WHERE A GENRE LISTS KITS (rung E4): a lo-fi piece that draws its "808" kit
/// plays the 808 kick, snare and hat and none of the other kits' drums; an electro swing piece that
/// draws "909" plays the 909 kick, clap and hats. RED first (2026-11-14) with the kits not yet in the
/// genres' files: "lofi has no kit \"808\"".
#[test]
fn the_drum_machine_is_a_kit_where_a_genre_lists_kits() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    for (genre, kit, want, never) in [
        ("lofi", "808", &["kick_808", "snare_808", "hat_808_closed"][..], &["lofi_kick", "kick_909"][..]),
        ("electro_swing", "909", &["kick_909", "clap_909", "hat_909_open", "hat_909_closed"][..], &["house_kick", "brush_swish", "kick_808"][..]),
    ] {
        let g = &lib.genres[lib.genre_index(genre).unwrap()];
        let want_kit = g.kits.iter().position(|k| k.id == kit).unwrap_or_else(|| panic!("{genre} has no kit {kit:?}")) as u8;
        let weights: Vec<f32> = g.kits.iter().map(|k| k.weight).collect();
        let mut seed = 1u64;
        while crate::piece::Piece::new(seed).kit(&weights) != Some(want_kit) {
            seed += 1;
        }
        let mut c = Composer::new(RATE, seed, steer(&lib, "mothership_calm", genre));
        let mut rec = Recorder::default();
        run(&mut c, &res, 40.0, &mut rec);
        let played = |id: &str| {
            let i = lib.instrument_index(id).unwrap() as u16;
            rec.notes.iter().filter(|(_, n)| n.inst == i).count()
        };
        for id in want {
            assert!(played(id) > 0, "{genre} seed {seed} drew the {kit} kit, yet {id} never played");
        }
        for id in never {
            assert_eq!(played(id), 0, "{genre} seed {seed} drew the {kit} kit, yet {id} played");
        }
        println!("{genre}: seed {seed} draws the {kit} kit: {}", want.iter().map(|id| format!("{id} x{}", played(id))).collect::<Vec<_>>().join(", "));
    }
}

/// EVERY NEW OPTION IS AS LOUD AS WHAT IT STANDS BESIDE, so a piece that draws it keeps the genre's
/// balance and its loudness (the genre levels are measured with the old voices): each drum-machine
/// voice in a kit within 2 dB of the drum it plays in place of (one hit's RMS over its first 0.3 s, at
/// the genre's velocity and note; a kick's under 150 Hz, its body), deep dubstep's wavetable growl within 2 dB of the FM growl (a held
/// D2, A2 and D3, the first 2 s, its LFOs on a 140 bpm beat clock) and its pulse-width pad within 2 dB
/// of the dub pad (held notes across the register the genre gives each, A2 to A4 and A3 to A5, each 3
/// to 10 s, past its swell, the width swept in time); the 909 snare and the 808 clap, in no kit yet, beside the lo-fi snare and
/// the clap. RED first (2026-11-14) as the files were first written, levelled by eye (deep dubstep's
/// seed 20261113 then measured -16.5 LUFS, its target -18): "kick_909: +4.1 dB against house_kick;
/// clap_909: +3.5 dB against clap; hat_909_open: -5.4 dB against open_hat; ... snare_808: +9.4 dB
/// against lofi_snare; hat_808_closed: -6.4 dB against lofi_hat; wavetable_growl: +10.3 dB against
/// fm_growl".
#[test]
fn every_new_option_is_as_loud_as_what_it_stands_beside() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let idx = |id: &str| lib.instrument_index(id).unwrap_or_else(|| panic!("{id}")) as u16;
    let rms = |id: &str, midi: f32, from: f32, to: f32, vel: f32| {
        // velocity scales a voice's level linearly
        vel * crate::render::note_rms_between(&res, idx(id), midi, A4, from, to, RATE)
    };
    // A hit's RMS over its first 0.3 s under 150 Hz (two two-pole low-passes), velocity 1.
    let low_rms = |id: &str, midi: f32| {
        let x = hit(&lib, id, midi, 0.3);
        let c = SvfCoefs::new(150.0, 0.707, RATE);
        let (mut a, mut b) = (crate::dsp::Svf::default(), crate::dsp::Svf::default());
        (x.iter().map(|&v| b.lp(&c, a.lp(&c, v)).powi(2)).sum::<f32>() / x.len() as f32).sqrt()
    };
    let mut report = Vec::new();
    let mut bad = Vec::new();
    let mut check = |new: &str, old: &str, d: f32| {
        report.push(format!("{new} {d:+.1} dB against {old}"));
        if d.abs() > 2.0 {
            bad.push(format!("{new}: {d:+.1} dB against {old}"));
        }
    };
    // (new, old, midi, velocity new, velocity old): the kits' drums at their genres' notes and velocities.
    for (new, old, midi, vn, vo) in [
        ("kick_909", "house_kick", 33.0, 0.9, 0.95),
        ("clap_909", "clap", 50.0, 0.7, 0.7),
        ("hat_909_open", "open_hat", 74.0, 0.5, 0.5),
        ("hat_909_closed", "lofi_hat", 74.0, 0.35, 0.35),
        ("kick_808", "lofi_kick", 33.0, 0.85, 0.9),
        ("snare_808", "lofi_snare", 50.0, 0.7, 0.7),
        ("hat_808_closed", "lofi_hat", 74.0, 0.5, 0.45),
        ("snare_909", "lofi_snare", 50.0, 0.7, 0.7),
        ("clap_808", "clap", 50.0, 0.7, 0.7),
    ] {
        // A kick is levelled by its body (under 150 Hz, what the onset and the floor feel), the rest whole.
        let level = |id: &str, vel: f32| if new.starts_with("kick") { vel * low_rms(id, midi) } else { rms(id, midi, 0.0, 0.3, vel) };
        let d = 20.0 * (level(new, vn) / level(old, vo)).log10();
        check(new, old, d);
    }
    // A held note with its LFOs on the music's beat clock at deep dubstep's 140 bpm (`note_rms` holds
    // the clock at 0, which froze the pulse-width pad at its narrowest), its RMS from `a` to `b` s.
    let beat_rms = |id: &str, midi: f32, a: f64, b: f64| {
        let x = held(&lib.instruments[idx(id) as usize], midi, 140.0, b);
        let s = &x[(a * RATE as f64) as usize..];
        (s.iter().map(|v| v * v).sum::<f32>() / s.len() as f32).sqrt()
    };
    let growl = |id: &str| [38.0, 45.0, 50.0].iter().map(|&m| beat_rms(id, m, 0.0, 2.0).powi(2)).sum::<f32>().sqrt();
    check("wavetable_growl", "fm_growl", 20.0 * (growl("wavetable_growl") / growl("fm_growl")).log10());
    // A pad across the register the genre gives it (the dub pad A2 to A4, the pulse-width pad an octave
    // up), each note 3 to 10 s: past its swell, two cycles of the pulse width's sweep.
    let pad = |id: &str, lo: f32| [0.0, 5.0, 12.0, 17.0, 24.0].iter().map(|&m| beat_rms(id, lo + m, 3.0, 10.0).powi(2)).sum::<f32>().sqrt();
    check("pwm_pad", "dub_pad", 20.0 * (pad("pwm_pad", 57.0) / pad("dub_pad", 45.0)).log10());
    println!("the options' levels: {}", report.join("; "));
    assert!(bad.is_empty(), "options not levelled to what they stand beside: {}", bad.join("; "));
}

/// Whether an instrument is one of these rungs' (a drum machine, or a synth with a pulse, a wavetable or
/// unison).
fn is_new(i: &Instrument) -> bool {
    match &i.kind {
        InstrumentKind::DrumMachine(_) => true,
        InstrumentKind::Synth(s) => s.oscillators.iter().any(|o| matches!(o.wave, Wave::Pulse | Wave::Table) || o.unison.is_some()),
        _ => false,
    }
}

/// The calm score's cost in this run: the mean ms of a 128-frame block over 10 s after 2 s, the best of
/// three seeds (docs/design/music.md section 14's gate does the same).
pub(crate) fn calm_ms(lib: &crate::MusicLibrary) -> f64 {
    let mut best = f64::MAX;
    for seed in [1u64, 2, 3] {
        let ctx = crate::MusicContext { gain: 1.0, a4_hz: A4, duck_db: 0.0, ..Default::default() };
        let mut s = crate::ScoreEngine::new(Resolved::new(lib, &SampleLibrary::new()).unwrap(), RATE, seed, ctx);
        crate::render::render_score(&mut s, RATE, 2.0);
        let mut times = Vec::new();
        for _ in 0..(10.0 * RATE / 128.0) as usize {
            let mut buf = [[0f32; 2]; 128];
            let t0 = std::time::Instant::now();
            s.render(&mut buf);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        times.sort_by(|a, b| a.total_cmp(b));
        let kept = &times[..(times.len() as f64 * 0.98) as usize];
        best = best.min(kept.iter().sum::<f64>() / kept.len() as f64);
    }
    best
}

/// EVERY NEW INSTRUMENT FITS THE AUDIO THREAD'S PER-VOICE BUDGET (docs/design/music.md section 14), each
/// measured against the calm score in the same run (the merged lesson: costs swing twice over while
/// other agents build): one voice of each shipped instrument these rungs added (a held note for a
/// synth; a drum struck every 0.25 s, so a hit is always sounding), the mean of a 128-frame block, the
/// best of three runs, at most 0.02 ms (the bass toolkit's whole chain's gate, section 7d) scaled by
/// how much slower than its 0.037 ms the calm score ran this time (never below 1). RED first
/// (2026-11-14) with the gate at a tenth of that: "hat_808_open: 0.0124 ms a block for one voice
/// (budget 0.0034; the calm score 0.0633 ms this run)" (the machine busy with other agents' builds:
/// the calm score at 1.7 times its quiet cost, the budget scaled with it).
#[test]
fn every_new_instrument_fits_the_per_voice_budget() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let calm = calm_ms(&lib);
    let scale = (calm / 0.037).max(1.0);
    let budget = 0.02 * scale;
    let vowel = [(SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    let mut seen = 0;
    let mut worst = (0.0, String::new());
    for i in lib.instruments.iter().filter(|i| is_new(i)) {
        let insts = vec![i.clone()];
        let banks = vec![None];
        let drum = matches!(i.kind, InstrumentKind::DrumMachine(_));
        let mut best = f64::MAX;
        for run in 0..3 {
            let mut pool = VoicePool::new(run);
            let mut times = Vec::new();
            for k in 0..1500usize {
                if (drum && k % 94 == 0) || (!drum && k == 0) {
                    pool.note_on(NoteOn { inst: 0, role: 1, midi: 45.0, a4: A4, vel: 1.0, pan: 0.0, hold_s: if drum { 0.1 } else { 0.0 }, tag: k as u32 + 1, level: 1.0 }, &insts, &banks, RATE);
                }
                let beats = k as f64 * 128.0 / RATE as f64 * 2.0;
                let mut p = BlockParams::plain(RATE, 2.0, &vowel);
                p.bar = (beats / 4.0) as u64;
                p.beats = beats;
                let (mut l, mut r, mut sl, mut sr) = ([0f32; 128], [0f32; 128], [0f32; 128], [0f32; 128]);
                let t0 = std::time::Instant::now();
                pool.render(&insts, &banks, &p, &mut l, &mut r, &mut sl, &mut sr, 128);
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
            }
            times.sort_by(|a, b| a.total_cmp(b));
            let kept = &times[..(times.len() as f64 * 0.98) as usize];
            best = best.min(kept.iter().sum::<f64>() / kept.len() as f64);
        }
        println!("{}: {best:.4} ms a block for one voice", i.id);
        if best > worst.0 {
            worst = (best, i.id.clone());
        }
        seen += 1;
        assert!(best <= budget, "{}: {best:.4} ms a block for one voice (budget {budget:.4}; the calm score {calm:.4} ms this run)", i.id);
    }
    println!("{seen} new instruments, the costliest {} at {:.4} ms a block (budget {budget:.4}; the calm score {calm:.4} ms this run)", worst.1, worst.0);
    assert!(seen >= 16, "only {seen} of the rungs' instruments are shipped");
}
