//! Multiplayer networking: WebSocket client and ECS state synchronization.
//!
//! Native-only (requires tungstenite). The server relay handles message
//! routing; this module is the client side.

#[cfg(feature = "native")]
pub mod protocol;
#[cfg(feature = "native")]
pub mod client;
#[cfg(feature = "native")]
pub mod sync;
#[cfg(feature = "native")]
pub mod ws_client;
#[cfg(feature = "native")]
pub mod identity;
#[cfg(feature = "native")]
pub mod bip39_wordlist;
/// Post-quantum DM envelope (pure ML-KEM-768 → BLAKE3-KDF →
/// AES-256-GCM). Full-PQ replacement for the deleted ECDH `dm_crypto`;
/// the recipient keypair is deterministic from the BIP39 seed so web
/// and native derive the same key (kills the cross-client DM bug).
#[cfg(feature = "native")]
pub mod dm_pq;

/// Native client → relay v2 signed-object submission + invite ticket helpers
/// (P2P groups). HTTP via the same blocking-ureq pattern as image upload.
#[cfg(feature = "native")]
pub mod api_v2;

/// P2P group end-to-end encrypted messaging (Phase 2). Epoch-key sealing via
/// the same ML-KEM-768 → BLAKE3-KDF → AES-256-GCM scheme as `dm_pq`, and
/// AES-256-GCM message ciphertext under that epoch key. Byte-compatible with
/// the web client's `pq-object.js` Phase-2 helpers.
#[cfg(feature = "native")]
pub mod group_e2ee;
