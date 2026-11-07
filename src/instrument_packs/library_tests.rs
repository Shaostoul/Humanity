//! THE SEAM WITH THE MUSIC ENGINE: for each part of the score the engine plays the best recorded
//! instrument installed, and its synthesised voice when there is none, so the music always plays.
//!
//! The music engine's crate (crates/humanity-music, music.md section 4.3) was not on main when this
//! was written (2026-11-06; another agent was building its first rung in parallel), so its
//! instrument trait is STUBBED here as music.md describes a voice: a note starts at a key, a
//! velocity and a tuning, and renders into stereo frames. When the crate lands, these tests should
//! drive its real trait and its sampler voice instead of `Stub` and `Sampled` below.

use std::path::{Path, PathBuf};

use super::catalog::{packs_dir, Parts, SynthFallback};
use super::definition::note_hz;
use super::install;
use super::install_tests::{fixture_dir, fixture_entry, fixture_tar};
use super::library::{Available, Choice, InstrumentLibrary};
use super::samples::{SampleBank, SampleVoice};

const RATE: u32 = 48_000;

/// STUB of the music engine's instrument trait (see the file's head).
trait Instrument {
    fn note_on(&mut self, key: u8, velocity: u8, a4_hz: f64);
    fn render(&mut self, out: &mut [[f32; 2]]);
    /// What plays (for the assertions).
    fn what(&self) -> String;
}

/// STUB of a synthesised voice: a sine at the note, named by the part's fallback preset.
struct Stub {
    preset: String,
    hz: f64,
    phase: f64,
}

impl Instrument for Stub {
    fn note_on(&mut self, key: u8, _velocity: u8, a4_hz: f64) {
        self.hz = note_hz(key as f64, a4_hz);
        self.phase = 0.0;
    }
    fn render(&mut self, out: &mut [[f32; 2]]) {
        for o in out.iter_mut() {
            let v = (self.phase * std::f64::consts::TAU).sin() as f32 * 0.5;
            o[0] += v;
            o[1] += v;
            self.phase = (self.phase + self.hz / RATE as f64).fract();
        }
    }
    fn what(&self) -> String {
        format!("synth:{}", self.preset)
    }
}

/// A sampled instrument through this module's parts: the zone, the decoded sample, the rate that
/// tunes it, the voice that plays it.
struct Sampled<'a> {
    available: &'a Available,
    bank: SampleBank,
    voice: Option<SampleVoice>,
    turn: usize,
}

impl Instrument for Sampled<'_> {
    fn note_on(&mut self, key: u8, velocity: u8, a4_hz: f64) {
        let sampler = self.available.sampler();
        let zone = sampler.zone_for(key, velocity, self.turn).expect("a zone").clone();
        self.turn += 1;
        let sample = self.bank.get(&self.available.zone_path(&zone.file)).expect("the sample decodes");
        let rate = sampler.playback_rate(&zone, key, a4_hz);
        self.voice = Some(SampleVoice::start(sample, &zone, rate, RATE, 1.0));
    }
    fn render(&mut self, out: &mut [[f32; 2]]) {
        if let Some(v) = self.voice.as_mut() {
            v.render(out);
        }
    }
    fn what(&self) -> String {
        format!("sampled:{}", self.available.def.id)
    }
}

