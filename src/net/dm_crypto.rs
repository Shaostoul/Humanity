//! ECDH P-256 + AES-256-GCM DM encryption.
//!
//! Matches the web client's crypto.js implementation exactly so messages
//! can flow bidirectionally between web and native clients.
//!
//! Algorithm:
//! - Each identity has an ECDH P-256 keypair (separate from Ed25519).
//! - Public key is transmitted as 65-byte uncompressed SEC1 format (0x04 + X + Y),
//!   base64-encoded (standard base64 with padding).
//! - To encrypt/decrypt: derive shared secret via ECDH, use raw 32-byte X coordinate
//!   as AES-256-GCM key (no HKDF). This matches WebCrypto's behavior when
//!   deriveKey targets AES-GCM.
//! - IV is 12 random bytes, also base64-encoded for transmission.

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use p256::ecdh::diffie_hellman;
use p256::pkcs8::DecodePrivateKey;
use p256::{PublicKey, SecretKey};

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng as AesOsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};

/// Our ECDH keypair for encrypted DMs.
#[derive(Clone)]
pub struct DmKeypair {
    pub secret: SecretKey,
    pub public: PublicKey,
}

impl DmKeypair {
    /// Generate a fresh ECDH P-256 keypair.
    pub fn generate() -> Self {
        let secret = SecretKey::random(&mut rand_core_06::OsRng);
        let public = secret.public_key();
        Self { secret, public }
    }

    /// Load from raw 32-byte scalar (the secret bytes).
    pub fn from_secret_bytes(bytes: &[u8; 32]) -> Result<Self, String> {
        let secret = SecretKey::from_bytes(bytes.into())
            .map_err(|e| format!("Invalid ECDH secret: {e}"))?;
        let public = secret.public_key();
        Ok(Self { secret, public })
    }

    /// Import from the web client's PKCS8 DER format (base64-encoded).
    /// This is what `crypto.subtle.exportKey('pkcs8', privateKey)` produces in the browser.
    pub fn from_pkcs8_base64(pkcs8_b64: &str) -> Result<Self, String> {
        let der = B64
            .decode(pkcs8_b64.trim())
            .map_err(|e| format!("Invalid base64: {e}"))?;
        let secret = SecretKey::from_pkcs8_der(&der)
            .map_err(|e| format!("Invalid PKCS8 DER: {e}"))?;
        let public = secret.public_key();
        Ok(Self { secret, public })
    }

    /// Serialize secret as raw 32 bytes (for storage in the encrypted vault).
    pub fn secret_bytes(&self) -> [u8; 32] {
        let ga = self.secret.to_bytes();
        let mut out = [0u8; 32];
        out.copy_from_slice(&ga);
        out
    }

    /// Serialize public key as 65-byte uncompressed SEC1, base64-encoded.
    /// Matches the web client's `crypto.subtle.exportKey('raw', publicKey)` output.
    pub fn public_base64(&self) -> String {
        let encoded = self.public.to_sec1_bytes();
        B64.encode(&encoded)
    }
}

/// Parse a peer's base64-encoded ECDH public key.
pub fn parse_peer_public(base64_str: &str) -> Result<PublicKey, String> {
    let bytes = B64
        .decode(base64_str)
        .map_err(|e| format!("Invalid base64: {e}"))?;
    PublicKey::from_sec1_bytes(&bytes).map_err(|e| format!("Invalid ECDH public key: {e}"))
}

/// Derive the shared AES-256 key from our secret + peer's public key.
/// Returns the raw 32-byte shared secret (uses x-coordinate directly, matching WebCrypto).
fn derive_shared_key(my_secret: &SecretKey, peer_public: &PublicKey) -> [u8; 32] {
    let shared = diffie_hellman(my_secret.to_nonzero_scalar(), peer_public.as_affine());
    let raw = shared.raw_secret_bytes();
    let mut out = [0u8; 32];
    out.copy_from_slice(raw.as_slice());
    out
}

/// Encrypted DM output: ciphertext and IV, both base64-encoded.
pub struct EncryptedDm {
    pub content_b64: String,
    pub nonce_b64: String,
}

/// Encrypt plaintext for a peer. Returns base64-encoded ciphertext and nonce.
pub fn encrypt_dm(
    my_keypair: &DmKeypair,
    peer_public_b64: &str,
    plaintext: &str,
) -> Result<EncryptedDm, String> {
    let peer_public = parse_peer_public(peer_public_b64)?;
    let shared = derive_shared_key(&my_keypair.secret, &peer_public);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&shared));

    // Generate random 12-byte IV
    let nonce_bytes = Aes256Gcm::generate_nonce(&mut AesOsRng);
    let nonce = Nonce::from_slice(nonce_bytes.as_slice());

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("Encryption failed: {e}"))?;

    Ok(EncryptedDm {
        content_b64: B64.encode(&ciphertext),
        nonce_b64: B64.encode(nonce_bytes.as_slice()),
    })
}

/// Decrypt a DM ciphertext from a peer.
pub fn decrypt_dm(
    my_keypair: &DmKeypair,
    peer_public_b64: &str,
    ciphertext_b64: &str,
    nonce_b64: &str,
) -> Result<String, String> {
    let peer_public = parse_peer_public(peer_public_b64)?;
    let shared = derive_shared_key(&my_keypair.secret, &peer_public);
    let cipher = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&shared));

    let ciphertext = B64
        .decode(ciphertext_b64)
        .map_err(|e| format!("Invalid ciphertext base64: {e}"))?;
    let nonce_bytes = B64
        .decode(nonce_b64)
        .map_err(|e| format!("Invalid nonce base64: {e}"))?;
    if nonce_bytes.len() != 12 {
        return Err(format!("Nonce must be 12 bytes, got {}", nonce_bytes.len()));
    }
    let nonce = Nonce::from_slice(&nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|e| format!("Decryption failed: {e}"))?;
    String::from_utf8(plaintext).map_err(|e| format!("Invalid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let alice = DmKeypair::generate();
        let bob = DmKeypair::generate();

        let plaintext = "Hello from Alice!";
        let encrypted = encrypt_dm(&alice, &bob.public_base64(), plaintext).unwrap();
        let decrypted =
            decrypt_dm(&bob, &alice.public_base64(), &encrypted.content_b64, &encrypted.nonce_b64)
                .unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn secret_bytes_roundtrip() {
        let original = DmKeypair::generate();
        let bytes = original.secret_bytes();
        let restored = DmKeypair::from_secret_bytes(&bytes).unwrap();
        assert_eq!(original.public_base64(), restored.public_base64());
    }
}
