//! Tests of opening a Media item (open.rs): the transfers against a one-shot server on this
//! machine's loopback address (nothing leaves the machine), the names Download saves under, and
//! the temporary copies. Each was seen to fail once before it passed; what it saw red is in its
//! comment.

use super::*;
use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// A one-shot HTTP server on a random loopback port: it reads the request head (kept in the
/// returned cell), answers `head`, then writes each of `chunks` with `gap` between them, then
/// closes. Returns the address to fetch.
fn serve(head: &'static str, chunks: Vec<Vec<u8>>, gap: Duration) -> (String, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let port = listener.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(String::new()));
    let keep = seen.clone();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0u8; 4096];
            let mut got = Vec::new();
            while let Ok(n) = stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
                if got.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            *keep.lock().unwrap() = String::from_utf8_lossy(&got).to_string();
            let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\n{head}\r\n").as_bytes());
            for c in chunks {
                std::thread::sleep(gap);
                if stream.write_all(&c).is_err() {
                    break;
                }
                let _ = stream.flush();
            }
        }
    });
    (format!("http://127.0.0.1:{port}/api/media/file/7"), seen)
}

fn dest(tag: &str) -> crate::test_temp::TempPath {
    crate::test_temp::file(tag, "bin")
}

/// A folder for a test's transfers, deleted with the guard.
fn folder(tag: &str) -> crate::test_temp::TempPath {
    crate::test_temp::dir(tag)
}

/// The names in `dir`.
fn names_in(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().to_string()).collect()).unwrap_or_default();
    v.sort();
    v
}

/// A MEMBERS-ONLY FILE IS ASKED FOR WITH THE TOKEN IN A HEADER, NEVER IN THE ADDRESS, by both the
/// card's transfers and the tiles' pictures (the review's finding: ureq's error words hold the
/// whole address, and image_cache's `download_bytes` wrote it into its message).
///
/// Seen red 2026-10-11 with `download_bytes_with` leaving its header unset: "the picture's
/// request carries the token in its header" (the request head had no x-upload-token line).
#[test]
fn the_token_rides_in_a_header() {
    let (url, seen) = serve("Content-Length: 3\r\nConnection: close\r\n", vec![b"abc".to_vec()], Duration::ZERO);
    let out = folder("media_open_header");
    fetch_to(&transfer_agent(Duration::from_secs(5)), &url, Some((api::TOKEN_HEADER, "SECRET-1")), &out, "a.bin", 100).expect("fetched");
    let head = seen.lock().unwrap().to_ascii_lowercase();
    assert!(head.starts_with("get /api/media/file/7 http/1.1"), "no token in the address: {head}");
    assert!(head.contains("x-upload-token: secret-1"), "the card's request carries the token in its header: {head}");

    let (url, seen) = serve("Content-Length: 3\r\nConnection: close\r\n", vec![b"abc".to_vec()], Duration::ZERO);
    crate::gui::widgets::image_cache::download_bytes_with(&url, 100, Some((api::TOKEN_HEADER, "SECRET-2"))).expect("fetched");
    let head = seen.lock().unwrap().to_ascii_lowercase();
    assert!(head.contains("x-upload-token: secret-2"), "the picture's request carries the token in its header: {head}");

    // And a refusal's words never hold a token, even one given in an address.
    let err = crate::gui::widgets::image_cache::download_bytes("http://127.0.0.1:9/x?token=SECRET-3", 100).expect_err("nothing listens");
    assert!(!err.contains("SECRET-3"), "{err}");
}

