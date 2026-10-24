//! Handler submodules for the relay server.
//! Each submodule contains logically grouped functions extracted from relay.rs.

pub mod account_erase;
pub mod announce;
pub mod broadcast;
pub mod chat_gate;
pub mod chat_rate;
pub mod dm_answer;
pub mod dm_rate;
pub mod federation;
/// How fast each linked server may send; slowed, then paused, never dropped (2026-10-22).
pub mod federation_flow;
/// The numbered log a server sends its links, the catch-up, and what edits, deletes and bans
/// from a linked server do here (2026-10-22).
pub mod federation_log;
/// Phase 3's load-bearing test: two relays on loopback, end to end (2026-10-22).
#[cfg(test)]
mod federation_phase3_tests;
pub mod fleet_ledger;
pub mod friend_passes;
pub mod game_interest;
pub mod game_state;
pub mod home_plots;
/// Key cards (2026-10-22): kept at sign-in, served on request, passed on to linked servers.
pub mod key_cards;
pub mod live_conns;
pub mod move_check;
pub mod msg_handlers;
/// The profile save and request, and the copy for other servers each person's app signs (BUG-191).
pub mod profile;
pub mod reach;
pub mod reports;
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
pub use profile::{handle_profile_request, handle_profile_sign, handle_profile_update};
pub use utils::*;
