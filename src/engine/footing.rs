//! Where the floor is under a body aboard: the room it stands in, raised to the highest stair
//! tread, ramp, deck, platform, floor slab, built piece or elevator car it can step onto (v0.584,
//! v0.589, v0.590, ship homes increment 7).
//!
//! Moved out of the main loop on 2026-10-20 (BUG-207) so the aboard walk can ask it more than
//! once a frame. lib.rs asks it once before the walk moves the body, as it always did, and the
//! walk asks again between the shorter steps a long frame is cut into
//! (`CameraController::update_camera_with_floor`, `renderer::camera::WALK_SUBSTEP_M`), so a
//! slow frame walks over every stair tread a fast one does.
//!
//! STOREYS (ship homes increment 7, docs/design/ship-homes-increment-7-plan.md section 3, lane M).
//! A home can have more than one storey, each upper one standing on floor slabs
//! (`HomeStructure::floors`), so the floor is found BY HEIGHT: the highest floor at or below the
//! feet plus a step up, never just the first room over the point. A slab's top is a floor the way
//! a deck's is; a room upstairs has no floor of its own, only its slabs, so a stairwell or a shaft
//! inside an upstairs room's box is a hole you fall through, not air you walk on. The same file
//! keeps what moving between storeys needs besides the floor: the ceiling a jump stops under
//! (`Footing::ceiling_over`) and the slab edges that keep a head risen beside an opening from
//! going under a slab (`slab_edges`), the ladder a body is at (`ladder_at`), the elevator's stops,
//! rides and calls (`elevator_tick`, `pause_elevators`), and the two gates that keep a walk-in
//! drop shut: an elevator's landing door while its car is elsewhere (`elevator_gates`) and a
//! ladder hatch's gate (`ladder_gates`).

use std::collections::HashMap;

use glam::Vec3;

use crate::gui::RoomBounds;
use crate::renderer::camera::{Underfoot, HEAD_ABOVE_EYE_M};
use crate::ship::home_structure::{HomeStructure, STOREY_EPS};
use crate::ship::ship_structure::ShipStructure;
use crate::ship::structure::{self, StructureKind, StructureType};
use crate::ship::wall_collision::{self, WallSegment, EYE_HEIGHT_M};

/// The highest a body steps up in one go, metres (v0.584): onto a stair tread or a platform,
/// never up the side of a tall box (you use the stairs). Measured from the floor the body rests
/// on (`CameraController::step_up_from`), so a jump cannot carry you onto a tall box either.
pub const STEP_UP_M: f32 = 0.6;

/// How fast an elevator car travels, metres a second: v0.590's half of its 3 m travel a second.
pub const ELEVATOR_SPEED_M_S: f32 = 1.5;

/// Within this of an elevator stop's floor (metres), a body stands at that landing: on the car
/// when it is over the car (it rides), else at the landing door (it calls the car). v0.590's
/// riding margin.
pub const LANDING_M: f32 = 0.45;

/// How far from an elevator's shaft a body at a landing calls its car, metres across the floor
/// past the shaft's edge: within reach of the landing door's call button.
pub const CALL_REACH_M: f32 = 1.0;

/// A car within this of a stop (metres) is AT that landing, and its landing door opens.
pub const CAR_AT_STOP_M: f32 = 0.02;

/// How tall a gate stands (an elevator's landing door, a ladder hatch's gate), metres over the
/// floor it guards: over the eye of anyone standing there, so it stops the whole knee-to-eye band
/// (`WallSegment::stops_body`).
pub const GATE_HEIGHT_M: f32 = 2.2;

/// How much wider than its ladder a hatch's gap is on each side, metres (ship homes increment 7
/// review, 2026-10-21): the guards at a ladder's top leave the ladder's width plus this each side
/// (0.8 m over the 0.6 m ladder), and the gate across the top spans the whole gap
/// (`ladder_gates`). A body is 0.6 m across (`wall_collision::PLAYER_RADIUS`) and a balustrade's
/// end reaches 0.03 m into the gap each side, so a gap of the ladder's own 0.6 m held a climber
/// wherever the wall sweep settled (at 90 frames a second or more, or at 60 walking slowly);
/// 0.8 m leaves 0.14 m to spare. The homestead's guards G4 and G5 end at the gate's ends
/// (data/homes/homestead.ron, `the_homesteads_hatch_gate_spans_the_gap_between_its_guards`).
pub const HATCH_SIDE_CLEAR_M: f32 = 0.1;

/// How far over a floor slab's underside a head must be before the slab's edge stops the body
/// (`slab_edges`), metres: a body under a slab is held with its head AT the underside
/// (`CameraController::keep_head_under_ceiling`), and the slab it is under must not push it.
const SLAB_EDGE_HEAD_M: f32 = 0.02;

/// A ladder is the one a body is at only while its feet are no more than this under the ladder's
/// foot or over its top, metres (`ladder_at`): a ladder on another storey at the same x and z is
/// not, and hanging at the top (0.2 m over it) still is.
const LADDER_SPAN_MARGIN_M: f32 = 0.5;

/// Each elevator's runtime state, by (zone index, structure index): (the car's height over the
/// elevator's base, metres; the height it is heading for, metres, one of its stops
/// (`Lift::stops`); whether someone rides it). `EngineState::elevator_state`. Since ship homes
/// increment 7 the heights are metres, not v0.590's share of one 3 m travel, because an
/// elevator's stops are now the storeys its shaft spans.
pub type ElevatorCars = HashMap<(usize, usize), (f32, f32, bool)>;

/// Everything the floor under a body aboard is read from, gathered once a frame (lib.rs):
/// the rooms (`GuiState::room_bounds`), the ship (its zones' floor slabs and placed structures),
/// the elevator cars, and the finished pieces built aboard, each a world box
/// (`engine::build_place::built_piece_boxes`).
#[derive(Clone, Copy)]
pub struct Footing<'a> {
    pub rooms: &'a [RoomBounds],
    pub ship: Option<&'a ShipStructure>,
    pub cars: &'a ElevatorCars,
    pub built: &'a [(Vec3, Vec3)],
}

impl<'a> Footing<'a> {
    /// The floor under a body whose EYE is at `at` (world metres), which last rested on a floor at
    /// `step_from` (`CameraController::step_up_from`; its feet, for a body just placed somewhere,
    /// `reseat`). None when there is nothing to stand on at `at`: the walk then keeps the floor it
    /// has. Found by height, in two parts:
    ///
    /// 1. THE ROOM'S FLOOR. Of the rooms over the point, the highest floor no higher than the feet
    ///    plus [`STEP_UP_M`] (a body below every room floor there stands on the lowest, as before
    ///    storeys). Only the rooms on a body's ground: a room upstairs (its floor is a slab top of
    ///    the zone it is in) has a box that takes in the stairwell or the shaft in its middle, and
    ///    it is floored only where its slabs are, which part 2 counts.
    /// 2. RAISED to the highest surface there no more than [`STEP_UP_M`] above `step_from`: a floor
    ///    slab's top, a stair tread, ramp, deck or platform (`structure::walk_surface`), an
    ///    elevator's moving car, or the top of a finished built piece aboard (the aboard twin of
    ///    `planet_build::built_floor_radius`). Descending is always allowed: a lower surface simply
    ///    is the floor. With no room over the point (a doorway's cells belong to no room) the
    ///    highest such surface is the floor.
    pub fn floor_under(&self, at: Vec3, step_from: f32) -> Option<f32> {
        let feet = at.y - EYE_HEIGHT_M;
        let mut highest: Option<f32> = None;
        let mut lowest: Option<f32> = None;
        for r in self.rooms.iter().filter(|r| at.x >= r.min.x && at.x <= r.max.x && at.z >= r.min.z && at.z <= r.max.z) {
            if self.is_upper_room(r) {
                continue;
            }
            let f = r.min.y;
            if f <= feet + STEP_UP_M && highest.is_none_or(|h| f > h) {
                highest = Some(f);
            }
            if lowest.is_none_or(|l| f < l) {
                lowest = Some(f);
            }
        }
        let mut floor = highest.or(lowest);
        let reach = step_from + STEP_UP_M;
        let mut raise = |top: f32| {
            if top <= reach && floor.is_none_or(|f| top > f) {
                floor = Some(top);
            }
        };
        if let Some(ship) = self.ship {
            for (zi, zone) in ship.zones.iter().enumerate() {
                if ship.is_away_home(zi) {
                    continue; // a home put away is walked on nowhere
                }
                let o = zone.origin;
                for slab in &zone.body.floors {
                    if slab.contains(at.x - o.0, at.z - o.2) {
                        raise(o.1 + slab.top);
                    }
                }
                for (i, ps) in zone.body.structures.iter().enumerate() {
                    let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
                    let wpos = (ps.pos.0 + o.0, ps.pos.1 + o.1, ps.pos.2 + o.2);
                    let yaw = ps.rot_deg.to_radians();
                    let surf = if ty.kind == StructureKind::Elevator {
                        structure::in_footprint(ty, wpos, yaw, at.x, at.z).then(|| wpos.1 + self.cars.get(&(zi, i)).map_or(0.0, |e| e.0))
                    } else {
                        structure::walk_surface(ty, wpos, yaw, at.x, at.z)
                    };
                    if let Some(top) = surf {
                        raise(top);
                    }
                }
            }
        }
        for (lo, hi) in self.built {
            if at.x >= lo.x && at.x <= hi.x && at.z >= lo.z && at.z <= hi.z {
                raise(hi.y);
            }
        }
        floor
    }

    /// True when `r` is a room UPSTAIRS in a body of the ship: its floor is the top of a slab of
    /// the zone it stands in, above that zone's own floor. Its box is the bounding box of the
    /// cells it was flooded over (`HomeStructure::detect_rooms`), so it takes in any hole in its
    /// middle (the stairwell inside the upper hall), and only its slabs floor it.
    fn is_upper_room(&self, r: &RoomBounds) -> bool {
        let Some(ship) = self.ship else { return false };
        let (cx, cz) = ((r.min.x + r.max.x) * 0.5, (r.min.z + r.max.z) * 0.5);
        ship.zones.iter().any(|z| {
            let (ox, oy, oz) = z.origin;
            let b = &z.body;
            cx >= ox
                && cx <= ox + b.width
                && cz >= oz
                && cz <= oz + b.depth
                && r.min.y > oy + STOREY_EPS
                && r.min.y < oy + b.height.max(1.0)
                && b.floors.iter().any(|s| (oy + s.top - r.min.y).abs() < STOREY_EPS)
        })
    }

    /// The floor under a body just PLACED with its eye at `at` (a welcome, a correction, Respawn,
    /// a teleporter, the build editor's close): measured from its feet where it was put, not from
    /// the floor it stood on before, so a placement onto an upper floor stands there and one onto
    /// the ground floor under a slab stands on the ground floor.
    pub fn reseat(&self, at: Vec3) -> Option<f32> {
        self.floor_under(at, at.y - EYE_HEIGHT_M)
    }

    /// The underside of the lowest floor slab over a body whose eye is at `at` (world metres), if
    /// there is one: the ceiling a jump stops under (`CameraController::set_ceiling`). A slab over
    /// the point counts only while the eye is under its underside: a body already beside it with
    /// its head through its level (climbing past its edge) is not under it, and the slab's edge
    /// keeps such a body from moving under it (`slab_edges`, in the sweep's gates).
    pub fn ceiling_over(&self, at: Vec3) -> Option<f32> {
        let ship = self.ship?;
        let mut best: Option<f32> = None;
        for (zi, zone) in ship.zones.iter().enumerate() {
            if ship.is_away_home(zi) {
                continue;
            }
            let o = zone.origin;
            for slab in &zone.body.floors {
                let under = o.1 + slab.underside();
                if under > at.y && slab.contains(at.x - o.0, at.z - o.2) && best.is_none_or(|b| under < b) {
                    best = Some(under);
                }
            }
        }
        best
    }

    /// The floor and the ceiling a walk that began the frame at `frame_start` has reached part way
    /// through it at `at` (BUG-207), for a body clear of the walls when the frame began (last
    /// frame's sweep put it there).
    ///
    /// The walls are swept once the whole frame has been walked (lib.rs, `wall_collision::resolve`
    /// from where the frame began, after the walk), so a long frame's substep can put `at` part way
    /// into or through a wall. The floor is read where that same sweep holds the body instead, so a
    /// long frame walking into a wall does not step onto a deck on its far side. `walls` are the
    /// home's walls (`EngineState::wall_colliders`) and `gates` the shut landing doors, hatch
    /// gates and slab edges (`shut_gates`), so a substep does not read the floor down a shaft its
    /// gate keeps the body out of. The closed doors and built pieces the sweep adds are
    /// gathered after the walk and are not counted, and the most a missed one can do is lift the
    /// body by one step up (`STEP_UP_M`, 0.6 m) for part of a frame, a drop that lands at 3.4 m/s,
    /// far below the slowest landing that hurts (data/medical/falls.ron).
    pub fn along(&self, walls: &[WallSegment], gates: &[WallSegment], frame_start: Vec3, at: Vec3, step_from: f32) -> Underfoot {
        let held = if walls.is_empty() && gates.is_empty() {
            at
        } else {
            wall_collision::resolve(frame_start, at, wall_collision::PLAYER_RADIUS, walls, gates)
        };
        Underfoot { floor: self.floor_under(held, step_from), ceiling: self.ceiling_over(held) }
    }
}

/// One elevator aboard (`lifts`): its key in `ElevatorCars`, its base (world metres, the floor it
/// stands on), its yaw (radians), its type, and its STOPS.
pub struct Lift {
    pub key: (usize, usize),
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
    fn reaches(&self, x: f32, z: f32, margin: f32) -> bool {
        let (lx, lz) = to_local(self.base, self.yaw, x, z);
        lx.abs() <= self.ty.size.0 * 0.5 + margin && lz.abs() <= self.ty.size.2 * 0.5 + margin
    }
}

