//! Measuring the audio thread's cost per 128-frame block so that a loaded machine cannot fail a
//! budget the engine meets, while a slower engine still fails it (2026-11-15, remade 2026-11-21).
//!
//! FIRST (2026-11-15): the budget tests (docs/design/music.md section 14) kept failing in full test
//! runs and passing alone (data/coordination/flaky-tests.jsonl: deep dubstep, the four new genres,
//! streaming on). So the MEASUREMENT is repeated, never the verdict: every subject is rendered from
//! scratch several times, the subjects TAKING TURNS, and each block's time is its FASTEST across
//! those renders. The engine is deterministic: the same block of the same seed does the same work in
//! every render, so a block that is expensive because of the engine is expensive in every render and
//! keeps its time, while the OS taking the core away mid-block has to strike that very block in
//! every render to survive the minimum.
//!
//! THEN (2026-11-21): they still failed once in at least four agents' full runs while other compiles
//! loaded the machine, each passing alone: the calm score at 0.0634 and 0.0505 ms a block against its
//! 0.05 budget, the page's analyser, the style block budget. The minimum had already taken the OS's
//! interruptions out, and what was left was not interruptions at all. Measured that day with 12 busy
//! threads beside the test (one per logical processor, multiply-adds): the calm score's fastest blocks
//! read 0.0752 ms by the wall clock and 0.0760 ms by the thread's own CPU cycles
//! (`QueryThreadCycleTime`), against 0.0408 and 0.0415 ms alone. The thread was running the whole
//! time, on a core whose other hyperthread, cache and memory another program shared, so the same work
//! took 1.5 to 1.85 times as long. The thread's own CPU time counts those slower cycles as its own,
//! and a minimum cannot remove a slowdown that lasts the whole run, so neither can tell that load from
//! a slower engine (and the CPU-time clock costs a system call a block, so it is not used). The other
//! gates' baseline, the calm score's cost, was read in renders of its own at other moments than the
//! piece it judged, and it is the engine's own code: a slower oscillator slowed the baseline too and
//! loosened every budget scaled by it.
//!
//! So every timed block now has a YARDSTICK beside it. Before every [`CHUNK`]th block of a subject
//! (untimed for the subject), the [`Yardstick`] runs one unit to warm up and then [`YARD_UNITS`] units,
//! each timed: a fixed piece of arithmetic that shares no code with the engine. Its fastest units, read
//! through the same moments as the subject's fastest blocks, say how much slower than quiet this
//! machine is running such work right now ([`Timed::scale`]), and a subject's cost is its measured
//! cost divided by that, never by less than 1, so a quiet machine holds every budget at exactly its
//! figure. Measured 2026-11-21 by the calm score's own gate
//! (`tests_audio::the_audio_thread_cost_per_block_is_within_the_budget`, its cheapest seed), the busy
//! threads one per logical processor started beside it:
//!
//! | beside the gate                           | yardstick slower | calm score measured | judged    |
//! |-------------------------------------------|------------------|---------------------|-----------|
//! | nothing (a quiet moment)                  | 1.00             | 0.0411 ms           | 0.0411 ms |
//! | other agents' builds and tests            | 1.67             | 0.0652              | 0.0391    |
//! | those, and 12 threads of multiply-adds    | 1.70             | 0.0668              | 0.0393    |
//! | those, and 12 threads of integer work     | 1.76             | 0.0692              | 0.0393    |
//! | those, and half each kind                 | 1.75             | 0.0612              | 0.0349    |
//! | those, and 12 threads each walking 64 MB  | 2.23             | 0.0593              | 0.0266    |
//!
//! The old gate, judging the measured figure, failed the multiply-adds row: "the calm score: 0.0683
//! ms a block on average (budget 0.0500)". Every music gate passed every row (the gates' comments give
//! their figures). The yardstick slows at least as much as the engine under every load measured, so
//! a loaded machine reads the engine at or under its quiet cost and cannot fail a budget the engine
//! meets; and a slower engine is never hidden by a slower yardstick, which is not the engine's code.
//!
//! What a slowdown reads under load, said plainly. Work shaped like the engine's slows with the
//! yardstick and is judged at its full cost: the score's reverb run twice more each block (+0.0117
//! ms) read 0.0528 quiet, 0.0511 beside the multiply-adds and 0.0513 beside the integer work, red
//! each time. Work that load slows less than the engine is judged at its cost over the slowdown: a
//! plain spin read at about two thirds of its cost beside the busy threads, a purely serial chain at
//! about half. And the 64 MB walk, an extreme memory load (each thread evicting the core's shared
//! caches), slowed the yardstick 2.2 to 2.6 times and the engine about 1.45, so there the engine is
//! read low and the reverb twice over passed (0.0312). A quiet machine judges every slowdown at its
//! full cost, and the suite runs on quiet moments often enough to catch what a loaded run lets by.

