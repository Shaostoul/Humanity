//! The person's own ffmpeg, for every file the app's player cannot decode itself (in-world
//! screens ladder; docs/design/media-player.md, "Container and codecs" and "Through ffmpeg as it
//! plays").
//!
//! The player in this module's parent decodes AV1 video and Opus sound itself, on purpose: two
//! royalty-free codecs, no patent pool, no codec sprawl in the exe. On Windows it also asks the
//! computer's own decoders to play an ordinary MP4 (`system.rs`). Every other file a person owns
//! (an MKV with H.264, a MOV, an AVI, a video disc) is decoded by the ffmpeg already installed on
//! their machine, AS IT PLAYS (`stream.rs`): nothing is converted ahead and nothing is written to
//! disk. The source file is never touched.
//!
//! How a file gets from "chosen" to "playing" (`begin_ingest`):
//!
//! 1. A folder is a video disc (`dvd.rs`); a disc image is refused in words.
//! 2. `media::probe` accepts it: play it directly with our decoders or the computer's own.
//! 3. It refuses it: find ffmpeg (the Settings > Media path, then the PATH environment variable,
//!    then the well-known install spots listed in `data/media/ingest.json`) and take a short look
//!    at the file on a worker (`stream::Opening`: its picture size, length and sound), then the
//!    player starts ffmpeg as it plays.
//! 4. Anything missing is an honest message naming the fix (`FFMPEG_HINT`), never a panic and
//!    never a silent black screen.
//!
//! Until 2026-11-13 step 3 converted the whole file to WebM AV1 + Opus first and kept the copy
//! under `<data dir>/media/cache`, which broke the no-caches rule (CLAUDE.md, operator 2026-07-16)
//! and made a film wait for its whole conversion. A folder an older version left behind is
//! deleted once at start (`remove_old_cache_once`).
//!
//! Nothing here is required to run in a terminal: the Settings > Media page holds the ffmpeg path
//! and the Open button on a video screen drives all of this (CLAUDE.md, GUI-first
//! configurability).

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::stream::Opening;
use super::{probe, MediaError};
use humanity_core::i18n::tf;

/// The one-line fix shown whenever ffmpeg cannot be used. Names the in-app place to set the path
/// so nobody has to open a terminal.
pub const FFMPEG_HINT: &str = "Install ffmpeg or set its path in Settings > Media";

/// The executable name on this platform.
#[cfg(target_os = "windows")]
pub const FFMPEG_EXE: &str = "ffmpeg.exe";
#[cfg(not(target_os = "windows"))]
pub const FFMPEG_EXE: &str = "ffmpeg";

/// On Windows, a child process started from a GUI app gets its own console window unless this
/// flag is set; a black console popping up mid-film would be a bug in its own right.
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// The rules from `data/media/ingest.json`: what the Open picker lists and where ffmpeg is looked
/// for. A data file (infinite-of-x) so an operator can add an install location without a
/// rebuild; the embedded copy is the fallback for an exe shipped without its data folder.
#[derive(Debug, Clone)]
pub struct IngestRules {
    /// Lower case, no leading dot, the picker's filter.
    pub video_extensions: Vec<String>,
    /// Candidate ffmpeg locations for THIS platform, environment variables already expanded, in
    /// the order they are tried.
    pub ffmpeg_candidates: Vec<PathBuf>,
}

/// Expand `%NAME%` placeholders from the environment (the Windows spelling, which is what the
/// data file uses for `%LOCALAPPDATA%` and `%ProgramFiles%`). An unset variable leaves the
/// placeholder in place, which simply makes a path that does not exist, never a panic.
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

/// Parse the ingest rules from the JSON text. Public so the test can feed it a string; the
/// platform section is chosen here.
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

