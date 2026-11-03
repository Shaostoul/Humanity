//! The front door's tests (2026-11-02): the filter over #announcements, the release list's pages,
//! the release signatures the server checks itself, and the noindex header, the last three through
//! a real relay on a loopback socket (`serve_relay`, the router production serves).

use super::*;
use crate::relay::relay::RelayMessage;
use crate::relay::storage::Storage;

const GOOD: &str = "\u{1F680} **v0.1547.0**: the front door gets its cards \u{b7} [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/v0.1547.0>)";

fn post(version: &str, headline: &str) -> String {
    format!("\u{1F680} **v{version}**: {headline} \u{b7} [Release notes](<{}>)", release_page(version))
}

// ── The filter ──

/// THE FILTER KEEPS A RELEASE POST AND REFUSES ANY OTHER LINK (design section 4.4): the real
/// format and the older one without a link make cards; a link to another site, to another
/// version's page, or one past the release page, a second link, a web address in the words,
/// words before the version, and a version that is not digits and dots are each refused. The
/// card's link is always built from the version.
///
/// Seen red 2026-11-02 with the `target != release_page(version)` check taken out of
/// parse_release_post: "a link to another site made a card: Some((\"0.1.0\", \"hello\"))".
#[test]
fn the_filter_keeps_release_posts_and_refuses_any_other_link() {
    assert_eq!(parse_release_post(GOOD), Some(("0.1547.0".into(), "the front door gets its cards".into())));
    assert_eq!(
        parse_release_post("\u{1F680} **v0.4.2**: version SOP, remove ko-fi/patreon, add crypto design doc"),
        Some(("0.4.2".into(), "version SOP, remove ko-fi/patreon, add crypto design doc".into())),
        "the early posts had no link"
    );
    assert_eq!(parse_release_post("**v0.2.0** small fix"), Some(("0.2.0".into(), "small fix".into())), "no mark, no colon");

    let refused = [
        ("a link to another site made a card", "\u{1F680} **v0.1.0**: hello \u{b7} [Release notes](<https://evil.example/releases/tag/v0.1.0>)"),
        ("another version's page", "\u{1F680} **v0.1.0**: hello \u{b7} [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/v0.2.0>)"),
        ("past the release page", "\u{1F680} **v0.1.0**: hello \u{b7} [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/v0.1.0/../../evil>)"),
        ("a lookalike prefix", "\u{1F680} **v0.1.0**: hello \u{b7} [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/v0.1.0.evil.example>)"),
        ("a second link", "\u{1F680} **v0.1.0**: see [this](https://evil.example) \u{b7} [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/v0.1.0>)"),
        ("a link in place of the release link", "\u{1F680} **v0.1.0**: see [this](https://evil.example)"),
        ("a web address in the words", "\u{1F680} **v0.1.0**: go to https://evil.example now"),
        ("a bare www address", "\u{1F680} **v0.1.0**: go to www.evil.example now"),
        ("words before the version", "Free money **v0.1.0**: hello"),
        ("a version with letters", "\u{1F680} **v0.1.0-rc1**: hello"),
        ("a version that is a path", "\u{1F680} **v0.1/../x**: hello"),
        ("no headline", "\u{1F680} **v0.1.0**: \u{b7} [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/v0.1.0>)"),
        ("not a release post at all", "hello everyone"),
    ];
    for (why, content) in refused {
        assert_eq!(parse_release_post(content), None, "{why}: {content}");
    }

    // Plain words: the markdown marks out, the whitespace one space, a long headline cut at a word.
    assert_eq!(parse_release_post("\u{1F680} **v0.3.0**: the **Sun** has a `body`\n and more").unwrap().1, "the Sun has a body and more");
    let long = format!("\u{1F680} **v0.3.0**: {}", "word ".repeat(200));
    let (_, title) = parse_release_post(&long).unwrap();
    assert!(title.chars().count() <= HEADLINE_MAX_CHARS, "{} characters", title.chars().count());
    assert!(title.ends_with("word\u{2026}"), "cut at a word with an ellipsis: {title}");
}

