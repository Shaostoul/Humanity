//! HEIF and AVIF still picture metadata removal, in place (privacy
//! hardening, 2026-10-11).
//!
//! An iPhone saves its photos as HEIC, and more and more phones and
//! cameras save AVIF. Both are HEIF files: the same box format as an MP4
//! (an `ftyp` box first, then a row of boxes, each starting with a 4-byte
//! size and a 4-letter name), but with no movie in them. The picture lives
//! in the top-level `meta` box as a set of numbered ITEMS: the image itself
//! (on a phone, usually a grid of tiles), a thumbnail, sometimes a depth
//! map, and the photo's metadata. The metadata items are what this module
//! is about:
//! - an `Exif` item: the camera's EXIF block, which on a phone holds the
//!   GPS position where the photo was taken, the time it was taken, and
//!   the make, model and lens of the camera;
//! - XMP: a `mime` item whose content type is `application/rdf+xml`,
//!   which editing apps write and which can carry GPS as well.
//!
//! Where an item's bytes are is written in two tables inside `meta`:
//! `iinf` (item info: one `infe` entry per item, giving its number and its
//! type) and `iloc` (item locations: for each item number, one or more
//! extents, each a start and a length). This module reads both tables,
//! finds every extent of every Exif and XMP item, and overwrites those
//! bytes with zeros where they stand. Nothing moves and the file keeps its
//! exact size, so every other item's location stays right and the picture
//! shows as before; only the metadata now reads as empty. The image tiles,
//! the thumbnail, the item properties (`iprp`), the item references
//! (`iref`) and the tables themselves are never written.
//!
//! An extent's start is counted in one of three ways, the item's
//! "construction method": 0, from the start of the file (the usual one; the
//! bytes sit in the `mdat` box); 1, from the start of the `idat` box inside
//! `meta` (small items kept beside the tables); 2, an item made out of parts
//! of other items. Methods 0 and 1 are handled; an Exif or XMP item built
//! with method 2 is left as it is and reported.
//!
//! Safety rules:
//! - An extent is zeroed only when it lies wholly inside the body of an
//!   `mdat` box (method 0) or of the `idat` box (method 1), and overlaps no
//!   byte of any other item. Otherwise that whole item is left as it is,
//!   and the report says why in `HeifStrip::stopped_early`.
//! - When the tables themselves cannot be read with certainty (a size that
//!   makes no sense, a version this module does not know, two `iinf`
//!   boxes, a table cut short), NOTHING is written: without the whole
//!   table, the bytes of the other items cannot be known either, and they
//!   must never be touched.
//! - The file is read through a read-only handle, and opened for writing
//!   only when there is something to zero. A file that is not a HEIF still
//!   picture (a video, a JPEG, anything else) is never opened for writing.
//! - Every offset is added with checked arithmetic, every size is checked
//!   against the box holding it, and nothing panics on any input.
//!
//! A report with `stopped_early` entries means "not proven clean": the
//! upload rules refuse such a picture unless the person chose to keep its
//! hidden information (relay/upload_rules.rs `finish_file_as`).
//!
//! A HEIF file that also has a movie in it (`moov`: an image sequence, or a
//! video with HEIF brands) is left to the video strip
//! (strip_video_metadata.rs) and never opened for writing here.

use std::collections::{BTreeMap, HashSet};
use std::fs::{File, OpenOptions};
use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// The `ftyp` brands that mark a HEIF picture file: `heic`, `heix`, `heim`
/// and `heis` (HEVC pictures, as iPhones write), `mif1` and `msf1` (the
/// general HEIF picture and sequence brands), `avif` and `avis` (AV1).
pub const HEIF_BRANDS: [&[u8; 4]; 8] = [b"heic", b"heix", b"heim", b"heis", b"mif1", b"msf1", b"avif", b"avis"];

/// How many bytes are read to look at the `ftyp` box. A real one is a few
/// dozen bytes.
const FIRST_LOOK: u64 = 4096;

/// The most top-level boxes walked. A HEIF picture has three or four.
const MAX_TOP_BOXES: u32 = 100_000;

/// The most boxes walked inside `meta`. A HEIF picture has about ten.
const MAX_META_CHILDREN: u32 = 10_000;

/// The largest `iinf` or `iloc` table read into memory. An iPhone photo's
/// tables are a few kilobytes (about fifty items for a tiled picture).
const MAX_TABLE_BYTES: u64 = 8 * 1024 * 1024;

/// The most items and the most extents read from the tables. Only stops a
/// crafted file from keeping the relay busy: an extent can be written in
/// zero bytes (both of its sizes 0), so the table's own size is no limit.
const MAX_ITEMS: usize = 100_000;
const MAX_EXTENTS: usize = 1_000_000;

/// Zero bytes are written in pieces of this size.
const ZERO_CHUNK: usize = 64 * 1024;

/// What kind of file the strip found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeifKind {
    /// A HEIF or AVIF still picture: an `ftyp` box naming one of the
    /// `HEIF_BRANDS`, and no movie in the file. Its items were looked at.
    Still,
    /// HEIF brands, but a movie (`moov`) in it too: an image sequence or a
    /// video. Left to the video strip; never opened for writing here.
    WithMovie,
    /// Not a HEIF file (no `ftyp` box first, or none of the HEIF brands in
    /// it). Never opened for writing.
    NotHeif,
}

/// What `strip_heif_metadata_in_place` did to one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeifStrip {
    /// What kind of file it was. Only a `Still` is ever changed.
    pub kind: HeifKind,
    /// Every item whose bytes were overwritten with zeros: its item number,
    /// its item type (`Exif`, or `mime` for XMP), and how many bytes.
    pub items_zeroed: Vec<(u32, String, u64)>,
    /// Every byte range overwritten, as (first byte, the byte after the
    /// last), in the order written. For logs and tests.
    pub extents: Vec<(u64, u64)>,
    /// One plain sentence for each thing that could not be read or done:
    /// a table that made no sense, or an Exif or XMP item left as it is
    /// (and why). Empty for a well-formed picture.
    pub stopped_early: Vec<String>,
}

impl HeifStrip {
    fn left_alone(kind: HeifKind) -> Self {
        HeifStrip { kind, items_zeroed: Vec::new(), extents: Vec::new(), stopped_early: Vec::new() }
    }

    /// True when at least one item was zeroed.
    pub fn changed(&self) -> bool {
        !self.items_zeroed.is_empty()
    }

    /// How many bytes were overwritten with zeros.
    pub fn bytes_zeroed(&self) -> u64 {
        self.items_zeroed.iter().map(|(_, _, n)| n).sum()
    }

    /// True for a still picture whose tables were read through and whose
    /// every Exif and XMP item was zeroed (or that had none).
    pub fn complete(&self) -> bool {
        self.kind == HeifKind::Still && self.stopped_early.is_empty()
    }
}

/// True when these first bytes start a HEIF or AVIF still picture: the
/// first box is an `ftyp` box whose major brand or one of whose compatible
/// brands is in `HEIF_BRANDS`, and no `moov` box (a movie) is among the
/// top-level boxes these bytes reach. Given only the start of a file, a
/// `moov` further on cannot be seen; `strip_heif_metadata_in_place` walks
/// the whole top level and leaves a file with a movie alone.
pub fn is_heif_still(first_bytes: &[u8]) -> bool {
    has_heif_brand(first_bytes) && !moov_in_view(first_bytes)
}

/// The `ftyp` box first, with a HEIF brand in it.
fn has_heif_brand(b: &[u8]) -> bool {
    if b.len() < 8 || &b[4..8] != b"ftyp" {
        return false;
    }
    let (header_len, size) = match be32(&b[0..4]) {
        // Size 0 would make the ftyp box the whole file: no picture in it.
        0 => return false,
        1 => {
            if b.len() < 16 {
                return false;
            }
            let large = be64(&b[8..16]);
            if large < 16 {
                return false;
            }
            (16usize, large)
        }
        2..=7 => return false,
        n => (8usize, n as u64),
    };
    // The brands this view reaches: the major brand, the minor version
    // (4 bytes, not a brand), then the compatible brands.
    let end = size.min(b.len() as u64) as usize;
    if end < header_len + 4 {
        return false;
    }
    let body = &b[header_len..end];
    let major = &body[0..4];
    let compatible = body.get(8..).unwrap_or(&[]);
    std::iter::once(major).chain(compatible.chunks_exact(4)).any(|brand| HEIF_BRANDS.iter().any(|h| &h[..] == brand))
}

/// True when a `moov` box header is among the top-level boxes in `b`.
fn moov_in_view(b: &[u8]) -> bool {
    let len = b.len() as u64;
    let mut pos = 0u64;
    let mut seen = 0u32;
    while pos + 8 <= len && seen < MAX_TOP_BOXES {
        seen += 1;
        let at = pos as usize;
        if &b[at + 4..at + 8] == b"moov" {
            return true;
        }
        let size = match be32(&b[at..at + 4]) {
            0 => return false, // runs to the end: nothing after it
            1 => {
                if pos + 16 > len {
                    return false;
                }
                be64(&b[at + 8..at + 16])
            }
            n => n as u64,
        };
        if size < 8 {
            return false;
        }
        match pos.checked_add(size) {
            Some(next) => pos = next,
            None => return false,
        }
    }
    false
}

