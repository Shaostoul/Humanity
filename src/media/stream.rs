//! Playing a film through the person's own ffmpeg AS IT PLAYS (2026-11-13).
//!
//! The player decodes AV1 and Opus itself (docs/design/media-player.md, "Container and codecs"),
//! and on Windows hands an ordinary MP4 to the computer's own decoders (`system.rs`). Anything else
//! (an MKV with H.264, an AVI, a MOV, a video disc's MPEG-2) is decoded by the ffmpeg already on
//! the person's machine, while it plays: ffmpeg writes raw pictures (YUV4MPEG2, 8-bit 4:2:0) to one
//! pipe and raw sound (32-bit floats, 48 kHz stereo) to another, and the player shows and plays
//! them exactly as it shows its own decoder's. Nothing is converted ahead, nothing is written to
//! disk, and a seek starts ffmpeg again at the new place (`-ss` before `-i`: ffmpeg jumps to the
//! keyframe before it and decodes forward, so the first picture is the one asked for).
//!
//! Why: until 2026-11-13 the whole file was first converted to AV1 + Opus and the copy kept under
//! `<data dir>/media/cache`. That broke the no-caches rule (CLAUDE.md, operator 2026-07-16: "Every
//! cache is a chance to encounter a bug."), it kept every film on disk (595 MB for one film on the
//! dev machine), and nothing played until the whole conversion finished: 16 s for a one-minute 720p
//! clip, about half an hour for a two-hour film (measured 2026-11-13; media-player.md,
//! "Measurements"). Decoding as it plays needs no encoder at all, so the first picture comes in a
//! fraction of a second and a seek can go anywhere at once.
//!
//! Why two ffmpeg processes and not one: a process has one standard output, and a second pipe
//! would need a named pipe made differently on every platform. Each process reads the file itself
//! (the sound's decodes no pictures: `-vn`), both start at the same `-ss`, and the player's clock
//! follows the sound, as it does for every other file.
//!
//! What a crash leaves behind: nothing. The pipes are the only link to ffmpeg; when this process
//! ends, for any reason, ffmpeg's next write fails and it exits (pinned by
//! `a_crash_leaves_no_ffmpeg_running`), and it never writes a file.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, ExitStatus, Stdio};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use kira::Frame;

use super::ffmpeg::{ffmpeg_command, parse_duration_s, Ready};
use super::video::ColorMatrix;
use super::{push_frame, MediaError, MediaInfo, PassEnd, Push, Shared, VideoFrame};

/// The most rows a picture comes through the pipe at. A 4K film at 60 frames a second would be
/// 750 MB a second of raw pictures and a 33 MB colour conversion per picture; 1080 rows is what the
/// largest in-world screen and the app's own player window show, and 187 MB a second at 60 frames
/// is easy for a pipe. A smaller film keeps its own size (never enlarged).
pub const MAX_ROWS: u32 = 1080;

/// The sound's rate and channels as ffmpeg hands them over (`audio_args`): the rate kira mixes at
/// and the stereo every screen pans.
pub const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: usize = 2;

/// Frames handed to kira per `decode` (21 ms at 48 kHz), the Opus path's chunk.
const CHUNK_FRAMES: usize = 1024;

/// How much of ffmpeg's error output is kept for a message: the tail is where the reason lives.
const STDERR_TAIL_BYTES: usize = 4096;

/// What `ffmpeg -i` says about a file, the parts the player needs. ffmpeg prints a summary of
/// every stream when it opens a file (and then complains that no output was named, which is the
/// point of asking that way: no second program such as ffprobe is needed).
#[derive(Clone, Debug, PartialEq)]
pub struct Summary {
    /// `None` when the summary says `N/A` (a raw stream).
    pub duration_s: Option<f64>,
    /// The first picture stream's codec (`h264`, `mpeg2video`), for the log and `MediaInfo`.
    pub video_codec: String,
    pub width: u32,
    pub height: u32,
    /// The shape of one stored pixel (`[SAR 32:27 ...]`), 1:1 when the file does not say. A video
    /// disc stores a 16:9 picture as 720x480 with wide pixels.
    pub sar: (u32, u32),
    /// Pictures a second as printed (`23.98 fps`, else `tbr`), `None` when neither is there.
    pub fps: Option<f64>,
    /// The picture is stored as two interlaced fields (`top first`, `bottom first`): a video disc,
    /// a camcorder tape. It is deinterlaced on the way through.
    pub interlaced: bool,
    /// The colour space name ffmpeg printed (`bt709`, `smpte170m`), `None` when it printed none.
    pub colour: Option<String>,
    /// The first sound stream's codec (`aac`, `ac3`), `None` for a silent film.
    pub audio_codec: Option<String>,
}

