// Child of media/system.rs (#[path]): the computer's own decoders (Windows Media Foundation) on the
// real H.264 + AAC fixture, tests/fixtures/media/moving-box-h264-aac.mp4: 320 x 180, 60 frames at
// 30 fps (2.0 s), an orange 48-pixel box on dark navy moving right and down from the top-left
// corner, and a steady stereo tone at 48 kHz. A computer without Media Foundation (a Windows "N"
// edition without the Media Feature Pack) skips them, saying so. Each test was seen failing once on
// purpose, recorded at the test.

use super::*;
use crate::media::transcode::{begin_ingest, Ingest};
use crate::media::VideoPlayer;
use kira::sound::streaming::Decoder as _;
use std::time::{Duration, Instant};

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("media").join("moving-box-h264-aac.mp4")
}

/// True (and says so) when this computer has no Media Foundation to test.
fn skip() -> bool {
    if available() {
        false
    } else {
        println!("skipped: this copy of Windows has no Media Foundation");
        true
    }
}

fn px(f: &VideoFrame, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * f.width + x) * 4) as usize;
    [f.rgba[i], f.rgba[i + 1], f.rgba[i + 2], f.rgba[i + 3]]
}

/// The box's orange (255, 165, 0 before compression).
fn orange(p: [u8; 4]) -> bool {
    p[0] > 200 && (110..215).contains(&p[1]) && p[2] < 90 && p[3] == 255
}

/// The background's dark navy (about 33, 48, 64): dark, and bluer than it is red.
fn navy(p: [u8; 4]) -> bool {
    p[0] < 80 && p[1] < 90 && p[2] < 120 && p[2] > p[0] && p[3] == 255
}

/// The next frame at or after `at_least` seconds (a minute at most: the clip is two seconds long).
fn frame_at(player: &mut VideoPlayer, at_least: f64) -> VideoFrame {
    let started = Instant::now();
    loop {
        if let Some(f) = player.poll() {
            if f.pts_s >= at_least {
                return f;
            }
        }
        if let Some(e) = player.take_error() {
            panic!("decoding failed: {e}");
        }
        assert!(started.elapsed() < Duration::from_secs(60), "no frame at or after {at_least} s within a minute");
        std::thread::sleep(Duration::from_millis(3));
    }
}

/// WINDOWS READS THE MP4: its visible size, its codecs by name, its length and its sound's rate and
/// channels. Seen red 2026-10-18 with H.264 left out of `codec_name` (left: ("other", Some("AAC"))).
/// Not a red: ignoring the visible area (`MF_MT_MINIMUM_DISPLAY_APERTURE`) changed nothing in any
/// test, because Windows reports both fixtures' frames at their visible size and puts the decoder's
/// padding in the row stride only (1,728 bytes for the 426-wide one); the crop stays for a decoder
/// that reports its padded size.
#[test]
fn windows_reads_the_mp4_with_its_size_length_and_sound() {
    if skip() {
        return;
    }
    let info = crate::media::probe(&fixture()).expect("Windows reads the MP4");
    assert!(info.system, "played by the computer's own decoders");
    assert_eq!((info.width, info.height), (320, 180), "the visible picture");
    assert_eq!((info.video_codec.as_str(), info.audio_codec.as_deref()), ("H.264", Some("AAC")));
    assert_eq!((info.sample_rate, info.channels), (48_000, 2));
    assert!((info.duration_s - 2.0).abs() < 0.1, "about two seconds: {}", info.duration_s);
}

