// Child of outbound.rs (#[path], so `use super::*` reaches everything it has): which addresses
// count as public, which URL shapes are refused, the guarded clients' resolver, the push send side
// (a subscription row holding a refused endpoint, put straight into the database, is never
// connected to), and `OutboundGuard::get` following redirects hop by hop. The link preview path
// through it is handlers/link_preview_tests.rs.
//
// RED FIRST (2026-11-06, BUG-255). The first five gates below were seen failing before they were
// trusted, in one run of `cargo test --features relay --no-default-features --lib push_` with the
// guard taken out (the shape check only parsing the URL, `check` never looking the name up, the
// resolver handing the connector every address it found, and `is_public` refusing loopback alone):
// 10 of the 10 new push tests failed (these and push_routes_tests.rs' 5). The redirect gates were
// seen red with `get` following a Location without checking it. What each said is at the test.

use super::*;
use crate::relay::relay::RelayState;
use crate::relay::storage::{PushSubscriptionRecord, Storage};
use crate::relay::web_push::VapidKey;
use base64ct::{Base64UrlUnpadded, Encoding};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap_or_else(|_| panic!("{s} is not an address"))
}

/// Seen red with only loopback refused: "0.0.0.0 counted as public".
#[test]
fn only_public_addresses_count_as_public() {
    let not_public = [
        "127.0.0.1", "127.1.2.3", "0.0.0.0", "0.1.2.3", "10.0.0.1", "10.255.255.255", "172.16.0.1", "172.31.255.255",
        "192.168.1.1", "169.254.169.254", "100.64.0.1", "100.127.255.254", "192.0.0.170", "192.0.2.1", "198.51.100.7",
        "203.0.113.9", "198.18.0.1", "198.19.255.255", "224.0.0.1", "239.255.255.250", "240.0.0.1", "255.255.255.255",
        "::", "::1", "::127.0.0.1", "::ffff:127.0.0.1", "::ffff:10.0.0.1", "::ffff:169.254.169.254", "64:ff9b::7f00:1",
        "64:ff9b::a00:1", "64:ff9b:1::1", "2002:7f00:1::1", "2002:a00:1::1", "fc00::1", "fd12:3456::1", "fe80::1",
        "fec0::1", "ff02::1", "100::1", "2001:db8::1", "2001::1", "2001:2::1", "3fff::1",
    ];
    for a in not_public {
        assert!(!is_public(ip(a)), "{a} counted as public");
    }
    let public = [
        "8.8.8.8", "1.1.1.1", "142.250.72.106", "172.15.255.255", "172.32.0.1", "100.63.255.255", "100.128.0.1",
        "192.0.1.1", "198.17.0.1", "198.20.0.1", "223.255.255.254", "2606:4700:4700::1111", "2a00:1450:4001:82a::200a",
        "::ffff:8.8.8.8", "64:ff9b::808:808", "2002:808:808::1", "2001:4860:4860::8888",
    ];
    for a in public {
        assert!(is_public(ip(a)), "{a} counted as not public");
    }
}

