// Child of audio/loudness.rs (#[path]): the meter against the standards' own numbers.
//
// The signals are EBU Tech 3341's minimum-requirements test signals (V4, 11/2023, Table 1),
// synthesised here from the table's own descriptions (the EBU's WAV copies of them are not in the
// repo; a sine described by frequency, level and duration is the same signal). Cases 7 and 8 are
// recordings ("authentic programme") and cases 11 and 14 are the live-meter forms of 10 and 13;
// 20 to 23 need a 4x-rate synthesis and an anti-aliasing filter of their own and are not here.
// BS.1770-5's own statements are checked too: its 48 kHz filter tables, the -0.691 that cancels
// the filter's gain at 997 Hz, and a full-scale 997 Hz sine in one channel reading -3.01.
//
// Every test was seen failing on purpose once; the break and what it said are at the test.

use super::*;

/// The rate Tech 3341's signals are made at.
const FS: u32 = 48_000;

/// A stereo sine at `freq` Hz, the same in both channels (Tech 3341: "signal applied in phase to
/// both channels simultaneous"), in `segments` of (seconds, per-channel peak dBFS; None is
/// silence), its phase running on across segments.
fn stereo_tone(rate: u32, freq: f64, segments: &[(f64, Option<f64>)]) -> Vec<f32> {
    let mut out = Vec::new();
    let mut n: u64 = 0;
    for &(seconds, level) in segments {
        let amp = level.map_or(0.0, |db| 10f64.powf(db / 20.0));
        let count = (seconds * rate as f64).round() as u64;
        for _ in 0..count {
            let s = (amp * (std::f64::consts::TAU * freq * n as f64 / rate as f64).sin()) as f32;
            out.push(s);
            out.push(s);
            n += 1;
        }
    }
    out
}

fn assert_near(what: &str, got: Option<f64>, want: f64, tol: f64) {
    let got = got.unwrap_or_else(|| panic!("{what}: no reading"));
    assert!((got - want).abs() <= tol, "{what}: read {got:.3}, the standard says {want:.1} +/- {tol}");
}

/// BS.1770-5's 48 kHz tables come back exactly from the moved filters, and the 44.1 kHz filters
/// have the 48 kHz response (within 0.01 dB from 20 Hz to 10 kHz). RED first (2026-11-06): with
/// the forward transform's middle term written `2 p0 + 2 p2 K^2` the 48 kHz stage 1 came back with
/// a1 = 1.73 instead of -1.69 ("stage 1 at 48 kHz: Biquad { b: [1.535, 2.734, 1.198], a: [1.732,
/// 0.732] }"); and plain bilinear (no pre-warping) was 0.016 dB off at 32 kHz ("32000 Hz, 1500 Hz:
/// 2.0515 dB against 48 kHz's 2.0355 dB").
#[test]
fn the_filters_are_the_standards_tables_at_48k_and_its_response_elsewhere() {
    let [s1, s2] = k_weighting(48_000.0);
    for (got, want, name) in [(s1, STAGE1_48K, "stage 1"), (s2, STAGE2_48K, "stage 2")] {
        let close = got.b.iter().zip(want.b.iter()).chain(got.a.iter().zip(want.a.iter())).all(|(g, w)| (g - w).abs() < 1e-9);
        assert!(close, "{name} at 48 kHz: {got:?}, the table says {want:?}");
    }
    for rate in [44_100.0, 32_000.0, 96_000.0] {
        let moved = k_weighting(rate);
        for f in [20.0, 40.0, 100.0, 500.0, 997.0, 1500.0, 2000.0, 5000.0, 10_000.0] {
            let at48 = STAGE1_48K.gain_db(f, 48_000.0) + STAGE2_48K.gain_db(f, 48_000.0);
            let here = moved[0].gain_db(f, rate) + moved[1].gain_db(f, rate);
            if f < rate / 3.0 {
                assert!((here - at48).abs() < 0.01, "{rate} Hz, {f} Hz: {here:.4} dB against 48 kHz's {at48:.4} dB");
            }
        }
    }
    // BS.1770-5, NOTE 1: "The constant -0.691 in equation (2) cancels out the K-weighting gain for
    // 997 Hz."
    let g = s1.gain_db(997.0, 48_000.0) + s2.gain_db(997.0, 48_000.0);
    assert!((g - 0.691).abs() < 0.005, "K-weighting at 997 Hz: {g:.4} dB");
}

