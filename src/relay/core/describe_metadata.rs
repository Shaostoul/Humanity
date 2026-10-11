//! What a picture's or a video's hidden information says, in plain words (2026-10-11,
//! docs/design/media-showcase.md, "Files on the sender's device and in messages, exact", part 1).
//!
//! Before a file leaves the device, the attach step shows what is in it: "This photo says where it
//! was taken and which camera took it." Then one switch, "Remove location and camera details". This
//! module gives that sentence and the plain-word details under it. It only ever READS: the strips
//! beside it (strip_metadata.rs, strip_video_metadata.rs, strip_heif_metadata.rs) do the removing,
//! and this looks only inside what they remove, so the switch removes exactly what this lists.
//!
//! THE SPECIFICATION IS THE WEB'S `HosStrip.describe` (web/shared/strip-metadata.js), and this is a
//! port of it, function for function, so the desktop and the web say the same words about the same
//! file. The two are held together by shared fixtures (tests/fixtures/describe/, each file with the
//! web's own answer in expected.json, checked by both apps' tests) and by
//! scripts/tests/describe-agree.test.js, which compiles this file with rustc alone and requires the
//! same answer on thousands of generated files. So this file uses the standard library only.
//!
//! What "the same" took, in plain words. JavaScript strings are UTF-16 and the web decodes text with
//! the browser's UTF-8 decoder: here text is decoded the same way (a byte-order mark at the start
//! dropped, every broken sequence one replacement character), runs of control characters become one
//! space, the ends are trimmed of the characters JavaScript calls white space (not quite Rust's
//! list), and a length limit counts UTF-16 units. The regular expressions of the web are written out
//! by hand here, with the same "first match wins" order. Numbers are parsed the way `parseFloat`
//! reads them. The one difference left: where JavaScript would keep half of a character (a lone
//! surrogate, from a cut in the middle of an emoji or a broken UTF-16 place name), Rust cannot hold
//! one, so it is dropped (a cut) or replaced (a place name). No phone writes either.
//!
//! What it understands: Exif (GPS, camera make and model, serial numbers, lens, times, owner names,
//! comments), XMP (GPS, place names, camera, lens, serials, times, people, captions), IPTC in a
//! JPEG's APP13 (place names, people, captions), PNG text chunks (by keyword, including
//! ImageMagick's raw Exif profile), and in an MP4, MOV or 3GP: Apple's QuickTime keys, the
//! copyright-sign items (`\xA9xyz` is where Android writes the place), Android's own keys, and
//! 3GPP's `loci` place box.
//!
//! A big video is read from disk by its box headers only ([`describe_file`]): the metadata boxes are
//! loaded, the frames never are.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// One thing the hidden information says, in plain words for the attach step.
#[derive(Debug, Clone, PartialEq)]
pub struct Detail {
    /// What kind of thing: "location", "place-name", "camera", "serial", "lens", "time",
    /// "changed-time", "person", "software", "comment", "text", "unread" or "other".
    pub key: &'static str,
    /// The words shown ("Where it was taken", "Which camera or phone took it (Apple iPhone 15 Pro)").
    pub words: String,
}

