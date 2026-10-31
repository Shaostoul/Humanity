//! # The protocol core (`relay/core`; the crate split's planned humanity-protocol, docs/crates/humanity-protocol.md)
//!
//! Core types for the Humanity Network:
//! - Canonical CBOR encoding/decoding
//! - BLAKE3 hashing (object_id, block_id)
//! - Post-quantum signing (ML-DSA-65 / Dilithium3) and KEM (ML-KEM-768 / Kyber768)
//! - Argon2id-based password KDF
//! - Legacy Ed25519 (kept only for the one-time migration bridge and Solana opt-in)
//! - The immutable signed Object model
//!
//! This module has no storage code. It may use only the core (scripts/crate-layers.json):
//! the server (relay), the app's network side (net) and the GUI build on it.

/// What a picture's or video's hidden information says, in plain words, for the attach step
/// (2026-10-11): a port of the web's `HosStrip.describe`, held to it by shared fixtures.
pub mod describe_metadata;
pub mod did;
pub mod encoding;
pub mod error;
pub mod hash;
pub mod identity;
pub mod kdf;
/// Key cards (2026-10-22): a person's Kyber768 key signed by their own Dilithium3 identity, so
/// any app can check the key it seals to really is theirs whatever server delivered it. Shared by
/// the relay and the desktop app; web/shared/key-card.js is its twin.
pub mod key_card;
/// Market signed-object payload validators (provider_v1, offering_v1).
pub mod market_payloads;
/// Signed moderation objects (mod_action_v1, space_policy_v1). Rung 1 of
/// docs/design/signed_moderation_logs.md: the encoding half, byte-locked to
/// the web twin by .
pub mod moderation;
pub mod merkle_disclosure;
pub mod object;
pub mod pq_crypto;
/// The copy of a person's profile that other servers keep (BUG-191, 2026-10-12): the public copy
/// rule, the words its owner's app signs, the limits, when to ask, and the time on it. Shared by
/// the relay and the desktop app; web/shared/profile-copy.js is its twin.
pub mod profile_copy;
/// Recovery through trusted people (2026-10-21): Shamir pieces of the recovery phrase sealed to
/// trusted people, the one-time key a new device asks with, and the code that names an ask.
/// Shared by the relay and the desktop app; web/shared/recovery.js is its twin.
pub mod recovery;
pub mod signing;
pub mod solana_rpc;
/// HEIF still picture (HEIC, AVIF) metadata removal in place: the Exif and
/// XMP items are overwritten with zeros where they stand, so the file keeps
/// its exact size and the picture's own items never move.
pub mod strip_heif_metadata;
pub mod strip_metadata;
/// Video (MP4, MOV, 3GP) metadata removal in place: renames the boxes that
/// carry where a video was filmed to `free` and zeroes them, so the file
/// keeps its exact size and nothing it plays from moves.
pub mod strip_video_metadata;
