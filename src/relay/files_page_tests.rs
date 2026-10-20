//! One Media item's own page and its picture for social sites (Media round 5, 2026-10-12; lane 1B
//! of docs/design/media-round-5-plan.md, its 3.6). A child of relay/files_page.rs. Each test names
//! the break it was seen failing against ("Seen red").
//!
//! No socket and no relay process: the routes are called as functions on a relay state over a
//! temporary database, with the repository's own web/pages/files.html as the page, and nothing is
//! written outside a temporary folder.

use super::*;
use crate::relay::files::{publish_core, PublishBody};
use crate::relay::storage::files::{MEDIA_SHELF, ROLE_COVER};
use crate::relay::storage::Storage;

/// The site the tests' pages are on.
const SITE: &str = "https://united-humanity.us";

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

/// What one test item is published with.
#[derive(Default)]
struct Pub<'a> {
    file: &'a str,
    original: &'a str,
    bytes: &'a [u8],
    title: &'a str,
    description: &'a str,
    visibility: &'a str,
    labels: &'a [&'a str],
}

/// Ann (a member) uploads `p.bytes` for Media as `p.file` and publishes it.
fn published(state: &RelayState, d: &Dirs, p: Pub) -> FilesItem {
    if !state.db.is_member("ann") {
        state.db.join_server("ann", "Ann").unwrap();
    }
    std::fs::write(d.waiting.join(p.file), p.bytes).unwrap();
    state.db.record_upload("ann", p.file, 100, MEDIA_SHELF, p.original, p.bytes.len() as i64).unwrap();
    let s = state.db.get_server_settings().unwrap();
    let body = PublishBody {
        key: "ann".into(),
        filename: p.file.into(),
        title: p.title.into(),
        description: p.description.into(),
        labels: p.labels.iter().map(|l| l.to_string()).collect(),
        visibility: Some(if p.visibility.is_empty() { "public" } else { p.visibility }.into()),
        ..Default::default()
    };
    let answer = publish_core(&state.db, &s, labels(), d.folders(), &body).expect("test setup: published").0;
    state.db.media_item(answer["item"]["id"].as_i64().unwrap()).unwrap().unwrap()
}

/// A picture (its name says so) of `n` bytes.
fn picture<'a>(file: &'a str, bytes: &'a [u8]) -> Pub<'a> {
    Pub { file, original: file, bytes, ..Default::default() }
}

/// The repository's own web/pages/files.html, the page the relay serves.
fn files_html() -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/pages/files.html")).expect("web/pages/files.html")
}

