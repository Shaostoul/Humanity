//! Post-quantum cryptography primitives.
//!
//! HumanityOS uses a PQ-only stack from Phase 0 (see plan decision 4):
//! - **ML-DSA-65** (a.k.a. Dilithium3, FIPS 204) for object signing
//! - **ML-KEM-768** (a.k.a. Kyber768, FIPS 203) for key encapsulation
//! - **Argon2id** (separate `kdf` module) for password-based KDF
//! - **AES-256-GCM** / **XChaCha20-Poly1305** for symmetric encryption (existing modules)
//!
//! All keypairs derive deterministically from a 32/64-byte seed (in turn from BIP39).
//! Domain separators ensure independence between Dilithium and Kyber key material:
//! - `hum/dilithium3/v1`
//! - `hum/kyber768/v1`
//!
//! Sizes (ML-DSA-65 / ML-KEM-768):
//! - Dilithium3: pubkey 1952 B, signature 3309 B, seed 32 B
//! - Kyber768:   ek 1184 B, dk 2400 B, ciphertext 1088 B, shared secret 32 B, seed 64 B

use ml_dsa::{
    EncodedSignature, EncodedVerifyingKey, KeyGen, MlDsa65,
    Signature as DilithiumSignatureInner, SigningKey as DilithiumSigningKeyInner,
    VerifyingKey as DilithiumVerifyingKeyInner,
    signature::{Keypair, Signer, Verifier},
};
use ml_kem::{
    DecapsulationKey, EncapsulationKey, MlKem768,
    array::Array,
    kem::{Decapsulate, Encapsulate, FromSeed, Key, KeyExport, KeyInit, TryKeyInit},
};

use crate::relay::core::error::{Error, Result};

/// Size of an ML-DSA-65 (Dilithium3) public key, in bytes.
pub const DILITHIUM_PK_LEN: usize = 1952;
/// Size of an ML-DSA-65 (Dilithium3) signature, in bytes.
pub const DILITHIUM_SIG_LEN: usize = 3309;
/// ML-DSA-65 seed length, in bytes.
pub const DILITHIUM_SEED_LEN: usize = 32;

/// Size of an ML-KEM-768 (Kyber768) encapsulation key (public), in bytes.
pub const KYBER_EK_LEN: usize = 1184;
/// Size of an ML-KEM-768 (Kyber768) ciphertext, in bytes.
pub const KYBER_CIPHERTEXT_LEN: usize = 1088;
/// Size of an ML-KEM-768 (Kyber768) shared secret, in bytes.
pub const KYBER_SS_LEN: usize = 32;
/// ML-KEM-768 seed length, in bytes.
pub const KYBER_SEED_LEN: usize = 64;

/// BLAKE3 domain separator for deriving the Dilithium3 master seed from a user's BIP39 seed.
pub const DOMAIN_DILITHIUM: &str = "hum/dilithium3/v1";

/// BLAKE3 domain separator for deriving the Kyber768 master seed from a user's BIP39 seed.
pub const DOMAIN_KYBER: &str = "hum/kyber768/v1";

/// Fill a buffer with cryptographically-secure OS randomness.
fn os_random(buf: &mut [u8]) -> Result<()> {
    getrandom::getrandom(buf).map_err(|e| Error::InvalidField {
        field: "os_rng".into(),
        reason: e.to_string(),
    })
}

// =========================================================================
// Dilithium3 (ML-DSA-65) — object signing
// =========================================================================

/// A Dilithium3 keypair: signing key + derivable verifying key.
///
/// Wraps `ml_dsa::SigningKey<MlDsa65>` with a Vec<u8>-friendly API.
pub struct DilithiumKeypair {
    inner: DilithiumSigningKeyInner<MlDsa65>,
}

impl DilithiumKeypair {
    /// Generate a fresh keypair from the OS RNG.
    pub fn generate() -> Result<Self> {
        let mut seed = [0u8; DILITHIUM_SEED_LEN];
        os_random(&mut seed)?;
        Ok(Self::from_seed(&seed))
    }

    /// Deterministically derive a keypair from a 32-byte seed.
    ///
    /// The same seed always produces the same keypair (FIPS 204 KeyGen_internal).
    pub fn from_seed(seed: &[u8; DILITHIUM_SEED_LEN]) -> Self {
        let seed_arr = Array::<u8, _>::from(*seed);
        let inner = MlDsa65::from_seed(&seed_arr);
        Self { inner }
    }

    /// Get the 32-byte seed used to derive this keypair.
    pub fn to_seed(&self) -> [u8; DILITHIUM_SEED_LEN] {
        self.inner.to_seed().into()
    }

    /// Get the verifying (public) key as a 1952-byte Vec.
    pub fn public_key(&self) -> Vec<u8> {
        let vk = self.inner.verifying_key();
        vk.encode().to_vec()
    }

    /// Sign a message. Uses the deterministic ML-DSA variant (no RNG required for signing).
    pub fn sign(&self, message: &[u8]) -> Vec<u8> {
        let sig: DilithiumSignatureInner<MlDsa65> = self.inner.signing_key().sign(message);
        sig.encode().to_vec()
    }
}

/// Verify a Dilithium3 signature against a public key and message.
pub fn verify_dilithium(public_key: &[u8], message: &[u8], signature: &[u8]) -> Result<()> {
    if public_key.len() != DILITHIUM_PK_LEN {
        return Err(Error::InvalidPublicKey(format!(
            "Dilithium3 public key must be {DILITHIUM_PK_LEN} bytes, got {}",
            public_key.len()
        )));
    }
    if signature.len() != DILITHIUM_SIG_LEN {
        return Err(Error::InvalidSignature);
    }

    let pk_encoded = EncodedVerifyingKey::<MlDsa65>::try_from(public_key)
        .map_err(|_| Error::InvalidPublicKey("malformed Dilithium3 public key".into()))?;
    let vk = DilithiumVerifyingKeyInner::<MlDsa65>::decode(&pk_encoded);

    let sig_encoded = EncodedSignature::<MlDsa65>::try_from(signature)
        .map_err(|_| Error::InvalidSignature)?;
    let sig = DilithiumSignatureInner::<MlDsa65>::decode(&sig_encoded)
        .ok_or(Error::InvalidSignature)?;

    vk.verify(message, &sig)
        .map_err(|_| Error::SignatureVerificationFailed)
}

// =========================================================================
// Kyber768 (ML-KEM-768) — key encapsulation
// =========================================================================

/// A Kyber768 keypair holder: keeps both the decapsulation (private) and encapsulation (public) keys.
pub struct KyberKeypair {
    decap_key: DecapsulationKey<MlKem768>,
    encap_key: EncapsulationKey<MlKem768>,
}

impl KyberKeypair {
    /// Generate a fresh Kyber768 keypair from the OS RNG.
    pub fn generate() -> Result<Self> {
        let mut seed = [0u8; KYBER_SEED_LEN];
        os_random(&mut seed)?;
        Self::from_seed(&seed)
    }

    /// Deterministically derive a Kyber768 keypair from a 64-byte seed.
    pub fn from_seed(seed: &[u8; KYBER_SEED_LEN]) -> Result<Self> {
        let seed_arr = Array::<u8, _>::from(*seed);
        let (decap_key, encap_key) = MlKem768::from_seed(&seed_arr);
        Ok(Self { decap_key, encap_key })
    }

    /// Get the encapsulation key (public, 1184 bytes) as a Vec.
    pub fn public_key(&self) -> Vec<u8> {
        self.encap_key.to_bytes().to_vec()
    }

    /// Decapsulate a ciphertext using this keypair's decapsulation key.
    /// Returns the 32-byte shared secret.
    pub fn decapsulate(&self, ciphertext: &[u8]) -> Result<[u8; KYBER_SS_LEN]> {
        if ciphertext.len() != KYBER_CIPHERTEXT_LEN {
            return Err(Error::InvalidField {
                field: "kyber_ciphertext".into(),
                reason: format!("must be {KYBER_CIPHERTEXT_LEN} bytes"),
            });
        }
        let ct_arr = Array::<u8, _>::try_from(ciphertext)
            .map_err(|_| Error::InvalidField {
                field: "kyber_ciphertext".into(),
                reason: "malformed".into(),
            })?;
        let shared = self.decap_key.decapsulate(&ct_arr);
        let mut out = [0u8; KYBER_SS_LEN];
        out.copy_from_slice(shared.as_slice());
        Ok(out)
    }
}

/// Encapsulate a fresh shared secret to a recipient's encapsulation (public) key.
///
/// Returns `(ciphertext, shared_secret)`. The sender uses `shared_secret` immediately;
/// the recipient calls `decapsulate(ciphertext)` to derive the same value.
pub fn encapsulate_to(public_key: &[u8]) -> Result<(Vec<u8>, [u8; KYBER_SS_LEN])> {
    if public_key.len() != KYBER_EK_LEN {
        return Err(Error::InvalidPublicKey(format!(
            "Kyber768 encapsulation key must be {KYBER_EK_LEN} bytes, got {}",
            public_key.len()
        )));
    }
    let key_arr: Key<EncapsulationKey<MlKem768>> = Array::try_from(public_key)
        .map_err(|_| Error::InvalidPublicKey("malformed Kyber768 public key".into()))?;
    let ek = EncapsulationKey::<MlKem768>::new(&key_arr)
        .map_err(|_| Error::InvalidPublicKey("invalid Kyber768 key".into()))?;

    let (ct, ss) = ek.encapsulate();
    let mut shared = [0u8; KYBER_SS_LEN];
    shared.copy_from_slice(ss.as_slice());
    Ok((ct.to_vec(), shared))
}

// =========================================================================
// Seed derivation: BIP39 → Dilithium3 / Kyber768 master seeds
// =========================================================================

/// Derive a 32-byte Dilithium3 seed from any high-entropy source via BLAKE3 keyed-derivation
/// with the `hum/dilithium3/v1` domain separator.
///
/// Typical use: pass the 64-byte BIP39 PBKDF2 seed; output is independent of any other
/// seed derived from the same source under a different domain.
pub fn derive_dilithium_seed(master_seed: &[u8]) -> [u8; DILITHIUM_SEED_LEN] {
    let mut hasher = blake3::Hasher::new_derive_key(DOMAIN_DILITHIUM);
    hasher.update(master_seed);
    let mut out = [0u8; DILITHIUM_SEED_LEN];
    hasher.finalize_xof().fill(&mut out);
    out
}

/// Derive a 64-byte Kyber768 seed via BLAKE3 keyed-derivation with the `hum/kyber768/v1`
/// domain separator.
pub fn derive_kyber_seed(master_seed: &[u8]) -> [u8; KYBER_SEED_LEN] {
    let mut hasher = blake3::Hasher::new_derive_key(DOMAIN_KYBER);
    hasher.update(master_seed);
    let mut out = [0u8; KYBER_SEED_LEN];
    hasher.finalize_xof().fill(&mut out);
    out
}

// ── Friendship passes, v2 (2026-10-09, docs/design/blocking-and-safe-mode.md 10b) ──
// The `follows` table was the last server-side social graph and it went on 2026-08-24:
// friendship is a CLIENT-HELD credential. When two people become friends (a mutual follow),
// each gives the other a PASS:
//
//   preimage = "hum/friend/v2\n{server}\n{issuer}\n{grantee}\n{serial}\n{may}"
//   cert     = {"v":2,"serial":"...","may":"...","sig":"<base64 Dilithium3 over the preimage>"}
//
// - `server` is the did:hum: of the server it is given on (`Storage::server_did`, which
//   /api/server-info and every `identify_challenge` show). Friendship is formed over one
//   server's mailbox and each client keeps its passes per server, so a pass copied to another
//   server is useless there, and a withdrawal on this one is exact.
// - `serial` is 16 random bytes in lowercase hex, chosen by the issuer's client. It is what
//   makes ONE pass withdrawable without touching the others: the issuer sends
//   `cert_revoke {serial}` on its own signed-in socket and the relay keeps
//   `friend_cert_revocations (issuer_fingerprint, serial, revoked_day)`, under a keyed one-way
//   fingerprint of the issuer's key, for good (storage/friend_passes.rs). Withdrawn on Unfollow,
//   and later on Block and Remove friend.
// - `may` is what the friend may do: the sorted, comma-joined, de-duplicated subset of
//   FRIEND_PASS_KINDS. Changing what a friend may do is a new pass (new serial) and a
//   withdrawal of the old one. Until step B ("who can reach me") ships, the relay only reads
//   it; a valid pass lifts the knock budget, as v1 did.
// - NO END DATE (the operator, 2026-10-09: "I never want to stop being friends with my parents
//   and brothers"). A pass ends only when one of the two ends it, through the serial.
//
// The grantee presents the pass on every contact path addressed to the issuer (dm_put,
// trade_request, voice_call, a dc_offer, a profile request). The relay rebuilds the words from
// ITS OWN facts (its server DID, the person being reached as issuer, the sender's signed-in
// socket key as grantee) plus the serial and `may` the pass carries, so a pass given to someone
// else, by someone else or on another server is a bad signature. It stores no friends list;
// the withdrawal table holds random serials under a keyed fingerprint of the key that withdrew
// them, which names nobody without the relay's secret.
//
// v1 (`hum/friend/v1\n{issuer}\n{grantee}`, no serial, no server, nothing withdrawable) stops
// working outright: each client mints v2 passes for its current mutual follows on first run
// (no compatibility branch before launch, CLAUDE.md).
//
// MINT AND CHECK LIVE TOGETHER ON PURPOSE (2026-09-19), and the minting refuses whatever the
// check would refuse (the household permits' rule, below). `build_friend_cert` used to sit in
// `net::dm_pq`, which is native-gated, so the relay's own tests reached across a feature
// boundary to mint one. Keeping the two halves of one wire format side by side is also how they
// stay in agreement with the web client, whose preimage builder is
// web/shared/friend-pass.js, pinned to this file's test by scripts/tests/friend-pass.test.js.

