//! RESPAWNERS AS PLACES (docs/design/game-setting-and-intro.md section 5.6, rung 2, 2026-11-21).
//!
//! In the game's story a respawner is the pod that grows a new body for a mind that never stopped
//! (the Library page "Coming Back as Yourself"). Until this rung there were none: Respawn stood a
//! player at their own front door, and a guest in the Commons. Now a respawner is a MACHINE, a row
//! of a machine file whose type says `provides: Some("respawn")` (data/machines/*.ron), so where
//! they stand is data (infinite-of-X: a second pod, a pod in a new block, a pod at a mission base
//! later, is a row and nothing else). The shipped ones (2026-11-21):
//!   - each Homestead household's pod, in the main bedroom upstairs (home.ron, home_solo.ron);
//!   - Cabin Block A's shared pod, by its washroom (ship.ron, zone `cabins-1`), since a 7.5 m2
//!     cabin has no room for one;
//!   - the public pod in General Services, beside the Medical Bay (ship.ron, zone
//!     `general-services-1`).
//!
//! WHO MAY WAKE WHERE, by the zone a pod stands in (`Access`): a pod in the player's own home is
//! theirs; a pod in a "residence" zone (a cabin block's hall) belongs to the homes whose door opens
//! on that zone; a pod anywhere else on the ship (General Services, purpose "commons") is public,
//! and anyone may wake there, always. A player with no home aboard (a guest) has only the public
//! ones, so a guest wakes in General Services.
//!
//! WHERE ON IT: standing on the floor just outside the pod's door, which is its +Z face turned by
//! its yaw (`wake_floor`), at the pod's own storey: a pod upstairs wakes you upstairs.
//!
//! PLACE IDS (what `wake_at` in the save and `wake` in a relay join name): "home" for the player's
//! own pod (or their block's), else the id of the zone a public pod stands in
//! ("general-services-1"). One pod per zone is a place; a second pod in the same zone is the same
//! place.
//!
//! Pure: the engine (src/engine/wake.rs) and the relay (`ShipPlots::public_wake`) both read it.

use crate::machines::MachineHome;
use crate::ship_structure::{ShipStructure, HOME_ZONE_ID, SPAWN_EYE_HEIGHT_M};
use glam::Vec3;

/// What a machine type's `provides` says to make it a respawner.
pub const PROVIDES: &str = "respawn";

/// The place id of the player's own respawner (their home's pod, or their block's).
pub const HOME: &str = "home";

/// How far outside a pod's door a person wakes, metres: half a step, so they stand clear of it.
pub const DOOR_CLEARANCE_M: f32 = 0.5;

/// Who may wake at a respawner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// In the player's own home.
    Own,
    /// In a residence zone (a cabin block's hall): for the homes whose door opens on that zone.
    Residents,
    /// Anywhere else on the ship: anyone, always.
    Public,
}

/// One respawner placed on a ship.
#[derive(Debug, Clone, PartialEq)]
pub struct Respawner {
    /// The machine row's id (`respawn_pod_gs`).
    pub machine_id: String,
    /// The zone it stands in.
    pub zone: String,
    /// The zone's label ("General Services"), for the words a person reads.
    pub zone_label: String,
    pub access: Access,
    /// Where a person wakes at it: the floor just outside its door, ship metres, y the floor of
    /// the pod's own storey.
    pub floor: Vec3,
}

impl Respawner {
    /// Where the eye stands when a person wakes here (the floor plus the spawn eye height every
    /// arrival uses, `SPAWN_EYE_HEIGHT_M`).
    pub fn eye(&self) -> Vec3 {
        self.floor + Vec3::Y * SPAWN_EYE_HEIGHT_M
    }
}

/// One place a particular player may wake, as the death screen lists it.
#[derive(Debug, Clone, PartialEq)]
pub struct WakePlace {
    /// "home", or the zone id of a public respawner.
    pub place: String,
    pub respawner: Respawner,
}

impl WakePlace {
    pub fn is_home(&self) -> bool {
        self.place == HOME
    }
}

/// True when machine type `machine` of `home` is a respawner.
pub fn is_respawner(home: &MachineHome, machine: &str) -> bool {
    home.catalog.get(machine).and_then(|d| d.provides.as_deref()) == Some(PROVIDES)
}

/// The floor just outside the door of a machine standing at `pos` (its footprint's centre, y its
/// base), `depth` deep along its own Z, turned `rotation_deg` about Y: its door is its +Z face, and
/// a yaw turns local +Z to (sin yaw, cos yaw), as `Quat::from_rotation_y` draws it.
pub fn wake_floor(pos: Vec3, depth: f32, rotation_deg: f32) -> Vec3 {
    let yaw = rotation_deg.to_radians();
    let out = depth * 0.5 + DOOR_CLEARANCE_M;
    Vec3::new(pos.x + yaw.sin() * out, pos.y, pos.z + yaw.cos() * out)
}

