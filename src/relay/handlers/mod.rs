//! Handler submodules for the relay server.
//! Each submodule contains logically grouped functions extracted from relay.rs.

pub mod announce;
pub mod broadcast;
pub mod federation;
pub mod game_state;
pub mod home_plots;
pub mod live_conns;
pub mod msg_handlers;
pub mod ship_stores;
pub mod ship_world;
pub mod sign_ups;
pub mod utils;

pub use broadcast::*;
pub use federation::*;
pub use home_plots::handle_game_admin;
pub use msg_handlers::*;
pub use utils::*;
