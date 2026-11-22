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

// ── A key's fingerprint (2026-11-20, the owner branch's second review, W1) ──
//
// A relay chooses which members it lists and under what names, so when the owner hands the role
// over or names a successor, the person picked is told apart by their KEY, not their name. EVERY
// handover and EVERY naming of a successor goes ahead only once the groups of the listed key's
// fingerprint at FOUR positions this device draws at random ([`confirm_positions`]) match what the
// owner typed, and what the owner types comes from THE OTHER PERSON'S OWN SCREEN: each person sees
// their own key's fingerprint, in words, in Settings > Account in both apps and on the check prompt
// when they are named, and the owner's picker names only the POSITIONS ("ask them to read you
// groups 2, 3, 5 and 6 of their own fingerprint"). The listed key's fingerprint is NOT on the
// owner's screen until what was typed matched it (the owner branch's fifth review, H1, 2026-11-21:
// the picker used to show the listed key's whole fingerprint and told the owner to read it TO the
// person, so the typed groups were copied off the owner's own screen and a relay listing its own
// key as "Bea" passed with no grinding at all). A typed set that does not match is a warning in
// words, logged, and that key cannot be confirmed again in that session, so a wrong set never
// reads like an empty field and a ground key gets one try ([`judge_at`]). There is no exemption:
// not a friend given a friendship pass (the fourth review's finding 1, 2026-11-21: a mutual follow
// is one "Follow back" on a name the relay listed, and both apps then send passes by themselves),
// not a key card pinned (the third review's W1), and not a key confirmed earlier. The website's
// twin is web/shared/key-card.js `fingerprint`, `confirmPositions` and `judgeAt`;
// `the_fingerprint_is_pinned` below and scripts/tests/key-card-web.test.js hold the two to the
// same known answers.
//
// WHY FOUR GROUPS AT RANDOM POSITIONS (the third review's M2; the arithmetic corrected by the fourth
// review's finding 5 and recomputed for the fifth review's H1, 2026-11-21). A grinder does not make
// a whole ML-DSA-65 key pair per candidate: it keeps the public matrix and the secret s2 fixed and
// steps one coefficient of s1, so each candidate is one column of the matrix added into t and a
// BLAKE3 hash of the 1,952-byte public key, about 1 to 2 microseconds on one core, and every
// candidate is still a real key pair that can sign its card. Matching g chosen groups (16 bits
// each) of the real person's fingerprint takes about 2^(16g) candidates: three (2^48) about 10^5
// core-hours, four (2^64) about 5 to 10 billion core-hours (600,000 to 1.2 million core-years).
// The four positions are drawn uniformly from the 15 possible sets of four of the six, afresh for
// each ask, on the owner's device (the relay never learns them before the person reads them out).
// The person reads their REAL groups at those positions; the app compares them with the LISTED
// key's. So a key ground to match g of the real person's groups passes an ask only when all four
// drawn are among its g: C(g,4)/15, which is 0 for three or fewer, 1/15 for four (2^64 work), 1/3
// for five (2^80) and 1 only for all six (2^96). And it gets ONE ask: a mismatch blocks the key for
// the session with a warning that the person listed may not be who the owner thinks, so a grinder
// cannot wait for a fresh draw that suits it (the round-5 rule never asked the same set twice
// running, which only narrowed the grinder's next draw; it is gone). Without grinding, a key passes
// an ask with probability 2^-64. The typed groups ARE the comparison now: they come from the other
// person's own screen, and nothing on the owner's screen gives them away before they match.

/// The context label a fingerprint is derived under (BLAKE3 `derive_key`).
pub const FINGERPRINT_DOMAIN: &str = "hum/key-fingerprint/v1";
/// How many groups of four hex digits a fingerprint has.
pub const FINGERPRINT_GROUPS: usize = 6;