/// The page for `id` on [`SITE`]: status, headers and text.
async fn page_of(state: &RelayState, d: &Dirs, id: &str) -> (u16, HeaderMap, String) {
    let res = page_core(state, d.folders(), Some(&files_html()), id, SITE);
    let code = res.status().as_u16();
    let head = res.headers().clone();
    (code, head, String::from_utf8(axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap())
}

async fn preview_of(state: &RelayState, d: &Dirs, id: i64) -> (u16, HeaderMap, Vec<u8>) {
    let res = preview_core(state, d.folders(), id).await;
    let code = res.status().as_u16();
    let head = res.headers().clone();
    (code, head, axum::body::to_bytes(res.into_body(), 8 << 20).await.unwrap().to_vec())
}

fn set_media_visibility(state: &RelayState, to: &str) {
    let mut s = state.db.get_server_settings().unwrap();
    s.media_visibility = to.into();
    state.db.set_server_settings(&s, "admin").unwrap();
}

/// A PUBLIC ITEM'S PAGE CARRIES ITS OPEN GRAPH TAGS, ESCAPED: files.html with the item's title as
/// its `<title>` and, at its marker line, the canonical address, the site's name, the title, the
/// description (its line breaks made spaces), the page's address and its picture, every value
/// HTML-escaped, so a title of `"><script>` is text and never a tag. The rest of files.html is
/// unchanged. With no title the name it was uploaded under is the title; with no description, who
/// shared it; a long description is cut at 200 characters.
///
/// Seen red 2026-10-12 with the escaping removed (`escape` giving its text back unchanged):
/// "the title is text, never a tag".
#[tokio::test]
async fn a_public_items_page_carries_its_open_graph_tags_escaped() {
    let state = relay("mp_tags");
    let d = dirs("mp_tags");
    let mut s = state.db.get_server_settings().unwrap();
    s.server_name = "Ann's <Place>".into();
    state.db.set_server_settings(&s, "admin").unwrap();
    let title = "\"><script>alert(1)</script>";
    let item = published(&state, &d, Pub { title, description: "First line\nSecond & more", ..picture("1_dusk.png", b"PNG") });

    let (code, head, page) = page_of(&state, &d, &item.id.to_string()).await;
    assert_eq!(code, 200);
    assert_eq!(head.get(header::CONTENT_TYPE).unwrap(), "text/html; charset=utf-8");
    assert_eq!(head.get(header::CACHE_CONTROL).unwrap(), "no-cache");
    assert!(!page.contains("<script>alert"), "the title is text, never a tag:\n{page}");
    let shown = "&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;";
    let id = item.id;
    for tag in [
        format!("<title>{shown}, Files on united-humanity.us</title>"),
        format!("<link rel=\"canonical\" href=\"https://united-humanity.us/files/{id}\">"),
        "<meta property=\"og:type\" content=\"website\">".to_string(),
        "<meta property=\"og:site_name\" content=\"Ann&#39;s &lt;Place&gt;\">".to_string(),
        format!("<meta property=\"og:title\" content=\"{shown}\">"),
        "<meta property=\"og:description\" content=\"First line Second &amp; more\">".to_string(),
        format!("<meta property=\"og:url\" content=\"https://united-humanity.us/files/{id}\">"),
        format!("<meta property=\"og:image\" content=\"https://united-humanity.us/api/files/preview/{id}\">"),
        "<meta name=\"twitter:card\" content=\"summary_large_image\">".to_string(),
    ] {
        assert!(page.contains(&tag), "the page carries {tag}\n{page}");
    }
    assert_eq!(page.matches("<title>").count(), 1, "one title, the item's");
    assert!(!page.contains(OG_MARKER), "the tags are written at the marker");
    let html = files_html();
    let tail = &html[html.find("</head>").unwrap()..];
    assert!(page.ends_with(tail), "the rest of files.html is as it was");

    let plain = published(&state, &d, Pub { original: "Evening walk.png", ..picture("2_walk.png", b"PNG") });
    let (_, _, page) = page_of(&state, &d, &plain.id.to_string()).await;
    assert!(page.contains("<meta property=\"og:title\" content=\"Evening walk.png\">"), "no title: the name it was uploaded under\n{page}");
    assert!(page.contains("<meta property=\"og:description\" content=\"Shared by Ann on united-humanity.us.\">"), "no description: who shared it");

    let long = "word ".repeat(80);
    let wordy = published(&state, &d, Pub { title: "Wordy", description: &long, ..picture("3_wordy.png", b"PNG") });
    let (_, _, page) = page_of(&state, &d, &wordy.id.to_string()).await;
    let at = page.find("og:description\" content=\"").unwrap() + "og:description\" content=\"".len();
    let said = &page[at..at + page[at..].find('"').unwrap()];
    assert_eq!(said.chars().count(), DESCRIPTION_MAX_CHARS, "{said}");
    assert!(said.ends_with("..."), "cut, and says so: {said}");
}

/// A MEMBERS-ONLY ITEM, A MEMBERS-ONLY PAGE AND A MISSING ITEM ANSWER THE SAME BYTES: the page of a
/// members-only item, of a number that is no item, of something that is no number at all, and (once
/// the server's Files page is members only) of a public item, are one bare page, byte for byte and
/// header for header, still 200: its title names only the server, it asks not to be indexed, and
/// nothing of the item (its title, its description, its picture) is in it.
///
/// Seen red 2026-10-12 with the members-only case writing its title (`shown_item` giving every item
/// it found, `Some(item)`, without asking `shown_to_everyone`): "a members-only item's page is the
/// bare page / left: (200, {\"content-type\": \"text/html; charset=utf-8\", ...}, ...) / right: (200,
/// ...)", the two pages' texts differing.
#[tokio::test]
async fn a_members_only_item_a_members_only_page_and_a_missing_item_answer_the_same_bytes() {
    let state = relay("mp_hidden");
    let d = dirs("mp_hidden");
    let open = published(&state, &d, Pub { title: "Public dusk", ..picture("1_open.png", b"PNG") });
    let secret = published(&state, &d, Pub { title: "Secret plans", description: "Meet at dawn", visibility: "members", ..picture("2_secret.png", b"PNG") });

    let bare = page_of(&state, &d, "999999").await;
    assert_eq!(bare.0, 200, "still a page, so a member's browser loads it and signs in");
    assert_eq!(page_of(&state, &d, &secret.id.to_string()).await, bare, "a members-only item's page is the bare page");
    for not_a_number in ["0", "abc", "12a", "-1", "", "99999999999999999999", "1%20"] {
        assert_eq!(page_of(&state, &d, not_a_number).await, bare, "{not_a_number:?}");
    }
    let (_, _, text) = &bare;
    assert!(text.contains("<title>Files on united-humanity.us, HumanityOS</title>"), "{text}");
    assert!(text.contains("<meta property=\"og:title\" content=\"Files on united-humanity.us\">"));
    assert!(text.contains("<meta name=\"robots\" content=\"noindex\">"));
    for nothing in ["Secret", "dawn", "og:image", "og:description", "og:url", "canonical", "twitter:card"] {
        assert!(!text.contains(nothing), "the bare page says nothing of the item: {nothing}");
    }

    assert_ne!(page_of(&state, &d, &open.id.to_string()).await, bare, "a public item's page is its own");
    set_media_visibility(&state, "members");
    assert_eq!(page_of(&state, &d, &open.id.to_string()).await, bare, "an item on a members-only page is the bare page");
}

/// A LABELLED ITEM HAS NO PICTURE (decision 3): a public picture labelled spoiler and flashing
/// lights gets its title and a description that starts with its labels, by the names people see,
/// but no `og:image`, a plain `summary` card, and no preview picture; the same picture unlabelled
/// has all three.
///
/// Seen red 2026-10-12 with the label check removed from `preview_file`: "a labelled item's page
/// shows no picture".
#[tokio::test]
async fn a_labelled_item_has_no_picture() {
    let state = relay("mp_label");
    let d = dirs("mp_label");
    let labelled = published(&state, &d, Pub { title: "Strobe", description: "A light show", labels: &["flashing", "spoiler"], ..picture("1_strobe.png", b"PNG") });
    let plain = published(&state, &d, Pub { title: "Calm", ..picture("2_calm.png", b"PNG") });

    let (_, _, page) = page_of(&state, &d, &labelled.id.to_string()).await;
    assert!(!page.contains("og:image"), "a labelled item's page shows no picture\n{page}");
    assert!(page.contains("<meta name=\"twitter:card\" content=\"summary\">"));
    assert!(page.contains("<meta property=\"og:description\" content=\"Labelled: Spoiler, Flashing lights. A light show\">"), "{page}");
    assert!(page.contains("<meta property=\"og:title\" content=\"Strobe\">"), "its words are shown");
    assert_eq!(preview_of(&state, &d, labelled.id).await.0, 404, "and its picture is not given");

    let (_, _, page) = page_of(&state, &d, &plain.id.to_string()).await;
    assert!(page.contains("og:image"), "unlabelled, it has one");
    assert_eq!(preview_of(&state, &d, plain.id).await.0, 200);
}

/// A FORGED HOST NEVER REACHES THE TAGS: the site an item's page names is one of the server's own
/// configured addresses; the request's Host is used only when it is one of them (in any case, and
/// `www.` before one counts as it); any other Host, this machine's own addresses and the desktop
/// app's (which every relay allows) included, gets the first configured https address; a server
/// configured with http alone names that. Through the route itself, a forged Host appears nowhere
/// in the page, and the page is the website's own files.html with the item's tags in it.
///
/// Seen red 2026-10-12 with the Host used raw in `page` (`format!("https://{host}")`): "a forged
/// Host appears nowhere in the page", the page carrying `<link rel="canonical"
/// href="https://evil.example/files/1">`.
#[tokio::test]
async fn a_forged_host_never_reaches_the_tags() {
    let configured = crate::relay::parse_allowed_origins(Some("https://united-humanity.us, https://chat.united-humanity.us"), 3210);
    for (host, site) in [
        (Some("united-humanity.us"), "https://united-humanity.us"),
        (Some("chat.united-humanity.us"), "https://chat.united-humanity.us"),
        (Some("CHAT.United-Humanity.us"), "https://chat.united-humanity.us"),
        (Some("www.united-humanity.us"), "https://united-humanity.us"),
        (Some("evil.example"), "https://united-humanity.us"),
        (Some("united-humanity.us.evil.example"), "https://united-humanity.us"),
        (Some("united-humanity.us/evil"), "https://united-humanity.us"),
        (Some("tauri.localhost"), "https://united-humanity.us"),
        (Some("localhost:3210"), "https://united-humanity.us"),
        (Some("127.0.0.1:3210"), "https://united-humanity.us"),
        (None, "https://united-humanity.us"),
    ] {
        assert_eq!(site_origin(host, &configured), site, "Host {host:?}");
    }
    let home = crate::relay::parse_allowed_origins(Some("http://192.168.1.5:3210"), 3210);
    assert_eq!(site_origin(Some("evil.example"), &home), "http://192.168.1.5:3210", "http alone: that one");
    assert_eq!(site_origin(Some("192.168.1.5:3210"), &home), "http://192.168.1.5:3210");

    // Through the route: its own configured addresses (none set in a test: the defaults), its own
    // files.html (web/pages under the test's working folder), and a Host nobody configured.
    let state = relay("mp_host");
    let d = dirs("mp_host");
    let item = published(&state, &d, Pub { title: "Dusk", ..picture("1_dusk.png", b"PNG") });
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "evil.example".parse().unwrap());
    let res = page(State(state.clone()), Path(item.id.to_string()), headers).await;
    assert_eq!(res.status(), StatusCode::OK);
    let text = String::from_utf8(axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap().to_vec()).unwrap();
    assert!(!text.contains("evil.example"), "a forged Host appears nowhere in the page\n{text}");
    // The route serves the website's real Files page, which reads the item's number from the
    // address and opens it (lane 1G), with the item's tags added.
    for part in ["<div id=\"files-root\"></div>", "<script src=\"/pages/files-app.js", "<title>Dusk, Files on "] {
        assert!(text.contains(part), "the real Files page, with the item's tags: {part}");
    }
    let at = text.find("og:url\" content=\"").expect("an og:url") + "og:url\" content=\"".len();
    let url = &text[at..at + text[at..].find('"').unwrap()];
    let ours = crate::relay::allowed_origins();
    assert!(ours.iter().any(|o| url == format!("{o}/files/{}", item.id)), "{url} is on one of this server's addresses: {ours:?}");
}