/// Pass signature domain: the first line of what is signed. Web MUST use the identical string
/// (web/shared/friend-pass.js).
pub const FRIEND_CERT_DOMAIN: &str = "hum/friend/v2";

/// The `v` a pass's JSON carries.
pub const FRIEND_CERT_VERSION: u64 = 2;

/// Every kind of contact a pass can allow, sorted: the vocabulary of `may`. A protocol word
/// list, signed into every pass, so it lives here and in web/shared/friend-pass.js (pinned
/// together by scripts/tests/friend-pass.test.js) rather than in a data file.
pub const FRIEND_PASS_KINDS: [&str; 5] = ["call", "invite", "message", "trade", "voice_message"];

/// What a pass allows when two people become friends (10b): everything but calls, which come
/// only from people the person chooses. It is also the pass of a friend whose ticks in the
/// Safety page's "People I choose" were never changed (10c-ii: Message and Trade ticked, Call
/// not; the native client's `net::reach::FriendTicks::default`, pinned to this list by its test).
pub const FRIEND_PASS_DEFAULT_MAY: [&str; 4] = ["invite", "message", "trade", "voice_message"];

/// A pass serial's length, bytes (it travels as twice as many lowercase hex characters).
pub const FRIEND_CERT_SERIAL_BYTES: usize = 16;

/// The longest pass the check reads, bytes. A Dilithium3 signature is 4,412 characters of
/// base64; the rest is well under a hundred. Anything far longer is refused before it is parsed.
pub const FRIEND_CERT_MAX_LEN: usize = 8_192;

/// The words an issuer signs to give `grantee_hex` a pass on the server `server` (that server's
/// own `did:hum:`): the serial that names this pass and what it allows (`may`, canonical form).
pub fn friend_cert_preimage(server: &str, issuer_hex: &str, grantee_hex: &str, serial: &str, may: &str) -> String {
    format!("{FRIEND_CERT_DOMAIN}\n{server}\n{issuer_hex}\n{grantee_hex}\n{serial}\n{may}")
}

/// Why a friendship pass was not minted, or does not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendCertError {
    /// Not the v2 JSON shape (or too long to read), or a server, issuer or grantee that is
    /// empty or holds a line break (which would let two different passes sign the same words).
    Malformed,
    /// A serial that is not 16 bytes of lowercase hex.
    BadSerial,
    /// A `may` that is empty, names a kind not in [`FRIEND_PASS_KINDS`], or (in a pass being
    /// checked) is not in canonical form: sorted, comma-joined, each kind once.
    BadMay,
    /// The issuer's key or the signature is not well formed, or the signature is not the
    /// issuer's over these words.
    BadSignature,
}

/// What a friendship pass allows: a sorted, de-duplicated, non-empty subset of
/// [`FRIEND_PASS_KINDS`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendMay(Vec<&'static str>);

impl FriendMay {
    /// From kinds in any order, repeats allowed (the minting side). Refused when empty or when a
    /// word is not one of [`FRIEND_PASS_KINDS`].
    pub fn from_words<'a, I: IntoIterator<Item = &'a str>>(words: I) -> std::result::Result<Self, FriendCertError> {
        let mut kinds: Vec<&'static str> = Vec::new();
        for w in words {
            let Some(k) = FRIEND_PASS_KINDS.iter().find(|k| **k == w) else { return Err(FriendCertError::BadMay) };
            kinds.push(k);
        }
        kinds.sort_unstable();
        kinds.dedup();
        if kinds.is_empty() {
            return Err(FriendCertError::BadMay);
        }
        Ok(Self(kinds))
    }

    /// From the `may` a pass carries (the checking side): only the canonical form is read, so
    /// one set of kinds has exactly one spelling on the wire.
    pub fn parse(wire: &str) -> std::result::Result<Self, FriendCertError> {
        let may = Self::from_words(wire.split(','))?;
        if may.wire() != wire {
            return Err(FriendCertError::BadMay);
        }
        Ok(may)
    }

    /// The canonical form: sorted, comma-joined.
    pub fn wire(&self) -> String {
        self.0.join(",")
    }

    /// Does it allow `kind` (one of [`FRIEND_PASS_KINDS`])?
    pub fn allows(&self, kind: &str) -> bool {
        self.0.contains(&kind)
    }

    /// The kinds it allows, sorted.
    pub fn kinds(&self) -> &[&'static str] {
        &self.0
    }
}

/// A friendship pass that checked out: its serial and what it allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendPass {
    pub serial: String,
    pub may: FriendMay,
}

/// Is `serial` a pass serial: [`FRIEND_CERT_SERIAL_BYTES`] bytes as lowercase hex?
pub fn friend_cert_serial_ok(serial: &str) -> bool {
    serial.len() == FRIEND_CERT_SERIAL_BYTES * 2 && serial.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A fresh random serial for a pass about to be minted, or None when the OS gives no randomness.
pub fn new_friend_cert_serial() -> Option<String> {
    let mut bytes = [0u8; FRIEND_CERT_SERIAL_BYTES];
    os_random(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// A server, issuer or grantee fit to sign: not empty, no line break.
fn friend_cert_field_ok(s: &str) -> bool {
    !s.is_empty() && !s.contains(|c: char| c == '\n' || c == '\r')
}

/// Read a pass's JSON without checking its signature: its serial and `may`, and the signature.
/// Used by the check below, and by a client reading back a pass it issued itself (the echo of
/// its own control message, so it knows which serial to withdraw later).
pub fn parse_friend_cert(cert_json: &str) -> std::result::Result<(FriendPass, String), FriendCertError> {
    if cert_json.len() > FRIEND_CERT_MAX_LEN {
        return Err(FriendCertError::Malformed);
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(cert_json) else { return Err(FriendCertError::Malformed) };
    let text = |k: &str| v.get(k).and_then(|x| x.as_str());
    let (Some(serial), Some(may), Some(sig)) = (text("serial"), text("may"), text("sig")) else {
        return Err(FriendCertError::Malformed);
    };
    if v.get("v").and_then(|x| x.as_u64()) != Some(FRIEND_CERT_VERSION) {
        return Err(FriendCertError::Malformed);
    }
    if !friend_cert_serial_ok(serial) {
        return Err(FriendCertError::BadSerial);
    }
    let may = FriendMay::parse(may)?;
    Ok((FriendPass { serial: serial.to_string(), may }, sig.to_string()))
}

/// Mint MY friendship pass for `grantee_hex` on the server `server` (its own `did:hum:`, from
/// its `identify_challenge` or /api/server-info), named by `serial` (from
/// [`new_friend_cert_serial`]) and allowing `may` (any order, repeats folded): the v2 JSON, its
/// signature made with the Dilithium key derived from my BIP39 `seed`. Handed to the grantee in
/// a sealed control message; they present it whenever they reach me, and the relay checks it
/// with [`verify_friend_cert`]. Refused, minting nothing, whenever that check would refuse it: a
/// malformed id, a malformed serial, an empty `may` or a kind it does not know.
pub fn build_friend_cert(
    seed: &[u8],
    server: &str,
    my_hex: &str,
    grantee_hex: &str,
    serial: &str,
    may: &[&str],
) -> std::result::Result<String, FriendCertError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if ![server, my_hex, grantee_hex].into_iter().all(friend_cert_field_ok) {
        return Err(FriendCertError::Malformed);
    }
    if !friend_cert_serial_ok(serial) {
        return Err(FriendCertError::BadSerial);
    }
    let may = FriendMay::from_words(may.iter().copied())?.wire();
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
    let sig = kp.sign(friend_cert_preimage(server, my_hex, grantee_hex, serial, &may).as_bytes());
    Ok(serde_json::json!({ "v": FRIEND_CERT_VERSION, "serial": serial, "may": may, "sig": B64.encode(sig) }).to_string())
}

/// Did `issuer_hex` (a Dilithium3 public key, hex) really give `grantee_hex` this pass on the
/// server `server`, and what does it allow? Stateless: whether the issuer has WITHDRAWN it is
/// the relay's question (handlers/friend_passes.rs `friend_pass`, which every contact path
/// calls). `server`, `issuer_hex` and `grantee_hex` must be the CHECKER's own facts (the relay's
/// own `Storage::server_did`, the person being reached, the sender's signed-in socket key),
/// never anything the pass claims; only the serial and `may` come from the pass. Checked
/// cheapest first, so a malformed pass costs no signature check.
pub fn verify_friend_cert(
    server: &str,
    issuer_hex: &str,
    grantee_hex: &str,
    cert_json: &str,
) -> std::result::Result<FriendPass, FriendCertError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (pass, sig_b64) = parse_friend_cert(cert_json)?;
    if ![server, issuer_hex, grantee_hex].into_iter().all(friend_cert_field_ok) {
        return Err(FriendCertError::Malformed);
    }
    let Ok(issuer_pk) = hex_decode_str(issuer_hex) else { return Err(FriendCertError::BadSignature) };
    let Ok(sig) = B64.decode(sig_b64.trim()) else { return Err(FriendCertError::BadSignature) };
    let words = friend_cert_preimage(server, issuer_hex, grantee_hex, &pass.serial, &pass.may.wire());
    verify_dilithium(&issuer_pk, words.as_bytes(), &sig).map_err(|_| FriendCertError::BadSignature)?;
    Ok(pass)
}

// ── Household permits (ship homes increment 5, 2026-10-05; v2 2026-10-12) ──
// In a server's shared world a player builds only on their own plot. A HOUSEHOLD PERMIT lets
// someone else build there too: the plot's holder signs
//
//   permit = Dilithium_holder("hum/permit/v2\n{server}\n{plot}\n{grantee}\n{serial}\n{expiry}")
//
// where `server` is the `did:hum:` of the server it is given on (the relay's own identity,
// `Storage::server_did`, which /api/server-info shows), `plot` is the plot's id in the ship file
// ("p3"), `grantee` is the id the relay holds plots under for the person let in
// (`relay::storage::plots::plot_owner_id`: their `did:hum:`), `serial` is 16 random bytes in
// lowercase hex the holder chose (the friendship pass's serial shape, `friend_cert_serial_ok`),
// and `expiry` is when it runs out, Unix seconds. The grantee keeps it and sends it with each
// build or take-down they make on that plot (systems/construction/shared.rs `Permit`). The relay
// checks it STATELESSLY, the friendship certificate's pattern, and stores no permit.
//
// What the relay checks, split between this function and its caller (the build handler):
// - here: the signature is the issuer's over words the RELAY rebuilds from what it knows (its
//   own server DID, the frame's plot, the sender's own id), never over the server, plot and
//   grantee the permit claims, so a permit given on another server, one for p1 used on p2, or
//   one given to someone else fails as a bad signature; the serial's shape; and the end date:
//   not passed, and not more than PLOT_PERMIT_MAX_DAYS ahead;
// - the caller: the issuer has not taken this serial back (the withdrawal list friendship passes
//   use, `friend_cert_revocations`), and the issuer (`plot_owner_id` of the key in the permit) is
//   who holds that plot on this ship NOW, so a permit stops working when the plot changes hands.
//
// ONE SERVER (the coordinator's review of Wave 0, 2026-10-05). Every server hands out its plots
// in the ship file's order, so the same person often holds the same plot id (p1, say) on several
// servers. Without the server in the signed words, a permit given on one would let its holder
// build on all of them. Naming the server makes it good only where it was given.
//
// NO ENDLESS PERMITS (the operator, 2026-10-05). Every permit has an end date at most 90 days
// ahead, and is renewed by signing a new one. An expiry of 0, which the first design used for "a
// household member, no end", is simply long past.
//
// TAKING ONE BACK (ship homes 5b, amendment A1, 2026-10-12). v1 named no serial, so nothing
// could end a permit before its date, even a Block. Since 10b the relay keeps a withdrawal list
// for friendship passes (a keyed fingerprint of the giver's key and a random serial, never the
// key), so v2 names a serial and the giver withdraws it the way a pass is withdrawn:
// `cert_revoke {serial}` on their own signed-in socket. No v1 path is kept: nothing was ever
// given in a game, and nobody uses the platform yet. MINT AND CHECK LIVE TOGETHER, as the
// friendship certificate's do: the minting refuses what the check would refuse.

/// Household-permit signature domain: the first line of what is signed.
pub const PLOT_PERMIT_DOMAIN: &str = "hum/permit/v2";

/// The longest a household permit may run, days (the operator, 2026-10-05). The sentence a player
/// reads names it too (`systems::construction::shared::PERMIT_MAX_DAYS`, pinned to this by a
/// test).
pub const PLOT_PERMIT_MAX_DAYS: u64 = 90;

/// [`PLOT_PERMIT_MAX_DAYS`] in seconds.
pub const PLOT_PERMIT_MAX_SECS: u64 = PLOT_PERMIT_MAX_DAYS * 86_400;

/// How much longer than [`PLOT_PERMIT_MAX_SECS`] the CHECK allows, seconds: the issuer mints on
/// their own clock and the relay checks on its own, so a 90-day permit minted on a clock that
/// runs ahead of the relay's would otherwise be refused as too long. A day covers a clock set to
/// the wrong time zone as well as ordinary drift. The minting allows none.
pub const PLOT_PERMIT_CLOCK_SLACK_SECS: u64 = 86_400;

/// The longest a permit's server, plot or grantee id may be, bytes (a `did:hum:` is about 30).
pub const PLOT_PERMIT_FIELD_MAX_LEN: usize = 128;

/// What a household permit's issuer signs: the server it is given on (that server's own
/// `did:hum:`), the plot, the grantee, the serial that names it and the end date.
pub fn plot_permit_preimage(server: &str, plot: &str, grantee: &str, serial: &str, expiry: u64) -> String {
    format!("{PLOT_PERMIT_DOMAIN}\n{server}\n{plot}\n{grantee}\n{serial}\n{expiry}")
}

/// Why a household permit was not minted, or does not let its holder in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotPermitError {
    /// A server, plot or grantee id that is empty, longer than [`PLOT_PERMIT_FIELD_MAX_LEN`], or
    /// holds a line break (which would let two different permits sign the same words).
    Malformed,
    /// A serial that is not 16 bytes of lowercase hex ([`friend_cert_serial_ok`]).
    BadSerial,
    /// It has run out: the clock has reached its `expiry`.
    Expired,
    /// It runs out more than [`PLOT_PERMIT_MAX_DAYS`] days ahead.
    TooLong,
    /// The issuer's key or the signature is not well formed, or the signature is not the
    /// issuer's over these words.
    BadSignature,
}

/// A server, plot or grantee id fit to sign: not empty, not too long, no line break.
fn plot_permit_field_ok(s: &str) -> bool {
    !s.is_empty() && s.len() <= PLOT_PERMIT_FIELD_MAX_LEN && !s.contains(|c: char| c == '\n' || c == '\r')
}

/// Does a permit ending at `expiry` keep the rules at `now`: not run out, and not more than the
/// ceiling (plus `slack`) ahead?
fn plot_permit_window(expiry: u64, now: u64, slack: u64) -> std::result::Result<(), PlotPermitError> {
    if expiry <= now {
        return Err(PlotPermitError::Expired);
    }
    if expiry - now > PLOT_PERMIT_MAX_SECS + slack {
        return Err(PlotPermitError::TooLong);
    }
    Ok(())
}

/// Mint MY household permit for `grantee` on my plot `plot` of the server `server` (that
/// server's own `did:hum:`, from its /api/server-info), named by `serial` (from
/// [`new_friend_cert_serial`]: the same shape, and the same withdrawal list) and running out at
/// `expiry` (Unix seconds): a base64 Dilithium3 signature over [`plot_permit_preimage`], made with
/// the Dilithium key derived from my BIP39 `seed`, as [`build_friend_cert`] makes a friendship
/// note. `now` is my clock. Refused, minting nothing, when [`verify_plot_permit`] would refuse it:
/// a malformed id, a malformed serial, an end date already past, or one more than
/// [`PLOT_PERMIT_MAX_DAYS`] days ahead.
pub fn build_plot_permit(
    seed: &[u8],
    server: &str,
    plot: &str,
    grantee: &str,
    serial: &str,
    expiry: u64,
    now: u64,
) -> std::result::Result<String, PlotPermitError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if ![server, plot, grantee].into_iter().all(plot_permit_field_ok) {
        return Err(PlotPermitError::Malformed);
    }
    if !friend_cert_serial_ok(serial) {
        return Err(PlotPermitError::BadSerial);
    }
    plot_permit_window(expiry, now, 0)?;
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
    Ok(B64.encode(kp.sign(plot_permit_preimage(server, plot, grantee, serial, expiry).as_bytes())))
}

