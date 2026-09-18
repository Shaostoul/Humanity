//! Tests for the media player core. They use two committed fixtures made by
//! `scripts/make-media-fixtures.sh` from SYNTHETIC lavfi sources (nothing
//! recorded, nothing downloaded): a 2 s AV1 + Opus clip in which a 40 px red
//! bar slides right at 140 px/s over dark grey with a 440 Hz tone, and a
//! 0.2 s VP8 + Vorbis clip the player must refuse.
//!
//! Every assertion here is about the DECODED RESULT (pixels, samples, pts,
//! thread state), never about the setup that produced it. The live tests are
//! timing-based: each runs about 2 s alone, and the playback test runs until
//! the clip ends rather than for a fixed window, so a loaded machine makes it
//! slower, not red (see `real_time_floor` for what a count can and cannot
//! prove under load).

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use kira::sound::streaming::Decoder as _;
use kira::sound::PlaybackState;

use super::audio::OpusTrack;
use super::video::decode_video;
use super::{probe, take_due_frame, AudioAttach, MediaError, VideoFrame, VideoPlayer};

const BAR: &str = "colour-bar-av1-opus.webm";
const UNSUPPORTED: &str = "unsupported-vp8-vorbis.webm";
/// The fixture's declared duration: ffmpeg pads the Opus stream to 2.008 s.
const BAR_DURATION_S: f64 = 2.008;
/// Frames in the fixture: 2 s at 30 fps.
const BAR_FRAMES: usize = 60;
/// The bar's speed in the recipe (`overlay=x='mod(t*140,320)'`).
const BAR_SPEED_PX_PER_S: f64 = 140.0;
/// libopus look-ahead, as written in the fixture's OpusHead.
const PRE_SKIP: usize = 312;

