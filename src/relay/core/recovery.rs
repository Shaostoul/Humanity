//! Recovery through trusted people: the protocol the relay and both apps share (2026-10-21).
//!
//! The design, and why it is safe, is docs/design/recovery-through-trusted-people.md. In short:
//!
//! - **Set up.** A person's app splits the 32 bytes their recovery phrase writes down into
//!   `total` pieces with Shamir's secret sharing over GF(256) (the `blahaj` crate), any `needed`
//!   of which rebuild it. Each piece is sealed to one trusted person's Kyber768 key the way a
//!   sealed DM is (ML-KEM-768, then BLAKE3 `derive_key`, then AES-256-GCM, here with associated
//!   data that names the account, the setup and the slot), and posted as a `recovery_share_v1`
//!   object signed by the person. Beside the piece the object carries a note sealed to the
//!   person's OWN Kyber key that says who keeps the piece, and the hash of a random answer
//!   token that only the trusted person can read (it is inside their piece). The server never
//!   learns who the trusted people are: no trusted person's name or key is in anything it can
//!   read, so no social graph is kept (CLAUDE.md: the follows table was the last one, and it
//!   is gone).
//! - **Ask for help.** A new device cannot sign as the lost key. It makes a one-time Kyber768
//!   key pair, keeps the secret half in memory, and opens an "ask" on the server naming the
//!   account and the one-time public key. The ask's code is a fingerprint of both
//!   ([`ask_code`], 80 bits shown as 16 characters), so the code the person reads aloud to a
//!   trusted person commits to the key: a server that swapped in a key of its own would need a
//!   second preimage of an 80-bit fingerprint to keep the code.
//! - **Help.** The trusted person talks to the person outside the app (a call on a number they
//!   already had, or face to face), types the code they are read, and their app checks it
//!   against the ask's key before it sends anything. Their piece goes back sealed to the
//!   one-time key, so the server holds only ciphertext it cannot open; it accepts an answer
//!   only with the slot's answer token (the hash is public, the token is not).
//! - **Restore.** With `needed` pieces the new device rebuilds the 32 bytes, checks that they
//!   derive the very account that asked ([`rebuild`]), and restores from the recovery phrase
//!   they write down: the same account, no key change.
//!
//! Every byte layout and word here is mirrored in web/shared/recovery.js; the fixture
//! scripts/tests/fixtures/recovery-v1.json, read by `recovery_tests.rs` and by
//! scripts/tests/recovery-web.test.js, holds the two to each other. Change both or neither.