/// Does `sig_b64` let `grantee` build on `plot` of the server `server` until `expiry`, signed by
/// `issuer_hex` (a Dilithium3 public key, hex) and named `serial`? Stateless: whether the issuer
/// has taken `serial` back is the relay's question (handlers/shared_build.rs `permit_lets_in`).
/// `server`, `plot` and `grantee` must be the RELAY's own facts (its own `Storage::server_did`,
/// the frame's plot, the sender's `plot_owner_id`), never what the permit claims; only the
/// serial, the end date and the signature come from the permit. `now` is the relay's clock, Unix
/// seconds. Checked cheapest first, so a stale or absurd permit costs no signature check: the
/// ids, the serial, the end date (not passed, not more than [`PLOT_PERMIT_MAX_DAYS`] days plus
/// [`PLOT_PERMIT_CLOCK_SLACK_SECS`] ahead), then the signature. Whether the issuer holds the plot
/// is the caller's question too (see the note above).
#[allow(clippy::too_many_arguments)]
pub fn verify_plot_permit(
    issuer_hex: &str,
    server: &str,
    plot: &str,
    grantee: &str,
    serial: &str,
    expiry: u64,
    sig_b64: &str,
    now: u64,
) -> std::result::Result<(), PlotPermitError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if ![server, plot, grantee].into_iter().all(plot_permit_field_ok) {
        return Err(PlotPermitError::Malformed);
    }
    if !friend_cert_serial_ok(serial) {
        return Err(PlotPermitError::BadSerial);
    }
    plot_permit_window(expiry, now, PLOT_PERMIT_CLOCK_SLACK_SECS)?;
    let Ok(issuer_pk) = hex_decode_str(issuer_hex) else { return Err(PlotPermitError::BadSignature) };
    let Ok(sig) = B64.decode(sig_b64.trim()) else { return Err(PlotPermitError::BadSignature) };
    verify_dilithium(&issuer_pk, plot_permit_preimage(server, plot, grantee, serial, expiry).as_bytes(), &sig)
        .map_err(|_| PlotPermitError::BadSignature)
}

// ── File passes (device-to-device big files, 2026-10-11) ───────────────────
// docs/design/media-showcase.md, "Device-to-device big files: the shapes, exact", part 1. A file
// bigger than a person's private file limit goes from one device to the other, never to the
// server's disk. The receiver, once it says yes, dials the sender over WebRTC, and by default the
// packets go through the server's call forwarder so neither learns the other's address. The two
// need not be friends, so the relay needs some other word from the sender that this connection
// is wanted. That word is a FILE PASS, minted by the sender for this one transfer:
//
//   preimage = "hum/file-pass/v1\n{server}\n{issuer}\n{grantee}\n{id}\n{expiry}"
//   pass     = {"v":1,"id":"<id>","expiry":<n>,"sig":"<base64 Dilithium3 over the preimage>"}
//
// - `server` is the relay's own `did:hum:` (`Storage::server_did`, also in every
//   `identify_challenge` as `server_did`), so a pass given on one server is a bad signature on
//   any other.
// - `issuer` is the sender's key, `grantee` the receiver's (Dilithium3 hex, as everywhere).
// - `id` is the transfer's id, 16 random bytes as 32 lowercase hex characters: the same id the
//   offer note (`[[hum:offer:v1]]`) carries, and the data channel's label (`file:<id>`).
// - `expiry` is Unix seconds, at most FILE_PASS_MAX_DAYS ahead. No endless passes: the relay
//   keeps no list, so a pass cannot be withdrawn and must run out by itself.
//
// The pass says one thing: "the issuer wants the grantee to be able to reach them for this file".
// The relay checks it STATELESSLY, the friendship pass's and the household permit's pattern: it
// rebuilds the words from its OWN facts (its server DID, and the two keys it knows from the frame:
// the signed-in socket's key and the person named), never from the pass's claims, so a pass given
// to someone else, by someone else, for another file or on another server is a bad signature.
// Two doors read it, and nothing else:
// - a `dc_offer` from the grantee to the issuer (handlers/reach.rs `dc_offer_may_pass`): the
//   receiver dials the sender, the way round in which the relay can check it;
// - `call_credentials {file, peer, file_pass}` from either of the two (call_credentials.rs): the
//   forwarder's credentials for this one transfer.
//
// MINT AND CHECK LIVE TOGETHER (as the friendship pass and the household permit do): the minting
// refuses what the check would refuse. Both apps mint it too (desktop net/, web chat/); the shared
// fixture scripts/tests/fixtures/file-offer-v1.json holds one pass minted here from a fixed seed,
// and `file_pass_fixture_matches_the_builder` below holds this builder to it byte for byte.

/// File-pass signature domain: the first line of what is signed. Both apps MUST use the
/// identical string.
pub const FILE_PASS_DOMAIN: &str = "hum/file-pass/v1";

/// The `v` a file pass's JSON carries.
pub const FILE_PASS_VERSION: u64 = 1;

/// The longest a file pass may run, days (the spec, part 1). An offer nobody answers expires with
/// the DM mailbox; its pass by its own end date, a week at most.
pub const FILE_PASS_MAX_DAYS: u64 = 7;

/// [`FILE_PASS_MAX_DAYS`] in seconds.
pub const FILE_PASS_MAX_SECS: u64 = FILE_PASS_MAX_DAYS * 86_400;

/// How much longer than [`FILE_PASS_MAX_SECS`] the CHECK allows, seconds: the sender mints on its
/// own clock and the relay checks on its own (the household permit's day of slack, for the same
/// reason). The minting allows none.
pub const FILE_PASS_CLOCK_SLACK_SECS: u64 = 86_400;

/// A transfer id's length, bytes (it travels as twice as many lowercase hex characters).
pub const FILE_PASS_ID_BYTES: usize = 16;

/// The longest signature text the check reads, characters. A Dilithium3 signature is 4,412
/// characters of base64; anything far longer is refused before it is decoded.
pub const FILE_PASS_SIG_MAX_LEN: usize = 8_192;

/// What the sender signs to let the grantee reach them for transfer `id` on the server `server`
/// (that server's own `did:hum:`) until `expiry` (Unix seconds).
pub fn file_pass_preimage(server: &str, issuer_hex: &str, grantee_hex: &str, id: &str, expiry: u64) -> String {
    format!("{FILE_PASS_DOMAIN}\n{server}\n{issuer_hex}\n{grantee_hex}\n{id}\n{expiry}")
}

/// Why a file pass was not minted, or does not count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePassError {
    /// Not the v1 JSON shape, or a server, issuer or grantee that is empty or holds a line break
    /// (which would let two different passes sign the same words).
    Malformed,
    /// An id that is not 16 bytes of lowercase hex.
    BadId,
    /// It has run out: the clock has reached its `expiry`.
    Expired,
    /// It runs out more than [`FILE_PASS_MAX_DAYS`] days ahead.
    TooLong,
    /// The issuer's key or the signature is not well formed, or the signature is not the
    /// issuer's over these words.
    BadSignature,
}

