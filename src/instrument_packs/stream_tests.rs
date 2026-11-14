//! Streaming long recordings from disk (docs/design/music.md section 23, "Memory"): the memory a
//! score holds of a long instrument (and of the full Steinway, by hand), and the Settings figure.

use std::path::{Path, PathBuf};

use humanity_music::instrument::SampleZone;
use humanity_music::stream::StreamSettings;
use humanity_music::{MusicLibrary, SampleLibrary};

use super::catalog::{packs_dir, PackCatalog, Parts};
use super::install;
use super::library::InstrumentLibrary;
use super::samples::SampleBank;
use super::score::{attach, PartPlays};

/// How the shipped data says long recordings stream (data/music/streaming.ron): what the game uses.
fn shipped_streaming() -> StreamSettings {
    let (s, problem) = super::samples::stream_settings(&repo_data());
    assert!(problem.is_none(), "{problem:?}");
    s
}

fn repo_data() -> PathBuf {
    PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("data")
}

/// The memory a score's recordings hold: every zone's frames (a streamed zone's attack only).
fn held(samples: &SampleLibrary) -> usize {
    samples.values().map(|b| b.memory()).sum()
}

/// A long piano made for the test, in its own folder: `zones` zones of `seconds` each, 16-bit WAV
/// at 48 kHz (a decaying tone, so a zone has the shape of a struck string), its part the piano, rank
/// above every real piano so it is the one chosen.
fn long_piano(tag: &str, zones: usize, seconds: f32) -> humanity_core::test_temp::TempPath {
    let root = humanity_core::test_temp::dir(tag);
    let samples = root.join("samples").join("long_piano");
    std::fs::create_dir_all(&samples).unwrap();
    std::fs::create_dir_all(root.join("instruments")).unwrap();
    let mut rows = String::new();
    for z in 0..zones {
        let key = 36 + z as u8 * 6;
        let hz = 440.0 * 2f32.powf((key as f32 - 69.0) / 12.0);
        let frames: Vec<[f32; 2]> = (0..(seconds * 48000.0) as usize)
            .map(|i| {
                let t = i as f32 / 48000.0;
                let v = (2.0 * std::f32::consts::PI * hz * t).sin() * 0.5 * (-t / 3.0).exp();
                [v, v * 0.9]
            })
            .collect();
        let file = format!("long_{key}.wav");
        humanity_music::render::write_wav(&samples.join(&file), &frames, 48000).unwrap();
        rows.push_str(&format!("(file: \"{file}\", key: {key}, lo_key: {}, hi_key: {}, lo_vel: 1, hi_vel: 127, rr: 1, recorded_hz: Some({hz}), measured_hz: None, loop_start: None, loop_end: None),\n", key.saturating_sub(3), key + 2));
    }
    let def = format!(
        r#"Instrument(id: "long_piano", words: "instrument_packs.instrument.fixture_upright", part: "piano", rank: 1000,
    kind: Sampler((folder: "long_piano", pitched: true, source_a4_hz: 440.0, tuning_note: "made for the test", release_s: 0.7, normalized: false, zones: [{rows}])),
    range: (low: "C2", high: "C7"), reverb_send: 0.3,
    licence: (name: "CC0 1.0", words: "instrument_packs.licence.cc0", file: "LICENSE.txt"),
    source: (library: "Test", url: "https://example.invalid/test", version: "test", fetched: "2026-11-14"), credit: "Made by the test.")"#
    );
    std::fs::write(root.join("instruments").join("long_piano.ron"), def).unwrap();
    std::fs::write(samples.join("LICENSE.txt"), "CC0").unwrap();
    root
}

/// The shipped music's lo-fi genre (its piano part) with `library`'s recordings attached through
/// `bank`.
fn attach_lofi(library: &InstrumentLibrary, bank: &mut SampleBank) -> super::score::Attached {
    let mut music = MusicLibrary::load(&repo_data()).expect("the music loads");
    let parts = Parts::load(&repo_data()).expect("the parts load");
    let lofi = music.genre_index("lofi").expect("lo-fi");
    attach(&mut music, library, &parts, 432.0, bank, Some(&[lofi]))
}