/// The floor point (x, z) in a piece's own frame (placed at `pos`, turned `yaw` radians): the
/// inverse of the turn `structure::structure_mesh` draws it with.
fn to_local(pos: Vec3, yaw: f32, x: f32, z: f32) -> (f32, f32) {
    let (s, c) = yaw.sin_cos();
    let (dx, dz) = (x - pos.x, z - pos.z);
    (dx * c - dz * s, dx * s + dz * c)
}

/// The world floor point of a piece's local (lx, lz), placed at `pos` and turned `yaw` radians.
fn to_world(pos: Vec3, yaw: f32, lx: f32, lz: f32) -> (f32, f32) {
    let (s, c) = yaw.sin_cos();
    (pos.x + lx * c + lz * s, pos.z - lx * s + lz * c)
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
        for (i, ps) in zone.body.structures.iter().enumerate() {
            let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
            if ty.kind != StructureKind::Elevator {
                continue;
            }
            out.push(Lift {
                key: (zi, i),
                base: Vec3::new(ps.pos.0, ps.pos.1, ps.pos.2) + o,
                yaw: ps.rot_deg.to_radians(),
                ty,
                stops: lift_stops(&zone.body, ps.pos.1, ty.size.1),
            });
        }
    }
    out
}

/// One frame of every elevator aboard (lib.rs, before the footing): each car runs toward the stop
/// it is heading for at [`ELEVATOR_SPEED_M_S`], and is sent somewhere by the one body there is.
/// `feet`: where the body's feet are (world metres). `press`: the elevator's keys this frame
/// (`CameraController::take_lift_press`): +1 for Space, -1 for Shift, 0 for neither.
///
/// - RIDING (standing over the car within [`LANDING_M`] of its floor): Space sends the car up a
///   stop and Shift down a stop, from where it is heading if it is already going that way (so two
///   presses go two stops), else from where it is. The game stops a rider's Space from jumping
///   (the return value, `CameraController::set_on_lift`).
/// - AT A LANDING (within [`LANDING_M`] of a stop's floor and [`CALL_REACH_M`] of the shaft) with
///   the car elsewhere: calls the car to that landing, from any landing, not only the bottom.
///
/// Returns whether the body rides a car that has somewhere to go (more than one stop). Cars of
/// elevators no longer aboard are dropped.
pub fn elevator_tick(ship: &ShipStructure, cars: &mut ElevatorCars, feet: Vec3, press: i8, dt: f32) -> bool {
    let lifts = lifts(ship);
    cars.retain(|k, _| lifts.iter().any(|l| l.key == *k));
    let mut rides = false;
    for lift in &lifts {
        let e = cars.entry(lift.key).or_insert((0.0, 0.0, false));
        let (y, mut target, _) = *e;
        let inside = structure::in_footprint(lift.ty, (lift.base.x, lift.base.y, lift.base.z), lift.yaw, feet.x, feet.z);
        let riding = inside && (feet.y - (lift.base.y + y)).abs() < LANDING_M;
        if riding {
            if press > 0 {
                let from = if target > y { target } else { y };
                if let Some(&s) = lift.stops.iter().find(|&&s| s > from + CAR_AT_STOP_M) {
                    target = s;
                }
            } else if press < 0 {
                let from = if target < y { target } else { y };
                if let Some(&s) = lift.stops.iter().rev().find(|&&s| s < from - CAR_AT_STOP_M) {
                    target = s;
                }
            }
        } else if lift.reaches(feet.x, feet.z, CALL_REACH_M) {
            if let Some(&s) = lift.stops.iter().find(|&&s| (feet.y - (lift.base.y + s)).abs() < LANDING_M) {
                if (y - s).abs() > CAR_AT_STOP_M {
                    target = s;
                }
            }
        }
        let step = ELEVATOR_SPEED_M_S * dt.max(0.0);
        *e = (y + (target - y).clamp(-step, step), target, riding);
        // A car with nowhere to go (one stop, a home of one storey) leaves Space a jump.
        rides |= riding && lift.stops.len() > 1;
    }
    rides
}

/// PLAY HAS LEFT FIRST PERSON (lib.rs, the elevator tick's other branch: the third-person or
/// orbit camera, the build editor, or off the ship): nobody rides, and every car stays where it
/// is, at its height and heading for its stop, until first person comes back and `elevator_tick`
/// runs again. The other cameras never move the body, so a rider who changes camera at the top
/// is still standing on the car when they come back. lib.rs used to clear every car's state
/// here, which put each car back at the bottom: back in first person the rider at the top fell
/// the storey (`leaving_first_person_on_the_elevator_keeps_the_car_where_it_is`, ship homes
/// increment 7 review). A car whose elevator is no longer aboard (the build editor took it away)
/// is dropped by the next `elevator_tick`.
pub fn pause_elevators(cars: &mut ElevatorCars) {
    for car in cars.values_mut() {
        car.2 = false;
    }
}

/// The four sides of a footprint `w` x `d` turned `yaw` about `pos`, as gate segments standing from
/// world height `y0` for [`GATE_HEIGHT_M`].
fn footprint_sides(pos: Vec3, yaw: f32, w: f32, d: f32, y0: f32) -> [WallSegment; 4] {
    let (hw, hd) = (w * 0.5, d * 0.5);
    let c = [to_world(pos, yaw, -hw, -hd), to_world(pos, yaw, hw, -hd), to_world(pos, yaw, hw, hd), to_world(pos, yaw, -hw, hd)];
    std::array::from_fn(|k| WallSegment { a: c[k], b: c[(k + 1) % 4], half_thickness: 0.05, y0, y1: y0 + GATE_HEIGHT_M, kind: wall_collision::SegmentKind::Wall })
}

/// THE LANDING DOORS (plan decision 9): at every stop of every elevator where its car is not
/// (further than [`CAR_AT_STOP_M`]), the shaft is shut at that landing's height, round all four
/// sides, so nobody walks into a shaft whose car is on another floor (a 3 m drop) and a rider is
/// held in the car between floors. With the car at a landing its door there is open. For the
/// sweep (lib.rs) and the walk's substeps (`Footing::along`).
pub fn elevator_gates(ship: &ShipStructure, cars: &ElevatorCars) -> Vec<WallSegment> {
    let mut out = Vec::new();
    for lift in lifts(ship) {
        let y = cars.get(&lift.key).map_or(0.0, |e| e.0);
        for &s in lift.stops.iter().filter(|&&s| (y - s).abs() > CAR_AT_STOP_M) {
            out.extend(footprint_sides(lift.base, lift.yaw, lift.ty.size.0, lift.ty.size.2, lift.base.y + s));
        }
    }
    out
}

/// Whether the landing door at the world point `at` may open (plan decision 9, for the door
/// animation, `systems::door_anim`, lane G's): Some(true) when `at` (a door panel's bottom middle)
/// is at an elevator's landing, on a side of its shaft (within 0.3 m of the footprint's edge and
/// within [`LANDING_M`] of a stop's floor) and the car is at that stop; Some(false) there with the
/// car elsewhere; None for a door at no landing. The collision is shut by `elevator_gates`
/// whatever the door does; this is so the door is not seen to open onto an empty shaft.
pub fn lift_landing_open(ship: &ShipStructure, cars: &ElevatorCars, at: Vec3) -> Option<bool> {
    for lift in lifts(ship) {
        if !lift.reaches(at.x, at.z, 0.3) {
            continue;
        }
        let Some(&s) = lift.stops.iter().find(|&&s| (at.y - (lift.base.y + s)).abs() < LANDING_M) else { continue };
        let y = cars.get(&lift.key).map_or(0.0, |e| e.0);
        return Some((y - s).abs() <= CAR_AT_STOP_M);
    }
    None
}

/// THE LADDER HATCH GATES (plan decision 9, docs/design/homestead-upper-floor.md section 5): a
/// ladder whose top is a floor (a slab of its body stands at that height) climbs through a hatch,
/// and the gap a climber steps off through is shut by a gate across the ladder's top, its width
/// long, standing on that floor. The sweep (lib.rs) leaves it out for a body ON the ladder
/// (`CameraController::on_ladder`), so:
///
/// - UP: climb with Space; a climber is on the ladder, the gate lets it through, and walking
///   forward off the top steps onto the floor, where the gate shuts behind it.
/// - DOWN: stand at the gate and press Space: the body steps onto the ladder's top rungs (0.2 m
///   up, `CameraController`'s climb), is on the ladder, and the gate lets it through; walk out
///   over the hatch, holding on, and hold Shift to climb down.
///
/// Walking into the hatch from the floor, which used to be a 3 m drop, is a gate that stops you.
/// The gate spans the whole gap the hatch's guards leave: the ladder's width and
/// [`HATCH_SIDE_CLEAR_M`] each side.
pub fn ladder_gates(ship: &ShipStructure) -> Vec<WallSegment> {
    let mut out = Vec::new();
    for (zi, zone) in ship.zones.iter().enumerate() {
        if ship.is_away_home(zi) {
            continue;
        }
        let o = zone.origin_vec();
        for ps in &zone.body.structures {
            let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
            if ty.kind != StructureKind::Ladder {
                continue;
            }
            let top = ps.pos.1 + ty.size.1;
            if !zone.body.floors.iter().any(|s| (s.top - top).abs() < STOREY_EPS) {
                continue;
            }
            let pos = Vec3::new(ps.pos.0, ps.pos.1, ps.pos.2) + o;
            let yaw = ps.rot_deg.to_radians();
            let (hw, hd) = (ty.size.0 * 0.5 + HATCH_SIDE_CLEAR_M, ty.size.2 * 0.5);
            out.push(WallSegment {
                a: to_world(pos, yaw, -hw, 0.0),
                b: to_world(pos, yaw, hw, 0.0),
                half_thickness: hd.max(0.05),
                y0: o.y + top,
                y1: o.y + top + GATE_HEIGHT_M,
                kind: wall_collision::SegmentKind::Wall,
            });
        }
    }
    out
}

/// Every gate shut this frame, for the sweep and the walk's substeps (lib.rs): the elevators'
/// landing doors whose car is elsewhere (`elevator_gates`), the ladder hatches' gates
/// (`ladder_gates`) unless the body is `on_ladder` (`CameraController::on_ladder`), which they let
/// through, and the floor slabs' edges (`slab_edges`), which stop only a body whose head has
/// risen past a slab beside it. Nothing without a ship.
pub fn shut_gates(ship: Option<&ShipStructure>, cars: &ElevatorCars, on_ladder: bool) -> Vec<WallSegment> {
    let Some(ship) = ship else { return Vec::new() };
    let mut gates = elevator_gates(ship, cars);
    if !on_ladder {
        gates.extend(ladder_gates(ship));
    }
    gates.extend(slab_edges(ship));
    gates
}

/// THE FLOOR SLABS' EDGES (ship homes increment 7 review, 2026-10-21): every stretch of a slab's
/// edge that no other slab of its body at the same height continues past (an opening's rim: the
/// stairwell, a shaft, a ladder hatch, the greenhouse; the body's own sides), as a segment that
/// stops a body whose head is over the slab's underside while its feet are more than a step
/// ([`STEP_UP_M`]) under its top. The ceiling a jump stops under counts a slab only while the
/// eye is under it (`Footing::ceiling_over`), so without its edge a body whose head had risen
/// beside an opening (a jump on the stair's lower steps, a climb part way up a ladder) could move
/// under the slab with its head up through the floor overhead. Within a step of the top the body
/// steps up onto the slab instead (the stair's top tread, a ladder's top), so the edge lets it by.
///
/// The segment's heights are that rule in the sweep's terms (`WallSegment::stops_body`, which
/// stops a body whose knee-to-eye band a segment stands across), not a thing standing there: from
/// the underside less the head's height over the eye (`HEAD_ABOVE_EYE_M`), so it reaches any eye
/// whose head is over the underside (by more than `SLAB_EDGE_HEAD_M`), up to the top less a step
/// plus the knee (`wall_collision::KNEE_M`), so it reaches the knee of any body whose feet are
/// more than a step under the top. Each runs so its left side faces away from the slab: the sweep
/// pushes a body exactly on the line out of the slab, never into it. No thickness: a body is held
/// with its edge on the slab's.
///
/// Not covered: a body in the middle of a jump measures its step up from the floor it jumped
/// from (`CameraController::step_up_from`), so a jump whose feet rise to within a step of a slab's
/// top beside it is let by here and is not stood on the slab by `Footing::floor_under`. In the
/// homestead that takes a jump from the stair's seventh step or higher, beside a stairwell side
/// the upper hall's north wall and guard G1 already close.
pub fn slab_edges(ship: &ShipStructure) -> Vec<WallSegment> {
    const EPS: f32 = 1e-3;
    let mut out = Vec::new();
    for (zi, zone) in ship.zones.iter().enumerate() {
        if ship.is_away_home(zi) {
            continue;
        }
        let o = zone.origin_vec();
        let floors = &zone.body.floors;
        for (si, s) in floors.iter().enumerate() {
            let (x0, z0, x1, z1) = s.rect();
            if x1 - x0 < EPS || z1 - z0 < EPS {
                continue;
            }
            let (y0, y1) = (o.y + s.underside() - HEAD_ABOVE_EYE_M + SLAB_EDGE_HEAD_M, o.y + s.top - STEP_UP_M + wall_collision::KNEE_M);
            // Each side: along x (`along_x`) or along z, at the fixed coordinate `at`, from `lo` to
            // `hi` along it, and which way is out of the slab (`out_sign`, -1 toward the lower
            // coordinate).
            for (along_x, at, lo, hi, out_sign) in [(true, z0, x0, x1, -1.0f32), (true, z1, x0, x1, 1.0), (false, x0, z0, z1, -1.0), (false, x1, z0, z1, 1.0)] {
                // The stretches of this side another slab of the same height carries on past.
                let mut covered: Vec<(f32, f32)> = floors
                    .iter()
                    .enumerate()
                    .filter(|(j, n)| *j != si && (n.top - s.top).abs() < STOREY_EPS)
                    .filter_map(|(_, n)| {
                        let (nx0, nz0, nx1, nz1) = n.rect();
                        let (across, along) = if along_x { ((nz0, nz1), (nx0, nx1)) } else { ((nx0, nx1), (nz0, nz1)) };
                        let past = if out_sign < 0.0 { across.0 < at - EPS && across.1 > at - EPS } else { across.1 > at + EPS && across.0 < at + EPS };
                        (past && along.1 > lo && along.0 < hi).then(|| (along.0.max(lo), along.1.min(hi)))
                    })
                    .collect();
                covered.sort_by(|a, b| a.0.total_cmp(&b.0));
                let mut open_from = lo;
                let mut stretches = Vec::new();
                for (c0, c1) in covered {
                    if c0 > open_from + EPS {
                        stretches.push((open_from, c0));
                    }
                    open_from = open_from.max(c1);
                }
                if hi > open_from + EPS {
                    stretches.push((open_from, hi));
                }
                for (u0, u1) in stretches {
                    // Left of a to b is out of the slab: a side at the low coordinate runs
                    // toward lower `along` for an x side and higher for a z side.
                    let (from, to) = if (out_sign < 0.0) == along_x { (u1, u0) } else { (u0, u1) };
                    let point = |u: f32| if along_x { (o.x + u, o.z + at) } else { (o.x + at, o.z + u) };
                    out.push(WallSegment { a: point(from), b: point(to), half_thickness: 0.0, y0, y1, kind: wall_collision::SegmentKind::Wall });
                }
            }
        }
    }
    out
}

