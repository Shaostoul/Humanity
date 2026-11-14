//! INTERACTIONS IN THE WORLD (character animation rung 2b, 2026-11-10; docs/design/character-animation.md
//! sections 3.3 and 17; the anchors and their poses are `humanity_animation::interact`).
//!
//! This is the gameplay half: where the anchors stand in the world, who is granted one, and the
//! capsule moved only inside that grant. Objects name their anchors (a machine definition's
//! `anchors`, a structure type's, an opening style's); here each is placed at its object's origin and
//! turn.
//!
//! WHAT GRANTS AN INTERACTION (gameplay never waits for animation; the existing systems decide):
//! - SITTING and LYING: E at a seat (a chair, the couch) sits the player on the nearest seat of it;
//!   E at the bed sleeps the night as before (`built_uses::use_machine`) and lies them down in it.
//!   Both are ROOT DRIVEN: the capsule (the camera's eye) is eased from where the player stood to the
//!   anchor's root over the enter clip's approach (to its contact frame), held there (the walk,
//!   gravity and falls stop: `CameraController::held`), and eased back to the approach point over the
//!   exit. A movement key (WASD or jump) gets up.
//! - THE LADDER: the ladder's own climb (`renderer::camera`, `footing::ladder_at`) moves the capsule;
//!   while the body is on a ladder (off the floor in its climb zone) the climb is shown, hands and feet
//!   on its rungs. Not root driven.
//! - A STATION: while a machine's card is open (the player pressed E at it) and the player stands at
//!   it, a workbench's surface takes both hands. Not root driven.
//! - A DOOR: a swing door opening within arm's reach of a person (the doors open themselves as people
//!   come, `home_meshes`): their nearer hand closes round its handle while it swings. Worked out on
//!   every screen for everyone drawn (cosmetic, like the door itself), so it is never sent.
//!
//! WHAT CROSSES THE NETWORK: the anchor's id and the phase (`humanity_core::stance::Act`), with the
//! look's pitch, in the movement message, each through one clamping reader at both ends. The anchor's
//! world place is not sent: the sender's capsule already stands at its root, and the receiver puts the
//! hips on the seat from the anchor's own row, and finds a ladder or a workbench by its id near the
//! person.

use glam::{Quat, Vec3};
use humanity_animation::interact::{AnchorDef, AnchorKind, Phase};
use humanity_animation::CharacterLibrary;
use humanity_ship::crowd_nav::Footprint;
use humanity_ship::wall_collision::WallSegment;

use crate::engine::state::EngineState;

/// An anchor placed in the world: its row, its object's origin (the middle of the object's
/// footprint on its floor, home frame) and turn (radians about up, with a half turn added when it is
/// used from its other side), and what the object is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    pub anchor: usize,
    pub origin: Vec3,
    pub yaw: f32,
    pub object: ObjectRef,
}

/// What an anchor's object is, for the parts of it a pose needs beyond the row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ObjectRef {
    /// A placed machine (an index into the frame's machine labels).
    Machine(usize),
    /// A ladder piece: its height, rung count and width (its own size, structure_types.ron).
    Ladder { height: f32, rungs: u32, width: f32 },
    /// A door panel (an index into `EngineState::door_panels`), and whether it is swinging open now
    /// (a hand reaching for a shut one's handle is not yet on it: the probe measures the swing).
    Door { panel: usize, swinging: bool },
}

impl Placed {
    /// A point of the object's frame in the world.
    pub fn point(&self, local: [f32; 3]) -> Vec3 {
        self.origin + Quat::from_rotation_y(self.yaw) * Vec3::from(local)
    }

    /// Where the person's root stands while it holds, and which way they face (the turn of +Z about
    /// +Y, `engine::animation::Person::facing`).
    pub fn root(&self, def: &AnchorDef) -> (Vec3, f32) {
        (self.point(def.root), self.yaw + def.facing_deg.to_radians())
    }

    /// Where they stand before it, and get up to.
    pub fn approach(&self, def: &AnchorDef) -> Vec3 {
        self.point(def.approach)
    }
}

/// How a seat is sat at where it stands among the furniture and the walls (2026-11-13, BUG-298):
/// a chair tucked under a desk or a table, its approach crowded, is pulled out its row's
/// `pull_out_m` (back along its own +Z) when there is room behind it, and the person stands
/// beside it, clear of every footprint, before sitting and after getting up, while it is pushed
/// back in (or left out, the row's `tuck_after`). A chair whose approach is already clear is sat
/// at as its row says: the default plan.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct SeatPlan {
    /// How far the seat is drawn slid back along its own +Z while it is sat on, metres.
    pub pull_m: f32,
    /// Where the person stands before sitting and gets up to (world, the feet), when it is not
    /// the row's approach (slid with the seat).
    pub stand: Option<Vec3>,
}

impl SeatPlan {
    /// The seat's slide out, world.
    pub fn back(&self, placed: &Placed) -> Vec3 {
        Quat::from_rotation_y(placed.yaw) * Vec3::Z * self.pull_m
    }

    /// Where the person's root stands while seated, and which way they face: the row's root on
    /// the slid seat.
    pub fn root(&self, def: &AnchorDef, placed: &Placed) -> (Vec3, f32) {
        let (root, facing) = placed.root(def);
        (root + self.back(placed), facing)
    }

    /// Where they stand before sitting, and get up to.
    pub fn stand(&self, def: &AnchorDef, placed: &Placed) -> Vec3 {
        self.stand.unwrap_or_else(|| placed.approach(def) + self.back(placed))
    }
}

