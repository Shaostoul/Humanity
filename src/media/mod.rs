//! Purpose-built media player core (in-world screens ladder, rung 5).
//!
//! What this is: open a WebM/Matroska file, decode its AV1 video to RGBA
//! frames and its Opus audio to PCM, with a playback clock, play / pause and
//! seek-to-start. Decoding runs on a background thread; the caller (a later
//! rung: the in-world screen surface) polls for the frame that is due.
//!
//! What this is NOT: there is no window, no UI and no texture upload here.
//! The surface that draws the frames is built separately and connected later.
//!
//! Why purpose-built instead of embedding VLC: libvlc is LGPL with a C
//! runtime, brings its own window and render path, and would have to ship as
//! DLLs next to the exe. We need decoded pixels in OUR texture on an in-game
//! monitor, so a decoder core we own is the shorter road. Full reasoning,
//! licence audit and measurements: `docs/design/media-player.md`.
//!
//! Codecs: AV1 (rav1d, pure-Rust build) and Opus (unsafe-libopus). Both are
//! royalty-free. Anything else is refused at `open()` with an error naming
//! the codec; other formats get transcoded on ingest (a later rung), never
//! decoded here.
//!
//! ## Thread and clock model
//!
//! ```text
//! decode thread (ours)                        caller thread
//! --------------------                        -------------
//! demux -> rav1d -> RGBA -> bounded queue     poll(): newest frame with
//!          (waits while the queue is full)     pts <= clock; older due
//!                                              frames are dropped
//!
//! kira decode thread (kira's own)             kira audio thread
//! ------------------------------              -----------------
//! demux -> Opus -> Frame chunks               mixes, advances position()
//! ```
//!
//! The clock is monotonic. With audio attached and playing it is AUDIO-LED:
//! `poll()` re-bases the clock on kira's reported playback position, so the
//! picture follows the sound card rather than the other way round (an ear
//! notices a 20 ms audio glitch; an eye forgives a repeated frame). Without
//! audio, or once the sound has ended, a wall clock continues from the last
//! known position. The value handed out never decreases: a tiny audio/wall
//! disagreement holds the clock still for a few milliseconds instead of
//! stepping it back.

pub mod audio;
pub mod video;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use kira::sound::streaming::StreamingSoundHandle;
use kira::sound::PlaybackState;
use kira::tween::Tween;
use matroska_demuxer::{Frame as MkvPacket, MatroskaFile, TrackType};

/// Codec id Matroska uses for AV1 video, the only video codec this rung decodes.
pub const CODEC_AV1: &str = "V_AV1";
/// Codec id Matroska uses for Opus audio, the only audio codec this rung decodes.
pub const CODEC_OPUS: &str = "A_OPUS";

/// How many decoded frames may sit between the decode thread and the caller.
/// Four frames is 133 ms of lookahead at 30 fps and, at 1080p RGBA (8.3 MB a
/// frame), about 33 MB of memory. Enough to ride out a slow caller frame,
/// small enough that a seek does not have a long stale backlog to throw away.
pub const QUEUE_CAPACITY: usize = 4;

/// How far kira's reported audio position may disagree with the wall-clock
/// estimate before it is treated as stale rather than authoritative. kira
/// applies seek / pause / resume commands asynchronously, so for a few
/// milliseconds after `seek_to_start()` its position still reads the OLD
/// place in the file; a reading that far off the wall estimate is ignored
/// and the wall clock carries on until the two agree again.
pub const AUDIO_TRUST_WINDOW_S: f64 = 0.5;

/// Everything that can go wrong opening or decoding a file. Each variant
/// reads as a sentence a user could act on (which codec, which stage).
#[derive(Debug)]
pub enum MediaError {
    /// The file could not be read at all.
    Io(std::io::Error),
    /// The container is not a Matroska/WebM file we can parse.
    Demux(String),
    /// The file has no video track (this rung is a video player; audio-only
    /// files are not its job).
    NoVideoTrack,
    /// A track uses a codec this player does not decode. `kind` is "video"
    /// or "audio"; `codec_id` is the Matroska id (for example `V_VP8`).
    UnsupportedCodec { kind: &'static str, codec_id: String },
    /// The AV1 decoder refused something (a corrupt packet, an allocation).
    Decoder(String),
    /// The Opus track or its playback hookup failed.
    Audio(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::Io(e) => write!(f, "could not read the media file: {e}"),
            MediaError::Demux(e) => write!(f, "not a WebM/Matroska file we can read: {e}"),
            MediaError::NoVideoTrack => write!(f, "the file has no video track"),
            MediaError::UnsupportedCodec { kind, codec_id } => write!(
                f,
                "unsupported {kind} codec {codec_id}: this player decodes AV1 video ({CODEC_AV1}) \
                 and Opus audio ({CODEC_OPUS}) only; transcode the file to WebM AV1 + Opus"
            ),
            MediaError::Decoder(e) => write!(f, "AV1 decode failed: {e}"),
            MediaError::Audio(e) => write!(f, "audio failed: {e}"),
        }
    }
}

