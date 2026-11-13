//! Tests for playing through the person's ffmpeg as the film plays (`stream.rs`). The parsing and
//! the command lines are pure and always run; the ones that start ffmpeg are skipped (printed)
//! on a machine without it.

use super::*;
use crate::media::ffmpeg::find_ffmpeg;
use std::time::Instant;

fn scratch(name: &str) -> humanity_core::test_temp::TempPath {
    humanity_core::test_temp::dir(&format!("stream_{name}"))
}

/// `seconds` of a moving 1280x720 test pattern as MPEG-4 Part 2 with a 440 Hz tone as AC-3, in
/// Matroska: encoders every ffmpeg carries, and a file neither our decoders nor the computer's own
/// play, so it always takes this road.
fn make_clip(ffmpeg: &Path, dir: &Path, seconds: u32) -> PathBuf {
    make_clip_of(ffmpeg, dir, &format!("testsrc2=size=1280x720:rate=30:duration={seconds}"), seconds)
}

/// `make_clip` with any lavfi picture source (`picture`) and the same tone.
fn make_clip_of(ffmpeg: &Path, dir: &Path, picture: &str, seconds: u32) -> PathBuf {
    let out = dir.join("clip.mkv");
    let status = crate::media::ffmpeg::ffmpeg_command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg(picture)
        .args(["-f", "lavfi", "-i"])
        .arg(format!("sine=frequency=440:duration={seconds}"))
        .args(["-c:v", "mpeg4", "-q:v", "3", "-c:a", "ac3", "-shortest"])
        .arg(&out)
        .status()
        .expect("ffmpeg runs");
    assert!(status.success() && out.is_file(), "the clip was made");
    out
}

