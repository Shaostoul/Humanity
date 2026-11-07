// Child of chat/video_poster.rs (#[path]): a posted video's poster picture, made from a real video
// (tests/fixtures/media) and found at the address both apps agree on. Each test was seen failing
// once on purpose; the break and what it said are recorded at the test.

use super::*;

fn media_fixture(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/media").join(name)
}

/// The mean brightness (0 to 255) of a decoded picture's pixels.
fn mean_light(rgba: &[u8]) -> f64 {
    let px = rgba.chunks_exact(4).map(|p| (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0);
    let n = (rgba.len() / 4).max(1) as f64;
    px.sum::<f64>() / n
}

/// THE POSTING APP MAKES A POSTER PICTURE FROM THE VIDEO ITSELF: from each fixture video our own
/// player opens (AV1 in an MP4, AV1 in a WebM), a JPEG at most 640 pixels on its longest side and
/// under the 96 KB aimed for, which decodes to the video's shape and is not a black frame. A file
/// that is no video gives none, in words.
///
/// Seen red 2026-11-06 with `encode_poster` skipping its `fit` (the frame kept at its own size) on
/// a 1280 x 720 frame: "at most 640 pixels on its longest side: (1280, 720)".
#[test]
fn the_posting_app_makes_a_poster_picture_from_the_video_itself() {
    for name in ["sliding-bar-av1-opus.mp4", "colour-bar-av1-opus.webm"] {
        let path = media_fixture(name);
        let info = crate::media::probe(&path).expect("the fixture is a video");
        let jpeg = make_poster(&path).unwrap_or_else(|e| panic!("{name}: a poster picture is made: {e}"));
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]), "{name}: a JPEG");
        assert!(jpeg.len() <= POSTER_AIM_BYTES, "{name}: under the 96 KB aimed for ({} bytes)", jpeg.len());
        let (w, h, rgba) = decode_poster(&jpeg).expect("it decodes as a poster picture");
        assert!(w.max(h) <= POSTER_MAX_SIDE, "{name}: at most 640 pixels on its longest side: ({w}, {h})");
        let shape = (w as f64 / h as f64, info.width as f64 / info.height as f64);
        assert!((shape.0 - shape.1).abs() < 0.03, "{name}: the video's own shape: {shape:?}");
        assert!(mean_light(&rgba) > 8.0, "{name}: not a black frame (mean light {:.1})", mean_light(&rgba));
    }
    // A big frame is made small; a small one keeps its size.
    let big = vec![200u8; 1280 * 720 * 4];
    let (w, h, _) = decode_poster(&encode_poster(&big, 1280, 720).unwrap()).unwrap();
    assert_eq!((w, h), (640, 360), "at most 640 pixels on its longest side: ({w}, {h})");
    assert_eq!(fit(360, 640, 640), (360, 640));
    assert_eq!(fit(1080, 1920, 640), (360, 640), "a phone's upright video stays upright");

    let dir = humanity_core::test_temp::dir("poster_not_a_video");
    let fake = dir.join("not_a_film.mp4");
    std::fs::write(&fake, b"these bytes are no film").unwrap();
    assert!(make_poster(&fake).is_err(), "a file that is no video gives no poster picture");
}

/// BOTH APPS FIND A POSTER PICTURE AT THE SAME ADDRESS: the shared cases
/// (scripts/tests/fixtures/video-poster-addresses.json, which the website's test reads too), and
/// the relay's own name for it.
///
/// Seen red 2026-11-06 with `poster_url` accepting any last path part (the folder not checked):
/// "/uploads/sub/1760000000004_talk.mp4 / left: Some(\"/uploads/sub/1760000000004_talk.mp4.poster.jpg\")
/// / right: None".
#[test]
fn both_apps_find_a_poster_picture_at_the_same_address() {
    let text = std::fs::read_to_string(std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("scripts/tests/fixtures/video-poster-addresses.json")).unwrap();
    let cases: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cases = cases["cases"].as_array().unwrap();
    assert!(cases.len() >= 8);
    for c in cases {
        let video = c["video"].as_str().unwrap();
        assert_eq!(poster_url(video), c["poster"].as_str().map(str::to_string), "{video}");
    }
    let stored = "1760000000000_meetup.mp4";
    assert_eq!(poster_url(&format!("/uploads/{stored}")), Some(format!("/uploads/{}", crate::relay::storage::files::poster_name(stored))), "the relay's own name for it");
}

/// A FETCHED POSTER PICTURE IS ONE THE RELAY COULD HAVE KEPT: one over 640 pixels on a side is
/// refused from its header (a server serving something else there), and bytes that are no picture
/// are refused, each in words, never a panic.
///
/// Seen red 2026-11-06 with the side check taken out of `decode_poster`: "a 2000-pixel picture is
/// not a poster picture" (it decoded, Ok).
#[test]
fn a_fetched_poster_picture_is_one_the_relay_could_have_kept() {
    let img = image::RgbImage::from_fn(2000, 20, |_, _| image::Rgb([1, 2, 3]));
    let mut out = std::io::Cursor::new(Vec::new());
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 50).encode_image(&img).unwrap();
    assert!(decode_poster(&out.into_inner()).is_err(), "a 2000-pixel picture is not a poster picture");
    assert!(decode_poster(b"<html>not found</html>").is_err(), "a page is not a picture");
}

/// THE POSTER PICTURE GOES BEFORE THE FILE IN THE UPLOAD: the form's first part is named `poster`,
/// a JPEG, and the file's part follows with its own bytes, the length the body says.
///
/// Seen red 2026-11-06 with `with_poster` appending its part after the file's: "the poster part
/// comes first".
#[test]
fn the_poster_picture_goes_before_the_file_in_the_upload() {
    use std::io::Read;
    let body = super::super::attach_body::Multipart::with_poster("clip.mp4", "video/mp4", b"VIDEO-BYTES".to_vec(), b"\xFF\xD8\xFFJPEG-BYTES");
    let (boundary, len) = (body.boundary.clone(), body.len());
    let mut all = Vec::new();
    body.reader().read_to_end(&mut all).unwrap();
    assert_eq!(all.len() as u64, len, "the length the body says");
    let text = String::from_utf8_lossy(&all).into_owned();
    let poster = text.find("name=\"poster\"").expect("a poster part");
    let file = text.find("name=\"file\"").expect("a file part");
    assert!(poster < file, "the poster part comes first");
    assert!(text[poster..file].contains("Content-Type: image/jpeg") && text[poster..file].contains("JPEG-BYTES"), "a JPEG");
    assert!(text[file..].contains("VIDEO-BYTES") && text.ends_with(&format!("--{boundary}--\r\n")), "then the file, then the end");
}
