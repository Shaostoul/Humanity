//! The computer's own video decoders: Windows Media Foundation, for every file our own AV1 and
//! Opus path cannot play, an ordinary MP4 (H.264 video, AAC sound) above all (operator,
//! 2026-10-17: "How do we give users the ability to play MP4s in our app/website?").
//!
//! We ship no decoder. Windows 7 and later carry an H.264 and an AAC decoder in Media Foundation
//! for any desktop application, the H.264 one falling back to software on any graphics card
//! (docs/reference/findings/2026-10-17-windows-media-foundation-decoders.md); this asks them, the
//! way the app already asks the person's own ffmpeg (docs/reference/media-stance.md). The legal
//! reading is the September one, unchanged (docs/reference/findings/2026-09-19-video-codec-licensing.md).
//!
//! **Loaded at run time, never linked.** A Windows "N" edition has no Media Foundation unless the
//! person installs Microsoft's Media Feature Pack, and a program that names `mfplat.dll` in its
//! import table does not start at all there. So the five functions this uses are looked up with
//! `LoadLibraryW` and `GetProcAddress` the first time a file needs them (`Api`), and a computer
//! without them gets the sentence the platform finding chose, not a missing-file error before the
//! window opens. Everything else here is a call through a COM interface, which imports nothing.
//!
//! **Threads.** Media Foundation objects are made and used on threads that joined COM's
//! multithreaded apartment (`Apartment`). The probe runs on a short thread of its own, because the
//! caller may be the window's thread, which winit puts in a single-threaded apartment for drag and
//! drop; the picture is decoded on the player's own decode thread; the sound on kira's decoder
//! thread, which makes its reader and joins the apartment the first time it decodes
//! (`SystemAudio::reader`). kira's one call on the thread that starts the sound, a seek to the
//! start, is answered without making anything (`SystemAudio::seek`).
//!
//! **Time.** Windows's MP4 reader does not apply the file's edit list the way the file means it:
//! in the H.264 fixture (two B-frames) the first picture comes stamped 0.133 s and the first sound
//! 0.021 s, where the file starts both at 0 (measured 2026-10-18). So each stream is counted from
//! where Windows says it begins, set to where the file says it begins (`declared_starts`: the empty
//! edits opening each track's edit list): a sound that really starts half a second after the
//! picture still does, with silence before it (`SystemAudio::decode`). A file that is not an MP4
//! is taken at Windows's word.
//!
//! What comes out is what the AV1 path gives: `VideoFrame`s (RGBA, top-left origin, the visible
//! picture only) through the same queue, generations and seek rules (`decode_pass`), and kira
//! `Frame`s for the sound.

use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, OnceLock};

use kira::Frame;
use windows::core::{Interface, GUID, HRESULT, PCWSTR};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32};
use windows::Win32::System::Variant::VT_UI8;

use super::{push_frame, MediaError, MediaInfo, PassEnd, Push, Shared, VideoFrame};

/// What the app says when this copy of Windows has no Media Foundation (an "N" edition without the
/// Media Feature Pack). The platform finding's words
/// (docs/reference/findings/2026-10-17-platform-video-decoders.md, the table at its end).
pub const NO_DECODERS: &str = "This copy of Windows has no video decoders. Microsoft's free Media Feature Pack adds them.";

/// What the app says for an H.265 video on a Windows without Microsoft's HEVC extension (the same
/// table).
pub const NO_HEVC: &str = "This video is H.265. Windows plays it only with Microsoft's HEVC extension, sold separately because of patent licensing.";

/// The `MediaInfo::video_codec` and `audio_codec` of a file this path plays, by its own name; an
/// unlisted one by the four letters Windows names it with, else "unknown". Windows makes those
/// from the MP4 sample entry's name, high byte first (an Opus track comes as 0x4F707573, "Opus",
/// not as `MFAudioFormat_Opus`; measured 2026-10-18), the other way round from its own codes, which
/// are all listed above.
fn codec_name(subtype: &GUID) -> String {
    let known = match *subtype {
        s if s == MFVideoFormat_H264 => "H.264",
        s if s == MFVideoFormat_HEVC => "H.265",
        s if s == MFVideoFormat_VP90 => "VP9",
        s if s == MFVideoFormat_MPEG2 => "MPEG-2",
        s if s == MFVideoFormat_WMV3 => "WMV",
        s if s == MFVideoFormat_VP80 => "VP8",
        s if s == MFVideoFormat_AV1 => "AV1",
        s if s == MFAudioFormat_AAC => "AAC",
        s if s == MFAudioFormat_MP3 => "MP3",
        s if s == MFAudioFormat_Dolby_AC3 => "AC-3",
        s if s == MFAudioFormat_PCM => "PCM",
        s if s == MFAudioFormat_Opus => "Opus",
        s if s == MFAudioFormat_Vorbis => "Vorbis",
        s if s == MFAudioFormat_FLAC => "FLAC",
        s if s == MFAudioFormat_ALAC => "ALAC",
        s if s == MFAudioFormat_DTS => "DTS",
        s if s == MFAudioFormat_Dolby_DDPlus => "E-AC-3",
        _ => "",
    };
    if !known.is_empty() {
        return known.to_string();
    }
    let letters = subtype.data1.to_be_bytes();
    if letters.iter().all(|b| b.is_ascii_graphic()) {
        String::from_utf8_lossy(&letters).trim().to_string()
    } else {
        "unknown".to_string()
    }
}