/// Every respawner of machine file `home` (a household merged with the ship's rows, or the ship's
/// rows alone) placed on `ship`, in the file's order. A row whose zone is not on this ship is left
/// out (a home put away, `ShipStructure::home_is_away`, has no "home" zone aboard, and the placer
/// would otherwise stand its pod in the first zone, the Commons).
pub fn respawners_in(ship: &ShipStructure, home: &MachineHome) -> Vec<Respawner> {
    let rects = ship.zone_rects();
    let placed = home.placements(&std::collections::HashMap::new(), Some(&rects));
    let instances = home.all_instances();
    let mut out = Vec::new();
    for p in placed {
        let Some(inst) = instances.iter().find(|i| i.id == p.id) else { continue };
        if !is_respawner(home, &inst.machine) {
            continue;
        }
        let Some(zone) = ship.zones.iter().find(|z| z.id == inst.zone) else { continue };
        if zone.id == HOME_ZONE_ID && ship.home_is_away() {
            continue;
        }
        let access = if zone.id == HOME_ZONE_ID {
            Access::Own
        } else if zone.purpose == "residence" {
            Access::Residents
        } else {
            Access::Public
        };
        let pos = Vec3::new(p.pos.0, p.pos.1, p.pos.2);
        out.push(Respawner {
            machine_id: p.id.clone(),
            zone: zone.id.clone(),
            zone_label: zone.label.clone(),
            access,
            floor: wake_floor(pos, p.size.2, p.rotation),
        });
    }
    out
}

/// The places a player may wake, of `all` (`respawners_in`): their home's ("home": their own pod,
/// or when their home has none, the pod of the residence zone their door opens on, `door_zone`)
/// first, then every public one, one per zone. `has_home` false (a guest, whose home is put away)
/// gives the public ones only. Empty when the ship has none: the caller stands the player at their
/// door, as before respawners.
pub fn wake_places(all: &[Respawner], has_home: bool, door_zone: Option<&str>) -> Vec<WakePlace> {
    let mut out: Vec<WakePlace> = Vec::new();
    if has_home {
        let home = all
            .iter()
            .find(|r| r.access == Access::Own)
            .or_else(|| all.iter().find(|r| r.access == Access::Residents && Some(r.zone.as_str()) == door_zone));
        if let Some(r) = home {
            out.push(WakePlace { place: HOME.to_string(), respawner: r.clone() });
        }
    }
    for r in all.iter().filter(|r| r.access == Access::Public) {
        if !out.iter().any(|w| w.place == r.zone) {
            out.push(WakePlace { place: r.zone.clone(), respawner: r.clone() });
        }
    }
    out
}

/// Where a player wakes, of `places` (`wake_places`): the place they asked for on the death screen
/// (`asked`) when they may wake there, else their usual place (`usual`, the save's `wake_at`; None
/// is "home"), else home, else the first public one. None only when `places` is empty.
pub fn choose<'a>(places: &'a [WakePlace], asked: Option<&str>, usual: Option<&str>) -> Option<&'a WakePlace> {
    let find = |id: &str| places.iter().find(|w| w.place == id);
    asked
        .and_then(find)
        .or_else(|| find(usual.unwrap_or(HOME)))
        .or_else(|| find(HOME))
        .or_else(|| places.first())
}

/// `places` with the player's usual place first (`usual`, None is "home"), the rest in their own
/// order: the death screen's list, "with the default first" (the design's 5.6).
pub fn usual_first(places: &[WakePlace], usual: Option<&str>) -> Vec<WakePlace> {
    let first = choose(places, None, usual).map(|w| w.place.clone());
    let mut out: Vec<WakePlace> = places.iter().filter(|w| Some(&w.place) == first.as_ref()).cloned().collect();
    out.extend(places.iter().filter(|w| Some(&w.place) != first.as_ref()).cloned());
    out
}

/// A respawner of the SHIP (not of anyone's home), as the relay knows it (`ShipPlots::wake`).
#[derive(Debug, Clone, PartialEq)]
pub struct ShipWake {
    /// The zone it stands in, which is its place id.
    pub zone: String,
    /// Where a person's eye stands when they wake there, ship metres.
    pub eye: Vec3,
    /// True for a residence zone's pod (a cabin block's): only for the plots whose door opens on
    /// that zone. False: public, anyone's.
    pub residents: bool,
}

/// The ship's own respawners from its machine rows (data/machines/ship.ron) on a SHIP FILE (no
/// home on it), one per zone, in the file's order: what the relay stands a waking player at when
/// their join names one of them, and every waking guest (`ShipPlots::wake`).
pub fn ship_wake(ship: &ShipStructure, ship_rows: &MachineHome) -> Vec<ShipWake> {
    let mut out: Vec<ShipWake> = Vec::new();
    for r in respawners_in(ship, ship_rows) {
        if r.access == Access::Own || out.iter().any(|w| w.zone == r.zone) {
            continue;
        }
        out.push(ShipWake { eye: r.eye(), residents: r.access == Access::Residents, zone: r.zone });
    }
    out
}
