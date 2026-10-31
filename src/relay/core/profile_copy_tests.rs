// Child of profile_copy.rs (#[path], so `use super::*` reaches everything it has): the copy for
// other servers, its limits, when a server asks for one, the time on it, and the shared fixture
// both apps read (BUG-191, docs/design/bug-191-profile-copies-plan.md, Wave 0). The pinned words
// are in profile_copy.rs itself, where the website's test reads them.

use super::*;

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
pub(crate) const FIXTURE: &str = "scripts/tests/fixtures/profile-copy-v1.json";
/// The signer's seed byte: the 32-byte seed `[7; 32]`, the key every Rust fixture uses.
pub(crate) const FX_SIGNER: u8 = 7;
/// The device's clock in every case, in milliseconds.
pub(crate) const FX_NOW_MS: u64 = 1_760_000_000_000;

/// One case: the person's stored profile and privacy map, and the time of the copy the server
/// holds for the key (`signed_at`, 0 for none).
pub(crate) struct FxCase {
    pub(crate) label: &'static str,
    pub(crate) bio: &'static str,
    pub(crate) avatar_url: &'static str,
    pub(crate) banner_url: &'static str,
    pub(crate) socials: &'static str,
    pub(crate) pronouns: &'static str,
    pub(crate) location: &'static str,
    pub(crate) website: &'static str,
    pub(crate) privacy: &'static str,
    pub(crate) signed_at: u64,
}

pub(crate) fn fx_cases() -> [FxCase; 4] {
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
pub(crate) const FX_TIMES: [(u64, u64, u64, u64); 7] = [
    (1_000, 0, 0, 1_000),
    (1_000, 5_000, 0, 5_001),
    (1_000, 0, 3_000, 3_001),
    (1_000, 5_000, 7_000, 7_001),
    (9_000, 5_000, 7_000, 9_000),
    (1_000, 1_000, 0, 1_001),
    (FX_NOW_MS, FX_NOW_MS + 90_000, FX_NOW_MS + 90_001, FX_NOW_MS + 90_002),
];

pub(crate) fn fx_key() -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[FX_SIGNER; 32])).public_key())
}

/// What a device signs from an own-copy frame: `public_copy` of the frame's name and fields
/// exactly (an absent field is blank) under its privacy map.
pub(crate) fn copy_of_frame(frame: &Value) -> PublicCopy {
    let s = |k: &str| frame[k].as_str().unwrap_or("").to_string();
    public_copy(
        &s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"),
        &s("pronouns"), &s("location"), &s("website"), &s("privacy"),
    )
}