/// Split a stream line's description on its top-level commas: `yuv420p(tv, bt709, progressive)`
/// and `[SAR 1:1 DAR 16:9]` keep their inner commas.
fn split_top(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(s[start..].trim());
    out
}

/// A number as ffmpeg prints a rate: `23.98`, `30`, `90k`.
fn parse_rate(s: &str) -> Option<f64> {
    let s = s.trim();
    match s.strip_suffix('k') {
        Some(n) => n.parse::<f64>().ok().map(|v| v * 1000.0),
        None => s.parse::<f64>().ok(),
    }
}

/// Read the summary `ffmpeg -hide_banner -i <file>` writes to its error output. `None` when there
/// is no picture stream (a sound file, or not a media file at all). Cover art (`attached pic`) is
/// not a picture stream: the arguments pick `0:V:0`, which skips it the same way.
pub fn parse_summary(text: &str) -> Option<Summary> {
    let video_line = text.lines().find(|l| l.contains("Stream #") && l.contains("Video: ") && !l.contains("(attached pic)"))?;
    let desc = &video_line[video_line.find("Video: ")? + "Video: ".len()..];
    let parts = split_top(desc);
    let video_codec = parts.first()?.split_whitespace().next()?.to_string();
    let mut s = Summary {
        duration_s: parse_duration_s(text),
        video_codec,
        width: 0,
        height: 0,
        sar: (1, 1),
        fps: None,
        interlaced: false,
        colour: None,
        audio_codec: None,
    };
    // `SAR 32:27` wherever it sits: inside the size's brackets (the codec's own) or as a part of
    // its own after the size (the container's, which ffmpeg prefers when it differs). The later
    // one wins, which is the container's.
    let read_sar = |text: &str, sar: &mut (u32, u32)| {
        if let Some(i) = text.find("SAR ") {
            let ratio = text[i + 4..].split_whitespace().next().unwrap_or("");
            if let Some((a, b)) = ratio.split_once(':') {
                if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.trim_end_matches(']').parse::<u32>()) {
                    if a > 0 && b > 0 {
                        *sar = (a, b);
                    }
                }
            }
        }
    };
    let mut tbr = None;
    for part in &parts[1..] {
        if part.starts_with("SAR ") {
            read_sar(part, &mut s.sar);
            continue;
        }
        let first = part.split_whitespace().next().unwrap_or("");
        if let Some((w, h)) = first.split_once('x') {
            if let (Ok(w), Ok(h)) = (w.parse::<u32>(), h.parse::<u32>()) {
                s.width = w;
                s.height = h;
                read_sar(part, &mut s.sar);
                continue;
            }
        }
        if let Some(rate) = part.strip_suffix(" fps") {
            s.fps = parse_rate(rate);
        } else if let Some(rate) = part.strip_suffix(" tbr") {
            tbr = parse_rate(rate);
        } else if s.width == 0 {
            // The pixel format comes before the size: `yuv420p(tv, bt709, top first)`.
            if let (Some(open), Some(close)) = (part.find('('), part.rfind(')')) {
                for item in part[open + 1..close].split(", ") {
                    let item = item.trim();
                    if item.contains("first") {
                        s.interlaced = true;
                    } else if !matches!(item, "tv" | "pc" | "progressive") && s.colour.is_none() {
                        s.colour = item.split('/').next().map(|c| c.to_string());
                    }
                }
            }
        }
    }
    if s.fps.is_none() {
        s.fps = tbr;
    }
    if s.width == 0 || s.height == 0 {
        return None;
    }
    s.audio_codec = text
        .lines()
        .find(|l| l.contains("Stream #") && l.contains("Audio: "))
        .and_then(|l| l[l.find("Audio: ")? + "Audio: ".len()..].split(|c: char| c == ' ' || c == ',').next().map(|c| c.to_string()));
    Some(s)
}