/// The fingerprint of the Dilithium3 public key `key_hex` (lowercase hex): BLAKE3
/// `derive_key(`[`FINGERPRINT_DOMAIN`]`, the key's bytes)`, its first 12 bytes as six groups of four
/// hex digits separated by spaces ("3fa2 91c0 77de 0b4e 5a19 c2d0"). Empty for text that is not a
/// key in hex.
pub fn fingerprint(key_hex: &str) -> String {
    let Ok(bytes) = hex::decode(key_hex) else { return String::new() };
    if bytes.is_empty() {
        return String::new();
    }
    let digest = blake3::derive_key(FINGERPRINT_DOMAIN, &bytes);
    hex::encode(&digest[..FINGERPRINT_GROUPS * 2]).as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join(" ")
}

/// How many groups of a fingerprint the owner types for every handover and every naming of a
/// successor: four, at random positions (the module's "WHY FOUR GROUPS AT RANDOM POSITIONS").
pub const CONFIRM_GROUPS: usize = 4;

/// The group positions one ask names (0-based, ascending, all different).
pub type Asked = [usize; CONFIRM_GROUPS];

/// Every set of [`CONFIRM_GROUPS`] different group positions of the [`FINGERPRINT_GROUPS`], each
/// ascending (15 for four of six).
pub fn confirm_sets() -> Vec<Asked> {
    (0u32..1 << FINGERPRINT_GROUPS)
        .filter(|m| m.count_ones() as usize == CONFIRM_GROUPS)
        .map(|m| {
            let mut set = [0; CONFIRM_GROUPS];
            for (slot, pos) in (0..FINGERPRINT_GROUPS).filter(|p| m & (1 << p) != 0).enumerate() {
                set[slot] = pos;
            }
            set
        })
        .collect()
}

/// [`CONFIRM_GROUPS`] different group positions drawn for one ask, uniformly from all 15 sets of
/// [`confirm_sets`], whatever was asked before (the fifth review's H1: excluding the set asked last
/// only narrowed a grinder's next draw). `random` gives uniformly random numbers (the operating
/// system's generator in the apps, [`random_u32`]; a fixed sequence in tests).
pub fn confirm_positions_with(mut random: impl FnMut() -> u32) -> Asked {
    let sets = confirm_sets();
    // Rejection sampling, so every set is equally likely (no modulo bias): the zone is the largest
    // multiple of 15 that fits in 2^32.
    let n = sets.len() as u64;
    let zone = (1u64 << 32) - (1u64 << 32) % n;
    loop {
        let r = random() as u64;
        if r < zone {
            return sets[(r % n) as usize];
        }
    }
}

/// [`confirm_positions_with`] from the operating system's generator.
pub fn confirm_positions() -> Asked {
    confirm_positions_with(random_u32)
}

/// A uniformly random number from the operating system's generator.
pub fn random_u32() -> u32 {
    let mut b = [0u8; 4];
    // The OS generator does not fail on the platforms the apps run on; were it to, every ask would
    // draw the same set, which is still four groups to compare (never fewer).
    let _ = getrandom::getrandom(&mut b);
    u32::from_le_bytes(b)
}

/// The asked positions counted from one, as the words name them ("2", "3", "5", "6").
pub fn asked_words(positions: Asked) -> [String; CONFIRM_GROUPS] {
    positions.map(|p| (p + 1).to_string())
}

/// The groups of the fingerprint `fp` (as [`fingerprint`] writes it) at `positions`, joined by a
/// space ("91c0 77de 5a19 c2d0"). Empty when a position is past the fingerprint's end or two
/// positions are the same.
pub fn groups_at(fp: &str, positions: Asked) -> String {
    let groups: Vec<&str> = fp.split(' ').filter(|g| !g.is_empty()).collect();
    let repeated = positions.iter().enumerate().any(|(i, p)| positions[..i].contains(p));
    if positions.iter().any(|p| *p >= groups.len()) || repeated {
        return String::new();
    }
    positions.iter().map(|p| groups[*p]).collect::<Vec<_>>().join(" ")
}

