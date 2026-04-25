//! LoRa transport (Phase 7b — feature-gated `mesh`).
//!
//! Long-range low-power radio for off-grid resilience scenarios. Implements
//! send/receive over a UART-attached LoRa module (RFM95W, SX1262, etc.).
//! The hardware-specific AT-command dialect is module-agnostic in this driver:
//! we ship raw framed bytes and let the caller's hardware do the modulation.
//!
//! ## Frame format on the wire
//!
//! Each transmitted frame:
//!
//!   [ 4 bytes BE length ] [ payload bytes ] [ 4 bytes BE CRC32 of payload ]
//!
//! The receiver reads exactly `length` bytes after the header, verifies CRC,
//! and emits the payload. This is intentionally simple — LoRa physical layer
//! handles its own forward error correction, so we just need framing.
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
//! pubkey + payload) takes roughly 1 second to transmit. Throughput-sensitive
//! object types (chat messages, large media) do NOT use LoRa — see
//! `should_propagate` filter below.

use std::io::{Read, Write};
use std::time::Duration;

use crate::relay::core::error::{Error, Result};
use crate::relay::core::object::Object;

/// Configuration for the LoRa transport.
#[derive(Debug, Clone)]
pub struct LoraConfig {
    /// Serial port path for the radio module (e.g., "/dev/ttyUSB0", "COM3").
    pub serial_port: String,
    /// UART baud rate to the radio module. Common values: 9600, 57600, 115200.
    /// (Note: this is the UART rate, NOT the over-the-air LoRa data rate —
    /// that's set on the module itself via AT commands.)
    pub baud: u32,
    /// LoRa frequency in Hz. Region-specific (US: 915MHz, EU: 868MHz).
    pub frequency_hz: u32,
    /// Maximum object size to transmit. Larger objects rejected at this transport.
    pub max_object_bytes: usize,
    /// Read timeout for receive operations.
    pub read_timeout: Duration,
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self {
            serial_port: "/dev/ttyUSB0".to_string(),
            baud: 115_200,
            frequency_hz: 915_000_000,
            max_object_bytes: 8192,
            read_timeout: Duration::from_secs(10),
        }
    }
}

/// A LoRa transport endpoint.
pub struct LoraTransport {
    config: LoraConfig,
    port: Option<Box<dyn serialport::SerialPort>>,
}

impl std::fmt::Debug for LoraTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoraTransport")
            .field("config", &self.config)
            .field("port_open", &self.port.is_some())
            .finish()
    }
}

impl LoraTransport {
    pub fn new(config: LoraConfig) -> Self {
        Self { config, port: None }
    }

    /// Open the serial port. Idempotent: already-open is a no-op.
    pub fn open(&mut self) -> Result<()> {
        if self.port.is_some() {
            return Ok(());
        }
        let port = serialport::new(&self.config.serial_port, self.config.baud)
            .timeout(self.config.read_timeout)
            .open()
            .map_err(|e| Error::InvalidField {
                field: "lora_serial".into(),
                reason: format!("failed to open {}: {e}", self.config.serial_port),
            })?;
        self.port = Some(port);
        Ok(())
    }

    /// Close the serial port.
    pub fn close(&mut self) {
        self.port = None;
    }

    /// Send a signed object as a framed packet.
    pub fn send_object(&mut self, object: &Object) -> Result<()> {
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
        if !self.should_propagate(&object.object_type) {
            return Err(Error::InvalidField {
                field: "object_type".into(),
                reason: format!(
                    "object_type '{}' is not LoRa-propagated (saving bandwidth)",
                    object.object_type
                ),
            });
        }

        self.open()?;
        let port = self.port.as_mut().expect("opened above");

        let len = u32::try_from(canonical.len()).map_err(|_| Error::InvalidField {
            field: "object_size".into(),
            reason: "exceeds u32".into(),
        })?;
        let crc = crc32(&canonical);

        let mut frame = Vec::with_capacity(8 + canonical.len());
        frame.extend_from_slice(&len.to_be_bytes());
        frame.extend_from_slice(&canonical);
        frame.extend_from_slice(&crc.to_be_bytes());

        port.write_all(&frame).map_err(|e| Error::InvalidField {
            field: "lora_write".into(),
            reason: e.to_string(),
        })?;
        port.flush().map_err(|e| Error::InvalidField {
            field: "lora_flush".into(),
            reason: e.to_string(),
        })?;
        Ok(())
    }

    /// Read one framed packet, returning the canonical bytes (caller must
    /// reconstruct + verify the Object via the substrate).
    pub fn recv_packet(&mut self) -> Result<Vec<u8>> {
        self.open()?;
        let port = self.port.as_mut().expect("opened above");

        let mut len_buf = [0u8; 4];
        port.read_exact(&mut len_buf).map_err(|e| Error::InvalidField {
            field: "lora_read_len".into(),
            reason: e.to_string(),
        })?;
        let len = u32::from_be_bytes(len_buf) as usize;
        if len > self.config.max_object_bytes {
            return Err(Error::InvalidField {
                field: "lora_frame_len".into(),
                reason: format!("declared len {len} exceeds max"),
            });
        }

        let mut payload = vec![0u8; len];
        port.read_exact(&mut payload).map_err(|e| Error::InvalidField {
            field: "lora_read_payload".into(),
            reason: e.to_string(),
        })?;

        let mut crc_buf = [0u8; 4];
        port.read_exact(&mut crc_buf).map_err(|e| Error::InvalidField {
            field: "lora_read_crc".into(),
            reason: e.to_string(),
        })?;
        let claimed_crc = u32::from_be_bytes(crc_buf);
        let actual_crc = crc32(&payload);
        if claimed_crc != actual_crc {
            return Err(Error::InvalidField {
                field: "lora_frame_crc".into(),
                reason: format!("CRC mismatch: claimed {claimed_crc:08x}, actual {actual_crc:08x}"),
            });
        }

        Ok(payload)
    }

    /// Should this object_type be transmitted over LoRa? Bandwidth-conserving
    /// allowlist of "important" types; chat skips LoRa entirely.
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
                | "subject_class_v1"
                | "controlled_by_v1"
        )
    }

    pub fn config(&self) -> &LoraConfig {
        &self.config
    }
}

/// CRC32 (IEEE 802.3 polynomial) over arbitrary bytes. Small inline impl
/// avoids pulling in `crc32fast` crate.
fn crc32(bytes: &[u8]) -> u32 {
    const POLY: u32 = 0xEDB88320;
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ POLY
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_is_us_915() {
        let cfg = LoraConfig::default();
        assert_eq!(cfg.frequency_hz, 915_000_000);
        assert_eq!(cfg.baud, 115_200);
    }

    #[test]
    fn governance_propagates_chat_does_not() {
        let lora = LoraTransport::new(LoraConfig::default());
        assert!(lora.should_propagate("proposal_v1"));
        assert!(lora.should_propagate("vote_v1"));
        assert!(lora.should_propagate("recovery_request_v1"));
        assert!(lora.should_propagate("dispute_v1"));
        assert!(!lora.should_propagate("chat_message"));
        assert!(!lora.should_propagate("dm"));
        assert!(!lora.should_propagate("attached_image"));
    }

    #[test]
    fn crc32_known_vector() {
        // Standard CRC-32 of "123456789" = 0xCBF43926
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_empty_is_zero() {
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn crc32_detects_single_bit_flip() {
        let a = crc32(b"hello world");
        let b = crc32(b"hellp world"); // flip 'o' → 'p'
        assert_ne!(a, b);
    }
}
