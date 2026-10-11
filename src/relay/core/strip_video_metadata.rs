//! Video metadata removal, in place (privacy hardening, 2026-10-11).
//!
//! A phone video carries where it was filmed. Apple phones write the place
//! into QuickTime metadata (a `meta` box inside `moov`, under the key
//! `com.apple.quicktime.location.ISO6709`, beside the make, model and
//! software); Android phones and many cameras write a `©xyz` entry inside
//! `moov/udta`; 3GPP files use a `loci` box inside `udta`; and editing
//! tools put XMP (which can carry GPS) in a top-level `uuid` box. Pictures
//! have been stripped since 2026-08-23 (`strip_metadata.rs`); this module
//! does the same job for MP4, MOV and 3GP video.
//!
//! HOW, in plain words. An MP4 file is a row of "boxes" (Apple calls them
//! atoms). Each starts with an 8-byte header: a 4-byte size and a 4-byte
//! name such as `moov` (the movie's index) or `mdat` (the pictures and
//! sound themselves). Some boxes hold other boxes. Cutting a box out would
//! move every byte after it, and the index (`stco`/`co64` inside `moov`)
//! records the exact byte position of every chunk of frames, so when the
//! index comes before the frames, removing anything from it breaks
//! playback. So nothing is removed. Instead each metadata box is
//! NEUTRALISED where it stands: its name is changed to `free` (the
//! standard name for "empty space, skip me") and everything inside it is
//! overwritten with zero bytes. The file keeps its exact size, nothing
//! moves, and every player already skips a `free` box. No re-encoding, so
//! the picture and sound are untouched.
//!
//! What is neutralised:
//! - every `udta` (user data) and `meta` box directly inside `moov`, and
//!   directly inside each `trak` (one track) in `moov`;
//! - every XMP `uuid` box at the top level, directly inside `moov`, and
//!   directly inside each `trak`;
//! - a top-level `meta` box, but only in a file that also has a `moov`.
//!   A HEIF or AVIF still picture shares this file format and keeps the
//!   picture itself in its top-level `meta`, so a file with no movie in it
//!   is never touched there.
//!
//! What is never touched: `mdat`, `mvhd`, a track's `tkhd`, `mdia` and
//! `edts`, and every other box. The walk reads box headers only and seeks
//! past everything else, so even a file of many gigabytes costs a few
//! dozen small reads: the work grows with the metadata, not the file.
//!
//! Broken files. If a box claims a size under 8, or a size that runs past
//! the end of the box holding it (or past the end of the file), the walk
//! stops at that level, keeps everything already neutralised, and says so
//! in `VideoStrip::stopped_early`. Boxes after the broken one at that
//! level cannot be found (nobody can know where the next one starts), so
//! the caller should read a report with `stopped_early` entries as "made
//! better, not proven clean". It never writes outside the file and never
//! panics on any input. This differs from the picture strip, which falls
//! back to the original bytes on trouble: here the original is the leak,
//! and every neutralised box is a strict improvement on it.
//!
//! Not an MP4 at all (no `ftyp` box first, or a WebM/Matroska file): the
//! file is never opened for writing and is reported as left untouched.

use std::fs::{File, OpenOptions};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// The 16-byte id that Adobe's XMP specification gives the `uuid` box
/// carrying XMP in an MP4 file: BE7ACFCB-97A9-42E8-9C71-999491E3AFAC.
pub const XMP_UUID: [u8; 16] = [
    0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC,
];

/// The first four bytes of every WebM and Matroska file (the EBML magic).
const EBML_MAGIC: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

/// The most box headers one call will read. A phone video has a few dozen
/// at the levels walked here, a long fragmented stream some tens of
/// thousands. The cap only stops a crafted file made of millions of tiny
/// empty boxes from keeping the relay busy.
const MAX_BOXES: u32 = 1_000_000;

/// Zero bytes are written in pieces of this size, so a big metadata box
/// (cover art, a thumbnail) never needs a big buffer.
const ZERO_CHUNK: usize = 64 * 1024;

/// What kind of file the strip found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VideoKind {
    /// MP4, MOV, 3GP and the rest of the ISO base media family (the file
    /// starts with an `ftyp` box): its boxes were walked.
    Mp4Family,
    /// WebM or Matroska: a different format, left untouched.
    Matroska,
    /// Anything else, or too short to tell: left untouched.
    Other,
}

/// One box that was neutralised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NeutralisedBox {
    /// Where it sat: "moov/udta", "moov/meta", "moov/trak/udta",
    /// "moov/trak/meta", "uuid(xmp)", "moov/uuid(xmp)",
    /// "moov/trak/uuid(xmp)" or "meta" (a top-level meta box).
    pub path: String,
    /// Byte position of the box's first byte in the file.
    pub offset: u64,
    /// The box's whole size in bytes, its header included.
    pub size: u64,
}

/// What `strip_video_metadata_in_place` did to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStrip {
    /// What kind of file it was. Only `Mp4Family` files are ever changed.
    pub kind: VideoKind,
    /// True when a `moov` box (the movie itself) was found and walked.
    /// False for a still picture in this format (HEIF, AVIF) and for a
    /// file cut off before its `moov`.
    pub found_moov: bool,
    /// Every box that was renamed `free` and filled with zeros, in file
    /// order. Empty when nothing needed doing.
    pub neutralised: Vec<NeutralisedBox>,
    /// How many bytes were overwritten with zeros (box contents, not the
    /// four renamed letters of each header).
    pub bytes_zeroed: u64,
    /// One plain sentence for each place the walk had to stop early
    /// because a box size made no sense. Empty for a well-formed file.
    pub stopped_early: Vec<String>,
}

impl VideoStrip {
    /// The report for a file that was not walked at all.
    fn left_untouched(kind: VideoKind) -> Self {
        VideoStrip { kind, found_moov: false, neutralised: Vec::new(), bytes_zeroed: 0, stopped_early: Vec::new() }
    }

    /// True when at least one box was neutralised (the file was changed).
    pub fn changed(&self) -> bool {
        !self.neutralised.is_empty()
    }

    /// True when this was a movie whose whole box structure walked cleanly,
    /// so every place this module knows to look was looked at.
    pub fn complete(&self) -> bool {
        self.kind == VideoKind::Mp4Family && self.found_moov && self.stopped_early.is_empty()
    }

    /// Just the paths of the neutralised boxes, for logs and tests.
    pub fn paths(&self) -> Vec<&str> {
        self.neutralised.iter().map(|b| b.path.as_str()).collect()
    }
}