/// The size the pictures come through the pipe at: square pixels (a disc's wide pixels widened,
/// never its height squashed), at most `MAX_ROWS` rows, both sides even (4:2:0 halves them).
pub fn output_size(width: u32, height: u32, sar: (u32, u32)) -> (u32, u32) {
    let (n, d) = if sar.0 == 0 || sar.1 == 0 { (1, 1) } else { sar };
    let mut w = width as f64 * n as f64 / d as f64;
    let mut h = height as f64;
    if h > MAX_ROWS as f64 {
        w *= MAX_ROWS as f64 / h;
        h = MAX_ROWS as f64;
    }
    let even = |x: f64| (((x / 2.0).round() as u32) * 2).max(2);
    (even(w), even(h))
}

/// The pictures-a-second ffmpeg is asked for, as a fraction: the NTSC rates ffmpeg prints rounded
/// (`29.97`) are their exact 1000/1001 fractions, so a long film does not drift from its sound.
/// Unknown or absurd rates play at 30.
pub fn fps_fraction(fps: Option<f64>) -> (u32, u32) {
    let Some(fps) = fps.filter(|f| f.is_finite() && *f >= 1.0 && *f <= 240.0) else { return (30, 1) };
    for (n, d) in [(24_000u32, 1001u32), (30_000, 1001), (48_000, 1001), (60_000, 1001), (120_000, 1001)] {
        if (fps - n as f64 / d as f64).abs() < 0.006 {
            return (n, d);
        }
    }
    if (fps - fps.round()).abs() < 0.01 {
        return (fps.round() as u32, 1);
    }
    ((fps * 1000.0).round() as u32, 1000)
}

/// The colour matrix for the YUV the pipe carries: what the file says, else the convention players
/// use (BT.709 from 720 rows up, BT.601 below), as `video::choose_matrix` does for AV1.
pub fn matrix_for(colour: Option<&str>, source_height: u32) -> ColorMatrix {
    match colour {
        Some("bt709") => ColorMatrix::Bt709,
        Some("smpte170m" | "bt470bg" | "bt601" | "fcc" | "smpte240m") => ColorMatrix::Bt601,
        Some("bt2020nc" | "bt2020c") => ColorMatrix::Bt2020,
        _ if source_height >= 720 => ColorMatrix::Bt709,
        _ => ColorMatrix::Bt601,
    }
}

/// Everything needed to play one film through ffmpeg: where ffmpeg is, what it reads, and what
/// the short look at the file found (`look`).
#[derive(Clone, Debug)]
pub struct StreamSource {
    pub ffmpeg: PathBuf,
    /// A file, or `concat:<part>|<part>` for a disc's title (`ffmpeg::concat_input`).
    pub input: PathBuf,
    pub summary: Summary,
    /// The pictures' size as they come through the pipe (`output_size`).
    pub width: u32,
    pub height: u32,
    /// Pictures a second, asked of ffmpeg (`fps_fraction`).
    pub fps: (u32, u32),
    pub matrix: ColorMatrix,
}

impl StreamSource {
    /// Build the source from a summary (public for the tests, which feed it real ffmpeg output).
    pub fn from_summary(ffmpeg: PathBuf, input: PathBuf, summary: Summary) -> Self {
        let (width, height) = output_size(summary.width, summary.height, summary.sar);
        let fps = fps_fraction(summary.fps);
        let matrix = matrix_for(summary.colour.as_deref(), summary.height);
        Self { ffmpeg, input, summary, width, height, fps, matrix }
    }