/// THE PREVIEW SERVES ONLY PUBLIC, UNLABELLED PICTURES, INLINE, UNDER `default-src 'none'`: a public
/// picture is given whole, as its own picture type, inline, with `Content-Security-Policy:
/// default-src 'none'` and five minutes in shared caches; a model's cover is its preview; a model
/// with no cover, a labelled picture, a members-only picture, a picture over 5,000,000 bytes and a
/// missing item have none; and once the server's Files page is members only, neither does the
/// public picture. Every refusal is the same 404 words.
///
/// Seen red 2026-10-12 with the visibility check removed from `preview_core` (`Ok(Some(item)) =>
/// item`): "the members-only page's picture is not given / left: 200 / right: 404".
#[tokio::test]
async fn the_preview_serves_only_public_unlabelled_pictures_inline_under_default_src_none() {
    let state = relay("mp_preview");
    let d = dirs("mp_preview");
    let open = published(&state, &d, picture("1_dusk.png", b"\x89PNG dusk"));
    let model = published(&state, &d, Pub { file: "2_staff.glb", original: "staff.glb", bytes: b"glTF", ..Default::default() });
    std::fs::write(d.waiting.join("3_cover.jpg"), b"JPEG cover").unwrap();
    state.db.record_upload("ann", "3_cover.jpg", 100, MEDIA_SHELF, "cover.jpg", 10).unwrap();
    let f = d.folders();
    state.db.attach_media_file("ann", model.id, "3_cover.jpg", ROLE_COVER, 0, |members| f.place("3_cover.jpg", members)).unwrap();
    let bare_model = published(&state, &d, Pub { file: "4_plain.glb", original: "plain.glb", bytes: b"glTF", ..Default::default() });
    let labelled = published(&state, &d, Pub { labels: &["graphic"], ..picture("5_cut.png", b"PNG") });
    let secret = published(&state, &d, Pub { visibility: "members", ..picture("6_secret.png", b"PNG") });
    let at_limit = vec![7u8; PREVIEW_MAX_BYTES as usize];
    let just_over = vec![7u8; PREVIEW_MAX_BYTES as usize + 1];
    let big = published(&state, &d, picture("7_big.webp", &at_limit));
    let too_big = published(&state, &d, picture("8_huge.webp", &just_over));

    let (code, head, body) = preview_of(&state, &d, open.id).await;
    assert_eq!((code, body), (200, b"\x89PNG dusk".to_vec()), "a public picture is given");
    assert_eq!(head.get(header::CONTENT_TYPE).unwrap(), "image/png");
    assert_eq!(head.get(header::CONTENT_DISPOSITION).unwrap(), "inline");
    assert_eq!(head.get(header::CONTENT_SECURITY_POLICY).unwrap(), "default-src 'none'");
    assert_eq!(head.get(header::CACHE_CONTROL).unwrap(), "public, max-age=300");
    let (code, head, body) = preview_of(&state, &d, model.id).await;
    assert_eq!((code, body), (200, b"JPEG cover".to_vec()), "a model's cover is its preview");
    assert_eq!(head.get(header::CONTENT_TYPE).unwrap(), "image/jpeg");
    assert_eq!(preview_of(&state, &d, big.id).await.0, 200, "5,000,000 bytes is still given");

    let none = (404u16, NO_PREVIEW.as_bytes().to_vec());
    for (id, what) in [
        (bare_model.id, "a model with no cover"),
        (labelled.id, "a labelled picture"),
        (secret.id, "a members-only picture"),
        (too_big.id, "a picture over 5,000,000 bytes"),
        (999_999, "a missing item"),
    ] {
        let (code, _, body) = preview_of(&state, &d, id).await;
        assert_eq!((code, body), none, "{what} has no preview");
    }
    set_media_visibility(&state, "members");
    let (code, _, body) = preview_of(&state, &d, open.id).await;
    assert_eq!(code, 404, "the members-only page's picture is not given");
    assert_eq!(body, none.1);
}