/// True when a body whose feet are at `feet` stands clear of `pieces` (each the body's radius and
/// `margin` off, by the walk's knee-to-eye rule) and of `walls` (the walk's collision does not
/// move it).
fn stands_clear(feet: Vec3, pieces: &[Footprint], walls: &[WallSegment], margin: f32) -> bool {
    let r = humanity_ship::wall_collision::PLAYER_RADIUS;
    if pieces.iter().any(|f| f.stops_body(feet.y) && f.distance(feet.x, feet.z) < r + margin) {
        return false;
    }
    let eye = feet + Vec3::Y * humanity_ship::wall_collision::EYE_HEIGHT_M;
    let held = humanity_ship::wall_collision::resolve(eye, eye, r + margin, walls, &[]);
    horizontal(held - eye).length() < 1e-3
}

/// The footprint moved by `by` (world).
fn moved(f: &Footprint, by: Vec3) -> Footprint {
    Footprint { centre: (f.centre.0 + by.x, f.centre.1 + by.z), ..*f }
}

/// The step of the grid of stand-here points tried round a crowded seat's approach, metres.
const STAND_STEP_M: f32 = 0.15;
/// How far the grid reaches either side of the approach (across the way the seated person faces),
/// in steps: 1.2 m, past a chair's side and the body's width, or along a couch from its middle seat
/// to clear of the low table in front of it (couch_1 and side_table_5 at the family home: at 0.9 m
/// its middle seat had no point clear, 2026-11-13).
const STAND_SIDE_STEPS: i32 = 8;
/// How far the grid reaches back from the approach toward the seat and past it (steps, 1.35 m:
/// beside a dining chair whose approach is under its table, or behind a chair pulled out) and on
/// out in front (2 steps).
const STAND_BACK_STEPS: i32 = 9;
const STAND_FRONT_STEPS: i32 = 2;

/// True when the seat's box at `out` has room: clear of the walls and of every other piece, taken
/// as the circle round its corners (a little more room than the box needs, never less).
fn has_room(out: &Footprint, feet_y: f32, others: &[Footprint], walls: &[WallSegment]) -> bool {
    let round = out.half.0.hypot(out.half.1);
    if others.iter().any(|f| f.stops_body(feet_y) && f.distance(out.centre.0, out.centre.1) < round) {
        return false;
    }
    let eye = Vec3::new(out.centre.0, feet_y + humanity_ship::wall_collision::EYE_HEIGHT_M, out.centre.1);
    let held = humanity_ship::wall_collision::resolve(eye, eye, round, walls, &[]);
    horizontal(held - eye).length() < 1e-3
}

/// How the person sits at seat `def` of `placed` (`SeatPlan`): `own` the seat's own footprint,
/// `others` every other piece's, `walls` the walk's walls. A seat whose approach stands clear is
/// sat at as its row says. Otherwise it is pulled out its row's `pull_out_m` when its box has room
/// there (or left where it is), and the stand-here point is the nearest to the row's approach (on
/// the seat where it is sat on) of a grid round it, laid along the way the seated person faces,
/// that stands clear of every other piece, of the seat both pulled out and where it settles, and of
/// the walls, and is reached from the seated root without crossing a wall. With no such point the
/// row's own approach is kept (the shipped seats each have one: the test
/// `getting_up_from_every_shipped_seat_stands_clear_of_the_furniture`).
pub(crate) fn plan_seat(def: &AnchorDef, placed: &Placed, own: Option<&Footprint>, others: &[Footprint], walls: &[WallSegment]) -> SeatPlan {
    let row = SeatPlan::default();
    if def.kind != AnchorKind::Sit {
        return row;
    }
    let feet_y = placed.origin.y;
    let mut all: Vec<Footprint> = others.to_vec();
    all.extend(own.copied());
    if stands_clear(placed.approach(def), &all, walls, 0.0) {
        return row;
    }
    let pulls = [def.pull_out_m, 0.0];
    for (i, &pull) in pulls.iter().enumerate() {
        if i == 0 && pull <= 0.0 {
            continue;
        }
        let plan = SeatPlan { pull_m: pull, stand: None };
        let back = plan.back(placed);
        let out = own.map(|f| moved(f, back));
        if pull > 0.0 && !out.is_some_and(|o| has_room(&o, feet_y, others, walls)) {
            continue;
        }
        // Clear of every other piece, of the seat pulled out (where it is as they get up) and of
        // where it settles (pushed back in, or left out).
        let mut clear_of: Vec<Footprint> = others.to_vec();
        clear_of.extend(out);
        if def.tuck_after || pull <= 0.0 {
            clear_of.extend(own.copied());
        }
        let approach = plan.stand(def, placed);
        let (root, facing) = plan.root(def, placed);
        // The way the seated person faces (toward the approach), and across it.
        let ahead = Quat::from_rotation_y(facing) * Vec3::Z;
        let across = Vec3::new(ahead.z, 0.0, -ahead.x);
        let mut grid: Vec<(f32, Vec3)> = Vec::new();
        for s in -STAND_SIDE_STEPS..=STAND_SIDE_STEPS {
            for f in -STAND_BACK_STEPS..=STAND_FRONT_STEPS {
                let off = across * (s as f32 * STAND_STEP_M) + ahead * (f as f32 * STAND_STEP_M);
                grid.push((off.length(), approach + off));
            }
        }
        grid.sort_by(|a, b| a.0.total_cmp(&b.0));
        let eye = Vec3::Y * humanity_ship::wall_collision::EYE_HEIGHT_M;
        for (_, feet) in grid {
            if !stands_clear(feet, &clear_of, walls, 0.02) {
                continue;
            }
            // Reached from the seat without crossing a wall: a thin body carried from the root.
            let to = humanity_ship::wall_collision::resolve(root + eye, feet + eye, 0.05, walls, &[]);
            if horizontal(to - (feet + eye)).length() > 1e-3 {
                continue;
            }
            let stand = (horizontal(feet - approach).length() > 1e-4).then_some(feet);
            return SeatPlan { pull_m: pull, stand };
        }
    }
    row
}