    /// What `VideoPlayer::info` reports for it: the pipe's picture size, the file's length, and
    /// the sound as ffmpeg hands it over (48 kHz stereo) when the file has any.
    pub fn info(&self) -> MediaInfo {
        let sound = self.summary.audio_codec.is_some();
        MediaInfo {
            width: self.width,
            height: self.height,
            duration_s: self.summary.duration_s.unwrap_or(0.0),
            video_codec: self.summary.video_codec.clone(),
            audio_codec: self.summary.audio_codec.clone(),
            sample_rate: if sound { SAMPLE_RATE } else { 0 },
            channels: if sound { CHANNELS as u16 } else { 0 },
            pre_skip: 0,
            track_count: if sound { 2 } else { 1 },
            video_track: 0,
            audio_track: if sound { Some(0) } else { None },
            timestamp_scale_ns: 0,
            system: false,
            mp4: false,
            system_sound: false,
            sound_lost: None,
            stream: true,
        }
    }
}

/// Take the short look at a file (`ffmpeg -hide_banner -i <input>`, which prints the summary and
/// exits): `Err` with ffmpeg's last words when it cannot be run or finds no picture.
pub fn look(ffmpeg: &Path, input: &Path) -> Result<StreamSource, String> {
    let out = ffmpeg_command(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-i"])
        .arg(input)
        .stdout(Stdio::null())
        .output()
        .map_err(|e| format!("could not run ffmpeg at {}: {e}", ffmpeg.display()))?;
    let text = String::from_utf8_lossy(&out.stderr);
    match parse_summary(&text) {
        Some(summary) => Ok(StreamSource::from_summary(ffmpeg.to_path_buf(), input.to_path_buf(), summary)),
        None => {
            let tail: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).rev().take(2).collect::<Vec<_>>().into_iter().rev().collect();
            Err(format!("ffmpeg found no picture in it ({})", tail.join(" | ").trim()))
        }
    }
}

/// The short look running on a worker, so a screen opening a film on a slow disc drive never
/// holds a frame: poll it each frame, or `wait` on a thread that may block. Its answer is a
/// `Ready`: the stream to play, the file to play as it is after all (`fallback`), or the words.
pub struct Opening {
    rx: Receiver<Ready>,
}

impl Opening {
    /// Look at `input` with `ffmpeg` on a worker. When the look fails, `fallback` (a file the
    /// computer's own decoders play without its sound) is played as it is, silent rather than not
    /// at all; otherwise `fail` turns ffmpeg's reason into the sentence a person reads (what the
    /// file is, why ffmpeg was needed, the fix: `ffmpeg::ffmpeg_failed`).
    pub fn spawn(ffmpeg: PathBuf, input: PathBuf, fallback: Option<PathBuf>, fail: impl FnOnce(&str) -> String + Send + 'static) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("media-ffmpeg-look".into()).spawn(move || {
            let ready = match look(&ffmpeg, &input) {
                Ok(src) => {
                    log::info!(
                        "[Media] playing {} through ffmpeg as it plays: {} {}x{} (as {}x{} at {}/{} fps), sound: {}",
                        input.display(),
                        src.summary.video_codec,
                        src.summary.width,
                        src.summary.height,
                        src.width,
                        src.height,
                        src.fps.0,
                        src.fps.1,
                        src.summary.audio_codec.as_deref().unwrap_or("none")
                    );
                    Ready::Stream(src)
                }
                Err(e) => match fallback {
                    Some(path) => {
                        log::warn!("[Media] ffmpeg could not play {}, so it plays without its sound: {e}", input.display());
                        Ready::Direct(path)
                    }
                    None => Ready::Failed(fail(&e)),
                },
            };
            let _ = tx.send(ready);
        });
        if let Err(e) = spawned {
            let (tx, rx2) = std::sync::mpsc::channel();
            let _ = tx.send(Ready::Failed(format!("could not start a worker to look at the file: {e}")));
            return Self { rx: rx2 };
        }
        Self { rx }
    }

    /// The look's answer once it has come (handed out once), `None` while it runs.
    pub fn poll(&mut self) -> Option<Ready> {
        match self.rx.try_recv() {
            Ok(r) => Some(r),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Ready::Failed("the look at the file stopped without an answer".into())),
        }
    }

    /// Wait for the answer (a worker's road: the chat's video view decides on its fetch worker).
    pub fn wait(self, timeout: Duration) -> Ready {
        self.rx.recv_timeout(timeout).unwrap_or_else(|_| Ready::Failed(format!("ffmpeg did not answer within {} s", timeout.as_secs())))
    }
}