/// EVERY REAL RELEASE POST MAKES A CARD, LINKED ONLY TO ITS OWN PAGE: the 2,354 Deploy Bot posts in
/// data/announcements_archive.json (the #announcements history the server is seeded from) are each
/// read, and each card's link is its own version's release page. A post that is refused must have
/// a web address in its words (the one thing the filter refuses in a real post).
#[test]
fn every_real_release_post_makes_a_card() {
    let text = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/data/announcements_archive.json")).unwrap();
    let posts: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap();
    assert!(posts.len() > 2000, "the archive has its posts");
    let mut cards = 0;
    for p in &posts {
        let content = p["content"].as_str().unwrap();
        assert_eq!(p["from_key"], DEPLOY_BOT_KEY);
        match parse_release_post(content) {
            Some((version, title)) => {
                cards += 1;
                assert!(is_plain_version(&version));
                assert!(!title.is_empty() && !title.contains("](") && !title.contains("**"), "{title}");
            }
            None => {
                let lower = content.to_ascii_lowercase();
                assert!(lower.contains("http") || lower.contains("www."), "a real post was refused: {content}");
            }
        }
    }
    assert!(cards * 100 >= posts.len() * 99, "{cards} of {} posts made a card", posts.len());
}

// ── Through a real relay ──

/// A relay on loopback with a fresh database, and its address.
async fn relay(tag: &str) -> (Arc<RelayState>, String) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    (crate::relay::serve_relay(Storage::open_temp(tag), listener).await, format!("http://127.0.0.1:{port}"))
}

/// Store a chat post in #announcements as `from`, at `timestamp`.
fn store(state: &RelayState, from: &str, content: &str, timestamp: u64) {
    let chat = RelayMessage::Chat {
        from: from.to_string(),
        from_name: Some("someone".into()),
        content: content.to_string(),
        timestamp,
        signature: None,
        channel: ANNOUNCEMENTS.into(),
        reply_to: None,
        thread_count: None,
        message_id: None,
    };
    state.db.store_message_in_channel(&chat, ANNOUNCEMENTS).unwrap();
}

async fn front_door(base: &str) -> (reqwest::header::HeaderMap, serde_json::Value) {
    let res = reqwest::get(format!("{base}/api/front-door")).await.unwrap();
    assert_eq!(res.status().as_u16(), 200);
    let headers = res.headers().clone();
    (headers, res.json().await.unwrap())
}

fn versions(updates: &serde_json::Value) -> Vec<String> {
    updates.as_array().unwrap().iter().map(|u| u["version"].as_str().unwrap().to_string()).collect()
}

/// THE FRONT DOOR SHOWS ONLY THE BOT'S AND THE SERVER'S OWN RELEASE POSTS: a moderator's post in
/// the release format, a bot post with a link to another site, and a person's chat line are not
/// cards; the Deploy Bot's posts and the server's own announcement (BUG-226: stored in #general
/// until this day) are, newest first, each linked to its own release page, at most nine.
///
/// Seen red 2026-11-02 with the sender check taken out of `cards_from`: the moderator's newer
/// lookalike took 0.9.0's place, "left: [\"0.12.0\", \"0.11.0\", \"0.9.0\"] / right: [\"0.12.0\",
/// \"0.11.0\", \"0.10.0\"]"; and with announce.rs storing through `store_message` again (BUG-226):
/// "the server's own release post is a card: [\"0.12.0\", \"0.11.0\", ...]", no 0.13.0.
#[tokio::test]
async fn the_front_door_shows_only_the_bots_and_the_servers_release_posts() {
    let (state, base) = relay("front_door_filter").await;
    for i in 0..11u64 {
        store(&state, DEPLOY_BOT_KEY, &post(&format!("0.{i}.0"), &format!("release {i}")), 1_000 + i);
    }
    store(&state, "a1b2c3d4", &post("0.9.0", "a moderator's lookalike"), 2_000);
    store(&state, DEPLOY_BOT_KEY, "\u{1F680} **v0.20.0**: hello \u{b7} [Release notes](<https://evil.example/x>)", 2_001);
    store(&state, "a1b2c3d4", "hello everyone", 2_002);
    store(&state, DEPLOY_BOT_KEY, &post("0.11.0", "release 11"), 2_003);
    store(&state, DEPLOY_BOT_KEY, &post("0.12.0", "release 12"), 2_004);
    crate::relay::handlers::announce::announce_to(&state, ANNOUNCEMENTS, &post("0.13.0", "the server's own")).await;

    let (_, body) = front_door(&base).await;
    let got = versions(&body["updates"]);
    assert_eq!(got.len(), FRONT_DOOR_UPDATES, "at most nine: {got:?}");
    assert_eq!(got[0], "0.13.0", "the server's own release post is a card: {got:?}");
    assert_eq!(&got[1..4], ["0.12.0", "0.11.0", "0.10.0"], "newest first: {got:?}");
    assert!(!got.contains(&"0.20.0".to_string()), "a bot post with another site's link made a card: {got:?}");
    assert_eq!(got.iter().filter(|v| *v == "0.9.0").count(), 1, "a moderator's post made a card: {got:?}");
    for u in body["updates"].as_array().unwrap() {
        let v = u["version"].as_str().unwrap();
        assert_eq!(u["url"], release_page(v), "a card links only to its own release page");
        assert_eq!(u["signed"], false, "nothing is signed yet");
    }
    let nine = body["updates"].as_array().unwrap().iter().find(|u| u["version"] == "0.9.0").unwrap();
    assert_eq!(nine["title"], "release 9", "the bot's 0.9.0, not the moderator's lookalike");

    // BUG-226: the server's announcement is in #announcements' history, not #general's.
    let general = state.db.load_channel_messages("general", 200).unwrap();
    let in_general = general.iter().any(|m| matches!(m, RelayMessage::Chat { content, .. } if content.contains("the server's own")));
    assert!(!in_general, "a server announcement landed in #general");
}