/// The player's granted interaction, from frame to frame (state, like a position).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Engaged {
    pub placed: Placed,
    pub phase: Phase,
    /// Seconds into the phase.
    pub t: f32,
    /// Where their root stood and which way they faced when it began (the root's ease starts there).
    pub from: (Vec3, f32),
    /// Asked to get up (a movement key): the hold ends at once, the exit plays.
    pub leave: bool,
    /// The seconds of walking in to the approach point at the start of the enter phase (0 when they
    /// began there): the body walks (the locomotion graph, reading the root's motion), then the
    /// enter clip starts. Not sent: the act goes out once the walk-in is done.
    pub approach_s: f32,
    /// Where the seat is drawn while sat on and where they stand up to (`plan_seat`); the default
    /// (the row's own approach, nothing moved) for everything but a crowded seat.
    pub seat: SeatPlan,
}

/// The interactions' runtime: the player's grant, and the switches the rigs' red twins use.
#[derive(Debug, Clone)]
pub(crate) struct InteractRuntime {
    pub me: Option<Engaged>,
    /// Motion warping on (showcase `warp` 0 is the red twin: every clip plays as made, so the hips
    /// land wherever the clip puts them, not on this seat).
    pub warp_on: bool,
    /// Hands and feet onto their contacts (showcase `hand_ik` 0 is the red twin: the climb and the
    /// workbench keep the idle's hands).
    pub hand_ik_on: bool,
    /// The first-person head hidden (showcase `fp_head` 0 is the red twin: the head drawn round the
    /// eye).
    pub fp_head_hidden: bool,
    /// Rung 2b at all (showcase `rung2b` 0: no interactions, no look-at, no first-person body, as
    /// main drew before this rung): the same-boot twin its cost is measured against.
    pub rung2b_on: bool,
    /// Seated, plant the feet with the whole leg (showcase `seat_plant` 1, the red twin of the
    /// hanging legs: the thighs slope down through a seat high for the body to reach the floor).
    pub seat_plant: bool,
    /// Each door's open fraction on the last frame (a door opening is one whose fraction rose).
    pub door_prev: Vec<f32>,
    /// When each door's fraction last rose (the runtime clock): a door that opened within
    /// `DOOR_HOLD_S` is still being pushed open by whoever is at it.
    pub door_rose_at: Vec<f32>,
    /// The interaction probe while it runs (showcase `anchor_probe`, engine::anchor_probe).
    pub probe: Option<crate::engine::anchor_probe::AnchorProbe>,
    /// The chairs drawn off where they are placed: the one the player sits on pulled out, one
    /// being pushed back in after they got up, one left out (`SeatPlan`, BUG-298). Each is drawn
    /// there (`home_meshes::push_home_machines`), its footprint stands there
    /// (`crowd_walk::refresh_furniture`) and its anchors are placed there (`machine_anchors`).
    pub chairs: Vec<ChairSlide>,
}

impl Default for InteractRuntime {
    fn default() -> Self {
        InteractRuntime { me: None, warp_on: true, hand_ik_on: true, fp_head_hidden: true, rung2b_on: true, seat_plant: false, door_prev: Vec::new(), door_rose_at: Vec::new(), probe: None, chairs: Vec::new() }
    }
}

impl InteractRuntime {
    /// How far the placed machine `machine_id` is drawn off its placement (world): zero for every
    /// machine but a chair pulled out.
    pub fn chair_offset(&self, machine_id: &str) -> Vec3 {
        self.chairs.iter().find(|c| c.machine_id == machine_id).map_or(Vec3::ZERO, ChairSlide::offset)
    }

    /// The player sits at chair `machine_id`: it slides `by` (world) as they sit, and is pushed
    /// back in after or left out (`tuck`). A chair held before is let go first.
    pub fn hold_chair(&mut self, machine_id: String, by: Vec3, tuck: bool) {
        for c in self.chairs.iter_mut().filter(|c| c.held) {
            c.release();
        }
        let base = self.chairs.iter().position(|c| c.machine_id == machine_id).map_or(Vec3::ZERO, |i| self.chairs.remove(i).offset());
        self.chairs.push(ChairSlide { machine_id, base, by, k: 0.0, tuck, held: true });
    }
}

/// How long a chair takes to slide out, or back in, seconds: one pull or push of a chair.
pub(crate) const CHAIR_SLIDE_S: f32 = 0.5;

/// A chair drawn off its placement (`InteractRuntime::chairs`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChairSlide {
    /// The placed machine's id (`MachineLabel::machine_id`).
    pub machine_id: String,
    /// Where it already stood off its placement before this sit (one left out before), world.
    pub base: Vec3,
    /// This sit's slide, world (its own +Z times the plan's pull).
    pub by: Vec3,
    /// How much of the slide has been made, 0 to 1 (the time over `CHAIR_SLIDE_S`).
    pub k: f32,
    /// Pushed back in once the player is up (the row's `tuck_after`).
    pub tuck: bool,
    /// The player's sit holds it.
    pub held: bool,
}

