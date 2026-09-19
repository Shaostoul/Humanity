//! Transcode on ingest (in-world screens ladder; docs/design/media-player.md,
//! "Container and codecs" and "Transcode on ingest").
//!
//! The player in this module's parent decodes exactly one format, WebM with
//! AV1 video and Opus audio, on purpose: two royalty-free codecs, no patent
//! pool, no codec sprawl in the exe. Every other file a person owns (an MP4
//! from a phone, an MKV with H.264, a MOV, an AVI) is converted ONCE, by the
//! machine's own ffmpeg, into that one format, and the converted copy is
//! what plays. The source file is never touched.
//!
//! How a file gets from "chosen" to "playing" (`begin_ingest`):
//!
//! 1. `media::probe` accepts it: play it directly, no conversion.
//! 2. It refuses it: look in the cache. The cache key is a BLAKE3 hash of
//!    the source path, size and modification time, so a re-saved file gets
//!    a fresh conversion and an unchanged one is a cache hit with NO ffmpeg
//!    run (`cache_path_in`).
//! 3. No cached copy: find ffmpeg (the Settings > Media path, then the PATH
//!    environment variable, then the well-known install spots listed in
//!    `data/media/ingest.json`), ask it once which AV1 encoder it has
//!    (`detect_encoder`: SVT-AV1 preferred, libaom as the fallback), and
//!    run the conversion on a background thread (`TranscodeJob`) with
//!    `-progress pipe:1` parsed into a percentage the screen shows.
//! 4. Anything missing is an honest message naming the fix
//!    (`FFMPEG_HINT`), never a panic and never a silent black screen.
//!
//! The exact ffmpeg command line is `transcode_args`, a pure function pinned
//! by a unit test, so a later edit cannot quietly lower the quality or drop
//! the audio. The output is capped at 720 rows (`scale=-2:min(720\,ih)`):
//! a pure-Rust AV1 decoder does 1080p at about 69 fps on the dev machine
//! with threads and 28 fps without (measured, media-player.md), and 720p is
//! 2.25x fewer pixels, so a converted film plays smoothly on more machines.
//! Sound is downmixed to stereo (`-ac 2`) because the player's Opus path
//! handles mono and stereo only (multistream Opus is refused at probe).
//!
//! Nothing here is required to run in a terminal: the Settings > Media page
//! holds the ffmpeg path and the Open button on a video screen drives all
//! of this (CLAUDE.md, GUI-first configurability).

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{probe, MediaError};

/// The one-line fix shown whenever a conversion cannot start. Names the
/// in-app place to set the path so nobody has to open a terminal.
pub const FFMPEG_HINT: &str = "Install ffmpeg or set its path in Settings > Media";

/// The executable name on this platform.
#[cfg(target_os = "windows")]
pub const FFMPEG_EXE: &str = "ffmpeg.exe";
#[cfg(not(target_os = "windows"))]
pub const FFMPEG_EXE: &str = "ffmpeg";

/// On Windows, a child process started from a GUI app gets its own console
/// window unless this flag is set; a black console popping up mid-film
/// would be a bug in its own right.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// How much of ffmpeg's stderr is kept for an error message: the tail is
/// where the reason lives ("Unknown encoder", "Invalid data found").
const STDERR_TAIL_BYTES: usize = 4096;

/// The rules from `data/media/ingest.json`: what the Open picker lists and
/// where ffmpeg is looked for. A data file (infinite-of-x) so an operator
/// can add an install location without a rebuild; the embedded copy is the
/// fallback for an exe shipped without its data folder.
#[derive(Debug, Clone)]
pub struct IngestRules {
    /// Lower case, no leading dot, the picker's filter.
    pub video_extensions: Vec<String>,
    /// Candidate ffmpeg locations for THIS platform, environment variables
    /// already expanded, in the order they are tried.
    pub ffmpeg_candidates: Vec<PathBuf>,
}

/// Expand `%NAME%` placeholders from the environment (the Windows spelling,
/// which is what the data file uses for `%LOCALAPPDATA%` and
/// `%ProgramFiles%`). An unset variable leaves the placeholder in place,
/// which simply makes a path that does not exist, never a panic.
pub fn expand_env(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(start) = rest.find('%') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) => {
                let name = &after[..end];
                match std::env::var(name) {
                    Ok(v) if !name.is_empty() => out.push_str(&v),
                    _ => {
                        out.push('%');
                        out.push_str(name);
                        out.push('%');
                    }
                }
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// Parse the ingest rules from the JSON text. Public so the test can feed
/// it a string; the platform section is chosen here.
pub fn parse_ingest_rules(text: &str) -> IngestRules {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or(serde_json::Value::Null);
    let video_extensions = v
        .get("video_extensions")
        .and_then(|e| e.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| s.trim_start_matches('.').to_lowercase()).collect())
        .unwrap_or_default();
    let platform = if cfg!(target_os = "windows") { "windows" } else { "unix" };
    let ffmpeg_candidates = v
        .get("ffmpeg_candidates")
        .and_then(|c| c.get(platform))
        .and_then(|p| p.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str()).map(|s| PathBuf::from(expand_env(s))).collect())
        .unwrap_or_default();
    IngestRules { video_extensions, ffmpeg_candidates }
}

/// The rules, read once: the data dir's copy first (editable live), the
/// embedded copy otherwise.
pub fn ingest_rules() -> &'static IngestRules {
    static RULES: OnceLock<IngestRules> = OnceLock::new();
    RULES.get_or_init(|| {
        let on_disk = crate::data_dir().join("media").join("ingest.json");
        let text = std::fs::read_to_string(&on_disk)
            .unwrap_or_else(|_| crate::embedded_data::MEDIA_INGEST_JSON.to_string());
        parse_ingest_rules(&text)
    })
}

