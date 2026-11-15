//! NO FURNITURE STANDS IN A DOORWAY, AND EVERY ROOM IS WALKED INTO ROUND ITS FURNITURE (2026-11-15).
//!
//! Seen that day in the room finishes rung 2 builder's kitchen screenshot: the homestead's kitchen
//! counter (kitchen_counter_1, data/machines/home.ron) stood along the pantry wall straight across
//! the pantry's 1.6 m sliding door, 2 cm from it. The furniture box fix (BUG-313) had checked "no
//! new piece in front of an interior door" with a one-off survey, not a kept test, and since
//! 2026-11-13 the walk stops at every piece (`Footprint::of_placed`, engine/crowd_walk.rs
//! `furniture_walls_near`), so a door with a piece across it is a room nobody can reach.
//!
//! Two rules, over every shipped design with every household's machine file its starts give it
//! (data/homes/starts.ron, as furniture_fit_tests.rs reads them), the pieces as the game draws and
//! collides with them (`MachineHome::placements` in the home's zone, `Footprint::of_placed`):
//!
//! - **A doorway's clear zone holds no piece.** In front of every doorway, on each side a person
//!   comes through it from, a rectangle the doorway's full width and `CLEAR_M` deep from the wall's
//!   face (deeper for a swinging door whose leaf is wider: its leaf sweeps a quarter circle as
//!   deep as it is wide) holds no piece's box at the heights the doorway stands across. The
//!   doorways are every interior wall's door that opens (style other than "fixed") and every cut the
//!   shipped ship makes through the home's shell (the front door's corridor), except a cut along
//!   an open span (a row house's yard open to the green: a line, not a doorway). An interior door
//!   is gone through from both sides, and a swinging one swings away from whoever opens it
//!   (`door_anim::swing_away_from`), so every opener pushes and both sides get the zone; a shell
//!   cut gets it on the home's side.
//! - **Every room is walked into round its furniture.** On each storey, the walk's floor (the
//!   ground storey's whole box, an upper storey's slabs) with the home's walls and every piece's
//!   box, at the player's radius (`wall_collision::PLAYER_RADIUS`, 0.3 m, the radius the player's
//!   walk is swept at; the crowd's `crowd_nav::BODY_RADIUS_M` is smaller, so a room the player walks
//!   into the crowd walks into too), as the crowd's navigation mesh builds it (`NavMesh::build`,
//!   which leaves out a piece under the knee as the walk steps over it): a body stands a step
//!   inside every doorway on each side, and the walkable floor nearest each room's middle is joined
//!   by a walk to one of the room's own doorways (to any doorway on its storey, for a room with
//!   none of its own: a part of a room open to the rest of it).
//!
//! THE FIGURE, from the 2010 ADA Standards for Accessible Design, read 2026-11-15 at
//! https://www.access-board.gov/ada/ (the standards adopted by the Department of Justice on
//! September 15, 2010). Section 404.2.4: "Maneuvering clearances shall extend the full width of the
//! doorway and the required latch side or hinge side clearance." Table 404.2.4.1, swinging doors,
//! from the front: "Push | 48 inches (1220 mm) | 0 inches (0 mm) 1" (footnote 1: "Add 12 inches
//! (305 mm) if closer and latch are provided."; our doors have neither) and "Pull | 60 inches (1525
//! mm)". Section 404.2.4.2 gives doorways without doors and sliding doors "less than 36 inches (915
//! mm) wide" the same front approach, Table 404.2.4.2: "From Front | 48 inches (1220 mm)". So
//! `CLEAR_M` is the push side's 1220 mm across the doorway's full width: every opener here pushes.
//! Two places this asks more than the standard: it is applied inside private homes, which the
//! standard does not cover, and to sliding doorways 36 inches wide and wider, which it leaves
//! without a figure; the depth a person needs to come straight through a doorway does not shrink
//! because the doorway is wide, and a counter across a 1.6 m sliding door is exactly what this is for.

use crate::crowd_nav::{FloorRect, Footprint, NavInput, NavMesh};
use crate::home_structure::{HomeStructure, OpeningKind, ShellCut};
use crate::machines::{MachineHome, PlacedMachine, ZoneRect};
use crate::ship_structure::{HomeDesign, ShipStructure};
use crate::starts::Starts;
use crate::wall_collision::{wall_segments_with_shell_cuts, PLAYER_RADIUS};
use glam::Vec2;
use std::collections::{BTreeSet, HashMap};