impl std::error::Error for MediaError {}

impl From<std::io::Error> for MediaError {
    fn from(e: std::io::Error) -> Self {
        MediaError::Io(e)
    }
}

impl From<matroska_demuxer::DemuxError> for MediaError {
    fn from(e: matroska_demuxer::DemuxError) -> Self {
        MediaError::Demux(e.to_string())
    }
}

/// One decoded picture, ready to become a texture: tightly packed RGBA8,
/// top-left origin, `width * height * 4` bytes.
pub struct VideoFrame {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Presentation time in seconds from the start of the file.
    pub pts_s: f64,
}

/// What `probe()` learns about a file before any decoding happens.
#[derive(Clone, Debug)]
pub struct MediaInfo {
    pub width: u32,
    pub height: u32,
    /// Container duration in seconds, 0.0 when the file does not declare one
    /// (a live-captured WebM with no Duration element). Every fixture and
    /// every ffmpeg-muxed file declares it.
    pub duration_s: f64,
    /// Matroska codec id of the video track (`V_AV1`).
    pub video_codec: String,
    /// Matroska codec id of the audio track, if there is one (`A_OPUS`).
    pub audio_codec: Option<String>,
    /// Audio sample rate in Hz (Opus always decodes at 48000), 0 without audio.
    pub sample_rate: u32,
    /// Audio channel count, 0 without audio.
    pub channels: u16,
    /// Opus samples to drop at the start of the stream (the encoder's
    /// look-ahead), from the OpusHead in CodecPrivate.
    pub pre_skip: u32,
    /// Total tracks in the container, of any type.
    pub track_count: usize,
    /// Matroska track numbers, used to route packets while demuxing.
    pub video_track: u64,
    pub audio_track: Option<u64>,
    /// Nanoseconds per timestamp tick (1_000_000 for the usual millisecond scale).
    pub timestamp_scale_ns: u64,
}

/// Open the container with a buffered reader. Both the video thread and the
/// Opus track hold their OWN handle to the file so neither has to wait on
/// the other; the file is small next to the decoded frames anyway.
pub(crate) fn open_demuxer(path: &Path) -> Result<MatroskaFile<BufReader<File>>, MediaError> {
    let file = File::open(path)?;
    Ok(MatroskaFile::open(BufReader::new(file))?)
}