// ---------------------------------------------------------------------------
// The five functions, loaded at run time
// ---------------------------------------------------------------------------

type MfStartupFn = unsafe extern "system" fn(u32, u32) -> HRESULT;
type MfShutdownFn = unsafe extern "system" fn() -> HRESULT;
type MfCreateAttributesFn = unsafe extern "system" fn(*mut *mut c_void, u32) -> HRESULT;
type MfCreateMediaTypeFn = unsafe extern "system" fn(*mut *mut c_void) -> HRESULT;
type MfCreateSourceReaderFromUrlFn = unsafe extern "system" fn(PCWSTR, *mut c_void, *mut *mut c_void) -> HRESULT;

/// Media Foundation's entry points on this computer, looked up once. The two libraries stay loaded
/// for the rest of the run (they are never freed), so the pointers stay good.
struct Api {
    startup: MfStartupFn,
    shutdown: MfShutdownFn,
    create_attributes: MfCreateAttributesFn,
    create_media_type: MfCreateMediaTypeFn,
    create_source_reader_from_url: MfCreateSourceReaderFromUrlFn,
}

/// `None` when this computer has no Media Foundation.
fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(|| unsafe {
        // From System32 only: never a file of the same name beside the exe.
        let plat = LoadLibraryExW(windows::core::w!("mfplat.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
        let readwrite = LoadLibraryExW(windows::core::w!("mfreadwrite.dll"), None, LOAD_LIBRARY_SEARCH_SYSTEM32).ok()?;
        let find = |module, name: &[u8]| GetProcAddress(module, windows::core::PCSTR(name.as_ptr()));
        Some(Api {
            startup: std::mem::transmute::<_, MfStartupFn>(find(plat, b"MFStartup\0")?),
            shutdown: std::mem::transmute::<_, MfShutdownFn>(find(plat, b"MFShutdown\0")?),
            create_attributes: std::mem::transmute::<_, MfCreateAttributesFn>(find(plat, b"MFCreateAttributes\0")?),
            create_media_type: std::mem::transmute::<_, MfCreateMediaTypeFn>(find(plat, b"MFCreateMediaType\0")?),
            create_source_reader_from_url: std::mem::transmute::<_, MfCreateSourceReaderFromUrlFn>(find(readwrite, b"MFCreateSourceReaderFromURL\0")?),
        })
    })
    .as_ref()
}

/// Whether this computer has Media Foundation at all.
pub fn available() -> bool {
    api().is_some()
}

/// This thread in COM's multithreaded apartment, with Media Foundation started, for as long as the
/// value lives. Joining twice is harmless (COM counts it; so does `MFStartup`).
struct Apartment {
    joined_com: bool,
    /// The thread that joined: COM is left only there (a thread that did not join must never
    /// leave, or it would undo someone else's join, such as the window's own for drag and drop).
    thread: std::thread::ThreadId,
}

impl Apartment {
    fn join() -> Result<Self, MediaError> {
        let api = api().ok_or_else(|| MediaError::System(NO_DECODERS.to_string()))?;
        // S_OK or S_FALSE: joined, and balanced by CoUninitialize. RPC_E_CHANGED_MODE: the thread
        // is already in a single-threaded apartment; carry on in it without balancing.
        let joined_com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        if let Err(e) = unsafe { (api.startup)(MF_VERSION, MFSTARTUP_NOSOCKET) }.ok() {
            if joined_com {
                unsafe { CoUninitialize() };
            }
            log::warn!("media: Media Foundation would not start: {e}");
            return Err(MediaError::System(NO_DECODERS.to_string()));
        }
        Ok(Self { joined_com, thread: std::thread::current().id() })
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if let Some(api) = api() {
            let _ = unsafe { (api.shutdown)() };
        }
        if self.joined_com && std::thread::current().id() == self.thread {
            unsafe { CoUninitialize() };
        }
    }
}

fn win(e: windows::core::Error) -> MediaError {
    MediaError::System(format!("Windows could not decode it ({e})"))
}

fn attributes(count: u32) -> Result<IMFAttributes, MediaError> {
    let api = api().ok_or_else(|| MediaError::System(NO_DECODERS.to_string()))?;
    let mut raw = std::ptr::null_mut();
    unsafe { (api.create_attributes)(&mut raw, count) }.ok().map_err(win)?;
    Ok(unsafe { IMFAttributes::from_raw(raw) })
}

fn media_type() -> Result<IMFMediaType, MediaError> {
    let api = api().ok_or_else(|| MediaError::System(NO_DECODERS.to_string()))?;
    let mut raw = std::ptr::null_mut();
    unsafe { (api.create_media_type)(&mut raw) }.ok().map_err(win)?;
    Ok(unsafe { IMFMediaType::from_raw(raw) })
}

/// A Source Reader on `path` with only the first `stream` (video or audio) selected, or `None` when
/// the file has no such stream. The video one asks Media Foundation for its own colour conversion,
/// so the decoder's output can be had as RGB.
fn open_reader(path: &Path, stream: MF_SOURCE_READER_CONSTANTS) -> Result<Option<IMFSourceReader>, MediaError> {
    std::fs::metadata(path)?; // a missing file reads as the I/O error the AV1 path gives
    let api = api().ok_or_else(|| MediaError::System(NO_DECODERS.to_string()))?;
    let attrs = attributes(1)?;
    if stream == MF_SOURCE_READER_FIRST_VIDEO_STREAM {
        unsafe { attrs.SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1) }.map_err(win)?;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide_nul();
    let mut raw = std::ptr::null_mut();
    unsafe { (api.create_source_reader_from_url)(PCWSTR(wide.as_ptr()), attrs.as_raw(), &mut raw) }.ok().map_err(|e| {
        if e.code() == MF_E_UNSUPPORTED_BYTESTREAM_TYPE {
            MediaError::System("Windows does not know this kind of file.".to_string())
        } else {
            win(e)
        }
    })?;
    let reader = unsafe { IMFSourceReader::from_raw(raw) };
    unsafe { reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false) }.map_err(win)?;
    match unsafe { reader.SetStreamSelection(stream.0 as u32, true) } {
        Ok(()) => Ok(Some(reader)),
        // No stream of that kind (measured: this is where a missing one shows, not later).
        Err(e) if e.code() == MF_E_INVALIDSTREAMNUMBER => Ok(None),
        Err(e) => Err(win(e)),
    }
}