/// BS.1770-5: "If a 0 dB FS, 1 kHz (997 Hz to be exact ...) sine wave is applied to the left,
/// centre, or right channel input, the indicated loudness will equal -3.01 LKFS." RED first: with
/// the window's sum of squares not divided by its length (a sum, not a mean) it read 39.823.
#[test]
fn a_full_scale_997_hz_sine_in_one_channel_reads_minus_3_01() {
    let mut m = LoudnessMeter::new(FS, 2);
    let n = 10 * FS as usize;
    let mut buf = Vec::with_capacity(n * 2);
    for i in 0..n {
        buf.push((std::f64::consts::TAU * 997.0 * i as f64 / FS as f64).sin() as f32);
        buf.push(0.0);
    }
    m.push_interleaved(&buf);
    assert_near("integrated", m.integrated(), -3.01, 0.01);
}

/// Tech 3341 cases 1 and 2: a stereo 1 kHz sine at -23 and -33 dBFS reads -23.0 and -33.0 on M, S
/// and I; and at the other rates a file comes in at, the same (the moved filters). Its section 2.9
/// calibration line too: -18 dBFS reads -18.0. RED first: with the second channel weighted 0,
/// "case 1 at 48000 Hz, M: read -26.004, the standard says -23.0 +/- 0.1".
#[test]
fn tech_3341_cases_1_and_2_steady_tones() {
    for rate in [48_000, 44_100, 22_050, 96_000] {
        for (case, level) in [("case 1", -23.0), ("case 2", -33.0), ("calibration", -18.0)] {
            let mut m = LoudnessMeter::new(rate, 2);
            m.push_interleaved(&stereo_tone(rate, 1000.0, &[(20.0, Some(level))]));
            assert_near(&format!("{case} at {rate} Hz, M"), m.momentary(), level, 0.1);
            assert_near(&format!("{case} at {rate} Hz, S"), m.short_term(), level, 0.1);
            assert_near(&format!("{case} at {rate} Hz, I"), m.integrated(), level, 0.1);
        }
    }
}

/// Tech 3341 cases 3, 4 and 5: the gates. Quiet lead-ins and tails (-36 dBFS: under the relative
/// gate; -72: under the absolute one) leave the integrated reading at the loud part's -23.0, and a
/// -20 dBFS stretch between two -26 stretches reads -23.0 (both in, neither gated). RED first: with
/// the relative gate removed, "case 3: read -24.155, the standard says -23.0 +/- 0.1".
#[test]
fn tech_3341_cases_3_to_5_gating() {
    let cases: [(&str, Vec<(f64, Option<f64>)>); 3] = [
        ("case 3", vec![(10.0, Some(-36.0)), (60.0, Some(-23.0)), (10.0, Some(-36.0))]),
        ("case 4", vec![(10.0, Some(-72.0)), (10.0, Some(-36.0)), (60.0, Some(-23.0)), (10.0, Some(-36.0)), (10.0, Some(-72.0))]),
        ("case 5", vec![(20.0, Some(-26.0)), (20.1, Some(-20.0)), (20.0, Some(-26.0))]),
    ];
    for (case, segments) in cases {
        let mut m = LoudnessMeter::new(FS, 2);
        m.push_interleaved(&stereo_tone(FS, 1000.0, &segments));
        assert_near(case, m.integrated(), -23.0, 0.1);
    }
}

