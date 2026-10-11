//! Tests of part files (part_file.rs). Each was seen to fail once before it passed; what it saw
//! red is in its comment.

use super::*;
use std::io::Write;

/// TWO WRITERS NEVER GET THE SAME NAME (the review's finding 7: Download picked a free name by
/// checking, then created it, so two downloads of one name at once could both take it, and one
/// overwrote the other): eight parts finished under one name at the same moment take eight names,
/// each holding its own bytes.
///
/// Seen red 2026-10-11 with `claim_and_move` checking `exists()` and then renaming (the old
/// `free_path` way): "eight names for eight files" (left: 1 / right: 8: all eight were moved onto
/// "same.bin", one over another).
#[test]
fn two_writers_never_get_the_same_name() {
    let dir = crate::test_temp::dir("part_same_name");
    std::fs::create_dir_all(&dir).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let handles: Vec<_> = (0..8u8)
        .map(|i| {
            let (dir, barrier) = (dir.to_path_buf(), barrier.clone());
            std::thread::spawn(move || {
                let mut part = Part::create(&dir).expect("a part");
                part.file().write_all(&[i; 64]).unwrap();
                barrier.wait();
                part.finish(&dir, "same.bin").expect("named")
            })
        })
        .collect();
    let paths: Vec<PathBuf> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let distinct: std::collections::HashSet<&PathBuf> = paths.iter().collect();
    assert_eq!(distinct.len(), 8, "eight names for eight files: {paths:?}");
    let mut seen: Vec<u8> = paths.iter().map(|p| std::fs::read(p).unwrap()[0]).collect();
    seen.sort();
    assert_eq!(seen, (0..8).collect::<Vec<u8>>(), "each holds its own bytes");
    let left: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| is_part_name(n)).collect();
    assert!(left.is_empty(), "no part file is left: {left:?}");
}

/// A PART THAT IS NOT FINISHED LEAVES NOTHING, AND NOTHING EVER HAS THE REAL NAME UNTIL IT IS
/// COMPLETE: while it is written only the hidden part exists; dropped unfinished, it is deleted.
///
/// Seen red 2026-10-11 with `Drop for Part` deleting nothing: "an unfinished part leaves nothing"
/// (left: 1 / right: 0).
#[test]
fn an_unfinished_part_leaves_nothing() {
    let dir = crate::test_temp::dir("part_unfinished");
    let mut part = Part::create(&dir).expect("a part");
    part.file().write_all(b"half of it").unwrap();
    let names: Vec<String> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect();
    assert_eq!(names.len(), 1, "only the part while it is written: {names:?}");
    assert!(is_part_name(&names[0]), "and it is hidden: {names:?}");
    drop(part);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "an unfinished part leaves nothing");
}

/// THE APP'S EXIT DELETES THE PARTS STILL BEING WRITTEN, AND THE NEXT START DELETES WHAT A CRASH
/// LEFT, but only parts nobody has written to for a while and never anything else in the folder.
///
/// Seen red 2026-10-11 with `sweep_stale` ignoring the age: "only the old part goes" (left: 2 /
/// right: 1: the part being written went too).
#[test]
fn leftovers_are_swept_and_nothing_else() {
    let dir = crate::test_temp::dir("part_sweep");
    let mut live = Part::create(&dir).expect("a part");
    live.file().write_all(b"x").unwrap();
    let live_path = live.path().to_path_buf();
    // What a crash left: a part an hour old, and the person's own file beside it.
    let old = dir.join(format!("{PREFIX}1-2-3{SUFFIX}"));
    std::fs::write(&old, b"old").unwrap();
    let hour_ago = std::time::SystemTime::now() - Duration::from_secs(3600);
    std::fs::File::options().write(true).open(&old).unwrap().set_modified(hour_ago).unwrap();
    std::fs::write(dir.join("holiday.png"), b"mine").unwrap();

    assert_eq!(sweep_stale(&dir, STALE_AFTER), 1, "only the old part goes");
    assert!(!old.exists(), "what a crash left goes");
    assert!(live_path.exists(), "a part being written now is left alone");
    assert!(dir.join("holiday.png").exists(), "and nothing else is touched");

    // As if the app quit with it still being written: its handle closed, the part still listed.
    drop(live.file.take());
    std::mem::forget(live);
    assert!(is_listed(&live_path), "the part is listed for the exit");
    sweep_own_in(&dir); // the exit's sweep, kept to this test's folder (tests run side by side)
    assert!(!live_path.exists(), "the exit deletes the parts this run was still writing");
}