/// Where an MP4 says its first picture and its first sound begin, in seconds: the empty edits that
/// open each track's edit list (`elst` entries with media time -1), in the movie's timescale. A
/// track with no edit list starts at 0. `None` for a file that is not an MP4, or one this cannot
/// read: then Windows's own times are used as they are. Reads only box headers and the few small
/// boxes it needs, never the media.
pub(crate) fn declared_starts(path: &Path) -> Option<(f64, f64)> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    // The boxes directly inside [start, end): (type, body start, end).
    let mut children = |f: &mut std::fs::File, start: u64, end: u64| -> Option<Vec<([u8; 4], u64, u64)>> {
        let mut out = Vec::new();
        let mut at = start;
        while at + 8 <= end && out.len() < 4096 {
            f.seek(SeekFrom::Start(at)).ok()?;
            let mut head = [0u8; 8];
            f.read_exact(&mut head).ok()?;
            let mut size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as u64;
            let kind = [head[4], head[5], head[6], head[7]];
            let mut body = at + 8;
            if size == 1 {
                let mut big = [0u8; 8];
                f.read_exact(&mut big).ok()?;
                size = u64::from_be_bytes(big);
                body += 8;
            } else if size == 0 {
                size = end - at;
            }
            if size < body - at || at + size > end {
                return None;
            }
            out.push((kind, body, at + size));
            at += size;
        }
        Some(out)
    };
    let read_at = |f: &mut std::fs::File, at: u64, n: usize| -> Option<Vec<u8>> {
        f.seek(SeekFrom::Start(at)).ok()?;
        let mut b = vec![0u8; n];
        f.read_exact(&mut b).ok()?;
        Some(b)
    };
    let find = |list: &[([u8; 4], u64, u64)], kind: &[u8; 4]| list.iter().find(|(k, _, _)| k == kind).copied();
    let top = children(&mut f, 0, len)?;
    let (_, moov_start, moov_end) = find(&top, b"moov")?;
    let moov = children(&mut f, moov_start, moov_end)?;
    let (_, mvhd, _) = find(&moov, b"mvhd")?;
    let version = read_at(&mut f, mvhd, 1)?[0];
    let at = mvhd + if version == 1 { 20 } else { 12 };
    let scale = read_at(&mut f, at, 4)?;
    let movie_scale = u32::from_be_bytes([scale[0], scale[1], scale[2], scale[3]]) as f64;
    if movie_scale <= 0.0 {
        return None;
    }
    let (mut video, mut audio) = (None, None);
    for (_, trak_start, trak_end) in moov.iter().filter(|(k, _, _)| k == b"trak") {
        let trak = children(&mut f, *trak_start, *trak_end)?;
        let Some((_, mdia_start, mdia_end)) = find(&trak, b"mdia") else { continue };
        let mdia = children(&mut f, mdia_start, mdia_end)?;
        let Some((_, hdlr, _)) = find(&mdia, b"hdlr") else { continue };
        let handler = read_at(&mut f, hdlr + 8, 4)?;
        let slot = match &handler[..] {
            b"vide" if video.is_none() => &mut video,
            b"soun" if audio.is_none() => &mut audio,
            _ => continue,
        };
        // The leading empty edits: how long the track waits before its first sample shows.
        let mut empty = 0.0;
        if let Some((_, edts_start, edts_end)) = find(&trak, b"edts") {
            if let Some((_, elst, _)) = find(&children(&mut f, edts_start, edts_end)?, b"elst") {
                let head = read_at(&mut f, elst, 8)?;
                let wide = head[0] == 1;
                let count = u32::from_be_bytes([head[4], head[5], head[6], head[7]]).min(64);
                let entry = if wide { 20 } else { 12 };
                for i in 0..count as u64 {
                    let e = read_at(&mut f, elst + 8 + i * entry, entry as usize)?;
                    let (duration, media_time) = if wide {
                        (u64::from_be_bytes(e[0..8].try_into().ok()?), i64::from_be_bytes(e[8..16].try_into().ok()?))
                    } else {
                        (u32::from_be_bytes(e[0..4].try_into().ok()?) as u64, i32::from_be_bytes(e[4..8].try_into().ok()?) as i64)
                    };
                    if media_time != -1 {
                        break;
                    }
                    empty += duration as f64 / movie_scale;
                }
            }
        }
        *slot = Some(empty);
    }
    Some((video.unwrap_or(0.0), audio.unwrap_or(0.0)))
}

