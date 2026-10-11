//! Cleaning a file on the sender's own device, before it leaves (docs/design/media-showcase.md,
//! "Files on the sender's device and in messages, exact", parts 1 and 2; the operator's answers 4
//! and 5 of 2026-10-11).
//!
//! WHY HERE: the relay strips public uploads, but a file sent in a direct message or a group only
//! ever reaches it as a sealed blob it can never open, so until now a photo sent in a DM kept where
//! it was taken. The sender's app is the one place that sees every file in the clear, so it cleans
//! every file on every upload path, with the relay's own Rust strips (`strip_metadata`,
//! `strip_video_metadata`, `strip_heif_metadata`), so the rules are the server's to the byte. The
//! relay still strips public uploads as a backstop; a second pass changes nothing.
//!
//! A FILE THAT COULD NOT BE CHECKED: a picture whose structure cannot be walked (the strip gives it
//! back as it came), a video whose boxes stop making sense, a video cut off before its movie box, a
//! HEIC or AVIF whose item tables cannot be read with certainty, or a file named as one of these
//! whose bytes are not one. The verdict is the relay's own (relay/upload_rules.rs
//! `finish_file_as`), so a file the app sends to a public place as cleaned is one the server takes.
//! In a private conversation the person may send it anyway, and its marker says so (`unchecked`).
//!
//! [`look_file`] is what the attach step shows before anything is sent: what the hidden
//! information says (the shared `describe`) and whether the file could be checked. It reads a video
//! by its box headers only. [`clean_bytes`] cleans a file held in memory (a chat file, read on the
//! upload's worker); [`clean_copy`] cleans a COPY of a big file on disk (an Add to Media upload),
//! never the person's own file.

use std::path::{Path, PathBuf};

use crate::relay::core::describe_metadata::{self, Description};
use crate::relay::core::{strip_heif_metadata as heif, strip_metadata as pictures, strip_video_metadata as video};

/// What the attach step shows about a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Look {
    /// What its hidden information says (the same words as the web's attach step).
    pub description: Description,
    /// It could not be checked for where it was taken (answer 5).
    pub unchecked: bool,
}

/// What a clean did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Cleaned {
    /// Something was removed.
    pub changed: bool,
    /// It could not be checked all the way through: where it was taken may still be in it.
    pub unchecked: bool,
}

/// A file in the MP4 family with no movie in it (a HEIC or AVIF photo, or a video cut off before
/// its movie box) is read into memory to be checked only up to this size; a bigger one counts as
/// not checked (a photo is never this big; a video with no movie box is unplayable anyway).
pub const NO_MOVIE_READ_MAX: u64 = 256 * 1024 * 1024;

/// The type the picture strip takes for a kind of picture it cleans.
fn picture_type(kind: &str) -> Option<&'static str> {
    match kind {
        "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// The web's rule for a file whose bytes are none of the kinds cleaned: it could not be checked when
/// its name says it is one of them (a ".jpg" that is not a JPEG), and is fine otherwise (a PDF).
fn named_as_cleanable(name: &str) -> bool {
    matches!(describe_metadata::claimed_kind(name), Some("jpeg" | "png" | "webp" | "mp4" | "heif"))
}

/// The video strip, then for a file with no movie the HEIF strip, on bytes in memory; the verdict is
/// the relay's `finish_file_as`.
fn clean_boxes(bytes: &mut [u8]) -> Cleaned {
    let Ok(v) = video::strip_video_metadata_in_buffer(bytes) else { return Cleaned { changed: false, unchecked: true } };
    if v.found_moov {
        return Cleaned { changed: v.changed(), unchecked: !v.stopped_early.is_empty() };
    }
    match heif::strip_heif_metadata_in_buffer(bytes) {
        Ok(h) if h.kind == heif::HeifKind::Still => Cleaned { changed: v.changed() || h.changed(), unchecked: !h.complete() || !v.stopped_early.is_empty() },
        // No movie and not a HEIF photo: a video cut off before its movie box.
        Ok(_) | Err(_) => Cleaned { changed: v.changed(), unchecked: true },
    }
}

/// Clean a file held in memory, named `name`: what leaves the device, and what was done. A picture
/// may come back shorter; a video or a HEIF photo keeps its length (its metadata boxes are zeroed in
/// place).
pub fn clean_bytes(mut bytes: Vec<u8>, name: &str) -> (Vec<u8>, Cleaned) {
    let kind = describe_metadata::sniff_kind(&bytes);
    if let Some(ct) = picture_type(kind) {
        // The picture strip falls back to the original bytes when it cannot walk a picture; the
        // describe walks it the same way and says so (`complete`).
        let walked = describe_metadata::describe(&bytes, name).complete;
        let out = pictures::strip_image_metadata(ct, &bytes);
        let changed = out != bytes;
        return (out, Cleaned { changed, unchecked: !walked });
    }
    match kind {
        "gif" | "webm" => (bytes, Cleaned::default()),
        "mp4" => {
            let c = clean_boxes(&mut bytes);
            (bytes, c)
        }
        _ => (bytes, Cleaned { changed: false, unchecked: named_as_cleanable(name) }),
    }
}

/// What the attach step shows for a file held in memory (a pasted picture, the tests): a dry run of
/// [`clean_bytes`] on a copy, so the two never disagree.
pub fn look_bytes(bytes: &[u8], name: &str) -> Look {
    let description = describe_metadata::describe(bytes, name);
    let (_, c) = clean_bytes(bytes.to_vec(), name);
    Look { description, unchecked: c.unchecked }
}

/// Read a whole file, when it is no bigger than `max`.
fn read_bounded(path: &Path, max: u64) -> std::io::Result<Option<Vec<u8>>> {
    if std::fs::metadata(path)?.len() > max {
        return Ok(None);
    }
    std::fs::read(path).map(Some)
}

/// What the attach step shows for the file at `path`, named `name`. A video is read by its box
/// headers only; a picture whole (up to the describe's own limit); a HEIC whole (up to
/// [`NO_MOVIE_READ_MAX`]) to run the HEIF strip's check on a copy in memory; anything else not past
/// its first bytes.
pub fn look_file(path: &Path, name: &str) -> std::io::Result<Look> {
    let description = describe_metadata::describe_file(path, name)?;
    let unchecked = match description.kind {
        "jpeg" | "png" | "webp" => !description.complete,
        "gif" | "webm" => false,
        // A movie read all the way through: the strip walks the same boxes the same way.
        "mp4" if description.complete => false,
        // A movie whose boxes stop early, or a file in the family with no movie and no HEIF brand.
        "mp4" => true,
        "heif" => match read_bounded(path, NO_MOVIE_READ_MAX)? {
            Some(mut bytes) => clean_boxes(&mut bytes).unchecked,
            None => true,
        },
        _ => named_as_cleanable(name),
    };
    Ok(Look { description, unchecked })
}

/// Where cleaned copies of big files wait to be uploaded: beside the DM history, in the app's own
/// data folder. A copy stays only while its upload is unfinished (the Media page sweeps the rest).
pub fn cleaned_dir() -> PathBuf {
    let dms = crate::net::dm_store::DmStore::store_dir();
    dms.parent().map(|p| p.join("cleaned-uploads")).unwrap_or_else(|| PathBuf::from("cleaned-uploads"))
}

/// A new file name in `dir` nobody else uses.
fn fresh_path(dir: &Path) -> PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let mut salt = [0u8; 8];
    getrandom::getrandom(&mut salt).ok();
    let salt: String = salt.iter().map(|b| format!("{b:02x}")).collect();
    dir.join(format!("{nanos:x}-{salt}.clean"))
}

