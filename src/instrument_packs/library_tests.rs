//! THE SEAM WITH THE MUSIC ENGINE: for each part of the score the engine plays the best recorded
//! instrument installed, and its synthesised voice when there is none, so the music always plays.
//!
//! First written (2026-11-06) against a STUB of the engine's instrument trait, because the music
//! engine's crate was being built in parallel; since the crate landed the same day, the seam test
//! drives the REAL engine: the bridge (`super::score::attach`) decodes the installed recordings into
//! humanity-music's sample bank by part, `Resolved` picks each role's instrument (the recording, or
//! the role's synthesised voice without it), and the notes are played by humanity-music's own
//! `VoicePool` (its sampler voice and its membrane drum).

use std::path::{Path, PathBuf};

use humanity_music::mood::RoleKind;
use humanity_music::voice::{BlockParams, NoteOn, VoicePool};
use humanity_music::{MusicLibrary, Resolved};

use super::catalog::{packs_dir, Parts};
use super::install;
use super::install_tests::{fixture_dir, fixture_entry, fixture_tar};
use super::library::{Choice, InstrumentLibrary};
use super::samples::SampleBank;
use super::score::{attach, PartPlays};

const RATE: u32 = 48_000;

/// The live score as the game builds it, for the fixture's data folder: the shipped music (its
/// instruments, its moods) plus a mood whose roles name the two parts the fixture covers (the deep
/// drum on the percussion role, the piano on the pad), with the installed recordings attached by
/// part through the real bridge, and resolved the way `ScoreEngine` takes it.
struct Score {
    res: Resolved,
    plays: Vec<(String, PartPlays)>,
    mood: usize,
}

fn score_for(data: &Path) -> Score {
    let mut music = MusicLibrary::load(&PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data")).expect("the shipped music");
    let seam: humanity_music::mood::Mood = ron::from_str(
        r#"Mood(id: "seam", keys: [(root: "D", mode: Ionian)], chords: [(name: "D", root: 0, notes: [0, 4, 7])],
            moves: [("D", "D", 1.0)], cadences: ["D"], start: "D", phrase_bars: [8], chord_bars: (2, 2), tempo_bpm: (60.0, 60.0),
            roles: [(role: Percussion, instrument: "deep_drum", part: Some("deep_drum"), low: "A1", high: "G#2"),
                    (role: Pad, instrument: "electric_piano", part: Some("piano"), low: "C3", high: "C5", voices: 3)])"#,
    )
    .expect("the seam mood");
    music.moods.push(seam);
    let parts = Parts::load(data).expect("parts.ron");
    let installed = InstrumentLibrary::load(data);
    assert!(installed.problems.is_empty(), "{:?}", installed.problems);
    let attached = attach(&mut music, &installed, &parts, 432.0, &mut SampleBank::new(), None);
    assert!(attached.problems.is_empty(), "{:?}", attached.problems);
    let mood = music.mood_index("seam").unwrap();
    Score { res: Resolved::new(&music, &attached.samples).expect("resolves"), plays: attached.plays, mood }
}

impl Score {
    /// What plays `part`: "sampled:<definition>" or "synth:<the score's instrument>".
    fn what(&self, part: &str) -> String {
        match self.plays.iter().find(|(p, _)| p == part).map(|(_, x)| x) {
            Some(PartPlays::Recorded { id }) => format!("sampled:{id}"),
            Some(PartPlays::Synth { instrument }) => format!("synth:{instrument}"),
            None => "nothing".into(),
        }
    }

    /// One note of the role that plays `role`, through humanity-music's own voices.
    fn play(&self, role: RoleKind, key: u8, velocity: u8, a4: f32, frames: usize) -> Vec<[f32; 2]> {
        let inst = self.res.moods[self.mood].roles[role.index()].as_ref().expect("the role").inst;
        let mut pool = VoicePool::new(1);
        pool.note_on(NoteOn { inst, role: 0, midi: key as f32, a4, vel: velocity as f32 / 127.0, pan: 0.0, hold_s: 0.0, tag: 1, level: 1.0 }, &self.res.instruments, &self.res.banks, RATE as f32);
        let vowel = [(humanity_music::dsp::SvfCoefs::new(800.0, 10.0, RATE as f32), 1.0); 5];
        let p = BlockParams { rate: RATE as f32, pitch_mul: 1.0, beat_hz: 1.0, vowel: &vowel };
        let mut out = Vec::with_capacity(frames);
        let (mut l, mut r, mut sl, mut sr) = ([0f32; 64], [0f32; 64], [0f32; 64], [0f32; 64]);
        while out.len() < frames {
            l.fill(0.0);
            r.fill(0.0);
            pool.render(&self.res.instruments, &self.res.banks, &p, &mut l, &mut r, &mut sl, &mut sr, 64);
            out.extend(l.iter().zip(r.iter()).map(|(a, b)| [*a, *b]));
        }
        out.truncate(frames);
        out
    }
}

