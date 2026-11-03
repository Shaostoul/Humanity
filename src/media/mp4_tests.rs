//! Tests for the MP4 reader (BUG-228). The fixtures are real files made by the operator's ffmpeg
//! (the gyan.dev 2025-01-22 full build) from its own synthetic sources, nothing recorded and
//! nothing downloaded (`scripts/make-media-fixtures.sh`, recipes 5 to 7):
//!
//! - `sliding-bar-av1-opus.mp4` (8,004 bytes): 1 s of 160x90 AV1 at 30 fps (libaom, `-cpu-used 8
//!   -crf 40 -g 15`, so keyframes at frames 0 and 15) with a 20 px red bar sliding right at
//!   100 px/s over dark grey, and stereo Opus at 32 kbit/s (a 440 Hz sine), `+faststart`.
//! - `sliding-bar-av1-aac.mp4` (9,205 bytes): the same picture with AAC sound at 48 kbit/s.
//! - `sliding-bar-av1-fragmented.mp4` (2,197 bytes): the Opus file's picture alone, copied into a
//!   fragmented MP4 (`-movflags +frag_keyframe+empty_moov+default_base_moof`), the shape a
//!   streaming site's download takes: no sample tables, two fragments.
//!
//! Every expectation below was read from the files by ffprobe, not from our own reader: the first
//! picture is 89 bytes at byte 2,256 and a keyframe, the 16th (74 bytes at 4,731) the other one; the
//! Opus track has 51 packets and the AAC track 48.

use std::path::PathBuf;

use super::*;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("tests").join("fixtures").join("media").join(name)
}

const AV1_OPUS: &str = "sliding-bar-av1-opus.mp4";
const AV1_AAC: &str = "sliding-bar-av1-aac.mp4";
const AV1_FRAGMENTED: &str = "sliding-bar-av1-fragmented.mp4";

/// THE READER LISTS EVERY SAMPLE WHERE THE FILE HAS IT: the AV1 track's 30 pictures in order, their
/// bytes where ffprobe found them, the two keyframes and nothing else marked as one, a picture every
/// thirtieth of a second from 0; the Opus track's 51 packets with its priming cut by the edit list
/// (312 samples, the encoder's look-ahead); the AAC track's 48. Seen red 2026-11-03 with the
/// sample-to-chunk table's runs ignored (every chunk given the first run's count): the picture held
/// (one sample a chunk throughout) and the sound did not, "every Opus packet" (left: 31, right: 51).
#[test]
fn the_reader_lists_every_sample_where_the_file_has_it() {
    let movie = open(&fixture(AV1_OPUS)).expect("the Opus fixture reads");
    assert_eq!(movie.container, Container::Mp4);
    assert!((movie.duration_s - 1.0).abs() < 1.0e-6, "the file says 1 s: {}", movie.duration_s);
    let video = movie.video.as_ref().expect("a picture");
    assert!(matches!(video.codec, TrackCodec::Av1 { .. }), "{:?}", video.codec);
    assert_eq!((video.width, video.height), (160, 90));
    assert_eq!(video.samples.len(), 30, "one sample a picture");
    assert_eq!((video.samples[0].offset, video.samples[0].size, video.samples[0].sync), (2256, 89, true), "the first picture's bytes are where ffprobe found them");
    assert_eq!((video.samples[15].offset, video.samples[15].size, video.samples[15].sync), (4731, 74, true), "the 16th picture is the other keyframe");
    assert_eq!(video.samples.iter().filter(|s| s.sync).count(), 2, "and only those two are keyframes");
    for (i, s) in video.samples.iter().enumerate() {
        let want = i as f64 / 30.0;
        assert!((video.pts_s(s) - want).abs() < 1.0e-6, "picture {i} shows at {want} s, not {}", video.pts_s(s));
    }
    let sound = movie.audio.as_ref().expect("sound");
    match sound.codec {
        TrackCodec::Opus { channels, pre_skip, multistream } => {
            assert_eq!((channels, pre_skip, multistream), (2, 312, false));
        }
        ref other => panic!("Opus sound, not {other:?}"),
    }
    assert_eq!(sound.samples.len(), 51, "every Opus packet");
    // Its sample-to-chunk table has 20 runs (one sample a chunk, then two, ...): the third and fourth
    // packets share the third chunk.
    let at = |i: usize| (sound.samples[i].offset, sound.samples[i].size);
    assert_eq!((at(0), at(2), at(3), at(50)), ((2172, 84), (2614, 69), (2683, 69), (7851, 153)), "the packets' bytes are where ffprobe found them");
    assert!(sound.edited && sound.start_ticks == 312 && sound.timescale == 48_000, "the edit list cuts the priming: {} ticks at {}", sound.start_ticks, sound.timescale);
    assert_eq!(movie.kind().words().as_deref(), Some("an MP4 file with AV1 video and Opus sound"));

    let aac = open(&fixture(AV1_AAC)).expect("the AAC fixture reads");
    let sound = aac.audio.as_ref().expect("sound");
    assert_eq!(sound.codec, TrackCodec::Other("AAC".into()));
    assert_eq!(sound.samples.len(), 48);
    assert_eq!((sound.samples[0].offset, sound.samples[47].offset), (2271, 9070));
    assert_eq!(aac.kind().words().as_deref(), Some("an MP4 file with AV1 video and AAC sound"));
}