use aes_gcm::{
    aead::{Aead, KeyInit, Payload},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ciborium::Value;

use super::did::did_for_pubkey;
use super::encoding::{cbor_bytes, cbor_int, cbor_map, cbor_text, from_canonical_bytes};
use super::object::{Object, ObjectBuilder};
use super::pq_crypto::{
    derive_dilithium_seed, encapsulate_to, DilithiumKeypair, KyberKeypair, DILITHIUM_PK_LEN, KYBER_CIPHERTEXT_LEN,
    KYBER_EK_LEN, KYBER_SEED_LEN,
};

/// The object type a setup is posted as, one per piece.
pub const SHARE_OBJECT_TYPE: &str = "recovery_share_v1";

/// The secret split: the 32 bytes a 24-word recovery phrase writes down.
pub const SECRET_LEN: usize = 32;
/// One piece of it as blahaj writes a share: the x coordinate, then one y byte per secret byte.
pub const SHARE_LEN: usize = 1 + SECRET_LEN;
/// The answer token inside each piece.
pub const TOKEN_LEN: usize = 32;

/// The fewest trusted people a setup may have.
pub const MIN_PEOPLE: usize = 2;
/// The most trusted people a setup may have (and so the most answers one ask can collect).
pub const MAX_PEOPLE: usize = 10;
/// The fewest who must help: one alone is never enough (a piece of a 1-of-n split IS the secret).
pub const MIN_NEEDED: usize = 2;

/// A note to self is padded to this many bytes before it is sealed, so its length says
/// nothing about the name in it.
pub const NOTE_PLAIN_LEN: usize = 256;

/// What sealing adds: the ML-KEM-768 ciphertext, the 12-byte nonce and the 16-byte GCM tag.
pub const SEAL_OVERHEAD: usize = KYBER_CIPHERTEXT_LEN + 12 + 16;
/// A sealed piece: its share and its answer token.
pub const PIECE_LEN: usize = SEAL_OVERHEAD + SHARE_LEN + TOKEN_LEN;
/// A sealed note to self.
pub const NOTE_LEN: usize = SEAL_OVERHEAD + NOTE_PLAIN_LEN;
/// A sealed answer: the share alone.
pub const ANSWER_LEN: usize = SEAL_OVERHEAD + SHARE_LEN;

/// How long an ask stays open.
pub const ASK_DAYS: u64 = 7;
/// [`ASK_DAYS`] in milliseconds.
pub const ASK_MS: u64 = ASK_DAYS * 86_400_000;

/// The bytes of fingerprint an ask's code shows: 80 bits.
pub const CODE_BYTES: usize = 10;
/// The characters of a code: 80 bits at 5 a character.
pub const CODE_CHARS: usize = 16;
/// Crockford's base 32: no I, L, O or U, so a code read aloud is not misheard as another.
pub const CODE_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// BLAKE3 `derive_key` contexts, one per use of a shared secret or a fingerprint.
pub const PIECE_DOMAIN: &str = "hum/recovery-piece/v1";
pub const NOTE_DOMAIN: &str = "hum/recovery-note/v1";
pub const ANSWER_DOMAIN: &str = "hum/recovery-answer/v1";
pub const ASK_DOMAIN: &str = "hum/recovery-ask/v1";

/// The purpose a person's app signs (`purpose\ntimestamp`, the relay's signed REST shape,
/// files.rs `check_signed`) to take all their pieces off the server.
pub const WITHDRAW_PURPOSE: &str = "recovery_withdraw";

/// The purpose a person's own app signs to close every request for help that names them ("that
/// was not me"). A request has no code the person's other devices could know, so it is closed by
/// the account it names, not by its code.
pub const NOT_ME_PURPOSE: &str = "recovery_not_me";

// ── The words sealed alongside each kind of message (AES-GCM associated data) ──

/// What a piece is bound to: whose it is, which setup, which slot, and the split's shape. A
/// piece copied into any other setup, slot or account does not open.
pub fn piece_aad(holder_did: &str, set_id: &str, slot: usize, needed: usize, total: usize) -> String {
    format!("{PIECE_DOMAIN}\n{holder_did}\n{set_id}\n{slot}\n{needed}\n{total}")
}

/// What a note to self is bound to.
pub fn note_aad(holder_did: &str, set_id: &str, slot: usize) -> String {
    format!("{NOTE_DOMAIN}\n{holder_did}\n{set_id}\n{slot}")
}

/// What an answer is bound to: the ask it answers, the account and the slot.
pub fn answer_aad(code: &str, holder_did: &str, slot: usize) -> String {
    format!("{ANSWER_DOMAIN}\n{code}\n{holder_did}\n{slot}")
}

// ── Sealing ──

/// Seal `plain` to the holder of the Kyber768 public key `recipient`: ML-KEM-768
/// encapsulation, the AES key from BLAKE3 `derive_key(domain, shared secret)`, a random
/// 12-byte nonce, AES-256-GCM over `plain` with `aad`. Bytes: KEM ciphertext, nonce, body.
pub fn seal(domain: &str, recipient: &[u8], aad: &[u8], plain: &[u8]) -> Result<Vec<u8>, String> {
    if recipient.len() != KYBER_EK_LEN {
        return Err(format!("a Kyber768 public key is {KYBER_EK_LEN} bytes, not {}", recipient.len()));
    }
    let (kem_ct, ss) = encapsulate_to(recipient).map_err(|e| format!("encapsulate: {e}"))?;
    let key = blake3::derive_key(domain, &ss);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut nonce).map_err(|e| format!("random: {e}"))?;
    let body = cipher
        .encrypt(Nonce::from_slice(&nonce), Payload { msg: plain, aad })
        .map_err(|e| format!("seal: {e}"))?;
    let mut out = Vec::with_capacity(kem_ct.len() + 12 + body.len());
    out.extend_from_slice(&kem_ct);
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Open what [`seal`] made for `me`. `None` for anything that is not ours, was altered, or
/// was sealed with other associated data.
pub fn open(domain: &str, me: &KyberKeypair, aad: &[u8], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < SEAL_OVERHEAD {
        return None;
    }
    let (kem_ct, rest) = sealed.split_at(KYBER_CIPHERTEXT_LEN);
    let (nonce, body) = rest.split_at(12);
    let ss = me.decapsulate(kem_ct).ok()?;
    let key = blake3::derive_key(domain, &ss);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&key));
    cipher.decrypt(Nonce::from_slice(nonce), Payload { msg: body, aad }).ok()
}

