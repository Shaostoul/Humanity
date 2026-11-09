// Child of upload_rooms.rs (#[path]). BUG-282 (2026-11-07): a file attached in a members-only room
// opened for anyone who had its address, and the address could be guessed (the upload time and the
// file name).
//
// Seen red on 2026-11-08 against the code before the fix: "two names for one file never match"
// failed (the stored name was `<ms>_<stem>.<ext>`, the same twice in one millisecond), and through
// the relay's own router a stranger's GET of the members-only room's file answered 200 (ServeDir
// with nothing in front of it), where 401 was wanted.

use super::*;
use humanity_protocol::chat_access::{PostLevel, ReadLevel};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[test]
fn a_stored_name_cannot_be_guessed() {
    let a = crate::relay::upload_rules::stored_name("Plan for Tuesday.png", "png");
    let b = crate::relay::upload_rules::stored_name("Plan for Tuesday.png", "png");
    assert_ne!(a, b, "two names for one file never match, even in one millisecond");
    let parts: Vec<&str> = a.splitn(3, '_').collect();
    assert_eq!(parts.len(), 3, "{a}");
    assert!(parts[0].bytes().all(|c| c.is_ascii_digit()), "the time first: {a}");
    assert!(parts[1].len() == 32 && parts[1].bytes().all(|c| c.is_ascii_hexdigit()), "then 128 random bits: {a}");
    assert_eq!(parts[2], "Plan_for_Tuesday.png", "then the safe stem: {a}");
}

#[test]
fn the_links_in_a_line_are_found() {
    let names = linked_names("look https://united-humanity.us/uploads/1_ab_x.png and /uploads/2_cd_y.mp4?t=1 (/uploads/../etc) /uploads/1_ab_x.png");
    assert_eq!(names, vec!["1_ab_x.png".to_string(), "2_cd_y.mp4".to_string()]);
}

/// The status line's code of `GET path` (with `header`, if any) on the relay at `port`.
async fn get(port: u16, path: &str, header: Option<(&str, &str)>) -> u16 {
    let mut s = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let extra = header.map(|(k, v)| format!("{k}: {v}\r\n")).unwrap_or_default();
    s.write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{extra}Connection: close\r\n\r\n").as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    let _ = s.read_to_end(&mut buf).await;
    let text = String::from_utf8_lossy(&buf);
    text.split_whitespace().nth(1).and_then(|c| c.parse().ok()).unwrap_or(0)
}

#[tokio::test]
async fn a_members_only_rooms_file_opens_for_its_readers_alone() {
    let db = Storage::open_temp("upload_rooms_route");
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let state = crate::relay::serve_relay(db, listener).await;

    // Ann, a member, posts a file in a room that reads at Members and one in a room open to anyone.
    let dil = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&humanity_protocol::pq_crypto::derive_dilithium_seed(&[0x52; 32]));
    let ann = hex::encode(dil.public_key());
    state.db.join_server(&ann, "Ann").unwrap();
    state.db.register_name("Ann", &ann).unwrap();
    state.db.create_channel("staff-room", "Staff", None, &ann, PostLevel::Members).unwrap();
    state.db.set_channel_access("staff-room", ReadLevel::Members, PostLevel::Members).unwrap();
    state.db.create_channel("lobby", "Lobby", None, &ann, PostLevel::Members).unwrap();
    state.db.set_channel_access("lobby", ReadLevel::Anyone, PostLevel::Members).unwrap();
    std::fs::create_dir_all("data/uploads").unwrap();
    let private = crate::relay::upload_rules::stored_name("plan.png", "png");
    let public = crate::relay::upload_rules::stored_name("view.png", "png");
    for name in [&private, &public] {
        std::fs::write(format!("data/uploads/{name}"), b"PNG").unwrap();
        state.db.record_upload(&ann, name, 50, 0, "x.png", 3).unwrap();
    }
    note_posted(&state.db, &ann, &format!("the plan: /uploads/{private}"), "staff-room");
    note_posted(&state.db, &ann, &format!("the view: /uploads/{public}"), "lobby");

    let _cleanup = RemoveOnDrop(vec![format!("data/uploads/{private}"), format!("data/uploads/{public}")]);
    assert_eq!(get(port, &format!("/uploads/{private}"), None).await, 401, "a stranger cannot open the members-only room's file");
    assert_eq!(get(port, &format!("/uploads/{public}"), None).await, 200, "the open room's file is public, as the room is");
    // With the session's upload token in the header (the desktop app).
    state.upload_tokens.write().await.insert("ann-token".into(), ann.clone());
    assert_eq!(get(port, &format!("/uploads/{private}"), Some(("X-Upload-Token", "ann-token"))).await, 200, "a reader with their token opens it");
    assert_eq!(get(port, &format!("/uploads/{private}"), Some(("X-Upload-Token", "not-a-token"))).await, 401);
    // With the room's ticket in the address (the website).
    let ticket = mint_ticket("staff-room", &ann, now_secs()).unwrap();
    assert_eq!(get(port, &format!("/uploads/{private}?t={ticket}"), None).await, 200, "a reader's room ticket opens it");
    let other = mint_ticket("another-room", &ann, now_secs()).unwrap();
    assert_eq!(get(port, &format!("/uploads/{private}?t={other}"), None).await, 401, "another room's ticket does not");
    let stale = mint_ticket("staff-room", &ann, now_secs() - 2 * TICKET_LIFE_SECS).unwrap();
    assert_eq!(get(port, &format!("/uploads/{private}?t={stale}"), None).await, 401, "a ticket that ran out does not");
    // A ban ends it at once: the ticket's holder is checked at every request.
    state.db.ban_user(&ann, "Ann").unwrap();
    assert_eq!(get(port, &format!("/uploads/{private}?t={ticket}"), None).await, 401, "a banned holder's ticket opens nothing");
    // The room opened to anyone: its file is public from then on.
    state.db.set_channel_access("staff-room", ReadLevel::Anyone, PostLevel::Members).unwrap();
    assert_eq!(get(port, &format!("/uploads/{private}"), None).await, 200, "the room's levels are read at each request");
}

/// Removes the test's files from data/uploads when the test ends, passed or failed.
struct RemoveOnDrop(Vec<String>);
impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        for p in &self.0 {
            let _ = std::fs::remove_file(p);
        }
    }
}
