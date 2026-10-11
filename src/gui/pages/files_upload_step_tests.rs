//! Tests for files_upload_step.rs: what the Files page uploads to Shared Files.

use super::*;
use crate::relay::core::describe_metadata::describe;

fn fixture(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/describe").join(name)
}

/// A photo goes up cleaned (and shared); kept by choice it goes exactly as it is with
/// `keep_metadata`; one that could not be checked does not go while details are to be removed.
/// Seen red 2026-10-11 with `prepare` not cleaning: "a shared photo goes up saying nothing" failed.
#[test]
fn what_goes_to_shared_files() {
    let photo = fixture("jpeg-phone.jpg");
    let (bytes, flags) = prepare(&photo, "IMG_0001.JPG", true).unwrap();
    assert!(describe(&bytes, "x.jpg").details.is_empty(), "a shared photo goes up saying nothing");
    assert!(flags.share && !flags.encrypted && !flags.keep_metadata);
    let (kept, flags) = prepare(&photo, "IMG_0001.JPG", false).unwrap();
    assert_eq!(kept, std::fs::read(&photo).unwrap(), "kept by choice: exactly as picked");
    assert!(flags.keep_metadata);
    let broken = fixture("mp4-broken-box.mp4");
    assert!(prepare(&broken, "clip.mp4", true).unwrap_err().starts_with("Not uploaded:"));
    assert!(prepare(&broken, "clip.mp4", false).is_ok(), "kept by choice, it goes as it is");
}