// ── Shamir's secret sharing ──

/// Split the 32 secret bytes into `total` pieces, any `needed` of which rebuild them. Piece
/// `i` (slot `i`) has x coordinate `i + 1`. The polynomial's coefficients come from the
/// operating system's random source through blahaj (its `dealer`, `rand::rng()`).
pub fn split(secret: &[u8], needed: usize, total: usize) -> Result<Vec<[u8; SHARE_LEN]>, String> {
    if secret.len() != SECRET_LEN {
        return Err(format!("the secret is {SECRET_LEN} bytes, not {}", secret.len()));
    }
    shape_ok(needed, total)?;
    let sharks = blahaj::Sharks(needed as u8);
    let shares: Vec<[u8; SHARE_LEN]> = sharks
        .dealer(secret)
        .take(total)
        .map(|s| {
            let bytes = Vec::from(&s);
            let mut out = [0u8; SHARE_LEN];
            out.copy_from_slice(&bytes);
            out
        })
        .collect();
    if shares.len() != total || shares.iter().enumerate().any(|(i, s)| s[0] as usize != i + 1) {
        return Err("the split did not give one piece per slot".into());
    }
    Ok(shares)
}

/// Put the 32 bytes back together from `needed` or more pieces. Pieces with the same x
/// coordinate count once (the first is kept), and a piece at x = 0 is refused: it would be the
/// secret itself, and no split here ever makes one.
pub fn combine(shares: &[[u8; SHARE_LEN]], needed: usize) -> Result<[u8; SECRET_LEN], String> {
    if needed < MIN_NEEDED {
        return Err(format!("at least {MIN_NEEDED} pieces are needed"));
    }
    let mut seen = [false; 256];
    let mut kept = Vec::new();
    for s in shares {
        if s[0] == 0 {
            return Err("a piece with no x coordinate".into());
        }
        if !seen[s[0] as usize] {
            seen[s[0] as usize] = true;
            kept.push(blahaj::Share::try_from(&s[..]).map_err(|e| e.to_string())?);
        }
    }
    if kept.len() < needed {
        return Err(format!("{} different pieces, and {needed} are needed", kept.len()));
    }
    let bytes = blahaj::Sharks(needed as u8).recover(kept.iter()).map_err(|e| e.to_string())?;
    let mut out = [0u8; SECRET_LEN];
    if bytes.len() != SECRET_LEN {
        return Err("the pieces are not of a 32-byte secret".into());
    }
    out.copy_from_slice(&bytes);
    Ok(out)
}

/// The account id the 32 bytes derive: what a rebuilt secret must match before anything is
/// restored from it.
pub fn account_of(secret: &[u8]) -> String {
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(secret));
    did_for_pubkey(&kp.public_key())
}

