//! Tests of the Files page's requests and answers (api.rs). Each was seen to fail once before it
//! passed; what it saw red is in its comment.

use super::*;

/// A signer from a fixed seed, with the public key the relay would hold for it.
fn signer() -> Signer {
    let seed = [7u8; 32];
    let key = crate::net::identity::derive_pq_identity(&seed).expect("derive").dilithium_hex;
    Signer::new(&key, &seed)
}

fn body(req: &Request) -> serde_json::Value {
    serde_json::from_str(req.body.as_deref().expect("a body")).expect("the body is JSON")
}

/// Whether the relay would accept `req`'s signature over `purpose` (its own check,
/// handlers/broadcast.rs `verify_dilithium_signature`, the one `check_signed` runs).
fn relay_accepts(req: &Request, purpose: &str) -> bool {
    let b = body(req);
    crate::relay::handlers::broadcast::verify_dilithium_signature(
        b["key"].as_str().unwrap_or(""),
        purpose,
        b["timestamp"].as_u64().unwrap_or(0),
        b["sig"].as_str().unwrap_or(""),
    )
}

fn item_ref(id: i64, filename: &str) -> ItemRef {
    ItemRef { id, filename: filename.into() }
}

/// EVERY SIGNED REQUEST NAMES ITS TARGET, AND THE RELAY ACCEPTS IT: each write signs the purpose
/// the relay rebuilds from the same body (relay/files.rs, the review's F5), using the relay's own
/// functions and body types where it has them, and its signature verifies there; a signature over
/// a purpose without the target does not. Every action on an item also names the item's file
/// (`filename`, the relay's W5), which the relay's own body types read back.
///
/// Seen red 2026-10-11 with `pick_purpose` signing plain "media_pick": "the pick names its item"
/// (left: Some("media_pick") / right: Some("media_pick:42")), and with `categories_request`
/// hashing the list with a space after each comma but sending it without: "the relay hashes the
/// categories as sent" (two different BLAKE3 purposes). Seen red again 2026-10-11 (round 2) as
/// the relay's W5 left it: a removal by number sent without `filename` panicked at "the relay
/// reads the removal" with the relay's own words, "Name the item's file too (`filename`), so a
/// page that is out of date never acts on another item."; and with `keep_request` leaving the
/// file out: "the keep names the item's file" (left: "" / right: "1700_photo.png"). Media round 5
/// (2026-10-12) changed it: the publish purpose carries the licence and the game tick
/// (`media_publish:<file>:<licence>:<0|1>`, the relay's `publish_purpose`), and the feature's where
/// it goes (`media_feature:<id>:<main|category|none>`).
#[test]
fn every_signed_request_names_its_target_and_the_relay_accepts_it() {
    let s = signer();

    let publish = publish_request(&s, &PublishFields { filename: "/uploads/1700_photo.png".into(), visibility: "public".into(), category_id: 1, ..Default::default() });
    let relay_body: crate::relay::files::PublishBody = serde_json::from_str(publish.body.as_deref().unwrap()).unwrap();
    let relay_purpose = crate::relay::files::publish_purpose(crate::relay::files::stored_name(&relay_body.filename).unwrap(), relay_body.licence(), relay_body.project_use);
    assert_eq!(publish.purpose.as_deref(), Some(relay_purpose.as_str()), "the publish names its file, licence and game tick");
    assert_eq!(relay_purpose, "media_publish:1700_photo.png:all_rights_reserved:0");
    assert!(relay_accepts(&publish, &relay_purpose));
    assert!(!relay_accepts(&publish, "media_publish:1700_photo.png"), "a signature names one file and its licence, no other");
    let licensed = publish_request(&s, &PublishFields { filename: "1700_photo.png".into(), licence: "cc0_1".into(), project_use: true, ..Default::default() });
    assert_eq!(licensed.purpose.as_deref(), Some("media_publish:1700_photo.png:cc0_1:1"));
    assert_eq!((body(&licensed)["licence"].as_str(), body(&licensed)["project_use"].as_bool()), (Some("cc0_1"), Some(true)));

    for target in [RemoveTarget::Item(item_ref(42, "1700_photo.png")), RemoveTarget::Upload("/uploads/1700_clip.mp4".into())] {
        let req = remove_request(&s, &target);
        let parsed: crate::relay::files::RemoveBody = serde_json::from_str(req.body.as_deref().unwrap()).unwrap();
        let (_, relay_purpose) = parsed.target().unwrap_or_else(|e| panic!("the relay reads the removal: {}", e.1));
        assert_eq!(req.purpose.as_deref(), Some(relay_purpose.as_str()), "the removal names its target");
        assert!(relay_accepts(&req, &relay_purpose));
    }
    let by_number = remove_request(&s, &RemoveTarget::Item(item_ref(42, "/uploads/1700_photo.png")));
    assert_eq!(body(&by_number)["filename"], "1700_photo.png", "a removal by number names the file, bare");

    let photo = item_ref(42, "1700_photo.png");
    for on in [true, false] {
        let req = pick_request(&s, &photo, on);
        assert_eq!(req.purpose.as_deref(), Some("media_pick:42"), "the pick names its item");
        assert_eq!(req.path, if on { "/api/files/pick" } else { "/api/files/unpick" });
        let relay_body: crate::relay::files::ItemBody = serde_json::from_str(req.body.as_deref().unwrap()).unwrap();
        assert_eq!((relay_body.id, relay_body.filename.as_str()), (42, "1700_photo.png"), "the pick names the item's file");
        assert!(relay_accepts(&req, "media_pick:42"));
    }

    let feature = feature_request(&s, &item_ref(9, "9_a.png"), Featured::None);
    assert_eq!(feature.purpose.as_deref(), Some("media_feature:9:none"));
    let relay_body: crate::relay::files::FeatureBody = serde_json::from_str(feature.body.as_deref().unwrap()).unwrap();
    assert_eq!((relay_body.featured.as_str(), relay_body.filename.as_str()), ("none", "9_a.png"), "the feature names the item's file");
    assert!(relay_accepts(&feature, "media_feature:9:none"));

    for kept in [true, false] {
        let req = keep_request(&s, &photo, kept);
        let relay_purpose = crate::relay::files::keep_purpose(42, kept);
        assert_eq!(req.purpose.as_deref(), Some(relay_purpose.as_str()), "the keep names its item and its answer");
        let relay_body: crate::relay::files::KeepBody = serde_json::from_str(req.body.as_deref().unwrap()).unwrap();
        assert_eq!(relay_body.filename, "1700_photo.png", "the keep names the item's file");
        assert_eq!(relay_body.kept, kept);
        assert_eq!(req.path, "/api/files/keep");
        assert!(relay_accepts(&req, &relay_purpose));
    }

    for accept in [true, false] {
        let req = decide_request(&s, 5, accept);
        let relay_purpose = crate::relay::files::decide_purpose(5, accept);
        assert_eq!(req.purpose.as_deref(), Some(relay_purpose.as_str()), "the decision names its suggestion and its answer");
        assert!(relay_accepts(&req, &relay_purpose));
    }

    let cats = categories_request(
        &s,
        &[
            CategoryEdit { id: Some(1), name: "General".into(), description: String::new() },
            CategoryEdit { id: None, name: "Fire flow arts".into(), description: "Staff, poi, darts".into() },
        ],
    );
    let (parsed, relay_purpose) = crate::relay::files::categories_request(cats.body.as_deref().unwrap()).expect("the relay reads the list");
    assert_eq!(cats.purpose.as_deref(), Some(relay_purpose.as_str()), "the relay hashes the categories as sent");
    assert_eq!(parsed.categories.len(), 2);
    assert_eq!(parsed.categories[1].id, None, "a new category has no id");
    assert!(relay_accepts(&cats, &relay_purpose));

    // The reads and the suggestion keep their plain purposes.
    for (req, purpose) in [
        (suggest_request(&s, "Builds", "Ships and homes"), "media_suggest"),
        (suggestions_request(&s), "media_suggestions"),
        (usage_request(&s), "media_usage"),
        (mine_request(&s), "media_mine"),
        (list_request(&Filters::default(), None, PAGE, &s.key, Some(&s)), "media_list"),
    ] {
        assert_eq!(req.purpose.as_deref(), Some(purpose));
        assert!(relay_accepts(&req, purpose), "{purpose}: the relay accepts it");
    }
}

