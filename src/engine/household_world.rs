//! The household permits in the running world (ship homes increment 5b, lane 2A, 2026-10-12):
//! the plots this game may walk into, opened on the live ship every frame, and what the dev IPC
//! reports of the household. The permits themselves (giving, taking back, receiving, the checks
//! with the relay and their answers) are engine/household.rs, which works on the GUI's state and
//! is the GUI crate's; these work on the engine's (the camera, the door panels, the lights), so
//! they are the engine's. Split out in the crate split's rung 0, part B (2026-10-31): they were
//! the second half of engine/household.rs, the half that reached up into the engine.

use std::collections::BTreeSet;

use crate::engine::household::{friends, now_s, Standing};
use crate::gui::GuiState;
use crate::net::household::{self as hh, GivenPermit, HeldPermit, Verdict};
use crate::relay::storage::plot_owner_id;

/// THE OPEN PLOTS NOW (plan 3.7; [`apply_open`]'s core, pure on its parts). Sets `household.usable`
/// from the permits `held` (works this session, not run out at `now_s`, a neighbour's plot of
/// `ship`), then `walkable` = usable, plus the plot the player stands in or in the corridor of
/// (`standing`, ship metres; None while not aboard) when it was walkable already, and `lit` = that
/// plot when walkable. Standing somewhere never opens a plot that was shut, so nobody opens a home
/// by pressing into its corridor; a game restarted inside one is put at its own door on arrival
/// instead (engine/home_plot.rs `plan_welcome`). Returns what `ship.open` should be.
pub(crate) fn open_for(
    household: &mut hh::Household,
    held: &[HeldPermit],
    ship: &crate::ship::ship_structure::ShipStructure,
    standing: Option<glam::Vec3>,
    now_s: u64,
) -> crate::ship::ship_structure::OpenPlots {
    // Nothing held: nothing usable, and no set of the ship's plots built every frame for it.
    household.usable = if held.is_empty() {
        BTreeSet::new()
    } else {
        let neighbours: BTreeSet<&str> = ship.neighbour_plots().map(|p| p.id.as_str()).collect();
        household.usable_plots(held, |p| neighbours.contains(p), now_s)
    };
    // Where the player stands matters only when something is or was open.
    let here: Option<String> = if household.usable.is_empty() && household.walkable.is_empty() {
        None
    } else {
        standing.and_then(|p| crate::engine::home_plot::plot_ground_of(ship, p)).map(|p| p.id.clone())
    };
    let mut walkable = household.usable.clone();
    if let Some(h) = here.as_ref().filter(|h| household.walkable.contains(*h)) {
        walkable.insert(h.clone());
    }
    household.lit = here.filter(|h| walkable.contains(h));
    household.walkable = walkable;
    crate::ship::ship_structure::OpenPlots { plots: household.walkable.clone(), lit: household.lit.clone() }
}

/// Make `ship.open` `want`; true when it changed. Compared with the LIVE ship each frame, so a ship
/// rebuilt since (which starts with nothing open) is opened again.
pub(crate) fn set_open(ship: &mut crate::ship::ship_structure::ShipStructure, want: crate::ship::ship_structure::OpenPlots) -> bool {
    if ship.open == want {
        return false;
    }
    ship.open = want;
    true
}

/// Once a frame (engine/shared_build.rs `tick`, joined or not): the plots this game may walk into
/// ([`open_for`]), from where the camera stands while aboard; when they changed, the ship takes
/// them and the colliders, doors and lights that read them are redone
/// (engine/home_meshes.rs `open_plots_changed`).
pub(crate) fn apply_open(state: &mut crate::engine::state::EngineState) {
    let standing = crate::engine::home_plot::aboard(state).then_some(state.camera.position);
    let gs = &mut state.gui_state;
    let Some(ship) = gs.ship_structure.as_ref() else { return };
    let held: &[HeldPermit] = gs.dm_store.as_ref().map_or(&[], |s| s.permits_held());
    let want = open_for(&mut gs.household, held, ship, standing, now_s());
    let Some(ship) = gs.ship_structure.as_mut() else { return };
    if set_open(ship, want) {
        crate::engine::home_meshes::open_plots_changed(state);
    }
}

// ── What the dev IPC reports (engine/ipc.rs, the recorder's `household`) ───────────────────────

/// The probe's `household` (engine/ipc.rs's recorder, done JSON; scripts/verify-copresence.js
/// --household reads it).
pub(crate) fn probe_json(state: &crate::engine::state::EngineState) -> serde_json::Value {
    probe_core(&state.gui_state, &state.door_panels)
}

