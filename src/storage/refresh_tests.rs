//! Tests for storage/refresh.rs: the data folder after an update (BUG-163 part 2,
//! docs/design/data-folder-updates.md section 7). Every folder here is a temporary one
//! (`test_temp`); none is the real data folder or the repo's data/.

use super::*;
use std::time::{Duration, SystemTime};

/// A shipped set: (path under the folder, text).
type Set<'a> = [(&'a str, &'a str)];

fn bytes<'a>(set: &'a Set<'a>) -> Vec<(&'a str, &'a [u8])> {
    set.iter().map(|(rel, text)| (*rel, text.as_bytes())).collect()
}

/// Every data file this exe ships, as the game itself refreshes with.
fn real_shipped() -> Vec<(&'static str, &'static [u8])> {
    crate::embedded_data::EMBEDDED_KEYS
        .iter()
        .filter_map(|rel| crate::embedded_data::get_embedded(rel).map(|text| (*rel, text.as_bytes())))
        .collect()
}

fn read(dir: &Path, rel: &str) -> String {
    std::fs::read_to_string(dir.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn write(dir: &Path, rel: &str, text: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("make the folder");
    std::fs::write(path, text).expect("write the file");
}

/// Set when a file was last written, as an old first run or a later edit left it.
fn set_time(dir: &Path, rel: &str, t: SystemTime) {
    std::fs::File::options()
        .write(true)
        .open(dir.join(rel))
        .and_then(|f| f.set_modified(t))
        .unwrap_or_else(|e| panic!("set the time of {rel}: {e}"));
}

fn refreshed(outcome: Outcome) -> Report {
    match outcome {
        Outcome::Refreshed(r) => r,
        other => panic!("expected a refresh, got {other:?}"),
    }
}

/// Every file under `dir` and its bytes, by path (links are not followed: a link is listed as
/// a link), so "left alone byte for byte" can be compared whole, stamps and stray files included.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(at).expect("read the folder").flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
            let ty = std::fs::symlink_metadata(&path).unwrap().file_type();
            if ty.is_symlink() {
                out.insert(rel, b"<link>".to_vec());
            } else if ty.is_dir() {
                walk(root, &path, out);
            } else {
                out.insert(rel, std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// Make `link` a junction (Windows; a symlink elsewhere) to the folder `target`, as every rig
/// links its data/ to the checkout's.
fn link_dir(link: &Path, target: &Path) {
    #[cfg(windows)]
    {
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .expect("run mklink");
        assert!(out.status.success(), "mklink /J failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("make the link");
}

const V1: &Set = &[("a.csv", "a 1\n"), ("sub/b.ron", "b 1\n"), ("c.json", "c\n"), ("gone.txt", "only version 1 ships this\n")];
const V2: &Set = &[("a.csv", "a 2\n"), ("sub/b.ron", "b 2\n"), ("c.json", "c\n"), ("new/d.ron", "d 2\n")];

/// A STAMPED FOLDER: AN UNEDITED FILE GETS THE NEW COPY, AN EDITED ONE IS KEPT AND NAMED (design
/// section 7, first test; the operator's decision of 2026-10-12: a player's edit is never
/// overwritten). A file the new version adds is written; one it no longer ships is left alone; a
/// second start of the same version reads only the stamp.
///
/// Seen red 2026-10-12 with the stamp check broken (`unedited` true for every present file, as a
/// refresh that replaced everything would be): "an edited file is kept as the player left it:
/// left: \"b 2\\n\" right: \"mine\\n\"".
#[test]
fn a_stamped_folder_gets_the_new_copy_of_an_unedited_file_and_keeps_an_edited_one() {
    let dir = crate::test_temp::path("refresh_stamped");
    let (written, failed) = write_fresh(&dir, "1.0.0", &bytes(V1));
    assert_eq!((written, failed.len()), (4, 0), "the first run writes every file");
    write(&dir, "sub/b.ron", "mine\n");

    let r = refreshed(refresh(&dir, "2.0.0", &bytes(V2)));
    assert_eq!(read(&dir, "a.csv"), "a 2\n", "an unedited file gets this version's copy");
    assert_eq!(read(&dir, "sub/b.ron"), "mine\n", "an edited file is kept as the player left it");
    assert_eq!(read(&dir, "new/d.ron"), "d 2\n", "a file this version adds is written");
    assert_eq!(read(&dir, "gone.txt"), "only version 1 ships this\n", "a file no longer shipped is left alone");
    assert_eq!(r.from.as_deref(), Some("1.0.0"));
    assert_eq!(r.to, "2.0.0");
    assert_eq!(r.updated, vec!["a.csv".to_string()]);
    assert_eq!(r.added, vec!["new/d.ron".to_string()]);
    assert_eq!(r.kept, vec!["sub/b.ron".to_string()], "the edited file is named in the result");
    assert!(r.finished(), "{:?}", r.failed);

    let stamp = read_stamp(&dir).expect("a stamp");
    assert_eq!(stamp.version.as_deref(), Some("2.0.0"));
    assert_eq!(stamp.files["a.csv"], hash(b"a 2\n"), "what the game wrote is recorded");
    assert_eq!(stamp.files["sub/b.ron"], hash(b"b 1\n"), "the kept file keeps the record of what the game last wrote there");
    assert_eq!(stamp.last_update.as_ref().map(|l| l.kept.clone()), Some(vec!["sub/b.ron".to_string()]));
    assert_eq!(refresh(&dir, "2.0.0", &bytes(V2)), Outcome::Current, "a second start of the same version has nothing to do");

    // The next update keeps the edit again, and a player who put back the file the game wrote
    // gets the newer one.
    const V3: &Set = &[("a.csv", "a 3\n"), ("sub/b.ron", "b 3\n"), ("c.json", "c 3\n")];
    let r = refreshed(refresh(&dir, "3.0.0", &bytes(V3)));
    assert_eq!(r.kept, vec!["sub/b.ron".to_string()], "still kept at the next update");
    assert_eq!(read(&dir, "sub/b.ron"), "mine\n");
    write(&dir, "sub/b.ron", "b 1\n");
    let r = refreshed(refresh(&dir, "4.0.0", &bytes(&[("sub/b.ron", "b 4\n")])));
    assert_eq!((r.updated.clone(), r.kept.clone()), (vec!["sub/b.ron".to_string()], Vec::<String>::new()), "{r:?}");
}

/// Two builds of ONE version that ship different data are told apart: the folder follows the
/// data, not only the version number (an agent's build of an unbumped tree, for one).
///
/// Seen red 2026-10-12 with `refresh` judging the stamp by its version alone: "a build of the same
/// version with different data refreshes the folder: Current".
#[test]
fn a_build_of_the_same_version_with_different_data_refreshes_the_folder() {
    let dir = crate::test_temp::path("refresh_same_version");
    write_fresh(&dir, "1.0.0", &bytes(V1));
    let other_build: &Set = &[("a.csv", "a 1, rebuilt\n"), ("sub/b.ron", "b 1\n"), ("c.json", "c\n")];
    let outcome = refresh(&dir, "1.0.0", &bytes(other_build));
    assert!(matches!(outcome, Outcome::Refreshed(_)), "a build of the same version with different data refreshes the folder: {outcome:?}");
    assert_eq!(read(&dir, "a.csv"), "a 1, rebuilt\n");
}

/// AN UNSTAMPED FOLDER WITH ALL ITS FILES WRITTEN AT ONE TIME IS REFRESHED WHOLE; ONE FILE WITH A
/// LATER TIME IS KEPT (design section 7, second test, and section 3's rule). The folder is shaped
/// like the operator's (written 2026-07-11 by one first run, every file one second): this exe's
/// real data files, each in an older form, all written in one moment; one edited an hour later;
/// one copied in with an OLDER time than the first run (kept: it was not written by it); one
/// absent; one already this version's.
///
/// Seen red 2026-10-12, twice:
/// - with `first_run_files` judging every file unedited whatever its time: "a file changed after
///   the first run is kept, left: \"# HumanityOS Plants Database\\r\\n...\" right: \"// edited by
///   hand\\nid,name\\n\"" (the edit was overwritten with the shipped plants.csv);
/// - with the first run's moment taken as the folder's OLDEST file (the design's first wording)
///   instead of the earliest stretch holding most of the files: "a file copied in with a time
///   older than the first run is kept, left: \"# HumanityOS Crafting Recipes\\r\\n...\" right:
///   \"// my own recipes, copied from a backup\\n\"" (that file alone was taken for the first run
///   and overwritten).
#[test]
fn an_unstamped_folder_written_at_one_time_is_refreshed_whole_and_a_later_edit_kept() {
    let dir = crate::test_temp::dir("refresh_unstamped");
    let shipped = real_shipped();
    assert!(shipped.len() > 40, "the real data set");
    let first_run = SystemTime::now() - Duration::from_secs(90 * 86_400);
    let (edited, copied_in, absent, unchanged) = ("plants.csv", "recipes.csv", "status_effects.csv", "game.csv");
    for (rel, text) in &shipped {
        if *rel == absent {
            continue;
        }
        let old = if *rel == unchanged { String::from_utf8_lossy(text).to_string() } else { format!("// as July shipped it\n{}", String::from_utf8_lossy(text)) };
        write(&dir, rel, &old);
        set_time(&dir, rel, first_run + Duration::from_millis(300));
    }
    set_time(&dir, shipped[0].0, first_run);
    write(&dir, edited, "// edited by hand\nid,name\n");
    set_time(&dir, edited, first_run + Duration::from_secs(3600));
    write(&dir, copied_in, "// my own recipes, copied from a backup\n");
    set_time(&dir, copied_in, first_run - Duration::from_secs(30 * 86_400));
    assert!(!dir.join(STAMP_FILE).exists());

    let r = refreshed(refresh(&dir, "2.0.0", &shipped));
    assert_eq!(read(&dir, edited), "// edited by hand\nid,name\n", "a file changed after the first run is kept");
    assert_eq!(read(&dir, copied_in), "// my own recipes, copied from a backup\n", "a file copied in with a time older than the first run is kept");
    let mut kept = r.kept.clone();
    kept.sort();
    let mut want = vec![copied_in.to_string(), edited.to_string()];
    want.sort();
    assert_eq!(kept, want);
    assert_eq!(r.added, vec![absent.to_string()], "the absent file is written");
    assert_eq!(r.from, None, "a folder from before stamps");
    for (rel, text) in &shipped {
        if *rel != edited && *rel != copied_in {
            assert_eq!(std::fs::read(dir.join(rel)).unwrap(), *text, "{rel} holds this version's copy");
        }
    }
    assert_eq!(r.updated.len(), shipped.len() - 4, "every other file was brought up to date: {r:?}");
    let stamp = read_stamp(&dir).expect("the folder is stamped now");
    assert_eq!(stamp.version.as_deref(), Some("2.0.0"));
    assert!(!stamp.files.contains_key(edited), "nothing is known of what the game wrote over an edited file");
}

/// A folder with no stamp whose files were NOT written in one moment (most of them changed at
/// different times) has no first run to judge by, so nothing in it is judged unedited: a file
/// left old shows in Settings > Data, a file overwritten is lost.
///
/// Seen red 2026-10-12 with `first_run_files` taking any stretch of one file as the first run
/// (`need` of 1): "with no first-run moment nothing is overwritten, left: [\"f00.csv\"] right: []".
#[test]
fn an_unstamped_folder_with_no_first_run_moment_keeps_every_file() {
    let dir = crate::test_temp::dir("refresh_no_moment");
    let start = SystemTime::now() - Duration::from_secs(400 * 86_400);
    let set: Vec<(String, String)> = (0..12).map(|i| (format!("f{i:02}.csv"), format!("new {i}\n"))).collect();
    for (i, (rel, _)) in set.iter().enumerate() {
        write(&dir, rel, &format!("old {i}\n"));
        set_time(&dir, rel, start + Duration::from_secs(86_400 * i as u64));
    }
    let shipped: Vec<(&str, &[u8])> = set.iter().map(|(r, t)| (r.as_str(), t.as_bytes())).collect();
    let r = refreshed(refresh(&dir, "2.0.0", &shipped));
    assert_eq!(r.updated, Vec::<String>::new(), "with no first-run moment nothing is overwritten");
    assert_eq!(r.kept.len(), 12);
    assert_eq!(read(&dir, "f03.csv"), "old 3\n");
}

/// A SOURCE TREE AND A LINKED FOLDER ARE LEFT BYTE FOR BYTE ALONE AND GET NO STAMP (design section
/// 4 and section 7, third test). Four folders, none of them the game's own copy:
/// - a checkout's data/ (the folder above it holds Cargo.toml), which `just launch` reads in place;
/// - a rig's data/, a junction to that checkout's data/ (every rig's, BUG-133);
/// - a junction to a plain folder that is no source tree (a link can lead anywhere);
/// - a real data/ whose `sub` folder is a junction into the checkout (probe-sweep's shipped-assets
///   rig links data/'s entries one by one).
/// Each holds files an older version wrote, which a refresh would otherwise replace.
///
/// Seen red 2026-10-12, one break of this test's at a time (each run also carried a break of
/// another test's, which is why the reports below replace files instead of keeping them):
/// - both Cargo.toml checks removed (the folder's own and where it leads): "the checkout's data/
///   is left alone: Refreshed(Report { from: None, to: \"2.0.0\", .., updated: [\"a.csv\",
///   \"sub/b.ron\"], added: [\"new/d.ron\"], .. })";
/// - the check that the folder itself is a link removed: "a junction to a plain folder is left
///   alone: Refreshed(Report { .., updated: [\"a.csv\", \"sub/b.ron\"], added: [\"new/d.ron\"],
///   .. })" (the rig's junction into the checkout was still caught, by where it leads);
/// - the per-file link check removed: "a data/ with a junctioned subfolder is left alone:
///   Refreshed(Report { .., updated: [\"a.csv\", \"sub/b.ron\"], .. })": sub/b.ron was rewritten
///   through the junction, in the checkout.
#[test]
fn a_source_tree_and_a_linked_folder_are_left_alone_byte_for_byte() {
    let root = crate::test_temp::dir("refresh_left_alone");
    let checkout = root.join("checkout");
    let checkout_data = checkout.join("data");
    std::fs::create_dir_all(&checkout_data).unwrap();
    std::fs::write(checkout.join("Cargo.toml"), "[package]\nname = \"humanity-engine\"\n").unwrap();
    for (rel, text) in V1 {
        write(&checkout_data, rel, text);
    }
    let plain = root.join("plain").join("data");
    std::fs::create_dir_all(&plain).unwrap();
    for (rel, text) in V1 {
        write(&plain, rel, text);
    }
    let rig = root.join("rig");
    std::fs::create_dir_all(&rig).unwrap();
    link_dir(&rig.join("data"), &checkout_data);
    let other = root.join("other");
    std::fs::create_dir_all(&other).unwrap();
    link_dir(&other.join("data"), &plain);
    let mirror = root.join("mirror").join("data");
    std::fs::create_dir_all(&mirror).unwrap();
    for (rel, text) in V1.iter().filter(|(rel, _)| !rel.starts_with("sub/")) {
        write(&mirror, rel, text);
    }
    link_dir(&mirror.join("sub"), &checkout_data.join("sub"));

    let before = (snapshot(&checkout_data), snapshot(&plain), snapshot(&mirror));
    let (rig_data, other_data) = (rig.join("data"), other.join("data"));
    let cases = [
        (&checkout_data, "the checkout's data/ is left alone"),
        (&rig_data, "the rig's junction into the checkout is left alone"),
        (&other_data, "a junction to a plain folder is left alone"),
        (&mirror, "a data/ with a junctioned subfolder is left alone"),
    ];
    let v2_rels: Vec<&str> = V2.iter().map(|(rel, _)| *rel).collect();
    for (dir, what) in cases {
        let outcome = refresh(dir, "2.0.0", &bytes(V2));
        assert!(matches!(outcome, Outcome::LeftAlone(_)), "{what}: {outcome:?}");
        let (written, _) = write_fresh(dir, "2.0.0", &bytes(V2));
        assert_eq!(written, 0, "{what}: a first run writes nothing there either");
        assert!(use_games_copy_in(dir, "a.csv", "2.0.0", b"a 2\n", &v2_rels).is_err(), "{what}: nor does Use the game's copy");
    }
    assert_eq!((snapshot(&checkout_data), snapshot(&plain), snapshot(&mirror)), before, "every file byte for byte as it was, and no stamp");
}

/// A REFRESH THAT DIES PART-WAY (A WRITE REFUSED) LEAVES EVERY FILE WHOLE AND THE STAMP'S OLD
/// VERSION, SO THE NEXT START FINISHES IT (design section 7, fourth test). The files it did write
/// are recorded as the game's even so: should the next start be a newer version still, they are
/// refreshed, not taken for edits.
///
/// The refused write: a folder squats where `a.csv`'s new copy is written beside it.
///
/// Seen red 2026-10-12, twice:
/// - with the stamp written with the NEW version whatever the pass did: "the stamp keeps the old
///   version until every file is written: left: Some(\"2.0.0\")";
/// - with the stamp left exactly as it was after a failed pass (the design's first wording): "what
///   was written is recorded, left: \"d1cd1ec4..\" (the hash of version 1's c.json) right:
///   \"b85947a7..\" (version 2's)". Its record still said version 1's bytes, so at the next
///   version the file version 2 wrote would have been kept as an edit (the assertion at the end).
#[test]
fn a_refresh_that_dies_part_way_leaves_every_file_whole_and_the_next_start_finishes_it() {
    let dir = crate::test_temp::path("refresh_part_way");
    write_fresh(&dir, "1.0.0", &bytes(V1));
    write(&dir, "sub/b.ron", "mine\n");
    let v2: &Set = &[("a.csv", "a 2\n"), ("sub/b.ron", "b 2\n"), ("c.json", "c 2\n"), ("new/d.ron", "d 2\n")];
    let squat = tmp_path(&dir.join("a.csv"));
    std::fs::create_dir_all(&squat).unwrap();

    let r = refreshed(refresh(&dir, "2.0.0", &bytes(v2)));
    assert_eq!(r.failed.len(), 1, "{:?}", r.failed);
    assert_eq!(r.failed[0].file, "a.csv");
    assert!(r.failed[0].why.starts_with("it could not be written"), "{}", r.failed[0].why);
    assert_eq!(read(&dir, "a.csv"), "a 1\n", "the refused file is whole, as it was");
    assert_eq!(read(&dir, "c.json"), "c 2\n", "the others went on");
    assert_eq!(read(&dir, "sub/b.ron"), "mine\n");
    let stamp = read_stamp(&dir).expect("a stamp");
    assert_eq!(stamp.version.as_deref(), Some("1.0.0"), "the stamp keeps the old version until every file is written");
    assert_eq!(stamp.files["c.json"], hash(b"c 2\n"), "what was written is recorded");
    let stray: Vec<String> = snapshot(&dir).into_keys().filter(|k| k.ends_with(".tmp")).collect();
    assert_eq!(stray, Vec::<String>::new(), "no half-written copy is left behind");

    // The next start of the same version finishes it.
    std::fs::remove_dir(&squat).unwrap();
    let r = refreshed(refresh(&dir, "2.0.0", &bytes(v2)));
    assert_eq!(r.updated, vec!["a.csv".to_string()], "only what was left");
    assert_eq!(r.kept, vec!["sub/b.ron".to_string()]);
    assert_eq!(read(&dir, "a.csv"), "a 2\n");
    assert_eq!(read_stamp(&dir).unwrap().version.as_deref(), Some("2.0.0"));

    // Had the next start been a newer version, the files the failed pass wrote are the game's.
    let dir = crate::test_temp::path("refresh_part_way_then_newer");
    write_fresh(&dir, "1.0.0", &bytes(V1));
    let squat = tmp_path(&dir.join("a.csv"));
    std::fs::create_dir_all(&squat).unwrap();
    refreshed(refresh(&dir, "2.0.0", &bytes(v2)));
    std::fs::remove_dir(&squat).unwrap();
    let v3: &Set = &[("a.csv", "a 3\n"), ("c.json", "c 3\n")];
    let r = refreshed(refresh(&dir, "3.0.0", &bytes(v3)));
    assert_eq!(r.kept, Vec::<String>::new(), "a file the failed pass did write is the game's at the next version");
    assert_eq!(read(&dir, "c.json"), "c 3\n");
}

/// THE FIRST RUN STAMPS WHAT IT WROTE: the real extraction (`storage::extract_embedded_to`)
/// records this version and the hash of every file, each matching the file on disk, so the next
/// start reads only the stamp and the first update can tell an edit from the game's own copy.
///
/// Seen red 2026-10-12 with `write_fresh` not writing the stamp: "the first run writes a stamp".
#[test]
fn the_first_run_stamps_every_file_it_wrote() {
    let dir = crate::test_temp::path("refresh_first_run");
    super::super::extract_embedded_to(&dir);
    let stamp = read_stamp(&dir).expect("the first run writes a stamp");
    assert_eq!(stamp.version.as_deref(), Some(VERSION));
    let shipped = real_shipped();
    assert_eq!(stamp.files.len(), shipped.len());
    for (rel, text) in &shipped {
        assert_eq!(stamp.files[*rel], hash(&std::fs::read(dir.join(rel)).unwrap()), "{rel}");
        assert_eq!(stamp.files[*rel], hash(text), "{rel}");
    }
    assert_eq!(refresh(&dir, VERSION, &shipped), Outcome::Current);
}

/// "USE THE GAME'S COPY" MOVES THE PLAYER'S FILE ASIDE AND NEVER DELETES IT (design section 6):
/// theirs becomes `<name>.yours-<version>` beside it (`-2` when that name is taken), the game's
/// copy takes its place, and the stamp records it and no longer lists the file as kept, so the
/// next update brings it up to date like any other.
///
/// Seen red 2026-10-12 with the move aside left out (the game's copy written over theirs): "their
/// file was moved" (nothing was kept aside).
#[test]
fn use_the_games_copy_moves_the_players_file_aside_and_never_deletes_it() {
    let dir = crate::test_temp::path("refresh_use_games_copy");
    write_fresh(&dir, "1.0.0", &bytes(V1));
    write(&dir, "sub/b.ron", "mine\n");
    let r = refreshed(refresh(&dir, "2.0.0", &bytes(V2)));
    assert_eq!(r.kept, vec!["sub/b.ron".to_string()]);

    let aside = use_games_copy_in(&dir, "sub/b.ron", "2.0.0", b"b 2\n", &[]).expect("done").expect("their file was moved");
    assert_eq!(aside, dir.join("sub").join("b.ron.yours-2.0.0"));
    assert_eq!(std::fs::read_to_string(&aside).ok().as_deref(), Some("mine\n"), "the player's file is kept aside, whole");
    assert_eq!(read(&dir, "sub/b.ron"), "b 2\n", "the game's copy is in its place");
    let stamp = read_stamp(&dir).unwrap();
    assert_eq!(stamp.files["sub/b.ron"], hash(b"b 2\n"));
    assert_eq!(stamp.last_update.unwrap().kept, Vec::<String>::new(), "no longer listed as kept");

    write(&dir, "sub/b.ron", "mine again\n");
    let second = use_games_copy_in(&dir, "sub/b.ron", "2.0.0", b"b 2\n", &[]).unwrap().unwrap();
    assert_eq!(second, dir.join("sub").join("b.ron.yours-2.0.0-2"), "a second move never overwrites the first");
    assert_eq!(std::fs::read_to_string(&aside).unwrap(), "mine\n");
    assert_eq!(std::fs::read_to_string(&second).unwrap(), "mine again\n");

    let r = refreshed(refresh(&dir, "3.0.0", &bytes(&[("sub/b.ron", "b 3\n")])));
    assert_eq!(r.updated, vec!["sub/b.ron".to_string()], "the next update brings it up to date");
}
