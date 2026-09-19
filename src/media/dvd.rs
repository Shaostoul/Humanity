//! Playing a video DISC on an in-world screen (2026-09-18).
//!
//! The operator asked: "Can I play a DVD that's in my PC on an in-game
//! display?" This module is the answer, and it holds one line firmly:
//!
//! * **A disc we can read as files, we play.** A disc the operator burned
//!   themselves, a camcorder disc, a data disc full of video files, an
//!   unprotected video disc: its `.VOB` files are ordinary MPEG-2 program
//!   streams. The MPEG-2 video patents and the AC-3 audio patents expired
//!   around 2017 and 2018, so the codec policy in
//!   `docs/design/media-player.md` has nothing against them, and nothing
//!   new ships in our exe either way: the machine's own ffmpeg reads the
//!   stream and the existing transcode-on-ingest path converts it once into
//!   the one format the player decodes (WebM AV1 + Opus).
//! * **A copy-protected commercial disc, we do not touch.** Those discs are
//!   encrypted. HumanityOS does not break disc protection, and this code
//!   contains nothing that could: when a disc turns out to be protected the
//!   screen says so plainly, in the words of `PROTECTED_MESSAGE`, and
//!   stops. There is no fallback, no second attempt and no suggestion of
//!   another route.
//!
//! What the code actually does, in order (`begin_disc_ingest`):
//!
//! 1. **Find the disc.** A video disc keeps its film in a `VIDEO_TS`
//!    folder. The person may hand us the drive (`F:\`) or the folder
//!    itself; `find_video_ts` accepts either and refuses anything else with
//!    a message that says what was looked for.
//! 2. **Choose the main title.** A disc holds several title sets, numbered:
//!    `VTS_01_*`, `VTS_02_*`. The film is the BIGGEST one; the extras and
//!    the trailers are the small ones. Inside a set, part `_0` is the MENU
//!    and the film is parts `_1` upward, in order. `main_title` decides
//!    this from nothing but a list of names and sizes, so it is tested
//!    without a disc in the drive.
//! 3. **Read a little of it.** `disc_read` looks at the first part's
//!    packets. A protected disc marks its packets as scrambled (the PES
//!    scrambling field is not zero) or simply cannot be read at all; either
//!    way the answer is the honest message, before any conversion starts.
//! 4. **Convert the whole title as one film.** The parts of a title are one
//!    continuous stream cut at a file-size boundary, so they are handed to
//!    ffmpeg as a chain through its `concat:` protocol and come out as a
//!    single converted file in the media cache. From there the existing
//!    player path takes over unchanged.
//!
//! Everything a disc could do that we cannot follow (a menu, several
//! angles, seamless branching, subtitles) is simply not done: the main
//! title plays, beginning to end.

use std::path::{Path, PathBuf};

use super::transcode::{self, Ingest, TranscodeJob, FFMPEG_HINT};

/// The name of the folder a video disc keeps its film in.
pub const VIDEO_TS: &str = "VIDEO_TS";

/// What the screen says when a disc turns out to be copy protected. It is a
/// statement of what this app does, not a workaround: no tool is named
/// here or anywhere else in the code, the data files, the tests or the
/// docs, because naming one would be pointing at circumvention.
pub const PROTECTED_MESSAGE: &str = "This disc is copy protected. HumanityOS does not break disc protection, so it cannot play it. A disc you burned yourself, a camcorder disc, or any unprotected video disc plays here.";

/// The same point, as a line appended to a conversion failure on a disc, so
/// a disc that fails LATER (ffmpeg choking on scrambled payload that the
/// header check did not catch) still gets a plain answer instead of a
/// decoder error nobody can act on.
pub const PROTECTED_HINT: &str = "If this is a commercial film disc it is copy protected, and HumanityOS does not break disc protection. A disc you burned yourself, or any unprotected video disc, plays here.";

/// What the screen says when the disc's files cannot be read at all. Both
/// causes are named because both are real and we cannot tell them apart
/// from outside: protection, and a damaged or dirty disc.
pub fn unreadable_message(detail: &str) -> String {
    format!(
        "This disc's video files could not be read ({detail}). That is what a copy protected disc does, and it is also what a scratched or dirty one does. HumanityOS does not break disc protection; a disc you burned yourself, or any unprotected video disc, plays here."
    )
}

/// The `VIDEO_TS` folder for a path the person chose: the path itself when
/// it IS that folder, or the one inside it when it holds one. `None` for
/// anything else, which the caller turns into a message naming what was
/// looked for.
///
/// The comparison ignores case both ways: Windows filesystems are
/// case-insensitive, and a disc written on another system may spell the
/// folder in lower case.
pub fn find_video_ts(path: &Path) -> Option<PathBuf> {
    if !path.is_dir() {
        return None;
    }
    if path.file_name().map(|n| n.to_string_lossy().eq_ignore_ascii_case(VIDEO_TS)).unwrap_or(false) {
        return Some(path.to_path_buf());
    }
    // Scan rather than `join("VIDEO_TS")`: the join would only find the
    // exact spelling on a case-sensitive filesystem.
    for entry in std::fs::read_dir(path).ok()?.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().eq_ignore_ascii_case(VIDEO_TS) && entry.path().is_dir() {
            return Some(entry.path());
        }
    }
    None
}

