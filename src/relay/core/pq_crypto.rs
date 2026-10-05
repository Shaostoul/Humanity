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

// ── Friendship certificates (follows-graph removal, 2026-08-24) ────────────
// The `follows` table was the last server-side social graph. It is gone:
// friendship is now a CLIENT-HELD credential. When two people become
// friends, each issues the other a certificate:
//
//   cert = Dilithium_issuer("hum/friend/v1\n{issuer_hex}\n{grantee_hex}")
//
// The grantee presents it on every dm_put addressed to the issuer (and on
// friends-visibility profile requests). The relay verifies STATELESSLY and
// stores nothing — a subpoena or breach finds no who-is-friends-with-whom
// data because it is never recorded. Certs are minted by the CLIENT (the
// issuer's seed never leaves their machine) and delivered over the sealed
// mailbox; `build_friend_cert` below is that minting step.
// Known v1 limitation (documented): certs do not expire and cannot be
// server-side revoked; "unfriending" is client-side (your client stops
// showing them; their mail still lands under the knock budget rules).
//
// MINT AND CHECK LIVE TOGETHER ON PURPOSE (2026-09-19). `build_friend_cert`
// used to sit in `net::dm_pq`, which is native-gated, so the relay's own DM
// tests reached across a feature boundary to mint a cert and the whole relay
// test target stopped compiling. Nothing in the builder is native: it is the
// three primitives already in this file (`derive_dilithium_seed`,
// `DilithiumKeypair::sign`, `friend_cert_preimage`). Keeping the two halves of
// one wire format side by side is also how they stay in agreement with the
// web client, which builds the same string inline.

/// Certificate signature domain. Web MUST use the identical string.
pub const FRIEND_CERT_DOMAIN: &str = "hum/friend/v1";

/// The preimage an issuer signs to authorize a grantee.
pub fn friend_cert_preimage(issuer_hex: &str, grantee_hex: &str) -> String {
    format!("{FRIEND_CERT_DOMAIN}\n{issuer_hex}\n{grantee_hex}")
}

/// Mint MY friendship certificate for `grantee_hex`: a base64 Dilithium3
/// signature over `friend_cert_preimage`, made with the Dilithium key derived
/// from my BIP39 `seed`. Handed to the grantee via a sealed control message;
/// they present it on every dm_put addressed to me, and the relay checks it
/// with `verify_friend_cert` without storing a thing.
pub fn build_friend_cert(seed: &[u8], my_hex: &str, grantee_hex: &str) -> String {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let dil_seed = derive_dilithium_seed(seed);
    let kp = DilithiumKeypair::from_seed(&dil_seed);
    B64.encode(kp.sign(friend_cert_preimage(my_hex, grantee_hex).as_bytes()))
}

/// Verify a friendship certificate: did `issuer_hex` really authorize
/// `grantee_hex`? Stateless.
pub fn verify_friend_cert(issuer_hex: &str, grantee_hex: &str, cert_b64: &str) -> bool {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    let Ok(issuer_pk) = hex_decode_str(issuer_hex) else { return false };
    let Ok(sig) = B64.decode(cert_b64.trim()) else { return false };
    verify_dilithium(
        &issuer_pk,
        friend_cert_preimage(issuer_hex, grantee_hex).as_bytes(),
        &sig,
    )
    .is_ok()
}

