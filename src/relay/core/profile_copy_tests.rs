// Child of profile_copy.rs (#[path], so `use super::*` reaches everything it has): the copy for
// other servers, its limits, when a server asks for one, the time on it, and the shared fixture
// both apps read (BUG-191, docs/design/bug-191-profile-copies-plan.md, Wave 0). The pinned words
// are in profile_copy.rs itself, where the website's test reads them.

use super::*;
use serde_json::json;

/// A key with the shape copies carry (3,904 lowercase hex characters), for tests about limits.
fn key() -> String {
    "ab".repeat(KEY_HEX_LEN / 2)
}

/// Ann's profile as anyone may see it, inside every limit.
fn ann() -> PublicCopy {
    PublicCopy {
        name: "Ann".to_string(),
        bio: "Γεια!\nline two".to_string(),
        avatar_url: "https://ann.example/a.png".to_string(),
        banner_url: String::new(),
        socials: r#"{"github":"ann"}"#.to_string(),
        pronouns: "she/her".to_string(),
        location: "Lisbon".to_string(),
        website: "https://ann.example".to_string(),
    }
}

/// THE PRIVACY RULE: the copy that leaves this server holds only what anyone may see. A field
/// kept for friends is blank in it; a public one and the name, bio, avatar and links are as stored;
/// an unlisted person's copy is the withdrawal; links stored blank read as `{}`; a map that is not
/// an object keeps nothing for friends and lists the person. A map holding one value that is not a
/// word still keeps a field for friends (the relay's older reading made every field public then).
///
/// Seen red 2026-10-12 with the `"private"` check removed from `public_copy` (every field shown):
/// "a location kept for friends went into the copy for other servers" failed (left "Lisbon",
/// right "").
#[test]
fn the_copy_for_other_servers_holds_only_what_anyone_may_see() {
    let copy = |privacy: &str| {
        public_copy("Ann", "hi", "https://ann.example/a.png", "", r#"{"github":"ann"}"#, "she/her", "Lisbon", "https://ann.example", privacy)
    };

    // Listed, location kept for friends, pronouns marked public, website not in the map.
    let c = copy(r#"{"location":"private","pronouns":"public"}"#);
    assert_eq!(c.location, "", "a location kept for friends went into the copy for other servers");
    assert_eq!((c.pronouns.as_str(), c.website.as_str()), ("she/her", "https://ann.example"), "the public fields are in it");
    assert_eq!(
        (c.name.as_str(), c.bio.as_str(), c.avatar_url.as_str(), c.banner_url.as_str(), c.socials.as_str()),
        ("Ann", "hi", "https://ann.example/a.png", "", r#"{"github":"ann"}"#),
        "name, bio, pictures and links always, exactly as stored"
    );
    let c = copy(r#"{"pronouns":"private","location":"private","website":"private"}"#);
    assert_eq!((c.pronouns.as_str(), c.location.as_str(), c.website.as_str()), ("", "", ""), "all three kept for friends");
    assert_eq!(copy(r#"{"location":"private","since":2019}"#).location, "", "a map holding a number still keeps the location");
    assert_eq!(copy(r#"{"location":"friends"}"#).location, "Lisbon", "only the word \"private\" keeps a field");

    // Unlisted: the withdrawal, whatever else the map says.
    let w = copy(r#"{"directory":"unlisted","location":"public"}"#);
    assert_eq!(w, withdrawal(), "an unlisted person's copy is the withdrawal");
    assert!(is_withdrawal(&w) && w.socials == "{}" && w.bio.is_empty() && w.avatar_url.is_empty());
    assert_eq!(copy(r#"{"directory":"listed"}"#).name, "Ann", "any other directory word is listed");
    assert!(!unlisted(r#"{"directory":["unlisted"]}"#), "the word itself, not a list holding it");

    // Links stored blank, map "{}".
    let c = public_copy("Ann", "", "", "", "", "they", "Porto", "", "{}");
    assert_eq!((c.socials.as_str(), c.pronouns.as_str(), c.location.as_str()), ("{}", "they", "Porto"), "blank links read as {{}}");

    // A map that is not an object: listed, nothing kept for friends.
    for map in ["[]", "\"private\"", "not json", ""] {
        let c = copy(map);
        assert_eq!(c, copy("{}"), "the map {map:?} keeps nothing and lists the person");
        assert!(!unlisted(map) && !kept_for_friends(map, "location"));
    }
}

/// A COPY IS DATED AFTER THE NEWEST SEEN: after the device's clock, after the copy the server
/// holds (from any device or server) and after this device's own last copy for that server, so a
/// device with a fast clock no longer wins every later edit. Saturating at the top.
///
/// Seen red 2026-10-12 with `next_copy_time` returning `now_ms` alone: "a copy held from a device
/// whose clock runs fast: the next is dated after it" failed (left 1000, right 5001).
#[test]
fn each_copy_is_dated_after_the_newest_seen() {
    assert_eq!(next_copy_time(1_000, 0, 0), 1_000, "nothing seen: the clock");
    assert_eq!(next_copy_time(1_000, 5_000, 0), 5_001, "a copy held from a device whose clock runs fast: the next is dated after it");
    assert_eq!(next_copy_time(1_000, 0, 3_000), 3_001, "after this device's own last copy for that server");
    assert_eq!(next_copy_time(1_000, 5_000, 7_000), 7_001, "after the newer of the two");
    assert_eq!(next_copy_time(9_000, 5_000, 7_000), 9_000, "a clock ahead of both: the clock");
    assert_eq!(next_copy_time(1_000, 1_000, 0), 1_001, "the same time is not after");
    assert_eq!(next_copy_time(5, u64::MAX, 0), u64::MAX, "saturating");
}

/// THE LIMITS, held by the server that asks and by every server that receives: what a save would
/// refuse is never kept. No line break in any field but the bio (so the signed words split one
/// way only); the bio counted in characters; links a JSON object; picture and website links
/// https; a withdrawal holding anything; a key in any spelling but lowercase hex.
///
/// Seen red 2026-10-12 with the line-break rule removed from `fields_within_limits`: "a location
/// holding a line break passed" failed.
#[test]
fn a_copy_over_the_limits_or_with_a_line_break_outside_the_bio_is_refused() {
    let k = key();
    assert!(within_limits(&k, &ann()), "a copy a save would keep passes");
    assert!(within_limits(&k, &withdrawal()), "the withdrawal passes");
    let with = |edit: &dyn Fn(&mut PublicCopy)| {
        let mut c = ann();
        edit(&mut c);
        within_limits(&k, &c)
    };

    assert!(!with(&|c| c.location = "Lis\nbon".into()), "a location holding a line break passed");
    assert!(!with(&|c| c.pronouns = "she\nher".into()), "pronouns holding a line break passed");
    assert!(!with(&|c| c.website = "https://a.example\n".into()), "a website holding a line break passed");
    assert!(!with(&|c| c.avatar_url = "https://a.example/\n.png".into()), "an avatar holding a line break passed");
    assert!(!with(&|c| c.banner_url = "https://a.example/\n.png".into()), "a banner holding a line break passed");
    assert!(!with(&|c| c.socials = "{\n\"github\":\"ann\"}".into()), "links holding a line break passed");
    assert!(with(&|c| c.bio = "one\ntwo\nthree".into()), "the bio may hold line breaks");

    assert!(with(&|c| c.bio = "α".repeat(BIO_MAX_CHARS)), "280 Greek letters (560 bytes)");
    assert!(!with(&|c| c.bio = "a".repeat(BIO_MAX_CHARS + 1)), "281 characters");
    assert!(!with(&|c| c.bio = "<b>hi".into()), "a < in the bio (the save removes it)");
    assert!(!with(&|c| c.bio = "hi>".into()), "a > in the bio");
    assert!(!with(&|c| c.socials = "[]".into()), "links that are not an object");
    assert!(!with(&|c| c.socials = String::new()), "links blank (the copy reads them as {{}})");
    assert!(!with(&|c| c.socials = format!(r#"{{"a":"{}"}}"#, "x".repeat(LINKS_MAX_BYTES))), "links over 1,024 bytes");
    assert!(!with(&|c| c.avatar_url = "http://ann.example/a.png".into()), "an http avatar");
    assert!(!with(&|c| c.banner_url = "javascript:alert(1)".into()), "a banner that is not a link");
    assert!(!with(&|c| c.avatar_url = format!("https://{}", "a".repeat(PICTURE_LINK_MAX_BYTES - 7))), "an avatar of 513 bytes");
    assert!(with(&|c| c.avatar_url = format!("https://{}", "a".repeat(PICTURE_LINK_MAX_BYTES - 8))), "an avatar of 512 bytes");
    assert!(!with(&|c| c.website = format!("https://{}", "a".repeat(WEBSITE_MAX_BYTES - 7))), "a website of 257 bytes");
    assert!(!with(&|c| c.pronouns = "a".repeat(PRONOUNS_MAX_CHARS + 1)), "65 characters of pronouns");
    assert!(with(&|c| c.location = "é".repeat(LOCATION_MAX_CHARS)), "128 characters of location (256 bytes)");
    assert!(!with(&|c| c.location = "a".repeat(LOCATION_MAX_CHARS + 1)), "129 characters of location");
    assert!(!with(&|c| c.name = "Ann Lee".into()), "a space in a name");
    assert!(!with(&|c| c.name = "Ánn".into()), "a letter outside A-Z in a name");
    assert!(!with(&|c| c.name = "a".repeat(NAME_MAX_BYTES + 1)), "a name of 25");

    assert!(!within_limits(&k, &PublicCopy { bio: "left behind".into(), ..withdrawal() }), "a withdrawal carrying a bio");
    assert!(!within_limits(&k, &PublicCopy { location: "Lisbon".into(), ..withdrawal() }), "a withdrawal carrying a location");
    assert!(!within_limits(&k, &PublicCopy { socials: String::new(), ..withdrawal() }), "a withdrawal with links other than {{}}");

    assert!(!within_limits(&k.to_uppercase(), &ann()), "an upper-case key");
    assert!(!within_limits(&k[..KEY_HEX_LEN - 2], &ann()), "a short key");
    assert!(!within_limits("bot_helper", &ann()), "a bot's key");
}

/// WHEN A SERVER ASKS A DEVICE TO SIGN. A held copy that shows more than the person allows here
/// (they are unlisted, or a field they keep for friends is in it) is "exposing"; a withdrawal
/// never is. After a save: when there is no copy out there yet, or the held one differs. On
/// connect or the person's own request: only to narrow what is out there, never to push an older
/// profile over a newer save made elsewhere. Never for a copy over the limits, and never a
/// withdrawal where nothing was passed on.
///
/// Seen red 2026-10-12 with `exposes` ignoring the directory: "an unlisted person's listed copy
/// was left out there" failed.
#[test]
fn the_relay_asks_for_a_copy_only_when_one_is_due() {
    let listed = "{}";
    let loc_private = r#"{"location":"private"}"#;
    let unlisted_map = r#"{"directory":"unlisted"}"#;
    let full = ann();
    let narrowed = PublicCopy { location: String::new(), ..ann() };
    let other_bio = PublicCopy { bio: "saved on another server".into(), ..ann() };
    let w = withdrawal();
    let over = PublicCopy { bio: "a".repeat(BIO_MAX_CHARS + 1), ..ann() };

    assert!(exposes(&full, unlisted_map), "an unlisted person's listed copy was left out there");
    assert!(exposes(&full, loc_private), "a location kept for friends is out there");
    assert!(!exposes(&narrowed, loc_private), "the location is blank in it");
    assert!(!exposes(&w, unlisted_map), "a withdrawal shows nothing");
    assert!(!exposes(&full, listed), "nothing kept for friends");

    let rows: [(Option<&PublicCopy>, &PublicCopy, bool, &str, bool, &str); 15] = [
        (None, &full, true, listed, true, "a first save asks"),
        (None, &full, false, listed, false, "a connect with no copy out there does not ask"),
        (None, &w, true, unlisted_map, false, "no withdrawal where nothing was passed on"),
        (None, &w, false, unlisted_map, false, "nor on connect"),
        (Some(&full), &w, true, unlisted_map, true, "going unlisted withdraws the copy out there"),
        (Some(&full), &w, false, unlisted_map, true, "a device that connects after going unlisted elsewhere withdraws it"),
        (Some(&w), &w, true, unlisted_map, false, "a withdrawal already out there"),
        (Some(&full), &full, true, listed, false, "a save that changed nothing in the copy"),
        (Some(&full), &narrowed, true, loc_private, true, "a save that made the location friends-only"),
        (Some(&narrowed), &full, true, listed, true, "a save that made it public again"),
        (Some(&full), &narrowed, false, loc_private, true, "a connect after the location was made friends-only"),
        (Some(&other_bio), &full, false, listed, false, "a benign difference does not ask on connect"),
        (Some(&w), &full, false, listed, false, "a connect never lists a withdrawn copy again"),
        (Some(&full), &over, true, listed, false, "a copy over the limits is never asked for"),
        (None, &over, true, listed, false, "not even the first"),
    ];
    for (held, now, just_saved, privacy, want, why) in rows {
        assert_eq!(sign_wanted(held, now, just_saved, privacy), want, "{why}");
    }
}

// ── The shared fixture ──

/// The fixture both apps read.
const FIXTURE: &str = "scripts/tests/fixtures/profile-copy-v1.json";
/// The signer's seed byte: the 32-byte seed `[7; 32]`, the key every Rust fixture uses.
const FX_SIGNER: u8 = 7;
/// The device's clock in every case, in milliseconds.
const FX_NOW_MS: u64 = 1_760_000_000_000;

/// One case: the person's stored profile and privacy map, and the time of the copy the server
/// holds for the key (`signed_at`, 0 for none).
struct FxCase {
    label: &'static str,
    bio: &'static str,
    avatar_url: &'static str,
    banner_url: &'static str,
    socials: &'static str,
    pronouns: &'static str,
    location: &'static str,
    website: &'static str,
    privacy: &'static str,
    signed_at: u64,
}

fn fx_cases() -> [FxCase; 4] {
    [
        FxCase {
            label: "listed; location kept for friends; pronouns marked public; a Greek bio with a line break; links in an order a re-serializer would change; the copy held is dated ahead of the clock",
            bio: "Γεια σας!\nΖω στη Λισαβόνα.",
            avatar_url: "https://ann.example/a.png",
            banner_url: "",
            socials: r#"{"github":"ann","discord":"ann#1"}"#,
            pronouns: "she/her",
            location: "Lisbon",
            website: "https://ann.example",
            privacy: r#"{"location":"private","pronouns":"public"}"#,
            signed_at: FX_NOW_MS + 90_000,
        },
        FxCase {
            label: "unlisted: the withdrawal",
            bio: "not for other servers",
            avatar_url: "https://ann.example/a.png",
            banner_url: "https://ann.example/b.png",
            socials: r#"{"github":"ann"}"#,
            pronouns: "she/her",
            location: "Lisbon",
            website: "",
            privacy: r#"{"directory":"unlisted"}"#,
            signed_at: FX_NOW_MS - 5_000,
        },
        FxCase {
            label: "links stored blank (read as {}), map {}; no copy held",
            bio: "",
            avatar_url: "",
            banner_url: "",
            socials: "",
            pronouns: "they/them",
            location: "Porto",
            website: "",
            privacy: "{}",
            signed_at: 0,
        },
        FxCase {
            label: "a map that is not an object: listed, nothing kept for friends; the copy held has the clock's own time",
            bio: "hello",
            avatar_url: "",
            banner_url: "",
            socials: "{}",
            pronouns: "she/her",
            location: "Lisbon",
            website: "https://ann.example",
            privacy: "[]",
            signed_at: FX_NOW_MS,
        },
    ]
}

/// The fixture's `next_copy_time` rows: (now_ms, signed_at, last_here, at).
const FX_TIMES: [(u64, u64, u64, u64); 7] = [
    (1_000, 0, 0, 1_000),
    (1_000, 5_000, 0, 5_001),
    (1_000, 0, 3_000, 3_001),
    (1_000, 5_000, 7_000, 7_001),
    (9_000, 5_000, 7_000, 9_000),
    (1_000, 1_000, 0, 1_001),
    (FX_NOW_MS, FX_NOW_MS + 90_000, FX_NOW_MS + 90_001, FX_NOW_MS + 90_002),
];

fn fx_key() -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[FX_SIGNER; 32])).public_key())
}

/// The person's own copy exactly as the relay sends it (the echo of a save, asking): the frame a
/// device reads its copy from.
fn own_copy_frame(key: &str, c: &FxCase) -> Value {
    use crate::relay::relay::RelayMessage;
    let opt = |s: &str| if s.is_empty() { None } else { Some(s.to_string()) };
    serde_json::to_value(RelayMessage::ProfileData {
        name: "Ann".to_string(),
        bio: c.bio.to_string(),
        socials: c.socials.to_string(),
        avatar_url: opt(c.avatar_url),
        banner_url: opt(c.banner_url),
        pronouns: opt(c.pronouns),
        location: opt(c.location),
        website: opt(c.website),
        accepts_files: Some("friends".to_string()),
        privacy: Some(c.privacy.to_string()),
        stored: Some(true),
        signed_at: Some(c.signed_at),
        sign_now: Some(true),
        target: Some(key.to_string()),
    })
    .expect("a frame")
}

/// What a device signs from an own-copy frame: `public_copy` of the frame's name and fields
/// exactly (an absent field is blank) under its privacy map.
fn copy_of_frame(frame: &Value) -> PublicCopy {
    let s = |k: &str| frame[k].as_str().unwrap_or("").to_string();
    public_copy(
        &s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"),
        &s("pronouns"), &s("location"), &s("website"), &s("privacy"),
    )
}

/// The whole fixture, as this code makes it today.
fn fixture_now() -> Value {
    let key = fx_key();
    let cases: Vec<Value> = fx_cases()
        .iter()
        .map(|c| {
            let frame = own_copy_frame(&key, c);
            let copy = copy_of_frame(&frame);
            let at = next_copy_time(FX_NOW_MS, c.signed_at, 0);
            json!({
                "label": c.label,
                "now_ms": FX_NOW_MS,
                "frame": frame,
                "copy": copy,
                "at": at,
                "text": copy_text(&key, &copy, at),
                "signature": sign_copy(&[FX_SIGNER; 32], &key, &copy, at),
            })
        })
        .collect();
    let times: Vec<Value> = FX_TIMES
        .iter()
        .map(|&(now_ms, signed_at, last_here, at)| json!({ "now_ms": now_ms, "signed_at": signed_at, "last_here": last_here, "at": at }))
        .collect();
    json!({
        "about": "The copy of a person's profile that other servers keep (BUG-191, 2026-10-12, docs/design/bug-191-profile-copies-plan.md 3.1 to 3.4). Made by the relay's own builder (src/relay/core/profile_copy.rs; ML-DSA signing there is deterministic) for the key of the 32-byte seed [signer_master_byte; 32]. Each case: `frame` is the person's own copy exactly as the relay sends it to a device (a profile_data with privacy, signed_at and sign_now: true); `copy` is public_copy of the frame's name and fields (an absent field is blank) under its privacy map; `at` is next_copy_time(now_ms, frame.signed_at, 0); `text` the exact words signed (copy_text); `signature` their Dilithium3 signature, lowercase hex. A web signature over the same words differs (noble's signing is hedged) but verifies the same. `next_copy_time` rows: at = max(now_ms, signed_at + 1, last_here + 1). Written by profile_copy_tests.rs write_the_profile_copy_fixture; checked by the_shared_fixture_is_what_the_builder_makes; read by both apps' tests.",
        "signer_master_byte": FX_SIGNER,
        "public_key": key,
        "cases": cases,
        "next_copy_time": times,
    })
}

/// WRITES scripts/tests/fixtures/profile-copy-v1.json from fixed seeds. Run it again only when the
/// shape changes on purpose: `node scripts/lib-tests.js write_the_profile_copy_fixture --ignored`.
#[test]
#[ignore = "writes scripts/tests/fixtures/profile-copy-v1.json; run: node scripts/lib-tests.js write_the_profile_copy_fixture --ignored"]
fn write_the_profile_copy_fixture() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    std::fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&fixture_now()).unwrap())).expect("the fixture is written");
}

/// THE SHARED FIXTURE BOTH APPS READ IS WHAT THIS CODE MAKES. From each case's own-copy frame the
/// builder makes the fixture's copy, time, words and signature byte for byte (ML-DSA signing here
/// is deterministic); the signature verifies with the key of the fixture's seed, and one changed
/// character in the words (the time, or the bio) fails it; the `next_copy_time` rows hold; and the
/// whole file is what the writer makes today, so a change to the relay's frame shows here.
///
/// Seen red 2026-10-12 with the first case's bio in the fixture's frame edited (`Λισαβόνα` to
/// `Λισσαβόνα`): "case 0: the copy is public_copy of the frame" failed.
#[test]
fn the_shared_fixture_is_what_the_builder_makes() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE);
    let fx: Value = serde_json::from_str(&std::fs::read_to_string(&path).expect("the fixture is there")).expect("JSON");
    let seed = [fx["signer_master_byte"].as_u64().expect("the seed byte") as u8; 32];
    let key = fx["public_key"].as_str().expect("the key").to_string();
    assert_eq!(key, fx_key(), "the key the seed gives");
    assert!(is_key_text(&key));

    let cases = fx["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 4, "four cases");
    for (i, case) in cases.iter().enumerate() {
        let frame = &case["frame"];
        assert_eq!((frame["type"].as_str(), frame["sign_now"].as_bool()), (Some("profile_data"), Some(true)), "case {i}: an own copy that asks");
        let copy = copy_of_frame(frame);
        let fixed: PublicCopy = serde_json::from_value(case["copy"].clone()).expect("a copy");
        assert_eq!(copy, fixed, "case {i}: the copy is public_copy of the frame");
        assert!(within_limits(&key, &copy), "case {i}: within the limits");
        let at = next_copy_time(case["now_ms"].as_u64().unwrap(), frame["signed_at"].as_u64().unwrap(), 0);
        assert_eq!(Some(at), case["at"].as_u64(), "case {i}: the time");
        let text = copy_text(&key, &copy, at);
        assert_eq!(Some(text.as_str()), case["text"].as_str(), "case {i}: the words");
        let sig = sign_copy(&seed, &key, &copy, at);
        assert_eq!(Some(sig.as_str()), case["signature"].as_str(), "case {i}: the signature, byte for byte");
        assert!(verify_copy(&key, &copy, at, &sig), "case {i}: it verifies");
        assert!(!verify_copy(&key, &copy, at ^ 1, &sig), "case {i}: one changed digit in the time fails it");
        if !copy.bio.is_empty() {
            let mut bio = copy.bio.chars().collect::<Vec<_>>();
            bio[0] = if bio[0] == 'x' { 'y' } else { 'x' };
            let changed = PublicCopy { bio: bio.into_iter().collect(), ..copy.clone() };
            assert!(!verify_copy(&key, &changed, at, &sig), "case {i}: one changed letter in the bio fails it");
        }
    }
    assert!(is_withdrawal(&serde_json::from_value(cases[1]["copy"].clone()).unwrap()), "the unlisted case is the withdrawal");

    for row in fx["next_copy_time"].as_array().expect("time rows") {
        let n = |k: &str| row[k].as_u64().unwrap();
        assert_eq!(next_copy_time(n("now_ms"), n("signed_at"), n("last_here")), n("at"), "{row}");
    }
    assert_eq!(fx, fixture_now(), "the fixture is what the writer makes today");
}
