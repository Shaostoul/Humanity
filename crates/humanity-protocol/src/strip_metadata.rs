//! Lossless image-metadata stripping (privacy hardening, 2026-08-23).
//!
//! A phone photo carries EXIF: GPS coordinates, capture time, camera
//! serial. Members uploading a picture of their garden should not be
//! publishing their home address to a public URL, and nobody consents to
//! a leak they don't know is happening. So the relay strips metadata from
//! every image at upload time, BEFORE the bytes ever touch disk.
//!
//! The strippers are BYTE-LEVEL and LOSSLESS: they drop whole metadata
//! segments without re-encoding pixels, so image quality is untouched.
//!
//! Coverage (the formats that realistically carry location data):
//! - JPEG: drops APP1 (EXIF + XMP), APP13 (Photoshop/IPTC — captions and
//!   location fields), and COM (comments). Keeps JFIF/ICC/Adobe segments
//!   so colors render identically.
//! - PNG: drops tEXt/zTXt/iTXt (free-text, often software + author),
//!   eXIf (embedded EXIF) and tIME. Keeps everything else, including
//!   APNG animation chunks.
//! - WebP: drops the EXIF and XMP RIFF chunks and clears their flag bits
//!   in the VP8X header; RIFF size is fixed up. Keeps ICC/alpha/anim.
//! - GIF (since 2026-11-07, docs/BUGS.md BUG-263; the operator: "Let's do that."): drops comment
//!   blocks, every application block but looping (NETSCAPE2.0 and its twin ANIMEXTS1.0) and the
//!   colour profile (ICCRGBG1 012), any extension block of a kind the GIF standard does not define,
//!   and anything after the trailer. Keeps the header, the colour tables, every frame, each frame's
//!   timing and transparency (the graphic control blocks) and the plain-text blocks, byte for byte.
//!   GIF has no standard place for a location, but an editing program can write one in: XMP goes in
//!   an application block ("XMP DataXMP"), and comments say whatever they were given.
//!
//! THE RULE every format follows, written down 2026-11-07: WHAT THE PICTURE IS MADE OF STAYS; WORDS
//! AND DATA ABOUT THE PICTURE THAT NO VIEWER IS SHOWN GO. Stays: the pixels and frames, the colour
//! tables and the colour profile (a JPEG's ICC segment, a PNG's iCCP, gAMA, cHRM and sRGB, a WebP's
//! ICCP, a GIF's ICCRGBG1 block: without them the same pixels show different colours), animation
//! and its timing and looping (APNG's chunks, WebP's ANIM/ANMF, a GIF's graphic control and
//! NETSCAPE2.0 blocks), transparency. Goes: EXIF, XMP, IPTC, Photoshop blocks, comments, text notes
//! (PNG's tEXt, zTXt, iTXt), timestamps (tIME), application blocks of programs, and bytes after the
//! end. A GIF's plain-text block is the one judgement call: the GIF standard defines it as text drawn
//! onto the picture, so it is part of what the picture is made of and stays (today's browsers no
//! longer draw it, but removing it would also move which frame the graphic control block before it
//! times, so a frame could change its delay). No program writes a location or a name into one.
//!
//! A file that does not walk cleanly is never rewritten (a guessed rewrite could corrupt it).
//! [`try_strip_image_metadata`] says so (None), and the relay REFUSES such a picture rather than
//! store it with whatever it carries (2026-11-06, docs/BUGS.md BUG-260: it used to store the
//! original bytes, location and all); the person's app asks them whether to send it anyway, and
//! only their signed yes lets it through (relay/upload_rules.rs). [`strip_image_metadata`] is the
//! fail-open form, for a caller that judges "could it be checked" separately (the desktop's own
//! clean before sending, net/file_clean.rs, which marks such a file unchecked).

/// Strip metadata from an image, dispatching on the validated
/// content-type. Non-image or unhandled types return the input unchanged,
/// and so does a picture that does not walk cleanly (see [`try_strip_image_metadata`]).
pub fn strip_image_metadata(content_type: &str, data: &[u8]) -> Vec<u8> {
    try_strip_image_metadata(content_type, data).unwrap_or_else(|| data.to_vec())
}