// ── Household permits (ship homes increment 5, 2026-10-05) ─────────────────
// In a server's shared world a player builds only on their own plot. A HOUSEHOLD PERMIT lets
// someone else build there too: the plot's holder signs
//
//   permit = Dilithium_holder("hum/permit/v1\n{plot}\n{grantee}\n{expiry}")
//
// where `plot` is the plot's id in the ship file ("p3"), `grantee` is the id the relay holds
// plots under for the person let in (`relay::storage::plots::plot_owner_id`: their `did:hum:`),
// and `expiry` is when it runs out, Unix seconds. The grantee keeps it and sends it with each
// build or take-down they make on that plot (systems/construction/shared.rs `Permit`). The
// relay checks it STATELESSLY, the friendship certificate's pattern, and stores nothing.
//
// What the relay checks, split between this function and its caller (the build handler):
// - here: the signature is the issuer's over words the RELAY rebuilds from what it knows (the
//   frame's plot, the sender's own id), never over the plot and grantee the permit claims, so a
//   permit for p1 used on p2, or one given to someone else, fails as a bad signature; and the
//   end date: not passed, and not more than PLOT_PERMIT_MAX_DAYS ahead;
// - the caller: the issuer (`plot_owner_id` of the key in the permit) is who holds that plot on
//   this ship NOW, so a permit stops working when the plot changes hands.
//
// NO ENDLESS PERMITS (the operator, 2026-10-05). The relay keeps no list of permits, so none can
// be withdrawn before it runs out; every permit therefore has an end date at most 90 days ahead,
// and is renewed by signing a new one. An expiry of 0, which the first design used for "a
// household member, no end", is simply long past. MINT AND CHECK LIVE TOGETHER, as the
// friendship certificate's do: the minting refuses what the check would refuse.
//
// Known limit (v1, 2026-10-05): the signed words name no server. Someone who holds the same plot
// id on two servers (likely: plots are handed out in the ship file's order, so many people hold
// p1 somewhere) lets the grantee in on both. Naming the server (its `did:hum:`, which
// /api/server-info gives) in the signed words would close it; to be settled before the household
// page (increment 5b) mints real permits, while nothing has been signed in this format yet.

/// Household-permit signature domain: the first line of what is signed.
pub const PLOT_PERMIT_DOMAIN: &str = "hum/permit/v1";

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

/// The longest a permit's plot id or grantee id may be, bytes (a `did:hum:` is about 30).
pub const PLOT_PERMIT_FIELD_MAX_LEN: usize = 128;

/// What a household permit's issuer signs.
pub fn plot_permit_preimage(plot: &str, grantee: &str, expiry: u64) -> String {
    format!("{PLOT_PERMIT_DOMAIN}\n{plot}\n{grantee}\n{expiry}")
}

/// Why a household permit was not minted, or does not let its holder in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlotPermitError {
    /// A plot or grantee id that is empty, longer than [`PLOT_PERMIT_FIELD_MAX_LEN`], or holds a
    /// line break (which would let two different permits sign the same words).
    Malformed,
    /// It has run out: the clock has reached its `expiry`.
    Expired,
    /// It runs out more than [`PLOT_PERMIT_MAX_DAYS`] days ahead.
    TooLong,
    /// The issuer's key or the signature is not well formed, or the signature is not the
    /// issuer's over these words.
    BadSignature,
}

/// A plot or grantee id fit to sign: not empty, not too long, no line break.
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

/// Mint MY household permit for `grantee` on my plot `plot`, running out at `expiry` (Unix
/// seconds): a base64 Dilithium3 signature over [`plot_permit_preimage`], made with the Dilithium
/// key derived from my BIP39 `seed`, as [`build_friend_cert`] makes a friendship note. `now` is
/// my clock. Refused, minting nothing, when [`verify_plot_permit`] would refuse it: a malformed
/// id, an end date already past, or one more than [`PLOT_PERMIT_MAX_DAYS`] days ahead.
pub fn build_plot_permit(
    seed: &[u8],
    plot: &str,
    grantee: &str,
    expiry: u64,
    now: u64,
) -> std::result::Result<String, PlotPermitError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if !plot_permit_field_ok(plot) || !plot_permit_field_ok(grantee) {
        return Err(PlotPermitError::Malformed);
    }
    plot_permit_window(expiry, now, 0)?;
    let kp = DilithiumKeypair::from_seed(&derive_dilithium_seed(seed));
    Ok(B64.encode(kp.sign(plot_permit_preimage(plot, grantee, expiry).as_bytes())))
}

