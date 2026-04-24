//! # humanity-core
//!
//! Core types for the Humanity Network:
//! - Canonical CBOR encoding/decoding
//! - BLAKE3 hashing (object_id, block_id)
//! - Post-quantum signing (ML-DSA-65 / Dilithium3) and KEM (ML-KEM-768 / Kyber768)
//! - Argon2id-based password KDF
//! - Legacy Ed25519 (kept only for the one-time migration bridge and Solana opt-in)
//! - The immutable signed Object model
//!
//! This crate has no network code and no storage code.
//! It is the foundation that all other Humanity crates build on.

pub mod encoding;
pub mod error;
pub mod hash;
pub mod identity;
pub mod kdf;
pub mod object;
pub mod pq_crypto;
pub mod signing;
