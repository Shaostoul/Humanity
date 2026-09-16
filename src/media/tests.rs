//! Tests for the media player core. They use two committed fixtures made by
//! `scripts/make-media-fixtures.sh` from SYNTHETIC lavfi sources (nothing
//! recorded, nothing downloaded): a 2 s AV1 + Opus clip in which a 40 px red
//! bar slides right at 140 px/s over dark grey with a 440 Hz tone, and a
//! 0.2 s VP8 + Vorbis clip the player must refuse.
//!
//! Every assertion here is about the DECODED RESULT (pixels, samples, pts,
//! thread state), never about the setup that produced it. The live tests are
//! timing-based with wide margins; they take about 2.5 s each.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use kira::sound::streaming::Decoder as _;

use super::audio::OpusTrack;
use super::video::decode_video;
use super::{probe, take_due_frame, MediaError, VideoFrame, VideoPlayer};

const BAR: &str = "colour-bar-av1-opus.webm";
const UNSUPPORTED: &str = "unsupported-vp8-vorbis.webm";
/// The fixture's declared duration: ffmpeg pads the Opus stream to 2.008 s.
const BAR_DURATION_S: f64 = 2.008;
/// The bar's speed in the recipe (`overlay=x='mod(t*140,320)'`).
const BAR_SPEED_PX_PER_S: f64 = 140.0;
/// libopus look-ahead, as written in the fixture's OpusHead.
const PRE_SKIP: usize = 312;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("media")
        .join(name)
}

#[test]
fn probe_reports_tracks_codecs_and_duration() {
    let info = probe(&fixture(BAR)).expect("probe");
    assert_eq!(info.track_count, 2);
    assert_eq!(info.video_codec, "V_AV1");
    assert_eq!(info.audio_codec.as_deref(), Some("A_OPUS"));
    assert_eq!((info.width, info.height), (320, 180));
    assert_eq!(info.sample_rate, 48_000);
    assert_eq!(info.channels, 2);
    assert_eq!(info.pre_skip as usize, PRE_SKIP);
    assert!(
        (info.duration_s - BAR_DURATION_S).abs() < 0.01,
        "duration {} s, expected {BAR_DURATION_S}",
        info.duration_s
    );
}

#[test]
fn refuses_unsupported_codec_naming_it() {
    let err = match VideoPlayer::open(fixture(UNSUPPORTED)) {
        Ok(_) => panic!("a VP8 file must be refused"),
        Err(e) => e,
    };
    match &err {
        MediaError::UnsupportedCodec { kind, codec_id } => {
            assert_eq!(*kind, "video");
            assert_eq!(codec_id, "V_VP8");
        }
        other => panic!("wrong error kind: {other:?}"),
    }
    let text = err.to_string();
    assert!(
        text.contains("V_VP8") && text.contains("AV1") && text.contains("Opus"),
        "the message must name the codec and the fix: {text}"
    );
}

/// Left edge of the red bar on the middle row, in pixels, or None.
fn bar_left_edge(f: &VideoFrame) -> Option<usize> {
    let row = (f.height / 2) as usize * f.width as usize * 4;
    (0..f.width as usize).find(|&x| {
        let p = &f.rgba[row + x * 4..row + x * 4 + 4];
        p[0] > 150 && p[1] < 90 && p[2] < 90
    })
}

#[test]
fn decodes_every_frame_in_order_at_the_right_size_with_the_bar_moving() {
    let mut frames = Vec::new();
    let n = decode_video(&fixture(BAR), 1, |f| frames.push(f)).expect("decode");
    assert_eq!(n, 60, "2 s at 30 fps");
    assert_eq!(frames.len(), 60);
    let mut last = -1.0;
    for (i, f) in frames.iter().enumerate() {
        assert_eq!((f.width, f.height), (320, 180));
        assert_eq!(f.rgba.len(), 320 * 180 * 4);
        assert!(f.pts_s > last, "pts must strictly increase: frame {i} pts {} after {last}", f.pts_s);
        last = f.pts_s;

        // The picture itself: the bar's left edge sits where the recipe put
        // it for this pts. A decoder that returned garbage, the wrong frame,
        // or a wrong colour matrix would fail here, not just a size check.
        let expected = (f.pts_s * BAR_SPEED_PX_PER_S).round() as i64;
        let left = bar_left_edge(f).unwrap_or_else(|| panic!("frame {i}: no red bar found")) as i64;
        assert!(
            (left - expected).abs() <= 3,
            "frame {i} at {} s: bar left edge {left}, expected {expected}",
            f.pts_s
        );
        // The field is neutral dark grey (0x202020): a wrong range or matrix
        // would tint it or lift it.
        let mid_row = (f.height / 2) as usize * f.width as usize * 4;
        let bg_x = (left as usize + 200) % 320;
        let bg = &f.rgba[mid_row + bg_x * 4..mid_row + bg_x * 4 + 4];
        assert!(
            bg[0] < 64 && bg[1] < 64 && bg[2] < 64 && (bg[0] as i32 - bg[2] as i32).abs() < 12,
            "frame {i}: background {bg:?} is not neutral dark grey"
        );
        assert_eq!(bg[3], 255, "opaque alpha");
        // Inside the bar it is red, not merely reddish.
        let in_bar = (left as usize + 20).min(319);
        let px = &f.rgba[mid_row + in_bar * 4..mid_row + in_bar * 4 + 4];
        assert!(px[0] > 200 && px[1] < 60 && px[2] < 60, "frame {i}: bar pixel {px:?} is not red");
    }
    assert!(frames[0].pts_s.abs() < 1e-9, "first frame at 0, got {}", frames[0].pts_s);
    assert!((last - 59.0 / 30.0).abs() < 0.01, "last frame at 1.967 s, got {last}");
}

