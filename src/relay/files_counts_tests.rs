//! Views and downloads counted once a person a day (Media round 5, 2026-10-12; lane 1B of
//! docs/design/media-round-5-plan.md, its 3.5). A child of relay/files_counts.rs. Each test names
//! the break it was seen failing against ("Seen red").
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, the time is named so a day can pass, and nothing is written outside a
//! temporary folder.

use super::*;
use crate::relay::files::{labels, publish_core, PublishBody, UPLOAD_TOKEN_HEADER};
use crate::relay::files_ticket::{self, TicketBody};
use crate::relay::storage::files::{MEDIA_SHELF, ROLE_FILE};
use crate::relay::storage::Storage;
use std::time::Instant;

/// Noon on a day well inside the range the tests use.
const NOON: i64 = 20_000 * DAY_MS + DAY_MS / 2;

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

/// `key` uploads `bytes` for Media as `file` (the name it had on their device: `original`) and
/// publishes it with `visibility`.
fn published(state: &RelayState, d: &Dirs, key: &str, file: &str, original: &str, bytes: &[u8], visibility: &str) -> FilesItem {
    std::fs::write(d.waiting.join(file), bytes).unwrap();
    state.db.record_upload(key, file, 100, MEDIA_SHELF, original, bytes.len() as i64).unwrap();
    let s = state.db.get_server_settings().unwrap();
    let body = PublishBody { key: key.into(), filename: file.into(), visibility: Some(visibility.into()), ..Default::default() };
    let answer = publish_core(&state.db, &s, labels(), d.folders(), &body).expect("test setup: published").0;
    state.db.media_item(answer["item"]["id"].as_i64().unwrap()).unwrap().unwrap()
}

/// `key` joins the server, signed in with `token`.
fn member(state: &RelayState, key: &str, token: &str) {
    state.db.join_server(key, key).unwrap();
    state.upload_tokens.try_write().expect("test setup: nobody holds the tokens").insert(token.into(), key.into());
}

/// The query of a download: which file, and a ticket.
fn query(file: Option<i64>, ticket: Option<&str>) -> TokenQuery {
    TokenQuery { token: None, ticket: ticket.map(str::to_string), file }
}

/// A download of item `id` with `q`, the headers `headers` and the method `method`, at `now`: its
/// status, headers and body.
async fn fetch_as(state: &RelayState, d: &Dirs, method: &str, id: i64, q: TokenQuery, headers: &[(&str, &str)], now: i64) -> (u16, HeaderMap, Vec<u8>) {
    let mut b = axum::http::Request::builder().method(method).uri(format!("/api/files/download/{id}"));
    for (name, value) in headers {
        b = b.header(*name, *value);
    }
    let res = download_from(state, d.folders(), id, q, b.body(axum::body::Body::empty()).unwrap(), now).await;
    let code = res.status().as_u16();
    let head = res.headers().clone();
    (code, head, axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec())
}

async fn fetch(state: &RelayState, d: &Dirs, id: i64, q: TokenQuery, headers: &[(&str, &str)], now: i64) -> (u16, Vec<u8>) {
    let (code, _, body) = fetch_as(state, d, "GET", id, q, headers, now).await;
    (code, body)
}

fn downloads(state: &RelayState, id: i64) -> i64 {
    state.db.media_item(id).unwrap().unwrap().downloads
}

fn views(state: &RelayState, id: i64) -> i64 {
    state.db.media_item(id).unwrap().unwrap().views
}

/// The refusal every download that may not be had gets.
fn refused() -> (u16, Vec<u8>) {
    (404, NO_SUCH_FILE.as_bytes().to_vec())
}

/// The ticket in a ticket answer's `url`.
fn ticket_of(answer: &serde_json::Value) -> String {
    answer["url"].as_str().unwrap().split("ticket=").nth(1).expect("a ticket in the url").to_string()
}

