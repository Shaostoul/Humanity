//! "Report a copyright problem" and the public takedown log (2026-10-13). A child of
//! relay/media_takedown.rs. Each test names the break it was seen failing against ("Seen red"),
//! with the words it failed with.
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, the time is named so a day can pass, and nothing is written outside a
//! temporary folder.

use super::*;
use crate::relay::media::{labels, publish_core, PublishBody};
use crate::relay::relay::Peer;
use crate::relay::storage::media::{MediaItem, MEDIA_SHELF};
use axum::http::HeaderValue;

/// Noon on a day well inside the range the tests use.
const NOON: i64 = 20_000 * DAY_MS + DAY_MS / 2;

const ANN: &str = "ann_key";
const OPERATOR: &str = "operator_key";

fn relay(tag: &str) -> Arc<RelayState> {
    Arc::new(RelayState::new(Storage::open_temp(tag)))
}

/// The four folders of a test's own.
struct Dirs {
    uploads: crate::test_temp::TempPath,
    members: crate::test_temp::TempPath,
    resume: crate::test_temp::TempPath,
    waiting: crate::test_temp::TempPath,
}

fn dirs(tag: &str) -> Dirs {
    let d = |what: &str| crate::test_temp::dir(&format!("{tag}_{what}"));
    Dirs { uploads: d("uploads"), members: d("members"), resume: d("resume"), waiting: d("waiting") }
}

impl Dirs {
    fn folders(&self) -> Folders<'_> {
        Folders { uploads: &self.uploads, members: &self.members, resume: &self.resume, waiting: &self.waiting }
    }
}

/// `key` uploads `file` for Media and publishes it with `visibility`, titled `title`.
fn published(state: &RelayState, d: &Dirs, key: &str, file: &str, title: &str, visibility: &str) -> MediaItem {
    state.db.join_server(key, key).unwrap();
    std::fs::write(d.waiting.join(file), b"the work").unwrap();
    state.db.record_upload(key, file, 100, MEDIA_SHELF, file, 8).unwrap();
    let s = state.db.get_server_settings().unwrap();
    let body = PublishBody {
        key: key.into(),
        filename: file.into(),
        title: title.into(),
        visibility: Some(visibility.into()),
        ..Default::default()
    };
    let answer = publish_core(&state.db, &s, labels(), d.folders(), &body).expect("test setup: published").0;
    state.db.media_item(answer["item"]["id"].as_i64().unwrap()).unwrap().unwrap()
}

/// Request headers: each (name, value).
fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
    let mut h = HeaderMap::new();
    for (name, value) in pairs {
        h.append(axum::http::HeaderName::from_bytes(name.as_bytes()).unwrap(), HeaderValue::from_str(value).unwrap());
    }
    h
}

/// A complete form about `item`, the box ticked.
fn form(item: i64) -> String {
    json!({
        "item": item,
        "name": "Rosa Painter",
        "contact": "rosa@example.org",
        "statement": "I painted this in 2019; it is on my own site with the date.",
        "good_faith": true,
    })
    .to_string()
}

/// Send `text` from `from` (headers) at `now`: the status and the answer's words or JSON.
async fn send(state: &RelayState, from: &[(&str, &str)], text: &str, now: i64) -> (u16, String) {
    match takedown_core(state, &headers(from), text, now).await {
        Ok(Json(v)) => (200, v.to_string()),
        Err((code, words)) => (code.as_u16(), words),
    }
}

fn count(state: &RelayState) -> i64 {
    state.db.with_conn(|c| c.query_row("SELECT COUNT(*) FROM takedown_requests", [], |r| r.get(0))).unwrap()
}