/// Read the track table and refuse anything this player cannot decode. This
/// is the ONLY place codec ids are checked, so the error a user sees always
/// names the codec that stopped them.
pub fn probe(path: &Path) -> Result<MediaInfo, MediaError> {
    let mkv = open_demuxer(path)?;
    let scale = mkv.info().timestamp_scale().get();
    // Matroska Duration is a float in timestamp-scale ticks.
    let duration_s = mkv
        .info()
        .duration()
        .map(|ticks| ticks * scale as f64 / 1e9)
        .unwrap_or(0.0);

    let mut video: Option<(u64, u32, u32, String)> = None;
    let mut audio: Option<(u64, u32, u16, u32, String)> = None;
    for track in mkv.tracks() {
        match track.track_type() {
            TrackType::Video if video.is_none() => {
                let codec = track.codec_id().to_string();
                if codec != CODEC_AV1 {
                    return Err(MediaError::UnsupportedCodec { kind: "video", codec_id: codec });
                }
                let v = track
                    .video()
                    .ok_or_else(|| MediaError::Demux("video track without a Video element".into()))?;
                video = Some((
                    track.track_number().get(),
                    v.pixel_width().get() as u32,
                    v.pixel_height().get() as u32,
                    codec,
                ));
            }
            TrackType::Audio if audio.is_none() => {
                let codec = track.codec_id().to_string();
                if codec != CODEC_OPUS {
                    return Err(MediaError::UnsupportedCodec { kind: "audio", codec_id: codec });
                }
                let head = audio::parse_opus_head(track.codec_private().unwrap_or(&[]))?;
                if head.channels == 0 || head.channels > 2 {
                    return Err(MediaError::Audio(format!(
                        "Opus with {} channels needs the multistream decoder, which is not wired; \
                         mono and stereo only",
                        head.channels
                    )));
                }
                audio = Some((
                    track.track_number().get(),
                    audio::OPUS_RATE,
                    head.channels,
                    head.pre_skip as u32,
                    codec,
                ));
            }
            _ => {}
        }
    }

    let (video_track, width, height, video_codec) = video.ok_or(MediaError::NoVideoTrack)?;
    let (audio_track, sample_rate, channels, pre_skip, audio_codec) = match audio {
        Some((t, sr, ch, ps, codec)) => (Some(t), sr, ch, ps, Some(codec)),
        None => (None, 0, 0, 0, None),
    };
    Ok(MediaInfo {
        width,
        height,
        duration_s,
        video_codec,
        audio_codec,
        sample_rate,
        channels,
        pre_skip,
        track_count: mkv.tracks().len(),
        video_track,
        audio_track,
        timestamp_scale_ns: scale,
    })
}

// ---------------------------------------------------------------------------
// The playback clock
// ---------------------------------------------------------------------------

/// A monotonic playback position in seconds. `base` is where the clock was
/// at `anchor`; while playing, elapsed wall time is added on top. `reported`
/// remembers the largest value ever handed out so a re-base to a slightly
/// earlier audio position never makes the clock run backwards.
struct Clock {
    playing: bool,
    anchor: Instant,
    base: f64,
    /// 0.0 means unknown (no clamping at the end).
    duration: f64,
    reported: Cell<f64>,
}

impl Clock {
    fn new(duration: f64) -> Self {
        Self { playing: false, anchor: Instant::now(), base: 0.0, duration, reported: Cell::new(0.0) }
    }

    /// The raw estimate, before the monotonic guard.
    fn raw(&self) -> f64 {
        let p = if self.playing { self.base + self.anchor.elapsed().as_secs_f64() } else { self.base };
        if self.duration > 0.0 {
            p.min(self.duration)
        } else {
            p
        }
    }

    /// The position handed to callers: never smaller than the last one.
    fn position(&self) -> f64 {
        let p = self.raw().max(self.reported.get());
        self.reported.set(p);
        p
    }

    fn play(&mut self) {
        if !self.playing {
            self.anchor = Instant::now();
            self.playing = true;
        }
    }

    fn pause(&mut self) {
        self.base = self.raw();
        self.playing = false;
    }

    /// Move the clock to `pos` without touching the playing flag (audio sync).
    fn rebase(&mut self, pos: f64) {
        self.base = pos;
        self.anchor = Instant::now();
    }

    /// Back to zero: the only seek this rung supports.
    fn reset(&mut self) {
        self.base = 0.0;
        self.anchor = Instant::now();
        self.reported.set(0.0);
    }
}

// ---------------------------------------------------------------------------
// The decode thread and its bounded queue
// ---------------------------------------------------------------------------

/// Frames waiting for the caller, plus the flags the two threads agree on.
struct DecodeQueue {
    /// In presentation order; the decoder emits display order and pushes in
    /// sequence, so the front is always the oldest.
    frames: VecDeque<VideoFrame>,
    /// Which playback generation the queued frames belong to (see `Shared`).
    generation: u64,
    /// The decode thread delivered the last frame of this generation.
    eof: bool,
    /// A decode error waiting to be reported to the caller.
    error: Option<String>,
}

/// What the decode thread and the player share.
struct Shared {
    queue: Mutex<DecodeQueue>,
    /// Signalled whenever the queue gains space or a command arrives; the
    /// decode thread waits on it instead of spinning.
    wake: Condvar,
    /// Set by Drop. The decode thread exits at its next check.
    stop: AtomicBool,
    /// Bumped by `seek_to_start()`. The decode thread compares it with the
    /// generation it is decoding for and restarts from the top of the file
    /// when they differ; frames from an old generation are never queued.
    generation: AtomicU64,
    /// False once the decode thread has exited (cleared by a guard on its
    /// stack). Its own Arc so a test can hold it across `drop(player)` and
    /// prove that Drop joined the thread rather than leaking it.
    alive: Arc<AtomicBool>,
}

