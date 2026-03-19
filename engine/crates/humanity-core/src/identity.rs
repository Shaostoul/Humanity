//! Identity key management.
//!
//! Handles Ed25519 keypair generation and public key representation.
//! Private keys stay client-side — this module provides the tools
//! to create and work with them.

use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;

/// A user identity keypair (Ed25519).
///
/// The signing key (private) never leaves the client.
/// The verifying key (public) is shared with the network.
pub struct Keypair {
    signing: SigningKey,
}

impl Keypair {
    /// Generate a new random keypair using the OS cryptographic RNG.
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        Self { signing }
    }

    /// Create a keypair from existing secret key bytes (32 bytes).
    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Self {
        let signing = SigningKey::from_bytes(bytes);
        Self { signing }
    }

    /// Get the signing key (private). Handle with care.
    pub fn signing_key(&self) -> &SigningKey {
        &self.signing
    }

    /// Get the verifying key (public). Safe to share.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signing.verifying_key()
    }

    /// Get the public key as raw bytes (32 bytes).
    pub fn public_key_bytes(&self) -> [u8; 32] {
        self.verifying_key().to_bytes()
    }

    /// Get the secret key as raw bytes (32 bytes).
    /// Be very careful with this — it's the private key.
    pub fn secret_key_bytes(&self) -> &[u8; 32] {
        self.signing.as_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_keypair() {
        let kp = Keypair::generate();
        let pubkey = kp.public_key_bytes();
        assert_eq!(pubkey.len(), 32);
        // Public key should not be all zeros
        assert!(pubkey.iter().any(|&b| b != 0));
    }

    #[test]
    fn roundtrip_from_secret_bytes() {
        let kp1 = Keypair::generate();
        let secret = *kp1.secret_key_bytes();
        let kp2 = Keypair::from_secret_bytes(&secret);
        assert_eq!(kp1.public_key_bytes(), kp2.public_key_bytes());
    }
}