/// The ladder a body whose eye is at `at` is at, as its span (its foot's and its top's world
/// heights), for the climb (`CameraController::set_climb_zone`): the nearest whose footprint is
/// within reach across the floor (half its footprint plus 0.9 m, the v0.589 reach: a ladder
/// mounted flush against a wall gets the player pushed radius plus half a thickness off it by the
/// sweep, and a wider zone keeps them on it) AND whose span holds the feet, within
/// `LADDER_SPAN_MARGIN_M` (ship homes increment 7): a ladder on another storey at the same x and z
/// is not the one you are at. A placed piece of a type the game does not know is passed over (it
/// used to end the whole search). Not a home put away.
pub fn ladder_at(ship: &ShipStructure, at: Vec3) -> Option<(f32, f32)> {
    let feet = at.y - EYE_HEIGHT_M;
    let mut best: Option<((f32, f32), f32)> = None;
    for (zi, zone) in ship.zones.iter().enumerate() {
        if ship.is_away_home(zi) {
            continue;
        }
        let o = zone.origin;
        for ps in &zone.body.structures {
            let Some(ty) = structure::structure_type(&ps.type_id) else { continue };
            if ty.kind != StructureKind::Ladder {
                continue;
            }
            let (base, top) = (ps.pos.1 + o.1, ps.pos.1 + o.1 + ty.size.1);
            if feet < base - LADDER_SPAN_MARGIN_M || feet > top + LADDER_SPAN_MARGIN_M {
                continue;
            }
            let (dx, dz) = (at.x - (ps.pos.0 + o.0), at.z - (ps.pos.2 + o.2));
            let d2 = dx * dx + dz * dz;
            let reach = ty.size.0.max(ty.size.2) * 0.5 + 0.9;
            if d2 <= reach * reach && best.is_none_or(|(_, b)| d2 < b) {
                best = Some(((base, top), d2));
            }
        }
    }
    best.map(|(span, _)| span)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::bindings::GameAction as A;
    use crate::renderer::camera::{Camera, CameraController, SPRINT_FACTOR, STEP_DOWN_M, WALK_SPEED_MPS, WALK_SUBSTEP_M};
    use crate::ship::ship_structure::ShipZone;
    use crate::systems::falls::drop_height_m;

    /// The homestead's gravity (data/game.csv `gravity_m_s2`).
    const G: f32 = 9.81;
    /// How many starting points, spread over one frame's travel, each slow walk is tried from:
    /// whether a frame's stride lands on a tread depends on where the walk began.
    const PHASES: usize = 12;

    /// A 20 x 20 m room with one piece of the homestead's "stairs" (data/blueprints/
    /// structure_types.ron: 3.0 m high, its steps up its run) centred at z = 6, climbing +Z from
    /// its foot to its top, with nothing beyond the top: an edge to walk off.
    struct Room {
        rooms: Vec<RoomBounds>,
        ship: ShipStructure,
        cars: ElevatorCars,
        built: Vec<(Vec3, Vec3)>,
    }

    impl Room {
        fn footing(&self) -> Footing<'_> {
            Footing { rooms: &self.rooms, ship: Some(&self.ship), cars: &self.cars, built: &self.built }
        }
    }

    fn room_with_stairs() -> Room {
        let zone: ShipZone = ron::from_str(
            r#"(id: "home", body: (width: 20.0, depth: 20.0, height: 6.0,
                structures: [(id: "stairs-1", type_id: "stairs", pos: (10.0, 0.0, 6.0))]))"#,
        )
        .expect("the test zone parses");
        let room = RoomBounds {
            id: "room".into(),
            min: Vec3::ZERO,
            max: Vec3::new(20.0, 6.0, 20.0),
            display_name: String::new(),
            purpose: String::new(),
            actions: Vec::new(),
            action_pages: Vec::new(),
            access: String::new(),
        };
        Room { rooms: vec![room], ship: ShipStructure { zones: vec![zone], ..Default::default() }, cars: ElevatorCars::new(), built: Vec::new() }
    }

    /// The stairs' tread heights, bottom to top: its height over its steps each.
    fn tread_heights() -> Vec<f32> {
        let ty = structure::structure_type("stairs").expect("the stairs type");
        (1..=ty.steps).map(|k| k as f32 * ty.size.1 / ty.steps as f32).collect()
    }

    /// One frame as lib.rs runs it: the floor (and the ceiling) under the body before it moves,
    /// then the walk, which asks for them again between a long frame's substeps (`Footing::along`;
    /// this room has no walls). `rested` collects every floor the walk was given to stand on.
    fn frame(ctl: &mut CameraController, cam: &mut Camera, dt: f32, room: &Room, rested: &mut Vec<f32>) {
        let start = cam.position;
        let footing = room.footing();
        if let Some(f) = footing.floor_under(start, ctl.step_up_from(start.y)) {
            ctl.set_ground_floor(f);
        }
        ctl.set_ceiling(footing.ceiling_over(start));
        rested.push(ctl.ground_floor());
        let mut floor_at = |at: Vec3, step_from: f32| {
            let u = footing.along(&[], &[], start, at, step_from);
            rested.extend(u.floor);
            u
        };
        ctl.update_camera_with_floor(cam, dt, Some(&mut floor_at));
    }

    /// One frame of the walk as it was before BUG-207's fix: the floor asked once, before the
    /// move, and the frame walked in one step (`update_camera`, which never cuts a frame).
    fn frame_in_one_step(ctl: &mut CameraController, cam: &mut Camera, dt: f32, room: &Room) {
        let p = cam.position;
        if let Some(f) = room.footing().floor_under(p, ctl.step_up_from(p.y)) {
            ctl.set_ground_floor(f);
        }
        ctl.update_camera(cam, dt);
    }

    fn how(sprint: bool) -> &'static str {
        if sprint { "sprinting" } else { "walking" }
    }

    /// A walker on the room's floor at `z`, facing up the stairs (+Z), W held (and Shift for a
    /// sprint).
    fn walker(z: f32, sprint: bool) -> (CameraController, Camera) {
        let mut ctl = CameraController::new(WALK_SPEED_MPS, 1.0);
        ctl.set_interior_gravity(G);
        ctl.set_ground_floor(0.0);
        let mut cam = Camera::new();
        cam.position = Vec3::new(10.0, ctl.eye_height(), z);
        cam.yaw = std::f32::consts::PI;
        ctl.apply_action(A::MoveForward, true);
        if sprint {
            ctl.apply_action(A::Sprint, true);
        }
        (ctl, cam)
    }

    /// The metres one frame of `dt` carries a walker (or a sprinter).
    fn stride(dt: f32, sprint: bool) -> f32 {
        WALK_SPEED_MPS * if sprint { SPRINT_FACTOR } else { 1.0 } * dt
    }

    /// Walk from `z` up the stairs and on off the top at `dt` a frame until the landing lib.rs
    /// would take: the height it fell, read back from the landing's speed (h = v^2 / 2g), and
    /// the speed.
    fn off_the_top(dt: f32, sprint: bool, z: f32) -> (f32, f32) {
        let room = room_with_stairs();
        let (mut ctl, mut cam) = walker(z, sprint);
        let mut rested = Vec::new();
        for _ in 0..(10.0 / dt) as usize {
            frame(&mut ctl, &mut cam, dt, &room, &mut rested);
            if let Some(v) = ctl.take_landing() {
                return (drop_height_m(v, G), v);
            }
        }
        panic!("walking off the top of the stairs at {dt} s frames from z = {z} never landed");
    }

    /// Walk from `z` up the stairs and past their top at `dt` a frame: every floor the walk was
    /// given to stand on. Walking up without jumping the body stands on each of them (a higher
    /// floor lifts it the frame it is given), so these are the treads it stood on.
    fn up_the_stairs(dt: f32, sprint: bool, z: f32) -> Vec<f32> {
        let room = room_with_stairs();
        let (mut ctl, mut cam) = walker(z, sprint);
        let mut rested = Vec::new();
        while cam.position.z < 9.0 {
            frame(&mut ctl, &mut cam, dt, &room, &mut rested);
        }
        rested
    }

    /// A SLOW FRAME FALLS OFF THE TOP OF THE STAIRS FROM THE SAME HEIGHT A FAST ONE DOES
    /// (BUG-207). Walking up the 3.0 m stairs and on off the top lands from 3.0 m, so at 7.67
    /// m/s, at 4 ms frames, and from every starting point at 54 ms frames walking and sprinting
    /// and at 100 ms frames (the longest the main loop runs, `dt` is clamped there).
    ///
    /// Seen red 2026-10-20 on the walk before the fix (the floor asked once a frame, before the
    /// move, and the frame walked in one step): "at 4 ms frames the walk fell 3.000 m (7.67 m/s);
    /// 10 slow walks did not: [\"sprinting at 54 ms from z = 2.214: fell 2.786 m (7.39 m/s)\",
    /// ... (5 of the 12 sprints at 54 ms), \"walking at 100 ms from z = 2.000: fell 2.786 m
    /// (7.39 m/s)\", ... (5 of the 12 walks at 100 ms)]". Walking at a steady 54 ms never fell
    /// short: its 0.27 m stride cannot pass over the top tread, which is 0.307 m deep with the
    /// 0.05 m the footprint reaches past its end (`structure::walk_surface`), so the game's 2.79 m
    /// came from a frame of 62 ms or more, or a sprint.
    #[test]
    fn walking_off_the_top_of_the_stairs_falls_the_same_height_at_any_frame_rate() {
        let (reference, v_ref) = off_the_top(0.004, false, 2.0);
        assert!(
            (reference - 3.0).abs() < 0.01,
            "at 4 ms frames the walk fell {reference:.3} m ({v_ref:.2} m/s) off the 3.0 m stairs"
        );
        let mut wrong = Vec::new();
        for (dt, sprint) in [(0.054, false), (0.054, true), (0.1, false)] {
            for k in 0..PHASES {
                let z = 2.0 + stride(dt, sprint) * k as f32 / PHASES as f32;
                let (h, v) = off_the_top(dt, sprint, z);
                if (h - reference).abs() >= 0.01 {
                    wrong.push(format!("{} at {:.0} ms from z = {z:.3}: fell {h:.3} m ({v:.2} m/s)", how(sprint), dt * 1000.0));
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "at 4 ms frames the walk fell {reference:.3} m ({v_ref:.2} m/s); {} slow walks did not: {wrong:#?}",
            wrong.len()
        );
    }

    /// A SLOW FRAME WALKS UP EVERY STAIR TREAD (BUG-207). Walking up the 3.0 m stairs at 54 ms
    /// frames (walking and sprinting) and at 100 ms frames, from every starting point, the body
    /// stands on each of its treads (14 when this was written, 16 since ship homes increment 7),
    /// the top one included, as it does at a high frame rate.
    ///
    /// Seen red 2026-10-20 on the walk before the fix: "43 slow walks up the stairs skipped a
    /// tread: [\"walking at 54 ms from z = 2.000: never stood on [\\\"0.857\\\"]\", ... (7 of the
    /// 12 walks at 54 ms, each missing one tread), \"sprinting at 54 ms from z = 2.000: never stood
    /// on [\\\"0.214\\\", \\\"0.643\\\", ... \\\"2.786\\\"]\", ... \"sprinting at 100 ms from z =
    /// 2.000: never stood on [all 14, \\\"0.214\\\" to \\\"3.000\\\"]\", ...]". Sprinting at 100
    /// ms every stride rose more than the 0.6 m step up (`STEP_UP_M`), so the body could not
    /// climb the stairs at all and walked on through them at floor level.
    #[test]
    fn walking_up_the_stairs_at_a_low_frame_rate_stands_on_every_tread() {
        let treads = tread_heights();
        // 14 steps when this was written, 16 from ship homes increment 7's stair rules: the count
        // is the data's, and this only guards against a stair of no treads passing by checking none.
        assert!(treads.len() >= 10, "the homestead's stairs have {} steps", treads.len());
        let mut wrong = Vec::new();
        for (dt, sprint) in [(0.054, false), (0.054, true), (0.1, false), (0.1, true)] {
            for k in 0..PHASES {
                let z = 2.0 + stride(dt, sprint) * k as f32 / PHASES as f32;
                let rested = up_the_stairs(dt, sprint, z);
                let missed: Vec<String> = treads
                    .iter()
                    .filter(|t| !rested.iter().any(|r| (r - **t).abs() < 1e-4))
                    .map(|t| format!("{t:.3}"))
                    .collect();
                if !missed.is_empty() {
                    wrong.push(format!("{} at {:.0} ms from z = {z:.3}: never stood on {missed:?}", how(sprint), dt * 1000.0));
                }
            }
        }
        assert!(wrong.is_empty(), "{} slow walks up the stairs skipped a tread: {wrong:#?}", wrong.len());
    }

    /// AT 60 FRAMES A SECOND THE WALK IS EXACTLY WHAT IT WAS (BUG-207). Walking and sprinting up
    /// the stairs and on off the top, the walk that may cut a frame into substeps puts the body in
    /// the very same place every frame, to the bit, as the walk that never does
    /// (`frame_in_one_step`, the walk as it was before the fix), and hands over the same landing.
    /// At 60 a second a walk moves 0.083 m a frame and a sprint 0.158 m, both under
    /// `WALK_SUBSTEP_M`, so each frame is still one step.
    ///
    /// Seen red 2026-10-20 with WALK_SUBSTEP_M at 0.1 m, which cuts a sprint's frame in two:
    /// "sprinting at 60 frames a second, frame 13: the substepped walk left the body somewhere
    /// else; left: Vec3(10.0, 1.7, 4.2166667), right: Vec3(10.0, 1.7, 4.216666)".
    #[test]
    fn at_sixty_frames_a_second_the_walk_is_unchanged() {
        let dt = 1.0 / 60.0;
        let room = room_with_stairs();
        for sprint in [false, true] {
            let (mut cut, mut cam_cut) = walker(2.0, sprint);
            let (mut whole, mut cam_whole) = walker(2.0, sprint);
            let mut landings = 0;
            for n in 0..240 {
                frame(&mut cut, &mut cam_cut, dt, &room, &mut Vec::new());
                frame_in_one_step(&mut whole, &mut cam_whole, dt, &room);
                assert_eq!(
                    cam_cut.position,
                    cam_whole.position,
                    "{} at 60 frames a second, frame {n}: the substepped walk left the body somewhere else",
                    how(sprint)
                );
                let (a, b) = (cut.take_landing(), whole.take_landing());
                assert_eq!(a, b, "{} at 60 frames a second, frame {n}: a different landing", how(sprint));
                landings += u32::from(a.is_some());
            }
            assert_eq!(landings, 1, "{}: the walk went off the top of the stairs and landed once", how(sprint));
        }
    }

    /// EVERY STAIR TREAD IS DEEPER THAN A WALK SUBSTEP (BUG-207). A step no longer than a tread
    /// cannot pass over one, which is what lets a slow frame stand on every tread; a stair type
    /// with shallower treads would bring the skipped treads back, so this holds the data
    /// (data/blueprints/structure_types.ron, built into the game) to `WALK_SUBSTEP_M`. It counts
    /// the stair types it checked, so it cannot pass by checking none.
    ///
    /// Seen red 2026-10-20 with WALK_SUBSTEP_M at 0.3 m: "the stairs type's treads are 0.257 m
    /// deep, shallower than a 0.3 m walk substep". The same run shows why it matters: at 0.3 m a
    /// 54 ms walk (0.27 m a frame) is one step again, and
    /// `walking_up_the_stairs_at_a_low_frame_rate_stands_on_every_tread` failed with "7 slow
    /// walks up the stairs skipped a tread".
    #[test]
    fn every_stair_tread_is_deeper_than_a_walk_substep() {
        let mut checked = 0;
        for ty in structure::structure_types().iter().filter(|t| t.shape == structure::MeshShape::Steps) {
            let tread = ty.size.2 / ty.steps.max(1) as f32;
            assert!(
                tread > WALK_SUBSTEP_M,
                "the {} type's treads are {tread:.3} m deep, shallower than a {WALK_SUBSTEP_M} m walk substep",
                ty.id
            );
            checked += 1;
        }
        assert!(checked > 0, "no stair type was checked");
    }

    /// EVERY STAIR RISER IS A STEP DOWN (BUG-208). The walk follows a floor that drops no deeper
    /// than `STEP_DOWN_M` and treats a deeper drop as an edge, so a stair type with taller risers
    /// would make walking down it a run of falls again, and a ramp steep enough to drop more than
    /// that over one walk substep would too. Holds data/blueprints/structure_types.ron to it, and
    /// counts what it checked.
    ///
    /// Seen red 2026-10-20 with STEP_DOWN_M at 0.2 m: "walking down the stairs type drops 0.214 m
    /// a step, deeper than the walk's 0.2 m step down" (and in the same run walking down the
    /// stairs fell from the top again, each riser an edge).
    #[test]
    fn every_stair_riser_is_a_step_down() {
        let mut checked = 0;
        for ty in structure::structure_types() {
            let (_, h, d) = ty.size;
            let drop = match ty.shape {
                structure::MeshShape::Steps => h / ty.steps.max(1) as f32,
                structure::MeshShape::Ramp => h / d * WALK_SUBSTEP_M,
                _ => continue,
            };
            assert!(
                drop < STEP_DOWN_M,
                "walking down the {} type drops {drop:.3} m a step, deeper than the walk's {STEP_DOWN_M} m step down",
                ty.id
            );
            checked += 1;
        }
        assert!(checked >= 2, "only {checked} stair and ramp types were checked");
    }

    /// A walker standing on `floor` at `z`, facing down the stairs (-Z), W held (and Shift).
    fn walker_down(z: f32, floor: f32, sprint: bool) -> (CameraController, Camera) {
        let (mut ctl, mut cam) = walker(z, sprint);
        ctl.set_ground_floor(floor);
        cam.position.y = floor + ctl.eye_height();
        cam.yaw = 0.0;
        (ctl, cam)
    }

    /// SPACE MID-FALL DOES NOTHING (BUG-208). Walking on off the top of the 3.0 m stairs, an edge
    /// with nothing beyond it, the body is in the air: Space pressed a few frames into the fall
    /// and held must not launch a jump. The body keeps falling every frame and lands from 3.0 m
    /// (7.67 m/s), at 60 frames a second and at 54 ms frames.
    ///
    /// Seen red 2026-10-20 on the walk before the fix (6cc5ad8c9): "at 17 ms frames, Space held
    /// from 3 frames into the fall lifted the body 0.081 m on frame 0 of the fall". Walking off an
    /// edge never cleared `is_grounded`, so the first Space of the fall launched a full jump.
    #[test]
    fn space_pressed_in_mid_fall_does_not_jump() {
        for dt in [1.0 / 60.0, 0.054] {
            let room = room_with_stairs();
            let (mut ctl, mut cam) = walker(2.0, false);
            let mut rested = Vec::new();
            // Up the stairs until the floor has fallen away under the body past the top.
            let mut n = 0;
            while !(cam.position.z > 7.85 && ctl.ground_floor() == 0.0) {
                frame(&mut ctl, &mut cam, dt, &room, &mut rested);
                n += 1;
                assert!(n < 1000, "never walked off the top of the stairs");
            }
            for _ in 0..3 {
                frame(&mut ctl, &mut cam, dt, &room, &mut rested);
            }
            ctl.apply_action(A::Jump, true);
            let mut landing = None;
            for k in 0..(5.0 / dt) as usize {
                let before = cam.position.y;
                frame(&mut ctl, &mut cam, dt, &room, &mut rested);
                landing = landing.or(ctl.take_landing());
                if landing.is_some() {
                    break;
                }
                assert!(
                    cam.position.y < before,
                    "at {:.0} ms frames, Space held from 3 frames into the fall lifted the body {:.3} m on frame {k} of the fall",
                    dt * 1000.0,
                    cam.position.y - before
                );
            }
            let v = landing.expect("the fall lands");
            let h = drop_height_m(v, G);
            assert!((h - 3.0).abs() < 0.01, "at {:.0} ms frames the fall landed from {h:.3} m ({v:.2} m/s), not 3.0 m", dt * 1000.0);
        }
    }

    /// WALKING DOWN THE STAIRS IS NOT A FALL (BUG-208). Down the 3.0 m stairs from the top
    /// tread, walking and sprinting, at 60 frames a second and at 54 and 100 ms frames, the body
    /// steps down each tread: its feet are on its floor at the end of every frame, and nothing
    /// lands hard enough to count (`falls::LANDING_M_S`).
    ///
    /// Seen red 2026-10-20 on the walk before the fix (6cc5ad8c9): "walking down the stairs left
    /// the floor: [\"walking at 17 ms: feet up to 0.972 m above the floor, landing
    /// Some(7.672027)\", \"sprinting at 17 ms: feet up to 2.248 m above the floor, landing None\",
    /// \"walking at 54 ms: feet up to 0.963 m above the floor, landing Some(7.672027)\", ...
    /// \"walking at 100 ms: ... landing Some(7.672027)\", ...]". Nothing followed the floor down:
    /// at 5 m/s a body leaving the top tread flew clear of the whole flight and landed at the
    /// bottom from 3.0 m (a sprint flew on past the stairs' foot before landing).
    #[test]
    fn walking_down_the_stairs_is_not_a_fall() {
        let room = room_with_stairs();
        let mut wrong = Vec::new();
        for dt in [1.0 / 60.0, 0.054, 0.1] {
            for sprint in [false, true] {
                let (mut ctl, mut cam) = walker_down(7.7, 3.0, sprint);
                let mut rested = Vec::new();
                let mut worst_above = 0.0_f32;
                let mut landing = None;
                while cam.position.z > 3.0 {
                    frame(&mut ctl, &mut cam, dt, &room, &mut rested);
                    worst_above = worst_above.max(cam.position.y - ctl.eye_height() - ctl.ground_floor());
                    landing = landing.or(ctl.take_landing());
                }
                if worst_above > 1e-4 || landing.is_some() {
                    wrong.push(format!(
                        "{} at {:.0} ms: feet up to {worst_above:.3} m above the floor, landing {landing:?}",
                        how(sprint),
                        dt * 1000.0
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "walking down the stairs left the floor: {wrong:#?}");
    }

    /// A LONG FRAME DOES NOT STEP ONTO A DECK BEHIND A WALL (BUG-207). The walls are swept after
    /// the whole frame's walk, so a substep's point can be part way through one; the floor is read
    /// where that sweep holds the body, on this side of the wall.
    ///
    /// Seen red 2026-10-20 with `floor_along` reading the floor at the point itself: "a substep
    /// through the wall stood the body on the deck behind it; left: Some(0.2), right: Some(0.0)".
    #[test]
    fn a_substep_reads_the_floor_where_the_walls_hold_the_body() {
        let zone: ShipZone = ron::from_str(
            r#"(id: "home", body: (width: 20.0, depth: 20.0, height: 6.0,
                structures: [(id: "deck-1", type_id: "deck", pos: (10.0, 0.0, 12.0))]))"#,
        )
        .expect("the test zone parses");
        let mut room = room_with_stairs();
        room.ship = ShipStructure { zones: vec![zone], ..Default::default() };
        // A wall across the room at z = 10, and a 0.2 m deck (4 m square, z 10 to 14) against
        // its far side. The frame began 0.6 m short of the wall; a substep's point is past it.
        let wall = WallSegment::at_every_height((0.0, 10.0), (20.0, 10.0), 0.075);
        let (start, through) = (Vec3::new(10.0, 1.7, 9.4), Vec3::new(10.0, 1.7, 10.4));
        let raw = room.footing().floor_under(through, 0.0);
        assert_eq!(raw, Some(0.2), "the deck is under the point past the wall");
        let held = room.footing().along(&[wall], &[], start, through, 0.0).floor;
        assert_eq!(held, Some(0.0), "a substep through the wall stood the body on the deck behind it");
    }
}

/// Test support for walking between storeys (ship homes increment 7, lane M): a small two-storey
/// home and one whole frame of the walk aboard as lib.rs runs it. Used by this file's
/// `storey_tests`, and by engine/move_check.rs (the teleporter) and engine/home_plot.rs (the
/// placements and the editor's close).
#[cfg(test)]
pub(crate) mod test_aboard {
    use super::*;
    use crate::renderer::camera::{Camera, CameraController, WALK_SPEED_MPS};
    use crate::ship::ship_structure::ShipZone;

    /// The homestead's gravity (data/game.csv `gravity_m_s2`).
    pub(crate) const G: f32 = 9.81;

    /// THE TEST HOME: 12 x 12 x 6 m. Its north half (z 0 to 6) lies under floor slabs at 3.0 with
    /// an upstairs zone over all of it; its south half (z 6 to 12) is open to the roof. Two holes
    /// in the slabs: elevator-1's shaft (the elevator at (10, 0, 2), its 1.8 m footprint, x 9.1 to
    /// 10.9 and z 1.1 to 2.9) and a ladder hatch (x 1.5 to 2.5, z 0.9 to 2.1), with ladder-1 at
    /// (2, 0, 2) on the hatch's south edge, guarded upstairs by 1.0 m walls (base 3.0) on three
    /// sides and on the fourth either side of a 0.8 m gap over the 0.6 m ladder (the ladder's
    /// width and `HATCH_SIDE_CLEAR_M` each side, where the gate ends). The hatch, as the homestead's
    /// (docs/design/homestead-upper-floor.md section 5), is climbed facing south and stepped off
    /// south over the ladder's top. Teleporter-1 on the ground at (4, 0, 9) is paired with
    /// teleporter-2 upstairs at (6, 3, 4). Not the homestead: another lane is writing its layout.
    pub(crate) const TWO_STOREYS: &str = r#"(id: "home", purpose: "residence", body: (width: 12.0, depth: 12.0, height: 6.0,
        floors: [
            (a: (0.0, 0.0), b: (12.0, 0.9), top: 3.0),
            (a: (0.0, 0.9), b: (1.5, 6.0), top: 3.0),
            (a: (1.5, 2.1), b: (2.5, 6.0), top: 3.0),
            (a: (2.5, 0.9), b: (9.1, 6.0), top: 3.0),
            (a: (9.1, 0.9), b: (10.9, 1.1), top: 3.0),
            (a: (9.1, 2.9), b: (10.9, 6.0), top: 3.0),
            (a: (10.9, 0.9), b: (12.0, 6.0), top: 3.0),
        ],
        zones: [(id: "upstairs", type_id: "room_hall", origin: (0.0, 3.0, 0.0), size: (12.0, 3.0, 6.0))],
        walls: [
            (a: (1.5, 0.9), b: (1.5, 2.1), base: 3.0, height: 1.0),
            (a: (2.5, 0.9), b: (2.5, 2.1), base: 3.0, height: 1.0),
            (a: (1.5, 0.9), b: (2.5, 0.9), base: 3.0, height: 1.0),
            (a: (1.5, 2.1), b: (1.6, 2.1), base: 3.0, height: 1.0),
            (a: (2.4, 2.1), b: (2.5, 2.1), base: 3.0, height: 1.0),
        ],
        structures: [
            (id: "elevator-1", type_id: "elevator", pos: (10.0, 0.0, 2.0)),
            (id: "ladder-1", type_id: "ladder", pos: (2.0, 0.0, 2.0)),
            (id: "teleporter-1", type_id: "teleporter", pos: (4.0, 0.0, 9.0), pair: Some("teleporter-2")),
            (id: "teleporter-2", type_id: "teleporter", pos: (6.0, 3.0, 4.0), pair: Some("teleporter-1")),
        ]))"#;

    /// A ship aboard and everything the walk stands on and walks against there.
    pub(crate) struct Aboard {
        pub ship: ShipStructure,
        pub rooms: Vec<RoomBounds>,
        pub cars: ElevatorCars,
        pub built: Vec<(Vec3, Vec3)>,
        pub walls: Vec<WallSegment>,
    }

    impl Aboard {
        /// The test home (`TWO_STOREYS`) as the only zone of a ship.
        pub(crate) fn two_storeys() -> Self {
            let zone: ShipZone = ron::from_str(TWO_STOREYS).expect("the test home parses");
            Self::of(ShipStructure { zones: vec![zone], ..Default::default() })
        }

        /// A ship's rooms, as the game makes them at load (`ShipStructure::generate_meshes`'
        /// rooms, home_meshes.rs), and its walls (`wall_collision::ship_wall_segments`).
        pub(crate) fn of(ship: ShipStructure) -> Self {
            let rooms = ship
                .generate_meshes()
                .room_info
                .iter()
                .map(|r| RoomBounds {
                    id: r.id.clone(),
                    min: r.center - r.dimensions * 0.5,
                    max: r.center + r.dimensions * 0.5,
                    display_name: String::new(),
                    purpose: String::new(),
                    actions: Vec::new(),
                    action_pages: Vec::new(),
                    access: String::new(),
                })
                .collect();
            let walls = wall_collision::ship_wall_segments(&ship);
            Aboard { ship, rooms, cars: ElevatorCars::new(), built: Vec::new(), walls }
        }

        /// The homestead the exe ships (data/homes/shipped/homestead.ron, through
        /// `HomeDesign::built_in`) as the only zone of a ship, at the origin, so its own metres
        /// are world metres: its upper floor, stair, elevator and ladder hatch as built.
        pub(crate) fn homestead() -> Self {
            let design = crate::ship::ship_structure::HomeDesign::built_in("homestead").expect("the homestead is built in");
            let zone = ShipZone { id: "home".into(), label: String::new(), purpose: "residence".into(), origin: (0.0, 0.0, 0.0), body: design.body };
            Self::of(ShipStructure { zones: vec![zone], ..Default::default() })
        }

        pub(crate) fn footing(&self) -> Footing<'_> {
            Footing { rooms: &self.rooms, ship: Some(&self.ship), cars: &self.cars, built: &self.built }
        }

        /// elevator-1's car's height over its base, metres.
        pub(crate) fn car(&self) -> f32 {
            self.cars.get(&(0, 0)).map_or(0.0, |e| e.0)
        }
    }

    /// A body standing with its eye at `eye`, on the floor `EYE_HEIGHT_M` under it, facing
    /// north (-Z), nothing held.
    pub(crate) fn body_at(eye: Vec3) -> (CameraController, Camera) {
        let mut ctl = CameraController::new(WALK_SPEED_MPS, 1.0);
        ctl.set_interior_gravity(G);
        ctl.set_ground_floor(eye.y - ctl.eye_height());
        let mut cam = Camera::new();
        cam.position = eye;
        (ctl, cam)
    }

    /// Facings (`Camera::yaw`): the camera at yaw a looks along (sin a, 0, -cos a).
    pub(crate) const NORTH: f32 = 0.0;
    pub(crate) const SOUTH: f32 = std::f32::consts::PI;
    pub(crate) const EAST: f32 = std::f32::consts::FRAC_PI_2;
    pub(crate) const WEST: f32 = -std::f32::consts::FRAC_PI_2;

    /// Stand the body with its eye at `at` the way every placement does (home_plot.rs
    /// `put_player_at` and `reseat_floor`): the camera there, and the floor re-seated from the
    /// feet there (`Footing::reseat`).
    pub(crate) fn place(ctl: &mut CameraController, cam: &mut Camera, w: &Aboard, at: Vec3) {
        cam.position = at;
        if let Some(f) = w.footing().reseat(at) {
            ctl.set_ground_floor(f);
        }
    }

    /// ONE WHOLE FRAME of the walk aboard, in lib.rs's order: the elevators (with this frame's
    /// Space or Shift), the floor and the ceiling under the body (measured from the feet after a
    /// placement), the ladder, the walk with its substeps (`Footing::along`, the gates counted),
    /// then the sweep against the walls and the gates shut now.
    pub(crate) fn live_frame(ctl: &mut CameraController, cam: &mut Camera, dt: f32, w: &mut Aboard) {
        let eye_h = ctl.eye_height();
        let p = cam.position;
        let press = ctl.take_lift_press();
        let riding = elevator_tick(&w.ship, &mut w.cars, Vec3::new(p.x, p.y - eye_h, p.z), press, dt);
        ctl.set_on_lift(riding);
        let footing = w.footing();
        let step_from = if ctl.placed_since_walk(p.y) { p.y - eye_h } else { ctl.step_up_from(p.y) };
        if let Some(f) = footing.floor_under(p, step_from) {
            ctl.set_ground_floor(f);
        }
        ctl.set_ceiling(footing.ceiling_over(p));
        ctl.set_climb_zone(ladder_at(&w.ship, p));
        let gates = shut_gates(Some(&w.ship), &w.cars, ctl.on_ladder(p.y));
        let walls = &w.walls;
        let mut floor_at = |at: Vec3, step_from: f32| footing.along(walls, &gates, p, at, step_from);
        ctl.update_camera_with_floor(cam, dt, Some(&mut floor_at));
        let gates = shut_gates(Some(&w.ship), &w.cars, ctl.on_ladder(cam.position.y));
        cam.position = wall_collision::resolve(p, cam.position, wall_collision::PLAYER_RADIUS, walls, &gates);
    }
}

