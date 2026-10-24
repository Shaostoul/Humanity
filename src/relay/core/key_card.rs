//! Key cards (2026-10-22, docs/design/key-cards.md): a small object, signed by a person's own
//! Dilithium3 (ML-DSA-65) identity key, that says which Kyber768 (ML-KEM-768) key to seal things
//! to for them. Any app can check a card whatever server delivered it, so a server can no longer
//! hand out a key of its own for someone and read what is sealed to it.
//!
//! One function per rule, used by the relay (handlers/key_cards.rs, storage/key_cards.rs, the
//! federation link) and by the desktop app (`native` includes `relay`; engine/key_cards.rs and
//! net/key_pins.rs). The website's twin is web/shared/key-card.js; the shared fixture
//! scripts/tests/fixtures/key-card-v1.json, written by `write_the_key_card_fixture` in
//! key_card_tests.rs from fixed seeds, holds the two to the same bytes.
//!
//! THE CARD, as it travels (JSON): `{"v":1,"key":K,"kyber":Y,"at":T,"sig":S}`
//! - `key`: the person's Dilithium3 public key in lowercase hex (3,904 characters): who they are;
//! - `kyber`: their Kyber768 encapsulation key in standard base64 with padding (1,580 characters);
//! - `at`: when it was signed, Unix milliseconds;
//! - `sig`: the Dilithium3 signature by `key` over [`card_text`], standard base64 (4,412 characters).
//!
//! No server is named in a card: it is good on every server, which is the point.
//!
//! ONE SPELLING. Both encodings must be the canonical one (decoding and encoding again gives the
//! same text) and the key lowercase, so a card has exactly one text and the words signed cannot be
//! spelled two ways.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::pq_crypto::{
    derive_dilithium_seed, derive_kyber_seed, verify_dilithium, DilithiumKeypair, KyberKeypair, DILITHIUM_PK_LEN,
    DILITHIUM_SIG_LEN, KYBER_EK_LEN,
};

/// The tag that opens the signed words.
pub const CARD_TAG: &str = "hum/key-card/v1";
/// The card's version, its `v`.
pub const CARD_VERSION: u64 = 1;
/// A person's key in a card: a Dilithium3 public key in lowercase hex.
pub const KEY_HEX_LEN: usize = DILITHIUM_PK_LEN * 2;
/// A Kyber768 key in standard base64 with padding: 4 characters for every 3 bytes, rounded up.
pub const KYBER_B64_LEN: usize = KYBER_EK_LEN.div_ceil(3) * 4;
/// A card's signature in standard base64 with padding.
pub const SIG_B64_LEN: usize = DILITHIUM_SIG_LEN.div_ceil(3) * 4;
/// How far ahead of a server's clock a card may be dated and still be kept: a day, for clocks
/// that drift (profile copies' rule, `profile_copy::MAX_AHEAD_MS`). Without an upper bound a card
/// dated far ahead would win over every later key.
pub const MAX_AHEAD_MS: u64 = 24 * 60 * 60 * 1000;
/// The most cards one `key_card_request` may ask for (and one `key_cards` answer carries).
pub const REQUEST_MAX: usize = 16;

/// A key card. Field order is the order of the JSON both apps write.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCard {
    pub v: u64,
    pub key: String,
    pub kyber: String,
    pub at: u64,
    pub sig: String,
}

/// The words a card's signature covers, as UTF-8 bytes: `hum/key-card/v1\n{key}\n{kyber}\n{at}`,
/// `at` in decimal milliseconds. No field can hold a line break (hex and base64), so the words
/// split one way only.
pub fn card_text(key: &str, kyber: &str, at: u64) -> String {
    format!("{CARD_TAG}\n{key}\n{kyber}\n{at}")
}

/// Sign a card for the identity of `seed32` (the person's 32-byte recovery seed): its Dilithium3
/// key and its Kyber768 key are both derived from it, as for every other key they use, dated
/// `at`. ML-DSA signing here is deterministic, which is what lets the shared fixture pin it.
pub fn sign_card(seed32: &[u8], at: u64) -> Result<KeyCard, String> {
    let dil = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed32));
    let kyber = KyberKeypair::from_seed(&derive_kyber_seed(seed32)).map_err(|e| format!("Kyber768 key: {e}"))?;
    Ok(sign_card_with(&dil, &kyber.public_key(), at))
}

