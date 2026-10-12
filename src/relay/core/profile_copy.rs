//! The copy of a person's profile that other servers keep (BUG-191, 2026-10-12,
//! docs/design/bug-191-profile-copies-plan.md sections 3.1 to 3.4): what it may hold, the words
//! its owner's app signs, the limits the sending and the receiving server both hold it to, when a
//! server asks a device for one, and the time on it.
//!
//! One function per rule, used by the relay (handlers/profile.rs, handlers/federation.rs and
//! storage/signed_profiles.rs) and by the desktop app (`native` includes `relay`). The website's
//! twin is web/shared/profile-copy.js; the shared fixture scripts/tests/fixtures/profile-copy-v1.json,
//! written by `write_the_profile_copy_fixture` in profile_copy_tests.rs from fixed seeds, holds the
//! two to the same bytes.
//!
//! THE PRIVACY RULE. The copy is the profile as anyone may see it:
//! - name, bio, avatar, banner and links always;
//! - pronouns, location and website blank when the person keeps them for friends;
//! - every field blank (a "withdrawal") when the person is unlisted.
//!
//! A field kept for friends never leaves the server, signed or not. `accepts_files` is never in
//! the copy: it is this server's setting.
//!
//! WHY THE APP SIGNS AFTER THE SAVE, not inside it: the relay cleans the bio and trims the other
//! fields, and an app would have to repeat that byte for byte to sign the same words. Instead the
//! relay sends each of the person's devices its own complete copy of what it stored, and a device
//! that is asked (`sign_now`) signs the public copy of exactly that.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::pq_crypto::{derive_dilithium_seed, verify_dilithium, DilithiumKeypair, DILITHIUM_PK_LEN, DILITHIUM_SIG_LEN};

/// The fields a person may keep for friends: `"private"` under the field's name in their privacy
/// map. Blank in the copy for other servers when kept.
pub const FIELDS_FOR_FRIENDS: [&str; 3] = ["pronouns", "location", "website"];

/// The tag that opens the signed words. Nothing was ever signed under it before this fix (no app
/// signed a profile), so it stays and nothing needs a shim.
pub const COPY_TAG: &str = "profile_v1";

/// The longest bio, in characters (after the relay's clean-up), the same on both apps and both
/// servers. Was `msg_handlers::BIO_MAX_CHARS` until 2026-10-12.
pub const BIO_MAX_CHARS: usize = 280;
/// The longest name, in bytes (names are ASCII: letters, digits, `_` and `-`).
pub const NAME_MAX_BYTES: usize = 24;
/// The links (a JSON object), in bytes.
pub const LINKS_MAX_BYTES: usize = 1_024;
/// An avatar or banner link, in bytes.
pub const PICTURE_LINK_MAX_BYTES: usize = 512;
/// The website link, in bytes.
pub const WEBSITE_MAX_BYTES: usize = 256;
/// Pronouns, in characters.
pub const PRONOUNS_MAX_CHARS: usize = 64;
/// Location, in characters.
pub const LOCATION_MAX_CHARS: usize = 128;

/// A person's key as it appears in a copy: a Dilithium3 public key in lowercase hex.
pub const KEY_HEX_LEN: usize = DILITHIUM_PK_LEN * 2;
/// A copy's signature: a Dilithium3 signature in lowercase hex.
pub const SIGNATURE_HEX_LEN: usize = DILITHIUM_SIG_LEN * 2;

/// How far ahead of a server's clock a copy may be dated and still be kept: a day, for clocks that
/// drift. Without an upper bound a copy dated far ahead would win every later save until the wall
/// clock caught up (the audit of 2026-06-12).
pub const MAX_AHEAD_MS: u64 = 24 * 60 * 60 * 1000;

/// The profile as anyone may see it, exactly as its owner's app signs it. Field order is the order
/// of the signed words.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicCopy {
    /// This server's display name for the person. Empty only in a withdrawal.
    pub name: String,
    pub bio: String,
    pub avatar_url: String,
    pub banner_url: String,
    /// The links, as the JSON text stored (never re-serialized: key order is part of the words).
    pub socials: String,
    pub pronouns: String,
    pub location: String,
    pub website: String,
}

