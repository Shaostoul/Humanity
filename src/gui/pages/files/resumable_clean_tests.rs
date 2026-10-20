//! Add to Files's upload cleaned on this device first (2026-10-11, docs/design/files-showcase.md,
//! the files section, parts 1 and 2), against the simulated server of resumable_tests.rs.

use super::tests::{run_on, Fake, TOKEN};
use super::*;
use crate::relay::core::describe_metadata::describe;
use std::sync::Arc;

fn plan_for(fixture: &str, name: &str, clean: bool) -> (crate::test_temp::TempPath, Plan, Vec<u8>) {
    let bytes = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/describe").join(fixture)).unwrap();
    let path = crate::test_temp::file(&format!("media_clean_{name}"), "bin");
    std::fs::write(&path, &bytes).unwrap();
    let plan = Plan { path: path.to_path_buf(), name: name.into(), size: bytes.len() as u64, content_type: "video/quicktime".into(), resume: None, file_changed: false, clean, keep_metadata: !clean };
    (path, plan, bytes)
}

/// A phone video goes up saying nothing: the worker cleans a COPY (the person's own file is never
/// written), tells the page where it is, and uploads it; the server's copy says nothing.
/// Seen red 2026-10-11 with `work` skipping the clean: "the page is told where the copy is" failed
/// (no copy was made, and the file went up as it was).
#[test]
fn a_phone_video_is_cleaned_in_a_copy_and_the_copy_goes_up() {
    let (own, plan, before) = plan_for("mov-apple.mov", "IMG_0003.MOV", true);
    let fake = Fake::new(8192);
    let events = run_on(fake.clone(), plan, Arc::new(Control::new(Some(TOKEN.into()))), |_, _| {});
    let copy = events.iter().find_map(|e| if let Event::Cleaned { path, .. } = e { Some(path.clone()) } else { None }).expect("the page is told where the copy is");
    assert!(matches!(events.last(), Some(Event::Done { .. })), "{events:?}");
    let up = fake.s().data.clone();
    assert!(describe(&up, "x.mov").details.is_empty(), "the server's copy says nothing");
    assert_eq!(up.len(), before.len(), "a video keeps its length");
    assert_eq!(std::fs::read(&own).unwrap(), before, "the person's own file is never written");
    assert!(copy.starts_with(crate::net::file_clean::cleaned_dir()) && copy != own.to_path_buf());
    let _ = std::fs::remove_file(copy);
}

/// Kept by choice the file goes exactly as picked, its start saying `keep_metadata: 1`; one that
/// could not be checked is not sent at all (Media is public). Seen red 2026-10-11 with
/// `start_body` leaving the flag out: "the start says keep_metadata" failed.
#[test]
fn kept_by_choice_or_not_sent() {
    let (_own, plan, before) = plan_for("mov-apple.mov", "IMG_0003.MOV", false);
    assert_eq!(start_body(&plan, "h")["keep_metadata"], 1, "the start says keep_metadata");
    assert!(start_body(&Plan { keep_metadata: false, ..plan.clone() }, "h").get("keep_metadata").is_none());
    let fake = Fake::new(8192);
    let events = run_on(fake.clone(), plan, Arc::new(Control::new(Some(TOKEN.into()))), |_, _| {});
    assert!(!events.iter().any(|e| matches!(e, Event::Cleaned { .. })));
    assert_eq!(fake.s().data, before, "exactly as picked");

    let (_own, plan, _) = plan_for("mp4-broken-box.mp4", "clip.mp4", true);
    let fake = Fake::new(8192);
    let events = run_on(fake.clone(), plan, Arc::new(Control::new(Some(TOKEN.into()))), |_, _| {});
    assert_eq!(events.last(), Some(&Event::Failed { words: NOT_CHECKED_FILES.into(), gone: true }));
    assert!(fake.s().id.is_none(), "nothing was started on the server");
}