/// Real summaries, copied from `ffmpeg -hide_banner -i` (gyan.dev build 2025-01-22) on
/// 2026-11-13: a video disc's title, the H.264 MP4 fixture, an AV1 WebM whose pixel shape is the
/// container's (`, SAR 1:1 DAR ...` as a part of its own), an interlaced MPEG-2 with its colour
/// space, an H.264 MKV with sound, and a sound file with no picture.
#[test]
fn the_summary_is_read_from_real_ffmpeg_output() {
    let disc = "Input #0, mpeg, from 'title.vob':\n  Duration: 00:01:00.03, start: 0.528033, bitrate: 3168 kb/s\n  Stream #0:0[0x1bf]: Data: dvd_nav_packet\n  Stream #0:1[0x1e0]: Video: mpeg2video (Main), yuv420p(tv, progressive), 720x480 [SAR 32:27 DAR 16:9], 29.97 fps, 29.97 tbr, 90k tbn\n  Stream #0:2[0x80]: Audio: ac3, 48000 Hz, stereo, fltp, 448 kb/s\n";
    let s = parse_summary(disc).expect("a picture");
    assert_eq!((s.width, s.height, s.sar), (720, 480, (32, 27)));
    assert_eq!(s.video_codec, "mpeg2video");
    assert_eq!(s.audio_codec.as_deref(), Some("ac3"));
    assert!((s.duration_s.unwrap() - 60.03).abs() < 1e-6);
    assert!((s.fps.unwrap() - 29.97).abs() < 1e-6);
    assert!(!s.interlaced);
    assert_eq!(s.colour, None);

    let mp4 = "  Duration: 00:00:02.00, start: 0.000000, bitrate: 91 kb/s\n  Stream #0:0[0x1](und): Video: h264 (High) (avc1 / 0x31637661), yuv420p(progressive), 320x180 [SAR 1:1 DAR 16:9], 12 kb/s, 30 fps, 30 tbr, 15360 tbn (default)\n  Stream #0:1[0x2](und): Audio: aac (LC) (mp4a / 0x6134706D), 48000 Hz, stereo, fltp, 63 kb/s (default)\n";
    let s = parse_summary(mp4).unwrap();
    assert_eq!((s.width, s.height, s.sar, s.fps), (320, 180, (1, 1), Some(30.0)));
    assert_eq!((s.video_codec.as_str(), s.audio_codec.as_deref()), ("h264", Some("aac")));

    let webm = "  Duration: 01:57:02.06, start: -0.007000, bitrate: 710 kb/s\n  Stream #0:0(jpn): Video: av1 (libdav1d) (Main), yuv420p(tv, progressive), 1280x692, SAR 3:4 DAR 320:173, 23.98 fps, 23.98 tbr, 1k tbn\n  Stream #0:1(jpn): Audio: opus, 48000 Hz, stereo, fltp (default)\n";
    let s = parse_summary(webm).unwrap();
    assert_eq!((s.width, s.height, s.sar), (1280, 692, (3, 4)), "the container's pixel shape, a part of its own");
    assert!((s.duration_s.unwrap() - 7022.06).abs() < 1e-6);

    let interlaced = "  Duration: 00:00:01.00, start: 0.500000, bitrate: 5000 kb/s\n  Stream #0:0[0x1e0]: Video: mpeg2video (Main), yuv420p(tv, bt470bg/unknown/unknown, top first), 720x480 [SAR 8:9 DAR 4:3], 29.97 fps, 29.97 tbr, 90k tbn\n";
    let s = parse_summary(interlaced).unwrap();
    assert!(s.interlaced, "top first is interlaced");
    assert_eq!(s.colour.as_deref(), Some("bt470bg"));
    assert_eq!(s.audio_codec, None, "a silent film");

    let mkv = "  Duration: 00:01:00.02, start: -0.021000, bitrate: 1258 kb/s\n  Stream #0:0(jpn): Video: h264 (High), yuv420p(tv, progressive), 1280x692 [SAR 1:1 DAR 320:173], 23.98 fps, 23.98 tbr, 1k tbn\n  Stream #0:1(jpn): Audio: aac (LC), 48000 Hz, stereo, fltp (default)\n";
    assert_eq!(parse_summary(mkv).unwrap().audio_codec.as_deref(), Some("aac"));

    let sound_only = "  Duration: 00:00:01.04, start: 0.025057, bitrate: 65 kb/s\n  Stream #0:0: Audio: mp3 (mp3float), 44100 Hz, mono, fltp, 64 kb/s\n";
    assert_eq!(parse_summary(sound_only), None, "no picture, nothing to play on a screen");

    // Cover art is not the film's picture.
    let cover = "  Duration: 00:03:00.00, start: 0.000000, bitrate: 320 kb/s\n  Stream #0:0: Audio: mp3 (mp3float), 44100 Hz, stereo, fltp, 320 kb/s\n  Stream #0:1: Video: mjpeg (Baseline), yuvj420p(pc, bt470bg/unknown/unknown), 600x600 [SAR 1:1 DAR 1:1], 90k tbr, 90k tbn (attached pic)\n";
    assert_eq!(parse_summary(cover), None);
    assert_eq!(parse_summary("not media at all: Invalid data found when processing input"), None);
}

/// Square pixels, at most 1080 rows, even sides: a 16:9 disc's 720x480 widens to 854x480, a 4:3
/// one narrows to 640x480, a 4K film comes through at 1920x1080, a small film keeps its size.
#[test]
fn the_pipe_size_squares_pixels_and_caps_rows() {
    assert_eq!(output_size(720, 480, (32, 27)), (854, 480));
    assert_eq!(output_size(720, 480, (8, 9)), (640, 480));
    assert_eq!(output_size(3840, 2160, (1, 1)), (1920, 1080));
    assert_eq!(output_size(320, 180, (1, 1)), (320, 180));
    assert_eq!(output_size(1280, 692, (1, 1)), (1280, 692));
    assert_eq!(output_size(321, 181, (0, 1)), (322, 182), "odd sides made even; an unknown shape is square");
}

