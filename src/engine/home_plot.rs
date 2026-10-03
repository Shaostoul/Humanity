//! Your home on the shared ship (increment 1b of docs/design/ship-homes-and-logistics.md).
//!
//! WHY: a relay now hands every player a plot of the mothership and spawns them on it
//! (src/relay/handlers/game_state.rs `assign_home`). The game first builds the home on the
//! ship's default plot (p1, increment 1a), because it does not know its plot until it joins.
//! If the relay then says p2 and the game kept drawing its home at p1, the player would stand
//! in p1 while the relay holds them at p2's front door, 99 to 139 m away: every update they
//! sent would be refused by the relay's 100 m rule, and everyone else would see them frozen.
//! So the two halves land together, and this file is the game's half.
//!
//! On every `game_welcome` the game:
//!   1. checks the relay's ship is its own ship (the `ship.hash`, `ShipStructure::ship_hash`):
//!      positions only agree when everyone has the same ship, so on a mismatch it refuses to
//!      join, with one plain sentence;
//!   2. moves its `home` zone to the plot the relay named, through 1a's assembly (the ship
//!      file plus the home design on that plot plus the plot's door corridor), rebuilds the
//!      home through the editor's rebuild path, and puts the camera at the plot's spawn, the
//!      same point the relay spawned the player at (`ShipStructure::plot_spawn`);
//!   3. as a guest (the ship is full, `home_plot` null), puts the camera in the Commons, where
//!      the relay put them (`ShipStructure::guest_spawn`).
//!
//! `plan_welcome` decides, and is pure (tested below); `apply_welcome_home` does it.
//!
//! What follows the home on a move: walls, floors, collision, room lights, the sealed bounds,
//! machines (with their grow plots, screens and pipes) and door panels, which is everything
//! the construction editor's rebuild moves. Decoration plants, livestock anchors and the
//! hologram room are placed once by the world load; they follow on the next world entry,
//! once increment 2 remembers the plot so the home is built in the right place before joining.

use crate::engine::state::EngineState;
use crate::ship::ship_structure::ShipStructure;
use glam::Vec3;

/// The sentence a player reads when the server's ship is not theirs (the design: "one plain
/// sentence: positions only agree when everyone has the same ship").
pub(crate) const SHIP_MISMATCH: &str =
    "Not joining the shared world: this server has a different ship from yours, and positions only agree when everyone has the same ship.";

/// What a welcome asks of the home.
#[derive(Debug)]
pub(crate) enum WelcomeHome {
    /// Do not join: the sentence to show.
    Refuse(String),
    /// The home already stands on the plot the relay named: nothing moves.
    Stay,
    /// Move the home: the ship re-assembled on `plot`, and where the camera goes.
    Move { ship: ShipStructure, plot: String, spawn: Vec3 },
    /// The ship is full: a guest, arriving in the Commons at `spawn`.
    Guest { spawn: Vec3 },
}

/// Decide what a `game_welcome` asks of the home, from the ship the game runs now (`None` when
/// no ship assembled and the legacy layout is showing). Pure.
pub(crate) fn plan_welcome(ship: Option<&ShipStructure>, welcome: &serde_json::Value) -> WelcomeHome {
    let theirs = welcome.get("ship").and_then(|s| s.get("hash")).and_then(|h| h.as_str());
    let Some(ship) = ship else { return WelcomeHome::Refuse(SHIP_MISMATCH.to_string()) };
    if theirs != Some(ship.ship_hash().as_str()) {
        return WelcomeHome::Refuse(SHIP_MISMATCH.to_string());
    }
    let Some(hp) = welcome.get("home_plot").filter(|v| !v.is_null()) else {
        return match ship.guest_spawn() {
            Some(spawn) => WelcomeHome::Guest { spawn },
            None => WelcomeHome::Stay,
        };
    };
    let Some(id) = hp.get("id").and_then(|i| i.as_str()) else {
        return WelcomeHome::Refuse(SHIP_MISMATCH.to_string());
    };
    if ship.home.as_ref().is_some_and(|a| a.plot == id) {
        return WelcomeHome::Stay;
    }
    let (Some(design), Some(plot)) = (ship.home_design(), ship.plots.iter().find(|p| p.id == id)) else {
        return WelcomeHome::Refuse(SHIP_MISMATCH.to_string());
    };
    let spawn = ShipStructure::plot_spawn(plot, &design);
    match ship.ship_file().assemble(design, id) {
        Ok(moved) => WelcomeHome::Move { ship: moved, plot: id.to_string(), spawn },
        Err(e) => WelcomeHome::Refuse(format!("Not joining the shared world: your home does not fit the plot this server gave you ({e}).")),
    }
}