/// Clean a COPY of the big file at `path` into `dir`, never touching the person's own file. Returns
/// the copy, or None when there is nothing to remove (upload the file itself) or it could not be
/// checked (`unchecked`: the caller decides; nothing is copied). A picture is read and cleaned in
/// memory; a video or a HEIC is copied whole, then its metadata boxes are zeroed in place in the copy.
pub fn clean_copy(path: &Path, name: &str, dir: &Path) -> std::io::Result<(Option<PathBuf>, Cleaned)> {
    let look = look_file(path, name)?;
    if look.unchecked {
        return Ok((None, Cleaned { changed: false, unchecked: true }));
    }
    std::fs::create_dir_all(dir)?;
    let kind = look.description.kind;
    if picture_type(kind).is_some() {
        let Some(bytes) = read_bounded(path, describe_metadata::PICTURE_READ_MAX)? else {
            return Ok((None, Cleaned { changed: false, unchecked: true }));
        };
        let (out, c) = clean_bytes(bytes, name);
        if !c.changed {
            return Ok((None, c));
        }
        let to = fresh_path(dir);
        std::fs::write(&to, &out)?;
        return Ok((Some(to), c));
    }
    if !matches!(kind, "mp4" | "heif") || (look.description.complete && look.description.details.is_empty()) {
        // Nothing in it to remove (or not a kind that carries any): the file goes as it is.
        return Ok((None, Cleaned::default()));
    }
    let to = fresh_path(dir);
    std::fs::copy(path, &to)?;
    let result = (|| -> std::io::Result<Cleaned> {
        let v = video::strip_video_metadata_in_place(&to)?;
        if v.found_moov {
            return Ok(Cleaned { changed: v.changed(), unchecked: !v.stopped_early.is_empty() });
        }
        let h = heif::strip_heif_metadata_in_place(&to)?;
        Ok(Cleaned { changed: v.changed() || h.changed(), unchecked: h.kind != heif::HeifKind::Still || !h.complete() || !v.stopped_early.is_empty() })
    })();
    match result {
        Ok(c) if c.changed && !c.unchecked => Ok((Some(to), c)),
        Ok(c) => {
            let _ = std::fs::remove_file(&to);
            Ok((None, c))
        }
        Err(e) => {
            let _ = std::fs::remove_file(&to);
            Err(e)
        }
    }
}

/// Remove the cleaned copies in `dir` that nothing uses any more: not in `in_use`, and not touched
/// for `idle` (so a copy being made, whose upload has not said where it is yet, is never taken).
pub fn sweep_cleaned(dir: &Path, in_use: &[PathBuf], idle: std::time::Duration) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let now = std::time::SystemTime::now();
    let mut removed = 0;
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) != Some("clean") || in_use.iter().any(|u| u == &p) {
            continue;
        }
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| now.duration_since(t).ok()).is_some_and(|age| age >= idle);
        if old && std::fs::remove_file(&p).is_ok() {
            removed += 1;
        }
    }
    removed
}

#[cfg(test)]
#[path = "file_clean_tests.rs"]
mod tests;
