// Child of profile_copy.rs (#[path], so `use super::*` reaches everything it has): the copy for
// other servers, its limits, when a server asks for one, and the time on it (BUG-191,
// docs/design/bug-191-profile-copies-plan.md, Wave 0). The shared fixture both apps read is in
// profile_copy_fixtures.rs; the pinned words are in profile_copy.rs itself, where the website's
// test reads them.

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
/// an object keeps nothing for friends and does not list the person (since BUG-241, 2026-11-03,
/// only `"directory": "listed"` lists: `only_the_word_listed_lists_a_person`). A map holding one
/// value that is not a word still keeps a field for friends (the relay's older reading made every
/// field public then).
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
    let c = copy(r#"{"directory":"listed","location":"private","pronouns":"public"}"#);
    assert_eq!(c.location, "", "a location kept for friends went into the copy for other servers");
    assert_eq!((c.pronouns.as_str(), c.website.as_str()), ("she/her", "https://ann.example"), "the public fields are in it");
    assert_eq!(
        (c.name.as_str(), c.bio.as_str(), c.avatar_url.as_str(), c.banner_url.as_str(), c.socials.as_str()),
        ("Ann", "hi", "https://ann.example/a.png", "", r#"{"github":"ann"}"#),
        "name, bio, pictures and links always, exactly as stored"
    );
    let c = copy(r#"{"directory":"listed","pronouns":"private","location":"private","website":"private"}"#);
    assert_eq!((c.pronouns.as_str(), c.location.as_str(), c.website.as_str()), ("", "", ""), "all three kept for friends");
    assert_eq!(copy(r#"{"directory":"listed","location":"private","since":2019}"#).location, "", "a map holding a number still keeps the location");
    assert_eq!(copy(r#"{"directory":"listed","location":"friends"}"#).location, "Lisbon", "only the word \"private\" keeps a field");

    // Unlisted: the withdrawal, whatever else the map says.
    let w = copy(r#"{"directory":"unlisted","location":"public"}"#);
    assert_eq!(w, withdrawal(), "an unlisted person's copy is the withdrawal");
    assert!(is_withdrawal(&w) && w.socials == "{}" && w.bio.is_empty() && w.avatar_url.is_empty());
    assert_eq!(copy(r#"{"location":"public"}"#), withdrawal(), "no directory word: never chose, unlisted");
    assert!(unlisted(r#"{"directory":["listed"]}"#), "the word itself, not a list holding it");

    // Links stored blank, listed.
    let c = public_copy("Ann", "", "", "", "", "they", "Porto", "", r#"{"directory":"listed"}"#);
    assert_eq!((c.socials.as_str(), c.pronouns.as_str(), c.location.as_str()), ("{}", "they", "Porto"), "blank links read as {{}}");

    // A map that is not an object: nothing kept for friends, and nobody listed.
    for map in ["[]", "\"private\"", "not json", ""] {
        assert_eq!(copy(map), withdrawal(), "the map {map:?} lists nobody");
        assert!(unlisted(map) && !kept_for_friends(map, "location"));
    }
}

/// ONLY THE WORD "listed" LISTS A PERSON (BUG-241, 2026-11-03). Someone is in a server's public
/// member directory, and their copy goes to the servers it is linked with, only once they chose
/// it: `"directory": "listed"` in their privacy map, the word both apps write when the box is
/// ticked. Everything else is unlisted and gives the withdrawal: no `directory` key (they never
/// chose, or a profile saved before this fix, which both apps wrote by taking the key out), the
/// word `"unlisted"`, any other word, a value that is not a word, an empty map, and a map that is
/// not an object. Safe by default (CLAUDE.md, 2026-10-09: "people open up what they want").
///
/// Seen red 2026-11-03 against the old rule (only `"unlisted"` withdrew, so no key listed; `listed`
/// put in as its opposite to compile): "never chose: no key in the map" failed, left
/// `PublicCopy { name: "Ann", bio: "hi", avatar_url: "https://ann.example/a.png", ... }`, right
/// `PublicCopy { name: "", bio: "", avatar_url: "", banner_url: "", socials: "{}", ... }`.
#[test]
fn only_the_word_listed_lists_a_person() {
    let copy = |privacy: &str| {
        public_copy("Ann", "hi", "https://ann.example/a.png", "", r#"{"github":"ann"}"#, "she/her", "Lisbon", "https://ann.example", privacy)
    };
    assert_eq!(DIRECTORY_LISTED, "listed", "the word both apps write");
    assert_eq!(DIRECTORY_UNLISTED, "unlisted", "the word both apps write when the box is unticked");
    let listed_copy = copy(r#"{"directory":"listed"}"#);
    assert_eq!(listed_copy.name, "Ann", "the word \"listed\" lists");
    assert!(listed(r#"{"directory":"listed","location":"private"}"#) && !unlisted(r#"{"directory":"listed"}"#));

    for (map, why) in [
        (r#"{"location":"private"}"#, "never chose: no key in the map"),
        ("{}", "never chose: an empty map"),
        (r#"{"directory":"unlisted"}"#, "chose unlisted"),
        (r#"{"directory":"Listed"}"#, "another spelling of the word"),
        (r#"{"directory":"yes"}"#, "any other word"),
        (r#"{"directory":true}"#, "a value that is not a word"),
        (r#"{"directory":["listed"]}"#, "a list holding the word"),
        ("[]", "a map that is not an object"),
        ("not json", "text that is not JSON"),
        ("", "nothing at all"),
    ] {
        assert_eq!(copy(map), withdrawal(), "{why}");
        assert!(unlisted(map) && !listed(map), "{why}: unlisted");
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
///
/// Since BUG-241 (2026-11-03) a map without `"directory": "listed"` is unlisted, so a person who
/// was listed before (no key: choosing listed took it out) and connects with a copy out there is
/// asked for the withdrawal; the last two rows hold that.
#[test]
fn the_relay_asks_for_a_copy_only_when_one_is_due() {
    let listed = r#"{"directory":"listed"}"#;
    let loc_private = r#"{"directory":"listed","location":"private"}"#;
    let unlisted_map = r#"{"directory":"unlisted"}"#;
    let never_chose = "{}";
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
    assert!(exposes(&full, never_chose), "a person who never chose is not listed, so a listed copy out there exposes them");

    let rows: [(Option<&PublicCopy>, &PublicCopy, bool, &str, bool, &str); 17] = [
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
        (Some(&full), &w, false, never_chose, true, "a connect with no directory word and a copy out there withdraws it"),
        (None, &w, true, never_chose, false, "a first save that never chose sends nothing out"),
    ];
    for (held, now, just_saved, privacy, want, why) in rows {
        assert_eq!(sign_wanted(held, now, just_saved, privacy), want, "{why}");
    }
}
