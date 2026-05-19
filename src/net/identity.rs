//! BIP39 seed phrase identity recovery.
//!
//! Matches the web client's key derivation: the 24-word BIP39 mnemonic
//! directly encodes the 32-byte Ed25519 seed as its entropy (not using
//! PBKDF2-based `to_seed()`). This ensures the same mnemonic produces
//! the same public key on both web and native.

/// Derive an Ed25519 keypair from a 24-word BIP39 mnemonic.
///
/// Returns `(public_key_hex, private_key_bytes)` on success.
/// The private_key_bytes are the raw 32-byte Ed25519 seed.
pub fn derive_keypair_from_mnemonic(mnemonic_str: &str) -> Result<(String, Vec<u8>), String> {
    // Try strict parsing first (validates checksum)
    let entropy = match bip39::Mnemonic::parse(mnemonic_str) {
        Ok(mnemonic) => mnemonic.to_entropy(),
        Err(_) => {
            // Fallback: manually decode words to entropy without checksum validation.
            // The web client's BIP39 implementation may produce slightly different
            // checksums in edge cases. The words themselves encode the seed correctly.
            decode_words_to_entropy(mnemonic_str)?
        }
    };
    if entropy.len() != 32 {
        return Err(format!("Expected 32-byte entropy, got {} bytes", entropy.len()));
    }

    let secret_key_bytes: [u8; 32] = entropy.try_into()
        .map_err(|_| "Entropy conversion failed".to_string())?;

    // Derive the Ed25519 public key from the seed
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&secret_key_bytes);
    let public_key = signing_key.verifying_key();
    let public_key_hex = hex::encode(public_key.as_bytes());

    Ok((public_key_hex, secret_key_bytes.to_vec()))
}

/// Manually decode BIP39 words to 32-byte entropy without checksum validation.
/// Each of 24 words maps to an 11-bit index in the BIP39 word list.
/// 24 words x 11 bits = 264 bits = 256 bits entropy + 8 bits checksum.
/// We extract only the 256 entropy bits (32 bytes).
fn decode_words_to_entropy(mnemonic_str: &str) -> Result<Vec<u8>, String> {
    let words: Vec<&str> = mnemonic_str.trim().split_whitespace().collect();
    if words.len() != 24 {
        return Err(format!("Expected 24 words, got {}", words.len()));
    }

    // Use our custom wordlist (matches the web client's bip39-english.js)
    let word_list = &super::bip39_wordlist::WORDLIST;
    let mut bits = Vec::with_capacity(264);

    for word in &words {
        let idx = word_list.iter().position(|w| *w == *word)
            .ok_or_else(|| format!("Unknown BIP39 word: '{}'", word))?;
        for bit in (0..11).rev() {
            bits.push(((idx >> bit) & 1) as u8);
        }
    }

    // First 256 bits are entropy (32 bytes), last 8 bits are checksum (ignored)
    let mut entropy = vec![0u8; 32];
    for i in 0..256 {
        if bits[i] == 1 {
            entropy[i / 8] |= 1 << (7 - (i % 8));
        }
    }

    Ok(entropy)
}

// ── Full-PQ identity (Dilithium3 id+signing, Kyber768 DM) ───────────────────
// Both keys derive from the SAME 32-byte BIP39 seed the Ed25519 key uses,
// byte-identical to the web client (crypto.js attachPqIdentity) and the
// relay — locked by pq_crypto::{dilithium,kyber}_cross_language_kat +
// scripts/pq-kat.mjs. Ed25519 is kept ONLY as the seed/Solana wallet.

/// The post-quantum identity presented to the relay.
pub struct PqIdentity {
    /// Dilithium3 public key hex — THE chat identity (`public_key`).
    pub dilithium_hex: String,
    /// Kyber768 public key base64 — advertised at identify for DMs.
    pub kyber_public_b64: String,
}

/// Derive the full-PQ identity from the 32-byte BIP39 seed.
pub fn derive_pq_identity(seed32: &[u8]) -> Result<PqIdentity, String> {
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    let dm = crate::net::dm_pq::DmPqKeypair::from_bip39_seed(seed32)?;
    Ok(PqIdentity {
        dilithium_hex: hex::encode(dil.public_key()),
        kyber_public_b64: dm.public_base64(),
    })
}

