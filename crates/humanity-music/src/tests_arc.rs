//! SEEKABLE COMPOSITION AND THE LONG ARC (docs/design/music.md section 7h; the operator, 2026-11-20:
//! "couldn't we just say, here's how the song will look X minutes out? Then isn't a 'new song' just a
//! new seed? Would that help us have more complex songs?"). Every test measures what the composer or
//! the score produces (the notes struck, the rendered audio, the time taken), never what it was told,
//! and each was seen failing first; the red is in its comment.

use std::collections::HashMap;

use crate::arc::StageKind;
use crate::composer::{Composer, Silent, Sink, Steer};
use crate::resolved::Resolved;
use crate::tests::{shipped, steer};
use crate::voice::NoteOn;
use crate::SampleLibrary;

const RATE: f32 = 48000.0;

/// Everything a composer strikes and lets go, with the sample it happened on.
#[derive(Default)]
struct Tape {
    pos: u64,
    /// (sample, tag, instrument, role, pitch, velocity, pan, hold), the floats as bits.
    ons: Vec<(u64, u32, u16, u8, u32, u32, u32, u32)>,
    /// (sample, role, pitch): the note let go, found by its tag.
    offs: Vec<(u64, u8, u32)>,
    live: HashMap<u32, (u8, u32)>,
}

impl Sink for Tape {
    fn note_on(&mut self, n: NoteOn) {
        self.ons.push((self.pos, n.tag, n.inst, n.role, n.midi.to_bits(), n.vel.to_bits(), n.pan.to_bits(), n.hold_s.to_bits()));
        self.live.insert(n.tag, (n.role, n.midi.to_bits()));
    }
    fn note_off(&mut self, tag: u32) {
        if let Some((role, midi)) = self.live.remove(&tag) {
            self.offs.push((self.pos, role, midi));
        }
    }
}

/// Play `c` into `tape` until sample `until`.
fn play(c: &mut Composer, res: &Resolved, tape: &mut Tape, until: u64) {
    while c.pos() < until {
        tape.pos = c.pos();
        c.process(res, tape);
        let n = c.until_next().min((until - c.pos()) as usize).max(1);
        c.advance(n);
    }
}

/// Bar `bar`'s first and last-plus-one samples in a planned piece (the composer computes a beat's
/// start from its number, never by summing).
fn bar_samples(c: &Composer, bar: u64) -> (u64, u64) {
    let bpb = c.beats_per_bar().max(1) as u64;
    let beat = RATE as f64 * 60.0 / c.tempo() as f64;
    (((bar * bpb) as f64 * beat).ceil() as u64, (((bar + 1) * bpb) as f64 * beat).ceil() as u64)
}

fn steer_len(lib: &crate::MusicLibrary, mood: &str, genre: &str, piece_s: f32) -> Steer {
    Steer { piece_s, ..steer(lib, mood, genre) }
}

/// Bar `bar` computed alone: a fresh composer seeks to it (into a tape, so the notes held from before
/// can be named when let go), then plays it. Returns the bar's notes struck and let go.
fn bar_alone(res: &Resolved, seed: u64, st: Steer, bar: u64) -> (Vec<(u64, u32, u16, u8, u32, u32, u32, u32)>, Vec<(u64, u8, u32)>, u64) {
    let mut c = Composer::new(RATE, seed, st);
    let mut tape = Tape::default();
    c.seek_to_bar(res, bar, &mut tape);
    let (a, b) = bar_samples(&c, bar);
    assert_eq!(c.pos(), a, "the seek stands at bar {bar}'s first sample");
    play(&mut c, res, &mut tape, b);
    let ons = tape.ons.into_iter().filter(|x| x.0 >= a && x.0 < b).collect();
    let offs = tape.offs.into_iter().filter(|x| x.0 >= a && x.0 < b).collect();
    (ons, offs, a)
}

