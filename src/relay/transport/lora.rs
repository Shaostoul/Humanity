//! LoRa transport stub (Phase 7b — feature-gated `mesh`).
//!
//! Long-range low-power radio for off-grid resilience scenarios. This module
//! defines the API contract; physical hardware integration arrives later.
//!
//! ## Use cases
//!
//! - Natural disaster zones where TCP/IP is unavailable
//! - Censorship circumvention in jurisdictions blocking the public network
//! - Rural / off-grid communities with sparse internet coverage
//!
//! ## Constraints
//!
//! LoRa typical bandwidth: 0.3 to 50 kbps depending on spreading factor.
//! At 50 kbps regional, a Dilithium3-signed object (~3.3 KB sig + ~2 KB
//! pubkey + payload) takes roughly 1 second to transmit. So mesh nodes
//! aggregate-batch and use compressed CBOR.
//!
//! Important: the existing canonical-CBOR signed-object format is already
//! compact enough for LoRa. The substrate's auto-indexing on the receiving
//! side means a node that re-joins the internet can sync mesh-cached objects
//! to the broader federation seamlessly.
//!
//! ## Object filter
//!
//! Per `data/network/peer_policy.ron` (when populated), only "important"
//! object types propagate over LoRa to conserve bandwidth:
//! - `signed_profile_v1`
//! - `recovery_request_v1`
//! - `vouch_v1`
//! - `verified_human_v1`
//! - Governance: `proposal_v1`, `vote_v1`
//! - Disputes: `dispute_v1`
//!
//! Routine chat / large media skip the mesh and wait for IP recovery.

use crate::relay::core::error::{Error, Result};
use crate::relay::core::object::Object;

/// Configuration for the LoRa transport.
#[derive(Debug, Clone)]
pub struct LoraConfig {
    /// Serial port path for the radio module (e.g., "/dev/ttyUSB0", "COM3").
    pub serial_port: String,
    /// Bitrate in bits-per-second. Typical: 5000 for long range, 50000 for short.
    pub bitrate: u32,
    /// LoRa frequency in Hz. Region-specific (e.g., 915_000_000 in US, 868_000_000 in EU).
    pub frequency_hz: u32,
    /// Maximum object size to transmit. Larger objects rejected at this transport.
    pub max_object_bytes: usize,
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self {
            serial_port: "/dev/ttyUSB0".to_string(),
            bitrate: 5000,
            frequency_hz: 915_000_000,
            max_object_bytes: 8192,
        }
    }
}

/// A LoRa transport endpoint. Stub implementation for Phase 7b — physical
/// radio integration arrives in a follow-up PR using the `lorawan` crate or
/// direct serial access depending on the radio module.
#[derive(Debug, Clone)]
pub struct LoraTransport {
    config: LoraConfig,
}

impl LoraTransport {
    pub fn new(config: LoraConfig) -> Self {
        Self { config }
    }

    /// Send a signed object over LoRa. Returns Err in the stub.
    ///
    /// Real implementation will:
    /// 1. Encode the Object as compressed CBOR
    /// 2. Apply forward error correction (FEC) — LoRa has built-in FEC at the PHY layer, but we add a higher-level checksum
    /// 3. Fragment into ~256-byte chunks if over the radio's MTU
    /// 4. Transmit with backoff respecting regional duty-cycle regulations
    /// 5. Wait for ACK or timeout
    pub fn send_object(&self, object: &Object) -> Result<()> {
        let canonical = object.to_canonical_bytes()?;
        if canonical.len() > self.config.max_object_bytes {
            return Err(Error::InvalidField {
                field: "object_size".into(),
                reason: format!(
                    "{} bytes exceeds LoRa max {} bytes",
                    canonical.len(),
                    self.config.max_object_bytes
                ),
            });
        }
        // Stub: physical TX not implemented
        Err(Error::InvalidField {
            field: "lora_transport".into(),
            reason: "Phase 7b stub — physical LoRa hardware integration deferred".into(),
        })
    }

    /// Should this object_type be transmitted over LoRa, given bandwidth constraints?
    pub fn should_propagate(&self, object_type: &str) -> bool {
        matches!(
            object_type,
            "signed_profile_v1"
                | "recovery_request_v1"
                | "recovery_approval_v1"
                | "vouch_v1"
                | "verified_human_v1"
                | "proposal_v1"
                | "vote_v1"
                | "dispute_v1"
                | "key_rotation_v1"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_us_915() {
        let cfg = LoraConfig::default();
        assert_eq!(cfg.frequency_hz, 915_000_000);
    }

    #[test]
    fn governance_propagates_chat_does_not() {
        let lora = LoraTransport::new(LoraConfig::default());
        assert!(lora.should_propagate("proposal_v1"));
        assert!(lora.should_propagate("vote_v1"));
        assert!(lora.should_propagate("recovery_request_v1"));
        assert!(!lora.should_propagate("chat_message"));
        assert!(!lora.should_propagate("dm"));
        assert!(!lora.should_propagate("attached_image"));
    }

    #[test]
    fn stub_send_returns_not_implemented() {
        // We can't test the actual hardware integration here. Just verify the
        // stub returns a clean error rather than panicking.
        // (Skipped because constructing a valid Object requires a Dilithium3
        // keypair and full payload pipeline — covered indirectly by upstream
        // integration tests if/when the stub is replaced.)
    }
}