impl ChairSlide {
    /// Where it is drawn off its placement now, world: the slide eased in and out.
    pub fn offset(&self) -> Vec3 {
        let k = self.k.clamp(0.0, 1.0);
        self.base + self.by * (k * k * (3.0 - 2.0 * k))
    }

    /// The sit let it go: one left out stays where it is drawn.
    fn release(&mut self) {
        self.held = false;
        if !self.tuck {
            self.base = self.offset();
            self.by = Vec3::ZERO;
            self.k = 0.0;
        }
    }
}

/// One frame of the chairs' slides: the one the player's sit holds follows it (`held`: the sit's
/// phase and the seconds into its own enter clip), out over `CHAIR_SLIDE_S` from when the enter
/// begins, out through the hold and the getting up; once they are up it is pushed back in over
/// `CHAIR_SLIDE_S`, or stays out. A chair back where it is placed is dropped.
pub(crate) fn step_chairs(chairs: &mut Vec<ChairSlide>, held: Option<(Phase, f32)>, dt: f32) {
    for c in chairs.iter_mut() {
        match (c.held, held) {
            (true, Some((Phase::Enter, clip_t))) => c.k = (clip_t / CHAIR_SLIDE_S).clamp(0.0, 1.0),
            (true, Some(_)) => c.k = 1.0,
            (true, None) => c.release(),
            (false, _) if c.tuck => c.k = (c.k - dt.max(0.0) / CHAIR_SLIDE_S).max(0.0),
            (false, _) => {}
        }
    }
    chairs.retain(|c| c.held || c.k > 0.0 || c.base != Vec3::ZERO);
}

/// How fast the root eases into a root-driven anchor when its enter has no clip (seconds); with a
/// clip it is eased over the clip's approach (to its contact frame).
const ROOT_EASE_S: f32 = 0.6;

/// How fast a granted person walks in to the approach point (metres a second): an unhurried walk.
const WALK_IN_MPS: f32 = 1.2;
/// A start nearer the approach point than this needs no walk-in (metres).
const WALK_IN_FROM_M: f32 = 0.15;

impl Engaged {
    /// The seconds into the anchor's own enter (the clip's time): negative during the walk-in.
    pub fn clip_t(&self) -> f32 {
        if self.phase == Phase::Enter {
            self.t - self.approach_s
        } else {
            self.t
        }
    }
}

/// The anchors a placed machine offers, placed in the world (by its label's index): its definition's
/// `anchors`, each at the machine's draw position and turn.
pub(crate) fn machine_anchors(state: &EngineState, lib: &CharacterLibrary, label: usize) -> Vec<Placed> {
    let (Some(l), Some(obj), Some(home)) = (state.gui_state.machine_labels.get(label), state.machine_objects.get(label), state.gui_state.home_machines.as_ref()) else {
        return Vec::new();
    };
    let types = home.instance_types();
    let Some(def) = types.get(&l.machine_id).and_then(|t| home.catalog.get(t)) else { return Vec::new() };
    // Where it is drawn: a chair left out stands where it was left (BUG-298).
    let origin = obj.2 + state.interact.chair_offset(&l.machine_id);
    def.anchors
        .iter()
        .filter_map(|id| lib.anchors.index(id))
        .map(|anchor| Placed { anchor, origin, yaw: obj.3.to_radians(), object: ObjectRef::Machine(label) })
        .collect()
}