/// THE BOX MUST BE TICKED. A form with `good_faith` missing, false, or the word "true" instead of
/// true is refused in plain words, nothing is kept, and the refusal costs the address nothing:
/// its three reports for the day still go through after it. A form without a statement, or naming
/// no item or an item the server does not have, is refused the same way.
///
/// Seen red 2026-10-13 with the `good_faith` check taken out of `read_form`: "a form with
/// good_faith missing was taken in / left: (200, ...Received...) / right: (400, \"Tick the box to
/// say you believe in good faith...\")".
#[tokio::test]
async fn a_report_without_the_box_ticked_is_refused() {
    let state = relay("takedown_good_faith");
    let d = dirs("takedown_good_faith");
    let item = published(&state, &d, ANN, "1_dusk.png", "Dusk", "public");
    let at = [("x-real-ip", "203.0.113.7")];
    let mut without = serde_json::from_str::<Value>(&form(item.id)).unwrap();
    without.as_object_mut().unwrap().remove("good_faith");
    for (what, body) in [
        ("missing", without.to_string()),
        ("false", form(item.id).replace("\"good_faith\":true", "\"good_faith\":false")),
        ("the word", form(item.id).replace("\"good_faith\":true", "\"good_faith\":\"true\"")),
    ] {
        assert_eq!(send(&state, &at, &body, NOON).await, (400, NEEDS_GOOD_FAITH.to_string()), "a form with good_faith {what} was taken in");
    }
    let blank = form(item.id).replace("I painted this in 2019; it is on my own site with the date.", "   ");
    assert_eq!(send(&state, &at, &blank, NOON).await, (400, NEEDS_STATEMENT.to_string()));
    assert_eq!(send(&state, &at, &form(0), NOON).await, (400, NAME_THE_ITEM.to_string()));
    assert_eq!(send(&state, &at, &form(999), NOON).await, (404, NO_SUCH_ITEM.to_string()));
    assert_eq!(send(&state, &at, "not json", NOON).await, (400, NOT_A_FORM.to_string()));
    let long = form(item.id).replace("I painted this", &"x".repeat(STATEMENT_MAX + 1));
    assert_eq!(send(&state, &at, &long, NOON).await.0, 400, "a statement past the limit");
    assert_eq!(count(&state), 0, "a refused form is never kept");

    for n in 1..=TAKEDOWNS_PER_ADDRESS_PER_DAY {
        let (code, answer) = send(&state, &at, &form(item.id), NOON).await;
        assert_eq!(code, 200, "report {n} after the refusals: {answer}");
        assert!(answer.contains(RECEIVED), "{answer}");
    }
    assert_eq!(count(&state), TAKEDOWNS_PER_ADDRESS_PER_DAY as i64);
}

/// ONE ADDRESS SENDS THREE A DAY, AND X-FORWARDED-FOR CHANGES NOTHING. Three reports from one
/// X-Real-IP go through; the fourth is refused (429, in plain words) however its
/// X-Forwarded-For is written, and nothing more is kept; another address goes through; two
/// addresses in one IPv6 /64 count as one; and the next UTC day the first address may send again.
///
/// Seen red 2026-10-13 with the route counting by the X-Forwarded-For header instead of
/// `address_of`: "a new X-Forwarded-For made a new person / left: (200, ...Received...) / right:
/// (429, \"3 copyright reports have come from your connection today...\")".
#[tokio::test]
async fn one_address_sends_three_a_day_and_x_forwarded_for_changes_nothing() {
    let state = relay("takedown_limit");
    let d = dirs("takedown_limit");
    let item = published(&state, &d, ANN, "1_dusk.png", "Dusk", "public");
    for n in 0..TAKEDOWNS_PER_ADDRESS_PER_DAY {
        let xff = format!("198.51.100.{n}");
        let (code, answer) = send(&state, &[("x-real-ip", "203.0.113.7"), ("x-forwarded-for", &xff)], &form(item.id), NOON).await;
        assert_eq!(code, 200, "report {n}: {answer}");
    }
    for xff in ["198.51.100.99", "10.0.0.1, 203.0.113.7", ""] {
        let from = [("x-real-ip", "203.0.113.7"), ("x-forwarded-for", xff)];
        assert_eq!(send(&state, &from, &form(item.id), NOON).await, (429, address_full_words()), "a new X-Forwarded-For made a new person");
    }
    assert_eq!(count(&state), 3, "nothing more was kept");
    assert_eq!(send(&state, &[("x-real-ip", "198.51.100.4")], &form(item.id), NOON).await.0, 200, "another address");

    // IPv6: one home's /64 is one address.
    let v6a = [("x-real-ip", "2001:db8:1:2::10")];
    let v6b = [("x-real-ip", "2001:db8:1:2:ffff::99")];
    for _ in 0..2 {
        assert_eq!(send(&state, &v6a, &form(item.id), NOON).await.0, 200);
    }
    assert_eq!(send(&state, &v6b, &form(item.id), NOON).await.0, 200, "the third from the /64");
    assert_eq!(send(&state, &v6a, &form(item.id), NOON).await.0, 429, "the /64's fourth");

    // The next UTC day starts again.
    let tomorrow = NOON + DAY_MS;
    assert_eq!(send(&state, &[("x-real-ip", "203.0.113.7")], &form(item.id), tomorrow).await.0, 200, "the next day");
}

