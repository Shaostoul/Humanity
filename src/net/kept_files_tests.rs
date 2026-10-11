//! Tests for kept_files.rs: a kept file comes back exactly, is encrypted at rest under the
//! identity's key, is named by a keyed hash, stays under the cap (the longest-unopened going first),
//! and "Clear kept files" empties the folder.

use super::*;

const SEED: [u8; 32] = [7u8; 32];

/// What is kept comes back byte for byte, in memory and into a file; the bytes on disk are not the
/// ciphertext given (encrypted again under the identity's key), and another identity cannot read
/// them. Seen red 2026-10-11 with the keystream not applied on write: reading it back (which
/// applies it) no longer gave the ciphertext, the first assertion on `get`.
#[test]
fn a_kept_file_comes_back_exactly_and_is_encrypted_at_rest() {
    let dir = crate::test_temp::dir("kept_roundtrip");
    let kept = KeptFiles::in_dir(dir.to_path_buf(), &SEED, 1 << 20);
    let sealed: Vec<u8> = (0..5000u32).map(|i| (i * 31 % 251) as u8).collect();
    assert!(!kept.has("/uploads/a.enc", "k", "n"));
    assert!(kept.put("/uploads/a.enc", "k", "n", &sealed).unwrap());
    assert!(kept.has("/uploads/a.enc", "k", "n"));
    assert_eq!(kept.get("/uploads/a.enc", "k", "n", 1 << 20).as_deref(), Some(&sealed[..]));
    let mut out = Vec::new();
    assert!(kept.get_into("/uploads/a.enc", "k", "n", &mut out));
    assert_eq!(out, sealed);
    assert_eq!(kept.get("/uploads/a.enc", "k", "n", 100), None, "not past the size asked for");

    let on_disk: Vec<Vec<u8>> = std::fs::read_dir(&*dir).unwrap().flatten().map(|e| std::fs::read(e.path()).unwrap()).collect();
    assert_eq!(on_disk.len(), 1);
    assert_eq!(on_disk[0].len(), sealed.len() + 16);
    assert!(!on_disk[0].windows(64).any(|w| w == &sealed[..64]), "the bytes on disk are not the ciphertext given");
    let name = std::fs::read_dir(&*dir).unwrap().flatten().next().unwrap().file_name().to_string_lossy().to_string();
    assert!(!name.contains("uploads") && name.ends_with(".kept"), "a keyed hash names it: {name}");

    let other = KeptFiles::in_dir(dir.to_path_buf(), &[9u8; 32], 1 << 20);
    assert!(!other.has("/uploads/a.enc", "k", "n"), "another identity does not even find it");
    kept.forget("/uploads/a.enc", "k", "n");
    assert!(!kept.has("/uploads/a.enc", "k", "n"));
}

/// The cap: a file alone over it is not kept; adding past it removes the files opened longest ago,
/// and opening a file makes it the newest. Seen red 2026-10-11 with the eviction sorted newest
/// first: "b was opened longest ago, so it went" failed.
#[test]
fn the_cap_keeps_the_most_recently_opened() {
    let dir = crate::test_temp::dir("kept_cap");
    let kept = KeptFiles::in_dir(dir.to_path_buf(), &SEED, 3 * 1016);
    let file = |c: u8| vec![c; 1000];
    assert!(!kept.put("/x", "k", "n", &vec![0u8; 4000]).unwrap(), "over the cap alone: not kept");
    let old = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
    for (i, url) in ["/a", "/b", "/c"].iter().enumerate() {
        assert!(kept.put(url, "k", "n", &file(i as u8)).unwrap());
        // Make the order of opening certain: a first, then b, then c.
        let p = kept.path_of(url, "k", "n");
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(old + std::time::Duration::from_secs(i as u64 * 10)).unwrap();
    }
    assert!(kept.get("/a", "k", "n", 1 << 20).is_some()); // a is opened again: now the newest
    assert!(kept.put("/d", "k", "n", &file(3)).unwrap());
    assert!(kept.has("/a", "k", "n"), "a was opened most recently, so it stays");
    assert!(!kept.has("/b", "k", "n"), "b was opened longest ago, so it went");
    assert!(kept.has("/c", "k", "n") && kept.has("/d", "k", "n"));
    assert!(used(&dir) <= 3 * 1016);
    let none = KeptFiles::in_dir(dir.to_path_buf(), &SEED, 0);
    assert!(!none.put("/e", "k", "n", &file(4)).unwrap(), "a cap of 0 keeps nothing");
}

/// "Clear kept files" empties the folder and says how much. Seen red 2026-10-11 with `clear` not
/// counting what it removed: (0, 182) against (2, 182).
#[test]
fn clear_empties_the_folder() {
    let dir = crate::test_temp::dir("kept_clear");
    let kept = KeptFiles::in_dir(dir.to_path_buf(), &SEED, 1 << 20);
    kept.put("/a", "k", "n", &[1u8; 100]).unwrap();
    kept.put("/b", "k", "n", &[2u8; 50]).unwrap();
    assert_eq!(clear(&dir), (2, 182));
    assert_eq!(used(&dir), 0);
    assert!(!kept.has("/a", "k", "n"));
}

/// An unfinished write (the app closed mid-write) leaves a `.tmp` outside the cap: making room
/// removes one older than an hour and leaves a fresh one (a write in progress), and "Clear kept
/// files" removes them all (BUG-196's review, 2026-10-11).
///
/// Seen red 2026-10-11 with `leftovers` answering no files: "an old unfinished write is removed".
#[test]
fn unfinished_writes_are_removed() {
    let dir = crate::test_temp::dir("kept_leftovers");
    std::fs::create_dir_all(&*dir).unwrap();
    let old = dir.join("aaaa.tmp");
    let fresh = dir.join("bbbb.tmp");
    std::fs::write(&old, vec![1u8; 4096]).unwrap();
    std::fs::write(&fresh, vec![2u8; 4096]).unwrap();
    let two_hours_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(7200);
    File::options().write(true).open(&old).unwrap().set_modified(two_hours_ago).unwrap();
    evict(&dir, 1 << 20);
    assert!(!old.exists(), "an old unfinished write is removed");
    assert!(fresh.exists(), "a write that may still be going is left");
    clear(&dir);
    assert!(!fresh.exists(), "Clear kept files removes it too");
}