/// NO TWO SIGNATURES SHARE A TIMESTAMP: the relay refuses a (key, purpose, timestamp) it has seen
/// (its replay memory), so two reads in one millisecond would have the second refused.
///
/// Seen red 2026-10-11 with `next_timestamp` allowed to repeat the last value (`now.max(prev)`):
/// "two timestamps in a row differ (1791708446270 then 1791708446270)".
#[test]
fn no_two_signatures_share_a_timestamp() {
    let mut last = 0;
    for _ in 0..2000 {
        let t = next_timestamp();
        assert!(t > last, "two timestamps in a row differ ({last} then {t})");
        last = t;
    }
}

/// THE LIST ASKS THE RIGHT WAY: without a signer, the public GET with its filters in the address
/// (the search encoded); with one, the signed POST with them in its body (a signed address would
/// be about 10 KB). My picks, signed, is `POST /api/files/picks` with `of` (the relay's A3), so a
/// member's picks include members-only items; the featured strip asks with `featured`, and the
/// Reports page's Open with `id`.
///
/// Seen red 2026-10-11 with the search not encoded: "the search is encoded" (the address held
/// "fire staff" with its space). Seen red again 2026-10-11 (round 2) with My picks still the
/// public GET for a member: "a member's picks are the signed POST" (left: (Get,
/// "/api/files/picks/abc?limit=100") / right: (Post, "/api/files/picks")). Media round 5
/// (2026-10-12) changed the paging: the last answer's `next_cursor` goes back as `cursor` (it was
/// `before`, a number).
#[test]
fn the_list_asks_the_right_way() {
    let f = Filters { category: Some(3), kind: Some("video".into()), q: "fire staff".into(), ..Default::default() };
    let public = list_request(&f, Some("120.120"), 48, "me", None);
    assert_eq!(public.method, Method::Get);
    assert_eq!(public.path, "/api/files?limit=48&category=3&kind=video&q=fire+staff&cursor=120.120", "the search is encoded");
    assert!(public.purpose.is_none() && public.body.is_none());

    let s = signer();
    let signed = list_request(&f, Some("120.120"), 48, &s.key, Some(&s));
    assert_eq!((signed.method, signed.path.as_str()), (Method::Post, "/api/files/list"));
    let b = body(&signed);
    assert_eq!((b["category"].as_i64(), b["kind"].as_str(), b["q"].as_str(), b["cursor"].as_str()), (Some(3), Some("video"), Some("fire staff"), Some("120.120")));
    assert!(signed.path.len() < 100, "nothing signed rides in the address");

    let picks = list_request(&Filters { my_picks: true, ..Default::default() }, Some("9.9"), 48, "abc", Some(&s));
    assert_eq!((picks.method, picks.path.as_str()), (Method::Post, "/api/files/picks"), "a member's picks are the signed POST");
    let pb = body(&picks);
    assert_eq!((pb["of"].as_str(), pb["cursor"].as_str()), (Some("abc"), Some("9.9")), "whose picks, paged past the first page");
    let relay_params: crate::relay::files::ListParams = serde_json::from_value(pb).expect("the relay reads it");
    assert_eq!(relay_params.of.as_deref(), Some("abc"));
    let public_picks = list_request(&Filters { my_picks: true, ..Default::default() }, None, 100, "abc", None);
    assert_eq!((public_picks.method, public_picks.path.as_str()), (Method::Get, "/api/files/picks/abc?limit=100"), "the public view of them");

    let featured = list_request(&Filters { featured: true, ..Default::default() }, None, 48, &s.key, Some(&s));
    assert_eq!(body(&featured)["featured"].as_i64(), Some(1), "the strip asks for the featured items");
    assert!(list_request(&Filters { featured: true, ..Default::default() }, None, 48, "me", None).path.contains("featured=1"));
    let one = list_request(&Filters { id: Some(12), ..Default::default() }, None, 48, &s.key, Some(&s));
    assert_eq!(body(&one)["id"].as_i64(), Some(12), "one item by its number");
}