/// THE SERVER TAKES IN ITS DAY AND NO MORE: past the day's total nobody new is taken in, with
/// the server's own words; a new UTC day starts again. (The limits on their own, small ones.)
///
/// Seen red 2026-10-13 with the total check taken out of `Limits::take`: "the server's sixth of
/// the day was taken in / left: Ok(()) / right: Err(Server)".
#[test]
fn the_server_takes_in_its_day_and_no_more() {
    let limits = Limits::with(3, 5);
    for n in 0..5 {
        assert_eq!(limits.take(&format!("192.0.2.{n}"), NOON), Ok(()), "report {n}");
    }
    assert_eq!(limits.take("192.0.2.200", NOON), Err(Over::Server), "the server's sixth of the day was taken in");
    assert_eq!(limits.take("192.0.2.200", NOON + DAY_MS), Ok(()), "the next day");
    // The words a person sees.
    assert!(address_full_words().starts_with("3 copyright reports have come from your connection today"));
}

/// THE PUBLIC LOG NEVER HOLDS NAME, CONTACT OR STATEMENT: two reports decided (one removed, one
/// kept) and one still open; the log lists the two decided, the latest decision first, each with
/// exactly its id, item, title, dates, outcome and note, and the log's whole text holds none of
/// what the people wrote, nor the deciding admin's key. A members-only item's title is not shown.
///
/// Seen red 2026-10-13 with `public_json` answering the row serialised whole
/// (`serde_json::to_value(r)`): "the public log holds what a person wrote: Rosa Painter".
#[tokio::test]
async fn the_public_log_never_holds_name_contact_or_statement() {
    let state = relay("takedown_log");
    let d = dirs("takedown_log");
    state.db.set_role(OPERATOR, "admin").unwrap();
    let dusk = published(&state, &d, ANN, "1_dusk.png", "Dusk", "public");
    let secret = published(&state, &d, ANN, "2_secret.png", "Secret garden", "members");
    let at = [("x-real-ip", "203.0.113.7")];
    for id in [dusk.id, secret.id] {
        assert_eq!(send(&state, &at, &form(id), NOON).await.0, 200);
    }
    let open_one = send(&state, &[("x-real-ip", "198.51.100.4")], &form(dusk.id).replace("Rosa", "Ines"), NOON).await;
    assert_eq!(open_one.0, 200);
    decide_core(&state.db, d.folders(), OPERATOR, 1, Outcome::Kept, "The uploader showed it is theirs.", NOON + 1000).unwrap();
    decide_core(&state.db, d.folders(), OPERATOR, 2, Outcome::Removed, "Removed while we ask.", NOON + 2000).unwrap();

    let log = log_core(&state.db).unwrap().0;
    let text = log.to_string();
    for personal in ["Rosa Painter", "Ines Painter", "rosa@example.org", "I painted this", OPERATOR] {
        assert!(!text.contains(personal), "the public log holds what a person wrote: {personal}");
    }
    let rows = log["takedowns"].as_array().unwrap();
    assert_eq!(rows.len(), 2, "the open report is not in the log");
    let mut keys: Vec<&str> = rows[0].as_object().unwrap().keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["created_at", "decided_at", "id", "item_id", "item_title", "note", "outcome"]);
    assert_eq!((rows[0]["id"].as_i64(), rows[0]["outcome"].as_str()), (Some(2), Some("removed")), "the latest decision first");
    assert_eq!(rows[0]["item_title"], "", "a members-only item's title is not shown");
    assert_eq!(rows[0]["note"], "Removed while we ask.");
    assert_eq!((rows[1]["id"].as_i64(), rows[1]["outcome"].as_str(), rows[1]["item_title"].as_str()), (Some(1), Some("kept"), Some("Dusk")));
    assert_eq!(rows[1]["decided_at"].as_i64(), Some(NOON + 1000));

    // The admins' list holds what the person wrote, for the one still open.
    let open = open_core(&state.db, OPERATOR).unwrap().0;
    let open = open["requests"].as_array().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!((open[0]["name"].as_str(), open[0]["contact"].as_str()), (Some("Ines Painter"), Some("rosa@example.org")));
    assert_eq!(open[0]["item_on_page"], true);
}