/// The rules, read from the data dir's copy at every call (editable while the app runs, and
/// nothing kept that could go stale: the file is a kilobyte, read in microseconds), the embedded
/// copy when it cannot be read (said once per run in the log).
pub fn ingest_rules() -> IngestRules {
    use std::sync::atomic::{AtomicBool, Ordering};
    static NOTED: AtomicBool = AtomicBool::new(false);
    let on_disk = humanity_core::storage::data_dir().join("media").join("ingest.json");
    let text = std::fs::read_to_string(&on_disk).unwrap_or_else(|e| {
        if !NOTED.swap(true, Ordering::Relaxed) {
            humanity_core::embedded_data::note_builtin_copy("media/ingest.json", format_args!("{} could not be read ({e})", on_disk.display()));
        }
        humanity_core::embedded_data::MEDIA_INGEST_JSON.to_string()
    });
    parse_ingest_rules(&text)
}

/// Where ffmpeg is. `configured` is the Settings > Media value: a file path or a folder holding
/// the exe (empty means "find it for me"). After that: every directory on PATH, then the
/// well-known spots from the data file. `None` when nothing is found, which the caller turns into
/// `FFMPEG_HINT`.
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
    ingest_rules().ffmpeg_candidates.into_iter().find(|p| p.is_file())
}

/// A `Command` for ffmpeg that never opens a console window and never waits on stdin.
pub(crate) fn ffmpeg_command(ffmpeg: &Path) -> Command {
    let mut cmd = Command::new(ffmpeg);
    cmd.stdin(Stdio::null());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// ONE ffmpeg input for a chain of files that are one continuous stream cut at file boundaries (a
/// disc title's `VTS_01_1.VOB`, `VTS_01_2.VOB`, ...).
///
/// ffmpeg's `concat:` protocol joins the BYTES of the listed files before anything reads them,
/// which is exactly right here and exactly wrong for most formats: it works because an MPEG
/// program stream is a chain of self-describing packets and a disc's parts were cut out of one
/// such chain. The separator is `|`; a Windows drive letter's colon is not a separator, so
/// `concat:F:\a.VOB|F:\b.VOB` means what it looks like. The arguments reach ffmpeg directly, never
/// through a shell, so nothing here needs quoting.
pub fn concat_input(parts: &[PathBuf]) -> PathBuf {
    let joined: Vec<String> = parts.iter().map(|p| p.display().to_string()).collect();
    PathBuf::from(format!("concat:{}", joined.join("|")))
}

/// `Duration: 00:00:02.00, start: ...` from ffmpeg's stream summary on stderr, in seconds. `None`
/// when the summary has no duration (a raw stream).
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

/// Does this path end in `.<ext>`? Case-insensitive, and false for a path with no extension at
/// all.
pub fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case(ext)).unwrap_or(false)
}

/// What `begin_ingest` decided for a file.
pub enum Ingest {
    /// The player opens this file as it is (our decoders, or the computer's own).
    Direct(PathBuf),
    /// The person's ffmpeg decodes it as it plays. `opening` is the short look at the file running
    /// on a worker; its answer goes to `VideoPlayer::open_stream`. `why` is the reason our own
    /// decoders could not take it, phrased as the middle of a sentence ("is an MKV file with H.264
    /// video, which this app cannot play as it is").
    Stream { opening: Opening, why: String },
    /// Nothing can play, and the message names the fix.
    Failed(String),
}

/// What plays once a stream's look has answered (`Opening::poll`, `Ingest::wait`).
pub enum Ready {
    /// Play the file as it is: a direct open, or a file whose sound only ffmpeg could play when
    /// ffmpeg could not run after all (played silent rather than not at all).
    Direct(PathBuf),
    /// Play it through ffmpeg: `VideoPlayer::open_stream`.
    Stream(super::stream::StreamSource),
    Failed(String),
}

impl Ingest {
    /// Wait for a stream's look (up to `timeout`), on a thread that may block.
    pub fn wait(self, timeout: std::time::Duration) -> Ready {
        match self {
            Ingest::Direct(p) => Ready::Direct(p),
            Ingest::Failed(m) => Ready::Failed(m),
            Ingest::Stream { opening, .. } => opening.wait(timeout),
        }
    }
}