/// Windows's encoding of a path as a NUL-terminated wide string.
trait WideNul {
    fn encode_wide_nul(&self) -> Vec<u16>;
}

impl WideNul for std::ffi::OsStr {
    fn encode_wide_nul(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;
        self.encode_wide().chain(std::iter::once(0)).collect()
    }
}

/// The file's length in seconds, from the source itself (0 when it does not say).
fn duration_s(reader: &IMFSourceReader) -> f64 {
    let Ok(value) = (unsafe { reader.GetPresentationAttribute(MF_SOURCE_READER_MEDIASOURCE.0 as u32, &MF_PD_DURATION) }) else { return 0.0 };
    hns_of(&value).map(|hns| hns as f64 / 1.0e7).unwrap_or(0.0)
}

/// A PROPVARIANT holding a 64-bit unsigned count of 100-nanosecond units, read directly (the
/// Windows crate's own conversion links propsys.dll, which this module avoids importing).
fn hns_of(value: &PROPVARIANT) -> Option<u64> {
    unsafe {
        let inner = &value.Anonymous.Anonymous;
        (inner.vt == VT_UI8).then(|| inner.Anonymous.uhVal)
    }
}

/// The reader's position set to `seconds`, in its 100-nanosecond units. Media Foundation lands on
/// the keyframe at or before it; what comes before the target is thrown away by the caller.
fn set_position(reader: &IMFSourceReader, seconds: f64) -> Result<(), MediaError> {
    let at = PROPVARIANT::from((seconds.max(0.0) * 1.0e7) as i64);
    unsafe { reader.SetCurrentPosition(&GUID::zeroed(), &at) }.map_err(win)
}

/// Where Media Foundation says `stream` begins, in seconds: the time of its first sample (see the
/// module's note on time). Leaves the reader back at the start. Windows tells the decoder's real
/// output format only with that first sample (and not again after the seek back), so a caller
/// must read the format again afterwards (a 2026-10-18 review: an 854-wide H.264 picture kept the
/// 3,416-byte rows it was announced with, not the 3,456 it came in, and drew skewed).
fn start_of(reader: &IMFSourceReader, stream: MF_SOURCE_READER_CONSTANTS) -> Result<f64, MediaError> {
    set_position(reader, 0.0)?;
    let mut changed = false;
    let start = read(reader, stream, &mut changed)?.map(|(at, _)| at).unwrap_or(0.0);
    set_position(reader, 0.0)?;
    Ok(start)
}

/// One sample from the reader's selected stream: its time in seconds and the sample, or `None` at
/// the end of the stream. `changed` is set when the stream's format changed with this sample.
fn read(reader: &IMFSourceReader, stream: MF_SOURCE_READER_CONSTANTS, changed: &mut bool) -> Result<Option<(f64, IMFSample)>, MediaError> {
    loop {
        let mut flags = 0u32;
        let mut time = 0i64;
        let mut sample: Option<IMFSample> = None;
        unsafe { reader.ReadSample(stream.0 as u32, 0, None, Some(&mut flags), Some(&mut time), Some(&mut sample)) }.map_err(win)?;
        if flags & MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED.0 as u32 != 0 {
            *changed = true;
        }
        if let Some(sample) = sample {
            return Ok(Some((time as f64 / 1.0e7, sample)));
        }
        if flags & (MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 | MF_SOURCE_READERF_ERROR.0 as u32) != 0 {
            return Ok(None);
        }
        // A gap in the stream (MF_SOURCE_READERF_STREAMTICK) carries no sample: read on.
    }
}

// ---------------------------------------------------------------------------
// What a file holds
// ---------------------------------------------------------------------------

/// What `path` holds, when Windows can play it: its picture's size, its length, and its sound's
/// rate and channels (no sound: `audio_track` None). Runs on a short thread of its own (see the
/// module's note on threads). A video Windows cannot decode is refused with a sentence naming
/// what is missing.
pub fn probe(path: &Path) -> Result<MediaInfo, MediaError> {
    let path = path.to_path_buf();
    std::thread::Builder::new()
        .name("media-probe".into())
        .spawn(move || {
            let _apartment = Apartment::join()?;
            probe_here(&path)
        })
        .map_err(MediaError::Io)?
        .join()
        .unwrap_or_else(|_| Err(MediaError::System("Windows stopped while reading the video.".to_string())))
}

fn probe_here(path: &Path) -> Result<MediaInfo, MediaError> {
    let video = VideoReader::open(path)?;
    let mut sound_lost = None;
    let (audio_codec, sample_rate, channels) = match AudioFormat::of(path) {
        Ok(Some(a)) => (Some(a.codec.to_string()), a.rate, a.channels),
        Ok(None) => (None, 0, 0),
        Err(lost) => {
            // A picture Windows can show with a sound it cannot. The screens convert it with the
            // person's ffmpeg when there is one (transcode.rs `begin_ingest`); otherwise it plays
            // silent, and the Media page says why.
            log::warn!("media: {}: Windows cannot decode its sound ({})", path.display(), lost);
            sound_lost = Some(lost);
            (None, 0, 0)
        }
    };
    Ok(MediaInfo {
        width: video.width,
        height: video.height,
        duration_s: duration_s(&video.reader),
        video_codec: video.codec.to_string(),
        audio_track: audio_codec.as_ref().map(|_| 0),
        audio_codec,
        sample_rate,
        channels,
        pre_skip: 0,
        track_count: 0,
        video_track: 0,
        timestamp_scale_ns: 100,
        system: true,
        sound_lost,
    })
}