/// `-ss` for a start place: nothing at the top of the file, else seconds to the millisecond.
fn push_start(args: &mut Vec<OsString>, start_s: f64) {
    if start_s > 0.0 {
        args.push("-ss".into());
        args.push(format!("{start_s:.3}").into());
    }
}

/// THE picture command line, pinned by `the_command_lines_are_pinned`.
///
/// * `-loglevel error`: only what went wrong reaches the error output (kept for a message).
/// * `-ss` BEFORE `-i`: ffmpeg seeks the file to the keyframe before the place and decodes
///   forward, handing out nothing before it, so a seek is as fast as one keyframe interval.
/// * `-map 0:V:0 -an -sn -dn`: the first picture stream that is not cover art, nothing else.
/// * `bwdif` (interlaced sources only): deinterlaced to full frames at the same rate; a disc's
///   comb-edged fields would otherwise show on every moving edge.
/// * `fps=`: a steady rate, so each picture's time is its number over the rate (the pipe carries
///   no times). A phone's uneven rate is evened the way a player does.
/// * `scale=W:H:out_range=tv,setsar=1`: square pixels at `output_size`, studio range, which is
///   what `video::yuv420_to_rgba` reads.
/// * `-pix_fmt yuv420p -f yuv4mpegpipe pipe:1`: 8-bit 4:2:0 in the simplest self-describing raw
///   format (a header line, then `FRAME` and the three planes per picture), on standard output.
pub fn video_args(src: &StreamSource, start_s: f64) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-nostdin", "-loglevel", "error"].iter().map(OsString::from).collect();
    push_start(&mut a, start_s);
    a.push("-i".into());
    a.push(src.input.as_os_str().to_os_string());
    for s in ["-map", "0:V:0", "-an", "-sn", "-dn", "-vf"] {
        a.push(s.into());
    }
    let mut vf = String::new();
    if src.summary.interlaced {
        vf.push_str("bwdif=mode=send_frame:deint=interlaced,");
    }
    vf.push_str(&format!("fps={}/{},scale={}:{}:out_range=tv,setsar=1", src.fps.0, src.fps.1, src.width, src.height));
    a.push(vf.into());
    for s in ["-pix_fmt", "yuv420p", "-f", "yuv4mpegpipe", "pipe:1"] {
        a.push(s.into());
    }
    a
}

/// THE sound command line, pinned with the picture's.
///
/// * `-map 0:a:0 -vn -sn -dn`: the first sound stream; no picture is decoded for it.
/// * `aresample=48000:async=1:first_pts=0`: 48 kHz, gaps in the file's times filled with
///   silence and drift stretched away, and a sound that starts after the picture padded from the
///   start, so the sound's sample count IS its time (the pipe carries no times).
/// * `-ac 2 -f f32le pipe:1`: stereo (surround folded down by ffmpeg's own matrix), 32-bit float
///   samples, interleaved, on standard output.
pub fn audio_args(src: &StreamSource, start_s: f64) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-nostdin", "-loglevel", "error"].iter().map(OsString::from).collect();
    push_start(&mut a, start_s);
    a.push("-i".into());
    a.push(src.input.as_os_str().to_os_string());
    for s in ["-map", "0:a:0", "-vn", "-sn", "-dn", "-af", "aresample=48000:async=1:first_pts=0", "-ac", "2", "-f", "f32le", "pipe:1"] {
        a.push(s.into());
    }
    a
}

/// A running ffmpeg's process, shared with the player's `KillSwitch`.
type ChildSlot = Arc<Mutex<Option<Child>>>;

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// One running ffmpeg: killed and reaped when dropped, so a player that moves on (a seek, a new
/// film, the screen removed) never leaves one decoding behind it.
struct Running {
    child: ChildSlot,
    /// Drains the error output (a full pipe would stall ffmpeg) and keeps its tail.
    stderr: Option<JoinHandle<String>>,
}