/// How deep a doorway's clear zone is on each side it is come through from, metres: the 2010 ADA
/// Standards' push-side front approach, 48 inches (1220 mm) (the module's note quotes it).
const CLEAR_M: f32 = 1.22;
/// A piece no taller than this over a doorway's foot is a floor covering (the catalog's rugs are
/// 0.02 m): a door is hung over the floor covering, and a body walks over it.
const FLOOR_COVER_M: f32 = 0.025;
/// How far a piece may reach into a clear zone and still be clear: a few millimetres, the rounding
/// of a hand-typed offset (furniture_fit_tests.rs `TOL_M`).
const TOL_M: f32 = 0.005;
/// Where a body stands to come through a doorway: this far past the wall's face, beyond its radius.
const STEP_IN_M: f32 = 0.1;

fn repo() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("HUMANITY_REPO_ROOT"))
}

/// One shipped home as it is checked: its design kind, design, household machine file, and the
/// cuts the shipped ship makes through its shell.
struct Home {
    kind: String,
    design: HomeDesign,
    file: String,
    cuts: Vec<ShellCut>,
}

impl Home {
    fn machines(&self) -> MachineHome {
        MachineHome::load(&repo().join("data").join("machines").join(&self.file)).unwrap_or_else(|| panic!("data/machines/{} parses", self.file))
    }
}