/// One picture from the video at `path`, for a preview (the file picker's Pictures view,
/// 2026-10-18): the frame a tenth of the way in, at most five seconds (past a fade in from black),
/// decoded by Windows on the calling thread, which joins the apartment for it. None when Windows
/// cannot decode it.
pub fn still_of(path: &Path) -> Option<VideoFrame> {
    let _apartment = Apartment::join().ok()?;
    let mut video = VideoReader::open(path).ok()?;
    let at = (duration_s(&video.reader) * 0.1).clamp(0.0, 5.0);
    video.seek(at).ok()?;
    while let Some((pts, sample)) = video.next().ok()? {
        if pts + 1.0e-6 >= at {
            return video.picture(pts, &sample).ok();
        }
    }
    None
}

// ---------------------------------------------------------------------------
// The picture
// ---------------------------------------------------------------------------

/// A Source Reader giving the file's pictures as 32-bit RGB, and how to cut the visible picture
/// out of each.
struct VideoReader {
    reader: IMFSourceReader,
    codec: String,
    /// The whole decoded frame (an H.264 1080p frame is 1088 rows: the codec works in 16-row blocks).
    frame_w: u32,
    frame_h: u32,
    /// The visible picture inside it.
    left: u32,
    top: u32,
    width: u32,
    height: u32,
    /// Bytes from one row to the next when the buffer has no 2D view (negative: bottom-up).
    stride: i32,
    /// What is taken off Windows's times to put the pictures on the file's own timeline: Windows's
    /// time of the first picture less the file's declared start for it (the module's note on time).
    start_s: f64,
}

impl VideoReader {
    fn open(path: &Path) -> Result<Self, MediaError> {
        let reader = open_reader(path, MF_SOURCE_READER_FIRST_VIDEO_STREAM)?.ok_or(MediaError::NoVideoTrack)?;
        let native = unsafe { reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, 0) }.map_err(win)?;
        let subtype = unsafe { native.GetGUID(&MF_MT_SUBTYPE) }.unwrap_or_default();
        let codec = codec_name(&subtype);
        let rgb = media_type()?;
        unsafe {
            rgb.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(win)?;
            rgb.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(win)?;
        }
        if let Err(e) = unsafe { reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32, None, &rgb) } {
            log::warn!("media: no decoder for {codec} video: {e}");
            return Err(MediaError::System(if subtype == MFVideoFormat_HEVC {
                NO_HEVC.to_string()
            } else {
                format!("This computer has no decoder for this video ({codec}).")
            }));
        }
        let mut me = Self { reader, codec, frame_w: 0, frame_h: 0, left: 0, top: 0, width: 0, height: 0, stride: 0, start_s: 0.0 };
        let first = start_of(&me.reader, MF_SOURCE_READER_FIRST_VIDEO_STREAM)?;
        // After the first picture: only now does the format say how the decoder really lays it out.
        me.read_format()?;
        me.start_s = first - declared_starts(path).map(|(video, _)| video).unwrap_or(first);
        Ok(me)
    }

    /// The decoded frame's size, the visible picture inside it and the row stride, from the
    /// reader's current output format (again whenever it changes mid-stream).
    fn read_format(&mut self) -> Result<(), MediaError> {
        let current = unsafe { self.reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32) }.map_err(win)?;
        let size = unsafe { current.GetUINT64(&MF_MT_FRAME_SIZE) }.map_err(win)?;
        let (frame_w, frame_h) = ((size >> 32) as u32, size as u32);
        let mut area = [0u8; 16];
        let (left, top, width, height) = match unsafe { current.GetBlob(&MF_MT_MINIMUM_DISPLAY_APERTURE, &mut area, None) } {
            // MFVideoArea: OffsetX and OffsetY are MFOffset { fract: u16, value: i16 }, then SIZE { cx, cy }.
            Ok(()) => {
                let x = i16::from_le_bytes([area[2], area[3]]).max(0) as u32;
                let y = i16::from_le_bytes([area[6], area[7]]).max(0) as u32;
                let w = i32::from_le_bytes([area[8], area[9], area[10], area[11]]).max(0) as u32;
                let h = i32::from_le_bytes([area[12], area[13], area[14], area[15]]).max(0) as u32;
                (x, y, w.min(frame_w.saturating_sub(x)), h.min(frame_h.saturating_sub(y)))
            }
            Err(_) => (0, 0, frame_w, frame_h),
        };
        let stride = unsafe { current.GetUINT32(&MF_MT_DEFAULT_STRIDE) }.map(|s| s as i32).unwrap_or(frame_w as i32 * 4);
        if width == 0 || height == 0 {
            return Err(MediaError::System("Windows gave a picture with no size.".to_string()));
        }
        (self.frame_w, self.frame_h, self.left, self.top, self.width, self.height, self.stride) = (frame_w, frame_h, left, top, width, height, stride);
        Ok(())
    }

    /// The next picture's time on the file's own timeline (`start_s` taken off) and its sample, or
    /// `None` at the end.
    fn next(&mut self) -> Result<Option<(f64, IMFSample)>, MediaError> {
        let mut changed = false;
        let next = read(&self.reader, MF_SOURCE_READER_FIRST_VIDEO_STREAM, &mut changed)?;
        if changed {
            self.read_format()?;
        }
        Ok(next.map(|(at, sample)| (at - self.start_s, sample)))
    }

    /// Reposition for a picture at `seconds` on the file's own timeline.
    fn seek(&self, seconds: f64) -> Result<(), MediaError> {
        set_position(&self.reader, seconds + self.start_s)
    }

    /// `sample` as RGBA, top-left origin, the visible picture only.
    fn picture(&self, pts_s: f64, sample: &IMFSample) -> Result<VideoFrame, MediaError> {
        let buffer = unsafe { sample.ConvertToContiguousBuffer() }.map_err(win)?;
        let mut rgba = vec![0u8; self.width as usize * self.height as usize * 4];
        // The 2D view knows the real row pitch (and whether the rows run bottom-up); a plain buffer
        // is read with the format's default stride.
        if let Ok(two_d) = buffer.cast::<IMF2DBuffer>() {
            let mut scan0 = std::ptr::null_mut();
            let mut pitch = 0i32;
            unsafe { two_d.Lock2D(&mut scan0, &mut pitch) }.map_err(win)?;
            let bytes = (pitch.unsigned_abs() as usize) * self.frame_h as usize;
            // Lock2D gives the first row's address; with a negative pitch the rows run backwards
            // from it, so the block starts (frame_h - 1) rows earlier.
            let start = if pitch < 0 { unsafe { scan0.offset(pitch as isize * (self.frame_h as isize - 1)) } } else { scan0 };
            let data = unsafe { std::slice::from_raw_parts(start, bytes) };
            self.copy_rows(data, pitch, &mut rgba);
            let _ = unsafe { two_d.Unlock2D() };
        } else {
            let mut ptr = std::ptr::null_mut();
            let mut len = 0u32;
            unsafe { buffer.Lock(&mut ptr, None, Some(&mut len)) }.map_err(win)?;
            let data = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
            let need = self.stride.unsigned_abs() as usize * self.frame_h as usize;
            if data.len() >= need {
                self.copy_rows(&data[..need], self.stride, &mut rgba);
            }
            let _ = unsafe { buffer.Unlock() };
        }
        Ok(VideoFrame { rgba, width: self.width, height: self.height, pts_s })
    }

    /// Copy the visible picture out of `data` (the whole frame, its rows `pitch` bytes apart and
    /// bottom-up when `pitch` is negative), turning Media Foundation's B, G, R, X bytes into R, G,
    /// B, A.
    fn copy_rows(&self, data: &[u8], pitch: i32, rgba: &mut [u8]) {
        let row_bytes = pitch.unsigned_abs() as usize;
        for y in 0..self.height as usize {
            let source_row = self.top as usize + y;
            // In a bottom-up block the top picture row is the LAST row in memory.
            let row = if pitch < 0 { self.frame_h as usize - 1 - source_row } else { source_row };
            let from = row * row_bytes + self.left as usize * 4;
            let Some(src) = data.get(from..from + self.width as usize * 4) else { return };
            let dst = &mut rgba[y * self.width as usize * 4..(y + 1) * self.width as usize * 4];
            for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                d[0] = s[2];
                d[1] = s[1];
                d[2] = s[0];
                d[3] = 255;
            }
        }
    }
}