/// Every placed machine's anchors whose row is `anchor`, nearest `at` first, within `reach` of their
/// approach (a remote player or a stand-in named by id: find the seat or the workbench they use).
pub(crate) fn machine_anchor_near(state: &EngineState, lib: &CharacterLibrary, anchor: usize, at: Vec3, reach: f32) -> Option<Placed> {
    let def = lib.anchors.anchors.get(anchor)?;
    let home = state.gui_state.home_machines.as_ref()?;
    let types = home.instance_types();
    let mut best: Option<(f32, Placed)> = None;
    for (i, (l, obj)) in state.gui_state.machine_labels.iter().zip(&state.machine_objects).enumerate() {
        let Some(mdef) = types.get(&l.machine_id).and_then(|t| home.catalog.get(t)) else { continue };
        if !mdef.anchors.iter().any(|a| a == &def.id) {
            continue;
        }
        for flip in [0.0, std::f32::consts::PI] {
            // A station is used from either side; a seat only from its front.
            if flip != 0.0 && def.kind != AnchorKind::Station {
                continue;
            }
            let p = Placed { anchor, origin: obj.2, yaw: obj.3.to_radians() + flip, object: ObjectRef::Machine(i) };
            let (root, _) = p.root(def);
            let d = horizontal(root - at).length().min(horizontal(p.approach(def) - at).length());
            if (root.y - at.y).abs() < 1.0 && d <= reach && best.is_none_or(|(b, _)| d < b) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// Whether a person with their feet at `feet` stands at a station where both hands reach its work
/// surface: in front of its edge by `STATION_NEAR_M` to `STATION_FAR_M` (the edge is 13 cm beyond the
/// hands' line, the row's contact), along it within its span and a little more, on its floor. A
/// person further off keeps their hands (the card stays open; their arms do not stretch for it).
pub(crate) fn at_station(def: &AnchorDef, placed: &Placed, feet: Vec3) -> bool {
    let Some(c) = def.contact(humanity_animation::interact::ContactPart::Hands) else { return false };
    let local = Quat::from_rotation_y(-placed.yaw) * (feet - placed.origin);
    let edge = c.at[2] - STATION_EDGE_INSET_M;
    let off = edge - local.z;
    (STATION_NEAR_M..=STATION_FAR_M).contains(&off) && local.x.abs() <= def.span_m.unwrap_or(0.0) + 0.3 && local.y.abs() < 0.6
}

/// A station's edge stands this far beyond its hands' line (the rows put the hands 8 cm in).
const STATION_EDGE_INSET_M: f32 = 0.08;
/// How near and how far in front of a station's edge a person stands to work at it (metres): the
/// shortest body's arms reach a hand 8 cm in from the edge of a 0.85 m top from 25 cm off, upright.
const STATION_NEAR_M: f32 = 0.03;
const STATION_FAR_M: f32 = 0.25;

/// The ladder piece a body standing with its feet at `feet` is at (within its climb reach), placed as
/// its anchor's object, facing the side the body is on; None when there is none, or the ladder type
/// names no anchor the library has.
pub(crate) fn ladder_near(state: &EngineState, lib: &CharacterLibrary, feet: Vec3) -> Option<Placed> {
    let ship = state.gui_state.ship_structure.as_ref()?;
    let mut best: Option<(f32, Placed)> = None;
    for (zi, zone) in ship.zones.iter().enumerate() {
        if ship.is_away_home(zi) {
            continue;
        }
        let o = zone.origin;
        for ps in &zone.body.structures {
            let Some(ty) = humanity_ship::structure::structure_type(&ps.type_id) else { continue };
            let Some(anchor) = ty.anchors.iter().find_map(|a| lib.anchors.index(a)) else { continue };
            if lib.anchors.anchors[anchor].kind != AnchorKind::Climb {
                continue;
            }
            let origin = Vec3::new(ps.pos.0 + o.0, ps.pos.1 + o.1, ps.pos.2 + o.2);
            if feet.y < origin.y - 0.6 || feet.y > origin.y + ty.size.1 + 0.6 {
                continue;
            }
            let yaw = ps.rot_deg.to_radians();
            let d = horizontal(feet - origin).length();
            let reach = ty.size.0.max(ty.size.2) * 0.5 + 0.9;
            if d > reach {
                continue;
            }
            // Which face the body is on: the anchor is written for the -Z face.
            let local = Quat::from_rotation_y(-yaw) * (feet - origin);
            let flip = if local.z > 0.0 { std::f32::consts::PI } else { 0.0 };
            let p = Placed { anchor, origin, yaw: yaw + flip, object: ObjectRef::Ladder { height: ty.size.1, rungs: ty.steps.max(2), width: ty.size.0 } };
            if best.is_none_or(|(b, _)| d < b) {
                best = Some((d, p));
            }
        }
    }
    best.map(|(_, p)| p)
}

/// A swing door's handle this frame: the door, its anchor, the handle's world point on the face
/// toward `toward`, which way that face looks, and whether the door is opening now.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Handle {
    pub door: usize,
    pub anchor: usize,
    pub at: Vec3,
    pub out: Vec3,
    pub opening: bool,
    pub open: f32,
}

/// The handle of a door panel on the face toward `toward`, where `door_anim` has swung it this
/// frame; None for a door whose style names no door anchor.
pub(crate) fn door_handle(state: &EngineState, lib: &CharacterLibrary, door: usize, toward: Vec3) -> Option<Handle> {
    door_contact(state, lib, door, toward, false)
}

/// How near the hinge a palm pushing a swinging door goes (metres along the panel from its hinge
/// edge): a hand pushes the slab, not the jamb.
const PUSH_FROM_HINGE_M: f32 = 0.15;

/// `door_handle`, or with `slide` the point a palm pushes a SWINGING door at: on the same face at
/// the handle's height, slid along the panel from the handle toward `toward` (no nearer the hinge
/// than `PUSH_FROM_HINGE_M`), as a person walking through a door they opened keeps their palm on
/// the slab where it passes them (rung 2b's follow-up: the slab draws no handle, so the palm is
/// flat on it). The handle itself is where the hand opens a shut one.
fn door_contact(state: &EngineState, lib: &CharacterLibrary, door: usize, toward: Vec3, slide: bool) -> Option<Handle> {
    let (p, open) = state.door_panels.get(door)?;
    if p.is_window {
        return None;
    }
    let anchor = humanity_systems::door_anim::style_anchors(&p.style).iter().find_map(|a| lib.anchors.index(a))?;
    let def = &lib.anchors.anchors[anchor];
    if def.kind != AnchorKind::Door {
        return None;
    }
    let c = def.contacts.first()?;
    // The panel's frame: x along the wall from its hinge side, the panel centred on `center` with its
    // sill there; the row's x is from the FREE edge back toward the hinge.
    let m = humanity_systems::door_anim::panel_motion(&p.style, *open, p.size.x, p.size.y);
    // The way it swings (a hand door swings away from whoever opened it, rung 2b's follow-up).
    let side = state.door_swing.get(door).map_or(1.0, |s| s.side);
    let hinge_rot = Quat::from_rotation_y(m.hinge * side);
    let side_of = |sign: f32| {
        let point = |x: f32| {
            let local = Vec3::new(x, c.at[1], sign * (p.size.z * 0.5 + c.at[2].abs()));
            let closed = p.center + p.rotation * (Vec3::new(m.offset.0, m.offset.1, m.offset.2) + local);
            p.hinge + hinge_rot * (closed - p.hinge)
        };
        let handle = point(p.size.x * 0.5 + c.at[0]);
        let at = if slide {
            // The nearest point to `toward` across the floor on the face from the handle to near the hinge.
            let inner = point(-p.size.x * 0.5 + PUSH_FROM_HINGE_M);
            let span = horizontal(inner - handle);
            let t = (horizontal(toward - handle).dot(span) / span.length_squared().max(1e-8)).clamp(0.0, 1.0);
            handle + (inner - handle) * t
        } else {
            handle
        };
        let out = (hinge_rot * p.rotation * Vec3::Z * sign).normalize_or_zero();
        (at, out)
    };
    let (a, oa) = side_of(1.0);
    let (b, ob) = side_of(-1.0);
    let (at, out) = if (toward - a).length_squared() <= (toward - b).length_squared() { (a, oa) } else { (b, ob) };
    // Opening: it rose this frame, or within the last DOOR_HOLD_S (a person goes on pushing a door
    // that is still swinging wide, and walks through holding it); a door standing open is not held.
    let prev = state.interact.door_prev.get(door).copied().unwrap_or(*open);
    let rose = state.interact.door_rose_at.get(door).is_some_and(|t| state.animation.clock() - *t < DOOR_HOLD_S);
    Some(Handle { door, anchor, at, out, opening: (*open > prev + 1e-4 || rose) && *open > 0.01, open: *open })
}

/// The swing door whose handle is nearest a person's shoulder (`shoulder`, world) within `reach`
/// metres while it opens.
pub(crate) fn opening_handle_near(state: &EngineState, lib: &CharacterLibrary, shoulder: Vec3, reach: f32) -> Option<Handle> {
    let mut best: Option<(f32, Handle)> = None;
    for i in 0..state.door_panels.len() {
        let (p, open) = &state.door_panels[i];
        if *open <= 0.01 || (p.center - shoulder).length_squared() > 9.0 {
            continue;
        }
        // Swinging, the palm pushes the slab where it passes the person (`door_contact`'s slide).
        let Some(h) = door_contact(state, lib, i, shoulder, true) else { continue };
        let d = (h.at - shoulder).length();
        if h.opening && d <= reach && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, h));
        }
    }
    best.map(|(_, h)| h)
}