/// Dilithium3 chat signature (hex) over `content\ntimestamp` — the EXACT
/// preimage the web client signs and the relay verifies (Inc3).
pub fn pq_sign_chat(seed32: &[u8], content: &str, timestamp: u64) -> String {
    let dil_seed = crate::relay::core::pq_crypto::derive_dilithium_seed(seed32);
    let dil = crate::relay::core::pq_crypto::DilithiumKeypair::from_seed(&dil_seed);
    hex::encode(dil.sign(format!("{content}\n{timestamp}").as_bytes()))
}

/// Generate a fresh 32-byte BIP39 seed (256-bit entropy) from the OS
/// CSPRNG. This 32-byte value IS the identity — it deterministically
/// re-derives Ed25519 (Solana wallet), Dilithium3 (the chat identity),
/// and Kyber768 (DM). Persist it (encrypted) and back it up as the
/// 24-word phrase via `mnemonic_from_seed`.
pub fn generate_new_seed() -> Vec<u8> {
    let mut entropy = [0u8; 32];
    getrandom::getrandom(&mut entropy).expect("OS RNG failed");
    entropy.to_vec()
}

/// Render a 32-byte seed as its 24-word BIP39 mnemonic, for backup and
/// in-app display. Exact inverse of `derive_keypair_from_mnemonic`'s
/// entropy path: re-entering this phrase restores the same identity.
pub fn mnemonic_from_seed(seed: &[u8]) -> Option<String> {
    if seed.len() != 32 {
        return None;
    }
    bip39::Mnemonic::from_entropy(seed).ok().map(|m| m.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_seed_mnemonic_roundtrips_and_derives_pq() {
        // Generate → phrase → recover must yield the SAME 32-byte seed,
        // and that seed must derive a stable PQ identity. This is what
        // makes "Generate New Identity" + later recovery consistent.
        let seed = generate_new_seed();
        assert_eq!(seed.len(), 32);
        let phrase = mnemonic_from_seed(&seed).expect("mnemonic");
        assert_eq!(phrase.split_whitespace().count(), 24);
        let (_pk, recovered) =
            derive_keypair_from_mnemonic(&phrase).expect("recover");
        assert_eq!(recovered, seed, "phrase must restore the exact seed");
        let a = derive_pq_identity(&seed).unwrap();
        let b = derive_pq_identity(&recovered).unwrap();
        assert_eq!(a.dilithium_hex, b.dilithium_hex);
        assert_eq!(a.kyber_public_b64, b.kyber_public_b64);
        // Two generated seeds must differ (distinct identities).
        assert_ne!(generate_new_seed(), generate_new_seed());
    }

    #[test]
    fn pq_identity_deterministic_from_seed() {
        // Same seed → same Dilithium id + Kyber DM key (the property that
        // makes web↔native interop work; cross-language locked by KAT).
        let seed = [7u8; 32];
        let a = derive_pq_identity(&seed).unwrap();
        let b = derive_pq_identity(&seed).unwrap();
        assert_eq!(a.dilithium_hex, b.dilithium_hex);
        assert_eq!(a.kyber_public_b64, b.kyber_public_b64);
        assert_eq!(a.dilithium_hex.len(), 1952 * 2, "ML-DSA-65 pubkey hex");
        // Signature preimage matches the web/relay contract.
        let sig = pq_sign_chat(&seed, "hi", 123);
        assert_eq!(sig.len(), 3309 * 2, "ML-DSA-65 sig hex");
    }

    #[test]
    fn test_mnemonic_roundtrip() {
        // Generate a random mnemonic and verify we can derive a keypair.
        // bip39 0.7+ requires explicit entropy: 32 bytes = 256 bits = 24 words.
        let mut entropy = [0u8; 32];
        getrandom::getrandom(&mut entropy).expect("OS RNG failed");
        let mnemonic = bip39::Mnemonic::from_entropy(&entropy)
            .expect("Failed to generate mnemonic");
        let phrase = mnemonic.to_string();

        let (pubkey_hex, privkey) = derive_keypair_from_mnemonic(&phrase)
            .expect("Failed to derive keypair");

        assert_eq!(pubkey_hex.len(), 64, "Public key hex should be 64 chars");
        assert_eq!(privkey.len(), 32, "Private key should be 32 bytes");

        // Derive again — should produce the same key
        let (pubkey_hex2, _) = derive_keypair_from_mnemonic(&phrase)
            .expect("Failed to derive keypair second time");
        assert_eq!(pubkey_hex, pubkey_hex2, "Same mnemonic should produce same key");
    }

    #[test]
    fn test_invalid_mnemonic() {
        let result = derive_keypair_from_mnemonic("not a valid mnemonic phrase");
        assert!(result.is_err());
    }
}
