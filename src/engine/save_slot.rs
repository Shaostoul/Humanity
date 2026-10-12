//! Which save is live, and the home it builds (ship homes increment 6,
//! docs/design/ship-homes-increment-6-plan.md sections 3.6 and 3.7).
//!
//! Your own home is saves/offline_home.json; each server you play on keeps its own save for you,
//! made FRESH from the start you choose there (the operator's rule of 2026-10-04: server worlds
//! start every player fresh; only your name, look and outfit come along). This module decides
//! which save is live (`wanted`, from the pairing the picker made, never a moment's step out of
//! the shared world), makes a server's fresh save (`fresh_save`), swaps the live save when the
//! pairing changes (`follow`), builds the home a save's start puts on its plot (`home_for`), and
//! takes the server's `game_home` answers (`on_game_message`, `choose_on_server`).
//!
//! WAVE 0 STUBS. These are the signatures the later lanes fill: `wanted` and `fresh_save` by lane
//! 1C, `follow`, `home_for`, `on_game_message`, `enter_pairing` and `choose_on_server` by lane 2A.
//! Until then each does what the game did before increment 6 (one save, your own home, for every
//! world), and nothing calls them. The types they share are persistence's `SaveSlot`,
//! `ShipPlace` and `ShipMeals`, save_load's `ActiveHome`, and ship::starts.

use crate::engine::state::EngineState;
use crate::persistence::{SaveSlot, SavedHome, ShipPlace, WorldSave};
use crate::ship::ship_structure::ShipStructure;
use crate::ship::starts::Starts;
use std::path::Path;

/// What crosses into every world you enter: your character's name, look and outfit, nothing else.
#[derive(Debug, Clone, Default)]
pub struct Look {
    pub name: String,
    pub appearance: crate::ecs::components::Appearance,
    pub outfit: crate::ecs::components::Outfit,
}

/// The save the pairing asks for (3.6, pure): your own home when the picker's pairing is a
/// home (`launcher_last_world` starting "home:") or there is no identity, else this server's
/// save for this identity. Dev travel, fly mode and the `solo` verb never change it.
///
/// STUB (lane 1C fills it): always your own home, as before increment 6.
pub fn wanted(_gui: &crate::gui::GuiState) -> SaveSlot {
    SaveSlot::Offline
}

/// A server's fresh save of `start` for `household` (3.6, pure): the start's kit, opening,
/// storage and a fresh body, the default credits, the fresh asteroids, nothing of your own home,
/// and `look`'s name, look and outfit.
///
/// STUB (lane 1C fills it): an empty save of that start and household with the look, and none
/// of the rest yet.
pub fn fresh_save(_data_dir: &Path, starts: &Starts, start: &str, household: &str, look: &Look) -> WorldSave {
    let start = starts.resolve(Some(start));
    let mut save = WorldSave::new_offline("My Homestead", start.id.clone());
    save.household = starts.household(&start.id, household).id.clone();
    save.character_name = look.name.clone();
    save.appearance = look.appearance.clone();
    save.outfit = look.outfit.clone();
    save
}

/// Swap the live save when the pairing asks for another (3.6, every frame after
/// `home_plot::follow_server`): write the live one, read or make the other, apply it onto the
/// running world, and build its home.
///
/// STUB (lane 2A fills it): nothing; one save serves every world.
pub fn follow(_state: &mut EngineState) {}

/// The home a save's start puts on the ship (3.7, pure): its plot (`ship_place` when this ship's
/// and of the start's kind, else `ShipStructure::default_plot_of_kind`) and its design (the
/// save's own home outside the Dev mode, the data file in the Dev mode, the SHIPPED design on a
/// server, the data file offline).
///
/// STUB (lane 2A fills it): refuses, so nothing builds from it yet.
#[allow(clippy::too_many_arguments)]
pub fn home_for(
    _ship_file: ShipStructure,
    _data_dir: &Path,
    _starts: &Starts,
    _start: &str,
    _household: &str,
    _ship_place: Option<&ShipPlace>,
    _own: Option<&SavedHome>,
    _dev: bool,
    _slot: &SaveSlot,
) -> Result<ShipStructure, String> {
    Err("the home of a start is built from increment 6's Wave 2 on (engine/save_slot.rs home_for)".into())
}

/// The server's `game_home` answer (3.7): kept for the picker and Settings; a `chose` while a
/// guest steps out and joins again so the welcome gives the plot. True when it was one.
///
/// STUB (lane 2A fills it): takes none.
pub fn on_game_message(_state: &mut EngineState, _v: &serde_json::Value) -> bool {
    false
}

/// The picker's Enter (3.9; the probe's `{"pairing":"home"}` / `{"pairing":"server"}`): pair
/// with your own home or the connected server, which `follow` then makes the live save.
///
/// STUB (lane 2A fills it).
pub fn enter_pairing(_state: &mut EngineState, _pairing: &str) -> Result<(), String> {
    Err("choosing where you live comes with increment 6's Wave 2".into())
}

/// A server card's Choose (3.9; the probe's `{"choose_start":"cabin"}`): send
/// `game_choose_home` for `start`, and on `chose: true` write the server save's start and place.
///
/// STUB (lane 2A fills it).
pub fn choose_on_server(_state: &mut EngineState, _start: &str) -> Result<(), String> {
    Err("choosing where you live comes with increment 6's Wave 2".into())
}