/// THE DAY'S MARKS COUNT A PERSON ONCE AND START AGAIN AT MIDNIGHT: the first view of an item by
/// an address counts and the second does not; another item, another kind of count, another
/// address and a key (even one spelling the same text) each count once; the next UTC day counts
/// the same person again; and past the day's cap nobody new is counted until midnight.
///
/// Seen red 2026-10-12 with `first_today` keeping yesterday's marks (the day left out of the
/// reset: `if today.is_none()`): "the next day counts the same person again".
#[test]
fn the_day_marks_count_a_person_once_and_start_again_at_midnight() {
    let marks = Marks::default();
    let noon = 20_000 * DAY_MS + DAY_MS / 2;
    let ann = Who::Address("203.0.113.7");
    assert!(marks.first_today(Count::View, 12, ann, noon), "the first view counts");
    assert!(!marks.first_today(Count::View, 12, ann, noon + 60_000), "the second that day does not");
    assert!(marks.first_today(Count::View, 13, ann, noon), "another item counts");
    assert!(marks.first_today(Count::Download, 12, ann, noon), "a download is counted apart from a view");
    assert!(marks.first_today(Count::View, 12, Who::Address("198.51.100.4"), noon), "another address counts");
    assert!(marks.first_today(Count::View, 12, Who::Key("203.0.113.7"), noon), "a key is never taken for an address");
    assert!(!marks.first_today(Count::View, 12, ann, 20_001 * DAY_MS - 1), "the last millisecond of the day is still that day");
    assert_eq!(marks.len(), 5);
    assert!(marks.first_today(Count::View, 12, ann, 20_001 * DAY_MS), "the next day counts the same person again");
    assert_eq!(marks.len(), 1, "yesterday's marks are forgotten");

    let small = Marks::with_cap(2);
    assert!(small.first_today(Count::View, 1, ann, noon));
    assert!(small.first_today(Count::View, 2, ann, noon));
    assert!(!small.first_today(Count::View, 3, ann, noon), "past the cap nobody new is counted");
    assert!(!small.first_today(Count::View, 1, ann, noon), "and those counted are not counted again");
    assert!(small.first_today(Count::View, 3, ann, noon + DAY_MS), "the cap is a day's");
}

/// A DOWNLOAD COUNTS ONCE A PERSON A DAY: anyone may download a public item on a public Files page;
/// the first download from an address counts, the same address again that day does not, another
/// address does, and the next UTC day the first address counts again. Every download that may not
/// be had (no such item, no such file on it, a members-only item without a ticket or a token, a
/// public item once the page is members only) gets the same 404 words and counts nothing.
///
/// Seen red 2026-10-12 with the day's mark not consulted (`if true` in place of `first_today` in
/// `download_from`): "the same person again that day is not counted / left: 2 / right: 1".
#[tokio::test]
async fn a_download_counts_once_per_person_per_day() {
    let state = relay("mc_once");
    let d = dirs("mc_once");
    member(&state, "ann", "tok_ann");
    let item = published(&state, &d, "ann", "1_dusk.png", "dusk.png", b"0123456789", "public");
    let secret = published(&state, &d, "ann", "2_secret.png", "secret.png", b"SECRET", "members");
    let ann_ip = [("x-real-ip", "203.0.113.7")];

    assert_eq!(fetch(&state, &d, item.id, query(None, None), &ann_ip, NOON).await, (200, b"0123456789".to_vec()), "the file is given");
    assert_eq!(downloads(&state, item.id), 1, "and counted");
    assert_eq!(fetch(&state, &d, item.id, query(Some(0), None), &ann_ip, NOON + 60_000).await.0, 200);
    assert_eq!(downloads(&state, item.id), 1, "the same person again that day is not counted");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[("x-real-ip", "198.51.100.4")], NOON).await.0, 200);
    assert_eq!(downloads(&state, item.id), 2, "another address is another person");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &ann_ip, NOON + DAY_MS).await.0, 200);
    assert_eq!(downloads(&state, item.id), 3, "the next day the first address counts again");

    assert_eq!(fetch(&state, &d, 999, query(None, None), &ann_ip, NOON).await, refused(), "no such item");
    assert_eq!(fetch(&state, &d, item.id, query(Some(7), None), &ann_ip, NOON).await, refused(), "no such file on it");
    assert_eq!(fetch(&state, &d, secret.id, query(None, None), &ann_ip, NOON).await, refused(), "a members-only item, asked by anyone");
    assert_eq!(downloads(&state, secret.id), 0);

    let mut s = state.db.get_server_settings().unwrap();
    s.media_visibility = "members".into();
    state.db.set_server_settings(&s, "admin").unwrap();
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[("x-real-ip", "192.0.2.1")], NOON).await, refused(), "a public item on a members-only page");
    assert_eq!(downloads(&state, item.id), 3, "nothing refused is counted");
}

