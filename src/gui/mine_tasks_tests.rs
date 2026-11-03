// Child of gui/mine_tasks.rs (#[path]): the rules of tasks of my own that need no socket and no
// GUI (docs/design/tasks.md sections 2 to 4). Each test was seen failing once on purpose, recorded
// at the test.

use super::*;

const FIXTURE: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/scripts/tests/fixtures/task-note-v1.json"));

fn rec(id: &str, title: &str, status: &str, at: u64) -> Record {
    Record {
        id: id.into(),
        deleted: false,
        title: title.into(),
        description: String::new(),
        status: status.into(),
        priority: "medium".into(),
        labels: vec![],
        chain: None,
        created_at: 1,
        updated_at: at,
    }
}

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

/// THE NOTE (both apps): this app builds exactly the shared fixture's text from its records and
/// reads every fixture text to exactly its records, so the desktop opens the website's notes and
/// the website the desktop's. Seen red 2026-11-02 with `created_at` declared after `updated_at` in
/// `Record`: "the note text, byte for byte" failed on the first record.
#[test]
fn the_note_is_the_shared_fixture() {
    let fx: serde_json::Value = serde_json::from_str(FIXTURE).expect("the fixture reads");
    assert_eq!(fx["marker"].as_str(), Some(CTL_TASK), "the marker is fixed (the website matches it)");
    assert_eq!(CTL_TASK, "[[hum:task:v1]]");
    let records: Vec<Record> = serde_json::from_value(fx["records"].clone()).expect("the records read");
    let texts = note_texts(&records);
    assert_eq!(texts.len(), 1, "three small records go in one note");
    assert_eq!(texts[0], fx["text"].as_str().unwrap(), "the note text, byte for byte");
    assert_eq!(parse_note(&texts[0]), records, "and it reads back");
    for case in fx["read"].as_array().expect("read cases") {
        let name = case["name"].as_str().unwrap_or("");
        let want: Vec<Record> = serde_json::from_value(case["records"].clone()).expect("the case's records");
        assert_eq!(parse_note(case["text"].as_str().unwrap()), want, "{name}");
    }
    assert!(is_note("[[hum:task:v1]]anything"), "a text starting with the marker is never a message");
    assert!(!is_note("I wrote [[hum:task:v1]] in a message"));
}

/// THE MERGE: the later change wins in any order of arrival; at the same time a deletion wins,
/// then the later status, then the higher priority; a repeat changes nothing; a deletion keeps no
/// words and an older version cannot bring the task back. Seen red 2026-11-02 with `rank` leaving
/// out the deletion: "at the same time a deletion wins" failed.
#[test]
fn the_later_change_wins_in_any_order_and_a_deletion_stays() {
    let v1 = rec(A, "Buy seeds", "todo", 10);
    let v2 = rec(A, "Buy seeds", "in_progress", 20);
    let gone = Record::gone(A, 30);
    for order in [[&v1, &v2, &gone], [&gone, &v2, &v1], [&v2, &gone, &v1]] {
        let mut m = MineTasks::in_memory("me");
        for r in order {
            m.apply(vec![(*r).clone()]);
        }
        assert!(m.tasks().is_empty(), "the deletion is the latest, whatever the order");
        assert_eq!(m.record(A), Some(Record::gone(A, 30)));
    }
    let mut m = MineTasks::in_memory("me");
    assert!(m.apply(vec![v2.clone()]));
    assert!(!m.apply(vec![v2.clone()]), "a repeat changes nothing");
    assert!(!m.apply(vec![v1.clone()]), "an older version changes nothing");
    assert_eq!(m.get(A).unwrap().status, TaskStatus::InProgress);
    assert!(m.apply(vec![Record::gone(A, 20)]), "at the same time a deletion wins");
    assert!(m.get(A).is_none());
    let mut done_same_time = rec(B, "Water the beds", "done", 50);
    let todo_same_time = rec(B, "Water the beds", "todo", 50);
    let mut m = MineTasks::in_memory("me");
    m.apply(vec![todo_same_time.clone()]);
    assert!(m.apply(vec![done_same_time.clone()]), "then the later status");
    assert!(!m.apply(vec![todo_same_time]), "and the other order agrees");
    done_same_time.priority = "critical".into();
    assert!(m.apply(vec![done_same_time]), "then the higher priority");
    assert_eq!(m.get(B).unwrap().priority, TaskPriority::Critical);
}