/// A FRAGMENTED MP4 READS LIKE A WHOLE ONE: its pictures come from its fragments' runs, and they are
/// the same bytes, at the same times, with the same keyframes, as in the file it was copied from.
/// Seen red 2026-11-03 with a run's data offset counted from the moof's body instead of its first
/// byte (8 bytes on): "picture 0 is the same bytes" failed.
#[test]
fn a_fragmented_mp4_reads_like_a_whole_one() {
    let whole = SampleReader::open(&fixture(AV1_OPUS), true).expect("whole");
    let mut parts = SampleReader::open(&fixture(AV1_FRAGMENTED), true).expect("fragmented");
    assert!(parts.track.samples.len() == 30, "every picture, from the fragments: {}", parts.track.samples.len());
    let mut whole = whole;
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for i in 0..30 {
        let (ta, ka) = whole.next(&mut a).unwrap().expect("whole has it");
        let (tb, kb) = parts.next(&mut b).unwrap().expect("fragmented has it");
        assert_eq!(a, b, "picture {i} is the same bytes");
        assert!((ta - tb).abs() < 1.0e-6 && ka == kb, "picture {i}: {ta} s {ka} against {tb} s {kb}");
    }
    assert!(parts.next(&mut b).unwrap().is_none(), "and nothing after the last");
}

/// A SEEK LANDS ON THE KEYFRAME AT OR BEFORE THE TIME: decoding has to start at one. Seen red
/// 2026-11-03 with the first keyframe taken instead of the last: "a seek to 0.5 s starts at 0 s
/// (keyframe: true), not 0.5 s".
#[test]
fn a_seek_lands_on_the_keyframe_at_or_before_the_time() {
    let mut r = SampleReader::open(&fixture(AV1_OPUS), true).unwrap();
    let mut buf = Vec::new();
    for (at, want) in [(0.0, 0.0), (0.49, 0.0), (0.5, 0.5), (0.9, 0.5), (5.0, 0.5)] {
        r.seek_to_sync(at);
        let (pts, sync) = r.next(&mut buf).unwrap().unwrap();
        assert!(sync && (pts - want).abs() < 1.0e-6, "a seek to {at} s starts at {pts} s (keyframe: {sync}), not {want} s");
    }
}

/// The smallest MP4 our reader takes, built by `mp4-atom`'s own encoder: one AV1 track whose tables
/// say what `stsz`, `stsc` and `stts` are given, and after it `fragment` (a `moof`, if any). The
/// folder holding it goes when the first value returned is dropped.
fn hostile_file(name: &str, stsz: mp4_atom::Stsz, samples_per_chunk: u32) -> (humanity_core::test_temp::TempPath, PathBuf) {
    hostile_file_with(name, stsz, samples_per_chunk, None)
}

