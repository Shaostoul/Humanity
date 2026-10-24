// Child of pages/dev_agents/mod.rs: Dev > Claude's readers, fed fixture files, and the panel drawn
// headless from a fixture report. Each test was seen failing once against a broken reader; what it
// saw red is at the test.

use super::delivery::{self, ExeInfo, Manifest, Problem};
use super::read::{self, LockState, Reading};
use super::*;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// A folder of its own under the system temp folder, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("hos-dev-claude-{tag}-{}-{}", std::process::id(), read::now_unix()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).expect("scratch folder");
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What `git worktree list --porcelain` printed on 2026-10-22, cut to five kinds of entry.
const PORCELAIN: &str = "worktree C:/Humanity\n\
HEAD 0b2ceecb01d570cf8a2d537642f0825a780ccb0c\n\
branch refs/heads/main\n\
\n\
worktree C:/Humanity/.claude/worktrees/agent-a09aebf21a04c0245\n\
HEAD 109ad311d0298ceae7076c5c3b4cf6fd144e95b9\n\
branch refs/heads/worktree-agent-a09aebf21a04c0245\n\
locked claude agent agent-a09aebf21a04c0245 (pid 16820)\n\
\n\
worktree C:/Humanity/.claude/worktrees/agent-a58cb506551d3ff08\n\
HEAD a407070ad2dbc8869e314c7bf5650014814d975a\n\
branch refs/heads/worktree-agent-a58cb506551d3ff08\n\
\n\
worktree C:/Humanity/.claude/worktrees/agent-detached\n\
HEAD 1195483357a3ccbc65e67e091d02fb597feac9a9\n\
detached\n\
\n\
worktree C:/Humanity/.claude/worktrees/agent-gone\n\
HEAD a16e87453135f720272061f7e5fdd8264a263b60\n\
branch refs/heads/worktree-agent-gone\n\
prunable gitdir file points to non-existent location\n";

/// THE WORKTREE LISTING PARSES: five entries, the first marked the main checkout, branches without
/// refs/heads/, a detached one with no branch, the harness's lock with its pid, a prunable one.
///
/// Seen red 2026-10-23 with the main-checkout mark left out: "assertion `left == right` failed:
/// the first entry is the main checkout; left: false; right: true".
#[test]
fn the_worktree_listing_parses() {
    let list = read::parse_worktree_list(PORCELAIN);
    assert_eq!(list.len(), 5, "{list:#?}");
    assert_eq!(list[0].main, true, "the first entry is the main checkout");
    assert!(list[1..].iter().all(|w| !w.main), "only the first is the main checkout");
    assert_eq!(list[0].branch.as_deref(), Some("main"));
    assert_eq!(list[0].name(), "Humanity");
    assert_eq!(list[1].name(), "agent-a09aebf21a04c0245");
    assert_eq!(list[1].branch.as_deref(), Some("worktree-agent-a09aebf21a04c0245"));
    assert_eq!(list[1].head, "109ad311d0298ceae7076c5c3b4cf6fd144e95b9");
    assert_eq!(list[1].lock_pid(), Some(16820), "the harness names its pid in the lock");
    assert_eq!(list[2].locked, None, "a finished agent's worktree is not locked");
    assert_eq!(list[3].branch, None, "a detached HEAD has no branch");
    assert!(list[4].prunable);
    // Windows line endings from git make no difference.
    assert_eq!(read::parse_worktree_list(&PORCELAIN.replace('\n', "\r\n")), list);
}

/// LAST COMMITS AND UNCOMMITTED FILES: `git log`'s tab-separated lines map each head to its time
/// and subject; `git status`'s lines count, except the harness's own settings file, which every
/// agent worktree carries and is nobody's work.
///
/// Seen red 2026-10-23 with the IGNORED_CHANGES filter removed: "assertion `left == right` failed:
/// the harness's settings file is not work; left: 3; right: 2".
#[test]
fn last_commits_and_uncommitted_files() {
    let log = read::parse_log(
        "0b2ceecb01d570cf8a2d537642f0825a780ccb0c\t1761200000\tThe share guard leaves a test's own processes alone\n\
         109ad311d0298ceae7076c5c3b4cf6fd144e95b9\t1761190000\tPRIORITIES and journal: worktrees cleaned\tand a tab\n",
    );
    assert_eq!(log.len(), 2);
    assert_eq!(log["0b2ceecb01d570cf8a2d537642f0825a780ccb0c"], (1_761_200_000, "The share guard leaves a test's own processes alone".to_string()));
    assert_eq!(log["109ad311d0298ceae7076c5c3b4cf6fd144e95b9"].1, "PRIORITIES and journal: worktrees cleaned\tand a tab", "a tab in a subject stays");
    let status = " M .claude/settings.local.json\n M src/gui/pages/dev_tab.rs\n?? src/gui/pages/dev_agents/\n";
    assert_eq!(read::count_uncommitted(status), 2, "the harness's settings file is not work");
    assert_eq!(read::count_uncommitted(" M .claude/settings.local.json\n"), 0, "a landed worktree reads as clean");
    assert_eq!(read::count_uncommitted(""), 0);
}