/// A BIG FILE IS NOT CUT OFF BY A CLOCK: a transfer that keeps arriving is never stopped for
/// taking long, only a connection that goes quiet (the review: Download and Play failed past 10
/// minutes, because ureq's `timeout` covers the whole body).
///
/// Seen red 2026-10-11 with `transfer_agent` setting ureq's whole-request `timeout` to the idle
/// time: "a transfer longer than the idle limit finishes: Err(\"The download stopped (A connection
/// attempt failed because the connected party did not properly respond after a period of time
/// ... (os error 10060)).\")".
#[test]
fn a_long_transfer_is_not_cut_off_by_a_clock() {
    // Five pieces 300 ms apart: 1.5 s in all, against a 1 s limit on going quiet.
    let chunks = vec![vec![b'a'; 10]; 5];
    let (url, _) = serve("Content-Length: 50\r\nConnection: close\r\n", chunks, Duration::from_millis(300));
    let out = folder("media_open_slow");
    let got = fetch_to(&transfer_agent(Duration::from_secs(1)), &url, None, &out, "slow.bin", 100);
    assert!(got.is_ok(), "a transfer longer than the idle limit finishes: {got:?}");
    assert_eq!(std::fs::read(out.join("slow.bin")).unwrap().len(), 50);
}

/// A FAILED FETCH LEAVES NO PARTIAL FILE: a body larger than allowed, or one cut off, leaves
/// nothing in the folder, neither under its name nor as a part file.
///
/// Seen red 2026-10-11 with no removal on a failure: "a cut-off body leaves nothing" (the partial
/// file was still there). Seen red again in round 3 with a part file not deleted when dropped:
/// "a cut-off body leaves nothing: [\".humanityos-24740-1-18dd7a53e7ff4e4c.part\"]".
#[test]
fn a_failed_fetch_leaves_no_partial_file() {
    let (url, _) = serve("Content-Length: 500\r\nConnection: close\r\n", vec![vec![b'x'; 100]], Duration::ZERO);
    let out = folder("media_open_cut");
    assert!(fetch_to(&transfer_agent(Duration::from_secs(5)), &url, None, &out, "cut.bin", 1000).is_err(), "a body cut off is an error");
    assert!(names_in(&out).is_empty(), "a cut-off body leaves nothing: {:?}", names_in(&out));

    let (url, _) = serve("Connection: close\r\n", vec![vec![b'y'; 300]], Duration::ZERO);
    let out = folder("media_open_big");
    assert!(fetch_to(&transfer_agent(Duration::from_secs(5)), &url, None, &out, "big.bin", 100).is_err());
    assert!(names_in(&out).is_empty(), "a body larger than allowed leaves nothing: {:?}", names_in(&out));
}

/// A DOWNLOAD HAS ITS REAL NAME ONLY WHEN IT IS COMPLETE (round 3, finding 6: a download cut off
/// by the app quitting was left, partial, under its final name): while it arrives only a hidden
/// part file is in the folder; when it is done, only the file under its name. And a second
/// download of the same name never lands on the first (finding 7).
///
/// Seen red 2026-10-11 with `fetch_to` writing straight to the final name (as in round 2): "only a
/// part file while it arrives" (left: ["dusk.webm"]).
#[test]
fn a_download_has_its_name_only_when_complete() {
    let out = folder("media_open_part");
    let (url, _) = serve("Content-Length: 30\r\nConnection: close\r\n", vec![vec![b'a'; 10], vec![b'b'; 10], vec![b'c'; 10]], Duration::from_millis(300));
    let (dir, u) = (out.to_path_buf(), url.clone());
    let worker = std::thread::spawn(move || fetch_to(&transfer_agent(Duration::from_secs(5)), &u, None, &dir, "dusk.webm", 100));
    std::thread::sleep(Duration::from_millis(450)); // the first piece is in, the rest not
    let during = names_in(&out);
    assert!(during.len() == 1 && crate::gui::widgets::part_file::is_part_name(&during[0]), "only a part file while it arrives (left: {during:?})");
    let path = worker.join().unwrap().expect("fetched");
    assert_eq!(names_in(&out), vec!["dusk.webm".to_string()], "only the file, under its name, once done");
    assert_eq!(std::fs::read(&path).unwrap().len(), 30);

    let (url, _) = serve("Content-Length: 3\r\nConnection: close\r\n", vec![b"xyz".to_vec()], Duration::ZERO);
    let second = fetch_to(&transfer_agent(Duration::from_secs(5)), &url, None, &out, "dusk.webm", 100).expect("fetched");
    assert_eq!(second.file_name().and_then(|n| n.to_str()), Some("dusk (2).webm"), "never over the first");
    assert_eq!(std::fs::read(&path).unwrap().len(), 30, "the first is untouched");
}