/// Overwrite the Exif and XMP items of the HEIF or AVIF still picture at
/// `path` with zeros, in place, without changing its size or anything else
/// in it. See the module notes for exactly what is and is not written.
///
/// Returns an error only when the file cannot be opened, read or written.
/// A file that is not a HEIF still is never opened for writing; one whose
/// tables cannot be read is not written at all, and the report says why.
pub fn strip_heif_metadata_in_place(path: &Path) -> io::Result<HeifStrip> {
    // Everything is decided through a read-only handle first.
    let mut reader = File::open(path)?;
    let len = reader.metadata()?.len();
    let (mut report, plans) = plan(&mut reader, len)?;
    drop(reader);
    if plans.is_empty() {
        return Ok(report);
    }

    // Opening for writing (without truncating) never shortens or grows it.
    let mut file = OpenOptions::new().write(true).open(path)?;
    if file.metadata()?.len() != len {
        return Err(io::Error::new(io::ErrorKind::Other, "the file changed size while it was being read"));
    }
    apply(&mut file, len, plans, &mut report)?;
    file.flush()?;
    // The zeros reach the disk before anyone serves the file.
    file.sync_all()?;
    Ok(report)
}

/// The same work on a picture held in memory. The buffer keeps its length.
pub fn strip_heif_metadata_in_buffer(data: &mut [u8]) -> io::Result<HeifStrip> {
    let len = data.len() as u64;
    let (mut report, plans) = plan(&mut Cursor::new(&*data), len)?;
    if !plans.is_empty() {
        apply(&mut Cursor::new(data), len, plans, &mut report)?;
    }
    Ok(report)
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

/// An item to zero: its number, its type, and its extents in the file.
#[derive(Debug)]
struct Plan {
    item_id: u32,
    item_type: &'static str,
    extents: Vec<(u64, u64)>,
}

/// Write the zeros the plans name, and fill in the report.
fn apply<F: Write + Seek>(f: &mut F, len: u64, plans: Vec<Plan>, report: &mut HeifStrip) -> io::Result<()> {
    for p in plans {
        // A second lock on the same door: never write outside the file. The
        // planning already refuses such an extent; if one ever got through,
        // the whole item is left as it is and the report is not complete,
        // so a photo still holding its Exif is never called clean.
        if let Some(&(start, end)) = p.extents.iter().find(|&&(s, e)| !(s < e && e <= len)) {
            report.stopped_early.push(format!(
                "item {} ({}) has an extent (bytes {start} to {end}) outside the file, which a second check refused to write, so it was left as it is",
                p.item_id, p.item_type
            ));
            continue;
        }
        let mut bytes = 0;
        for (start, end) in p.extents {
            write_zeros(f, start, end)?;
            bytes += end - start;
            report.extents.push((start, end));
        }
        report.items_zeroed.push((p.item_id, p.item_type.to_string(), bytes));
    }
    f.flush()
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

// ---------- the walk ----------

/// Why a walk stopped: a read failed, or something made no sense (the
/// sentence says what).
enum Stop {
    Io(io::Error),
    Why(String),
}

impl From<io::Error> for Stop {
    fn from(e: io::Error) -> Self {
        Stop::Io(e)
    }
}

/// A sentence as a stop.
fn why<T>(s: impl Into<String>) -> Result<T, Stop> {
    Err(Stop::Why(s.into()))
}

/// One box header.
#[derive(Debug, Clone, Copy)]
struct BoxHeader {
    name: [u8; 4],
    start: u64,
    body_start: u64,
    end: u64,
}

/// What reading a header found.
enum Header {
    Box(BoxHeader),
    /// Fewer than 8 bytes left in the box holding it: nothing can hide
    /// there, and the level is finished.
    NoRoom,
    /// A size that makes no sense. The sentence says why.
    Bad(String),
}

/// Decode the box header in `h` (the first up to 16 bytes at `pos`), for a
/// box that must end by `end`. `place` names the box holding it, in words.
fn decode_header(h: &[u8], pos: u64, end: u64, place: &str) -> Header {
    let room = end.saturating_sub(pos);
    if room < 8 || h.len() < 8 {
        return Header::NoRoom;
    }
    let name = [h[4], h[5], h[6], h[7]];
    let bad = |what: String| Header::Bad(format!("in {place}, the box '{}' at byte {pos} {what}", show_name(&name)));
    let (header_len, size) = match be32(&h[0..4]) {
        // Size 0: the box runs to the end of whatever holds it.
        0 => (8u64, room),
        // Size 1: the real size follows as a 64-bit number.
        1 => {
            if room < 16 || h.len() < 16 {
                return bad("says a 64-bit size follows, but the data ends first".to_string());
            }
            let large = be64(&h[8..16]);
            if large < 16 {
                return bad(format!("claims a 64-bit size of {large}, smaller than its own header"));
            }
            (16u64, large)
        }
        n @ 2..=7 => return bad(format!("claims a size of {n}, smaller than its own header")),
        n => (8u64, n as u64),
    };
    if size > room {
        return bad(format!("claims {size} bytes, but only {room} are left there"));
    }
    // size <= room, so pos + size cannot pass `end` and cannot overflow.
    Header::Box(BoxHeader { name, start: pos, body_start: pos + header_len, end: pos + size })
}

/// Read and decode the header of the box at `pos` in the file.
fn read_header<F: Read + Seek>(f: &mut F, pos: u64, end: u64, place: &str) -> io::Result<Header> {
    let want = end.saturating_sub(pos).min(16) as usize;
    let mut h = [0u8; 16];
    read_at(f, pos, &mut h[..want])?;
    Ok(decode_header(&h[..want], pos, end, place))
}

/// Read exactly `buf.len()` bytes starting at `pos`.
fn read_at<F: Read + Seek>(f: &mut F, pos: u64, buf: &mut [u8]) -> io::Result<()> {
    f.seek(SeekFrom::Start(pos))?;
    f.read_exact(buf)
}

/// What the top level of the file holds.
struct Top {
    moov: bool,
    metas: Vec<BoxHeader>,
    /// The `mdat` boxes' bodies (first byte, byte after the last), in file
    /// order: the only places a method-0 extent may be zeroed.
    mdats: Vec<(u64, u64)>,
    stopped_early: Vec<String>,
}

/// Walk the top-level boxes, reading headers only.
fn walk_top<F: Read + Seek>(f: &mut F, len: u64) -> io::Result<Top> {
    let mut top = Top { moov: false, metas: Vec::new(), mdats: Vec::new(), stopped_early: Vec::new() };
    let mut pos = 0;
    let mut seen = 0;
    while pos < len {
        if seen >= MAX_TOP_BOXES {
            top.stopped_early.push(format!("stopped after {MAX_TOP_BOXES} top-level boxes, more than any real picture has"));
            break;
        }
        seen += 1;
        match read_header(f, pos, len, "the top level of the file")? {
            Header::NoRoom => break,
            Header::Bad(why) => {
                // Nobody can know where the next box starts.
                top.stopped_early.push(why);
                break;
            }
            Header::Box(b) => {
                match &b.name {
                    b"moov" => top.moov = true,
                    b"meta" => top.metas.push(b),
                    b"mdat" => top.mdats.push((b.body_start, b.end)),
                    _ => {}
                }
                pos = b.end;
            }
        }
    }
    Ok(top)
}

/// Decide everything: what kind of file it is, and which bytes to zero.
/// Nothing is written here.
fn plan<F: Read + Seek>(f: &mut F, len: u64) -> io::Result<(HeifStrip, Vec<Plan>)> {
    let mut first = vec![0u8; len.min(FIRST_LOOK) as usize];
    f.seek(SeekFrom::Start(0))?;
    let got = read_up_to(f, &mut first)?;
    first.truncate(got);
    if !has_heif_brand(&first) {
        return Ok((HeifStrip::left_alone(HeifKind::NotHeif), Vec::new()));
    }

    let top = walk_top(f, len)?;
    if top.moov {
        return Ok((HeifStrip::left_alone(HeifKind::WithMovie), Vec::new()));
    }
    let mut report = HeifStrip { stopped_early: top.stopped_early, ..HeifStrip::left_alone(HeifKind::Still) };
    let meta = match top.metas.as_slice() {
        [meta] => *meta,
        [] => {
            report.stopped_early.push("found no meta box at the top level, which is where a HEIF picture keeps its items".to_string());
            return Ok((report, Vec::new()));
        }
        more => {
            report.stopped_early.push(format!(
                "found {} meta boxes at the top level where a HEIF picture has one, so which one a viewer reads cannot be told",
                more.len()
            ));
            return Ok((report, Vec::new()));
        }
    };
    match plan_items(f, len, &meta, &top.mdats) {
        Ok((plans, skipped)) => {
            report.stopped_early.extend(skipped);
            Ok((report, plans))
        }
        Err(Stop::Why(why)) => {
            report.stopped_early.push(why);
            Ok((report, Vec::new()))
        }
        Err(Stop::Io(e)) => Err(e),
    }
}

/// Read the `meta` box's tables and decide which extents to zero. Returns
/// the plans and a sentence for each Exif or XMP item left as it is. A
/// `Stop::Why` means the tables could not be read, and nothing is zeroed.
fn plan_items<F: Read + Seek>(f: &mut F, len: u64, meta: &BoxHeader, mdats: &[(u64, u64)]) -> Result<(Vec<Plan>, Vec<String>), Stop> {
    // `meta` is a full box: a version byte and three flag bytes first.
    if meta.end - meta.body_start < 4 {
        return why("the meta box is too small to hold its version");
    }
    let mut version = [0u8; 4];
    read_at(f, meta.body_start, &mut version)?;
    if version[0] != 0 {
        return why(format!("the meta box has version {}, and only version 0 exists", version[0]));
    }

    let (mut iinf, mut iloc, mut idat) = (None, None, None);
    let mut pos = meta.body_start + 4;
    let mut seen = 0;
    while pos < meta.end {
        seen += 1;
        if seen > MAX_META_CHILDREN {
            return why(format!("the meta box holds more than {MAX_META_CHILDREN} boxes, more than any real picture has"));
        }
        let b = match read_header(f, pos, meta.end, "the meta box")? {
            Header::Box(b) => b,
            Header::NoRoom => break,
            Header::Bad(w) => return why(w),
        };
        let slot = match &b.name {
            b"iinf" => Some(&mut iinf),
            b"iloc" => Some(&mut iloc),
            b"idat" => Some(&mut idat),
            _ => None,
        };
        if let Some(slot) = slot {
            if slot.is_some() {
                return why(format!(
                    "the meta box holds two '{}' boxes where a HEIF picture has one, so which one a viewer reads cannot be told",
                    show_name(&b.name)
                ));
            }
            *slot = Some(b);
        }
        pos = b.end;
    }
    let Some(iinf) = iinf else {
        return why("the meta box has no item list (iinf), so which items hold metadata cannot be told");
    };
    let Some(iloc) = iloc else {
        return why("the meta box has no item locations (iloc), so where the metadata is cannot be told");
    };

    let items = parse_iinf(&read_body(f, &iinf, "item list (iinf)")?, iinf.body_start)?;
    let entries = parse_iloc(&read_body(f, &iloc, "item locations (iloc)")?)?;
    let idat = idat.map(|b| (b.body_start, b.end));
    Ok(choose(&items, &entries, idat, mdats, len))
}

/// A box's contents read into memory, if they are not too big to be real.
fn read_body<F: Read + Seek>(f: &mut F, b: &BoxHeader, what: &str) -> Result<Vec<u8>, Stop> {
    let n = b.end - b.body_start;
    if n > MAX_TABLE_BYTES {
        return why(format!("the {what} is {n} bytes, more than any real picture's"));
    }
    let mut body = vec![0u8; n as usize];
    read_at(f, b.body_start, &mut body)?;
    Ok(body)
}

// ---------- the two tables ----------

/// One item from the item list.
#[derive(Debug)]
struct ItemInfo {
    id: u32,
    item_type: [u8; 4],
    /// For a `mime` item, its content type.
    content_type: Vec<u8>,
}

/// Reads big-endian numbers and strings from a table in memory. Every read
/// answers None at the end of the data instead of panicking.
struct Fields<'a> {
    b: &'a [u8],
    at: usize,
}

impl<'a> Fields<'a> {
    fn new(b: &'a [u8]) -> Self {
        Fields { b, at: 0 }
    }
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        let s = self.b.get(self.at..end)?;
        self.at = end;
        Some(s)
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|s| s[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|s| u16::from_be_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(be32)
    }
    /// An unsigned number of 0, 4 or 8 bytes (0 bytes reads as 0).
    fn sized(&mut self, bytes: u8) -> Option<u64> {
        match bytes {
            0 => Some(0),
            4 => self.u32().map(u64::from),
            8 => self.take(8).map(be64),
            _ => None,
        }
    }
    fn four(&mut self) -> Option<[u8; 4]> {
        self.take(4).map(|s| [s[0], s[1], s[2], s[3]])
    }
    /// A string ending in a zero byte, or at the end of the data (some
    /// writers leave the last zero off).
    fn text(&mut self) -> Vec<u8> {
        let rest = &self.b[self.at.min(self.b.len())..];
        match rest.iter().position(|&c| c == 0) {
            Some(n) => {
                self.at += n + 1;
                rest[..n].to_vec()
            }
            None => {
                self.at = self.b.len();
                rest.to_vec()
            }
        }
    }
}

/// The item list (`iinf`, a full box): a count, then one `infe` box per
/// item. `abs` is where the body starts in the file, for the sentences.
fn parse_iinf(body: &[u8], abs: u64) -> Result<Vec<ItemInfo>, Stop> {
    let short = || Stop::Why("the item list (iinf) ends before its count of items".to_string());
    let mut r = Fields::new(body);
    let version = r.u8().ok_or_else(short)?;
    r.take(3).ok_or_else(short)?;
    let count = match version {
        0 => u64::from(r.u16().ok_or_else(short)?),
        1 => u64::from(r.u32().ok_or_else(short)?),
        v => return why(format!("the item list (iinf) has version {v}, which this strip does not read")),
    };

    let mut items = Vec::new();
    let end = body.len() as u64;
    let mut pos = r.at as u64;
    while pos < end {
        if items.len() >= MAX_ITEMS {
            return why(format!("the item list (iinf) holds more than {MAX_ITEMS} items, more than any real picture has"));
        }
        let at = pos as usize;
        let h = &body[at..(at + 16).min(body.len())];
        // Positions in the sentences are the file's, so the box is decoded
        // at its place in the file and its body found again in the table.
        let b = match decode_header(h, abs + pos, abs + end, "the item list (iinf)") {
            Header::Box(b) => b,
            Header::NoRoom => break,
            Header::Bad(w) => return why(w),
        };
        // Any other box in the list is not an item; it is stepped over.
        if &b.name == b"infe" {
            items.push(parse_infe(&body[(b.body_start - abs) as usize..(b.end - abs) as usize], b.start)?);
        }
        pos = b.end - abs;
    }
    if (items.len() as u64) < count {
        return why(format!("the item list (iinf) says it holds {count} items, but only {} could be read", items.len()));
    }
    let mut ids = HashSet::new();
    for item in &items {
        if !ids.insert(item.id) {
            return why(format!("the item list (iinf) names item {} twice, so what that item is cannot be told", item.id));
        }
    }
    Ok(items)
}

/// One item info entry (`infe`, a full box), versions 2 and 3: the item's
/// number (16 bits in version 2, 32 in version 3), a protection index, its
/// 4-letter type, its name, and for a `mime` item its content type.
fn parse_infe(body: &[u8], at: u64) -> Result<ItemInfo, Stop> {
    let short = || Stop::Why(format!("the item info entry at byte {at} ends before its item type"));
    let mut r = Fields::new(body);
    let version = r.u8().ok_or_else(short)?;
    r.take(3).ok_or_else(short)?;
    let id = match version {
        2 => u32::from(r.u16().ok_or_else(short)?),
        3 => r.u32().ok_or_else(short)?,
        v => return why(format!("the item info entry at byte {at} has version {v}; this strip reads versions 2 and 3")),
    };
    r.u16().ok_or_else(short)?; // the protection index
    let item_type = r.four().ok_or_else(short)?;
    let _name = r.text();
    let content_type = if &item_type == b"mime" { r.text() } else { Vec::new() };
    Ok(ItemInfo { id, item_type, content_type })
}

/// One item's entry in the item locations.
#[derive(Debug)]
struct Location {
    id: u32,
    method: u8,
    data_ref: u16,
    base: u64,
    /// (offset, length) pairs, as written.
    extents: Vec<(u64, u64)>,
}

/// The item locations (`iloc`, a full box), versions 0, 1 and 2.
fn parse_iloc(body: &[u8]) -> Result<Vec<Location>, Stop> {
    let mut r = Fields::new(body);
    let short = || Stop::Why("the item locations (iloc) end before their header does".to_string());
    let version = r.u8().ok_or_else(short)?;
    if version > 2 {
        return why(format!("the item locations (iloc) have version {version}, which this strip does not read"));
    }
    r.take(3).ok_or_else(short)?;
    let sizes = r.u8().ok_or_else(short)?;
    let (offset_size, length_size) = (sizes >> 4, sizes & 0x0F);
    let sizes = r.u8().ok_or_else(short)?;
    // The low half is the index size in versions 1 and 2, reserved in 0.
    let (base_size, index_size) = (sizes >> 4, if version >= 1 { sizes & 0x0F } else { 0 });
    for (what, n) in [("offset", offset_size), ("length", length_size), ("base offset", base_size), ("index", index_size)] {
        if ![0, 4, 8].contains(&n) {
            return why(format!("the item locations (iloc) give a {what} size of {n} bytes; only 0, 4 and 8 exist"));
        }
    }
    let count = (if version < 2 { r.u16().map(u32::from) } else { r.u32() }).ok_or_else(short)?;

    let mut out = Vec::new();
    let mut extents_read = 0usize;
    for k in 0..count {
        let cut = || Stop::Why(format!("the item locations (iloc) end part way through item {} of {count}", k + 1));
        if out.len() >= MAX_ITEMS {
            return why(format!("the item locations (iloc) list more than {MAX_ITEMS} items, more than any real picture has"));
        }
        let id = (if version < 2 { r.u16().map(u32::from) } else { r.u32() }).ok_or_else(cut)?;
        // 12 reserved bits, then the 4-bit construction method (versions 1 and 2).
        let method = if version >= 1 { (r.u16().ok_or_else(cut)? & 0x0F) as u8 } else { 0 };
        let data_ref = r.u16().ok_or_else(cut)?;
        let base = r.sized(base_size).ok_or_else(cut)?;
        let n = r.u16().ok_or_else(cut)?;
        let mut extents = Vec::with_capacity(usize::from(n).min(64));
        for _ in 0..n {
            extents_read += 1;
            if extents_read > MAX_EXTENTS {
                return why(format!("the item locations (iloc) list more than {MAX_EXTENTS} extents, more than any real picture has"));
            }
            // An index into the item's references, used by method 2 only.
            r.take(usize::from(index_size)).ok_or_else(cut)?;
            let offset = r.sized(offset_size).ok_or_else(cut)?;
            let length = r.sized(length_size).ok_or_else(cut)?;
            extents.push((offset, length));
        }
        out.push(Location { id, method, data_ref, base, extents });
    }
    Ok(out)
}

// ---------- choosing what to zero ----------

/// The metadata kind of an item, as reported: `Exif`, or `mime` for XMP.
fn metadata_type(item: &ItemInfo) -> Option<&'static str> {
    match &item.item_type {
        b"Exif" => Some("Exif"),
        b"mime" if is_xmp(&item.content_type) => Some("mime"),
        _ => None,
    }
}

