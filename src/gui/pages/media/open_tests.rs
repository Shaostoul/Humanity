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

/// A MEMBERS-ONLY FILE IS ASKED FOR WITH THE TOKEN IN A HEADER, NEVER IN THE ADDRESS, by both the
/// card's transfers and the tiles' pictures (the review's finding: ureq's error words hold the
/// whole address, and image_cache's `download_bytes` wrote it into its message).
///
/// Seen red 2026-10-11 with `download_bytes_with` leaving its header unset: "the picture's
/// request carries the token in its header" (the request head had no x-upload-token line).
#[test]
fn the_token_rides_in_a_header() {
    let (url, seen) = serve("Content-Length: 3\r\nConnection: close\r\n", vec![b"abc".to_vec()], Duration::ZERO);
    let out = dest("media_open_header");
    fetch_to(&transfer_agent(Duration::from_secs(5)), &url, Some((api::TOKEN_HEADER, "SECRET-1")), &out, 100).expect("fetched");
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
    let out = dest("media_open_slow");
    let got = fetch_to(&transfer_agent(Duration::from_secs(1)), &url, None, &out, 100);
    assert!(got.is_ok(), "a transfer longer than the idle limit finishes: {got:?}");
    assert_eq!(std::fs::read(&out).unwrap().len(), 50);
}

/// A FAILED FETCH LEAVES NO PARTIAL FILE: a body larger than allowed, or one cut off, removes what
/// was written.
///
/// Seen red 2026-10-11 with no removal on a failure: "a cut-off body leaves nothing" (the partial
/// file was still there).
#[test]
fn a_failed_fetch_leaves_no_partial_file() {
    let (url, _) = serve("Content-Length: 500\r\nConnection: close\r\n", vec![vec![b'x'; 100]], Duration::ZERO);
    let out = dest("media_open_cut");
    assert!(fetch_to(&transfer_agent(Duration::from_secs(5)), &url, None, &out, 1000).is_err(), "a body cut off is an error");
    assert!(!out.exists(), "a cut-off body leaves nothing");

    let (url, _) = serve("Connection: close\r\n", vec![vec![b'y'; 300]], Duration::ZERO);
    let out = dest("media_open_big");
    assert!(fetch_to(&transfer_agent(Duration::from_secs(5)), &url, None, &out, 100).is_err());
    assert!(!out.exists(), "a body larger than allowed leaves nothing");
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