/// Why the player refused a file, as the start of a sentence for the screen ("<name> is not a
/// WebM/Matroska file"). Errors that ffmpeg cannot fix (no video track, unreadable file) are
/// returned as `Err`.
fn why_ffmpeg(e: &MediaError) -> Result<String, String> {
    match e {
        // What the file is, in plain names, and (where one applies) the sentence about this computer
        // (BUG-228): "is an MP4 file with H.265 video and AAC sound, which this app cannot play as it
        // is. This video is H.265. Windows plays it only with ...".
        MediaError::Unplayable { kind, reason } => Ok(match reason {
            Some(r) => format!("{}. {}", kind.why(), r.trim_end_matches('.')),
            None => kind.why(),
        }),
        MediaError::Demux(_) => Ok(super::kind::FileKind::default().why()),
        MediaError::UnsupportedCodec { kind, codec_id } => Ok(format!("uses the {kind} codec {codec_id}, which this player does not decode")),
        MediaError::Audio(a) => Ok(format!("has audio the player cannot take as it is ({a})")),
        MediaError::Decoder(d) => Ok(format!("could not be decoded as it is ({d})")),
        // The computer's own decoders could not play it (no decoders at all on a Windows "N"
        // edition, or H.265 without the extension): the person's own ffmpeg still can.
        MediaError::System(s) => Ok(format!("cannot be played by this computer's own decoders ({s})")),
        MediaError::NoVideoTrack => Err("the file has no video track".to_string()),
        MediaError::Io(io) => Err(format!("could not read the file: {io}")),
    }
}

/// The sentence for a film ffmpeg could not start: what it is, why ffmpeg was needed, what
/// ffmpeg said, and the fix.
pub fn ffmpeg_failed(name: &str, why: &str, error: &str) -> String {
    tf("video.ffmpeg_failed", &[("name", &name), ("why", &why), ("error", &error), ("hint", &FFMPEG_HINT)])
}

/// Hand `input` to ffmpeg to play as it goes: the short look starts on a worker. `name` and `why`
/// are for the words should ffmpeg fail; `fallback` is played as it is instead when it does.
pub(crate) fn stream(ffmpeg: PathBuf, input: PathBuf, fallback: Option<PathBuf>, name: &str, why: String) -> Ingest {
    let (name_for_words, why_for_words) = (name.to_string(), why.clone());
    let opening = Opening::spawn(ffmpeg, input, fallback, move |e| ffmpeg_failed(&name_for_words, &why_for_words, e));
    Ingest::Stream { opening, why }
}

/// Decide how `src` gets to the screen: play it directly, hand it to the person's ffmpeg to play
/// as it goes, or explain why neither is possible. `ffmpeg_setting` is the Settings > Media path
/// (empty for auto-detect).
pub fn begin_ingest(src: &Path, ffmpeg_setting: &str) -> Ingest {
    let name = src.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| src.display().to_string());
    begin_ingest_as(src, &name, ffmpeg_setting)
}

/// `begin_ingest` for a file a person knows by another name than its own: a video posted in chat
/// plays from a temporary copy named like `3f2a...-41-7.video`, and its words must say
/// "WH40K.mp4 is ..." (BUG-228).
pub fn begin_ingest_as(src: &Path, name: &str, ffmpeg_setting: &str) -> Ingest {
    let name = name.to_string();
    // A FOLDER is a disc (or the drive holding one): a different shape of film, with its own
    // module. One entry point, so the picker, the dev IPC and a remembered choice all take the
    // same road.
    if src.is_dir() {
        return super::dvd::begin_disc_ingest(src, ffmpeg_setting);
    }
    if !src.is_file() {
        return Ingest::Failed(format!("file not found: {}", src.display()));
    }
    // A disc IMAGE is a whole filesystem in a file, not a stream: ffmpeg cannot read one and
    // neither can we. Say what to do instead rather than letting the player report a container
    // it does not know.
    if has_extension(src, "iso") {
        return Ingest::Failed(format!(
            "{name} is a disc image, which is a whole filesystem in one file rather than a video. Open the disc itself, or the image's {} folder once the image is opened as a drive.",
            super::dvd::VIDEO_TS
        ));
    }
    // A file the computer's own decoders play only without its sound (2026-10-18): played with
    // its sound through ffmpeg when ffmpeg is here, else played silent rather than not at all.
    let mut silent_fallback = false;
    let why = match probe(src) {
        Ok(info) => match info.sound_lost {
            None => return Ingest::Direct(src.to_path_buf()),
            Some(codec) => {
                silent_fallback = true;
                format!("has {codec} sound, which this computer's own decoders cannot play")
            }
        },
        Err(e) => match why_ffmpeg(&e) {
            Ok(why) => why,
            Err(fatal) => return Ingest::Failed(format!("{name}: {fatal}")),
        },
    };
    let Some(ffmpeg) = find_ffmpeg(ffmpeg_setting) else {
        if silent_fallback {
            return Ingest::Direct(src.to_path_buf());
        }
        return Ingest::Failed(tf("video.needs_ffmpeg", &[("name", &name), ("why", &why), ("hint", &FFMPEG_HINT)]));
    };
    let fallback = silent_fallback.then(|| src.to_path_buf());
    stream(ffmpeg, src.to_path_buf(), fallback, &name, why)
}

