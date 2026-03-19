//! Error types for humanity-core.

use thiserror::Error;

/// Errors that can occur during object operations.
#[derive(Debug, Error)]
pub enum Error {
    #[error("CBOR encoding error: {0}")]
    CborEncode(String),

    #[error("CBOR decoding error: {0}")]
    CborDecode(String),

    #[error("canonical CBOR violation: {0}")]
    CanonicalViolation(String),

    #[error("invalid signature")]
    InvalidSignature,

    #[error("signature verification failed")]
    SignatureVerificationFailed,

    #[error("invalid public key: {0}")]
    InvalidPublicKey(String),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("invalid field value: {field}: {reason}")]
    InvalidField { field: String, reason: String },

    #[error("unsupported protocol version: {0}")]
    UnsupportedVersion(u64),

    #[error("unsupported payload encoding: {0}")]
    UnsupportedPayloadEncoding(String),
}

/// Result type alias for humanity-core operations.
pub type Result<T> = std::result::Result<T, Error>;
