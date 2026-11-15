//! Measuring the audio thread's cost per 128-frame block so that a loaded machine does not fail a
//! budget the engine meets (2026-11-15).
//!
//! WHY: the budget tests (docs/design/music.md section 14) kept failing in full test runs and
//! passing alone (data/coordination/flaky-tests.jsonl, 2026-11-14 and 15: deep dubstep, the four new
//! genres, streaming on). Each already scaled its budget by the calm score's cost "in the same
//! run", but measured that calm score ONCE, before or after the piece under test: the genre test
//! read the calm score at 0.0417 ms in its first half second and synthwave at 0.0603 ms ten seconds
//! later, when the machine had become busier, and failed a budget it meets. And a block's time on a
//! shared core is the engine's work plus whatever the OS takes away mid-block, which fills a 99th
//! percentile with preemptions.
//!
//! So two things, both repeats of the MEASUREMENT, never of the verdict:
//! - Every subject is rendered from scratch several times, the subjects TAKING TURNS (calm, piece,
//!   calm, piece, ...), so the baseline and the piece are measured through the same minutes of
//!   load rather than at two different moments.
//! - Each block's time is its FASTEST across those renders. The engine is deterministic: the same
//!   block of the same seed does the same work in every render, so a block that is expensive
//!   because of the engine is expensive in every render and keeps its time, while a preemption has
//!   to strike that very block in every render to survive the minimum (the method
//!   `tests_audio::the_audio_thread_cost_per_block_is_within_the_budget` took on 2026-11-14).

use crate::resolved::Resolved;
use crate::SampleLibrary;

const RATE: f32 = 48000.0;

/// The calm score's quiet cost on the dev machine (i7-8700K, the release-optimised crate): the
/// mean ms of a 128-frame block, which every budget here scales from.
pub(crate) const CALM_QUIET_MS: f64 = 0.037;

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

/// Each subject's blocks, each block's FASTEST time across `reps` renders, the subjects' renders
/// taking turns. A subject builds its engine from scratch, renders whatever comes before the
/// measured stretch untimed, and returns the ms of each timed block, the same count every time.
pub(crate) fn fastest_blocks<'a>(reps: usize, mut subjects: Vec<Box<dyn FnMut() -> Vec<f64> + 'a>>) -> Vec<Vec<f64>> {
    let mut fastest: Vec<Vec<f64>> = vec![Vec::new(); subjects.len()];
    for _ in 0..reps {
        for (i, subject) in subjects.iter_mut().enumerate() {
            let times = subject();
            if fastest[i].is_empty() {
                fastest[i] = times;
            } else {
                assert_eq!(fastest[i].len(), times.len(), "a subject renders the same blocks every time");
                for (f, t) in fastest[i].iter_mut().zip(times) {
                    *f = f.min(t);
                }
            }
        }
    }
    fastest
}

/// Time `blocks` 128-frame blocks of `render`, one by one.
pub(crate) fn time_blocks(blocks: usize, mut render: impl FnMut(&mut [[f32; 2]; 128])) -> Vec<f64> {
    let mut times = Vec::with_capacity(blocks);
    for _ in 0..blocks {
        let mut buf = [[0f32; 2]; 128];
        let t0 = std::time::Instant::now();
        render(&mut buf);
        times.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    times
}

/// The calm score's blocks for one seed, from scratch: 10 s after 2 s, the way `tests_e5::calm_ms`
/// measures it.
pub(crate) fn calm_blocks(lib: &crate::MusicLibrary, seed: u64) -> Vec<f64> {
    let ctx = crate::MusicContext { gain: 1.0, a4_hz: 432.0, duck_db: 0.0, ..Default::default() };
    let mut s = crate::ScoreEngine::new(Resolved::new(lib, &SampleLibrary::new()).unwrap(), RATE, seed, ctx);
    crate::render::render_score(&mut s, RATE, 2.0);
    time_blocks((10.0 * RATE / 128.0) as usize, |buf| s.render(buf))
}

/// How much slower than its quiet cost the calm score ran in these renders (never below 1): the
/// best of its seeds' means over the quiet 0.037 ms.
pub(crate) fn scale_of(calm_seeds: &[Vec<f64>]) -> (f64, f64) {
    let calm = calm_seeds.iter().map(|t| BlockCost::of(t).mean).fold(f64::MAX, f64::min);
    (calm, (calm / CALM_QUIET_MS).max(1.0))
}