/// True for XMP's content type, `application/rdf+xml`, in any letter case
/// and with any parameters after a `;` ignored.
fn is_xmp(content_type: &[u8]) -> bool {
    let text = String::from_utf8_lossy(content_type);
    text.split(';').next().unwrap_or("").trim().eq_ignore_ascii_case("application/rdf+xml")
}

/// From the two tables, the plans for every Exif and XMP item that can be
/// zeroed safely, and a sentence for each that cannot.
fn choose(items: &[ItemInfo], entries: &[Location], idat: Option<(u64, u64)>, mdats: &[(u64, u64)], len: u64) -> (Vec<Plan>, Vec<String>) {
    let metadata: BTreeMap<u32, &'static str> = items.iter().filter_map(|i| metadata_type(i).map(|t| (i.id, t))).collect();

    // The bytes of every other item, which must never be written. A length
    // of 0 means "to the end of the data", so it guards everything after.
    // Whatever its data reference (R3-4, 2026-10-11): 0 is this file, and so
    // is any entry of `dinf/dref` that is a self-contained `url ` (flag 1).
    // The `dref` table is not read, so every item is taken to be in this
    // file; one that is really elsewhere only guards bytes it does not have,
    // which can refuse a photo but never damage one.
    let mut guarded = Vec::new();
    for e in entries.iter().filter(|e| !metadata.contains_key(&e.id)) {
        let (area_start, area_end) = match (e.method, idat) {
            (0, _) => (0, len),
            (1, Some(area)) => area,
            _ => continue, // built from other items, or no idat: no bytes of its own
        };
        for &(offset, length) in &e.extents {
            let Some(start) = e.base.checked_add(offset).and_then(|rel| area_start.checked_add(rel)) else { continue };
            if start >= area_end {
                continue;
            }
            let end = if length == 0 { area_end } else { start.saturating_add(length).min(area_end) };
            guarded.push((start, end));
        }
    }
    let guarded = merged(guarded);

    let mut plans = Vec::new();
    let mut skipped = Vec::new();
    for (&id, &item_type) in &metadata {
        let mut extents = Vec::new();
        let mut problem = None;
        for e in entries.iter().filter(|e| e.id == id) {
            for &(offset, length) in &e.extents {
                match locate(e, offset, length, idat, mdats, len, &guarded) {
                    Ok(range) => extents.push(range),
                    Err(why) => {
                        problem = Some(why);
                        break;
                    }
                }
            }
            if problem.is_some() {
                break;
            }
        }
        match problem {
            // The whole item is left as it is: a part of it cannot be found
            // with certainty, so none of it is written.
            Some(why) => skipped.push(format!("item {id} ({item_type}) {why}, so it was left as it is")),
            None if !extents.is_empty() => plans.push(Plan { item_id: id, item_type, extents }),
            None => {} // listed, but with no bytes anywhere
        }
    }
    (plans, skipped)
}