/// Where ffmpeg is. `configured` is the Settings > Media value: a file path
/// or a folder holding the exe (empty means "find it for me"). After that:
/// every directory on PATH, then the well-known spots from the data file.
/// `None` when nothing is found, which the caller turns into `FFMPEG_HINT`.
pub fn find_ffmpeg(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let p = PathBuf::from(expand_env(configured));
        if p.is_file() {
            return Some(p);
        }
        let in_dir = p.join(FFMPEG_EXE);
        if in_dir.is_file() {
            return Some(in_dir);
        }
        log::warn!("[Media] ffmpeg_path {:?} is not ffmpeg or a folder holding it; looking elsewhere", configured);
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(FFMPEG_EXE);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    ingest_rules().ffmpeg_candidates.iter().find(|p| p.is_file()).cloned()
}

/// Which AV1 encoder ffmpeg was built with. SVT-AV1 is the fast one (a
/// 720p film converts at several times real time on a desktop); libaom is
/// the reference encoder, slower, present in almost every build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Av1Encoder {
    SvtAv1,
    LibAom,
}

impl Av1Encoder {
    /// The name ffmpeg knows it by (`-c:v <name>`).
    pub fn ffmpeg_name(self) -> &'static str {
        match self {
            Av1Encoder::SvtAv1 => "libsvtav1",
            Av1Encoder::LibAom => "libaom-av1",
        }
    }
}

/// Pick the encoder from the text of `ffmpeg -hide_banner -encoders`. Each
/// line is ` V..... libsvtav1   SVT-AV1 ...`: flags, then the name. The
/// Opus encoder must be there too (a build without libopus cannot make a
/// file the player accepts), so its absence is an error naming it.
pub fn parse_encoders(listing: &str) -> Result<Av1Encoder, String> {
    let names: Vec<&str> = listing.lines().filter_map(|l| l.split_whitespace().nth(1)).collect();
    if !names.contains(&"libopus") {
        return Err("this ffmpeg has no libopus audio encoder".to_string());
    }
    if names.contains(&"libsvtav1") {
        Ok(Av1Encoder::SvtAv1)
    } else if names.contains(&"libaom-av1") {
        Ok(Av1Encoder::LibAom)
    } else {
        Err("this ffmpeg has no AV1 video encoder (libsvtav1 or libaom-av1)".to_string())
    }
}

/// A `Command` for ffmpeg that never opens a console window and never
/// waits on stdin.
fn ffmpeg_command(ffmpeg: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.stdin(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Ask ffmpeg which encoders it has, ONCE per ffmpeg path for the life of
/// the process (the answer cannot change without swapping the binary, and
/// the Settings page asks every time its Media section is drawn).
pub fn detect_encoder(ffmpeg: &Path) -> Result<Av1Encoder, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Result<Av1Encoder, String>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().unwrap().get(ffmpeg) {
        return hit.clone();
    }
    let result = match ffmpeg_command(ffmpeg).args(["-hide_banner", "-encoders"]).output() {
        Ok(out) => parse_encoders(&String::from_utf8_lossy(&out.stdout)),
        Err(e) => Err(format!("could not run ffmpeg at {}: {e}", ffmpeg.display())),
    };
    cache.lock().unwrap().insert(ffmpeg.to_path_buf(), result.clone());
    result
}

/// THE command line, pinned by `transcode_args_are_pinned`. Everything
/// after `-i` is an output option; the order among them does not matter to
/// ffmpeg but is fixed here so the test is exact.
///
/// * `-hide_banner -y -nostdin`: no version banner in the log, overwrite a
///   half-written destination, never block on a terminal.
/// * `-map 0:v:0 -map 0:a:0? -sn -dn`: the first video stream, the first
///   audio stream if there is one, no subtitles or data streams (WebM
///   cannot carry most of them and ffmpeg would refuse the whole file).
/// * SVT-AV1 `-preset 8 -crf 35`: preset 8 converts at several times real
///   time; CRF 35 is a good streaming quality. libaom `-cpu-used 8 -crf 35
///   -b:v 0 -row-mt 1`: the same target in libaom's dialect (`-b:v 0` is
///   what makes its CRF a constant-quality mode instead of a rate cap).
/// * `-pix_fmt yuv420p`: the 8-bit 4:2:0 layout every decoder path in
///   `src/media/video.rs` handles at full speed.
/// * `-vf scale=-2:min(720\,ih)`: at most 720 rows, width to match, even
///   (the `-2`), never upscaled (a 480p source stays 480p). The backslash
///   is ffmpeg's own escape for the comma inside a filter argument; the
///   arguments go to the process directly, not through a shell, so it
///   reaches ffmpeg exactly as written here.
/// * `-c:a libopus -b:a 96k -ac 2`: Opus at 96 kbit/s, stereo (the
///   player's Opus path decodes mono and stereo only).
/// * `-progress pipe:1 -nostats`: machine-readable progress on stdout
///   (`out_time_us=`, `progress=end`), no carriage-return stats on stderr.
/// * `-f webm`: the container, stated rather than guessed from the name.
pub fn transcode_args(src: &Path, dst: &Path, encoder: Av1Encoder) -> Vec<OsString> {
    let mut args: Vec<OsString> = Vec::new();
    let mut push = |s: &str| args.push(OsString::from(s));
    for s in ["-hide_banner", "-y", "-nostdin", "-i"] {
        push(s);
    }
    args.push(src.as_os_str().to_os_string());
    let mut push = |s: &str| args.push(OsString::from(s));
    for s in ["-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn"] {
        push(s);
    }
    match encoder {
        Av1Encoder::SvtAv1 => {
            for s in ["-c:v", "libsvtav1", "-preset", "8", "-crf", "35"] {
                push(s);
            }
        }
        Av1Encoder::LibAom => {
            for s in ["-c:v", "libaom-av1", "-cpu-used", "8", "-crf", "35", "-b:v", "0", "-row-mt", "1"] {
                push(s);
            }
        }
    }
    for s in [
        "-pix_fmt", "yuv420p", "-vf", "scale=-2:min(720\\,ih)", "-c:a", "libopus", "-b:a", "96k", "-ac", "2",
        "-progress", "pipe:1", "-nostats", "-f", "webm",
    ] {
        push(s);
    }
    args.push(dst.as_os_str().to_os_string());
    args
}

/// The cache key of a source file: BLAKE3 over its path, size and
/// modification time (nanoseconds since the epoch). Editing or replacing
/// the file changes the key; moving it changes the key (a different path
/// is a different file to the player); an untouched file keeps it.
pub fn cache_key(src: &Path) -> std::io::Result<String> {
    cache_key_parts(std::slice::from_ref(&src.to_path_buf()))
}

/// The same key for a film that arrives as SEVERAL files converted as one
/// (a disc's title, `src/media/dvd.rs`): every part's path, size and
/// modification time, in order, into one hash. A single part gives exactly
/// the same digest as `cache_key`, so nothing about the one-file case
/// changed when this was added.
pub fn cache_key_parts(parts: &[PathBuf]) -> std::io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    for src in parts {
        let meta = std::fs::metadata(src)?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        hasher.update(src.to_string_lossy().as_bytes());
        hasher.update(b"\n");
        hasher.update(meta.len().to_string().as_bytes());
        hasher.update(b"\n");
        hasher.update(mtime.to_string().as_bytes());
    }
    Ok(hasher.finalize().to_hex().to_string())
}