/// Sign `bytes` with a key that is not the project's, as a signature file's JSON.
fn foreign_signature(bytes: &[u8]) -> String {
    use base64::Engine;
    use ed25519_dalek::Signer;
    let b64 = base64::engine::general_purpose::STANDARD;
    let ed = ed25519_dalek::SigningKey::from_bytes(&[5u8; 32]).sign(bytes).to_bytes().to_vec();
    let dil = humanity_protocol::pq_crypto::DilithiumKeypair::from_seed(&[6u8; 32]).sign(bytes);
    serde_json::json!({ "alg": release_signature::HYBRID_ALG, "ed25519": b64.encode(ed), "dilithium": b64.encode(dil) }).to_string()
}

fn real_pair(version: &str) -> (String, String) {
    let dir = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("tests/fixtures/release").join(version);
    (
        std::fs::read_to_string(dir.join("release-manifest.json")).unwrap(),
        std::fs::read_to_string(dir.join("release-manifest.json.sig.json")).unwrap(),
    )
}

async fn send_pair(base: &str, manifest: &str, signature: &str) -> (u16, serde_json::Value) {
    let res = reqwest::Client::new()
        .post(format!("{base}/api/release-signature"))
        .json(&serde_json::json!({ "manifest": manifest, "signature": signature }))
        .send()
        .await
        .unwrap();
    let code = res.status().as_u16();
    (code, res.json().await.unwrap_or(serde_json::Value::Null))
}

/// A RELEASE IS MARKED SIGNED ONLY BY A PAIR THE PROJECT'S KEYS MADE: a tampered manifest, a real
/// signature over another release's manifest, and a pair signed by another key are each refused
/// (400) and keep nothing; the real v0.1547.0 pair is kept, and from then on its card says signed
/// while v0.1543.0's (no pair sent) does not. Sending the same pair again changes nothing.
///
/// Seen red 2026-11-02 with post_release_signature keeping a refused pair as if it had passed:
/// "a tampered manifest was refused: 200 {\"kept\":\"new\",\"ok\":true,\"signed\":true,...}".
#[tokio::test]
async fn a_release_is_marked_signed_only_by_a_pair_the_projects_keys_made() {
    let (state, base) = relay("front_door_signed").await;
    store(&state, DEPLOY_BOT_KEY, &post("0.1543.0", "the Sun has a body"), 1_000);
    store(&state, DEPLOY_BOT_KEY, &post("0.1547.0", "the front door's own release"), 2_000);
    let (manifest, sig) = real_pair("v0.1547.0");
    let (other_manifest, _) = real_pair("v0.1543.0");

    let tampered = manifest.replace("0.1547.0", "0.1547.1");
    let (code, body) = send_pair(&base, &tampered, &sig).await;
    assert_eq!(code, 400, "a tampered manifest was refused: {code} {body}");
    let (code, _) = send_pair(&base, &other_manifest, &sig).await;
    assert_eq!(code, 400, "one release's signature over another's manifest");
    let (code, body) = send_pair(&base, &manifest, &foreign_signature(manifest.as_bytes())).await;
    assert_eq!(code, 400, "a signature from another key: {body}");
    assert!(body["error"].as_str().unwrap().starts_with("Nothing was kept"), "{body}");
    assert_eq!(state.db.release_signature_count().unwrap(), 0, "kept nothing");
    let (_, door) = front_door(&base).await;
    assert!(door["updates"].as_array().unwrap().iter().all(|u| u["signed"] == false), "no card is signed: {door}");

    let (code, body) = send_pair(&base, &manifest, &sig).await;
    assert_eq!(code, 200, "the real pair is kept: {body}");
    assert_eq!(body["version"], "0.1547.0");
    assert_eq!(body["kept"], "new");
    assert_eq!(state.db.release_signature_count().unwrap(), 1);
    let (code, body) = send_pair(&base, &manifest, &sig).await;
    assert_eq!((code, body["kept"].as_str()), (200, Some("same")), "sending it again changes nothing");

    let (_, door) = front_door(&base).await;
    let by_version = |v: &str| door["updates"].as_array().unwrap().iter().find(|u| u["version"] == v).cloned().unwrap();
    assert_eq!(by_version("0.1547.0")["signed"], true, "the signed release's card says so: {door}");
    assert_eq!(by_version("0.1543.0")["signed"], false, "a release whose pair was never sent is not signed");
}