/// Tech 3341 case 6: five channels (L and R at -28 dBFS, C at -24, Ls and Rs at -30) read -23.0,
/// the surrounds weighted 1.41. RED first: with every channel weighted 1.0, "case 6: read -23.389".
#[test]
fn tech_3341_case_6_five_channels() {
    let levels = [-28.0, -28.0, -24.0, -30.0, -30.0];
    let n = 20 * FS as usize;
    let mut buf = Vec::with_capacity(n * 5);
    for i in 0..n {
        let s = (std::f64::consts::TAU * 1000.0 * i as f64 / FS as f64).sin();
        for db in levels {
            buf.push((10f64.powf(db / 20.0) * s) as f32);
        }
    }
    let mut m = LoudnessMeter::new(FS, 5);
    m.push_interleaved(&buf);
    assert_near("case 6", m.integrated(), -23.0, 0.1);
}

/// Tech 3341 cases 9 and 12: a tone switching between -20 and -30 dBFS reads a steady -23.0 on the
/// window it repeats in: Short-term after 3 s (1.34 s and 1.66 s, five times), Momentary after 1 s
/// (0.18 s and 0.22 s, 25 times). Read every 10 ms. RED first: with the Momentary window at 30
/// steps (300 ms), case 12 swung: "case 12, M at 1.00 s: read -21.932".
#[test]
fn tech_3341_cases_9_and_12_steady_windows() {
    let case9: Vec<(f64, Option<f64>)> = (0..5).flat_map(|_| [(1.34, Some(-20.0)), (1.66, Some(-30.0))]).collect();
    let case12: Vec<(f64, Option<f64>)> = (0..25).flat_map(|_| [(0.18, Some(-20.0)), (0.22, Some(-30.0))]).collect();
    for (case, segments, after, short) in [("case 9", case9, 3.0, true), ("case 12", case12, 1.0, false)] {
        let samples = stereo_tone(FS, 1000.0, &segments);
        let mut m = LoudnessMeter::new(FS, 2);
        let step = (FS as f64 * STEP_S) as usize * 2;
        let mut checked = 0;
        for (k, chunk) in samples.chunks(step).enumerate() {
            m.push_interleaved(chunk);
            let t = (k + 1) as f64 * STEP_S;
            if t >= after - 1e-9 {
                let (what, got) = if short { ("S", m.short_term()) } else { ("M", m.momentary()) };
                assert_near(&format!("{case}, {what} at {t:.2} s"), got, -23.0, 0.1);
                checked += 1;
            }
        }
        assert!(checked > 50, "{case}: {checked} readings checked");
    }
}

/// Tech 3341 cases 10 and 13 (the file-based forms): a 3 s tone after i x 150 ms of silence reads a
/// Max Short-term of -23.0, and a 400 ms tone after i x 20 ms reads a Max Momentary of -23.0, for
/// every i to 19. And the same with offsets the 10 ms step does not divide (i x 23 ms + 7 ms), so
/// the pass is not the step lining up with the table's offsets: the best window is at most 5 ms off,
/// -0.055 LU at worst. RED first: with the maximum Momentary taken only every 100 ms (on the gating
/// hop), "case 13, i = 1: read -23.216, the standard says -23.0 +/- 0.1".
#[test]
fn tech_3341_cases_10_and_13_maximum_readings() {
    for i in 0..20 {
        let mut m = LoudnessMeter::new(FS, 2);
        m.push_interleaved(&stereo_tone(FS, 1000.0, &[(i as f64 * 0.15, None), (3.0, Some(-23.0)), (1.0, None)]));
        assert_near(&format!("case 10, i = {i}"), m.max_short_term(), -23.0, 0.1);
        for (form, lead) in [("case 13", i as f64 * 0.020), ("off-step", i as f64 * 0.023 + 0.007)] {
            let mut m = LoudnessMeter::new(FS, 2);
            m.push_interleaved(&stereo_tone(FS, 1000.0, &[(lead, None), (0.4, Some(-23.0)), (1.0, None)]));
            assert_near(&format!("{form}, i = {i}"), m.max_momentary(), -23.0, 0.1);
        }
    }
}