/// What the engine would build for `part`.
fn instrument_for<'a>(lib: &'a InstrumentLibrary, parts: &'a Parts, part: &str) -> Box<dyn Instrument + 'a> {
    match lib.choose(part, parts).expect("a known part") {
        Choice::Sampled(a) => Box::new(Sampled { available: a, bank: SampleBank::new(), voice: None, turn: 0 }),
        Choice::Synth(SynthFallback { preset, .. }) => Box::new(Stub { preset: preset.clone(), hz: 0.0, phase: 0.0 }),
    }
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

fn play(inst: &mut dyn Instrument, key: u8, velocity: u8, a4: f64, frames: usize) -> Vec<[f32; 2]> {
    let mut out = vec![[0.0f32; 2]; frames];
    inst.note_on(key, velocity, a4);
    for block in out.chunks_mut(128) {
        inst.render(block);
    }
    out
}

/// THE MUSIC FALLS BACK TO SYNTHESIS WITHOUT THE PACK AND PLAYS ITS SAMPLES WITH IT.
///
/// Without the fixture pack: the deep drum part has no recorded instrument, so its synthesised
/// voice (parts.ron's `membrane_low`) plays; the piano part has the core set's upright. With the
/// pack installed (from its archive, checked): the drum part plays the pack's drum, whose sound is
/// the decoded sample itself (an unpitched hit, played at its recorded rate), and the piano part
/// takes the pack's grand (rank 90 over the core's 10), recorded at A = 440 and measured at 432.00
/// and 440.00 Hz when played at each tuning: the sample is retuned as it plays, never on disk.
/// Removed, the drum part falls back to its synthesised voice again.
///
/// Seen red 2026-11-06, under one mutation at a time: `best_for` comparing ranks the wrong way
/// round, "with the pack, the piano part takes its grand" (left: "sampled:fixture_upright");
/// `playback_rate` ignoring the tuning, "A4 at 432 measures 432 Hz: 439.98153349734014"; its
/// pitched / unpitched rule inverted, "the drum's sound is its sample, frame 1".
#[test]
fn the_music_falls_back_to_synthesis_without_the_pack_and_plays_its_samples_with_it() {
    let data = data_folder("instr_seam");
    let parts = Parts::load(&data).expect("parts.ron");

    let lib = InstrumentLibrary::load(&data);
    assert!(lib.problems.is_empty(), "{:?}", lib.problems);
    assert_eq!(instrument_for(&lib, &parts, "deep_drum").what(), "synth:membrane_low", "no drum installed: the synthesised voice");
    assert_eq!(instrument_for(&lib, &parts, "piano").what(), "sampled:fixture_upright", "the core set's piano");
    let mut drum = instrument_for(&lib, &parts, "deep_drum");
    let synth_hz = frequency(&play(drum.as_mut(), 45, 100, 432.0, RATE as usize / 2));
    assert!((synth_hz - 108.0).abs() < 0.05, "the stub synth plays A2 at 432: {synth_hz}");

    install::install_from_reader(&mut fixture_tar().as_slice(), &fixture_entry(), &packs_dir(&data), &install::new_progress()).expect("the pack installs");
    let lib = InstrumentLibrary::load(&data);
    assert!(lib.problems.is_empty(), "{:?}", lib.problems);
    assert_eq!(instrument_for(&lib, &parts, "piano").what(), "sampled:fixture_grand", "with the pack, the piano part takes its grand");
    let mut drum = instrument_for(&lib, &parts, "deep_drum");
    assert_eq!(drum.what(), "sampled:fixture_drum");
    let hit = play(drum.as_mut(), 36, 120, 432.0, 4096);
    let drum_avail = lib.get("fixture_drum").unwrap();
    let zone = drum_avail.sampler().zone_for(36, 120, 0).unwrap().clone();
    let sample = SampleBank::new().get(&drum_avail.zone_path(&zone.file)).unwrap();
    assert_eq!(sample.sample_rate, RATE);
    for i in 0..4096 {
        assert!((hit[i][0] - sample.frames[i][0]).abs() < 1e-6, "the drum's sound is its sample, frame {i}");
    }
    assert!(hit.iter().map(|f| f[0].abs()).fold(0.0, f32::max) > 0.05, "and it is not silence");

    let mut piano = instrument_for(&lib, &parts, "piano");
    for a4 in [432.0, 440.0] {
        let hz = frequency(&play(piano.as_mut(), 69, 100, a4, RATE as usize / 2));
        assert!((hz - a4).abs() < 0.05, "A4 at {a4} measures {a4} Hz: {hz}");
    }
    let low = frequency(&play(piano.as_mut(), 57, 30, 432.0, RATE as usize / 2));
    assert!((low - 216.0).abs() < 0.05, "A3 at 432 measures 216 Hz: {low}");

    install::remove_pack(&packs_dir(&data), "fixture").expect("removes");
    let lib = InstrumentLibrary::load(&data);
    assert_eq!(instrument_for(&lib, &parts, "deep_drum").what(), "synth:membrane_low", "removed: the synthesised voice again");
    assert_eq!(instrument_for(&lib, &parts, "piano").what(), "sampled:fixture_upright");
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