fn hostile_file_with(name: &str, stsz: mp4_atom::Stsz, samples_per_chunk: u32, fragment: Option<Moof>) -> (humanity_core::test_temp::TempPath, PathBuf) {
    use mp4_atom::Encode;
    let mut trak = Trak::default();
    trak.tkhd.track_id = 1;
    trak.mdia.mdhd.timescale = 30;
    trak.mdia.hdlr.handler = mp4_atom::FourCC::new(b"vide");
    trak.mdia.minf.vmhd = Some(Default::default());
    let stbl = &mut trak.mdia.minf.stbl;
    stbl.stsd.codecs.push(Codec::Av01(Default::default()));
    stbl.stsz = stsz;
    stbl.stsc.entries.push(mp4_atom::StscEntry { first_chunk: 1, samples_per_chunk, sample_description_index: 1 });
    stbl.stco = Some(mp4_atom::Stco { entries: vec![0] });
    stbl.stts.entries.push(mp4_atom::SttsEntry { sample_count: u32::MAX, sample_delta: 1 });
    let mvex = fragment.as_ref().map(|_| mp4_atom::Mvex { trex: vec![mp4_atom::Trex { track_id: 1, default_sample_description_index: 1, ..Default::default() }], ..Default::default() });
    let moov = Moov { mvhd: mp4_atom::Mvhd { timescale: 30, ..Default::default() }, trak: vec![trak], mvex, ..Default::default() };
    let mut bytes = Vec::new();
    Ftyp { major_brand: mp4_atom::FourCC::new(b"isom"), minor_version: 0, compatible_brands: vec![] }.encode(&mut bytes).unwrap();
    moov.encode(&mut bytes).unwrap();
    if let Some(moof) = fragment {
        moof.encode(&mut bytes).unwrap();
    }
    let dir = humanity_core::test_temp::dir(&format!("mp4_{name}"));
    let path = dir.join(format!("{name}.mp4"));
    std::fs::write(&path, &bytes).unwrap();
    (dir, path)
}

/// A STRANGER'S FILE CANNOT MAKE US BUILD WITHOUT LIMIT: a file of a few hundred bytes whose tables
/// claim four billion one-byte samples is refused in words at once, so is one whose samples lie
/// past its end, and so is one whose single fragment claims five million samples of no size, which
/// the box costs eight bytes to say and the list would cost 120 MB to hold (four billion of them,
/// 100 GB). A video posted in chat is anyone's file. Seen red 2026-11-03 with the count checks
/// taken out: the fragment opened with every one of its empty samples listed ("five million samples
/// in a small file is a lie: Some(5000000)"), and the four-billion table was stopped only by the
/// second guard (`Demux("MP4: a sample lies past the end of the file")` where "more than the file
/// could hold" was expected).
#[test]
fn a_strangers_file_cannot_make_us_build_without_limit() {
    let run = mp4_atom::Trun { data_offset: None, samples: mp4_atom::TrunSamples::Inherited { count: 5_000_000, first_flags: None } };
    let tfhd = mp4_atom::Tfhd { track_id: 1, default_sample_size: Some(0), default_sample_duration: Some(1), default_base_is_moof: true, ..Default::default() };
    let moof = Moof { mfhd: mp4_atom::Mfhd { sequence_number: 1 }, traf: vec![mp4_atom::Traf { tfhd, trun: vec![run], ..Default::default() }] };
    let empty = mp4_atom::Stsz { samples: mp4_atom::StszSamples::Different { sizes: Vec::new() } };
    let (_dir0, fragmented) = hostile_file_with("huge_fragment", empty, 1, Some(moof));
    assert!(std::fs::metadata(&fragmented).unwrap().len() < 1024, "a small file");
    let err = open(&fragmented).map(|m| m.video.map(|v| v.samples.len())).expect_err("five million samples in a small file is a lie");
    assert!(format!("{err:?}").contains("more samples than the file could hold"), "{err:?}");

    let (_dir, huge) = hostile_file("huge_count", mp4_atom::Stsz { samples: mp4_atom::StszSamples::Identical { count: u32::MAX, size: 1 } }, u32::MAX);
    assert!(std::fs::metadata(&huge).unwrap().len() < 1024, "a small file");
    let err = open(&huge).expect_err("four billion samples in a small file is a lie");
    assert!(format!("{err:?}").contains("more than the file could hold"), "{err:?}");

    let (_dir2, past) = hostile_file("past_the_end", mp4_atom::Stsz { samples: mp4_atom::StszSamples::Identical { count: 3, size: 1_000_000 } }, 3);
    let err = open(&past).expect_err("samples past the end");
    assert!(format!("{err:?}").contains("past the end of the file"), "{err:?}");
}

/// What is not an MP4 is not taken for one: the next reader gets it without a word from this one.
/// Seen red 2026-11-03 with the first-box check taken out: the text file came back as a broken MP4,
/// `Demux("MP4: it has no index (no moov box)")`, which `probe` would have named "an MP4 file".
#[test]
fn a_file_that_is_not_an_mp4_goes_on_to_the_next_reader() {
    let webm = open(&fixture("colour-bar-av1-opus.webm")).expect_err("WebM is not an MP4");
    assert!(not_mp4(&webm), "{webm:?}");
    let dir = humanity_core::test_temp::dir("mp4_text");
    let text = dir.join("notes.mp4");
    std::fs::write(&text, b"these are not the boxes you are looking for").unwrap();
    assert!(not_mp4(&open(&text).expect_err("text")), "a text file named .mp4");
}