/// The title set and part numbers of a disc file name, when it is one of a
/// title's video files: `VTS_01_1.VOB` is set 1, part 1. Case is ignored
/// (`vts_01_1.vob` counts). Anything else (`VIDEO_TS.VOB`, an `.IFO`, a
/// stray file) is `None`.
///
/// Both numbers are read as plain decimals rather than pinned to the two
/// digits the specification uses, so an oddly written disc still lines up.
pub fn parse_title_vob(name: &str) -> Option<(u32, u32)> {
    let lower = name.to_ascii_lowercase();
    let stem = lower.strip_suffix(".vob")?;
    let rest = stem.strip_prefix("vts_")?;
    let (set, part) = rest.split_once('_')?;
    if set.is_empty() || part.is_empty() || !set.chars().all(|c| c.is_ascii_digit()) || !part.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    Some((set.parse().ok()?, part.parse().ok()?))
}

/// The film a disc leads with: which title set, its parts in playing order,
/// and how many bytes they are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MainTitle {
    /// The title set's number (the `01` of `VTS_01_1.VOB`).
    pub title_set: u32,
    /// The file names of its parts, in the order they play.
    pub parts: Vec<String>,
    /// The parts' total size, which is how the set was chosen.
    pub total_bytes: u64,
}

/// Choose the main title from a listing of `(file name, size in bytes)`.
/// PURE: it touches no disk, so the rule is tested without a disc.
///
/// The rule, and why:
///
/// * Only `VTS_<set>_<part>.VOB` files count. `VIDEO_TS.VOB` is the disc's
///   opening menu and is not part of any title.
/// * Part `_0` of a set is that set's MENU and is dropped. A menu is a few
///   seconds of a looping background; playing it instead of the film is
///   exactly the mistake this function exists to avoid.
/// * The set with the largest TOTAL is the film. A disc's extras and
///   trailers live in their own sets and are always smaller.
/// * A tie goes to the lower-numbered set, so the answer never depends on
///   the order the filesystem happened to list the files in.
/// * The parts play in ascending part number, which is the order they were
///   cut in.
pub fn main_title(listing: &[(String, u64)]) -> Result<MainTitle, String> {
    // (set number) -> the parts found for it, as (part number, name, size).
    let mut sets: std::collections::BTreeMap<u32, Vec<(u32, String, u64)>> = std::collections::BTreeMap::new();
    for (name, size) in listing {
        let Some((set, part)) = parse_title_vob(name) else { continue };
        if part == 0 {
            continue; // the set's menu, never the film
        }
        sets.entry(set).or_default().push((part, name.clone(), *size));
    }
    if sets.is_empty() {
        return Err(format!(
            "this disc has no title files ({VIDEO_TS} holds no VTS_01_1.VOB or later). A video disc keeps its film in numbered VOB files; a data disc with ordinary video files on it can be opened file by file instead."
        ));
    }
    // BTreeMap iterates by set number, so `max_by_key` on the total keeps
    // the FIRST set on a tie (max_by_key returns the last maximum, hence the
    // explicit fold rather than max_by_key).
    let mut best: Option<(u32, u64)> = None;
    for (set, parts) in &sets {
        let total: u64 = parts.iter().map(|(_, _, size)| *size).sum();
        match best {
            Some((_, best_total)) if total <= best_total => {}
            _ => best = Some((*set, total)),
        }
    }
    let (title_set, total_bytes) = best.expect("the map is not empty");
    let mut parts = sets.remove(&title_set).expect("the chosen set is in the map");
    parts.sort_by_key(|(part, _, _)| *part);
    Ok(MainTitle { title_set, parts: parts.into_iter().map(|(_, name, _)| name).collect(), total_bytes })
}

/// List a `VIDEO_TS` folder as `(name, size)` pairs for `main_title`.
/// Unreadable entries are skipped rather than failing the whole disc: one
/// unreadable extra must not stop a film that is perfectly readable.
pub fn list_video_ts(dir: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let Ok(meta) = e.metadata() else { continue };
            if !meta.is_file() {
                continue;
            }
            if let Some(name) = e.file_name().to_str() {
                out.push((name.to_string(), meta.len()));
            }
        }
    }
    out
}

/// What a look at a disc's first video file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscRead {
    /// Plain program-stream packets: this converts.
    Playable,
    /// The packets are MARKED as scrambled: a copy-protected disc.
    CopyProtected,
    /// The file could not be read at all, with the reason.
    Unreadable(String),
}

/// How much of the first part is examined. A program stream packs its data
/// in 2048 byte packs, so this is about 128 packs: far more than enough to
/// meet the first video packets, and small enough to be instant even on a
/// slow optical drive.
const SNIFF_BYTES: usize = 256 * 1024;