/// Seen red with the shape only parsed: `"http://fcm.googleapis.com/fcm/send/abc"`, left: None.
#[test]
fn url_shapes_the_server_does_not_send_to() {
    let https = Schemes::HttpsOnly;
    let refused = [
        ("http://fcm.googleapis.com/fcm/send/abc", Refusal::NotHttps),
        ("ftp://fcm.googleapis.com/x", Refusal::NotHttps),
        ("https://user:pw@fcm.googleapis.com/fcm/send/abc", Refusal::HasUserinfo),
        ("https://user@fcm.googleapis.com/fcm/send/abc", Refusal::HasUserinfo),
        ("https://127.0.0.1/x", Refusal::AddressNotName),
        ("https://10.0.0.1/x", Refusal::AddressNotName),
        ("https://169.254.169.254/latest/meta-data/", Refusal::AddressNotName),
        ("https://8.8.8.8/x", Refusal::AddressNotName),
        ("https://[::1]/x", Refusal::AddressNotName),
        ("https://[fc00::1]/x", Refusal::AddressNotName),
        ("https://[::ffff:127.0.0.1]/x", Refusal::AddressNotName),
        // Other spellings of 127.0.0.1 the URL standard reads as an IPv4 address.
        ("https://2130706433/x", Refusal::AddressNotName),
        ("https://0x7f.1/x", Refusal::AddressNotName),
        ("https://127.0.0.1./x", Refusal::AddressNotName),
        ("https://localhost/x", Refusal::PrivateNetwork),
        ("https://LOCALHOST./x", Refusal::PrivateNetwork),
        ("https://push.localhost/x", Refusal::PrivateNetwork),
        ("", Refusal::NotAWebAddress),
        ("not a url", Refusal::NotAWebAddress),
        ("https://fcm.googleapis.com/a\nb", Refusal::NotAWebAddress),
        ("https://fcm.googleapis.com/a b", Refusal::NotAWebAddress),
        ("https://fcm.googleapis.com/\u{e9}", Refusal::NotAWebAddress),
    ];
    for (url, why) in refused {
        assert_eq!(url_shape(url, https).err(), Some(why), "{url:?}");
    }
    let long = format!("https://fcm.googleapis.com/fcm/send/{}", "a".repeat(MAX_URL_LEN));
    assert_eq!(url_shape(&long, https).err(), Some(Refusal::TooLong));
    let longest = format!("https://fcm.googleapis.com/fcm/send/{}", "a".repeat(MAX_URL_LEN - 36));
    assert_eq!(longest.len(), MAX_URL_LEN);
    assert!(url_shape(&longest, https).is_ok(), "exactly the limit is kept");

    // What browsers and self-hosted push services actually hand out stays good.
    for url in [
        "https://fcm.googleapis.com/fcm/send/dQw4w9WgXcQ:APA91bHPRgkF3JUikC4ENAHEeMrd41Zxv3hVZjC9KtT8OvPVGJ",
        "https://updates.push.services.mozilla.com/wpush/v2/gAAAAABl3x9Y_long-token_with-dashes",
        "https://web.push.apple.com/QGuQyavXutnMH0xyz",
        "https://wns2-par02p.notify.windows.com/w/?token=BQYAAAB%2bx%3d",
        "https://ntfy.example.org:8443/upAbCdEf?up=1",
    ] {
        assert!(url_shape(url, https).is_ok(), "{url} refused");
    }

    // A web page may be plain http, under the same rules otherwise; other schemes are not web pages.
    let web = Schemes::HttpOrHttps;
    assert!(url_shape("http://example.org/a", web).is_ok());
    assert_eq!(url_shape("http://127.0.0.1/a", web).err(), Some(Refusal::AddressNotName));
    assert_eq!(url_shape("http://localhost:8080/a", web).err(), Some(Refusal::PrivateNetwork));
    assert_eq!(url_shape("ftp://example.org/a", web).err(), Some(Refusal::NotAWebAddress));
    assert_eq!(url_shape("file:///etc/passwd", web).err(), Some(Refusal::NotAWebAddress));
}

/// Seen red with the resolver unchecked: "a name leading to loopback is handed over: Ok([127.0.0.1:0])
/// (push.example.com)"; with a failed lookup let through as no addresses: "a name with no address
/// is refused".
#[tokio::test]
async fn the_guarded_clients_resolver_hands_over_only_public_addresses() {
    let resolver = PublicOnly(Lookup::Fixed(Arc::new(
        [
            ("push.example.com".to_string(), vec![ip("127.0.0.1")]),
            ("metadata.example.com".to_string(), vec![ip("169.254.169.254")]),
            ("v6.example.com".to_string(), vec![ip("::1")]),
            ("mixed.example.com".to_string(), vec![ip("8.8.8.8"), ip("10.0.0.1")]),
            ("good.example.com".to_string(), vec![ip("142.250.72.106"), ip("2a00:1450:4001:82a::200a")]),
        ]
        .into_iter()
        .collect(),
    )));
    let resolve = |name: &str| reqwest::dns::Resolve::resolve(&resolver, reqwest::dns::Name::from_str(name).unwrap());
    for name in ["push.example.com", "metadata.example.com", "v6.example.com", "mixed.example.com"] {
        let got = resolve(name).await.map(|a| a.collect::<Vec<_>>());
        assert!(got.is_err(), "a name leading to loopback is handed over: {got:?} ({name})");
    }
    assert!(resolve("nowhere.example.com").await.is_err(), "a name with no address is refused");
    let good: Vec<SocketAddr> = resolve("good.example.com").await.unwrap().collect();
    assert_eq!(good, vec![SocketAddr::new(ip("142.250.72.106"), 0), SocketAddr::new(ip("2a00:1450:4001:82a::200a"), 0)]);
}

/// A listener on 127.0.0.1 that counts every connection made to it and closes each at once.
pub(crate) async fn tripwire() -> (u16, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let count = hits.clone();
    tokio::spawn(async move {
        while let Ok((socket, _)) = listener.accept().await {
            count.fetch_add(1, Ordering::SeqCst);
            drop(socket);
        }
    });
    (port, hits)
}

