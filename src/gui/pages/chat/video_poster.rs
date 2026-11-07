//! A posted video's poster picture, the desktop half (2026-11-06; docs/design/media-player.md,
//! "Rung 2: the poster picture"). The first rung left a video row black before Play, because a
//! still per row would cost a read and a decode of every video on screen. Now the app that posts a
//! video makes one small still of it, once, and sends it with the video; the relay keeps it beside
//! the file (relay/poster.rs), and every row draws it before Play at the cost of one small picture.
//!
//! - **Making one** ([`make_poster`]): the app's own player (`media::VideoPlayer`, which also opens
//!   an ordinary MP4 through Windows's own decoders) is opened on the file, moved to about a second
//!   in ([`POSTER_AT_S`]; the first frame of a shorter video), and the frame it lands on is made a
//!   JPEG at most [`POSTER_MAX_SIDE`] pixels on its longest side, at quality 80, stepped down until
//!   it is under [`POSTER_AIM_BYTES`]. A video nothing here decodes gets none (no ffmpeg run for a
//!   poster picture): the video still goes. Runs on the upload's worker (chat/attach_send.rs
//!   `run`), never on the frame.
//! - **Its address** ([`poster_url`]): the video's own with `.poster.jpg` after it, for a video at
//!   `/uploads/<name>` (relay/storage/files.rs `poster_name`). The website works it out the same
//!   way (web/shared/video-poster.js `posterUrlOf`); both are held to
//!   scripts/tests/fixtures/video-poster-addresses.json.
//! - **Drawing one** is chat/inline_video.rs (`ChatVideos::want_poster`): asked for once while its
//!   row is on screen, fetched and decoded here ([`fetch_poster`]) on a worker.

use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) use crate::relay::poster::{POSTER_MAX_BYTES, POSTER_MAX_SIDE};

/// What the app aims a poster picture under; the relay refuses one over [`POSTER_MAX_BYTES`].
pub(crate) const POSTER_AIM_BYTES: usize = 96 * 1024;
/// The JPEG qualities tried in turn, until one is under [`POSTER_AIM_BYTES`].
pub(crate) const QUALITIES: [u8; 4] = [80, 65, 50, 35];
/// How far into a video its poster picture is taken: a first frame is often black or a fade.
pub(crate) const POSTER_AT_S: f64 = 1.0;
/// How long the decoder may take to give that picture before the video goes without one.
const POSTER_WAIT: Duration = Duration::from_secs(10);
/// The most a poster picture's fetch reads: twice what the relay keeps, so a server that serves
/// something else at the address is cut off early.
pub(crate) const POSTER_FETCH_MAX: usize = 2 * POSTER_MAX_BYTES;

/// Whether a file named `name` is a video the relay takes a poster picture with (an MP4 or a
/// WebM: relay/upload_rules.rs `file_type`'s videos).
pub(crate) fn is_video_name(name: &str) -> bool {
    crate::gui::widgets::file_browser::name_matches_ext(name, &["mp4", "webm"])
}

/// The poster picture of the video at `path`: a frame about a second in (the first of a shorter
/// one), as a JPEG ([`encode_poster`]). An error in words when nothing here decodes it or no
/// picture comes within [`POSTER_WAIT`]. BLOCKING (a decode): on a worker thread.
pub(crate) fn make_poster(path: &Path) -> Result<Vec<u8>, String> {
    let mut player = crate::media::VideoPlayer::open(path).map_err(|e| e.to_string())?;
    if player.duration_s() > POSTER_AT_S {
        player.seek_to(POSTER_AT_S);
    }
    let started = Instant::now();
    let frame = loop {
        if let Some(frame) = player.poll() {
            break frame;
        }
        if let Some(e) = player.take_error() {
            return Err(e);
        }
        if started.elapsed() > POSTER_WAIT {
            return Err("no picture came from the video in time".to_string());
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    encode_poster(&frame.rgba, frame.width, frame.height)
}

/// The size a `w` x `h` picture is drawn at with its longest side at most `max`, its shape kept
/// (never 0 on a side).
pub(crate) fn fit(w: u32, h: u32, max: u32) -> (u32, u32) {
    if w.max(h) <= max {
        return (w.max(1), h.max(1));
    }
    let scale = max as f64 / w.max(h) as f64;
    (((w as f64 * scale).round() as u32).clamp(1, max), ((h as f64 * scale).round() as u32).clamp(1, max))
}

/// A decoded frame (`rgba`, `w` x `h`) as a poster picture: made small ([`fit`]) and a JPEG at the
/// first of [`QUALITIES`] under [`POSTER_AIM_BYTES`] (the last one if none is, as long as it is
/// under what the relay keeps).
pub(crate) fn encode_poster(rgba: &[u8], w: u32, h: u32) -> Result<Vec<u8>, String> {
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or_else(|| "the picture's size does not match its pixels".to_string())?;
    let (tw, th) = fit(w, h, POSTER_MAX_SIDE);
    let rgb = image::DynamicImage::ImageRgba8(img).resize_exact(tw, th, image::imageops::FilterType::Triangle).to_rgb8();
    let mut last = Vec::new();
    for q in QUALITIES {
        let mut out = std::io::Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, q).encode_image(&rgb).map_err(|e| e.to_string())?;
        last = out.into_inner();
        if last.len() <= POSTER_AIM_BYTES {
            return Ok(last);
        }
    }
    if last.len() <= POSTER_MAX_BYTES {
        Ok(last)
    } else {
        Err(format!("the poster picture came out at {} KB, over the {} KB a server keeps", last.len() / 1024, POSTER_MAX_BYTES / 1024))
    }
}

/// The address of the poster picture of the video at `video_url`: the video's own with
/// `.poster.jpg` after it, for a video at `/uploads/<name>` with no query or fragment; None for any
/// other address (a video on another website, a private file).
pub(crate) fn poster_url(video_url: &str) -> Option<String> {
    let (folder, name) = video_url.rsplit_once('/')?;
    let bare = !name.is_empty() && !name.starts_with('.') && !name.contains(['\\', '?', '#']) && !name.contains("..");
    (folder.ends_with("/uploads") && bare).then(|| format!("{video_url}{}", crate::relay::storage::files::POSTER_SUFFIX))
}

/// Fetch and decode the poster picture at `url`: at most [`POSTER_FETCH_MAX`] read, refused when it
/// is over [`POSTER_MAX_SIDE`] on a side (read from its header before any pixels are made: it is
/// not one the relay kept). Its width, height and RGBA pixels. BLOCKING: on a worker thread.
pub(crate) fn fetch_poster(url: &str) -> Result<(u32, u32, Vec<u8>), String> {
    decode_poster(&crate::gui::widgets::image_cache::download_bytes(url, POSTER_FETCH_MAX)?)
}

/// [`fetch_poster`]'s decode of the bytes fetched.
pub(crate) fn decode_poster(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| e.to_string())?;
    let (w, h) = reader.into_dimensions().map_err(|e| format!("not a picture: {e}"))?;
    if w == 0 || h == 0 || w > POSTER_MAX_SIDE || h > POSTER_MAX_SIDE {
        return Err(format!("a {w} x {h} picture is not a poster picture"));
    }
    let img = image::load_from_memory(bytes).map_err(|e| format!("not a picture: {e}"))?.to_rgba8();
    let (w, h) = img.dimensions();
    Ok((w, h, img.into_raw()))
}

#[cfg(test)]
#[path = "video_poster_tests.rs"]
mod tests;
