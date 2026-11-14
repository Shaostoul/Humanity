//! Rungs E3 and E6 (docs/design/music.md section 7g, 2026-11-14): the mix's effects for the electronic
//! genres (the tempo delay, the gated reverb, the reverb's low cut, the downlifter, the reversed cymbal,
//! the tape stop), drum fills down the toms, and four genres as files (house, drum and bass, synthwave,
//! ambient). Every test measures the rendered sound or the composer's decisions, and each was seen
//! failing first; the red is in its comment.

use crate::composer::{Composer, Event};
use crate::dsp::{Fdn, Svf, SvfCoefs};
use crate::fx::{GatedReverb, RDelay, RGated, TempoDelay};
use crate::instrument::Instrument;
use crate::resolved::Resolved;
use crate::tests::{run, shipped, steer, Recorder};
use crate::tests_electronic::band_onsets;
use crate::voice::{BlockParams, NoteOn, VoicePool};
use crate::SampleLibrary;

const RATE: f32 = 48000.0;

fn db(x: f64) -> f64 {
    10.0 * x.max(1e-30).log10()
}

/// The energy of `x[a..b]` (seconds) after a two-pole band of `lo..hi` Hz (0: no edge).
fn band_energy(x: &[f32], a: f64, b: f64, lo: f32, hi: f32) -> f64 {
    let (cl, ch) = (SvfCoefs::new(hi.max(1.0), 0.707, RATE), SvfCoefs::new(lo.max(1.0), 0.707, RATE));
    let (mut s1, mut s2, mut s3, mut s4) = (Svf::default(), Svf::default(), Svf::default(), Svf::default());
    let (i0, i1) = ((a * RATE as f64) as usize, ((b * RATE as f64) as usize).min(x.len()));
    let mut e = 0.0f64;
    for (i, &v) in x.iter().enumerate().take(i1) {
        let mut y = v;
        if hi > 0.0 {
            y = s2.lp(&cl, s1.lp(&cl, y));
        }
        if lo > 0.0 {
            y = s4.hp(&ch, s3.hp(&ch, y));
        }
        if i >= i0 {
            e += (y * y) as f64;
        }
    }
    e / (i1.saturating_sub(i0)).max(1) as f64
}

// ----- The tempo delay ----------------------------------------------------------------------------