/// Strip metadata from an image, or None when a JPEG, PNG, WebP or GIF does not walk cleanly, so
/// its metadata could not be removed with certainty. Any other type comes back unchanged.
pub fn try_strip_image_metadata(content_type: &str, data: &[u8]) -> Option<Vec<u8>> {
    match content_type {
        "image/jpeg" => strip_jpeg(data),
        "image/png" => strip_png(data),
        "image/webp" => strip_webp(data),
        "image/gif" => strip_gif(data),
        _ => Some(data.to_vec()),
    }
}

/// The application blocks a GIF keeps, by their 11-byte name (8 letters of program, 3 of code):
/// looping, without which an animation plays once and stops (Netscape's, and ANIMEXTS1.0, the same
/// block under another name), and the colour profile (ICC's own registered name), kept for the
/// reason a JPEG's ICC segment is. Every other application block goes (XMP's "XMP DataXMP",
/// ImageMagick's IPTC and Photoshop blocks, any program's own).
pub const GIF_KEPT_APPLICATIONS: [&[u8; 11]; 3] = [b"NETSCAPE2.0", b"ANIMEXTS1.0", b"ICCRGBG1012"];

/// One block a GIF strip removes: what it is, where it starts and its size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifRemoved {
    /// "comment", "XMP", "application" (a program's own block), "extension" (a kind the standard
    /// does not define) or "bytes after the trailer".
    pub label: &'static str,
    pub offset: usize,
    pub size: usize,
}

/// A GIF's block walk: the byte ranges that make the cleaned file, in order, and what was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifWalk {
    pub keep: Vec<(usize, usize)>,
    pub removed: Vec<GifRemoved>,
}

/// The end of a run of data sub-blocks starting at `i` (each a length byte, then that many bytes,
/// until a length of zero): the index just past the zero. None when the data ends first.
fn gif_sub_blocks_end(d: &[u8], mut i: usize) -> Option<usize> {
    loop {
        let len = *d.get(i)? as usize;
        i += 1;
        if len == 0 {
            return Some(i);
        }
        i += len;
        if i > d.len() {
            return None;
        }
    }
}

/// Walk a GIF block by block (the GIF89a specification's grammar), deciding each block by the
/// module's rule. None when the stream does not parse: a block runs past the end, or a byte where a
/// block should start is not one. A file that ends after a whole block without its trailer is
/// walked (every byte was read; nothing is added to it).
pub fn walk_gif(d: &[u8]) -> Option<GifWalk> {
    if d.len() < 13 || !(d.starts_with(b"GIF87a") || d.starts_with(b"GIF89a")) {
        return None;
    }
    // The header, the logical screen descriptor and the global colour table, if it has one.
    let packed = d[10];
    let mut i = 13usize;
    if packed & 0x80 != 0 {
        i += 3usize << ((packed & 0x07) + 1);
    }
    if i > d.len() {
        return None;
    }
    let mut w = GifWalk { keep: vec![(0, i)], removed: Vec::new() };
    loop {
        if i == d.len() {
            return Some(w); // ended after a whole block, with no trailer
        }
        match d[i] {
            0x3B => {
                w.keep.push((i, i + 1));
                if i + 1 < d.len() {
                    w.removed.push(GifRemoved { label: "bytes after the trailer", offset: i + 1, size: d.len() - i - 1 });
                }
                return Some(w);
            }
            0x2C => {
                // An image: its descriptor, its own colour table, the LZW code size, its data.
                if i + 10 > d.len() {
                    return None;
                }
                let local = d[i + 9];
                let mut j = i + 10;
                if local & 0x80 != 0 {
                    j += 3usize << ((local & 0x07) + 1);
                }
                j += 1; // the LZW minimum code size
                if j > d.len() {
                    return None;
                }
                let end = gif_sub_blocks_end(d, j)?;
                w.keep.push((i, end));
                i = end;
            }
            0x21 => {
                let label = *d.get(i + 1)?;
                let end = gif_sub_blocks_end(d, i + 2)?;
                let gone = match label {
                    0xF9 | 0x01 => None, // graphic control (timing, transparency); plain text (drawn)
                    0xFE => Some("comment"),
                    0xFF => {
                        let name = if d[i + 2] == 11 && i + 14 <= end { d.get(i + 3..i + 14) } else { None };
                        match name {
                            Some(n) if GIF_KEPT_APPLICATIONS.iter().any(|k| &k[..] == n) => None,
                            Some(b"XMP DataXMP") => Some("XMP"),
                            _ => Some("application"),
                        }
                    }
                    _ => Some("extension"),
                };
                match gone {
                    None => w.keep.push((i, end)),
                    Some(label) => w.removed.push(GifRemoved { label, offset: i, size: end - i }),
                }
                i = end;
            }
            _ => return None,
        }
    }
}

