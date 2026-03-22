//! The Humanity Network object model.
//!
//! All shareable content is represented as immutable signed objects.
//! See `design/network/object_format.md` for the full specification.
//!
//! An object is a CBOR map with these fields:
//! - protocol_version: integer
//! - object_type: text
//! - space_id: optional text
//! - channel_id: optional text
//! - author_public_key: bytes (32 bytes, Ed25519)
//! - created_at: optional integer (informational only)
//! - references: array of text (object_id hex strings)
//! - payload_schema_version: integer
//! - payload_encoding: text
//! - payload: bytes
//! - signature: bytes (64 bytes, Ed25519)

use ciborium::Value;

use crate::encoding::{cbor_bytes, cbor_int, cbor_text, to_canonical_bytes};
use crate::error::Result;
use crate::hash::Hash;
use crate::signing;

/// Current protocol version.
pub const PROTOCOL_VERSION: u64 = 1;

/// Payload encoding: plaintext canonical CBOR.
pub const PAYLOAD_ENCODING_PLAINTEXT: &str = "cbor_canonical_v1";

/// Payload encoding: encrypted (XChaCha20-Poly1305).
pub const PAYLOAD_ENCODING_ENCRYPTED: &str = "xchacha20poly1305_v1";

/// A signed, immutable Humanity Network object.
#[derive(Debug, Clone)]
pub struct Object {
    /// Protocol version (currently 1).
    pub protocol_version: u64,
    /// The type of this object (e.g., "thread_create", "post", "moderation_action").
    pub object_type: String,
    /// Space this object belongs to (optional).
    pub space_id: Option<String>,
    /// Channel within the space (optional).
    pub channel_id: Option<String>,
    /// Author's Ed25519 public key (32 bytes).
    pub author_public_key: [u8; 32],
    /// Informational timestamp (not trusted for ordering).
    pub created_at: Option<u64>,
    /// References to other objects by object_id hex.
    pub references: Vec<String>,
    /// Schema version for the payload.
    pub payload_schema_version: u64,
    /// Payload encoding type.
    pub payload_encoding: String,
    /// Payload bytes (plaintext CBOR or ciphertext).
    pub payload: Vec<u8>,
    /// Ed25519 signature (64 bytes).
    pub signature: [u8; 64],
}

impl Object {
    /// Convert this object to a canonical CBOR Value (map).
    fn to_cbor_value(&self) -> Value {
        let mut entries: Vec<(Value, Value)> = Vec::new();

        entries.push((cbor_text("author_public_key"), cbor_bytes(&self.author_public_key)));

        if let Some(ref ch) = self.channel_id {
            entries.push((cbor_text("channel_id"), cbor_text(ch)));
        }

        if let Some(ts) = self.created_at {
            entries.push((cbor_text("created_at"), cbor_int(ts)));
        }

        entries.push((cbor_text("object_type"), cbor_text(&self.object_type)));
        entries.push((cbor_text("payload"), cbor_bytes(&self.payload)));
        entries.push((cbor_text("payload_encoding"), cbor_text(&self.payload_encoding)));
        entries.push((cbor_text("payload_schema_version"), cbor_int(self.payload_schema_version)));
        entries.push((cbor_text("protocol_version"), cbor_int(self.protocol_version)));

        let refs = Value::Array(
            self.references.iter().map(|r| cbor_text(r)).collect(),
        );
        entries.push((cbor_text("references"), refs));

        entries.push((cbor_text("signature"), cbor_bytes(&self.signature)));

        if let Some(ref sid) = self.space_id {
            entries.push((cbor_text("space_id"), cbor_text(sid)));
        }

        Value::Map(entries)
    }

    /// Encode this object to canonical CBOR bytes.
    /// These bytes define the object_id and are what gets signed.
    pub fn to_canonical_bytes(&self) -> Result<Vec<u8>> {
        let value = self.to_cbor_value();
        to_canonical_bytes(&value)
    }

    /// Compute this object's identifier: BLAKE3(canonical_bytes).
    pub fn object_id(&self) -> Result<Hash> {
        let bytes = self.to_canonical_bytes()?;
        Ok(Hash::digest(&bytes))
    }

    /// Get the bytes that need to be signed.
    /// This is the canonical encoding with an empty signature field.
    fn signable_bytes(&self) -> Result<Vec<u8>> {
        let mut unsigned = self.clone();
        unsigned.signature = [0u8; 64];
        unsigned.to_canonical_bytes()
    }

    /// Verify this object's signature against its author_public_key.
    pub fn verify_signature(&self) -> Result<()> {
        let message = self.signable_bytes()?;
        signing::verify(&self.author_public_key, &message, &self.signature)
    }
}

/// Builder for creating and signing new objects.
pub struct ObjectBuilder {
    object_type: String,
    space_id: Option<String>,
    channel_id: Option<String>,
    created_at: Option<u64>,
    references: Vec<String>,
    payload_schema_version: u64,
    payload_encoding: String,
    payload: Vec<u8>,
}