/// ONE ffmpeg input for a chain of files that are one continuous stream cut
/// at file boundaries (a disc title's `VTS_01_1.VOB`, `VTS_01_2.VOB`, ...).
///
/// ffmpeg's `concat:` protocol joins the BYTES of the listed files before
/// anything reads them, which is exactly right here and exactly wrong for
/// most formats: it works because an MPEG program stream is a chain of
/// self-describing packets and a disc's parts were cut out of one such
/// chain. The separator is `|`; a Windows drive letter's colon is not a
/// separator, so `concat:F:\a.VOB|F:\b.VOB` means what it looks like. The
/// arguments reach ffmpeg directly, never through a shell, so nothing here
/// needs quoting.
pub fn concat_input(parts: &[PathBuf]) -> PathBuf {
    let joined: Vec<String> = parts.iter().map(|p| p.display().to_string()).collect();
    PathBuf::from(format!("concat:{}", joined.join("|")))
}

/// The media cache: `<HumanityOS data dir>/media/cache/`, beside
/// config.json (the per-user dir, the portable folder, or
/// `HUMANITY_DATA_DIR`), so a converted copy of a personal video lives with
/// the person's own files and never in the game's distributed data tree.
pub fn cache_dir() -> PathBuf {
    let cfg = crate::config::AppConfig::config_path();
    cfg.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| PathBuf::from(".")).join("media").join("cache")
}

/// Where the converted copy of `src` goes inside `cache_dir`.
pub fn cache_path_in(cache_dir: &Path, src: &Path) -> std::io::Result<PathBuf> {
    Ok(cache_dir.join(format!("{}.webm", cache_key(src)?)))
}

/// Where the converted copy of a multi-part film goes (a disc's title).
pub fn cache_path_in_parts(cache_dir: &Path, parts: &[PathBuf]) -> std::io::Result<PathBuf> {
    Ok(cache_dir.join(format!("{}.webm", cache_key_parts(parts)?)))
}

/// `Duration: 00:00:02.00, start: ...` from ffmpeg's stream summary on
/// stderr, in seconds. `None` when the summary has no duration (a raw
/// stream), in which case the percentage stays at 0 until the end.
pub fn parse_duration_s(text: &str) -> Option<f64> {
    let idx = text.find("Duration: ")?;
    let rest = &text[idx + "Duration: ".len()..];
    let token: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == ':' || *c == '.').collect();
    let mut parts = token.split(':');
    let h: f64 = parts.next()?.parse().ok()?;
    let m: f64 = parts.next()?.parse().ok()?;
    let s: f64 = parts.next()?.parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

/// One line of `-progress pipe:1` output, classified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressLine {
    /// `out_time_us=<microseconds encoded so far>`.
    OutTimeUs(u64),
    /// `progress=end`: the last block; the process exits right after.
    End,
    /// Anything else (`frame=`, `fps=`, `speed=`, `progress=continue`).
    Other,
}

/// Classify a progress line. `out_time_us` is the field used; ffmpeg also
/// prints `out_time_ms`, which (confusingly) carries microseconds as well
/// in every version since the field was added, so it is ignored rather
/// than second-guessed.
pub fn parse_progress_line(line: &str) -> ProgressLine {
    let line = line.trim();
    if line == "progress=end" {
        return ProgressLine::End;
    }
    if let Some(v) = line.strip_prefix("out_time_us=") {
        if let Ok(us) = v.trim().parse::<i64>() {
            return ProgressLine::OutTimeUs(us.max(0) as u64);
        }
    }
    ProgressLine::Other
}

