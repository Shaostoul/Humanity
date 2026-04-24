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
}