/// The picture's decode thread for a file Windows decodes: the same queue, generations and seek
/// rule as the AV1 path (`super::decode_pass`): reposition, decode, throw away what comes before
/// the target, queue the rest, mark the end.
pub(super) fn decode_thread(path: PathBuf, shared: Arc<Shared>) {
    let opened = Apartment::join().and_then(|apartment| Ok((apartment, VideoReader::open(&path)?)));
    let (_apartment, mut video) = match opened {
        Ok(v) => v,
        Err(e) => {
            super::drive(&shared, |_, _, _| Err(MediaError::System(e.to_string())));
            return;
        }
    };
    super::drive(&shared, |generation, start_s, _from_top| decode_pass(&mut video, &shared, generation, start_s));
}

fn decode_pass(video: &mut VideoReader, shared: &Shared, generation: u64, start_s: f64) -> Result<PassEnd, MediaError> {
    video.seek(start_s)?;
    loop {
        if shared.stop.load(Ordering::SeqCst) {
            return Ok(PassEnd::Stop);
        }
        if shared.generation.load(Ordering::SeqCst) != generation {
            return Ok(PassEnd::Restart);
        }
        let Some((pts_s, sample)) = video.next()? else { break };
        #[cfg(test)]
        shared.pictures_decoded.fetch_add(1, Ordering::SeqCst);
        // Decoded only to get from the keyframe to the target; never drawn, so never converted.
        if pts_s + 1.0e-6 < start_s {
            continue;
        }
        #[cfg(test)]
        if shared.restart_instead_of_queueing.load(Ordering::SeqCst) {
            return Ok(PassEnd::Restart);
        }
        let frame = video.picture(pts_s, &sample)?;
        match push_frame(shared, generation, frame) {
            Push::Pushed => {}
            Push::Restart => return Ok(PassEnd::Restart),
            Push::Stop => return Ok(PassEnd::Stop),
        }
    }
    let mut q = shared.queue.lock().unwrap_or_else(|e| e.into_inner());
    if shared.generation.load(Ordering::SeqCst) == generation {
        q.eof = true;
    }
    Ok(PassEnd::Eof)
}

// ---------------------------------------------------------------------------
// The sound
// ---------------------------------------------------------------------------

