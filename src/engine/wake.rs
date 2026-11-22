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

/// Where the player may wake now (`respawners::wake_places`): the household's machine file
/// (which merges the ship's rows when it names them) and, for a household that does not (the
/// one-person homestead), the ship's own rows read from data/machines/ship.ron, so General
/// Services is always among them. Empty with no ship.
pub(crate) fn places(state: &EngineState) -> Vec<WakePlace> {
    let Some(ship) = state.gui_state.ship_structure.as_ref() else { return Vec::new() };
    places_for(ship, state.gui_state.home_machines.as_ref(), &state.data_dir)
}

/// `places` for a ship and a household's machines (pure apart from reading data/machines/ship.ron
/// for a household file that links none of the ship's rows).
pub(crate) fn places_for(ship: &ShipStructure, home: Option<&MachineHome>, data_dir: &std::path::Path) -> Vec<WakePlace> {
    let mut all = home.map(|h| respawners::respawners_in(ship, h)).unwrap_or_default();
    if !home.is_some_and(|h| h.ship_machines.is_some()) {
        let file = data_dir.join("machines").join("ship.ron");
        if let Some(rows) = MachineHome::load_file(&file) {
            for r in respawners::respawners_in(ship, &rows) {
                if !all.iter().any(|a| a.machine_id == r.machine_id) {
                    all.push(r);
                }
            }
        }
    }
    places_on(ship, &all)
}

/// Is the respawner of `w` drawn in this household's world? The world draws the household's
/// machine file (`MachineHome::placements`), so a pod that file does not hold is not drawn: the
/// ship's own pods for a household file that links none of the ship's rows. home_solo.ron is such a
/// file (2026-11-21): the one-person homestead's machine list is the honest baseline of what one
/// person runs, and its power, water and food figures add up every machine it holds, so merging the
/// ship's rows would count the Commons' garden, humidifiers and fish tank as this household's own
/// (and draw the ship's composter and wall screen with the one-person home's smaller definitions).
/// Until the household's figures count only its own machines, the death screen says so instead
/// (`entries`).
pub(crate) fn drawn_here(home: Option<&MachineHome>, w: &WakePlace) -> bool {
    home.is_some_and(|h| h.all_instances().iter().any(|i| i.id == w.respawner.machine_id))
}

/// The death screen's list from the places to list (the usual place first): each one's words, the
/// usual one marked, and for a place whose pod this household does not draw (`drawn_here`) the
/// sentence that says why (`hud.wake_not_drawn`), so nobody wakes beside a pod they never saw.
pub(crate) fn entries(listed: &[WakePlace], home: Option<&MachineHome>) -> Vec<crate::gui::wake_view::WakeEntry> {
    listed
        .iter()
        .enumerate()
        .map(|(i, w)| crate::gui::wake_view::WakeEntry {
            place: w.place.clone(),
            name: name_of(w),
            usual: i == 0,
            note: (!drawn_here(home, w)).then(|| t("hud.wake_not_drawn").to_string()),
        })
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
    state.gui_state.wake.places = entries(&listed, state.gui_state.home_machines.as_ref());
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
        // The listed places whose pod this household does not draw, each said so on the screen.
        "not_drawn": state.gui_state.wake.places.iter().filter(|w| w.note.is_some()).map(|w| w.place.clone()).collect::<Vec<_>>(),
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