/// A SHUT door a hand opens, being walked up to (rung 2b's follow-up): the one whose handle (on the
/// face toward `shoulder`) is nearest `feet` within its row's `reach_m` and `lead` more across the
/// floor, so the hand reaches for the handle before the feet are close enough for the door to start
/// swinging, and is on it as it does.
pub(crate) fn shut_hand_handle_near(state: &EngineState, lib: &CharacterLibrary, feet: Vec3, shoulder: Vec3, lead: f32) -> Option<Handle> {
    let mut best: Option<(f32, Handle)> = None;
    for i in 0..state.door_panels.len() {
        let (p, open) = &state.door_panels[i];
        let Some(hand) = humanity_systems::door_anim::style_hand(&p.style) else { continue };
        if *open > 0.02 || p.is_window || !p.auto_open || (p.center - feet).length_squared() > 9.0 {
            continue;
        }
        let Some(h) = door_handle(state, lib, i, shoulder) else { continue };
        let d = horizontal(h.at - feet).length();
        if d <= hand.reach_m + lead && best.is_none_or(|(b, _)| d < b) {
            best = Some((d, h));
        }
    }
    best.map(|(_, h)| h)
}

/// The handle's place on a hand door's panel for the door's trigger (rung 2b's follow-up): how far
/// in from the free edge (negative, toward the hinge) and how far out from the face, from the
/// door anchor's row (data/characters/interactions.ron, `door_swing_handle`) when the bodies are
/// read, else that row's numbers as shipped.
pub(crate) fn handle_local(lib: Option<&CharacterLibrary>) -> (f32, f32) {
    let shipped = (-0.07, 0.045);
    let Some(lib) = lib else { return shipped };
    lib.anchors
        .anchors
        .iter()
        .find(|a| a.kind == AnchorKind::Door)
        .and_then(|a| a.contacts.first())
        .map_or(shipped, |c| (c.at[0], c.at[2].abs()))
}

/// How long after a door last swung wider it still counts as being opened by whoever is at it
/// (seconds): a person walking through keeps a hand on it for a stride or two after it stops. The
/// door settles in under a second (`door_anim::ease_open`), so at 1.0 s a person reaching the handle
/// as it settled held it for a few frames only (the third rig sweep).
const DOOR_HOLD_S: f32 = 1.5;

/// Keep each door's open fraction and when it last rose, once a frame after the doors moved (the
/// door hand reads whether a door is opening from them).
pub(crate) fn note_doors(state: &mut EngineState) {
    let now = state.animation.clock();
    let n = state.door_panels.len();
    let rt = &mut state.interact;
    rt.door_prev.resize(n, 0.0);
    rt.door_rose_at.resize(n, f32::NEG_INFINITY);
    for (i, (_, open)) in state.door_panels.iter().enumerate() {
        if *open > rt.door_prev[i] + 1e-4 {
            rt.door_rose_at[i] = now;
        }
        rt.door_prev[i] = *open;
    }
}