/// A LONG INSTRUMENT HOLDS ONLY ITS ATTACKS (the full Steinway's case, made small enough for every
/// run): a piano of eight zones of 12 s each (the full Steinway's zones average 21 s), attached to
/// lo-fi's piano part as the game attaches it, holds at most its eight attacks (8 x 0.5 s of 16-bit
/// stereo at 48 kHz, 768 kB), not its 96 s of recordings (18 MiB); every zone streams, and with
/// streaming off it is held whole again. RED first (2026-11-14) at today's whole-file decode
/// (`streaming.ron`'s `enabled: false`, the path every recording took before): "every zone streams"
/// (left: 0, right: 8), its eight recordings held whole, 18,432,000 bytes.
#[test]
fn a_long_instrument_holds_only_its_attacks() {
    let root = long_piano("st-long-piano", 8, 12.0);
    let library = InstrumentLibrary::load_roots(&[(root.to_path_buf(), Some("long".to_string()))]);
    assert!(library.problems.is_empty(), "{:?}", library.problems);
    let bound = 8 * (0.5 * 48000.0) as usize * 4;
    let mut bank = SampleBank::new();
    bank.reuse_override = Some(false);
    bank.configure(shipped_streaming());
    let a = attach_lofi(&library, &mut bank);
    assert_eq!(a.plays("piano"), Some(&PartPlays::Recorded { id: "long_piano".into() }), "the long piano plays the piano part");
    let piano = a.samples.get("recorded:long_piano").expect("attached");
    let streamed = (0..8).filter(|&i| piano.zone(i).stream.is_some()).count();
    assert_eq!(streamed, 8, "every zone streams");
    let bytes = piano.memory();
    assert!(bytes <= bound, "the long piano holds {bytes} bytes (bound {bound})");
    // Off: whole again (and the same frames as far as the attack goes).
    let mut whole_bank = SampleBank::new();
    whole_bank.reuse_override = Some(false);
    whole_bank.configure(StreamSettings { enabled: false, ..Default::default() });
    let w = attach_lofi(&library, &mut whole_bank);
    let whole = w.samples.get("recorded:long_piano").expect("attached");
    assert_eq!(whole.memory(), 8 * 12 * 48000 * 4, "with streaming off it is held whole");
    assert!(whole.memory() > bound, "held whole it is over the bound: the gate can fail");
    let (zs, zw): (&SampleZone, &SampleZone) = (piano.zone(0), whole.zone(0));
    assert_eq!(&zs.frames[..], &zw.frames[..zs.frames.len()], "the attack is the recording's start");
    assert!(zw.stream.is_none());
}

/// THE SETTINGS FIGURE IS WHAT THE SCORE HOLDS: Settings > Audio > Instruments' memory for the long
/// piano's set, counted from its files' headers alone (nothing decoded), equals what the score
/// attached for it holds, and says what it would be held whole. RED first (2026-11-14) at today's
/// whole-file decode (`streaming.ron`'s `enabled: false`): "Settings says the long recordings stream:
/// 18432000 of 18432000 bytes".
#[test]
fn the_settings_figure_is_what_the_score_holds() {
    let root = long_piano("st-long-figure", 8, 12.0);
    let library = InstrumentLibrary::load_roots(&[(root.to_path_buf(), Some("long".to_string()))]);
    let parts = Parts::load(&repo_data()).unwrap();
    let mut bank = SampleBank::new();
    bank.reuse_override = Some(false);
    bank.configure(shipped_streaming());
    let a = attach_lofi(&library, &mut bank);
    let score = a.samples.get("recorded:long_piano").unwrap().memory() as u64;
    let m = super::memory::count(&library, &parts, &shipped_streaming());
    let long = m.get("long").expect("the long set is counted");
    assert_eq!(long.instruments, 1);
    assert_eq!(long.held, score, "the Settings figure {} against the score's {score}", long.held);
    assert_eq!(long.whole, 8 * 12 * 48000 * 4);
    assert!(long.held < long.whole, "Settings says the long recordings stream: {} of {} bytes", long.held, long.whole);
    assert_eq!(long.unread, 0);
}