/// Look at the start of a title's first file and say whether it is
/// something we can hand to ffmpeg.
/// The Windows error for a sector the drive refuses to hand over because it
/// is encrypted (`STG_E_STATUS_COPY_PROTECTION_FAILURE`). The operating
/// system has already decided the question here, so we take its word rather
/// than reading packets that will never arrive.
///
/// Matched by CODE and not by the message text, because the text is
/// translated into the reader's own language and a match on English words
/// would quietly stop working on most of the world's machines. Proven
/// against a real pressed disc on 2026-09-18: a commercial film in F: gave
/// exactly this code on its first title part.
#[cfg(target_os = "windows")]
const COPY_PROTECTION_FAILURE: i32 = -2147286263;

/// Does this read error mean the disc is protected, rather than dirty or
/// broken? Only when the operating system says so in as many words.
fn is_copy_protection_error(e: &std::io::Error) -> bool {
    #[cfg(target_os = "windows")]
    {
        e.raw_os_error() == Some(COPY_PROTECTION_FAILURE)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = e;
        false
    }
}

pub fn disc_read(first_part: &Path) -> DiscRead {
    use std::io::Read;
    let mut file = match std::fs::File::open(first_part) {
        Ok(f) => f,
        Err(e) if is_copy_protection_error(&e) => return DiscRead::CopyProtected,
        Err(e) => return DiscRead::Unreadable(e.to_string()),
    };
    let mut buf = vec![0u8; SNIFF_BYTES];
    let mut filled = 0;
    // `read` is allowed to return less than asked for; keep going until the
    // buffer is full or the file ends. A read error partway through is the
    // unreadable answer, which is exactly what a bad sector gives.
    loop {
        match file.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => {
                filled += n;
                if filled == buf.len() {
                    break;
                }
            }
            Err(e) if is_copy_protection_error(&e) => return DiscRead::CopyProtected,
            Err(e) => return DiscRead::Unreadable(e.to_string()),
        }
    }
    buf.truncate(filled);
    if buf.is_empty() {
        return DiscRead::Unreadable("the file is empty".to_string());
    }
    match scrambled_packet(&buf) {
        Some(_) => DiscRead::CopyProtected,
        None => DiscRead::Playable,
    }
}

/// The stream id of the first packet in `bytes` that is MARKED AS
/// SCRAMBLED, if there is one.
///
/// How this works, and why it is a fact about the file rather than a guess:
/// an MPEG program stream is a chain of packets, each starting with the
/// three bytes `00 00 01` and then a stream id. For a media packet (a video
/// stream `E0`-`EF`, an audio or private stream `BD`, `C0`-`DF`) the
/// seventh byte carries flag bits, and two of them are the "scrambling
/// control" field. A plain stream leaves them at zero. An encrypted disc
/// sets them, on the packets it encrypted, to say so. So a disc that is
/// protected declares it in its own bytes, and we can stop before asking
/// ffmpeg to read something it cannot.
///
/// The seventh byte's top two bits must be `10` for a real packet header;
/// checking them throws out the byte sequences that merely LOOK like a
/// start code inside the compressed picture data (the fixture VOB has one
/// of those a couple of kilobytes in, which is why the check is here).
pub fn scrambled_packet(bytes: &[u8]) -> Option<u8> {
    let mut i = 0usize;
    while i + 9 <= bytes.len() {
        if bytes[i] != 0 || bytes[i + 1] != 0 || bytes[i + 2] != 1 {
            i += 1;
            continue;
        }
        let stream_id = bytes[i + 3];
        let is_media = stream_id == 0xBD || (0xC0..=0xEF).contains(&stream_id);
        if is_media {
            let packet_len = u16::from_be_bytes([bytes[i + 4], bytes[i + 5]]);
            let flags = bytes[i + 6];
            let header_looks_real = (flags & 0xC0) == 0x80 && packet_len != 0;
            if header_looks_real && (flags & 0x30) != 0 {
                return Some(stream_id);
            }
        }
        i += 4;
    }
    None
}