/// THE ORDER: the main checkout first, then the newest commit first.
///
/// Seen red 2026-10-23 with the sort turned oldest first: "left: [\"Humanity\",
/// \"agent-a09aebf21a04c0245\", \"agent-a58cb506551d3ff08\", \"agent-detached\", \"agent-gone\"]".
#[test]
fn the_main_checkout_first_then_the_newest() {
    let mut list = read::parse_worktree_list(PORCELAIN);
    for (i, w) in list.iter_mut().enumerate() {
        w.committed_at = Some(1000 + i as i64 * 10);
    }
    list[0].committed_at = Some(1); // the oldest, and still first
    read::sort_worktrees(&mut list);
    let names: Vec<String> = list.iter().map(|w| w.name()).collect();
    assert_eq!(names, ["Humanity", "agent-gone", "agent-detached", "agent-a58cb506551d3ff08", "agent-a09aebf21a04c0245"]);
}

/// HANDOFF NOTES: every `.md` in `.claude/handoff/` of the checkout and of each worktree, titled by
/// its first line, newest first; a folder with no handoff folder has none (not an error), and a
/// file that is not markdown is not a note.
///
/// Seen red 2026-10-23 with the sort turned oldest first: "newest first, from both folders, the
/// .txt left out; left: [(\"agent-x\", \"Plain first line, no heading\"), (\"main\", ...), ...]".
#[test]
fn handoff_notes_are_read_from_each_folder() {
    let dir = Scratch::new("notes");
    let main = dir.0.join("main");
    let wt = dir.0.join("agent-x");
    for (d, file, text, age) in [
        (&main, "maps.md", "\n# Handoff: Maps increment 3\n\nDone: the level picker.\n", 600u64),
        (&wt, "dev-claude-panel.md", "# Handoff: Dev > Claude panel\n", 60),
        (&wt, "old.md", "Plain first line, no heading\n", 7200),
    ] {
        let folder = d.join(".claude").join("handoff");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(file);
        std::fs::write(&path, text).unwrap();
        let when = std::time::SystemTime::now() - std::time::Duration::from_secs(age);
        std::fs::File::options().write(true).open(&path).unwrap().set_modified(when).unwrap();
    }
    std::fs::write(wt.join(".claude").join("handoff").join("notes.txt"), "not a note").unwrap();
    assert_eq!(read::note_title("\n\n## Handoff: x  \nbody"), "Handoff: x");
    assert!(read::read_notes(&dir.0.join("nowhere"), "nowhere").is_empty(), "no folder, no notes");
    let worktrees = vec![
        read::Worktree { path: main.display().to_string(), main: true, ..Default::default() },
        read::Worktree { path: wt.display().to_string(), ..Default::default() },
    ];
    let notes = read::all_notes(&worktrees);
    let titles: Vec<(&str, &str)> = notes.iter().map(|n| (n.place.as_str(), n.title.as_str())).collect();
    assert_eq!(
        titles,
        [("agent-x", "Handoff: Dev > Claude panel"), ("main", "Handoff: Maps increment 3"), ("agent-x", "Plain first line, no heading")],
        "newest first, from both folders, the .txt left out"
    );
}

/// THE JOURNAL'S TAIL: the newest three decisions (the journal keeps the newest at the bottom),
/// newest first, each on one line, dated by `date` or by `at` in the older entries.
///
/// Seen red 2026-10-23 with the reversal left out: "assertion `left == right` failed: newest first;
/// left: \"2026-10-18\"; right: \"2026-10-22\"".
#[test]
fn the_journal_tail_is_the_newest_three_newest_first() {
    let json = r#"{
        "current_focus": "x",
        "recent_decisions": [
            {"date": "2026-10-18", "decision": "oldest"},
            {"date": "2026-10-19", "decision": "older"},
            {"date": "2026-10-20", "decision": "third newest"},
            {"at": "2026-10-21T08:00:00Z", "decision": "second\n  newest,\twith   breaks"},
            {"date": "2026-10-22", "decision": "the newest", "why": "because"}
        ]
    }"#;
    let tail = read::journal_tail(json, 3).expect("parses");
    assert_eq!(tail.len(), 3);
    assert_eq!(tail[0].date, "2026-10-22", "newest first");
    assert_eq!(tail[0].text, "the newest");
    assert_eq!(tail[1].date, "2026-10-21T08:00:00Z", "an older entry's `at` dates it");
    assert_eq!(tail[1].text, "second newest, with breaks", "one line each");
    assert_eq!(tail[2].text, "third newest");
    assert!(read::journal_tail("{}", 3).is_err(), "no list is said, not shown as an empty journal");
    assert!(read::journal_tail("not json", 3).is_err());
}