/// Whether `typed` is the groups of fingerprint `fp` at `positions`, in order, in any letter case,
/// spaces anywhere ignored ("91c0 77de 5a19 c2d0", "91C077DE5A19C2D0").
pub fn groups_match_at(fp: &str, positions: Asked, typed: &str) -> bool {
    let want: String = groups_at(fp, positions).chars().filter(|c| !c.is_whitespace()).collect();
    let got: String = typed.chars().filter(|c| !c.is_whitespace()).collect();
    !want.is_empty() && got.eq_ignore_ascii_case(&want)
}

/// Whether `typed` confirms the fingerprint of the key `key_hex` at the asked `positions`
/// ([`groups_match_at`] of [`fingerprint`]).
pub fn fingerprint_confirmed_at(key_hex: &str, positions: Asked, typed: &str) -> bool {
    groups_match_at(&fingerprint(key_hex), positions, typed)
}

/// What the owner typed for one ask, judged against the listed key ([`judge_at`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Typed {
    /// Nothing typed yet.
    Empty,
    /// Something typed, but not all four groups (fewer or more than 16 characters, spaces
    /// ignored): no judgement yet, so a slip of the keyboard is not a mismatch.
    Short,
    /// All four groups typed, and they are the listed key's at the positions asked.
    Matched,
    /// All four groups typed (16 characters), and they are NOT the listed key's: the person read
    /// out a different key's groups, so the person listed may not be who the owner thinks. The
    /// apps warn in words, log it and refuse that key for the rest of the session.
    Mismatched,
}

/// Judge `typed` (what the other person read from their own screen) against the fingerprint of the
/// listed key `key_hex` at the asked `positions`.
pub fn judge_at(key_hex: &str, positions: Asked, typed: &str) -> Typed {
    let got: String = typed.chars().filter(|c| !c.is_whitespace()).collect();
    if got.is_empty() {
        Typed::Empty
    } else if got.chars().count() != CONFIRM_GROUPS * 4 {
        Typed::Short
    } else if fingerprint_confirmed_at(key_hex, positions, typed) {
        Typed::Matched
    } else {
        Typed::Mismatched
    }
}

// ── The names a relay lists (the third review's M2) ──

/// The longest name an honest relay lets a person take (relay.rs `identify`).
pub const NAME_MAX: usize = 24;

/// Whether `name` is one an honest relay could list: 1 to [`NAME_MAX`] letters, digits, `_` or
/// `-` (the rule relay.rs `identify` holds every name to). Anything else (spaces, brackets, a
/// fingerprint written into the name) came from a relay that is not following its own rules, so the
/// owner's picker shows it as a refused name in words, never the text itself. A reserved name
/// ([`reserved_name`]) is not honest either: an honest relay refuses it at identify. The website's
/// twin is web/shared/key-card.js `honestName`.
pub fn honest_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= NAME_MAX && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') && !reserved_name(name)
}

/// The names that belong to the app's own voice and the server's (the owner branch's seventh review,
/// H1 (b), 2026-11-21): a member named "System" wrote lines that looked like the app's own. Nobody
/// may take one of these, or a name that folds to one of them ([`fold_name`]). The relay refuses
/// them at identify (relay.rs) and on the bot route (api.rs); the website's twin is
/// web/shared/key-card.js `RESERVED_NAMES`, held to this list by scripts/tests/key-card-web.test.js.
pub const RESERVED_NAMES: &[&str] = &["System", "HumanityOS", "HumanityOSApp", "Humanity", "UnitedHumanity", "FromThisServer", "ThisServer", "Server", "App"];