/// LETTING GO OF THE RECORDINGS LETS GO OF THE STREAMING THREAD (the batch critic, 2026-11-14):
/// `SampleBank::clear` (the music's idle let-go, and every build with the reuse switch off) drops the
/// bank's rings, so their worker thread ends once the last playing note gives its ring back, instead of
/// waking every few milliseconds for the rest of the run with the music off.
///
/// Seen red 2026-11-14 on the code before the fix (clear kept `streamer`): "the rings outlive the let-go:
/// 4194304 bytes still held".
#[test]
fn letting_go_of_the_recordings_lets_go_of_the_streaming_thread() {
    let mut bank = SampleBank::new();
    bank.configure(shipped_streaming());
    assert!(bank.stream_for(Path::new("no-such-recording.flac"), None).is_some(), "the streaming thread starts");
    assert!(bank.stream_memory() > 0, "the rings are made");
    bank.clear();
    assert_eq!(bank.stream_memory(), 0, "the rings outlive the let-go: {} bytes still held", bank.stream_memory());
    assert_eq!(bank.streams(), (0, 0), "no rings are left after the let-go");
}

/// The repo's `data/music` and `data/audio` copied into a temp data folder (its installed packs left
/// behind), where a pack can be installed beside it.
pub(crate) fn music_data(tag: &str) -> humanity_core::test_temp::TempPath {
    let data = humanity_core::test_temp::dir(tag);
    copy_dir(&repo_data().join("music"), &data.join("music"));
    // The hum's sources, which the music's data reads beside its own.
    copy_dir(&repo_data().join("audio"), &data.join("audio"));
    data
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            if e.file_name() == "packs" {
                continue;
            }
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            std::fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// Install the catalog's `id` pack from the staged archive in `HUMANITY_INSTRUMENT_PACKS_OUT` (where
/// `just instrument-packs` writes them), checked against its published checksum as a download is.
pub(crate) fn install_staged(data: &Path, id: &str) -> Result<(), String> {
    let out = std::env::var_os("HUMANITY_INSTRUMENT_PACKS_OUT").ok_or("HUMANITY_INSTRUMENT_PACKS_OUT is not set")?;
    let catalog = PackCatalog::load(&repo_data())?;
    let entry = catalog.pack(id).ok_or(format!("no {id} pack in packs.ron"))?.clone();
    let mut f = std::fs::File::open(Path::new(&out).join(&entry.file)).map_err(|e| format!("{}: {e}", entry.file))?;
    install::install_from_reader(&mut f, &entry, &packs_dir(data), &install::new_progress()).map_err(|e| format!("{e:?}"))
}

/// The memory bound the full Steinway's part plays under: its 126 attacks (126 x 0.5 s of 16-bit
/// stereo at 48 kHz, 11.5 MiB) and every stream ring (32 x 0.68 s, 4 MiB): under 20 MiB, against about 490
/// MiB decoded whole.
const STEINWAY_BOUND: usize = 20 << 20;

/// PLAYING THE FULL STEINWAY PART STAYS UNDER ITS MEMORY BOUND (run by hand with the staged packs,
/// which are not in the repository: `HUMANITY_INSTRUMENT_PACKS_OUT=<folder> node scripts/lib-tests.js
/// the_full_steinway_part_plays_under_its_memory_bound --ignored --nocapture`). The keys pack (version
/// 2) installed, lo-fi built as the game builds it (its piano part takes the full Steinway), every
/// zone of the Steinway played (each of its 126 recordings, one note after another with the stream
/// worker running), and the memory the score holds, its recordings and its rings, measured while they
/// play: under 20 MiB. RED first (2026-11-14) at today's whole-file decode (`enabled: false`): see the
/// measurement printed in the commit and in music.md section 23.
#[test]
#[ignore = "needs the staged keys pack; run by hand with HUMANITY_INSTRUMENT_PACKS_OUT"]
fn the_full_steinway_part_plays_under_its_memory_bound() {
    use humanity_music::dsp::SvfCoefs;
    use humanity_music::voice::{BlockParams, NoteOn, VoicePool};
    let data = music_data("st-steinway");
    install_staged(&data, "keys").expect("the keys pack installs");
    for enabled in [false, true] {
        let mut bank = SampleBank::new();
        bank.reuse_override = Some(false);
        let t0 = std::time::Instant::now();
        let mut settings = super::samples::stream_settings(&data).0;
        settings.enabled = enabled;
        bank.configure(settings);
        let library = InstrumentLibrary::load(&data);
        let parts = Parts::load(&data).unwrap();
        let mut music = MusicLibrary::load(&data).unwrap();
        let lofi = music.genre_index("lofi").unwrap();
        let a = attach(&mut music, &library, &parts, 432.0, &mut bank, Some(&[lofi]));
        let took = t0.elapsed().as_secs_f64();
        assert_eq!(a.plays("piano"), Some(&PartPlays::Recorded { id: "grand_piano_steinway".into() }), "the full Steinway plays the piano part");
        let steinway = a.samples.get("recorded:grand_piano_steinway").unwrap().clone();
        let res = humanity_music::Resolved::new(&music, &a.samples).unwrap();
        let inst = music.instrument_index("recorded:grand_piano_steinway").unwrap() as u16;
        // Every zone, one after another, each note half a second, with the worker streaming.
        let mut pool = VoicePool::new(1);
        pool.offline = true;
        let vowel = [(SvfCoefs::new(800.0, 10.0, 48000.0), 1.0); 5];
        let p = BlockParams::plain(48000.0, 1.0, &vowel);
        let (mut l, mut r, mut sl, mut sr) = ([0f32; 128], [0f32; 128], [0f32; 128], [0f32; 128]);
        let mut most_streaming = 0;
        for key in 21..=108 {
            for vel in [0.2f32, 0.6, 0.95] {
                pool.note_on(NoteOn { inst, role: 0, midi: key as f32, a4: 432.0, vel, pan: 0.0, hold_s: 0.6, tag: key * 4, level: 1.0 }, &res.instruments, &res.banks, 48000.0);
                for _ in 0..(0.05 * 48000.0 / 128.0) as usize {
                    pool.render(&res.instruments, &res.banks, &p, &mut l, &mut r, &mut sl, &mut sr, 128);
                }
                most_streaming = most_streaming.max(pool.streaming());
            }
        }
        let memory = steinway.memory() + bank.stream_memory();
        println!(
            "streaming {}: the full Steinway part holds {:.1} MiB (its recordings {:.1} MiB, the rings {:.1} MiB; every recording the lo-fi score holds {:.1} MiB), {} notes streaming at most; built in {took:.2} s",
            if enabled { "on" } else { "off" },
            memory as f64 / 1048576.0,
            steinway.memory() as f64 / 1048576.0,
            bank.stream_memory() as f64 / 1048576.0,
            held(&a.samples) as f64 / 1048576.0,
            most_streaming
        );
        if enabled {
            assert!(memory <= STEINWAY_BOUND, "the full Steinway's part holds {memory} bytes (bound {STEINWAY_BOUND})");
        } else {
            // Today's whole-file decode is over the bound every run: the gate can fail.
            assert!(memory > STEINWAY_BOUND, "decoded whole, the Steinway would pass the bound ({memory} bytes): the gate cannot fail");
        }
    }
    // Settings' figure for the keys pack, counted from the files' headers (the snapshot shows it).
    let m = super::memory::by_set(&data);
    if let Some(k) = m.get("keys") {
        println!("Settings: the keys pack holds {} bytes while its parts play ({} held whole), {} instruments, {} unread", k.held, k.whole, k.instruments, k.unread);
    }
}