/// Where one extent of an Exif or XMP item lies in the file, or why it
/// cannot be zeroed safely.
fn locate(
    e: &Location,
    offset: u64,
    length: u64,
    idat: Option<(u64, u64)>,
    mdats: &[(u64, u64)],
    len: u64,
    guarded: &[(u64, u64)],
) -> Result<(u64, u64), String> {
    if e.data_ref != 0 {
        return Err(format!("is kept through a data reference ({}), which this strip does not follow", e.data_ref));
    }
    let area = match e.method {
        0 => None,
        1 => Some(idat.ok_or("is kept in an idat box that the meta box does not have")?),
        2 => return Err("is built from parts of other items (construction method 2), which this strip does not handle".to_string()),
        m => return Err(format!("uses construction method {m}, which does not exist")),
    };
    if length == 0 {
        return Err("has an extent of length 0 (\"to the end of the data\"), which this strip does not guess at".to_string());
    }
    let too_big = || "has an extent whose start is too large to add up".to_string();
    let rel = e.base.checked_add(offset).ok_or_else(too_big)?;
    let start = match area {
        Some((idat_start, _)) => idat_start.checked_add(rel).ok_or_else(too_big)?,
        None => rel,
    };
    let end = start.checked_add(length).ok_or_else(too_big)?;
    match area {
        Some((_, idat_end)) => {
            if end > idat_end {
                return Err(format!("has an extent (bytes {start} to {end}) that runs past the end of its idat box"));
            }
        }
        None => {
            if end > len {
                return Err(format!("has an extent (bytes {start} to {end}) that runs past the end of the file ({len} bytes)"));
            }
            // Only inside an mdat box's body: never a box header, never the
            // tables, never anything the box structure needs.
            let i = mdats.partition_point(|m| m.0 <= start);
            if i == 0 || end > mdats[i - 1].1 {
                return Err(format!("has an extent (bytes {start} to {end}) outside the file's media data (mdat)"));
            }
        }
    }
    // Never a byte of any other item.
    let i = guarded.partition_point(|g| g.1 <= start);
    if i < guarded.len() && guarded[i].0 < end {
        return Err(format!("has an extent (bytes {start} to {end}) that overlaps another item's bytes"));
    }
    Ok((start, end))
}