/// GIF block walk: the kept blocks, in order. Returns None if the stream doesn't parse.
fn strip_gif(data: &[u8]) -> Option<Vec<u8>> {
    let w = walk_gif(data)?;
    let mut out = Vec::with_capacity(data.len());
    for (s, e) in &w.keep {
        out.extend_from_slice(&data[*s..*e]);
    }
    Some(out)
}

/// JPEG marker walk. Returns None if the stream doesn't parse.
fn strip_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return None;
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(&data[..2]); // SOI
    let mut i = 2usize;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            return None; // lost sync
        }
        let marker = data[i + 1];
        // Standalone markers without a length (RSTn, TEM) shouldn't appear
        // between header segments, but tolerate them.
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            out.extend_from_slice(&data[i..i + 2]);
            i += 2;
            continue;
        }
        // Start of scan: entropy-coded data follows until EOI. Copy the
        // rest verbatim — metadata never lives after SOS in practice, and
        // walking entropy data is where rewriters corrupt files.
        if marker == 0xDA {
            out.extend_from_slice(&data[i..]);
            return Some(out);
        }
        if marker == 0xD9 {
            out.extend_from_slice(&data[i..i + 2]); // EOI
            return Some(out);
        }
        let len = ((data[i + 2] as usize) << 8) | data[i + 3] as usize;
        if len < 2 || i + 2 + len > data.len() {
            return None;
        }
        let drop = matches!(marker,
            0xE1 |   // APP1: EXIF + XMP (the GPS lives here)
            0xED |   // APP13: Photoshop IRB / IPTC (captions, location)
            0xFE     // COM: comments
        );
        if !drop {
            out.extend_from_slice(&data[i..i + 2 + len]);
        }
        i += 2 + len;
    }
    None
}

/// PNG chunk walk. Returns None if the stream doesn't parse.
fn strip_png(data: &[u8]) -> Option<Vec<u8>> {
    const SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if data.len() < 8 || &data[..8] != SIG {
        return None;
    }
    let mut out = Vec::with_capacity(data.len());
    out.extend_from_slice(SIG);
    let mut i = 8usize;
    while i + 12 <= data.len() {
        let len = u32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]) as usize;
        let name = &data[i + 4..i + 8];
        let total = 12 + len; // len + name + data + crc
        if i + total > data.len() {
            return None;
        }
        let drop = matches!(name, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME");
        if !drop {
            out.extend_from_slice(&data[i..i + total]);
        }
        if name == b"IEND" {
            return Some(out);
        }
        i += total;
    }
    None
}