/// A click through the tempo delay at `bpm` with `spec`, `seconds` long (mono, the left output).
fn delay_click(spec: &RDelay, bpm: f32, seconds: f64) -> Vec<f32> {
    let mut d = TempoDelay::new();
    let total = (seconds * RATE as f64) as usize;
    let mut out = Vec::with_capacity(total);
    let mut i = 0;
    while i < total {
        let n = 64.min(total - i);
        let mut sl = [0f32; 64];
        let mut sr = [0f32; 64];
        if i == 0 {
            sl[0] = 1.0;
            sr[0] = 1.0;
        }
        let (mut ol, mut or, mut rl, mut rr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
        d.process(spec, bpm / 60.0, RATE, &sl[..n], &sr[..n], &mut ol[..n], &mut or[..n], &mut rl[..n], &mut rr[..n]);
        out.extend(ol[..n].iter().zip(or[..n].iter()).map(|(a, b)| a + b));
        i += n;
    }
    out
}

/// Where the repeats are: the times (s) of the loudest sample in each window around `period` multiples.
fn repeat_times(x: &[f32], period: f64, count: usize) -> Vec<(f64, f32)> {
    (1..=count)
        .map(|k| {
            let (a, b) = (((k as f64 - 0.5) * period * RATE as f64) as usize, (((k as f64 + 0.5) * period * RATE as f64) as usize).min(x.len()));
            let (i, v) = x[a..b].iter().enumerate().fold((0, 0.0f32), |m, (i, &v)| if v.abs() > m.1 { (i, v.abs()) } else { m });
            ((a + i) as f64 / RATE as f64, v)
        })
        .collect()
}

/// THE DELAY REPEATS AT THE TEMPO'S DIVISION, measured in its rendered output: a click through the
/// delay at 120 bpm repeats every dotted eighth (0.375 s) for "1/8.", every quarter (0.5 s) for
/// "1/4" and every eighth-note triplet (0.1667 s) for "1/8t", each repeat within 2 samples of its time,
/// each quieter than the one before (the feedback); at 90 bpm the dotted eighth is 0.5 s; and the
/// repeats are band-limited where the data says (600 Hz to 1.2 kHz, inside the loop): the first repeat
/// at least 25 dB under the unfiltered repeat over 7 kHz and 20 dB under it below 150 Hz, within 4 dB of
/// it from 700 Hz to 1.1 kHz (measured: -42.1, -29.1 and -2.9 dB). RED first (2026-11-14) with
/// `division_beats` ignoring the dot: "1/8. at 120 bpm: the repeats at [0.25, 0.75, 1.0], every 0.375
/// s asked".
#[test]
fn the_tempo_delay_repeats_at_the_tempos_division() {
    for (div, bpm, period) in [("1/8.", 120.0f32, 0.375f64), ("1/4", 120.0, 0.5), ("1/8t", 120.0, 1.0 / 6.0), ("1/8.", 90.0, 0.5)] {
        let beats = crate::genre::division_beats(div).unwrap_or_else(|| panic!("{div} is a note value"));
        let spec = RDelay { beats, feedback: 0.5, wet: 1.0, ..Default::default() };
        let x = delay_click(&spec, bpm, period * 4.5);
        let reps = repeat_times(&x, period, 3);
        println!("{div} at {bpm} bpm: the repeats at {:?}", reps.iter().map(|r| (r.0 * 1000.0).round() / 1000.0).collect::<Vec<_>>());
        for (k, (t, _)) in reps.iter().enumerate() {
            let want = period * (k + 1) as f64;
            assert!((t - want).abs() <= 2.0 / RATE as f64, "{div} at {bpm} bpm: the repeats at {:?}, every {period:.3} s asked", reps.iter().map(|r| r.0).collect::<Vec<_>>());
        }
        assert!(reps[1].1 < reps[0].1 && reps[2].1 < reps[1].1, "{div}: each repeat quieter than the last: {:?}", reps.iter().map(|r| r.1).collect::<Vec<_>>());
    }
    // Band-limited repeats: a click's first repeat through the 600 Hz to 1.2 kHz loop, against the same
    // repeat unfiltered, over 7 kHz, from 700 Hz to 1.1 kHz, and under 150 Hz.
    let first = |lo: f32, hi: f32| {
        let spec = RDelay { beats: 1.0, feedback: 0.0, low_cut_hz: lo, high_cut_hz: hi, wet: 1.0, ..Default::default() };
        let x = delay_click(&spec, 120.0, 1.0);
        [band_energy(&x, 0.45, 0.6, 7000.0, 0.0), band_energy(&x, 0.45, 0.6, 700.0, 1100.0), band_energy(&x, 0.45, 0.6, 0.0, 150.0)]
    };
    let (cut, open) = (first(600.0, 1200.0), first(0.0, 0.0));
    let d: Vec<f64> = (0..3).map(|k| db(cut[k]) - db(open[k])).collect();
    println!("the first repeat band-limited against unfiltered: over 7 kHz {:.1} dB, 700 Hz to 1.1 kHz {:.1} dB, under 150 Hz {:.1} dB", d[0], d[1], d[2]);
    assert!(d[0] <= -25.0 && d[2] <= -20.0 && d[1].abs() <= 4.0, "band-limited repeats: over 7 kHz {:.1} dB, the middle {:.1} dB, under 150 Hz {:.1} dB against unfiltered", d[0], d[1], d[2]);
}

// ----- The gated reverb and the reverb's low cut --------------------------------------------------

/// A 20 ms noise burst through the gated room with `spec`, the gate opened at the burst.
fn gated_burst(spec: &RGated, seconds: f64) -> Vec<f32> {
    let mut g = GatedReverb::new(RATE);
    g.set(spec.t60_s, RATE);
    let mut rng = crate::rng::Rng::new(5);
    let total = (seconds * RATE as f64) as usize;
    let mut out = Vec::with_capacity(total);
    let mut i = 0;
    g.trigger();
    while i < total {
        let n = 64.min(total - i);
        let mut s = [0f32; 64];
        for (k, v) in s.iter_mut().enumerate().take(n) {
            if i + k < (0.02 * RATE) as usize {
                *v = rng.bipolar();
            }
        }
        let (mut ol, mut or) = ([0f32; 64], [0f32; 64]);
        g.process(spec, &s[..n], &s[..n], &mut ol[..n], &mut or[..n]);
        out.extend(ol[..n].iter().zip(or[..n].iter()).map(|(a, b)| a + b));
        i += n;
    }
    out
}

/// THE GATE CUTS THE REVERB'S TAIL AT ITS TIME, measured in the rendered output: a burst through the
/// gated room (2.5 s decay, the gate held 0.25 s and closed over 30 ms): from 50 to 200 ms the tail
/// stays within 6 dB of itself (the long decay's level tail, the "non-linear" reverb's flat
/// envelope), and from 0.3 s on it is silent (more than 60 dB under that); the same room with the gate
/// held open 5 s is still within 12 dB of its early level from 0.3 to 0.45 s, so the cut is the gate's.
/// (Measured: -3.3 dB while open, silent after, the open room -10.3 dB.) RED first (2026-11-14) with
/// the gate's gain left at 1 after its hold (`gate_at` returning 1.0 always and the shut room never skipped): "after the gate: -10.3 dB against the held tail (it should be
/// under -60)".
#[test]
fn the_gate_cuts_the_reverb_tail_at_its_time() {
    let spec = RGated { t60_s: 2.5, hold_s: 0.25, release_s: 0.03, wet: 1.0 };
    let x = gated_burst(&spec, 0.6);
    let early = band_energy(&x, 0.05, 0.12, 0.0, 0.0);
    let held = band_energy(&x, 0.13, 0.2, 0.0, 0.0);
    let after = band_energy(&x, 0.3, 0.45, 0.0, 0.0);
    let open = gated_burst(&RGated { hold_s: 5.0, ..spec }, 0.6);
    let open_after = band_energy(&open, 0.3, 0.45, 0.0, 0.0);
    let open_early = band_energy(&open, 0.05, 0.12, 0.0, 0.0);
    println!("gated reverb: held tail {:.1} dB against its start; after the gate {:.1} dB; the open room {:.1} dB at the same time", db(held) - db(early), db(after) - db(early), db(open_after) - db(open_early));
    assert!(early > 1e-6, "the room sounds ({early:.2e})");
    assert!((db(held) - db(early)).abs() <= 6.0, "the tail stays level while the gate is open: {:.1} dB", db(held) - db(early));
    assert!(db(after) - db(early) <= -60.0, "after the gate: {:.1} dB against the held tail (it should be under -60)", db(after) - db(early));
    assert!(db(open_after) - db(open_early) >= -12.0, "the open room still rings at that time: {:.1} dB", db(open_after) - db(open_early));
}

/// White noise for `a..b` seconds through the reverb send's low cut (at `cut` Hz, 0: none) and the
/// score's reverb (8 s decay), the wet output only.
fn reverb_tail(cut: f32, seconds: f64) -> Vec<f32> {
    let mut f = Fdn::new(8.0, RATE);
    let mut hp = [Svf::default(); 2];
    let mut rng = crate::rng::Rng::new(9);
    let total = (seconds * RATE as f64) as usize;
    let mut out = Vec::with_capacity(total);
    let mut i = 0;
    while i < total {
        let n = 64.min(total - i);
        let (mut sl, mut sr) = ([0f32; 64], [0f32; 64]);
        if i < (0.5 * RATE) as usize {
            for k in 0..n {
                sl[k] = rng.bipolar() * 0.3;
                sr[k] = rng.bipolar() * 0.3;
            }
        }
        if cut > 0.0 {
            crate::fx::low_cut(&mut hp, cut, RATE, &mut sl[..n], &mut sr[..n]);
        }
        let (mut ol, mut or) = ([0f32; 64], [0f32; 64]);
        f.process(&sl[..n], &sr[..n], &mut ol[..n], &mut or[..n], 1.0);
        out.extend(ol[..n].iter().zip(or[..n].iter()).map(|(a, b)| a + b));
        i += n;
    }
    out
}

/// THE REVERB'S LOW CUT REMOVES ENERGY UNDER ITS CORNER, measured in the reverb's rendered output (the
/// score's own `low_cut` on the send, then its `Fdn`): half a second of white noise into the hall
/// with the cut at 350 Hz (ambient's), the tail from 0.6 to 2 s carries at least 18 dB less under 120
/// Hz than the same hall without the cut, and within 2 dB the same over 1.4 kHz (measured: -22.4 and
/// -0.0 dB). RED first (2026-11-14) with `low_cut` returning its input untouched: "under 120 Hz: 0.0 dB with the cut".
#[test]
fn the_reverb_low_cut_removes_energy_under_its_corner() {
    let (with, without) = (reverb_tail(350.0, 2.0), reverb_tail(0.0, 2.0));
    let low = db(band_energy(&with, 0.6, 2.0, 0.0, 120.0)) - db(band_energy(&without, 0.6, 2.0, 0.0, 120.0));
    let high = db(band_energy(&with, 0.6, 2.0, 1400.0, 0.0)) - db(band_energy(&without, 0.6, 2.0, 1400.0, 0.0));
    println!("the reverb's low cut at 350 Hz: under 120 Hz {low:.1} dB, over 1.4 kHz {high:.1} dB");
    assert!(low <= -18.0, "under 120 Hz: {low:.1} dB with the cut");
    assert!(high.abs() <= 2.0, "over 1.4 kHz: {high:.1} dB with the cut");
}

// ----- The transitions: downlifter, reversed cymbal ----------------------------------------------

/// One note of the shipped instrument `id` (held `hold_s`), rendered alone, mono.
fn one_note(lib: &crate::MusicLibrary, id: &str, midi: f32, hold_s: f32, seconds: f64) -> Vec<f32> {
    let i = lib.instrument_index(id).unwrap_or_else(|| panic!("the instrument {id}"));
    let insts: Vec<Instrument> = vec![lib.instruments[i].clone()];
    let banks = vec![None];
    let mut pool = VoicePool::new(7);
    let vowel = [(SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    pool.note_on(NoteOn { inst: 0, role: 254, midi, a4: 432.0, vel: 1.0, pan: 0.0, gate: 0.0, hold_s, tag: 1, level: 1.0 }, &insts, &banks, RATE);
    let total = (seconds * RATE as f64) as usize;
    let mut out = Vec::with_capacity(total);
    while out.len() < total {
        let (mut l, mut r, mut sl, mut sr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
        pool.render(&insts, &banks, &BlockParams::plain(RATE, 2.0, &vowel), &mut l, &mut r, &mut sl, &mut sr, 64);
        out.extend(l.iter().zip(r.iter()).map(|(a, b)| a + b));
    }
    out.truncate(total);
    out
}

/// The spectral centroid (Hz) of `x[a..b]` seconds, from the energy in octave bands 125 Hz to 16 kHz.
fn centroid(x: &[f32], a: f64, b: f64) -> f64 {
    let mut num = 0.0;
    let mut den = 0.0;
    let mut f = 125.0f32;
    while f <= 16000.0 {
        let e = band_energy(x, a, b, f / 1.414, f * 1.414);
        num += e * f as f64;
        den += e;
        f *= 2.0;
    }
    num / den.max(1e-30)
}

/// THE DOWNLIFTER FALLS AND THE REVERSED CYMBAL SWELLS, measured in their rendered sound: the
/// downlifter's spectral centroid in its first 0.3 s is at least three times its centroid 1.5 to 2 s
/// in, and it is quieter there; the reversed cymbal held 2 s is at least 30 dB louder in its last 0.1 s
/// than in its first, rises the whole way (each quarter louder than the one before), and is silent
/// (60 dB under its peak) from 10 ms after its end (measured: the centroid 7231 Hz, then 785 Hz; the
/// cymbal's quarters -66.4, -54.5, -42.4 and -30.4 dB, its last 0.1 s +45.7 dB over its first, -67.6 dB
/// after). RED first (2026-11-14) with the drum machine's `reverse` ignored (the cymbal struck
/// forwards): "the reversed cymbal: its last 0.1 s -50.5 dB against its first (a swell is at
/// least +30)" (its quarters falling: -30.8, -44.2, -57.5, -70.8 dB).
#[test]
fn the_downlifter_falls_and_the_reversed_cymbal_swells() {
    let lib = shipped();
    let d = one_note(&lib, "downlifter", 60.0, 0.0, 2.2);
    let (c0, c1) = (centroid(&d, 0.02, 0.3), centroid(&d, 1.5, 2.0));
    let (e0, e1) = (band_energy(&d, 0.02, 0.3, 0.0, 0.0), band_energy(&d, 1.5, 2.0, 0.0, 0.0));
    println!("the downlifter: centroid {c0:.0} Hz at first, {c1:.0} Hz 1.5 to 2 s in; {:.1} dB quieter there", db(e0) - db(e1));
    assert!(c0 >= 3.0 * c1, "the downlifter falls: centroid {c0:.0} Hz, then {c1:.0} Hz");
    assert!(e1 < e0, "the downlifter fades as it falls");
    let r = one_note(&lib, "reverse_cymbal", 60.0, 2.0, 2.3);
    let quarters: Vec<f64> = (0..4).map(|k| db(band_energy(&r, 0.5 * k as f64, 0.5 * (k + 1) as f64, 0.0, 0.0))).collect();
    let (first, last) = (db(band_energy(&r, 0.0, 0.1, 0.0, 0.0)), db(band_energy(&r, 1.9, 2.0, 0.0, 0.0)));
    let after = db(band_energy(&r, 2.01, 2.3, 0.0, 0.0));
    println!("the reversed cymbal: quarters {quarters:.1?} dB; its last 0.1 s {:.1} dB against its first; after its end {:.1} dB", last - first, after - last);
    assert!(last - first >= 30.0, "the reversed cymbal: its last 0.1 s {:.1} dB against its first (a swell is at least +30)", last - first);
    assert!(quarters.windows(2).all(|w| w[1] > w[0]), "the reversed cymbal rises the whole way: {quarters:.1?}");
    assert!(after - last <= -60.0, "the reversed cymbal stops dead at its end: {:.1} dB after", after - last);
}

// ----- The tape stop -------------------------------------------------------------------------------

/// A sine-ish test voice, and a genre whose first section holds one note and ends in a two-beat tape
/// stop at 120 bpm (a bar 2 s, the stop from 3 s to 4 s), the second holding the same note.
const SINE: &str = r#"Instrument(id: "tape_sine", kind: Synth((
    oscillators: [(wave: Sine)],
    envelope: (attack_s: 0.01, decay_s: 0.0, sustain: 1.0, release_s: 0.05),
)), level_db: -6.0)"#;
const TAPE_GENRE: &str = r#"Genre(
    id: "tape_test",
    name: "Tape test",
    tempo_bpm: Some((120.0, 120.0)),
    beat_felt: Some(true),
    beats_per_bar: Some(4),
    groove: Some((swing: 0.5, subdivision: 4)),
    structure: Some((sections: [(name: "a", bars: 2, roles: [Pad], tape_stop_beats: 2.0), (name: "b", bars: 2, roles: [Pad])], gap_beats: 0.0)),
    progressions: Some((keys: [(root: "A", mode: Aeolian, weight: 1.0)], chords: [(name: "i", root: 0, notes: [0])], moves: [("i", "i", 1.0)], cadences: ["i"], start: "i")),
    roles_off: [Choir, Drone, Percussion, Bells, Melody, Bass, Texture],
    roles_add: [(role: Pad, instrument: "tape_sine", low: "A3", high: "A3", voices: 1)],
    recipe: (reverb_scale: 0.0),
)"#;