use std::cell::RefCell;

const RATE: f32 = 48000.0;

/// The yardstick's quiet cost on the dev machine (i7-8700K, the release-optimised crate): the mean ms
/// of one unit, each unit its fastest across renders, the slowest 2% set aside (measured 2026-11-21:
/// 0.0020 ms alone in each of three quiet stretches, and 0.00196 to 0.00204 timed among the gates'
/// blocks at quiet moments). `quiet_the_yardstick_reads_its_quiet_cost` (run by
/// `just music-quiet-budgets`) fails when this no longer matches what the machine measures with
/// nothing else running: a new compiler can change it, and a figure too low would loosen every budget.
pub(crate) const YARDSTICK_QUIET_MS: f64 = 0.0020;

/// The yardstick runs before every CHUNK-th timed block of a subject...
pub(crate) const CHUNK: usize = 16;
/// ...this many timed units (after one untimed, so each timed unit starts with its data in cache).
pub(crate) const YARD_UNITS: usize = 4;

/// The yardstick: 128 samples through ten state-variable band-passes and four feedback delay lines
/// indexed by remainder, a fixed amount of work whatever the numbers (they stay far from the denormal
/// range) that shares no code with the engine. Its shape is the one measured against the engine's
/// cost under load (the table above): change it only with those measurements made again and
/// [`YARDSTICK_QUIET_MS`] with them.
pub(crate) struct Yardstick {
    lines: Vec<Vec<f32>>,
    pos: usize,
    svf: [(f32, f32); 10],
    x: f32,
}

impl Yardstick {
    pub(crate) fn new() -> Self {
        Yardstick { lines: (0..4).map(|k| vec![0.0; 1499 + k * 211]).collect(), pos: 0, svf: [(0.0, 0.0); 10], x: 0.1 }
    }

    /// One unit of work.
    pub(crate) fn unit(&mut self) {
        let mut acc = 0.0f32;
        for i in 0..128 {
            self.x = self.x * -0.99 + 0.013 * i as f32;
            let m = self.x;
            for (k, s) in self.svf.iter_mut().enumerate() {
                let g = 0.05 + 0.03 * k as f32;
                let hp = m - s.0 - 1.4 * s.1;
                let bp = g * hp + s.1;
                let lp = g * bp + s.0;
                *s = (lp, bp);
                acc += bp * bp;
            }
            let mut fb = 0.0;
            for l in self.lines.iter_mut() {
                let n = l.len();
                let r = l[self.pos % n];
                fb += r;
                l[self.pos % n] = m + 0.45 * r;
            }
            acc += fb;
            self.pos += 1;
        }
        std::hint::black_box(acc);
    }
}

thread_local! {
    /// This thread's yardstick, and the times of the units it ran since the last [`take_yard`].
    static YARD: RefCell<(Option<Yardstick>, Vec<f64>)> = const { RefCell::new((None, Vec::new())) };
}