/// Walking between storeys (ship homes increment 7, docs/design/ship-homes-increment-7-plan.md
/// section 3, lane M), in the test home (`test_aboard::TWO_STOREYS`).
#[cfg(test)]
mod storey_tests {
    use super::test_aboard::*;
    use super::*;
    use crate::input::bindings::GameAction as A;
    use crate::renderer::camera::{Camera, CameraController, HEAD_ABOVE_EYE_M, WALK_SPEED_MPS};
    use crate::systems::falls::drop_height_m;

    const EYE: f32 = EYE_HEIGHT_M;
    const DT: f32 = 1.0 / 60.0;

    /// The body's feet.
    fn feet(ctl: &CameraController, cam: &Camera) -> f32 {
        cam.position.y - ctl.eye_height()
    }

    /// Run `n` frames, failing on any landing that could hurt (a drop of more than half a metre;
    /// a 0.2 m step off a ladder's top lands at 2 m/s, under it): the landings it did hand over,
    /// as heights fallen.
    fn frames(ctl: &mut CameraController, cam: &mut Camera, w: &mut Aboard, n: usize, what: &str) -> Vec<f32> {
        let mut drops = Vec::new();
        for k in 0..n {
            live_frame(ctl, cam, DT, w);
            if let Some(v) = ctl.take_landing() {
                let h = drop_height_m(v, G);
                assert!(h < 0.5, "{what}: frame {k} landed from {h:.3} m ({v:.2} m/s) at {:?}", cam.position);
                drops.push(h);
            }
        }
        drops
    }