/// The pitch (Hz) of `x[a..b]` seconds from its upward zero crossings.
fn crossing_hz(x: &[f32], a: f64, b: f64) -> f64 {
    let (i0, i1) = ((a * RATE as f64) as usize, (b * RATE as f64) as usize);
    let ups: Vec<usize> = (i0 + 1..i1).filter(|&i| x[i - 1] <= 0.0 && x[i] > 0.0).collect();
    if ups.len() < 2 {
        return 0.0;
    }
    (ups.len() - 1) as f64 * RATE as f64 / (ups[ups.len() - 1] - ups[0]) as f64
}

/// THE TAPE STOP'S PITCH FALLS, AND ITS LEVEL WITH IT, measured in the rendered sound: one held A3
/// (216 Hz at A = 432) in a section ending in a two-beat tape stop at 120 bpm (3 s to 4 s): before the
/// stop it sounds at 216 Hz; across the stop its pitch falls quarter by quarter (each lower than the
/// last, the last under 40% of the first), and its level with it (its last 20 ms at least 30 dB under
/// where it began); the next section strikes the note again at 216 Hz (measured: 189.4, 135.5, 82.3
/// and 35.0 Hz, where a straight-line fall of the speed predicts 189, 135, 81 and 27; -37.4 dB at the
/// end). RED first (2026-11-14) with the score ignoring the composer's tape speed (the tape path never
/// taken): "the pitch across the stop [216.0, 216.0, 216.0, 216.0] Hz: it should fall".
#[test]
fn the_tape_stop_pitch_falls() {
    let mut lib = shipped();
    lib.instruments.push(ron::from_str(SINE).expect("the test voice parses"));
    lib.genres.push(ron::from_str(TAPE_GENRE).expect("the test genre parses"));
    let g = lib.genre_index("tape_test").unwrap() as u16;
    let ctx = crate::MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index("mothership_calm").unwrap() as u16, genre_a: g, genre_b: g, ..Default::default() };
    let mut s = crate::ScoreEngine::new(Resolved::new(&lib, &SampleLibrary::new()).unwrap(), RATE, 3, ctx);
    let r = crate::render::render_score(&mut s, RATE, 6.0);
    let x: Vec<f32> = r.frames.iter().map(|f| 0.5 * (f[0] + f[1])).collect();
    let before = crossing_hz(&x, 2.5, 2.95);
    let quarters: Vec<f64> = (0..4).map(|k| crossing_hz(&x, 3.0 + 0.25 * k as f64, 3.0 + 0.25 * (k + 1) as f64)).collect();
    let levels: Vec<f64> = [(2.8, 2.84), (3.98, 4.0)].iter().map(|&(a, b)| db(band_energy(&x, a, b, 0.0, 0.0))).collect();
    let next = crossing_hz(&x, 4.3, 4.9);
    println!("the tape stop: {before:.1} Hz before, {quarters:.1?} Hz across it, its level {:.1} dB at the end against before, {next:.1} Hz after", levels[1] - levels[0]);
    assert!((before - 216.0).abs() < 2.0, "before the stop: {before:.1} Hz");
    assert!(quarters.windows(2).all(|w| w[1] < w[0]) && quarters[3] < 0.4 * quarters[0], "the pitch across the stop {quarters:.1?} Hz: it should fall");
    assert!(levels[1] - levels[0] <= -30.0, "silent at the stop's end: {:.1} dB", levels[1] - levels[0]);
    assert!((next - 216.0).abs() < 2.0, "the next section at {next:.1} Hz");
}