/// TIER 0'S HEADINGS: the `###` headings under `## TIER 0`, in order, stopping at the next `##`.
///
/// Seen red 2026-10-23 with the stop at the next `##` left out: "the next tier is not TIER 0:
/// [\"DONE 2026-10-21/22: the five pieces\", \"THIS WEEK: playable multiplayer\", \"Hardening\"]".
#[test]
fn tier0_headings_stop_at_the_next_tier() {
    let md = "# HumanityOS: Priorities\n\n## How to read this\n### Not this one\n\n## TIER 0: the next thing to work on\n\
              Some prose.\n#3) not a heading\n### DONE 2026-10-21/22: the five pieces\ntext\n### THIS WEEK: playable multiplayer\n\n\
              ## TIER 1: hardening\n### Hardening\n";
    let h = read::tier0_headings(md, 6);
    assert_eq!(h, ["DONE 2026-10-21/22: the five pieces", "THIS WEEK: playable multiplayer"], "the next tier is not TIER 0: {h:?}");
    assert_eq!(read::tier0_headings(md, 1).len(), 1, "at most max");
    assert!(read::tier0_headings("# nothing here\n", 6).is_empty());
}

/// A MISSING FILE IS "NOT RUNNING", NOT AN ERROR: no share-guard.json reads as Absent and the panel
/// says the guard has never run here; a stale one says it stopped; a fresh one (with the
/// byte-order mark PowerShell writes) says it is running. The same for the disk guard, and a
/// missing boot lock is a free lock.
///
/// Seen red 2026-10-23 with a missing file read as unreadable: "a missing file is Absent:
/// Unreadable(\"...\\share-guard.json: The system cannot find the file specified. (os error 2)\")".
#[test]
fn a_missing_file_reads_as_not_running() {
    let theme = crate::gui::theme::load_theme();
    let dir = Scratch::new("guards");
    let missing = read::read_text(&dir.0.join("share-guard.json"));
    assert!(matches!(missing, Reading::Absent(_)), "a missing file is Absent: {missing:?}");
    let g = missing.then(|t| read::parse_share_guard(&t));
    let (text, _) = share_guard_text(&g, 1_000_000, &theme);
    assert_eq!(text, tr("share_never"));
    assert!(text.starts_with("not running"), "{text}");

    // The guard's own file, as PowerShell 5.1 writes it: a byte-order mark, then JSON.
    let at = "2026-10-22T23:16:13.5876404-07:00";
    let at_unix = read::parse_rfc3339(at).unwrap();
    std::fs::write(
        dir.0.join("share-guard.json"),
        format!("\u{feff}{{\r\n    \"at\":  \"{at}\",\r\n    \"kept_for_the_operator\":  2,\r\n    \"dev_processes_now\":  17,\r\n    \"logical_processors\":  12,\r\n    \"priority_changes_since_start\":  4\r\n}}\r\n"),
    )
    .unwrap();
    let g = read::read_text(&dir.0.join("share-guard.json")).then(|t| read::parse_share_guard(&t));
    let running = share_guard_text(&g, at_unix + 40, &theme).0;
    assert!(running.starts_with("running") && running.contains("17") && running.contains("2 of 12"), "{running}");
    let stopped = share_guard_text(&g, at_unix + 3 * 3600, &theme).0;
    assert!(stopped.starts_with("not running: its last pass was 3 h ago"), "{stopped}");

    let disk = read::read_text(&dir.0.join("disk-guard-last.json")).then(|t| read::parse_disk_guard(&t));
    assert_eq!(disk_guard_text(&disk, 0, &theme).0, tr("disk_never"));
    std::fs::write(
        dir.0.join("disk-guard-last.json"),
        r#"{"at":"2026-10-23T06:05:05.551Z","dry_run":false,"before_pct":45.8,"after_pct":45.8,"free_gb":1084,"actions":[],"skipped":"busy: cargo.exe, rustc.exe running, nothing touched"}"#,
    )
    .unwrap();
    let disk = read::read_text(&dir.0.join("disk-guard-last.json")).then(|t| read::parse_disk_guard(&t));
    let at = read::parse_rfc3339("2026-10-23T06:05:05.551Z").unwrap();
    let ran = disk_guard_text(&disk, at + 600, &theme).0;
    assert_eq!(ran, "ran 10 min ago: skipped (busy: cargo.exe, rustc.exe running, nothing touched). C: was 46% used, 1084 GB free");
    assert!(disk_guard_text(&disk, at + 5 * 3600, &theme).0.starts_with("last ran 5 h ago, over three hours ago"));

    let lock = read::lock_state(&read::read_text(&dir.0.join("rig-boot.lock")), None);
    assert_eq!(lock, Reading::Found(LockState::Free), "no lock file: no rig is booting");
    assert_eq!(boot_lock_text(&lock, 0, &theme).0, tr("lock_free"));
}

