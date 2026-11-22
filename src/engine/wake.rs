//! WAKING AT A RESPAWNER: the engine's half of respawners as places
//! (docs/design/game-setting-and-intro.md 5.5 and 5.6, rung 2, 2026-11-21).
//!
//! The places themselves are data and pure logic (crates/humanity-ship/src/respawners.rs): every
//! machine whose type `provides: "respawn"`, who may use it by the zone it stands in. This file
//! joins them to the running game:
//!   - `places`: where THIS player may wake now: their home's pod (or their cabin block's), and
//!     General Services, from the household's machine file and the ship's own rows;
//!   - `refresh_view`: the death screen's list (gui/wake_view.rs), filled once per death, their
//!     usual place first;
//!   - `respawn_spot`: at the respawn (frame_settles.rs), where they wake: the place they pressed,
//!     else their usual place, else home, else General Services; None when the ship has no
//!     respawner they may use, and the respawn stands them at their door as before;
//!   - `join_field`: in the shared world the respawn goes through the relay (it steps out and joins
//!     again, home_plot.rs `respawn_through_relay`), and the join it causes carries `wake`, so the
//!     relay stands them at the same pod (src/relay/handlers/game_state.rs `assign_home`);
//!   - `choose_here`: E at a respawner makes it the usual place (`wake_at` in the save);
//!   - `usual` / `put_usual`: the usual place lives in the DataStore while the game runs, and in
//!     the save (save_load.rs gathers and restores it, as it does the mess hall's meter).
//!
//! A guest (the shared world gave them no plot, so their home is put away) has only the public
//! respawners, so a guest wakes in General Services.

use crate::engine::state::EngineState;
use glam::Vec3;
use humanity_core::hot_reload::data_store::DataStore;
use humanity_core::i18n::{fill, t};
use humanity_ship::machines::MachineHome;
use humanity_ship::respawners::{self, Access, WakePlace};
use humanity_ship::ship_structure::ShipStructure;
use std::sync::Mutex;

/// The DataStore key of the usual place: Mutex<Option<String>>, None being "home".
pub(crate) const USUAL_KEY: &str = "wake_at";

/// The dev-only place the rigs may ask for: the front door (`respawn_spot`). Never listed.
pub(crate) const DEV_DOOR: &str = "door";

/// Where the player may wake now (`respawners::wake_places`): the respawners in the household's
/// machine file, which merges the ship's own rows (General Services' and Cabin Block A's pods), so
/// General Services is always among them. Empty with no ship.
pub(crate) fn places(state: &EngineState) -> Vec<WakePlace> {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else { return Vec::new() };
    places_for(ship, state.gui_state.home_machines.as_ref())
}

/// `places` for a ship and a household's machines (pure). Every shipped household file merges the
/// ship's rows since 2026-11-21, the one-person homestead's too (a household's figures count only
/// its own machines, `humanity_ship::machines::owner`), so the ship's pods are among its machines
/// and drawn with its home (crates/humanity-ship/src/respawners_tests.rs
/// `every_household_draws_the_ships_pods`). Until then this read data/machines/ship.ron for a
/// household file that linked none of the ship's rows, and the death screen said under General
/// Services that its pod was not drawn.
pub(crate) fn places_for(ship: &ShipStructure, home: Option<&MachineHome>) -> Vec<WakePlace> {
    let all = home.map(|h| respawners::respawners_in(ship, h)).unwrap_or_default();
    places_on(ship, &all)
}

/// The death screen's list from the places to list (the usual place first): each one's words and
/// the usual one marked.
pub(crate) fn entries(listed: &[WakePlace]) -> Vec<crate::gui::wake_view::WakeEntry> {
    listed
        .iter()
        .enumerate()
        .map(|(i, w)| crate::gui::wake_view::WakeEntry { place: w.place.clone(), name: name_of(w), usual: i == 0 })
        .collect()
}

/// `places` from respawners already found: a home put away (a guest's) has none of its own.
pub(crate) fn places_on(ship: &ShipStructure, all: &[respawners::Respawner]) -> Vec<WakePlace> {
    let has_home = ship.home.is_some() && !ship.home_is_away();
    let door_zone = ship.home_plot().map(|p| p.door.zone.clone());
    respawners::wake_places(all, has_home, door_zone.as_deref())
}