/// THE PAGE KEEPS `blob:` PICTURES IN ITS CSP: an item's page, and the bare page, carry the relay's
/// Content-Security-Policy with `blob:` added to `img-src` (the Files page shows members-only
/// pictures through `blob:` addresses), and the relay's header middleware keeps it rather than
/// putting its own, which has no `blob:`, in its place.
///
/// Seen red 2026-10-12 with the route's CSP left to the middleware (the `insert` taken out of
/// `page_core`): "the page's own policy is kept (1) / left: Some(\"default-src 'self'; ... img-src
/// 'self' data: https:; ...\") / right: Some(\"... img-src 'self' data: blob: https:; ...\")".
#[tokio::test]
async fn the_page_keeps_blob_pictures_in_its_csp() {
    let state = relay("mp_csp");
    let d = dirs("mp_csp");
    let item = published(&state, &d, picture("1_dusk.png", b"PNG"));
    assert!(page_csp().contains("img-src 'self' data: blob: https:;"), "{}", page_csp());
    assert_ne!(page_csp(), crate::relay::csp_value(), "it is not the relay's usual policy");
    let connect = |csp: &str| csp.split(';').map(str::trim).find(|d| d.starts_with("connect-src")).map(str::to_string);
    // The relay's connect-src, from the same configured addresses, plus the shell's check for a
    // newer release (seen red 2026-10-12 on the live /files/2: the browser refused the fetch).
    let want = connect(crate::relay::csp_value()).map(|c| format!("{c} {}", super::SHELL_VERSION_CHECK));
    assert_eq!(connect(page_csp()), want, "the relay's connect-src and the shell's version check");
    let shell = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/shared/shell.js")).unwrap();
    assert!(shell.contains(&format!("fetch('{}/", super::SHELL_VERSION_CHECK)), "the shell still asks that address");
    assert_eq!(super::with_connect_source("connect-src 'self';", "https://a.example"), "connect-src 'self' https://a.example;");
    assert_eq!(super::with_connect_source("connect-src 'self' https://a.example;", "https://a.example"), "connect-src 'self' https://a.example;", "never twice");

    for id in [item.id.to_string(), "999999".to_string()] {
        let mut res = page_core(&state, d.folders(), Some(&files_html()), &id, SITE);
        crate::relay::set_security_headers(res.headers_mut(), false);
        let csp: Vec<_> = res.headers().get_all(header::CONTENT_SECURITY_POLICY).iter().map(|v| v.to_str().unwrap().to_string()).collect();
        assert_eq!(csp.first().map(String::as_str), Some(page_csp()), "the page's own policy is kept ({id})");
        assert_eq!(csp.len(), 1, "and it is the only one");
    }

    assert_eq!(with_blob_pictures("default-src 'self'; img-src 'self' data: https:;"), "default-src 'self'; img-src 'self' data: blob: https:;");
    assert_eq!(with_blob_pictures("img-src 'self' blob:;"), "img-src 'self' blob:;", "never twice");
    assert_eq!(with_blob_pictures("img-src 'self';"), "img-src 'self' blob:;");
}