// ----- Fills ---------------------------------------------------------------------------------------

/// FILLS LAND AT PHRASE ENDS, from the composer's decisions over 6 minutes of house: every tom note
/// (the fill lines' instruments) falls in the last bar of its phrase, in at least half the phrases
/// that may have one (not a build's, whose roll is its fill); and from a fill's first tom to the bar's
/// end, no hat, clap or snare is struck (the fill takes over), while the kick, marked to play under
/// fills, keeps the floor. RED first (2026-11-14) with `fill` ignored by the composer (a fill line
/// played in every bar like any other): "seed 3: a tom 7.5 bars before its phrase's end".
#[test]
fn fills_land_at_phrase_ends() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let gi = lib.genre_index("house").expect("the house genre");
    let g = &lib.genres[gi];
    let id_of = |n: &NoteOn| lib.instruments[n.inst as usize].id.clone();
    for seed in [3u64, 4] {
        let mut c = Composer::new(48000.0, seed, steer(&lib, "mothership_calm", "house"));
        let mut rec = Recorder::default();
        run(&mut c, &res, 360.0, &mut rec);
        let bpm = c.tempo() as f64;
        let bar = 4.0 * 60.0 / bpm;
        // Phrase starts, whether each is a build, and whether a fill line plays in it.
        let st = g.structure.as_ref().expect("house has a structure");
        let fill_sections: Vec<&String> = g.drums.iter().filter(|d| d.fill).flat_map(|d| d.sections.iter()).collect();
        let mut phrases: Vec<(f64, bool, bool)> = Vec::new();
        for e in &rec.events {
            if let Event::Structure { at_s, build, index, .. } = e {
                phrases.push((*at_s, *build, fill_sections.contains(&&st.sections[*index as usize].name)));
            }
        }
        assert!(phrases.len() >= 6, "house plays its structure ({} sections)", phrases.len());
        let mut filled = 0;
        let mut could = 0;
        for w in phrases.windows(2) {
            let (start, end, build, may) = (w[0].0, w[1].0, w[0].1, w[0].2);
            let toms: Vec<f64> = rec.notes.iter().filter(|(t, n)| *t >= start - 1e-6 && *t < end - 1e-6 && n.role == 255 && id_of(n).starts_with("tom_")).map(|(t, _)| *t).collect();
            for &t in &toms {
                let before_end = (end - t) / bar;
                assert!(before_end <= 1.0 + 1e-3, "seed {seed}: a tom {before_end:.1} bars before its phrase's end");
            }
            if !build && may {
                could += 1;
                if let Some(&first) = toms.first() {
                    filled += 1;
                    let others: Vec<String> = rec.notes.iter().filter(|(t, n)| *t > first + 1e-4 && *t < end - 1e-4 && n.role == 255).map(|(_, n)| id_of(n)).filter(|id| !id.starts_with("tom_") && !id.starts_with("kick_")).collect();
                    assert!(others.is_empty(), "seed {seed}: under the fill at {first:.2} s: {others:?}");
                }
            } else {
                assert!(toms.is_empty(), "seed {seed}: no fill in a build, or in a section without a fill line");
            }
        }
        println!("house seed {seed}: fills in {filled} of {could} phrases that may have one");
        assert!(filled * 2 >= could, "seed {seed}: fills in only {filled} of {could} phrases");
    }
}

// ----- The genres ----------------------------------------------------------------------------------

/// A score of `genre` over the calm mood, seed `seed`.
fn genre_score(lib: &crate::MusicLibrary, genre: &str, seed: u64) -> crate::ScoreEngine {
    let g = lib.genre_index(genre).unwrap_or_else(|| panic!("the {genre} genre")) as u16;
    let ctx = crate::MusicContext { gain: 1.0, a4_hz: 432.0, mood: lib.mood_index("mothership_calm").unwrap() as u16, genre_a: g, genre_b: g, blend: 0.0, intensity: 0.3, duck_db: 0.0, ..Default::default() };
    crate::ScoreEngine::new(Resolved::new(lib, &SampleLibrary::new()).unwrap(), RATE, seed, ctx)
}

///// The period of onsets on a grid of about `approx` seconds: each onset's distance from the first over
/// the whole number of periods between them (from four periods on, within 15% of a period of the grid,
/// so a millisecond of onset jitter is a fraction of a percent of the answer), the median.
fn grid_period(onsets: &[f64], approx: f64) -> f64 {
    let Some(&t0) = onsets.first() else { return 0.0 };
    let mut v: Vec<f64> = onsets
        .iter()
        .skip(1)
        .filter_map(|&t| {
            let k = (t - t0) / approx;
            let n = k.round();
            (n >= 4.0 && (k - n).abs() < 0.15).then(|| (t - t0) / n)
        })
        .collect();
    v.sort_by(|a, b| a.total_cmp(b));
    v.get(v.len() / 2).copied().unwrap_or(0.0)
}

