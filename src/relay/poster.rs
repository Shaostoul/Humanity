//! A video's poster picture, the relay half (2026-11-06; docs/design/media-player.md, "Rung 2: the
//! poster picture"). The app that posts a video in chat makes one small still of it and sends it in
//! the same `POST /api/upload`, as a form part named [`POSTER_FIELD`] before the `file` part
//! (api.rs `upload_file_in`). This module holds what such a picture must be and the check that
//! holds it to that; where it is kept and how it leaves with its video is storage/files.rs
//! (`poster_name`, `Folders::delete`, `Folders::move_to`) and files.rs (`store_upload`,
//! `delete_pruned`).
//!
//! A poster picture is refused, with words saying why, when it is not a JPEG, when it does not
//! decode, when either side is over [`POSTER_MAX_SIDE`] pixels, when it is over
//! [`POSTER_MAX_BYTES`], when a second one is sent, when it comes with anything but a video, and
//! when it comes with a file from a private conversation (an `encrypted=1` upload is ciphertext:
//! a plain picture of it, at an address anyone can fetch, would show what the sealed file holds).
//! It is cleaned of its hidden information like every picture, always: the app made it, so it
//! holds nothing of the person's to keep.

use axum::http::StatusCode;

type Refused = (StatusCode, String);

/// The form part a video's poster picture comes in, before the `file` part.
pub const POSTER_FIELD: &str = "poster";
/// The most a poster picture may be. The apps aim under 96 KB (a few tens of KB is usual for a
/// 640-pixel frame); this leaves room for a busy one without letting a picture grow into a file.
pub const POSTER_MAX_BYTES: usize = 128 * 1024;
/// The most pixels either side of a poster picture may be.
pub const POSTER_MAX_SIDE: u32 = 640;

/// What is said for a poster picture over [`POSTER_MAX_BYTES`].
pub fn too_large(len: usize) -> Refused {
    (
        StatusCode::PAYLOAD_TOO_LARGE,
        format!("The video's poster picture is {} KB; one can be at most {} KB.", len.div_ceil(1024), POSTER_MAX_BYTES / 1024),
    )
}

/// What is said for a poster picture sent with a file from a private conversation.
pub const PRIVATE_REFUSED: &str =
    "A file sent in a private conversation cannot carry a poster picture: the picture would be readable by anyone with its address while the file is sealed.";
/// What is said for a poster picture sent with anything but a video.
pub const NOT_A_VIDEO: &str = "Only a video (MP4 or WebM) can carry a poster picture.";
/// What is said for a second poster picture in one upload.
pub const TWO_POSTERS: &str = "An upload can carry one poster picture, not two.";
/// What is said for a poster picture that is not a JPEG.
pub const NOT_A_JPEG: &str = "The video's poster picture must be a JPEG picture.";
/// What is said for a JPEG that does not decode.
pub const UNREADABLE: &str = "The video's poster picture is not a JPEG picture that can be read.";

/// Whether a poster picture may come with a file of `file_type` (upload_rules.rs `file_type`),
/// `encrypted` or not: only a video in the open.
pub fn allowed_with(file_type: &str, encrypted: bool) -> Result<(), Refused> {
    if encrypted {
        return Err((StatusCode::BAD_REQUEST, PRIVATE_REFUSED.to_string()));
    }
    if file_type != "video" {
        return Err((StatusCode::BAD_REQUEST, NOT_A_VIDEO.to_string()));
    }
    Ok(())
}

/// Read a poster picture's form part into memory, refusing it the moment it passes
/// [`POSTER_MAX_BYTES`] (so a large part is never read whole).
pub async fn read_part(field: &mut axum::extract::multipart::Field<'_>) -> Result<Vec<u8>, Refused> {
    let mut bytes = Vec::new();
    while let Some(chunk) = field.chunk().await.map_err(|e| (StatusCode::BAD_REQUEST, format!("Failed to read the poster picture: {e}")))? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > POSTER_MAX_BYTES {
            return Err(too_large(bytes.len()));
        }
    }
    Ok(bytes)
}

/// Hold a poster picture to what one must be, then clean it: a JPEG (its first bytes), at most
/// [`POSTER_MAX_BYTES`], at most [`POSTER_MAX_SIDE`] pixels on either side (read from its header
/// before anything is decoded, so a header claiming a huge picture is refused before a byte of
/// pixels is made), that decodes whole; its hidden information removed (upload_rules.rs
/// `finish_picture`, the strip every picture gets). BLOCKING (the decode): on a worker thread.
pub fn check_poster(bytes: Vec<u8>) -> Result<Vec<u8>, Refused> {
    if bytes.len() > POSTER_MAX_BYTES {
        return Err(too_large(bytes.len()));
    }
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, NOT_A_JPEG.to_string()));
    }
    let reader = image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Jpeg);
    let (w, h) = reader.into_dimensions().map_err(|_| (StatusCode::UNSUPPORTED_MEDIA_TYPE, UNREADABLE.to_string()))?;
    if w == 0 || h == 0 {
        return Err((StatusCode::UNSUPPORTED_MEDIA_TYPE, UNREADABLE.to_string()));
    }
    if w > POSTER_MAX_SIDE || h > POSTER_MAX_SIDE {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("The video's poster picture is {w} by {h} pixels; one can be at most {POSTER_MAX_SIDE} pixels on either side."),
        ));
    }
    image::load_from_memory_with_format(&bytes, image::ImageFormat::Jpeg).map_err(|_| (StatusCode::UNSUPPORTED_MEDIA_TYPE, UNREADABLE.to_string()))?;
    super::upload_rules::finish_picture("image/jpeg", bytes)
}

#[cfg(test)]
#[path = "poster_tests.rs"]
mod tests;