#[test]
fn opus_sample_count_matches_duration_within_5_percent() {
    let info = probe(&fixture(BAR)).unwrap();
    let (pcm, channels, rate) = OpusTrack::decode_all_pcm(&fixture(BAR), &info).expect("opus");
    assert_eq!((channels, rate), (2, 48_000));
    let expected = info.duration_s * rate as f64 * channels as f64;
    let ratio = pcm.len() as f64 / expected;
    assert!(
        (ratio - 1.0).abs() < 0.05,
        "{} interleaved samples vs {expected:.0} expected (ratio {ratio:.3})",
        pcm.len()
    );
    // A 440 Hz tone, not silence and not clipping.
    let rms = (pcm.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / pcm.len() as f64).sqrt();
    assert!(rms > 0.02 && rms < 0.9, "rms {rms:.3}");
    // `-ac 2` duplicated a mono source, so left and right must match.
    let diff = pcm.chunks_exact(2).map(|p| (p[0] - p[1]).abs() as f64).sum::<f64>() / (pcm.len() / 2) as f64;
    assert!(diff < 1e-3, "channels should carry the same signal, mean |L-R| {diff}");
}

#[test]
fn opus_track_feeds_kira_in_order_and_rewinds_sample_exactly() {
    let info = probe(&fixture(BAR)).unwrap();
    let mut track = OpusTrack::open(&fixture(BAR), &info).unwrap();
    assert_eq!(track.sample_rate(), 48_000);
    let expected_frames = (info.duration_s * 48_000.0).floor() as usize - PRE_SKIP;
    assert_eq!(track.num_frames(), expected_frames);

    let first = track.decode().unwrap();
    assert_eq!(first.len(), 960 - PRE_SKIP, "first 20 ms packet minus the pre-skip");
    let mut total = first.len();
    let mut chunks = 1;
    while total < track.num_frames() {
        let chunk = track.decode().unwrap();
        assert!(!chunk.is_empty(), "kira's scheduler must always get frames back");
        total += chunk.len();
        chunks += 1;
    }
    assert!(chunks >= 100, "one chunk per 20 ms packet, got {chunks}");

    // Rewind: index 0 reached exactly, and the samples after it are the
    // same as the first pass (a fresh decoder state, not a mid-stream one).
    assert_eq!(track.seek(0).unwrap(), 0);
    let again = track.decode().unwrap();
    assert_eq!(again.len(), first.len());
    assert!(
        again.iter().zip(&first).all(|(a, b)| a.left == b.left && a.right == b.right),
        "a rewind must reproduce the first chunk sample for sample"
    );

    // Seeking into the stream lands on a packet boundary at or before the
    // target and keeps the straddling packet for the next decode().
    let reached = track.seek(10_000).unwrap();
    assert_eq!(reached, (960 - PRE_SKIP) + 960 * 9, "packet-aligned index at or before 10000");
    let next = track.decode().unwrap();
    assert_eq!(next.len(), 960);
}

#[test]
fn take_due_frame_never_hands_out_a_frame_ahead_of_the_clock() {
    let frame = |pts: f64| VideoFrame { rgba: Vec::new(), width: 0, height: 0, pts_s: pts };
    let mut q: VecDeque<VideoFrame> = [0.0, 0.1, 0.2, 0.3].into_iter().map(frame).collect();
    assert!(take_due_frame(&mut q, -0.01).is_none(), "nothing is due before the first pts");
    assert_eq!(q.len(), 4);
    // At 0.25 the newest at-or-before frame is 0.2; 0.0 and 0.1 are dropped
    // as stale; 0.3 stays because it is ahead of the clock.
    assert_eq!(take_due_frame(&mut q, 0.25).map(|f| f.pts_s), Some(0.2));
    assert_eq!(q.len(), 1);
    assert!(take_due_frame(&mut q, 0.25).is_none(), "already delivered");
    assert!(take_due_frame(&mut q, 0.2999).is_none(), "0.3 is ahead of the clock");
    assert_eq!(take_due_frame(&mut q, 0.3).map(|f| f.pts_s), Some(0.3));
    assert!(q.is_empty());
}