/// Tech 3341 cases 15 to 19: true peak. Sines at fs/4, fs/6 and fs/8 whose peaks fall between the
/// samples read -6.0 dBTP (+0.2 / -0.4), and one at 1.41 of full scale reads +3.0, each tapered
/// with a 10 ms fade in and out. RED first: reading the sample peak alone, case 16 (fs/4 at 45
/// degrees, every sample at 0.354) read -9.03 ("case 16: -9.031 dBTP").
#[test]
fn tech_3341_cases_15_to_19_true_peak() {
    let cases: [(&str, f64, f64, f64, f64); 5] = [("case 15", 4.0, 0.50, 0.0, -6.0), ("case 16", 4.0, 0.50, 45.0, -6.0), ("case 17", 6.0, 0.50, 60.0, -6.0), ("case 18", 8.0, 0.50, 67.5, -6.0), ("case 19", 4.0, 1.41, 45.0, 3.0)];
    for (case, div, amp, phase_deg, want) in cases {
        let n = FS as usize; // one second
        let fade = (0.010 * FS as f64) as usize;
        let mut buf = Vec::with_capacity(n * 2);
        for i in 0..n {
            let taper = (i.min(n - 1 - i) as f64 / fade as f64).min(1.0);
            let s = (taper * amp * (std::f64::consts::TAU * i as f64 / div + phase_deg.to_radians()).sin()) as f32;
            buf.push(s);
            buf.push(s);
        }
        let mut m = LoudnessMeter::new(FS, 2);
        m.push_interleaved(&buf);
        let got = m.true_peak_dbtp();
        assert!(got <= want + 0.2 && got >= want - 0.4, "{case}: {got:.3} dBTP, the standard says {want:.1} +0.2/-0.4");
    }
}

/// A short sound's loudness is its loudest 400 ms, with the rest of its window silence: a 100 ms
/// burst reads 10 log10(0.1 / 0.4) = 6.02 LU under the same tone held, and a long sound reads its
/// integrated loudness. Five seconds of tone then silence is not quite -23.0: the three gating
/// blocks that straddle the tone's end (75%, 50% and 25% tone) are above the relative gate and
/// count, so the mean is 48.5 blocks' worth over 50, 10 log10(0.97) = -0.13 LU (BS.1770 keeps them
/// on purpose: a sound's own ending is part of it). RED first: without `finish`'s padding a 100 ms
/// click had no reading at all ("a 100 ms burst: -inf").
#[test]
fn a_one_shot_is_its_loudest_window_and_a_long_sound_its_integrated_loudness() {
    let click = measure_sound(FS, 2, &stereo_tone(FS, 1000.0, &[(0.1, Some(-23.0))]));
    assert_eq!(click.measure, Measure::MaxMomentary);
    assert!((click.lufs - (-23.0 - 6.02)).abs() < 0.1, "a 100 ms burst: {:.3}", click.lufs);
    let long = measure_sound(FS, 2, &stereo_tone(FS, 1000.0, &[(5.0, Some(-23.0)), (1.0, None)]));
    assert_eq!(long.measure, Measure::Integrated);
    let want = -23.0 + 10.0 * (48.5f64 / 50.0).log10();
    assert!((long.lufs - want).abs() < 0.02, "five seconds then silence: {:.3}, want {want:.3}", long.lufs);
    assert!((long.true_peak_dbtp + 23.0).abs() < 0.05, "its peak: {:.3}", long.true_peak_dbtp);
    let silent = measure_sound(FS, 2, &vec![0.0; FS as usize]);
    assert_eq!(silent.lufs, f64::NEG_INFINITY, "silence has no loudness");
}