/// Rebuild the secret of `holder_did` from the pieces that came back (slot, share). Any piece
/// can be wrong (a trusted person's app could send a piece of another setup), so every way of
/// choosing `needed` of them is tried until one derives `holder_did`: at most C(10, 5) = 252
/// tries. `None` when none does.
pub fn rebuild(pieces: &[[u8; SHARE_LEN]], needed: usize, holder_did: &str) -> Option<[u8; SECRET_LEN]> {
    if needed < MIN_NEEDED || pieces.len() < needed || pieces.len() > 32 {
        return None;
    }
    let mut pick: Vec<usize> = (0..needed).collect();
    loop {
        let chosen: Vec<[u8; SHARE_LEN]> = pick.iter().map(|&i| pieces[i]).collect();
        if let Ok(secret) = combine(&chosen, needed) {
            if account_of(&secret) == holder_did {
                return Some(secret);
            }
        }
        // The next combination, in order.
        let mut i = needed;
        loop {
            if i == 0 {
                return None;
            }
            i -= 1;
            if pick[i] < pieces.len() - needed + i {
                break;
            }
        }
        pick[i] += 1;
        for j in i + 1..needed {
            pick[j] = pick[j - 1] + 1;
        }
    }
}

/// Whether `needed` of `total` is a setup shape this protocol allows.
pub fn shape_ok(needed: usize, total: usize) -> Result<(), String> {
    if !(MIN_PEOPLE..=MAX_PEOPLE).contains(&total) {
        return Err(format!("a setup has {MIN_PEOPLE} to {MAX_PEOPLE} trusted people, not {total}"));
    }
    if needed < MIN_NEEDED || needed > total {
        return Err(format!("{needed} of {total} cannot be: at least {MIN_NEEDED}, at most all of them"));
    }
    Ok(())
}

// ── The code an ask is known by ──

/// The fingerprint of an ask: BLAKE3 `derive_key(ASK_DOMAIN, holder_did "\n" one_time_key)`,
/// its first 10 bytes in Crockford base 32. The relay files the ask under it, and the trusted
/// person's app recomputes it from the key the relay hands back before it sends anything.
pub fn ask_code(holder_did: &str, one_time_key: &[u8]) -> String {
    let mut h = blake3::Hasher::new_derive_key(ASK_DOMAIN);
    h.update(holder_did.as_bytes());
    h.update(b"\n");
    h.update(one_time_key);
    let digest = h.finalize();
    base32(&digest.as_bytes()[..CODE_BYTES])
}

/// Crockford base 32, most significant bits first, no padding.
pub fn base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(CODE_ALPHABET[((acc >> bits) & 31) as usize] as char);
        }
        acc &= (1 << bits) - 1;
    }
    if bits > 0 {
        out.push(CODE_ALPHABET[((acc << (5 - bits)) & 31) as usize] as char);
    }
    out
}

/// A code as typed or read back: any case, spaces and dashes ignored, O read as 0 and I or L
/// as 1 (Crockford's rule). `None` unless it is 16 characters of the alphabet.
pub fn normalize_code(typed: &str) -> Option<String> {
    let mut out = String::new();
    for c in typed.chars() {
        let c = match c.to_ascii_uppercase() {
            ' ' | '-' | '\t' => continue,
            'O' => '0',
            'I' | 'L' => '1',
            other => other,
        };
        if !c.is_ascii() || !CODE_ALPHABET.contains(&(c as u8)) {
            return None;
        }
        out.push(c);
    }
    (out.len() == CODE_CHARS).then_some(out)
}

/// A code the way a screen shows it: four groups of four.
pub fn show_code(code: &str) -> String {
    code.as_bytes().chunks(4).map(|c| String::from_utf8_lossy(c).into_owned()).collect::<Vec<_>>().join("-")
}

// ── Small shapes ──