fn horizontal(v: Vec3) -> Vec3 {
    Vec3::new(v.x, 0.0, v.z)
}

/// E at the machine the player looks at, when it offers a seat or a bed: the nearest anchor of it
/// (lying first at a bed, which E also sleeps in) is granted. True when one was.
pub(crate) fn press_e(state: &mut EngineState) -> bool {
    // Already seated or lying: E gets up (the exit plays), as a movement key does.
    if let Some(e) = state.interact.me.as_mut() {
        e.leave = true;
        return true;
    }
    if !state.aboard_station || state.gui_state.construction_active {
        return false;
    }
    let Some(lib) = state.animation.library() else { return false };
    let Some(label) = state.gui_state.targeted_machine else { return false };
    let feet = state.camera.position - Vec3::Y * state.controller.eye_height();
    let mut best: Option<(f32, Placed)> = None;
    for p in machine_anchors(state, &lib, label) {
        let def = &lib.anchors.anchors[p.anchor];
        if !matches!(def.kind, AnchorKind::Sit | AnchorKind::Lie) {
            continue;
        }
        let d = horizontal(p.approach(def) - feet).length();
        // A bed's E is the night's sleep: it lies down, never on the edge.
        let rank = d + if def.kind == AnchorKind::Lie { -10.0 } else { 0.0 };
        if d <= def.reach_m && best.is_none_or(|(b, _)| rank < b) {
            best = Some((rank, p));
        }
    }
    let Some((_, placed)) = best else { return false };
    let facing = crate::engine::animation::facing_for_camera_yaw(state.camera.yaw);
    grant(state, placed, feet, facing);
    true
}

/// Grant the player an anchor: it begins entering from where they stand and face.
pub(crate) fn grant(state: &mut EngineState, placed: Placed, feet: Vec3, facing: f32) {
    let Some(lib) = state.animation.library() else { return };
    let def = &lib.anchors.anchors[placed.anchor];
    // A seat among the furniture: pulled out from the desk or table it is tucked under, and a
    // stand-here point clear of every piece (BUG-298).
    let mut seat = SeatPlan::default();
    if let (AnchorKind::Sit, ObjectRef::Machine(label)) = (def.kind, placed.object) {
        crate::engine::crowd_walk::refresh_furniture(state);
        seat = seat_plan_now(state, def, &placed);
        if let Some(id) = state.gui_state.machine_labels.get(label).map(|l| l.machine_id.clone()) {
            state.interact.hold_chair(id, seat.back(&placed), def.tuck_after);
        }
    }
    let stand = seat.stand(def, &placed);
    let d = horizontal(stand - feet).length();
    let approach_s = if def.root_driven && d > WALK_IN_FROM_M { d / WALK_IN_MPS } else { 0.0 };
    log::info!("Interaction: granted {} (the {:?}) at {:?}, a {d:.2} m walk in, the seat pulled out {:.2} m, standing at {stand:?}", def.id, def.kind, placed.origin, seat.pull_m);
    state.interact.me = Some(Engaged { placed, phase: Phase::Enter, t: 0.0, from: (feet, facing), leave: false, approach_s, seat });
    state.controller.stop_movement();
}

/// `plan_seat` for a placed seat among the furniture the walk reads (its own footprint the one
/// standing where the seat is) and the walk's walls.
fn seat_plan_now(state: &EngineState, def: &AnchorDef, placed: &Placed) -> SeatPlan {
    let furniture = state.animation.walk.furniture();
    let o = placed.origin;
    let own = furniture.iter().position(|f| (f.centre.0 - o.x).abs() < 0.01 && (f.centre.1 - o.z).abs() < 0.01 && (f.y0 - o.y).abs() < 0.05);
    let others: Vec<Footprint> = furniture.iter().enumerate().filter(|(i, _)| Some(*i) != own).map(|(_, f)| *f).collect();
    plan_seat(def, placed, own.map(|i| &furniture[i]), &others, &state.wall_colliders)
}

/// How long the root eases over on the way in (to the enter clip's contact frame) for the body the
/// player is drawn with, seconds.
fn ease_in_s(lib: &CharacterLibrary, def: &AnchorDef, body: usize) -> f32 {
    let b = &lib.bodies[body.min(lib.bodies.len() - 1)];
    let hips = b.hands.as_ref().map(|h| h.hips);
    match (def.enter.clip.is_some(), hips) {
        (true, Some(h)) => humanity_animation::interact::contact_time(def, &b.clips, &b.skeleton, h),
        _ => ROOT_EASE_S.min(humanity_animation::interact::phase_len(def, Phase::Enter, &b.clips)),
    }
}

