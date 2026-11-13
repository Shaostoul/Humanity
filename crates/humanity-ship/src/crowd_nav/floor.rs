//! Where the floor is, from the ship's layout as data (crowd navigation rung 1;
//! docs/design/crowd-navigation.md, section 3.1): every zone's ground floor, every corridor's deck
//! and every slab, on the storey whose floor is at a given height.

use super::mesh::FloorRect;
use crate::ship_structure::ShipStructure;

/// How near a floor's height must be to the feet to be the floor they stand on, metres: half the
/// highest step (`wall_collision::STEP_UP_M`), so a step's top is not taken for the floor.
const SAME_FLOOR_M: f32 = 0.3;

/// The floors of `ship` at the height `feet_y` (world metres): each zone whose deck is there (its
/// whole box, the ground storey), each corridor whose deck is there (its tube), and each slab whose
/// walking surface is there (a storey upstairs). A hole in a slab (a stairwell) is no rectangle,
/// so it is not floor.
pub fn floor_rects(ship: &ShipStructure, feet_y: f32) -> Vec<FloorRect> {
    let mut out = Vec::new();
    for z in &ship.zones {
        let (ox, oy, oz) = z.origin;
        if (oy - feet_y).abs() <= SAME_FLOOR_M {
            out.push(FloorRect { min: (ox, oz), max: (ox + z.body.width, oz + z.body.depth) });
        }
        for s in &z.body.floors {
            if (oy + s.top - feet_y).abs() <= SAME_FLOOR_M {
                let (x0, z0, x1, z1) = s.rect();
                out.push(FloorRect { min: (ox + x0, oz + z0), max: (ox + x1, oz + z1) });
            }
        }
    }
    for c in &ship.corridors {
        let Ok(g) = ship.corridor_geometry(c) else { continue };
        if (g.floor_y - feet_y).abs() > SAME_FLOOR_M {
            continue;
        }
        let half = g.width * 0.5;
        let r = match g.axis {
            crate::ship_structure::CorridorAxis::X => FloorRect { min: (g.start, g.lat - half), max: (g.end, g.lat + half) },
            crate::ship_structure::CorridorAxis::Z => FloorRect { min: (g.lat - half, g.start), max: (g.lat + half, g.end) },
        };
        out.push(r);
    }
    out
}