/// THE SERVER'S ANSWERS READ: an item with its category, labels, members-only address, and the
/// follow-ups' fields (its own file name, what Remove does, kept, picked by me, its stored name);
/// the list's paging and views; the usage's numbers with what is kept; a member's own use; the
/// suggestions.
///
/// Seen red 2026-10-11 with the category read from "category_id" (the relay sends
/// `category: {id, name}`): "the item's category" (left: (0, "Fire flow arts") / right: (2, "Fire
/// flow arts")). Seen red again 2026-10-11 (round 2) with `stored` read from the address alone:
/// "a members-only item's stored name comes from `filename`" (left: "" / right: "7_clip.webm").
/// Media round 5 (2026-10-12) changed it: `featured` is a word ("main" here) and the paging is
/// `next_cursor`.
#[test]
fn the_servers_answers_read() {
    let v = serde_json::json!({
        "items": [{"id": 7, "url": "/api/files/file/7", "filename": "7_clip.webm", "title": "", "description": "Spinning at dusk", "category": {"id": 2, "name": "Fire flow arts"},
                   "kind": "video", "labels": ["flashing"], "visibility": "members", "featured": "main", "size_bytes": 1048576, "views": 0,
                   "uploaded_at": 1, "published_at": 2, "uploader_key": "k1", "uploader_name": "Shaostoul", "picks": 3,
                   "original_name": "", "keeps_file": false, "kept": true, "picked_by_me": true},
                  {"id": 8, "url": "/uploads/8_a.png", "kind": "picture", "visibility": "public", "original_name": "Garden.png", "keeps_file": true}],
        "next_cursor": "7.7", "members_view": true, "media_visibility": "public", "members_only_for_you": false,
        "categories": [{"id": 1, "name": "General", "description": "", "position": 0}],
        "labels": [{"id": "flashing", "label": "Flashing lights", "desc": "Rapid flashes"}],
    });
    let a = parse_list(&v);
    let item = &a.items[0];
    assert_eq!((item.category_id, item.category_name.as_str()), (2, "Fire flow arts"), "the item's category");
    assert!(item.members_only() && item.featured == Featured::Main && item.upload_name().is_none());
    assert_eq!(item.display_title(), "Video", "an untitled members-only item is called by its kind");
    assert_eq!(item.stored, "7_clip.webm", "a members-only item's stored name comes from `filename`");
    assert_eq!(item.reference(), Ok(ItemRef { id: 7, filename: "7_clip.webm".into() }));
    assert!(item.kept && item.picked_by_me && !item.keeps_file);
    let public = &a.items[1];
    assert_eq!((public.stored.as_str(), public.display_title(), public.keeps_file), ("8_a.png", "Garden.png".to_string(), true), "a public item's from its address");
    let unnamed = Item { stored: String::new(), ..item.clone() };
    assert_eq!(unnamed.reference(), Err(NO_FILE_NAME.to_string()), "nothing is asked of an item whose file is not known");
    assert_eq!((a.next_cursor.as_deref(), a.members_view, a.labels[0].label.as_str()), (Some("7.7"), true, "Flashing lights"));

    let u = parse_usage(&serde_json::json!({"used_bytes": 10, "budget_bytes": 20, "per_person_bytes": 5, "items": 1, "kept_bytes": 4,
        "largest": [{"id": 1, "url": "/uploads/a.png"}], "oldest": [], "least_viewed": [],
        "waiting": [{"filename": "b.png", "original_name": "Beach.png", "uploader_key": "k", "uploader_name": "Ann", "size_bytes": 4, "uploaded_at": 1, "expires_at": 2}]}));
    assert_eq!((u.used_bytes, u.budget_bytes, u.kept_bytes, u.largest[0].upload_name(), u.waiting[0].uploader_name.as_str()), (10, 20, 4, Some("a.png"), "Ann"));
    assert_eq!(u.waiting[0].shown_name(), "Beach.png");

    let m = parse_mine(&serde_json::json!({"used_bytes": 100, "share_bytes": 512, "budget_left_bytes": 9000, "in_progress_bytes": 12, "file_limit_bytes": 0,
        "waiting": [{"filename": "3_w.mp4", "original_name": "Clip.mp4", "size_bytes": 40, "uploaded_at": 5}]}));
    assert_eq!((m.share_left(), m.waiting[0].filename.as_str(), m.waiting[0].shown_name()), (400, "3_w.mp4", "Clip.mp4"), "what is still arriving counts as used");

    let s = parse_suggestions(&serde_json::json!({"suggestions": [{"id": 4, "suggester_key": "k", "suggester_name": "Ann", "name": "Builds", "note": "", "created_at": 1, "state": "open"}]}));
    assert_eq!((s[0].id, s[0].suggester_name.as_str(), s[0].state.as_str()), (4, "Ann", "open"));
}