/// A BAR COMPUTED ALONE EQUALS THE BAR REACHED BY PLAYING FROM THE START (music.md section 7h): in four
/// genres (two with arcs, a structure's arc, and a genre with no arc playing its form over and over),
/// bars from the first minutes to twenty minutes in, and one bar two days in, each sought by a fresh
/// composer and played, strike and let go the same notes on the same samples, with the same tags,
/// velocities, pans and holds, as the same seed playing from its start. RED first (2026-11-21) with the
/// reseed at each bar taken out (the composer's random numbers carried on from wherever it stood): "electro_swing
/// bar 37: 39 notes alone, 38 from the start, first difference at note 0: Some((3330000, 151556, 25, 255,
/// 1107558400, 1062338321, 0, 0)) against Some((3330000, 151558, 23, 0, 1113325568, 1058003856, 0, 1041235968))".
#[test]
fn a_bar_computed_alone_equals_the_bar_played_from_the_start() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let cases: [(&str, &str, f32, u64, &[u64]); 4] = [
        ("mothership_calm", "electro_swing", 240.0, 11, &[0, 5, 37, 130, 640]),
        ("mothership_calm", "mothership_calm", 600.0, 23, &[3, 16, 41, 300]),
        ("mothership_calm", "deep_dubstep", 600.0, 7, &[9, 24, 88, 610]),
        ("garden_morning", "lofi", 0.0, 5, &[2, 33, 200, 560]),
    ];
    for (mood, genre, piece_s, seed, bars) in cases {
        let st = steer_len(&lib, mood, genre, piece_s);
        // One run from the start, kept to the last bar asked for.
        let mut c = Composer::new(RATE, seed, st);
        let mut tape = Tape::default();
        let last = *bars.iter().max().unwrap();
        // The composer's tempo is known after its first phrase: play a sample, then to the end.
        play(&mut c, &res, &mut tape, 1);
        let (_, end) = bar_samples(&c, last);
        play(&mut c, &res, &mut tape, end);
        for &bar in bars {
            let (a, b) = bar_samples(&c, bar);
            let from_start: Vec<_> = tape.ons.iter().copied().filter(|x| x.0 >= a && x.0 < b).collect();
            let offs_start: Vec<_> = tape.offs.iter().copied().filter(|x| x.0 >= a && x.0 < b).collect();
            let (alone, offs_alone, at) = bar_alone(&res, seed, st, bar);
            assert_eq!(at, a, "{genre} bar {bar} starts on the same sample");
            assert!(!from_start.is_empty() || bar == 0, "{genre} bar {bar}: no notes struck at all");
            if alone != from_start {
                let k = alone.iter().zip(from_start.iter()).position(|(x, y)| x != y).unwrap_or(alone.len().min(from_start.len()));
                panic!("{genre} bar {bar}: {} notes alone, {} from the start, first difference at note {k}: {:?} against {:?}", alone.len(), from_start.len(), alone.get(k), from_start.get(k));
            }
            assert_eq!(offs_alone, offs_start, "{genre} bar {bar}: the notes let go differ");
        }
    }
    // Two days in (electro swing, an endless piece going round its arc every 8 minutes).
    let st = steer_len(&lib, "mothership_calm", "electro_swing", 480.0);
    let mut c = Composer::new(RATE, 3, st);
    let mut tape = Tape::default();
    play(&mut c, &res, &mut tape, 1);
    let bar = (2.0 * 86400.0 / (c.beats_per_bar() as f64 * 60.0 / c.tempo() as f64)) as u64;
    let (a, b) = bar_samples(&c, bar);
    // From the start, keeping only the last bar's notes (two days of notes would be gigabytes).
    let mut keep = Tape::default();
    while c.pos() < b {
        keep.pos = c.pos();
        let near = c.pos() + 48_000 * 60 >= a;
        if near {
            c.process(&res, &mut keep);
        } else {
            c.process(&res, &mut Silent);
        }
        let n = c.until_next().min((b - c.pos()) as usize).max(1);
        c.advance(n);
    }
    let from_start: Vec<_> = keep.ons.iter().copied().filter(|x| x.0 >= a && x.0 < b).collect();
    let (alone, _, _) = bar_alone(&res, 3, st, bar);
    assert!(!from_start.is_empty(), "two days in: bar {bar} strikes nothing");
    assert_eq!(alone, from_start, "two days in (bar {bar}): computed alone it differs from the bar played from the start");
}