impl Running {
    /// Start ffmpeg with `args`, its standard output a pipe handed back.
    fn spawn(ffmpeg: &Path, args: &[OsString]) -> Result<(Self, ChildStdout), MediaError> {
        let mut child = ffmpeg_command(ffmpeg)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| MediaError::Decoder(format!("could not start ffmpeg at {}: {e}", ffmpeg.display())))?;
        let stdout = child.stdout.take().ok_or_else(|| MediaError::Decoder("ffmpeg has no output pipe".into()))?;
        let stderr = child.stderr.take().map(|mut e| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                let _ = e.read_to_end(&mut buf);
                let start = buf.len().saturating_sub(STDERR_TAIL_BYTES);
                String::from_utf8_lossy(&buf[start..]).trim().to_string()
            })
        });
        Ok((Self { child: Arc::new(Mutex::new(Some(child))), stderr }, stdout))
    }

    /// Wait for ffmpeg to exit (its output was read to the end) and hand back how it ended and the
    /// tail of what it said.
    fn finish(mut self) -> (Option<ExitStatus>, String) {
        let child = lock(&self.child).take();
        let status = child.and_then(|mut c| c.wait().ok());
        let tail = self.stderr.take().and_then(|t| t.join().ok()).unwrap_or_default();
        (status, tail)
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(mut c) = lock(&self.child).take() {
            let _ = c.kill();
            let _ = c.wait();
        }
        // The error pipe closed with the process, so its reader ends at once.
        if let Some(t) = self.stderr.take() {
            let _ = t.join();
        }
    }
}

/// The running picture ffmpeg, where the player's Drop can reach it: killing it ends a read that
/// is waiting on a slow drive, so dropping a player never waits on ffmpeg.
#[derive(Clone, Default)]
pub(crate) struct KillSwitch(Arc<Mutex<Option<ChildSlot>>>);

impl KillSwitch {
    fn set(&self, slot: Option<ChildSlot>) {
        *lock(&self.0) = slot;
    }

    /// Stop the running picture ffmpeg, if there is one (the player is going away). The decode
    /// thread still holds the process and reaps it.
    pub(crate) fn kill(&self) {
        if let Some(slot) = lock(&self.0).as_ref() {
            if let Some(c) = lock(slot).as_mut() {
                let _ = c.kill();
            }
        }
    }
}

/// The YUV4MPEG2 stream header: `YUV4MPEG2 W1280 H720 F30000:1001 Ip A1:1 C420jpeg`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Y4mHeader {
    pub width: u32,
    pub height: u32,
    pub fps: (u32, u32),
}

/// Read the header line (without its newline). Only 4:2:0 is accepted: the arguments ask for it.
pub fn parse_y4m_header(line: &str) -> Result<Y4mHeader, String> {
    let mut tokens = line.split_whitespace();
    if tokens.next() != Some("YUV4MPEG2") {
        return Err(format!("not a YUV4MPEG2 stream: {:?}", line.chars().take(40).collect::<String>()));
    }
    let (mut width, mut height, mut fps) = (0u32, 0u32, (30u32, 1u32));
    for t in tokens {
        let (tag, value) = t.split_at(1);
        match tag {
            "W" => width = value.parse().unwrap_or(0),
            "H" => height = value.parse().unwrap_or(0),
            "F" => {
                if let Some((n, d)) = value.split_once(':') {
                    if let (Ok(n), Ok(d)) = (n.parse::<u32>(), d.parse::<u32>()) {
                        if n > 0 && d > 0 {
                            fps = (n, d);
                        }
                    }
                }
            }
            "C" if !value.starts_with("420") => return Err(format!("pictures came as {value}, not 4:2:0")),
            _ => {}
        }
    }
    if width == 0 || height == 0 {
        return Err(format!("the stream header gives no size: {line:?}"));
    }
    Ok(Y4mHeader { width, height, fps })
}