/// A file pass that checked out: the transfer it is for, and when it runs out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePass {
    pub id: String,
    pub expiry: u64,
}

/// Is `id` a transfer id: [`FILE_PASS_ID_BYTES`] bytes as lowercase hex?
pub fn file_pass_id_ok(id: &str) -> bool {
    id.len() == FILE_PASS_ID_BYTES * 2 && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A fresh random transfer id, or None when the OS gives no randomness.
pub fn new_file_pass_id() -> Option<String> {
    let mut bytes = [0u8; FILE_PASS_ID_BYTES];
    os_random(&mut bytes).ok()?;
    Some(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Does a pass ending at `expiry` keep the rules at `now`: not run out, and not more than the
/// ceiling (plus `slack`) ahead?
fn file_pass_window(expiry: u64, now: u64, slack: u64) -> std::result::Result<(), FilePassError> {
    if expiry <= now {
        return Err(FilePassError::Expired);
    }
    if expiry - now > FILE_PASS_MAX_SECS + slack {
        return Err(FilePassError::TooLong);
    }
    Ok(())
}

/// Read a pass's JSON object without checking its signature: its id and end date, and the
/// signature. Only the v1 shape is read: `v` 1, `id` 32 lowercase hex, `expiry` a whole number,
/// `sig` a string.
pub fn parse_file_pass(pass: &serde_json::Value) -> std::result::Result<(FilePass, String), FilePassError> {
    let Some(obj) = pass.as_object() else { return Err(FilePassError::Malformed) };
    if obj.get("v").and_then(|x| x.as_u64()) != Some(FILE_PASS_VERSION) {
        return Err(FilePassError::Malformed);
    }
    let (Some(id), Some(expiry), Some(sig)) = (
        obj.get("id").and_then(|x| x.as_str()),
        obj.get("expiry").and_then(|x| x.as_u64()),
        obj.get("sig").and_then(|x| x.as_str()),
    ) else {
        return Err(FilePassError::Malformed);
    };
    if sig.len() > FILE_PASS_SIG_MAX_LEN {
        return Err(FilePassError::Malformed);
    }
    if !file_pass_id_ok(id) {
        return Err(FilePassError::BadId);
    }
    Ok((FilePass { id: id.to_string(), expiry }, sig.to_string()))
}

/// Mint MY file pass for `grantee_hex` on the server `server` (its own `did:hum:`), for transfer
/// `id` (from [`new_file_pass_id`]), running out at `expiry` (Unix seconds); `now` is my clock.
/// The JSON text, keys in the spec's order (`v`, `id`, `expiry`, `sig`), its signature made with
/// the Dilithium key derived from my BIP39 `seed`. Refused, minting nothing, whenever
/// [`verify_file_pass`] would refuse it: a malformed id or key, an end date already past, or one
/// more than [`FILE_PASS_MAX_DAYS`] days ahead.
pub fn build_file_pass(
    seed: &[u8],
    server: &str,
    my_hex: &str,
    grantee_hex: &str,
    id: &str,
    expiry: u64,
    now: u64,
) -> std::result::Result<String, FilePassError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if ![server, my_hex, grantee_hex].into_iter().all(friend_cert_field_ok) {
        return Err(FilePassError::Malformed);
    }
    if !file_pass_id_ok(id) {
        return Err(FilePassError::BadId);
    }
    file_pass_window(expiry, now, 0)?;
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
    let sig = B64.encode(kp.sign(file_pass_preimage(server, my_hex, grantee_hex, id, expiry).as_bytes()));
    // Written by hand so the keys keep the spec's order (serde_json sorts them). Nothing in it
    // needs escaping: hex, a number and base64.
    Ok(format!("{{\"v\":{FILE_PASS_VERSION},\"id\":\"{id}\",\"expiry\":{expiry},\"sig\":\"{sig}\"}}"))
}

/// Did `issuer_hex` (a Dilithium3 public key, hex) really give `grantee_hex` this pass on the
/// server `server`, and is it good at `now` (the checker's clock, Unix seconds)? Stateless.
/// `server`, `issuer_hex` and `grantee_hex` must be the CHECKER's own facts (the relay's own
/// `Storage::server_did`, and the two keys of the frame: the signed-in socket's key and the
/// person it names, each in the place the door puts them), never anything the pass claims; only
/// the id and the end date come from the pass, and whether the id is the transfer asked about is
/// the caller's question. Checked cheapest first, so a stale or absurd pass costs no signature
/// check: the shape, the ids, the end date (not passed, not more than [`FILE_PASS_MAX_DAYS`] days
/// plus [`FILE_PASS_CLOCK_SLACK_SECS`] ahead), then the signature.
pub fn verify_file_pass(
    server: &str,
    issuer_hex: &str,
    grantee_hex: &str,
    pass: &serde_json::Value,
    now: u64,
) -> std::result::Result<FilePass, FilePassError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (pass, sig_b64) = parse_file_pass(pass)?;
    if ![server, issuer_hex, grantee_hex].into_iter().all(friend_cert_field_ok) {
        return Err(FilePassError::Malformed);
    }
    file_pass_window(pass.expiry, now, FILE_PASS_CLOCK_SLACK_SECS)?;
    let Ok(issuer_pk) = hex_decode_str(issuer_hex) else { return Err(FilePassError::BadSignature) };
    let Ok(sig) = B64.decode(sig_b64.trim()) else { return Err(FilePassError::BadSignature) };
    let words = file_pass_preimage(server, issuer_hex, grantee_hex, &pass.id, pass.expiry);
    verify_dilithium(&issuer_pk, words.as_bytes(), &sig).map_err(|_| FilePassError::BadSignature)?;
    Ok(pass)
}

// =========================================================================
// Reports the admins can check (2026-10-09, docs/design/blocking-and-safe-mode.md 10e)
// =========================================================================
//
// Two signatures meet in a report, and the relay checks both here, in the relay build:
//
//   report   = "hum/report/v1\n{reporter}\n{target}\n{reason}\n{evidence_hash}\n{ts}"
//              signed by the REPORTER, so a report is theirs and its evidence is the evidence
//              they chose: `evidence_hash` is the lowercase hex BLAKE3 of the `evidence` array's
//              JSON text exactly as it was sent (clients send compact JSON), and `reporter` is
//              the key of the signed-in socket it arrived on, never a field of the report.
//   dm inner = "hum/dm/v2\n{from}\n{to}\n{ts}\n{text}"
//              signed by a DM's SENDER inside every sealed message (net::dm_pq
//              `build_signed_inner`, web crypto.js `_dmSigPreimage`). A reporter hands over the
//              readable copy their client holds; the relay rebuilds these words with the
//              reported person as `from` and the reporter as `to`, so a message forged, or
//              one written to someone else, does not check.
//
// The web client builds the report's words too; scripts read the pinned literals in the tests
// below (`report_preimage_and_evidence_hash_are_pinned`) to hold it to them.

/// Report signature domain: the first line of what a reporter signs.
pub const REPORT_DOMAIN: &str = "hum/report/v1";

/// DM v2 inner payload signature domain (net::dm_pq `DM_SIG_DOMAIN`, web `DM_SIG_DOMAIN_V2`).
pub const DM_INNER_DOMAIN: &str = "hum/dm/v2";

/// What a reporter signs.
pub fn report_preimage(reporter: &str, target: &str, reason: &str, evidence_hash: &str, ts: u64) -> String {
    format!("{REPORT_DOMAIN}\n{reporter}\n{target}\n{reason}\n{evidence_hash}\n{ts}")
}

/// The lowercase hex BLAKE3 of a report's `evidence` JSON text, as sent.
pub fn report_evidence_hash(evidence_json: &str) -> String {
    blake3::hash(evidence_json.as_bytes()).to_hex().to_string()
}

/// MY report's signature (base64 Dilithium3, the key derived from my BIP39 `seed`) over
/// [`report_preimage`], for evidence sent as `evidence_json`. The clients' half; the relay's
/// tests use it to make real reports.
pub fn build_report_sig(seed: &[u8], reporter: &str, target: &str, reason: &str, evidence_json: &str, ts: u64) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
    B64.encode(kp.sign(report_preimage(reporter, target, reason, &report_evidence_hash(evidence_json), ts).as_bytes()))
}

/// Did `reporter_hex` (a Dilithium3 public key, hex) sign this report? `reporter_hex` must be the
/// relay's own fact (the signed-in socket's key) and `evidence_hash` the hash the relay computed
/// itself from the text it received.
pub fn verify_report_sig(reporter_hex: &str, target: &str, reason: &str, evidence_hash: &str, ts: u64, sig_b64: &str) -> bool {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (Ok(pk), Ok(sig)) = (hex_decode_str(reporter_hex), B64.decode(sig_b64.trim())) else { return false };
    verify_dilithium(&pk, report_preimage(reporter_hex, target, reason, evidence_hash, ts).as_bytes(), &sig).is_ok()
}

/// What a DM's sender signs inside the seal (net::dm_pq `sig_preimage`, byte for byte).
pub fn dm_inner_preimage(from: &str, to: &str, ts: u64, text: &str) -> String {
    format!("{DM_INNER_DOMAIN}\n{from}\n{to}\n{ts}\n{text}")
}

/// Did `from_hex` sign this DM to `to` at `ts` saying `text`? The relay passes the reported
/// person as `from_hex` and the reporter as `to`, its own facts, never the evidence's claims.
pub fn verify_dm_inner(from_hex: &str, to: &str, ts: u64, text: &str, sig_b64: &str) -> bool {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let (Ok(pk), Ok(sig)) = (hex_decode_str(from_hex), B64.decode(sig_b64.trim())) else { return false };
    verify_dilithium(&pk, dm_inner_preimage(from_hex, to, ts, text).as_bytes(), &sig).is_ok()
}

