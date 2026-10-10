//! Handler submodules for the relay server.
//! Each submodule contains logically grouped functions extracted from relay.rs.

pub mod announce;
pub mod broadcast;
pub mod federation;
pub mod fleet_ledger;
pub mod friend_passes;
pub mod game_interest;
pub mod game_state;
pub mod home_plots;
pub mod live_conns;
pub mod move_check;
pub mod msg_handlers;
pub mod reach;
pub mod server_settings_update;
pub mod shared_build;
pub mod ship_stores;
pub mod ship_world;
pub mod sign_ups;
pub mod utils;

pub use broadcast::*;
pub use federation::*;
pub use home_plots::handle_game_admin;
pub use msg_handlers::*;
pub use utils::*;