/// Poll until a frame arrives or `timeout` passes (the decode thread needs a
/// moment to open the file and produce its first picture).
fn wait_for_frame(player: &mut VideoPlayer, timeout: Duration) -> Option<VideoFrame> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(f) = player.poll() {
            return Some(f);
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    None
}

#[test]
fn live_playback_delivers_frames_in_order_and_never_ahead_of_the_clock() {
    let mut player = VideoPlayer::open_with_threads(fixture(BAR), 1).expect("open");
    assert!(!player.is_playing());
    assert_eq!(player.position_s(), 0.0);
    assert!((player.duration_s() - BAR_DURATION_S).abs() < 0.01);

    // Paused at 0: exactly one frame (pts 0) is ever due, however long we wait.
    let first = wait_for_frame(&mut player, Duration::from_secs(5)).expect("frame 0 is due at position 0");
    assert_eq!(first.pts_s, 0.0);
    std::thread::sleep(Duration::from_millis(100));
    assert!(player.poll().is_none(), "paused clock: nothing new is due");

    player.play();
    let start = Instant::now();
    let mut last_pts = first.pts_s;
    let mut delivered = 0;
    while start.elapsed() < Duration::from_millis(2600) {
        if let Some(f) = player.poll() {
            let clock = player.position_s();
            assert!(f.pts_s <= clock + 1e-9, "frame at {} s handed out when the clock read {clock} s", f.pts_s);
            assert!(f.pts_s > last_pts, "out of order: {} after {last_pts}", f.pts_s);
            last_pts = f.pts_s;
            delivered += 1;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(delivered >= 40, "most of the 59 remaining frames should arrive in real time, got {delivered}");
    // WebM stores pts in millisecond ticks, so frame 59 sits at 1.967 s, not 59/30.
    assert!((last_pts - 59.0 / 30.0).abs() < 0.002, "the last frame reached the caller, last pts {last_pts}");
    assert!(player.at_end(), "position {} of {}", player.position_s(), player.duration_s());
    assert!(!player.is_playing(), "the clock stops at the declared end");
    assert!((player.position_s() - BAR_DURATION_S).abs() < 0.01);
    assert!(player.take_error().is_none());
}

#[test]
fn seek_to_start_rewinds_and_replays() {
    let mut player = VideoPlayer::open_with_threads(fixture(BAR), 1).unwrap();
    player.play();
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut reached = None;
    while Instant::now() < deadline {
        if let Some(f) = player.poll() {
            if f.pts_s >= 0.4 {
                reached = Some(f.pts_s);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(reached.is_some(), "never got past 0.4 s");

    player.seek_to_start();
    assert!(player.is_playing(), "a seek keeps the play state");
    assert!(player.position_s() < 0.05, "clock back near zero, got {}", player.position_s());
    let after = wait_for_frame(&mut player, Duration::from_secs(3)).expect("frames resume after a rewind");
    assert!(after.pts_s < 0.3, "the first frame after a rewind is near the start, got {}", after.pts_s);
    assert!(player.take_error().is_none());
}

#[test]
fn drop_joins_the_decode_thread() {
    let player = VideoPlayer::open_with_threads(fixture(BAR), 1).unwrap();
    let alive = player.decode_thread_alive_flag();
    std::thread::sleep(Duration::from_millis(100));
    assert!(alive.load(Ordering::SeqCst), "the decode thread runs (parked on a full queue) while the player lives");
    drop(player);
    assert!(
        !alive.load(Ordering::SeqCst),
        "Drop must join the decode thread; a leaked thread would still be alive here"
    );
}

/// Measured, not guessed: frames per second of the full pipeline (demux,
/// AV1 decode, RGBA conversion) on this machine. Set
/// HUMANITY_MEDIA_BENCH_FILE to a larger AV1 WebM (a 1080p one, say) to
/// measure that instead of the 320x180 fixture. Numbers and method are
/// recorded in docs/design/media-player.md.
#[test]
#[ignore = "release-mode benchmark; run: cargo test --release --features native --lib media::tests::bench -- --ignored --nocapture"]
fn bench_decode_fps() {
    let path = std::env::var_os("HUMANITY_MEDIA_BENCH_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| fixture(BAR));
    let info = probe(&path).expect("probe");
    println!(
        "bench file: {} ({}x{}, {:.2} s, {} logical processors)",
        path.display(),
        info.width,
        info.height,
        info.duration_s,
        std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0)
    );
    for threads in [1, 0] {
        let t0 = Instant::now();
        let mut pixels = 0u64;
        let n = decode_video(&path, threads, |f| pixels += (f.width * f.height) as u64).expect("decode");
        let s = t0.elapsed().as_secs_f64();
        let label = if threads == 0 { "auto threads".to_string() } else { format!("{threads} thread") };
        println!(
            "{label}: {n} frames in {s:.3} s = {:.1} fps ({:.1} Mpx/s), RGBA conversion included",
            n as f64 / s,
            pixels as f64 / s / 1e6
        );
    }
}