/// The percentage to show for `out_time_us` of a `duration_s` file: held
/// below 100 until `progress=end` arrives, because the muxer's trailer
/// write comes after the last timestamp and a "100%" that then sits for a
/// second reads as a hang.
pub fn percent(out_time_us: u64, duration_s: f64) -> f32 {
    if duration_s <= 0.0 {
        return 0.0;
    }
    let p = (out_time_us as f64 / 1.0e6 / duration_s * 100.0) as f32;
    p.clamp(0.0, 99.0)
}

/// The source's duration, read from `ffmpeg -i <src>` (which prints the
/// stream summary and exits complaining that no output was given). One
/// extra short process per conversion; every ffmpeg build prints it.
pub fn probe_duration_s(ffmpeg: &Path, src: &Path) -> Option<f64> {
    let out = ffmpeg_command(ffmpeg).args(["-hide_banner", "-i"]).arg(src).output().ok()?;
    parse_duration_s(&String::from_utf8_lossy(&out.stderr))
}

/// What the worker thread and the owner share.
struct JobInner {
    pct: f32,
    /// `Some` once the conversion ended, either way.
    done: Option<Result<PathBuf, String>>,
    /// The running ffmpeg, held here so `Drop` can kill it.
    child: Option<Child>,
    /// The owner went away before the conversion ended.
    cancelled: bool,
}

/// One conversion running on a background thread. Poll `percent()` for the
/// screen and `result()` for the outcome. Dropping the job KILLS ffmpeg and
/// joins the thread: a screen that is removed mid-conversion must not leave
/// an encoder running at full CPU behind it.
pub struct TranscodeJob {
    inner: Arc<Mutex<JobInner>>,
    thread: Option<JoinHandle<()>>,
    /// Where the finished file lands; the encoder writes to a `.part` twin
    /// and the worker renames it on success, so a killed conversion never
    /// leaves a half file that would later read as a cache hit.
    dst: PathBuf,
}

impl TranscodeJob {
    /// Start converting `src` to `dst` with `ffmpeg` and `encoder`.
    pub fn spawn(ffmpeg: PathBuf, encoder: Av1Encoder, src: PathBuf, dst: PathBuf) -> Self {
        let inner = Arc::new(Mutex::new(JobInner { pct: 0.0, done: None, child: None, cancelled: false }));
        let shared = Arc::clone(&inner);
        let dst_for_thread = dst.clone();
        let thread = std::thread::Builder::new()
            .name("media-transcode".into())
            .spawn(move || run_job(shared, ffmpeg, encoder, src, dst_for_thread))
            .ok();
        Self { inner, thread, dst }
    }

    /// 0..100, rising while ffmpeg reports progress, 100 once the file is
    /// finished and renamed into place.
    pub fn percent(&self) -> f32 {
        self.inner.lock().unwrap().pct
    }

    /// `None` while running; `Some(Ok(path))` with the finished file;
    /// `Some(Err(reason))` when ffmpeg failed, with its last words.
    pub fn result(&self) -> Option<Result<PathBuf, String>> {
        self.inner.lock().unwrap().done.clone()
    }

    /// The destination the job was started for.
    pub fn dst(&self) -> &Path {
        &self.dst
    }
}

impl Drop for TranscodeJob {
    fn drop(&mut self) {
        {
            let mut g = self.inner.lock().unwrap();
            g.cancelled = true;
            if let Some(child) = g.child.as_mut() {
                // Killing closes the pipes; the worker's read sees EOF and
                // finishes promptly.
                let _ = child.kill();
            }
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        let _ = std::fs::remove_file(part_path(&self.dst));
    }
}

/// The in-progress twin of a destination: `x.webm` is written as
/// `x.webm.part` and renamed when ffmpeg exits successfully.
fn part_path(dst: &Path) -> PathBuf {
    let mut s = dst.as_os_str().to_os_string();
    s.push(".part");
    PathBuf::from(s)
}

/// The worker: probe the duration, spawn ffmpeg, follow its progress on
/// stdout while a helper drains stderr, then rename the finished file.
fn run_job(inner: Arc<Mutex<JobInner>>, ffmpeg: PathBuf, encoder: Av1Encoder, src: PathBuf, dst: PathBuf) {
    let finish = |result: Result<PathBuf, String>| {
        let mut g = inner.lock().unwrap();
        if result.is_ok() {
            g.pct = 100.0;
        }
        g.done = Some(result);
    };
    if let Some(parent) = dst.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            finish(Err(format!("could not create the media cache at {}: {e}", parent.display())));
            return;
        }
    }
    let duration_s = probe_duration_s(&ffmpeg, &src).unwrap_or(0.0);
    let part = part_path(&dst);
    let args = transcode_args(&src, &part, encoder);
    log::info!(
        "[Media] transcoding {} -> {} with {} ({})",
        src.display(),
        dst.display(),
        ffmpeg.display(),
        encoder.ffmpeg_name()
    );
    let mut child = match ffmpeg_command(&ffmpeg).args(&args).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn() {
        Ok(c) => c,
        Err(e) => {
            finish(Err(format!("could not start ffmpeg at {}: {e}. {FFMPEG_HINT}", ffmpeg.display())));
            return;
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    // Park the child where Drop can reach it. If the owner already went
    // away while we were spawning, stop right here.
    {
        let mut g = inner.lock().unwrap();
        if g.cancelled {
            let _ = child.kill();
            let _ = child.wait();
            let _ = std::fs::remove_file(&part);
            return;
        }
        g.child = Some(child);
    }
    // stderr is drained on its own thread (a full pipe would stall ffmpeg);
    // only the tail is kept, which is where the reason for a failure sits.
    let stderr_thread = stderr.map(|mut e| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = e.read_to_end(&mut buf);
            let start = buf.len().saturating_sub(STDERR_TAIL_BYTES);
            String::from_utf8_lossy(&buf[start..]).to_string()
        })
    });
    if let Some(out) = stdout {
        let reader = BufReader::new(out);
        for line in reader.lines() {
            let Ok(line) = line else { break };
            match parse_progress_line(&line) {
                ProgressLine::OutTimeUs(us) => {
                    inner.lock().unwrap().pct = percent(us, duration_s);
                }
                ProgressLine::End => {
                    inner.lock().unwrap().pct = 99.0;
                }
                ProgressLine::Other => {}
            }
        }
    }
    // EOF on stdout: ffmpeg has exited (or was killed). Reap it.
    let status = {
        let mut g = inner.lock().unwrap();
        match g.child.take() {
            Some(mut c) => c.wait().ok(),
            None => None,
        }
    };
    let stderr_tail = stderr_thread.and_then(|t| t.join().ok()).unwrap_or_default();
    if inner.lock().unwrap().cancelled {
        let _ = std::fs::remove_file(&part);
        return;
    }
    match status {
        Some(s) if s.success() && part.is_file() => match std::fs::rename(&part, &dst) {
            Ok(()) => {
                log::info!("[Media] transcode finished: {}", dst.display());
                finish(Ok(dst));
            }
            Err(e) => finish(Err(format!("conversion finished but the file could not be moved into the cache: {e}"))),
        },
        Some(s) => {
            let _ = std::fs::remove_file(&part);
            let tail: Vec<&str> = stderr_tail.lines().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            finish(Err(format!("ffmpeg exited with {s}: {}", tail.join(" | ").trim())));
        }
        None => {
            let _ = std::fs::remove_file(&part);
            finish(Err("ffmpeg did not report an exit status".to_string()));
        }
    }
}