    /// THE EYE HEIGHT THE SWEEP AND THE FOOTING ASSUME IS THE BODY'S (`wall_collision::EYE_HEIGHT_M`
    /// against `CameraController::eye_height`): both read the feet off the eye with it.
    ///
    /// Seen red 2026-10-20 with `EYE_HEIGHT_M` at 1.75: "assertion `left == right` failed" (left:
    /// 1.7, right: 1.75).
    #[test]
    fn the_sweeps_eye_height_is_the_bodys() {
        assert_eq!(CameraController::new(1.0, 1.0).eye_height(), EYE_HEIGHT_M);
    }

    /// A BODY UNDER A SLAB STANDS ON THE GROUND FLOOR AND ONE OVER IT STANDS ON THE SLAB. At
    /// (6, 4) under slab 4 of the test home: the floor for a body on the ground is the ground's
    /// (0), for one upstairs the slab's top (3.0). Over the ladder hatch, which the upstairs
    /// room's box takes in, there is no floor upstairs: the ground (a body walking out over it
    /// falls). Over the shaft, the car's floor (at the bottom, 0). In the half open to the roof,
    /// the ground at any height. Then walking: 2 m along on each floor, each body stays on its
    /// own (the one downstairs is not lifted onto the slab over it, the one upstairs does not
    /// sink through it).
    ///
    /// Seen red 2026-10-20 with the slabs' tops not counted in `floor_under` (`raise(o.1 +
    /// slab.top)` taken out): "a body over the slab" (left: Some(0.0), right: Some(3.0)).
    #[test]
    fn a_body_stands_on_the_floor_of_its_own_storey() {
        let mut w = Aboard::two_storeys();
        let f = w.footing();
        let at = |x: f32, y: f32, z: f32| Vec3::new(x, y + EYE, z);
        assert_eq!(f.floor_under(at(6.0, 0.0, 4.0), 0.0), Some(0.0), "a body under the slab");
        assert_eq!(f.floor_under(at(6.0, 3.0, 4.0), 3.0), Some(3.0), "a body over the slab");
        assert_eq!(f.floor_under(at(2.0, 3.0, 1.5), 3.0), Some(0.0), "a body over the ladder hatch");
        assert_eq!(f.floor_under(at(10.0, 3.0, 2.0), 3.0), Some(0.0), "a body over the shaft, the car at the bottom");
        assert_eq!(f.floor_under(at(6.0, 3.0, 9.0), 3.0), Some(0.0), "a body 3 m up in the half open to the roof");
        for storey in [0.0, 3.0] {
            let (mut ctl, mut cam) = body_at(at(6.0, storey, 4.0));
            cam.yaw = EAST;
            ctl.apply_action(A::MoveForward, true);
            frames(&mut ctl, &mut cam, &mut w, 24, "walking along a floor");
            assert!(cam.position.x > 7.9, "the walk on the floor at {storey} did not go along it: {:?}", cam.position);
            assert!((feet(&ctl, &cam) - storey).abs() < 1e-4, "walking on the floor at {storey} the body stood at {}", feet(&ctl, &cam));
        }
    }

    /// A JUMP UNDER A SLAB DOES NOT PASS IT. Under slab 4 (its underside 2.75 m) a jump would lift
    /// the eye 1.27 m, to 2.97, the head to about 3.1: through the floor. It stops with the eye
    /// `HEAD_ABOVE_EYE_M` under the underside and comes back down to the ground floor, at 60
    /// frames a second and at 100 ms frames (moving, so the substeps ask for the ceiling too). In
    /// the half open to the roof the same jump rises its full height.
    ///
    /// Seen red 2026-10-20 with `keep_head_under_ceiling` doing nothing: "at 17 ms frames a jump
    /// under the slab put the eye at 2.933 m, the head through its underside at 2.75".
    #[test]
    fn a_jump_under_a_slab_does_not_pass_it() {
        for (dt, moving) in [(DT, false), (0.1, true)] {
            let mut w = Aboard::two_storeys();
            let (mut ctl, mut cam) = body_at(Vec3::new(4.0, EYE, 4.0));
            cam.yaw = EAST;
            ctl.apply_action(A::MoveForward, moving);
            ctl.apply_action(A::Jump, true);
            let mut top = cam.position.y;
            for _ in 0..(2.0 / dt) as usize {
                live_frame(&mut ctl, &mut cam, dt, &mut w);
                top = top.max(cam.position.y);
                ctl.apply_action(A::Jump, false);
            }
            let under = 3.0 - 0.25;
            assert!(top <= under - HEAD_ABOVE_EYE_M + 1e-4, "at {:.0} ms frames a jump under the slab put the eye at {top:.3} m, the head through its underside at {under}", dt * 1000.0);
            // Sampled once a frame: at 100 ms frames the substeps after the bump start the fall
            // before the frame ends, so the sample is a centimetre or so under the clamp.
            assert!(top > under - HEAD_ABOVE_EYE_M - 0.05, "at {:.0} ms frames the jump stopped short at {top:.3}", dt * 1000.0);
            assert!(feet(&ctl, &cam).abs() < 1e-4, "after the jump the body stood at {} m, not on the ground floor", feet(&ctl, &cam));
        }
        let mut w = Aboard::two_storeys();
        let (mut ctl, mut cam) = body_at(Vec3::new(6.0, EYE, 9.0));
        ctl.apply_action(A::Jump, true);
        let mut top = cam.position.y;
        for _ in 0..120 {
            live_frame(&mut ctl, &mut cam, DT, &mut w);
            top = top.max(cam.position.y);
            ctl.apply_action(A::Jump, false);
        }
        assert!(top > EYE + 1.2, "in the open half a jump rose only to {top:.3}");
    }

