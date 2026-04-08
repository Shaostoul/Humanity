//! BLAKE3 hashing for object and block identifiers.
//!
//! - `object_id = BLAKE3(canonical_object_bytes)`
//! - `block_id  = BLAKE3(block_bytes)`

/// Length of a BLAKE3 hash output in bytes (256 bits).
pub const HASH_LEN: usize = 32;

/// A BLAKE3 hash (32 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Hash([u8; HASH_LEN]);

impl Hash {
    /// Compute BLAKE3 hash of arbitrary bytes.
    pub fn digest(data: &[u8]) -> Self {
        let h = blake3::hash(data);
        Self(*h.as_bytes())
    }

    /// Create a Hash from raw bytes.
    pub fn from_bytes(bytes: [u8; HASH_LEN]) -> Self {
        Self(bytes)
    }

    /// Get the raw bytes of this hash.
    pub fn as_bytes(&self) -> &[u8; HASH_LEN] {
        &self.0
    }

    /// Encode as lowercase hex string.
    pub fn to_hex(&self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

impl std::fmt::Debug for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Hash({})", self.to_hex())
    }
}

impl std::fmt::Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

/// Compute a block_id from raw block bytes.
pub fn block_id(block_bytes: &[u8]) -> Hash {
    Hash::digest(block_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_hash() {
        // BLAKE3 hash of empty input — deterministic and well-known
        let h = Hash::digest(b"");
        assert_eq!(h.as_bytes().len(), 32);
        // Verify it's the known BLAKE3 empty hash
        let expected = blake3::hash(b"");
        assert_eq!(h.as_bytes(), expected.as_bytes());
    }

    #[test]
    fn abc_hash() {
        let h = Hash::digest(b"abc");
        // Should be deterministic
        let h2 = Hash::digest(b"abc");
        assert_eq!(h, h2);
    }

    #[test]
    fn different_inputs_different_hashes() {
        let h1 = Hash::digest(b"hello");
        let h2 = Hash::digest(b"world");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hex_representation() {
        let h = Hash::digest(b"");
        let hex = h.to_hex();
        assert_eq!(hex.len(), 64); // 32 bytes = 64 hex chars
    }
}
