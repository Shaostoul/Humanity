//! The ship's ELEVATORS as plain data (v0.590, ship homes increment 7): each one aboard with its key,
//! its base, its turn, its type and the STOPS its shaft spans, read from the ship's zones. The
//! footing runs the cars and rides them (engine/footing.rs `elevator_tick`, `ElevatorCars`), the
//! HUD names one by its stops (gui/pages/hud.rs `lift_card`), the probe and the drawing list them.
//! Here, in the ship, since the crate split's rung 0, part B (2026-10-31): it is ship geometry and
//! nothing else, and the HUD (the GUI crate's, under the engine) reads it too; it was
//! engine/footing.rs's.

use glam::Vec3;

use crate::ship::home_structure::{HomeStructure, STOREY_EPS};
use crate::ship::ship_structure::ShipStructure;
use crate::ship::structure::{self, StructureKind, StructureType};

/// An elevator's key in `ElevatorCars`: its zone's id (`ShipZone::id`) and its own id
/// (`PlacedStructure::id`, unique in its body and never empty: `settle_structures` gives every
/// piece read one, and the editor every piece it places). Neither changes when the build editor
/// adds or takes out a piece (the increment 7 follow-ups, 2026-10-21): the cars were keyed by
/// (zone index, structure index), and since a car's state survives the editor
/// (`pause_elevators`), taking out a piece listed before an elevator handed its car to the next
/// one down the list.
pub type LiftKey = (String, String);

/// One elevator aboard (`lifts`): its key in `ElevatorCars`, its base (world metres, the floor it
/// stands on), its yaw (radians), its type, and its STOPS.
pub struct Lift {
    pub key: LiftKey,
    pub base: Vec3,
    pub yaw: f32,
    pub ty: &'static StructureType,
    /// Metres over the base, bottom up, the first 0: the storey floors its shaft spans, the
    /// base and every storey floor of its body over it up to the shaft's top (the type's height,
    /// `StructureType::size.1`). The shaft's own top is not a stop unless a storey floor is there
    /// (docs/design/homestead-upper-floor.md section 5). In a body of one storey an elevator has
    /// the one stop and stays where it is: there is nowhere for it to go.
    pub stops: Vec<f32>,
}

impl Lift {
    /// True when the floor point (x, z) is within `margin` metres of the shaft's footprint (0:
    /// in it), in the shaft's own frame, so a turned shaft reaches as far each way.
    pub fn reaches(&self, x: f32, z: f32, margin: f32) -> bool {
        let (lx, lz) = structure::world_to_local_xz((self.base.x, self.base.y, self.base.z), self.yaw, x, z);
        lx.abs() <= self.ty.size.0 * 0.5 + margin && lz.abs() <= self.ty.size.2 * 0.5 + margin
    }
}

/// The stops of an elevator standing at body-local height `base_y` in `body`, with a shaft
/// `shaft` metres tall (`Lift::stops`).
fn lift_stops(body: &HomeStructure, base_y: f32, shaft: f32) -> Vec<f32> {
    let mut stops = vec![0.0];
    for storey in body.storeys() {
        let up = storey.floor - base_y;
        if up > STOREY_EPS && up <= shaft + STOREY_EPS {
            stops.push(up);
        }
    }
    stops
}

/// Every elevator of the ship, with its stops (not a home put away: nobody is there).
pub fn lifts(ship: &ShipStructure) -> Vec<Lift> {
    let mut out = Vec::new();
    for (zi, zone) in ship.zones.iter().enumerate() {
        if ship.is_away_home(zi) {
            continue;
        }
        let o = zone.origin_vec();
        for ps in &zone.body.structures {
            let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
            if ty.kind != StructureKind::Elevator {
                continue;
            }
            out.push(Lift {
                key: (zone.id.clone(), ps.id.clone()),
                base: Vec3::new(ps.pos.0, ps.pos.1, ps.pos.2) + o,
                yaw: ps.rot_deg.to_radians(),
                ty,
                stops: lift_stops(&zone.body, ps.pos.1, ty.size.1),
            });
        }
    }
    out
}