/// Ranges sorted and joined where they touch or overlap.
fn merged(mut ranges: Vec<(u64, u64)>) -> Vec<(u64, u64)> {
    ranges.sort_unstable();
    let mut out: Vec<(u64, u64)> = Vec::with_capacity(ranges.len());
    for (s, e) in ranges {
        match out.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

fn be32(b: &[u8]) -> u32 {
    u32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

fn be64(b: &[u8]) -> u64 {
    u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
}

/// A box name made printable for a message.
fn show_name(name: &[u8; 4]) -> String {
    name.iter().map(|&c| if c.is_ascii_graphic() || c == b' ' { c as char } else { '?' }).collect()
}

/// Test pictures built byte by byte, shared with the upload rules' tests
/// (relay/upload_rules_tests.rs). No binary fixtures.
#[cfg(test)]
pub(crate) mod build {
    use std::ops::Range;

    /// Where the test pictures were "taken", as an iPhone's EXIF and XMP
    /// would carry it (in text, so a test can look for it).
    pub(crate) const PLACE: &[u8] = b"GPSLatitude=47.6553N GPSLongitude=122.6870W";

    /// One box with an ordinary 32-bit size.
    pub(crate) fn bx(name: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut v = ((payload.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(name);
        v.extend_from_slice(payload);
        v
    }

    /// One full box: a version byte and three flag bytes before the payload.
    pub(crate) fn full(name: &[u8; 4], version: u8, payload: &[u8]) -> Vec<u8> {
        bx(name, &[&[version, 0, 0, 0][..], payload].concat())
    }

    /// Bytes that are never ASCII letters, so no box name or test text can
    /// appear inside them by accident.
    pub(crate) fn filler(n: usize, seed: usize) -> Vec<u8> {
        (0..n).map(|i| 0x80 | ((i * 7 + seed * 13) % 128) as u8).collect()
    }

    pub(crate) fn ftyp_heic() -> Vec<u8> {
        bx(b"ftyp", b"heic\0\0\0\0mif1heicmiaf")
    }

    pub(crate) fn ftyp_avif() -> Vec<u8> {
        bx(b"ftyp", b"avif\0\0\0\0avifmif1miaf")
    }

    /// What an item is.
    #[derive(Debug, Clone, PartialEq)]
    pub(crate) enum Kind {
        /// A picture item of this 4-letter type (`hvc1`, `av01`, `grid`).
        Picture(&'static [u8; 4]),
        Exif,
        /// A `mime` item with this content type.
        Mime(&'static str),
    }

    /// Where an item's bytes go.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub(crate) enum Place {
        /// In the `mdat` box (construction method 0).
        Mdat,
        /// In the `idat` box inside `meta` (construction method 1).
        Idat,
        /// Construction method 2, pointing at nothing real.
        Method2,
    }

    #[derive(Debug, Clone)]
    pub(crate) struct Item {
        pub id: u32,
        pub kind: Kind,
        pub data: Vec<u8>,
        pub place: Place,
    }

    /// How the tables are written.
    #[derive(Debug, Clone, Copy)]
    pub(crate) struct Tables {
        pub iloc_version: u8,
        pub infe_version: u8,
        pub offset_size: u8,
        pub length_size: u8,
        pub base_size: u8,
        pub index_size: u8,
        /// Split each item's bytes into this many extents.
        pub extents_per_item: usize,
        /// Add this (wrapping) to the offset of every extent of every Exif
        /// and `mime` item in the table, to point it past the end, before
        /// where it is, or out of 64 bits: 0 for a true file.
        pub skew: u64,
    }

    impl Default for Tables {
        fn default() -> Self {
            Tables { iloc_version: 1, infe_version: 2, offset_size: 4, length_size: 4, base_size: 0, index_size: 0, extents_per_item: 1, skew: 0 }
        }
    }

    /// A built picture and where each item's bytes ended up.
    pub(crate) struct Built {
        pub bytes: Vec<u8>,
        /// (item id, its byte ranges in the file).
        pub ranges: Vec<(u32, Vec<Range<usize>>)>,
    }

    impl Built {
        pub(crate) fn ranges_of(&self, id: u32) -> &[Range<usize>] {
            &self.ranges.iter().find(|(i, _)| *i == id).expect("test setup: item present").1
        }
    }

    fn sized(n: u8, v: u64) -> Vec<u8> {
        match n {
            0 => Vec::new(),
            4 => (v as u32).to_be_bytes().to_vec(),
            8 => v.to_be_bytes().to_vec(),
            _ => panic!("test setup: size {n}"),
        }
    }

    fn infe(t: &Tables, item: &Item) -> Vec<u8> {
        let mut p = if t.infe_version == 2 { (item.id as u16).to_be_bytes().to_vec() } else { item.id.to_be_bytes().to_vec() };
        p.extend_from_slice(&[0, 0]); // protection index
        match &item.kind {
            Kind::Picture(ty) => {
                p.extend_from_slice(&ty[..]);
                p.push(0);
            }
            Kind::Exif => {
                p.extend_from_slice(b"Exif");
                p.push(0);
            }
            Kind::Mime(ct) => {
                p.extend_from_slice(b"mime");
                p.push(0);
                p.extend_from_slice(ct.as_bytes());
                p.push(0);
            }
        }
        full(b"infe", t.infe_version, &p)
    }

    /// The `meta` box, given where each item's bytes start (`starts`, as
    /// file offsets for mdat items and idat offsets for idat items).
    fn meta(t: &Tables, items: &[Item], starts: &[u64], idat: &[u8]) -> Vec<u8> {
        let hdlr = full(b"hdlr", 0, &[&[0u8; 4][..], b"pict", &[0u8; 12], b"\0"].concat());
        let pitm = full(b"pitm", 0, &(items[0].id as u16).to_be_bytes());
        let mut list = Vec::new();
        list.extend_from_slice(&(items.len() as u16).to_be_bytes());
        for item in items {
            list.extend(infe(t, item));
        }
        let iinf = full(b"iinf", 0, &list);

        let mut loc = vec![(t.offset_size << 4) | t.length_size, (t.base_size << 4) | if t.iloc_version >= 1 { t.index_size } else { 0 }];
        if t.iloc_version < 2 {
            loc.extend_from_slice(&(items.len() as u16).to_be_bytes());
        } else {
            loc.extend_from_slice(&(items.len() as u32).to_be_bytes());
        }
        for (item, &start) in items.iter().zip(starts) {
            if t.iloc_version < 2 {
                loc.extend_from_slice(&(item.id as u16).to_be_bytes());
            } else {
                loc.extend_from_slice(&item.id.to_be_bytes());
            }
            if t.iloc_version >= 1 {
                let method: u16 = match item.place {
                    Place::Mdat => 0,
                    Place::Idat => 1,
                    Place::Method2 => 2,
                };
                loc.extend_from_slice(&method.to_be_bytes());
            }
            loc.extend_from_slice(&[0, 0]); // data reference index: this file
            let base = if t.base_size > 0 { start } else { 0 };
            loc.extend(sized(t.base_size, base));
            let pieces = split(item.data.len(), t.extents_per_item);
            loc.extend_from_slice(&(pieces.len() as u16).to_be_bytes());
            // Only the metadata items are moved by `skew`: the picture stays where it is.
            let skew = if matches!(item.kind, Kind::Picture(_)) { 0 } else { t.skew };
            for piece in pieces {
                loc.extend(sized(t.index_size, 1));
                let at = if t.base_size > 0 { piece.start as u64 } else { start + piece.start as u64 };
                loc.extend(sized(t.offset_size, at.wrapping_add(skew)));
                loc.extend(sized(t.length_size, piece.len() as u64));
            }
        }
        let iloc = full(b"iloc", t.iloc_version, &loc);
        // Item properties and references, which must come through untouched.
        let iprp = bx(b"iprp", &[bx(b"ipco", &[bx(b"hvcC", &filler(23, 3)), full(b"ispe", 0, &[0, 0, 0x0F, 0xC0, 0, 0, 0x0B, 0xD0])].concat()), full(b"ipma", 0, &[0, 0, 0, 1, 0, 1, 2, 0x81, 0x82])].concat());
        let iref = full(b"iref", 0, &bx(b"cdsc", &[0, 3, 0, 1, 0, 1]));
        let mut kids = vec![hdlr, pitm, iinf, iloc, iref, iprp];
        if !idat.is_empty() {
            kids.push(bx(b"idat", idat));
        }
        full(b"meta", 0, &kids.concat())
    }

    /// `n` bytes split into `parts` ranges (fewer when there are fewer bytes).
    fn split(n: usize, parts: usize) -> Vec<Range<usize>> {
        let parts = parts.max(1).min(n.max(1));
        let step = n.div_ceil(parts).max(1);
        (0..n).step_by(step).map(|s| s..(s + step).min(n)).collect()
    }

    /// A HEIF picture: `ftyp`, then `meta` with the tables (and an `idat`
    /// for the idat items), then `mdat` with the mdat items' bytes, each
    /// after a little filler.
    pub(crate) fn heif(ftyp: Vec<u8>, t: &Tables, items: &[Item]) -> Built {
        // The meta box's size does not depend on the offsets in it, so it is
        // built once with zeros to measure, then again with the real ones.
        let mut idat = Vec::new();
        let mut idat_starts = Vec::new();
        for item in items {
            if item.place == Place::Idat {
                idat.extend(filler(5, item.id as usize));
                idat_starts.push(Some(idat.len() as u64));
                idat.extend_from_slice(&item.data);
            } else {
                idat_starts.push(None);
            }
        }
        let zeros = vec![0u64; items.len()];
        let meta_len = meta(t, items, &zeros, &idat).len();
        let mdat_body = (ftyp.len() + meta_len + 8) as u64;
        let mut mdat = Vec::new();
        let mut starts = Vec::new();
        for (item, idat_start) in items.iter().zip(&idat_starts) {
            match idat_start {
                Some(s) => starts.push(*s),
                None => {
                    mdat.extend(filler(7, item.id as usize));
                    starts.push(mdat_body + mdat.len() as u64);
                    if item.place == Place::Mdat {
                        mdat.extend_from_slice(&item.data);
                    }
                }
            }
        }
        mdat.extend(filler(9, 99));
        let meta = meta(t, items, &starts, &idat);
        assert_eq!(meta.len(), meta_len, "test setup: the meta box keeps its size");
        let bytes = [ftyp.clone(), meta.clone(), bx(b"mdat", &mdat)].concat();

        // Where each item's bytes are in the file.
        let idat_body = bytes.len() - mdat.len() - 8 - idat.len();
        let mut ranges = Vec::new();
        for (item, &start) in items.iter().zip(&starts) {
            let base = match item.place {
                Place::Mdat => start as usize,
                Place::Idat => idat_body + start as usize,
                Place::Method2 => continue,
            };
            let pieces = split(item.data.len(), t.extents_per_item).into_iter().map(|p| base + p.start..base + p.end).collect();
            ranges.push((item.id, pieces));
        }
        Built { bytes, ranges }
    }

    /// The EXIF block a phone writes, with where it was taken in it.
    pub(crate) fn exif_bytes() -> Vec<u8> {
        [&[0u8, 0, 0, 6][..], b"Exif\0\0MM\0*", b" Apple iPhone 15 ", PLACE, b" 2026:10:11 09:41:00"].concat()
    }

    /// XMP as an editing app writes it, with where it was taken in it.
    pub(crate) fn xmp_bytes() -> Vec<u8> {
        [&b"<x:xmpmeta xmlns:x='adobe:ns:meta/'><rdf:RDF><rdf:Description exif:"[..], PLACE, b"/></rdf:RDF></x:xmpmeta>"].concat()
    }

    /// An iPhone-style HEIC: the picture (item 1), its thumbnail (item 2),
    /// the EXIF with the place (item 3), all in mdat.
    pub(crate) fn iphone_heic() -> Built {
        heif(
            ftyp_heic(),
            &Tables::default(),
            &[
                Item { id: 1, kind: Kind::Picture(b"hvc1"), data: filler(300, 1), place: Place::Mdat },
                Item { id: 2, kind: Kind::Picture(b"hvc1"), data: filler(60, 2), place: Place::Mdat },
                Item { id: 3, kind: Kind::Exif, data: exif_bytes(), place: Place::Mdat },
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    //! Every file here is built byte by byte (`build` above); there are no
    //! binary fixtures. Each test was seen failing once against a
    //! deliberately broken copy of the code (then the code was restored);
    //! its doc comment says what that red run showed.

    use super::build::*;
    use super::*;
    use crate::test_temp::TempPath;

    // ---------- running the strip ----------

    /// Strip a copy of `bytes` written to a real file, through the public
    /// path API, and read the result back.
    fn strip_file(bytes: &[u8]) -> (Vec<u8>, HeifStrip) {
        let path: TempPath = crate::test_temp::file("strip_heif", "heic");
        std::fs::write(&path, bytes).unwrap();
        let report = strip_heif_metadata_in_place(&path).expect("strip must not fail on a readable file");
        (std::fs::read(&path).unwrap(), report)
    }

    /// Strip a copy of `bytes` in memory.
    fn strip_mem(bytes: &[u8]) -> (Vec<u8>, HeifStrip) {
        let mut copy = bytes.to_vec();
        let report = strip_heif_metadata_in_buffer(&mut copy).expect("strip must not fail in memory");
        (copy, report)
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    /// The file is the same size, every reported extent holds only zeros,
    /// the byte counts add up, and no byte outside a reported extent changed.
    fn assert_only_reported_extents_changed(before: &[u8], after: &[u8], r: &HeifStrip) {
        assert_eq!(before.len(), after.len(), "the size must never change");
        let mut sum = 0;
        for &(s, e) in &r.extents {
            assert!(s < e && e as usize <= after.len(), "extent {s}..{e} reported outside the file");
            assert!(after[s as usize..e as usize].iter().all(|&b| b == 0), "extent {s}..{e} must hold only zeros");
            sum += e - s;
        }
        assert_eq!(sum, r.bytes_zeroed(), "the byte counts must add up");
        for i in 0..before.len() {
            if before[i] != after[i] {
                let inside = r.extents.iter().any(|&(s, e)| s as usize <= i && i < e as usize);
                assert!(inside, "byte {i} changed outside every reported extent");
            }
        }
    }

    /// `before` with exactly `ranges` zeroed.
    fn zeroed(before: &[u8], ranges: &[std::ops::Range<usize>]) -> Vec<u8> {
        let mut want = before.to_vec();
        for r in ranges {
            want[r.clone()].fill(0);
        }
        want
    }

    fn picture(id: u32, n: usize) -> Item {
        Item { id, kind: Kind::Picture(b"hvc1"), data: filler(n, id as usize), place: Place::Mdat }
    }

    fn exif(id: u32, place: Place) -> Item {
        Item { id, kind: Kind::Exif, data: exif_bytes(), place }
    }

    fn xmp(id: u32, place: Place) -> Item {
        Item { id, kind: Kind::Mime("application/rdf+xml"), data: xmp_bytes(), place }
    }

    /// An iPhone-style HEIC: its Exif item (with the place, the camera and
    /// the time) is zeroed, and not one other byte changes.
    ///
    /// Seen red 2026-10-11 with `metadata_type` matching `EXIF` instead of
    /// `Exif`: "the place is gone from the file".
    #[test]
    fn an_exif_item_with_gps_is_zeroed() {
        let b = iphone_heic();
        assert!(contains(&b.bytes, PLACE), "test setup: the place is in the file");
        let (after, r) = strip_file(&b.bytes);
        assert!(!contains(&after, PLACE), "the place is gone from the file");
        assert!(!contains(&after, b"iPhone 15"), "and the camera with it");
        assert_eq!(r.kind, HeifKind::Still);
        assert_eq!(r.items_zeroed, vec![(3, "Exif".to_string(), exif_bytes().len() as u64)]);
        assert!(r.complete(), "{:?}", r.stopped_early);
        assert_eq!(after, zeroed(&b.bytes, b.ranges_of(3)), "exactly the Exif item's bytes, and nothing else");
        // In memory, the same.
        assert_eq!(strip_mem(&b.bytes), (after, r));
    }

    /// XMP (a `mime` item of type `application/rdf+xml`, in any letter case
    /// and with parameters) is zeroed; a `mime` item of another type stays.
    ///
    /// Seen red 2026-10-11 with `is_xmp` comparing against
    /// "application/xmp+xml": "the XMP place is gone".
    #[test]
    fn an_xmp_mime_item_is_zeroed_and_another_mime_item_is_not() {
        let items = [
            picture(1, 200),
            xmp(5, Place::Mdat),
            Item { id: 6, kind: Kind::Mime("text/plain"), data: b"a caption the picture needs".to_vec(), place: Place::Mdat },
            Item { id: 7, kind: Kind::Mime("Application/RDF+XML; charset=utf-8"), data: xmp_bytes(), place: Place::Mdat },
        ];
        let b = heif(ftyp_heic(), &Tables::default(), &items);
        let (after, r) = strip_file(&b.bytes);
        assert!(!contains(&after, PLACE), "the XMP place is gone");
        assert!(contains(&after, b"a caption the picture needs"), "a mime item that is not XMP stays");
        let ids: Vec<(u32, &str)> = r.items_zeroed.iter().map(|(i, t, _)| (*i, t.as_str())).collect();
        assert_eq!(ids, vec![(5, "mime"), (7, "mime")]);
        let want = [b.ranges_of(5), b.ranges_of(7)].concat();
        assert_eq!(after, zeroed(&b.bytes, &want));
        assert!(r.complete());
    }

    /// The picture, its thumbnail, a grid item in idat, the item properties
    /// and references, and every other byte are identical before and after;
    /// only the Exif and XMP items' bytes (three extents each) are zeros.
    ///
    /// Seen red 2026-10-11 with each extent written one byte too long
    /// (`write_zeros(f, start, end + 1)`): "every byte outside the Exif and
    /// XMP items is identical".
    #[test]
    fn the_picture_and_every_other_byte_are_identical() {
        let items = [
            picture(1, 500),
            picture(2, 80), // the thumbnail
            exif(3, Place::Mdat),
            Item { id: 4, kind: Kind::Picture(b"grid"), data: vec![0, 0, 1, 1, 0x0F, 0xC0, 0x0B, 0xD0], place: Place::Idat },
            xmp(5, Place::Mdat),
        ];
        let tables = Tables { extents_per_item: 3, ..Tables::default() };
        let b = heif(ftyp_heic(), &tables, &items);
        let (after, r) = strip_file(&b.bytes);
        let want = [b.ranges_of(3), b.ranges_of(5)].concat();
        assert_eq!(after, zeroed(&b.bytes, &want), "every byte outside the Exif and XMP items is identical");
        for id in [1, 2, 4] {
            for range in b.ranges_of(id) {
                assert_eq!(after[range.clone()], b.bytes[range.clone()], "item {id}'s bytes are untouched");
            }
        }
        assert_only_reported_extents_changed(&b.bytes, &after, &r);
        assert_eq!(r.extents.len(), 6, "three extents each");
        assert!(r.complete());
        // A second run finds the same items, already zero, and changes no byte.
        let (again, _) = strip_file(&after);
        assert_eq!(again, after);
    }

    /// Every iloc version (0, 1, 2) with offset and length sizes 4 and 8,
    /// base offset sizes 0, 4 and 8, index sizes 0, 4 and 8 (versions 1 and
    /// 2), and infe versions 2 and 3: the Exif is zeroed exactly, each time.
    ///
    /// Seen red 2026-10-11 with the index field never skipped: "iloc
    /// version 1, offset 4, length 4, base 0, index 4, infe 2: [\"the item
    /// locations (iloc) end part way through item 2 of 3\"]".
    #[test]
    fn iloc_versions_0_to_2_and_their_field_sizes() {
        let mut tried = 0;
        for iloc_version in 0..=2u8 {
            for offset_size in [4u8, 8] {
                for length_size in [4u8, 8] {
                    for base_size in [0u8, 4, 8] {
                        for index_size in if iloc_version == 0 { vec![0u8] } else { vec![0, 4, 8] } {
                            for infe_version in [2u8, 3] {
                                let tables = Tables {
                                    iloc_version,
                                    infe_version,
                                    offset_size,
                                    length_size,
                                    base_size,
                                    index_size,
                                    extents_per_item: 2,
                                    skew: 0,
                                };
                                let items = [picture(1, 120), exif(3, Place::Mdat), picture(2, 40)];
                                let b = heif(ftyp_avif(), &tables, &items);
                                let (after, r) = strip_mem(&b.bytes);
                                let what = format!(
                                    "iloc version {iloc_version}, offset {offset_size}, length {length_size}, base {base_size}, index {index_size}, infe {infe_version}"
                                );
                                assert!(r.complete(), "{what}: {:?}", r.stopped_early);
                                assert_eq!(after, zeroed(&b.bytes, b.ranges_of(3)), "{what}");
                                assert!(!contains(&after, PLACE), "{what}");
                                tried += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(tried, 2 * 2 * 2 * 3 * (1 + 3 + 3));
        // A 32-bit item number in infe version 3 and iloc version 2.
        let tables = Tables { iloc_version: 2, infe_version: 3, ..Tables::default() };
        let b = heif(ftyp_heic(), &tables, &[picture(1, 50), exif(0x0001_0005, Place::Mdat)]);
        let (after, r) = strip_mem(&b.bytes);
        assert_eq!(r.items_zeroed[0].0, 0x0001_0005);
        assert_eq!(after, zeroed(&b.bytes, b.ranges_of(0x0001_0005)));
    }

    /// Construction method 1: an Exif and an XMP kept in `idat` are zeroed
    /// there, with and without a base offset, and the grid item beside them
    /// in the same `idat` is not touched.
    ///
    /// Seen red 2026-10-11 with method 1 counted from the start of the file
    /// (the idat start not added): "the Exif in idat is zeroed".
    #[test]
    fn construction_method_1_zeroes_inside_idat() {
        let items = [
            picture(1, 300),
            Item { id: 2, kind: Kind::Picture(b"grid"), data: vec![0, 0, 1, 1, 0x0F, 0xC0, 0x0B, 0xD0], place: Place::Idat },
            exif(3, Place::Idat),
            xmp(4, Place::Idat),
        ];
        for base_size in [0u8, 4] {
            let tables = Tables { base_size, extents_per_item: 2, ..Tables::default() };
            let b = heif(ftyp_heic(), &tables, &items);
            let (after, r) = strip_file(&b.bytes);
            assert!(r.complete(), "{:?}", r.stopped_early);
            assert!(!contains(&after, PLACE), "the Exif in idat is zeroed");
            let want = [b.ranges_of(3), b.ranges_of(4)].concat();
            assert_eq!(after, zeroed(&b.bytes, &want), "only the Exif and XMP, never the grid beside them");
        }
    }

    /// What is not handled is reported and nothing is written: an Exif built
    /// from other items (construction method 2), one kept through a data
    /// reference (another file, as far as the table says), and an extent of
    /// length 0 ("to the end of the data").
    ///
    /// Seen red 2026-10-11 with the data reference check taken out: "nothing
    /// is written for an Exif kept through a data reference".
    #[test]
    fn method_2_a_data_reference_and_a_zero_length_are_reported_not_handled() {
        // Method 2: the Exif item is built from other items.
        let items = [picture(1, 100), Item { id: 3, kind: Kind::Exif, data: exif_bytes(), place: Place::Method2 }];
        let b = heif(ftyp_heic(), &Tables::default(), &items);
        let (after, r) = strip_file(&b.bytes);
        assert_eq!(after, b.bytes, "nothing is written for a method-2 Exif");
        assert!(!r.complete());
        assert_eq!(r.stopped_early.len(), 1, "{:?}", r.stopped_early);
        assert!(r.stopped_early[0].contains("item 3 (Exif)") && r.stopped_early[0].contains("construction method 2"), "{}", r.stopped_early[0]);

        // A data reference: the Exif named through another file entry.
        let b = iphone_heic();
        let mut bytes = b.bytes.clone();
        let entry = find_iloc_entry(&bytes, 3);
        bytes[entry + 4..entry + 6].copy_from_slice(&1u16.to_be_bytes()); // after id (2) and method (2)
        let (after, r) = strip_file(&bytes);
        assert_eq!(after, bytes, "nothing is written for an Exif kept through a data reference");
        assert!(r.stopped_early[0].contains("data reference"), "{:?}", r.stopped_early);

        // A length of 0: "to the end of the data".
        let mut bytes = b.bytes.clone();
        let entry = find_iloc_entry(&bytes, 3);
        bytes[entry + 12..entry + 16].copy_from_slice(&0u32.to_be_bytes()); // the extent's length
        let (after, r) = strip_file(&bytes);
        assert_eq!(after, bytes);
        assert!(r.stopped_early[0].contains("length 0"), "{:?}", r.stopped_early);
    }

    /// R3-4 (2026-10-11). AN ITEM KEPT THROUGH A DATA REFERENCE IS STILL
    /// PROTECTED: a self-contained `url ` entry (flag 1) in `dinf/dref`
    /// means its bytes are in THIS file, so an Exif extent over them is
    /// refused, not zeroed. Here the picture (item 1) names data reference 1
    /// and the Exif's extent is pointed into the picture's bytes: the
    /// picture is untouched, the photo is not called clean, and the report
    /// says the Exif overlaps another item.
    ///
    /// Seen red 2026-10-11 with the guard built only from items whose data
    /// reference is 0, as merged: "the picture's bytes are untouched".
    #[test]
    fn an_item_kept_through_a_data_reference_is_still_protected() {
        let b = iphone_heic();
        let mut bytes = b.bytes.clone();
        let pic = find_iloc_entry(&bytes, 1);
        bytes[pic + 4..pic + 6].copy_from_slice(&1u16.to_be_bytes()); // after id (2) and method (2)
        let exif = find_iloc_entry(&bytes, 3);
        let into_picture = b.ranges_of(1)[0].start as u32 + 10;
        bytes[exif + 8..exif + 12].copy_from_slice(&into_picture.to_be_bytes()); // the extent's offset
        let (after, r) = strip_file(&bytes);
        let picture = b.ranges_of(1)[0].clone();
        assert_eq!(after[picture.clone()], bytes[picture], "the picture's bytes are untouched");
        assert!(!r.complete(), "{:?}", r.stopped_early);
        assert!(
            r.stopped_early.iter().any(|w| w.contains("item 3 (Exif)") && w.contains("overlaps another item")),
            "{:?}",
            r.stopped_early
        );
    }

    /// Where item `id`'s entry starts in the iloc of a file built with the
    /// default tables (version 1, sizes 4/4/0/0, one extent): id (2),
    /// method (2), data reference (2), extent count (2), offset (4),
    /// length (4).
    fn find_iloc_entry(bytes: &[u8], id: u16) -> usize {
        let iloc = bytes.windows(4).position(|w| w == b"iloc").unwrap() + 4;
        let mut at = iloc + 4 + 2 + 2; // version and flags, sizes, count
        loop {
            if u16::from_be_bytes([bytes[at], bytes[at + 1]]) == id {
                return at;
            }
            at += 16;
        }
    }

    /// An Exif extent the table puts past the end of the file, out of 64
    /// bits, over another item's bytes, or inside the meta box (outside the
    /// media data): nothing is written, the photo is not called clean, and
    /// the report says which.
    ///
    /// Seen red 2026-10-11 with the "past the end of the file" and "inside
    /// an mdat box" checks taken out of `locate`: first "assertion failed:
    /// !r.complete() && !r.changed()" (the second lock in `apply` kept the
    /// write inside the file, but the report called the photo clean with its
    /// Exif still in it, which is why `apply` now reports a refused extent);
    /// then, with that, "runs past the end of the file: item 3 (Exif) has an
    /// extent (bytes 10427 to 10521) outside the file, which a second check
    /// refused to write, so it was left as it is".
    #[test]
    fn an_iloc_pointing_past_the_end_writes_nothing_out_of_bounds() {
        let items = [picture(1, 100), exif(3, Place::Mdat)];
        let cases = [
            // 10,000 bytes past where it should be: past the end of the file.
            (Tables { skew: 10_000, ..Tables::default() }, "runs past the end of the file"),
            (Tables { offset_size: 8, length_size: 8, skew: 1 << 40, ..Tables::default() }, "runs past the end of the file"),
            // A base and an offset that cannot be added in 64 bits.
            (Tables { base_size: 8, offset_size: 8, skew: u64::MAX - 3, ..Tables::default() }, "too large to add up"),
            // 21 bytes early: over the end of the picture before it.
            (Tables { offset_size: 8, skew: 0u64.wrapping_sub(21), ..Tables::default() }, "overlaps another item's bytes"),
            // 150 bytes early: inside the meta box, where the tables are.
            (Tables { offset_size: 8, skew: 0u64.wrapping_sub(150), ..Tables::default() }, "outside the file's media data"),
        ];
        for (tables, words) in cases {
            let b = heif(ftyp_heic(), &tables, &items);
            let (after, r) = strip_mem(&b.bytes);
            assert_eq!(after, b.bytes, "nothing is written ({words})");
            assert!(!r.complete() && !r.changed());
            assert_eq!(r.stopped_early.len(), 1, "{:?}", r.stopped_early);
            assert!(r.stopped_early[0].starts_with("item 3 (Exif) "), "{}", r.stopped_early[0]);
            assert!(r.stopped_early[0].contains(words), "{words}: {}", r.stopped_early[0]);
            let (after_file, r_file) = strip_file(&b.bytes);
            assert_eq!((after_file, r_file), (after, r), "file and memory agree ({words})");
        }
    }

    /// Every cut of a whole picture, from 0 bytes to all of them: no panic,
    /// no error, the size unchanged, nothing written outside a reported
    /// extent, and a cut file never called clean.
    ///
    /// Seen red 2026-10-11 with the `size > room` check taken out of
    /// `decode_header`: "strip must not fail in memory: Error { kind:
    /// UnexpectedEof, message: \"failed to fill whole buffer\" }".
    #[test]
    fn truncated_files_never_panic_and_never_write_out_of_bounds() {
        let full = iphone_heic().bytes;
        for cut in 0..=full.len() {
            let before = &full[..cut];
            let (after, r) = strip_mem(before);
            assert_only_reported_extents_changed(before, &after, &r);
            if cut < full.len() {
                assert!(!r.complete(), "a cut file is never proven clean (cut {cut})");
            }
            if cut % 41 == 0 {
                let (after_file, r_file) = strip_file(before);
                assert_eq!((after_file, r_file), (after, r), "file and memory agree at cut {cut}");
            }
        }
        assert!(strip_mem(&full).1.complete());
    }

    /// A read-only copy of `bytes`, stripped: the result, the file after,
    /// and whether it was readable throughout.
    fn strip_read_only(bytes: &[u8]) -> (io::Result<HeifStrip>, Vec<u8>) {
        let path: TempPath = crate::test_temp::file("strip_heif_ro", "bin");
        std::fs::write(&path, bytes).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms.clone()).unwrap();
        let result = strip_heif_metadata_in_place(&path);
        let after = std::fs::read(&path).unwrap();
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
        (result, after)
    }

    /// An MP4 video, a HEIF file with a movie in it, a JPEG, and a HEIC with
    /// nothing to zero, all read-only: each is left as it is and never
    /// opened for writing (which on a read-only file would fail).
    ///
    /// Seen red 2026-10-11 with the `moov` check taken out of `plan`: "never
    /// opened for writing: Os { code: 5, kind: PermissionDenied, message:
    /// \"Access is denied.\" }".
    #[test]
    fn an_mp4_video_and_a_jpeg_are_left_untouched() {
        // A phone video: ftyp, moov with a location, mdat.
        let video = [bx(b"ftyp", b"isom\0\0\x02\0isomiso2mp41"), bx(b"moov", &bx(b"udta", &bx(b"\xA9xyz", b"+47.6553-122.6870/"))), bx(b"mdat", &filler(400, 1))].concat();
        let (r, after) = strip_read_only(&video);
        assert_eq!(r.expect("never opened for writing").kind, HeifKind::NotHeif);
        assert_eq!(after, video);

        // A HEIF picture WITH a movie (an image sequence): left to the video strip.
        let b = iphone_heic();
        let ftyp_len = ftyp_heic().len();
        let with_movie = [b.bytes[..ftyp_len].to_vec(), bx(b"moov", &bx(b"mvhd", &filler(100, 2))), b.bytes[ftyp_len..].to_vec()].concat();
        let (r, after) = strip_read_only(&with_movie);
        assert_eq!(r.expect("never opened for writing").kind, HeifKind::WithMovie);
        assert_eq!(after, with_movie);

        // A JPEG with its EXIF.
        let jpeg = [&[0xFF, 0xD8, 0xFF, 0xE1, 0x00, 0x30][..], b"Exif\0\0", PLACE, &[0xFF, 0xD9]].concat();
        let (r, after) = strip_read_only(&jpeg);
        assert_eq!(r.expect("never opened for writing").kind, HeifKind::NotHeif);
        assert_eq!(after, jpeg);

        // A HEIC with nothing to zero is not opened for writing either.
        let plain = heif(ftyp_heic(), &Tables::default(), &[picture(1, 100)]).bytes;
        let (r, after) = strip_read_only(&plain);
        let r = r.expect("nothing to zero: never opened for writing");
        assert!(r.complete() && !r.changed());
        assert_eq!(after, plain);
    }

    /// `is_heif_still`: every HEIF brand, as the major brand or a compatible
    /// one, a 64-bit ftyp size, and no movie in view.
    ///
    /// Seen red 2026-10-11 with the compatible brands not read: "mif1 as a
    /// compatible brand is enough".
    #[test]
    fn is_heif_still_reads_the_brands_and_looks_for_a_movie() {
        assert!(is_heif_still(&iphone_heic().bytes));
        assert!(is_heif_still(&ftyp_heic()), "the ftyp box alone is enough");
        assert!(is_heif_still(&bx(b"ftyp", b"avif\0\0\0\0")), "avif");
        assert!(is_heif_still(&bx(b"ftyp", b"MiHE\0\0\0\0MiHAmif1")), "mif1 as a compatible brand is enough");
        for brand in HEIF_BRANDS {
            assert!(is_heif_still(&bx(b"ftyp", &[&brand[..], b"\0\0\0\0"].concat())), "{}", String::from_utf8_lossy(brand));
        }
        assert!(!is_heif_still(&bx(b"ftyp", b"isom\0\0\x02\0isomiso2mp41")), "an MP4 video");
        assert!(!is_heif_still(&bx(b"ftyp", b"qt  \0\0\0\0qt  ")), "a QuickTime movie");
        let sequence = [bx(b"ftyp", b"msf1\0\0\0\0msf1heic"), bx(b"moov", &filler(20, 1))].concat();
        assert!(!is_heif_still(&sequence), "a movie in view");
        assert!(!is_heif_still(b"\xFF\xD8\xFF\xE0\x00\x10JFIF\0"), "a JPEG");
        assert!(!is_heif_still(&ftyp_heic()[..7]), "too short to tell");
        assert!(!is_heif_still(b"\0\0\0\x04ftypheic"), "a size smaller than a header");
        assert!(!is_heif_still(b"\0\0\0\0ftypheic"), "size 0: the whole file is ftyp");
        assert!(!is_heif_still(&bx(b"ftyp", b"hei")), "no whole brand");
        let mut large = 1u32.to_be_bytes().to_vec();
        large.extend_from_slice(b"ftyp");
        large.extend_from_slice(&24u64.to_be_bytes());
        large.extend_from_slice(b"heic\0\0\0\0");
        assert!(is_heif_still(&large), "a 64-bit ftyp size");
    }

    /// Tables that cannot be read with certainty (two iinf boxes, an infe
    /// version this strip does not read, an iloc field size of 3, no meta at
    /// all): nothing is written, and the report says why.
    ///
    /// Seen red 2026-10-11 with the duplicate-box check in `plan_items` taken
    /// out: "two iinf boxes stop the walk" (the table's now-shifted offsets
    /// were trusted and zeroed bytes that are not the Exif).
    #[test]
    fn tables_that_make_no_sense_stop_the_walk_and_write_nothing() {
        let b = iphone_heic();
        // A second iinf inside meta.
        let iinf_at = b.bytes.windows(4).position(|w| w == b"iinf").unwrap() - 4;
        let iinf_len = u32::from_be_bytes(b.bytes[iinf_at..iinf_at + 4].try_into().unwrap()) as usize;
        let iinf = b.bytes[iinf_at..iinf_at + iinf_len].to_vec();
        let meta_at = b.bytes.windows(4).position(|w| w == b"meta").unwrap() - 4;
        let mut twice = b.bytes.clone();
        twice.splice(iinf_at..iinf_at, iinf.clone());
        let meta_len = u32::from_be_bytes(twice[meta_at..meta_at + 4].try_into().unwrap()) as usize + iinf.len();
        twice[meta_at..meta_at + 4].copy_from_slice(&(meta_len as u32).to_be_bytes());
        // The iloc's offsets now point iinf.len() bytes too early: nothing
        // must be written at all, wherever they point.
        let (after, r) = strip_mem(&twice);
        assert_eq!(after, twice, "two iinf boxes stop the walk");
        assert!(r.stopped_early.iter().any(|s| s.contains("two 'iinf' boxes")), "{:?}", r.stopped_early);

        // An infe of version 1, which this strip does not read.
        let mut v1 = b.bytes.clone();
        let infe_at = v1.windows(4).position(|w| w == b"infe").unwrap() + 4;
        v1[infe_at] = 1;
        let (after, r) = strip_mem(&v1);
        assert_eq!(after, v1);
        assert!(r.stopped_early[0].contains("versions 2 and 3"), "{:?}", r.stopped_early);

        // An iloc offset size of 3 bytes.
        let mut odd = b.bytes.clone();
        let iloc_at = odd.windows(4).position(|w| w == b"iloc").unwrap() + 4;
        odd[iloc_at + 4] = 0x34;
        let (after, r) = strip_mem(&odd);
        assert_eq!(after, odd);
        assert!(r.stopped_early[0].contains("offset size of 3"), "{:?}", r.stopped_early);

        // No meta at all.
        let none = [ftyp_heic(), bx(b"mdat", &filler(50, 1))].concat();
        let (after, r) = strip_mem(&none);
        assert_eq!((after, r.kind), (none, HeifKind::Still));
        assert!(r.stopped_early[0].contains("no meta box"), "{:?}", r.stopped_early);
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
            if n == 0 {
                0
            } else {
                self.next() % n
            }
        }
        fn bytes(&mut self, n: usize) -> Vec<u8> {
            (0..n).map(|_| self.next() as u8).collect()
        }
    }

    const NAMES: [&[u8; 4]; 10] = [b"meta", b"iinf", b"iloc", b"idat", b"infe", b"mdat", b"moov", b"iprp", b"ftyp", b"free"];

    /// A random run of boxes, nested up to two deep, with sizes usually
    /// right and sometimes 0 or simply wrong.
    fn random_boxes(rng: &mut Rng, depth: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for _ in 0..rng.below(5) {
            let name = *NAMES[rng.below(NAMES.len() as u64) as usize];
            let payload = if depth < 2 && matches!(&name, b"meta" | b"iinf") && rng.below(3) != 0 {
                [rng.bytes(4), random_boxes(rng, depth + 1)].concat()
            } else {
                let n = rng.below(48) as usize;
                rng.bytes(n)
            };
            match rng.below(8) {
                0 => {
                    out.extend(0u32.to_be_bytes());
                    out.extend(name);
                    out.extend(payload);
                }
                1 => {
                    out.extend((rng.below(90) as u32).to_be_bytes());
                    out.extend(name);
                    out.extend(payload);
                }
                _ => out.extend(bx(&name, &payload)),
            }
        }
        out
    }

    /// A real picture with random tables: iloc version, extents per item,
    /// base offset size, and the Exif in mdat or idat.
    fn random_picture(rng: &mut Rng) -> Vec<u8> {
        let tables = Tables {
            iloc_version: rng.below(3) as u8,
            extents_per_item: 1 + rng.below(3) as usize,
            base_size: [0, 4, 8][rng.below(3) as usize],
            ..Tables::default()
        };
        let place = if tables.iloc_version >= 1 && rng.below(2) == 0 { Place::Idat } else { Place::Mdat };
        heif(ftyp_heic(), &tables, &[picture(1, 80), exif(3, place), xmp(4, Place::Mdat)]).bytes
    }

    /// Every box name a built picture holds.
    const BUILT_NAMES: [&[u8; 4]; 14] =
        [b"meta", b"hdlr", b"pitm", b"iinf", b"infe", b"iloc", b"iref", b"cdsc", b"iprp", b"ipco", b"hvcC", b"ispe", b"ipma", b"idat"];

    /// A random picture: a real one damaged, a real one with one box's size
    /// changed, or a random box tree behind a HEIF ftyp.
    fn random_file(rng: &mut Rng) -> Vec<u8> {
        match rng.below(5) {
            0 => [ftyp_heic(), random_boxes(rng, 0)].concat(),
            1 => {
                let n = rng.below(120) as usize;
                let noise = rng.bytes(n);
                if rng.below(2) == 0 { [ftyp_avif(), noise].concat() } else { noise }
            }
            // One box's size made small, large, a 64-bit marker, or nearly right.
            2 => {
                let mut f = random_picture(rng);
                let name = BUILT_NAMES[rng.below(BUILT_NAMES.len() as u64) as usize];
                if let Some(at) = f.windows(4).skip(4).position(|w| w == &name[..]) {
                    let size_at = at; // `skip(4)` makes the position the size field's
                    let old = u32::from_be_bytes(f[size_at..size_at + 4].try_into().unwrap());
                    let new = match rng.below(5) {
                        0 => rng.below(8) as u32,
                        1 => rng.below(64) as u32,
                        2 => rng.next() as u32,
                        3 => old.wrapping_add(rng.below(9) as u32).wrapping_sub(4),
                        _ => (f.len() - size_at) as u32,
                    };
                    f[size_at..size_at + 4].copy_from_slice(&new.to_be_bytes());
                }
                f
            }
            _ => {
                let mut f = random_picture(rng);
                for _ in 0..rng.below(5) {
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

    /// 4,000 fixed-seed random inputs (random box trees behind a HEIF ftyp,
    /// noise, real pictures with one box size changed, real pictures with
    /// bytes changed or cut short): no panic, no size change, no byte
    /// changed outside a reported extent, a second run changing nothing, and
    /// file and memory agreeing.
    ///
    /// Seen red 2026-10-11 with sizes 2 to 7 taken as real box sizes in
    /// `decode_header`: "slice index starts at 14 but ends at 11". The
    /// generator as first written (without the box-size mode) passed with
    /// that break, which is why that mode exists.
    #[test]
    fn random_inputs_never_panic_or_change_size() {
        let mut rng = Rng(0x5EED_4E1F_0000_0001);
        let mut walked = 0;
        let mut zeroed_some = 0;
        for i in 0..4000 {
            let before = random_file(&mut rng);
            let (after, r) = strip_mem(&before);
            assert_only_reported_extents_changed(&before, &after, &r);
            if r.kind == HeifKind::Still {
                walked += 1;
            }
            if r.changed() {
                zeroed_some += 1;
            }
            // A second run changes no byte.
            let (again, _) = strip_mem(&after);
            assert_eq!(again, after, "input {i}");
            if i % 64 == 0 {
                let (after_file, r_file) = strip_file(&before);
                assert_eq!((after_file, r_file), (after, r), "input {i}: file and memory must agree");
            }
        }
        assert!(walked > 2000, "most inputs should reach the item tables, got {walked}");
        assert!(zeroed_some > 500, "many damaged pictures still have a readable Exif, got {zeroed_some}");
    }
}