/// Get a video disc onto the screen: find its `VIDEO_TS`, pick the main
/// title, check it can be read, then convert the whole title as one film
/// (or serve the copy already in the media cache). `ffmpeg_setting` is the
/// Settings > Media path (empty means find it), `cache_dir` is the media
/// cache.
///
/// Every failure comes back as `Ingest::Failed` with a sentence a person
/// can act on, never a panic and never a silent black screen.
pub fn begin_disc_ingest(chosen: &Path, ffmpeg_setting: &str, cache_dir: &Path) -> Ingest {
    let name = disc_name(chosen);
    let Some(video_ts) = find_video_ts(chosen) else {
        return Ingest::Failed(format!(
            "{name} is not a video disc: no {VIDEO_TS} folder in {}. A video disc has a {VIDEO_TS} folder holding its VOB files; pick the disc drive, that folder, or a single video file instead.",
            chosen.display()
        ));
    };
    let title = match main_title(&list_video_ts(&video_ts)) {
        Ok(t) => t,
        Err(e) => return Ingest::Failed(format!("{name}: {e}")),
    };
    let parts: Vec<PathBuf> = title.parts.iter().map(|n| video_ts.join(n)).collect();
    log::info!(
        "[Media] disc {}: title set {} chosen, {} part(s), {} bytes: {:?}",
        video_ts.display(),
        title.title_set,
        parts.len(),
        title.total_bytes,
        title.parts
    );
    // The cache first: a disc converted once plays instantly the next time,
    // with no drive spin-up and no ffmpeg run.
    let dst = match transcode::cache_path_in_parts(cache_dir, &parts) {
        Ok(d) => d,
        Err(e) => return Ingest::Failed(format!("{name}: the disc's files could not be read ({e}).")),
    };
    if dst.is_file() && std::fs::metadata(&dst).map(|m| m.len() > 0).unwrap_or(false) {
        log::info!("[Media] disc {}: cache hit {}", video_ts.display(), dst.display());
        return Ingest::Cached(dst);
    }
    match disc_read(&parts[0]) {
        DiscRead::Playable => {}
        DiscRead::CopyProtected => {
            log::info!("[Media] disc {}: copy protected, not played", video_ts.display());
            return Ingest::Failed(PROTECTED_MESSAGE.to_string());
        }
        DiscRead::Unreadable(detail) => {
            log::info!("[Media] disc {}: unreadable ({detail})", video_ts.display());
            return Ingest::Failed(unreadable_message(&detail));
        }
    }
    let why = format!(
        "is a video disc: title {} in {} part(s), MPEG-2 video",
        title.title_set,
        parts.len()
    );
    let Some(ffmpeg) = transcode::find_ffmpeg(ffmpeg_setting) else {
        return Ingest::Failed(format!("{name} {why}; converting it needs ffmpeg, which was not found. {FFMPEG_HINT}"));
    };
    let encoder = match transcode::detect_encoder(&ffmpeg) {
        Ok(enc) => enc,
        Err(e) => return Ingest::Failed(format!("{name} {why}; {e} ({}). {FFMPEG_HINT}", ffmpeg.display())),
    };
    // One ffmpeg run over the whole chain of parts: see `concat_input`.
    let src = transcode::concat_input(&parts);
    Ingest::Transcoding { job: TranscodeJob::spawn(ffmpeg, encoder, src, dst), why }
}

/// What to call the disc on screen: the folder's own name, or the drive
/// letter when the person handed us a drive root (which has no name).
pub fn disc_name(chosen: &Path) -> String {
    chosen
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| chosen.display().to_string())
}

/// What kind of drive a browsable root is, so a picker can label it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    /// An internal hard disk or SSD.
    Fixed,
    /// A USB stick, memory card or external drive.
    Removable,
    /// A CD, DVD or Blu-ray drive with a disc in it.
    Disc,
}

/// How a drive button reads. The volume's own name when it has one, so a
/// person sees the drive they named rather than a bare letter, with the
/// letter kept in brackets because that is how Windows itself talks about
/// drives and how a path is written.
pub fn drive_label(kind: DriveKind, letter: char, volume_name: Option<&str>) -> String {
    let named = volume_name.map(|v| v.trim()).filter(|v| !v.is_empty());
    match (kind, named) {
        (DriveKind::Disc, Some(v)) => format!("{v} ({letter}:)"),
        (DriveKind::Disc, None) => format!("Disc ({letter}:)"),
        (DriveKind::Removable, Some(v)) => format!("{v} ({letter}:)"),
        (DriveKind::Removable, None) => format!("Removable ({letter}:)"),
        (DriveKind::Fixed, Some(v)) => format!("{v} ({letter}:)"),
        (DriveKind::Fixed, None) => format!("Drive ({letter}:)"),
    }
}

/// Every drive on this machine a person could browse, as
/// `(label, path, kind)` for a file picker's quick-access row.
///
/// The operator hit exactly why this exists (2026-09-18): his films live on
/// E:, the picker offered only the folders under his home directory plus the
/// disc drive, and there was no way to reach another drive at all, because
/// the picker has no place to type a path. A person with more than one drive
/// is normal, not an edge case.
///
/// Windows: ask which drive letters exist, ask what kind each one is, and
/// keep it only while its root reads as a directory, which is false for an
/// empty card reader or an optical drive with no disc. NETWORK drives are
/// deliberately left out: a share whose server has gone away can block for
/// seconds on the very call that asks about it, and this runs behind a
/// picker that redraws every frame. That is a known gap, and the honest fix
/// is somewhere to type a path, not a risky probe.
///
/// Everywhere else: nothing yet. Linux and macOS mount a volume under
/// `/media`, `/run/media` or `/Volumes` with no fixed name, so a person
/// reaches it through the ordinary folders.
pub fn drive_roots() -> Vec<(String, PathBuf, DriveKind)> {
    #[cfg(target_os = "windows")]
    {
        windows_drive_roots()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Vec::new()
    }
}

/// The disc drives that currently HOLD a disc, for callers that want only
/// those. Empty when there is no drive or no disc in it.
pub fn disc_roots() -> Vec<(String, PathBuf)> {
    drive_roots()
        .into_iter()
        .filter(|(_, _, kind)| *kind == DriveKind::Disc)
        .map(|(label, path, _)| (label, path))
        .collect()
}