/// THE WORDS: sizes as people read them, the meter and its kept line, the share line from the
/// server's own numbers, and the stop for a file over the role's largest Media file, the share or
/// the whole space; Remove's confirmation from the item's own `keeps_file`.
///
/// Seen red 2026-10-11 with `size_words` keeping ".0": "left: \"50.0 GB\" / right: \"50 GB\"".
/// Seen red again 2026-10-11 (round 2) with `remove_words` still guessing from this session's own
/// uploads: an item the server says keeps its file read "Remove from Files and delete the file."
/// ("the server's word decides").
#[test]
fn the_words_say_what_they_mean() {
    const MB: i64 = 1024 * 1024;
    assert_eq!(size_words(50 * 1024 * MB), "50 GB");
    assert_eq!(meter_words(12 * 1024 * MB + 410 * MB, 50 * 1024 * MB), "12.4 GB of 50 GB used");
    assert!(kept_words(3 * MB).starts_with("3 MB of it kept by the server"));
    let mine = Mine { used_bytes: 100 * MB, share_bytes: 512 * MB, budget_left_bytes: 40 * 1024 * MB, ..Default::default() };
    assert_eq!(share_line(&mine), "You have 412 MB of your 512 MB left");
    assert_eq!(size_words(1536 * 1024), "1.5 MB");
    // A small file in KB or bytes, never "0 MB" (2026-10-17, as the website words it). Seen red the
    // same day with the KB step left out: 4000 bytes said "4000 bytes".
    assert_eq!((size_words(4000), size_words(300 * 1024), size_words(1), size_words(512)), ("4 KB".to_string(), "300 KB".to_string(), "1 byte".to_string(), "512 bytes".to_string()));
    assert_eq!(size_words(150 * MB), "150 MB");
    // Every shared case, the website's too (2026-10-17). Seen red the same day with the
    // half-to-even rounding of {:.1}: "1310720 bytes" (left: "1.2 MB", right: "1.3 MB").
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/tests/fixtures/size-words-cases.json");
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let cases = v["cases"].as_array().expect("the shared cases");
    assert!(cases.len() >= 15);
    for c in cases {
        let bytes = c[0].as_i64().unwrap();
        assert_eq!(size_words(bytes), c[1].as_str().unwrap(), "{bytes} bytes");
    }

    assert_eq!(room_problem(10, Some(&mine)), None, "a file that fits goes");
    assert_eq!(room_problem(1 << 40, None), None, "unknown numbers stop nothing (the server checks them)");
    let over_share = room_problem(500 * MB as u64, Some(&mine)).expect("over the share is stopped");
    assert!(over_share.contains("This file is 500 MB, more than the 412 MB left of your share"), "{over_share}");
    let arriving = Mine { in_progress_bytes: 400 * MB, ..mine.clone() };
    assert!(room_problem(20 * MB as u64, Some(&arriving)).is_some(), "an upload still arriving counts against the share");
    let limited = Mine { file_limit_bytes: 25 * MB, ..mine.clone() };
    let over_limit = room_problem(30 * MB as u64, Some(&limited)).expect("over the role's Media file limit is stopped");
    assert!(over_limit.contains("over the largest file your role may put on this server's Files page (25 MB)"), "{over_limit}");
    let full = Mine { budget_left_bytes: 5 * MB, ..mine };
    let over_space = room_problem(10 * MB as u64, Some(&full)).expect("over the whole space is stopped");
    assert!(over_space.contains("more than the 5 MB left in this server's whole file space"), "{over_space}");

    let public = Item { id: 1, visibility: "public".into(), url: "/uploads/x.png".into(), ..Default::default() };
    assert_eq!(remove_words(&Item { keeps_file: true, ..public.clone() }), REMOVE_KEEPS, "the server's word decides");
    assert_eq!(remove_words(&Item { keeps_file: false, ..public }), REMOVE_DELETES);
}