/// A RANGE AFTER THE FIRST BYTE COUNTS NOTHING: a video player asking for a later piece
/// (`bytes=4-`), or the last bytes (`bytes=-3`), is given that piece and counts nothing, nor does a
/// HEAD; a range from byte 0 (`bytes=0-3`) is the start of a download and counts once.
///
/// Seen red 2026-10-12 with ranges not looked at (`from_the_first_byte` replaced by `true` in
/// `download_from`): "a later piece counts nothing / left: 1 / right: 0".
#[tokio::test]
async fn a_range_after_the_first_byte_counts_nothing() {
    let state = relay("mc_range");
    let d = dirs("mc_range");
    member(&state, "ann", "tok_ann");
    let item = published(&state, &d, "ann", "1_clip.png", "clip.png", b"0123456789", "public");
    let ip = ("x-real-ip", "203.0.113.7");

    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[ip, ("range", "bytes=4-")], NOON).await, (206, b"456789".to_vec()), "the piece is given");
    assert_eq!(downloads(&state, item.id), 0, "a later piece counts nothing");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[ip, ("range", "bytes=-3")], NOON).await, (206, b"789".to_vec()));
    assert_eq!(downloads(&state, item.id), 0, "nor the last bytes");
    assert_eq!(fetch_as(&state, &d, "HEAD", item.id, query(None, None), &[ip], NOON).await.0, 200);
    assert_eq!(downloads(&state, item.id), 0, "nor a HEAD");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[ip, ("range", "bytes=0-3")], NOON).await, (206, b"0123".to_vec()));
    assert_eq!(downloads(&state, item.id), 1, "a range from byte 0 is a download");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[ip], NOON).await.0, 200);
    assert_eq!(downloads(&state, item.id), 1, "and the whole file after it is the same person's");

    let range = |v: &str| {
        let mut h = HeaderMap::new();
        h.insert(header::RANGE, v.parse().unwrap());
        from_the_first_byte(&h)
    };
    assert!(from_the_first_byte(&HeaderMap::new()), "no Range");
    for from_zero in ["bytes=0-", "Bytes=0-499", "bytes=00-5", " bytes=0-1, 5-6"] {
        assert!(range(from_zero), "{from_zero}");
    }
    for later in ["bytes=1-", "bytes=-5", "bytes=10-0", "items=0-", "bytes=", "bytes=x-"] {
        assert!(!range(later), "{later}");
    }
}

