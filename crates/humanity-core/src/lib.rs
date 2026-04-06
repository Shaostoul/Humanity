//! # humanity-core
//!
//! Core types for the Humanity Network:
//! - Canonical CBOR encoding/decoding
//! - BLAKE3 hashing (object_id, block_id)
//! - Ed25519 identity keys, signing, and verification
//! - The immutable signed Object model
//!
//! This crate has no network code and no storage code.
//! It is the foundation that all other Humanity crates build on.

pub mod encoding;
pub mod error;
pub mod hash;
pub mod identity;
pub mod object;
pub mod signing;