/// The floor on how many of `frames_due` a real-time polling loop must
/// receive: half of them. Why half and not all: `poll()` hands out the
/// NEWEST due frame and drops the older due ones (the caller wants the
/// present, not a backlog), so every time the test thread is off the CPU
/// for longer than one frame period (33 ms, ten times its 3 ms sleep) a
/// frame is skipped BY DESIGN. Under machine load that is common: a run on
/// a busy desktop delivered 37 of 59 against a fixed threshold of 40 and
/// failed three times in a row while the same code passed alone. Half rate
/// means the loop was away no more than one frame in two on average, which
/// a loaded desktop still manages, while every defect this count exists to
/// catch delivers a handful at most: a decode thread that never wakes once
/// the four-frame queue fills (at most 4), frames dropped as a stale
/// generation (0 after the drop), a decoder that stalls on the caller. The
/// real-time proof is carried by the OTHER assertions in each live test
/// (the last frame reached the caller, the clock stopped at the declared
/// end, or the clip wrapped): this floor only has to tell "frames flow"
/// from "frames stopped", and half is far from both.
fn real_time_floor(frames_due: usize) -> usize {
    frames_due / 2
}

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

    // `start` is taken BEFORE play() so the wall reading below can never
    // be behind the clock's own anchor.
    let start = Instant::now();
    player.play();
    let mut last_pts = first.pts_s;
    let mut delivered = 0;
    // The loop's own cadence, so a failure message can say whether the
    // machine or the player was late: the longest gap between two polls.
    let mut polls = 0u32;
    let mut last_poll = Instant::now();
    let mut longest_gap = Duration::ZERO;
    // Run until the clock STOPS at the declared end, which the player only
    // does once the decoder has delivered its last frame, with a deadline
    // far beyond real time so a loaded machine finishes late instead of
    // failing early. A fixed 2.6 s window used to sit here; with the rest
    // of the suite sharing the machine a debug-build decoder ran at about
    // 15 fps and the window closed with the last frame at 1.233 s, which is
    // no defect of the player (the clock is the master, frames follow it,
    // and a slower decoder means dropped frames, not a wrong result).
    let deadline = start + Duration::from_secs(15);
    while player.is_playing() {
        assert!(
            Instant::now() < deadline,
            "the clip did not finish within 15 s: {delivered} frames delivered, last pts {last_pts}"
        );
        let now = Instant::now();
        longest_gap = longest_gap.max(now - last_poll);
        last_poll = now;
        polls += 1;
        if let Some(f) = player.poll() {
            let clock = player.position_s();
            // The two properties that hold on EVERY delivered frame whatever
            // the machine is doing: never ahead of the clock, never out of
            // order. These are the gate; the count below is only a floor.
            assert!(f.pts_s <= clock + 1e-9, "frame at {} s handed out when the clock read {clock} s", f.pts_s);
            assert!(f.pts_s > last_pts, "out of order: {} after {last_pts}", f.pts_s);
            last_pts = f.pts_s;
            delivered += 1;
        }
        // The REAL-TIME proof, on the clock and not on the decoder: the
        // position tracks the wall whatever the machine is doing. The
        // position is read FIRST and the wall second, so a preemption
        // between the two reads can only make the wall larger; the clock
        // may therefore never read ahead of the wall, and it may lag it by
        // no more than a preemption's worth (0.3 s is generous; a clock
        // running at half speed would be a second behind by the end).
        let pos = player.position_s();
        let wall = start.elapsed().as_secs_f64().min(player.duration_s());
        assert!(pos <= wall + 1e-3, "the clock ran ahead of the wall: {pos} s at {wall} s");
        assert!(wall - pos < 0.3, "the clock fell behind the wall: {pos} s at {wall} s");
        std::thread::sleep(Duration::from_millis(3));
    }
    let elapsed = start.elapsed().as_secs_f64();
    assert!(
        elapsed >= BAR_DURATION_S - 0.05,
        "a wall-driven clock cannot reach the end early: finished after {elapsed:.3} s"
    );
    let floor = real_time_floor(BAR_FRAMES - 1);
    assert!(
        delivered >= floor,
        "frames stopped flowing: {delivered} of {} delivered (floor {floor}; the loop polled {polls} times over \
         {elapsed:.2} s, longest gap between polls {:.1} ms)",
        BAR_FRAMES - 1,
        longest_gap.as_secs_f64() * 1e3
    );
    println!("live playback: {delivered} of 59 frames, finished after {elapsed:.3} s, longest poll gap {:.1} ms", longest_gap.as_secs_f64() * 1e3);
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

/// The kira hookup end to end, on a machine with an audio device: attach the
/// Opus track, play muted, and check that the AUDIO-LED clock advances in
/// real time, frames still never run ahead of it, pause holds it, and a
/// rewind restarts both. Ignored because CI has no audio device; run it by
/// hand after touching attach_audio / sync_clock_to_audio / play_stream.
#[test]
#[ignore = "needs an audio output device; run: cargo test --features native --lib media::tests::audio_led -- --ignored --nocapture"]
fn audio_led_clock_follows_kira_when_a_device_exists() {
    let mut audio = match crate::audio::AudioManager::try_new() {
        Ok(a) => a,
        Err(e) => {
            println!("skipped: no audio device ({e})");
            return;
        }
    };
    // Muted: this is about timing, not the speakers. kira's transport still
    // advances position() at zero amplitude.
    audio.set_master_volume(0.0);

    let mut player = VideoPlayer::open_with_threads(fixture(BAR), 1).expect("open");
    player.attach_audio(&mut audio).expect("attach the Opus track to kira");
    assert!(player.has_audio());

    // Attached while paused: the clock must not move.
    std::thread::sleep(Duration::from_millis(200));
    assert!(player.position_s() < 0.01, "paused with audio attached, position {}", player.position_s());

    player.play();
    let start = Instant::now();
    let mut delivered = 0;
    let mut last_pts = -1.0;
    while start.elapsed() < Duration::from_millis(1500) {
        if let Some(f) = player.poll() {
            let clock = player.position_s();
            assert!(f.pts_s <= clock + 1e-9, "frame at {} s ahead of the audio-led clock {clock}", f.pts_s);
            assert!(f.pts_s > last_pts, "out of order: {} after {last_pts}", f.pts_s);
            last_pts = f.pts_s;
            delivered += 1;
        }
        std::thread::sleep(Duration::from_millis(3));
    }
    let pos = player.position_s();
    assert!((pos - 1.5).abs() < 0.25, "the audio-led clock should sit near 1.5 s after 1.5 s of play, got {pos}");
    assert!(delivered >= 30, "frames should keep flowing under the audio clock, got {delivered}");

    player.pause();
    let held = player.position_s();
    std::thread::sleep(Duration::from_millis(200));
    assert!((player.position_s() - held).abs() < 1e-9, "a paused clock holds still");

    player.seek_to_start();
    assert!(player.position_s() < 0.05, "rewound, got {}", player.position_s());
    player.play();
    let f = wait_for_frame(&mut player, Duration::from_secs(2)).expect("frames after a rewind with audio attached");
    assert!(f.pts_s < 0.3, "first frame after rewind at {}", f.pts_s);
    std::thread::sleep(Duration::from_millis(300));
    let after = player.position_s();
    assert!(after > 0.2 && after < 0.8, "the clock runs again after the rewind, got {after}");
    println!("audio-led: {delivered} frames in 1.5 s, clock {pos:.3} s, after rewind {after:.3} s");
}