/// THE DOWNLOAD IS NAMED BY ITS ORIGINAL NAME AND NOTHING ELSE: a name holding a quote and a line
/// break (and a would-be header after it) is given as `filename="..."` with the quote made an
/// underscore and the line break gone, and as `filename*=UTF-8''...` percent-encoded (RFC 6266 and
/// RFC 8187), so neither can end the name or start a header; an added file is named by its own
/// original name, letters beyond ASCII and all; a file with no original name by its stored one.
///
/// Seen red 2026-10-12 with the raw name put in the header
/// (`HeaderValue::from_str(&format!("attachment; filename=\"{name}\""))`, which a line break makes
/// fail, falling back to a bare `attachment`): "left: \"attachment\" / right: \"attachment;
/// filename=\\\"my _dusk_Set-Cookie: x=1.png\\\"; ...\"".
#[tokio::test]
async fn the_download_is_named_by_its_original_name_and_nothing_else() {
    let state = relay("mc_name");
    let d = dirs("mc_name");
    member(&state, "ann", "tok_ann");
    let item = published(&state, &d, "ann", "1_dusk.png", "my \"dusk\"\r\nSet-Cookie: x=1.png", b"PNG", "public");
    std::fs::write(d.waiting.join("2_extra.zip"), b"ZIP").unwrap();
    state.db.record_upload("ann", "2_extra.zip", 100, MEDIA_SHELF, "D\u{e4}mmerung \u{2600} 100%.zip", 3).unwrap();
    let f = d.folders();
    let added = state.db.attach_media_file("ann", item.id, "2_extra.zip", ROLE_FILE, 0, |members| f.place("2_extra.zip", members)).unwrap().item;
    let n = added.files[0].file;
    let nameless = published(&state, &d, "ann", "3_x.png", "", b"X", "public");

    let disposition = |head: &HeaderMap| head.get(header::CONTENT_DISPOSITION).expect("a Content-Disposition").to_str().unwrap().to_string();
    let (code, head, _) = fetch_as(&state, &d, "GET", item.id, query(None, None), &[], NOON).await;
    assert_eq!(code, 200);
    let named = disposition(&head);
    assert_eq!(named, "attachment; filename=\"my _dusk_Set-Cookie: x=1.png\"; filename*=UTF-8''my%20%22dusk%22Set-Cookie%3A%20x%3D1.png");
    assert!(!named.contains('\r') && !named.contains('\n'), "no line break reaches the header");
    assert_eq!(named.matches('"').count(), 2, "the only quotes are the ones around the plain name");
    assert!(head.get("set-cookie").is_none(), "and nothing in the name became a header");

    let (_, head, body) = fetch_as(&state, &d, "GET", item.id, query(Some(n), None), &[], NOON).await;
    assert_eq!(body, b"ZIP".to_vec(), "the added file");
    assert_eq!(disposition(&head), "attachment; filename=\"D_mmerung _ 100_.zip\"; filename*=UTF-8''D%C3%A4mmerung%20%E2%98%80%20100%25.zip");

    let (_, head, _) = fetch_as(&state, &d, "GET", nameless.id, query(None, None), &[], NOON).await;
    assert_eq!(disposition(&head), "attachment; filename=\"3_x.png\"; filename*=UTF-8''3_x.png", "no original name: the stored one");
}