/// Cleared on the way out of the decode thread, however it exits.
struct AliveGuard(Arc<AtomicBool>);

impl Drop for AliveGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Why a decode pass over the file ended.
enum PassEnd {
    /// The whole file was decoded and delivered.
    Eof,
    /// `seek_to_start()` asked for a fresh pass.
    Restart,
    /// Drop asked the thread to leave.
    Stop,
}

/// Outcome of trying to queue one frame.
enum Push {
    Pushed,
    Restart,
    Stop,
}

/// Queue one frame, waiting while the queue is full. Returns early when a
/// stop or a restart arrives so the thread never blocks on a caller that has
/// moved on.
fn push_frame(shared: &Shared, generation: u64, frame: VideoFrame) -> Push {
    let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Push::Stop;
        }
        if shared.generation.load(Ordering::SeqCst) != generation {
            return Push::Restart;
        }
        if q.frames.len() < QUEUE_CAPACITY {
            q.generation = generation;
            q.frames.push_back(frame);
            return Push::Pushed;
        }
        // Full: sleep until poll() takes a frame (or up to 20 ms, so a stop
        // that raced the notify is still seen promptly).
        q = shared
            .wake
            .wait_timeout(q, Duration::from_millis(20))
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
}

/// One pass over the file: open, decode every video packet, drain, mark EOF.
fn decode_pass(
    path: &Path,
    info: &MediaInfo,
    shared: &Shared,
    generation: u64,
    threads: i32,
) -> Result<PassEnd, MediaError> {
    let mut mkv = open_demuxer(path)?;
    let mut decoder = video::Av1Decoder::new(threads)?;
    let mut packet = MkvPacket::default();
    let scale = info.timestamp_scale_ns;

    // The closure the decoder calls for each finished picture. It returns
    // false to make the decoder stop early and records why in `early`
    // (a Cell so the closure and the loop below can both touch it).
    let early: Cell<Option<PassEnd>> = Cell::new(None);
    let mut deliver = |frame: VideoFrame| -> bool {
        match push_frame(shared, generation, frame) {
            Push::Pushed => true,
            Push::Restart => {
                early.set(Some(PassEnd::Restart));
                false
            }
            Push::Stop => {
                early.set(Some(PassEnd::Stop));
                false
            }
        }
    };

    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Ok(PassEnd::Stop);
        }
        if shared.generation.load(Ordering::SeqCst) != generation {
            return Ok(PassEnd::Restart);
        }
        if !mkv.next_frame(&mut packet)? {
            break;
        }
        if packet.track != info.video_track {
            continue;
        }
        let pts_ns = packet.timestamp.saturating_mul(scale) as i64;
        if !decoder.decode_packet(&packet.data, pts_ns, &mut deliver)? {
            return Ok(early.take().unwrap_or(PassEnd::Stop));
        }
    }
    // No more packets: pull the pictures the decoder is still holding
    // (frame threads keep a few in flight).
    if !decoder.drain(&mut deliver)? {
        return Ok(early.take().unwrap_or(PassEnd::Stop));
    }
    // Mark the end, but only if no seek arrived meanwhile: a seek bumps the
    // generation before clearing the queue, so a stale pass must not mark
    // the NEW generation finished.
    let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    if shared.generation.load(Ordering::SeqCst) == generation {
        q.eof = true;
    }
    Ok(PassEnd::Eof)
}

