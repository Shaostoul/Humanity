//! `just music-render <mood> <style> <seconds> [options]`: render the REAL music engine offline to a
//! WAV and report how it measures (docs/design/music.md, section 16). Permanent dev tooling.
//!
//! The WAV is written where `--out` says, else into the system temp folder; never into the repo.
//! The report: BS.1770 integrated loudness and true peak (humanity_core::audio::loudness, the app's
//! one meter), the loudness range and the largest one-second change of short-term loudness, the
//! tuning (A4 and the drone's root found in the mix), and the audio thread's cost per 128-frame block
//! against the budget (0.05 ms mean, 0.25 ms worst).
//!
//! Options:
//!   --a4 432|440          the tuning (432 by default)
//!   --seed N              the composer's seed (20261106 by default)
//!   --out PATH            where the WAV goes
//!   --blend STYLE:T       blend toward a second style by T (0..1)
//!   --intensity V         a steady intensity (0.3 by default)
//!   --ramp "S:V,S:V,..."  intensity over time: seconds:value points, linear between them
//!   --hum                 mix the ship's hum under it at its default level (Ship hum level 0.35)
//!   --breach              the hum's alarm follows the intensity above 0.35 (implies --hum)
//!   --stems               also print each role's loudness against the whole (calm moods)
//!   --rate HZ             the sample rate (48000)
//!   --fade S              fade the file out over its last S seconds (6 by default; 0 for none)

use std::path::PathBuf;

use humanity_core::audio::loudness::LoudnessMeter;
use humanity_music::bed::{hum_place, BedContext, BedEngine};
use humanity_music::mood::RoleKind;
use humanity_music::render::write_wav;
use humanity_music::score::{MusicContext, ScoreEngine};
use humanity_music::style::Style;
use humanity_music::{MusicLibrary, Resolved, SampleLibrary};

fn arg<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).map(|s| s.as_str())
}

fn ramp_at(points: &[(f64, f32)], t: f64, steady: f32) -> f32 {
    if points.is_empty() {
        return steady;
    }
    if t <= points[0].0 {
        return points[0].1;
    }
    for w in points.windows(2) {
        let ((t0, v0), (t1, v1)) = (w[0], w[1]);
        if t >= t0 && t <= t1 {
            let x = ((t - t0) / (t1 - t0).max(1e-9)) as f32;
            return v0 + (v1 - v0) * x;
        }
    }
    points[points.len() - 1].1
}

struct Measured {
    integrated: f64,
    true_peak: f64,
    range: f64,
    max_step: f64,
}

/// Loudness and true peak of the whole; the range and the steps of short-term loudness between
/// `skip_s` from the start and `end_s` before the end (the fades in and out are not the music's).
fn measure(frames: &[[f32; 2]], rate: u32, skip_s: f64) -> Measured {
    measure_between(frames, rate, skip_s, 0.0)
}

fn measure_between(frames: &[[f32; 2]], rate: u32, skip_s: f64, end_s: f64) -> Measured {
    let last_s = frames.len() as f64 / rate as f64 - end_s;
    let mut m = LoudnessMeter::new(rate, 2);
    let mut st = Vec::new();
    let per_s = rate as usize;
    for (i, f) in frames.iter().enumerate() {
        m.push_frame(f);
        if i % per_s == per_s - 1 && i as f64 / rate as f64 >= skip_s && i as f64 / rate as f64 <= last_s {
            if let Some(s) = m.short_term() {
                st.push(s);
            }
        }
    }
    m.finish();
    // Loudness range (EBU Tech 3342): short-term values gated at -70 LUFS and 20 LU under their
    // mean, the spread between the 10th and the 95th percentile.
    let abs: Vec<f64> = st.iter().copied().filter(|v| *v > -70.0).collect();
    let range = if abs.len() > 3 {
        let mean = 10.0 * (abs.iter().map(|v| 10f64.powf(v / 10.0)).sum::<f64>() / abs.len() as f64).log10();
        let mut g: Vec<f64> = abs.iter().copied().filter(|v| *v > mean - 20.0).collect();
        g.sort_by(|a, b| a.total_cmp(b));
        let p = |q: f64| g[((g.len() - 1) as f64 * q).round() as usize];
        p(0.95) - p(0.10)
    } else {
        0.0
    };
    let max_step = st.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f64::max);
    Measured { integrated: m.integrated().unwrap_or(f64::NEG_INFINITY), true_peak: m.true_peak_dbtp(), range, max_step }
}