/// Where in a bar (in beats, 0 .. 4) each onset falls, the bar starting at `t0`.
fn bar_beats(onsets: &[f64], t0: f64, bar: f64) -> Vec<f64> {
    onsets.iter().map(|&t| ((t - t0) / bar).rem_euclid(1.0) * 4.0).collect()
}

/// A piece of `genre` (seed `seed`, `seconds` long) rendered as it is, and what the drum lines whose
/// instruments start with each of `lines` add to it: the same piece with those lines struck at a
/// ten-thousandth of their velocity, subtracted (the composer decides the very same notes, and a
/// kick that ducks still ducks, so the difference is those lines' own sound in the mix). Mono, with
/// the composer's tempo.
fn with_lines(lib: &crate::MusicLibrary, genre: &str, seed: u64, seconds: f64, lines: &[&str]) -> (f64, Vec<f32>, Vec<Vec<f32>>) {
    let mono = |lib: &crate::MusicLibrary| -> (f64, Vec<f32>) {
        let mut s = genre_score(lib, genre, seed);
        let r = crate::render::render_score(&mut s, RATE, seconds);
        (s.composer().tempo() as f64, r.frames.iter().map(|f| 0.5 * (f[0] + f[1])).collect())
    };
    let (bpm, full) = mono(lib);
    let gi = lib.genre_index(genre).unwrap();
    let parts = lines
        .iter()
        .map(|prefix| {
            let mut quiet = lib.clone();
            for d in quiet.genres[gi].drums.iter_mut().filter(|d| d.instrument.starts_with(prefix)) {
                d.velocity *= 1e-4;
            }
            let (_, q) = mono(&quiet);
            full.iter().zip(q.iter()).map(|(a, b)| a - b).collect()
        })
        .collect();
    (bpm, full, parts)
}

/// The frames of `r` from `a` to `b` seconds.
fn slice(r: &crate::render::Rendered, a: f64, b: f64) -> &[[f32; 2]] {
    let (i0, i1) = ((a * RATE as f64) as usize, ((b * RATE as f64) as usize).min(r.frames.len()));
    &r.frames[i0..i1]
}

/// Onsets (s) in mono `x` from `a` to `b` seconds, in a band (as `band_onsets`), measured from 0.
fn onsets(x: &[f32], a: f64, b: f64, lo: f64, hi: f64, floor: f64) -> Vec<f64> {
    let (i0, i1) = ((a * RATE as f64) as usize, ((b * RATE as f64) as usize).min(x.len()));
    let frames: Vec<[f32; 2]> = x[i0..i1].iter().map(|&v| [v, v]).collect();
    band_onsets(&frames, lo, hi, floor).into_iter().map(|t| t + a).collect()
}

/// HOUSE RENDERS FOUR ON THE FLOOR AT ITS TEMPO, THE CLAP ON 2 AND 4, ITS BREAKDOWN WITHOUT THE KICK,
/// measured in the sound (each drum line's own sound in the mix, as `with_lines` takes it): in the
/// intro and the groove (bars 1 to 16) the kick's onsets lie on a grid of one beat at the tempo the
/// composer chose (within 0.5 bpm, in 120 to 126), at least 90% of them within 20 ms of a beat and at
/// least 56 of them (one on every beat); the clap's onsets once it joins (bars 9 to 16) at least 90% on
/// 2 and 4; and in the breakdown (bars 33 to 40) the whole mix's low band (40 to 90 Hz) at least 10 dB
/// quieter than in the main section (bars 17 to 32). RED first (2026-11-14) with the kick written
/// "x.......x......." (on 1 and 3): "house: 33 kick onsets on a 0.4902 s grid, 122.39 bpm" (half the
/// kicks a beat wants: the grid fits every other beat).
#[test]
fn house_renders_four_on_the_floor() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let (bpm, full, lines) = with_lines(&lib, "house", 11, 80.0, &["kick_", "clap_"]);
    let beat = 60.0 / bpm;
    let bar = 4.0 * beat;
    let kicks = onsets(&lines[0], 0.0, 16.0 * bar, 30.0, 2000.0, 0.2);
    let period = grid_period(&kicks, beat);
    let measured = 60.0 / period.max(1e-6);
    let on_beat = bar_beats(&kicks, 0.0, bar).iter().filter(|&&p| (p - p.round()).abs() * beat <= 0.02).count() as f64 / kicks.len().max(1) as f64;
    println!("house: {} kick onsets in bars 1 to 16 on a {period:.4} s grid, {measured:.2} bpm (the composer {bpm:.2}); {:.0}% within 20 ms of a beat", kicks.len(), on_beat * 100.0);
    assert!((120.0..=126.0).contains(&bpm), "house's tempo {bpm:.1}");
    assert!(kicks.len() >= 56 && (measured - bpm).abs() <= 0.5, "house: {} kick onsets on a {period:.4} s grid, {measured:.2} bpm", kicks.len());
    assert!(on_beat >= 0.9, "house: {:.0}% of the kick onsets on a beat", on_beat * 100.0);
    let claps = onsets(&lines[1], 8.0 * bar, 16.0 * bar, 200.0, 8000.0, 0.2);
    let share = bar_beats(&claps, 0.0, bar).iter().filter(|&&p| (p - 1.0).abs() < 0.08 || (p - 3.0).abs() < 0.08).count() as f64 / claps.len().max(1) as f64;
    println!("house: {} clap onsets in bars 9 to 16, {:.0}% on 2 and 4", claps.len(), share * 100.0);
    assert!(claps.len() >= 14 && share >= 0.9, "house: {} clap onsets, {:.0}% on 2 and 4", claps.len(), share * 100.0);
    let main_low = band_energy(&full, 16.0 * bar + 1.0, 32.0 * bar, 40.0, 90.0);
    let breakdown_low = band_energy(&full, 32.0 * bar + 1.0, 40.0 * bar - 0.5, 40.0, 90.0);
    println!("house: the breakdown's low band {:.1} dB against the main section's", db(breakdown_low) - db(main_low));
    assert!(db(breakdown_low) - db(main_low) <= -10.0, "house's breakdown keeps the kick: {:.1} dB", db(breakdown_low) - db(main_low));
}

