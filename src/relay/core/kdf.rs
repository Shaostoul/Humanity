//! Argon2id-based key derivation function.
//!
//! Replaces PBKDF2-SHA256 for password/passphrase-based key derivation in vault sync,
//! seed-decryption, and any other place a user-provided secret needs to be stretched
//! into a cryptographic key.
//!
//! Argon2id is memory-hard, mitigating GPU/ASIC attacks that PBKDF2 cannot resist.
//! Quantum-resistant (Grover's algorithm halves effective bits, but 256-bit output
//! still gives 128 bits of post-quantum security).
//!
//! Default parameters target ~250ms on a modern desktop CPU, ~1s on mobile:
//! - memory: 64 MiB
//! - iterations (time cost): 3
//! - parallelism: 4 lanes
//!
//! These are tunable via [`KdfParams`] for callers that need different latency/cost
//! tradeoffs (e.g., headless server bulk operations vs. interactive logins).

use argon2::{Algorithm, Argon2, Params, Version};

use crate::relay::core::error::{Error, Result};

/// Recommended salt length for Argon2id KDF: 16 bytes.
pub const SALT_LEN: usize = 16;

/// Standard derived key length (256 bits) — enough for AES-256-GCM, ChaCha20, BLAKE3 keying.
pub const DERIVED_KEY_LEN: usize = 32;

/// Argon2id parameter bundle. Tunable when the default profile is wrong for a use case.
#[derive(Clone, Copy, Debug)]
pub struct KdfParams {
    /// Memory cost in KiB. Default 65536 (64 MiB).
    pub memory_kib: u32,
    /// Time cost (iterations). Default 3.
    pub iterations: u32,
    /// Parallelism (lanes). Default 4.
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 65536,
            iterations: 3,
            parallelism: 4,
        }
    }
}

impl KdfParams {
    /// Lighter parameters for resource-constrained contexts (mobile, embedded).
    pub fn light() -> Self {
        Self {
            memory_kib: 16384,
            iterations: 2,
            parallelism: 2,
        }
    }

    /// Heavier parameters for high-value, infrequent operations (master seed unlock).
    pub fn strong() -> Self {
        Self {
            memory_kib: 131072,
            iterations: 4,
            parallelism: 4,
        }
    }
}

/// Derive a 32-byte key from a password and salt using Argon2id with default parameters.
pub fn derive_key(password: &[u8], salt: &[u8]) -> Result<[u8; DERIVED_KEY_LEN]> {
    derive_key_with_params(password, salt, KdfParams::default())
}

/// Derive a 32-byte key with explicit Argon2id parameters.
pub fn derive_key_with_params(
    password: &[u8],
    salt: &[u8],
    params: KdfParams,
) -> Result<[u8; DERIVED_KEY_LEN]> {
    if salt.len() < 8 {
        return Err(Error::InvalidField {
            field: "salt".into(),
            reason: format!("must be at least 8 bytes (got {})", salt.len()),
        });
    }

    let argon_params = Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(DERIVED_KEY_LEN),
    )
    .map_err(|e| Error::InvalidField {
        field: "argon2_params".into(),
        reason: e.to_string(),
    })?;

    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut output = [0u8; DERIVED_KEY_LEN];
    argon
        .hash_password_into(password, salt, &mut output)
        .map_err(|e| Error::InvalidField {
            field: "argon2_hash".into(),
            reason: e.to_string(),
        })?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 9106 section 5.3, the Argon2id test vector (password 32 x 0x01, salt 16 x 0x02, secret
    /// 8 x 0x03, associated data 12 x 0x04, t = 3, m = 32 KiB, p = 4, a 32-byte tag): the
    /// algorithm itself, so moving the argon2 crate (0.6.0-rc.8 to 0.6.0 on 2026-10-24, and any
    /// later move) cannot change it unseen. Passed on rc.8 before that move, then on 0.6.0.
    #[test]
    fn argon2id_matches_the_rfc_9106_vector() {
        use argon2::{AssociatedData, ParamsBuilder};
        let ad = AssociatedData::try_from(&[4u8; 12][..]).expect("12 bytes of associated data");
        let params = ParamsBuilder::new().m_cost(32).t_cost(3).p_cost(4).data(ad).output_len(32).build().expect("the vector's parameters");
        let argon = Argon2::new_with_secret(&[3u8; 8], Algorithm::Argon2id, Version::V0x13, params).expect("a secret");
        let mut tag = [0u8; 32];
        argon.hash_password_into(&[1u8; 32], &[2u8; 16], &mut tag).expect("hash");
        assert_eq!(hex::encode(tag), "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659");
    }

    #[test]
    fn deterministic_with_same_inputs() {
        let pw = b"correct horse battery staple";
        let salt = b"a-stable-salt-16";
        let k1 = derive_key(pw, salt).unwrap();
        let k2 = derive_key(pw, salt).unwrap();
        assert_eq!(k1, k2);
    }

    #[test]
    fn different_passwords_yield_different_keys() {
        let salt = b"a-stable-salt-16";
        let k1 = derive_key(b"password1", salt).unwrap();
        let k2 = derive_key(b"password2", salt).unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn different_salts_yield_different_keys() {
        let pw = b"same password";
        let k1 = derive_key(pw, b"salt-one--16byte").unwrap();
        let k2 = derive_key(pw, b"salt-two--16byte").unwrap();
        assert_ne!(k1, k2);
    }

    #[test]
    fn rejects_short_salt() {
        let result = derive_key(b"pw", b"short");
        assert!(result.is_err());
    }

    #[test]
    fn light_params_succeed() {
        let k = derive_key_with_params(b"pw", b"salt----16bytes!", KdfParams::light()).unwrap();
        assert_eq!(k.len(), DERIVED_KEY_LEN);
    }

    #[test]
    fn strong_params_succeed() {
        let k = derive_key_with_params(b"pw", b"salt----16bytes!", KdfParams::strong()).unwrap();
        assert_eq!(k.len(), DERIVED_KEY_LEN);
    }

    #[test]
    fn output_is_full_32_bytes_high_entropy() {
        let k = derive_key(b"pw", b"salt----16bytes!").unwrap();
        // Sanity check: not all zeros, not all the same byte.
        assert!(k.iter().any(|&b| b != 0));
        assert!(k.iter().any(|&b| b != k[0]));
    }
}