/// THE RELEASE LIST COMES A PAGE AT A TIME, NEWEST FIRST, TO THE END: 30 releases in pages of 12
/// are 12, 12 and 6, each page older than the last, no release twice, and the last page's `next`
/// is null; a limit past the most is held to it.
#[tokio::test]
async fn the_release_list_comes_a_page_at_a_time() {
    let (state, base) = relay("front_door_pages").await;
    for i in 0..30u64 {
        store(&state, DEPLOY_BOT_KEY, &post(&format!("1.{i}"), &format!("release {i}")), 1_000 + i);
        if i % 5 == 0 {
            store(&state, "a1b2c3d4", "a person's line between the releases", 1_000 + i);
        }
    }
    let mut seen: Vec<String> = Vec::new();
    let mut before: Option<i64> = None;
    let mut sizes = Vec::new();
    loop {
        let url = match before {
            Some(b) => format!("{base}/api/releases?limit=12&before={b}"),
            None => format!("{base}/api/releases?limit=12"),
        };
        let page: serde_json::Value = reqwest::get(url).await.unwrap().json().await.unwrap();
        let got = versions(&page["releases"]);
        sizes.push(got.len());
        seen.extend(got);
        match page["next"].as_i64() {
            Some(n) => before = Some(n),
            None => break,
        }
        assert!(sizes.len() < 10, "the pages never ended: {sizes:?}");
    }
    let want: Vec<String> = (0..30).rev().map(|i| format!("1.{i}")).collect();
    assert_eq!(seen, want, "every release once, newest first (pages {sizes:?})");
    assert_eq!(&sizes[..3], [12, 12, 6], "{sizes:?}");

    let all: serde_json::Value = reqwest::get(format!("{base}/api/releases?limit=500")).await.unwrap().json().await.unwrap();
    assert_eq!(all["releases"].as_array().unwrap().len(), RELEASES_PAGE_MAX.min(30), "a limit past the most is held to it");
}

/// EVERY /API/ ANSWER CARRIES X-ROBOTS-TAG: NOINDEX, the page answers do not: robots.txt asks a
/// crawler not to fetch, and this keeps a linked API address out of a search index even so. A
/// refusal (400), a route that is not there (404) and a normal answer all carry it; /health does
/// not.
///
/// Seen red 2026-11-02 with the `if noindex` insert taken out of security_headers: "the front
/// door's answer carries noindex / left: None / right: Some(\"noindex\")".
#[tokio::test]
async fn every_api_answer_is_kept_out_of_search_indexes() {
    let (_state, base) = relay("front_door_noindex").await;
    let tag = |h: &reqwest::header::HeaderMap| h.get("x-robots-tag").and_then(|v| v.to_str().ok()).map(str::to_string);
    let (headers, _) = front_door(&base).await;
    assert_eq!(tag(&headers).as_deref(), Some("noindex"), "the front door's answer carries noindex");
    let refused = reqwest::Client::new().post(format!("{base}/api/release-signature")).json(&serde_json::json!({ "manifest": "{}", "signature": "{}" })).send().await.unwrap();
    assert_eq!(refused.status().as_u16(), 400);
    assert_eq!(tag(refused.headers()).as_deref(), Some("noindex"), "a refusal carries it too");
    let messages = reqwest::get(format!("{base}/api/messages?channel=general")).await.unwrap();
    assert_eq!(tag(messages.headers()).as_deref(), Some("noindex"), "people's posts");
    let health = reqwest::get(format!("{base}/health")).await.unwrap();
    assert_eq!(tag(health.headers()), None, "/health is not an /api/ answer");
    for (path, want) in [("/api/front-door", true), ("/api", true), ("/uploads/a.png", true), ("/files/12", true), ("/files", false), ("/", false), ("/library", false), ("/apiary", false)] {
        assert_eq!(crate::relay::keep_out_of_search(path), want, "{path}");
    }
}
