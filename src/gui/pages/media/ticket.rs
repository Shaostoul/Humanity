//! Tickets (the relay's R3-8, round 3 of docs/design/media-showcase.md): a members-only file is
//! watched (Play here) or downloaded through an address that opens that one file for ten minutes,
//! instead of a request carrying this session's upload token for as long as a big transfer takes.
//!
//! `POST /api/media/ticket`, the token in `X-Upload-Token`, the body `{id, filename, file,
//! download}` (Media round 5: which of the item's files, and whether the address is the counted
//! download route), answers `{url: "/api/media/file/<id>?file=<n>&ticket=<random>", expires_in_s:
//! 600}`, or `/api/media/download/<id>?file=<n>&ticket=...` for a download. The ticket reveals nothing
//! about the session, a copied link stops working after ten minutes, and range requests work with
//! it. The address is used only when it is that file's on this server: a ticket for another
//! address, or another server's, is refused here. A ticket in words shown or logged is hidden the
//! way a token is (`hide_token`).
//!
//! Pictures shown in place keep being fetched with the token in the header (tiles.rs): they are at
//! most 16 MB and fetched once.

use std::time::Duration;

use serde_json::Value;

use super::api::{self, ItemRef};
use crate::gui::widgets::image_cache::hide_token;

/// The ticket route.
pub const TICKET_PATH: &str = "/api/media/ticket";

/// The full address a ticket's answer gives, on the server at `base`: only the members' file
/// route for that item, with its ticket; anything else is refused.
pub fn ticket_address(base: &str, id: i64, answer: &Value) -> Result<String, String> {
    ticket_address_for(base, id, false, answer)
}

/// [`ticket_address`] for a ticket asked with `download` (Media round 5): then only the counted
/// download route for that item, `/api/media/download/<id>?file=<n>&ticket=`, is taken, and a
/// download never quietly becomes an uncounted fetch (nor the other way round).
pub fn ticket_address_for(base: &str, id: i64, download: bool, answer: &Value) -> Result<String, String> {
    let url = answer.get("url").and_then(Value::as_str).unwrap_or("");
    let route = format!("/api/media/{}/{id}?", if download { "download" } else { "file" });
    if !url.starts_with(&route) || !url.contains("ticket=") || url.contains("://") || url.contains("..") {
        return Err("The server's answer for this file was not understood.".to_string());
    }
    Ok(format!("{}{url}", base.trim_end_matches('/')))
}

/// Ask the server at `base` for a ticket to `item`'s own file (file 0, to watch it), with this
/// session's upload `token` in the header. Blocking: call it on a worker thread. The words never
/// hold the token or a ticket.
pub fn ask(base: &str, token: &str, item: &ItemRef) -> Result<String, String> {
    ask_for(base, token, item, 0, false)
}