/// Sign a card binding `dil`'s identity to `kyber_public` (raw bytes), dated `at`. The tests use
/// it to make cards for keys that do not come from one seed (a substituted key, a wrong signer).
pub fn sign_card_with(dil: &DilithiumKeypair, kyber_public: &[u8], at: u64) -> KeyCard {
    let key = hex::encode(dil.public_key());
    let kyber = B64.encode(kyber_public);
    let sig = B64.encode(dil.sign(card_text(&key, &kyber, at).as_bytes()));
    KeyCard { v: CARD_VERSION, key, kyber, at, sig }
}

fn is_lower_hex(s: &str) -> bool {
    s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `text` decoded as standard base64 to exactly `len` bytes, and only when it is the canonical
/// spelling of them (encoding them again gives `text`).
fn canonical_b64(text: &str, len: usize) -> Option<Vec<u8>> {
    let bytes = B64.decode(text).ok()?;
    (bytes.len() == len && B64.encode(&bytes) == text).then_some(bytes)
}

/// Is `key` a person's key as cards carry it: 3,904 lowercase hex characters.
pub fn is_key_text(key: &str) -> bool {
    key.len() == KEY_HEX_LEN && is_lower_hex(key)
}

/// Does the card have the one shape a card may have: version 1, a key as [`is_key_text`], a
/// Kyber768 key and a signature in canonical base64 of the right lengths, and a time after the
/// epoch. Checks no signature.
pub fn shape_ok(card: &KeyCard) -> bool {
    card.v == CARD_VERSION
        && is_key_text(&card.key)
        && card.at > 0
        && card.kyber.len() == KYBER_B64_LEN
        && card.sig.len() == SIG_B64_LEN
        && canonical_b64(&card.kyber, KYBER_EK_LEN).is_some()
        && canonical_b64(&card.sig, DILITHIUM_SIG_LEN).is_some()
}

/// Is this a good card: the right shape, and signed by the very key it names. CPU-bound (one
/// Dilithium3 verify): the relay runs it off the async pool.
pub fn verify_card(card: &KeyCard) -> bool {
    if !shape_ok(card) {
        return false;
    }
    let (Ok(key), Some(sig)) = (hex::decode(&card.key), canonical_b64(&card.sig, DILITHIUM_SIG_LEN)) else {
        return false;
    };
    verify_dilithium(&key, card_text(&card.key, &card.kyber, card.at).as_bytes(), &sig).is_ok()
}

/// Is this a good card FOR `person` (their key, lowercase hex): what an app checks before it
/// seals anything to them with the card's key. A good card of somebody else is not one.
pub fn card_for(card: &KeyCard, person: &str) -> bool {
    card.key == person && verify_card(card)
}

/// Read a card from JSON: an object with exactly the fields of a card, of the right types. Any
/// other field, or one missing, and it is not a card (so nothing unsigned can ride along).
pub fn parse_card(v: &Value) -> Option<KeyCard> {
    let obj = v.as_object()?;
    if obj.len() != 5 {
        return None;
    }
    Some(KeyCard {
        v: obj.get("v")?.as_u64()?,
        key: obj.get("key")?.as_str()?.to_string(),
        kyber: obj.get("kyber")?.as_str()?.to_string(),
        at: obj.get("at")?.as_u64()?,
        sig: obj.get("sig")?.as_str()?.to_string(),
    })
}

/// The card as JSON, in the field order both apps write.
pub fn card_json(card: &KeyCard) -> Value {
    serde_json::json!({ "v": card.v, "key": card.key, "kyber": card.kyber, "at": card.at, "sig": card.sig })
}

/// The Kyber768 key a card carries, as raw bytes (None for a card of the wrong shape).
pub fn kyber_bytes(card: &KeyCard) -> Option<Vec<u8>> {
    canonical_b64(&card.kyber, KYBER_EK_LEN)
}

// ── What a server keeps ──

/// What a server does with a good card for someone, given the card it holds for them (its
/// Kyber768 key and time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keep {
    /// Nothing held: keep it.
    New,
    /// A different key, newer: it replaces the one held.
    Replace,
    /// The same key as the one held: nothing changes, and the time stays the one first held, so
    /// other apps do not fetch a card again just because someone signed in again.
    SameKey,
    /// A different key, not newer than the one held: refused, so an old card sent again cannot
    /// undo a newer one.
    Older,
    /// Dated more than [`MAX_AHEAD_MS`] ahead of the server's clock: refused.
    TooFarAhead,
}