/// NO MEDIA.HTML SAYS SO IN WORDS: a server that cannot find files.html answers 503 with words its
/// operator can act on (where it looked, and that WEB_ROOT sets it), never a bare error. The
/// template is looked for in order, the first that can be read is the one used, the last place is
/// web/pages/files.html under the working folder, and the repository's own files.html has its
/// marker line once.
///
/// Seen red 2026-10-12 with a 500 and no words in place of the 503 (`StatusCode::
/// INTERNAL_SERVER_ERROR, ""`): "left: (500, \"\") / right: (503, \"This server cannot find its
/// Files page ...\")".
#[tokio::test]
async fn no_media_html_says_so_in_words() {
    let state = relay("mp_none");
    let d = dirs("mp_none");
    let missing = d.uploads.join("no-such-folder").join("files.html");
    assert_eq!(read_template(&[missing.clone(), d.members.join("files.html")]).await, None);
    let res = page_core(&state, d.folders(), None, "1", SITE);
    let code = res.status().as_u16();
    let words = String::from_utf8(axum::body::to_bytes(res.into_body(), 1 << 10).await.unwrap().to_vec()).unwrap();
    assert_eq!((code, words.as_str()), (503, NO_FILES_HTML));
    assert!(NO_FILES_HTML.contains("files.html") && NO_FILES_HTML.contains("WEB_ROOT"));

    let found = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("web/pages/files.html");
    assert_eq!(read_template(&[missing, found]).await, Some(files_html()), "the first that can be read");
    assert_eq!(template_paths().last().unwrap(), &PathBuf::from("web").join("pages").join("files.html"));
    assert_eq!(files_html().matches(OG_MARKER).count(), 1, "files.html carries its marker line once");
    assert_eq!(files_html().matches("<title>").count(), 1);
}