/// The sound's format as Windows decodes it to 32-bit floats.
struct AudioFormat {
    codec: String,
    rate: u32,
    channels: u16,
}

impl AudioFormat {
    /// The first sound track's format, `None` when the file has none.
    /// `Err` (the codec's name) when the file has sound Windows cannot decode.
    fn of(path: &Path) -> Result<Option<Self>, String> {
        let reader = match open_reader(path, MF_SOURCE_READER_FIRST_AUDIO_STREAM) {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(None),
            Err(e) => return Err(e.to_string()),
        };
        Self::set_float(&reader).map_err(|e| match e {
            MediaError::Audio(codec) => codec,
            other => other.to_string(),
        })
    }

    /// Ask for 32-bit float samples and read back the rate and channels they come at.
    fn set_float(reader: &IMFSourceReader) -> Result<Option<Self>, MediaError> {
        let native = match unsafe { reader.GetNativeMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, 0) } {
            Ok(t) => t,
            Err(e) if e.code() == MF_E_INVALIDSTREAMNUMBER => return Ok(None),
            Err(e) => return Err(win(e)),
        };
        let codec = codec_name(&unsafe { native.GetGUID(&MF_MT_SUBTYPE) }.unwrap_or_default());
        let float = media_type()?;
        unsafe {
            float.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(win)?;
            float.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float).map_err(win)?;
            reader.SetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32, None, &float).map_err(|e| {
                log::warn!("media: no decoder for {codec} sound: {e}");
                MediaError::Audio(codec.to_string())
            })?;
        }
        let current = unsafe { reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32) }.map_err(win)?;
        let rate = unsafe { current.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND) }.map_err(win)?;
        let channels = unsafe { current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS) }.map_err(win)? as u16;
        if rate == 0 || channels == 0 {
            return Ok(None);
        }
        Ok(Some(Self { codec, rate, channels }))
    }
}

/// The Source Reader, inside a sound kira may move between threads. A Source Reader is
/// free-threaded (Media Foundation objects take the free-threaded marshaler); this one is made on
/// kira's decoder thread the first time it decodes and used there; the Windows crate cannot know
/// that, hence the promise.
struct SendReader(IMFSourceReader);
// SAFETY: see above: a free-threaded COM object, used from one thread at a time (kira's decoder
// owns the sound), and released from any.
unsafe impl Send for SendReader {}

/// The file's sound for kira, decoded by Windows to float samples. kira calls it on its own
/// decoder thread, so the reader is made there on first use.
pub struct SystemAudio {
    path: PathBuf,
    rate: u32,
    channels: u16,
    num_frames: usize,
    reader: Option<SendReader>,
    /// Media Foundation kept started (and this thread in the apartment) while the reader lives.
    apartment: Option<SendApartment>,
    /// The frame index the next `decode` starts at.
    cursor: usize,
    /// A block already read, with the frame index it belongs at, handed out by the next `decode`.
    pending: Option<(i64, Vec<Frame>)>,
    /// What is taken off Windows's times to put the sound on the file's own timeline (the module's
    /// note on time); set when the reader is made.
    start_s: f64,
    /// Where the file says its sound begins (`declared_starts`), `None` to take Windows's word.
    declared_s: Option<f64>,
}

/// `Apartment` kept with the sound. Its drop balances `MFStartup` (a process-wide count, fine from
/// any thread) and, on the thread that joined, COM.
struct SendApartment(Apartment);
// SAFETY: MFShutdown may be called from any thread, and `Apartment` leaves COM only on the thread
// that joined it, whichever thread kira drops the sound on.
unsafe impl Send for SendApartment {}

/// How many silent frames to hand kira past the end of the sound (the Opus path's chunk).
const SILENCE_CHUNK: usize = 1024;

impl SystemAudio {
    /// The sound of a file `probe` read (`info.system`, with a sound track).
    pub fn open(path: &Path, info: &MediaInfo) -> Result<Self, MediaError> {
        if info.sample_rate == 0 || info.channels == 0 {
            return Err(MediaError::Audio("the file has no sound Windows can decode".into()));
        }
        Ok(Self {
            path: path.to_path_buf(),
            rate: info.sample_rate,
            channels: info.channels,
            num_frames: (info.duration_s * info.sample_rate as f64).round() as usize,
            reader: None,
            apartment: None,
            cursor: 0,
            pending: None,
            start_s: 0.0,
            declared_s: declared_starts(path).map(|(_, audio)| audio),
        })
    }