/// ONLY AN ADMIN CAN DECIDE, OR SEE THE OPEN REPORTS: a member, a moderator and a stranger are
/// refused both, and the report stays open and the item on the page; the owner and an admin may.
///
/// Seen red 2026-10-13 with the admin check taken out of `decide_core`: "member_key decided a
/// copyright report / left: None / right: Some(403)".
#[tokio::test]
async fn only_an_admin_can_decide() {
    let state = relay("takedown_admin_only");
    let d = dirs("takedown_admin_only");
    state.db.join_server("member_key", "Member").unwrap();
    state.db.set_role("mod_key", "mod").unwrap();
    state.db.set_role("owner_key", "owner").unwrap();
    state.db.set_role(OPERATOR, "admin").unwrap();
    let item = published(&state, &d, ANN, "1_dusk.png", "Dusk", "public");
    for n in 0..2 {
        assert_eq!(send(&state, &[("x-real-ip", "203.0.113.7")], &form(item.id), NOON + n).await.0, 200);
    }
    for who in ["member_key", "mod_key", "stranger_key", ANN] {
        let refused = decide_core(&state.db, d.folders(), who, 1, Outcome::Removed, "", NOON);
        assert_eq!(refused.err().map(|e| e.0), Some(StatusCode::FORBIDDEN), "{who} decided a copyright report");
        assert_eq!(open_core(&state.db, who).err().map(|e| e.0), Some(StatusCode::FORBIDDEN), "{who} read the open reports");
    }
    assert_eq!(state.db.takedown_by_id(1).unwrap().unwrap().state, "open", "a moderator decided a copyright report");
    assert!(state.db.media_item(item.id).unwrap().is_some(), "the item is still on the page");
    assert!(decide_core(&state.db, d.folders(), "owner_key", 1, Outcome::Kept, "", NOON).is_ok(), "the owner decides");
    assert!(decide_core(&state.db, d.folders(), OPERATOR, 2, Outcome::Kept, "", NOON).is_ok(), "an admin decides");
}