/// The Settings > Media status line: what was found, or the hint. Cheap enough to call per frame
/// (file checks only; ffmpeg is not run).
pub fn ffmpeg_status_line(setting: &str) -> String {
    match find_ffmpeg(setting) {
        None => format!("ffmpeg not found. {FFMPEG_HINT}."),
        Some(p) => tf("settings.media_ffmpeg_found", &[("path", &p.display())]),
    }
}

/// The folder versions before 2026-11-13 kept converted films in: `<data dir>/media/cache`,
/// beside config.json (the per-user dir, the portable folder, or `HUMANITY_DATA_DIR`).
pub fn old_cache_dir(data_root: &Path) -> PathBuf {
    data_root.join("media").join("cache")
}

/// Delete the converted-film folder an older version left under `data_root`, with a log line
/// naming what went and how much. True when there was one. Best effort: a file another program
/// holds open stays, and is tried again at the next start.
pub fn remove_old_cache(data_root: &Path) -> bool {
    let dir = old_cache_dir(data_root);
    if !dir.is_dir() {
        return false;
    }
    let (mut files, mut bytes) = (0u64, 0u64);
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        files += 1;
        bytes += e.metadata().map(|m| m.len()).unwrap_or(0);
    }
    match std::fs::remove_dir_all(&dir) {
        Ok(()) => log::info!(
            "[Media] removed the converted-video folder an older version kept ({}: {files} file(s), {:.1} MB); films now play through ffmpeg as they go and nothing is kept",
            dir.display(),
            bytes as f64 / 1.0e6
        ),
        Err(e) => log::warn!("[Media] could not remove the old converted-video folder {}: {e}", dir.display()),
    }
    // `media/` itself goes too when the cache was all it held (it held nothing else).
    if let Some(media) = dir.parent() {
        let _ = std::fs::remove_dir(media);
    }
    true
}