/// THE BOOT LOCK: a holder that is running holds it; one whose pid is gone left it behind (the
/// next rig takes it over, lib/boot-lock.js); a file with no JSON is being written or damaged.
///
/// Seen red 2026-10-23 with a holder that has stopped still counted as holding: "Found(Held(BootLock
/// { pid: Some(4242), rig: \"probe-sweep\", since: Some(1792735200) }))".
#[test]
fn the_boot_lock_names_its_holder() {
    let theme = crate::gui::theme::load_theme();
    let text = Reading::Found(
        r#"{"pid": 4242, "start_ms": 1761200000000, "rig": "probe-sweep", "script": "C:\\Humanity\\scripts\\probe-sweep.js", "cwd": "C:\\Humanity", "since": "2026-10-23T06:00:00.000Z"}"#.to_string(),
    );
    let since = read::parse_rfc3339("2026-10-23T06:00:00.000Z").unwrap();
    let running: HashSet<u32> = [4242].into_iter().collect();
    let held = read::lock_state(&text, Some(&running));
    assert!(matches!(&held, Reading::Found(LockState::Held(l)) if l.rig == "probe-sweep" && l.pid == Some(4242)), "{held:?}");
    assert_eq!(boot_lock_text(&held, since + 240, &theme).0, "held by probe-sweep (pid 4242), taken 4 min ago");
    let stale = read::lock_state(&text, Some(&HashSet::new()));
    assert!(matches!(stale, Reading::Found(LockState::Stale(_))), "{stale:?}");
    assert!(read::lock_state(&text, None).found().is_some_and(|s| matches!(s, LockState::Held(_))), "unknown counts as held, as the rigs count it");
    assert_eq!(read::lock_state(&Reading::Found(String::new()), None), Reading::Found(LockState::Unreadable));
}

/// THE BUILD SLOT: the compiling (`held-<pid>.txt`) listed before the waiting (`wait-<pid>.txt`),
/// each oldest first; a record whose process is gone, a junk file and the admission lock left out;
/// no slot folder reads as "not set up", not as an error.
///
/// Seen red 2026-10-23 with the running-process filter left out: "compiling first, then waiting
/// oldest first, the gone one left out; left: [(300, false), (100, false), (201, true), (200,
/// true)]; right: [(100, false), (201, true), (200, true)]".
#[test]
fn the_build_slot_lists_who_compiles_and_who_waits() {
    let theme = crate::gui::theme::load_theme();
    let dir = Scratch::new("slot");
    let rec = |pid: u32, kind: &str, mb: u64, since_ms: i64, place: &str| {
        format!(
            "pid={pid}\nstart=134372778611558254\nchild_pid=1\nkind={kind}\ncrate=humanity_engine\nmb={mb}\nsince_ms={since_ms}\ncwd=C:\\Humanity\\.claude\\worktrees\\{place}\n"
        )
    };
    std::fs::write(dir.0.join("held-100.txt"), rec(100, "lib-test-native", 14950, 1_761_200_100_000, "agent-adf426edfa8138122")).unwrap();
    std::fs::write(dir.0.join("wait-200.txt"), rec(200, "lib-check-native", 6041, 1_761_200_050_000, "agent-abce505a06b97567b")).unwrap();
    std::fs::write(dir.0.join("wait-201.txt"), rec(201, "bin-build-relay", 3277, 1_761_200_000_000, "agent-a29f6d5cc50304f11")).unwrap();
    std::fs::write(dir.0.join("held-300.txt"), rec(300, "lib-build-native", 9800, 1_761_199_000_000, "gone")).unwrap();
    std::fs::write(dir.0.join("admit.lock"), "pid=100\n").unwrap();
    std::fs::write(dir.0.join("held-junk.txt"), "not a record").unwrap();
    let running: HashSet<u32> = [100, 200, 201].into_iter().collect();
    let slot = read::read_build_slot(&dir.0, Some(&running));
    let list = slot.found().expect("read").clone();
    let seen: Vec<(u32, bool)> = list.iter().map(|e| (e.pid, e.waiting)).collect();
    assert_eq!(seen, [(100, false), (201, true), (200, true)], "compiling first, then waiting oldest first, the gone one left out");
    assert_eq!(list[0].place(), "agent-adf426edfa8138122");
    assert_eq!(list[0].since, Some(1_761_200_100));
    assert_eq!(build_slot_text(&slot, &theme).0, "1 compiling, 2 waiting their turn for memory");
    assert_eq!(
        slot_line(&list[0], 1_761_200_203),
        "compiling humanity_engine (lib-test-native) in agent-adf426edfa8138122, up to 14.6 GB, started 103 s ago"
    );
    assert_eq!(
        slot_line(&list[2], 1_761_200_477),
        "waiting: humanity_engine (lib-check-native) in agent-abce505a06b97567b, needs 5.9 GB, since 7 min ago"
    );
    let none = read::read_build_slot(&dir.0.join("nowhere"), None);
    assert!(matches!(none, Reading::Absent(_)), "{none:?}");
    assert_eq!(build_slot_text(&none, &theme).0, tr("slot_none"));
    assert_eq!(build_slot_text(&Reading::Found(vec![]), &theme).0, tr("slot_idle"));
}