/// NGINX SENDS ITEM PAGES TO THE RELAY: the repository's nginx config and the self-hosting guide
/// both send `^/files/[0-9]+$` to the relay (a regular-expression location, which wins over the
/// static `location /`, so `/files` itself stays the static page), rate-limited and answering a
/// refused rate with 429, and passing `X-Real-IP` from the connection, which the counts tell people
/// apart by; every `/api/` location passes `X-Real-IP` too. Neither file proxies `/files` itself,
/// and each has one location for the pattern (the relay's replaces the static stand-in).
///
/// Seen red 2026-10-12 with the location missing from the guide: "docs/admin/SELF-HOSTING.md:
/// location ~ ^/files/[0-9]+$ {"; and with the static stand-in left in the guide beside the
/// relay's block: "docs/admin/SELF-HOSTING.md: one /files/<number> location, the relay's / left: 2
/// / right: 1".
#[test]
fn nginx_sends_item_pages_to_the_relay() {
    for (file, zone) in [("scripts/nginx/humanity.conf", "api_read_limit"), ("docs/admin/SELF-HOSTING.md", "general")] {
        let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file)).unwrap().replace("\r\n", "\n");
        let block_at = |at: usize| &text[at..at + text[at..].find("\n    }").unwrap()];
        let location = "location ~ ^/files/[0-9]+$ {";
        let at = text.find(location).unwrap_or_else(|| panic!("{file}: {location}"));
        let block = block_at(at);
        for line in [
            "proxy_pass http://127.0.0.1:3210;",
            "proxy_set_header Host $host;",
            "proxy_set_header X-Real-IP $remote_addr;",
            "limit_req_status 429;",
            &format!("limit_req zone={zone} "),
        ] {
            assert!(block.contains(line), "{file}: the item pages' location has {line}");
        }
        assert!(!text.contains("location /files") && !text.contains("location = /files"), "{file}: /files itself stays static");
        // One location for the pattern: the relay's replaces the static stand-in (`try_files
        // /files.html`) the live server carried while this was built, since nginx would use
        // whichever came first. Comment lines are not locations.
        let locations = text.lines().filter(|l| l.trim_start().starts_with("location ~ ^/files/")).count();
        assert_eq!(locations, 1, "{file}: one /files/<number> location, the relay's");
        let mut apis = 0;
        for (at, _) in text.match_indices("location /api/ {") {
            apis += 1;
            assert!(block_at(at).contains("proxy_set_header X-Real-IP $remote_addr;"), "{file}: an /api/ location passes X-Real-IP");
        }
        assert!(apis > 0, "{file}: has an /api/ location");
    }
}