fn row(endpoint: String) -> PushSubscriptionRecord {
    PushSubscriptionRecord {
        public_key: "alice-key".into(),
        endpoint,
        p256dh: Base64UrlUnpadded::encode_string(&ua_key()),
        auth: Base64UrlUnpadded::encode_string(&[7u8; 16]),
    }
}

/// A browser's P-256 public key (uncompressed SEC1), from RFC 8291's example user agent.
fn ua_key() -> Vec<u8> {
    Base64UrlUnpadded::decode_vec("BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4").unwrap()
}

fn state_with(guard: OutboundGuard, tag: &str) -> RelayState {
    let mut state = RelayState::new(Storage::open_temp(tag));
    state.vapid_key = Some(VapidKey::generate().unwrap());
    state.outbound = guard;
    state
}

/// THE PUSH SEND SIDE. Rows holding refused endpoints, all pointing at a listener on 127.0.0.1, put
/// straight into the subscription list: the relay sends to every one and the listener hears
/// nothing. Then, through the test seam, the same listener hears the one push it should: the
/// tripwire can fire, so its silence above means something.
///
/// Seen red with the guard taken out: "the relay connected to 127.0.0.1 4 time(s)" (the IP-address
/// rows and the name leading to loopback; `http://` was still refused by the client's https-only).
#[tokio::test]
async fn a_refused_endpoint_in_the_database_is_never_connected_to() {
    let (port, hits) = tripwire().await;
    let names = [("push.example.com", ip("127.0.0.1")), ("v6.example.com", ip("::1"))];
    let state = state_with(OutboundGuard::with_names(&names), "push_tripwire");
    let rows = vec![
        row(format!("http://127.0.0.1:{port}/plain")),
        row(format!("https://127.0.0.1:{port}/literal")),
        row(format!("https://[::ffff:127.0.0.1]:{port}/mapped")),
        row(format!("https://2130706433:{port}/decimal")),
        row(format!("https://localhost:{port}/localhost")),
        row(format!("https://push.example.com:{port}/name-to-loopback")),
        row(format!("https://v6.example.com:{port}/name-to-v6-loopback")),
        row(format!("https://user:pw@push.example.com:{port}/userinfo")),
    ];
    state.deliver_pushes(rows, r#"{"title":"t"}"#.into()).await;
    assert_eq!(hits.load(Ordering::SeqCst), 0, "the relay connected to 127.0.0.1 {} time(s)", hits.load(Ordering::SeqCst));

    // The positive control: the stand-in service's own address, let through by the test seam.
    let seam = state_with(OutboundGuard::default().allowing_test_service(port), "push_tripwire_control");
    seam.deliver_pushes(vec![row(format!("http://127.0.0.1:{port}/allowed"))], r#"{"title":"t"}"#.into()).await;
    assert_eq!(hits.load(Ordering::SeqCst), 1, "the tripwire hears a push that is let through");
    // And the seam lets through that one address only: another port on loopback stays refused.
    let (other, other_hits) = tripwire().await;
    seam.deliver_pushes(vec![row(format!("http://127.0.0.1:{other}/other-port"))], r#"{"title":"t"}"#.into()).await;
    assert_eq!(other_hits.load(Ordering::SeqCst), 0, "the seam opened another loopback port");
}

/// The test seam is narrow: only plain http to 127.0.0.1 at its own port. Seen red with the shape
/// only parsed: "http://127.0.0.1:4568/x let through".
#[test]
fn the_test_seam_lets_through_one_address_only() {
    let seam = OutboundGuard::default().allowing_test_service(4567);
    assert!(seam.shape("http://127.0.0.1:4567/wpush/v2/abc", Schemes::HttpsOnly).is_ok());
    for url in ["http://127.0.0.1:4568/x", "http://localhost:4567/x", "http://[::1]:4567/x", "https://127.0.0.1:4567/x", "http://10.0.0.1:4567/x"] {
        assert!(seam.shape(url, Schemes::HttpOrHttps).is_err(), "{url} let through");
    }
    assert!(OutboundGuard::default().shape("http://127.0.0.1:4567/wpush/v2/abc", Schemes::HttpOrHttps).is_err(), "the default guard has no seam");
    assert!(!seam.https_only() && OutboundGuard::default().https_only(), "only the seam's client speaks plain http");
}

/// A stand-in web server on 127.0.0.1 (reached through the test seam): `/page` is a page with a
/// title, `/to-page` redirects there, `/loop` redirects to itself, and `/to?<url>` redirects to the
/// URL given. Counts the requests it answers.
pub(crate) async fn web_server() -> (u16, Arc<AtomicUsize>) {
    let served = Arc::new(AtomicUsize::new(0));
    let count = served.clone();
    let app = axum::Router::new().fallback(move |uri: axum::http::Uri| {
        let count = count.clone();
        async move {
            use axum::response::IntoResponse;
            count.fetch_add(1, Ordering::SeqCst);
            let redirect = |to: String| (axum::http::StatusCode::FOUND, [(axum::http::header::LOCATION, to)]).into_response();
            match uri.path() {
                "/page" => (
                    [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    "<html><head><meta property=\"og:title\" content=\"A page\"><title>x</title></head></html>",
                )
                    .into_response(),
                "/to-page" => redirect("/page".into()),
                "/loop" => redirect("/loop".into()),
                "/to" => redirect(uri.query().unwrap_or("").to_string()),
                _ => axum::http::StatusCode::NOT_FOUND.into_response(),
            }
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (port, served)
}

/// `get` follows a redirect only to an address it may send to, checked hop by hop: a redirect to
/// 127.0.0.1 (written as an address, as localhost, or as a public-looking name that leads there)
/// or to a name leading to 10.x is refused and nothing connects; one to a name that does not
/// resolve is refused; a loop stops at the limit; and a redirect it may follow is followed.
///
/// Seen red with `get` following each Location unchecked (the old fetcher's client did, with
/// `Policy::limited(3)`): "a redirect to http://127.0.0.1:<port>/x: refused as AddressNotName
/// expected, got Failed(\"error sending request ...\")" (it connected, and the tripwire hung up);
/// and with the resolver unchecked: "a redirect to http://loopback.example.com:<port>/x: refused as
/// PrivateNetwork expected, got Failed(...)".
#[tokio::test]
async fn redirects_are_followed_only_to_addresses_checked_hop_by_hop() {
    let (web, _) = web_server().await;
    let (trip, hits) = tripwire().await;
    let guard = OutboundGuard::with_names(&[("loopback.example.com", ip("127.0.0.1")), ("internal.example.com", ip("10.255.255.1"))])
        .allowing_test_service(web);
    let get = |url: String| {
        let guard = &guard;
        async move { guard.get(&url, Schemes::HttpOrHttps, 3, |r| r.timeout(Duration::from_secs(5))).await }
    };
    let start = |to: &str| format!("http://127.0.0.1:{web}/to?{to}");

    for (to, why) in [
        (format!("http://127.0.0.1:{trip}/x"), Refusal::AddressNotName),
        (format!("http://[::ffff:127.0.0.1]:{trip}/x"), Refusal::AddressNotName),
        (format!("http://localhost:{trip}/x"), Refusal::PrivateNetwork),
        (format!("http://loopback.example.com:{trip}/x"), Refusal::PrivateNetwork),
        ("http://internal.example.com/x".to_string(), Refusal::PrivateNetwork),
        ("http://nowhere.example.com/x".to_string(), Refusal::UnknownName),
        ("file:///etc/passwd".to_string(), Refusal::NotAWebAddress),
    ] {
        match get(start(&to)).await {
            Err(FetchError::Refused(r)) => assert_eq!(r, why, "a redirect to {to}"),
            Err(e) => panic!("a redirect to {to}: refused as {why:?} expected, got {e:?}"),
            Ok(resp) => panic!("a redirect to {to} was followed: {} ({} hit(s))", resp.status(), hits.load(Ordering::SeqCst)),
        }
        assert_eq!(hits.load(Ordering::SeqCst), 0, "a redirect to {to} connected: {} hit(s)", hits.load(Ordering::SeqCst));
    }

    // A loop stops at the limit; a redirect to the page it may reach is followed.
    assert!(matches!(get(format!("http://127.0.0.1:{web}/loop")).await, Err(FetchError::TooManyRedirects)));
    let page = get(format!("http://127.0.0.1:{web}/to-page")).await.expect("followed");
    assert_eq!(page.status(), 200);
    assert!(page.text().await.unwrap().contains("A page"));
    // The first address is checked the same way (no stand-in there: refused before anything is sent).
    assert!(matches!(get(format!("http://127.0.0.1:{trip}/x")).await, Err(FetchError::Refused(Refusal::AddressNotName))));
    assert!(matches!(get("http://internal.example.com/".into()).await, Err(FetchError::Refused(Refusal::PrivateNetwork))));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
