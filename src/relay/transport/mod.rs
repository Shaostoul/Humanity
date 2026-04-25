//! Transport layer (Phase 7b — LoRa mesh stub).
//!
//! The relay's primary transport is HTTP/WebSocket over TCP. This module
//! defines additional transports for resilience scenarios:
//!
//! - **LoRa**: long-range low-power radio for off-grid / disaster zones,
//!   feature-gated behind `mesh`. Stub now; physical hardware integration
//!   later when the use case actually arrives.
//!
//! Per the plan (decision 3): LoRa transport is documented + scaffolded but
//! not scheduled for production until after every other phase is stable.

#[cfg(feature = "mesh")]
pub mod lora;