/// A MEMBERS-ONLY FILE IS FETCHED WITH THE SESSION'S TOKEN IN A HEADER, NEVER IN THE ADDRESS (the
/// relay's W3b): an address ends up in error words, run.log and lists. A public one at its own
/// address, with no token at all.
///
/// Seen red 2026-10-11 with the members' route not used: "left: \"https://h/api/files/file/7\" /
/// right: \"https://h/api/files/file/7?token=abc\"". Seen red again 2026-10-11 (round 2) with
/// `token_header` giving no header for a members-only item: "left: None / right:
/// Some((\"X-Upload-Token\", \"abc\"))".
#[test]
fn a_members_only_file_is_fetched_with_the_token_in_a_header() {
    let members = Item { id: 7, visibility: "members".into(), url: "/api/files/file/7".into(), ..Default::default() };
    let url = file_url("https://h/", &members);
    assert_eq!(url, "https://h/api/files/file/7");
    assert!(!url.contains("token"), "no token in the address");
    assert_eq!(token_header(&members, Some("abc")), Some((TOKEN_HEADER, "abc".to_string())));
    assert_eq!(TOKEN_HEADER.to_ascii_lowercase(), crate::relay::files::UPLOAD_TOKEN_HEADER, "the header the relay reads");
    let public = Item { id: 8, visibility: "public".into(), url: "/uploads/a.png".into(), ..Default::default() };
    assert_eq!(file_url("https://h", &public), "https://h/uploads/a.png");
    assert_eq!(token_header(&public, Some("abc")), None, "no token on a public address");
}