/// DRUM AND BASS RENDERS THE TWO-STEP AT 170 TO 176, measured in the sound of its first drop (bars 25
/// to 40, after the 16-bar intro and the 8-bar build; each drum line's own sound in the mix, as
/// `with_lines` takes it): the snare's onsets lie on a grid of half a bar at the composer's tempo
/// (within 0.5 bpm, in 170 to 176), at least 40% of them on 2 and at least 85% on the backbeat (2, and
/// 4 or an eighth either side of it: the displaced second snare a phrase may draw); and at least 90%
/// of its bars have a kick on 1 and one on the "and" of 3, and no kick falls on 2 or 4
/// (the two-step, not four on the floor). RED first (2026-11-14) with the kick written four on the
/// floor: "drum and bass: 0 of 15 bars with a kick on 1 and on the and of 3, 32 kicks on 2 or 4".
#[test]
fn drum_and_bass_renders_the_two_step() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let (bpm, _, lines) = with_lines(&lib, "drum_and_bass", 11, 60.0, &["kick_", "snare_"]);
    let beat = 60.0 / bpm;
    let bar = 4.0 * beat;
    let (from, to) = (24.0 * bar, (40.0 * bar).min(59.5));
    let snares = onsets(&lines[1], from, to, 200.0, 10000.0, 0.25);
    let period = grid_period(&snares, 2.0 * beat);
    let measured = 2.0 * 60.0 / period.max(1e-6);
    let sb = bar_beats(&snares, from, bar);
    let share = |at: &[f64]| sb.iter().filter(|&&p| at.iter().any(|&a| (p - a).abs() < 0.1)).count() as f64 / snares.len().max(1) as f64;
    // The first snare of each bar on 2; the second on 4, or an eighth either side of it (2.5 or 3.5
    // in the bar's beats counted from 0) where the phrase drew a displaced pattern.
    let (on_2, backbeat) = (share(&[1.0]), share(&[1.0, 3.0, 2.5, 3.5]));
    println!("drum and bass: {} snare onsets in the drop on a {period:.4} s grid, {measured:.2} bpm (the composer {bpm:.2}); {:.0}% on 2, {:.0}% on 2, 4 or 4 displaced", snares.len(), on_2 * 100.0, backbeat * 100.0);
    assert!((170.0..=176.0).contains(&bpm), "drum and bass's tempo {bpm:.1}");
    assert!(snares.len() >= 16 && (measured - bpm).abs() <= 0.5, "drum and bass: {} snare onsets on a {period:.4} s grid, {measured:.2} bpm", snares.len());
    assert!(on_2 >= 0.4 && backbeat >= 0.85, "drum and bass: {:.0}% of the snare onsets on 2, {:.0}% on the backbeat", on_2 * 100.0, backbeat * 100.0);
    let kicks = onsets(&lines[0], from, to, 30.0, 2000.0, 0.2);
    // Bar by bar: a kick on 1 and one on the "and" of 3 (2.5 beats in), never one on 2 or 4 (a
    // phrase may draw a pattern with a third kick elsewhere: on the "and" of 1, or late in the bar).
    let bars = ((to - from) / bar).floor() as usize;
    let near = |b: usize, beats: f64| kicks.iter().any(|&t| (t - (from + b as f64 * bar + beats * beat)).abs() < 0.15 * beat);
    let with_both = (0..bars).filter(|&b| near(b, 0.0) && near(b, 2.5)).count();
    let on_24k = bar_beats(&kicks, from, bar).iter().filter(|&&p| (p - 1.0).abs() < 0.15 || (p - 3.0).abs() < 0.15).count();
    println!("drum and bass: {} kick onsets in the drop; {with_both} of {bars} bars with a kick on 1 and on the and of 3, {on_24k} on 2 or 4", kicks.len());
    assert!(bars >= 12 && with_both * 10 >= bars * 9 && on_24k == 0, "drum and bass: {with_both} of {bars} bars with a kick on 1 and on the and of 3, {on_24k} kicks on 2 or 4");
}

/// SYNTHWAVE RENDERS ITS BACKBEAT WITH A GATED SNARE, measured in the sound of its first verse (bars 9
/// to 24): the gated room's own sound (the piece rendered with it less the same piece without it: the
/// same notes, so the difference is the room) begins at each snare on 2 and 4, on a grid of half a
/// bar at the composer's tempo (within 0.5 bpm, in 80 to 118); from 60 to 180 ms after each hit it is
/// no more than 6 dB under the whole mix (a big room, heard), and from 330 to 450 ms after it at least
/// 60 dB under itself (cut dead by the gate). RED first (2026-11-14) with the snare line's `gated` at
/// 0: "synthwave: the gated room begins 0 times"; and, as first balanced, the room's wet at 1.2: "the
/// gated room 60-180 ms after a snare is -19.5 dB under the mix" (a room nobody would hear), then -6.5
/// dB with the wet asked at 6.5 and at 8 alike, which showed the data loader holding it to 4.
#[test]
fn synthwave_renders_its_gated_backbeat() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let render = |lib: &crate::MusicLibrary| -> (f64, Vec<f32>) {
        let mut s = genre_score(lib, "synthwave", 11);
        let r = crate::render::render_score(&mut s, RATE, 75.0);
        (s.composer().tempo() as f64, r.frames.iter().map(|f| 0.5 * (f[0] + f[1])).collect())
    };
    let (bpm, with) = render(&lib);
    let mut dry = lib.clone();
    let gi = dry.genre_index("synthwave").unwrap();
    dry.genres[gi].gated_reverb = None;
    let (_, without) = render(&dry);
    let beat = 60.0 / bpm;
    let bar = 4.0 * beat;
    let (from, to) = (8.0 * bar, (24.0 * bar).min(74.0));
    let room: Vec<f32> = with.iter().zip(without.iter()).map(|(a, b)| a - b).collect();
    let hits = onsets(&room, from, to, 200.0, 10000.0, 0.1);
    let period = grid_period(&hits, 2.0 * beat);
    let measured = 2.0 * 60.0 / period.max(1e-6);
    let on_24 = bar_beats(&hits, from, bar).iter().filter(|&&p| (p - 1.0).abs() < 0.1 || (p - 3.0).abs() < 0.1).count() as f64 / hits.len().max(1) as f64;
    let (mut room_held, mut room_shut, mut mix_held) = (0.0, 0.0, 0.0);
    for &t in &hits {
        room_held += band_energy(&room, t + 0.06, t + 0.18, 0.0, 0.0);
        room_shut += band_energy(&room, t + 0.33, t + 0.45, 0.0, 0.0);
        mix_held += band_energy(&with, t + 0.06, t + 0.18, 0.0, 0.0);
    }
    let (heard, cut) = (db(room_held) - db(mix_held), db(room_shut) - db(room_held));
    println!("synthwave: the gated room begins {} times on a {period:.4} s grid, {measured:.2} bpm (the composer {bpm:.2}), {:.0}% on 2 and 4; 60-180 ms after a hit it is {heard:.1} dB against the mix, 330-450 ms after {cut:.1} dB against itself", hits.len(), on_24 * 100.0);
    assert!((80.0..=118.0).contains(&bpm), "synthwave's tempo {bpm:.1}");
    assert!(hits.len() >= 12 && (measured - bpm).abs() <= 0.5 && on_24 >= 0.9, "synthwave: the gated room begins {} times, {measured:.2} bpm, {:.0}% on 2 and 4", hits.len(), on_24 * 100.0);
    assert!(heard >= -6.0, "synthwave: the gated room 60-180 ms after a snare is {heard:.1} dB under the mix");
    assert!(cut <= -60.0, "synthwave: the gated room 330-450 ms after a snare {cut:.1} dB against itself earlier (the gate cuts it)");
}

