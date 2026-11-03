//! The default channels' tests (2026-11-02, docs/design/chat-channels.md section 4): the file is
//! whole, a new server makes the set, and a live server gains #feedback at its next start while
//! every channel its admin already set up keeps what they set.

use humanity_protocol::chat_access::{PostLevel, ReadLevel, MAIN_CHANNEL};

use super::*;

/// What a channel is on this server: its levels, place, voice, federation, local-only flag and
/// description.
fn shape(db: &Storage, id: &str) -> (ReadLevel, PostLevel, i64, bool, bool, bool, String) {
    let (read, post, _) = db.channel_access(id).unwrap_or_else(|| panic!("#{id} is there"));
    let (position, voice, description): (i64, bool, String) = db
        .with_conn(|c| {
            c.query_row(
                "SELECT position, COALESCE(voice_enabled, 1), COALESCE(description, '') FROM channels WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get::<_, i64>(1)? != 0, r.get(2)?)),
            )
        })
        .unwrap();
    (read, post, position, voice, db.is_channel_federated(id).unwrap(), db.is_channel_local_only(id).unwrap(), description)
}

fn ids(db: &Storage) -> Vec<String> {
    db.list_channels().unwrap().into_iter().map(|c| c.0).collect()
}

/// THE FILE IS WHOLE: it parses (it is built into the relay, so this is the check that a broken
/// file never reaches a server), every id is its own, #global is in it and made at every start, one
/// channel is the local room, every description is a word key en.json has, every level is a word
/// the levels know, and only a channel anyone may read is federated (only those ever travel,
/// relay/chat_access.rs `travels`). #feedback reads at Anyone and posts at Members, federated, so
/// feedback from every linked server gathers in one place.
///
/// Seen red 2026-11-02 with #feedback's federated set to false in a copy of the file: "#feedback
/// gathers from every linked server".
#[test]
fn the_default_channels_file_is_whole() {
    let all = all();
    let mut seen = std::collections::HashSet::new();
    for ch in all {
        assert!(seen.insert(ch.id.as_str()), "#{} is listed once", ch.id);
        assert_ne!(ch.words(), ch.description, "#{}: its description {} is a word key en.json has", ch.id, ch.description);
        assert!(ReadLevel::parse(&ch.read_level).is_some() && PostLevel::parse(&ch.post_level).is_some(), "#{}: its levels are words the levels know", ch.id);
        assert!(!ch.federated || ch.read() == ReadLevel::Anyone, "#{}: only a channel anyone reads is federated", ch.id);
        assert!(!(ch.federated && ch.local_only), "#{}: a local-only room is never federated", ch.id);
    }
    let main = get(MAIN_CHANNEL).expect("#global is a default channel");
    assert_eq!((main.made, main.local_only), (Made::Always, false), "#global is made at every start");
    assert_eq!(all.iter().filter(|c| c.local_only).count(), 1, "one local room");
    let feedback = get("feedback").expect("#feedback is a default channel");
    assert_eq!((feedback.read(), feedback.post()), (ReadLevel::Anyone, PostLevel::Members), "#feedback: anyone reads, people with a name post");
    assert!(feedback.federated, "#feedback gathers from every linked server");
}

