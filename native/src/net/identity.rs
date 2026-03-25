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
    // Parse and validate the mnemonic (checks word list + checksum)
    let mnemonic = bip39::Mnemonic::parse(mnemonic_str)
        .map_err(|e| format!("Invalid seed phrase: {}", e))?;

    // Extract the raw entropy (32 bytes for a 24-word / 256-bit mnemonic).
    // This matches the web's seedFromMnemonic() which decodes words back to
    // the original 32-byte seed — NOT the PBKDF2-derived 64-byte seed.
    let entropy = mnemonic.to_entropy();
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