/// The words for a place on the death screen and in a notice.
pub(crate) fn name_of(w: &WakePlace) -> String {
    if w.is_home() {
        t("hud.wake_home").to_string()
    } else {
        w.respawner.zone_label.clone()
    }
}

/// The usual place while the game runs: Some(None) for "home", None when nothing has registered
/// it yet (no save applied this session; the save then keeps what it had).
pub(crate) fn usual(data: &DataStore) -> Option<Option<String>> {
    data.get::<Mutex<Option<String>>>(USUAL_KEY).and_then(|m| m.lock().ok().map(|v| v.clone()))
}

/// Make `place` the usual place, registering it when it is not there yet (a save applied).
pub(crate) fn put_usual(data: &mut DataStore, place: Option<String>) {
    if data.get::<Mutex<Option<String>>>(USUAL_KEY).is_none() {
        data.insert(USUAL_KEY, Mutex::new(None::<String>));
    }
    set_usual(data, place);
}

/// `put_usual` through a shared DataStore: only when it is registered.
pub(crate) fn set_usual(data: &DataStore, place: Option<String>) {
    if let Some(Ok(mut m)) = data.get::<Mutex<Option<String>>>(USUAL_KEY).map(|m| m.lock()) {
        *m = place;
    }
}

/// What a save written now says of the usual place: the running game's when it has one, else what
/// the file already said (`file`), so a save before any save was applied does not forget it.
pub(crate) fn usual_for_save(data: &DataStore, file: impl FnOnce() -> Option<String>) -> Option<String> {
    match usual(data) {
        Some(v) => v,
        None => file(),
    }
}

/// Fill the death screen's list once per death, the usual place first, and empty it once the
/// player lives again. Called every frame before the respawn (frame_settles.rs).
pub(crate) fn refresh_view(state: &mut EngineState) {
    if state.gui_state.player_death_cause.is_none() {
        if state.gui_state.wake.filled {
            state.gui_state.wake.places.clear();
            state.gui_state.wake.filled = false;
        }
        return;
    }
    if state.gui_state.wake.filled {
        return;
    }
    let usual = usual(&state.data_store).flatten();
    let listed = respawners::usual_first(&places(state), usual.as_deref());
    state.gui_state.wake.places = entries(&listed);
    state.gui_state.wake.filled = true;
}

/// Where the respawn wakes the player (eye height), taking the place they pressed: the chosen
/// respawner, with the `wake` field the next relay join carries set (`join_field`). None when no
/// respawner is theirs to use: the caller stands them at their door (`fps_spawn`).
pub(crate) fn respawn_spot(state: &mut EngineState) -> Option<Vec3> {
    let asked = state.gui_state.wake.asked.take();
    // The dev place "door" (the showcase `{"respawn":"door"}`, permanent dev tooling): wake at the
    // front door as every respawn did before respawners, with no `wake` in the relay join, so the
    // relay stands the player at their door. verify-copresence's Respawn leg uses it: its later
    // legs walk from the door on the ground floor (verify-respawn covers the pods).
    if asked.as_deref() == Some(DEV_DOOR) {
        state.gui_state.wake.pending_join = None;
        state.gui_state.wake.last = Some((DEV_DOOR.to_string(), state.fps_spawn));
        return None;
    }
    let usual = usual(&state.data_store).flatten();
    let places = places(state);
    let w = respawners::choose(&places, asked.as_deref(), usual.as_deref())?.clone();
    state.gui_state.wake.pending_join = join_field(state.gui_state.ship_structure.as_ref(), &w);
    log::info!("[Vitals] waking at {} ({}, machine {}) at {:?}", w.place, w.respawner.zone, w.respawner.machine_id, w.respawner.floor);
    state.gui_state.wake.last = Some((w.place.clone(), w.respawner.eye()));
    Some(w.respawner.eye())
}