/// `remove_old_cache` against the running app's data folder, once per run (the first frame,
/// `gui::pages::files::poll_upload`, beside the other "what a run left behind" sweeps).
pub fn remove_old_cache_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    // A test run never touches the person's own data folder (pages that call this are drawn by
    // tests; the tests drive `remove_old_cache` on scratch folders).
    if cfg!(test) {
        return;
    }
    static DONE: AtomicBool = AtomicBool::new(false);
    if !DONE.swap(true, Ordering::SeqCst) {
        let cfg = crate::config::AppConfig::config_path();
        if let Some(root) = cfg.parent() {
            remove_old_cache(root);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("HUMANITY_REPO_ROOT")).join("tests").join("fixtures").join("media")
    }

    fn scratch(name: &str) -> humanity_core::test_temp::TempPath {
        humanity_core::test_temp::dir(&format!("ffmpeg_{name}"))
    }

    /// The MP4 fixture (H.264 + AAC) is refused by the player's own decoders, and the refusal says
    /// what the file IS, in plain names (BUG-228: it used to pass on the Matroska reader's
    /// "Expected: Ebml Found: Unknown"). (Windows's own decoders play it: media/system_tests.rs;
    /// they are off here.)
    #[test]
    fn probe_refuses_the_mp4_fixture_by_name() {
        super::super::system_decoders_off_on_this_thread();
        let mp4 = fixtures().join("moving-box-h264-aac.mp4");
        assert!(mp4.is_file(), "{}", mp4.display());
        let err = probe(&mp4).expect_err("an H.264 MP4 is not ours to decode");
        assert!(matches!(err, MediaError::Unplayable { .. }), "{err:?}");
        let why = why_ffmpeg(&err).expect("a file we cannot decode is something ffmpeg plays");
        assert_eq!(why, "is an MP4 file with H.264 video and AAC sound, which this app cannot play as it is");
    }

    #[test]
    fn duration_lines_parse() {
        let stderr = "Input #0, mov,mp4\n  Duration: 00:01:02.50, start: 0.000000, bitrate: 91 kb/s\n";
        assert_eq!(parse_duration_s(stderr), Some(62.5));
        assert_eq!(parse_duration_s("no summary here"), None);
        assert_eq!(parse_duration_s("  Duration: N/A, start: 0.0"), None);
    }

    #[test]
    fn expand_env_replaces_known_vars_and_leaves_unknown() {
        std::env::set_var("HUM_FFMPEG_TEST_VAR", "C:/apps");
        assert_eq!(expand_env("%HUM_FFMPEG_TEST_VAR%/ffmpeg/bin"), "C:/apps/ffmpeg/bin");
        assert_eq!(expand_env("%HUM_NO_SUCH_VAR_XYZ%/x"), "%HUM_NO_SUCH_VAR_XYZ%/x");
        assert_eq!(expand_env("plain/path"), "plain/path");
        assert_eq!(expand_env("odd%"), "odd%");
        assert_eq!(expand_env("100%%"), "100%%", "an empty name is left alone");
    }

    #[test]
    fn ingest_rules_parse_extensions_and_platform_candidates() {
        let r = parse_ingest_rules(humanity_core::embedded_data::MEDIA_INGEST_JSON);
        for ext in ["webm", "mkv", "mp4", "mov", "m4v", "avi", "mpg", "mpeg", "wmv", "flv", "ogv", "ts", "vob", "m2v", "vro"] {
            assert!(r.video_extensions.iter().any(|e| e == ext), "missing {ext}");
        }
        // A disc IMAGE is not a video file and must never be offered by the picker: an image is a
        // filesystem, not a stream.
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
        // A wrong setting falls through to auto-detect rather than failing outright; whatever that
        // finds is not this scratch file.
        let missing = d.join("nowhere").display().to_string();
        assert_ne!(find_ffmpeg(&missing), Some(d.join("nowhere")));
    }

    /// ffmpeg that cannot start says so, naming the fix, and never panics: an ffmpeg setting
    /// pointing at a text file that is not ffmpeg. The look runs on a worker, so the words come
    /// from its answer.
    #[test]
    fn a_broken_ffmpeg_setting_fails_with_the_hint() {
        super::super::system_decoders_off_on_this_thread(); // the ffmpeg road, not Windows's decoders
        let d = scratch("broken");
        let fake = d.join(FFMPEG_EXE);
        std::fs::write(&fake, b"echo").unwrap();
        let mp4 = fixtures().join("moving-box-h264-aac.mp4");
        match begin_ingest(&mp4, &fake.display().to_string()).wait(std::time::Duration::from_secs(30)) {
            Ready::Failed(msg) => {
                assert!(msg.contains(FFMPEG_HINT), "{msg}");
                assert!(msg.contains("moving-box-h264-aac.mp4"), "{msg}");
            }
            Ready::Stream(_) => panic!("a text file is not ffmpeg"),
            Ready::Direct(_) => panic!("an MP4 cannot play directly with Windows's decoders off"),
        }
        match begin_ingest(&d.join("missing.mp4"), "") {
            Ingest::Failed(msg) => assert!(msg.contains("not found"), "{msg}"),
            _ => panic!("a missing file is a failure"),
        }
    }

    /// A disc image is refused in words, naming what to do instead, rather than being handed to a
    /// player that would report an unknown container. The file need not be a real image: the
    /// extension is the whole point, because opening the filesystem inside is what we do not do.
    #[test]
    fn a_disc_image_is_refused_with_what_to_do_instead() {
        let d = scratch("iso");
        let iso = d.join("home_movies.ISO");
        std::fs::write(&iso, b"not really an image").unwrap();
        match begin_ingest(&iso, "") {
            Ingest::Failed(msg) => {
                assert!(msg.contains("disc image"), "{msg}");
                assert!(msg.contains("VIDEO_TS"), "the message names the folder to open instead: {msg}");
                assert!(msg.contains("home_movies.ISO"), "{msg}");
            }
            _ => panic!("a disc image is not a video stream"),
        }
    }

    /// The chain a disc title is handed to ffmpeg as.
    #[test]
    fn a_multi_part_film_has_one_concat_input() {
        let d = scratch("parts");
        let a = d.join("VTS_01_1.VOB");
        let b = d.join("VTS_01_2.VOB");
        let joined = concat_input(&[a.clone(), b.clone()]).display().to_string();
        assert!(joined.starts_with("concat:"), "{joined}");
        assert_eq!(joined.matches('|').count(), 1, "one separator between two parts: {joined}");
        assert!(joined.contains(&a.display().to_string()) && joined.contains(&b.display().to_string()));
    }

    /// The real thing, when the machine has ffmpeg: the MP4 fixture (Windows's decoders off) goes
    /// to ffmpeg to play as it goes, the look finds its picture and sound, and NOTHING is written:
    /// not beside the fixture, not anywhere a cache could be. Skipped (printed) without ffmpeg.
    #[test]
    fn an_mp4_goes_to_ffmpeg_as_it_plays() {
        if find_ffmpeg("").is_none() {
            println!("skipped: no ffmpeg on this machine");
            return;
        }
        super::super::system_decoders_off_on_this_thread();
        let mp4 = fixtures().join("moving-box-h264-aac.mp4");
        let before: Vec<_> = std::fs::read_dir(fixtures()).unwrap().flatten().map(|e| e.file_name()).collect();
        let src = match begin_ingest(&mp4, "") {
            Ingest::Stream { opening, why } => {
                assert!(why.contains("H.264"), "the screen is told why ffmpeg plays it: {why}");
                match opening.wait(std::time::Duration::from_secs(30)) {
                    Ready::Stream(src) => src,
                    Ready::Failed(e) => panic!("{e}"),
                    Ready::Direct(p) => panic!("no fallback for a file with no sound problem: {}", p.display()),
                }
            }
            Ingest::Failed(msg) => panic!("{msg}"),
            Ingest::Direct(_) => panic!("an H.264 MP4 cannot play directly with Windows's decoders off"),
        };
        assert_eq!((src.width, src.height, src.fps), (320, 180, (30, 1)));
        assert_eq!(src.summary.audio_codec.as_deref(), Some("aac"));
        assert!((src.summary.duration_s.unwrap() - 2.0).abs() < 0.1);
        let after: Vec<_> = std::fs::read_dir(fixtures()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(before, after, "nothing written beside the source");
    }

    /// THE OLD CACHE GOES, ONCE, WITH A LOG LINE: a data folder holding `media/cache` with a
    /// converted film and a half-written one (what an older version, or one that crashed
    /// mid-conversion, left) is emptied of it at start; a second start finds nothing to do; the
    /// rest of the data folder is untouched.
    ///
    /// RED first (2026-11-13, `remove_old_cache` stubbed to do nothing and answer false): "there
    /// was one".
    #[test]
    fn the_old_cache_folder_is_removed_once_at_start() {
        let d = scratch("oldcache");
        let cache = old_cache_dir(&d);
        std::fs::create_dir_all(&cache).unwrap();
        std::fs::write(cache.join("0436eb1e.webm"), vec![0u8; 4096]).unwrap();
        std::fs::write(cache.join("9f1c2d3e.webm.part"), vec![0u8; 512]).unwrap();
        std::fs::write(d.join("config.json"), b"{}").unwrap();
        assert!(remove_old_cache(&d), "there was one");
        assert!(!cache.exists(), "the old cache folder is gone");
        assert!(!d.join("media").exists(), "and the media folder it alone filled");
        assert!(d.join("config.json").is_file(), "nothing else is touched");
        assert!(!remove_old_cache(&d), "a second start has nothing to do");
    }
}