/// True when these first bytes of a file start an MP4-family file: the
/// first box is an `ftyp` box with a usable size. Eight bytes are enough.
/// (HEIF and AVIF still pictures also start this way; the strip leaves
/// their pictures alone, see the module notes.)
pub fn is_mp4_family(first_bytes: &[u8]) -> bool {
    if first_bytes.len() < 8 || &first_bytes[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes([first_bytes[0], first_bytes[1], first_bytes[2], first_bytes[3]]);
    // 1 means a 64-bit size follows, 8 or more is an ordinary size. 0 would
    // mean "this ftyp box is the whole file", and 2 to 7 cannot even hold
    // the header, so neither starts a real video.
    size == 1 || size >= 8
}

/// Decide what kind of file these first bytes start.
fn sniff(first_bytes: &[u8]) -> VideoKind {
    if first_bytes.len() >= 4 && first_bytes[..4] == EBML_MAGIC {
        VideoKind::Matroska
    } else if is_mp4_family(first_bytes) {
        VideoKind::Mp4Family
    } else {
        VideoKind::Other
    }
}

/// Remove location and other personal metadata from the MP4, MOV or 3GP
/// file at `path`, in place, without changing its size or re-encoding it.
/// See the module notes for exactly what is neutralised.
///
/// Returns an error only when the file cannot be opened, read or written.
/// A file that is not an MP4 is never opened for writing; a broken one is
/// walked as far as it makes sense and the report says where it stopped.
pub fn strip_video_metadata_in_place(path: &Path) -> io::Result<VideoStrip> {
    // Look at the first bytes through a read-only handle first, so a file
    // that is not an MP4 is never opened for writing at all.
    let mut first = [0u8; 8];
    let got = read_up_to(&mut File::open(path)?, &mut first)?;
    let kind = sniff(&first[..got]);
    if kind != VideoKind::Mp4Family {
        return Ok(VideoStrip::left_untouched(kind));
    }

    // Opening for reading and writing never shortens or grows the file.
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let len = file.metadata()?.len();
    // The zeros reach the disk before any box is renamed (see `strip_stream`).
    let report = strip_stream(&mut file, len, |f| f.sync_all())?;
    if report.changed() {
        // Make sure the zeros are on the disk before anyone serves the file.
        file.sync_all()?;
    }
    Ok(report)
}

/// The same work on a video held in memory (an upload not yet written to
/// disk). The buffer keeps its length; it cannot grow.
pub fn strip_video_metadata_in_buffer(data: &mut [u8]) -> io::Result<VideoStrip> {
    let len = data.len() as u64;
    strip_stream(&mut Cursor::new(data), len, |_| Ok(()))
}

/// Fill `buf` from the reader, stopping early only at the end of the data.
fn read_up_to<R: Read>(reader: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut got = 0;
    while got < buf.len() {
        match reader.read(&mut buf[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(got)
}

/// The whole job on anything that can be read, written and moved around
/// in: a file on disk, or bytes in memory. `len` is the data's length.
/// `sync` makes what was written so far durable (`File::sync_all` for a
/// file; nothing for memory). It runs between zeroing and renaming: every
/// planned box is zeroed and synced FIRST, and only then renamed `free`. In
/// the other order a crash between the two left a box already called `free`
/// with the place still inside it, and a second run skips `free` boxes, so
/// the file would be stored with its location (review, 2026-10-11). In this
/// order a crash leaves a box with its old name and zeros inside, which the
/// next run plans and finishes.
fn strip_stream<F: Read + Write + Seek>(
    f: &mut F,
    len: u64,
    mut sync: impl FnMut(&mut F) -> io::Result<()>,
) -> io::Result<VideoStrip> {
    f.seek(SeekFrom::Start(0))?;
    let mut first = [0u8; 8];
    let got = read_up_to(f, &mut first)?;
    let kind = sniff(&first[..got]);
    if kind != VideoKind::Mp4Family {
        return Ok(VideoStrip::left_untouched(kind));
    }

    // Step 1: walk the box headers and make a list of what to neutralise.
    // Nothing is written during the walk, so a decision that depends on the
    // whole file (a top-level meta only counts in a file with a movie) can
    // be made before the first byte changes.
    let mut walk = Walk {
        f: &mut *f,
        boxes_read: 0,
        found_moov: false,
        plans: Vec::new(),
        top_meta: Vec::new(),
        stopped_early: Vec::new(),
    };
    walk.level(0, len, Level::Top)?;
    let Walk { found_moov, mut plans, top_meta, stopped_early, .. } = walk;
    if found_moov {
        plans.extend(top_meta);
    }
    plans.sort_by_key(|p| p.offset);

    // The walk only plans boxes that lie wholly inside the data. This is a
    // second lock on the same door: never write outside the file.
    plans.retain(|p| p.end <= len && p.offset + 8 <= p.body_start && p.body_start <= p.end);

    // Step 2: zero what is inside every planned box, and make it durable.
    let mut report = VideoStrip { kind, found_moov, neutralised: Vec::new(), bytes_zeroed: 0, stopped_early };
    for p in &plans {
        write_zeros(f, p.body_start, p.end)?;
        report.bytes_zeroed += p.end - p.body_start;
    }
    if !plans.is_empty() {
        f.flush()?;
        sync(f)?;
    }

    // Step 3: only now rename each one `free`. The name sits after the
    // 4-byte size, whichever size form is used.
    for p in plans {
        f.seek(SeekFrom::Start(p.offset + 4))?;
        f.write_all(b"free")?;
        report.neutralised.push(NeutralisedBox { path: p.path, offset: p.offset, size: p.end - p.offset });
    }
    f.flush()?;
    Ok(report)
}

/// Overwrite the bytes from `from` up to (not including) `to` with zeros.
fn write_zeros<F: Write + Seek>(f: &mut F, from: u64, to: u64) -> io::Result<()> {
    static ZEROS: [u8; ZERO_CHUNK] = [0; ZERO_CHUNK];
    f.seek(SeekFrom::Start(from))?;
    let mut left = to - from;
    while left > 0 {
        let n = left.min(ZERO_CHUNK as u64) as usize;
        f.write_all(&ZEROS[..n])?;
        left -= n as u64;
    }
    Ok(())
}

/// Which level of the box tree the walk is on. Only three are ever
/// walked: the top of the file, the inside of `moov`, and the inside of
/// each `trak` in `moov`. Nothing deeper is read, so the recursion can
/// never go deeper than this however the file is built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Level {
    Top,
    Moov,
    Trak,
}

impl Level {
    /// What goes in front of a box name to make its reported path.
    fn prefix(self) -> &'static str {
        match self {
            Level::Top => "",
            Level::Moov => "moov/",
            Level::Trak => "moov/trak/",
        }
    }

    /// The level in plain words, for the stopped-early sentences.
    fn name(self) -> &'static str {
        match self {
            Level::Top => "the top level of the file",
            Level::Moov => "moov",
            Level::Trak => "a moov/trak",
        }
    }
}

/// One box header as read from the file.
struct BoxHeader {
    /// The box's 4-letter name.
    name: [u8; 4],
    /// Where the box starts.
    start: u64,
    /// Where its contents start: 8 bytes in, or 16 with a 64-bit size.
    body_start: u64,
    /// The first byte after the box.
    end: u64,
}

/// What reading a header found.
enum Header {
    /// A box that fits inside its level.
    Box(BoxHeader),
    /// Fewer than 8 bytes left at this level: too small to hold a box, so
    /// nothing can hide there. (QuickTime even ends some lists with four
    /// zero bytes.) The level is simply finished.
    NoRoom,
    /// A size that makes no sense. The sentence says why.
    Bad(String),
}

/// A box the walk decided to neutralise.
struct Plan {
    path: String,
    offset: u64,
    body_start: u64,
    end: u64,
}

impl Plan {
    fn new(path: String, b: &BoxHeader) -> Self {
        Plan { path, offset: b.start, body_start: b.body_start, end: b.end }
    }
}

/// The walk's state while it reads the box headers.
struct Walk<'a, F> {
    f: &'a mut F,
    boxes_read: u32,
    found_moov: bool,
    /// Boxes to neutralise, whatever else the file holds.
    plans: Vec<Plan>,
    /// Top-level meta boxes: neutralised only if the file has a moov.
    top_meta: Vec<Plan>,
    stopped_early: Vec<String>,
}