/// The strongest frequency between lo and hi Hz in the left channel (a Hann-windowed Goertzel scan).
fn peak_hz(frames: &[[f32; 2]], lo: f64, hi: f64) -> f64 {
    let n = frames.len();
    let mut best = (lo, 0.0);
    let mut f = lo;
    while f <= hi {
        let cw = 2.0 * (std::f64::consts::TAU * f / RATE as f64).cos();
        let (mut q1, mut q2) = (0.0f64, 0.0f64);
        for (i, x) in frames.iter().enumerate() {
            let w = 0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos();
            let q0 = cw * q1 - q2 + x[0] as f64 * w;
            q2 = q1;
            q1 = q0;
        }
        let p = q1 * q1 + q2 * q2 - cw * q1 * q2;
        if p > best.1 {
            best = (f, p);
        }
        f += 0.05;
    }
    best.0
}

/// The fixture's data folder (its core set: `fixture_upright`, the piano part, rank 10) with the
/// game's real `parts.ron`, copied where a pack can be installed beside it.
fn data_folder(tag: &str) -> humanity_core::test_temp::TempPath {
    let data = humanity_core::test_temp::dir(tag);
    copy_dir(&fixture_dir().join("data"), &data);
    let parts = PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data").join("music").join("parts.ron");
    std::fs::copy(parts, data.join("music").join("parts.ron")).expect("parts.ron");
    data
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// The frequency of a rendered tone, from its rising zero crossings over the middle of `frames`
/// (interpolated between samples).
fn frequency(frames: &[[f32; 2]]) -> f64 {
    let x: Vec<f64> = frames.iter().map(|f| f[0] as f64).collect();
    let (from, to) = (x.len() / 8, x.len() * 7 / 8);
    let mut crossings = Vec::new();
    for i in from..to {
        if x[i - 1] < 0.0 && x[i] >= 0.0 {
            crossings.push(i as f64 - 1.0 + (-x[i - 1]) / (x[i] - x[i - 1]));
        }
    }
    let periods = (crossings.len() - 1) as f64;
    RATE as f64 * periods / (crossings[crossings.len() - 1] - crossings[0])
}

/// THE MUSIC FALLS BACK TO SYNTHESIS WITHOUT THE PACK AND PLAYS ITS SAMPLES WITH IT.
///
/// Through the REAL engine (humanity-music). Without the fixture pack: the deep drum part has no
/// recorded instrument, so the score's own synthesised deep drum (a membrane voice) plays, its A2
/// landing on 108.0 Hz at A = 432; the piano part has the core set's upright. With the pack
/// installed (from its archive, checked): the drum part plays the pack's drum through the engine's
/// sampler voice, whose sound is the decoded sample itself scaled by one gain (an unpitched hit,
/// played at its recorded rate), and the piano part takes the pack's grand (rank 90 over the core's
/// 10), recorded at A = 440 and measured at 432.00 and 440.00 Hz when played at each tuning: the
/// sample is retuned as it plays, never on disk. Removed, the drum part falls back to its
/// synthesised voice again.
///
/// Seen red 2026-11-06 against the stub, under one mutation at a time: `best_for` comparing ranks
/// the wrong way round, "with the pack, the piano part takes its grand" (left:
/// "sampled:fixture_upright"); `playback_rate` ignoring the tuning, "A4 at 432 measures 432 Hz:
/// 439.98153349734014"; its pitched / unpitched rule inverted, "the drum's sound is its sample,
/// frame 1". Against the real engine, with the bridge's sample path disabled (every part left to
/// synthesis): "the core set's piano" (left: "synth:electric_piano", right: "sampled:fixture_upright").
#[test]
fn the_music_falls_back_to_synthesis_without_the_pack_and_plays_its_samples_with_it() {
    let data = data_folder("instr_seam");
    let score = score_for(&data);
    assert_eq!(score.what("deep_drum"), "synth:deep_drum", "no drum installed: the score's synthesised deep drum");
    assert_eq!(score.what("piano"), "sampled:fixture_upright", "the core set's piano");
    let synth = score.play(RoleKind::Percussion, 45, 100, 432.0, RATE as usize);
    let synth_hz = peak_hz(&synth[(0.15 * RATE as f64) as usize..], 100.0, 116.0);
    assert!((synth_hz - 108.0).abs() < 0.3, "the synthesised drum plays A2 at 432: {synth_hz}");

    install::install_from_reader(&mut fixture_tar().as_slice(), &fixture_entry(), &packs_dir(&data), &install::new_progress()).expect("the pack installs");
    let score = score_for(&data);
    assert_eq!(score.what("piano"), "sampled:fixture_grand", "with the pack, the piano part takes its grand");
    assert_eq!(score.what("deep_drum"), "sampled:fixture_drum", "with the pack, the drum part plays its samples");
    let hit = score.play(RoleKind::Percussion, 36, 120, 432.0, 4096);
    let lib = InstrumentLibrary::load(&data);
    let drum_avail = lib.get("fixture_drum").unwrap();
    let zone = drum_avail.sampler().zone_for(36, 120, 0).unwrap().clone();
    let sample = SampleBank::new().get(&drum_avail.zone_path(&zone.file)).unwrap();
    assert_eq!(sample.sample_rate, RATE);
    // Past the voice's 2 ms rise, the engine's output is the sample times one gain (its level and
    // velocity): fitted by least squares, the rest is nothing.
    let range = 200..4096;
    let (num, den) = range.clone().fold((0.0f64, 0.0f64), |(n, d), i| (n + hit[i][0] as f64 * sample.frames[i][0] as f64, d + (sample.frames[i][0] as f64).powi(2)));
    let k = num / den.max(1e-12);
    let peak = range.clone().map(|i| sample.frames[i][0].abs() as f64).fold(0.0, f64::max);
    for i in range {
        assert!((hit[i][0] as f64 - k * sample.frames[i][0] as f64).abs() < 1e-3 * k.abs() * peak.max(1e-9), "the drum's sound is its sample, frame {i}");
    }
    assert!(hit.iter().map(|f| f[0].abs()).fold(0.0, f32::max) > 1e-3 && k > 0.0, "and it is not silence (gain {k})");
    for a4 in [432.0f32, 440.0] {
        let hz = frequency(&score.play(RoleKind::Pad, 69, 100, a4, RATE as usize / 2));
        assert!((hz - a4 as f64).abs() < 0.05, "A4 at {a4} measures {a4} Hz: {hz}");
    }
    let low = frequency(&score.play(RoleKind::Pad, 57, 30, 432.0, RATE as usize / 2));
    assert!((low - 216.0).abs() < 0.05, "A3 at 432 measures 216 Hz: {low}");

    install::remove_pack(&packs_dir(&data), "fixture").expect("removes");
    let score = score_for(&data);
    assert_eq!(score.what("deep_drum"), "synth:deep_drum", "removed: the synthesised voice again");
    assert_eq!(score.what("piano"), "sampled:fixture_upright");
    // The library's own choice agrees with what the score plays.
    let parts = Parts::load(&data).unwrap();
    assert!(matches!(InstrumentLibrary::load(&data).choose("deep_drum", &parts), Some(Choice::Synth(_))));
}

/// DECODED SAMPLES OBEY THE "TURN OFF ALL REUSE" SWITCH: with reuse on, a sample decoded once is
/// kept and handed out again (the same memory); with it off, nothing is kept and every note decodes
/// its file again.
///
/// Seen red 2026-11-06 with `get` ignoring the switch (always keeping): "with reuse off, every note
/// decodes again".
#[test]
fn decoded_samples_obey_the_reuse_switch() {
    let file = fixture_dir().join("data").join("music").join("samples").join("fixture_upright").join("Up_A3_v1.ogg");
    let mut on = SampleBank::new();
    on.reuse_override = Some(true);
    let a = on.get(&file).expect("decodes");
    let b = on.get(&file).expect("decodes");
    assert!(std::sync::Arc::ptr_eq(&a, &b), "with reuse on, the decoded sample is kept");
    assert_eq!(on.kept(), 1);
    let mut off = SampleBank::new();
    off.reuse_override = Some(false);
    let c = off.get(&file).expect("decodes");
    let d = off.get(&file).expect("decodes");
    assert!(!std::sync::Arc::ptr_eq(&c, &d), "with reuse off, every note decodes again");
    assert_eq!(off.kept(), 0, "with reuse off, nothing is kept");
    assert_eq!(*c, *d, "the same sound either way");
}

/// AN EXACT-KEY INSTRUMENT PLAYS ONLY ITS OWN NOTES: the Hang (FreePats, D minor) has nine notes
/// and its readme asks that the semitones between them not be filled by pitching its neighbours
/// ("Other notes are silent"), because every note carries the others' resonance. With
/// `exact_keys`, a key the instrument does not have finds no zone; without it, the nearest zone
/// plays, as for every other instrument.
///
/// Seen red 2026-11-07 before `exact_keys` existed (the flag was read and ignored): "a note the
/// Hang does not have is silent" (left: Some(62), right: None).
#[test]
fn an_exact_key_instrument_plays_only_its_own_notes() {
    let ron = |exact: &str| {
        format!(
            "Instrument(id: \"hang\", words: \"w\", kind: Sampler((folder: \"hang\", source_a4_hz: 440.0{exact}, zones: [\
             (file: \"A3.ogg\", key: 57, lo_key: 57, hi_key: 57, lo_vel: 1, hi_vel: 127, recorded_hz: Some(220.0)),\
             (file: \"D4.ogg\", key: 62, lo_key: 62, hi_key: 62, lo_vel: 1, hi_vel: 127, recorded_hz: Some(293.66))])))"
        )
    };
    let exact = super::definition::Instrument::parse(&ron(", exact_keys: true")).expect("parses");
    let s = exact.sampler().unwrap();
    assert_eq!(s.zone_for(57, 100, 0).map(|z| z.key), Some(57));
    assert_eq!(s.zone_for(62, 100, 0).map(|z| z.key), Some(62));
    assert_eq!(s.zone_for(60, 100, 0).map(|z| z.key), None, "a note the Hang does not have is silent");
    let spread = super::definition::Instrument::parse(&ron("")).expect("parses");
    assert_eq!(spread.sampler().unwrap().zone_for(60, 100, 0).map(|z| z.key), Some(62), "other instruments take the nearest zone");
}

/// An instrument definition of another kind (the music engine's synthesised voices share the
/// folder) is recognised and left alone, and a sampled one whose sample is missing is left out
/// with the reason, so its part falls back instead of playing silence.
///
/// Seen red 2026-11-06 against the first `Kind` reader, which read the variant's name as a quoted
/// string: "a Synth definition is not a problem: [... soft_saw_pad.ron: 1:78: Expected string]"
/// (and the Sampler definitions failed the same way).
#[test]
fn other_kinds_are_left_alone_and_a_missing_sample_is_reported() {
    let data = data_folder("instr_kinds");
    let dir = data.join("music").join("instruments");
    std::fs::write(
        dir.join("soft_saw_pad.ron"),
        "Instrument(id: \"soft_saw_pad\", words: \"music.instrument.soft_saw_pad\", kind: Synth((oscillators: [(wave: Saw, detune_cents: -7)], spread: 0.65)), range: (low: \"D2\", high: \"A5\"), reverb_send: 0.4)",
    )
    .unwrap();
    let lib = InstrumentLibrary::load(&data);
    assert!(lib.problems.is_empty(), "a Synth definition is not a problem: {:?}", lib.problems);
    assert!(lib.get("soft_saw_pad").is_none(), "and not a sampled instrument");
    std::fs::remove_file(data.join("music").join("samples").join("fixture_upright").join("Up_A3_v1.ogg")).unwrap();
    let lib = InstrumentLibrary::load(&data);
    assert!(lib.get("fixture_upright").is_none(), "an instrument missing a sample is left out");
    assert!(lib.problems.iter().any(|p| p.contains("Up_A3_v1.ogg")), "{:?}", lib.problems);
    let parts = Parts::load(&data).unwrap();
    assert!(matches!(lib.choose("piano", &parts), Some(Choice::Synth(_))), "so the piano part falls back");
}