// ── Reading the privacy map (one rule, both sides) ──

/// The privacy map as an object, or None when the text is not a JSON object (then nothing is kept
/// for friends and the person is listed).
fn map_of(privacy: &str) -> Option<serde_json::Map<String, Value>> {
    match serde_json::from_str::<Value>(privacy) {
        Ok(Value::Object(map)) => Some(map),
        _ => None,
    }
}

/// Is the person unlisted: `map["directory"]` is the string `"unlisted"`.
pub fn unlisted(privacy: &str) -> bool {
    map_of(privacy).is_some_and(|m| m.get("directory").and_then(Value::as_str) == Some("unlisted"))
}

/// Does the person keep `field` for friends: `map[field]` is the string `"private"`. Any other
/// value, a value that is not a string, or a map that is not an object leaves it public. (The
/// relay's older reading parsed the map as words only, so ONE value that was not a string made
/// every field public.)
pub fn kept_for_friends(privacy: &str, field: &str) -> bool {
    map_of(privacy).is_some_and(|m| m.get(field).and_then(Value::as_str) == Some("private"))
}

// ── The copy ──

/// The withdrawal: what an unlisted person's copy is. Every field empty, the name included, links
/// `"{}"`. It replaces every copy already passed on.
pub fn withdrawal() -> PublicCopy {
    PublicCopy { socials: "{}".to_string(), ..PublicCopy::default() }
}

/// Is this copy a withdrawal. A real name is never empty (the relay refuses an empty name at sign
/// in), so the empty name is the mark.
pub fn is_withdrawal(c: &PublicCopy) -> bool {
    c.name.is_empty()
}

/// The copy for other servers of what this server stored: unlisted gives the withdrawal;
/// otherwise the exact stored text of every field, links blank read as `"{}"`, and each of
/// [`FIELDS_FOR_FRIENDS`] blank when the person keeps it for friends. No trimming and no
/// reformatting: the input is what the server stored, which the person's own copy carries.
#[allow(clippy::too_many_arguments)]
pub fn public_copy(
    name: &str,
    bio: &str,
    avatar_url: &str,
    banner_url: &str,
    socials: &str,
    pronouns: &str,
    location: &str,
    website: &str,
    privacy: &str,
) -> PublicCopy {
    if unlisted(privacy) {
        return withdrawal();
    }
    let shown = |field: &str, value: &str| if kept_for_friends(privacy, field) { String::new() } else { value.to_string() };
    PublicCopy {
        name: name.to_string(),
        bio: bio.to_string(),
        avatar_url: avatar_url.to_string(),
        banner_url: banner_url.to_string(),
        socials: if socials.is_empty() { "{}".to_string() } else { socials.to_string() },
        pronouns: shown("pronouns", pronouns),
        location: shown("location", location),
        website: shown("website", website),
    }
}

/// The value of one of [`FIELDS_FOR_FRIENDS`] in a copy.
fn field_of<'a>(c: &'a PublicCopy, field: &str) -> &'a str {
    match field {
        "pronouns" => &c.pronouns,
        "location" => &c.location,
        "website" => &c.website,
        _ => "",
    }
}

// ── The words signed ──

/// The words a copy's signature covers, as UTF-8 bytes:
/// `profile_v1\n{public_key}\n{name}\n{bio}\n{avatar_url}\n{banner_url}\n{socials}\n{pronouns}\n{location}\n{website}\n{at}`,
/// `at` in decimal milliseconds. Not ambiguous: no field but the bio may hold a line break
/// ([`within_limits`]), so the words split one way only (the last eight lines and the two after
/// the tag are fixed; the bio is the rest). Was federation.rs `canonical_profile_message`.
pub fn copy_text(public_key: &str, c: &PublicCopy, at: u64) -> String {
    format!(
        "{COPY_TAG}\n{public_key}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{at}",
        c.name, c.bio, c.avatar_url, c.banner_url, c.socials, c.pronouns, c.location, c.website
    )
}