/// What a file's hidden information says (`HosStrip.describe`'s answer).
#[derive(Debug, Clone, PartialEq)]
pub struct Description {
    /// What the BYTES are: "jpeg", "png", "webp", "gif", "mp4" (MOV and 3GP too), "heif", "webm" or
    /// "other".
    pub kind: &'static str,
    /// What the name or type said, in the same words, if anything.
    pub claimed: Option<&'static str>,
    /// It says where it was taken (a position or a place name).
    pub has_location: bool,
    /// It says which camera (make, model, serial or lens).
    pub has_camera: bool,
    /// The plain words, in a fixed order, only for what the strips remove.
    pub details: Vec<Detail>,
    /// "This photo says where it was taken and which camera took it." or None when nothing was found.
    pub summary: Option<String>,
    /// False when the file could not be read all the way through.
    pub complete: bool,
    /// The position, as (latitude, longitude): never put into the words.
    pub coordinates: Option<(f64, f64)>,
    /// A place name, if one was written: never put into the words.
    pub place: Option<String>,
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// Reading bytes (a byte past the end reads as 0, as `undefined` does in JavaScript's bit operators)
// ════════════════════════════════════════════════════════════════════════════════════════════════

fn at(b: &[u8], i: usize) -> u32 {
    b.get(i).map_or(0, |&x| x as u32)
}
fn be16(b: &[u8], i: usize) -> u32 {
    (at(b, i) << 8) | at(b, i.saturating_add(1))
}
fn be32(b: &[u8], i: usize) -> u32 {
    (at(b, i) << 24) | (at(b, i.saturating_add(1)) << 16) | (at(b, i.saturating_add(2)) << 8) | at(b, i.saturating_add(3))
}
fn le32(b: &[u8], i: usize) -> u32 {
    at(b, i) | (at(b, i.saturating_add(1)) << 8) | (at(b, i.saturating_add(2)) << 16) | (at(b, i.saturating_add(3)) << 24)
}

/// True when the bytes at `i` are `s`.
fn starts_at(b: &[u8], i: usize, s: &[u8]) -> bool {
    i.checked_add(s.len()).is_some_and(|end| end <= b.len()) && &b[i..i + s.len()] == s
}

fn has_content(b: &[u8], s: usize, e: usize) -> bool {
    let e = e.min(b.len());
    s < e && b[s..e].iter().any(|&x| x != 0)
}

fn has_text(b: &[u8], s: usize, e: usize) -> bool {
    let e = e.min(b.len());
    s < e && b[s..e].iter().any(|&x| x > 0x20)
}

fn find_from(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || from > hay.len() || hay.len() - from < needle.len() {
        return None;
    }
    hay[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// JavaScript's text rules
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// What JavaScript's `trim()` and `\s` call white space (Rust's list differs: it adds U+0085 and
/// leaves out U+FEFF).
fn js_space(c: char) -> bool {
    matches!(c, '\u{9}'..='\u{D}' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}' | '\u{FEFF}')
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(js_space)
}

/// The first `max` UTF-16 units of `s` (`String.prototype.slice(0, max)`). A character that would
/// be cut in half is left out (see the module notes).
fn utf16_prefix(s: &str, max: usize) -> String {
    let mut units = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        units += c.len_utf16();
        if units > max {
            break;
        }
        out.push(c);
    }
    out
}

/// `TextDecoder('utf-8').decode`: a byte-order mark at the start is dropped, and every broken
/// sequence is one replacement character (the WHATWG rule, which Rust's lossy decoding follows).
fn decode_utf8(view: &[u8]) -> String {
    let v = if view.starts_with(&[0xEF, 0xBB, 0xBF]) { &view[3..] } else { view };
    String::from_utf8_lossy(v).into_owned()
}

/// The web's `textOf`: the text of `b[s..e]`, control characters made spaces, trimmed, at most
/// `max` UTF-16 units (200 when `max` is 0).
fn text_of(b: &[u8], s: usize, e: usize, max: usize) -> String {
    let e = e.min(b.len());
    if e <= s {
        return String::new();
    }
    let max = if max == 0 { 200 } else { max };
    let view = &b[s..e.min(s.saturating_add(max.saturating_mul(4)))];
    let t = decode_utf8(view);
    let mut out = String::with_capacity(t.len());
    let mut in_run = false;
    for c in t.chars() {
        if c <= '\u{1F}' || c == '\u{7F}' {
            if !in_run {
                out.push(' ');
                in_run = true;
            }
        } else {
            out.push(c);
            in_run = false;
        }
    }
    utf16_prefix(js_trim(&out), max)
}

/// `parseFloat`: leading white space, then the longest number at the start (a sign, digits, a
/// point, more digits, an exponent; or "Infinity"), NaN when there is none.
fn parse_float_js(t: &str) -> f64 {
    let s = t.trim_start_matches(js_space).as_bytes();
    let mut i = 0usize;
    let mut sign = "";
    if i < s.len() && (s[i] == b'+' || s[i] == b'-') {
        if s[i] == b'-' {
            sign = "-";
        }
        i += 1;
    }
    if s[i..].starts_with(b"Infinity") {
        return if sign == "-" { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    let int_start = i;
    while i < s.len() && s[i].is_ascii_digit() {
        i += 1;
    }
    let int = &s[int_start..i];
    let mut frac: &[u8] = &[];
    if i < s.len() && s[i] == b'.' {
        let f0 = i + 1;
        let mut j = f0;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        frac = &s[f0..j];
        // "5." reads as 5; "." alone is not a number.
        if !int.is_empty() || !frac.is_empty() {
            i = j;
        }
    }
    if int.is_empty() && frac.is_empty() {
        return f64::NAN;
    }
    let mut exp = String::new();
    if i < s.len() && (s[i] == b'e' || s[i] == b'E') {
        let mut j = i + 1;
        let mut esign = "";
        if j < s.len() && (s[j] == b'+' || s[j] == b'-') {
            if s[j] == b'-' {
                esign = "-";
            }
            j += 1;
        }
        let d0 = j;
        while j < s.len() && s[j].is_ascii_digit() {
            j += 1;
        }
        if j > d0 {
            exp = format!("e{esign}{}", String::from_utf8_lossy(&s[d0..j]));
        }
    }
    let int_s = if int.is_empty() { "0".to_string() } else { String::from_utf8_lossy(int).into_owned() };
    let frac_s = if frac.is_empty() { "0".to_string() } else { String::from_utf8_lossy(frac).into_owned() };
    format!("{sign}{int_s}.{frac_s}{exp}").parse::<f64>().unwrap_or(f64::NAN)
}

/// A digit string as a number (`parseInt(x, 10)` on a short all-digit string).
fn digits_value(d: &str) -> f64 {
    d.parse::<f64>().unwrap_or(f64::NAN)
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// What was found
// ════════════════════════════════════════════════════════════════════════════════════════════════

#[derive(Default)]
struct Found {
    keys: BTreeSet<&'static str>,
    hits: u64,
    make: String,
    model: String,
    coords: Option<(f64, f64)>,
    place: Option<String>,
}

impl Found {
    fn note(&mut self, key: &'static str) {
        self.keys.insert(key);
        self.hits += 1;
    }
    fn set_coords(&mut self, c: Option<(f64, f64)>) {
        if c.is_some() && self.coords.is_none() {
            self.coords = c;
        }
    }
    fn set_place(&mut self, t: &str) {
        if !t.is_empty() && self.place.is_none() {
            self.place = Some(t.to_string());
        }
    }
    fn set_make(&mut self, t: &str) {
        self.note("camera");
        if !t.is_empty() && self.make.is_empty() {
            self.make = t.to_string();
        }
    }
    fn set_model(&mut self, t: &str) {
        self.note("camera");
        if !t.is_empty() && self.model.is_empty() {
            self.model = t.to_string();
        }
    }
}

/// Look inside one removed block; anything with content that says nothing known is "other".
fn inspect(f: &mut Found, b: &[u8], s: usize, e: usize, look: impl FnOnce(&mut Found)) {
    let before = f.hits;
    look(f);
    if f.hits == before && has_content(b, s, e) {
        f.note("other");
    }
}

fn valid_coords(lat: f64, lon: f64) -> Option<(f64, f64)> {
    if !lat.is_finite() || !lon.is_finite() || lat.abs() > 90.0 || lon.abs() > 180.0 {
        return None;
    }
    Some((lat, lon))
}

/// Digits starting at `i`: (end, the digits). End is `i` when there are none.
fn digits_at(s: &[u8], i: usize) -> usize {
    let mut j = i;
    while j < s.len() && s[j].is_ascii_digit() {
        j += 1;
    }
    j
}

/// `(\d+)(\.\d+)?` at `i`: (end of the digits, end of the fraction or the same), None without a digit.
fn number_at(s: &[u8], i: usize) -> Option<(usize, usize)> {
    let d = digits_at(s, i);
    if d == i {
        return None;
    }
    if d < s.len() && s[d] == b'.' {
        let f = digits_at(s, d + 1);
        if f > d + 1 {
            return Some((d, f));
        }
    }
    Some((d, d))
}

/// ISO 6709 text ("+37.3349-122.0090+030.000/"), as Apple and Android write it: the first match of
/// `([+-])(\d+)(\.\d+)?([+-])(\d+)(\.\d+)?`.
fn parse_iso6709(t: &str) -> Option<(f64, f64)> {
    let s = t.as_bytes();
    for p in 0..s.len() {
        if s[p] != b'+' && s[p] != b'-' {
            continue;
        }
        let Some((d1, f1)) = number_at(s, p + 1) else { continue };
        if f1 >= s.len() || (s[f1] != b'+' && s[f1] != b'-') {
            continue;
        }
        let Some((d2, f2)) = number_at(s, f1 + 1) else { continue };
        let text = |a: usize, b: usize| String::from_utf8_lossy(&s[a..b]).into_owned();
        let part = |sign: u8, int: String, frac: String, deg: usize| -> f64 {
            let v = if int.len() <= deg {
                parse_float_js(&(int.clone() + &frac))
            } else if int.len() == deg + 2 {
                digits_value(&int[..deg]) + parse_float_js(&(int[deg..].to_string() + &frac)) / 60.0
            } else if int.len() == deg + 4 {
                digits_value(&int[..deg]) + digits_value(&int[deg..deg + 2]) / 60.0 + parse_float_js(&(int[deg + 2..].to_string() + &frac)) / 3600.0
            } else {
                return f64::NAN;
            };
            if sign == b'-' {
                -v
            } else {
                v
            }
        };
        let lat = part(s[p], text(p + 1, d1), text(d1, f1), 2);
        let lon = part(s[f1], text(f1 + 1, d2), text(d2, f2), 3);
        return valid_coords(lat, lon);
    }
    None
}

/// An XMP GPS value: "37,20.094N", "37,20,5.6N", or a plain decimal.
fn parse_xmp_gps(t: &str) -> f64 {
    if t.is_empty() {
        return f64::NAN;
    }
    // `^\s*(\d+(?:\.\d+)?),(\d+(?:\.\d+)?)(?:,(\d+(?:\.\d+)?))?\s*([NSEW])`, case-insensitive.
    let lead = t.len() - t.trim_start_matches(js_space).len();
    let s = t.as_bytes();
    let matched = (|| {
        let (_, e1) = number_at(s, lead)?;
        if s.get(e1) != Some(&b',') {
            return None;
        }
        let (_, e2) = number_at(s, e1 + 1)?;
        let mut end = e2;
        let mut third = None;
        if s.get(e2) == Some(&b',') {
            if let Some((_, e3)) = number_at(s, e2 + 1) {
                third = Some((e2 + 1, e3));
                end = e3;
            }
        }
        let rest = &t[end..];
        let after = rest.trim_start_matches(js_space);
        let c = after.chars().next()?;
        if !matches!(c, 'N' | 'S' | 'E' | 'W' | 'n' | 's' | 'e' | 'w') {
            return None;
        }
        Some(((lead, e1), (e1 + 1, e2), third, c))
    })();
    let Some((n1, n2, n3, dir)) = matched else { return parse_float_js(t) };
    let num = |r: (usize, usize)| parse_float_js(&t[r.0..r.1]);
    let v = num(n1) + num(n2) / 60.0 + n3.map_or(0.0, |r| num(r) / 3600.0);
    if matches!(dir, 'S' | 'W' | 's' | 'w') {
        -v
    } else {
        v
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// Exif (a TIFF structure)
// ════════════════════════════════════════════════════════════════════════════════════════════════

const TIFF_UNIT: [u64; 14] = [0, 1, 1, 2, 4, 8, 1, 1, 2, 4, 8, 4, 8, 4];

#[derive(Clone, Copy, PartialEq)]
enum Ifd {
    Zero,
    Exif,
    Gps,
}

#[derive(Default)]
struct Gps {
    lat_ref: String,
    lat: Option<f64>,
    lon_ref: String,
    lon: Option<f64>,
    other: bool,
}

struct Tiff<'a, 'f> {
    b: &'a [u8],
    start: u64,
    end: u64,
    little: bool,
    seen: Vec<u64>,
    gps: Gps,
    f: &'f mut Found,
}

impl Tiff<'_, '_> {
    fn u16(&self, o: u64) -> u64 {
        let o = o as usize;
        (if self.little { at(self.b, o) | (at(self.b, o.saturating_add(1)) << 8) } else { be16(self.b, o) }) as u64
    }
    fn u32(&self, o: u64) -> u64 {
        let o = o as usize;
        (if self.little { le32(self.b, o) } else { be32(self.b, o) }) as u64
    }
    /// ASCII values end at their first zero byte.
    fn ascii(&self, a: u64, n: u64) -> String {
        let mut stop = a;
        while stop < a + n && at(self.b, stop as usize) != 0 {
            stop += 1;
        }
        text_of(self.b, a as usize, stop as usize, 120)
    }
    fn rationals(&self, a: u64) -> [f64; 3] {
        let mut out = [0.0; 3];
        for (k, v) in out.iter_mut().enumerate() {
            let k = k as u64;
            let den = self.u32(a + 8 * k + 4);
            *v = if den != 0 { self.u32(a + 8 * k) as f64 / den as f64 } else { f64::NAN };
        }
        out
    }
    fn ifd(&mut self, rel: u64, which: Ifd) {
        if self.seen.contains(&rel) || self.seen.len() >= 8 {
            return;
        }
        self.seen.push(rel);
        let a = self.start + rel;
        if rel < 8 || a + 2 > self.end {
            return;
        }
        let count = self.u16(a);
        let mut k = 0u64;
        while k < count && k < 1024 {
            let e = a + 2 + 12 * k;
            k += 1;
            if e + 12 > self.end {
                return;
            }
            let tag = self.u16(e);
            let ty = self.u16(e + 2);
            let n = self.u32(e + 4);
            let bytes = TIFF_UNIT.get(ty as usize).copied().unwrap_or(0) * n;
            let val_at = if bytes <= 4 { e + 8 } else { self.start + self.u32(e + 8) };
            let ok = bytes > 0 && val_at >= self.start && val_at + bytes <= self.end;
            match which {
                Ifd::Zero => {
                    if tag == 0x8769 {
                        let to = self.u32(e + 8);
                        self.ifd(to, Ifd::Exif);
                    } else if tag == 0x8825 {
                        let to = self.u32(e + 8);
                        self.ifd(to, Ifd::Gps);
                    } else if tag == 0x010F {
                        let t = if ok && ty == 2 { self.ascii(val_at, n) } else { String::new() };
                        self.f.set_make(&t);
                    } else if tag == 0x0110 {
                        let t = if ok && ty == 2 { self.ascii(val_at, n) } else { String::new() };
                        self.f.set_model(&t);
                    } else if tag == 0x0131 {
                        self.f.note("software");
                    } else if tag == 0x0132 {
                        self.f.note("time");
                    } else if tag == 0x013B || tag == 0x8298 || tag == 0x9C9D {
                        self.f.note("person");
                    } else if tag == 0x010E || (0x9C9B..=0x9C9F).contains(&tag) {
                        self.f.note("comment");
                    }
                }
                Ifd::Exif => {
                    if tag == 0x9003 || tag == 0x9004 {
                        self.f.note("time");
                    } else if tag == 0xA430 {
                        self.f.note("person");
                    } else if tag == 0xA431 || tag == 0xA435 {
                        self.f.note("serial");
                    } else if (0xA432..=0xA434).contains(&tag) {
                        self.f.note("lens");
                    } else if tag == 0x9286 && ok && has_text(self.b, (val_at + 8) as usize, (val_at + bytes) as usize) {
                        self.f.note("comment");
                    } else if tag == 0x927C || tag == 0xA420 {
                        self.f.note("other"); // maker notes, a unique id
                    }
                }
                Ifd::Gps => {
                    if tag == 1 && ok {
                        self.gps.lat_ref = self.ascii(val_at, n);
                    } else if tag == 3 && ok {
                        self.gps.lon_ref = self.ascii(val_at, n);
                    } else if tag == 2 || tag == 4 {
                        self.f.note("location");
                        if ok && ty == 5 && n >= 3 {
                            let r = self.rationals(val_at);
                            let v = r[0] + r[1] / 60.0 + r[2] / 3600.0;
                            if tag == 2 {
                                self.gps.lat = Some(v);
                            } else {
                                self.gps.lon = Some(v);
                            }
                        }
                    } else {
                        self.gps.other = true;
                    }
                }
            }
        }
    }
}

/// Read an Exif block (TIFF header at `start`) for what it says.
fn read_tiff(b: &[u8], start: usize, end: usize, f: &mut Found) {
    let end = end.min(b.len());
    if end < start || end - start < 8 {
        return;
    }
    let little = if b[start] == 0x49 && b[start + 1] == 0x49 {
        true
    } else if b[start] == 0x4D && b[start + 1] == 0x4D {
        false
    } else {
        return;
    };
    let mut t = Tiff { b, start: start as u64, end: end as u64, little, seen: Vec::new(), gps: Gps::default(), f };
    if t.u16(start as u64 + 2) != 42 {
        return;
    }
    let first = t.u32(start as u64 + 4);
    t.ifd(first, Ifd::Zero);
    let gps = std::mem::take(&mut t.gps);
    if let (Some(lat), Some(lon)) = (gps.lat, gps.lon) {
        let has = |r: &str, c: char| r.contains(c) || r.contains(c.to_ascii_lowercase());
        let lat = if has(&gps.lat_ref, 'S') { -lat } else { lat };
        let lon = if has(&gps.lon_ref, 'W') { -lon } else { lon };
        t.f.set_coords(valid_coords(lat, lon));
    } else if gps.other && !t.f.keys.contains("location") {
        t.f.note("other");
    }
}

/// An Exif block that may begin with "Exif\0\0" (PNG eXIf and WebP EXIF sometimes do).
fn read_exif_block(b: &[u8], s: usize, e: usize, f: &mut Found) {
    let start = if starts_at(b, s, b"Exif\0\0") { s + 6 } else { s };
    read_tiff(b, start, e, f);
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// XMP (text)
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// The char at byte `i` of `t`, if `i` starts one.
fn char_at(t: &str, i: usize) -> Option<char> {
    t.get(i..).and_then(|r| r.chars().next())
}

/// The web's `xmpValue`: the first match of `tag\s*=\s*"([^"]*)"` or
/// `<tag(?:\s[^>]*)?>([^<]*)</tag>`, trimmed; "" when neither is there.
fn xmp_value(text: &str, tag: &str) -> String {
    let s = text.as_bytes();
    let close = format!("</{tag}>");
    let mut from = 0usize;
    while let Some(q) = find_from(s, tag.as_bytes(), from) {
        from = q + 1;
        let after = q + tag.len();
        // The element form, starting one byte earlier (so it is tried first).
        if q >= 1 && s[q - 1] == b'<' {
            let content = match char_at(text, after) {
                Some('>') => Some(after + 1),
                Some(c) if js_space(c) => find_from(s, b">", after).map(|g| g + 1),
                _ => None,
            };
            if let Some(c0) = content {
                let lt = find_from(s, b"<", c0).unwrap_or(s.len());
                if starts_at(s, lt, close.as_bytes()) {
                    return js_trim(&text[c0..lt]).to_string();
                }
            }
        }
        // The attribute form.
        let mut i = after;
        while let Some(c) = char_at(text, i).filter(|&c| js_space(c)) {
            i += c.len_utf8();
        }
        if s.get(i) == Some(&b'=') {
            i += 1;
            while let Some(c) = char_at(text, i).filter(|&c| js_space(c)) {
                i += c.len_utf8();
            }
            if s.get(i) == Some(&b'"') {
                if let Some(qe) = find_from(s, b"\"", i + 1) {
                    return js_trim(&text[i + 1..qe]).to_string();
                }
            }
        }
    }
    String::new()
}

/// Any of `lits` in `hay`, each followed by a word boundary when `boundary` (the web's `\b`, ASCII).
fn has_any(hay: &[u8], lits: &[&str], boundary: bool) -> bool {
    lits.iter().any(|lit| {
        let mut from = 0usize;
        while let Some(q) = find_from(hay, lit.as_bytes(), from) {
            let next = hay.get(q + lit.len()).copied();
            if !boundary || !next.is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_') {
                return true;
            }
            from = q + 1;
        }
        false
    })
}

/// The web's `XMP_RULES`, in its order.
fn xmp_rules(text: &str, f: &mut Found) {
    let s = text.as_bytes();
    let lower = text.to_ascii_lowercase();
    if has_any(lower.as_bytes(), &["gpslatitude", "gpslongitude"], true) {
        f.note("location");
    }
    let place = [
        "photoshop:City", "photoshop:State", "photoshop:Country", "Iptc4xmpCore:Location", "Iptc4xmpCore:CountryCode",
        "Iptc4xmpExt:LocationShown", "Iptc4xmpExt:LocationCreated", "Iptc4xmpExt:City", "Iptc4xmpExt:CountryName",
        "Iptc4xmpExt:ProvinceState", "Iptc4xmpExt:Sublocation",
    ];
    if has_any(s, &place, true) {
        f.note("place-name");
    }
    if has_any(s, &["aux:SerialNumber", "exifEX:BodySerialNumber", "LensSerialNumber"], false) {
        f.note("serial");
    }
    if has_any(s, &["aux:Lens"], true) || has_any(s, &["aux:LensInfo", "aux:LensID", "exifEX:LensModel", "exifEX:LensMake"], false) {
        f.note("lens");
    }
    if has_any(s, &["xmp:CreatorTool", "tiff:Software"], false) {
        f.note("software");
    }
    if has_any(s, &["xmp:CreateDate", "exif:DateTimeOriginal", "photoshop:DateCreated", "xmp:ModifyDate"], false) {
        f.note("time");
    }
    if has_any(s, &["dc:creator", "xmpRights:Owner", "aux:OwnerName", "exifEX:CameraOwnerName", "tiff:Artist", "dc:rights"], false) {
        f.note("person");
    }
    if has_any(s, &["dc:description", "dc:title", "exif:UserComment", "photoshop:Headline"], false) {
        f.note("comment");
    }
}

fn or_else(a: String, b: impl FnOnce() -> String) -> String {
    if a.is_empty() {
        b()
    } else {
        a
    }
}

fn scan_xmp(b: &[u8], s: usize, e: usize, f: &mut Found) {
    let text = text_of(b, s, e.min(s.saturating_add(1_048_576)), 1_048_576);
    xmp_rules(&text, f);
    let make = xmp_value(&text, "tiff:Make");
    let model = xmp_value(&text, "tiff:Model");
    if !make.is_empty() {
        f.set_make(&make);
    }
    if !model.is_empty() {
        f.set_model(&model);
    }
    if f.keys.contains("location") {
        let lat = or_else(xmp_value(&text, "exif:GPSLatitude"), || xmp_value(&text, "drone-dji:GpsLatitude"));
        let lon = or_else(xmp_value(&text, "exif:GPSLongitude"), || xmp_value(&text, "drone-dji:GpsLongitude"));
        f.set_coords(valid_coords(parse_xmp_gps(&lat), parse_xmp_gps(&lon)));
    }
    let place = or_else(xmp_value(&text, "photoshop:City"), || xmp_value(&text, "Iptc4xmpCore:Location"));
    f.set_place(&place);
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// IPTC (in a JPEG's APP13)
// ════════════════════════════════════════════════════════════════════════════════════════════════

fn scan_iptc(b: &[u8], s: usize, e: usize, f: &mut Found) {
    let mut i = s;
    while i + 5 <= e {
        if at(b, i) == 0x1C && at(b, i + 1) == 0x02 {
            let ds = at(b, i + 2);
            let len = be16(b, i + 3) as usize;
            if len & 0x8000 == 0 && i + 5 + len <= e {
                if [90, 92, 95, 100, 101].contains(&ds) {
                    f.note("place-name");
                    if ds == 90 {
                        f.set_place(&text_of(b, i + 5, i + 5 + len, 120));
                    }
                } else if [80, 116, 118].contains(&ds) {
                    f.note("person");
                } else if [5, 105, 120].contains(&ds) {
                    f.note("comment");
                } else if ds == 55 || ds == 60 {
                    f.note("time");
                } else if ds == 65 {
                    f.note("software");
                } else if ds == 25 {
                    f.note("text");
                }
                i += 5 + len;
                continue;
            }
        }
        i += 1;
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// PNG text chunks
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// The keyword of a tEXt, zTXt or iTXt chunk (Latin-1, up to 79 letters), lower-cased, and where
/// what follows it starts (None when no zero byte ends it).
fn png_keyword(b: &[u8], s: usize, e: usize) -> (Vec<u8>, Option<usize>) {
    let mut k = Vec::new();
    let mut i = s;
    while i < e.min(s + 80) {
        let c = at(b, i) as u8;
        if c == 0 {
            return (k, Some(i + 1));
        }
        k.push(c.to_ascii_lowercase());
        i += 1;
    }
    (k, None)
}

/// A Latin-1 byte JavaScript's `\s` matches.
fn latin1_space(c: u8) -> bool {
    matches!(c, 0x09..=0x0D | 0x20 | 0xA0)
}

/// ImageMagick's "Raw profile type exif" text ("exif", the length, then hex), from s to e.
fn raw_profile_exif(b: &[u8], s: usize, e: usize, f: &mut Found) -> bool {
    let end = e.min(s.saturating_add(4_194_304)).min(b.len());
    let t = if s < end { &b[s..end] } else { &[][..] };
    let tokens: Vec<&[u8]> = t.split(|&c| latin1_space(c)).filter(|x| !x.is_empty()).collect();
    if tokens.len() < 3 || !tokens[1].iter().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let hex: Vec<u8> = tokens[2..].concat();
    if !hex.iter().all(|c| c.is_ascii_hexdigit()) || hex.len() < 16 {
        return false;
    }
    let n = (hex.len() >> 1).min(1_048_576);
    let val = |c: u8| (c as char).to_digit(16).unwrap_or(0) as u8;
    let bytes: Vec<u8> = (0..n).map(|k| (val(hex[2 * k]) << 4) | val(hex[2 * k + 1])).collect();
    read_exif_block(&bytes, 0, bytes.len(), f);
    true
}

fn png_text_chunk(b: &[u8], name: &str, s: usize, e: usize, f: &mut Found) {
    let (word, next) = png_keyword(b, s, e);
    // Where the text itself starts, when it is not compressed.
    let mut text_at: Option<usize> = None;
    if name == "tEXt" {
        text_at = next;
    } else if name == "iTXt" {
        if let Some(n) = next.filter(|&n| n + 2 <= e && at(b, n) == 0) {
            let mut p = Some(n + 2);
            for _ in 0..2 {
                // the language tag, then the translated keyword
                let Some(mut q) = p else { break };
                while q < e && at(b, q) != 0 {
                    q += 1;
                }
                p = if q < e { Some(q + 1) } else { None };
            }
            text_at = p;
        }
    }
    match word.as_slice() {
        b"xml:com.adobe.xmp" => match text_at {
            Some(t) => scan_xmp(b, t, e, f),
            None => f.note("unread"),
        },
        b"raw profile type exif" | b"raw profile type app1" => {
            // Usually compressed: its contents cannot be read here.
            if !text_at.is_some_and(|t| raw_profile_exif(b, t, e, f)) {
                f.note("unread");
            }
        }
        b"author" | b"copyright" | b"artist" => f.note("person"),
        b"title" | b"description" | b"comment" => f.note("comment"),
        b"creation time" => f.note("time"),
        b"software" => f.note("software"),
        b"source" => {
            let t = text_at.map_or(String::new(), |t| text_of(b, t, e, 80));
            f.set_model(&t);
        }
        _ => f.note("text"),
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// MP4 metadata boxes
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// The child boxes between `pos` and `e`, one at a time, each costing one from a budget shared by
/// the whole description (so a crafted file of millions of boxes is cut off at the same box as on
/// the web). Stops at the first size that makes no sense.
struct Children {
    pos: usize,
    e: usize,
    done: bool,
}

impl Children {
    fn new(s: usize, e: usize) -> Self {
        Children { pos: s, e, done: false }
    }
    /// (name, body start, end, the box's own start).
    fn next(&mut self, b: &[u8], budget: &mut i64) -> Option<([u8; 4], usize, usize, usize)> {
        if self.done || self.pos + 8 > self.e {
            return None;
        }
        *budget -= 1;
        if *budget < 0 {
            self.done = true;
            return None;
        }
        let pos = self.pos;
        let mut size = be32(b, pos) as u64;
        let mut hl = 8usize;
        if size == 0 {
            size = (self.e - pos) as u64;
        } else if size == 1 {
            if pos + 16 > self.e || be32(b, pos + 8) != 0 {
                self.done = true;
                return None;
            }
            size = be32(b, pos + 12) as u64;
            hl = 16;
            if size < 16 {
                self.done = true;
                return None;
            }
        } else if size < 8 {
            self.done = true;
            return None;
        }
        if pos as u64 + size > self.e as u64 {
            self.done = true;
            return None;
        }
        let size = size as usize;
        let name = [at(b, pos + 4) as u8, at(b, pos + 5) as u8, at(b, pos + 6) as u8, at(b, pos + 7) as u8];
        self.pos = pos + size;
        Some((name, pos + hl, pos + size, pos))
    }
}

/// The text of a metadata item: a `data` box (ilst style) or QuickTime's length-prefixed text.
fn atom_text(b: &[u8], s: usize, e: usize) -> String {
    if e >= s + 16 && starts_at(b, s + 4, b"data") {
        return text_of(b, s + 16, e.min(s + be32(b, s) as usize), 200);
    }
    if e >= s + 4 {
        return text_of(b, s + 4, e.min(s + 4 + be16(b, s) as usize), 200);
    }
    String::new()
}

/// What each item name in `udta` or an `ilst` says (0xA9 is the copyright sign).
fn atom_key(nm: &[u8; 4]) -> Option<&'static str> {
    Some(match nm {
        b"\xA9xyz" => "location",
        b"\xA9mak" | b"manu" => "make",
        b"\xA9mod" | b"modl" => "model",
        b"\xA9swr" | b"\xA9too" | b"\xA9enc" | b"FIRM" => "software",
        b"\xA9day" => "time",
        b"\xA9ART" | b"\xA9aut" | b"\xA9wrt" | b"\xA9prf" | b"\xA9cpy" | b"auth" | b"cprt" | b"perf" => "person",
        b"\xA9nam" | b"\xA9cmt" | b"\xA9des" | b"\xA9inf" | b"desc" | b"dscp" | b"titl" => "comment",
        b"kywd" | b"\xA9key" => "text",
        b"CAME" => "serial",
        b"LENS" => "lens",
        _ => return None,
    })
}

fn apply_item(f: &mut Found, key: &'static str, value: &str) {
    match key {
        "location" => {
            f.note("location");
            f.set_coords(parse_iso6709(value));
        }
        "make" => f.set_make(value),
        "model" => f.set_model(value),
        _ => f.note(key),
    }
}

/// One of Apple's QuickTime metadata keys (com.apple.quicktime.*), or Android's.
fn apple_key(f: &mut Found, key: &str, value: &str) {
    let k = key.strip_prefix("com.apple.quicktime.").unwrap_or(key);
    if k == "location.ISO6709" {
        f.note("location");
        f.set_coords(parse_iso6709(value));
    } else if k == "location.name" {
        f.note("place-name");
        f.set_place(value);
    } else if k == "make" || key == "com.android.manufacturer" {
        f.set_make(value);
    } else if k == "model" || key == "com.android.model" {
        f.set_model(value);
    } else if k == "software" || key == "com.android.version" {
        f.note("software");
    } else if k == "creationdate" {
        f.note("time");
    } else if ["author", "artist", "copyright", "publisher"].contains(&k) {
        f.note("person");
    } else if ["comment", "description", "title", "displayname", "information"].contains(&k) {
        f.note("comment");
    } else if k == "keywords" {
        f.note("text");
    } else if k.starts_with("camera.lens") || k.starts_with("camera.identifier") {
        f.note("lens");
    } else {
        f.note("other");
    }
}

/// A `meta` box: QuickTime's (children right away) or ISO's (4 bytes of version and flags first).
fn meta_box(b: &[u8], s: usize, e: usize, f: &mut Found, budget: &mut i64) {
    let mut c = s;
    if e >= s + 12 && !starts_at(b, s + 4, b"hdlr") && (starts_at(b, s + 8, b"hdlr") || be32(b, s) == 0) {
        c = s + 4;
    }
    let mut keys: Vec<String> = Vec::new();
    let mut ilst: Option<(usize, usize)> = None;
    let mut it = Children::new(c, e);
    while let Some((nm, cs, ce, _)) = it.next(b, budget) {
        if &nm == b"keys" {
            let mut p = cs + 8; // version and flags, then the count
            let count = if ce >= cs + 8 { be32(b, cs + 4) as u64 } else { 0 };
            let mut k = 0u64;
            while k < count && k < 4096 && p + 8 <= ce {
                let size = be32(b, p) as usize;
                if size < 8 || p + size > ce {
                    break;
                }
                keys.push(text_of(b, p + 8, p + size, 200));
                p += size;
                k += 1;
            }
        } else if &nm == b"ilst" {
            ilst = Some((cs, ce));
        } else if &nm == b"XMP_" {
            scan_xmp(b, cs, ce, f);
        }
    }
    let Some((is, ie)) = ilst else { return };
    let mut it = Children::new(is, ie);
    while let Some((nm, cs, ce, a)) = it.next(b, budget) {
        let index = be32(b, a + 4) as usize;
        let value = atom_text(b, cs, ce);
        if !keys.is_empty() && index >= 1 && index <= keys.len() {
            apple_key(f, &keys[index - 1], &value);
        } else if let Some(key) = atom_key(&nm) {
            apply_item(f, key, &value);
        } else if has_content(b, cs, ce) {
            f.note("other");
        }
    }
}

/// 3GPP's `loci`: a named place with its coordinates (16.16 fixed point, longitude first).
fn loci_box(b: &[u8], s: usize, e: usize, f: &mut Found) {
    f.note("location");
    f.note("place-name");
    let mut p = s + 6; // version and flags, language
    if p >= e {
        return;
    }
    let name: String;
    if at(b, p) == 0xFE && at(b, p + 1) == 0xFF {
        let mut q = p + 2;
        let mut units: Vec<u16> = Vec::new();
        while q + 1 < e && (at(b, q) | at(b, q + 1)) != 0 {
            units.push(((at(b, q) << 8) | at(b, q + 1)) as u16);
            q += 2;
        }
        name = String::from_utf16_lossy(&units);
        p = q + 2;
    } else {
        let mut q = p;
        while q < e && at(b, q) != 0 {
            q += 1;
        }
        name = text_of(b, p, q, 120);
        p = q + 1;
    }
    f.set_place(js_trim(&name));
    p += 1; // role
    if p + 8 > e {
        return;
    }
    let fixed = |o: usize| (be32(b, o) as i32) as f64 / 65536.0;
    f.set_coords(valid_coords(fixed(p + 4), fixed(p)));
}

fn udta_box(b: &[u8], s: usize, e: usize, f: &mut Found, budget: &mut i64) {
    let mut it = Children::new(s, e);
    while let Some((nm, cs, ce, _)) = it.next(b, budget) {
        if &nm == b"meta" {
            meta_box(b, cs, ce, f, budget);
        } else if &nm == b"XMP_" {
            scan_xmp(b, cs, ce, f);
        } else if &nm == b"loci" {
            loci_box(b, cs, ce, f);
        } else if let Some(key) = atom_key(&nm) {
            apply_item(f, key, &atom_text(b, cs, ce));
        } else if has_content(b, cs, ce) {
            f.note("other");
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// What kind of file
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// What a type ("image/jpeg") says the file is, in the web's words.
fn type_kind(t: &str) -> Option<&'static str> {
    Some(match t {
        "image/jpeg" | "image/jpg" | "image/pjpeg" => "jpeg",
        "image/png" | "image/apng" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" | "video/quicktime" | "video/3gpp" | "video/3gpp2" | "video/x-m4v" | "audio/mp4" | "audio/x-m4a" | "audio/m4a" => "mp4",
        "image/heic" | "image/heif" | "image/heic-sequence" | "image/heif-sequence" | "image/avif" => "heif",
        "video/webm" | "audio/webm" | "video/x-matroska" => "webm",
        _ => return None,
    })
}

/// What a file name's extension says the file is.
fn ext_kind(x: &str) -> Option<&'static str> {
    Some(match x {
        "jpg" | "jpeg" | "jpe" | "jfif" => "jpeg",
        "png" | "apng" => "png",
        "webp" => "webp",
        "gif" => "gif",
        "mp4" | "m4v" | "m4a" | "mov" | "qt" | "3gp" | "3g2" => "mp4",
        "heic" | "heif" | "hif" | "avif" => "heif",
        "webm" | "mkv" => "webm",
        _ => return None,
    })
}

/// What a file name ("IMG_0001.JPG") or a type ("image/jpeg") says the file is, or None.
pub fn claimed_kind(name_or_type: &str) -> Option<&'static str> {
    if name_or_type.is_empty() {
        return None;
    }
    let s = js_trim(name_or_type).to_lowercase();
    let t = js_trim(s.split(';').next().unwrap_or(""));
    if let Some(k) = type_kind(t) {
        return Some(k);
    }
    match s.rfind('.') {
        Some(dot) => ext_kind(&s[dot + 1..]),
        None => ext_kind(&s),
    }
}

/// The relay's `is_mp4_family`: the first box is an `ftyp` box with a usable size.
fn is_mp4_family(b: &[u8]) -> bool {
    if b.len() < 8 || !starts_at(b, 4, b"ftyp") {
        return false;
    }
    let size = be32(b, 0);
    size == 1 || size >= 8
}

/// What the bytes are (only the first twelve are looked at): "jpeg", "png", "webp", "gif", "webm",
/// "mp4" or "other". (A HEIC is "mp4" here: its box family; `describe` names it.)
pub fn sniff_kind(b: &[u8]) -> &'static str {
    let n = b.len();
    if n >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF {
        "jpeg"
    } else if n >= 4 && b[..4] == [0x89, 0x50, 0x4E, 0x47] {
        "png"
    } else if n >= 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        "webp"
    } else if n >= 6 && (&b[..6] == b"GIF87a" || &b[..6] == b"GIF89a") {
        "gif"
    } else if n >= 4 && b[..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        "webm"
    } else if is_mp4_family(b) {
        "mp4"
    } else {
        "other"
    }
}

/// The brands an `ftyp` box gives a HEIF or AVIF still picture (the web's list).
const HEIF_BRANDS: [&[u8; 4]; 14] = [b"heic", b"heix", b"heim", b"heis", b"hevc", b"hevx", b"hevm", b"hevs", b"mif1", b"mif2", b"msf1", b"avif", b"avis", b"avci"];
/// The brands of an MP4-family file that is sound only (for the words).
const AUDIO_BRANDS: [&[u8; 4]; 5] = [b"M4A ", b"M4B ", b"M4P ", b"F4A ", b"F4B "];

/// The `ftyp` box's major and compatible brands, from the file's first bytes (`head`, at least
/// the first 272 when the file has them) and its whole length.
fn ftyp_brands(head: &[u8], total: u64) -> Vec<[u8; 4]> {
    let name = |i: usize| [at(head, i) as u8, at(head, i + 1) as u8, at(head, i + 2) as u8, at(head, i + 3) as u8];
    let size = be32(head, 0) as u64;
    if size < 16 || size > total {
        return if total >= 12 { vec![name(8)] } else { Vec::new() };
    }
    let mut out = vec![name(8)];
    let mut i = 16u64;
    while i + 4 <= size && out.len() < 64 {
        out.push(name(i as usize));
        i += 4;
    }
    out
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// The walks: what the strips remove (pictures) or neutralise (videos)
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// One block a picture strip removes: its label, where it starts and its size.
struct Removed {
    label: &'static str,
    offset: usize,
    size: usize,
}

struct PictureWalk {
    ok: bool,
    removed: Vec<Removed>,
}

/// strip_metadata.rs's JPEG marker walk, reporting what it drops.
fn walk_jpeg(d: &[u8]) -> PictureWalk {
    let n = d.len();
    let mut w = PictureWalk { ok: false, removed: Vec::new() };
    if n < 4 || d[0] != 0xFF || d[1] != 0xD8 {
        return w;
    }
    let mut i = 2usize;
    while i + 4 <= n {
        if d[i] != 0xFF {
            return w; // lost sync
        }
        let marker = d[i + 1];
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        if marker == 0xDA {
            w.ok = true;
            return w;
        }
        if marker == 0xD9 {
            w.removed.push(Removed { label: "bytes after the end marker", offset: i + 2, size: n - i - 2 });
            w.ok = true;
            return w;
        }
        let len = be16(d, i + 2) as usize;
        if len < 2 || i + 2 + len > n {
            return w;
        }
        if marker == 0xE1 || marker == 0xED || marker == 0xFE {
            let label = if marker == 0xE1 {
                if starts_at(d, i + 4, b"Exif\0\0") {
                    "APP1 Exif"
                } else if starts_at(d, i + 4, b"http://ns.adobe.com/xap/1.0/\0") {
                    "APP1 XMP"
                } else if starts_at(d, i + 4, b"http://ns.adobe.com/xmp/extension/\0") {
                    "APP1 XMP extension"
                } else {
                    "APP1"
                }
            } else if marker == 0xED {
                "APP13"
            } else {
                "COM"
            };
            w.removed.push(Removed { label, offset: i, size: 2 + len });
        }
        i += 2 + len;
    }
    w
}

/// strip_metadata.rs's PNG chunk walk, reporting what it drops.
fn walk_png(d: &[u8]) -> PictureWalk {
    let n = d.len() as u64;
    let mut w = PictureWalk { ok: false, removed: Vec::new() };
    if d.len() < 8 || d[..8] != [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A] {
        return w;
    }
    let mut i = 8u64;
    while i + 12 <= n {
        let total = 12 + be32(d, i as usize) as u64;
        if i + total > n {
            return w;
        }
        let name = &d[i as usize + 4..i as usize + 8];
        let label = match name {
            b"tEXt" => Some("tEXt"),
            b"zTXt" => Some("zTXt"),
            b"iTXt" => Some("iTXt"),
            b"eXIf" => Some("eXIf"),
            b"tIME" => Some("tIME"),
            _ => None,
        };
        if let Some(label) = label {
            w.removed.push(Removed { label, offset: i as usize, size: total as usize });
        }
        if name == b"IEND" {
            if i + total < n {
                w.removed.push(Removed { label: "bytes after IEND", offset: (i + total) as usize, size: (n - i - total) as usize });
            }
            w.ok = true;
            return w;
        }
        i += total;
    }
    w
}

/// strip_metadata.rs's WebP RIFF walk, reporting what it drops.
fn walk_webp(d: &[u8]) -> PictureWalk {
    let n = d.len() as u64;
    let mut w = PictureWalk { ok: false, removed: Vec::new() };
    if d.len() < 12 || &d[..4] != b"RIFF" || &d[8..12] != b"WEBP" {
        return w;
    }
    let mut i = 12u64;
    while i + 8 <= n {
        let len = le32(d, i as usize + 4) as u64;
        let padded = len + (len & 1);
        if i + 8 + padded > n && i + 8 + len != n {
            return w;
        }
        let end = (i + 8 + padded).min(n);
        if starts_at(d, i as usize, b"EXIF") || starts_at(d, i as usize, b"XMP ") {
            let label = if starts_at(d, i as usize, b"EXIF") { "EXIF" } else { "XMP" };
            w.removed.push(Removed { label, offset: i as usize, size: (end - i) as usize });
        }
        i += 8 + padded;
        if i >= n {
            break;
        }
    }
    if i < n {
        w.removed.push(Removed { label: "bytes after the last chunk", offset: i as usize, size: (n - i) as usize });
    }
    w.ok = true;
    w
}

/// Somewhere to read a video's boxes from: the bytes in memory, or a file on disk.
trait Src {
    fn len(&self) -> u64;
    /// Fill `buf` from `pos`; false when it cannot.
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> bool;
}

struct MemSrc<'a>(&'a [u8]);

impl Src for MemSrc<'_> {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> bool {
        let (p, n) = (pos as usize, buf.len());
        if pos > self.0.len() as u64 || self.0.len() - p < n {
            return false;
        }
        buf.copy_from_slice(&self.0[p..p + n]);
        true
    }
}

struct FileSrc {
    file: File,
    len: u64,
}

impl Src for FileSrc {
    fn len(&self) -> u64 {
        self.len
    }
    fn read_at(&mut self, pos: u64, buf: &mut [u8]) -> bool {
        self.file.seek(SeekFrom::Start(pos)).is_ok() && self.file.read_exact(buf).is_ok()
    }
}

/// The video strip's levels.
#[derive(Clone, Copy, PartialEq)]
enum Level {
    Top,
    Moov,
    Trak,
}

/// Which kind of box a plan neutralises (the last part of its path).
#[derive(Clone, Copy, PartialEq)]
enum Leaf {
    Udta,
    Meta,
    XmpUuid,
}

struct BoxPlan {
    leaf: Leaf,
    offset: u64,
    body_start: u64,
    end: u64,
}

#[derive(Default)]
struct Mp4Walk {
    boxes_read: u32,
    found_moov: bool,
    stopped: bool,
    plans: Vec<BoxPlan>,
    top_meta: Vec<BoxPlan>,
}

/// The XMP `uuid` box's 16-byte id (Adobe's XMP specification).
const XMP_UUID: [u8; 16] = [0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC];
/// The most box headers one walk reads (the video strip's MAX_BOXES).
const MAX_BOXES: u32 = 1_000_000;

/// One box header at `pos` on a level ending at `end`: Ok(None) for "no room", Err(()) for a size
/// that makes no sense, else (name, body start, end).
fn box_header(src: &mut dyn Src, pos: u64, end: u64) -> Result<Option<([u8; 4], u64, u64)>, ()> {
    let room = end - pos;
    if room < 8 {
        return Ok(None);
    }
    let mut h = [0u8; 16];
    let want = if room >= 16 { 16 } else { 8 };
    if !src.read_at(pos, &mut h[..want]) {
        return Err(());
    }
    let size32 = be32(&h, 0) as u64;
    let name = [h[4], h[5], h[6], h[7]];
    let (header_len, size) = if size32 == 0 {
        (8, room)
    } else if size32 == 1 {
        if room < 16 {
            return Err(());
        }
        let large = ((be32(&h, 8) as u64) << 32) | be32(&h, 12) as u64;
        if large < 16 || large > room {
            return Err(());
        }
        (16, large)
    } else if size32 < 8 {
        return Err(());
    } else {
        (8, size32)
    };
    if size > room {
        return Err(());
    }
    Ok(Some((name, pos + header_len, pos + size)))
}

fn is_xmp_uuid(src: &mut dyn Src, body_start: u64, end: u64) -> bool {
    if end - body_start < 16 {
        return false;
    }
    let mut id = [0u8; 16];
    src.read_at(body_start, &mut id) && id == XMP_UUID
}

/// The video strip's walk of one level.
fn walk_level(src: &mut dyn Src, st: &mut Mp4Walk, start: u64, end: u64, level: Level) {
    let mut pos = start;
    while pos < end {
        if st.boxes_read >= MAX_BOXES {
            st.stopped = true;
            return;
        }
        st.boxes_read += 1;
        let (nm, body_start, box_end) = match box_header(src, pos, end) {
            Ok(Some(h)) => h,
            Ok(None) => return,
            Err(()) => {
                st.stopped = true;
                return;
            }
        };
        let plan = |leaf| BoxPlan { leaf, offset: pos, body_start, end: box_end };
        if level == Level::Top && &nm == b"moov" {
            st.found_moov = true;
            walk_level(src, st, body_start, box_end, Level::Moov);
        } else if level == Level::Moov && &nm == b"trak" {
            walk_level(src, st, body_start, box_end, Level::Trak);
        } else if level == Level::Top && &nm == b"meta" {
            st.top_meta.push(plan(Leaf::Meta)); // only if the file has a movie
        } else if (level == Level::Moov || level == Level::Trak) && (&nm == b"udta" || &nm == b"meta") {
            st.plans.push(plan(if &nm == b"udta" { Leaf::Udta } else { Leaf::Meta }));
        } else if &nm == b"uuid" && is_xmp_uuid(src, body_start, box_end) {
            st.plans.push(plan(Leaf::XmpUuid));
        }
        pos = box_end;
    }
}

/// The video strip's step 1: the boxes it would neutralise, in file order.
fn walk_mp4(src: &mut dyn Src) -> Mp4Walk {
    let mut st = Mp4Walk::default();
    let len = src.len();
    walk_level(src, &mut st, 0, len, Level::Top);
    if st.found_moov {
        let top = std::mem::take(&mut st.top_meta);
        st.plans.extend(top);
    }
    st.plans.sort_by_key(|p| p.offset);
    st.plans.retain(|p| p.end <= len && p.offset + 8 <= p.body_start && p.body_start <= p.end);
    st
}

/// Look inside one neutralised box: `b[s..e]` is its body.
fn inspect_box(f: &mut Found, b: &[u8], s: usize, e: usize, leaf: Leaf, budget: &mut i64) {
    match leaf {
        Leaf::XmpUuid => inspect(f, b, s + 16, e, |f| scan_xmp(b, s + 16, e, f)),
        Leaf::Meta => inspect(f, b, s, e, |f| meta_box(b, s, e, f, budget)),
        Leaf::Udta => inspect(f, b, s, e, |f| udta_box(b, s, e, f, budget)),
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// The words
// ════════════════════════════════════════════════════════════════════════════════════════════════

#[derive(Clone, Copy, PartialEq)]
enum Medium {
    Photo,
    Video,
    Recording,
}

const ORDER: [&str; 13] = ["location", "place-name", "camera", "serial", "lens", "time", "changed-time", "person", "software", "comment", "text", "unread", "other"];

/// The camera's name from its make and model ("Apple iPhone 15 Pro"; "Canon EOS R5", not
/// "Canon Canon EOS R5").
fn camera_name(f: &Found) -> String {
    let make = js_trim(&f.make);
    let model = js_trim(&f.model);
    if make.is_empty() {
        return model.to_string();
    }
    if model.is_empty() {
        return make.to_string();
    }
    if model.to_lowercase().starts_with(&make.to_lowercase()) {
        model.to_string()
    } else {
        format!("{make} {model}")
    }
}

fn words_for(key: &str, medium: Medium, f: &Found) -> String {
    let taken = match medium {
        Medium::Photo => "taken",
        Medium::Video => "filmed",
        Medium::Recording => "recorded",
    };
    let took = match medium {
        Medium::Photo => "took",
        Medium::Video => "filmed",
        Medium::Recording => "recorded",
    };
    let device = if medium == Medium::Recording { "device" } else { "camera or phone" };
    match key {
        "location" => format!("Where it was {taken}"),
        "place-name" => "The name of a place (an address, a city or a country)".to_string(),
        "camera" => {
            let name = camera_name(f);
            let named = if name.is_empty() { String::new() } else { format!(" ({name})") };
            format!("Which {device} {took} it{named}")
        }
        "serial" => "The camera's own serial number".to_string(),
        "lens" => "Which lens was used".to_string(),
        "time" => format!("When it was {taken}"),
        "changed-time" => "When it was last changed".to_string(),
        "person" => "A person's name (who made it or who owns the camera)".to_string(),
        "software" => "Which program or app made or edited it".to_string(),
        "comment" => "A title, caption or comment".to_string(),
        "text" => "Text notes stored inside the file".to_string(),
        "unread" => format!("Packed details this app could not open (they may say where it was {taken})"),
        _ => "Other hidden details".to_string(),
    }
}

fn summary_for(medium: Medium, has_location: bool, has_camera: bool, anything: bool) -> Option<String> {
    let (noun, taken, which) = match medium {
        Medium::Photo => ("This photo", "taken", "which camera took it"),
        Medium::Video => ("This video", "filmed", "which camera filmed it"),
        Medium::Recording => ("This recording", "recorded", "which device recorded it"),
    };
    Some(if has_location && has_camera {
        format!("{noun} says where it was {taken} and {which}.")
    } else if has_location {
        format!("{noun} says where it was {taken}.")
    } else if has_camera {
        format!("{noun} says {which}.")
    } else if anything {
        format!("{noun} carries other hidden details.")
    } else {
        return None;
    })
}

fn finish(kind: &'static str, name_or_type: &str, medium: Medium, complete: bool, f: Found) -> Description {
    let details: Vec<Detail> = ORDER.iter().copied().filter(|k| f.keys.contains(k)).map(|key| Detail { key, words: words_for(key, medium, &f) }).collect();
    let has_location = f.keys.contains("location") || f.keys.contains("place-name");
    let has_camera = f.keys.contains("camera") || f.keys.contains("serial") || f.keys.contains("lens");
    Description {
        kind,
        claimed: claimed_kind(name_or_type),
        has_location,
        has_camera,
        summary: summary_for(medium, has_location, has_camera, !details.is_empty()),
        details,
        complete,
        coordinates: f.coords,
        place: f.place,
    }
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// describe
// ════════════════════════════════════════════════════════════════════════════════════════════════

/// What the hidden information in `bytes` says, in plain words (read only). `name_or_type` is the
/// file's name or type, reported as `claimed`; the bytes decide the kind.
pub fn describe(bytes: &[u8], name_or_type: &str) -> Description {
    let b = bytes;
    let mut kind = sniff_kind(b);
    let mut f = Found::default();
    let mut budget: i64 = 100_000;
    let mut complete = false;
    let mut medium = Medium::Photo;
    match kind {
        "jpeg" => {
            let w = walk_jpeg(b);
            complete = w.ok;
            for r in &w.removed {
                let (s, e) = (r.offset + 4, r.offset + r.size);
                match r.label {
                    "APP1 Exif" => inspect(&mut f, b, s, e, |f| read_tiff(b, s + 6, e, f)),
                    "APP1 XMP" | "APP1 XMP extension" => inspect(&mut f, b, s, e, |f| scan_xmp(b, s, e, f)),
                    "APP13" => inspect(&mut f, b, s, e, |f| scan_iptc(b, s, e, f)),
                    "COM" => {
                        if has_text(b, s, e) {
                            f.note("comment");
                        }
                    }
                    _ => inspect(&mut f, b, r.offset, e, |_| {}),
                }
            }
        }
        "png" => {
            let w = walk_png(b);
            complete = w.ok;
            for r in &w.removed {
                let s = r.offset + 8;
                let e = (r.offset + r.size).saturating_sub(4); // the chunk's data, without its CRC
                match r.label {
                    "eXIf" => inspect(&mut f, b, s, e, |f| read_exif_block(b, s, e, f)),
                    "tIME" => f.note("changed-time"),
                    "tEXt" | "zTXt" | "iTXt" => png_text_chunk(b, r.label, s, e, &mut f),
                    _ => inspect(&mut f, b, r.offset, r.offset + r.size, |_| {}),
                }
            }
        }
        "webp" => {
            let w = walk_webp(b);
            complete = w.ok;
            for r in &w.removed {
                let (s, e) = (r.offset + 8, r.offset + r.size);
                match r.label {
                    "EXIF" => inspect(&mut f, b, s, e, |f| read_exif_block(b, s, e, f)),
                    "XMP" => inspect(&mut f, b, s, e, |f| scan_xmp(b, s, e, f)),
                    _ => inspect(&mut f, b, r.offset, e, |_| {}),
                }
            }
        }
        "mp4" => {
            let w = walk_mp4(&mut MemSrc(b));
            complete = w.found_moov && !w.stopped;
            let brands = ftyp_brands(b, b.len() as u64);
            if !w.found_moov && brands.iter().any(|x| HEIF_BRANDS.contains(&x)) {
                kind = "heif";
            } else {
                medium = if brands.first().is_some_and(|x| AUDIO_BRANDS.contains(&x)) { Medium::Recording } else { Medium::Video };
            }
            for p in &w.plans {
                inspect_box(&mut f, b, p.body_start as usize, p.end as usize, p.leaf, &mut budget);
            }
        }
        "gif" => complete = true, // nothing standard to look for, and nothing is removed
        _ => {}
    }
    finish(kind, name_or_type, medium, complete, f)
}

/// The most a picture read from disk to be described may be. Bigger ones are reported as not read
/// through (no phone writes one; the relay refuses a picture well under this).
pub const PICTURE_READ_MAX: u64 = 64 * 1024 * 1024;

/// The most of one video metadata box read from disk. A box bigger than this (no camera writes one)
/// is looked into only as far as this.
const BOX_READ_MAX: u64 = 64 * 1024 * 1024;

/// [`describe`] for a file on disk, without loading a video: an MP4, MOV, 3GP or HEIF file is walked
/// by its box headers and only its metadata boxes are read; a picture is read whole (up to
/// [`PICTURE_READ_MAX`]); anything else is not read past its first bytes. Gives the same answer as
/// [`describe`] on the file's bytes (one test holds it to that).
pub fn describe_file(path: &Path, name_or_type: &str) -> std::io::Result<Description> {
    let mut file = File::open(path)?;
    let len = file.metadata()?.len();
    let mut head = vec![0u8; len.min(272) as usize];
    file.read_exact(&mut head)?;
    let kind = sniff_kind(&head);
    if kind != "mp4" {
        if matches!(kind, "jpeg" | "png" | "webp" | "gif") {
            if len > PICTURE_READ_MAX {
                return Ok(finish(kind, name_or_type, Medium::Photo, false, Found::default()));
            }
            let mut all = Vec::with_capacity(len as usize);
            file.seek(SeekFrom::Start(0))?;
            file.read_to_end(&mut all)?;
            return Ok(describe(&all, name_or_type));
        }
        return Ok(describe(&head, name_or_type));
    }
    let mut src = FileSrc { file, len };
    let w = walk_mp4(&mut src);
    let complete = w.found_moov && !w.stopped;
    let brands = ftyp_brands(&head, len);
    let (kind, medium) = if !w.found_moov && brands.iter().any(|x| HEIF_BRANDS.contains(&x)) {
        ("heif", Medium::Photo)
    } else {
        ("mp4", if brands.first().is_some_and(|x| AUDIO_BRANDS.contains(&x)) { Medium::Recording } else { Medium::Video })
    };
    let mut f = Found::default();
    let mut budget: i64 = 100_000;
    for p in &w.plans {
        // The box's body and one byte after it (the place box may look one byte past its end, as
        // it does in a whole file in memory).
        let want_end = (p.end + 1).min(len).min(p.body_start + BOX_READ_MAX);
        let mut body = vec![0u8; (want_end - p.body_start) as usize];
        if !src.read_at(p.body_start, &mut body) {
            continue;
        }
        let e = ((p.end - p.body_start) as usize).min(body.len());
        inspect_box(&mut f, &body, 0, e, p.leaf, &mut budget);
    }
    Ok(finish(kind, name_or_type, medium, complete, f))
}

// ════════════════════════════════════════════════════════════════════════════════════════════════
// The answer as JSON, in the web's own field names (for the agreement test and the fixtures)
// ════════════════════════════════════════════════════════════════════════════════════════════════

fn json_string(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

impl Description {
    /// The answer as JSON with the web's field names (`hasLocation`, `hasCamera`, ...), the
    /// coordinates and place included only when `with_coordinates`, as `describe(..., {coordinates:
    /// true})` gives them.
    pub fn to_json(&self, with_coordinates: bool) -> String {
        let opt = |s: Option<&str>| s.map_or("null".to_string(), json_string);
        let details: Vec<String> = self.details.iter().map(|d| format!("{{\"key\":{},\"words\":{}}}", json_string(d.key), json_string(&d.words))).collect();
        let mut out = format!(
            "{{\"kind\":{},\"claimed\":{},\"hasLocation\":{},\"hasCamera\":{},\"details\":[{}],\"summary\":{},\"complete\":{}",
            json_string(self.kind),
            opt(self.claimed),
            self.has_location,
            self.has_camera,
            details.join(","),
            opt(self.summary.as_deref()),
            self.complete
        );
        if with_coordinates {
            let coords = self.coordinates.map_or("null".to_string(), |(lat, lon)| format!("{{\"latitude\":{lat},\"longitude\":{lon}}}"));
            out.push_str(&format!(",\"coordinates\":{coords},\"place\":{}", opt(self.place.as_deref())));
        }
        out.push('}');
        out
    }
}

#[cfg(test)]
#[path = "describe_metadata_tests.rs"]
mod tests;
