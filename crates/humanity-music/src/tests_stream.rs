//! Streaming long recordings (crate::stream; docs/design/music.md section 23, "Memory"): the packet
//! reader against kira's decode, a streamed note against the whole-file note, a stream that falls
//! behind (or never gets a ring), and the audio thread's cost with every voice streaming.

use std::path::PathBuf;
use std::sync::Arc;

use crate::dsp::SvfCoefs;
use crate::instrument::{SampleBank, SampleSet, SampleZone};
use crate::resolved::Resolved;
use crate::stream::{FileOpener, Service, SourceOpener, StreamPool, StreamSettings, ZoneStream};
use crate::tests::shipped;
use crate::tests_audio::heavy;
use crate::voice::{BlockParams, NoteOn, VoicePool};
use crate::SampleLibrary;

const RATE: f32 = 48000.0;

/// A core recording a few seconds long (the small upright's F#2, Ogg Vorbis at 48 kHz).
fn core_file() -> PathBuf {
    PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data").join("music").join("samples").join("upright_piano_small").join("Fs2vH.ogg")
}

/// The shipped music plus a recorded instrument `test_streamed` with `zones`, resolved.
fn resolved_with(zones: Vec<SampleZone>) -> (Resolved, u16) {
    let mut lib = shipped();
    let inst: crate::instrument::Instrument = ron::from_str(r#"Instrument(id: "test_streamed", kind: Sampler((pack: "test", folder: "test", fallback: "electric_piano", envelope: (attack_s: 0.002, decay_s: 0.0, sustain: 1.0, release_s: 0.5))))"#).unwrap();
    lib.instruments.push(inst);
    let mut samples = SampleLibrary::new();
    samples.insert("test_streamed".into(), Arc::new(SampleSet { zones, exact_keys: false }) as Arc<dyn SampleBank>);
    let res = Resolved::new(&lib, &samples).unwrap();
    let i = lib.instrument_index("test_streamed").unwrap() as u16;
    (res, i)
}

fn zone(frames: Vec<[i16; 2]>, rate: u32, stream: Option<ZoneStream>) -> SampleZone {
    SampleZone { frames: Arc::new(frames), rate: rate as f32, recorded_hz: 92.5, low_note: 0, high_note: 127, velocity: 0.8, loop_range: None, loop_xfade: 0, stream }
}

/// How many frames of a recording at `rate` the A2 of these tests reads a sample: A2 (110 Hz) over
/// the recording's F#2 (92.5 Hz), times its rate over the output's.
fn step_of(rate: u32) -> f32 {
    110.0 / 92.5 * rate as f32 / RATE
}

/// Play `notes` (MIDI keys, each held `hold_s`) on `res`'s instrument `inst` for `seconds`, the left
/// channel; `between` runs before every 64-frame block (the stream worker's pass, in a test).
fn play(res: &Resolved, inst: u16, notes: &[f32], hold_s: f32, seconds: f32, mut between: impl FnMut()) -> Vec<f32> {
    let mut pool = VoicePool::new(1);
    for (k, &midi) in notes.iter().enumerate() {
        pool.note_on(NoteOn { inst, role: 0, midi, a4: 440.0, vel: 0.8, pan: 0.0, gate: 0.0, hold_s, tag: k as u32 + 1, level: 1.0 }, &res.instruments, &res.banks, RATE);
    }
    let vowel = [(SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
    let p = BlockParams::plain(RATE, 1.0, &vowel);
    let mut x = Vec::new();
    let (mut l, mut r, mut sl, mut sr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
    for _ in 0..(seconds * RATE / 64.0) as usize {
        between();
        l.fill(0.0);
        r.fill(0.0);
        pool.render(&res.instruments, &res.banks, &p, &mut l, &mut r, &mut sl, &mut sr, 64);
        x.extend_from_slice(&l);
    }
    x
}

/// The largest step between two samples in a row.
fn max_step(x: &[f32]) -> f32 {
    x.windows(2).map(|w| (w[1] - w[0]).abs()).fold(0.0, f32::max)
}

/// THE PACKET READER DECODES WHAT KIRA DECODES: every core recording of three instruments, an Ogg
/// Vorbis file each, and a 16-bit WAV, decode to the same 16-bit frames through `decode::Reader`
/// (what the score and the stream now use) as through kira's `StaticSoundData::from_file` (what the
/// score used before 2026-11-14), so streaming changed no recording. RED first (2026-11-14) with the
/// reader's stereo channels swapped: "...vibraphone_small/Vibes_soft_A2_v1_rr1_Main.ogg: frame 4
/// differs".
#[test]
fn the_reader_decodes_what_kira_decodes() {
    let kira16 = |p: &std::path::Path| {
        let d = kira::sound::static_sound::StaticSoundData::from_file(p).unwrap();
        (d.sample_rate, crate::instrument::frames16(d.frames.iter().map(|f| [f.left, f.right])))
    };
    let mut files: Vec<PathBuf> = Vec::new();
    for folder in ["upright_piano_small", "vibraphone_small", "cellos_small"] {
        let dir = PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data").join("music").join("samples").join(folder);
        let mut these: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ogg")).collect();
        these.sort();
        files.extend(these.into_iter().take(4));
    }
    let wav = std::env::temp_dir().join(format!("humanity-music-reader-{}.wav", std::process::id()));
    let tone: Vec<[f32; 2]> = (0..30000).map(|i| [(i as f32 * 0.01).sin() * 0.7, (i as f32 * 0.013).cos() * 0.3]).collect();
    crate::render::write_wav(&wav, &tone, 44100).unwrap();
    files.push(wav.clone());
    assert!(files.len() >= 9, "the core recordings are there: {files:?}");
    for f in &files {
        let (rate, frames) = crate::decode::decode_file(f).unwrap();
        let (k_rate, k_frames) = kira16(f);
        assert_eq!(rate, k_rate, "{}: the rate", f.display());
        assert_eq!(frames.len(), k_frames.len(), "{}: the length", f.display());
        if let Some(i) = (0..frames.len()).find(|&i| frames[i] != k_frames[i]) {
            panic!("{}: frame {i} differs", f.display());
        }
        // The head decode is the same frames, cut.
        let (_, head, whole) = crate::decode::decode_head(f, 5000).unwrap();
        assert_eq!(whole, frames.len() <= 5000, "{}: whole or cut", f.display());
        assert_eq!(&head[..], &frames[..frames.len().min(5000)], "{}: the head", f.display());
    }
    let _ = std::fs::remove_file(&wav);
}

/// A STREAMED NOTE IS THE WHOLE-FILE NOTE, SAMPLE FOR SAMPLE: a core recording (an Ogg of a few
/// seconds at 44.1 kHz) played three semitones up (so the read steps 1.09 frames a sample and
/// interpolates across the attack's end) once decoded whole and once with only its first 3,000 frames in memory and the
/// rest streamed through a ring of 4,096 frames (so the ring wraps over thirty times), the worker
/// passing once a block, gives the same output to the last bit for the note's whole length. RED
/// first (2026-11-14) with the worker's source starting one frame late (`skip: start + 1`): "the
/// streamed note differs from the whole-file note at sample 2745 of 384128".
#[test]
fn a_streamed_note_is_the_whole_file_note_sample_for_sample() {
    let _heavy = heavy();
    let file = core_file();
    let (rate, whole) = crate::decode::decode_file(&file).unwrap();
    assert!(whole.len() > 100_000, "a recording of a few seconds: {} frames", whole.len());
    let settings = StreamSettings { ring_s: 0.05, ..Default::default() };
    let pool = StreamPool::new(&settings);
    assert_eq!(pool.ring_frames(), 4096);
    let opener = Arc::new(FileOpener::default());
    let source = opener.register(&file);
    let mut service = Service::new(pool.clone(), opener.clone() as Arc<dyn SourceOpener>);
    let head: Vec<[i16; 2]> = whole[..3000].to_vec();
    let stream = ZoneStream { pool: pool.clone(), source, frames: Some(whole.len() as u64) };
    let (res_whole, iw) = resolved_with(vec![zone(whole.clone(), rate, None)]);
    let (res_streamed, is) = resolved_with(vec![zone(head, rate, Some(stream))]);
    let step = step_of(rate);
    let seconds = (whole.len() as f32 / step / RATE) + 0.5;
    let a = play(&res_whole, iw, &[45.0], 0.0, seconds, || {});
    let b = play(&res_streamed, is, &[45.0], 0.0, seconds, || {
        service.pass();
    });
    assert!(a.iter().any(|x| x.abs() > 0.01), "the note sounds");
    let last = a.iter().rposition(|x| *x != 0.0).unwrap();
    assert!(last as f32 > (whole.len() as f32 / step) * 0.95, "the whole-file note plays its length ({last} samples)");
    if let Some(i) = (0..a.len()).find(|&i| a[i].to_bits() != b[i].to_bits()) {
        panic!("the streamed note differs from the whole-file note at sample {i} of {}", a.len());
    }
    // Every ring given back once the note ended.
    service.pass();
    assert_eq!(pool.in_use(), 0, "the ring is free again");
}

/// A STREAM THAT FALLS BEHIND FADES, WITHOUT A CLICK (measured), and so does a note the budget has no
/// ring for: with the worker stalled (it never passes), a streamed note plays its attack and then
/// fades out over the frames it has, ending before they run out; with one ring and two notes, the
/// second plays its attack and fades the same way. The measure, over the attack's last 0.1 s: no step
/// between two samples in a row larger than the whole-file note's own largest there plus 1% of its
/// level (a 20 ms ramp adds a thousandth), and the last sample before silence within 2% of the
/// level (faded out, not cut). RED first (2026-11-14) with the fade cut to one frame (a stop dead):
/// "the stalled stream ends at 0.02033, not faded out (level 0.3911)". (A first measure, against
/// the note's largest step over its whole attack, passed red: the hammer's onset steps further than
/// any cut.)
#[test]
fn an_underrun_fades_without_a_click() {
    let _heavy = heavy();
    let file = core_file();
    let (rate, whole) = crate::decode::decode_file(&file).unwrap();
    let opener = Arc::new(FileOpener::default());
    let source = opener.register(&file);
    let attack = 12000usize;
    let (res_whole, iw) = resolved_with(vec![zone(whole.clone(), rate, None)]);
    let reference = play(&res_whole, iw, &[45.0], 0.0, 0.5, || {});
    let attack_out = attack as f32 / step_of(rate);
    // The whole-file note over the last 0.1 s of the attack: its own largest step and its level.
    let from = (attack_out - 0.1 * RATE) as usize;
    let near = &reference[from..attack_out as usize];
    let reference_step = max_step(near);
    let level = near.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    assert!(level > 0.01, "the note still sounds at the attack's end: {level}");
    let check = |x: &[f32], what: &str| {
        let s = max_step(&x[from..]);
        let end = x.iter().rposition(|v| *v != 0.0).unwrap_or(0);
        println!("the {what}: largest step {s:.5} against the note's own {reference_step:.5}, last sample {:.5}, level {level:.4}", x[end].abs());
        assert!(s <= reference_step + level / 100.0, "the {what}'s largest step {s:.5} against the note's own {reference_step:.5} (level {level:.4})");
        assert!(x[end].abs() <= level / 50.0, "the {what} ends at {:.5}, not faded out (level {level:.4})", x[end].abs());
        assert!((end as f32) < attack_out, "the {what} ends within its attack ({end} samples)");
        assert!(end as f32 > attack_out - 0.05 * RATE, "the {what} plays its attack first ({end} samples)");
    };
    // The worker stalls: a ring, but nothing comes.
    let pool = StreamPool::new(&StreamSettings::default());
    let stream = ZoneStream { pool: pool.clone(), source, frames: None };
    let (res, i) = resolved_with(vec![zone(whole[..attack].to_vec(), rate, Some(stream))]);
    let stalled = play(&res, i, &[45.0], 0.0, 0.5, || {});
    check(&stalled, "stalled stream");
    // The budget: one ring, two notes at once; the second gets none.
    let one = StreamPool::new(&StreamSettings { streams: 1, ..Default::default() });
    let mut service = Service::new(one.clone(), opener.clone() as Arc<dyn SourceOpener>);
    let stream = ZoneStream { pool: one.clone(), source, frames: None };
    let (res, i) = resolved_with(vec![zone(whole[..attack].to_vec(), rate, Some(stream))]);
    let both = play(&res, i, &[45.0, 45.0], 0.0, 0.5, || {
        service.pass();
    });
    // The first play's ring, given back as its pool went, is free once the worker passes.
    service.pass();
    let streamed_alone = play(&res, i, &[45.0], 0.0, 0.5, || {
        service.pass();
    });
    // Both notes are the same: the second is the pair minus the first.
    let second: Vec<f32> = both.iter().zip(&streamed_alone).map(|(a, b)| a - b).collect();
    check(&second, "note with no ring");
    assert!(streamed_alone[(attack_out) as usize + 2000].abs() > 0.0, "the note with the ring plays on past its attack");
}

/// The audio thread's cost of `voices` notes of the core recording (`rate`, `whole`) at once over
/// twelve keys, each streamed (its attack in memory, the rest from a ring) or held whole: one run of
/// 1.5 s from scratch, timed block by block (the worker's passes, between blocks, are another
/// thread's work and not timed). The test takes each block's fastest of several runs
/// (`tests_cost::fastest_blocks`).
fn block_times(rate: u32, whole: &[[i16; 2]], streamed: bool, voices: usize) -> Vec<f64> {
    let file = core_file();
    {
        let pool = StreamPool::new(&StreamSettings::default());
        let opener = Arc::new(FileOpener::default());
        let source = opener.register(&file);
        let mut service = Service::new(pool.clone(), opener.clone() as Arc<dyn SourceOpener>);
        // Twelve zones, one a key, each its own copy in memory (as a real instrument's recordings
        // are separate files): held whole, or each an attack streaming the rest.
        let zones = (0..12)
            .map(|k| {
                let mut z = match streamed {
                    true => zone(whole[..24000].to_vec(), rate, Some(ZoneStream { pool: pool.clone(), source, frames: None })),
                    false => zone(whole.to_vec(), rate, None),
                };
                (z.low_note, z.high_note) = (40 + k, 40 + k);
                z
            })
            .collect();
        let (res, inst) = resolved_with(zones);
        let mut vp = VoicePool::new(1);
        for k in 0..voices as u32 {
            vp.note_on(NoteOn { inst, role: 0, midi: 40.0 + (k % 12) as f32, a4: 440.0, vel: 0.8, pan: 0.0, gate: 0.0, hold_s: 0.0, tag: k + 1, level: 1.0 }, &res.instruments, &res.banks, RATE);
        }
        assert_eq!(vp.streaming(), if streamed { voices } else { 0 }, "every note streams, or none");
        let vowel = [(SvfCoefs::new(800.0, 10.0, RATE), 1.0); 5];
        let p = BlockParams::plain(RATE, 1.0, &vowel);
        let (mut l, mut r, mut sl, mut sr) = ([0f32; 128], [0f32; 128], [0f32; 128], [0f32; 128]);
        let mut times = Vec::new();
        // 1.5 s: the attack's half second, then a second read from the rings.
        for _ in 0..(1.5 * RATE / 128.0) as usize {
            service.pass();
            l.fill(0.0);
            r.fill(0.0);
            let t0 = std::time::Instant::now();
            vp.render(&res.instruments, &res.banks, &p, &mut l, &mut r, &mut sl, &mut sr, 128);
            times.push(t0.elapsed().as_secs_f64() * 1000.0);
        }
        assert_eq!(vp.voices.iter().filter(|v| v.active).count(), voices, "every note still sounds at the end");
        if streamed {
            assert_eq!(vp.streaming(), voices, "every note still streams at the end");
        }
        times
    }
}

/// THE AUDIO THREAD'S COST WITH STREAMING ON stays within music.md section 14's budget, and streaming
/// costs the audio thread little over a recording held whole. Sixteen recorded notes sounding at once
/// (more than a calm score's piano and harp ever hold: its moods use 12 to 24 voices of every kind),
/// every one streamed, cost under the whole score's budget (0.05 ms a 128-frame block on average,
/// 0.25 ms at the 99th percentile); and 32 streamed notes (every ring the default budget has) cost no
/// more than 25% over the same 32 held whole (measured 2026-11-14 on a quiet machine: 0.0509 against
/// 0.0438 ms, 16% over: the ring's lookups and the block's acquire and release). RED first
/// (2026-11-14) with a sequentially consistent fence on every frame read from a ring (what a ring
/// guarded frame by frame would cost): "32 streamed notes 0.0748 ms a block against 0.0581 ms held
/// whole". (The first version, against one recording shared by every note, read 0.123 against 0.092:
/// every note reading the same memory flattered the whole-file side, so the baseline now gives each
/// key its own copy, as separate files are.)
///
/// MEASURED SO A LOADED MACHINE DOES NOT FAIL IT (2026-11-15, data/coordination/flaky-tests.jsonl:
/// "32 streamed notes 0.0752 ms a block against 0.0538 ms held whole" and 0.0837 against 0.0665,
/// two full runs on 2026-11-14, each passing alone). The three cases were measured one after the
/// other, each the best of its own three runs, so the streamed 32 and the whole 32 were read at
/// different moments of a machine whose load swings costs twice over, and the ratio judged the load.
/// Now the three cases, and the calm score that scales the absolute budget, take turns through five
/// rounds, and each block's time is its fastest of the five (`tests_cost`): measured 2026-11-15
/// beside other agents' builds, 32 streamed 0.0510 ms against 0.0467 held whole (9% over).
///
/// THE QUARTER IS JUDGED ON A QUIET MACHINE ONLY (2026-11-15). Measured that way, five rounds
/// beside other agents' release builds still failed one: "32 streamed notes 0.0631 ms a block
/// against 0.0500 ms held whole", the whole 32 up 6% on its other rounds and the streamed 32 up
/// 24%. That is not noise a minimum removes: reading 32 rings the worker has just written is more
/// sensitive to builds crowding the processor's shared cache than reading recordings straight
/// through, so under that load the ratio itself moves, and the quarter's margin (9% measured, the
/// red 39%) is too thin to judge there. So this suite test holds the 16 notes to the budget and
/// the 32 to twice the whole (red 2026-11-15 with a lock taken on every frame read from a ring:
/// "32 streamed notes 0.1117 ms a block against 0.0470 ms held whole (at most twice on a loaded
/// machine)"), and
/// `quiet_streaming_costs_at_most_a_quarter_more_than_whole` (ignored here) holds the quarter, run
/// by `just music-quiet-budgets`, which waits for a machine with no build running.
#[test]
fn the_block_cost_stays_within_budget_with_streaming_on() {
    let _heavy = heavy();
    let c = streaming_costs();
    assert!(c.s16.mean <= 0.05 * c.scale, "16 streamed notes: {:.4} ms a block on average (budget {:.4}; the calm score {:.4} ms beside it)", c.s16.mean, 0.05 * c.scale, c.calm);
    assert!(c.s16.p99 <= 0.25 * c.scale, "16 streamed notes: {:.4} ms at the 99th percentile (budget {:.4})", c.s16.p99, 0.25 * c.scale);
    assert!(c.s32 <= c.w32 * 2.0, "32 streamed notes {:.4} ms a block against {:.4} ms held whole (at most twice on a loaded machine)", c.s32, c.w32);
}

/// 32 streamed notes cost at most a quarter more than the same 32 held whole: the gate above's
/// quarter, for a quiet machine (`just music-quiet-budgets`; why, in the test above). Still RED
/// for its defect (2026-11-15, the sequentially consistent fence on every frame read from a ring):
/// "32 streamed notes 0.0651 ms a block against 0.0470 ms held whole".
#[test]
#[ignore = "a timing judged on a quiet machine only: just music-quiet-budgets"]
fn quiet_streaming_costs_at_most_a_quarter_more_than_whole() {
    let _heavy = heavy();
    let c = streaming_costs();
    assert!(c.s32 <= c.w32 * 1.25, "32 streamed notes {:.4} ms a block against {:.4} ms held whole", c.s32, c.w32);
}

/// The streaming gates' measurements: 16 and 32 streamed notes, 32 held whole, and the calm score
/// that scales the absolute budget, taking turns through five rounds, each block its fastest.
struct StreamingCosts {
    s16: crate::tests_cost::BlockCost,
    s32: f64,
    w32: f64,
    calm: f64,
    scale: f64,
}

fn streaming_costs() -> StreamingCosts {
    use crate::tests_cost::{calm_blocks, fastest_blocks, scale_of, BlockCost};
    let lib = shipped();
    let (rate, whole) = crate::decode::decode_file(&core_file()).unwrap();
    let whole = &whole;
    let lib = &lib;
    let subjects: Vec<Box<dyn FnMut() -> Vec<f64> + '_>> = vec![
        Box::new(move || calm_blocks(lib, 1)),
        Box::new(move || block_times(rate, whole, true, 16)),
        Box::new(move || block_times(rate, whole, true, 32)),
        Box::new(move || block_times(rate, whole, false, 32)),
    ];
    let blocks = fastest_blocks(5, subjects);
    let (calm, scale) = scale_of(&blocks[..1]);
    let c = StreamingCosts { s16: BlockCost::of(&blocks[1]), s32: BlockCost::of(&blocks[2]).mean, w32: BlockCost::of(&blocks[3]).mean, calm, scale };
    println!(
        "16 streamed notes: {:.4} ms a block on average, {:.4} ms at the 99th percentile; 32 streamed {:.4} ms, 32 held whole {:.4} ms (the calm score {calm:.4} ms beside them)",
        c.s16.mean, c.s16.p99, c.s32, c.w32
    );
    c
}