/// Does this path end in `.<ext>`? Case-insensitive, and false for a path
/// with no extension at all.
pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case(ext)).unwrap_or(false)
}

/// What `begin_ingest` decided for a file.
pub enum Ingest {
    /// The player opens this file as it is.
    Direct(PathBuf),
    /// A converted copy already sits in the cache; no ffmpeg run.
    Cached(PathBuf),
    /// A conversion is running; the job's destination plays when it ends.
    /// `why` is the reason the file could not play as it is, phrased as the
    /// middle of a sentence for the screen ("<name> uses the h264 codec,
    /// which this player does not decode").
    Transcoding { job: TranscodeJob, why: String },
    /// Nothing can play, and the message names the fix.
    Failed(String),
}

/// Why the player refused a file, as the start of a sentence for the
/// screen ("<name> is not a WebM/Matroska file"). Errors that a conversion
/// cannot fix (no video track, unreadable file) are returned as `Err`.
fn why_convert(e: &MediaError) -> Result<String, String> {
    match e {
        MediaError::Demux(_) => Ok("is not a WebM AV1 + Opus file".to_string()),
        MediaError::UnsupportedCodec { kind, codec_id } => Ok(format!("uses the {kind} codec {codec_id}, which this player does not decode")),
        MediaError::Audio(a) => Ok(format!("has audio the player cannot take as it is ({a})")),
        MediaError::Decoder(d) => Ok(format!("could not be decoded as it is ({d})")),
        MediaError::NoVideoTrack => Err("the file has no video track".to_string()),
        MediaError::Io(io) => Err(format!("could not read the file: {io}")),
    }
}

/// Decide how `src` gets to the screen: play it directly, serve the cached
/// conversion, start one, or explain why none of that is possible.
/// `ffmpeg_setting` is the Settings > Media path (empty for auto-detect);
/// `cache_dir` is where conversions live (`cache_dir()` in the app, a
/// scratch dir in tests).
pub fn begin_ingest(src: &Path, ffmpeg_setting: &str, cache_dir: &Path) -> Ingest {
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| src.display().to_string());
    // A FOLDER is a disc (or the drive holding one): a different shape of
    // film, with its own module. One entry point, so the picker, the dev
    // IPC and a remembered choice all take the same road.
    if src.is_dir() {
        return super::dvd::begin_disc_ingest(src, ffmpeg_setting, cache_dir);
    }
    if !src.is_file() {
        return Ingest::Failed(format!("file not found: {}", src.display()));
    }
    // A disc IMAGE is a whole filesystem in a file, not a stream: ffmpeg
    // cannot read one and neither can we. Say what to do instead rather
    // than letting the player report a container it does not know.
    if has_extension(src, "iso") {
        return Ingest::Failed(format!(
            "{name} is a disc image, which is a whole filesystem in one file rather than a video. Open the disc itself, or the image's {} folder once the image is opened as a drive.",
            super::dvd::VIDEO_TS
        ));
    }
    let why = match probe(src) {
        Ok(_) => return Ingest::Direct(src.to_path_buf()),
        Err(e) => match why_convert(&e) {
            Ok(why) => why,
            Err(fatal) => return Ingest::Failed(format!("{name}: {fatal}")),
        },
    };
    let dst = match cache_path_in(cache_dir, src) {
        Ok(d) => d,
        Err(e) => return Ingest::Failed(format!("{name}: could not read the file's size and date for the cache key: {e}")),
    };
    if dst.is_file() && std::fs::metadata(&dst).map(|m| m.len() > 0).unwrap_or(false) {
        log::info!("[Media] cache hit for {}: {}", src.display(), dst.display());
        return Ingest::Cached(dst);
    }
    let Some(ffmpeg) = find_ffmpeg(ffmpeg_setting) else {
        return Ingest::Failed(format!("{name} {why}; converting it needs ffmpeg, which was not found. {FFMPEG_HINT}"));
    };
    let encoder = match detect_encoder(&ffmpeg) {
        Ok(enc) => enc,
        Err(e) => return Ingest::Failed(format!("{name} {why}; {e} ({}). {FFMPEG_HINT}", ffmpeg.display())),
    };
    Ingest::Transcoding { job: TranscodeJob::spawn(ffmpeg, encoder, src.to_path_buf(), dst), why }
}

