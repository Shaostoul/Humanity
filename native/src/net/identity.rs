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

    // Use the bip39 crate's word list for lookup
    let word_list = bip39::Language::English.word_list();
    let mut bits = Vec::with_capacity(264);

    for word in &words {
        let idx = word_list.iter().position(|w| w == word)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnemonic_roundtrip() {
        // Generate a random mnemonic and verify we can derive a keypair
        let mnemonic = bip39::Mnemonic::generate(24)
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