/// A REFUSAL KEEPS ITS STATUS AND NEVER CARRIES A TOKEN: the page tells a 409 (a tile out of
/// date) from any other refusal by its status, and the words shown have no upload token in them.
///
/// Seen red 2026-10-11 with `Refusal::new` keeping the words as given: "the token is hidden"
/// (left: "GET https://h/x?token=SECRET: refused" ...).
#[test]
fn a_refusal_keeps_its_status_and_never_carries_a_token() {
    let r = Refusal::new(409, "That is not the item you were looking at: the page is out of date. Load it again.");
    assert!(r.stale());
    assert!(!Refusal::new(403, "no").stale());
    let leaky = Refusal::new(0, "GET https://h/x?token=SECRET: refused");
    assert_eq!(leaky.words, "GET https://h/x?token=(hidden) refused", "the token is hidden");
    // And a request that cannot reach anyone says so with status 0.
    let unreachable = send("http://127.0.0.1:9", &Request { method: Method::Get, path: "/api/files?token=SECRET".into(), body: None, purpose: None })
        .expect_err("nothing listens there");
    assert_eq!(unreachable.status, 0);
    assert!(!unreachable.words.contains("SECRET"), "{}", unreachable.words);
}

/// SEVERAL FILES MUST FIT TOGETHER (2026-10-16): files chosen at once are checked one by one (a
/// file over the role's limit is named) and then all together against what is left of the share
/// and of the server's whole Media space, so a batch never starts uploads its last file cannot
/// finish.
///
/// Seen red 2026-10-16 with the total left out (each file only): "two that fit alone but not
/// together" found nothing wrong.
#[test]
fn several_files_must_fit_the_share_together_not_only_one_by_one() {
    let mine = Mine { share_bytes: 100, used_bytes: 20, budget_left_bytes: 1000, file_limit_bytes: 60, ..Default::default() };
    let both = room_problem_together(&[("a.stl", 50), ("b.stl", 50)], Some(&mine)).expect("two that fit alone but not together");
    assert!(both.starts_with("These 2 files come to") && both.contains("left of your share") && both.contains("none was uploaded"), "{both}");
    assert_eq!(room_problem_together(&[("a.stl", 40), ("b.stl", 40)], Some(&mine)), None, "80 of the 80 left fits");
    let named = room_problem_together(&[("a.stl", 10), ("big.mkv", 70)], Some(&mine)).expect("one over the role's limit");
    assert!(named.starts_with("big.mkv: "), "it is named: {named}");
    let space = Mine { budget_left_bytes: 50, ..mine.clone() };
    let whole = room_problem_together(&[("a.stl", 30), ("b.stl", 30)], Some(&space)).expect("more than the whole space has left");
    assert!(whole.contains("whole file space"), "{whole}");
    assert_eq!(room_problem_together(&[("a.stl", 50)], Some(&mine)), room_problem(50, Some(&mine)), "one file is checked as one always was");
    assert_eq!(room_problem_together(&[("a.stl", 50), ("b.stl", 50)], None), None, "with nothing known, the server checks");
}