    fn reader(&mut self) -> Result<&IMFSourceReader, MediaError> {
        if self.reader.is_none() {
            self.apartment = Some(SendApartment(Apartment::join()?));
            let reader = open_reader(&self.path, MF_SOURCE_READER_FIRST_AUDIO_STREAM)?.ok_or_else(|| MediaError::Audio("the file has no sound".into()))?;
            let format = AudioFormat::set_float(&reader)?.ok_or_else(|| MediaError::Audio("the file has no sound".into()))?;
            if format.rate != self.rate || format.channels != self.channels {
                // The probe and this reader disagree: trust the reader for the channels; the rate
                // was already promised to kira, so say so in the log.
                log::warn!("media: sound reads {} Hz {} ch now, {} Hz {} ch at the probe", format.rate, format.channels, self.rate, self.channels);
                self.channels = format.channels;
            }
            let first = start_of(&reader, MF_SOURCE_READER_FIRST_AUDIO_STREAM)?;
            self.start_s = first - self.declared_s.unwrap_or(first);
            // After the first block, as for the picture (`start_of`): the channels as decoded.
            if let Ok(current) = unsafe { reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32) } {
                if let Ok(ch) = unsafe { current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS) } {
                    self.channels = ch.max(1) as u16;
                }
            }
            self.reader = Some(SendReader(reader));
        }
        Ok(&self.reader.as_ref().expect("made above").0)
    }

    /// The next block of sound: the frame index it belongs at on the file's own timeline (negative
    /// for encoder priming before 0), and its frames; `None` at the end.
    fn next(&mut self) -> Result<Option<(i64, Vec<Frame>)>, MediaError> {
        self.reader()?;
        let rate = self.rate as f64;
        let start_s = self.start_s;
        let reader = &self.reader.as_ref().expect("made above").0;
        let mut changed = false;
        let Some((at_s, sample)) = read(reader, MF_SOURCE_READER_FIRST_AUDIO_STREAM, &mut changed)? else { return Ok(None) };
        if changed {
            // The decoder's real output, told with its first block: its channel count is what the
            // samples are split by. (The rate was promised to kira already; a change is only logged.)
            if let Ok(current) = unsafe { reader.GetCurrentMediaType(MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32) } {
                if let Ok(ch) = unsafe { current.GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS) } {
                    self.channels = ch.max(1) as u16;
                }
                if let Ok(r) = unsafe { current.GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND) } {
                    if r != self.rate {
                        log::warn!("media: the sound came at {r} Hz, {} Hz was expected", self.rate);
                    }
                }
            }
        }
        let channels = self.channels as usize;
        let buffer = unsafe { sample.ConvertToContiguousBuffer() }.map_err(win)?;
        let mut ptr = std::ptr::null_mut();
        let mut len = 0u32;
        unsafe { buffer.Lock(&mut ptr, None, Some(&mut len)) }.map_err(win)?;
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len as usize) };
        let frames = frames_of(bytes, channels);
        let _ = unsafe { buffer.Unlock() };
        Ok(Some((((at_s - start_s) * rate).round() as i64, frames)))
    }
}

/// Interleaved 32-bit float samples of `channels` channels as kira frames: mono to both sides,
/// stereo as it is, more channels as their front pair with the centre (the third, in Windows's
/// channel order) added to both at 0.707.
fn frames_of(bytes: &[u8], channels: usize) -> Vec<Frame> {
    let floats: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    match channels {
        0 => Vec::new(),
        1 => floats.iter().map(|&s| Frame::from_mono(s)).collect(),
        2 => floats.chunks_exact(2).map(|s| Frame::new(s[0], s[1])).collect(),
        n => floats
            .chunks_exact(n)
            .map(|s| {
                let centre = s[2] * std::f32::consts::FRAC_1_SQRT_2;
                Frame::new(s[0] + centre, s[1] + centre)
            })
            .collect(),
    }
}

impl kira::sound::streaming::Decoder for SystemAudio {
    type Error = MediaError;

    fn sample_rate(&self) -> u32 {
        self.rate
    }

    fn num_frames(&self) -> usize {
        self.num_frames
    }

    /// The next frames, placed by their times: silence where the sound has not started yet (or
    /// skips), and what was already handed out never twice.
    fn decode(&mut self) -> Result<Vec<Frame>, MediaError> {
        // How far a block may sit past the cursor before the gap is filled with silence: rounding
        // of Windows's times is a frame or two, a real gap is far more.
        let slack = (self.rate / 100) as i64;
        loop {
            let (at, mut frames) = match self.pending.take() {
                Some(block) => block,
                None => match self.next()? {
                    Some(block) => block,
                    None => {
                        // Past the end: silence keeps kira's index moving (the Opus path's reason).
                        self.cursor += SILENCE_CHUNK;
                        return Ok(vec![Frame::ZERO; SILENCE_CHUNK]);
                    }
                },
            };
            if frames.is_empty() {
                continue;
            }
            let cursor = self.cursor as i64;
            if at > cursor + slack {
                let gap = ((at - cursor) as usize).min(SILENCE_CHUNK);
                self.pending = Some((at, frames));
                self.cursor += gap;
                return Ok(vec![Frame::ZERO; gap]);
            }
            if at < cursor {
                let overlap = (cursor - at) as usize;
                if overlap >= frames.len() {
                    continue;
                }
                frames.drain(..overlap);
            }
            self.cursor += frames.len();
            return Ok(frames);
        }
    }

    /// Reposition Windows's reader (it lands at or before the target) and keep the first block that
    /// reaches `index` for the next `decode`, which trims or pads it into place. Returns `index`.
    fn seek(&mut self, index: usize) -> Result<usize, MediaError> {
        // kira's first call, from the thread that starts the sound: a seek to the start of a sound
        // nothing has been read from yet moves nothing, so nothing is made on that thread.
        if self.reader.is_none() && index == 0 {
            self.pending = None;
            self.cursor = 0;
            return Ok(0);
        }
        let target_s = index as f64 / self.rate as f64;
        self.reader()?;
        set_position(&self.reader.as_ref().expect("made above").0, target_s + self.start_s)?;
        self.pending = None;
        self.cursor = index;
        loop {
            match self.next()? {
                None => return Ok(index),
                Some((at, frames)) => {
                    if at + (frames.len() as i64) <= index as i64 {
                        continue;
                    }
                    self.pending = Some((at, frames));
                    return Ok(index);
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;