/// Does `sig_b64` let `grantee` build on `plot` until `expiry`, signed by `issuer_hex` (a
/// Dilithium3 public key, hex)? Stateless. `plot` and `grantee` must be the RELAY's own facts
/// (the frame's plot, the sender's `plot_owner_id`), not what the permit claims. `now` is the
/// relay's clock, Unix seconds. Checked cheapest first, so a stale or absurd permit costs no
/// signature check: the ids, the end date (not passed, not more than [`PLOT_PERMIT_MAX_DAYS`]
/// days plus [`PLOT_PERMIT_CLOCK_SLACK_SECS`] ahead), then the signature. Whether the issuer
/// holds the plot is the caller's question (see the note above).
pub fn verify_plot_permit(
    issuer_hex: &str,
    plot: &str,
    grantee: &str,
    expiry: u64,
    sig_b64: &str,
    now: u64,
) -> std::result::Result<(), PlotPermitError> {
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    if !plot_permit_field_ok(plot) || !plot_permit_field_ok(grantee) {
        return Err(PlotPermitError::Malformed);
    }
    plot_permit_window(expiry, now, PLOT_PERMIT_CLOCK_SLACK_SECS)?;
    let Ok(issuer_pk) = hex_decode_str(issuer_hex) else { return Err(PlotPermitError::BadSignature) };
    let Ok(sig) = B64.decode(sig_b64.trim()) else { return Err(PlotPermitError::BadSignature) };
    verify_dilithium(&issuer_pk, plot_permit_preimage(plot, grantee, expiry).as_bytes(), &sig)
        .map_err(|_| PlotPermitError::BadSignature)
}