    /// THE ELEVATOR RIDES 0 TO 3 TO 0, AND COMES WHEN CALLED FROM THE UPPER LANDING (plan
    /// decision 5). Its stops are the test home's storeys its 3 m shaft spans, 0 and 3. On the
    /// car, Space sends it up a storey (and does not jump: the eye never rises over the car's
    /// floor plus the eye); at the top the rider walks off onto the upper floor, back on, and Shift
    /// sends it down. Then from the upper landing, beside the shaft with the car at the bottom, the
    /// car is called up, and the body walks onto it there. Nothing on the way is a fall.
    ///
    /// Seen red 2026-10-20 with v0.590's ride brought back into `elevator_tick` (stepping on sends
    /// the car to its other end, Space and Shift do nothing, and only a body in the shaft at its
    /// base calls it): "the car left before anyone pressed anything" (left: 0.7499998, right:
    /// 0.0): stepping on had sent it up.
    #[test]
    fn the_elevator_rides_a_storey_up_and_down_and_comes_when_called_from_upstairs() {
        let mut w = Aboard::two_storeys();
        assert_eq!(lifts(&w.ship)[0].stops, vec![0.0, 3.0], "the elevator's stops");
        let (mut ctl, mut cam) = body_at(Vec3::new(10.0, EYE, 2.0));
        frames(&mut ctl, &mut cam, &mut w, 30, "standing on the car");
        assert_eq!(w.car(), 0.0, "the car left before anyone pressed anything");
        // Up.
        ctl.apply_action(A::Jump, true);
        let mut highest = 0.0f32;
        for _ in 0..240 {
            frames(&mut ctl, &mut cam, &mut w, 1, "riding up");
            highest = highest.max(cam.position.y - EYE - w.car());
            ctl.apply_action(A::Jump, false);
        }
        assert!((w.car() - 3.0).abs() < 1e-4, "Space on the car left it at {}", w.car());
        assert!((feet(&ctl, &cam) - 3.0).abs() < 1e-3, "riding up left the feet at {}", feet(&ctl, &cam));
        assert!(highest < 0.01, "Space on the car jumped: the eye rose {highest:.3} m over its floor");
        // Off onto the upper floor and back.
        cam.yaw = WEST;
        ctl.apply_action(A::MoveForward, true);
        frames(&mut ctl, &mut cam, &mut w, 24, "walking off the car upstairs");
        ctl.apply_action(A::MoveForward, false);
        assert!(cam.position.x < 8.1 && (feet(&ctl, &cam) - 3.0).abs() < 1e-4, "off the car upstairs the body is at {:?}", cam.position);
        cam.yaw = EAST;
        ctl.apply_action(A::MoveForward, true);
        frames(&mut ctl, &mut cam, &mut w, 24, "walking back onto the car");
        ctl.apply_action(A::MoveForward, false);
        assert!(cam.position.x > 9.6, "back on the car: {:?}", cam.position);
        // Down.
        ctl.apply_action(A::Sprint, true);
        frames(&mut ctl, &mut cam, &mut w, 1, "riding down");
        ctl.apply_action(A::Sprint, false);
        frames(&mut ctl, &mut cam, &mut w, 240, "riding down");
        assert!(w.car().abs() < 1e-4, "Shift on the car left it at {}", w.car());
        assert!(feet(&ctl, &cam).abs() < 1e-3, "riding down left the feet at {}", feet(&ctl, &cam));
        // Called from the upper landing.
        let (mut ctl, mut cam) = body_at(Vec3::new(8.4, 3.0 + EYE, 2.0));
        frames(&mut ctl, &mut cam, &mut w, 180, "waiting at the upper landing");
        assert!((w.car() - 3.0).abs() < 1e-4, "a body waiting at the upper landing left the car at {}", w.car());
        cam.yaw = EAST;
        ctl.apply_action(A::MoveForward, true);
        frames(&mut ctl, &mut cam, &mut w, 20, "walking onto the called car");
        assert!(cam.position.x > 9.6 && (feet(&ctl, &cam) - 3.0).abs() < 1e-3, "onto the called car: {:?}, feet {}", cam.position, feet(&ctl, &cam));
    }

    /// A LANDING WITH THE CAR AWAY DOES NOT LET A BODY WALK INTO THE SHAFT (plan decision 9). At
    /// the upper landing with the car at the bottom, a body walking east into the shaft is held
    /// at its landing door (the gate across the shaft at that height) every frame the car is not
    /// there, and never falls; its walk calls the car, and once the car has come it walks on
    /// onto it. The same at the ground landing with the car at the top.
    ///
    /// Seen red 2026-10-20 with `elevator_gates` returning no gates: "at the landing at 3, frame
    /// 16: the body walked into the shaft to Vec3(8.916664, 4.7, 2.0) with the car at 0.000".
    #[test]
    fn a_landing_with_the_car_away_does_not_let_a_body_into_the_shaft() {
        for (storey, car_at) in [(3.0, 0.0), (0.0, 3.0)] {
            let mut w = Aboard::two_storeys();
            w.cars.insert((0, 0), (car_at, car_at, false));
            let (mut ctl, mut cam) = body_at(Vec3::new(7.5, storey + EYE, 2.0));
            cam.yaw = EAST;
            ctl.apply_action(A::MoveForward, true);
            let mut held = 0;
            for k in 0..300 {
                frames(&mut ctl, &mut cam, &mut w, 1, "walking at the landing door");
                if (w.car() - storey).abs() > CAR_AT_STOP_M {
                    held += 1;
                    assert!(
                        cam.position.x <= 9.1 - wall_collision::PLAYER_RADIUS + 0.06,
                        "at the landing at {storey}, frame {k}: the body walked into the shaft to {:?} with the car at {:.3}",
                        cam.position,
                        w.car()
                    );
                }
                assert!((feet(&ctl, &cam) - storey).abs() < 1e-3, "at the landing at {storey}, frame {k}: the feet went to {}", feet(&ctl, &cam));
            }
            assert!(held > 30, "the car came before the body reached the door ({held} frames held)");
            assert!(cam.position.x > 9.6, "at the landing at {storey}, the body never walked onto the car that came: {:?}", cam.position);
        }
        // The landing doors' hook for the door animation: a door on the shaft's west side at each
        // landing opens only with the car there; a door elsewhere is no landing door.
        let mut w = Aboard::two_storeys();
        let door = |storey: f32| Vec3::new(9.1, storey, 2.0);
        assert_eq!(lift_landing_open(&w.ship, &w.cars, door(0.0)), Some(true), "the ground landing's door, the car there");
        assert_eq!(lift_landing_open(&w.ship, &w.cars, door(3.0)), Some(false), "the upper landing's door, the car below");
        w.cars.insert((0, 0), (3.0, 3.0, false));
        assert_eq!(lift_landing_open(&w.ship, &w.cars, door(3.0)), Some(true), "the upper landing's door, the car there");
        assert_eq!(lift_landing_open(&w.ship, &w.cars, door(0.0)), Some(false), "the ground landing's door, the car above");
        assert_eq!(lift_landing_open(&w.ship, &w.cars, Vec3::new(6.0, 3.0, 4.0)), None, "a door at no landing");
    }

    /// THE LADDER HATCH IS SHUT TO A WALK AND OPEN TO A CLIMBER (plan decision 9). Upstairs, a body
    /// walking north at the hatch's gap (0.8 m in the guard over the 0.6 m ladder) is stopped at the gate
    /// across the ladder's top and does not fall 3 m. From below, a climb (Space) goes through it,
    /// and walking south off the top stands the body on the upper floor. From above, Space at the
    /// gate steps onto the ladder's top rungs, the body walks out over the hatch holding on, and
    /// Shift climbs it down to the ground. No landing on the way hurts.
    ///
    /// Seen red 2026-10-20 with `ladder_gates` returning no gates: "frame 14: the walk upstairs
    /// went through the hatch's gate to Vec3(2.0, 4.7, 2.2500012)".
    #[test]
    fn the_ladder_hatch_is_shut_to_a_walk_and_open_to_a_climber() {
        let mut w = Aboard::two_storeys();
        // Walking into it upstairs.
        let (mut ctl, mut cam) = body_at(Vec3::new(2.0, 3.0 + EYE, 3.5));
        cam.yaw = NORTH;
        ctl.apply_action(A::MoveForward, true);
        for k in 0..120 {
            frames(&mut ctl, &mut cam, &mut w, 1, "walking into the hatch");
            assert!(cam.position.z > 2.0 + wall_collision::PLAYER_RADIUS - 0.01, "frame {k}: the walk upstairs went through the hatch's gate to {:?}", cam.position);
            assert!((feet(&ctl, &cam) - 3.0).abs() < 1e-3, "frame {k}: walking at the hatch the feet went to {}", feet(&ctl, &cam));
        }
        // Up from below, and off the top onto the floor.
        let (mut ctl, mut cam) = body_at(Vec3::new(2.0, EYE, 1.5));
        ctl.apply_action(A::Jump, true);
        frames(&mut ctl, &mut cam, &mut w, 90, "climbing up");
        ctl.apply_action(A::Jump, false);
        assert!(feet(&ctl, &cam) > 3.0, "the climb up got only to {}", feet(&ctl, &cam));
        cam.yaw = SOUTH;
        ctl.apply_action(A::MoveForward, true);
        frames(&mut ctl, &mut cam, &mut w, 24, "stepping off the top");
        ctl.apply_action(A::MoveForward, false);
        frames(&mut ctl, &mut cam, &mut w, 30, "standing at the top");
        assert!(cam.position.z > 3.0 && (feet(&ctl, &cam) - 3.0).abs() < 1e-3, "off the top of the ladder the body is at {:?}, feet {}", cam.position, feet(&ctl, &cam));
        // Down from above.
        let (mut ctl, mut cam) = body_at(Vec3::new(2.0, 3.0 + EYE, 2.5));
        ctl.apply_action(A::Jump, true);
        frames(&mut ctl, &mut cam, &mut w, 6, "stepping onto the ladder's top");
        ctl.apply_action(A::Jump, false);
        cam.yaw = NORTH;
        ctl.apply_action(A::MoveForward, true);
        frames(&mut ctl, &mut cam, &mut w, 12, "out over the hatch");
        ctl.apply_action(A::MoveForward, false);
        assert!(cam.position.z < 2.0, "the body on the ladder's top did not get out over the hatch: {:?}", cam.position);
        ctl.apply_action(A::Sprint, true);
        frames(&mut ctl, &mut cam, &mut w, 120, "climbing down");
        ctl.apply_action(A::Sprint, false);
        assert!(feet(&ctl, &cam).abs() < 1e-3, "the climb down ended at {}", feet(&ctl, &cam));
    }

    /// SHIFT AT A LADDER'S TOP ON A FLOOR SPRINTS AND DOES NOT CLIMB DOWN; ONLY SHIFT ON THE LADDER
    /// DOES. Upstairs at the ladder's top (in its reach, on slab 3), Shift held for a second does
    /// not move the body down (it used to climb it down into the slab), and with W it sprints along
    /// the floor. Part way up the ladder, Shift climbs down.
    ///
    /// Seen red 2026-10-20 with Shift climbing down at any height in a ladder's reach (the
    /// `on_ladder` test taken out of `walk_step_moves`): "Shift on the floor at the ladder's top
    /// took the body down to 0" (down the ladder, through the slab, to the ground floor).
    #[test]
    fn shift_at_a_ladders_top_on_a_floor_sprints_and_only_shift_on_the_ladder_climbs_down() {
        let mut w = Aboard::two_storeys();
        let (mut ctl, mut cam) = body_at(Vec3::new(2.0, 3.0 + EYE, 2.6));
        ctl.apply_action(A::Sprint, true);
        frames(&mut ctl, &mut cam, &mut w, 60, "Shift at the ladder's top");
        assert!((feet(&ctl, &cam) - 3.0).abs() < 1e-4, "Shift on the floor at the ladder's top took the body down to {}", feet(&ctl, &cam));
        cam.yaw = SOUTH;
        ctl.apply_action(A::MoveForward, true);
        frames(&mut ctl, &mut cam, &mut w, 10, "sprinting from the ladder's top");
        assert!(cam.position.z > 2.6 + 0.15, "Shift and W at the ladder's top did not sprint: {:?}", cam.position);
        assert!((feet(&ctl, &cam) - 3.0).abs() < 1e-4, "sprinting from the ladder's top the feet went to {}", feet(&ctl, &cam));
        // Part way up, Shift climbs down.
        let (mut ctl, mut cam) = body_at(Vec3::new(2.0, EYE, 1.5));
        ctl.apply_action(A::Jump, true);
        frames(&mut ctl, &mut cam, &mut w, 30, "climbing");
        ctl.apply_action(A::Jump, false);
        let up = feet(&ctl, &cam);
        assert!(up > 1.0, "the climb got only to {up}");
        ctl.apply_action(A::Sprint, true);
        frames(&mut ctl, &mut cam, &mut w, 10, "climbing down");
        assert!(feet(&ctl, &cam) < up - 0.3, "Shift on the ladder at {up} m left the feet at {}", feet(&ctl, &cam));
    }