/// The root and facing a root-driven engagement draws (and puts the capsule at) `t` seconds into its
/// phase: eased from where it began to the anchor's root over the way in, held there, and eased back
/// to the approach point over the way out.
pub(crate) fn engaged_root(lib: &CharacterLibrary, e: &Engaged, body: usize) -> (Vec3, f32) {
    let def = &lib.anchors.anchors[e.placed.anchor];
    // On the seat where it is drawn while sat on, from and back to where they stand (`SeatPlan`).
    let (root, facing) = e.seat.root(def, &e.placed);
    let stand = e.seat.stand(def, &e.placed);
    let b = &lib.bodies[body.min(lib.bodies.len() - 1)];
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    match e.phase {
        Phase::Enter => {
            let approach = stand;
            let walk = Vec3::new(approach.x - e.from.0.x, 0.0, approach.z - e.from.0.z);
            let walk_facing = if walk.length() > 1e-3 { walk.x.atan2(walk.z) } else { e.from.1 };
            if e.t < e.approach_s {
                // Walking in at a steady pace, turned to the way it goes over its first steps.
                let k = (e.t / e.approach_s).clamp(0.0, 1.0);
                let turn = humanity_core::turning::shortest(e.from.1, walk_facing);
                return (e.from.0.lerp(approach, k), e.from.1 + turn * smooth(e.t / 0.35));
            }
            let (p0, f0) = if e.approach_s > 0.0 { (approach, walk_facing) } else { e.from };
            let k = smooth(e.clip_t() / ease_in_s(lib, def, body).max(1e-3));
            let turn = humanity_core::turning::shortest(f0, facing);
            (p0.lerp(root, k), f0 + turn * k)
        }
        Phase::Hold => (root, facing),
        Phase::Exit => {
            let len = humanity_animation::interact::phase_len(def, Phase::Exit, &b.clips);
            // The way out: off the seat from its lift-off, then to the approach point.
            let start = match (def.exit.clip.is_some(), b.hands.as_ref()) {
                (true, Some(h)) => humanity_animation::interact::release_time(def, &b.clips, &b.skeleton, h.hips),
                _ => 0.0,
            };
            let k = smooth((e.t - start) / (len - start).max(1e-3));
            (root.lerp(stand, k), facing)
        }
    }
}

/// One frame of the player's grant (before the walk): the phase moves on, a movement key asks to get
/// up, the capsule is held where the engagement puts it, and when the exit has played the player
/// stands at the approach point and walks again.
pub(crate) fn tick(state: &mut EngineState, dt: f32) {
    // The chair the sit holds follows it; one let go is pushed back in (BUG-298).
    let held = state.interact.me.map(|e| (e.phase, e.clip_t()));
    if !state.interact.chairs.is_empty() {
        step_chairs(&mut state.interact.chairs, held, dt);
    }
    let Some(mut e) = state.interact.me else {
        state.controller.held = None;
        return;
    };
    let Some(lib) = state.animation.library() else {
        state.interact.me = None;
        state.controller.held = None;
        return;
    };
    // Off the ship, the build editor, or an anchor of a library read before this one: let go at once.
    if !state.aboard_station || state.gui_state.construction_active || e.placed.anchor >= lib.anchors.anchors.len() {
        state.interact.me = None;
        state.controller.held = None;
        return;
    }
    let def = &lib.anchors.anchors[e.placed.anchor];
    let body = lib.body_for(Some(&state.gui_state.appearance.body_type), "player", 0.0);
    let clips = &lib.bodies[body.min(lib.bodies.len() - 1)].clips;
    if state.controller.wants_move() && e.phase != Phase::Exit {
        e.leave = true;
    }
    e.t += dt.max(0.0);
    match e.phase {
        Phase::Enter if e.clip_t() >= humanity_animation::interact::phase_len(def, Phase::Enter, clips) => {
            e.phase = Phase::Hold;
            e.t = 0.0;
        }
        Phase::Hold if e.leave => {
            e.phase = Phase::Exit;
            e.t = 0.0;
        }
        Phase::Exit if e.t >= humanity_animation::interact::phase_len(def, Phase::Exit, clips) => {
            // Stood up: back to walking, at the approach point.
            let (feet, _) = engaged_root(&lib, &Engaged { t: f32::MAX, ..e }, body);
            state.camera.position = feet + Vec3::Y * state.controller.eye_height();
            state.controller.held = None;
            state.interact.me = None;
            log::info!("Interaction: {} let go, standing at {feet:?}", def.id);
            return;
        }
        _ => {}
    }
    let (feet, _) = engaged_root(&lib, &e, body);
    let eye = feet + Vec3::Y * state.controller.eye_height();
    state.controller.held = Some(eye);
    // The seated or lying eye under the standing one, for a view that does not ride the head.
    state.controller.held_drop = match def.kind {
        AnchorKind::Sit => (state.controller.eye_height() - (def.contact(humanity_animation::interact::ContactPart::Hips).map_or(0.45, |c| c.at[1]) + 0.72)).max(0.0),
        AnchorKind::Lie => (state.controller.eye_height() - 0.55).max(0.0),
        _ => 0.0,
    };
    state.interact.me = Some(e);
}

/// What the player is doing, for the movement message (rung 2b): the look's pitch and the act (the
/// granted anchor and its phase, or the ladder while on it, or the workbench in use).
pub(crate) fn local_doing(state: &EngineState) -> humanity_core::stance::Doing {
    let pitch = state.camera.pitch;
    // Not during a walk-in (the receiver times the enter from when the act arrives).
    let walking_in = state.interact.me.is_some_and(|e| e.clip_t() < 0.0);
    let act = state.animation.my_act().filter(|_| !walking_in).map(|(id, phase)| humanity_core::stance::Act { anchor: id, phase: phase.code() });
    humanity_core::stance::Doing { pitch, act }
}

/// The stance an act shows (Sit for sitting and lying, Climb for a ladder), for the movement
/// message's stance byte.
pub(crate) fn act_stance(lib: &CharacterLibrary, anchor: &str) -> Option<humanity_core::stance::Stance> {
    let def = lib.anchors.anchors.get(lib.anchors.index(anchor)?)?;
    humanity_core::stance::Stance::from_code(def.kind.stance_code()).filter(|s| *s != humanity_core::stance::Stance::Stand)
}