/// [`ask`] for file `file` of `item` (Media round 5: 0 is the item's own, any other an added file
/// or the cover), through the counted download route when `download`.
pub fn ask_for(base: &str, token: &str, item: &ItemRef, file: i64, download: bool) -> Result<String, String> {
    let url = format!("{}{TICKET_PATH}", base.trim_end_matches('/'));
    let result = ureq::post(&url)
        .timeout(Duration::from_secs(30))
        .set(api::TOKEN_HEADER, token)
        .set("Content-Type", "application/json")
        .send_string(&super::api_items::ticket_request(item, file, download).to_string());
    let resp = result.map_err(|e| {
        let words = match e {
            ureq::Error::Status(code, r) => {
                let text = r.into_string().unwrap_or_default();
                let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
                match v.get("error").and_then(Value::as_str) {
                    Some(err) => err.to_string(),
                    None if text.trim().is_empty() || text.trim_start().starts_with('<') => format!("The server refused it ({code})."),
                    None => text.trim().to_string(),
                }
            }
            other => format!("The server could not be reached ({other})."),
        };
        hide_token(&words)
    })?;
    let v: Value = serde_json::from_str(&resp.into_string().map_err(|e| format!("The answer could not be read ({e})."))?).unwrap_or(Value::Null);
    ticket_address_for(base, item.id, download, &v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::{Read as _, Write as _};

    /// A TICKET'S ADDRESS IS USED ONLY WHEN IT IS THAT FILE'S, ON THIS SERVER: the answer's
    /// `/api/media/file/<id>?ticket=` is put on this server's address; an answer naming another
    /// server, another item, or no ticket is refused.
    ///
    /// Seen red 2026-10-11 with an absolute `url` in the answer used as it came: "refused:
    /// {\"url\":\"https://elsewhere.example/api/media/file/7?ticket=abc\"}" (it was taken).
    #[test]
    fn a_tickets_address_must_be_that_files_on_this_server() {
        let base = "https://chat.example";
        let good = json!({ "url": "/api/media/file/7?ticket=abc123", "expires_in_s": 600 });
        assert_eq!(ticket_address(base, 7, &good).as_deref(), Ok("https://chat.example/api/media/file/7?ticket=abc123"));
        for bad in [
            json!({ "url": "https://elsewhere.example/api/media/file/7?ticket=abc" }),
            json!({ "url": "/api/media/file/8?ticket=abc" }),
            json!({ "url": "/api/media/file/7?other=1" }),
            json!({ "url": "/api/media/file/7?ticket=a/../../x" }),
            json!({}),
        ] {
            assert!(ticket_address(base, 7, &bad).is_err(), "refused: {bad}");
        }
        let r = ItemRef { id: 7, filename: "/uploads/1700_dusk.webm".into() };
        // Round 5: the body names which file, and whether it is a counted download.
        assert_eq!(super::super::api_items::ticket_request(&r, 0, false), json!({ "id": 7, "filename": "1700_dusk.webm", "file": 0, "download": false }));
        // A download's ticket is taken only on the download route, and a watch's only on the file route.
        let dl = json!({ "url": "/api/media/download/7?file=1&ticket=abc123" });
        assert_eq!(ticket_address_for(base, 7, true, &dl).as_deref(), Ok("https://chat.example/api/media/download/7?file=1&ticket=abc123"));
        assert!(ticket_address_for(base, 7, false, &dl).is_err() && ticket_address_for(base, 7, true, &good).is_err());
    }

    /// THE TICKET IS ASKED FOR WITH THE TOKEN IN A HEADER, and a refusal's words hold neither the
    /// token nor a ticket.
    ///
    /// Seen red 2026-10-11 with `hide_token` hiding only `token=`: "a ticket in words is hidden"
    /// (left: "GET http://h/api/media/file/7?ticket=SECRET-TICKET: refused").
    #[test]
    fn a_ticket_is_asked_with_the_token_in_a_header() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut got = Vec::new();
            let mut buf = [0u8; 4096];
            // The head and the small JSON body.
            while !String::from_utf8_lossy(&got).contains("\"filename\"") {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                got.extend_from_slice(&buf[..n]);
            }
            let body = r#"{"url":"/api/media/file/7?ticket=T1","expires_in_s":600}"#;
            let _ = stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes());
            String::from_utf8_lossy(&got).to_ascii_lowercase()
        });
        let base = format!("http://127.0.0.1:{port}");
        let got = ask(&base, "SECRET-TOKEN-T", &ItemRef { id: 7, filename: "1700_dusk.webm".into() });
        assert_eq!(got, Ok(format!("{base}/api/media/file/7?ticket=T1")));
        let head = server.join().unwrap();
        assert!(head.starts_with("post /api/media/ticket http/1.1"), "no token in the address: {head}");
        assert!(head.contains("x-upload-token: secret-token-t"), "it rides in the header: {head}");
        assert!(head.contains(r#""id":7"#), "the body names the item: {head}");

        assert_eq!(hide_token("GET http://h/api/media/file/7?ticket=SECRET-TICKET: refused"), "GET http://h/api/media/file/7?ticket=(hidden) refused", "a ticket in words is hidden");
    }
}