/// A setup's id: 16 random bytes in lowercase hex.
pub fn new_set_id() -> Result<String, String> {
    let mut b = [0u8; 16];
    getrandom::getrandom(&mut b).map_err(|e| format!("random: {e}"))?;
    Ok(hex::encode(b))
}

/// Whether `s` is a setup id: 32 lowercase hex characters.
pub fn set_id_ok(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Whether `s` is an account id (`did:hum:` and base58).
pub fn account_id_ok(s: &str) -> bool {
    match s.strip_prefix("did:hum:") {
        Some(rest) => (16..=32).contains(&rest.len()) && rest.bytes().all(|b| b.is_ascii_alphanumeric()),
        None => false,
    }
}

/// The public hash of an answer token: plain BLAKE3.
pub fn token_hash(token: &[u8]) -> [u8; 32] {
    *blake3::hash(token).as_bytes()
}

/// The hash a close secret is filed under, lowercase hex.
pub fn close_hash(secret: &[u8]) -> String {
    hex::encode(blake3::hash(secret).as_bytes())
}

// ── The object a setup posts, one per piece ──

/// The payload of one `recovery_share_v1`.
#[derive(Debug, Clone, PartialEq)]
pub struct SharePayload {
    pub set_id: String,
    pub slot: usize,
    pub needed: usize,
    pub total: usize,
    /// The piece, sealed to the trusted person ([`PIECE_LEN`] bytes).
    pub piece: Vec<u8>,
    /// Who keeps it, sealed to the person themselves ([`NOTE_LEN`] bytes).
    pub note: Vec<u8>,
    /// BLAKE3 of the answer token inside the piece.
    pub token_hash: [u8; 32],
}

const PAYLOAD_KEYS: [&str; 7] = ["needed", "note", "piece", "set_id", "slot", "token_hash", "total"];

impl SharePayload {
    /// The CBOR map the object carries.
    pub fn to_cbor(&self) -> Value {
        cbor_map(vec![
            ("set_id", cbor_text(&self.set_id)),
            ("slot", cbor_int(self.slot as u64)),
            ("needed", cbor_int(self.needed as u64)),
            ("total", cbor_int(self.total as u64)),
            ("piece", cbor_bytes(&self.piece)),
            ("note", cbor_bytes(&self.note)),
            ("token_hash", cbor_bytes(&self.token_hash)),
        ])
    }

    /// Read and check a payload: exactly the seven fields, each of its type, a shape this
    /// protocol allows, a slot inside it, and every sealed part exactly the length it must be.
    /// The error is said in words, for the 400 the relay answers with.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let value = from_canonical_bytes(bytes).map_err(|e| format!("the payload is not canonical CBOR ({e})"))?;
        let Value::Map(entries) = value else { return Err("the payload is not a map".into()) };
        let mut keys: Vec<String> = Vec::new();
        let (mut set_id, mut slot, mut needed, mut total) = (None, None, None, None);
        let (mut piece, mut note, mut hash) = (None, None, None);
        for (k, v) in entries {
            let Value::Text(k) = k else { return Err("a payload key is not text".into()) };
            match (k.as_str(), v) {
                ("set_id", Value::Text(s)) => set_id = Some(s),
                ("slot", Value::Integer(i)) => slot = small_int(i),
                ("needed", Value::Integer(i)) => needed = small_int(i),
                ("total", Value::Integer(i)) => total = small_int(i),
                ("piece", Value::Bytes(b)) => piece = Some(b),
                ("note", Value::Bytes(b)) => note = Some(b),
                ("token_hash", Value::Bytes(b)) => hash = Some(b),
                (other, _) => return Err(format!("the payload's {other} is not what a {SHARE_OBJECT_TYPE} carries")),
            }
            keys.push(k);
        }
        keys.sort();
        if keys != PAYLOAD_KEYS {
            return Err(format!("a {SHARE_OBJECT_TYPE} carries exactly {}", PAYLOAD_KEYS.join(", ")));
        }
        let set_id = set_id.filter(|s| set_id_ok(s)).ok_or("set_id is not 32 lowercase hex characters")?;
        let (slot, needed, total) = (
            slot.ok_or("slot is not a small number")?,
            needed.ok_or("needed is not a small number")?,
            total.ok_or("total is not a small number")?,
        );
        shape_ok(needed, total)?;
        if slot >= total {
            return Err(format!("slot {slot} is outside a setup of {total}"));
        }
        let piece = piece.filter(|b| b.len() == PIECE_LEN).ok_or(format!("the piece is not {PIECE_LEN} bytes"))?;
        let note = note.filter(|b| b.len() == NOTE_LEN).ok_or(format!("the note is not {NOTE_LEN} bytes"))?;
        let hash: [u8; 32] = hash.and_then(|b| b.try_into().ok()).ok_or("token_hash is not 32 bytes")?;
        Ok(Self { set_id, slot, needed, total, piece, note, token_hash: hash })
    }
}

