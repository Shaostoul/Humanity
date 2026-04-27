//! Handler submodules for the relay server.
//! Each submodule contains logically grouped functions extracted from relay.rs.

pub mod announce;
pub mod broadcast;
pub mod federation;
pub mod game_state;
pub mod msg_handlers;
pub mod utils;

pub use broadcast::*;
pub use federation::*;
pub use msg_handlers::*;
pub use utils::*;
