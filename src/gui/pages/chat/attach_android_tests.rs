//! Tests of Attach on a phone, the Rust side of Android's document picker (attach_android.rs,
//! 2026-11-23): the picker's answer read, the copies named and placed, and what was read handed to
//! the attach step. Each was seen to fail before it passed; what it saw red is in its comment. (The
//! Java's signatures are held by humanity-core's platform/android_java.rs test; only a phone runs
//! the picker itself.)

use super::*;

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// PickFiles.take's three answers, and nothing else acted on; an address that is not content:// is
/// never read.
///
/// Seen red 2026-11-23 with the content:// filter left out: "left: Some(Picked([\"content://a/1\",
/// \"file:///data/x\", \"content://b/2\"]))".
#[test]
fn the_pickers_answer_is_read() {
    assert_eq!(
        parse(&strings(&["picked", "content://a/1", "file:///data/x", "content://b/2"])),
        Some(Outcome::Picked(strings(&["content://a/1", "content://b/2"])))
    );
    assert_eq!(parse(&strings(&["picked"])), Some(Outcome::Picked(vec![])));
    assert_eq!(parse(&strings(&["cancelled"])), Some(Outcome::Cancelled));
    assert_eq!(parse(&strings(&["failed", "this phone has no document picker"])), Some(Outcome::Failed("this phone has no document picker".into())));
    assert_eq!(parse(&strings(&["failed"])), Some(Outcome::Failed(String::new())));
    assert_eq!(parse(&[]), None);
    assert_eq!(parse(&strings(&["something else", "content://a/1"])), None);
}

/// A provider's name for a document becomes a name the copy may have: never a path out of its
/// folder, never hidden or empty, long ones cut with their extension kept.
///
/// Seen red 2026-11-23 with '/' left out of the replaced characters: "left: \"/../etc/passwd\"" (a
/// name that leaves the copy's folder).
#[test]
fn a_documents_name_becomes_a_safe_file_name() {
    assert_eq!(safe_name("IMG_2041.jpg"), "IMG_2041.jpg");
    assert_eq!(safe_name("../../etc/passwd"), "_.._etc_passwd");
    assert_eq!(safe_name("a\\b:c*d?.png"), "a_b_c_d_.png");
    assert_eq!(safe_name(".hidden"), "hidden");
    assert_eq!(safe_name("   "), "file");
    assert_eq!(safe_name(""), "file");
    assert_eq!(safe_name(".."), "file");
    let long = format!("{}.jpeg", "x".repeat(300));
    let cut = safe_name(&long);
    assert_eq!(cut.chars().count(), NAME_MAX);
    assert!(cut.ends_with(".jpeg"), "{cut}");
    // Each document of a pick has a folder of its own: two of the same name never collide.
    let root = Path::new("root");
    assert_ne!(copy_path(root, 0, "image.jpg"), copy_path(root, 1, "image.jpg"));
    assert_eq!(copy_path(root, 1, "../x.png"), root.join("1").join("_x.png"));
}

/// The size cap the desktop's picker is given decides what is copied: at most the cap, or unknown
/// (the copy then stops at the cap).
///
/// Seen red 2026-11-23 with an unknown size refused: "an unknown size is copied (up to the cap)".
#[test]
fn documents_over_the_cap_are_not_copied() {
    assert!(fits(Some(10), 10));
    assert!(!fits(Some(11), 10));
    assert!(fits(None, 10), "an unknown size is copied (up to the cap)");
}

fn temp_files(tag: &str, names: &[&str]) -> Vec<PathBuf> {
    let dir = std::env::temp_dir().join(format!("hos_attach_android_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    names
        .iter()
        .map(|n| {
            let p = dir.join(n);
            std::fs::write(&p, b"picture").unwrap();
            p
        })
        .collect()
}

/// What was read goes where the desktop's picker sends its files (`attach_send::start_attach_all`):
/// the first into its attach step, the rest queued for the same conversation; one over the cap and
/// one unreadable are each said in a notice.
///
/// Seen red 2026-11-23 with `finish` dropping the ready files when there were several: "the second
/// picture waits its turn for the same conversation: None".
#[test]
fn what_was_read_goes_to_the_attach_step() {
    let files = temp_files("handoff", &["one.jpg", "two.jpg"]);
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    let read = vec![
        Read::Ready(files[0].clone()),
        Read::TooLarge { name: "film.mp4".into(), len: 900 * 1024 * 1024 },
        Read::Failed { name: "gone.pdf".into(), why: "FileNotFoundException".into() },
        Read::Ready(files[1].clone()),
    ];
    finish(&mut state, "general", 500 * 1024 * 1024, read);
    let queued = state.files_flow.queue.as_ref().map(|(ch, q)| (ch.clone(), q.iter().cloned().collect::<Vec<_>>()));
    assert_eq!(queued, Some(("general".to_string(), vec![files[1].clone()])), "the second picture waits its turn for the same conversation: {queued:?}");
    assert!(state.pending_notices.iter().any(|n| n.contains("film.mp4") && n.contains("900.0 MB")), "the film over the cap is said: {:?}", state.pending_notices);
    assert!(state.pending_notices.iter().any(|n| n.contains("gone.pdf") && n.contains("FileNotFoundException")), "the unreadable one is said: {:?}", state.pending_notices);
}

/// A file chosen in one conversation never goes to another: when the person has left it while the
/// files were read, nothing is sent, the copies are deleted, and that is said.
///
/// Seen red 2026-11-23 with the conversation check left out (and the ready files dropped when
/// several, as above): "the copies are deleted".
#[test]
fn files_read_after_leaving_the_conversation_are_not_sent() {
    let files = temp_files("left", &["one.jpg", "two.jpg"]);
    let mut state = GuiState::default();
    state.chat_active_channel = "general".into();
    finish(&mut state, "dm:somebody", 6 * 1024 * 1024, files.iter().cloned().map(Read::Ready).collect());
    assert!(state.files_flow.queue.is_none() && state.files_flow.step.is_none(), "nothing is queued for another conversation: {:?}", state.files_flow.queue);
    assert!(files.iter().all(|f| !f.exists()), "the copies are deleted");
    assert!(state.pending_notices.iter().any(|n| n.contains("2 files")), "leaving before they went is said: {:?}", state.pending_notices);
}