/// REMOVED REMOVES THE ITEM, AND A REPORT IS DECIDED ONCE: "removed" takes the item off the Media
/// page through its own remove path (its file, uploaded for Media, is deleted) and says so; the
/// same report decided again is refused (409) and changes nothing; a second report about the same
/// item, decided "removed" after it went, counts as removed with the item already gone; "kept"
/// leaves its item where it is.
///
/// Seen red 2026-10-13 with `decide_core` recording "removed" without calling `remove_core`: "the
/// item is still on the Media page after removed / left: true / right: false".
#[tokio::test]
async fn removed_removes_the_item() {
    let state = relay("takedown_removed");
    let d = dirs("takedown_removed");
    state.db.set_role(OPERATOR, "admin").unwrap();
    let dusk = published(&state, &d, ANN, "1_dusk.png", "Dusk", "public");
    let dawn = published(&state, &d, ANN, "2_dawn.png", "Dawn", "public");
    assert!(d.uploads.join("1_dusk.png").exists(), "test setup: the published file is in the public folder");
    for id in [dusk.id, dusk.id, dawn.id] {
        assert_eq!(send(&state, &[("x-real-ip", "203.0.113.7")], &form(id), NOON).await.0, 200);
    }

    let answer = decide_core(&state.db, d.folders(), OPERATOR, 1, Outcome::Removed, "Not theirs to share.", NOON).unwrap().0;
    assert_eq!((answer["outcome"].as_str(), answer["item_removed"].as_bool()), (Some("removed"), Some(true)));
    assert_eq!(state.db.media_item(dusk.id).unwrap().is_some(), false, "the item is still on the Files page after removed");
    assert!(!d.uploads.join("1_dusk.png").exists(), "its file, uploaded for Media, is deleted");

    let again = decide_core(&state.db, d.folders(), OPERATOR, 1, Outcome::Kept, "", NOON);
    assert_eq!(again.err().map(|e| e.0), Some(StatusCode::CONFLICT), "a report is decided once");
    assert_eq!(state.db.takedown_by_id(1).unwrap().unwrap().state, "removed");

    let gone = decide_core(&state.db, d.folders(), OPERATOR, 2, Outcome::Removed, "", NOON).unwrap().0;
    assert_eq!((gone["item_removed"].as_bool(), gone["item_already_gone"].as_bool()), (Some(false), Some(true)));
    assert_eq!(state.db.takedown_by_id(2).unwrap().unwrap().state, "removed");

    decide_core(&state.db, d.folders(), OPERATOR, 3, Outcome::Kept, "", NOON).unwrap();
    assert!(state.db.media_item(dawn.id).unwrap().is_some(), "kept leaves its item");
    assert_eq!(decide_core(&state.db, d.folders(), OPERATOR, 99, Outcome::Kept, "", NOON).err().map(|e| e.0), Some(StatusCode::NOT_FOUND));
}

/// THE ADMINS SIGNED IN NOW ARE TOLD, AND ONLY THEY: a report sends a system line to the admin
/// and the owner signed in, never to a moderator or a member, and the line holds none of what the
/// person wrote.
///
/// Seen red 2026-10-13 with `tell_admins` filtering on nothing: "a member was told of a copyright
/// report: [\"member_key\", \"mod_key\", \"operator_key\", \"owner_key\"]".
#[tokio::test]
async fn the_admins_signed_in_now_are_told() {
    let state = relay("takedown_told");
    let d = dirs("takedown_told");
    state.db.set_role(OPERATOR, "admin").unwrap();
    state.db.set_role("owner_key", "owner").unwrap();
    state.db.set_role("mod_key", "mod").unwrap();
    state.db.join_server("member_key", "Member").unwrap();
    let item = published(&state, &d, ANN, "1_dusk.png", "Dusk", "public");
    {
        let mut peers = state.peers.write().await;
        for (i, key) in [OPERATOR, "owner_key", "mod_key", "member_key"].into_iter().enumerate() {
            peers.insert(
                key.to_string(),
                Peer { public_key_hex: key.to_string(), display_name: None, upload_token: None, kyber_public: None, conn_id: i as u64 + 1 },
            );
        }
    }
    let mut rx = state.broadcast_tx.subscribe();
    assert_eq!(send(&state, &[("x-real-ip", "203.0.113.7")], &form(item.id), NOON).await.0, 200);
    let mut told = Vec::new();
    while let Ok(m) = rx.try_recv() {
        if let RelayMessage::Private { to, message } = m {
            told.push((to, message));
        }
    }
    let mut who: Vec<&str> = told.iter().map(|(to, _)| to.as_str()).collect();
    who.sort_unstable();
    assert!(!who.contains(&"member_key") && !who.contains(&"mod_key"), "a member was told of a copyright report: {who:?}");
    assert_eq!(who, [OPERATOR, "owner_key"]);
    for (_, words) in &told {
        assert_eq!(words, "New copyright report #1 about item 1 \"Dusk\" on the Files page. Open its Admin tab to decide it.");
    }
}

/// THE DECISION'S PURPOSE NAMES THE REPORT AND THE OUTCOME, so a signature for one decision is
/// good for no other.
#[test]
fn a_decision_signs_its_report_and_outcome() {
    assert_eq!(decide_purpose(7, Outcome::Removed), "media_takedown:7:removed");
    assert_eq!(decide_purpose(7, Outcome::Kept), "media_takedown:7:kept");
    assert_eq!(Outcome::from_word("removed"), Some(Outcome::Removed));
    assert_eq!(Outcome::from_word("open"), None);
}
