// Child of origins.rs (#[path], so `use super::*` reaches everything it has): which websites may
// call a relay whose owner never set ALLOWED_ORIGINS (BUG-329), as the rules and through a real
// relay's /ws on loopback.

use super::*;

fn vars(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string())
}

/// Its own site comes from the names the call forwarder already uses, and only a plain domain
/// name counts.
#[test]
fn own_site_is_this_servers_domain_name_and_nothing_else() {
    assert_eq!(own_site(vars(&[("TURN_PUBLIC_HOST", "public.guide")])).as_deref(), Some("https://public.guide"));
    assert_eq!(own_site(vars(&[("TURN_PUBLIC_HOST", " Public.Guide ")])).as_deref(), Some("https://public.guide"));
    assert_eq!(own_site(vars(&[("TURN_SERVER_HOST", "relay.example")])).as_deref(), Some("https://relay.example"), "the forwarder's fallback name");
    assert_eq!(
        own_site(vars(&[("TURN_PUBLIC_HOST", "calls.example"), ("TURN_SERVER_HOST", "relay.example")])).as_deref(),
        Some("https://calls.example")
    );
    for not_a_site in ["", "  ", "203.0.113.4", "2001:db8::7", "localhost", "https://public.guide", "public.guide/x", "a b.example", ".example", "x..example", "-x.example"] {
        assert_eq!(own_site(|k: &str| (k == "TURN_PUBLIC_HOST").then(|| not_a_site.to_string())), None, "{not_a_site:?}");
    }
    assert_eq!(own_site(vars(&[])), None);
}

/// The WebSocket's rule, as a table: the list always; with no list, also a page this server
/// served itself over https, and nothing else.
///
/// Seen red 2026-11-20 with the same-origin rule taken out of `accepts`: "unconfigured true,
/// Origin https://public.guide, Host Some(\"public.guide\") left: false, right: true".
#[test]
fn with_no_list_only_this_servers_own_https_pages_pass() {
    let list = vec!["tauri://localhost".to_string(), "http://127.0.0.1:3210".to_string()];
    let cases: &[(bool, &str, Option<&str>, bool)] = &[
        (true, "https://public.guide", Some("public.guide"), true),
        (true, "https://chat.public.guide", Some("chat.public.guide"), true),
        (true, "https://Public.Guide", Some("public.guide"), true),
        (true, "https://public.guide:8443", Some("public.guide:8443"), true),
        (true, "https://united-humanity.us", Some("public.guide"), false),
        (true, "http://public.guide", Some("public.guide"), false),
        (true, "https://public.guide", None, false),
        (true, "https://public.guide", Some(""), false),
        (true, "https://public.guide:8443", Some("public.guide"), false),
        (true, "https://evil.example", Some("evil.example, public.guide"), false),
        (true, "tauri://localhost", Some("public.guide"), true),
        (false, "https://public.guide", Some("public.guide"), false),
        (false, "tauri://localhost", None, true),
    ];
    for &(unconfigured, origin, host, want) in cases {
        assert_eq!(accepts(&list, unconfigured, origin, host), want, "unconfigured {unconfigured}, Origin {origin}, Host {host:?}");
    }
    assert!(configured(Some("https://x.example")) && !configured(Some(" ")) && !configured(None));
    assert!(not_configured_line(None).contains("ALLOWED_ORIGINS=https://<this server's domain>"));
    assert!(not_configured_line(Some("https://public.guide")).contains("https://public.guide (its TURN_PUBLIC_HOST)"));
}

/// THROUGH A REAL RELAY'S /ws, with no ALLOWED_ORIGINS (as on a server never told it): a browser
/// page of united-humanity.us is refused (403) at a server answering as public.guide, a page that
/// server served itself over https is let in, the same page over plain http is refused, and a
/// client with no Origin (the desktop app, a bot) is let in. This is what the browser sees, the
/// handshake's answer, not what the rule was told.
///
/// Seen red 2026-11-20 with the old default put back in `parse_allowed_origins` (united-humanity.us's
/// two addresses for an unset list) and the same-origin rule taken out of `accepts`: "Origin
/// https://united-humanity.us at Host public.guide: let in, should be refused / Origin
/// https://public.guide at Host public.guide: refused, should be let in / Origin
/// https://chat.public.guide at Host chat.public.guide: refused, should be let in".
#[tokio::test]
async fn a_relay_never_told_its_websites_answers_only_its_own_pages() {
    use tokio_tungstenite::tungstenite::{client::IntoClientRequest, Error as WsError};
    for k in ["ALLOWED_ORIGINS", "TURN_PUBLIC_HOST", "TURN_SERVER_HOST"] {
        if std::env::var(k).is_ok_and(|v| !v.trim().is_empty()) {
            eprintln!("skipped: {k} is set in this test's environment, so this is not a relay that was never told");
            return;
        }
    }
    let dir = std::env::temp_dir().join(format!("hos_origins_{}_{}", std::process::id(), rand::random::<u32>()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = crate::relay::storage::Storage::open(&dir.join("relay.db")).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let _state = crate::relay::serve_relay(db, listener).await;

    // Whether the handshake at Host `host` with Origin `origin` was let in (101) or refused (403).
    let let_in = |origin: Option<&'static str>, host: &'static str| async move {
        let mut request = format!("ws://127.0.0.1:{port}/ws").into_client_request().unwrap();
        request.headers_mut().insert("host", host.parse().unwrap());
        if let Some(o) = origin {
            request.headers_mut().insert("origin", o.parse().unwrap());
        }
        match tokio_tungstenite::connect_async(request).await {
            Ok(_) => true,
            Err(WsError::Http(res)) if res.status() == 403 => false,
            Err(e) => panic!("Origin {origin:?} at Host {host}: neither let in nor refused: {e}"),
        }
    };
    // Every case is tried before judging, so a red run names every one that went wrong.
    let mut wrong = Vec::new();
    for (origin, host, want) in [
        (Some("https://united-humanity.us"), "public.guide", false),
        (Some("https://public.guide"), "public.guide", true),
        (Some("https://chat.public.guide"), "chat.public.guide", true),
        (Some("http://public.guide"), "public.guide", false),
        (Some("https://evil.example"), "public.guide", false),
        (None, "public.guide", true),
    ] {
        if let_in(origin, host).await != want {
            wrong.push(format!("Origin {} at Host {host}: {}", origin.unwrap_or("(none)"), if want { "refused, should be let in" } else { "let in, should be refused" }));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
    let _ = std::fs::remove_dir_all(&dir);
}