/// A name folded for the reserved-name check: lower case, `_` and `-` dropped, and the ASCII
/// lookalikes an honest name can hold read as the letter they imitate (0 o, 1 l, i l, 2 z, 3 e, 4 a,
/// 5 s, 6 g, 7 t, 8 b, 9 g, then "rn" m and "vv" w), so "Sy5tem", "SYS_TEM" and "Hurnanity0S" all
/// fold to a reserved name. Only names of the honest set's letters matter: anything else is refused
/// by [`honest_name`] already.
pub fn fold_name(name: &str) -> String {
    let mapped: String = name
        .chars()
        .filter(|c| *c != '_' && *c != '-')
        .map(|c| match c.to_ascii_lowercase() {
            '0' => 'o',
            '1' | 'i' => 'l',
            '2' => 'z',
            '3' => 'e',
            '4' => 'a',
            '5' => 's',
            '6' | '9' => 'g',
            '7' => 't',
            '8' => 'b',
            other => other,
        })
        .collect();
    mapped.replace("rn", "m").replace("vv", "w")
}

/// Whether `name` is one of [`RESERVED_NAMES`], in any case and through any lookalike
/// [`fold_name`] reads.
pub fn reserved_name(name: &str) -> bool {
    let folded = fold_name(name);
    RESERVED_NAMES.iter().any(|r| fold_name(r) == folded)
}

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

    /// THE FINGERPRINT IS PINNED (the owner branch's second review, W1): six groups of four hex
    /// digits of BLAKE3 derive_key("hum/key-fingerprint/v1", key bytes), the same known answer the
    /// website's test (scripts/tests/key-card-web.test.js) holds `fingerprint` to; the owner types
    /// the groups at the positions asked, in order, in any case, spaces anywhere; one group, or the
    /// right groups at other positions, does not confirm.
    /// Seen red 2026-11-20 with `blake3::hash` in place of `derive_key`: the known answer differed.
    /// Seen red 2026-11-21 (the third review's M2; run by mutation, scratchpad ownfix5-rust-mutants2.js)
    /// with `groups_at` reading the LAST two groups whatever was asked (the round-3 rule): the groups
    /// at positions 1 and 4 failed. Four groups since round 5 (the fourth review's finding 5).
    #[test]
    fn the_fingerprint_is_pinned() {
        let key = "ab".repeat(1952);
        assert_eq!(fingerprint(&key), "ca45 e5d0 d2cc 223c c757 ed6b");
        assert_eq!(groups_at(&fingerprint(&key), [1, 2, 4, 5]), "e5d0 d2cc c757 ed6b");
        assert!(fingerprint_confirmed_at(&key, [1, 2, 4, 5], "  E5D0 D2CC C757 ED6B "));
        assert!(fingerprint_confirmed_at(&key, [1, 2, 4, 5], "e5d0d2ccc757ed6b"));
        assert!(!fingerprint_confirmed_at(&key, [1, 2, 4, 5], "c757 ed6b"), "two groups do not confirm four asked");
        assert!(!fingerprint_confirmed_at(&key, [1, 2, 4, 5], "d2cc 223c c757 ed6b"), "nor the last four when others were asked");
        assert!(!fingerprint_confirmed_at(&key, [1, 2, 4, 5], "ed6b c757 d2cc e5d0"), "nor the asked groups out of order");
        assert!(!fingerprint_confirmed_at(&key, [1, 2, 4, 6], "e5d0 d2cc c757 ed6b") && !fingerprint_confirmed_at("not hex", [0, 1, 2, 3], ""));
        assert_eq!(groups_at(&fingerprint(&key), [1, 1, 4, 5]), "", "a position named twice is no ask");
        assert_eq!(fingerprint("zz"), "");
        assert_eq!(groups_at("", [0, 1, 2, 3]), "");
    }

    /// A KEY GROUND ON THREE GROUPS PASSES NO ASK (the fourth review's finding 5, 2026-11-21). A
    /// relay that makes keys until one's fingerprint matches Bea's at three positions (about 2^48
    /// candidates at 1 to 2 microseconds each: some 10^5 core-hours, within a relay's reach) is
    /// refused at every one of the 15 sets of four the app can ask, since each holds a group it
    /// does not match; a key ground on four groups (2^64) passes exactly one set in 15. (The draw
    /// itself: `the_draw_is_uniform_over_all_fifteen_sets`.)
    ///
    /// Seen red 2026-11-21 (scratchpad ownfix7-batch1.log, mutant P-1 of ownfix6-rust-mutants.json)
    /// with `groups_match_at` comparing only the first two of the positions asked (the round-4
    /// two-group rule in effect): the test panicked "a key ground on three groups [0, 1, 2] passed the
    /// set [0, 1, 2, 3]".
    #[test]
    fn a_key_ground_on_three_groups_passes_no_ask() {
        let bea = fingerprint(&"ab".repeat(1952));
        let groups: Vec<&str> = bea.split(' ').collect();
        let ground_on = |at: &[usize]| groups.iter().enumerate().map(|(i, g)| if at.contains(&i) { g.to_string() } else { "0000".into() }).collect::<Vec<_>>().join(" ");
        let sets = confirm_sets();
        assert_eq!(sets.len(), 15, "every set of four of the six");
        for three in [[0, 1, 2], [1, 3, 5], [3, 4, 5]] {
            let ground = ground_on(&three);
            for set in &sets {
                // The owner types Bea's real groups at the positions asked (what she reads out).
                assert!(!groups_match_at(&ground, *set, &groups_at(&bea, *set)), "a key ground on three groups {three:?} passed the set {set:?}");
            }
        }
        let four = ground_on(&[0, 2, 3, 5]);
        let passing: Vec<Asked> = sets.iter().copied().filter(|s| groups_match_at(&four, *s, &groups_at(&bea, *s))).collect();
        assert_eq!(passing, vec![[0, 2, 3, 5]], "a key ground on four groups passes one set in 15");
    }

    /// THE DRAW IS UNIFORM OVER ALL 15 SETS, EVERY ASK (the fifth review's H1, 2026-11-21). The
    /// round-5 rule never drew the set asked the time before for a key, which only narrowed a
    /// grinder's next draw to 14; now every ask draws from all 15 alike. Each of the first 15
    /// values a generator can give lands on a different set (so each set has the same share of the
    /// rejection zone), the value just past the zone is drawn again, a generator that repeats gives
    /// the same set again, and 30,000 draws from the operating system's generator land on each set
    /// between 1,700 and 2,300 times (2,000 expected, a standard deviation of about 43).
    ///
    /// Seen red 2026-11-21 with round 5's rule put back (the set drawn the time before left out of
    /// the next draw, remembered between calls; scratchpad ownfix8-red-draw.log): "each of the first
    /// 15 values lands on a different set, left: 14, right: 15".
    #[test]
    fn the_draw_is_uniform_over_all_fifteen_sets() {
        let sets = confirm_sets();
        let mut by_value = std::collections::HashSet::new();
        for r in 0..15u32 {
            let mut once = Some(r);
            by_value.insert(confirm_positions_with(|| once.take().expect("one value is enough")));
        }
        assert_eq!(by_value.len(), 15, "each of the first 15 values lands on a different set");
        let zone = (1u64 << 32) - (1u64 << 32) % 15;
        let mut values = [zone as u32, 3].into_iter();
        assert_eq!(confirm_positions_with(|| values.next().unwrap()), sets[3], "a value past the zone is drawn again");
        let first = confirm_positions_with(|| 7);
        let again = confirm_positions_with(|| 7);
        assert_eq!(first, again, "a generator that repeats draws the same set again: {first:?} then {again:?}");
        let mut counts = std::collections::HashMap::new();
        for _ in 0..30_000 {
            let at = confirm_positions();
            assert!(at.windows(2).all(|w| w[0] < w[1]) && at[CONFIRM_GROUPS - 1] < FINGERPRINT_GROUPS, "four different ascending positions: {at:?}");
            *counts.entry(at).or_insert(0u32) += 1;
        }
        assert_eq!(counts.len(), 15, "every set is drawn");
        for (set, n) in &counts {
            assert!((1_700..=2_300).contains(n), "{set:?} drawn {n} times of 30,000 (2,000 expected)");
        }
    }

    /// A WRONG SET IS NOT AN EMPTY ONE (the fifth review's H1 (c)): nothing typed, a few characters,
    /// the right sixteen and a wrong sixteen are four different answers, and only a full set of
    /// sixteen characters that differs is a mismatch.
    /// Seen red 2026-11-21 with `judge_at` answering `Short` for any set that does not match (the
    /// round-5 behaviour, where a wrong set left the change waiting like an empty field; scratchpad
    /// ownfix8-red-judge.log): "a wrong set of four is a mismatch, left: Short, right: Mismatched".
    #[test]
    fn a_wrong_set_is_not_an_empty_one() {
        let key = "ab".repeat(1952);
        let fp = fingerprint(&key);
        let at = [0, 2, 3, 5];
        assert_eq!(judge_at(&key, at, ""), Typed::Empty);
        assert_eq!(judge_at(&key, at, "   "), Typed::Empty);
        assert_eq!(judge_at(&key, at, "ca45 e5d0"), Typed::Short, "two groups are not yet a judgement");
        assert_eq!(judge_at(&key, at, &groups_at(&fp, at).to_uppercase()), Typed::Matched);
        assert_eq!(judge_at(&key, at, "0000 0000 0000 0000"), Typed::Mismatched, "a wrong set of four is a mismatch");
        assert_eq!(judge_at(&key, at, &groups_at(&fp, [0, 1, 2, 3])), Typed::Mismatched, "the right key's groups at other positions");
        assert_eq!(judge_at(&key, at, &format!("{} 0", groups_at(&fp, at))), Typed::Short, "seventeen characters are not four groups");
    }

    /// THE NAME RULE IS THE HONEST RELAY'S (relay.rs `identify`: 1 to 24 of [A-Za-z0-9_-]), so a
    /// relay that writes a fingerprint into a name has it refused.
    /// Seen red 2026-11-21 (run by mutation, scratchpad ownfix5-rust-mutants2.js) with `honest_name`
    /// allowing spaces and brackets: "Bea " passed (the decorated name stays refused by its length).
    #[test]
    fn the_name_rule_is_the_honest_relays() {
        for ok in ["Bea", "a", "Cal_2", "x-y", &"z".repeat(24)] {
            assert!(honest_name(ok), "{ok}");
        }
        for bad in ["", "Bea (3fa2 91c0 77de 0b4e 5a19 c2d0)", "Bea ", "Bé", &"z".repeat(25), "a\u{202e}b", "a.b"] {
            assert!(!honest_name(bad), "{bad:?}");
        }
    }

    /// THE APP'S AND THE SERVER'S NAMES ARE NOBODY'S (the seventh review, H1 (b)): "System" and its
    /// lookalikes are refused, in any case; names merely containing one are not.
    /// Seen red 2026-11-21 before `reserved_name` existed: `honest_name("System")` was true, and with
    /// the fold's digit map removed "Sy5tem" and "Hurnanity0S" passed.
    #[test]
    fn the_apps_and_servers_names_are_nobodys() {
        for bad in ["System", "system", "SYSTEM", "Sys_tem", "Sy5tem", "5y5t3m", "HumanityOS", "humanity-os", "Hurnanity0S", "HumanityOSApp", "FromThisServer", "From_this_server", "Fr0mThi5Server", "Server", "App", "4pp"] {
            assert!(reserved_name(bad), "{bad}");
            assert!(!honest_name(bad), "{bad}");
        }
        for ok in ["Systems", "Ada", "Bea", "Serverly", "Apple", "Humanist", "SysAdmin"] {
            assert!(!reserved_name(ok), "{ok}");
            assert!(honest_name(ok), "{ok}");
        }
    }
}

#[cfg(test)]
#[path = "key_card_tests.rs"]
mod tests;