/// One warm-up unit, then [`YARD_UNITS`] units, each timed.
fn yard_chunk() {
    YARD.with(|y| {
        let (stick, times) = &mut *y.borrow_mut();
        let stick = stick.get_or_insert_with(Yardstick::new);
        stick.unit();
        for _ in 0..YARD_UNITS {
            let t0 = std::time::Instant::now();
            stick.unit();
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
    });
}

/// The yardstick units this thread ran since the last call, in order.
fn take_yard() -> Vec<f64> {
    YARD.with(|y| std::mem::take(&mut y.borrow_mut().1))
}

/// Time block `k` of a subject: the yardstick first when `k` is a multiple of [`CHUNK`] (untimed for
/// the subject), then `f`; the ms `f` took.
pub(crate) fn timed(k: usize, f: impl FnOnce()) -> f64 {
    if k % CHUNK == 0 {
        yard_chunk();
    }
    let t0 = std::time::Instant::now();
    f();
    t0.elapsed().as_secs_f64() * 1000.0
}

/// One subject's blocks: the mean without the slowest 2% (the OS taking the core mid-block, as the
/// calm score's own gate says) and the 99th percentile.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BlockCost {
    pub mean: f64,
    pub p99: f64,
}

impl BlockCost {
    pub(crate) fn of(times: &[f64]) -> Self {
        let mut t = times.to_vec();
        t.sort_by(|a, b| a.total_cmp(b));
        let kept = &t[..((t.len() as f64 * 0.98) as usize).max(1)];
        let mean = kept.iter().sum::<f64>() / kept.len() as f64;
        let p99 = t[((t.len() as f64 * 0.99) as usize).min(t.len() - 1)];
        BlockCost { mean, p99 }
    }
}

/// One subject, measured: each block's fastest time across the renders, and the yardstick's units run
/// among those blocks, each its fastest.
pub(crate) struct Timed {
    pub blocks: Vec<f64>,
    pub yard: Vec<f64>,
}

impl Timed {
    /// The yardstick's cost beside this subject (its units' mean without the slowest 2%).
    pub(crate) fn yard_ms(&self) -> f64 {
        if self.yard.is_empty() {
            YARDSTICK_QUIET_MS
        } else {
            BlockCost::of(&self.yard).mean
        }
    }

    /// How many times slower than quiet the machine ran the yardstick beside this subject, never
    /// below 1.
    pub(crate) fn scale(&self) -> f64 {
        (self.yard_ms() / YARDSTICK_QUIET_MS).max(1.0)
    }

    /// The blocks as measured.
    pub(crate) fn raw(&self) -> BlockCost {
        BlockCost::of(&self.blocks)
    }

    /// The blocks' cost on a quiet machine: as measured, over [`Timed::scale`].
    pub(crate) fn cost(&self) -> BlockCost {
        let (raw, s) = (self.raw(), self.scale());
        BlockCost { mean: raw.mean / s, p99: raw.p99 / s }
    }

    /// The plain mean of every block (the hum's measure), over [`Timed::scale`].
    pub(crate) fn plain_mean(&self) -> f64 {
        self.blocks.iter().sum::<f64>() / self.blocks.len().max(1) as f64 / self.scale()
    }

    /// What a failure message says about the measurement: the figure measured and the machine's state.
    pub(crate) fn how(&self) -> String {
        format!(
            "measured {:.4} ms a block, the machine running {:.2} times slower than quiet: the yardstick {:.5} ms a unit against its quiet {YARDSTICK_QUIET_MS:.4}",
            self.raw().mean,
            self.scale(),
            self.yard_ms()
        )
    }
}

/// Of several measured subjects (a piece's seeds), the cheapest by [`Timed::cost`]'s mean.
pub(crate) fn cheapest(t: &[Timed]) -> &Timed {
    t.iter().min_by(|a, b| a.cost().mean.total_cmp(&b.cost().mean)).expect("at least one subject")
}

/// Each subject's blocks, each block's FASTEST time across `reps` renders, the subjects' renders
/// taking turns, with the yardstick's units run among them ([`timed`]). A subject builds its engine
/// from scratch, renders whatever comes before the measured stretch untimed, and returns the ms of
/// each timed block (each through [`timed`] or [`time_blocks`]), the same count every time.
pub(crate) fn fastest_blocks<'a>(reps: usize, mut subjects: Vec<Box<dyn FnMut() -> Vec<f64> + 'a>>) -> Vec<Timed> {
    let mut fastest: Vec<Timed> = (0..subjects.len()).map(|_| Timed { blocks: Vec::new(), yard: Vec::new() }).collect();
    for _ in 0..reps {
        for (i, subject) in subjects.iter_mut().enumerate() {
            take_yard();
            let times = subject();
            let yard = take_yard();
            let f = &mut fastest[i];
            if f.blocks.is_empty() {
                (f.blocks, f.yard) = (times, yard);
            } else {
                assert_eq!(f.blocks.len(), times.len(), "a subject renders the same blocks every time");
                assert_eq!(f.yard.len(), yard.len(), "and runs the same yardstick units among them");
                for (a, t) in f.blocks.iter_mut().zip(times) {
                    *a = a.min(t);
                }
                for (a, t) in f.yard.iter_mut().zip(yard) {
                    *a = a.min(t);
                }
            }
        }
    }
    fastest
}