/// One line from the pipe, without its newline; `None` at the end. A line longer than 256 bytes
/// is not YUV4MPEG2.
fn read_line(r: &mut impl BufRead) -> Result<Option<String>, MediaError> {
    let mut line = Vec::with_capacity(64);
    loop {
        let mut byte = [0u8; 1];
        match r.read(&mut byte) {
            Ok(0) => return Ok(if line.is_empty() { None } else { Some(String::from_utf8_lossy(&line).to_string()) }),
            Ok(_) if byte[0] == b'\n' => return Ok(Some(String::from_utf8_lossy(&line).to_string())),
            Ok(_) => {
                line.push(byte[0]);
                if line.len() > 256 {
                    return Err(MediaError::Decoder("the picture pipe is not YUV4MPEG2".into()));
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(MediaError::Io(e)),
        }
    }
}

/// The picture's decode thread for a film ffmpeg decodes: the same queue, generations and seek
/// rule as the other decoders (`super::drive`).
pub(super) fn decode_thread(src: Arc<StreamSource>, kill: KillSwitch, shared: Arc<Shared>) {
    super::drive(&shared, |generation, start_s, _from_top| decode_pass(&src, &kill, &shared, generation, start_s));
}

fn decode_pass(src: &StreamSource, kill: &KillSwitch, shared: &Shared, generation: u64, start_s: f64) -> Result<PassEnd, MediaError> {
    let (running, stdout) = Running::spawn(&src.ffmpeg, &video_args(src, start_s))?;
    kill.set(Some(running.child.clone()));
    // A stop that came while ffmpeg was starting: the kill switch had nothing to kill yet.
    if shared.stop.load(Ordering::SeqCst) {
        kill.set(None);
        return Ok(PassEnd::Stop);
    }
    let result = read_pictures(src, shared, generation, start_s, stdout);
    kill.set(None);
    match result {
        // The pipe ended because the player's Drop killed ffmpeg: not the film's end.
        Ok((PassEnd::Eof, _)) if shared.stop.load(Ordering::SeqCst) => Ok(PassEnd::Stop),
        Ok((PassEnd::Eof, pictures)) => {
            let (status, tail) = running.finish();
            if pictures == 0 && !status.is_some_and(|s| s.success()) {
                return Err(MediaError::Decoder(format!("ffmpeg gave no pictures: {tail}")));
            }
            if !status.is_some_and(|s| s.success()) {
                log::warn!("media: ffmpeg ended the film early after {pictures} pictures: {tail}");
            }
            let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
            if shared.generation.load(Ordering::SeqCst) == generation {
                q.eof = true;
            }
            Ok(PassEnd::Eof)
        }
        // A restart or a stop: `running` is dropped here, which kills ffmpeg.
        Ok((end, _)) => Ok(end),
        Err(e) => {
            let (_, tail) = running.finish();
            Err(if tail.is_empty() { e } else { MediaError::Decoder(format!("{e}: {tail}")) })
        }
    }
}

/// Read pictures from the pipe until it ends, a seek or a stop: the end, and how many pictures.
fn read_pictures(src: &StreamSource, shared: &Shared, generation: u64, start_s: f64, stdout: ChildStdout) -> Result<(PassEnd, u64), MediaError> {
    let mut reader = BufReader::with_capacity(1 << 20, stdout);
    let Some(line) = read_line(&mut reader)? else { return Ok((PassEnd::Eof, 0)) };
    let header = parse_y4m_header(&line).map_err(MediaError::Decoder)?;
    let (w, h) = (header.width as usize, header.height as usize);
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut buf = vec![0u8; w * h + 2 * cw * ch];
    let frame_s = header.fps.1 as f64 / header.fps.0 as f64;
    let mut n: u64 = 0;
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Ok((PassEnd::Stop, n));
        }
        if shared.generation.load(Ordering::SeqCst) != generation {
            return Ok((PassEnd::Restart, n));
        }
        match read_line(&mut reader)? {
            None => return Ok((PassEnd::Eof, n)),
            Some(l) if l.starts_with("FRAME") => {}
            Some(l) => return Err(MediaError::Decoder(format!("unexpected line in the picture pipe: {:?}", l.chars().take(40).collect::<String>()))),
        }
        match reader.read_exact(&mut buf) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok((PassEnd::Eof, n)),
            Err(e) => return Err(MediaError::Io(e)),
        }
        let pts_s = start_s + n as f64 * frame_s;
        n += 1;
        #[cfg(test)]
        shared.pictures_decoded.fetch_add(1, Ordering::SeqCst);
        #[cfg(test)]
        if shared.restart_instead_of_queueing.load(Ordering::SeqCst) {
            return Ok((PassEnd::Restart, n));
        }
        let (y, rest) = buf.split_at(w * h);
        let (u, v) = rest.split_at(cw * ch);
        let rgba = super::video::yuv420_to_rgba(w, h, y, u, v, src.matrix);
        match push_frame(shared, generation, VideoFrame { rgba, width: header.width, height: header.height, pts_s }) {
            Push::Pushed => {}
            Push::Restart => return Ok((PassEnd::Restart, n)),
            Push::Stop => return Ok((PassEnd::Stop, n)),
        }
    }
}