/// The decode thread body: decode the file, then wait for a restart or a
/// stop. After a restart it decodes the file again from the top.
fn decode_thread(path: PathBuf, info: MediaInfo, shared: Arc<Shared>, threads: i32) {
    let _guard = AliveGuard(shared.alive.clone());
    loop {
        let generation = shared.generation.load(Ordering::SeqCst);
        match decode_pass(&path, &info, &shared, generation, threads) {
            Ok(PassEnd::Stop) => return,
            Ok(PassEnd::Restart) => continue,
            Ok(PassEnd::Eof) => {}
            Err(e) => {
                let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
                q.error = Some(e.to_string());
                if shared.generation.load(Ordering::SeqCst) == generation {
                    q.eof = true;
                }
            }
        }
        // Finished (or failed) this generation: idle until told otherwise.
        let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if shared.stop.load(Ordering::SeqCst) {
                return;
            }
            if shared.generation.load(Ordering::SeqCst) != generation {
                break;
            }
            q = shared
                .wake
                .wait_timeout(q, Duration::from_millis(50))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

/// Pick the frame that is due: the NEWEST queued frame whose pts is at or
/// before `clock`. Older due frames are dropped (the caller wants the
/// present, not a backlog) and nothing ahead of the clock is ever returned.
/// Pure so it can be tested with synthetic frames.
pub(crate) fn take_due_frame(frames: &mut VecDeque<VideoFrame>, clock: f64) -> Option<VideoFrame> {
    let mut chosen = None;
    while frames.front().map_or(false, |f| f.pts_s <= clock) {
        chosen = frames.pop_front();
    }
    chosen
}

// ---------------------------------------------------------------------------
// The player
// ---------------------------------------------------------------------------

/// A video player with no screen: decodes on a background thread, keeps a
/// clock, and hands out the frame that is due when asked. Dropping it stops
/// and JOINS the decode thread (never leaked) and stops the audio.
pub struct VideoPlayer {
    path: PathBuf,
    info: MediaInfo,
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
    clock: Clock,
    /// The kira sound playing the Opus track, once `attach_audio` ran.
    audio: Option<StreamingSoundHandle<MediaError>>,
    /// pts of the last frame handed out this generation, so poll() can
    /// guarantee frames leave in order even across an audio re-base.
    last_delivered_pts: f64,
    last_error: Option<String>,
}

impl VideoPlayer {
    /// Probe the file, refuse unsupported codecs, start the decode thread.
    /// The player starts PAUSED at position 0; call `play()`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, MediaError> {
        Self::open_with_threads(path, 0)
    }

    /// Like `open`, with an explicit rav1d thread count (0 = one per logical
    /// processor, rav1d's default). Tests and the benchmark use 1 to measure
    /// single-core cost.
    pub fn open_with_threads(path: impl AsRef<Path>, threads: i32) -> Result<Self, MediaError> {
        let path = path.as_ref().to_path_buf();
        let info = probe(&path)?;
        let shared = Arc::new(Shared {
            queue: Mutex::new(DecodeQueue {
                frames: VecDeque::with_capacity(QUEUE_CAPACITY),
                generation: 0,
                eof: false,
                error: None,
            }),
            wake: Condvar::new(),
            stop: AtomicBool::new(false),
            generation: AtomicU64::new(0),
            alive: Arc::new(AtomicBool::new(true)),
        });
        let thread = {
            let (path, info, shared) = (path.clone(), info.clone(), shared.clone());
            std::thread::Builder::new()
                .name("media-decode".into())
                .spawn(move || decode_thread(path, info, shared, threads))
                .map_err(MediaError::Io)?
        };
        Ok(Self {
            clock: Clock::new(info.duration_s),
            path,
            info,
            shared,
            thread: Some(thread),
            audio: None,
            last_delivered_pts: -1.0,
            last_error: None,
        })
    }

    pub fn info(&self) -> &MediaInfo {
        &self.info
    }

    /// Hand the Opus track to kira so it plays through the app's mixer. The
    /// sound starts in the player's current state (paused unless `play()`
    /// already ran). A file with no audio track attaches nothing and plays
    /// silent on the wall clock. Call once; a second call replaces the sound.
    pub fn attach_audio(&mut self, manager: &mut crate::audio::AudioManager) -> Result<(), MediaError> {
        if self.info.audio_track.is_none() {
            return Ok(());
        }
        if let Some(mut old) = self.audio.take() {
            old.stop(zero_tween());
        }
        let track = audio::OpusTrack::open(&self.path, &self.info)?;
        let data = kira::sound::streaming::StreamingSoundData::from_decoder(track);
        let mut handle = manager.play_stream(data, 1.0).map_err(MediaError::Audio)?;
        if self.clock.playing {
            handle.seek_to(self.clock.position());
        } else {
            // kira applies the play and the pause command in the same audio
            // thread pass, so no sound leaks out before the first play().
            handle.pause(zero_tween());
        }
        self.audio = Some(handle);
        Ok(())
    }

    /// Whether a kira sound is attached (false for silent files and tests).
    pub fn has_audio(&self) -> bool {
        self.audio.is_some()
    }

    pub fn play(&mut self) {
        if self.at_end() {
            return;
        }
        self.clock.play();
        if let Some(h) = &mut self.audio {
            h.resume(zero_tween());
        }
    }

    pub fn pause(&mut self) {
        self.clock.pause();
        if let Some(h) = &mut self.audio {
            h.pause(zero_tween());
        }
    }

    /// Rewind to the first frame. Keeps the play / pause state. The decode
    /// thread restarts from the top of the file; anything it had queued for
    /// the old position is discarded.
    pub fn seek_to_start(&mut self) {
        let generation = self.shared.generation.fetch_add(1, Ordering::SeqCst) + 1;
        {
            let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            q.frames.clear();
            q.generation = generation;
            q.eof = false;
        }
        self.shared.wake.notify_all();
        self.clock.reset();
        self.last_delivered_pts = -1.0;
        if let Some(h) = &mut self.audio {
            h.seek_to(0.0);
        }
    }

    /// The newest decoded frame whose pts is at or before the clock, or None
    /// when nothing new is due. Call once per drawn frame.
    pub fn poll(&mut self) -> Option<VideoFrame> {
        self.sync_clock_to_audio();
        let clock = self.clock.position();
        let (frame, ended) = {
            let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(e) = q.error.take() {
                self.last_error = Some(e);
            }
            let frame = take_due_frame(&mut q.frames, clock);
            (frame, q.eof && q.frames.is_empty())
        };
        if frame.is_some() {
            // Space opened up: the decode thread may be waiting for it.
            self.shared.wake.notify_all();
        }
        // The clip is over when the decoder is done, nothing is queued and the
        // clock has reached the declared end. Stop the clock there so
        // `is_playing()` turns false and the surface can show "ended".
        if ended && self.clock.playing && self.info.duration_s > 0.0 && clock >= self.info.duration_s {
            self.clock.pause();
        }
        let frame = frame.filter(|f| f.pts_s >= self.last_delivered_pts);
        if let Some(f) = &frame {
            self.last_delivered_pts = f.pts_s;
        }
        frame
    }

    /// Container duration in seconds (0.0 if the file does not declare one).
    pub fn duration_s(&self) -> f64 {
        self.info.duration_s
    }

    /// The playback clock in seconds: audio-led while sound plays, wall
    /// clock otherwise, never decreasing between seeks.
    pub fn position_s(&self) -> f64 {
        self.clock.position()
    }

    pub fn is_playing(&self) -> bool {
        self.clock.playing
    }

    /// True once playback has reached the declared end of the clip.
    pub fn at_end(&self) -> bool {
        self.info.duration_s > 0.0 && self.clock.position() >= self.info.duration_s
    }

    /// A decode error the background thread hit, reported once.
    pub fn take_error(&mut self) -> Option<String> {
        self.last_error.take()
    }

    /// The decode thread's liveness flag (true while it runs, cleared on its
    /// way out). Public so a test can hold it across `drop(player)` and prove
    /// that Drop joined the thread instead of leaking it.
    pub fn decode_thread_alive_flag(&self) -> Arc<AtomicBool> {
        self.shared.alive.clone()
    }

    /// Audio-led clock: while the kira sound is playing, adopt its reported
    /// position, provided it is within the trust window of the wall estimate
    /// (a reading far off is a stale pre-seek value or a stalled device; the
    /// wall clock carries on until the two agree).
    fn sync_clock_to_audio(&mut self) {
        let Some(h) = &self.audio else { return };
        if !self.clock.playing || h.state() != PlaybackState::Playing {
            return;
        }
        let audio_pos = h.position();
        let wall_pos = self.clock.raw();
        if (audio_pos - wall_pos).abs() < AUDIO_TRUST_WINDOW_S {
            self.clock.rebase(audio_pos);
        }
    }
}

/// A zero-length tween: apply the command now, no fade.
fn zero_tween() -> Tween {
    Tween { duration: Duration::ZERO, ..Default::default() }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // Tell the decode thread to leave, wake it if it is waiting on a
        // full queue, and JOIN it. A player dropped mid-decode waits at most
        // one picture (tens of milliseconds at 1080p single-thread).
        self.shared.stop.store(true, Ordering::SeqCst);
        self.shared.wake.notify_all();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Some(mut h) = self.audio.take() {
            h.stop(zero_tween());
        }
    }
}