fn small_int(i: ciborium::value::Integer) -> Option<usize> {
    let raw: i128 = i.into();
    (0..=255).contains(&raw).then_some(raw as usize)
}

/// One trusted person, as the setting-up app knows them.
#[derive(Debug, Clone)]
pub struct TrustedPerson {
    /// Their account id (`did:hum:`).
    pub did: String,
    /// The name the setting-up person knows them by, for the note to self.
    pub name: String,
    /// Their Kyber768 public key.
    pub kyber_public: Vec<u8>,
}

/// What a note to self holds: who keeps the piece.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NoteToSelf {
    pub did: String,
    pub name: String,
}

/// The note's plaintext: its JSON, then zero bytes to [`NOTE_PLAIN_LEN`]. A name too long for
/// that is cut at a character boundary.
pub fn note_plain(note: &NoteToSelf) -> Vec<u8> {
    let mut name = note.name.clone();
    loop {
        let json = serde_json::to_vec(&NoteToSelf { did: note.did.clone(), name: name.clone() }).unwrap_or_default();
        if json.len() <= NOTE_PLAIN_LEN {
            let mut out = json;
            out.resize(NOTE_PLAIN_LEN, 0);
            return out;
        }
        name.pop();
    }
}

/// Read a note's plaintext back.
pub fn parse_note(plain: &[u8]) -> Option<NoteToSelf> {
    let end = plain.iter().position(|&b| b == 0).unwrap_or(plain.len());
    serde_json::from_slice(&plain[..end]).ok()
}

/// Build a whole setup: one signed `recovery_share_v1` per trusted person, slot by slot in the
/// order given. `secret` is the 32 bytes the recovery phrase writes down, `holder` the person's
/// Dilithium3 key pair and `holder_kyber` their own Kyber768 public key (for the notes to
/// self). Returns the setup's id and the objects to post.
pub fn build_setup(
    secret: &[u8],
    holder: &DilithiumKeypair,
    holder_kyber: &[u8],
    people: &[TrustedPerson],
    needed: usize,
    created_at_ms: u64,
) -> Result<(String, Vec<Object>), String> {
    let total = people.len();
    shape_ok(needed, total)?;
    let holder_did = did_for_pubkey(&holder.public_key());
    if account_of(secret) != holder_did {
        return Err("this recovery phrase is not this account's".into());
    }
    for (i, p) in people.iter().enumerate() {
        if p.did == holder_did {
            return Err("you cannot keep a piece of your own".into());
        }
        if people[..i].iter().any(|q| q.did == p.did) {
            return Err(format!("{} is chosen twice", p.name));
        }
        if p.kyber_public.len() != KYBER_EK_LEN {
            return Err(format!("{}'s key is not known", p.name));
        }
    }
    let set_id = new_set_id()?;
    let shares = split(secret, needed, total)?;
    let mut objects = Vec::with_capacity(total);
    for (slot, (person, share)) in people.iter().zip(shares.iter()).enumerate() {
        let mut token = [0u8; TOKEN_LEN];
        getrandom::getrandom(&mut token).map_err(|e| format!("random: {e}"))?;
        let mut plain = Vec::with_capacity(SHARE_LEN + TOKEN_LEN);
        plain.extend_from_slice(share);
        plain.extend_from_slice(&token);
        let piece = seal(PIECE_DOMAIN, &person.kyber_public, piece_aad(&holder_did, &set_id, slot, needed, total).as_bytes(), &plain)?;
        let note = seal(
            NOTE_DOMAIN,
            holder_kyber,
            note_aad(&holder_did, &set_id, slot).as_bytes(),
            &note_plain(&NoteToSelf { did: person.did.clone(), name: person.name.clone() }),
        )?;
        let payload = SharePayload { set_id: set_id.clone(), slot, needed, total, piece, note, token_hash: token_hash(&token) };
        let object = ObjectBuilder::new(SHARE_OBJECT_TYPE)
            .created_at(created_at_ms)
            .payload_cbor(&payload.to_cbor())
            .map_err(|e| e.to_string())?
            .sign(holder)
            .map_err(|e| e.to_string())?;
        objects.push(object);
    }
    Ok((set_id, objects))
}