/// TIMES: JavaScript's `...Z` and PowerShell's round-trip format with an offset land on the same
/// Unix second.
///
/// Seen red 2026-10-23 with the offset ignored: "assertion `left == right` failed; left:
/// Some(1792736173); right: Some(1792710973)".
#[test]
fn times_from_both_writers() {
    assert_eq!(read::parse_rfc3339("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(read::parse_rfc3339("2026-10-23T06:16:13.000Z"), read::parse_rfc3339("2026-10-22T23:16:13.5876404-07:00"));
    assert_eq!(read::parse_rfc3339("2000-03-01T00:00:00+01:00"), Some(951_865_200));
    assert_eq!(read::parse_rfc3339("yesterday"), None);
}

/// A WORKTREE READS THE MAIN CHECKOUT: the panel run from an agent's worktree (or its
/// target/release) reports on the main checkout its `.git` file names, where the journal is
/// written and the taskbar exe sits.
///
/// Seen red 2026-10-23 with a worktree reporting on itself: "from a worktree's target/release; left:
/// Some(\"...\\main\\.claude\\worktrees\\agent-x\")".
#[test]
fn a_worktree_resolves_to_its_main_checkout() {
    assert_eq!(read::main_from_gitdir("gitdir: C:/Humanity/.git/worktrees/agent-x\n"), Some(PathBuf::from("C:/Humanity")));
    assert_eq!(read::main_from_gitdir("gitdir: C:/elsewhere/modules/x\n"), None);
    let dir = Scratch::new("checkout");
    let main = dir.0.join("main");
    let wt = main.join(".claude").join("worktrees").join("agent-x");
    for d in [&main, &wt] {
        std::fs::create_dir_all(d.join("data").join("coordination")).unwrap();
        std::fs::write(d.join("Cargo.toml"), "[package]\nversion = \"0.1.0\"\n").unwrap();
    }
    std::fs::create_dir_all(main.join(".git").join("worktrees").join("agent-x")).unwrap();
    std::fs::write(wt.join(".git"), format!("gitdir: {}\n", main.join(".git").join("worktrees").join("agent-x").display())).unwrap();
    let release = wt.join("target").join("release");
    std::fs::create_dir_all(&release).unwrap();
    assert_eq!(read::find_checkout(&[release]), Some(main.clone()), "from a worktree's target/release");
    assert_eq!(read::find_checkout(&[main.join("data")]), Some(main.clone()), "from the main checkout's data folder");
    assert_eq!(read::find_checkout(&[dir.0.join("nowhere")]), None);
}

/// VERSIONS: Cargo.toml's package version, and the comparison with a release tag.
///
/// Seen red 2026-10-23 with the versions compared as text: "by number, not by text; left: Less;
/// right: Greater".
#[test]
fn versions_compare_by_number() {
    assert_eq!(read::cargo_version("[package]\nname = \"humanity-engine\"\nversion = \"0.1543.1\"\n").as_deref(), Some("0.1543.1"));
    assert_eq!(read::compare_versions("0.1543.1", "v0.1543.1"), std::cmp::Ordering::Equal);
    assert_eq!(read::compare_versions("0.1543.10", "v0.1543.9"), std::cmp::Ordering::Greater, "by number, not by text");
    assert_eq!(read::compare_versions("0.999.0", "v0.1000.0"), std::cmp::Ordering::Less);
    let rel = read::parse_release(r#"{"tag_name":"v0.1543.1","published_at":"2026-10-23T04:00:00Z","name":"v0.1543.1"}"#).unwrap();
    assert_eq!(rel.tag, "v0.1543.1");
    assert!(read::parse_release(r#"{"message":"API rate limit exceeded"}"#).unwrap_err().contains("rate limit"));
}

// ── Delivery ─────────────────────────────────────────────────────────────────────────────────

/// A stamp's text exactly as build.rs `write_source_stamp` lays it out.
fn stamp_text(files: &Manifest) -> String {
    let mut manifest = String::from("hos-src-manifest v1\n");
    for (path, hash) in files {
        manifest.push_str(&format!("{hash} {path}\n"));
    }
    let fp = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(manifest.as_bytes()));
    format!("{}1\nfingerprint {fp}\nfiles {}\nprofile release\nfeatures default,native\n{manifest}{}", delivery::stamp_begin(), files.len(), delivery::stamp_end())
}

fn tree() -> Manifest {
    [("src/lib.rs", "a"), ("src/main.rs", "b"), ("Cargo.toml", "c"), ("assets/shaders/pbr/90-fragment-main.wgsl", "d"), ("assets/shaders/cloud_resolve.wgsl", "e")]
        .into_iter()
        .map(|(p, h)| (p.to_string(), h.repeat(64)))
        .collect()
}

fn exe_with(changes: &[(&str, Option<&str>)]) -> ExeInfo {
    let mut files = tree();
    for (path, hash) in changes {
        match hash {
            Some(h) => files.insert(path.to_string(), h.repeat(64)),
            None => files.remove(*path),
        };
    }
    ExeInfo { exists: true, stamp: Some(delivery::Stamp { fingerprint: String::new(), files }) }
}

const NONE: ExeInfo = ExeInfo { exists: false, stamp: None };
const NO_PROBLEMS: [Problem; 0] = [];

/// THE VERDICT IS CHECK-DELIVERY'S: the cases of scripts/tests/check-delivery.test.js, through
/// the Rust port.
///
/// Seen red 2026-10-23 with the ignore list emptied: "a version bump alone does not call the exe
/// stale: [NotBuilt { differ: 2, first: \"Cargo.toml\" }]".
#[test]
fn the_delivery_verdict_matches_check_delivery() {
    let t = tree();
    assert_eq!(delivery::judge(&exe_with(&[]), &NONE, &t), NO_PROBLEMS, "the taskbar exe built from this tree is current");
    let old_lib = exe_with(&[("src/lib.rs", Some("f"))]);
    assert_eq!(
        delivery::judge(&old_lib, &exe_with(&[]), &t),
        [Problem::NotDelivered { differ: 1, first: "src/lib.rs".into() }],
        "built, not archived"
    );
    assert_eq!(delivery::judge(&old_lib, &old_lib, &t), [Problem::NotBuilt { differ: 1, first: "src/lib.rs".into() }], "built nowhere");
    let bumped = exe_with(&[("Cargo.toml", Some("f")), ("Cargo.lock", Some("f"))]);
    let v = delivery::judge(&bumped, &NONE, &t);
    assert_eq!(v, NO_PROBLEMS, "a version bump alone does not call the exe stale: {v:?}");
    assert_eq!(delivery::judge(&exe_with(&[("assets/shaders/pbr/90-fragment-main.wgsl", Some("f"))]), &NONE, &t), NO_PROBLEMS, "a live PBR edit needs no rebuild");
    assert_eq!(
        delivery::judge(&exe_with(&[("assets/shaders/cloud_resolve.wgsl", Some("f"))]), &NONE, &t),
        [Problem::NotBuilt { differ: 1, first: "assets/shaders/cloud_resolve.wgsl".into() }],
        "a compiled-in shader needs a rebuild"
    );
    assert_eq!(delivery::judge(&exe_with(&[("src/main.rs", None)]), &NONE, &t), [Problem::NotBuilt { differ: 1, first: "src/main.rs".into() }], "only in the tree");
    assert_eq!(delivery::judge(&exe_with(&[("src/gone.rs", Some("g"))]), &NONE, &t), [Problem::NotBuilt { differ: 1, first: "src/gone.rs".into() }], "only in the build");
    let no_stamp = ExeInfo { exists: true, stamp: None };
    assert_eq!(delivery::judge(&no_stamp, &exe_with(&[]), &t), [Problem::TaskbarNoStamp]);
    assert_eq!(delivery::judge(&NONE, &exe_with(&[]), &t), [Problem::TaskbarMissing]);
    assert_eq!(delivery::judge(&NONE, &NONE, &t), [Problem::TaskbarMissing, Problem::BuiltMissing]);
    assert_eq!(delivery::judge(&no_stamp, &old_lib, &t), [Problem::TaskbarNoStamp, Problem::BuiltStale]);
}

/// A STAMP IS FOUND IN AN EXE'S BYTES, AND ONLY A WHOLE ONE COUNTS: one stamp among other bytes
/// parses back to its manifest; a changed hash (the manifest no longer hashes to its fingerprint),
/// two stamps, or a begin marker with nothing after it all read as no stamp, as stampOf decides.
///
/// Seen red 2026-10-23 with the fingerprint check left out of parse_stamp: "a garbled manifest is
/// refused: Some(Stamp { fingerprint: \"37b787e2...\", files: { ... } })".
#[test]
fn a_whole_stamp_is_found_and_a_garbled_one_refused() {
    let files = tree();
    let text = stamp_text(&files);
    let mut exe = vec![0u8; 4096];
    exe.extend_from_slice(text.as_bytes());
    exe.extend_from_slice(&[7u8; 4096]);
    let stamp = delivery::stamp_in(&exe).expect("the stamp is found");
    assert_eq!(stamp.files, files);
    let garbled = text.replacen(&"a".repeat(64), &"0".repeat(64), 1);
    let found = delivery::stamp_in(garbled.as_bytes());
    assert!(found.is_none(), "a garbled manifest is refused: {found:?}");
    assert!(delivery::stamp_in(format!("{text}{text}").as_bytes()).is_none(), "two stamps vouch for nothing");
    assert!(delivery::stamp_in(format!("{}1\n", delivery::stamp_begin()).as_bytes()).is_none(), "a marker with no stamp");
    assert!(delivery::stamp_in(b"no stamp at all").is_none());
}

/// THE MARKERS ARE NOT IN THE BINARY: the scripts refuse an exe holding a begin marker with no
/// well-formed stamp after it, so a literal marker in this module would break every rig's
/// freshness gate. This test program holds all of this module's code; the forward marker must not
/// be anywhere in it (it is built at run time from a reversed string).
///
/// Seen red 2026-10-23 with stamp_begin returning the marker as a plain literal: "the stamp's
/// begin marker is written out somewhere in the compiled code; left: Some(184819720); right: None".
#[test]
fn markers_are_not_in_the_binary() {
    let exe = std::fs::read(std::env::current_exe().expect("the test program's path")).expect("read the test program");
    let begin = delivery::stamp_begin();
    assert_eq!(begin.len(), 15);
    assert_eq!(delivery::find(&exe, begin.as_bytes(), 0), None, "the stamp's begin marker is written out somewhere in the compiled code");
}

/// THE PORT FINGERPRINTS THE TREE THE WAY build.rs DOES: build.rs wrote this crate's stamp into
/// OUT_DIR when this test program was built; the Rust port, run over the same tree now, lists the
/// same files with the same hashes.
///
/// Seen red 2026-10-23 with CRLF kept as it is (and the ignore list emptied): "the same hashes
/// (first difference: Some(\"Cargo.lock\")); left: 1103; right: 0".
#[test]
fn the_tree_fingerprint_matches_build_rs() {
    let inputs = delivery::read_inputs(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs")).unwrap()).unwrap();
    assert_eq!(&inputs[..2], ["src", "assets/shaders"]);
    assert!(delivery::read_inputs("const FINGERPRINT_INPUTS: &[&str] = &[src];").is_err(), "a list that is not string literals");
    assert!(delivery::read_inputs("nothing").is_err());
    let built = std::fs::read_to_string(concat!(env!("OUT_DIR"), "/hos_src_stamp.txt")).expect("build.rs's stamp");
    let stamp = delivery::parse_stamp(&built).expect("build.rs's stamp parses");
    let tree = delivery::fingerprint_tree(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the tree");
    let (differ, first) = delivery::code_diff(&stamp.files, &tree);
    assert_eq!(stamp.files.len(), tree.len(), "the same file list");
    assert_eq!(differ, 0, "the same hashes (first difference: {first:?})");
}

// ── The panel ────────────────────────────────────────────────────────────────────────────────

/// Every key the panel uses is in en.json, as a string, with no em dash.
///
/// Seen red 2026-10-23 with a key added that en.json lacks: "en.json has
/// dev_claude.zz_not_in_en_json".
#[test]
fn every_key_is_in_en_json() {
    let en: serde_json::Value = serde_json::from_str(include_str!("../../../../data/i18n/en.json")).unwrap();
    let section = en.get("dev_claude").expect("en.json has a dev_claude section");
    for key in KEYS {
        let v = section.get(*key).and_then(|v| v.as_str()).unwrap_or_else(|| panic!("en.json has dev_claude.{key}"));
        assert!(!v.contains('\u{2014}'), "dev_claude.{key} has an em dash");
    }
}

/// The report the snapshot and the drawing test show: a main checkout with two files of its own,
/// an agent at work, a finished one, the journal, TIER 0, a delivered exe, the share guard never
/// run, the disk guard an hour ago, a free lock.
pub(crate) fn fixture(now: i64) -> Report {
    let mut worktrees = read::parse_worktree_list(PORCELAIN);
    worktrees.truncate(3);
    let subjects = [
        "The share guard leaves a test's own processes alone (an ignore file), fixing a race",
        "PRIORITIES and journal: worktrees cleaned, the VPS nginx edit made (reload pending)",
        "v0.1543.0: the Sun has a body",
    ];
    for (i, w) in worktrees.iter_mut().enumerate() {
        w.committed_at = Some(now - 300 - i as i64 * 3000);
        w.subject = Some(subjects[i].to_string());
        w.uncommitted = Some([2, 5, 0][i]);
    }
    worktrees[1].holder_running = Some(true);
    Report {
        checkout: Reading::Found(PathBuf::from("C:/Humanity")),
        worktrees: Reading::Found(worktrees),
        notes: Reading::Found(vec![read::Note {
            title: "Handoff: Maps increment 3 (into the ship deck by deck)".into(),
            place: "agent-a09aebf21a04c0245".into(),
            path: "C:/Humanity/.claude/worktrees/agent-a09aebf21a04c0245/.claude/handoff/maps.md".into(),
            modified: Some(now - 600),
        }]),
        decisions: Reading::Found(vec![
            read::Decision { date: "2026-10-22".into(), text: "Cleaned the worktrees (operator: let's clean things up): 241 removed, the 6 running agents' kept, every branch kept.".into() },
            read::Decision { date: "2026-10-22".into(), text: "Usage reset (operator: let's resume). The power-meters and flaky-upload-tests agents resumed from their worktrees.".into() },
            read::Decision { date: "2026-10-22".into(), text: "Released v0.1543.0: the Sun as a body (the operator's two screenshots).".into() },
        ]),
        tier0: Reading::Found(vec![
            "DONE 2026-10-21/22: the five pieces paused at the usage cap all shipped".into(),
            "THIS WEEK (planned 2026-10-02): playable multiplayer, proven with two people".into(),
            "RESUME HERE: where the work stands (updated 2026-10-10)".into(),
        ]),
        version: Reading::Found("0.1543.1".into()),
        release: Reading::Found(read::Release { tag: "v0.1543.1".into(), published_at: Some(now - 7200) }),
        delivery: Reading::Found(delivery::Delivery { problems: vec![], taskbar_version: Some("0.1543.0".into()) }),
        share_guard: Reading::Absent("share-guard.json".into()),
        disk_guard: Reading::Found(read::DiskGuard { at: now - 3600, skipped: None, cleared: 0, used_pct: 45.8, free_gb: 1084.0 }),
        boot_lock: Reading::Found(LockState::Free),
        build_slot: Reading::Found(vec![
            read::SlotEntry {
                pid: 27052,
                kind: "lib-test-native".into(),
                crate_name: "humanity_engine".into(),
                mb: 14950,
                since: Some(now - 103),
                cwd: "C:\\Humanity\\.claude\\worktrees\\agent-adf426edfa8138122".into(),
                waiting: false,
            },
            read::SlotEntry {
                pid: 8052,
                kind: "lib-check-native".into(),
                crate_name: "humanity_engine".into(),
                mb: 6041,
                since: Some(now - 427),
                cwd: "C:\\Humanity\\.claude\\worktrees\\agent-abce505a06b97567b".into(),
                waiting: true,
            },
        ]),
        files_at: Some(now - 1),
        git_at: Some(now - 4),
        delivery_at: Some(now - 30),
        release_at: Some(now - 200),
    }
}

/// Every text drawn.
fn texts(out: &egui::FullOutput) -> Vec<String> {
    fn visit(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| visit(s, out)),
            _ => {}
        }
    }
    let mut found = Vec::new();
    for cs in &out.shapes {
        visit(&cs.shape, &mut found);
    }
    found
}

/// THE PANEL DRAWS A REPORT: the fixture drawn headless shows each card's words: the main checkout
/// and its uncommitted files, the agent at work, the journal, TIER 0, the delivered exe trailing a
/// docs-only bump, the share guard never run, a free lock.
///
/// Seen red 2026-10-23 in the full red run on its first line, "the panel shows \"Main checkout\"",
/// because the worktree listing's main-checkout mark was also broken there; and on its last
/// assertion with the branch shortcut taken out (the text list it prints holds
/// \"worktree-agent-a09aebf21a04c0245\" beside the folder's name).
#[test]
fn the_panel_draws_a_report() {
    let ctx = egui::Context::default();
    crate::gui::fonts::install_font_fallbacks(&ctx);
    let theme = crate::gui::theme::load_theme();
    theme.apply_to_egui(&ctx);
    let now = 1_761_200_000;
    let report = fixture(now);
    let mut last = None;
    for _ in 0..2 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 1000.0))), ..Default::default() };
        last = Some(ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_report(ui, &theme, &report, now));
        }));
    }
    let t = texts(&last.unwrap());
    let has = |s: &str| t.iter().any(|x| x.contains(s));
    for want in [
        "What Claude is doing",
        "Main checkout",
        "uncommitted: 2 files",
        "agent-a09aebf21a04c0245",
        "held by a running agent",
        "nothing uncommitted",
        "no agent holds it",
        "5 min ago: The share guard",
        "2026-10-22",
        "1. DONE 2026-10-21/22",
        "agent-a09aebf21a04c0245, written 10 min ago:",
        "Handoff: Maps increment 3",
        "v0.1543.1, published 2 h ago; in sync",
        "delivered: it holds this tree's code; its title bar reads v0.1543.0",
        "not running: it has never run on this computer",
        "free: no rig is booting the game",
        "1 compiling, 1 waiting their turn for memory",
        // Each slot line is one line cut to the width: their starts are what is sure to show.
        "compiling humanity_engine (lib-test-native) in agent-adf426edfa8138122",
        "waiting: humanity_engine (lib-check-native) in agent-abce505a06b97567b",
    ] {
        assert!(has(want), "the panel shows {want:?}: {t:#?}");
    }
    // An agent's branch repeats its folder's name: it is not said twice.
    assert!(!has("worktree-agent-a09aebf21a04c0245"), "{t:#?}");
}

/// THE PANEL BEFORE IT HAS READ ANYTHING, AND AWAY FROM A CHECKOUT: "Reading..." first; an exe
/// that is not in a source folder says so instead of drawing empty cards.
///
/// Seen red 2026-10-23 with the cards drawn away from a checkout: "no empty cards: [\"What Claude
/// is doing\", ..., \"Agent worktrees\", ...]".
#[test]
fn the_panel_says_when_there_is_nothing_to_read() {
    let ctx = egui::Context::default();
    let theme = crate::gui::theme::load_theme();
    let draw = |report: &Report| {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 700.0))), ..Default::default() };
        texts(&ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| draw_report(ui, &theme, report, 0));
        }))
    };
    let t = draw(&Report::default());
    assert!(t.iter().any(|s| s == &tr("waiting")), "{t:?}");
    let t = draw(&Report { checkout: Reading::Absent("Cargo.toml".into()), ..Default::default() });
    assert!(t.iter().any(|s| s.starts_with("This copy of HumanityOS is not running from the source folder")), "{t:?}");
    assert!(!t.iter().any(|s| s == &tr("worktrees")), "no empty cards: {t:?}");
}