/// The keep rule, one place for every way a card arrives (a sign-in, a linked server).
pub fn keep_rule(held: Option<(&str, u64)>, card: &KeyCard, now_ms: u64) -> Keep {
    if now_ms > 0 && card.at > now_ms.saturating_add(MAX_AHEAD_MS) {
        return Keep::TooFarAhead;
    }
    match held {
        None => Keep::New,
        Some((kyber, _)) if kyber == card.kyber => Keep::SameKey,
        Some((_, at)) if card.at > at => Keep::Replace,
        Some(_) => Keep::Older,
    }
}

impl Keep {
    /// Was the card stored (and so passed on to linked servers).
    pub fn stored(self) -> bool {
        matches!(self, Keep::New | Keep::Replace)
    }
}

// ── What an app remembers (pinning) ──

/// What a good card for someone means to an app, given the key it pinned for them (the key it
/// first accepted, and that card's time).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Seen {
    /// Nothing pinned yet: pin this one.
    First,
    /// The key pinned: nothing changes (a newer time is noted, so the app does not fetch again).
    Same,
    /// A different key, newer than the pinned one: the person's key changed. Nothing is sealed to
    /// them until the person using the app accepts the new key.
    Changed,
    /// A different key, not newer: an old card sent again. Ignored; the pinned key stays.
    Stale,
}

/// The pin rule, the same in both apps (web/shared/key-card.js `compare`).
pub fn compare(pin: Option<(&str, u64)>, card_kyber: &str, card_at: u64) -> Seen {
    match pin {
        None => Seen::First,
        Some((kyber, _)) if kyber == card_kyber => Seen::Same,
        Some((_, at)) if card_at > at => Seen::Changed,
        Some(_) => Seen::Stale,
    }
}

impl Seen {
    /// The word the shared fixture uses for it.
    pub fn word(self) -> &'static str {
        match self {
            Seen::First => "first",
            Seen::Same => "same",
            Seen::Changed => "changed",
            Seen::Stale => "stale",
        }
    }
}

// ── What a person reads ──

/// Why nothing can be sealed to someone, in the words both apps show. `name` is how the app
/// names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The server says they have no key card, or sent one that did not check out.
    NoCard,
    /// Their card has been asked for and has not arrived yet.
    Fetching,
    /// Their key changed and the new key has not been accepted here.
    Changed,
}

impl Refusal {
    /// The sentence. Both apps hold these words (key-card.js `refusalText`); the fixture pins them.
    pub fn text(self, name: &str) -> String {
        match self {
            Refusal::NoCard => format!(
                "Nothing was sent: {name} has no signed key card on this server, so there is no key we can check is really theirs. It appears once they sign in with a current version of the app."
            ),
            Refusal::Fetching => format!("Nothing was sent: {name}'s key card is still on its way from the server. Try again in a moment."),
            Refusal::Changed => format!(
                "Nothing was sent: {name}'s encryption key changed. Accept the new key in your conversation with them first."
            ),
        }
    }
}

/// The notice an app shows in a conversation with someone whose key changed, above its "Accept
/// the new key" button. The fixture pins it.
pub fn change_notice(name: &str) -> String {
    format!(
        "{name}'s encryption key changed. This happens when someone resets their keys. If you did not expect it, check with {name} in person or on a call you trust before you accept. Until you do, nothing is sent to them."
    )
}

/// The button under [`change_notice`].
pub const ACCEPT_LABEL: &str = "Accept the new key";

/// The pinned words, kept in THIS file because the website's test (scripts/tests/key-card-web.test.js)
/// reads the expected literal out of it and holds web/shared/key-card.js `cardText` to it.
#[cfg(test)]
mod pinned {
    use super::*;

    /// THE SIGNED WORDS ARE PINNED: the tag, the field order, one `\n` between fields, `at` in
    /// decimal milliseconds.
    /// Seen red 2026-10-23 with `kyber` and `key` swapped in `card_text`: left
    /// `"hum/key-card/v1\nS3k=\nab12\n1760000000001"`, right `"hum/key-card/v1\nab12\nS3k=\n1760000000001"`.
    #[test]
    fn the_card_text_is_pinned() {
        assert_eq!(card_text("ab12", "S3k=", 1_760_000_000_001), "hum/key-card/v1\nab12\nS3k=\n1760000000001");
    }
}

#[cfg(test)]
#[path = "key_card_tests.rs"]
mod tests;