/// The strongest frequency between lo and hi Hz in a stretch of the mix (a Goertzel scan).
fn goertzel_peak(frames: &[[f32; 2]], rate: f64, t0: f64, t1: f64, lo: f64, hi: f64, step: f64) -> f64 {
    let s0 = (t0 * rate) as usize;
    let s1 = ((t1 * rate) as usize).min(frames.len());
    let n = s1.saturating_sub(s0);
    let x: Vec<f64> = (0..n).map(|i| (frames[s0 + i][0] + frames[s0 + i][1]) as f64 * (0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / n as f64).cos())).collect();
    let (mut best, mut best_p) = (lo, -1.0);
    let mut f = lo;
    while f <= hi {
        let cw = 2.0 * (2.0 * std::f64::consts::PI * f / rate).cos();
        let (mut q1, mut q2) = (0.0, 0.0);
        for &v in &x {
            let q0 = cw * q1 - q2 + v;
            q2 = q1;
            q1 = q0;
        }
        let p = q1 * q1 + q2 * q2 - cw * q1 * q2;
        if p > best_p {
            best_p = p;
            best = f;
        }
        f += step;
    }
    best
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let pos: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    // Positional: mood, style, seconds (options' values are skipped by looking them up by name).
    let opt_vals: Vec<&str> = ["--a4", "--seed", "--out", "--blend", "--intensity", "--ramp", "--rate", "--fade"].iter().filter_map(|o| arg(&args, o)).collect();
    let pos: Vec<&str> = pos.into_iter().map(|s| s.as_str()).filter(|s| !opt_vals.contains(s)).collect();
    if pos.len() < 3 {
        eprintln!("usage: music-render <mood> <style> <seconds> [--a4 432] [--seed N] [--out PATH] [--blend STYLE:T] [--intensity V] [--ramp S:V,...] [--hum] [--breach] [--stems]");
        std::process::exit(2);
    }
    let (mood_id, style_id) = (pos[0], pos[1]);
    let seconds: f64 = pos[2].parse().expect("seconds");
    let a4: f32 = arg(&args, "--a4").map(|v| v.parse().expect("--a4")).unwrap_or(432.0);
    let seed: u64 = arg(&args, "--seed").map(|v| v.parse().expect("--seed")).unwrap_or(20261106);
    let rate: f32 = arg(&args, "--rate").map(|v| v.parse().expect("--rate")).unwrap_or(48000.0);
    let steady: f32 = arg(&args, "--intensity").map(|v| v.parse().expect("--intensity")).unwrap_or(0.3);
    let ramp: Vec<(f64, f32)> = arg(&args, "--ramp")
        .map(|r| r.split(',').filter_map(|p| p.split_once(':')).map(|(a, b)| (a.trim().parse().expect("ramp time"), b.trim().parse().expect("ramp value"))).collect())
        .unwrap_or_default();
    let breach = args.iter().any(|a| a == "--breach");
    let hum = breach || args.iter().any(|a| a == "--hum");
    let data = PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data");
    let lib = MusicLibrary::load(&data).unwrap_or_else(|e| panic!("the music data: {e}"));
    let mood = lib.mood_index(mood_id).unwrap_or_else(|| panic!("no mood {mood_id:?}; moods: {:?}", lib.moods.iter().map(|m| &m.id).collect::<Vec<_>>()));
    let style = lib.style_index(style_id).unwrap_or_else(|| panic!("no style {style_id:?}; styles: {:?}", lib.styles.iter().map(|m| &m.id).collect::<Vec<_>>()));
    let (style_b, blend) = match arg(&args, "--blend").and_then(|b| b.split_once(':')) {
        Some((s, t)) => (lib.style_index(s).unwrap_or_else(|| panic!("no style {s:?}")), t.parse::<f32>().expect("blend")),
        None => (style, 0.0),
    };
    let out = arg(&args, "--out").map(PathBuf::from).unwrap_or_else(|| std::env::temp_dir().join(format!("humanityos-music-{mood_id}-{style_id}-{}.wav", a4 as u32)));
    let ctx_at = |t: f64| MusicContext { gain: 1.0, a4_hz: a4, mood: mood as u16, style_a: style as u16, style_b: style_b as u16, blend, intensity: ramp_at(&ramp, t, steady), duck_db: 0.0 };

    // (frames, mean us, worst us, limiter samples, the 99.9th percentile block us)
    let render = |lib: &MusicLibrary, style: usize| -> (Vec<[f32; 2]>, f64, f64, u64, f64) {
        let res = Resolved::new(lib, &SampleLibrary::new()).expect("resolve");
        let mut ctx = ctx_at(0.0);
        ctx.style_a = style as u16;
        let mut eng = ScoreEngine::new(res, rate, seed, ctx);
        let total = (seconds * rate as f64) as usize;
        let mut frames = vec![[0.0f32; 2]; total];
        let (mut sum, mut worst, mut blocks) = (0.0, 0.0f64, 0u64);
        let mut times = Vec::with_capacity(total / 128 + 1);
        let mut i = 0;
        while i < total {
            let n = 128.min(total - i);
            let mut c = ctx_at(i as f64 / rate as f64);
            c.style_a = style as u16;
            if blend == 0.0 {
                c.style_b = style as u16;
            }
            eng.set_context(c);
            let t0 = std::time::Instant::now();
            eng.render(&mut frames[i..i + n]);
            let us = t0.elapsed().as_secs_f64() * 1e6;
            sum += us;
            worst = worst.max(us);
            blocks += 1;
            times.push(us);
            i += n;
        }
        times.sort_by(|a, b| a.total_cmp(b));
        let p999 = times.get(((times.len() as f64 - 1.0) * 0.999) as usize).copied().unwrap_or(0.0);
        (frames, sum / blocks as f64, worst, eng.status().limited, p999)
    };

    let t_start = std::time::Instant::now();
    let (mut music, mean_us, worst_us, limited, p999_us) = render(&lib, style);
    let wall = t_start.elapsed().as_secs_f64();
    let music_m = measure(&music, rate as u32, 3.0);
    let mut hum_report = String::new();
    if hum {
        let mut bed = BedEngine::with_alarm(rate, seed ^ 0xbed, BedContext { a4_hz: a4, gain: 0.35, ..Default::default() }, lib.hum.alarm);
        let place = hum_place(&lib.hum, (40.0, 1.7, 60.0), 0, None);
        let total = music.len();
        let mut h = vec![[0.0f32; 2]; total];
        let mut i = 0;
        let (mut hsum, mut hworst, mut hb) = (0.0, 0.0f64, 0u64);
        while i < total {
            let n = 128.min(total - i);
            let t = i as f64 / rate as f64;
            let alarm = if breach { ((ramp_at(&ramp, t, steady) - 0.35) / 0.4).clamp(0.0, 1.0) } else { 0.0 };
            bed.set_context(BedContext { aboard: true, place, key_pc: 2, a4_hz: a4, gain: 0.35, tone_hz: 528.0, tone_gain: 0.0, alarm });
            let t0 = std::time::Instant::now();
            bed.render(&mut h[i..i + n]);
            let us = t0.elapsed().as_secs_f64() * 1e6;
            hsum += us;
            hworst = hworst.max(us);
            hb += 1;
            i += n;
        }
        let hm = measure(&h, rate as u32, 3.0);
        hum_report = format!(
            "  the ship's hum alone (level 0.35{}): {:.1} LUFS ({:+.1} LU against the music), true peak {:.2} dBTP; cost {:.4} ms mean, {:.4} ms worst per block\n",
            if breach { ", alarm with the intensity" } else { "" },
            hm.integrated,
            hm.integrated - music_m.integrated,
            hm.true_peak,
            hsum / hb as f64 / 1000.0,
            hworst / 1000.0
        );
        for (m, x) in music.iter_mut().zip(h.iter()) {
            m[0] += x[0];
            m[1] += x[1];
        }
    }
    // A listening file ends with a fade (the game's own score never ends; a WAV does): the last
    // `--fade` seconds (6 by default, 0 for none), a raised cosine, measured as written.
    let fade_s: f64 = arg(&args, "--fade").map(|v| v.parse().expect("--fade")).unwrap_or(6.0f64.min(seconds / 4.0));
    let fade_n = (fade_s * rate as f64) as usize;
    let len = music.len();
    for (k, f) in music.iter_mut().enumerate().skip(len.saturating_sub(fade_n)) {
        let x = (k + fade_n - len) as f32 / fade_n.max(1) as f32;
        let g = 0.5 + 0.5 * (std::f32::consts::PI * x).cos();
        f[0] *= g;
        f[1] *= g;
    }
    let mix = measure_between(&music, rate as u32, 3.0, fade_s);
    let clipped = write_wav(&out, &music, rate as u32).expect("write the WAV");
    let block_ms = 128.0 / rate as f64 * 1000.0;
    println!("music-render: mood {mood_id}, style {style_id}{}, {seconds} s at A4 = {a4} Hz, seed {seed}, {} Hz", if blend > 0.0 { format!(" blended {blend:.2} toward {}", lib.styles[style_b].id) } else { String::new() }, rate);
    println!("  {}", out.display());
    println!("  integrated loudness {:.1} LUFS, true peak {:.2} dBTP, loudness range {:.1} LU, largest one-second change of short-term loudness {:.2} LU, clipped samples {clipped}, limiter samples {limited}", mix.integrated, mix.true_peak, mix.range, mix.max_step);
    if hum {
        println!("  (the music alone: {:.1} LUFS, true peak {:.2} dBTP)", music_m.integrated, music_m.true_peak);
        print!("{hum_report}");
    }
    println!(
        "  audio thread, score: {:.4} ms mean, {:.4} ms at the 99.9th percentile and {:.4} ms worst per 128-frame block ({:.2}%, {:.1}% and {:.1}% of the block's {:.3} ms; budget 0.05 ms mean, 0.25 ms worst; the worst includes the OS preempting this offline loop); rendered {seconds} s in {wall:.1} s",
        mean_us / 1000.0,
        p999_us / 1000.0,
        worst_us / 1000.0,
        mean_us / 10.0 / block_ms,
        p999_us / 10.0 / block_ms,
        worst_us / 10.0 / block_ms,
        block_ms
    );
    let d2 = humanity_music::note_hz(38.0, a4) as f64;
    let found = goertzel_peak(&music, rate as f64, (seconds * 0.3).min(20.0), (seconds * 0.3).min(20.0) + 20.0f64.min(seconds * 0.6), d2 - 3.0, d2 + 3.0, 0.01);
    println!("  tuning: A4 = {:.3} Hz by note_hz; the drone's D2 found in the mix at {found:.2} Hz (expected {d2:.2})", humanity_music::note_hz(69.0, a4));
    if args.iter().any(|a| a == "--stems") {
        for role in RoleKind::ALL {
            let mut l2 = lib.clone();
            let id = format!("stem-{role:?}");
            l2.styles.push(Style {
                id: id.clone(),
                roles_off: RoleKind::ALL.iter().copied().filter(|r| *r != role).collect(),
                ..ron::from_str::<Style>(&format!("(id: \"{id}\")")).expect("an empty style")
            });
            let si = l2.style_index(&id).unwrap();
            let (stem, mean, _, _, _) = render(&l2, si);
            let m = measure(&stem, rate as u32, 3.0);
            if m.integrated.is_finite() {
                println!("  stem {role:?}: {:+.1} LU against all the music ({:.1} LUFS); {:.4} ms a block with the reverb", m.integrated - music_m.integrated, m.integrated, mean / 1000.0);
            }
        }
    }
}