/// A MEMBERS-ONLY DOWNLOAD COUNTS BY KEY THROUGH ITS TICKET: a members-only item is refused to
/// anyone without a ticket or a token (the same 404 words as a missing one); a member's ticket for
/// the download route opens it and counts the member, by their key, so the same member through a
/// second ticket from another address is the same person, and another member is another; the
/// session's token in `X-Upload-Token` opens it too, and counts the same key; a made-up ticket, and
/// the token of someone who is not a member, are refused.
///
/// Seen red 2026-10-12 with the ticket's key not used (`Who::Address(&address)` for every download):
/// "the same member through a second ticket, from another address, is the same person / left: 2 /
/// right: 1".
#[tokio::test]
async fn a_members_only_download_counts_by_key_through_its_ticket() {
    let state = relay("mc_ticket");
    let d = dirs("mc_ticket");
    member(&state, "ann", "tok_ann");
    member(&state, "bea", "tok_bea");
    state.upload_tokens.write().await.insert("tok_carl".into(), "carl".into());
    let item = published(&state, &d, "ann", "1_secret.mp4", "secret.mp4", b"SECRET", "members");
    let ticket_for = |key: &str| {
        let body = TicketBody { id: item.id, filename: item.upload_filename.clone(), file: 0, download: true };
        let answer = files_ticket::ticket_core(&state, key, &body, Instant::now()).unwrap().0;
        assert!(answer["url"].as_str().unwrap().starts_with(&format!("/api/files/download/{}?file=0&ticket=", item.id)), "{answer}");
        ticket_of(&answer)
    };
    let (a, b) = (("x-real-ip", "203.0.113.7"), ("x-real-ip", "198.51.100.4"));

    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[a], NOON).await, refused(), "nobody without a ticket or a token");
    let first = ticket_for("ann");
    assert_eq!(fetch(&state, &d, item.id, query(None, Some(&first)), &[a], NOON).await, (200, b"SECRET".to_vec()), "a member's ticket opens it");
    assert_eq!(downloads(&state, item.id), 1);
    let second = ticket_for("ann");
    assert_eq!(fetch(&state, &d, item.id, query(Some(0), Some(&second)), &[b], NOON).await.0, 200);
    assert_eq!(downloads(&state, item.id), 1, "the same member through a second ticket, from another address, is the same person");
    let bea = ticket_for("bea");
    assert_eq!(fetch(&state, &d, item.id, query(None, Some(&bea)), &[a], NOON).await.0, 200);
    assert_eq!(downloads(&state, item.id), 2, "another member, from the same address, is another person");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[b, (UPLOAD_TOKEN_HEADER, "tok_ann")], NOON).await.0, 200, "the session's token opens it too");
    assert_eq!(downloads(&state, item.id), 2, "and counts the same key");

    let made_up = "ab".repeat(32);
    assert_eq!(fetch(&state, &d, item.id, query(None, Some(&made_up)), &[a], NOON).await, refused(), "a made-up ticket");
    assert_eq!(fetch(&state, &d, item.id, query(None, None), &[a, (UPLOAD_TOKEN_HEADER, "tok_carl")], NOON).await, refused(), "someone who is not a member");
    assert_eq!(downloads(&state, item.id), 2);
}

/// AN ADDRESS IS X-REAL-IP, NEVER X-FORWARDED-FOR: two downloads from the same `X-Real-IP` count
/// once whatever `X-Forwarded-For` says (a client writes that one, and nginx only appends to it);
/// without `X-Real-IP` every request is the one shared unknown person, counted once, however many
/// different `X-Forwarded-For` values it brings. An address is written one way (spaces trimmed, an
/// IPv4 address written as IPv6 is the IPv4 one), and an IPv6 address counts as its /64.
///
/// Seen red 2026-10-12 with `X-Forwarded-For` read first in `address_of`: "a new X-Forwarded-For is
/// not a new person / left: 2 / right: 1".
#[tokio::test]
async fn an_address_is_x_real_ip_never_x_forwarded_for() {
    let state = relay("mc_xff");
    let d = dirs("mc_xff");
    member(&state, "ann", "tok_ann");
    let item = published(&state, &d, "ann", "1_dusk.png", "dusk.png", b"PNG", "public");
    let real = ("x-real-ip", "203.0.113.7");

    fetch(&state, &d, item.id, query(None, None), &[real, ("x-forwarded-for", "1.1.1.1")], NOON).await;
    assert_eq!(downloads(&state, item.id), 1);
    fetch(&state, &d, item.id, query(None, None), &[real, ("x-forwarded-for", "2.2.2.2")], NOON).await;
    assert_eq!(downloads(&state, item.id), 1, "a new X-Forwarded-For is not a new person");
    fetch(&state, &d, item.id, query(None, None), &[("x-forwarded-for", "3.3.3.3")], NOON).await;
    assert_eq!(downloads(&state, item.id), 2, "no X-Real-IP: the unknown person, counted once");
    fetch(&state, &d, item.id, query(None, None), &[("x-forwarded-for", "4.4.4.4")], NOON).await;
    assert_eq!(downloads(&state, item.id), 2, "and every request without one is that same person");

    let address = |pairs: &[(&str, &str)]| {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
        }
        address_of(&h)
    };
    assert_eq!(address(&[("x-real-ip", " 203.0.113.7 ")]), "203.0.113.7");
    assert_eq!(address(&[("x-real-ip", "::ffff:203.0.113.7")]), "203.0.113.7", "an IPv4 address written as IPv6");
    assert_eq!(address(&[("x-real-ip", "2001:db8:1:2::5")]), "2001:db8:1:2::/64");
    assert_eq!(address(&[("x-real-ip", "2001:db8:1:2:ffff::9")]), "2001:db8:1:2::/64", "one /64 is one person");
    assert_eq!(address(&[("x-real-ip", "not an address")]), UNKNOWN_ADDRESS);
    assert_eq!(address(&[("x-forwarded-for", "1.1.1.1")]), UNKNOWN_ADDRESS, "X-Forwarded-For is never read");
}