/// A CHANGE MADE HERE ALWAYS WINS OVER WHAT IT CHANGED, even when this device's clock is behind
/// the device that made the version it changed: it is stamped just after it. Seen red 2026-11-02
/// with `stamp` returning `now` alone: the record came back ("done", 5), not ("done", 101).
#[test]
fn a_change_here_is_stamped_after_what_it_changed() {
    let mut m = MineTasks::in_memory("me");
    m.apply(vec![rec(A, "Mend the fence", "todo", 100)]);
    assert!(m.change(A, Some(TaskStatus::Done), None, 5), "a slow clock");
    let r = m.record(A).unwrap();
    assert_eq!((r.status.as_str(), r.updated_at), ("done", 101));
    assert!(!m.change(A, Some(TaskStatus::Done), None, 200), "no change, no new version");
    assert!(m.delete(A, 6));
    assert_eq!(m.record(A).unwrap().updated_at, 102);
    assert!(!m.delete(A, 300), "deleted already");
}

/// NOTES FIT: many records go in as few notes as fit `NOTE_TEXT_MAX`, every record exactly once,
/// and the largest record the limits allow fits one note on its own, inside the largest padding
/// bucket once sealed. NOT seen red: on 2026-11-02 the fit test with the comma's byte left out
/// still passed on this data (a note lands a byte over the limit only by chance), so this test
/// guards the packing and the clamps, not that one byte.
#[test]
fn notes_carry_every_record_within_the_limit() {
    let big = Record {
        description: "\u{1F331}".repeat(DESCRIPTION_MAX + 50),
        title: "\u{00E9}".repeat(TITLE_MAX + 9),
        labels: (0..40).map(|i| format!("{i}").repeat(LABEL_MAX + 5)).collect(),
        ..rec(A, "", "todo", 1)
    }
    .clamped()
    .unwrap();
    assert_eq!(big.title.chars().count(), TITLE_MAX);
    assert_eq!(big.description.chars().count(), DESCRIPTION_MAX);
    assert_eq!(big.labels.len(), LABELS_MAX);
    let one = note_texts(&[big.clone()]);
    assert_eq!(one.len(), 1);
    assert!(one[0].len() <= NOTE_TEXT_MAX, "the largest record fits one note: {}", one[0].len());
    let many: Vec<Record> = (0..400u32).map(|i| rec(&format!("{i:032x}"), &format!("Task number {i}, with some words"), "todo", u64::from(i) + 1)).collect();
    let notes = note_texts(&many);
    assert!(notes.len() > 1 && notes.len() < 20, "{} notes", notes.len());
    for n in &notes {
        assert!(n.len() <= NOTE_TEXT_MAX, "a note of {} bytes", n.len());
    }
    let back: Vec<Record> = notes.iter().flat_map(|n| parse_note(n)).collect();
    assert_eq!(back, many, "every record, once, in order");
}

/// THE FILE: my tasks survive a restart with what was not sent yet, encrypted: the file holds no
/// word of them, and another seed reads nothing (and sets the file aside, below). Seen red
/// 2026-11-02 with `save` writing the JSON unencrypted: the restart read nothing (0 tasks, not 1),
/// the file no longer opening as the sealed file it must be.
#[test]
fn the_file_keeps_my_tasks_encrypted_across_a_restart() {
    let seed = [71u8; 32];
    let mut m = MineTasks::in_temp(&seed, "me", "file");
    let id = m.add("Call the dentist", "Ask about the crown", TaskPriority::High, vec!["health".into()], Some("first-steps".into()), 1_000);
    let other = m.add("Sharpen the hoe", "", TaskPriority::Low, vec![], None, 2_000);
    m.delete(&other, 3_000);
    m.mark_unsent(&[id.clone(), other.clone()]);
    m.save();
    let again = m.reopened(&seed);
    assert_eq!(again.tasks().len(), 1);
    let t = again.get(&id).expect("kept across a restart");
    assert_eq!((t.title.as_str(), t.priority, t.chain.as_deref()), ("Call the dentist", TaskPriority::High, Some("first-steps")));
    assert_eq!(again.record(&other), Some(Record::gone(&other, 3_000)), "with the deletion");
    assert_eq!(again.unsent(), [id.clone(), other.clone()], "and what is still to send");
    let raw = m.file_bytes_for_test();
    for word in ["dentist", "crown", "health", "Sharpen", "first-steps"] {
        assert!(!raw.windows(word.len()).any(|w| w == word.as_bytes()), "the file holds none of the words ({word})");
    }
    let other = m.reopened(&[72u8; 32]);
    assert!(other.tasks().is_empty(), "another seed reads nothing");
    let Some(OpenProblem::KeptAside(aside)) = other.problem().cloned() else { panic!("a file another seed cannot open is kept aside") };
    assert_eq!(std::fs::read(&aside).unwrap(), raw, "kept as it was");
    let _ = std::fs::remove_file(aside);
    m.remove_file_for_test();
}

/// A file of its own under the temp directory, not made yet.
fn temp_path(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("hum-tasks-test-{tag}-{nanos}.mine"))
}