/// Time `blocks` 128-frame blocks of `render`, one by one ([`timed`]).
pub(crate) fn time_blocks(blocks: usize, mut render: impl FnMut(&mut [[f32; 2]; 128])) -> Vec<f64> {
    let mut times = Vec::with_capacity(blocks);
    for k in 0..blocks {
        let mut buf = [[0f32; 2]; 128];
        times.push(timed(k, || render(&mut buf)));
    }
    times
}

/// One voice's blocks from scratch, for the per-voice budgets (2026-11-15): a fresh pool seeded
/// `seed` playing `insts`, `blocks` 128-frame blocks at `beat_hz` beats a second. `strike(k, pool)`
/// runs before block k, untimed (the note-ons a test schedules); only the pool's render is timed,
/// as the per-voice tests always measured it.
pub(crate) fn voice_blocks(
    insts: &[crate::instrument::Instrument],
    banks: &[Option<std::sync::Arc<dyn crate::SampleBank>>],
    seed: u64,
    blocks: usize,
    beat_hz: f32,
    mut strike: impl FnMut(usize, &mut crate::voice::VoicePool),
) -> Vec<f64> {
    let vowel = [(crate::dsp::SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    let mut pool = crate::voice::VoicePool::new(seed);
    let (mut l, mut r, mut sl, mut sr) = ([0f32; 128], [0f32; 128], [0f32; 128], [0f32; 128]);
    let mut times = Vec::with_capacity(blocks);
    for k in 0..blocks {
        strike(k, &mut pool);
        let beats = k as f64 * 128.0 / RATE as f64 * beat_hz as f64;
        let mut p = crate::voice::BlockParams::plain(RATE, beat_hz, &vowel);
        p.bar = (beats / 4.0) as u64;
        p.beats = beats;
        for b in [&mut l, &mut r, &mut sl, &mut sr] {
            b.fill(0.0);
        }
        times.push(timed(k, || pool.render(insts, banks, &p, &mut l, &mut r, &mut sl, &mut sr, 128)));
    }
    times
}

/// THE YARDSTICK STILL READS ITS QUIET COST (run by `just music-quiet-budgets` on the dev machine with
/// nothing else running; ignored in the suite, where other work makes it read slower, which is what
/// it is for). Every budget divides by how much slower than [`YARDSTICK_QUIET_MS`] the yardstick ran,
/// so a figure that has gone stale (a new compiler making the yardstick cheaper or dearer) would
/// tighten or loosen every gate. Within 15% either way.
#[test]
#[ignore = "a timing judged on a quiet machine only: just music-quiet-budgets"]
fn quiet_the_yardstick_reads_its_quiet_cost() {
    let _heavy = crate::tests_audio::heavy();
    let t = fastest_blocks(5, vec![Box::new(|| time_blocks(4000, |_| {}))]);
    let ms = t[0].yard_ms();
    println!("the yardstick: {ms:.5} ms a unit (quiet figure {YARDSTICK_QUIET_MS:.4})");
    assert!(
        (ms / YARDSTICK_QUIET_MS - 1.0).abs() <= 0.15,
        "the yardstick reads {ms:.5} ms a unit on this machine, more than 15% from YARDSTICK_QUIET_MS ({YARDSTICK_QUIET_MS:.4}) in crates/humanity-music/src/tests_cost.rs. If nothing else was running, the figure is stale: measure it again (this test prints it) and write it there, because every music budget scales by it."
    );
}