/// The shipped design of a kind (data/homes/shipped/), None when the file is absent.
fn shipped_design(kind: &str) -> Option<HomeDesign> {
    let p = repo().join("data").join("homes").join("shipped").join(format!("{kind}.ron"));
    let text = std::fs::read_to_string(&p).ok()?;
    Some(ron::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
}

/// The cuts the shipped ship makes through the shell of a home on the first plot of `plot_kind`
/// (the front door's corridor, a row house's yard opened to the green).
fn shell_cuts(plot_kind: &str) -> Vec<ShellCut> {
    let data = repo().join("data");
    let ship = ShipStructure::load_ship_file(&data).expect("the ship file loads");
    let plot = ship.plots.iter().find(|p| p.kind == plot_kind).map(|p| p.id.clone()).unwrap_or_else(|| panic!("the ship has a plot of kind {plot_kind}"));
    let ship = ShipStructure::load_and_assemble_shipped(&data, Some(&plot)).unwrap_or_else(|e| panic!("a {plot_kind} home on {plot}: {e}"));
    ship.shell_cuts_for_zone(ship.home_zone_index())
}

/// Every (design, household machine file) the starts ship, once each, with its shell cuts.
fn shipped_homes() -> Vec<Home> {
    let starts = Starts::load(&repo().join("data"));
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for s in &starts.starts {
        let Some(design) = shipped_design(&s.design) else {
            assert!(!s.offered, "start {} is offered but data/homes/shipped/{}.ron is missing", s.id, s.design);
            continue;
        };
        let mut cuts = None;
        for h in &s.households {
            if !repo().join("data").join("machines").join(&h.machines).exists() {
                assert!(!s.offered, "start {} is offered but its household {} names data/machines/{}, which is missing", s.id, h.id, h.machines);
                continue;
            }
            if seen.insert((s.design.clone(), h.machines.clone())) {
                let cuts = cuts.get_or_insert_with(|| shell_cuts(&s.plot_kind)).clone();
                out.push(Home { kind: s.design.clone(), design: design.clone(), file: h.machines.clone(), cuts });
            }
        }
    }
    out
}

/// The household's pieces in the home's own zone as the game draws them (furniture_fit_tests.rs
/// `placed`: the home's zone at the design's origin, so the 0.3 m clamp applies as in the game).
fn placed(design: &HomeDesign, home: &MachineHome) -> Vec<PlacedMachine> {
    let b = &design.body;
    let zones = vec![ZoneRect { id: "home".into(), origin: (0.0, 0.0, 0.0), size: (b.width, b.depth, b.height) }];
    let in_home: BTreeSet<String> = home.all_instances().into_iter().filter(|i| i.zone == "home").map(|i| i.id).collect();
    home.placements(&HashMap::new(), Some(&zones)).into_iter().filter(|p| in_home.contains(&p.id)).collect()
}

/// One doorway: where it is (`centre` on its wall's line, `u` along the wall, `n` across it), how
/// wide, how thick its wall is, the heights it stands across, whether its door swings, and the
/// sides (+1 along `n`, -1 against it) a person comes through it from.
#[derive(Debug, Clone)]
struct Doorway {
    name: String,
    centre: Vec2,
    u: Vec2,
    n: Vec2,
    width: f32,
    half_thickness: f32,
    foot: f32,
    top: f32,
    swings: bool,
    sides: Vec<f32>,
}

impl Doorway {
    /// How deep its clear zone is: `CLEAR_M`, or a swinging leaf's own width where that is more.
    fn depth(&self) -> f32 {
        if self.swings {
            CLEAR_M.max(self.width)
        } else {
            CLEAR_M
        }
    }

    /// The clear zone on one side: the doorway's full width along the wall, `depth` out from the
    /// wall's face, as a turned box (its own x along the wall).
    fn zone(&self, side: f32, depth: f32) -> Footprint {
        let c = self.centre + self.n * side * (self.half_thickness + depth * 0.5);
        Footprint { centre: (c.x, c.y), half: (self.width * 0.5, depth * 0.5), yaw: (-self.u.y).atan2(self.u.x), y0: self.foot, y1: self.top }
    }

    /// Where a body stands to come through from one side: on the doorway's middle, a step past the
    /// wall's face beyond its radius.
    fn step_in(&self, side: f32) -> Vec2 {
        self.centre + self.n * side * (self.half_thickness + PLAYER_RADIUS + STEP_IN_M)
    }
}

/// Every doorway of a design: its interior walls' doors that open, and the shell cuts that are not
/// an open span's line.
fn doorways(body: &HomeStructure, cuts: &[ShellCut]) -> Vec<Doorway> {
    let mut out = Vec::new();
    for w in &body.walls {
        let (a, b) = (Vec2::new(w.a.0, w.a.1), Vec2::new(w.b.0, w.b.1));
        let len = (b - a).length();
        if len < 1e-4 {
            continue;
        }
        let u = (b - a) / len;
        for o in w.openings.iter().filter(|o| o.kind == OpeningKind::Door && o.style != "fixed") {
            let centre = a + u * (o.at + o.width * 0.5);
            out.push(Doorway {
                name: format!("{:.2} m {} door at ({:.2}, {:.2})", o.width, o.style, centre.x, centre.y),
                centre,
                u,
                n: Vec2::new(-u.y, u.x),
                width: o.width,
                half_thickness: w.resolved_thickness() * 0.5,
                foot: w.base + o.sill,
                top: w.base + o.sill + o.height,
                swings: o.style == "swing",
                sides: vec![1.0, -1.0],
            });
        }
    }
    // The shell's edges in a cut's winding (`ShellCut`): 0 from (0,0) to (w,0), 1 to (w,d), 2 to
    // (0,d), 3 back to (0,0).
    let (w, d) = (body.width, body.depth);
    let corner = [Vec2::new(0.0, 0.0), Vec2::new(w, 0.0), Vec2::new(w, d), Vec2::new(0.0, d)];
    let spans: Vec<(usize, f32, f32)> = body.open_shell.iter().filter_map(|s| s.on_edge(w, d)).collect();
    let middle = Vec2::new(w * 0.5, d * 0.5);
    for c in cuts {
        if spans.iter().any(|&(e, from, to)| e == c.edge && c.at >= from - 1e-3 && c.at + c.width <= to + 1e-3) {
            continue;
        }
        let a = corner[c.edge];
        let u = (corner[(c.edge + 1) % 4] - a).normalize();
        let n = Vec2::new(-u.y, u.x);
        let centre = a + u * (c.at + c.width * 0.5);
        out.push(Doorway {
            name: format!("{:.2} m doorway through the shell at ({:.2}, {:.2})", c.width, centre.x, centre.y),
            centre,
            u,
            n,
            width: c.width,
            half_thickness: body.shell_resolved_thickness() * 0.5,
            foot: c.base,
            top: c.base + c.height,
            swings: false,
            sides: vec![if (middle - centre).dot(n) > 0.0 { 1.0 } else { -1.0 }],
        });
    }
    out
}

/// How deep two turned boxes overlap, plan view: the least overlap along their four axes (the
/// separating axis test); 0 or less when they are apart.
fn overlap(a: &Footprint, b: &Footprint) -> f32 {
    let axes = |f: &Footprint| {
        let (s, c) = f.yaw.sin_cos();
        // A footprint's own x and z in the world (`Footprint::walls`' turn).
        [Vec2::new(c, -s), Vec2::new(s, c)]
    };
    let (aa, ba) = (axes(a), axes(b));
    let between = Vec2::new(a.centre.0 - b.centre.0, a.centre.1 - b.centre.1);
    aa.iter()
        .chain(ba.iter())
        .map(|ax| {
            let reach = |f: &Footprint, fa: &[Vec2; 2]| f.half.0 * ax.dot(fa[0]).abs() + f.half.1 * ax.dot(fa[1]).abs();
            reach(a, &aa) + reach(b, &ba) - between.dot(*ax).abs()
        })
        .fold(f32::MAX, f32::min)
}

/// Every piece standing in a doorway's clear zone, each naming the piece, its room, how far in and
/// the doorway and side.
fn zone_problems(h: &Home, home: &MachineHome) -> Vec<String> {
    let doors = doorways(&h.design.body, &h.cuts);
    let pieces = placed(&h.design, home);
    let mut out = Vec::new();
    for d in &doors {
        for &side in &d.sides {
            let z = d.zone(side, d.depth());
            for p in &pieces {
                let f = Footprint::of_placed(p);
                if f.y1 <= d.foot + FLOOR_COVER_M || f.y0 >= d.top {
                    continue;
                }
                let o = overlap(&z, &f);
                if o > TOL_M {
                    let to = d.centre + d.n * side;
                    out.push(format!(
                        "{} / {}: {} ({}) at ({:.3}, {:.3}) stands {o:.3} m into the {:.2} m clear zone of the {} on its ({:.2}, {:.2}) side",
                        h.kind, h.file, p.id, p.room, p.pos.0, p.pos.2, d.depth(), d.name, to.x, to.y
                    ));
                }
            }
        }
    }
    out
}

/// The walk's floor on storey `k`: the ground storey's whole box, an upper storey's slabs.
fn floor_of(body: &HomeStructure, k: usize) -> Vec<FloorRect> {
    if k == 0 {
        return vec![FloorRect { min: (0.0, 0.0), max: (body.width, body.depth) }];
    }
    let storeys = body.storeys();
    body.floors
        .iter()
        .filter(|s| s.is_floor_of(&storeys[k]))
        .map(|s| {
            let r = s.rect();
            FloorRect { min: (r.0, r.1), max: (r.2, r.3) }
        })
        .collect()
}

/// What the walk check found in one home: every room a body cannot walk into round the furniture
/// and every doorway it cannot stand in front of, on every storey; and how many doorway sides and
/// rooms it checked (a room whose middle is open to the storey below is not one).
struct Walk {
    problems: Vec<String>,
    ways: usize,
    rooms: usize,
}

fn walk_problems(h: &Home, home: &MachineHome) -> Walk {
    let b = &h.design.body;
    let walls = wall_segments_with_shell_cuts(b, &h.cuts);
    let prints: Vec<Footprint> = placed(&h.design, home).iter().map(Footprint::of_placed).collect();
    let doors = doorways(b, &h.cuts);
    let mut out = Vec::new();
    let (mut way_count, mut room_count) = (0, 0);
    for (k, st) in b.storeys().iter().enumerate() {
        let floor = floor_of(b, k);
        let on_floor = |p: Vec2| floor.iter().any(|r| r.contains(p.x, p.y));
        let mesh = NavMesh::build(&NavInput { min: (0.0, 0.0), max: (b.width, b.depth), feet_y: st.floor, walls: &walls, footprints: &prints, floor: &floor, radius: PLAYER_RADIUS });
        let joined = |p: Vec2| mesh.poly_at(p).map(|q| mesh.component(q));
        let rooms: Vec<(usize, &crate::home_structure::Zone)> = b.zones.iter().enumerate().filter(|(_, z)| b.storey_of_zone(z) == k).collect();
        // Each doorway's way in from each side: the room it is in, and which piece of walkable
        // floor a body standing there is on (a little either way along the doorway).
        let mut ways: Vec<(Option<usize>, Option<u32>)> = Vec::new();
        for d in doors.iter().filter(|d| b.storey_of_y(d.foot) == k) {
            for &side in &d.sides {
                let p = d.step_in(side);
                if !on_floor(p) {
                    // Upstairs with no slab here: the lift's car or a stair's head is the floor.
                    continue;
                }
                let room = rooms.iter().find(|(_, z)| z.contains_xz(p.x, p.y)).map(|(i, _)| *i);
                let piece = [0.0f32, 0.1, -0.1, 0.2, -0.2].iter().find_map(|t| joined(p + d.u * *t));
                if piece.is_none() {
                    out.push(format!("{} / {}: no room to stand at ({:.2}, {:.2}), a step inside the {}", h.kind, h.file, p.x, p.y, d.name));
                }
                ways.push((room, piece));
                way_count += 1;
            }
        }
        for (i, z) in &rooms {
            let centre = Vec2::new(z.origin.0 + z.size.0 * 0.5, z.origin.2 + z.size.2 * 0.5);
            if !on_floor(centre) {
                // A room whose middle is open to the storey below (a landing over the stairwell).
                continue;
            }
            room_count += 1;
            // The walkable floor in the room nearest its middle.
            let step = mesh.cell;
            let mut middle: Option<(f32, Vec2)> = None;
            for a in 0..=(z.size.0 / step) as i32 {
                for c in 0..=(z.size.2 / step) as i32 {
                    let q = Vec2::new(z.origin.0 + a as f32 * step, z.origin.2 + c as f32 * step);
                    if mesh.walkable(q) && middle.map_or(true, |m| q.distance(centre) < m.0) {
                        middle = Some((q.distance(centre), q));
                    }
                }
            }
            let Some((_, m)) = middle else {
                out.push(format!("{} / {}: {} ({}) has no floor a body stands on round its furniture", h.kind, h.file, z.id, z.label));
                continue;
            };
            let at = joined(m);
            let own: Vec<Option<u32>> = ways.iter().filter(|w| w.0 == Some(*i)).map(|w| w.1).collect();
            let from = if own.is_empty() { ways.iter().map(|w| w.1).collect() } else { own.clone() };
            if !from.iter().any(|c| c.is_some() && *c == at) {
                out.push(format!(
                    "{} / {}: {} ({}): its middle ({:.2}, {:.2}) is not walked to from {}",
                    h.kind,
                    h.file,
                    z.id,
                    z.label,
                    m.x,
                    m.y,
                    if own.is_empty() { "any doorway on its storey".to_string() } else { format!("its {} doorway side(s)", own.len()) }
                ));
            }
        }
    }
    Walk { problems: out, ways: way_count, rooms: room_count }
}

/// The homestead's family home, and its machines with one piece moved to `(x, z)` (and turned to
/// `rot` degrees, when given), on a copy.
fn moved(homes: &[Home], kind: &str, file: &str, id: &str, x: f32, z: f32, rot: Option<f32>) -> (usize, MachineHome) {
    let i = homes.iter().position(|h| h.kind == kind && h.file == file).unwrap_or_else(|| panic!("{kind} / {file} is checked"));
    let mut home = homes[i].machines();
    let mut found = false;
    for inst in home.instances.iter_mut().filter(|inst| inst.id == id) {
        inst.offset.0 = x;
        inst.offset.2 = z;
        if let Some(r) = rot {
            inst.rotation = r;
        }
        found = true;
    }
    assert!(found, "{file} places {id}");
    (i, home)
}

/// NO PIECE STANDS IN A DOORWAY'S CLEAR ZONE, in any of the nine shipped homes.
///
/// RED on 2026-11-15's data, the first run: 19 pieces in 7 machine files, among them
/// "homestead / home.ron: kitchen_counter_1 (room-kitchen) at (37.000, 46.405) stands 0.695 m into
/// the 1.22 m clear zone of the 1.60 m slide door at (38.30, 46.00) on its (38.30, 47.00) side" (the
/// screenshot's counter), the workshop's extinguisher stand in front of its 3 m door, shelves beside
/// the entry's, the aquaponics room's and the bedroom's doors, circulation fans hung at 2.0 m in
/// front of two orchard doorways with no wall behind them, two composters, beds in six bedrooms, and
/// "cabin / cabin.ron: bed_single_1 (room-cabin) at (2.102, 0.498) stands 0.148 m into the 1.22 m
/// clear zone of the 0.90 m doorway through the shell at (0.00, 1.25)". Each moved or turned
/// (BUGS.md BUG-315 lists every move); the two pieces put back on copies below are caught again.
#[test]
fn no_piece_stands_in_a_doorway() {
    let homes = shipped_homes();
    assert!(homes.len() >= 9, "every shipped design with its households is checked, got {}", homes.len());
    let doors: usize = homes.iter().map(|h| doorways(&h.design.body, &h.cuts).len()).sum();
    assert!(doors >= 180, "the homes' doorways are read: {doors}");
    // A row house's front garden opens onto the street along an open span; the cabin's and the
    // homesteads' front doors are cut through the shell, and are doorways.
    let shell_doors = homes.iter().filter(|h| doorways(&h.design.body, &h.cuts).iter().any(|d| d.sides.len() == 1)).count();
    assert!(shell_doors >= 5, "the cabin's and the homesteads' front doors through the shell are doorways: {shell_doors} homes");
    let mut out = Vec::new();
    for h in &homes {
        out.extend(zone_problems(h, &h.machines()));
    }
    assert!(out.is_empty(), "{} piece(s) in a doorway's clear zone:\n{}", out.len(), out.join("\n"));

    // The kitchen counter back across the pantry's sliding door, where it stood that morning.
    let (i, home) = moved(&homes, "homestead", "home.ron", "kitchen_counter_1", 37.0, 46.405, None);
    let red = zone_problems(&homes[i], &home);
    assert!(red.iter().any(|m| m.contains("kitchen_counter_1") && m.contains("1.60 m slide door at (38.30, 46.00)")), "a counter across a door is caught: {red:?}");
    // The cabin's bed back along its south wall, its foot by the front door through the shell.
    let (i, home) = moved(&homes, "cabin", "cabin.ron", "bed_single_1", 2.102, 0.498, Some(0.0));
    let red = zone_problems(&homes[i], &home);
    assert!(red.iter().any(|m| m.contains("bed_single_1") && m.contains("doorway through the shell")), "a bed in front of the front door is caught: {red:?}");
}

/// EVERY ROOM IS WALKED INTO ROUND ITS FURNITURE, in any of the nine shipped homes: a body stands a
/// step inside every doorway, and walks from one of each room's doorways to its middle.
///
/// RED on 2026-11-15's data, the first run: 4 findings, "homestead / home.ron: no room to stand at
/// (38.30, 46.45), a step inside the 1.60 m slide door at (38.30, 46.00)" (the kitchen counter),
/// the entry's shelf at its 0.9 m door to the great room, and the one-person homestead's compost
/// bays: "one_person_homestead / one_person_homestead.ron: room-compost (Compost bays): its middle
/// (33.00, 11.10) is not walked to from its 1 doorway side(s)", the composter standing in the
/// middle of a room 2 m deep behind a door 2.6 m wide. On the moved data both come back on copies
/// with the two pieces put back.
#[test]
fn every_room_is_walked_into_round_its_furniture() {
    let homes = shipped_homes();
    let mut out = Vec::new();
    let (mut ways, mut rooms) = (0, 0);
    for h in &homes {
        let w = walk_problems(h, &h.machines());
        out.extend(w.problems);
        ways += w.ways;
        rooms += w.rooms;
    }
    assert!(ways >= 350 && rooms >= 150, "the walk checked {ways} doorway sides and {rooms} rooms");
    assert!(out.is_empty(), "{} way(s) in blocked:\n{}", out.len(), out.join("\n"));

    let (i, home) = moved(&homes, "homestead", "home.ron", "kitchen_counter_1", 37.0, 46.405, None);
    let red = walk_problems(&homes[i], &home).problems;
    assert!(red.iter().any(|m| m.contains("no room to stand at (38.30, 46.45)")), "a counter across a door leaves no room to stand inside it: {red:?}");
    let (i, home) = moved(&homes, "one_person_homestead", "one_person_homestead.ron", "composter_1", 33.0, 10.5, None);
    let red = walk_problems(&homes[i], &home).problems;
    assert!(red.iter().any(|m| m.contains("room-compost") && m.contains("is not walked to")), "a room whose middle is cut off is caught: {red:?}");
}