/// ITS PICTURES COME THE RIGHT WAY UP, IN THEIR OWN COLOURS, AND MOVING: the first frame has the box
/// at the top-left and navy along the bottom row (the picture, not padding); a seek while paused
/// lands on the frame at 1.0 s, where the box has moved; played on, frames arrive in order to the
/// end. Seen red 2026-10-18 three ways: with the blue and red bytes left as Windows gives them, "the
/// box at the top-left: [0, 166, 255, 255]"; with the rows read bottom-up, "the box at the top-left:
/// [33, 48, 64, 255]" (navy); and with Windows's own start time left on, "the first frame:
/// 0.1333332" (Windows ignores the MP4's edit list: `start_of`).
#[test]
fn its_pictures_come_the_right_way_up_in_their_colours_and_moving() {
    if skip() {
        return;
    }
    let mut player = VideoPlayer::open(fixture()).expect("open");
    let first = frame_at(&mut player, f64::NEG_INFINITY);
    assert_eq!((first.width, first.height, first.rgba.len()), (320, 180, 320 * 180 * 4));
    assert!(first.pts_s < 0.05, "the first frame: {}", first.pts_s);
    assert!(orange(px(&first, 10, 10)), "the box at the top-left: {:?}", px(&first, 10, 10));
    assert!(navy(px(&first, 300, 170)) && navy(px(&first, 10, 179)), "navy to the bottom row: {:?} {:?}", px(&first, 300, 170), px(&first, 10, 179));

    player.seek_to(1.0);
    let later = frame_at(&mut player, 0.99);
    assert!((later.pts_s - 1.0).abs() < 1.0 / 30.0, "the frame at 1.0 s: {}", later.pts_s);
    assert!(orange(px(&later, 144, 32)) && navy(px(&later, 10, 10)), "the box has moved: {:?} {:?}", px(&later, 144, 32), px(&later, 10, 10));

    player.play();
    let mut last = later.pts_s;
    let mut seen = 0;
    let started = Instant::now();
    while !player.at_end() {
        if let Some(f) = player.poll() {
            assert!(f.pts_s > last, "in order: {} after {last}", f.pts_s);
            last = f.pts_s;
            seen += 1;
        }
        assert!(started.elapsed() < Duration::from_secs(60), "played to the end within a minute");
        std::thread::sleep(Duration::from_millis(2));
    }
    assert!(seen >= 10 && last > 1.9, "played on to the end: {seen} frames, the last at {last} s");
}

/// ITS SOUND DECODES AND REWINDS: two seconds of a stereo tone at 48 kHz (not silence), the same
/// samples again after a seek back to the start, and a seek to the middle lands there. Seen red
/// 2026-10-18 with the seek not moving Windows's reader: "the same samples after a rewind".
#[test]
fn its_sound_decodes_and_rewinds() {
    if skip() {
        return;
    }
    let info = crate::media::probe(&fixture()).expect("probe");
    let mut sound = SystemAudio::open(&fixture(), &info).expect("the sound");
    assert_eq!(sound.sample_rate(), 48_000);
    assert!((sound.num_frames() as i64 - 96_000).abs() < 4_800, "two seconds of frames: {}", sound.num_frames());
    let first = sound.decode().expect("decode");
    assert!(!first.is_empty());
    let (mut total, mut energy) = (0usize, 0.0f64);
    let mut chunk = first.clone();
    while total < 90_000 {
        total += chunk.len();
        energy += chunk.iter().map(|f| (f.left as f64).powi(2) + (f.right as f64).powi(2)).sum::<f64>();
        chunk = sound.decode().expect("decode");
    }
    let rms = (energy / (2 * total) as f64).sqrt();
    assert!(rms > 0.02, "a tone, not silence: {rms}");

    assert_eq!(sound.seek(0).expect("rewind"), 0);
    let again = sound.decode().expect("decode");
    let n = first.len().min(again.len()).min(2048);
    assert!(n > 0);
    let same = first[..n].iter().zip(&again[..n]).all(|(a, b)| (a.left - b.left).abs() < 1e-3 && (a.right - b.right).abs() < 1e-3);
    assert!(same, "the same samples after a rewind");

    assert_eq!(sound.seek(48_000).expect("to the middle"), 48_000);
    assert!(!sound.decode().expect("decode").is_empty(), "sound at the middle");
}

/// AN MP4 PLAYS DIRECTLY ON A SCREEN, NO CONVERSION AND NO FFMPEG NEEDED: the in-world screens'
/// ingest hands it straight to the player. Seen red 2026-10-18 with the computer's own decoders left
/// out of `probe`: "played directly" (it started a conversion).
#[test]
fn an_mp4_plays_directly_with_no_conversion() {
    if skip() {
        return;
    }
    let cache = crate::test_temp::dir("system_ingest");
    match begin_ingest(&fixture(), "", &cache) {
        Ingest::Direct(p) => assert_eq!(p, fixture()),
        Ingest::Transcoding { .. } | Ingest::Cached(_) => panic!("played directly, not converted"),
        Ingest::Failed(msg) => panic!("played directly, not refused: {msg}"),
    }
}

