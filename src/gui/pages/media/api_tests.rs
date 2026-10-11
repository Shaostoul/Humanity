//! Tests of the Media page's requests and answers (api.rs). Each was seen to fail once before it
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

/// EVERY SIGNED REQUEST NAMES ITS TARGET, AND THE RELAY ACCEPTS ITS SIGNATURE: each write signs
/// the purpose the relay rebuilds from the same body (relay/media.rs, the review's F5), using the
/// relay's own functions where it has them, and its signature verifies there; a signature over a
/// purpose without the target does not.
///
/// Seen red 2026-10-11 with `pick_purpose` signing plain "media_pick": "the pick names its item"
/// (left: Some("media_pick") / right: Some("media_pick:42")), and with `categories_request`
/// hashing the list with a space after each comma but sending it without: "the relay hashes the
/// categories as sent" (two different BLAKE3 purposes).
#[test]
fn every_signed_request_names_its_target_and_the_relay_accepts_it() {
    let s = signer();

    let publish = publish_request(&s, &PublishFields { filename: "/uploads/1700_photo.png".into(), visibility: "public".into(), ..Default::default() });
    let relay_purpose = format!("media_publish:{}", crate::relay::media::stored_name(body(&publish)["filename"].as_str().unwrap()).unwrap());
    assert_eq!(publish.purpose.as_deref(), Some(relay_purpose.as_str()), "the publish names its file");
    assert_eq!(relay_purpose, "media_publish:1700_photo.png");
    assert!(relay_accepts(&publish, &relay_purpose));
    assert!(!relay_accepts(&publish, "media_publish"), "a signature names one file, no other");

    for target in [RemoveTarget::Item(42), RemoveTarget::Upload("/uploads/1700_clip.mp4".into())] {
        let req = remove_request(&s, &target);
        let parsed: crate::relay::media::RemoveBody = serde_json::from_str(req.body.as_deref().unwrap()).unwrap();
        let (_, relay_purpose) = parsed.target().expect("the relay reads the removal");
        assert_eq!(req.purpose.as_deref(), Some(relay_purpose.as_str()), "the removal names its target");
        assert!(relay_accepts(&req, &relay_purpose));
    }

    for on in [true, false] {
        let req = pick_request(&s, 42, on);
        assert_eq!(req.purpose.as_deref(), Some("media_pick:42"), "the pick names its item");
        assert_eq!(req.path, if on { "/api/media/pick" } else { "/api/media/unpick" });
        assert!(relay_accepts(&req, "media_pick:42"));
    }

    let feature = feature_request(&s, 9, false);
    assert_eq!(feature.purpose.as_deref(), Some("media_feature:9"));
    assert_eq!(body(&feature)["featured"], serde_json::json!(false));
    assert!(relay_accepts(&feature, "media_feature:9"));

    for accept in [true, false] {
        let req = decide_request(&s, 5, accept);
        let relay_purpose = crate::relay::media::decide_purpose(5, accept);
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
    let (parsed, relay_purpose) = crate::relay::media::categories_request(cats.body.as_deref().unwrap()).expect("the relay reads the list");
    assert_eq!(cats.purpose.as_deref(), Some(relay_purpose.as_str()), "the relay hashes the categories as sent");
    assert_eq!(parsed.categories.len(), 2);
    assert_eq!(parsed.categories[1].id, None, "a new category has no id");
    assert!(relay_accepts(&cats, &relay_purpose));

    // The reads and the suggestion keep their plain purposes.
    for (req, purpose) in [
        (suggest_request(&s, "Builds", "Ships and homes"), "media_suggest"),
        (suggestions_request(&s), "media_suggestions"),
        (usage_request(&s), "media_usage"),
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
/// be about 10 KB); My picks, always the public GET of the person's picks.
///
/// Seen red 2026-10-11 with the search not encoded: "the search is encoded" (the address held
/// "fire staff" with its space).
#[test]
fn the_list_asks_the_right_way() {
    let f = Filters { category: Some(3), kind: Some("video".into()), by: None, q: "fire staff".into(), my_picks: false };
    let public = list_request(&f, Some(120), 48, "me", None);
    assert_eq!(public.method, Method::Get);
    assert_eq!(public.path, "/api/media?limit=48&category=3&kind=video&q=fire+staff&before=120", "the search is encoded");
    assert!(public.purpose.is_none() && public.body.is_none());

    let s = signer();
    let signed = list_request(&f, Some(120), 48, &s.key, Some(&s));
    assert_eq!((signed.method, signed.path.as_str()), (Method::Post, "/api/media/list"));
    let b = body(&signed);
    assert_eq!((b["category"].as_i64(), b["kind"].as_str(), b["q"].as_str(), b["before"].as_i64()), (Some(3), Some("video"), Some("fire staff"), Some(120)));
    assert!(signed.path.len() < 100, "nothing signed rides in the address");

    let picks = list_request(&Filters { my_picks: true, ..Default::default() }, None, 100, "abc", Some(&s));
    assert_eq!((picks.method, picks.path.as_str()), (Method::Get, "/api/media/picks/abc?limit=100"));
}

/// THE SERVER'S ANSWERS READ: an item with its category, labels and members-only address; the
/// list's paging and views; the usage's numbers; the suggestions.
///
/// Seen red 2026-10-11 with the category read from "category_id" (the relay sends
/// `category: {id, name}`): "the item's category" (left: (0, "Fire flow arts") / right: (2, "Fire flow arts")).
#[test]
fn the_servers_answers_read() {
    let v = serde_json::json!({
        "items": [{"id": 7, "url": "/api/media/file/7", "title": "", "description": "Spinning at dusk", "category": {"id": 2, "name": "Fire flow arts"},
                   "kind": "video", "labels": ["flashing"], "visibility": "members", "featured": true, "size_bytes": 1048576, "views": 0,
                   "uploaded_at": 1, "published_at": 2, "uploader_key": "k1", "uploader_name": "Shaostoul", "picks": 3}],
        "next_before": 7, "members_view": true, "media_visibility": "public", "members_only_for_you": false,
        "categories": [{"id": 1, "name": "General", "description": "", "position": 0}],
        "labels": [{"id": "flashing", "label": "Flashing lights", "desc": "Rapid flashes"}],
    });
    let a = parse_list(&v);
    let item = &a.items[0];
    assert_eq!((item.category_id, item.category_name.as_str()), (2, "Fire flow arts"), "the item's category");
    assert!(item.members_only() && item.featured && item.upload_name().is_none());
    assert_eq!(item.display_title(), "Video", "an untitled members-only item is called by its kind");
    assert_eq!((a.next_before, a.members_view, a.labels[0].label.as_str()), (Some(7), true, "Flashing lights"));

    let u = parse_usage(&serde_json::json!({"used_bytes": 10, "budget_bytes": 20, "per_person_bytes": 5, "items": 1,
        "largest": [{"id": 1, "url": "/uploads/a.png"}], "oldest": [], "least_viewed": [],
        "waiting": [{"filename": "b.png", "uploader_key": "k", "uploader_name": "Ann", "size_bytes": 4, "uploaded_at": 1, "expires_at": 2}]}));
    assert_eq!((u.used_bytes, u.budget_bytes, u.largest[0].upload_name(), u.waiting[0].uploader_name.as_str()), (10, 20, Some("a.png"), "Ann"));

    let s = parse_suggestions(&serde_json::json!({"suggestions": [{"id": 4, "suggester_key": "k", "suggester_name": "Ann", "name": "Builds", "note": "", "created_at": 1, "state": "open"}]}));
    assert_eq!((s[0].id, s[0].suggester_name.as_str(), s[0].state.as_str()), (4, "Ann", "open"));
}

/// THE WORDS: sizes as people read them, the meter, the share line, and the stop for a file over
/// the share or the upload limit; Remove's confirmation names what happens to the file.
///
/// Seen red 2026-10-11 with `size_words` keeping ".0": "left: \"50.0 GB\" / right: \"50 GB\"".
#[test]
fn the_words_say_what_they_mean() {
    const MB: i64 = 1024 * 1024;
    assert_eq!(size_words(50 * 1024 * MB), "50 GB");
    assert_eq!(meter_words(12 * 1024 * MB + 410 * MB, 50 * 1024 * MB), "12.4 GB of 50 GB used");
    assert_eq!(share_line(100 * MB, 512 * MB), "You have 412 MB of your 512 MB left");
    assert_eq!(size_words(1536 * 1024), "1.5 MB");

    assert_eq!(room_problem(10, Some(100), Some(50)), None, "a file that fits goes");
    let over_share = room_problem(60 * MB as u64, None, Some(50 * MB)).expect("over the share is stopped");
    assert!(over_share.contains("This file is 60 MB, more than the 50 MB left of your share"), "{over_share}");
    let over_limit = room_problem(30 * MB as u64, Some(25 * MB as u64), Some(500 * MB)).expect("over the limit is stopped");
    assert!(over_limit.contains("over your 25 MB upload limit"), "{over_limit}");
    assert_eq!(room_problem(60 * MB as u64, None, None), None, "an unknown share stops nothing (the server checks it)");

    let public = Item { id: 1, visibility: "public".into(), url: "/uploads/x.png".into(), ..Default::default() };
    let members = Item { visibility: "members".into(), ..public.clone() };
    assert_eq!(remove_words(&members, false, false), REMOVE_DELETES, "a members-only item was uploaded for Media");
    assert_eq!(remove_words(&public, true, false), REMOVE_DELETES);
    assert_eq!(remove_words(&public, false, true), REMOVE_KEEPS);
    assert_eq!(remove_words(&public, false, false), REMOVE_EITHER, "not known: both cases said");
    assert_eq!(remove_words(&Item { from_shelf: Some(0), ..public.clone() }, true, false), REMOVE_KEEPS, "the server's word wins");
}

/// A MEDIA UPLOAD SAYS SO: the one upload every file takes carries `media=1` for the Media page,
/// and never `encrypted=1` with it (the relay refuses that pair).
///
/// Seen red 2026-10-11 with the media flag left out of `upload_query`: "left: \"token=t\" /
/// right: \"token=t&media=1\"".
#[test]
fn a_media_upload_says_so() {
    use crate::gui::pages::chat::{upload_query, UploadFlags};
    assert_eq!(upload_query("t", UploadFlags { media: true, ..Default::default() }), "token=t&media=1");
    assert_eq!(upload_query("t", UploadFlags { media: true, encrypted: true, share: false }), "token=t&media=1");
    assert_eq!(upload_query("t", UploadFlags { share: true, ..Default::default() }), "token=t&share=1", "Shared Files as before");
    assert_eq!(upload_query("t", UploadFlags { encrypted: true, ..Default::default() }), "token=t&encrypted=1", "a private file as before");
}

/// A MEMBERS-ONLY FILE IS FETCHED WITH THE SESSION'S TOKEN, a public one at its own address.
///
/// Seen red 2026-10-11 with the members' route not used: "left:
/// \"https://h/api/media/file/7\" / right: \"https://h/api/media/file/7?token=abc\"".
#[test]
fn a_members_only_file_is_fetched_with_the_token() {
    let members = Item { id: 7, visibility: "members".into(), url: "/api/media/file/7".into(), ..Default::default() };
    assert_eq!(file_url("https://h/", &members, Some("abc")), "https://h/api/media/file/7?token=abc");
    let public = Item { id: 8, visibility: "public".into(), url: "/uploads/a.png".into(), ..Default::default() };
    assert_eq!(file_url("https://h", &public, Some("abc")), "https://h/uploads/a.png", "no token on a public address");
}