/// The `wake` field of the join a respawn in the shared world causes: the home's own pod in
/// PLOT-LOCAL metres, its floor (`{"place":"home","at":[x,y,z]}`), so the relay stands the player
/// there on whichever plot it holds for them, upstairs included; a ship's respawner (a cabin
/// block's, General Services') by its zone (`{"place":"general-services-1"}`), which the relay
/// looks up and checks itself. None for a home pod with no plot (offline play never joins).
pub(crate) fn join_field(ship: Option<&ShipStructure>, w: &WakePlace) -> Option<serde_json::Value> {
    if w.respawner.access == Access::Own {
        let plot = ship?.home_plot()?;
        let at = w.respawner.floor - Vec3::new(plot.origin.0, plot.origin.1, plot.origin.2);
        Some(serde_json::json!({ "place": respawners::HOME, "at": [at.x, at.y, at.z] }))
    } else {
        Some(serde_json::json!({ "place": w.respawner.zone }))
    }
}

/// Put the pending `wake` field (`join_field`) into a join about to go out, taking it: only the
/// join a respawn causes carries one.
pub(crate) fn add_to_join(join: &mut serde_json::Value, state: &mut EngineState) {
    if let (Some(obj), Some(wake)) = (join.as_object_mut(), state.gui_state.wake.pending_join.take()) {
        obj.insert("wake".into(), wake);
    }
}

/// The places the player may wake whose pod the world drew (`probe_json`'s `pods_drawn`): those
/// whose respawner has a machine label this frame, the label the world builds with each placed
/// machine's mesh (engine/home_meshes.rs).
fn pods_drawn(state: &EngineState) -> Vec<String> {
    places(state)
        .iter()
        .filter(|w| state.gui_state.machine_labels.iter().any(|l| l.machine_id == w.respawner.machine_id))
        .map(|w| w.place.clone())
        .collect()
}

/// The probe's `wake` block (debug/remote_players_request.json, engine/ipc.rs): every place the
/// player may wake now with its eye, the place and eye the last respawn chose, and the floor stood
/// on and the room the HUD names this frame (engine/storey_probe.rs), so scripts/verify-respawn.js
/// judges where a respawn really stood the player.
pub(crate) fn probe_json(state: &EngineState) -> serde_json::Value {
    let places: Vec<serde_json::Value> = places(state)
        .iter()
        .map(|w| serde_json::json!({ "place": w.place, "zone": w.respawner.zone, "eye": [w.respawner.eye().x, w.respawner.eye().y, w.respawner.eye().z] }))
        .collect();
    let last = state.gui_state.wake.last.as_ref().map(|(p, e)| serde_json::json!({ "place": p, "eye": [e.x, e.y, e.z] }));
    serde_json::json!({
        "places": places,
        "listed": state.gui_state.wake.places.iter().map(|w| w.place.clone()).collect::<Vec<_>>(),
        // The listed places whose pod the world drew a machine for (its label, engine/home_meshes.rs,
        // built from the placed machines the world draws): every one, since every household file
        // merges the ship's rows (2026-11-21; the one-person homestead drew none of the ship's).
        "pods_drawn": pods_drawn(state),
        "last": last,
        "usual": usual(&state.data_store).flatten(),
        "storey": crate::engine::storey_probe::frame_json(state),
    })
}

/// E at the respawner machine `machine_id`: it becomes the usual place, when the player may wake
/// there, with a notice saying so; a block's pod that is not theirs says whose it is.
pub(crate) fn choose_here(state: &mut EngineState, machine_id: &str) {
    let places = places(state);
    let notice = match places.iter().find(|w| w.respawner.machine_id == machine_id) {
        Some(w) => {
            let place = (!w.is_home()).then(|| w.place.clone());
            if state.data_store.get::<Mutex<Option<String>>>(USUAL_KEY).is_none() {
                state.data_store.insert(USUAL_KEY, Mutex::new(None::<String>));
            }
            set_usual(&state.data_store, place);
            fill(t("hud.wake_chosen"), &[("place", &name_of(w))])
        }
        None => t("hud.wake_not_yours").to_string(),
    };
    state.gui_state.pending_notices.push(notice);
}

#[cfg(test)]
#[path = "wake_tests.rs"]
mod tests;