/// The NTSC rates ffmpeg prints rounded become their exact fractions (a two-hour film at 29.97
/// would otherwise slip 7 frames from its sound); whole rates stay whole; nonsense plays at 30.
#[test]
fn the_rate_is_an_exact_fraction() {
    assert_eq!(fps_fraction(Some(29.97)), (30_000, 1001));
    assert_eq!(fps_fraction(Some(23.98)), (24_000, 1001));
    assert_eq!(fps_fraction(Some(59.94)), (60_000, 1001));
    assert_eq!(fps_fraction(Some(25.0)), (25, 1));
    assert_eq!(fps_fraction(Some(12.5)), (12_500, 1000));
    assert_eq!(fps_fraction(None), (30, 1));
    assert_eq!(fps_fraction(Some(90_000.0)), (30, 1), "a time base, not a rate");
    assert_eq!(matrix_for(Some("bt709"), 480), ColorMatrix::Bt709);
    assert_eq!(matrix_for(Some("bt470bg"), 1080), ColorMatrix::Bt601);
    assert_eq!(matrix_for(None, 1080), ColorMatrix::Bt709);
    assert_eq!(matrix_for(Some("unknown"), 480), ColorMatrix::Bt601);
}

fn disc_source(interlaced: bool) -> StreamSource {
    let summary = Summary {
        duration_s: Some(60.0),
        video_codec: "mpeg2video".into(),
        width: 720,
        height: 480,
        sar: (32, 27),
        fps: Some(29.97),
        interlaced,
        colour: None,
        audio_codec: Some("ac3".into()),
    };
    StreamSource::from_summary(PathBuf::from("ffmpeg"), PathBuf::from("concat:F:\\VIDEO_TS\\VTS_01_1.VOB|F:\\VIDEO_TS\\VTS_01_2.VOB"), summary)
}

/// THE COMMAND LINES, exactly: a later edit cannot quietly drop the deinterlacing, the steady
/// rate, the studio range, the seek before the input or the sound's gap filling.
#[test]
fn the_command_lines_are_pinned() {
    let words = |a: Vec<OsString>| a.into_iter().map(|s| s.to_string_lossy().to_string()).collect::<Vec<_>>().join(" ");
    let src = disc_source(true);
    assert_eq!(
        words(video_args(&src, 0.0)),
        "-hide_banner -nostdin -loglevel error -i concat:F:\\VIDEO_TS\\VTS_01_1.VOB|F:\\VIDEO_TS\\VTS_01_2.VOB -map 0:V:0 -an -sn -dn -vf bwdif=mode=send_frame:deint=interlaced,fps=30000/1001,scale=854:480:out_range=tv,setsar=1 -pix_fmt yuv420p -f yuv4mpegpipe pipe:1"
    );
    assert_eq!(
        words(video_args(&disc_source(false), 61.25)),
        "-hide_banner -nostdin -loglevel error -ss 61.250 -i concat:F:\\VIDEO_TS\\VTS_01_1.VOB|F:\\VIDEO_TS\\VTS_01_2.VOB -map 0:V:0 -an -sn -dn -vf fps=30000/1001,scale=854:480:out_range=tv,setsar=1 -pix_fmt yuv420p -f yuv4mpegpipe pipe:1"
    );
    assert_eq!(
        words(audio_args(&src, 61.25)),
        "-hide_banner -nostdin -loglevel error -ss 61.250 -i concat:F:\\VIDEO_TS\\VTS_01_1.VOB|F:\\VIDEO_TS\\VTS_01_2.VOB -map 0:a:0 -vn -sn -dn -af aresample=48000:async=1:first_pts=0 -ac 2 -f f32le pipe:1"
    );
}

#[test]
fn the_raw_picture_header_is_read() {
    assert_eq!(
        parse_y4m_header("YUV4MPEG2 W854 H480 F30000:1001 Ip A1:1 C420jpeg XYSCSS=420JPEG"),
        Ok(Y4mHeader { width: 854, height: 480, fps: (30_000, 1001) })
    );
    assert_eq!(parse_y4m_header("YUV4MPEG2 W2 H2 F25:1 C420mpeg2").unwrap().fps, (25, 1));
    assert!(parse_y4m_header("YUV4MPEG2 W2 H2 F25:1 C444").is_err(), "only 4:2:0 is read");
    assert!(parse_y4m_header("RIFF").is_err());
    assert!(parse_y4m_header("YUV4MPEG2 F25:1").is_err(), "no size");
}