/// Sign a copy with the identity of `seed32` (the person's 32-byte recovery seed, from which the
/// Dilithium3 key is derived as for every other signature they make): lowercase hex, 6,618
/// characters. ML-DSA signing here is deterministic, which is what lets the shared fixture pin it.
pub fn sign_copy(seed32: &[u8], public_key: &str, c: &PublicCopy, at: u64) -> String {
    let keypair = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed32));
    hex::encode(keypair.sign(copy_text(public_key, c, at).as_bytes()))
}

/// Is `key` a person's key as copies carry it: 3,904 LOWERCASE hex characters. One spelling only,
/// because `signed_profiles` is keyed by the text: an upper-case spelling of the same key would be
/// a second row for the same person.
pub fn is_key_text(key: &str) -> bool {
    key.len() == KEY_HEX_LEN && is_lower_hex(key)
}

/// Is `sig` a copy's signature as it travels: 6,618 lowercase hex characters.
pub fn is_signature_text(sig: &str) -> bool {
    sig.len() == SIGNATURE_HEX_LEN && is_lower_hex(sig)
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Does `signature` sign this copy of `public_key`'s profile, dated `at`. Checks the signature's
/// shape and the Dilithium3 signature only, not the limits ([`within_limits`] is separate, so a
/// receiver can say which failed). CPU-bound: the relay runs it off the async pool.
pub fn verify_copy(public_key: &str, c: &PublicCopy, at: u64, signature: &str) -> bool {
    if !is_signature_text(signature) {
        return false;
    }
    let (Ok(key), Ok(sig)) = (hex::decode(public_key), hex::decode(signature)) else {
        return false;
    };
    verify_dilithium(&key, copy_text(public_key, c, at).as_bytes(), &sig).is_ok()
}

// ── The limits ──

/// Does a copy keep the save's own limits, with `public_key` a key as copies carry it. Checked by
/// the server that asks for a copy and by every server that receives one: a copy a save would
/// refuse is never kept.
pub fn within_limits(public_key: &str, c: &PublicCopy) -> bool {
    is_key_text(public_key) && fields_within_limits(c)
}

/// The field half of [`within_limits`]:
/// - the name is empty (a withdrawal) or 1 to 24 of `A-Z a-z 0-9 _ -`;
/// - a withdrawal has every other field empty and links `"{}"`;
/// - the bio is at most 280 characters and holds no `<` or `>` (the save removes both);
/// - links are a JSON object of at most 1,024 bytes;
/// - avatar and banner at most 512 bytes, website at most 256, each empty or `https://...`;
/// - pronouns at most 64 characters, location at most 128;
/// - no line break (`\n`) in any field but the bio.
pub fn fields_within_limits(c: &PublicCopy) -> bool {
    if is_withdrawal(c) {
        return *c == withdrawal();
    }
    let name_ok = c.name.len() <= NAME_MAX_BYTES
        && c.name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    let bio_ok = c.bio.chars().count() <= BIO_MAX_CHARS && !c.bio.contains(['<', '>']);
    let links_ok = c.socials.len() <= LINKS_MAX_BYTES
        && matches!(serde_json::from_str::<Value>(&c.socials), Ok(Value::Object(_)));
    let link_ok = |s: &str, max: usize| s.len() <= max && (s.is_empty() || s.starts_with("https://"));
    let links_of_pictures_ok = link_ok(&c.avatar_url, PICTURE_LINK_MAX_BYTES)
        && link_ok(&c.banner_url, PICTURE_LINK_MAX_BYTES)
        && link_ok(&c.website, WEBSITE_MAX_BYTES);
    let words_ok = c.pronouns.chars().count() <= PRONOUNS_MAX_CHARS && c.location.chars().count() <= LOCATION_MAX_CHARS;
    let one_line = [&c.name, &c.avatar_url, &c.banner_url, &c.socials, &c.pronouns, &c.location, &c.website]
        .iter()
        .all(|s| !s.contains('\n'));
    name_ok && bio_ok && links_ok && links_of_pictures_ok && words_ok && one_line
}

// ── Asking ──

/// Does the copy held here show more than the person allows here now: it is not a withdrawal, and
/// either the person is unlisted here or a field they keep for friends here is not blank in it.
pub fn exposes(held: &PublicCopy, privacy: &str) -> bool {
    if is_withdrawal(held) {
        return false;
    }
    unlisted(privacy)
        || FIELDS_FOR_FRIENDS.iter().any(|f| kept_for_friends(privacy, f) && !field_of(held, f).is_empty())
}

/// Should this server ask a device to sign its public copy now. `held` is the copy this server
/// holds for the device's key (from any server), `now` the public copy of what is stored here,
/// `just_saved` whether this is the echo of a save. Never when `now` is over the limits. Otherwise:
/// - no copy held here: only after a save, and never to send a withdrawal nobody needs;
/// - `now` is a withdrawal: when the held copy is not one;
/// - just saved: when the held copy differs from `now`;
/// - otherwise (a connect, or the person's own request): only when the held copy [`exposes`] more
///   than allowed here, so a device that connects never pushes an older profile over a newer save
///   made on another server, but always narrows what is out there.
pub fn sign_wanted(held: Option<&PublicCopy>, now: &PublicCopy, just_saved: bool, privacy: &str) -> bool {
    if !fields_within_limits(now) {
        return false;
    }
    match held {
        None => just_saved && !is_withdrawal(now),
        Some(h) if is_withdrawal(now) => !is_withdrawal(h),
        Some(h) if just_saved => h != now,
        Some(h) => exposes(h, privacy),
    }
}

// ── The time on a copy ──

/// The time a device puts on the next copy it signs for a server: after the clock, after the copy
/// that server holds (`signed_at`, from the own copy; 0 for none) and after the last one this
/// device signed for it in this run (`last_here`; 0 for none). Saturating. So a device whose clock
/// runs fast no longer wins every later edit: the next edit from any device is dated after it (the
/// friendship choice notes' rule, src/net/dm_store.rs).
pub fn next_copy_time(now_ms: u64, signed_at: u64, last_here: u64) -> u64 {
    now_ms.max(signed_at.saturating_add(1)).max(last_here.saturating_add(1))
}

/// The pinned words, kept in THIS file because the website's test (scripts/tests/
/// profile-copy-web.test.js, the friend-pass.test.js way) reads `PINNED_COPY` and the expected
/// literal out of it and holds web/shared/profile-copy.js `copyText` to them.
#[cfg(test)]
mod pinned {
    use super::*;

    /// The copy the pin signs: a bio with a line break and a letter outside ASCII, links in an
    /// order a re-serializer would change, a field left blank.
    const PINNED_COPY: &str = r#"{"name":"Ann","bio":"Γεια!\nline two","avatar_url":"https://ann.example/a.png","banner_url":"","socials":"{\"github\":\"ann\",\"discord\":\"ann#1\"}","pronouns":"she/her","location":"Lisbon","website":"https://ann.example"}"#;

    /// THE SIGNED WORDS ARE PINNED: the field order, the tag, one `\n` between fields, the bio's
    /// own line break kept, the links as stored, `at` in decimal milliseconds.
    /// Seen red 2026-10-12 with location and website swapped in `copy_text`: left
    /// `"profile_v1\nab12\nAnn\nΓεια!\nline two\nhttps://ann.example/a.png\n\n{\"github\":\"ann\",\"discord\":\"ann#1\"}\nshe/her\nhttps://ann.example\nLisbon\n1760000000001"`,
    /// right `"...\nshe/her\nLisbon\nhttps://ann.example\n1760000000001"`.
    #[test]
    fn the_copy_text_is_pinned() {
        let c: PublicCopy = serde_json::from_str(PINNED_COPY).expect("the pinned copy is JSON");
        assert_eq!(
            copy_text("ab12", &c, 1_760_000_000_001),
            "profile_v1\nab12\nAnn\nΓεια!\nline two\nhttps://ann.example/a.png\n\n{\"github\":\"ann\",\"discord\":\"ann#1\"}\nshe/her\nLisbon\nhttps://ann.example\n1760000000001"
        );
    }
}

#[cfg(test)]
#[path = "profile_copy_tests.rs"]
mod tests;