    fn piece(id: &str, ty: &str, pos: (f32, f32, f32)) -> crate::ship::home_structure::PlacedStructure {
        crate::ship::home_structure::PlacedStructure { id: id.into(), type_id: ty.into(), pos, rot_deg: 0.0, pair: None }
    }

    /// THE LADDER IS CHOSEN BY HEIGHT. With a second ladder upstairs (ladder-2 on slab 4 at (6, 3,
    /// 5), its foot on the upper floor) a body on the ground floor under it is at no ladder, and a
    /// body upstairs beside it is at it; the ground's ladder-1 is the one at its foot and, upstairs,
    /// at its top.
    ///
    /// Seen red 2026-10-20 with `ladder_at` choosing by x and z alone (the span test taken out,
    /// the lib.rs closure before this lane): "a body on the ground floor under the upstairs
    /// ladder" (left: Some((3.0, 6.0)), right: None).
    #[test]
    fn the_ladder_is_chosen_by_height() {
        let mut w = Aboard::two_storeys();
        w.ship.zones[0].body.structures.push(piece("ladder-2", "ladder", (6.0, 3.0, 5.0)));
        let at = |x: f32, feet: f32, z: f32| ladder_at(&w.ship, Vec3::new(x, feet + EYE, z));
        assert_eq!(at(6.0, 0.0, 5.0), None, "a body on the ground floor under the upstairs ladder");
        assert_eq!(at(6.0, 3.0, 5.0), Some((3.0, 6.0)), "a body upstairs at the upstairs ladder");
        assert_eq!(at(2.0, 0.0, 1.5), Some((0.0, 3.0)), "a body at the ground's ladder");
        assert_eq!(at(2.0, 3.0, 2.6), Some((0.0, 3.0)), "a body upstairs at the ground ladder's top");
    }

    /// A PIECE OF A TYPE THE GAME DOES NOT KNOW DOES NOT HIDE A LADDER. The lib.rs closure this
    /// replaced asked for each piece's type with `?`, which ended the whole search at the first
    /// unknown one, so a home with one (a mod's piece, a type renamed) had no working ladder.
    ///
    /// Seen red 2026-10-20 with `ladder_at` asking with `?` again: "an unknown piece listed first
    /// hid the ladder" (left: None, right: Some((0.0, 3.0))).
    #[test]
    fn an_unknown_piece_does_not_hide_a_ladder() {
        let mut w = Aboard::two_storeys();
        w.ship.zones[0].body.structures.insert(0, piece("mystery-1", "no_such_piece", (11.0, 0.0, 11.0)));
        assert_eq!(ladder_at(&w.ship, Vec3::new(2.0, EYE, 1.5)), Some((0.0, 3.0)), "an unknown piece listed first hid the ladder");
    }

    /// A BODY ON A PIECE BUILT ABOARD STANDS ON IT, AND WALKING OFF IT IS A FALL (the aboard twin
    /// of `planet_build::built_floor_radius`). A finished 2 x 2 m box 1.0 m tall in the open half:
    /// a body standing on top stands on its top for a second (it used to drop through it to the
    /// floor), and walking east off its edge drops the metre to the floor, a landing from 1.0 m
    /// (4.4 m/s).
    ///
    /// Seen red 2026-10-20 with the built pieces' tops not counted in `floor_under`: "standing on
    /// the built piece: frame 26 landed from 1.000 m (4.43 m/s) at Vec3(6.0, 1.7, 9.0)".
    #[test]
    fn a_body_stands_on_a_piece_built_aboard_and_walking_off_it_is_a_fall() {
        let mut w = Aboard::two_storeys();
        w.built = vec![(Vec3::new(5.0, 0.0, 8.0), Vec3::new(7.0, 1.0, 10.0))];
        let (mut ctl, mut cam) = body_at(Vec3::new(6.0, 1.0 + EYE, 9.0));
        frames(&mut ctl, &mut cam, &mut w, 60, "standing on the built piece");
        assert!((feet(&ctl, &cam) - 1.0).abs() < 1e-4, "a body put on the built piece came to rest at {}", feet(&ctl, &cam));
        cam.yaw = EAST;
        ctl.apply_action(A::MoveForward, true);
        let mut landed = None;
        for _ in 0..60 {
            live_frame(&mut ctl, &mut cam, DT, &mut w);
            landed = landed.or(ctl.take_landing());
        }
        let v = landed.expect("walking off the built piece is a fall that lands");
        let h = drop_height_m(v, G);
        assert!((h - 1.0).abs() < 0.02, "walking off the 1.0 m piece fell {h:.3} m ({v:.2} m/s)");
        assert!(feet(&ctl, &cam).abs() < 1e-4, "off the piece the body stood at {}", feet(&ctl, &cam));
    }

    /// A PLACEMENT STANDS ON THE FLOOR IT PUT THE BODY ON (every placement: a welcome, a
    /// correction, Respawn, the editor's close). A body on the ground put upstairs at (6, 4) stands
    /// on the slab there, at once and a second later; a body upstairs put on the ground floor
    /// under the slab (as the homestead's spawn at its door is, under the upper hall) stands on the
    /// ground floor, and is not lifted back onto the slab over it.
    ///
    /// Seen red 2026-10-20 with no re-seat (`Footing::reseat` giving None) and the step up measured
    /// from the floor stood on before after a placement too (`placed_since_walk` false): "the
    /// placement onto the upper floor left the body resting on 0".
    #[test]
    fn a_placement_stands_on_the_floor_it_was_put_on() {
        let mut w = Aboard::two_storeys();
        let (mut ctl, mut cam) = body_at(Vec3::new(6.0, EYE, 9.0));
        frames(&mut ctl, &mut cam, &mut w, 5, "standing on the ground");
        place(&mut ctl, &mut cam, &w, Vec3::new(6.0, 3.0 + EYE, 4.0));
        assert!((ctl.ground_floor() - 3.0).abs() < 1e-4, "the placement onto the upper floor left the body resting on {}", ctl.ground_floor());
        frames(&mut ctl, &mut cam, &mut w, 60, "after a placement upstairs");
        assert!((feet(&ctl, &cam) - 3.0).abs() < 1e-4, "a second after the placement upstairs the body stood at {}", feet(&ctl, &cam));
        place(&mut ctl, &mut cam, &w, Vec3::new(6.0, EYE, 4.0));
        assert!(ctl.ground_floor().abs() < 1e-4, "the placement onto the ground floor under the slab left the body resting on {}", ctl.ground_floor());
        frames(&mut ctl, &mut cam, &mut w, 60, "after a placement under the slab");
        assert!(feet(&ctl, &cam).abs() < 1e-4, "a second after the placement under the slab the body stood at {}", feet(&ctl, &cam));
    }

    /// Run `n` frames of `dt` (`live_frame`): the hardest landing handed over on the way, as the
    /// height fallen, 0 for none.
    fn run(ctl: &mut CameraController, cam: &mut Camera, w: &mut Aboard, dt: f32, n: usize) -> f32 {
        let mut hardest = 0.0f32;
        for _ in 0..n {
            live_frame(ctl, cam, dt, w);
            if let Some(v) = ctl.take_landing() {
                hardest = hardest.max(drop_height_m(v, G));
            }
        }
        hardest
    }

    /// A body standing with its eye at `eye` that walks at `pace` metres a second: a slower pace
    /// is what a load, thirst or an injury makes of the walk (`CameraController::speed_multiplier`).
    fn pacer(eye: Vec3, pace: f32) -> (CameraController, Camera) {
        let (mut ctl, cam) = body_at(eye);
        ctl.speed_multiplier = pace / WALK_SPEED_MPS;
        (ctl, cam)
    }

    /// The frame rates the homestead's hatch is walked at, a second, and the paces, metres a second.
    const HATCH_RATES: [f32; 3] = [30.0, 60.0, 144.0];
    const HATCH_PACES: [f32; 2] = [3.0, 5.0];

    /// THE HOMESTEAD'S LADDER HATCH LETS A CLIMBER OFF ONTO THE LOBBY AND BACK ON, AT ANY FRAME
    /// RATE AND PACE (ship homes increment 7 review). ladder-1 at (33, 0, 28) climbs through the
    /// hatch (x 32.5 to 34.0, z 26.9 to 28.1) and is stepped off south over its top, between the
    /// guards G4 and G5, onto the lift lobby's floor (the slab at 3.0 from z 28.1). UP: from (33,
    /// 27.4) in the hatch, Space climbs to the top and W walks south off it, out of the ladder's
    /// reach, onto the floor (z past 29.2, the feet at 3.0). DOWN: from the lobby at (33, 28.7),
    /// Space steps onto the ladder's top rungs, W walks north out over the hatch through the same
    /// gap (z under 27.8), and Shift climbs down to the ground. At 30, 60 and 144 frames a second,
    /// at 3 and 5 m/s, with no landing harder than the 0.2 m step off the top.
    ///
    /// The gap must pass a body (0.6 m across) and the balustrade's 0.06 m at its ends however
    /// small a frame's step: a wall sweep that settles fully each frame, as it must at any frame
    /// rate, holds a body in a gap any narrower than 0.66 m.
    ///
    /// Seen red first (2026-10-21) with the guards' 0.6 m gap (G4 ending at x 32.7, G5 starting at
    /// 33.3), on the wall sweep of that day: "the hatch held a climber: [\"up at 60 a second, 3 m/s:
    /// climbed to 3.20, ended at z 27.96 with the feet at 3.20, hardest landing 0.00 m\", \"down at
    /// 60 a second, 3 m/s: out over the hatch only to z 28.22, the climb down ended with the feet
    /// at 3.00, hardest landing 0.00 m\", \"up at 144 a second, 3 m/s: ... z 27.96 ...\", \"down at
    /// 144 a second, 3 m/s: ... z 28.23 ...\", \"up at 144 a second, 5 m/s: ... z 27.96 ...\",
    /// \"down at 144 a second, 5 m/s: ... z 28.23 ...\"]". Up, the climber hung at the ladder's top;
    /// down, the walk was held on the lobby side and Shift took the feet only to the floor there.
    /// At 30 a second, and at 60 at 5 m/s, a frame's step was long enough for the unsettled sweep
    /// to carry the body through.
    #[test]
    fn the_homesteads_ladder_hatch_lets_a_climber_off_and_on_at_any_frame_rate_and_pace() {
        let mut wrong = Vec::new();
        for fps in HATCH_RATES {
            let dt = 1.0 / fps;
            let n = |s: f32| (s / dt).round() as usize;
            for pace in HATCH_PACES {
                // Up from the hatch and off the top onto the lobby's floor.
                let mut w = Aboard::homestead();
                let (mut ctl, mut cam) = pacer(Vec3::new(33.0, EYE, 27.4), pace);
                cam.yaw = SOUTH;
                ctl.apply_action(A::Jump, true);
                let mut hard = run(&mut ctl, &mut cam, &mut w, dt, n(1.5));
                ctl.apply_action(A::Jump, false);
                let climbed = feet(&ctl, &cam);
                ctl.apply_action(A::MoveForward, true);
                hard = hard.max(run(&mut ctl, &mut cam, &mut w, dt, n(1.2)));
                ctl.apply_action(A::MoveForward, false);
                hard = hard.max(run(&mut ctl, &mut cam, &mut w, dt, n(0.5)));
                let (z, f) = (cam.position.z, feet(&ctl, &cam));
                if !(climbed > 3.0 && z > 29.2 && (f - 3.0).abs() < 1e-3 && hard < 0.5) {
                    wrong.push(format!(
                        "up at {fps} a second, {pace} m/s: climbed to {climbed:.2}, ended at z {z:.2} with the feet at {f:.2}, hardest landing {hard:.2} m"
                    ));
                }
                // Down from the lobby: onto the top rungs, out over the hatch, and down.
                let mut w = Aboard::homestead();
                let (mut ctl, mut cam) = pacer(Vec3::new(33.0, 3.0 + EYE, 28.7), pace);
                cam.yaw = NORTH;
                ctl.apply_action(A::Jump, true);
                let mut hard = run(&mut ctl, &mut cam, &mut w, dt, n(0.2));
                ctl.apply_action(A::Jump, false);
                ctl.apply_action(A::MoveForward, true);
                hard = hard.max(run(&mut ctl, &mut cam, &mut w, dt, n(0.6)));
                ctl.apply_action(A::MoveForward, false);
                let over = cam.position.z;
                ctl.apply_action(A::Sprint, true);
                hard = hard.max(run(&mut ctl, &mut cam, &mut w, dt, n(1.6)));
                ctl.apply_action(A::Sprint, false);
                hard = hard.max(run(&mut ctl, &mut cam, &mut w, dt, n(0.3)));
                let f = feet(&ctl, &cam);
                if !(over < 27.8 && f.abs() < 1e-3 && hard < 0.5) {
                    wrong.push(format!(
                        "down at {fps} a second, {pace} m/s: out over the hatch only to z {over:.2}, the climb down ended with the feet at {f:.2}, hardest landing {hard:.2} m"
                    ));
                }
            }
        }
        assert!(wrong.is_empty(), "the hatch held a climber: {wrong:#?}");
    }