/// The Settings > Media status line: what was found and which encoder it
/// carries, or the hint. Cheap enough to call per frame (file checks plus
/// the cached encoder detection).
pub fn ffmpeg_status_line(setting: &str) -> String {
    match find_ffmpeg(setting) {
        None => format!("ffmpeg not found. {FFMPEG_HINT}."),
        Some(p) => match detect_encoder(&p) {
            Ok(enc) => format!("Found {} (AV1 encoder: {})", p.display(), enc.ffmpeg_name()),
            Err(e) => format!("Found {} but {e}", p.display()),
        },
    }
}

/// Wait for a job to finish, for tests and tooling; the screen polls
/// instead. Returns the job's result or a timeout error.
pub fn wait_for(job: &TranscodeJob, timeout: Duration) -> Result<PathBuf, String> {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(r) = job.result() {
            return r;
        }
        if std::time::Instant::now() > deadline {
            return Err(format!("transcode did not finish within {} s", timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("media")
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hum_transcode_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The MP4 fixture (H.264 + AAC) is refused by the player's probe, and
    /// the refusal names the container the player does read, so a person
    /// reading the raw error knows what the file is not.
    #[test]
    fn probe_refuses_the_mp4_fixture_by_name() {
        let mp4 = fixtures().join("moving-box-h264-aac.mp4");
        assert!(mp4.is_file(), "{}", mp4.display());
        let err = probe(&mp4).expect_err("an MP4 must not probe as WebM");
        assert!(matches!(err, MediaError::Demux(_)), "{err:?}");
        let text = err.to_string();
        assert!(text.contains("WebM/Matroska"), "the refusal names the container: {text}");
        let why = why_convert(&err).expect("a container mismatch is something a conversion fixes");
        assert!(why.contains("WebM AV1 + Opus"), "{why}");
    }

    /// THE command line. Change it here and in the doc comment together, on
    /// purpose, or this fails.
    #[test]
    fn transcode_args_are_pinned() {
        let src = Path::new("C:/videos/holiday.mp4");
        let dst = Path::new("C:/cache/abc.webm");
        let got: Vec<String> = transcode_args(src, dst, Av1Encoder::SvtAv1)
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        let want: Vec<&str> = vec![
            "-hide_banner", "-y", "-nostdin", "-i", "C:/videos/holiday.mp4",
            "-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn",
            "-c:v", "libsvtav1", "-preset", "8", "-crf", "35",
            "-pix_fmt", "yuv420p", "-vf", "scale=-2:min(720\\,ih)",
            "-c:a", "libopus", "-b:a", "96k", "-ac", "2",
            "-progress", "pipe:1", "-nostats", "-f", "webm",
            "C:/cache/abc.webm",
        ];
        assert_eq!(got, want);
        // The audio must never be dropped and the quality never lowered by
        // an edit that forgets the doc: the load-bearing pieces, by name.
        assert!(got.windows(2).any(|w| w == ["-c:a", "libopus"]));
        assert!(got.windows(2).any(|w| w == ["-crf", "35"]));
        assert!(got.contains(&"scale=-2:min(720\\,ih)".to_string()), "the 720-row cap");

        let aom: Vec<String> = transcode_args(src, dst, Av1Encoder::LibAom)
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        let want_aom: Vec<&str> = vec![
            "-hide_banner", "-y", "-nostdin", "-i", "C:/videos/holiday.mp4",
            "-map", "0:v:0", "-map", "0:a:0?", "-sn", "-dn",
            "-c:v", "libaom-av1", "-cpu-used", "8", "-crf", "35", "-b:v", "0", "-row-mt", "1",
            "-pix_fmt", "yuv420p", "-vf", "scale=-2:min(720\\,ih)",
            "-c:a", "libopus", "-b:a", "96k", "-ac", "2",
            "-progress", "pipe:1", "-nostats", "-f", "webm",
            "C:/cache/abc.webm",
        ];
        assert_eq!(aom, want_aom);
    }

    /// The cache key does not move on its own, and moves when the file's
    /// modification time or size does.
    #[test]
    fn cache_key_is_stable_and_follows_the_mtime_and_size() {
        let d = scratch("key");
        let f = d.join("clip.mp4");
        std::fs::write(&f, b"0123456789").unwrap();
        let k1 = cache_key(&f).unwrap();
        let k2 = cache_key(&f).unwrap();
        assert_eq!(k1, k2, "an untouched file keeps its key");
        assert_eq!(k1.len(), 64, "a BLAKE3 hex digest");

        // Same bytes, a later modification time: a new key.
        let later = std::fs::metadata(&f).unwrap().modified().unwrap() + Duration::from_secs(90);
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(later).unwrap();
        let k3 = cache_key(&f).unwrap();
        assert_ne!(k1, k3, "a re-saved file gets a fresh conversion");

        // Same mtime, a different size: a new key.
        std::fs::write(&f, b"01234567890").unwrap();
        std::fs::File::options().write(true).open(&f).unwrap().set_modified(later).unwrap();
        let k4 = cache_key(&f).unwrap();
        assert_ne!(k3, k4);

        // A different path with identical bytes and mtime: a new key.
        let g = d.join("other.mp4");
        std::fs::write(&g, b"01234567890").unwrap();
        std::fs::File::options().write(true).open(&g).unwrap().set_modified(later).unwrap();
        assert_ne!(cache_key(&g).unwrap(), k4);

        assert_eq!(cache_path_in(&d, &g).unwrap(), d.join(format!("{}.webm", cache_key(&g).unwrap())));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn parse_encoders_prefers_svt_then_aom_and_needs_opus() {
        let both = " V..... libaom-av1  libaom AV1\n V..... libsvtav1  SVT-AV1\n A....D libopus  libopus Opus\n";
        assert_eq!(parse_encoders(both), Ok(Av1Encoder::SvtAv1));
        let aom = " V..... libaom-av1  libaom AV1\n A....D libopus  libopus Opus\n";
        assert_eq!(parse_encoders(aom), Ok(Av1Encoder::LibAom));
        let no_av1 = " V..... libx264  H.264\n A....D libopus  Opus\n";
        assert!(parse_encoders(no_av1).unwrap_err().contains("AV1"));
        let no_opus = " V..... libsvtav1  SVT-AV1\n A....D aac  AAC\n";
        assert!(parse_encoders(no_opus).unwrap_err().contains("libopus"));
        // A substring is not a match: "libopusenc" in a description column
        // does not count, only the name column does.
        let desc_only = " V..... libsvtav1  SVT-AV1\n A....D aac  uses libopus internally\n";
        assert!(parse_encoders(desc_only).is_err());
    }

    #[test]
    fn duration_and_progress_lines_parse() {
        let stderr = "Input #0, mov,mp4\n  Duration: 00:01:02.50, start: 0.000000, bitrate: 91 kb/s\n";
        assert_eq!(parse_duration_s(stderr), Some(62.5));
        assert_eq!(parse_duration_s("no summary here"), None);
        assert_eq!(parse_progress_line("out_time_us=1966667"), ProgressLine::OutTimeUs(1_966_667));
        assert_eq!(parse_progress_line("out_time_ms=1966667"), ProgressLine::Other, "ms is ignored on purpose");
        assert_eq!(parse_progress_line("progress=end"), ProgressLine::End);
        assert_eq!(parse_progress_line("progress=continue"), ProgressLine::Other);
        assert_eq!(parse_progress_line("out_time_us=N/A"), ProgressLine::Other);
        assert_eq!(parse_progress_line("out_time_us=-5"), ProgressLine::OutTimeUs(0));
        assert_eq!(percent(1_000_000, 2.0), 50.0);
        assert_eq!(percent(2_000_000, 2.0), 99.0, "held under 100 until progress=end");
        assert_eq!(percent(5, 0.0), 0.0, "no duration, no estimate");
    }

    #[test]
    fn expand_env_replaces_known_vars_and_leaves_unknown() {
        std::env::set_var("HUM_TRANSCODE_TEST_VAR", "C:/apps");
        assert_eq!(expand_env("%HUM_TRANSCODE_TEST_VAR%/ffmpeg/bin"), "C:/apps/ffmpeg/bin");
        assert_eq!(expand_env("%HUM_NO_SUCH_VAR_XYZ%/x"), "%HUM_NO_SUCH_VAR_XYZ%/x");
        assert_eq!(expand_env("plain/path"), "plain/path");
        assert_eq!(expand_env("odd%"), "odd%");
        assert_eq!(expand_env("100%%"), "100%%", "an empty name is left alone");
    }

    #[test]
    fn ingest_rules_parse_extensions_and_platform_candidates() {
        let r = parse_ingest_rules(crate::embedded_data::MEDIA_INGEST_JSON);
        for ext in ["webm", "mkv", "mp4", "mov", "m4v", "avi", "mpg", "mpeg", "wmv", "flv", "ogv", "ts", "vob", "m2v", "vro"] {
            assert!(r.video_extensions.iter().any(|e| e == ext), "missing {ext}");
        }
        // A disc IMAGE is not a video file and must never be offered by the
        // picker: an image is a filesystem, not a stream.
        assert!(!r.video_extensions.iter().any(|e| e == "iso"), "a disc image is not a stream");
        assert!(!r.ffmpeg_candidates.is_empty());
        let r2 = parse_ingest_rules("{\"video_extensions\": [\".MP4\"]}");
        assert_eq!(r2.video_extensions, vec!["mp4".to_string()], "lower case, no dot");
        assert!(r2.ffmpeg_candidates.is_empty());
        assert!(parse_ingest_rules("not json").video_extensions.is_empty(), "garbage is empty, never a panic");
    }

    /// The configured path wins, as a file or as the folder holding it.
    #[test]
    fn find_ffmpeg_honours_a_configured_file_or_folder() {
        let d = scratch("find");
        let exe = d.join(FFMPEG_EXE);
        std::fs::write(&exe, b"not really").unwrap();
        assert_eq!(find_ffmpeg(&exe.display().to_string()), Some(exe.clone()));
        assert_eq!(find_ffmpeg(&d.display().to_string()), Some(exe.clone()));
        // A wrong setting falls through to auto-detect rather than failing
        // outright; whatever that finds is not this scratch file.
        let missing = d.join("nowhere").display().to_string();
        assert_ne!(find_ffmpeg(&missing), Some(d.join("nowhere")));
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A conversion that cannot start says so, naming the fix, and never
    /// panics: an ffmpeg setting pointing at a text file that is not ffmpeg.
    #[test]
    fn a_broken_ffmpeg_setting_fails_with_the_hint() {
        let d = scratch("broken");
        let fake = d.join(FFMPEG_EXE);
        std::fs::write(&fake, b"echo").unwrap();
        let mp4 = fixtures().join("moving-box-h264-aac.mp4");
        match begin_ingest(&mp4, &fake.display().to_string(), &d.join("cache")) {
            Ingest::Failed(msg) => {
                assert!(msg.contains(FFMPEG_HINT), "{msg}");
                assert!(msg.contains("moving-box-h264-aac.mp4"), "{msg}");
            }
            Ingest::Transcoding { .. } => panic!("a text file is not an encoder"),
            Ingest::Direct(_) | Ingest::Cached(_) => panic!("an MP4 cannot play directly"),
        }
        match begin_ingest(&d.join("missing.mp4"), "", &d) {
            Ingest::Failed(msg) => assert!(msg.contains("not found"), "{msg}"),
            _ => panic!("a missing file is a failure"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A disc image is refused in words, naming what to do instead, rather
    /// than being handed to a player that would report an unknown
    /// container. The file need not be a real image: the extension is the
    /// whole point, because opening the filesystem inside is what we do not
    /// do.
    #[test]
    fn a_disc_image_is_refused_with_what_to_do_instead() {
        let d = scratch("iso");
        let iso = d.join("home_movies.ISO");
        std::fs::write(&iso, b"not really an image").unwrap();
        match begin_ingest(&iso, "", &d.join("cache")) {
            Ingest::Failed(msg) => {
                assert!(msg.contains("disc image"), "{msg}");
                assert!(msg.contains("VIDEO_TS"), "the message names the folder to open instead: {msg}");
                assert!(msg.contains("home_movies.ISO"), "{msg}");
            }
            _ => panic!("a disc image is not a video stream"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The chain a disc title is handed to ffmpeg as, and the key that
    /// covers every part of it.
    #[test]
    fn a_multi_part_film_has_one_concat_input_and_one_key() {
        let d = scratch("parts");
        let a = d.join("VTS_01_1.VOB");
        let b = d.join("VTS_01_2.VOB");
        std::fs::write(&a, b"aaaa").unwrap();
        std::fs::write(&b, b"bbbbbb").unwrap();
        let joined = concat_input(&[a.clone(), b.clone()]).display().to_string();
        assert!(joined.starts_with("concat:"), "{joined}");
        assert_eq!(joined.matches('|').count(), 1, "one separator between two parts: {joined}");
        assert!(joined.contains(&a.display().to_string()) && joined.contains(&b.display().to_string()));

        // One part gives exactly the one-file key, so adding the multi-part
        // path changed nothing about an ordinary file's cache entry.
        assert_eq!(cache_key_parts(&[a.clone()]).unwrap(), cache_key(&a).unwrap());
        // Both parts are in the key: editing the SECOND one re-converts.
        let k1 = cache_key_parts(&[a.clone(), b.clone()]).unwrap();
        std::fs::write(&b, b"bbbbbbb").unwrap();
        assert_ne!(cache_key_parts(&[a.clone(), b.clone()]).unwrap(), k1);
        // And the order matters: a different chain is a different film.
        assert_ne!(
            cache_key_parts(&[a.clone(), b.clone()]).unwrap(),
            cache_key_parts(&[b.clone(), a.clone()]).unwrap()
        );
        assert_eq!(
            cache_path_in_parts(&d, &[a.clone(), b.clone()]).unwrap(),
            d.join(format!("{}.webm", cache_key_parts(&[a, b]).unwrap()))
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    /// The real thing, when the machine has ffmpeg: the MP4 fixture is
    /// converted once into the scratch cache, the result probes as AV1 +
    /// Opus at the fixture's size, and a second ingest of the same file is
    /// a cache hit with no job spawned. Skipped (printed) without ffmpeg.
    #[test]
    fn an_mp4_is_transcoded_once_then_served_from_the_cache() {
        if find_ffmpeg("").is_none() {
            println!("skipped: no ffmpeg on this machine");
            return;
        }
        let d = scratch("ingest");
        let cache = d.join("cache");
        let mp4 = fixtures().join("moving-box-h264-aac.mp4");
        let dst = match begin_ingest(&mp4, "", &cache) {
            Ingest::Transcoding { job, why } => {
                assert!(!why.is_empty(), "the screen is told why a conversion is running");
                assert!(job.percent() <= 100.0);
                let dst = wait_for(&job, Duration::from_secs(120)).unwrap_or_else(|e| panic!("{e}"));
                assert_eq!(job.percent(), 100.0, "the job reads 100 once the file is in place");
                dst
            }
            Ingest::Failed(msg) => panic!("{msg}"),
            Ingest::Direct(_) | Ingest::Cached(_) => panic!("a fresh scratch cache cannot hit"),
        };
        assert!(dst.is_file(), "{}", dst.display());
        assert!(dst.starts_with(&cache));
        assert!(!part_path(&dst).exists(), "the .part twin was renamed away");
        let info = probe(&dst).expect("the converted file is what the player accepts");
        assert_eq!(info.video_codec, super::super::CODEC_AV1);
        assert_eq!(info.audio_codec.as_deref(), Some(super::super::CODEC_OPUS));
        assert_eq!((info.width, info.height), (320, 180), "no upscale below 720 rows");
        assert_eq!(info.channels, 2);
        assert!((info.duration_s - 2.0).abs() < 0.2, "{}", info.duration_s);

        match begin_ingest(&mp4, "", &cache) {
            Ingest::Cached(p) => assert_eq!(p, dst, "the second ingest is served from the cache"),
            Ingest::Transcoding { .. } => panic!("a cached file must not be converted again"),
            Ingest::Failed(m) => panic!("{m}"),
            Ingest::Direct(_) => panic!("an MP4 cannot play directly"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }
}