/// The Windows side of `drive_roots`. Raw FFI so this adds no dependency,
/// the same way `src/engine/launch_focus.rs` asks for its parent process.
#[cfg(target_os = "windows")]
fn windows_drive_roots() -> Vec<(String, PathBuf, DriveKind)> {
    /// `GetDriveTypeW`'s answers. 1 means the letter exists but holds
    /// nothing, 4 is a network share (see the note in `drive_roots`).
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_CDROM: u32 = 5;

    #[link(name = "kernel32")]
    extern "system" {
        /// One bit per drive letter, bit 0 = A:.
        fn GetLogicalDrives() -> u32;
        fn GetDriveTypeW(root: *const u16) -> u32;
        /// Fills the volume-name buffer; everything else is optional and
        /// passed as null here because only the name is wanted.
        fn GetVolumeInformationW(
            root: *const u16,
            volume_name: *mut u16,
            volume_name_size: u32,
            serial: *mut u32,
            max_component_len: *mut u32,
            flags: *mut u32,
            fs_name: *mut u16,
            fs_name_size: u32,
        ) -> i32;
    }

    /// The name a person gave the drive, when it has one. A drive that
    /// refuses the question simply has no name, which is not an error.
    fn volume_name(wide_root: &[u16]) -> Option<String> {
        let mut buf = [0u16; 64];
        let ok = unsafe {
            GetVolumeInformationW(
                wide_root.as_ptr(),
                buf.as_mut_ptr(),
                buf.len() as u32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        if ok == 0 {
            return None;
        }
        let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
        let name = String::from_utf16_lossy(&buf[..end]);
        if name.trim().is_empty() {
            None
        } else {
            Some(name)
        }
    }

    let mask = unsafe { GetLogicalDrives() };
    let mut out = Vec::new();
    for bit in 0..26u32 {
        if mask & (1 << bit) == 0 {
            continue;
        }
        let letter = (b'A' + bit as u8) as char;
        let root = format!("{letter}:\\");
        // A UTF-16 string ending in a zero, which is what the API wants.
        let wide: Vec<u16> = root.encode_utf16().chain(std::iter::once(0)).collect();
        let kind = match unsafe { GetDriveTypeW(wide.as_ptr()) } {
            DRIVE_FIXED => DriveKind::Fixed,
            DRIVE_REMOVABLE => DriveKind::Removable,
            DRIVE_CDROM => DriveKind::Disc,
            _ => continue,
        };
        let path = PathBuf::from(&root);
        // An empty drive's root is not a readable directory, so this is
        // also the "is there a disc or a card in it" question.
        if path.is_dir() {
            out.push((drive_label(kind, letter, volume_name(&wide).as_deref()), path, kind));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A drive button reads as the drive's own name when it has one, and
    /// falls back to what kind of drive it is. The letter is always in
    /// brackets, because that is how a path is written and how Windows
    /// talks about drives.
    #[test]
    fn a_drive_button_prefers_the_name_its_owner_gave_it() {
        assert_eq!(drive_label(DriveKind::Fixed, 'E', Some("Movies")), "Movies (E:)");
        assert_eq!(drive_label(DriveKind::Fixed, 'D', None), "Drive (D:)");
        assert_eq!(drive_label(DriveKind::Removable, 'G', Some("BACKUP")), "BACKUP (G:)");
        assert_eq!(drive_label(DriveKind::Removable, 'G', None), "Removable (G:)");
        assert_eq!(drive_label(DriveKind::Disc, 'F', None), "Disc (F:)");
        // A volume name that is only spaces is no name at all.
        assert_eq!(drive_label(DriveKind::Fixed, 'E', Some("   ")), "Drive (E:)");
        // Every label carries its letter, which is what the picker's own
        // test asserts about the row.
        for kind in [DriveKind::Fixed, DriveKind::Removable, DriveKind::Disc] {
            assert!(drive_label(kind, 'X', None).contains("(X:)"));
        }
    }

    fn listing(items: &[(&str, u64)]) -> Vec<(String, u64)> {
        items.iter().map(|(n, s)| (n.to_string(), *s)).collect()
    }

    fn fixture_video_ts() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("media").join("VIDEO_TS")
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hum_dvd_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// The names a disc uses, read the way the chooser reads them.
    #[test]
    fn title_vob_names_parse_and_other_files_do_not() {
        assert_eq!(parse_title_vob("VTS_01_1.VOB"), Some((1, 1)));
        assert_eq!(parse_title_vob("VTS_01_0.VOB"), Some((1, 0)), "the menu parses; the chooser drops it");
        assert_eq!(parse_title_vob("vts_12_3.vob"), Some((12, 3)), "case does not matter");
        assert_eq!(parse_title_vob("VIDEO_TS.VOB"), None, "the disc menu belongs to no title");
        assert_eq!(parse_title_vob("VTS_01_1.IFO"), None, "not a video file");
        assert_eq!(parse_title_vob("VTS_01.VOB"), None, "no part number");
        assert_eq!(parse_title_vob("VTS_ab_1.VOB"), None, "not a number");
        assert_eq!(parse_title_vob("holiday.vob"), None, "a plain file is not part of a title");
    }

    /// THE rule this increment exists for: the biggest title set is the
    /// film, its menu is not, and its parts play in order. Proven able to
    /// fail: dropping the `part == 0` skip in `main_title` puts
    /// VTS_01_0.VOB at the head of the parts and this fails on the first
    /// `assert_eq`.
    #[test]
    fn the_main_title_is_the_biggest_set_without_its_menu_in_part_order() {
        let disc = listing(&[
            ("VIDEO_TS.IFO", 12_288),
            ("VIDEO_TS.VOB", 3_000_000), // the disc's opening menu: never a title
            ("VTS_01_0.VOB", 5_000_000), // the title set's menu: never played
            ("VTS_01_2.VOB", 900_000_000),
            ("VTS_01_1.VOB", 1_073_741_824),
            ("VTS_01_3.VOB", 400_000_000),
            ("VTS_02_0.VOB", 2_000_000),
            ("VTS_02_1.VOB", 60_000_000), // an extra
            ("VTS_01_0.IFO", 40_960),
        ]);
        let t = main_title(&disc).expect("this disc has a film on it");
        assert_eq!(t.title_set, 1);
        assert_eq!(t.parts, vec!["VTS_01_1.VOB", "VTS_01_2.VOB", "VTS_01_3.VOB"], "in playing order");
        assert_eq!(t.total_bytes, 1_073_741_824 + 900_000_000 + 400_000_000, "the menu is not counted");
        assert!(!t.parts.iter().any(|p| p.ends_with("_0.VOB")), "no menu among the parts");

        // The film is not always set 1: a disc whose extras come first.
        let other = listing(&[("VTS_01_1.VOB", 50_000_000), ("VTS_02_1.VOB", 800_000_000), ("VTS_02_2.VOB", 700_000_000)]);
        assert_eq!(main_title(&other).unwrap().title_set, 2);

        // A tie goes to the lower set, so the answer never depends on the
        // order the folder listed its files in.
        let tie = listing(&[("VTS_03_1.VOB", 100), ("VTS_02_1.VOB", 100)]);
        assert_eq!(main_title(&tie).unwrap().title_set, 2);
        let tie_reversed = listing(&[("VTS_02_1.VOB", 100), ("VTS_03_1.VOB", 100)]);
        assert_eq!(main_title(&tie_reversed).unwrap().title_set, 2);

        // Parts beyond nine, in case a disc ever has them: ordered as
        // numbers, not as text ("10" after "9", not before "2").
        let many = listing(&[("VTS_01_10.VOB", 10), ("VTS_01_2.VOB", 10), ("VTS_01_9.VOB", 10)]);
        assert_eq!(many.len(), 3);
        assert_eq!(main_title(&many).unwrap().parts, vec!["VTS_01_2.VOB", "VTS_01_9.VOB", "VTS_01_10.VOB"]);
    }

    /// A folder with menus only, or with no VOB files at all, is refused in
    /// words rather than played silently as a menu.
    #[test]
    fn a_disc_with_no_title_files_is_refused_in_words() {
        let menus_only = listing(&[("VIDEO_TS.VOB", 10), ("VTS_01_0.VOB", 10), ("VTS_01_0.IFO", 10)]);
        let err = main_title(&menus_only).expect_err("menus are not a film");
        assert!(err.contains("no title files"), "{err}");
        assert!(err.contains("VTS_01_1.VOB"), "the message names what a title file looks like: {err}");
        let empty = main_title(&[]).expect_err("nothing is not a film");
        assert!(empty.contains("no title files"), "{empty}");
    }

    /// The disc folder is found whether the person hands us the drive or
    /// the folder, and an ordinary folder is not mistaken for a disc.
    #[test]
    fn video_ts_is_found_from_the_drive_or_from_the_folder_itself() {
        let root = scratch("find");
        let disc = root.join("disc_root");
        std::fs::create_dir_all(disc.join(VIDEO_TS)).unwrap();
        assert_eq!(find_video_ts(&disc), Some(disc.join(VIDEO_TS)), "a drive root holding the folder");
        assert_eq!(find_video_ts(&disc.join(VIDEO_TS)), Some(disc.join(VIDEO_TS)), "the folder itself");
        let plain = root.join("photos");
        std::fs::create_dir_all(&plain).unwrap();
        assert_eq!(find_video_ts(&plain), None, "an ordinary folder is not a disc");
        assert_eq!(find_video_ts(&root.join("nowhere")), None, "a path that is not there");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The fixture disc, on disk: the tree is really shaped like a disc and
    /// the chooser picks its two-part title, not its menu and not the
    /// second title set.
    #[test]
    fn the_fixture_disc_yields_its_two_part_main_title() {
        let dir = fixture_video_ts();
        assert!(dir.is_dir(), "{}", dir.display());
        let t = main_title(&list_video_ts(&dir)).expect("the fixture disc has a film");
        assert_eq!(t.title_set, 1);
        assert_eq!(t.parts, vec!["VTS_01_1.VOB", "VTS_01_2.VOB"]);
        assert_eq!(t.total_bytes, 49_152, "the two parts, not the menu");
        // And the whole thing is reachable from the folder that holds it.
        assert_eq!(find_video_ts(dir.parent().unwrap()), Some(dir.clone()));
    }

    /// The fixture's own bytes are plain program-stream packets, and a copy
    /// whose packets are MARKED scrambled (which is what a protected disc
    /// does) is recognised as protected. The copy is made here, in the
    /// test, by setting the flag bits: nothing is decrypted anywhere, this
    /// is the failure path being exercised.
    #[test]
    fn a_plain_disc_reads_and_a_scrambled_one_is_named_as_protected() {
        let first = fixture_video_ts().join("VTS_01_1.VOB");
        assert_eq!(disc_read(&first), DiscRead::Playable, "the fixture disc is not protected");
        let bytes = std::fs::read(&first).unwrap();
        assert_eq!(scrambled_packet(&bytes), None);

        // Mark every real packet header as scrambled, the way an encrypted
        // disc's headers are marked.
        let mut marked = bytes.clone();
        let mut i = 0usize;
        let mut marks = 0;
        while i + 9 <= marked.len() {
            let start = marked[i] == 0 && marked[i + 1] == 0 && marked[i + 2] == 1;
            let id = marked[i + 3];
            let media = id == 0xBD || (0xC0..=0xEF).contains(&id);
            if start && media && (marked[i + 6] & 0xC0) == 0x80 {
                marked[i + 6] |= 0x10;
                marks += 1;
            }
            i += 1;
        }
        assert!(marks > 0, "the fixture has packet headers to mark");
        assert!(scrambled_packet(&marked).is_some(), "a marked stream is seen as scrambled");

        let d = scratch("protected");
        let video_ts = d.join(VIDEO_TS);
        std::fs::create_dir_all(&video_ts).unwrap();
        std::fs::write(video_ts.join("VTS_01_1.VOB"), &marked).unwrap();
        assert_eq!(disc_read(&video_ts.join("VTS_01_1.VOB")), DiscRead::CopyProtected);
        match begin_disc_ingest(&d, "", &d.join("cache")) {
            Ingest::Failed(msg) => {
                assert_eq!(msg, PROTECTED_MESSAGE);
                assert!(msg.contains("copy protected"), "{msg}");
                assert!(msg.contains("does not break disc protection"), "{msg}");
            }
            _ => panic!("a protected disc must not be converted"),
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A file that cannot be read at all gets its own honest message, which
    /// names both causes rather than blaming protection for a scratch.
    #[test]
    fn an_unreadable_disc_file_says_so() {
        let d = scratch("unreadable");
        let missing = d.join("VTS_01_1.VOB");
        match disc_read(&missing) {
            DiscRead::Unreadable(detail) => {
                let msg = unreadable_message(&detail);
                assert!(msg.contains("could not be read"), "{msg}");
                assert!(msg.contains("scratched"), "{msg}");
                assert!(msg.contains("does not break disc protection"), "{msg}");
            }
            other => panic!("{other:?}"),
        }
        let empty = d.join("empty.VOB");
        std::fs::write(&empty, b"").unwrap();
        assert!(matches!(disc_read(&empty), DiscRead::Unreadable(_)), "an empty file is not a film");
        let _ = std::fs::remove_dir_all(&d);
    }

    /// A folder that is not a disc is refused with a message naming what
    /// was looked for, before anything is converted.
    #[test]
    fn a_folder_that_is_not_a_disc_is_refused_with_a_clear_message() {
        let d = scratch("notadisc");
        std::fs::write(d.join("holiday.mp4"), b"x").unwrap();
        match begin_disc_ingest(&d, "", &d.join("cache")) {
            Ingest::Failed(msg) => {
                assert!(msg.contains("not a video disc"), "{msg}");
                assert!(msg.contains(VIDEO_TS), "the message names the folder it looked for: {msg}");
            }
            _ => panic!("an ordinary folder is not a disc"),
        }
        // A VIDEO_TS with nothing playable in it: the other refusal.
        let hollow = scratch("hollow");
        std::fs::create_dir_all(hollow.join(VIDEO_TS)).unwrap();
        std::fs::write(hollow.join(VIDEO_TS).join("VTS_01_0.VOB"), b"menu").unwrap();
        match begin_disc_ingest(&hollow, "", &hollow.join("cache")) {
            Ingest::Failed(msg) => assert!(msg.contains("no title files"), "{msg}"),
            _ => panic!("a menu is not a film"),
        }
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&hollow);
    }

    /// The real thing, when the machine has ffmpeg: the fixture disc is
    /// converted once into a scratch cache as ONE film covering both parts,
    /// the result is the format the player decodes (AV1 + Opus), and a
    /// second open of the same disc is served from the cache with no
    /// ffmpeg run. Skipped (printed) without ffmpeg.
    #[test]
    fn the_fixture_disc_converts_as_one_film_and_then_hits_the_cache() {
        if transcode::find_ffmpeg("").is_none() {
            println!("skipped: no ffmpeg on this machine");
            return;
        }
        let d = scratch("ingest");
        let cache = d.join("cache");
        let disc = fixture_video_ts();
        let dst = match begin_disc_ingest(&disc, "", &cache) {
            Ingest::Transcoding { job, why } => {
                assert!(why.contains("video disc"), "the screen is told what it is converting: {why}");
                assert!(why.contains("2 part"), "{why}");
                transcode::wait_for(&job, std::time::Duration::from_secs(180)).unwrap_or_else(|e| panic!("{e}"))
            }
            Ingest::Failed(msg) => panic!("{msg}"),
            Ingest::Direct(_) | Ingest::Cached(_) => panic!("a fresh scratch cache cannot hit"),
        };
        assert!(dst.is_file(), "{}", dst.display());
        let info = super::super::probe(&dst).expect("the converted disc is what the player accepts");
        assert_eq!(info.video_codec, super::super::CODEC_AV1);
        assert_eq!(info.audio_codec.as_deref(), Some(super::super::CODEC_OPUS));
        assert_eq!((info.width, info.height), (320, 180));
        // BOTH parts: one part alone is one second. Anything under 1.5 s
        // means the chain was cut, which is the bug this test is here for.
        assert!(info.duration_s > 1.5, "both parts converted as one film: {} s", info.duration_s);

        match begin_disc_ingest(&disc, "", &cache) {
            Ingest::Cached(p) => assert_eq!(p, dst, "the second open is served from the cache"),
            Ingest::Transcoding { .. } => panic!("a converted disc must not be converted again"),
            Ingest::Failed(msg) => panic!("{msg}"),
            Ingest::Direct(p) => panic!("a disc is never played as it is: {}", p.display()),
        }
        let _ = std::fs::remove_dir_all(&d);
    }

    /// Asking the machine for its drives must never panic and must never
    /// offer a path that is not there. On a machine with an empty optical
    /// drive the disc answer is simply empty.
    ///
    /// The label is NOT asserted to start with "Disc": a disc carries a
    /// volume name and we show it, so the button reads as the disc's own
    /// title rather than the word Disc, which is more use to a person with
    /// several of them. The shape that must hold is the drive letter in
    /// brackets, which is how a path is written.
    #[test]
    fn disc_roots_are_real_directories_or_nothing() {
        for (label, path) in disc_roots() {
            assert!(path.is_dir(), "{label} -> {}", path.display());
            assert!(label.contains("(") && label.contains(":)"), "a disc names its letter: {label}");
        }
    }

    /// Point this at a REAL disc and see what the app would do with it,
    /// which is the one thing the disc work could not check without one in
    /// the drive. Ignored by default because it needs a disc. Run it with
    /// the drive named in the environment variable HUMANITY_DISC_DRIVE, for
    /// example F: followed by a backslash, and pass --ignored --nocapture so
    /// the test runs and prints.
    ///
    /// It only READS and reports; it converts nothing. A commercial disc is
    /// expected to be refused, and the point is to see the refusal come from
    /// the intended check rather than from an unrelated error.
    #[test]
    #[ignore]
    fn probe_a_real_disc() {
        let Ok(drive) = std::env::var("HUMANITY_DISC_DRIVE") else {
            println!("set HUMANITY_DISC_DRIVE to a drive with a disc in it");
            return;
        };
        let root = PathBuf::from(&drive);
        println!("drive: {}", root.display());
        let Some(video_ts) = find_video_ts(&root) else {
            println!("no {VIDEO_TS} folder found: not a video disc");
            return;
        };
        println!("video folder: {}", video_ts.display());
        let mut listing: Vec<(String, u64)> = Vec::new();
        for entry in std::fs::read_dir(&video_ts).expect("the disc lists").flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            listing.push((name, size));
        }
        listing.sort();
        println!("{} files", listing.len());
        match main_title(&listing) {
            Ok(title) => {
                println!(
                    "main title: set {}, {} part(s), {:.2} GB",
                    title.title_set,
                    title.parts.len(),
                    title.total_bytes as f64 / 1_073_741_824.0
                );
                let first = video_ts.join(&title.parts[0]);
                match disc_read(&first) {
                    DiscRead::Playable => {
                        println!("READABLE: this disc would be converted and played")
                    }
                    DiscRead::CopyProtected => {
                        println!("REFUSED as copy protected, and the screen would say:");
                        println!("  {PROTECTED_MESSAGE}");
                    }
                    DiscRead::Unreadable(why) => {
                        println!("REFUSED as unreadable, and the screen would say:");
                        println!("  {}", unreadable_message(&why));
                    }
                }
            }
            Err(why) => println!("no playable title found: {why}"),
        }
    }

    /// Every drive the machine reports is somewhere a person can actually
    /// browse to, whatever kind it is, and the disc list is exactly the
    /// disc-kind subset of it.
    #[test]
    fn every_drive_offered_is_a_real_place() {
        let all = drive_roots();
        for (label, path, _kind) in &all {
            assert!(path.is_dir(), "{label} -> {}", path.display());
            assert!(label.contains("(") && label.contains(":)"), "a drive names its letter: {label}");
        }
        let discs: Vec<_> = all.iter().filter(|(_, _, k)| *k == DriveKind::Disc).map(|(l, p, _)| (l.clone(), p.clone())).collect();
        assert_eq!(discs, disc_roots(), "disc_roots is the disc-kind subset of drive_roots");
    }
}