/// Local hex decode (the `hex` crate is native-gated in some builds; the
/// relay feature carries it too, but a dependency-free decode keeps this
/// function unconditionally available).
fn hex_decode_str(s: &str) -> std::result::Result<Vec<u8>, ()> {
    let s = s.trim();
    // Not ASCII is not hex, and slicing it two bytes at a time below could cut a character in
    // half and panic (a report's target is whatever the reporter typed).
    if s.len() % 2 != 0 || !s.is_ascii() {
        return Err(());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| ()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dilithium_generate_sign_verify() {
        let kp = DilithiumKeypair::generate().expect("generate keypair");
        let pk = kp.public_key();
        assert_eq!(pk.len(), DILITHIUM_PK_LEN);

        let msg = b"hello civilization";
        let sig = kp.sign(msg);
        assert_eq!(sig.len(), DILITHIUM_SIG_LEN);

        verify_dilithium(&pk, msg, &sig).expect("valid signature must verify");
    }

    #[test]
    fn dilithium_from_seed_is_deterministic() {
        let seed = [42u8; DILITHIUM_SEED_LEN];
        let kp1 = DilithiumKeypair::from_seed(&seed);
        let kp2 = DilithiumKeypair::from_seed(&seed);
        assert_eq!(kp1.public_key(), kp2.public_key());
        assert_eq!(kp1.to_seed(), seed);
    }

    #[test]
    fn dilithium_wrong_message_fails() {
        let kp = DilithiumKeypair::generate().unwrap();
        let pk = kp.public_key();
        let sig = kp.sign(b"original message");
        assert!(verify_dilithium(&pk, b"different message", &sig).is_err());
    }

    #[test]
    fn dilithium_wrong_pk_fails() {
        let kp1 = DilithiumKeypair::generate().unwrap();
        let kp2 = DilithiumKeypair::generate().unwrap();
        let sig = kp1.sign(b"msg");
        assert!(verify_dilithium(&kp2.public_key(), b"msg", &sig).is_err());
    }

    #[test]
    fn dilithium_invalid_pk_length_rejected() {
        let kp = DilithiumKeypair::generate().unwrap();
        let sig = kp.sign(b"msg");
        let too_short = vec![0u8; 100];
        assert!(verify_dilithium(&too_short, b"msg", &sig).is_err());
    }

    #[test]
    fn dilithium_invalid_sig_length_rejected() {
        let kp = DilithiumKeypair::generate().unwrap();
        let pk = kp.public_key();
        let too_short = vec![0u8; 100];
        assert!(verify_dilithium(&pk, b"msg", &too_short).is_err());
    }

    #[test]
    fn kyber_encapsulate_decapsulate() {
        let recipient = KyberKeypair::generate().expect("generate kyber keypair");
        let pk = recipient.public_key();
        assert_eq!(pk.len(), KYBER_EK_LEN);

        let (ct, ss_send) = encapsulate_to(&pk).expect("encapsulate");
        assert_eq!(ct.len(), KYBER_CIPHERTEXT_LEN);
        assert_eq!(ss_send.len(), KYBER_SS_LEN);

        let ss_recv = recipient.decapsulate(&ct).expect("decapsulate");
        assert_eq!(ss_send, ss_recv);
    }

    #[test]
    fn kyber_invalid_pk_length_rejected() {
        let too_short = vec![0u8; 100];
        assert!(encapsulate_to(&too_short).is_err());
    }

    #[test]
    fn kyber_invalid_ciphertext_length_rejected() {
        let recipient = KyberKeypair::generate().unwrap();
        let too_short = vec![0u8; 100];
        assert!(recipient.decapsulate(&too_short).is_err());
    }

    #[test]
    fn kyber_from_seed_is_deterministic() {
        let seed = [11u8; KYBER_SEED_LEN];
        let kp1 = KyberKeypair::from_seed(&seed).unwrap();
        let kp2 = KyberKeypair::from_seed(&seed).unwrap();
        assert_eq!(kp1.public_key(), kp2.public_key());
    }

    #[test]
    fn dilithium_kyber_seeds_are_independent() {
        let master = [7u8; 64];
        let dil_seed = derive_dilithium_seed(&master);
        let kyb_seed = derive_kyber_seed(&master);
        // Even truncating Kyber seed to 32 bytes, they must differ
        // (different domain separators ensure this).
        let kyb_first_32: [u8; 32] = kyb_seed[..32].try_into().unwrap();
        assert_ne!(dil_seed, kyb_first_32);
    }

    #[test]
    fn seed_derivation_is_deterministic() {
        let master = [3u8; 64];
        assert_eq!(derive_dilithium_seed(&master), derive_dilithium_seed(&master));
        assert_eq!(derive_kyber_seed(&master), derive_kyber_seed(&master));
    }

    /// CROSS-LANGUAGE KNOWN-ANSWER TEST (v0.251, PQ Increment 1).
    ///
    /// The chat client derives its Dilithium3 identity from the SAME
    /// 32-byte BIP39 seed using `@noble/post-quantum` (vendored at
    /// `web/shared/vendor/noble-pq.bundle.js`). If the JS and Rust
    /// derivations ever diverge, every client-presented PQ pubkey
    /// becomes unverifiable by the relay — a silent, unrecoverable
    /// identity break. These exact constants are asserted in BOTH this
    /// test AND `scripts/pq-kat.mjs` (run via `just pq-kat`). Changing
    /// the derivation MUST update both or CI fails. Verified
    /// byte-for-byte 2026-05-16: noble ml-dsa-65 == RustCrypto ml-dsa.
    #[test]
    fn dilithium_cross_language_kat() {
        let master = [7u8; 32];
        let dil_seed = derive_dilithium_seed(&master);
        assert_eq!(
            hex::encode(dil_seed),
            "f0dfc6e8cc3eebd2e0f0265d2aae0f339090f2d4f92726884e385a48e81e0cc4",
            "BLAKE3 derive_key(hum/dilithium3/v1) drift — JS side will mismatch"
        );
        let pk = DilithiumKeypair::from_seed(&dil_seed).public_key();
        assert_eq!(pk.len(), DILITHIUM_PK_LEN);
        assert_eq!(
            hex::encode(&pk[..32]),
            "9bb07f42e537f236574366c44c2f6103f1aa5ceb5b232ff5fdd2af598a34adc8"
        );
        assert_eq!(
            hex::encode(&pk[pk.len() - 16..]),
            "a4ee62c651699bd74984e7d69006936f"
        );
        assert_eq!(
            blake3::hash(&pk).to_hex().as_str(),
            "3f4ff5c7e6505ca7b0dd6cb32c53839f8cff19772e291d4f18b082d1f7dc0126",
            "ML-DSA-65 keygen drift — noble and RustCrypto no longer agree"
        );
    }

    /// CROSS-LANGUAGE KYBER768 KAT (full-PQ cutover, v0.262.x).
    ///
    /// The chat client derives its DM (Kyber768) keypair from the SAME
    /// BIP39 seed via noble `ml_kem768` (vendored bundle). If JS and
    /// Rust ML-KEM-768 keygen diverge from the same 64-byte seed, the
    /// recipient's advertised public key won't match what the other
    /// device derives → every cross-client DM silently fails (the exact
    /// bug we're killing). Asserted in BOTH this test AND
    /// `scripts/pq-kat.mjs`. Constants frozen from the first green run.
    #[test]
    fn kyber_cross_language_kat() {
        let master = [7u8; 32];
        let kseed = derive_kyber_seed(&master);
        assert_eq!(
            hex::encode(kseed),
            "817975ca77f0b8a878088723602d68e0b2ff863ab0071c0b4c091d9fa114c639117a1f6ced5be40be2fdc1c3781fbdaf84c83d9d25153703620a6a5c1498eb2b",
            "BLAKE3 derive_key(hum/kyber768/v1) drift — JS side will mismatch"
        );
        let pk = KyberKeypair::from_seed(&kseed).unwrap().public_key();
        assert_eq!(pk.len(), KYBER_EK_LEN);
        assert_eq!(
            blake3::hash(&pk).to_hex().as_str(),
            "e5325adfbe9bbcedda20dbb333b9b94524ca853d4c641f03a199a96568c92664",
            "ML-KEM-768 keygen drift — noble and RustCrypto no longer agree"
        );
    }

    /// CROSS-IMPL SIGNATURE KAT (v0.252, PQ Increment 2).
    ///
    /// A Dilithium3 signature produced by the JS client
    /// (`@noble/post-quantum`, vendored) MUST verify under the Rust
    /// relay (`verify_dilithium`, RustCrypto `ml-dsa`). This is exactly
    /// the Inc 2 dual-sign path: client signs `content\ntimestamp`, the
    /// relay soft-verifies it. The signature fixture was generated once
    /// by noble for the canonical KAT seed + message `hello\n170...`
    /// (ML-DSA signing is hedged/non-deterministic, but VERIFY is
    /// deterministic, so a frozen valid signature stays valid forever).
    /// If RustCrypto ever stops accepting noble signatures this fails —
    /// and Inc 2's soft-verify would silently log false mismatches.
    #[test]
    fn dilithium_js_signature_verifies_in_rust() {
        let master = [7u8; 32];
        let dil_seed = derive_dilithium_seed(&master);
        let pk = DilithiumKeypair::from_seed(&dil_seed).public_key();
        let msg = b"hello\n1700000000000";
        let sig_hex = include_str!("pq_kat_dilithium_sig.hex").trim();
        let sig = hex::decode(sig_hex).expect("fixture is valid hex");
        assert_eq!(sig.len(), DILITHIUM_SIG_LEN, "fixture sig wrong length");
        verify_dilithium(&pk, msg, &sig)
            .expect("noble-produced signature must verify under RustCrypto ml-dsa");
        // Tamper → must fail (guards against a verify that accepts anything).
        let mut bad = sig.clone();
        bad[0] ^= 0x01;
        assert!(verify_dilithium(&pk, msg, &bad).is_err());
        assert!(verify_dilithium(&pk, b"different", &sig).is_err());
    }

    #[test]
    fn full_bip39_to_dilithium_flow() {
        // Simulate a BIP39 master seed (would normally come from `bip39` crate).
        let master = [0xABu8; 64];
        let dil_seed = derive_dilithium_seed(&master);
        let kp = DilithiumKeypair::from_seed(&dil_seed);

        let msg = b"identity proof";
        let sig = kp.sign(msg);
        verify_dilithium(&kp.public_key(), msg, &sig).expect("end-to-end BIP39 flow");
    }

    #[test]
    fn full_bip39_to_kyber_flow() {
        let master = [0xCDu8; 64];
        let kyber_seed = derive_kyber_seed(&master);
        let recipient = KyberKeypair::from_seed(&kyber_seed).unwrap();

        let (ct, ss_send) = encapsulate_to(&recipient.public_key()).unwrap();
        let ss_recv = recipient.decapsulate(&ct).unwrap();
        assert_eq!(ss_send, ss_recv);
    }

    /// A person's identity key, hex, from the seed `[n; 32]`.
    fn key_hex_of(n: u8) -> String {
        hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[n; 32])).public_key())
    }

    /// FRIENDSHIP PASS ROUND TRIP + A PINNED PREIMAGE (v2, blocking-and-safe-mode.md 10b). The
    /// signed words are frozen here because the web client builds them too
    /// (web/shared/friend-pass.js), and scripts/tests/friend-pass.test.js reads THIS literal
    /// and compares the web builder's output with it. A pass minted by the SHIPPED builder
    /// verifies and reads back its serial and `may`; the minting folds `may` into its one
    /// canonical spelling; a pass is good only for the server, issuer and grantee it names.
    /// Seen red 2026-10-09 with the server left out of `friend_cert_preimage`'s words: left
    /// `"hum/friend/v2\nAABB\nCCDD\n00112233445566778899aabbccddeeff\nmessage,trade"`, right
    /// `"hum/friend/v2\ndid:hum:srv\nAABB\nCCDD\n00112233445566778899aabbccddeeff\nmessage,trade"`.
    #[test]
    fn friend_cert_roundtrip_and_pinned_preimage() {
        assert_eq!(
            friend_cert_preimage("did:hum:srv", "AABB", "CCDD", "00112233445566778899aabbccddeeff", "message,trade"),
            "hum/friend/v2\ndid:hum:srv\nAABB\nCCDD\n00112233445566778899aabbccddeeff\nmessage,trade"
        );
        let issuer_master = [0x11u8; 32];
        let (issuer, grantee, other) = (key_hex_of(0x11), key_hex_of(0x22), key_hex_of(0x33));
        let (server, other_server) = (server_did_of(0x55), server_did_of(0x66));
        let serial = new_friend_cert_serial().expect("the OS gives randomness");
        assert!(friend_cert_serial_ok(&serial), "a fresh serial is a serial: {serial}");
        // Out of order, with a repeat: minted in the one canonical spelling.
        let cert = build_friend_cert(&issuer_master, &server, &issuer, &grantee, &serial, &["trade", "message", "trade"])
            .expect("a pass is minted");
        let pass = verify_friend_cert(&server, &issuer, &grantee, &cert).expect("the pass it minted verifies");
        assert_eq!(pass.serial, serial);
        assert_eq!(pass.may.wire(), "message,trade");
        assert!(pass.may.allows("message") && pass.may.allows("trade") && !pass.may.allows("call"));
        let bad = Err(FriendCertError::BadSignature);
        assert_eq!(verify_friend_cert(&other_server, &issuer, &grantee, &cert), bad, "another server");
        assert_eq!(verify_friend_cert(&server, &issuer, &other, &cert), bad, "another grantee");
        assert_eq!(verify_friend_cert(&server, &other, &grantee, &cert), bad, "another issuer");
        assert_eq!(verify_friend_cert(&server, "zz", &grantee, &cert), bad, "not a key");
        // The defaults when two people become friends: no calls (10b).
        let defaults = build_friend_cert(&issuer_master, &server, &issuer, &grantee, &serial, &FRIEND_PASS_DEFAULT_MAY).unwrap();
        let pass = verify_friend_cert(&server, &issuer, &grantee, &defaults).unwrap();
        assert_eq!(pass.may.wire(), "invite,message,trade,voice_message");
        assert!(!pass.may.allows("call"), "calls come only from people the person chooses");
    }

    /// WHAT A PASS ALLOWS IS SIGNED, AND READ IN ONE SPELLING ONLY. A pass whose `may` was
    /// widened after signing (a friend giving themselves calls) is a bad signature; one with the
    /// same kinds spelled out of order, repeated, empty or naming a kind nobody knows is refused
    /// before any signature is checked, as is a malformed serial, a missing field, a v1 pass
    /// (bare base64) and a `v` other than 2. The serial is signed too: changing it is a bad
    /// signature, so a withdrawal of one serial cannot be dodged by renaming the pass.
    /// Seen red 2026-10-09 with `may` left out of `friend_cert_preimage`'s words (on both the
    /// minting and the checking side): "a widened may", left `Ok(FriendPass { serial: "0011..",
    /// may: FriendMay(["call", "invite", "message", "trade", "voice_message"]) })`, right
    /// `Err(BadSignature)`.
    #[test]
    fn a_tampered_or_malformed_pass_is_refused() {
        let issuer_master = [0x11u8; 32];
        let (issuer, grantee) = (key_hex_of(0x11), key_hex_of(0x22));
        let server = server_did_of(0x55);
        let serial = "00112233445566778899aabbccddeeff";
        let cert = build_friend_cert(&issuer_master, &server, &issuer, &grantee, serial, &FRIEND_PASS_DEFAULT_MAY).unwrap();
        let v: serde_json::Value = serde_json::from_str(&cert).unwrap();
        let with = |k: &str, val: serde_json::Value| {
            let mut w = v.clone();
            w[k] = val;
            verify_friend_cert(&server, &issuer, &grantee, &w.to_string())
        };
        assert!(with("may", "invite,message,trade,voice_message".into()).is_ok(), "the untouched pass, rebuilt, still verifies");
        assert_eq!(with("may", "call,invite,message,trade,voice_message".into()), Err(FriendCertError::BadSignature), "a widened may");
        assert_eq!(with("may", "message".into()), Err(FriendCertError::BadSignature), "a narrowed may");
        assert_eq!(with("serial", "ffeeddccbbaa99887766554433221100".into()), Err(FriendCertError::BadSignature), "another serial");
        let bad_may = Err(FriendCertError::BadMay);
        assert_eq!(with("may", "message,invite,trade,voice_message".into()), bad_may, "out of order");
        assert_eq!(with("may", "invite,invite,message,trade,voice_message".into()), bad_may, "a kind twice");
        assert_eq!(with("may", "".into()), bad_may, "nothing allowed");
        assert_eq!(with("may", "invite,message,shout".into()), bad_may, "a kind nobody knows");
        let bad_serial = Err(FriendCertError::BadSerial);
        assert_eq!(with("serial", "00112233445566778899AABBCCDDEEFF".into()), bad_serial, "upper case");
        assert_eq!(with("serial", "0011223344556677".into()), bad_serial, "too short");
        assert_eq!(with("serial", "00112233445566778899aabbccddeefg".into()), bad_serial, "not hex");
        let malformed = Err(FriendCertError::Malformed);
        assert_eq!(with("v", 1.into()), malformed, "v1");
        assert_eq!(with("sig", serde_json::Value::Null), malformed, "no signature");
        assert_eq!(verify_friend_cert(&server, &issuer, &grantee, "bm90LWEtc2ln"), malformed, "a bare v1 signature");
        assert_eq!(verify_friend_cert(&server, &issuer, &grantee, &"x".repeat(FRIEND_CERT_MAX_LEN + 1)), malformed, "too long to read");
        assert_eq!(verify_friend_cert("", &issuer, &grantee, &cert), malformed, "no server");
        assert_eq!(verify_friend_cert(&server, &issuer, "a\nb", &cert), malformed, "a line break in an id");
        assert_eq!(with("sig", "bm90LWEtc2ln".into()), Err(FriendCertError::BadSignature), "words that are not a signature");
    }

    /// THE MINTING REFUSES WHAT THE CHECK REFUSES (the household permits' rule). Every pass the
    /// builder refuses is one the check would refuse: no `may`, a kind it does not know, a
    /// serial that is not 16 bytes of lowercase hex, an empty or line-broken id.
    /// Seen red 2026-10-09 with the serial check removed from `build_friend_cert`: "a short
    /// serial", left `None`, right `Some(BadSerial)` (it minted a pass the check then refused).
    #[test]
    fn the_minting_refuses_what_the_check_refuses() {
        let m = [0x11u8; 32];
        let (issuer, grantee) = (key_hex_of(0x11), key_hex_of(0x22));
        let server = server_did_of(0x55);
        let serial = "00112233445566778899aabbccddeeff";
        let mint = |server: &str, issuer: &str, grantee: &str, serial: &str, may: &[&str]| {
            build_friend_cert(&m, server, issuer, grantee, serial, may).err()
        };
        assert_eq!(mint(&server, &issuer, &grantee, serial, &[]), Some(FriendCertError::BadMay), "nothing allowed");
        assert_eq!(mint(&server, &issuer, &grantee, serial, &["message", "shout"]), Some(FriendCertError::BadMay), "a kind nobody knows");
        assert_eq!(mint(&server, &issuer, &grantee, "0011", &["message"]), Some(FriendCertError::BadSerial), "a short serial");
        assert_eq!(mint(&server, &issuer, &grantee, &serial.to_uppercase(), &["message"]), Some(FriendCertError::BadSerial), "upper case");
        assert_eq!(mint("", &issuer, &grantee, serial, &["message"]), Some(FriendCertError::Malformed), "no server");
        assert_eq!(mint(&server, &issuer, "x\ny", serial, &["message"]), Some(FriendCertError::Malformed), "a line break");
        assert_eq!(mint(&server, &issuer, &grantee, serial, &FRIEND_PASS_KINDS), None, "every kind at once is a pass");
        // A wire `may` is read only in its canonical form; the words in any order mint it.
        assert_eq!(FriendMay::parse("call,message").map(|m| m.wire()), Ok("call,message".to_string()));
        assert_eq!(FriendMay::parse("message,call"), Err(FriendCertError::BadMay));
        assert_eq!(FriendMay::from_words(["message", "call"]).map(|m| m.wire()), Ok("call,message".to_string()));
        let mut sorted = FRIEND_PASS_KINDS;
        sorted.sort_unstable();
        assert_eq!(sorted, FRIEND_PASS_KINDS, "the vocabulary is kept sorted, so its order is the canonical one");
    }

    /// A PASS MINTED BY RUST FROM THE KAT SEED, FROZEN (pq_kat_friend_pass.json). ML-DSA signing
    /// here is the deterministic variant, so the builder reproduces the fixture byte for byte;
    /// scripts/pq-kat.mjs reads the same file, rebuilds the words with the web builder
    /// (web/shared/friend-pass.js) and checks the signature with the vendored noble bundle the
    /// web client ships, so the whole path, not only the string, agrees across the two.
    /// Seen red 2026-10-09 with the fixture's cert edited to `"may":"message,invite"`: "the frozen
    /// pass verifies: BadMay".
    #[test]
    fn friend_cert_kat_fixture_matches_the_builder() {
        let fx: serde_json::Value = serde_json::from_str(include_str!("pq_kat_friend_pass.json")).expect("the fixture is JSON");
        let s = |k: &str| fx[k].as_str().unwrap_or_else(|| panic!("fixture field {k}")).to_string();
        let master = [fx["issuer_master_byte"].as_u64().unwrap() as u8; 32];
        let grantee = key_hex_of(fx["grantee_master_byte"].as_u64().unwrap() as u8);
        let issuer = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&master)).public_key());
        let may: Vec<String> = s("may").split(',').map(str::to_string).collect();
        let may: Vec<&str> = may.iter().map(String::as_str).collect();
        let minted = build_friend_cert(&master, &s("server"), &issuer, &grantee, &s("serial"), &may).unwrap();
        let pass = verify_friend_cert(&s("server"), &issuer, &grantee, &s("cert")).expect("the frozen pass verifies");
        assert_eq!(pass.serial, s("serial"));
        assert_eq!(pass.may.wire(), s("may"));
        assert_eq!(minted, s("cert"), "the builder reproduces the frozen pass byte for byte");
    }

    /// A server's `did:hum:` as `Storage::server_did` makes it: from the Dilithium key of its own
    /// seed.
    fn server_did_of(seed: u8) -> String {
        crate::relay::core::did::did_for_pubkey(&DilithiumKeypair::from_seed(&derive_dilithium_seed(&[seed; 32])).public_key())
    }

    /// A permit serial for these tests (any 16 bytes of lowercase hex is one).
    const PERMIT_SERIAL: &str = "00112233445566778899aabbccddeeff";

    /// HOUSEHOLD PERMIT ROUND TRIP + A PINNED PREIMAGE (ship homes increment 5; v2 with its
    /// serial, increment 5b amendment A1). The signed words are frozen here, because the game
    /// that mints a permit and the relay that checks it must build them byte for byte alike (and
    /// scripts/tests/second-player.test.js reads this pin for the scripted walker's minting). A
    /// permit minted by the SHIPPED builder verifies, to its last second; each signed field
    /// matters (another server, plot, grantee, serial, end date or signer is a bad signature, as
    /// are words that are not a signature and a key that is not a key); it runs out at its
    /// expiry; ids that could frame two permits alike are refused by the minting and the check
    /// both, as is a serial that is not 16 bytes of lowercase hex, and minting one already run out.
    /// Seen red 2026-10-12 with the serial left out of `plot_permit_preimage`'s words: left
    /// `"hum/permit/v2\ndid:hum:srv\np3\ndid:hum:abc\n0"`, right
    /// `"hum/permit/v2\ndid:hum:srv\np3\ndid:hum:abc\n00112233445566778899aabbccddeeff\n0"`.
    /// (v1, 2026-10-05: with the server left out of the words, left
    /// `"hum/permit/v1\np3\ndid:hum:abc\n0"`.)
    #[test]
    fn plot_permit_roundtrip_and_pinned_preimage() {
        assert_eq!(
            plot_permit_preimage("did:hum:srv", "p3", "did:hum:abc", "00112233445566778899aabbccddeeff", 0),
            "hum/permit/v2\ndid:hum:srv\np3\ndid:hum:abc\n00112233445566778899aabbccddeeff\n0"
        );
        let issuer_master = [0x11u8; 32];
        let issuer_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&issuer_master)).public_key());
        let grantee = crate::relay::core::did::did_for_pubkey(
            &DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x22u8; 32])).public_key(),
        );
        let other_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x33u8; 32])).public_key());
        let (server, other_server) = (server_did_of(0x55), server_did_of(0x66));
        let s = PERMIT_SERIAL;
        let now = 1_760_000_000u64;
        let expiry = now + 30 * 86_400;
        let sig = build_plot_permit(&issuer_master, &server, "p3", &grantee, s, expiry, now).expect("a 30-day permit is minted");
        let verify = verify_plot_permit;
        assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, s, expiry, &sig, now), Ok(()));
        assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, s, expiry, &sig, expiry - 1), Ok(()), "good to its last second");
        let bad = Err(PlotPermitError::BadSignature);
        assert_eq!(verify(&issuer_hex, &other_server, "p3", &grantee, s, expiry, &sig, now), bad, "another server");
        assert_eq!(verify(&issuer_hex, &server, "p4", &grantee, s, expiry, &sig, now), bad, "another plot");
        assert_eq!(verify(&issuer_hex, &server, "p3", "did:hum:someoneelse", s, expiry, &sig, now), bad, "another grantee");
        let other_serial = "ffeeddccbbaa99887766554433221100";
        assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, other_serial, expiry, &sig, now), bad, "another serial");
        assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, s, expiry + 1, &sig, now), bad, "another end date");
        assert_eq!(verify(&other_hex, &server, "p3", &grantee, s, expiry, &sig, now), bad, "another signer");
        assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, s, expiry, "bm90LWEtc2ln", now), bad, "not a signature");
        assert_eq!(verify("zz", &server, "p3", &grantee, s, expiry, &sig, now), bad, "not a key");
        assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, s, expiry, &sig, expiry), Err(PlotPermitError::Expired));
        let malformed = Some(PlotPermitError::Malformed);
        assert_eq!(build_plot_permit(&issuer_master, &server, "p3\ndid:hum:x", "y", s, expiry, now).err(), malformed, "a line break in an id");
        assert_eq!(verify(&issuer_hex, &server, "p3", "", s, expiry, &sig, now).err(), malformed, "an empty grantee");
        assert_eq!(verify(&issuer_hex, "", "p3", &grantee, s, expiry, &sig, now).err(), malformed, "an empty server");
        assert_eq!(verify(&issuer_hex, &server, &"p".repeat(129), &grantee, s, expiry, &sig, now).err(), malformed, "an id over the cap");
        let bad_serial = Some(PlotPermitError::BadSerial);
        for (what, serial) in [("upper case", s.to_uppercase()), ("too short", "0011".to_string()), ("not hex", format!("{}g", &s[..31])), ("none", String::new())] {
            assert_eq!(verify(&issuer_hex, &server, "p3", &grantee, &serial, expiry, &sig, now).err(), bad_serial, "checked: {what}");
            assert_eq!(build_plot_permit(&issuer_master, &server, "p3", &grantee, &serial, expiry, now).err(), bad_serial, "minted: {what}");
        }
        assert_eq!(
            build_plot_permit(&issuer_master, &server, "p3", &grantee, s, now, now).err(),
            Some(PlotPermitError::Expired),
            "never minted run out"
        );
    }

    /// A PERMIT IS GOOD ONLY ON THE SERVER IT WAS GIVEN ON (the coordinator's review of Wave 0,
    /// 2026-10-05). Every server hands out its plots in the ship file's order, so one person often
    /// holds the same plot id on several servers: here Ada holds p1 on servers A and B. The
    /// permit she gives Bo on A lets him build on her p1 there, and the same permit shown to B,
    /// whose relay rebuilds the words with ITS OWN server id, is a bad signature. A permit she
    /// gives on B works on B and not on A. Nothing else differs between the two: holder, plot,
    /// grantee and end date are the same.
    /// Seen red 2026-10-05 with the server left out of `plot_permit_preimage`'s words: "given on
    /// A, refused on B", left `Ok(())`, right `Err(BadSignature)`.
    #[test]
    fn a_permit_given_on_one_server_is_refused_on_another() {
        let ada_master = [0x11u8; 32];
        let ada_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&ada_master)).public_key());
        let bo = crate::relay::core::did::did_for_pubkey(
            &DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x22u8; 32])).public_key(),
        );
        let (server_a, server_b) = (server_did_of(0x55), server_did_of(0x66));
        assert_ne!(server_a, server_b, "two servers, two identities");
        let now = 1_760_000_000u64;
        let expiry = now + 30 * 86_400;
        let given_on_a = build_plot_permit(&ada_master, &server_a, "p1", &bo, PERMIT_SERIAL, expiry, now).expect("minted on A");
        let given_on_b = build_plot_permit(&ada_master, &server_b, "p1", &bo, PERMIT_SERIAL, expiry, now).expect("minted on B");
        // Each relay passes its own server id, never one the permit claims.
        let on = |server: &str, sig: &str| verify_plot_permit(&ada_hex, server, "p1", &bo, PERMIT_SERIAL, expiry, sig, now);
        assert_eq!(on(&server_a, &given_on_a), Ok(()), "given on A, used on A");
        assert_eq!(on(&server_b, &given_on_a), Err(PlotPermitError::BadSignature), "given on A, refused on B");
        assert_eq!(on(&server_b, &given_on_b), Ok(()), "given on B, used on B");
        assert_eq!(on(&server_a, &given_on_b), Err(PlotPermitError::BadSignature), "given on B, refused on A");
    }

    /// NO PERMIT RUNS MORE THAN 90 DAYS (the operator, 2026-10-05: every household permit has an
    /// end date, renewable; since v2 one can also be taken back sooner, but none is endless).
    /// Permits signed by hand, as a modified game could make them, so the check is shown
    /// not to lean on the builder's: 90 days verifies, and so does 90 days minted on a clock an
    /// hour ahead of the relay's; 92 days, a year and "endless" are refused as too long; the old
    /// "no end" zero is long past. The builder never mints one over the ceiling. The words a
    /// player reads name the ceiling the check enforces.
    /// Seen red 2026-10-05 with the ceiling's line removed from `plot_permit_window`: "92 days",
    /// left `Ok(())`, right `Err(TooLong)`.
    #[test]
    fn a_permit_longer_than_90_days_is_refused() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        assert_eq!(crate::systems::construction::shared::PERMIT_MAX_DAYS, PLOT_PERMIT_MAX_DAYS, "the words name the ceiling enforced");
        let issuer_master = [0x11u8; 32];
        let issuer = DilithiumKeypair::from_seed(&derive_dilithium_seed(&issuer_master));
        let issuer_hex = hex::encode(issuer.public_key());
        let grantee = "did:hum:4dQe1bVHyiHm1Vh8rWbx2F";
        let server = server_did_of(0x55);
        let (now, day) = (1_760_000_000u64, 86_400u64);
        let check = |expiry: u64| {
            let sig = B64.encode(issuer.sign(plot_permit_preimage(&server, "p3", grantee, PERMIT_SERIAL, expiry).as_bytes()));
            verify_plot_permit(&issuer_hex, &server, "p3", grantee, PERMIT_SERIAL, expiry, &sig, now)
        };
        assert_eq!(check(now + 90 * day), Ok(()), "90 days, the most a permit may run");
        assert_eq!(check(now + 90 * day + 3600), Ok(()), "minted on a clock an hour ahead of the relay's");
        let too_long = Err(PlotPermitError::TooLong);
        assert_eq!(check(now + 92 * day), too_long, "92 days");
        assert_eq!(check(now + 365 * day), too_long, "a year");
        assert_eq!(check(u64::MAX), too_long, "endless");
        assert_eq!(check(0), Err(PlotPermitError::Expired), "the old no-end zero is long past");
        assert_eq!(
            build_plot_permit(&issuer_master, &server, "p3", grantee, PERMIT_SERIAL, now + 90 * day + 1, now).err(),
            Some(PlotPermitError::TooLong),
            "never minted over it"
        );
        assert!(build_plot_permit(&issuer_master, &server, "p3", grantee, PERMIT_SERIAL, now + 90 * day, now).is_ok());
    }

    /// FILE PASS ROUND TRIP + A PINNED PREIMAGE (device-to-device big files,
    /// docs/design/media-showcase.md, the shapes part 1). The signed words are frozen here
    /// because both apps mint passes too and the relay checks them; the shared fixture
    /// (scripts/tests/fixtures/file-offer-v1.json) carries one minted pass's words for the apps'
    /// tests. A pass minted by the SHIPPED builder verifies, to its last second, and reads back
    /// its id and end date; each signed field matters (another server, issuer, grantee, id or end
    /// date is a bad signature, as are words that are not a signature and a key that is not a
    /// key); it runs out at its expiry.
    /// Seen red 2026-10-11 with the server left out of `file_pass_preimage`'s words: left
    /// `"hum/file-pass/v1\nAABB\nCCDD\n00112233445566778899aabbccddeeff\n1760604800"`, right
    /// `"hum/file-pass/v1\ndid:hum:srv\nAABB\nCCDD\n00112233445566778899aabbccddeeff\n1760604800"`.
    #[test]
    fn file_pass_roundtrip_and_pinned_preimage() {
        assert_eq!(
            file_pass_preimage("did:hum:srv", "AABB", "CCDD", "00112233445566778899aabbccddeeff", 1_760_604_800),
            "hum/file-pass/v1\ndid:hum:srv\nAABB\nCCDD\n00112233445566778899aabbccddeeff\n1760604800"
        );
        let sender_master = [0x11u8; 32];
        let (sender, receiver, other) = (key_hex_of(0x11), key_hex_of(0x22), key_hex_of(0x33));
        let (server, other_server) = (server_did_of(0x55), server_did_of(0x66));
        let id = new_file_pass_id().expect("the OS gives randomness");
        assert!(file_pass_id_ok(&id), "a fresh id is an id: {id}");
        let now = 1_760_000_000u64;
        let expiry = now + 3 * 86_400;
        let text = build_file_pass(&sender_master, &server, &sender, &receiver, &id, expiry, now).expect("a pass is minted");
        assert!(text.starts_with(&format!("{{\"v\":1,\"id\":\"{id}\",\"expiry\":{expiry},\"sig\":\"")), "the spec's key order: {}", &text[..80]);
        let pass: serde_json::Value = serde_json::from_str(&text).expect("the pass is JSON");
        assert_eq!(verify_file_pass(&server, &sender, &receiver, &pass, now), Ok(FilePass { id: id.clone(), expiry }));
        assert!(verify_file_pass(&server, &sender, &receiver, &pass, expiry - 1).is_ok(), "good to its last second");
        assert_eq!(verify_file_pass(&server, &sender, &receiver, &pass, expiry), Err(FilePassError::Expired), "run out at its expiry");
        let bad = Err(FilePassError::BadSignature);
        assert_eq!(verify_file_pass(&other_server, &sender, &receiver, &pass, now), bad, "another server");
        assert_eq!(verify_file_pass(&server, &other, &receiver, &pass, now), bad, "another issuer");
        assert_eq!(verify_file_pass(&server, &sender, &other, &pass, now), bad, "another grantee");
        assert_eq!(verify_file_pass(&server, &receiver, &sender, &pass, now), bad, "the wrong way round");
        assert_eq!(verify_file_pass(&server, "zz", &receiver, &pass, now), bad, "not a key");
        let with = |k: &str, val: serde_json::Value| {
            let mut w = pass.clone();
            w[k] = val;
            verify_file_pass(&server, &sender, &receiver, &w, now)
        };
        assert_eq!(with("id", "ffeeddccbbaa99887766554433221100".into()), bad, "another file");
        assert_eq!(with("expiry", (expiry + 60).into()), bad, "another end date");
        assert_eq!(with("sig", "bm90LWEtc2ln".into()), bad, "words that are not a signature");
    }

    /// A FILE PASS RUNS A WEEK AT MOST, AND ONLY ITS ONE SHAPE IS READ. Passes signed by hand, as
    /// a modified app could make them, so the check is shown not to lean on the builder's: 7 days
    /// verifies, and so does 7 days minted on a clock an hour ahead of the relay's; 9 days, a
    /// year and "endless" are refused as too long; one already run out is refused. The builder
    /// never mints one over the week or run out. A `v` other than 1, an id not 16 bytes of
    /// lowercase hex, a missing or overlong signature, and anything not an object are refused
    /// before any signature is checked; ids that could frame two passes alike are refused by the
    /// minting and the check both.
    /// Seen red 2026-10-11 with the ceiling's line removed from `file_pass_window`: "9 days",
    /// left `Ok(FilePass { id: "0011..", expiry: 1760777600 })`, right `Err(TooLong)`.
    #[test]
    fn a_file_pass_longer_than_a_week_or_malformed_is_refused() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let sender_master = [0x11u8; 32];
        let sender_kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(&sender_master));
        let (sender, receiver) = (key_hex_of(0x11), key_hex_of(0x22));
        let server = server_did_of(0x55);
        let id = "00112233445566778899aabbccddeeff";
        let (now, day) = (1_760_000_000u64, 86_400u64);
        let signed = |expiry: u64| {
            let sig = B64.encode(sender_kp.sign(file_pass_preimage(&server, &sender, &receiver, id, expiry).as_bytes()));
            serde_json::json!({ "v": 1, "id": id, "expiry": expiry, "sig": sig })
        };
        let check = |expiry: u64| verify_file_pass(&server, &sender, &receiver, &signed(expiry), now);
        assert!(check(now + 7 * day).is_ok(), "7 days, the most a pass may run");
        assert!(check(now + 7 * day + 3600).is_ok(), "minted on a clock an hour ahead of the relay's");
        let too_long = Err(FilePassError::TooLong);
        assert_eq!(check(now + 9 * day), too_long, "9 days");
        assert_eq!(check(now + 365 * day), too_long, "a year");
        assert_eq!(check(u64::MAX), too_long, "endless");
        assert_eq!(check(now - 1), Err(FilePassError::Expired), "already run out");
        let mint = |id: &str, expiry: u64| build_file_pass(&sender_master, &server, &sender, &receiver, id, expiry, now).err();
        assert_eq!(mint(id, now + 7 * day + 1), Some(FilePassError::TooLong), "never minted over a week");
        assert_eq!(mint(id, now), Some(FilePassError::Expired), "never minted run out");
        assert_eq!(mint(id, now + 7 * day), None);
        assert_eq!(mint("0011", now + day), Some(FilePassError::BadId), "a short id");
        assert_eq!(mint(&id.to_uppercase(), now + day), Some(FilePassError::BadId), "upper case");
        assert_eq!(
            build_file_pass(&sender_master, &server, &sender, "a\nb", id, now + day, now).err(),
            Some(FilePassError::Malformed),
            "a line break in a key"
        );

        let good = signed(now + day);
        let with = |k: &str, val: serde_json::Value| {
            let mut w = good.clone();
            w[k] = val;
            verify_file_pass(&server, &sender, &receiver, &w, now)
        };
        assert!(with("v", 1.into()).is_ok(), "the untouched pass, rebuilt, still verifies");
        let malformed = Err(FilePassError::Malformed);
        assert_eq!(with("v", 2.into()), malformed, "v2");
        assert_eq!(with("sig", serde_json::Value::Null), malformed, "no signature");
        assert_eq!(with("expiry", "1760086400".into()), malformed, "an end date as text");
        assert_eq!(with("sig", "A".repeat(FILE_PASS_SIG_MAX_LEN + 1).into()), malformed, "too long to read");
        assert_eq!(with("id", "00112233445566778899AABBCCDDEEFF".into()), Err(FilePassError::BadId), "upper case");
        assert_eq!(with("id", "0011".into()), Err(FilePassError::BadId), "too short");
        assert_eq!(verify_file_pass(&server, &sender, &receiver, &serde_json::json!(good.to_string()), now), malformed, "the pass as text, not an object");
        assert_eq!(verify_file_pass("", &sender, &receiver, &good, now), malformed, "no server");
        assert_eq!(verify_file_pass(&server, &sender, "a\nb", &good, now), malformed, "a line break in an id");
    }

    /// THE SHARED FIXTURE BOTH APPS READ (scripts/tests/fixtures/file-offer-v1.json) IS WHAT THIS
    /// CODE MAKES. ML-DSA signing here is the deterministic variant, so the builder reproduces the
    /// fixture's pass byte for byte from its seeds, and the pass verifies at its minting time with
    /// this relay's check. The fixture's keys, server DID and preimage text are what the seeds
    /// give; its file (made from the rule it states) hashes to its `blake3`, its pieces to
    /// `piece_hashes`, and those to `hashes`; and its three note texts are the spec's shapes
    /// (the shapes part 3) with those values, keys in the spec's order, compact.
    /// Seen red 2026-10-11 with `build_file_pass` writing `expiry` before `id`: "the builder
    /// reproduces the fixture's pass byte for byte" failed (left `{"v":1,"expiry":1760604800,..`,
    /// right `{"v":1,"id":"0011..`).
    #[test]
    fn file_pass_fixture_matches_the_builder() {
        let fx: serde_json::Value =
            serde_json::from_str(include_str!("../../../scripts/tests/fixtures/file-offer-v1.json")).expect("the fixture is JSON");
        let s = |k: &str| fx[k].as_str().unwrap_or_else(|| panic!("fixture field {k}")).to_string();
        let n = |k: &str| fx[k].as_u64().unwrap_or_else(|| panic!("fixture field {k}"));
        let master = [n("issuer_master_byte") as u8; 32];
        let issuer = key_hex_of(n("issuer_master_byte") as u8);
        let grantee = key_hex_of(n("grantee_master_byte") as u8);
        let server = server_did_of(n("server_master_byte") as u8);
        assert_eq!((s("issuer"), s("grantee"), s("server")), (issuer.clone(), grantee.clone(), server.clone()), "the keys and DID the seeds give");
        let (id, expiry, minted_at) = (s("id"), n("expiry"), n("minted_at"));
        assert_eq!(s("preimage"), file_pass_preimage(&server, &issuer, &grantee, &id, expiry), "the words signed");
        let minted = build_file_pass(&master, &server, &issuer, &grantee, &id, expiry, minted_at).expect("minted");
        assert_eq!(minted, s("pass_text"), "the builder reproduces the fixture's pass byte for byte");
        let pass = fx["pass"].clone();
        assert_eq!(pass, serde_json::from_str::<serde_json::Value>(&minted).unwrap(), "the pass object is the pass text");
        assert_eq!(pass["sig"].as_str(), Some(s("sig").as_str()));
        assert_eq!(verify_file_pass(&server, &issuer, &grantee, &pass, minted_at), Ok(FilePass { id: id.clone(), expiry }), "the frozen pass verifies");

        // The file: byte i is i mod 251, `size` bytes, cut into `piece`-byte pieces.
        let file = &fx["file"];
        let fsz = |k: &str| file[k].as_u64().unwrap_or_else(|| panic!("file field {k}"));
        let (size, piece) = (fsz("size"), fsz("piece"));
        let bytes: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
        let pieces: Vec<&[u8]> = bytes.chunks(piece as usize).collect();
        assert_eq!(pieces.len() as u64, size.div_ceil(piece), "pieces is ceil(size / piece)");
        assert_eq!(file["pieces"].as_u64(), Some(pieces.len() as u64));
        let piece_hashes: Vec<blake3::Hash> = pieces.iter().map(|p| blake3::hash(p)).collect();
        let listed: Vec<String> = file["piece_hashes"].as_array().unwrap().iter().map(|h| h.as_str().unwrap().to_string()).collect();
        assert_eq!(listed, piece_hashes.iter().map(|h| h.to_hex().to_string()).collect::<Vec<_>>(), "each piece's BLAKE3");
        let mut joined = Vec::new();
        for h in &piece_hashes {
            joined.extend_from_slice(h.as_bytes());
        }
        let hashes = blake3::hash(&joined).to_hex().to_string();
        let whole = blake3::hash(&bytes).to_hex().to_string();
        assert_eq!(file["hashes"].as_str(), Some(hashes.as_str()), "BLAKE3 of the piece hashes, 32 raw bytes each, in order");
        assert_eq!(file["blake3"].as_str(), Some(whole.as_str()), "BLAKE3 of the whole file");

        let name = serde_json::to_string(file["name"].as_str().unwrap()).unwrap();
        let kind = serde_json::to_string(file["type"].as_str().unwrap()).unwrap();
        let offer = format!(
            "[[hum:offer:v1]]{{\"id\":\"{id}\",\"name\":{name},\"size\":{size},\"type\":{kind},\"blake3\":\"{whole}\",\"piece\":{piece},\"pieces\":{},\"hashes\":\"{hashes}\",\"pass\":{minted}}}",
            pieces.len()
        );
        assert_eq!(s("offer_note"), offer, "the offer note");
        assert_eq!(s("accept_note"), format!("[[hum:offer-answer:v1]]{{\"id\":\"{id}\",\"accept\":true}}"), "the answer, yes");
        assert_eq!(s("decline_note"), format!("[[hum:offer-answer:v1]]{{\"id\":\"{id}\",\"accept\":false}}"), "the answer, no");
        assert_eq!(s("cancel_note"), format!("[[hum:offer-cancel:v1]]{{\"id\":\"{id}\"}}"), "a cancel");
    }

    /// REPORTS: THE WORDS A REPORTER SIGNS AND THE EVIDENCE HASH, PINNED (blocking-and-safe-
    /// mode.md 10e). The web client builds both too, and its Node test reads THESE literals:
    /// the `report_preimage(...)` call with its expected string, and each
    /// `report_evidence_hash(...)` call with its expected hex. The first hash is BLAKE3's own
    /// published value for no input, so the hash is the real BLAKE3 and not a look-alike; the
    /// second freezes the hash of compact evidence JSON as a client sends it. The DM inner words
    /// are pinned beside them (net::dm_pq and web crypto.js build them; the relay rebuilds them).
    /// Seen red 2026-10-09 with `REPORT_DOMAIN` set to "hum/report/v0": left
    /// `"hum/report/v0\nAABB\nCCDD\nharassment\naf13..3262\n1760000000000"`, right the same with
    /// `v1`.
    #[test]
    fn report_preimage_and_evidence_hash_are_pinned() {
        assert_eq!(
            report_preimage("AABB", "CCDD", "harassment", "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262", 1760000000000),
            "hum/report/v1\nAABB\nCCDD\nharassment\naf1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262\n1760000000000"
        );
        assert_eq!(report_evidence_hash(""), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
        assert_eq!(
            report_evidence_hash(r#"[{"kind":"group_text","from":"CCDD","ts":1760000000000,"text":"hi "}]"#),
            "20edb13876aa9ba4cf1932ec62c896c3e11beff7f1fb40f50b339547124e6299"
        );
        assert_eq!(dm_inner_preimage("AABB", "CCDD", 1700000000000, "hello"), "hum/dm/v2\nAABB\nCCDD\n1700000000000\nhello");
    }

    /// A REPORT'S SIGNATURE IS ITS REPORTER'S, OVER ITS OWN WORDS. One made by the shipped builder
    /// checks; the same signature does not check for another reporter, another target, another
    /// reason, other evidence or another time; a key that is not hex (or not even ASCII, which a
    /// careless decode would panic on) is simply not a signer.
    /// Seen red 2026-10-09 with `verify_report_sig` building its words with `target` in the
    /// reporter's place: "the report it made checks" failed.
    #[test]
    fn a_report_signature_checks_only_for_its_reporter_and_its_words() {
        let master = [0x41u8; 32];
        let (reporter, target, other) = (key_hex_of(0x41), key_hex_of(0x42), key_hex_of(0x43));
        let evidence = r#"[{"kind":"post","from":"x","timestamp":5}]"#;
        let hash = report_evidence_hash(evidence);
        let ts = 1_760_000_000_000u64;
        let sig = build_report_sig(&master, &reporter, &target, "spam", evidence, ts);
        assert!(verify_report_sig(&reporter, &target, "spam", &hash, ts, &sig), "the report it made checks");
        assert!(!verify_report_sig(&other, &target, "spam", &hash, ts, &sig), "another reporter");
        assert!(!verify_report_sig(&reporter, &other, "spam", &hash, ts, &sig), "another target");
        assert!(!verify_report_sig(&reporter, &target, "scam", &hash, ts, &sig), "another reason");
        assert!(!verify_report_sig(&reporter, &target, "spam", &report_evidence_hash("[]"), ts, &sig), "other evidence");
        assert!(!verify_report_sig(&reporter, &target, "spam", &hash, ts + 1, &sig), "another time");
        assert!(!verify_report_sig("zz", &target, "spam", &hash, ts, &sig), "not hex");
        assert!(!verify_report_sig("\u{e9}\u{e9}", &target, "spam", &hash, ts, &sig), "not ASCII, and no panic");
        assert!(!verify_report_sig(&reporter, &target, "spam", &hash, ts, "bm90LWEtc2ln"), "not a signature");
    }

    /// A DM HANDED OVER AS EVIDENCE CHECKS ONLY AS WHAT IT IS: from its sender, to the person it
    /// was sent to, with its own time and text. Signed the way every client signs a DM (the
    /// sender's key over [`dm_inner_preimage`]); the relay passes the reported person as `from`
    /// and the reporter as `to`. A message pinned on someone else (another sender), one sent to
    /// someone else and handed over by the reporter, an edited text and an edited time are all
    /// refused, and a sender that is not even ASCII is refused without a panic.
    /// Seen red 2026-10-09 with `dm_inner_preimage` leaving `to` out of its words (so the signing
    /// here and the check both did): "sent to someone else" checked.
    #[test]
    fn a_dm_handed_over_as_evidence_checks_only_as_what_it_is() {
        use base64::{engine::general_purpose::STANDARD as B64, Engine};
        let sender = DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x51u8; 32]));
        let sender_hex = hex::encode(sender.public_key());
        let (reporter, someone_else) = (key_hex_of(0x52), key_hex_of(0x53));
        let ts = 1_760_000_000_000u64;
        let sig = B64.encode(sender.sign(dm_inner_preimage(&sender_hex, &reporter, ts, "you will regret this").as_bytes()));
        assert!(verify_dm_inner(&sender_hex, &reporter, ts, "you will regret this", &sig), "a genuine message checks");
        assert!(!verify_dm_inner(&someone_else, &reporter, ts, "you will regret this", &sig), "pinned on someone else");
        assert!(!verify_dm_inner(&sender_hex, &someone_else, ts, "you will regret this", &sig), "sent to someone else");
        assert!(!verify_dm_inner(&sender_hex, &reporter, ts, "you will regret this!", &sig), "an edited text");
        assert!(!verify_dm_inner(&sender_hex, &reporter, ts + 1, "you will regret this", &sig), "an edited time");
        assert!(!verify_dm_inner("\u{e9}\u{e9}", &reporter, ts, "you will regret this", &sig), "not ASCII, and no panic");
    }

    /// THE RELAY REBUILDS THE DESKTOP APP'S DM WORDS BYTE FOR BYTE: a payload made by the native
    /// builder itself (net::dm_pq `build_signed_inner`, padding and all) checks with the relay's
    /// verifier. Native builds only: the relay build has no net::dm_pq.
    /// Seen red 2026-10-09 with `DM_INNER_DOMAIN` set to "hum/dm/v1": "the desktop app's DM
    /// checks at the relay" failed.
    #[cfg(feature = "native")]
    #[test]
    fn the_relay_rebuilds_the_desktop_apps_dm_words_byte_for_byte() {
        let seed = [0x61u8; 32];
        let from = key_hex_of(0x61);
        let to = key_hex_of(0x62);
        let inner = crate::net::dm_pq::build_signed_inner(&seed, &from, &to, 1_760_000_000_123, "line one\nline two").unwrap();
        let v: serde_json::Value = serde_json::from_str(&inner).unwrap();
        let (ts, text, sig) = (v["ts"].as_u64().unwrap(), v["text"].as_str().unwrap(), v["sig"].as_str().unwrap());
        assert!(verify_dm_inner(&from, &to, ts, text, sig), "the desktop app's DM checks at the relay");
    }
}