/// THE MP4'S SOUND PLAYS THROUGH KIRA AND LEADS THE PICTURE, on a machine with a sound device,
/// muted (this is about timing, not the speakers): attached while paused the clock stands still;
/// played, the clock follows the sound to about 1.2 s after 1.2 s, and no frame runs ahead of it.
/// Ignored because CI has no sound device; run by hand after touching `SystemAudio`. Seen red
/// 2026-10-18 with the sound's length given to kira as 0: "kira's own position moved: Some(0.0)".
#[test]
#[ignore = "needs an audio output device; run: node scripts/lib-tests.js system::tests::the_mp4s_sound --ignored --nocapture"]
fn the_mp4s_sound_plays_through_kira_and_leads_the_picture() {
    if skip() {
        return;
    }
    let mut audio = match crate::audio::AudioManager::try_new() {
        Ok(a) => a,
        Err(e) => {
            println!("skipped: no audio device ({e})");
            return;
        }
    };
    audio.set_master_volume(0.0);
    let mut player = VideoPlayer::open(fixture()).expect("open");
    player.attach_audio(&mut audio).expect("attach the AAC sound to kira");
    assert!(player.has_audio());
    std::thread::sleep(Duration::from_millis(200));
    assert!(player.position_s() < 0.01, "paused with sound attached: {}", player.position_s());
    player.play();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1200) {
        if let Some(f) = player.poll() {
            assert!(f.pts_s <= player.position_s() + 1.0e-9 || f.pts_s < 0.05, "frame at {} ahead of the clock {}", f.pts_s, player.position_s());
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    let at = player.position_s();
    assert!((at - 1.2).abs() < 0.25, "the sound-led clock near 1.2 s after 1.2 s of play: {at}");
    assert!(player.audio_position_s().is_some_and(|s| s > 0.5), "kira's own position moved: {:?}", player.audio_position_s());
}

fn fixture_named(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("media").join(name)
}

/// A PICTURE WHOSE WIDTH IS NOT A MULTIPLE OF 16 IS NOT SKEWED (a review, 2026-10-18): the 426-wide
/// fixture (navy left half, orange right half) decodes in rows wider than it is, which Windows tells
/// only with the first picture; read with the rows it was announced with, every row slid further
/// along than the one above. Its rows 10 and 230 must both be navy on the left and orange on the
/// right. Seen red 2026-10-18 with the format not read again after `start_of`: "row 120, the left
/// half: [255, 166, 0, 255]" (orange where navy belongs: the rows had slid).
#[test]
fn a_picture_whose_width_is_not_a_multiple_of_16_is_not_skewed() {
    if skip() {
        return;
    }
    let mut player = VideoPlayer::open(fixture_named("split-426-late-sound-h264-aac.mp4")).expect("open");
    let f = frame_at(&mut player, f64::NEG_INFINITY);
    assert_eq!((f.width, f.height), (426, 240));
    for y in [10, 120, 230] {
        assert!(navy(px(&f, 100, y)), "row {y}, the left half: {:?}", px(&f, 100, y));
        assert!(orange(px(&f, 320, y)), "row {y}, the right half: {:?}", px(&f, 320, y));
    }
}

/// A SOUND THAT STARTS LATER THAN THE PICTURE STILL DOES (a review, 2026-10-18): the fixture's tone
/// starts half a second in (an empty edit in its MP4); counted from Windows's first sound it would
/// start at 0, half a second ahead of its picture. The first 0.4 s must be silent and 0.55 s to
/// 0.95 s the tone; after a seek to 0.2 s, silence still, then the tone. Seen red 2026-10-18 with
/// the MP4's empty edits read as nothing (so each stream counted from Windows's first time):
/// "silence before the tone starts: 0.0608" (the tone at once).
#[test]
fn a_sound_that_starts_later_than_the_picture_still_does() {
    if skip() {
        return;
    }
    let path = fixture_named("split-426-late-sound-h264-aac.mp4");
    let info = crate::media::probe(&path).expect("probe");
    let mut sound = SystemAudio::open(&path, &info).expect("the sound");
    let mut all: Vec<Frame> = Vec::new();
    while all.len() < 48_000 {
        all.extend(sound.decode().expect("decode"));
    }
    let rms = |from_s: f64, to_s: f64, frames: &[Frame]| {
        let (a, b) = ((from_s * 48_000.0) as usize, (to_s * 48_000.0) as usize);
        let part = &frames[a..b.min(frames.len())];
        (part.iter().map(|f| (f.left as f64).powi(2)).sum::<f64>() / part.len().max(1) as f64).sqrt()
    };
    assert!(rms(0.0, 0.4, &all) < 0.002, "silence before the tone starts: {}", rms(0.0, 0.4, &all));
    assert!(rms(0.55, 0.95, &all) > 0.05, "the tone after it starts: {}", rms(0.55, 0.95, &all));
    assert_eq!(sound.seek(9_600).expect("to 0.2 s"), 9_600);
    let mut after: Vec<Frame> = Vec::new();
    while after.len() < 24_000 {
        after.extend(sound.decode().expect("decode"));
    }
    assert!(rms(0.0, 0.2, &after) < 0.002, "after a seek to 0.2 s, silence to 0.4 s: {}", rms(0.0, 0.2, &after));
    assert!(rms(0.35, 0.5, &after) > 0.05, "then the tone: {}", rms(0.35, 0.5, &after));
}

/// A VIDEO WHOSE SOUND WINDOWS CANNOT PLAY SAYS SO, AND A SCREEN CONVERTS IT (a review, 2026-10-18):
/// the H.264 + Opus fixture plays its picture through Windows with `sound_lost` naming Opus (the
/// Media page says the sound cannot play here); with no ffmpeg a screen plays it silent rather than
/// not at all. Seen red 2026-10-18 with `sound_lost` left unset: "the lost sound is named" (left:
/// None).
#[test]
fn a_video_whose_sound_windows_cannot_play_says_so() {
    if skip() {
        return;
    }
    let path = fixture_named("h264-opus-sound.mp4");
    let info = crate::media::probe(&path).expect("Windows plays its picture");
    assert!(info.system && info.audio_track.is_none());
    assert_eq!(info.sound_lost.as_deref(), Some("Opus"), "the lost sound is named");
    assert!(crate::gui::pages::media::open::sound_lost_words("Opus").contains("Opus"));
    // A setting naming a file that is not ffmpeg: the conversion cannot start, so it plays silent.
    let d = crate::test_temp::dir("system_sound_lost");
    let fake = d.join("ffmpeg.exe");
    std::fs::write(&fake, b"echo").unwrap();
    match begin_ingest(&path, &fake.display().to_string(), &d.join("cache")) {
        Ingest::Direct(p) => assert_eq!(p, path),
        Ingest::Transcoding { .. } => panic!("no encoder to convert with: played silent"),
        Ingest::Cached(_) => panic!("a fresh cache cannot hit"),
        Ingest::Failed(msg) => panic!("played silent rather than not at all: {msg}"),
    }
    if crate::media::transcode::find_ffmpeg("").is_some() {
        match begin_ingest(&path, "", &d.join("cache")) {
            Ingest::Transcoding { why, .. } => assert!(why.contains("Opus"), "{why}"),
            other => panic!("with ffmpeg, converted for its sound: {:?}", std::mem::discriminant(&other)),
        }
    }
}

/// DECLARED STARTS: the moving-box fixture opens both tracks at 0; the late-sound one opens its sound
/// with an empty edit of 0.478 s (the tone at 0.5 s, after 1,024 samples of encoder priming); a file
/// that is not an MP4 declares nothing. Seen red 2026-10-18 with the empty edits skipped: "the
/// late-sound fixture: (0.0, 0.0)".
#[test]
fn declared_starts_read_the_mp4_edit_lists() {
    assert_eq!(declared_starts(&fixture()), Some((0.0, 0.0)), "the moving-box fixture");
    let (v, a) = declared_starts(&fixture_named("split-426-late-sound-h264-aac.mp4")).expect("an MP4");
    assert!(v == 0.0 && (a - 0.478).abs() < 1e-6, "the late-sound fixture: {:?}", (v, a));
    assert_eq!(declared_starts(&fixture_named("colour-bar-av1-opus.webm")), None, "not an MP4");
}
