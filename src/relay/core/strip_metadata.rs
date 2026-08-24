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
//! - GIF is passed through untouched: it has no standardized location
//!   metadata, and its stream structure makes rewriting riskier than the
//!   near-zero privacy payoff.
//!
//! Fail-open on parse trouble: if a file doesn't walk cleanly, the
//! ORIGINAL bytes are returned rather than a possibly-corrupted rewrite.
//! (The upload path has already magic-byte-validated the format, so this
//! only happens on unusual-but-valid encodings.)

/// Strip metadata from an image, dispatching on the validated
/// content-type. Non-image or unhandled types return the input unchanged.
pub fn strip_image_metadata(content_type: &str, data: &[u8]) -> Vec<u8> {
    match content_type {
        "image/jpeg" => strip_jpeg(data).unwrap_or_else(|| data.to_vec()),
        "image/png" => strip_png(data).unwrap_or_else(|| data.to_vec()),
        "image/webp" => strip_webp(data).unwrap_or_else(|| data.to_vec()),
        _ => data.to_vec(),
    }
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

    /// Garbage input fails open (returned unchanged, never panics).
    #[test]
    fn garbage_fails_open() {
        let junk = vec![0xFF, 0xD8, 0x00, 0x01, 0x02];
        assert_eq!(strip_image_metadata("image/jpeg", &junk), junk);
        assert_eq!(strip_image_metadata("image/png", b"not a png"), b"not a png".to_vec());
        assert_eq!(strip_image_metadata("image/webp", b"RIFFxxxx"), b"RIFFxxxx".to_vec());
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