/// THE SAME SEED IS THE SAME PIECE: ON TWO RUNS AND ACROSS A SAVE AND A LOAD; ANOTHER SEED, ANOTHER
/// PLAN. A piece liked while it plays (its seed and its tune saved as a favourite, written to a file
/// and read back) plans the same arc and writes down the same notes as the piece that played (the
/// sheet's hash over every note and its sample); a second run of the same seed too; a different seed
/// plans another piece (another climax bar, other section lengths or letters). RED first (2026-11-21),
/// twice: with a favourite's context not carrying the piece's length (piece_s 0, so it plays its form over
/// and over with no arc): "the loaded favourite plans another arc" (every loaded phrase's stage None at
/// energy 1, against Intro to Coda); and with the plan's seed and the form's choice both ignoring the
/// piece's seed: "seeds 77 and 78 plan the same piece". (The plan's seed alone ignoring the piece did NOT
/// fail it: the form, chosen from the piece's seed, still differs; so both had to go.)
#[test]
fn the_same_seed_is_the_same_piece_across_runs_and_a_save_and_a_load() {
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let genre = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    // The piece as a chosen piece plays it: endless, round its arc at the library's length.
    let ctx = crate::MusicContext { mood, genre_a: genre, genre_b: genre, piece_s: lib.endless_minutes * 60.0, ..Default::default() };
    let plan_of = |ctx: &crate::MusicContext, seed: u64| {
        let s = crate::sheet::skeleton(&res, seed, ctx, RATE, 600.0).expect("planned");
        s.plan.iter().map(|p| (p.at_s.to_bits(), p.letter, p.stage, p.energy.to_bits(), p.climax)).collect::<Vec<_>>()
    };
    let a = crate::sheet::transcribe(&res, 77, &ctx, RATE, 90.0);
    let b = crate::sheet::transcribe(&res, 77, &ctx, RATE, 90.0);
    assert_eq!(a.hash, b.hash, "two runs of one seed");
    assert_eq!(plan_of(&ctx, 77), plan_of(&ctx, 77));
    // Saved and loaded.
    let dir = std::env::temp_dir().join(format!("hos-arc-fav-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("favourites.json");
    let mut favs = crate::favourite::Favourites::default();
    favs.favourites.push(crate::favourite::Favourite { id: 1, name: "arc".into(), seed: 77, genre: "electro_swing".into(), mood: "mothership_calm".into(), tune: Default::default() });
    favs.save(&path).unwrap();
    let back = crate::favourite::Favourites::load(&path).unwrap();
    let loaded = back.favourites[0].context(&lib, ctx.a4_hz).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let (pa, pl) = (plan_of(&ctx, 77), plan_of(&loaded, back.favourites[0].seed));
    assert_eq!(pl.len(), pa.len(), "the loaded favourite plans {} phrases, the piece that played {}", pl.len(), pa.len());
    assert_eq!(pl, pa, "the loaded favourite plans another arc");
    let c = crate::sheet::transcribe(&res, back.favourites[0].seed, &loaded, RATE, 90.0);
    assert_eq!(c.hash, a.hash, "the loaded favourite writes down other notes");
    // Another seed, another plan.
    let other = plan_of(&ctx, 78);
    assert_ne!(other, pa, "seeds 77 and 78 plan the same piece");
}

/// The audio's level and its onsets per second in each of `spans` (seconds).
fn level_and_density(frames: &[[f32; 2]], spans: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let onsets = crate::tests_audio::onsets(frames, RATE, false);
    spans
        .iter()
        .map(|&(a, b)| {
            let (i, j) = ((a * RATE as f64) as usize, ((b * RATE as f64) as usize).min(frames.len()));
            let e: f64 = frames[i..j].iter().map(|f| (f[0] as f64).powi(2) + (f[1] as f64).powi(2)).sum::<f64>() / (j - i).max(1) as f64;
            let n = onsets.iter().filter(|&&t| t >= a && t < b).count() as f64;
            (10.0 * e.max(1e-12).log10(), n / (b - a).max(1e-6))
        })
        .collect()
}

/// THE PLANNED CLIMAX IS WHERE THE PLAN SAYS, MEASURED IN THE RENDERED AUDIO: a 5-minute electro swing
/// piece is rendered whole; of its stages (intro, theme, development, build, climax, return, coda, as
/// the plan lays them out), the climax is the loudest and plays more notes a second than the thinned
/// intro and coda, and the loudest 8 seconds of the piece
/// fall inside the climax. RED first (2026-11-21) with the stages not applied (apply_stage left every role,
/// the drums and the densities as the genre has them) and every energy's velocity flat at 1: "the loudest
/// stage is Build (-16.3 dB), not the climax (-16.8 dB)".
#[test]
fn the_planned_climax_is_where_the_plan_says_in_the_rendered_audio() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let genre = lib.genre_index("electro_swing").unwrap() as u16;
    let mood = lib.mood_index("mothership_calm").unwrap() as u16;
    let ctx = crate::MusicContext { mood, genre_a: genre, genre_b: genre, piece_s: 300.0, ..Default::default() };
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let sk = crate::sheet::skeleton(&res, 21, &ctx, RATE, 1e9).expect("planned");
    // One round of the arc: the stages' spans in seconds.
    let round_end = sk.plan.iter().take_while(|p| p.at_s < 1e5).scan(None, |seen: &mut Option<f64>, p| {
        if seen.is_some() && p.stage == Some(StageKind::Intro) {
            return None;
        }
        *seen = Some(p.at_s);
        Some(p.end_s)
    }).last().unwrap();
    let mut stages: Vec<(StageKind, f64, f64)> = Vec::new();
    for p in sk.plan.iter().take_while(|p| p.end_s <= round_end + 1e-6) {
        let k = p.stage.unwrap();
        match stages.last_mut() {
            Some(s) if s.0 == k => s.2 = p.end_s,
            _ => stages.push((k, p.at_s, p.end_s)),
        }
    }
    let kinds: Vec<StageKind> = stages.iter().map(|s| s.0).collect();
    assert!(kinds.contains(&StageKind::Climax) && kinds.contains(&StageKind::Intro) && kinds.contains(&StageKind::Coda), "the plan's stages: {kinds:?}");
    let mut s = crate::ScoreEngine::new(res, RATE, 21, ctx);
    let r = crate::render::render_score(&mut s, RATE, round_end);
    let spans: Vec<(f64, f64)> = stages.iter().map(|s| (s.1 + 1.0, s.2)).collect();
    let m = level_and_density(&r.frames, &spans);
    for (k, (s, (db, d))) in stages.iter().zip(m.iter()).enumerate() {
        println!("stage {k} {:?} {:.1}-{:.1} s: {db:.1} dB, {d:.2} onsets a second", s.0, s.1, s.2);
    }
    let ci = kinds.iter().position(|k| *k == StageKind::Climax).unwrap();
    let loudest = (0..m.len()).max_by(|&a, &b| m[a].0.total_cmp(&m[b].0)).unwrap();
    assert_eq!(loudest, ci, "the loudest stage is {:?} ({:.1} dB), not the climax ({:.1} dB)", kinds[loudest], m[loudest].0, m[ci].0);
    for (k, &(_, d)) in m.iter().enumerate() {
        if matches!(kinds[k], StageKind::Intro | StageKind::Coda) {
            assert!(m[ci].1 > d, "the climax plays {:.2} onsets a second, the {:?} {d:.2}", m[ci].1, kinds[k]);
        }
    }
    // The loudest 8 seconds (stepped by 1 s) fall in the climax.
    let windows: Vec<(f64, f64)> = (0..((round_end - 8.0) as usize)).map(|t| (t as f64, t as f64 + 8.0)).collect();
    let w = level_and_density(&r.frames, &windows);
    let best = (0..w.len()).max_by(|&a, &b| w[a].0.total_cmp(&w[b].0)).unwrap();
    let (c0, c1) = (stages[ci].1, stages[ci].2);
    assert!(windows[best].0 >= c0 - 1.0 && windows[best].1 <= c1 + 1.0, "the loudest 8 s are {:.0}-{:.0} s, the climax {c0:.1}-{c1:.1} s", windows[best].0, windows[best].1);
}

/// A phrase's riffs, stabs and chords as heard: (pitch classes counted over the pads, the choir or
/// stabs and the riffs; the riffs' onsets in quarter beats from its start: the tune's rhythm).
fn phrase_profile(tape: &Tape, a: u64, b: u64, beat: f64) -> ([f64; 12], std::collections::HashSet<i64>) {
    let mut pcs = [0f64; 12];
    let mut on = std::collections::HashSet::new();
    for x in tape.ons.iter().filter(|x| x.0 >= a && x.0 < b && [0u8, 3, 4, 5].contains(&x.3)) {
        let midi = f32::from_bits(x.4).round() as i64;
        pcs[midi.rem_euclid(12) as usize] += 1.0;
        if x.3 >= 4 {
            on.insert((((x.0 - a) as f64 / beat) * 4.0).round() as i64);
        }
    }
    (pcs, on)
}

fn cosine(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let n = |v: &[f64; 12]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    dot / (n(a) * n(b)).max(1e-9)
}

fn jaccard(a: &std::collections::HashSet<i64>, b: &std::collections::HashSet<i64>) -> f64 {
    let u = a.union(b).count();
    if u == 0 {
        return 1.0;
    }
    a.intersection(b).count() as f64 / u as f64
}

/// A 20-MINUTE PIECE HAS ITS THEME RETURN AFTER ITS CLIMAX, MEASURED: electro swing over 20 minutes;
/// the notes the composer strikes in the return's first phrase (after the climax) against the opening
/// theme's first phrase: their pitch classes (the pads, the stabs and the riffs) and their riffs' and
/// stabs' rhythm match (cosine at least 0.95, onsets at least 0.6 alike), and match the opening more
/// closely than any phrase of the development does. RED first (2026-11-21) with the song arc's return
/// stage not playing its section's tune (data/music/arcs/song.ron, the Return's tune: false): "seed 5: the
/// return is 0.22 alike in rhythm to the opening theme" (its pitch classes still 0.96 alike).
#[test]
fn a_twenty_minute_piece_has_its_theme_return_after_its_climax() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    for seed in [5u64, 9] {
        let st = steer_len(&lib, "mothership_calm", "electro_swing", 1200.0);
        let mut c = Composer::new(RATE, seed, st);
        let mut tape = Tape::default();
        play(&mut c, &res, &mut tape, 1);
        let plan = *c.plan();
        let (bpb, beat_len) = (c.beats_per_bar().max(1) as u64, RATE as f64 * 60.0 / c.tempo() as f64);
        let bar_of = move |b: u64| ((b * bpb) as f64 * beat_len).ceil() as u64;
        let first_of = |k: StageKind| (0..plan.n).find(|&i| plan.phrases[i].kind == Some(k)).unwrap();
        let (theme, climax, ret) = (first_of(StageKind::Theme), first_of(StageKind::Climax), first_of(StageKind::Return));
        assert!(ret > climax, "seed {seed}: the return (phrase {ret}) comes after the climax (phrase {climax})");
        let span = |i: usize| (bar_of(plan.phrases[i].start_bar as u64), bar_of((plan.phrases[i].start_bar + plan.phrases[i].bars) as u64));
        let end = span(plan.n - 1).1;
        play(&mut c, &res, &mut tape, end);
        assert!(end as f64 / RATE as f64 > 18.0 * 60.0, "seed {seed}: the piece lasts {:.1} min", end as f64 / RATE as f64 / 60.0);
        let beat = RATE as f64 * 60.0 / c.tempo() as f64;
        let prof = |i: usize| {
            let (a, b) = span(i);
            phrase_profile(&tape, a, b, beat)
        };
        let (open_pc, open_on) = prof(theme);
        let (ret_pc, ret_on) = prof(ret);
        let (pc_sim, on_sim) = (cosine(&open_pc, &ret_pc), jaccard(&open_on, &ret_on));
        println!("seed {seed}: the return against the opening: pitch classes {pc_sim:.2}, rhythm {on_sim:.2}");
        assert!(pc_sim >= 0.95, "seed {seed}: the return is {pc_sim:.2} alike in pitch classes to the opening theme");
        assert!(on_sim >= 0.6, "seed {seed}: the return is {on_sim:.2} alike in rhythm to the opening theme");
        for i in (0..plan.n).filter(|&i| plan.phrases[i].kind == Some(StageKind::Develop)) {
            let (pc, on) = prof(i);
            let (p, o) = (cosine(&open_pc, &pc), jaccard(&open_on, &on));
            assert!(p + o < pc_sim + on_sim, "seed {seed}: development phrase {i} is as alike to the opening ({p:.2}, {o:.2}) as the return ({pc_sim:.2}, {on_sim:.2})");
        }
    }
}