/// A VIEW COUNTS ONCE AND THE ANSWER SAYS NOTHING: `POST /api/files/viewed` counts a public item's
/// view once an address a day; a members-only item is counted only with a member's token, by
/// their key; and the answer is the same `{"ok":true}`, status and bytes, for an item that was
/// counted, one that was not, one the asker may not see, and one that does not exist.
///
/// Seen red 2026-10-12 with a missing item answered 404 (`return (StatusCode::NOT_FOUND, ...)` when
/// `item_or_none` found none): "the same answer for a missing item / left: (404, ...) / right:
/// (200, [123, 34, 111, 107, ...])".
#[tokio::test]
async fn a_view_counts_once_and_the_answer_says_nothing() {
    let state = relay("mc_view");
    let d = dirs("mc_view");
    member(&state, "ann", "tok_ann");
    let open = published(&state, &d, "ann", "1_open.png", "open.png", b"PNG", "public");
    let secret = published(&state, &d, "ann", "2_secret.png", "secret.png", b"PNG", "members");
    let headers = |pairs: &[(&str, &str)]| {
        let mut h = HeaderMap::new();
        for (k, v) in pairs {
            h.insert(axum::http::HeaderName::from_bytes(k.as_bytes()).unwrap(), v.parse().unwrap());
        }
        h
    };
    let ask = |h: HeaderMap, id: i64| {
        let state = state.clone();
        async move {
            let res = viewed(State(state), h, Json(ViewedBody { id })).await;
            let code = res.status().as_u16();
            (code, axum::body::to_bytes(res.into_body(), 1 << 10).await.unwrap().to_vec())
        }
    };
    let a = headers(&[("x-real-ip", "203.0.113.7")]);

    let answer = ask(a.clone(), open.id).await;
    assert_eq!(answer, (200, b"{\"ok\":true}".to_vec()));
    assert_eq!(views(&state, open.id), 1, "the first view counts");
    assert_eq!(ask(a.clone(), open.id).await, answer);
    assert_eq!(views(&state, open.id), 1, "the same person again that day does not");
    assert_eq!(viewed_core(&state, &headers(&[("x-real-ip", "198.51.100.4")]), open.id, now_ms()).await.status(), StatusCode::OK);
    assert_eq!(views(&state, open.id), 2, "another address does");

    assert_eq!(ask(a.clone(), 999_999).await, answer, "the same answer for a missing item");
    assert_eq!(ask(a.clone(), secret.id).await, answer, "and for a members-only item asked by anyone");
    assert_eq!(views(&state, secret.id), 0, "which is not counted");
    let ann = headers(&[("x-real-ip", "203.0.113.7"), (UPLOAD_TOKEN_HEADER, "tok_ann")]);
    assert_eq!(ask(ann.clone(), secret.id).await, answer);
    assert_eq!(views(&state, secret.id), 1, "a member's view of it counts");
    assert_eq!(ask(headers(&[("x-real-ip", "198.51.100.9"), (UPLOAD_TOKEN_HEADER, "tok_ann")]), secret.id).await, answer);
    assert_eq!(views(&state, secret.id), 1, "by their key, from any address");
}