/// AMBIENT RENDERS WITH NO BEAT AND ITS DELAY IN TIME, measured in the sound: over 60 s the low band
/// (40 to 100 Hz) has at most two onsets (a pad or the drone entering, no drums); and the delay's own
/// sound (the piece rendered with it less the same piece without it) follows the bells by the delay's
/// note value at the composer's tempo: the loudness envelopes of the bells' band (600 Hz to 1.2 kHz)
/// in the piece without the delay and in the delay's sound correlate best at that lag (within 20 ms,
/// searched from 0.2 to 3 s). RED first (2026-11-14) with the delay's time taken at 120 bpm whatever
/// the tempo: the run printed "the delay's sound follows the bells by 0.500 s, its note
/// value 0.951 s" (its first failure was the low-band count, broken alongside); and, as first written, with a dotted quarter
/// at 64.7 bpm (1.391 s, longer than the delay's 1.365 s): "the delay's sound follows the bells by
/// 1.366 s" (the data now refuses a delay its line cannot hold).
#[test]
fn ambient_renders_with_no_beat_and_its_delay_in_time() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let render = |lib: &crate::MusicLibrary| -> (f64, crate::render::Rendered) {
        let mut s = genre_score(lib, "ambient", 5);
        let r = crate::render::render_score(&mut s, RATE, 60.0);
        (s.composer().tempo() as f64, r)
    };
    let (bpm, with) = render(&lib);
    let lows = band_onsets(&with.frames, 40.0, 100.0, 0.25).len();
    let gi = lib.genre_index("ambient").unwrap();
    let division = lib.genres[gi].delay.as_ref().map(|d| crate::genre::division_beats(&d.division).unwrap()).expect("ambient has a delay");
    let mut dry = lib.clone();
    dry.genres[gi].delay = None;
    let (_, without) = render(&dry);
    // Envelopes at 1 kHz (1 ms windows) of the bells' band.
    let env = |x: &[f32]| -> Vec<f64> {
        let (c1, c2) = (SvfCoefs::new(600.0, 0.707, RATE), SvfCoefs::new(1200.0, 0.707, RATE));
        let (mut a, mut b) = (Svf::default(), Svf::default());
        let y: Vec<f32> = x.iter().map(|&v| b.lp(&c2, a.hp(&c1, v))).collect();
        let e: Vec<f64> = y.chunks(48).map(|c| (c.iter().map(|v| (v * v) as f64).sum::<f64>() / c.len() as f64).sqrt()).collect();
        let m = e.iter().sum::<f64>() / e.len().max(1) as f64;
        e.into_iter().map(|v| v - m).collect()
    };
    let mono = |r: &crate::render::Rendered| -> Vec<f32> { r.frames.iter().map(|f| 0.5 * (f[0] + f[1])).collect() };
    let (w, wo) = (mono(&with), mono(&without));
    let room: Vec<f32> = w.iter().zip(wo.iter()).map(|(a, b)| a - b).collect();
    let (dry_env, delay_env) = (env(&wo), env(&room));
    let xc = |lag: usize| dry_env.iter().zip(delay_env.iter().skip(lag)).map(|(p, q)| p * q).sum::<f64>();
    let best = (200..3000).max_by(|&a, &b| xc(a).total_cmp(&xc(b))).unwrap();
    let want = division as f64 * 60.0 / bpm;
    println!("ambient at {bpm:.1} bpm: {lows} low onsets; the delay's sound follows the bells by {:.3} s, its note value {want:.3} s", best as f64 / 1000.0);
    assert!(lows <= 2, "ambient has no drums: {lows} low onsets");
    assert!((best as f64 / 1000.0 - want).abs() <= 0.02, "ambient: the delay's sound follows the bells by {:.3} s, its note value is {want:.3} s", best as f64 / 1000.0);
}

/// THE FOUR GENRES PLAY THEIR STRUCTURES AND VARY BY SEED, from the composer's decisions: for each of
/// house, drum and bass and synthwave, its sections begin in the data's order at the data's bars (the
/// drop where the file puts it); for each of the four, four seeds give at least three different
/// (key, tempo) pairs or first-section chord lists. RED first (2026-11-14) with the pieces' draws
/// keyed by a fixed seed (`Piece::new(0)`): "house: 1 different pieces from 4 seeds".
#[test]
fn the_new_genres_play_their_structures_and_vary_by_seed() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    for id in ["house", "drum_and_bass", "synthwave", "ambient"] {
        let g = &lib.genres[lib.genre_index(id).unwrap()];
        let mut seen = std::collections::BTreeSet::new();
        for seed in [1u64, 2, 3, 4] {
            let mut c = Composer::new(48000.0, seed, steer(&lib, "mothership_calm", id));
            let mut rec = Recorder::default();
            run(&mut c, &res, 200.0, &mut rec);
            let chords: Vec<u16> = rec.events.iter().filter_map(|e| if let Event::Chord { chord, .. } = e { Some(*chord) } else { None }).take(6).collect();
            seen.insert(format!("{} {:.1} {chords:?}", c.key_pc(), c.tempo()));
            if let (Some(st), 1) = (&g.structure, seed) {
                let bar = 4.0 * 60.0 / c.tempo() as f64;
                let starts: Vec<(u8, f64)> = rec.events.iter().filter_map(|e| if let Event::Structure { index, at_s, .. } = e { Some((*index, *at_s)) } else { None }).collect();
                let mut at = 0.0;
                for (k, sec) in st.sections.iter().enumerate().take(starts.len().min(st.sections.len())) {
                    assert_eq!(starts[k].0 as usize, k, "{id}: section {k} in order");
                    assert!((starts[k].1 - at).abs() < 0.05, "{id}: {} begins at {:.2} s, the data puts it at {at:.2} s", sec.name, starts[k].1);
                    at += sec.bars as f64 * bar;
                }
                assert!(starts.len() >= 4, "{id}: {} sections in 200 s", starts.len());
            }
        }
        println!("{id}: {} different pieces from 4 seeds", seen.len());
        assert!(seen.len() >= 3, "{id}: {} different pieces from 4 seeds", seen.len());
    }
}

