//! Ed25519 signing and verification.
//!
//! Objects are signed over their canonical CBOR bytes.
//! The signature field is included in the canonical bytes
//! (it's part of the object map), so signing works as:
//!
//! 1. Build the object map with a placeholder (empty) signature
//! 2. Set the signature field to the real signature
//! 3. Encode the complete object to canonical CBOR
//! 4. object_id = BLAKE3(canonical_bytes)
//!
//! Verification:
//! 1. Extract signature from the object
//! 2. Replace signature with empty bytes in the map
//! 3. Encode that to canonical CBOR
//! 4. Verify signature over those bytes

use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::error::{Error, Result};

/// Sign a message with an Ed25519 signing key.
/// Returns the 64-byte signature.
pub fn sign(signing_key: &SigningKey, message: &[u8]) -> [u8; 64] {
    let sig = signing_key.sign(message);
    sig.to_bytes()
}

/// Verify an Ed25519 signature.
pub fn verify(
    public_key_bytes: &[u8; 32],
    message: &[u8],
    signature_bytes: &[u8; 64],
) -> Result<()> {
    let verifying_key = VerifyingKey::from_bytes(public_key_bytes)
        .map_err(|e| Error::InvalidPublicKey(e.to_string()))?;

    let signature = Signature::from_bytes(signature_bytes);

    verifying_key
        .verify(message, &signature)
        .map_err(|_| Error::SignatureVerificationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Keypair;

    #[test]
    fn sign_and_verify() {
        let kp = Keypair::generate();
        let message = b"hello humanity";

        let sig = sign(kp.signing_key(), message);
        let result = verify(&kp.public_key_bytes(), message, &sig);
        assert!(result.is_ok());
    }

    #[test]
    fn wrong_message_fails() {
        let kp = Keypair::generate();
        let sig = sign(kp.signing_key(), b"correct message");
        let result = verify(&kp.public_key_bytes(), b"wrong message", &sig);
        assert!(result.is_err());
    }

    #[test]
    fn wrong_key_fails() {
        let kp1 = Keypair::generate();
        let kp2 = Keypair::generate();
        let message = b"test";

        let sig = sign(kp1.signing_key(), message);
        let result = verify(&kp2.public_key_bytes(), message, &sig);
        assert!(result.is_err());
    }
}