/// The JSON a setup object is posted and served as: the `POST /api/v2/objects` submission
/// shape (binary fields in base64).
pub fn object_json(o: &Object) -> serde_json::Value {
    serde_json::json!({
        "protocol_version": o.protocol_version,
        "object_type": o.object_type,
        "space_id": o.space_id,
        "channel_id": o.channel_id,
        "author_public_key_b64": B64.encode(&o.author_public_key),
        "created_at": o.created_at,
        "references": o.references,
        "payload_schema_version": o.payload_schema_version,
        "payload_encoding": o.payload_encoding,
        "payload_b64": B64.encode(&o.payload),
        "signature_b64": B64.encode(&o.signature),
    })
}

/// One piece of a setup as an app reads it from the relay: checked (the signature verifies and
/// the payload is well formed), with the account it belongs to.
#[derive(Debug, Clone)]
pub struct ServedPiece {
    pub holder_did: String,
    pub created_at: u64,
    pub payload: SharePayload,
}

/// Read a setup object the relay served: the signature must verify, the type must be
/// [`SHARE_OBJECT_TYPE`], the payload must parse. The account is the signer's.
pub fn served_piece(v: &serde_json::Value) -> Result<ServedPiece, String> {
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).ok_or(format!("{k} is missing"));
    let b = |k: &str| -> Result<Vec<u8>, String> { B64.decode(s(k)?).map_err(|e| format!("{k}: {e}")) };
    let object = Object {
        protocol_version: v.get("protocol_version").and_then(|x| x.as_u64()).unwrap_or(1),
        object_type: s("object_type")?.to_string(),
        space_id: v.get("space_id").and_then(|x| x.as_str()).map(String::from),
        channel_id: v.get("channel_id").and_then(|x| x.as_str()).map(String::from),
        author_public_key: b("author_public_key_b64")?,
        created_at: v.get("created_at").and_then(|x| x.as_u64()),
        references: v
            .get("references")
            .and_then(|x| x.as_array())
            .map(|a| a.iter().filter_map(|r| r.as_str().map(String::from)).collect())
            .unwrap_or_default(),
        payload_schema_version: v.get("payload_schema_version").and_then(|x| x.as_u64()).unwrap_or(1),
        payload_encoding: s("payload_encoding")?.to_string(),
        payload: b("payload_b64")?,
        signature: b("signature_b64")?,
    };
    if object.object_type != SHARE_OBJECT_TYPE {
        return Err(format!("not a {SHARE_OBJECT_TYPE}"));
    }
    if object.author_public_key.len() != DILITHIUM_PK_LEN {
        return Err("the signer's key is not a Dilithium3 key".into());
    }
    object.verify_signature().map_err(|e| format!("the signature does not verify ({e})"))?;
    let payload = SharePayload::parse(&object.payload)?;
    Ok(ServedPiece {
        holder_did: did_for_pubkey(&object.author_public_key),
        created_at: object.created_at.unwrap_or(0),
        payload,
    })
}