//// THE FOUR GENRES RENDER AT THE MUSIC TARGET (docs/design/music.md section 13): one time through each
/// structure (ambient, without one: 150 s), two seeds, measures -18 LUFS within 1 LU under the -1 dBTP
/// ceiling; house's and drum and bass's first drops no more than 2 LU over the whole (heavy never means
/// louder than the category). Every genre and seed is measured before the verdict, so one run levels
/// all four. RED first (2026-11-14) with every new genre at a level of 0 dB, before each was measured:
/// "house seed 11: -23.3 LUFS, the target -18.0; ... drum_and_bass seed 11: -16.0 LUFS, the
/// target -18.0; ... synthwave seed 11: -20.4 LUFS, the target -18.0" (ambient -18.4 at 0 dB then;
/// it measured -22.7 when first written, before its hall and delay settled).
#[test]
fn the_new_genres_render_at_the_music_target() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let targets = humanity_core::audio::levels::LoudnessTargets::load(&crate::tests::data_dir()).unwrap();
    let music = targets.target_lufs("music");
    let mut wrong = Vec::new();
    for id in ["house", "drum_and_bass", "synthwave", "ambient"] {
        let g = &lib.genres[lib.genre_index(id).unwrap()];
        let bars: u32 = g.structure.as_ref().map(|s| s.sections.iter().map(|x| x.bars).sum()).unwrap_or(0);
        // Where the first drop is, in bars from the start.
        let drop = g.structure.as_ref().and_then(|s| {
            let i = s.sections.iter().position(|x| x.drop)?;
            Some((s.sections[..i].iter().map(|x| x.bars).sum::<u32>(), s.sections[i].bars))
        });
        for seed in [11u64, 7] {
            let mut s = genre_score(&lib, id, seed);
            crate::render::render_score(&mut s, RATE, 0.05);
            let bar = 4.0 * 60.0 / s.composer().tempo() as f64;
            let seconds = if bars > 0 { bars as f64 * bar - 0.05 } else { 150.0 };
            let r = crate::render::render_score(&mut s, RATE, seconds);
            let lufs = |a: f64, b: f64| {
                let mut m = humanity_core::audio::loudness::LoudnessMeter::new(RATE as u32, 2);
                for f in slice(&r, a, b) {
                    m.push_frame(f);
                }
                m.finish();
                (m.integrated().unwrap_or(f64::NEG_INFINITY), m.true_peak_dbtp())
            };
            let (whole, peak) = lufs(0.0, seconds);
            let drop_lufs = drop.map(|(at, len)| lufs(at as f64 * bar, (at + len) as f64 * bar).0);
            println!("{id} seed {seed}: {whole:.1} LUFS over {seconds:.0} s, true peak {peak:.2} dBTP{}", drop_lufs.map(|d| format!(", its first drop {d:.1}")).unwrap_or_default());
            if (whole - music).abs() > 1.0 {
                wrong.push(format!("{id} seed {seed}: {whole:.1} LUFS, the target {music:.1}"));
            }
            if peak > targets.ceiling_dbtp {
                wrong.push(format!("{id} seed {seed}: true peak {peak:.2} dBTP"));
            }
            if let Some(d) = drop_lufs.filter(|d| d - whole > 2.0) {
                wrong.push(format!("{id} seed {seed}: its drop {d:.1} LUFS, {:.1} LU over the whole", d - whole));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

/// THE FOUR GENRES FIT THE AUDIO BUDGET (docs/design/music.md section 14), their effects included: each
/// genre's busiest stretch (house and drum and bass in their first drop, synthwave in its first chorus,
/// ambient 30 s in), 128-frame blocks average at most 0.05 ms with the 99th percentile at most 0.25
/// ms, the best of three seeds, both scaled by how much slower than its quiet 0.037 ms the calm score
/// ran in the same run (the merged lesson: other agents' builds swing every cost twice over). Every
/// genre is measured before the verdict. RED first (2026-11-14), as first balanced: "synthwave: 0.0914
/// ms a block on average (budget 0.0871; the calm score 0.0645 ms this run)" (its supersaw arpeggio's
/// notes ringing 0.35 s each, three wavetable pad voices).
#[test]
fn the_new_genres_fit_the_audio_budget() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let calm = crate::tests_e5::calm_ms(&lib);
    let scale = (calm / 0.037).max(1.0);
    let mut wrong = Vec::new();
    for (id, bars_in) in [("house", 48.0), ("drum_and_bass", 24.0), ("synthwave", 24.0), ("ambient", 0.0)] {
        let mut best = (f64::MAX, f64::MAX, 0.0);
        for seed in [1u64, 2, 3] {
            let mut s = genre_score(&lib, id, seed);
            crate::render::render_score(&mut s, RATE, 0.05);
            let bpm = s.composer().tempo() as f64;
            let skip = if bars_in > 0.0 { bars_in * 4.0 * 60.0 / bpm } else { 30.0 };
            crate::render::render_score(&mut s, RATE, skip);
            let mut times = Vec::new();
            let mut voices = 0.0;
            for _ in 0..(8.0 * RATE / 128.0) as usize {
                let mut buf = [[0f32; 2]; 128];
                let t0 = std::time::Instant::now();
                s.render(&mut buf);
                times.push(t0.elapsed().as_secs_f64() * 1000.0);
                voices += s.status().voices as f64;
            }
            times.sort_by(|a, b| a.total_cmp(b));
            let kept = &times[..(times.len() as f64 * 0.98) as usize];
            let mean = kept.iter().sum::<f64>() / kept.len() as f64;
            let p99 = times[(times.len() as f64 * 0.99) as usize];
            if mean < best.0 {
                best = (mean, p99, voices / times.len() as f64);
            }
        }
        println!("{id}: {:.4} ms a block on average, {:.4} ms at the 99th percentile, {:.1} voices (the calm score {calm:.4} ms this run)", best.0, best.1, best.2);
        if best.0 > 0.05 * scale {
            wrong.push(format!("{id}: {:.4} ms a block on average (budget {:.4}; the calm score {calm:.4} ms this run)", best.0, 0.05 * scale));
        }
        if best.1 > 0.25 * scale {
            wrong.push(format!("{id}: {:.4} ms at the 99th percentile", best.1));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("; "));
}

/// THE NEW INSTRUMENTS FIT THE PER-VOICE BUDGET (as rung E5's do): one voice each of the reese bass, the
/// gliding lead (held), the downlifter and the reversed cymbal (struck every 2 s, held to it), the mean
/// of a 128-frame block, the best of three runs, at most 0.02 ms scaled by the calm score's cost this
/// run. RED first (2026-11-14) with the gate at a tenth: "reese_bass: 0.0038 ms a block for one voice (budget 0.0022; the
/// calm score 0.0398 ms this run)".
#[test]
fn the_new_instruments_fit_the_per_voice_budget() {
    let _g = crate::tests_audio::heavy();
    let lib = shipped();
    let calm = crate::tests_e5::calm_ms(&lib);
    let budget = 0.02 * (calm / 0.037).max(1.0);
    let vowel = [(SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    for id in ["reese_bass", "synth_lead", "downlifter", "reverse_cymbal"] {
        let i = lib.instrument_index(id).unwrap_or_else(|| panic!("the instrument {id}"));
        let insts = vec![lib.instruments[i].clone()];
        let banks = vec![None];
        let mut best = f64::MAX;
        for run in 0..3 {
            let mut pool = VoicePool::new(run);
            let mut times = Vec::new();
            for k in 0..1500usize {
                if k % 750 == 0 {
                    pool.clear();
                    pool.note_on(NoteOn { inst: 0, role: 1, midi: 45.0, a4: 432.0, vel: 1.0, pan: 0.0, gate: 0.0, hold_s: 2.0, tag: k as u32 + 1, level: 1.0 }, &insts, &banks, RATE);
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
        println!("{id}: {best:.4} ms a block for one voice (budget {budget:.4})");
        assert!(best <= budget, "{id}: {best:.4} ms a block for one voice (budget {budget:.4}; the calm score {calm:.4} ms this run)");
    }
}