/// PLAY HERE'S COPY HAS A NAME OF ITS OWN FOR EACH FETCH (round 3, finding 7: one name per item let
/// two fetches of it, two cards or a quick close and open, write into the same file).
///
/// Seen red 2026-10-11 with the name made from the server and the item alone (as in round 2):
/// "two fetches of one item, two names" (left and right both "b358598e3ca452c15669c977.video").
#[test]
fn play_heres_copy_has_a_name_of_its_own_for_each_fetch() {
    let (a, b) = (play_name("https://h", 7), play_name("https://h", 7));
    assert_ne!(a, b, "two fetches of one item, two names");
    assert!(a.ends_with(".video") && !a.contains('/') && !a.contains('\\'));
}

/// A MEMBERS-ONLY FILE IS FETCHED WITH A TICKET (R3-8): the token goes once, in a header, to ask
/// for the ticket; the transfer itself is the ticket's address, carrying no token. A public file
/// is fetched from its address, asking nothing first.
///
/// Seen red 2026-10-11 with `origin_of` sending a members-only file to its address (as in round
/// 2, with the token header): "a members-only file asks for a ticket (left:
/// Address(\"https://h/api/media/file/7\"))".
#[test]
fn a_members_only_file_is_fetched_with_a_ticket() {
    let members = Item { id: 7, stored: "1700_dusk.webm".into(), visibility: "members".into(), url: "/api/media/file/7".into(), ..Default::default() };
    let origin = origin_of("https://h", &members, Some("tok")).expect("an origin");
    assert!(matches!(&origin, Origin::Ticket { item, token, .. } if item.id == 7 && item.filename == "1700_dusk.webm" && token == "tok"), "a members-only file asks for a ticket (left: {origin:?})");
    assert!(origin_of("https://h", &members, None).is_err(), "and needs a sign-in");
    let public = Item { id: 8, url: "/uploads/1700_a.png".into(), visibility: "public".into(), ..Default::default() };
    assert_eq!(origin_of("https://h", &public, None), Ok(Origin::Address("https://h/uploads/1700_a.png".into())));
}

/// PLAY HERE'S COPY GOES WITH ITS CARD: closing the card (dropping it) deletes the temporary
/// file, and the folder is cleared of leftovers.
///
/// Seen red 2026-10-11 with no `Drop for Opened`: "the copy goes with the card" (the file was
/// still there).
#[test]
fn play_heres_copy_goes_with_its_card() {
    let copy = dest("media_open_temp");
    std::fs::write(&copy, b"video").unwrap();
    let mut card = Opened::new(Item { id: 1, kind: "video".into(), ..Default::default() });
    card.temp = Some(copy.to_path_buf());
    drop(card);
    assert!(!copy.exists(), "the copy goes with the card");

    let dir = crate::test_temp::dir("media_open_sweep");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.video"), b"1").unwrap();
    std::fs::write(dir.join("b.video"), b"2").unwrap();
    sweep(&dir);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "the leftovers go");
}

/// DOWNLOAD KEEPS THE FILE'S OWN NAME: the uploader's name for it (so a members-only file keeps
/// its extension), else its stored name, else its title and number.
///
/// Seen red 2026-10-11 with the members-only case named by its title: "a members-only file keeps
/// its extension" (left: "Dusk-7" / right: "dusk.webm").
#[test]
fn download_keeps_the_files_own_name() {
    let members = Item { id: 7, title: "Dusk".into(), original_name: "dusk.webm".into(), visibility: "members".into(), url: "/api/media/file/7".into(), ..Default::default() };
    assert_eq!(download_name(&members), "dusk.webm", "a members-only file keeps its extension");
    let public = Item { id: 8, stored: "1700_a.png".into(), ..Default::default() };
    assert_eq!(download_name(&public), "1700_a.png");
    let bare = Item { id: 9, title: "Garden".into(), ..Default::default() };
    assert_eq!(download_name(&bare), "Garden-9");
    assert_eq!(download_name(&Item { original_name: "..\\..\\evil.exe".into(), ..bare }), "_.._evil.exe", "never a path");
}