/// The LOOPING attach (`AudioAttach { looping: true }`, the `loop_region`
/// branch), on a machine with an audio device. This is the path every
/// in-world video screen takes, and until this test nothing executed it:
/// the device test above goes through the one-shot attach.
///
/// Phase A lets the stream run off its end with NO help from the caller and
/// proves what the loop region is for: kira's own position wraps back toward
/// zero and the sound is still `Playing` afterwards. Phase B then drives the
/// clip the way the screen provider does (rewind the picture when the clock
/// reaches the declared end) through a second full pass, so the run goes
/// past `duration_s` at least twice, and the sound is still `Playing` at the
/// end with no error raised. Phase C is the control that proves the test
/// can tell the difference: the same clip attached WITHOUT looping reads a
/// state other than `Playing` once it has run off its end.
///
/// Run it in RELEASE mode: rav1d's own debug-only `DisjointMut` borrow
/// checker can panic on one of rav1d's worker threads under a debug build
/// with several decoders running (observed once in 16 debug runs of the
/// suite, 2026-09-17; it aborts the whole test process), and this test keeps
/// a decoder alive for two full passes. See "Known limits" in
/// docs/design/media-player.md. A debug run that completes is still valid.
#[test]
#[ignore = "needs an audio output device; run: cargo test --release --features native --lib -- --ignored --nocapture media::tests::looping_audio (release: rav1d's debug-only borrow checker can abort a debug run, see docs/design/media-player.md Known limits)"]
fn looping_audio_keeps_playing_past_the_end_when_a_device_exists() {
    let mut audio = match crate::audio::AudioManager::try_new() {
        Ok(a) => a,
        Err(e) => {
            println!("skipped: no audio device ({e})");
            return;
        }
    };
    // Muted at the master AND at the attach: this is about the transport,
    // not the speakers.
    audio.set_master_volume(0.0);

    let mut player = VideoPlayer::open_with_threads(fixture(BAR), 1).expect("open");
    player
        .attach_audio_with(&mut audio, AudioAttach { looping: true, volume: 0.0, panning: 0.5 })
        .expect("attach the Opus track looping");
    assert!(player.has_audio());
    // kira applies the pause on its audio thread; give it a moment, then
    // the sound must be paused (nothing leaks out before the first play).
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(player.audio_state(), Some(PlaybackState::Paused), "attached while the player is paused");
    let dur = player.duration_s();

    // Phase A: run past the end on the stream's own; watch kira's position.
    player.play();
    let start = Instant::now();
    let mut prev_audio = 0.0;
    let mut audio_wraps = 0;
    let mut max_audio = 0.0f64;
    let mut frames_a = 0;
    while start.elapsed().as_secs_f64() < dur + 0.4 {
        if player.poll().is_some() {
            frames_a += 1;
        }
        let ap = player.audio_position_s().expect("a sound is attached");
        // A drop of more than half a second is a wrap, not jitter (kira
        // applies commands asynchronously, so tiny backward steps happen).
        if ap < prev_audio - 0.5 {
            audio_wraps += 1;
        }
        prev_audio = ap;
        max_audio = max_audio.max(ap);
        std::thread::sleep(Duration::from_millis(3));
    }
    let state_a = player.audio_state();
    assert!(player.at_end(), "the picture clock reached the declared end, position {}", player.position_s());
    assert!(
        audio_wraps >= 1,
        "kira's position never dropped back: the loop region did not wrap (max position {max_audio:.3} s of {dur} s)"
    );
    assert_eq!(state_a, Some(PlaybackState::Playing), "a looping stream must never finish; it read {state_a:?}");
    assert!(frames_a >= real_time_floor(BAR_FRAMES), "frames flowed during the first pass, got {frames_a}");
    assert!(player.take_error().is_none());

    // Phase B: the provider's loop rule, a second full pass.
    let mut picture_loops = 0;
    let mut frames_b = 0;
    let mut last_pts = -1.0;
    let mut second_pass_frames = 0;
    let start_b = Instant::now();
    while start_b.elapsed().as_secs_f64() < dur + 0.4 {
        if player.at_end() {
            player.seek_to_start();
            player.play();
            picture_loops += 1;
            last_pts = -1.0;
        }
        if let Some(f) = player.poll() {
            assert!(f.pts_s <= player.position_s() + 1e-9, "frame {} ahead of the clock", f.pts_s);
            assert!(f.pts_s > last_pts, "out of order after a wrap: {} after {last_pts}", f.pts_s);
            last_pts = f.pts_s;
            frames_b += 1;
            if picture_loops >= 1 {
                second_pass_frames += 1;
            }
        }
        assert_eq!(player.audio_state(), Some(PlaybackState::Playing), "the sound must stay attached and playing through a rewind");
        std::thread::sleep(Duration::from_millis(3));
    }
    assert!(picture_loops >= 1, "the picture wrapped at least once in phase B");
    assert!(
        second_pass_frames >= real_time_floor(BAR_FRAMES),
        "frames must keep flowing after the wrap, got {second_pass_frames}"
    );
    assert_eq!(player.audio_state(), Some(PlaybackState::Playing));
    assert!(player.take_error().is_none(), "no decode or audio error across two passes");
    let total_s = start.elapsed().as_secs_f64();
    assert!(total_s > 2.0 * dur, "the run must go past the duration twice, ran {total_s:.2} s");

    // Phase C: the control. Without the loop region the same stream is done
    // once it runs off its end, and the test would say so.
    let mut oneshot = VideoPlayer::open_with_threads(fixture(BAR), 1).expect("open");
    oneshot
        .attach_audio_with(&mut audio, AudioAttach { looping: false, volume: 0.0, panning: 0.5 })
        .expect("attach one-shot");
    oneshot.play();
    let start_c = Instant::now();
    while start_c.elapsed().as_secs_f64() < dur + 0.4 {
        let _ = oneshot.poll();
        std::thread::sleep(Duration::from_millis(3));
    }
    let state_c = oneshot.audio_state();
    assert_ne!(state_c, Some(PlaybackState::Playing), "a one-shot stream past its end is no longer playing (control)");

    println!(
        "looping audio: phase A {frames_a} frames, {audio_wraps} audio wrap(s), max kira position {max_audio:.3} s of {dur} s, state {state_a:?}; \
         phase B {frames_b} frames, {picture_loops} picture loop(s), {second_pass_frames} after the wrap; \
         total {total_s:.2} s; control one-shot state after its end {state_c:?}"
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