/// The copies set aside beside `path` (`<name>.unreadable-<seconds>`).
fn asides(path: &std::path::Path) -> Vec<std::path::PathBuf> {
    let prefix = format!("{}.unreadable-", path.file_name().unwrap().to_string_lossy());
    std::fs::read_dir(path.parent().unwrap())
        .map(|dir| dir.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.file_name().is_some_and(|n| n.to_string_lossy().starts_with(&prefix))).collect())
        .unwrap_or_default()
}

/// A FILE THAT IS THERE BUT DOES NOT OPEN IS KEPT ASIDE, NEVER WRITTEN OVER (the review of rung 1,
/// 2026-11-02): before anything is written it is renamed to `<name>.unreadable-<seconds>` beside
/// it, byte for byte, the list starts empty and says so, and the next save writes a new file
/// without touching the copy. Until this date the list started empty and the next save wrote over
/// the damaged file, losing every task in it. Seen red 2026-11-02 with `load_at` starting empty on
/// a file that does not open, as before: "a copy was kept aside" failed (None).
#[test]
fn a_damaged_file_is_kept_aside_and_never_written_over() {
    let seed = [81u8; 32];
    let path = temp_path("damaged");
    let damaged: Vec<u8> = b"\x01\x02 not a sealed file, but perhaps recoverable \x03".repeat(20);
    std::fs::write(&path, &damaged).unwrap();
    let mut m = MineTasks::at_path_for_test(&seed, "me", path.clone());
    assert!(m.tasks().is_empty(), "the list starts empty from there");
    let aside = match m.problem() {
        Some(OpenProblem::KeptAside(p)) => p.clone(),
        other => panic!("a copy was kept aside: {other:?}"),
    };
    assert_eq!(asides(&path), vec![aside.clone()], "beside it, named for it");
    assert_eq!(std::fs::read(&aside).unwrap(), damaged, "the copy is the damaged file, byte for byte");
    m.add("Fix the gate", "", TaskPriority::Medium, vec![], None, 1);
    m.save();
    assert_eq!(std::fs::read(&aside).unwrap(), damaged, "the next save did not write over the copy");
    let again = m.reopened(&seed);
    assert_eq!((again.tasks().len(), again.problem()), (1, None), "and the new file opens");
    let _ = std::fs::remove_file(&aside);
    m.remove_file_for_test();
}

/// A FILE THAT IS NOT THERE IS AN ORDINARY EMPTY START: no problem to say, nothing set aside, and
/// the first save makes the file. Seen red 2026-11-02 with `load_at` taking every read error as a
/// read that failed, "not there" included: "no file: an empty start, nothing to say" failed.
#[test]
fn a_file_that_is_not_there_is_an_ordinary_empty_start() {
    let seed = [82u8; 32];
    let path = temp_path("absent");
    let mut m = MineTasks::at_path_for_test(&seed, "me", path.clone());
    assert!(m.tasks().is_empty() && m.problem().is_none(), "no file: an empty start, nothing to say");
    assert!(asides(&path).is_empty(), "nothing is set aside");
    m.add("Sweep the porch", "", TaskPriority::Low, vec![], None, 1);
    m.save();
    assert_eq!(m.reopened(&seed).tasks().len(), 1, "the first save makes the file");
    m.remove_file_for_test();
}

/// A FILE THAT CANNOT BE READ (for a reason other than not being there: here a folder where the
/// file should be, as a lock or a permission would refuse it) is neither set aside nor saved over
/// this session, and says so. Seen red 2026-11-02 with `load_at` starting empty on any read
/// error, as before: "a read that failed keeps the session from saving over it" failed (None).
#[test]
fn a_file_that_cannot_be_read_is_never_saved_over() {
    let seed = [83u8; 32];
    let path = temp_path("unreadable");
    std::fs::create_dir_all(&path).unwrap();
    std::fs::write(path.join("inside"), b"kept").unwrap();
    let mut m = MineTasks::at_path_for_test(&seed, "me", path.clone());
    assert!(matches!(m.problem(), Some(OpenProblem::Unreadable(_))), "a read that failed keeps the session from saving over it: {:?}", m.problem());
    m.add("Oil the hinges", "", TaskPriority::Medium, vec![], None, 1);
    m.save();
    assert!(path.is_dir() && std::fs::read(path.join("inside")).unwrap() == b"kept", "nothing was saved over it");
    assert!(asides(&path).is_empty(), "nor was it renamed: it may be whole behind a lock");
    let _ = std::fs::remove_dir_all(&path);
}

/// Ids are 32 lowercase hex and never repeat.
#[test]
fn new_ids_are_random_hex() {
    let a = new_id();
    let b = new_id();
    assert!(is_id(&a) && is_id(&b) && a != b);
    assert!(!is_id("0123456789ABCDEF0123456789abcdef") && !is_id("abc"));
}