/// WebP RIFF walk: drop EXIF/XMP chunks, clear their VP8X flag bits,
/// fix up the RIFF size. Returns None if the container doesn't parse.
fn strip_webp(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return None;
    }
    let mut body: Vec<u8> = Vec::with_capacity(data.len());
    let mut i = 12usize;
    while i + 8 <= data.len() {
        let name = &data[i..i + 4];
        let len = u32::from_le_bytes([data[i + 4], data[i + 5], data[i + 6], data[i + 7]]) as usize;
        let padded = len + (len & 1); // chunks are 2-byte aligned
        if i + 8 + padded > data.len() {
            // Tolerate a final unpadded chunk at EOF.
            if i + 8 + len != data.len() {
                return None;
            }
        }
        let end = (i + 8 + padded).min(data.len());
        let drop = name == b"EXIF" || name == b"XMP ";
        if !drop {
            let start = body.len();
            body.extend_from_slice(&data[i..end]);
            // Clear the EXIF (bit 3) and XMP (bit 2) flags in VP8X so
            // readers don't look for chunks that are no longer there.
            if name == b"VP8X" && body.len() > start + 8 {
                body[start + 8] &= !(1 << 3);
                body[start + 8] &= !(1 << 2);
            }
        }
        i += 8 + padded;
        if i >= data.len() {
            break;
        }
    }
    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(body.len() as u32 + 4).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal real JPEG via the image crate, splice an EXIF APP1
    /// segment in, strip, and prove: EXIF gone + the image still DECODES.
    #[test]
    fn jpeg_exif_removed_and_still_decodes() {
        // A real 8x8 JPEG.
        let img = image::RgbImage::from_fn(8, 8, |x, y| image::Rgb([x as u8 * 30, y as u8 * 30, 128]));
        let mut jpeg: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
            .unwrap();
        // Splice a fake EXIF APP1 with "GPS" bytes right after SOI.
        let exif_payload = b"Exif\0\0FAKE-GPS-COORDINATES-12.34,56.78";
        let mut app1 = vec![0xFF, 0xE1];
        app1.extend_from_slice(&(((exif_payload.len() + 2) as u16).to_be_bytes()));
        app1.extend_from_slice(exif_payload);
        let mut tagged = jpeg[..2].to_vec();
        tagged.extend_from_slice(&app1);
        tagged.extend_from_slice(&jpeg[2..]);
        assert!(find(&tagged, b"GPS-COORDINATES").is_some(), "test setup: EXIF present");

        let stripped = strip_image_metadata("image/jpeg", &tagged);
        assert!(find(&stripped, b"GPS-COORDINATES").is_none(), "EXIF must be gone");
        assert!(stripped.len() < tagged.len());
        // Integrity: the stripped bytes still decode to the same-size image.
        let decoded = image::load_from_memory(&stripped).expect("stripped JPEG must decode");
        assert_eq!((decoded.width(), decoded.height()), (8, 8));
    }

    #[test]
    fn png_text_and_exif_chunks_removed_and_still_decodes() {
        let img = image::RgbaImage::from_fn(4, 4, |x, _| image::Rgba([x as u8 * 60, 0, 0, 255]));
        let mut png: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        // Splice a tEXt chunk (with CRC — decoders that check CRCs only
        // check chunks they read; we drop it anyway) before IEND.
        let iend = find(&png, b"IEND").unwrap() - 4;
        let payload = b"Author\0secret location: my house";
        let mut chunk = (payload.len() as u32).to_be_bytes().to_vec();
        chunk.extend_from_slice(b"tEXt");
        chunk.extend_from_slice(payload);
        chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
        let mut tagged = png[..iend].to_vec();
        tagged.extend_from_slice(&chunk);
        tagged.extend_from_slice(&png[iend..]);
        assert!(find(&tagged, b"secret location").is_some());

        let stripped = strip_image_metadata("image/png", &tagged);
        assert!(find(&stripped, b"secret location").is_none(), "tEXt must be gone");
        let decoded = image::load_from_memory(&stripped).expect("stripped PNG must decode");
        assert_eq!((decoded.width(), decoded.height()), (4, 4));
    }

    #[test]
    fn webp_exif_chunk_removed() {
        // Hand-built minimal WebP container: VP8X (with EXIF flag) + a
        // fake image chunk + an EXIF chunk. We only test container
        // surgery here (a real VP8 bitstream isn't needed to prove the
        // chunk walk drops EXIF and clears the flag).
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(b"VP8X");
        body.extend_from_slice(&10u32.to_le_bytes());
        body.extend_from_slice(&[0b0000_1000, 0, 0, 0, 3, 0, 0, 3, 0, 0]); // EXIF flag set, 4x4
        body.extend_from_slice(b"VP8L");
        body.extend_from_slice(&4u32.to_le_bytes());
        body.extend_from_slice(&[0x2F, 0, 0, 0]);
        body.extend_from_slice(b"EXIF");
        let exif = b"GPS 12.34,56.78 here";
        body.extend_from_slice(&(exif.len() as u32).to_le_bytes());
        body.extend_from_slice(exif);
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&(body.len() as u32 + 4).to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(&body);

        let stripped = strip_image_metadata("image/webp", &webp);
        assert!(find(&stripped, b"GPS 12.34").is_none(), "EXIF chunk must be gone");
        assert!(find(&stripped, b"VP8L").is_some(), "image chunk must survive");
        let vp8x = find(&stripped, b"VP8X").unwrap();
        assert_eq!(stripped[vp8x + 8] & (1 << 3), 0, "EXIF flag must be cleared");
        // RIFF size must match the rewritten payload.
        let riff_len = u32::from_le_bytes([stripped[4], stripped[5], stripped[6], stripped[7]]) as usize;
        assert_eq!(riff_len + 8, stripped.len());
    }

    /// A clean image without metadata passes through byte-identical.
    #[test]
    fn clean_images_unchanged() {
        let img = image::RgbImage::from_fn(4, 4, |_, _| image::Rgb([1, 2, 3]));
        let mut png: Vec<u8> = Vec::new();
        image::DynamicImage::ImageRgb8(img)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        // The image crate may itself emit no ancillary text chunks; the
        // walk should keep every remaining chunk verbatim.
        let stripped = strip_image_metadata("image/png", &png);
        let decoded = image::load_from_memory(&stripped).expect("must decode");
        assert_eq!((decoded.width(), decoded.height()), (4, 4));
    }

    /// A JPEG that carries a location and does not walk cleanly (its EXIF segment says it is longer
    /// than the file, as a truncated or damaged photo does): `try_strip_image_metadata` says it could
    /// not clean it (None), so the relay refuses it instead of storing the location (BUG-260);
    /// `strip_image_metadata` still hands it back unchanged for callers that judge it separately.
    #[test]
    fn a_picture_that_does_not_walk_cannot_be_cleaned() {
        let exif_payload = b"Exif\0\0GPS-COORDINATES-47.61,-122.33";
        let mut broken = vec![0xFF, 0xD8, 0xFF, 0xE1];
        broken.extend_from_slice(&0x4000u16.to_be_bytes()); // longer than the file
        broken.extend_from_slice(exif_payload);
        assert_eq!(try_strip_image_metadata("image/jpeg", &broken), None);
        assert_eq!(strip_image_metadata("image/jpeg", &broken), broken, "the fail-open form hands it back");
        assert_eq!(try_strip_image_metadata("image/png", b"\x89PNG\r\n\x1a\nshort"), None);
        assert_eq!(try_strip_image_metadata("image/gif", b"GIF89a..."), None, "a GIF that does not walk cannot be cleaned either");
    }

    /// Garbage input fails open (returned unchanged, never panics).
    #[test]
    fn garbage_fails_open() {
        let junk = vec![0xFF, 0xD8, 0x00, 0x01, 0x02];
        assert_eq!(strip_image_metadata("image/jpeg", &junk), junk);
        assert_eq!(strip_image_metadata("image/png", b"not a png"), b"not a png".to_vec());
        assert_eq!(strip_image_metadata("image/webp", b"RIFFxxxx"), b"RIFFxxxx".to_vec());
        assert_eq!(strip_image_metadata("image/gif", b"GIF89a\x01"), b"GIF89a\x01".to_vec());
    }

    /// A real looping animated GIF, three frames, from the image crate's own encoder (which writes
    /// the NETSCAPE2.0 looping block and a graphic control block before each frame).
    fn looping_gif() -> Vec<u8> {
        use image::codecs::gif::{GifEncoder, Repeat};
        let mut out = Vec::new();
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.set_repeat(Repeat::Infinite).unwrap();
            for k in 0..3u8 {
                let img = image::RgbaImage::from_fn(6, 4, |x, y| image::Rgba([x as u8 * 40 + k * 20, y as u8 * 60, 200 - k * 50, 255]));
                let frame = image::Frame::from_parts(img, 0, 0, image::Delay::from_numer_denom_ms(120 + k as u32 * 10, 1));
                enc.encode_frame(frame).unwrap();
            }
        }
        out
    }

    /// A GIF extension block: introducer, label, then `data` as sub-blocks of at most 255 bytes.
    fn gif_extension(label: u8, data: &[u8]) -> Vec<u8> {
        let mut b = vec![0x21, label];
        for part in data.chunks(255) {
            b.push(part.len() as u8);
            b.extend_from_slice(part);
        }
        b.push(0);
        b
    }

    /// An application block named `name` (11 bytes) carrying `data` as sub-blocks.
    fn gif_application(name: &[u8; 11], data: &[u8]) -> Vec<u8> {
        let mut b = vec![0x21, 0xFF, 11];
        b.extend_from_slice(name);
        let rest = gif_extension(0, data);
        b.extend_from_slice(&rest[2..]);
        b
    }

    /// XMP as Adobe's XMP specification (part 3) puts it in a GIF: the packet's raw bytes right after
    /// the name, then the 258-byte "magic trailer" that makes them read as sub-blocks.
    fn gif_xmp(packet: &str) -> Vec<u8> {
        let mut b = vec![0x21, 0xFF, 11];
        b.extend_from_slice(b"XMP DataXMP");
        b.extend_from_slice(packet.as_bytes());
        b.push(0x01);
        b.extend((0..=255u8).rev());
        b.push(0x00);
        b
    }

    /// Where the blocks start in a GIF: just past its header, screen descriptor and colour table.
    fn gif_blocks_start(g: &[u8]) -> usize {
        let packed = g[10];
        13 + if packed & 0x80 != 0 { 3usize << ((packed & 7) + 1) } else { 0 }
    }

    /// Every frame of a GIF, decoded: (its pixels, its delay).
    fn gif_frames(g: &[u8]) -> Vec<(Vec<u8>, (u32, u32))> {
        use image::AnimationDecoder;
        let dec = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(g)).expect("a GIF the decoder opens");
        dec.into_frames().map(|f| f.expect("a frame decodes")).map(|f| (f.buffer().to_vec(), f.delay().numer_denom_ms())).collect()
    }

    /// A LOOPING ANIMATED GIF LOSES ITS COMMENT AND ITS XMP, AND KEEPS THE LOOP AND EVERY FRAME BYTE
    /// FOR BYTE (2026-11-07, BUG-263). Into a real three-frame looping GIF go: a comment naming a
    /// place, an XMP block with GPS coordinates, ImageMagick's IPTC block, an extension of a kind
    /// the standard does not define, and words after the trailer. The strip gives back exactly the
    /// encoder's own bytes (so the NETSCAPE2.0 loop, the graphic control blocks and the frames are
    /// untouched), every frame decodes to the same pixels and delay, and a second strip changes
    /// nothing. A colour profile block and a plain-text block stay (the module's rule).
    ///
    /// Seen red 2026-11-07 with the "image/gif" arm out of `try_strip_image_metadata` (a GIF came back
    /// as it was): "the comment is gone" failed.
    #[test]
    fn a_looping_gif_loses_its_comment_and_xmp_and_keeps_the_loop_and_frames_byte_for_byte() {
        let clean = looping_gif();
        assert!(find(&clean, b"NETSCAPE2.0").is_some(), "test setup: the encoder wrote the looping block");
        let at = gif_blocks_start(&clean);
        let mut tagged = clean[..at].to_vec();
        tagged.extend(gif_extension(0xFE, b"Taken at 12 Garden Lane, Springfield"));
        tagged.extend(gif_xmp(r#"<x:xmpmeta><rdf:Description exif:GPSLatitude="47,36.6N" exif:GPSLongitude="122,19.8W"/></x:xmpmeta>"#));
        tagged.extend_from_slice(&clean[at..clean.len() - 1]); // the blocks, without the trailer
        tagged.extend(gif_application(b"MGKIPTC0000", b"\x1c\x02\x5a\x00\x06Lisbon"));
        tagged.extend(gif_extension(0x7A, b"a kind no standard defines"));
        tagged.push(0x3B);
        tagged.extend_from_slice(b"hidden after the end");
        assert!(find(&tagged, b"Garden Lane").is_some() && find(&tagged, b"GPSLatitude").is_some(), "test setup: the details are in it");
        // (The image crate's decoder refuses the tagged file outright, "unknown extension block
        // encountered", at the extension of a kind no standard defines; browsers step over it.)

        let stripped = try_strip_image_metadata("image/gif", &tagged).expect("a GIF that walks is cleaned");
        assert!(find(&stripped, b"Garden Lane").is_none(), "the comment is gone");
        assert!(find(&stripped, b"GPSLatitude").is_none(), "the XMP is gone");
        assert!(find(&stripped, b"Lisbon").is_none() && find(&stripped, b"no standard").is_none(), "the other blocks are gone");
        assert!(find(&stripped, b"after the end").is_none(), "nothing after the trailer");
        assert_eq!(stripped, clean, "the loop, the timing and every frame stay byte for byte");
        assert_eq!(gif_frames(&stripped), gif_frames(&clean));
        assert_eq!(try_strip_image_metadata("image/gif", &stripped).unwrap(), stripped, "a second strip changes nothing");

        // The rule's two keeps: the colour profile and a plain-text block (drawn on the picture).
        let mut kept = clean[..at].to_vec();
        let icc = gif_application(b"ICCRGBG1012", b"an ICC colour profile");
        // A plain-text block: its 12-byte header (grid place and size, cell size, colours), then its text.
        let text = [&[0x21u8, 0x01, 12][..], &[0, 0, 0, 0, 6, 0, 4, 0, 3, 4, 1, 0][..], &[2, b'H', b'i', 0][..]].concat();
        kept.extend_from_slice(&icc);
        kept.extend_from_slice(&text);
        kept.extend_from_slice(&clean[at..]);
        assert_eq!(try_strip_image_metadata("image/gif", &kept).unwrap(), kept, "the colour profile and a plain-text block stay");
    }

    /// A GIF THAT DOES NOT WALK CANNOT BE CLEANED (BUG-260's rule, 2026-11-07): cut off inside a frame,
    /// a byte where a block should start that is not one, an application block's sub-blocks running
    /// past the end. `try_strip_image_metadata` says None (the relay refuses it; the apps ask), the
    /// fail-open form hands it back. One that ends after a whole block with no trailer was read
    /// through, so it is cleaned as it is (nothing added). Seen red 2026-11-07 with the "image/gif"
    /// arm out: "cut off inside a frame" came back Some.
    #[test]
    fn a_gif_that_does_not_walk_cannot_be_cleaned() {
        let clean = looping_gif();
        let at = gif_blocks_start(&clean);
        let cut = &clean[..clean.len() - 6];
        assert_eq!(try_strip_image_metadata("image/gif", cut), None, "cut off inside a frame");
        assert_eq!(strip_image_metadata("image/gif", cut), cut.to_vec(), "the fail-open form hands it back");
        let mut bad = clean[..at].to_vec();
        bad.push(0x99);
        bad.extend_from_slice(&clean[at..]);
        assert_eq!(try_strip_image_metadata("image/gif", &bad), None, "not a block");
        let mut long = clean[..at].to_vec();
        long.extend_from_slice(&[0x21, 0xFE, 200, b'x', b'y']);
        assert_eq!(try_strip_image_metadata("image/gif", &long), None, "a comment running past the end");
        let mut no_trailer = clean[..clean.len() - 1].to_vec();
        no_trailer.extend(gif_extension(0xFE, b"note"));
        assert_eq!(try_strip_image_metadata("image/gif", &no_trailer).unwrap(), clean[..clean.len() - 1].to_vec(), "no trailer, read through: cleaned");
        assert_eq!(try_strip_image_metadata("image/gif", b"GIF89a\x01\x00\x01\x00"), None, "a screen descriptor cut short");
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }

    /// Tiny CRC32 (IEEE) for building a valid PNG test chunk.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in bytes {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
}