/// Act on a `game_welcome` (engine/net_route.rs, before the welcome reaches net_sync). Returns
/// false when the game refused to join: the caller then drops the welcome.
pub(crate) fn apply_welcome_home(state: &mut EngineState, welcome: &serde_json::Value) -> bool {
    match plan_welcome(state.gui_state.ship_structure.as_ref(), welcome) {
        WelcomeHome::Refuse(sentence) => {
            log::warn!("Co-presence: {sentence} (server ship {}, ours {:?})", welcome["ship"], state.gui_state.ship_structure.as_ref().map(|s| s.ship_hash()));
            if let Some(ref ws) = state.gui_state.ws_client {
                ws.send(&serde_json::json!({ "type": "game_leave" }).to_string());
            }
            state.game_joined = false;
            state.gui_state.copresence_active = false;
            // Not again on this server until the world is loaded afresh (lib.rs join gate).
            state.copresence_refused = Some(state.gui_state.server_url.clone());
            state.gui_state.pending_notices.push(sentence);
            return false;
        }
        WelcomeHome::Stay => {}
        WelcomeHome::Move { ship, plot, spawn } => {
            log::info!("Co-presence: the server gave us plot {plot}; moving the home there");
            state.gui_state.construction_zone = ship.home_zone_index();
            state.gui_state.ship_structure = Some(ship);
            // The editor's own rebuild, called directly rather than through the dirty flag: a
            // plot assignment is not an edit, so it must not arm the autosave or the undo history.
            crate::engine::home_meshes::rebuild_homestead(state);
            put_player_at(state, spawn);
        }
        WelcomeHome::Guest { spawn } => {
            log::info!("Co-presence: the ship is full; joining as a guest in the Commons");
            put_player_at(state, spawn);
        }
    }
    state.game_welcomed = true;
    true
}

/// Stand the player at `at` (ship metres, eye height): the camera and the walking body, the
/// way the showcase request's `cam` verb does it (engine/ipc.rs), since in first person the
/// camera follows the body each frame.
fn put_player_at(state: &mut EngineState, at: Vec3) {
    for (_e, (t, _c)) in state
        .game_world
        .world
        .query_mut::<(&mut crate::ecs::components::Transform, &crate::ecs::components::Controllable)>()
    {
        t.position = at;
    }
    state.camera.position = at;
}