impl<F: Read + Seek> Walk<'_, F> {
    /// Walk the boxes lying between `start` and `end` at one level.
    fn level(&mut self, start: u64, end: u64, level: Level) -> io::Result<()> {
        let mut pos = start;
        while pos < end {
            if self.boxes_read >= MAX_BOXES {
                self.stopped_early.push(format!(
                    "stopped after reading {MAX_BOXES} box headers, more than any real video has"
                ));
                return Ok(());
            }
            self.boxes_read += 1;

            let b = match self.header(pos, end, level)? {
                Header::Box(b) => b,
                Header::NoRoom => return Ok(()),
                Header::Bad(why) => {
                    // Nobody can know where the next box at this level
                    // starts, so this level ends here. The level above
                    // carries on: its own sizes are still good.
                    self.stopped_early.push(why);
                    return Ok(());
                }
            };

            let path = |name: &str| format!("{}{}", level.prefix(), name);
            match (level, &b.name) {
                // The movie: look inside it.
                (Level::Top, b"moov") => {
                    self.found_moov = true;
                    self.level(b.body_start, b.end, Level::Moov)?;
                }
                // One track: look inside it too.
                (Level::Moov, b"trak") => self.level(b.body_start, b.end, Level::Trak)?,
                // A top-level meta: only if the file turns out to hold a movie.
                (Level::Top, b"meta") => self.top_meta.push(Plan::new(path("meta"), &b)),
                // User data and metadata of the movie and of each track.
                (Level::Moov | Level::Trak, b"udta") => self.plans.push(Plan::new(path("udta"), &b)),
                (Level::Moov | Level::Trak, b"meta") => self.plans.push(Plan::new(path("meta"), &b)),
                // A uuid box is only ours when it is the XMP one; any other
                // uuid (360-degree video settings, a camera maker's data)
                // may be needed and is left alone.
                (_, b"uuid") => {
                    if self.is_xmp(&b)? {
                        self.plans.push(Plan::new(path("uuid(xmp)"), &b));
                    }
                }
                // Everything else (mdat, mvhd, tkhd, mdia, edts, free, ...)
                // is stepped over without reading a byte of it.
                _ => {}
            }
            pos = b.end;
        }
        Ok(())
    }

    /// Read the header of the box starting at `pos`, at a level that ends
    /// at `end`, and check that the box fits inside that level.
    fn header(&mut self, pos: u64, end: u64, level: Level) -> io::Result<Header> {
        let room = end - pos;
        if room < 8 {
            return Ok(Header::NoRoom);
        }
        let mut h = [0u8; 8];
        self.read_at(pos, &mut h)?;
        let size32 = u32::from_be_bytes([h[0], h[1], h[2], h[3]]);
        let name = [h[4], h[5], h[6], h[7]];
        // The sentence for a broken size, only built when one is found.
        let bad = |what: String| Header::Bad(format!("in {}, the box '{}' at byte {} {}", level.name(), show_name(&name), pos, what));

        let (header_len, size) = match size32 {
            // Size 0: the box runs to the end of whatever holds it (for a
            // top-level box, the end of the file).
            0 => (8u64, room),
            // Size 1: the real size follows as a 64-bit number.
            1 => {
                if room < 16 {
                    return Ok(bad("says a 64-bit size follows, but the data ends first".to_string()));
                }
                let mut l = [0u8; 8];
                self.read_at(pos + 8, &mut l)?;
                let large = u64::from_be_bytes(l);
                if large < 16 {
                    return Ok(bad(format!("claims a 64-bit size of {large}, smaller than its own header")));
                }
                (16u64, large)
            }
            // 2 to 7 cannot even hold the 8-byte header.
            2..=7 => return Ok(bad(format!("claims a size of {size32}, smaller than its own header"))),
            n => (8u64, n as u64),
        };

        if size > room {
            return Ok(bad(format!("claims {size} bytes, but only {room} are left there")));
        }
        // size <= room, so pos + size cannot pass `end` and cannot overflow.
        Ok(Header::Box(BoxHeader { name, start: pos, body_start: pos + header_len, end: pos + size }))
    }

    /// True when this uuid box carries the XMP id.
    fn is_xmp(&mut self, b: &BoxHeader) -> io::Result<bool> {
        if b.end - b.body_start < 16 {
            return Ok(false); // too small to hold any id at all
        }
        let mut id = [0u8; 16];
        self.read_at(b.body_start, &mut id)?;
        Ok(id == XMP_UUID)
    }

    /// Read exactly `buf.len()` bytes starting at `pos`.
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> io::Result<()> {
        self.f.seek(SeekFrom::Start(pos))?;
        self.f.read_exact(buf)
    }
}

/// A box name made printable for a message: letters, digits and a few
/// marks as they are, anything else as '?'.
fn show_name(name: &[u8; 4]) -> String {
    name.iter().map(|&c| if c.is_ascii_graphic() || c == b' ' { c as char } else { '?' }).collect()
}

#[cfg(test)]
mod tests {
    //! Every file here is built byte by byte in the test; there are no
    //! binary fixtures. Each test was seen failing once against a
    //! deliberately broken copy of the code (then the code was restored);
    //! its doc comment says what that red run showed.

    use super::*;
    use std::ops::Range;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    // ---------- building boxes ----------

    /// One box with an ordinary 32-bit size.
    fn bx(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(name);
        v.extend_from_slice(payload);
        v
    }