/// The fastest of `n` timings of `f`, microseconds.
fn fastest(n: usize, mut f: impl FnMut()) -> f64 {
    (0..n)
        .map(|_| {
            let t = std::time::Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1e6
        })
        .fold(f64::MAX, f64::min)
}

/// THE COST OF A BAR DOES NOT GROW WITH ITS POSITION, AND A 60-MINUTE PIECE PLANS IN THE BUDGET:
/// computing a bar (a fresh composer seeks to it and plays it) at minute 1, minute 60 and day 3 of an
/// electro swing piece takes the same time within a factor of two (the fastest of seven each, so a
/// moment taken by the machine does not count); planning a whole 60-minute piece takes under a
/// millisecond. The figures are printed for music.md section 14. RED first (2026-11-21) with a seek
/// playing on from the plan's first phrase instead of from the phrase before the bar: "the bar at minute 60
/// cost 18834 us, at minute 1 258 us" (the run printed 1450391 us for the bar at day 3).
#[test]
fn the_cost_of_a_bar_does_not_grow_with_its_position() {
    let _heavy = crate::tests_audio::heavy();
    let lib = shipped();
    let res = Resolved::new(&lib, &SampleLibrary::new()).unwrap();
    let st = steer_len(&lib, "mothership_calm", "electro_swing", 480.0);
    let probe = {
        let mut c = Composer::new(RATE, 13, st);
        play(&mut c, &res, &mut Tape::default(), 1);
        c.beats_per_bar() as f64 * 60.0 / c.tempo() as f64
    };
    let bar_at = |s: f64| (s / probe) as u64;
    let cost = |bar: u64| {
        fastest(7, || {
            let mut c = Composer::new(RATE, 13, st);
            c.seek_to_bar(&res, bar, &mut Silent);
            let (_, b) = bar_samples(&c, bar);
            play(&mut c, &res, &mut Tape::default(), b);
        })
    };
    let (m1, m60, d3) = (cost(bar_at(60.0)), cost(bar_at(3600.0)), cost(bar_at(3.0 * 86400.0)));
    println!("a bar at minute 1: {m1:.0} us, at minute 60: {m60:.0} us, at day 3: {d3:.0} us");
    assert!(m60 <= 2.0 * m1 + 200.0, "the bar at minute 60 cost {m60:.0} us, at minute 1 {m1:.0} us");
    assert!(d3 <= 2.0 * m1 + 200.0, "the bar at day 3 cost {d3:.0} us, at minute 1 {m1:.0} us");
    // Planning a 60-minute piece (the plan alone: a composer standing at its first bar).
    let hour = Steer { piece_s: 3600.0, ..st };
    let plan = fastest(7, || {
        let mut c = Composer::new(RATE, 13, hour);
        c.seek_to_bar(&res, 0, &mut Silent);
        assert!(c.plan().n > 100, "an hour is {} phrases", c.plan().n);
    });
    println!("planning a 60-minute piece: {plan:.1} us");
    assert!(plan < 1000.0, "planning a 60-minute piece took {plan:.0} us");
}
