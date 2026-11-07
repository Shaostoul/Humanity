// Child of handlers/utils.rs: the link preview fetcher on the shared outbound guard (2026-11-06,
// BUG-255). `fetch_link_preview` is not called yet (chat-access rung 3 switches it on); these hold
// it to the guard before it is: the stand-in web server and the tripwire are outbound_tests.rs'
// (both on 127.0.0.1, the web server reached through the test seam, the tripwire never).
//
// RED FIRST (2026-11-06): the old fetcher cannot be driven by these tests (its own client and the
// system's resolver, no seam), so each was seen red against the new code with one protection taken
// out at a time, in one run of `cargo test --features relay --no-default-features --lib
// link_preview`: `OutboundGuard::get` following each Location unchecked (what the old client's
// `Policy::limited(3)` did), the guarded resolver handing over every address it found (the old
// fetcher's second, unchecked lookup), and a failed lookup let through with no addresses instead of
// refused (the old "fetch anyway"). What each said is at the test.

use super::*;
use crate::relay::outbound::tests::{tripwire, web_server};
use crate::relay::outbound::{FetchError, OutboundGuard, Refusal, Schemes};
use crate::relay::storage::Storage;
use std::net::IpAddr;
use std::sync::atomic::Ordering;

fn ip(s: &str) -> IpAddr {
    s.parse().unwrap()
}

/// A relay whose guard lets the stand-in web server at `web` through and answers fixed names.
fn relay(web: u16, tag: &str) -> Arc<RelayState> {
    let mut state = RelayState::new(Storage::open_temp(&format!("link_preview_{tag}")));
    state.outbound = OutboundGuard::with_names(&[("loopback.example.com", ip("127.0.0.1")), ("internal.example.com", ip("10.255.255.1"))])
        .allowing_test_service(web);
    Arc::new(state)
}

/// The positive control: a page it may reach is read, through a redirect it may follow.
#[tokio::test]
async fn a_link_preview_reads_a_page_through_a_checked_redirect() {
    let (web, served) = web_server().await;
    let st = relay(web, "page");
    let preview = fetch_link_preview(&st, &format!("http://127.0.0.1:{web}/to-page")).await.expect("a preview");
    assert_eq!(preview.title.as_deref(), Some("A page"));
    assert_eq!(served.load(Ordering::SeqCst), 2, "the redirect and the page");
}

/// A redirect to 127.0.0.1, written as an address or as a public-looking name that leads there,
/// is not followed: no preview, and nothing connects to the tripwire. Seen red with `get` following
/// each Location unchecked: "a redirect to http://127.0.0.1:<port>/x connected: 1 hit(s)"; with the
/// resolver unchecked: "a redirect to http://loopback.example.com:<port>/x connected: 1 hit(s)".
#[tokio::test]
async fn a_link_preview_follows_no_redirect_to_loopback() {
    let (web, _) = web_server().await;
    let (trip, hits) = tripwire().await;
    let st = relay(web, "loopback");
    for to in [
        format!("http://127.0.0.1:{trip}/x"),
        format!("http://[::ffff:127.0.0.1]:{trip}/x"),
        format!("http://localhost:{trip}/x"),
        format!("http://loopback.example.com:{trip}/x"),
    ] {
        let preview = fetch_link_preview(&st, &format!("http://127.0.0.1:{web}/to?{to}")).await;
        assert!(preview.is_none(), "a redirect to {to} gave a preview");
        assert_eq!(hits.load(Ordering::SeqCst), 0, "a redirect to {to} connected: {} hit(s)", hits.load(Ordering::SeqCst));
    }
    // And a link straight to loopback is refused before anything is sent.
    assert!(fetch_link_preview(&st, &format!("http://127.0.0.1:{trip}/")).await.is_none());
    assert!(fetch_link_preview(&st, &format!("http://loopback.example.com:{trip}/")).await.is_none());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

/// A name that leads to 10.x gets no preview, refused as private before any connection is tried,
/// as a link or as a redirect. Seen red with the resolver unchecked: "http://internal.example.com/:
/// refused as private, got Err(Failed(\"error sending request for url (http://internal.example.com/)\"))"
/// (it went on to try 10.255.255.1 until the connect time limit).
#[tokio::test]
async fn a_link_preview_of_a_name_that_leads_to_10_x_is_refused() {
    let (web, _) = web_server().await;
    let st = relay(web, "internal");
    assert!(fetch_link_preview(&st, "http://internal.example.com/").await.is_none());
    assert!(fetch_link_preview(&st, &format!("http://127.0.0.1:{web}/to?http://internal.example.com/")).await.is_none());
    for url in ["http://internal.example.com/".to_string(), format!("http://127.0.0.1:{web}/to?http://internal.example.com/")] {
        match st.outbound.get(&url, Schemes::HttpOrHttps, 3, |r| r).await {
            Err(FetchError::Refused(Refusal::PrivateNetwork)) => {}
            other => panic!("{url}: refused as private, got {:?}", other.map(|r| r.status())),
        }
    }
}

/// A name that does not resolve gets no preview and is refused as unknown, never tried another way
/// (the old fetcher fetched anyway when its own lookup failed). Seen red with a failed lookup let
/// through with no addresses: "http://nowhere.example.com/: refused as unknown, got
/// Err(Failed(\"error sending request for url (http://nowhere.example.com/)\"))".
#[tokio::test]
async fn a_link_preview_of_an_unresolvable_name_is_refused() {
    let (web, _) = web_server().await;
    let st = relay(web, "unknown");
    assert!(fetch_link_preview(&st, "http://nowhere.example.com/").await.is_none());
    for url in ["http://nowhere.example.com/".to_string(), format!("http://127.0.0.1:{web}/to?http://nowhere.example.com/")] {
        match st.outbound.get(&url, Schemes::HttpOrHttps, 3, |r| r).await {
            Err(FetchError::Refused(Refusal::UnknownName)) => {}
            other => panic!("{url}: refused as unknown, got {:?}", other.map(|r| r.status())),
        }
    }
}

/// A description is cut on a character boundary (the old byte slice at 297 panics when a
/// multi-byte character sits there).
#[test]
fn a_long_description_is_cut_on_a_character_boundary() {
    let long = format!("{}{}", "a".repeat(296), "\u{e9}".repeat(10));
    let cut = at_most_chars(long, 300);
    assert_eq!(cut.chars().count(), 298);
    assert!(cut.ends_with('\u{2026}'));
    assert_eq!(at_most_chars("short".into(), 300), "short");
}