    /// One box with the 64-bit size form (size field 1, then 8 more bytes).
    fn bx64(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = 1u32.to_be_bytes().to_vec();
        v.extend_from_slice(name);
        v.extend_from_slice(&((payload.len() + 16) as u64).to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// One box with size 0: "runs to the end of whatever holds me".
    fn bx0(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = 0u32.to_be_bytes().to_vec();
        v.extend_from_slice(name);
        v.extend_from_slice(payload);
        v
    }

    fn cat(parts: &[Vec<u8>]) -> Vec<u8> {
        parts.concat()
    }

    /// Filler bytes that are never ASCII letters, so no 4-letter box name
    /// can appear inside them by accident.
    fn filler(n: usize, seed: usize) -> Vec<u8> {
        (0..n).map(|i| 0x80 | ((i * 7 + seed * 13) % 128) as u8).collect()
    }

    // ---------- the pieces of a movie ----------

    /// Where the test videos were "filmed": ISO 6709 text, as phones write it.
    const PLACE: &[u8] = b"+37.3349-122.0090+030.000/";

    fn ftyp_qt() -> Vec<u8> {
        bx(b"ftyp", b"qt  \x00\x00\x02\x00qt  ")
    }

    fn ftyp_mp4() -> Vec<u8> {
        bx(b"ftyp", b"isom\x00\x00\x02\x00isomiso2avc1mp41")
    }

    fn mvhd() -> Vec<u8> {
        bx(b"mvhd", &filler(100, 1))
    }

    /// The frames: filler with a decoy "udta" box in the middle, to prove
    /// the walk never looks inside mdat.
    fn frames() -> Vec<u8> {
        cat(&[filler(150, 2), bx(b"udta", b"DECOY-inside-the-frames"), filler(150, 3)])
    }

    /// One video track whose chunk index (`stco`) points at two places in
    /// mdat, plus whatever extra boxes the test wants inside the track.
    fn trak(chunk_offsets: [u32; 2], extra: &[Vec<u8>]) -> Vec<u8> {
        let stco = bx(
            b"stco",
            &cat(&[
                vec![0, 0, 0, 0], // version and flags
                2u32.to_be_bytes().to_vec(),
                chunk_offsets[0].to_be_bytes().to_vec(),
                chunk_offsets[1].to_be_bytes().to_vec(),
            ]),
        );
        let stbl = bx(
            b"stbl",
            &cat(&[bx(b"stsd", &filler(40, 4)), bx(b"stts", &filler(16, 5)), bx(b"stsz", &filler(20, 6)), stco]),
        );
        let minf = bx(b"minf", &cat(&[bx(b"vmhd", &filler(12, 7)), stbl]));
        let mdia = bx(b"mdia", &cat(&[bx(b"mdhd", &filler(24, 8)), bx(b"hdlr", &filler(25, 9)), minf]));
        let edts = bx(b"edts", &bx(b"elst", &filler(20, 10)));
        let tkhd = bx(b"tkhd", &filler(84, 11));
        let mut parts = vec![tkhd, edts, mdia];
        parts.extend_from_slice(extra);
        bx(b"trak", &cat(&parts))
    }

    /// Apple's QuickTime metadata: a `meta` box with a handler, a `keys`
    /// list and an `ilst` of values, the location first.
    fn apple_meta() -> Vec<u8> {
        let hdlr = bx(b"hdlr", &cat(&[vec![0; 8], b"mdta".to_vec(), vec![0; 13]]));
        let key = |k: &[u8]| cat(&[((k.len() + 8) as u32).to_be_bytes().to_vec(), b"mdta".to_vec(), k.to_vec()]);
        let keys = bx(
            b"keys",
            &cat(&[
                vec![0; 4],
                3u32.to_be_bytes().to_vec(),
                key(b"com.apple.quicktime.location.ISO6709"),
                key(b"com.apple.quicktime.make"),
                key(b"com.apple.quicktime.model"),
            ]),
        );
        // Each item is a box named by its key's number, holding a `data` box
        // (type 1 = UTF-8 text, then a locale, then the text).
        let item = |i: u32, text: &[u8]| {
            let data = bx(b"data", &cat(&[1u32.to_be_bytes().to_vec(), vec![0; 4], text.to_vec()]));
            cat(&[((data.len() + 8) as u32).to_be_bytes().to_vec(), i.to_be_bytes().to_vec(), data])
        };
        let ilst = bx(b"ilst", &cat(&[item(1, PLACE), item(2, b"Apple"), item(3, b"iPhone 15 Pro")]));
        bx(b"meta", &cat(&[hdlr, keys, ilst]))
    }

    /// Android's `©xyz` inside `udta`: text length, language code, text.
    fn android_udta() -> Vec<u8> {
        let xyz = bx(
            b"\xA9xyz",
            &cat(&[(PLACE.len() as u16).to_be_bytes().to_vec(), vec![0x15, 0xC7], PLACE.to_vec()]),
        );
        bx(b"udta", &xyz)
    }

    /// 3GPP's `loci` inside `udta`: a named place with its coordinates.
    fn loci_udta() -> Vec<u8> {
        let fixed = |deg: f64| ((deg * 65536.0) as i32).to_be_bytes().to_vec(); // 16.16 fixed point
        let loci = bx(
            b"loci",
            &cat(&[
                vec![0; 4],       // version and flags
                vec![0x15, 0xC7], // language
                b"Grandma's house\0".to_vec(),
                vec![0], // role: shooting location
                fixed(-122.009),
                fixed(37.3349),
                fixed(30.0),
                b"Earth\0".to_vec(),
                b"\0".to_vec(),
            ]),
        );
        bx(b"udta", &loci)
    }

    /// A track-level `meta` box with a marker in it.
    fn track_meta() -> Vec<u8> {
        bx(b"meta", &cat(&[vec![0; 4], bx(b"ilst", b"TRACK-META-camera-serial-123")]))
    }

    /// XMP in a top-level uuid box, with GPS in it.
    fn xmp_uuid() -> Vec<u8> {
        bx(
            b"uuid",
            &cat(&[
                XMP_UUID.to_vec(),
                b"<x:xmpmeta><rdf:Description exif:GPSLatitude=\"37,20.094N\" exif:GPSLongitude=\"122,0.54W\"/></x:xmpmeta>"
                    .to_vec(),
            ]),
        )
    }

    /// Google's 360-degree video uuid: not XMP, needed by players.
    const SPHERICAL_UUID: [u8; 16] =
        [0xFF, 0xCC, 0x82, 0x63, 0xF8, 0x55, 0x4A, 0x93, 0x88, 0x14, 0x58, 0x7A, 0x02, 0x52, 0x1F, 0xDD];

    fn spherical_uuid() -> Vec<u8> {
        bx(b"uuid", &cat(&[SPHERICAL_UUID.to_vec(), b"<GSpherical:Spherical>true</GSpherical:Spherical>".to_vec()]))
    }

    #[derive(Debug, Clone, Copy)]
    enum Layout {
        /// moov before mdat ("fast start", what most Android phones and web
        /// tools write): stco offsets point past moov, so moov must not
        /// change size.
        MoovFirst,
        /// mdat before moov (what iPhones and most cameras write).
        MdatFirst,
    }

    /// What a test movie holds besides the playback boxes.
    #[derive(Default, Clone)]
    struct Extras {
        top: Vec<Vec<u8>>,
        moov: Vec<Vec<u8>>,
        trak: Vec<Vec<u8>>,
        mdat64: bool,
        mdat_size0: bool,
        moov_size0: bool,
    }

    /// Build a whole movie with real chunk offsets pointing into mdat.
    fn movie(layout: Layout, ftyp: Vec<u8>, ex: &Extras) -> Vec<u8> {
        let moov_with = |offsets: [u32; 2]| {
            let mut parts = vec![mvhd(), trak(offsets, &ex.trak)];
            parts.extend(ex.moov.iter().cloned());
            if ex.moov_size0 { bx0(b"moov", &cat(&parts)) } else { bx(b"moov", &cat(&parts)) }
        };
        let mdat = if ex.mdat64 {
            bx64(b"mdat", &frames())
        } else if ex.mdat_size0 {
            bx0(b"mdat", &frames())
        } else {
            bx(b"mdat", &frames())
        };
        let mdat_header = if ex.mdat64 { 16 } else { 8 };
        let mut head = vec![ftyp];
        head.extend(ex.top.iter().cloned());
        let head = cat(&head);
        match layout {
            Layout::MoovFirst => {
                // Build moov once to learn its size, then again with the
                // real offsets (the same size: the numbers are fixed-width).
                let moov_len = moov_with([0, 0]).len();
                let first = (head.len() + moov_len + mdat_header) as u32;
                cat(&[head, moov_with([first, first + 150]), mdat])
            }
            Layout::MdatFirst => {
                let wide = bx(b"wide", &[]);
                let first = (head.len() + wide.len() + mdat_header) as u32;
                cat(&[head, wide, mdat, moov_with([first, first + 150])])
            }
        }
    }

    /// Every kind of metadata this module neutralises, in one movie.
    fn everything() -> Extras {
        Extras {
            top: vec![xmp_uuid(), spherical_uuid()],
            moov: vec![apple_meta(), android_udta()],
            trak: vec![loci_udta(), track_meta()],
            ..Extras::default()
        }
    }

    // ---------- running the strip ----------

    fn temp_path() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "hum-strip-video-{}-{}.mp4",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ))
    }

    /// Strip a copy of `bytes` written to a real file, through the public
    /// path API, and read the result back.
    fn strip_file(bytes: &[u8]) -> (Vec<u8>, VideoStrip) {
        let path = temp_path();
        std::fs::write(&path, bytes).unwrap();
        let report = strip_video_metadata_in_place(&path).expect("strip must not fail on a readable file");
        let after = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        (after, report)
    }

    /// Strip a copy of `bytes` in memory.
    fn strip_mem(bytes: &[u8]) -> (Vec<u8>, VideoStrip) {
        let mut copy = bytes.to_vec();
        let report = strip_video_metadata_in_buffer(&mut copy).expect("strip must not fail in memory");
        (copy, report)
    }

    // ---------- checking the result ----------

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    /// The byte range of the first box with this name (found by its name,
    /// so only use it for names that appear once in the test file).
    fn find_box(buf: &[u8], name: &[u8; 4]) -> Range<usize> {
        let at = find(&buf[4..], name).expect("test setup: box present") + 4;
        let start = at - 4;
        let size32 = u32::from_be_bytes([buf[start], buf[start + 1], buf[start + 2], buf[start + 3]]) as usize;
        let size = match size32 {
            0 => buf.len() - start,
            1 => u64::from_be_bytes(buf[at + 4..at + 12].try_into().unwrap()) as usize,
            n => n,
        };
        start..start + size
    }

    /// The file is the same size, every reported box now says `free` and
    /// holds only zeros (its size field kept), the zero count matches, and
    /// no byte outside a reported box changed.
    fn assert_only_reported_boxes_changed(before: &[u8], after: &[u8], r: &VideoStrip) {
        assert_eq!(before.len(), after.len(), "the size must never change");
        let mut zeroed = 0u64;
        for b in &r.neutralised {
            let (s, e) = (b.offset as usize, (b.offset + b.size) as usize);
            assert!(e <= after.len(), "{} reported past the end", b.path);
            assert_eq!(&after[s + 4..s + 8], b"free", "{} must be renamed free", b.path);
            let body = if before[s..s + 4] == [0, 0, 0, 1] { s + 16 } else { s + 8 };
            assert_eq!(&after[s..s + 4], &before[s..s + 4], "{}: size field kept", b.path);
            assert_eq!(&after[s + 8..body], &before[s + 8..body], "{}: 64-bit size kept", b.path);
            assert!(after[body..e].iter().all(|&x| x == 0), "{} must hold only zeros", b.path);
            zeroed += (e - body) as u64;
        }
        assert_eq!(zeroed, r.bytes_zeroed, "bytes_zeroed must add up");
        for i in 0..before.len() {
            if before[i] != after[i] {
                let inside = r.neutralised.iter().any(|b| b.offset as usize <= i && i < (b.offset + b.size) as usize);
                assert!(inside, "byte {i} changed outside every reported box");
            }
        }
    }

    /// The boxes playback needs are byte-identical, and the chunk offsets
    /// still point at the same frame bytes.
    fn assert_playback_bytes_identical(before: &[u8], after: &[u8]) {
        assert_eq!(before.len(), after.len(), "the size must never change");
        for name in [b"ftyp", b"mvhd", b"tkhd", b"edts", b"mdia", b"stco", b"mdat"] {
            let r = find_box(before, name);
            assert_eq!(before[r.clone()], after[r], "'{}' must be byte-identical", show_name(name));
        }
        let stco = find_box(before, b"stco");
        let mdat = find_box(before, b"mdat");
        for i in 0..2 {
            let at = stco.start + 16 + 4 * i;
            let off = u32::from_be_bytes(before[at..at + 4].try_into().unwrap()) as usize;
            assert!(mdat.contains(&off), "test setup: chunk offset {off} must point into mdat {mdat:?}");
            assert_eq!(before[off..off + 16], after[off..off + 16], "the frames at chunk offset {off} must not move");
        }
    }

    // ---------- the tests ----------

    /// Seen red: with the `meta` arm for moov removed, the report came back
    /// empty (`[]` where `["moov/meta"]` was expected).
    #[test]
    fn apple_location_in_moov_meta_is_gone() {
        let ex = Extras { moov: vec![apple_meta()], ..Extras::default() };
        let before = movie(Layout::MdatFirst, ftyp_qt(), &ex);
        assert!(contains(&before, b"location.ISO6709") && contains(&before, PLACE), "test setup");

        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/meta"]);
        assert!(!contains(&after, b"location.ISO6709"), "the location key must be gone");
        assert!(!contains(&after, PLACE), "the coordinates must be gone");
        assert!(!contains(&after, b"iPhone 15 Pro"), "the model must be gone");
        assert!(r.complete() && r.kind == VideoKind::Mp4Family);
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// Seen red: with the `udta` arm for moov removed, the report came back
    /// empty (`[]` where `["moov/udta"]` was expected).
    #[test]
    fn android_xyz_in_moov_udta_is_gone() {
        let ex = Extras { moov: vec![android_udta()], ..Extras::default() };
        let before = movie(Layout::MoovFirst, ftyp_mp4(), &ex);
        assert!(contains(&before, b"\xA9xyz") && contains(&before, PLACE), "test setup");

        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/udta"]);
        assert!(!contains(&after, b"\xA9xyz"), "the ©xyz entry must be gone");
        assert!(!contains(&after, PLACE), "the coordinates must be gone");
        assert!(r.complete());
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// A writer that refuses to write the four letters `free`, standing in
    /// for a crash between zeroing a box and renaming it.
    struct CrashAtRename<'a>(Cursor<&'a mut [u8]>);
    impl Read for CrashAtRename<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.0.read(buf)
        }
    }
    impl Seek for CrashAtRename<'_> {
        fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
            self.0.seek(pos)
        }
    }
    impl Write for CrashAtRename<'_> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if buf == b"free" {
                return Err(io::Error::new(io::ErrorKind::Other, "the power went out"));
            }
            self.0.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.0.flush()
        }
    }

    /// A CRASH BETWEEN ZEROING AND RENAMING NEVER LEAVES THE PLACE BEHIND
    /// (review, 2026-10-11): when the run dies as it renames the first box,
    /// the coordinates are already gone, the box still has its old name,
    /// and a second run plans it again and finishes. Seen red 2026-10-11 with
    /// the zeroing step skipped before the renames: "the place is gone before
    /// any box is renamed".
    #[test]
    fn a_crash_between_zeroing_and_renaming_leaves_no_place_behind() {
        let ex = Extras { moov: vec![android_udta()], ..Extras::default() };
        let mut data = movie(Layout::MoovFirst, ftyp_mp4(), &ex);
        let len = data.len() as u64;
        // Where the box with the place starts: its own size-and-name header.
        let head = android_udta()[..8].to_vec();
        let at = data.windows(8).position(|w| w == head.as_slice()).expect("test setup: the udta box");
        let synced = std::cell::Cell::new(0);
        let crashed = strip_stream(&mut CrashAtRename(Cursor::new(&mut data[..])), len, |_| {
            synced.set(synced.get() + 1);
            Ok(())
        });
        assert!(crashed.is_err(), "the stand-in crash happened");
        assert_eq!(synced.get(), 1, "the zeros were made durable before the rename");
        assert!(!contains(&data, PLACE), "the place is gone before any box is renamed");
        assert_eq!(&data[at + 4..at + 8], b"udta", "and the box still has its old name");

        let again = strip_video_metadata_in_buffer(&mut data).unwrap();
        assert_eq!(again.paths(), vec!["moov/udta"], "a second run finishes the job");
        assert_eq!(&data[at + 4..at + 8], b"free", "and renames the box");
    }

    /// Seen red: with the walk no longer stepping into `trak`, the report
    /// came back empty (`[]` where `["moov/trak/udta"]` was expected).
    #[test]
    fn track_udta_location_is_gone() {
        let ex = Extras { trak: vec![loci_udta()], ..Extras::default() };
        let before = movie(Layout::MoovFirst, ftyp_mp4(), &ex);
        assert!(contains(&before, b"Grandma's house"), "test setup");

        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/trak/udta"]);
        assert!(!contains(&after, b"Grandma's house"), "the loci place must be gone");
        assert!(!contains(&after, b"loci"));
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// Seen red: with `is_xmp` answering yes for every uuid box, the report
    /// listed "uuid(xmp)" twice: the 360-degree video box was zeroed too.
    #[test]
    fn xmp_uuid_is_zeroed_and_another_uuid_is_left_alone() {
        let ex = Extras { top: vec![spherical_uuid(), xmp_uuid()], ..Extras::default() };
        let before = movie(Layout::MdatFirst, ftyp_mp4(), &ex);
        let (after, r) = strip_file(&before);

        assert_eq!(r.paths(), vec!["uuid(xmp)"]);
        assert!(!contains(&after, b"GPSLatitude"), "the XMP GPS must be gone");
        assert!(!contains(&after, &XMP_UUID), "the XMP id itself is zeroed with the rest");
        let sph = find(&before, &SPHERICAL_UUID).unwrap() - 8;
        let sph = sph..sph + spherical_uuid().len();
        assert_eq!(before[sph.clone()], after[sph], "the other uuid box must be byte-identical");
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// Seen red: with the zeros written 16 bytes past each box's end, the
    /// XMP box's overrun wiped the start of the 360-degree uuid box right
    /// after it ("the 360 uuid stays" failed).
    #[test]
    fn moov_before_mdat_keeps_every_playback_byte() {
        let before = movie(Layout::MoovFirst, ftyp_mp4(), &everything());
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["uuid(xmp)", "moov/trak/udta", "moov/trak/meta", "moov/meta", "moov/udta"]);
        for gone in [&b"location.ISO6709"[..], PLACE, b"Grandma's house", b"GPSLatitude", b"TRACK-META"] {
            assert!(!contains(&after, gone), "{:?} must be gone", String::from_utf8_lossy(gone));
        }
        assert!(contains(&after, &SPHERICAL_UUID), "the 360 uuid stays");
        assert!(contains(&after, b"DECOY-inside-the-frames"), "mdat is never read or written");
        assert!(r.complete());
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// Seen red: with the same 16-byte overrun, the zeros ran past the last
    /// box of moov at the end of the file and the file grew from 1492 to
    /// 1508 bytes ("the size must never change").
    #[test]
    fn mdat_before_moov_keeps_every_playback_byte() {
        let before = movie(Layout::MdatFirst, ftyp_qt(), &everything());
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["uuid(xmp)", "moov/trak/udta", "moov/trak/meta", "moov/meta", "moov/udta"]);
        for gone in [&b"location.ISO6709"[..], PLACE, b"Grandma's house", b"GPSLatitude", b"TRACK-META"] {
            assert!(!contains(&after, gone), "{:?} must be gone", String::from_utf8_lossy(gone));
        }
        assert!(contains(&after, b"DECOY-inside-the-frames"), "mdat is never read or written");
        assert!(r.complete());
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// Seen red: with a 64-bit box's end computed 8 bytes too far, the walk
    /// landed inside moov's header, never found moov, and the report came
    /// back empty (`[]` where `["moov/udta"]` was expected).
    #[test]
    fn a_64_bit_mdat_is_stepped_over() {
        let ex = Extras { moov: vec![android_udta()], mdat64: true, ..Extras::default() };
        let before = movie(Layout::MdatFirst, ftyp_qt(), &ex);
        assert_eq!(&before[find_box(&before, b"mdat").start..][..4], &[0, 0, 0, 1], "test setup: 64-bit size");

        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/udta"]);
        assert!(!contains(&after, PLACE));
        assert!(contains(&after, b"DECOY-inside-the-frames"), "the decoy inside mdat is never touched");
        assert!(r.complete());
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);
    }

    /// Seen red: with size 0 treated as a broken size, the mdat running to
    /// the end of the file was reported as broken ("the box 'mdat' at byte
    /// 563 size 0") and `complete()` was false.
    #[test]
    fn a_size_zero_last_box_runs_to_the_end_of_the_file() {
        // mdat last with size 0 (a recorder that never went back to fill it in).
        let ex = Extras { moov: vec![android_udta()], mdat_size0: true, ..Extras::default() };
        let before = movie(Layout::MoovFirst, ftyp_mp4(), &ex);
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/udta"]);
        assert!(r.complete(), "{:?}", r.stopped_early);
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);

        // moov last with size 0: its children are walked up to the end.
        let ex = Extras { moov: vec![android_udta()], moov_size0: true, ..Extras::default() };
        let before = movie(Layout::MdatFirst, ftyp_qt(), &ex);
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/udta"]);
        assert!(r.complete(), "{:?}", r.stopped_early);
        assert!(!contains(&after, PLACE));
        assert_only_reported_boxes_changed(&before, &after, &r);
        assert_playback_bytes_identical(&before, &after);

        // An XMP uuid last with size 0: zeroed right up to the end.
        let before = cat(&[movie(Layout::MoovFirst, ftyp_mp4(), &Extras::default()), bx0(b"uuid", &xmp_uuid()[8..])]);
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["uuid(xmp)"]);
        assert_eq!(r.neutralised[0].offset + r.neutralised[0].size, before.len() as u64);
        assert!(!contains(&after, b"GPSLatitude"));
        assert_only_reported_boxes_changed(&before, &after, &r);
    }

    /// Seen red: with both the walk's fit check and the write guard
    /// removed, a cut-off moov was walked as if it were whole and the strip
    /// failed with an I/O error (UnexpectedEof) instead of reporting where
    /// it stopped.
    #[test]
    fn truncated_files_never_panic_and_never_write_outside_the_file() {
        for layout in [Layout::MoovFirst, Layout::MdatFirst] {
            let full = movie(layout, ftyp_mp4(), &everything());
            for cut in 0..=full.len() {
                let before = &full[..cut];
                let (after, r) = strip_mem(before);
                assert_only_reported_boxes_changed(before, &after, &r);
                if cut % 37 == 0 {
                    // Now and then through a real file as well: same answer.
                    let (after_file, r_file) = strip_file(before);
                    assert_eq!(after_file, after, "file and memory must agree at cut {cut}");
                    assert_eq!(r_file, r);
                }
            }
            // The whole file is clean and complete.
            assert!(strip_mem(&full).1.complete());
        }
        // A cut inside moov (mdat first): moov claims more than is left, so
        // it is not walked, nothing is written, and the report says so.
        let full = movie(Layout::MdatFirst, ftyp_mp4(), &everything());
        let moov = find_box(&full, b"moov");
        let cut = &full[..moov.start + 40];
        let (after, r) = strip_mem(cut);
        assert_eq!(after[moov.start..], cut[moov.start..], "nothing in the cut-off moov may be written");
        assert_only_reported_boxes_changed(cut, &after, &r);
        assert!(!r.found_moov && !r.complete());
        assert_eq!(r.paths(), vec!["uuid(xmp)"], "only the XMP before the cut is reached");
        assert_eq!(r.stopped_early.len(), 1, "{:?}", r.stopped_early);
    }

    /// Seen red: with one bad size stopping the whole walk (not just its
    /// level), the report held only `["moov/udta"]`: the XMP box after moov
    /// was no longer neutralised.
    #[test]
    fn a_box_running_past_its_parent_stops_only_that_level() {
        // Inside moov: a good udta, then a trak claiming 5000 bytes (far
        // more than moov has left), then an Apple meta the walk cannot reach.
        let bad_trak = cat(&[5000u32.to_be_bytes().to_vec(), b"trak".to_vec(), filler(40, 12)]);
        let moov = bx(b"moov", &cat(&[mvhd(), android_udta(), bad_trak, apple_meta()]));
        let before = cat(&[ftyp_mp4(), moov.clone(), xmp_uuid(), bx(b"mdat", &frames())]);
        let (after, r) = strip_file(&before);

        assert_eq!(r.paths(), vec!["moov/udta", "uuid(xmp)"], "before the bad box, and the next level up, still done");
        assert_eq!(r.stopped_early.len(), 1);
        assert!(r.stopped_early[0].contains("in moov") && r.stopped_early[0].contains("'trak'"), "{}", r.stopped_early[0]);
        assert!(!r.complete());
        // The honest limit: after a broken box nobody can know where the
        // next one starts, so the meta behind it is still there.
        assert!(contains(&after, b"location.ISO6709"));
        assert_only_reported_boxes_changed(&before, &after, &r);
        let mdat = find_box(&before, b"mdat");
        assert_eq!(before[mdat.clone()], after[mdat]);

        // A size under 8 inside a track stops that track only.
        let tiny = cat(&[4u32.to_be_bytes().to_vec(), b"udta".to_vec(), filler(20, 13)]);
        let trak = bx(b"trak", &cat(&[bx(b"tkhd", &filler(84, 14)), tiny]));
        let moov = bx(b"moov", &cat(&[mvhd(), trak, android_udta()]));
        let before = cat(&[ftyp_mp4(), moov, bx(b"mdat", &frames())]);
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["moov/udta"], "the moov level carries on after the broken track");
        assert_eq!(r.stopped_early.len(), 1);
        assert!(r.stopped_early[0].contains("a moov/trak"), "{}", r.stopped_early[0]);
        assert_only_reported_boxes_changed(&before, &after, &r);
    }

    /// Seen red: with every file sniffed as MP4, the read-only WebM file was
    /// opened for writing and the call failed with "Access is denied."
    #[test]
    fn webm_is_left_untouched_and_never_opened_for_writing() {
        // An EBML header for a WebM file, then bytes that happen to spell
        // MP4 box names, which must not matter.
        let mut webm = vec![0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81, 0x01];
        webm.extend_from_slice(b"\x42\x82\x84webm");
        webm.extend(cat(&[bx(b"moov", &android_udta()), xmp_uuid()]));

        let path = temp_path();
        std::fs::write(&path, &webm).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms.clone()).unwrap();

        let result = strip_video_metadata_in_place(&path);
        let after = std::fs::read(&path).unwrap();
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
        let _ = std::fs::remove_file(&path);

        let r = result.expect("a WebM file must not even be opened for writing");
        assert_eq!(r.kind, VideoKind::Matroska);
        assert!(!r.changed() && !r.complete());
        assert_eq!(after, webm);

        let (after, r) = strip_mem(&webm);
        assert_eq!((r.kind, after), (VideoKind::Matroska, webm));
        let (after, r) = strip_mem(b"just some text, not a video");
        assert_eq!((r.kind, after.as_slice()), (VideoKind::Other, &b"just some text, not a video"[..]));
    }

    /// Seen red: with the "only in a file with a movie" condition removed,
    /// the HEIF picture's own meta box was zeroed (the report was no longer
    /// empty).
    #[test]
    fn a_heif_still_picture_keeps_its_top_level_meta() {
        let meta = bx(b"meta", &cat(&[vec![0; 4], bx(b"hdlr", &filler(25, 15)), bx(b"pitm", &[0, 0, 0, 0, 0, 1])]));
        let before = cat(&[bx(b"ftyp", b"heic\0\0\0\0mif1heic"), meta, bx(b"mdat", &frames())]);
        let (after, r) = strip_file(&before);
        assert_eq!(r.kind, VideoKind::Mp4Family);
        assert!(!r.found_moov && !r.changed() && !r.complete());
        assert_eq!(after, before);

        // In a movie, the same top-level meta is neutralised.
        let before = cat(&[movie(Layout::MoovFirst, ftyp_mp4(), &Extras::default()), bx(b"meta", b"\0\0\0\0TOP-META")]);
        let (after, r) = strip_file(&before);
        assert_eq!(r.paths(), vec!["meta"]);
        assert!(!contains(&after, b"TOP-META"));
    }

    /// Seen red: with the size check dropped, an `ftyp` claiming 4 bytes
    /// counted as an MP4 ("size smaller than a header" failed).
    #[test]
    fn is_mp4_family_reads_the_first_box() {
        assert!(is_mp4_family(&ftyp_mp4()));
        assert!(is_mp4_family(&ftyp_qt()[..8]), "eight bytes are enough");
        assert!(is_mp4_family(&bx64(b"ftyp", b"isom\0\0\0\0")), "a 64-bit size is allowed");
        assert!(!is_mp4_family(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81]), "WebM");
        assert!(!is_mp4_family(b"\xFF\xD8\xFF\xE0\x00\x10JFIF"), "JPEG");
        assert!(!is_mp4_family(&ftyp_mp4()[..7]), "too short to tell");
        assert!(!is_mp4_family(b"\0\0\0\x04ftypisom"), "size smaller than a header");
        assert!(!is_mp4_family(b"\0\0\0\0ftypisom"), "size 0: the whole file is ftyp");
        assert!(!is_mp4_family(&bx(b"moov", &mvhd())), "no ftyp first");
    }

    /// Seen red: with `free` boxes inside moov and trak neutralised again,
    /// the second run reported the four boxes the first run had made there.
    #[test]
    fn running_twice_changes_nothing_more() {
        for layout in [Layout::MoovFirst, Layout::MdatFirst] {
            let before = movie(layout, ftyp_mp4(), &everything());
            let (once, r1) = strip_file(&before);
            assert_eq!(r1.neutralised.len(), 5);
            let (twice, r2) = strip_file(&once);
            assert!(!r2.changed() && r2.bytes_zeroed == 0, "{:?}", r2.paths());
            assert!(r2.complete());
            assert_eq!(twice, once);
        }
    }

    // ---------- fuzz-style ----------

    /// A small fixed-seed random number generator (xorshift64*), so the
    /// "random" files are the same on every run and need no new crate.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            if n == 0 { 0 } else { self.next() % n }
        }
        fn bytes(&mut self, n: usize) -> Vec<u8> {
            (0..n).map(|_| self.next() as u8).collect()
        }
    }

    const NAMES: [&[u8; 4]; 12] =
        [b"moov", b"trak", b"udta", b"meta", b"uuid", b"mdat", b"free", b"mvhd", b"tkhd", b"ftyp", b"mdia", b"\xA9xyz"];

    /// A random run of boxes, nested up to three deep, with sizes that are
    /// usually right and sometimes 0, 64-bit, or simply wrong.
    fn random_boxes(rng: &mut Rng, depth: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..rng.below(5) {
            let name: [u8; 4] = if rng.below(8) == 0 {
                let b = rng.bytes(4);
                [b[0], b[1], b[2], b[3]]
            } else {
                *NAMES[rng.below(NAMES.len() as u64) as usize]
            };
            let payload = if depth < 3 && matches!(&name, b"moov" | b"trak" | b"udta" | b"meta") && rng.below(4) != 0 {
                random_boxes(rng, depth + 1)
            } else if &name == b"uuid" && rng.below(2) == 0 {
                let n = rng.below(20) as usize;
                cat(&[XMP_UUID.to_vec(), rng.bytes(n)])
            } else {
                let n = rng.below(40) as usize;
                rng.bytes(n)
            };
            match rng.below(10) {
                0 => out.extend(bx0(&name, &payload)),
                1 => {
                    let large = if rng.below(2) == 0 { (payload.len() + 16) as u64 } else { rng.next() >> rng.below(64) };
                    out.extend(1u32.to_be_bytes());
                    out.extend(name);
                    out.extend(large.to_be_bytes());
                    out.extend(payload);
                }
                2 => {
                    let wrong = if rng.below(2) == 0 { rng.below(80) as u32 } else { rng.next() as u32 };
                    out.extend(wrong.to_be_bytes());
                    out.extend(name);
                    out.extend(payload);
                }
                _ => out.extend(bx(&name, &payload)),
            }
        }
        out
    }

    fn random_file(rng: &mut Rng, i: usize) -> Vec<u8> {
        match rng.below(4) {
            // Plain noise, sometimes with an ftyp header in front.
            0 => {
                let n = rng.below(200) as usize;
                let noise = rng.bytes(n);
                if rng.below(2) == 0 { cat(&[ftyp_mp4(), noise]) } else { noise }
            }
            // A random box tree behind a real ftyp.
            1 => cat(&[ftyp_mp4(), random_boxes(rng, 0)]),
            // A real movie, damaged: a few bytes changed, maybe cut short.
            _ => {
                let layout = if i % 2 == 0 { Layout::MoovFirst } else { Layout::MdatFirst };
                let mut f = movie(layout, ftyp_mp4(), &everything());
                for _ in 0..rng.below(4) {
                    let at = rng.below(f.len() as u64) as usize;
                    f[at] = rng.next() as u8;
                }
                if rng.below(3) == 0 {
                    let keep = rng.below(f.len() as u64 + 1) as usize;
                    f.truncate(keep);
                }
                f
            }
        }
    }

    /// Seen red: with the zeros written one byte past each box's end, the
    /// in-memory strip failed trying to write past the end of the buffer
    /// (WriteZero) on a box at the very end of a file.
    #[test]
    fn random_inputs_never_panic_or_change_size() {
        let mut rng = Rng(0x5EED_CAFE_F00D_0001);
        let mut walked = 0;
        for i in 0..4000 {
            let before = random_file(&mut rng, i);
            let (after, r) = strip_mem(&before);
            assert_only_reported_boxes_changed(&before, &after, &r);
            if r.kind == VideoKind::Mp4Family {
                walked += 1;
            }
            // Running again finds nothing more to do.
            let (again, r2) = strip_mem(&after);
            assert!(!r2.changed(), "input {i}: second run changed {:?}", r2.paths());
            assert_eq!(again, after, "input {i}");
            // Now and then through a real file: the same answer.
            if i % 64 == 0 {
                let (after_file, r_file) = strip_file(&before);
                assert_eq!(after_file, after, "input {i}: file and memory must agree");
                assert_eq!(r_file, r);
            }
        }
        assert!(walked > 2000, "most inputs should reach the box walk, got {walked}");
    }
}