/// A NEW SERVER MAKES THE SET: #announcements (Anyone / Moderators and admins, no voice), #local
/// (Members, local only), #global (Anyone / Members, federated) and #feedback (Anyone / Members,
/// federated, no voice), in the list's order, each with its description in words.
///
/// Seen red 2026-11-02 with `make_missing` making nothing: "all four, the first time" (left: [],
/// right: ["announcements", "local", "global", "feedback"]); with #feedback's federated set to false in
/// the file, its shape (left: (.., false, ..), right: (.., true, ..)).
#[test]
fn a_new_server_makes_the_set() {
    let db = Storage::open_temp("default_channels_new");
    let made = make_missing(&db, true);
    assert_eq!(made, vec!["announcements", "local", "global", "feedback"], "all four, the first time");
    let words = |id: &str| get(id).unwrap().words();
    assert_eq!(shape(&db, "announcements"), (ReadLevel::Anyone, PostLevel::Mods, 2, false, true, false, words("announcements")));
    assert_eq!(shape(&db, "local"), (ReadLevel::Members, PostLevel::Members, 3, true, false, true, words("local")));
    assert_eq!(shape(&db, "global"), (ReadLevel::Anyone, PostLevel::Members, 10, true, true, false, words("global")));
    assert_eq!(shape(&db, "feedback"), (ReadLevel::Anyone, PostLevel::Members, 11, false, true, false, words("feedback")));
    assert_eq!(ids(&db), vec!["announcements", "local", "global", "feedback"], "in the list's order");
    assert!(make_missing(&db, true).is_empty(), "a second start makes nothing");
}

/// A LIVE SERVER GAINS #FEEDBACK AT ITS NEXT START, and every channel its admin already set up keeps
/// what they set (here a #global moved to the top with words of its own, an #announcements whose
/// voice was turned on). #feedback, once deleted by an admin, stays deleted; #global, deleted by
/// hand in the database, comes back; #local comes back while the guaranteed local room is on, and
/// not when it is off; a #local that became ordinary while it was off is the local room again.
///
/// Seen red 2026-11-02 with `make_missing` making nothing: "a live server gains #feedback" (left: [],
/// right: ["feedback"]); with `once` channels not recorded as made: "a deleted #feedback stays
/// deleted" (left: ["feedback"], right: []); without the re-flag of an existing #local: "#local is
/// local only again" (the old relay/mod.rs block flagged it; the first version of this one did not).
#[test]
fn a_live_server_gains_feedback_and_keeps_what_its_admin_set() {
    let db = Storage::open_temp("default_channels_live");
    db.create_channel("announcements", "announcements", Some("Project updates and news"), "system", PostLevel::Mods).unwrap();
    db.set_channel_access("announcements", ReadLevel::Anyone, PostLevel::Mods).unwrap();
    db.set_channel_position("announcements", 2).unwrap();
    db.create_channel("local", "local", Some("This server only."), "system", PostLevel::Members).unwrap();
    db.set_channel_local_only("local", true).unwrap();
    db.create_channel("global", "global", Some("Talk here"), "system", PostLevel::Members).unwrap();
    db.set_channel_access("global", ReadLevel::Anyone, PostLevel::Members).unwrap();
    db.set_channel_position("global", 1).unwrap();
    let before = (shape(&db, "announcements"), shape(&db, "local"), shape(&db, "global"));

    assert_eq!(make_missing(&db, true), vec!["feedback"], "a live server gains #feedback");
    assert_eq!(shape(&db, "feedback").0, ReadLevel::Anyone);
    assert_eq!((shape(&db, "announcements"), shape(&db, "local"), shape(&db, "global")), before, "and keeps what its admin set");

    db.delete_channel("feedback").unwrap();
    assert_eq!(make_missing(&db, true), Vec::<String>::new(), "a deleted #feedback stays deleted");
    db.delete_channel("global").unwrap();
    assert_eq!(make_missing(&db, true), vec!["global"], "#global, which every server has, comes back");
    db.delete_channel("local").unwrap();
    assert_eq!(make_missing(&db, false), Vec::<String>::new(), "with the guaranteed local room off, no #local");
    assert_eq!(make_missing(&db, true), vec!["local"], "with it on, #local comes back");
    // With the guarantee off an existing #local is an ordinary channel; turned back on, it is the
    // local room again rather than a second one.
    db.set_channel_local_only("local", false).unwrap();
    assert!(make_missing(&db, true).is_empty(), "no second local room");
    assert!(db.is_channel_local_only("local").unwrap(), "#local is local only again");
}