/// Local hex decode (the `hex` crate is native-gated in some builds; the
/// relay feature carries it too, but a dependency-free decode keeps this
/// function unconditionally available).
fn hex_decode_str(s: &str) -> std::result::Result<Vec<u8>, ()> {
    let s = s.trim();
    if s.len() % 2 != 0 {
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

    /// Friendship-certificate roundtrip + a PINNED preimage. The web
    /// client builds the exact same string inline (chat-privacy/crypto.js
    /// `pqBuildFriendCert`); if this format ever changes, cross-client
    /// friendship silently breaks, so the preimage bytes are frozen here.
    #[test]
    fn friend_cert_roundtrip_and_pinned_preimage() {
        // Pinned wire format — web MUST match byte-for-byte.
        assert_eq!(
            friend_cert_preimage("AABB", "CCDD"),
            "hum/friend/v1\nAABB\nCCDD"
        );
        // Real issue + verify, through the SHIPPED minting function rather
        // than a hand-rolled copy of it: a builder the tests re-implement is a
        // builder nothing checks.
        let issuer_master = [0x11u8; 32];
        let dil_seed = derive_dilithium_seed(&issuer_master);
        let issuer = DilithiumKeypair::from_seed(&dil_seed);
        let issuer_hex = hex::encode(issuer.public_key());
        let grantee_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x22u8; 32])).public_key());
        let cert = build_friend_cert(&issuer_master, &issuer_hex, &grantee_hex);
        assert!(verify_friend_cert(&issuer_hex, &grantee_hex, &cert), "valid cert must verify");
        // Wrong grantee, wrong issuer, and garbage all fail.
        assert!(!verify_friend_cert(&issuer_hex, "deadbeef", &cert));
        assert!(!verify_friend_cert("deadbeef", &grantee_hex, &cert));
        assert!(!verify_friend_cert(&issuer_hex, &grantee_hex, "bm90LWEtc2ln"));
    }

    /// HOUSEHOLD PERMIT ROUND TRIP + A PINNED PREIMAGE (ship homes increment 5). The signed words
    /// are frozen here, because the game that mints a permit and the relay that checks it must
    /// build them byte for byte alike. A permit minted by the SHIPPED builder verifies, to its
    /// last second; each signed field matters (another plot, grantee, end date or signer is a
    /// bad signature, as are words that are not a signature and a key that is not a key); it
    /// runs out at its expiry; and ids that could frame two permits alike are refused by the
    /// minting and the check both, as is minting one already run out.
    /// Seen red 2026-10-05 with the plot and grantee swapped in `plot_permit_preimage`: left
    /// `"hum/permit/v1\ndid:hum:abc\np3\n0"`, right `"hum/permit/v1\np3\ndid:hum:abc\n0"`.
    #[test]
    fn plot_permit_roundtrip_and_pinned_preimage() {
        assert_eq!(plot_permit_preimage("p3", "did:hum:abc", 0), "hum/permit/v1\np3\ndid:hum:abc\n0");
        let issuer_master = [0x11u8; 32];
        let issuer_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&issuer_master)).public_key());
        let grantee = crate::relay::core::did::did_for_pubkey(
            &DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x22u8; 32])).public_key(),
        );
        let other_hex = hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[0x33u8; 32])).public_key());
        let now = 1_760_000_000u64;
        let expiry = now + 30 * 86_400;
        let sig = build_plot_permit(&issuer_master, "p3", &grantee, expiry, now).expect("a 30-day permit is minted");
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", &grantee, expiry, &sig, now), Ok(()));
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", &grantee, expiry, &sig, expiry - 1), Ok(()), "good to its last second");
        let bad = Err(PlotPermitError::BadSignature);
        assert_eq!(verify_plot_permit(&issuer_hex, "p4", &grantee, expiry, &sig, now), bad, "another plot");
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", "did:hum:someoneelse", expiry, &sig, now), bad, "another grantee");
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", &grantee, expiry + 1, &sig, now), bad, "another end date");
        assert_eq!(verify_plot_permit(&other_hex, "p3", &grantee, expiry, &sig, now), bad, "another signer");
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", &grantee, expiry, "bm90LWEtc2ln", now), bad, "not a signature");
        assert_eq!(verify_plot_permit("zz", "p3", &grantee, expiry, &sig, now), bad, "not a key");
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", &grantee, expiry, &sig, expiry), Err(PlotPermitError::Expired));
        let malformed = Some(PlotPermitError::Malformed);
        assert_eq!(build_plot_permit(&issuer_master, "p3\ndid:hum:x", "y", expiry, now).err(), malformed, "a line break in an id");
        assert_eq!(verify_plot_permit(&issuer_hex, "p3", "", expiry, &sig, now).err(), malformed, "an empty grantee");
        assert_eq!(verify_plot_permit(&issuer_hex, &"p".repeat(129), &grantee, expiry, &sig, now).err(), malformed, "an id over the cap");
        assert_eq!(build_plot_permit(&issuer_master, "p3", &grantee, now, now).err(), Some(PlotPermitError::Expired), "never minted run out");
    }

    /// NO PERMIT RUNS MORE THAN 90 DAYS (the operator, 2026-10-05: every household permit has an
    /// end date, renewable; the relay keeps no list, so an endless permit could never be taken
    /// back). Permits signed by hand, as a modified game could make them, so the check is shown
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
        let (now, day) = (1_760_000_000u64, 86_400u64);
        let check = |expiry: u64| {
            let sig = B64.encode(issuer.sign(plot_permit_preimage("p3", grantee, expiry).as_bytes()));
            verify_plot_permit(&issuer_hex, "p3", grantee, expiry, &sig, now)
        };
        assert_eq!(check(now + 90 * day), Ok(()), "90 days, the most a permit may run");
        assert_eq!(check(now + 90 * day + 3600), Ok(()), "minted on a clock an hour ahead of the relay's");
        let too_long = Err(PlotPermitError::TooLong);
        assert_eq!(check(now + 92 * day), too_long, "92 days");
        assert_eq!(check(now + 365 * day), too_long, "a year");
        assert_eq!(check(u64::MAX), too_long, "endless");
        assert_eq!(check(0), Err(PlotPermitError::Expired), "the old no-end zero is long past");
        assert_eq!(build_plot_permit(&issuer_master, "p3", grantee, now + 90 * day + 1, now).err(), Some(PlotPermitError::TooLong), "never minted over it");
        assert!(build_plot_permit(&issuer_master, "p3", grantee, now + 90 * day, now).is_ok());
    }
}