impl ObjectBuilder {
    /// Create a new builder for the given object type.
    pub fn new(object_type: &str) -> Self {
        Self {
            object_type: object_type.to_string(),
            space_id: None,
            channel_id: None,
            created_at: None,
            references: Vec::new(),
            payload_schema_version: 1,
            payload_encoding: PAYLOAD_ENCODING_PLAINTEXT.to_string(),
            payload: Vec::new(),
        }
    }

    /// Set the space ID.
    pub fn space_id(mut self, id: &str) -> Self {
        self.space_id = Some(id.to_string());
        self
    }

    /// Set the channel ID.
    pub fn channel_id(mut self, id: &str) -> Self {
        self.channel_id = Some(id.to_string());
        self
    }

    /// Set the informational timestamp.
    pub fn created_at(mut self, ts: u64) -> Self {
        self.created_at = Some(ts);
        self
    }

    /// Add a reference to another object.
    pub fn reference(mut self, object_id: &str) -> Self {
        self.references.push(object_id.to_string());
        self
    }

    /// Set the payload schema version.
    pub fn payload_schema_version(mut self, v: u64) -> Self {
        self.payload_schema_version = v;
        self
    }

    /// Set the payload encoding.
    pub fn payload_encoding(mut self, encoding: &str) -> Self {
        self.payload_encoding = encoding.to_string();
        self
    }

    /// Set the raw payload bytes.
    pub fn payload_raw(mut self, bytes: Vec<u8>) -> Self {
        self.payload = bytes;
        self
    }

    /// Set the payload from a CBOR Value (encodes to canonical bytes).
    pub fn payload_cbor(mut self, value: &Value) -> Result<Self> {
        self.payload = to_canonical_bytes(value)?;
        Ok(self)
    }

    /// Sign and build the final Object.
    pub fn sign(self, signing_key: &ed25519_dalek::SigningKey) -> Result<Object> {
        let author_public_key = signing_key.verifying_key().to_bytes();

        // Build object with empty signature first
        let mut obj = Object {
            protocol_version: PROTOCOL_VERSION,
            object_type: self.object_type,
            space_id: self.space_id,
            channel_id: self.channel_id,
            author_public_key,
            created_at: self.created_at,
            references: self.references,
            payload_schema_version: self.payload_schema_version,
            payload_encoding: self.payload_encoding,
            payload: self.payload,
            signature: [0u8; 64],
        };

        // Sign the canonical bytes (with empty signature)
        let signable = obj.signable_bytes()?;
        obj.signature = signing::sign(signing_key, &signable);

        Ok(obj)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::encoding::{cbor_map, cbor_text};
    use crate::identity::Keypair;

    #[test]
    fn create_sign_verify() {
        let kp = Keypair::generate();

        let payload = cbor_map(vec![
            ("title", cbor_text("Hello")),
            ("body", cbor_text("First post")),
        ]);

        let obj = ObjectBuilder::new("thread_create")
            .space_id("space_example")
            .created_at(0)
            .payload_cbor(&payload)
            .unwrap()
            .sign(kp.signing_key())
            .unwrap();

        assert_eq!(obj.protocol_version, PROTOCOL_VERSION);
        assert_eq!(obj.object_type, "thread_create");
        assert_eq!(obj.author_public_key, kp.public_key_bytes());

        // Signature should verify
        assert!(obj.verify_signature().is_ok());
    }

    #[test]
    fn tampered_object_fails_verification() {
        let kp = Keypair::generate();

        let payload = cbor_map(vec![("title", cbor_text("Hello"))]);

        let mut obj = ObjectBuilder::new("post")
            .space_id("space_example")
            .payload_cbor(&payload)
            .unwrap()
            .sign(kp.signing_key())
            .unwrap();

        // Tamper with the object
        obj.object_type = "thread_create".to_string();

        // Signature should no longer verify
        assert!(obj.verify_signature().is_err());
    }

    #[test]
    fn object_id_is_deterministic() {
        let kp = Keypair::generate();

        let payload = cbor_map(vec![("title", cbor_text("Test"))]);

        let obj = ObjectBuilder::new("post")
            .space_id("test_space")
            .created_at(1000)
            .payload_cbor(&payload)
            .unwrap()
            .sign(kp.signing_key())
            .unwrap();

        let id1 = obj.object_id().unwrap();
        let id2 = obj.object_id().unwrap();
        assert_eq!(id1, id2, "object_id must be deterministic");
    }

    #[test]
    fn canonical_bytes_are_stable() {
        let kp = Keypair::generate();

        let payload = cbor_map(vec![("msg", cbor_text("hi"))]);

        let obj = ObjectBuilder::new("message")
            .payload_cbor(&payload)
            .unwrap()
            .sign(kp.signing_key())
            .unwrap();

        let bytes1 = obj.to_canonical_bytes().unwrap();
        let bytes2 = obj.to_canonical_bytes().unwrap();
        assert_eq!(bytes1, bytes2, "canonical bytes must be identical across calls");
    }
}