    /// THE HOMESTEAD'S HATCH GATE SPANS THE GAP ITS GUARDS LEAVE, AND THE GAP PASSES A BODY. The
    /// gate across ladder-1's top (`ladder_gates`) ends where the guards beside the ladder (G4 and
    /// G5, the balustrades along the hatch's south side) end, so a walker upstairs meets a guard
    /// or the gate at every point of that side; and the gap between those guards' ends is wider
    /// than a body plus a balustrade's half thickness at each end, so a climber passes it at any
    /// frame rate (`the_homesteads_ladder_hatch_lets_a_climber_off_and_on_at_any_frame_rate_and_pace`).
    ///
    /// Seen red first (2026-10-21) with the guards' 0.6 m gap and the gate the ladder's width:
    /// "the gap between the guards' ends is 0.60 m, and a body and the balustrade's ends need 0.66 m
    /// and room to spare (0.76 m)".
    #[test]
    fn the_homesteads_hatch_gate_spans_the_gap_between_its_guards() {
        let w = Aboard::homestead();
        let gates = ladder_gates(&w.ship);
        assert_eq!(gates.len(), 1, "the homestead has one ladder hatch: {gates:?}");
        let g = gates[0];
        let (ax, az) = g.a;
        let (dx, dz) = (g.b.0 - ax, g.b.1 - az);
        let len = (dx * dx + dz * dz).sqrt();
        let (ux, uz) = (dx / len, dz / len);
        // Along the gate from its `a` end, and how far off its line.
        let along = |p: (f32, f32)| (p.0 - ax) * ux + (p.1 - az) * uz;
        let off = |p: (f32, f32)| ((p.0 - ax) * uz - (p.1 - az) * ux).abs();
        let mid = len * 0.5;
        // The guards whose ends lie along the gate's side of the hatch, within 0.15 m of its line.
        let guards: Vec<_> = w.ship.zones[0]
            .body
            .walls
            .iter()
            .filter(|wl| (wl.base - 3.0).abs() < 1e-3 && wl.height < 1.5 && off(wl.a) < 0.15 && off(wl.b) < 0.15)
            .collect();
        assert!(guards.len() >= 2, "no guards either side of the ladder's top: {guards:?}");
        let ends: Vec<f32> = guards.iter().flat_map(|wl| [along(wl.a), along(wl.b)]).collect();
        let west = ends.iter().copied().filter(|&u| u < mid).fold(f32::MIN, f32::max);
        let east = ends.iter().copied().filter(|&u| u > mid).fold(f32::MAX, f32::min);
        assert!(west.abs() < 0.01 && (east - len).abs() < 0.01, "the gate runs 0 to {len:.2} m along the hatch's side, and the guards beside it end at {west:.2} and {east:.2}");
        let half_t = guards.iter().map(|wl| wl.resolved_thickness() * 0.5).fold(0.0f32, f32::max);
        let need = 2.0 * (wall_collision::PLAYER_RADIUS + half_t);
        assert!(
            east - west >= need + 0.1,
            "the gap between the guards' ends is {:.2} m, and a body and the balustrade's ends need {need:.2} m and room to spare ({:.2} m)",
            east - west,
            need + 0.1
        );
    }

    /// Whether the body reaches over a floor slab it may not be over (the homestead's, which is
    /// zone 0 at the origin): its disk over the slab's rectangle by more than a centimetre, its head
    /// more than 3 cm over the slab's underside, and its feet more than a step up (`STEP_UP_M`)
    /// under the slab's top, so it is not stepping onto it. A description, None when it does not.
    fn head_through_a_slab(w: &Aboard, ctl: &CameraController, cam: &Camera) -> Option<String> {
        let p = cam.position;
        let (feet, head) = (p.y - ctl.eye_height(), p.y + HEAD_ABOVE_EYE_M);
        w.ship.zones[0].body.floors.iter().find_map(|s| {
            let (x0, z0, x1, z1) = s.rect();
            let (dx, dz) = ((x0 - p.x).max(p.x - x1).max(0.0), (z0 - p.z).max(p.z - z1).max(0.0));
            let over = wall_collision::PLAYER_RADIUS - (dx * dx + dz * dz).sqrt();
            let centre_over = s.contains(p.x, p.z);
            (over > 0.01 && head > s.underside() + 0.03 && feet < s.top - STEP_UP_M).then(|| {
                format!(
                    "the body at ({:.2}, {:.2}) reached {over:.2} m over slab {:?} ({}), its head {:.2} m over the slab's underside ({:.2} over its top), its feet at {feet:.2}",
                    p.x,
                    p.z,
                    s.label.as_deref().unwrap_or("?"),
                    if centre_over { "its centre under the slab" } else { "beside it" },
                    head - s.underside(),
                    head - s.top
                )
            })
        })
    }

    /// What a walk did to the slabs over it, frame by frame (`see`): the first frame the body
    /// reached over a slab it may not be over (`head_through_a_slab`), and the most its head rose
    /// over the underside of a slab its centre was under, with its feet more than a step under
    /// that slab's top (how far into or up through the floor overhead the head went).
    #[derive(Default)]
    struct SlabWatch {
        first: Option<String>,
        worst: f32,
        worst_over_top: f32,
    }

    impl SlabWatch {
        fn see(&mut self, k: usize, w: &Aboard, ctl: &CameraController, cam: &Camera) {
            if self.first.is_none() {
                self.first = head_through_a_slab(w, ctl, cam).map(|why| format!("frame {k}: {why}"));
            }
            let p = cam.position;
            let (feet, head) = (p.y - ctl.eye_height(), p.y + HEAD_ABOVE_EYE_M);
            for s in w.ship.zones[0].body.floors.iter().filter(|s| s.contains(p.x, p.z) && feet < s.top - STEP_UP_M) {
                if head - s.underside() > self.worst {
                    self.worst = head - s.underside();
                    self.worst_over_top = head - s.top;
                }
            }
        }

        /// What went wrong, None for nothing.
        fn wrong(&self, what: &str) -> Option<String> {
            self.first.as_ref().map(|first| {
                format!(
                    "{what}, {first}; at worst the head was {:.2} m over the underside of a slab the body's centre was under ({:.2} m over the floor upstairs)",
                    self.worst, self.worst_over_top
                )
            })
        }
    }

    /// A HEAD RISEN BESIDE AN OPENING DOES NOT GO UNDER THE SLAB: THE SLAB'S EDGE STOPS THE BODY
    /// (ship homes increment 7 review). The ceiling a jump stops under counts a slab only while the
    /// eye is under its underside (`Footing::ceiling_over`), so a body whose head has risen above a
    /// slab's underside beside it (a jump, a climb) and then moves under it would put its head up
    /// through the floor overhead. The slab's edge stops it instead (`slab_edges`), while its feet
    /// are more than a step under the slab's top (within a step it steps up onto it). In the
    /// homestead, at 60 frames a second and 5 m/s:
    ///
    /// - from step 3 of stairs-1, just west of the stairwell's east end (x 40.5), a jump in place
    ///   and then a walk east under the upper hall's slab east of the stairwell;
    /// - from part way up ladder-1 (the feet 1.5 m up, hanging on), a walk south out of the hatch
    ///   under the lift lobby's floor;
    /// - and walking up stairs-1 from its foot onto the stair-top landing and back down, which must
    ///   not catch on the landing's edge (the step up onto it) or a stairwell side.
    ///
    /// At the end of every frame the body's disk is clear of every slab its head is above the
    /// underside of (`head_through_a_slab`).
    ///
    /// Seen red first (2026-10-21) with no slab edges in the sweep (`shut_gates` without
    /// `slab_edges`): "a head through a slab: [\"the jump on stairs-1's step 3, frame 5: the body
    /// at (40.35, 38.65) reached 0.15 m over slab \\\"Upper floor, hall east of the stairwell\\\"
    /// (beside it), its head 0.08 m over the slab's underside (-0.17 over its top), its feet at
    /// 1.01; at worst the head was 0.15 m over the underside of a slab the body's centre was under
    /// (-0.10 m over the floor upstairs)\", \"the climber walking south, frame 4: the body at
    /// (33.00, 27.82) reached 0.02 m over slab \\\"Lift lobby floor, by the hatch\\\" (beside it),
    /// its head 0.57 m over the slab's underside (0.32 over its top), its feet at 1.50; at worst the
    /// head was 0.57 m over the underside of a slab the body's centre was under (0.32 m over the
    /// floor upstairs)\"]". (On the stair, guard G2 over the flight's foot holds the body while its
    /// eye is over 3.0 m, so the head went 0.15 m into the slab rather than through it.)
    #[test]
    fn a_head_risen_beside_an_opening_does_not_go_under_the_slab() {
        let dt = DT;
        let mut wrong = Vec::new();
        // The jump on the stair's step 3 (x 40.16 to 40.44), then east.
        let mut w = Aboard::homestead();
        let step3 = 3.0 * 3.0 / 16.0;
        let (mut ctl, mut cam) = body_at(Vec3::new(40.35, step3 + EYE, 38.65));
        assert!((w.footing().floor_under(cam.position, step3).unwrap_or(-1.0) - step3).abs() < 1e-4, "the body is not on step 3 of stairs-1");
        cam.yaw = EAST;
        ctl.apply_action(A::Jump, true);
        let mut watch = SlabWatch::default();
        for k in 0..90 {
            if k == 1 {
                ctl.apply_action(A::Jump, false);
            }
            if k == 15 {
                ctl.apply_action(A::MoveForward, true);
            }
            live_frame(&mut ctl, &mut cam, dt, &mut w);
            ctl.take_landing();
            watch.see(k, &w, &ctl, &cam);
        }
        wrong.extend(watch.wrong("the jump on stairs-1's step 3"));
        // Part way up the ladder, then south out of the hatch.
        let mut w = Aboard::homestead();
        let (mut ctl, mut cam) = body_at(Vec3::new(33.0, EYE, 27.4));
        cam.yaw = SOUTH;
        ctl.apply_action(A::Jump, true);
        run(&mut ctl, &mut cam, &mut w, dt, 30);
        ctl.apply_action(A::Jump, false);
        let up = feet(&ctl, &cam);
        assert!(up > 1.2 && up < 2.0, "the climb got to {up}");
        ctl.apply_action(A::MoveForward, true);
        let mut watch = SlabWatch::default();
        for k in 0..60 {
            live_frame(&mut ctl, &mut cam, dt, &mut w);
            ctl.take_landing();
            watch.see(k, &w, &ctl, &cam);
        }
        wrong.extend(watch.wrong("the climber walking south"));
        // Up stairs-1 from its foot (x 41.0, climbing west) onto the landing, and back down.
        let mut w = Aboard::homestead();
        let (mut ctl, mut cam) = body_at(Vec3::new(42.0, EYE, 38.65));
        cam.yaw = WEST;
        ctl.apply_action(A::MoveForward, true);
        let hard = run(&mut ctl, &mut cam, &mut w, dt, 150);
        let (x, f) = (cam.position.x, feet(&ctl, &cam));
        if !(x < 35.5 && (f - 3.0).abs() < 1e-3 && hard < 0.5) {
            wrong.push(format!("up stairs-1: ended at x {x:.2} with the feet at {f:.2}, hardest landing {hard:.2} m"));
        }
        cam.yaw = EAST;
        let hard = run(&mut ctl, &mut cam, &mut w, dt, 150);
        let (x, f) = (cam.position.x, feet(&ctl, &cam));
        if !(x > 41.5 && f.abs() < 1e-3 && hard < 0.5) {
            wrong.push(format!("down stairs-1: ended at x {x:.2} with the feet at {f:.2}, hardest landing {hard:.2} m"));
        }
        assert!(wrong.is_empty(), "a head through a slab: {wrong:#?}");
    }

    /// One frame with play OUT of first person (the third-person or orbit camera, or the build
    /// editor), as lib.rs runs it: the elevators are left as they are (`pause_elevators`), the
    /// floor and the ceiling under the body are read as ever, there is no ladder, and the body
    /// does not move (the other cameras move points of their own, never the body).
    fn away_frame(ctl: &mut CameraController, cam: &Camera, w: &mut Aboard) {
        ctl.take_lift_press();
        pause_elevators(&mut w.cars);
        ctl.set_on_lift(false);
        let p = cam.position;
        let footing = w.footing();
        if let Some(f) = footing.floor_under(p, ctl.step_up_from(p.y)) {
            ctl.set_ground_floor(f);
        }
        ctl.set_ceiling(footing.ceiling_over(p));
        ctl.set_climb_zone(None);
    }

    /// LEAVING FIRST PERSON ON THE ELEVATOR KEEPS THE CAR WHERE IT IS (ship homes increment 7
    /// review). A rider takes the test home's elevator to the top (3.0), then changes camera (third
    /// person, the orbit camera) or opens the build editor for a second and comes back: the car is
    /// still at the top and the body still on it, with no fall. lib.rs used to clear every car's
    /// state whenever play left first person, so the car was back at the bottom on return and the
    /// rider fell the 3 m storey (the car, called back up, catching them part way).
    ///
    /// Seen red first (2026-10-21) with `pause_elevators` clearing the cars, as lib.rs did: "out of
    /// first person the car went to 0".
    #[test]
    fn leaving_first_person_on_the_elevator_keeps_the_car_where_it_is() {
        let mut w = Aboard::two_storeys();
        let (mut ctl, mut cam) = body_at(Vec3::new(10.0, EYE, 2.0));
        ctl.apply_action(A::Jump, true);
        frames(&mut ctl, &mut cam, &mut w, 1, "riding up");
        ctl.apply_action(A::Jump, false);
        frames(&mut ctl, &mut cam, &mut w, 240, "riding up");
        assert!((w.car() - 3.0).abs() < 1e-4 && (feet(&ctl, &cam) - 3.0).abs() < 1e-3, "the ride up left the car at {} and the feet at {}", w.car(), feet(&ctl, &cam));
        for _ in 0..60 {
            away_frame(&mut ctl, &cam, &mut w);
        }
        assert!((w.car() - 3.0).abs() < 1e-4, "out of first person the car went to {}", w.car());
        assert!((ctl.ground_floor() - 3.0).abs() < 1e-4, "out of first person the floor under the rider went to {}", ctl.ground_floor());
        frames(&mut ctl, &mut cam, &mut w, 120, "back in first person on the car");
        assert!((w.car() - 3.0).abs() < 1e-4 && (feet(&ctl, &cam) - 3.0).abs() < 1e-3, "back in first person the car is at {} and the feet at {}", w.car(), feet(&ctl, &cam));
    }
}