/// [`probe_json`] on its parts: `doors` are the door panels with their open fractions
/// (`EngineState::door_panels`). Fields:
/// - `me` (my full key), `me_did` (my id as the relay holds plots under it), `server` (the server's
///   own `did:hum:` this DM store holds), `my_plot` (this session's);
/// - `friends`: the card's "Your plot" rows, each with its permit's standing;
/// - `given` and `held`: every permit given and held on this server, `given` with its state as the
///   card reads it, `held` with what the relay said this session (`verdict`: works, checking,
///   later, refused, or null; `why` for refused);
/// - `usable`, `walkable`, `lit`, `open_plots` (what the ship has open now) and `open_lights` (how
///   many lights the lit open plot adds);
/// - `mouths`: each neighbour plot's corridor door mouths, both ends, with `others_only` (true:
///   shut to this player) and `open` (the doors' largest open fraction);
/// - `checks_waiting`, `sending`, `withdrawals_waiting` and `line`.
pub(crate) fn probe_core(gs: &GuiState, doors: &[(crate::ship::door_panels::PanelPlacement, f32)]) -> serde_json::Value {
    use serde_json::json;
    let store = gs.dm_store.as_ref();
    let household = &gs.household;
    let me = gs.profile_public_key.as_str();
    let friend_rows: Vec<serde_json::Value> = friends(gs)
        .iter()
        .map(|r| {
            json!({
                "key": r.key,
                "name": r.name,
                "friend": r.friend,
                "blocked": r.blocked,
                "sealable": r.sealable,
                "permit": standing_json(&r.permit),
                "other_plots": r.other_plots,
            })
        })
        .collect();
    let given: Vec<serde_json::Value> = store
        .map(|s| {
            s.permits_given()
                .iter()
                .map(|g| {
                    let row = GivenPermit { state: household.given_state(g), ..g.clone() };
                    serde_json::to_value(row).unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default();
    let held: Vec<serde_json::Value> = store
        .map(|s| {
            s.permits_held()
                .iter()
                .map(|h| {
                    let (verdict, why) = match household.verdict(h) {
                        None => (None, None),
                        Some(Verdict::Works) => (Some("works"), None),
                        Some(Verdict::Checking) => (Some("checking"), None),
                        Some(Verdict::Later) => (Some("later"), None),
                        Some(Verdict::Refused(w)) => (Some("refused"), Some(w.as_str())),
                    };
                    json!({
                        "from": h.from,
                        "plot": h.permit.plot,
                        "serial": h.permit.serial,
                        "expiry": h.permit.expiry,
                        "at": h.at,
                        "verdict": verdict,
                        "why": why,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let ship = gs.ship_structure.as_ref();
    json!({
        "me": me,
        "me_did": (!me.is_empty()).then(|| plot_owner_id(me)),
        "server": store.and_then(|s| s.pass_server()),
        "my_plot": household.my_plot,
        "friends": friend_rows,
        "given": given,
        "held": held,
        "usable": household.usable,
        "walkable": household.walkable,
        "lit": household.lit,
        "open_plots": ship.map(|s| s.open.plots.clone()).unwrap_or_default(),
        "open_lights": ship.map_or(0, |s| crate::engine::home_meshes::open_plot_lights(s).len()),
        "mouths": ship.map_or_else(|| json!({}), |s| mouths_json(s, doors)),
        "checks_waiting": household.asked.len(),
        "sending": household.on_their_way.len(),
        "withdrawals_waiting": store.map_or(0, |s| s.pending_withdrawals().len()),
        "line": household.line,
    })
}

/// A [`Standing`] as the probe writes it: `{"state": "stands", "expiry": ..., "days": ...}`.
fn standing_json(s: &Standing) -> serde_json::Value {
    use serde_json::json;
    match s {
        Standing::None => json!({ "state": "none" }),
        Standing::Sending { expiry } => json!({ "state": "sending", "expiry": expiry }),
        Standing::Stands { expiry, days } => json!({ "state": "stands", "expiry": expiry, "days": days }),
        Standing::Unconfirmed { expiry } => json!({ "state": "unconfirmed", "expiry": expiry }),
        Standing::Refused { reason } => json!({ "state": "refused", "reason": reason }),
        Standing::RanOut { expiry } => json!({ "state": "ran_out", "expiry": expiry }),
        Standing::TakingBack { expiry } => json!({ "state": "taking_back", "expiry": expiry }),
        Standing::TakenBack { expiry } => json!({ "state": "taken_back", "expiry": expiry }),
    }
}

/// Each neighbour plot's corridor door mouths (`NeighbourView::mouths_by_plot`, both ends of its
/// corridor), keyed by plot: where the mouth is (its aperture's floor centre, ship metres), how many
/// door halves stand in it, whether they open only for the other players (`others_only`, null with
/// none), and how far open they are (the larger fraction). The halves are found by where they stand
/// among the neighbours' doors, which `ship_panel_placements` puts last.
fn mouths_json(ship: &crate::ship::ship_structure::ShipStructure, doors: &[(crate::ship::door_panels::PanelPlacement, f32)]) -> serde_json::Value {
    use crate::ship::ship_structure::CorridorAxis;
    let view = crate::ship::neighbours::neighbour_view(ship);
    let tail_len = crate::ship::door_panels::neighbour_panel_placements(ship).len();
    let tail = &doors[doors.len().saturating_sub(tail_len)..];
    let mut out: std::collections::BTreeMap<String, Vec<serde_json::Value>> = std::collections::BTreeMap::new();
    for (plot, m) in view.mouths_by_plot(ship) {
        let (x, z) = match m.axis {
            CorridorAxis::X => (m.plane, m.lat),
            CorridorAxis::Z => (m.lat, m.plane),
        };
        let in_mouth = |c: glam::Vec3| {
            let (along, across) = match m.axis {
                CorridorAxis::X => (c.x, c.z),
                CorridorAxis::Z => (c.z, c.x),
            };
            (along - m.plane).abs() < 0.05 && (across - m.lat).abs() <= m.door.0 * 0.5 + 0.01 && (c.y - m.floor_y).abs() < 0.05
        };
        let halves: Vec<&(crate::ship::door_panels::PanelPlacement, f32)> = tail.iter().filter(|(p, _)| in_mouth(p.center)).collect();
        let others_only = (!halves.is_empty()).then(|| halves.iter().all(|(p, _)| p.others_only));
        let open = halves.iter().map(|(_, f)| *f).fold(0.0f32, f32::max);
        out.entry(plot.to_string()).or_default().push(serde_json::json!({
            "at": [x, m.floor_y, z],
            "doors": halves.len(),
            "others_only": others_only,
            "open": open,
        }));
    }
    serde_json::to_value(out).unwrap_or_default()
}