/// For the recorder's probe (engine/ipc.rs): the plot the home stands on and every plot of the
/// ship, so a rig can check where the game put its camera and the other players.
pub(crate) fn probe_json(ship: Option<&ShipStructure>) -> (serde_json::Value, serde_json::Value) {
    let plot_json = |p: &crate::ship::ship_structure::Plot| {
        serde_json::json!({
            "id": p.id,
            "kind": p.kind,
            "origin": [p.origin.0, p.origin.1, p.origin.2],
            "size": [p.size.0, p.size.1, p.size.2],
        })
    };
    let Some(ship) = ship else { return (serde_json::Value::Null, serde_json::json!([])) };
    let home = ship.home_plot().map(plot_json).unwrap_or(serde_json::Value::Null);
    (home, serde_json::Value::Array(ship.plots.iter().map(plot_json).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_dir() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// The game as it boots: the home on the default plot, p1.
    fn booted() -> ShipStructure {
        ShipStructure::load_and_assemble(&data_dir(), None).expect("the shipped ship assembles")
    }

    /// A welcome as the relay sends it (relay/handlers/msg_handlers.rs handle_game_join).
    fn welcome(plot: Option<&str>, hash: &str) -> serde_json::Value {
        let ship = booted();
        let home_plot = match plot {
            Some(id) => {
                let p = ship.plots.iter().find(|p| p.id == id).unwrap();
                serde_json::json!({ "id": id, "kind": p.kind, "origin": [p.origin.0, p.origin.1, p.origin.2], "size": [p.size.0, p.size.1, p.size.2] })
            }
            None => serde_json::Value::Null,
        };
        serde_json::json!({ "type": "game_welcome", "player_id": 7, "home_plot": home_plot, "ship": { "id": "mothership-1", "hash": hash } })
    }

    /// THE CASE 1b EXISTS FOR: the relay says p2. The home moves to p2 (every room and the
    /// spawn by exactly p2's offset, through 1a's assembly) and the camera goes to p2's spawn,
    /// the point the relay holds the player at. Seen red 2026-10-03 with the Move arm of
    /// `plan_welcome` replaced by `WelcomeHome::Stay` (the 1a client, which never moved its
    /// home): "the relay said p2 and the home did not move: Stay".
    #[test]
    fn a_welcome_naming_another_plot_moves_the_home_there() {
        let ship = booted();
        let hash = ship.ship_hash();
        match plan_welcome(Some(&ship), &welcome(Some("p2"), &hash)) {
            WelcomeHome::Move { ship: moved, plot, spawn } => {
                assert_eq!(plot, "p2");
                assert_eq!(moved.home.as_ref().map(|a| a.plot.as_str()), Some("p2"));
                let dz = 99.0; // p2 sits 99 m south of p1 (design section 2.4)
                let home = |s: &ShipStructure| s.zones[s.home_zone_index()].origin;
                assert_eq!(home(&moved), (0.0, 0.0, dz));
                assert_eq!(moved.zones.len(), ship.zones.len());
                // The spawn: what the relay spawns at, and what the game's own path gives.
                assert!((spawn - Vec3::new(53.5, 1.7, 139.5)).length() < 1e-4, "spawn {spawn:?}");
                assert!((moved.home_spawn_world().unwrap() - spawn).length() < 1e-4);
                // The camera point lies inside p2's box, not p1's.
                let (lo, hi) = moved.home_plot().unwrap().aabb();
                assert!(spawn.cmpge(lo).all() && spawn.cmple(hi).all());
                // Moving is not a new ship: the fingerprint holds.
                assert_eq!(moved.ship_hash(), hash);
            }
            other => panic!("the relay said p2 and the home did not move: {other:?}"),
        }
    }

    /// The home already on the plot the relay named: nothing moves, the camera stays put.
    #[test]
    fn a_welcome_naming_our_own_plot_changes_nothing() {
        let ship = booted();
        let w = welcome(Some("p1"), &ship.ship_hash());
        assert!(matches!(plan_welcome(Some(&ship), &w), WelcomeHome::Stay));
    }

    /// A full ship: home_plot null, and the player arrives in the Commons, at the point the
    /// relay put them (game_state.rs assign_home uses the same `guest_spawn`).
    #[test]
    fn a_full_ship_makes_a_guest_in_the_commons() {
        let ship = booted();
        match plan_welcome(Some(&ship), &welcome(None, &ship.ship_hash())) {
            WelcomeHome::Guest { spawn } => assert!((spawn - Vec3::new(82.0, 1.7, 47.5)).length() < 1e-4, "{spawn:?}"),
            other => panic!("a full ship should make a guest: {other:?}"),
        }
    }

    /// Another ship (a different hash), no ship named at all, or no ship of our own: refuse,
    /// with the one sentence, and never move the home.
    #[test]
    fn another_ship_is_refused_in_one_plain_sentence() {
        let ship = booted();
        for w in [
            welcome(Some("p2"), "0123456789abcdef"),
            serde_json::json!({ "type": "game_welcome", "player_id": 7, "home_plot": null }),
        ] {
            match plan_welcome(Some(&ship), &w) {
                WelcomeHome::Refuse(s) => assert_eq!(s, SHIP_MISMATCH),
                other => panic!("a different ship must be refused: {other:?}"),
            }
        }
        assert!(matches!(plan_welcome(None, &welcome(Some("p1"), &ship.ship_hash())), WelcomeHome::Refuse(_)));
        // One sentence: one full stop, at the end.
        assert_eq!(SHIP_MISMATCH.matches(". ").count(), 0);
        assert!(SHIP_MISMATCH.ends_with('.'));
    }
}