/// Fill `buf` from `r` until it is full or the pipe ends; the bytes read.
fn fill(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match r.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(k) => got += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

/// The film's sound for kira, decoded by ffmpeg as it plays (`audio_args`). kira calls it on its
/// own decoder thread; ffmpeg is started there on the first `decode`, and again at every seek.
pub struct PipeAudio {
    src: Arc<StreamSource>,
    running: Option<(Running, ChildStdout)>,
    /// The frame index the next `decode` starts at.
    cursor: usize,
    /// The sound ran out: silence from here (kira's index keeps moving, the Opus path's reason).
    ended: bool,
    num_frames: usize,
    buf: Vec<u8>,
}

impl PipeAudio {
    pub fn new(src: Arc<StreamSource>) -> Self {
        // A length the file does not declare (a raw stream) is a day of sound: kira needs a count,
        // and the picture side ends the film when ffmpeg does.
        let duration_s = src.summary.duration_s.filter(|d| *d > 0.0).unwrap_or(86_400.0);
        let num_frames = (duration_s * SAMPLE_RATE as f64).round() as usize;
        Self { src, running: None, cursor: 0, ended: false, num_frames, buf: vec![0u8; CHUNK_FRAMES * CHANNELS * 4] }
    }
}

impl kira::sound::streaming::Decoder for PipeAudio {
    type Error = MediaError;

    fn sample_rate(&self) -> u32 {
        SAMPLE_RATE
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    fn decode(&mut self) -> Result<Vec<Frame>, MediaError> {
        if self.ended {
            self.cursor += CHUNK_FRAMES;
            return Ok(vec![Frame::ZERO; CHUNK_FRAMES]);
        }
        if self.running.is_none() {
            let start_s = self.cursor as f64 / SAMPLE_RATE as f64;
            self.running = Some(Running::spawn(&self.src.ffmpeg, &audio_args(&self.src, start_s)).map_err(|e| MediaError::Audio(e.to_string()))?);
        }
        let (_, stdout) = self.running.as_mut().expect("started above");
        let got = fill(stdout, &mut self.buf).map_err(MediaError::Io)?;
        let whole = got / (CHANNELS * 4) * (CHANNELS * 4);
        if whole == 0 {
            if let Some((running, _)) = self.running.take() {
                let (status, tail) = running.finish();
                if !status.is_some_and(|s| s.success()) {
                    log::warn!("media: ffmpeg's sound ended with {status:?}: {tail}");
                }
            }
            self.ended = true;
            self.cursor += CHUNK_FRAMES;
            return Ok(vec![Frame::ZERO; CHUNK_FRAMES]);
        }
        let frames: Vec<Frame> = self.buf[..whole]
            .chunks_exact(CHANNELS * 4)
            .map(|b| Frame::new(f32::from_le_bytes([b[0], b[1], b[2], b[3]]), f32::from_le_bytes([b[4], b[5], b[6], b[7]])))
            .collect();
        self.cursor += frames.len();
        Ok(frames)
    }

    /// Start again at `index` on the next `decode` (the running ffmpeg is stopped now).
    fn seek(&mut self, index: usize) -> Result<usize, MediaError> {
        if self.running.is_none() && !self.ended && self.cursor == index {
            return Ok(index);
        }
        self.running = None;
        self.ended = false;
        self.cursor = index;
        Ok(index)
    }
}

#[cfg(test)]
#[path = "stream_tests.rs"]
mod tests;
