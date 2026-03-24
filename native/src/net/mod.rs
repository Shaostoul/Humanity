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