/// A piece a trusted person opened: the share and the answer token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedPiece {
    pub share: [u8; SHARE_LEN],
    pub token: [u8; TOKEN_LEN],
}

/// Try to open a piece with my Kyber key pair: `Some` only when it was sealed to me, for this
/// account, setup and slot, its share sits at its slot's x coordinate, and its token matches
/// the public hash.
pub fn open_piece(me: &KyberKeypair, p: &ServedPiece) -> Option<OpenedPiece> {
    let pl = &p.payload;
    let aad = piece_aad(&p.holder_did, &pl.set_id, pl.slot, pl.needed, pl.total);
    let plain = open(PIECE_DOMAIN, me, aad.as_bytes(), &pl.piece)?;
    if plain.len() != SHARE_LEN + TOKEN_LEN || plain[0] as usize != pl.slot + 1 {
        return None;
    }
    let mut share = [0u8; SHARE_LEN];
    share.copy_from_slice(&plain[..SHARE_LEN]);
    let mut token = [0u8; TOKEN_LEN];
    token.copy_from_slice(&plain[SHARE_LEN..]);
    (token_hash(&token) == pl.token_hash).then_some(OpenedPiece { share, token })
}

/// Open a note to self with my own Kyber key pair.
pub fn open_note(me: &KyberKeypair, p: &ServedPiece) -> Option<NoteToSelf> {
    let aad = note_aad(&p.holder_did, &p.payload.set_id, p.payload.slot);
    parse_note(&open(NOTE_DOMAIN, me, aad.as_bytes(), &p.payload.note)?)
}

/// Seal a share back to the new device that asked.
pub fn seal_answer(one_time_key: &[u8], code: &str, holder_did: &str, slot: usize, share: &[u8; SHARE_LEN]) -> Result<Vec<u8>, String> {
    seal(ANSWER_DOMAIN, one_time_key, answer_aad(code, holder_did, slot).as_bytes(), share)
}

/// Open an answer on the new device: `Some` only for a share at its slot's x coordinate.
pub fn open_answer(one_time: &KyberKeypair, code: &str, holder_did: &str, slot: usize, sealed: &[u8]) -> Option<[u8; SHARE_LEN]> {
    let plain = open(ANSWER_DOMAIN, one_time, answer_aad(code, holder_did, slot).as_bytes(), sealed)?;
    if plain.len() != SHARE_LEN || plain[0] as usize != slot + 1 {
        return None;
    }
    let mut share = [0u8; SHARE_LEN];
    share.copy_from_slice(&plain);
    Some(share)
}

/// What a new device keeps while it asks: the one-time key pair and the secret that closes the
/// ask. Kept in memory only: closing the app means asking again with a new code.
pub struct OneTimeAsk {
    pub keys: KyberKeypair,
    pub public: Vec<u8>,
    pub close_secret: [u8; 32],
}

impl OneTimeAsk {
    /// A fresh one-time key pair and close secret from the operating system's random source.
    pub fn new() -> Result<Self, String> {
        let mut seed = [0u8; KYBER_SEED_LEN];
        getrandom::getrandom(&mut seed).map_err(|e| format!("random: {e}"))?;
        let keys = KyberKeypair::from_seed(&seed).map_err(|e| format!("Kyber keygen: {e}"))?;
        let mut close_secret = [0u8; 32];
        getrandom::getrandom(&mut close_secret).map_err(|e| format!("random: {e}"))?;
        let public = keys.public_key();
        Ok(Self { keys, public, close_secret })
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