/// A CRASH LEAVES NO FFMPEG RUNNING. When the app dies, Windows closes its end of the pipes and
/// nothing else: no Drop runs. The picture ffmpeg is started and read for a while, then its
/// reader is closed WITHOUT the kill a Drop would do (the `Running` is forgotten); ffmpeg must
/// notice on its next write and exit by itself, and it must have written no file.
///
/// RED first (2026-11-13): with the reader kept open instead of closed (what a converter writing
/// to a file amounts to: nothing ties it to the app), "ffmpeg still running 10 s after its reader
/// went".
#[test]
fn a_crash_leaves_no_ffmpeg_running() {
    let Some(ffmpeg) = find_ffmpeg("") else {
        println!("skipped: no ffmpeg on this machine");
        return;
    };
    let dir = scratch("crash");
    let clip = make_clip(&ffmpeg, &dir, 20);
    let src = look(&ffmpeg, &clip).expect("ffmpeg reads the clip");
    let (running, stdout) = Running::spawn(&ffmpeg, &video_args(&src, 0.0)).expect("ffmpeg starts");
    let mut reader = BufReader::new(stdout);
    let header = parse_y4m_header(&read_line(&mut reader).unwrap().unwrap()).unwrap();
    assert_eq!((header.width, header.height), (1280, 720));
    let mut frame = vec![0u8; 1280 * 720 * 3 / 2];
    assert!(read_line(&mut reader).unwrap().unwrap().starts_with("FRAME"));
    reader.read_exact(&mut frame).unwrap();
    let slot = running.child.clone();
    std::mem::forget(running); // the crash: no Drop, no kill
    drop(reader); // ... but the OS closes our end of the pipe
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut exited = false;
    while Instant::now() < deadline {
        if lock(&slot).as_mut().unwrap().try_wait().unwrap().is_some() {
            exited = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !exited {
        let _ = lock(&slot).as_mut().unwrap().kill();
    }
    assert!(exited, "ffmpeg still running 10 s after its reader went");
    let mut left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    left.sort();
    assert_eq!(left, vec!["clip.mkv".to_string()], "ffmpeg wrote nothing");
}

/// The whole road through the player: a clip only ffmpeg decodes opens at once, its pictures come
/// at its size, a seek lands where it was asked (the clip's brightness IS its time, so the picture
/// itself says where it landed, not only the time the pipe was given), its sound is the tone it
/// was made with (not silence), and dropping the player joins its thread.
#[test]
fn a_film_plays_through_ffmpeg_and_seeks() {
    let Some(ffmpeg) = find_ffmpeg("") else {
        println!("skipped: no ffmpeg on this machine");
        return;
    };
    let dir = scratch("play");
    // Grey that brightens one step a picture: luma 16 + 30 per second, so 136 at 4 s.
    let clip = make_clip_of(&ffmpeg, &dir, "color=c=black:s=320x240:r=30:d=6,format=yuv420p,geq=lum='16+30*T':cb=128:cr=128", 6);
    let src = look(&ffmpeg, &clip).expect("ffmpeg reads the clip");
    assert_eq!((src.width, src.height, src.fps), (320, 240, (30, 1)));
    // The red channel of a grey picture: limited-range luma made full range.
    let brightness = |f: &crate::media::VideoFrame| f.rgba.chunks_exact(4).map(|p| p[0] as f64).sum::<f64>() / (f.rgba.len() / 4) as f64;
    let grey_at = |t: f64| ((30.0 * t) * 255.0 / 219.0).clamp(0.0, 255.0);
    assert_eq!(src.summary.audio_codec.as_deref(), Some("ac3"));
    let mut player = crate::media::VideoPlayer::open_stream(src.clone()).expect("opens");
    assert!(player.info().stream && player.info().audio_track.is_some());
    assert!((player.duration_s() - 6.0).abs() < 0.2, "{}", player.duration_s());
    let wait_frame = |player: &mut crate::media::VideoPlayer| {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            if let Some(f) = player.poll() {
                return f;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        panic!("no picture within 30 s: {:?}", player.take_error());
    };
    let first = wait_frame(&mut player);
    assert_eq!((first.width, first.height), (320, 240));
    assert!(first.pts_s.abs() < 1e-9, "the first picture is at the start: {}", first.pts_s);
    assert!(brightness(&first) < 8.0, "the clip starts black: {}", brightness(&first));
    player.seek_to(4.0);
    let landed = wait_frame(&mut player);
    assert!((landed.pts_s - 4.0).abs() < 0.05, "the seek landed at {}", landed.pts_s);
    // Within five pictures of 4 s by what it SHOWS (each picture is one step of 1.16 brighter).
    let (got, want) = (brightness(&landed), grey_at(4.0));
    assert!((got - want).abs() < 6.0, "the picture at 4 s is {got:.1} bright, {want:.1} expected");

    // The sound: ffmpeg's 48 kHz stereo floats carry the tone, at the start and after a seek.
    let mut sound = PipeAudio::new(Arc::new(src));
    use kira::sound::streaming::Decoder;
    assert_eq!(sound.sample_rate(), 48_000);
    assert_eq!(sound.num_frames(), 6 * 48_000);
    assert_eq!(sound.seek(0).unwrap(), 0, "kira's first seek starts nothing");
    let mut got = Vec::new();
    while got.len() < 9600 {
        got.extend(sound.decode().unwrap());
    }
    let peak = got[4800..9600].iter().map(|f| f.left.abs().max(f.right.abs())).fold(0.0f32, f32::max);
    assert!(peak > 0.05, "the tone is heard, peak {peak}");
    assert_eq!(sound.seek(48_000 * 5).unwrap(), 48_000 * 5);
    let after: Vec<Frame> = (0..4).flat_map(|_| sound.decode().unwrap()).collect();
    assert!(after.iter().any(|f| f.left.abs() > 0.05), "sound after a seek");
    drop(sound);
    let alive = player.decode_thread_alive_flag();
    drop(player);
    assert!(!alive.load(Ordering::SeqCst), "the decode thread was joined");
}

/// A file that is not media: the look says so in ffmpeg's words, it never panics.
#[test]
fn a_file_that_is_not_media_is_refused_in_words() {
    let Some(ffmpeg) = find_ffmpeg("") else {
        println!("skipped: no ffmpeg on this machine");
        return;
    };
    let dir = scratch("notmedia");
    let f = dir.join("notes.bin");
    std::fs::write(&f, b"these bytes are no film").unwrap();
    let err = look(&ffmpeg, &f).unwrap_err();
    assert!(err.starts_with("ffmpeg found no picture in it"), "{err}");
}

/// The measurement behind media-player.md's "Through ffmpeg as it plays" table (not a gate):
/// `HUMANITY_STREAM_BENCH_FILE=<a video or a disc's VIDEO_TS folder> cargo test ... -- --ignored
/// --nocapture bench_first_picture` prints how long the look, the first picture and a seek to the
/// middle take, and how fast pictures come once it plays. Skipped without the variable or ffmpeg.
#[test]
#[ignore]
fn bench_first_picture() {
    let Some(path) = std::env::var_os("HUMANITY_STREAM_BENCH_FILE").map(PathBuf::from) else {
        println!("skipped: set HUMANITY_STREAM_BENCH_FILE");
        return;
    };
    let Some(ffmpeg) = find_ffmpeg("") else {
        println!("skipped: no ffmpeg on this machine");
        return;
    };
    let input = if path.is_dir() {
        let video_ts = crate::media::dvd::find_video_ts(&path).expect("a disc folder");
        let title = crate::media::dvd::main_title(&crate::media::dvd::list_video_ts(&video_ts)).expect("a title");
        crate::media::ffmpeg::concat_input(&title.parts.iter().map(|n| video_ts.join(n)).collect::<Vec<_>>())
    } else {
        path
    };
    let t0 = Instant::now();
    let src = look(&ffmpeg, &input).expect("ffmpeg reads it");
    let looked = t0.elapsed();
    let duration = src.summary.duration_s.unwrap_or(0.0);
    let (w, h) = (src.width, src.height);
    let mut player = crate::media::VideoPlayer::open_stream(src).expect("opens");
    let wait = |player: &mut crate::media::VideoPlayer| {
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if let Some(f) = player.poll() {
                return f;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        panic!("no picture within 60 s");
    };
    wait(&mut player);
    let first = t0.elapsed();
    let t1 = Instant::now();
    player.seek_to(duration / 2.0);
    wait(&mut player);
    let seek = t1.elapsed();
    // Pictures a second once rolling: drain the queue as fast as it fills for three seconds.
    player.play();
    let t2 = Instant::now();
    let mut pictures = 0u32;
    let mut last = -1.0;
    while t2.elapsed() < Duration::from_secs(3) {
        if let Some(f) = player.poll() {
            if f.pts_s > last {
                pictures += 1;
                last = f.pts_s;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    println!(
        "BENCH {}: {w}x{h}, {duration:.1} s long; look {:.3} s, first picture {:.3} s after the open began, seek to the middle {:.3} s, {:.1} pictures/s shown while playing",
        input.display(),
        looked.as_secs_f64(),
        first.as_secs_f64(),
        seek.as_secs_f64(),
        pictures as f64 / 3.0
    );
}

/// THE SOUND THROUGH FFMPEG PLAYS THROUGH KIRA AND LEADS THE PICTURE, on a machine with a sound
/// device, muted (this is about timing, not the speakers): attached while paused the clock stands
/// still; played, the clock follows the sound to about 1.2 s after 1.2 s (ffmpeg's start included),
/// and no picture runs ahead of it. Ignored because CI has no sound device; run by hand after
/// touching `PipeAudio`: `node scripts/lib-tests.js stream::tests::the_sound --ignored --nocapture`.
#[test]
#[ignore = "needs an audio output device and ffmpeg"]
fn the_sound_through_ffmpeg_plays_through_kira_and_leads_the_picture() {
    let Some(ffmpeg) = find_ffmpeg("") else {
        println!("skipped: no ffmpeg on this machine");
        return;
    };
    let mut audio = match humanity_core::audio::AudioManager::try_new() {
        Ok(a) => a,
        Err(e) => {
            println!("skipped: no audio device ({e})");
            return;
        }
    };
    audio.set_master_volume(0.0);
    let dir = scratch("kira");
    let clip = make_clip_of(&ffmpeg, &dir, "testsrc2=size=320x180:rate=30:duration=4", 4);
    let src = look(&ffmpeg, &clip).expect("ffmpeg reads the clip");
    let mut player = crate::media::VideoPlayer::open_stream(src).expect("open");
    player.attach_audio(&mut audio).expect("attach ffmpeg's sound to kira");
    assert!(player.has_audio());
    std::thread::sleep(Duration::from_millis(300));
    assert!(player.position_s() < 0.01, "paused with sound attached: {}", player.position_s());
    player.play();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1200) {
        if let Some(f) = player.poll() {
            assert!(f.pts_s <= player.position_s() + 1.0e-9 || f.pts_s < 0.05, "picture at {} ahead of the clock {}", f.pts_s, player.position_s());
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    let at = player.position_s();
    assert!((at - 1.2).abs() < 0.3, "the sound-led clock near 1.2 s after 1.2 s of play: {at}");
    assert!(player.audio_position_s().is_some_and(|s| s > 0.5), "kira's own position moved: {:?}", player.audio_position_s());
}
