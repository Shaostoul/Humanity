//! The planet walk stands on what the player built (2026-10-20). A child of
//! `engine::planet_build` (`use super::*`), in its own file so that module stays small.
//!
//! Until 2026-10-20 the planet walk's floor was the terrain alone: lib.rs fed
//! `surface_move::radial_step` the drawn ground's radius, and built pieces only stopped a step
//! sideways (`collide_on_site`). In the running game (`just verify-falls`) the eye put at standing
//! height over a Wood Roof on Sahara sand came to rest 0.03 m below standing height on the
//! ground: straight through the roof. Now the floor is the higher of the terrain and the top of a
//! built piece under the body within a step up of its feet (`built_floor_radius`,
//! `surface_move::walk_rest`).
//!
//! Each test walks the body frame by frame the way lib.rs's planet walk does
//! ([`Walker::frame`]): the step along the ground, the built pieces' sideways stop, the floor,
//! one `radial_step`, and the falls hand-off (`CameraController::surface_step`, whose landings
//! `take_landing` hands to the body). The ground is a sphere of Earth's radius, so the terrain's
//! floor is the same everywhere and only the built pieces change it.

use super::*;
use crate::renderer::camera::CameraController;
use crate::surface_move::{radial_step, rest_height, walk_rest, MoveMode, RadialStep};
use crate::surface_walk::DRAWN_CLEARANCE_M;
use crate::ecs::components::EYE_HEIGHT_M;

const EARTH_R: f64 = 6_371_000.0;
/// Earth's surface gravity (data/planets/earth.ron).
const G: f64 = 9.81;
const DT: f64 = 1.0 / 60.0;
/// An unhurt walk, metres a second.
const WALK_M_S: f64 = 5.0;

fn shipped() -> BlueprintRegistry {
    BlueprintRegistry::from_ron(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/data/blueprints/basic.ron"))).unwrap()
}

/// A site on the sphere of Earth's radius at a latitude and longitude (open Sahara sand, the
/// verify-falls planet ground, by default).
fn site_at(lat: f64, lon: f64) -> PlanetSite {
    PlanetSite { body: "earth".into(), origin: crate::terrain::osm_region::latlon_to_dir_f64(lat, lon) * EARTH_R }
}

/// Build `pieces` (id, x, z, quarter turns) in `site` the way the ghost and the dev `build` verb
/// place them (`placement_pose`, so a roof rests on the walls built before it), on flat ground.
fn build_in(world: &mut hecs::World, reg: &BlueprintRegistry, site: &PlanetSite, pieces: &[(&str, f32, f32, u8)]) {
    for (id, x, z, t) in pieces {
        let bp = reg.get(id).unwrap();
        let pose = placement::placement_pose(bp, Vec3::new(*x, 0.0, *z), *t, world, reg, Some(site));
        world.spawn((pose, Structure { blueprint_id: (*id).into(), health: 1.0, max_health: 1.0, provides: bp.provides.clone(), uid: 0 }, site.clone()));
    }
}

/// A hut open to the south, centred on (`x`, `z`): Wood Walls on its north, west and east sides
/// and a Wood Roof on them, spanning x - 2 to x + 2 and z - 2 to z + 2.
fn open_hut(x: f32, z: f32) -> [(&'static str, f32, f32, u8); 4] {
    [("wood_wall", x, z - 2.0, 0), ("wood_wall", x - 2.0, z, 1), ("wood_wall", x + 2.0, z, 1), ("roof", x, z, 0)]
}

/// The top of the first piece of blueprint `id`, metres over the site's ground (site-local y).
fn top_of(world: &hecs::World, id: &str) -> f32 {
    let mut q = world.query::<(&Structure, &Transform)>();
    let tf = q.iter().find(|(_e, (s, _))| s.blueprint_id == id).map(|(_e, (_, tf))| tf.clone()).expect("the piece is built");
    crate::ecs::components::world_aabb(&tf).1.y
}

/// What a frame of the walk is asked to do: a direction to walk (in the body's frame), and
/// whether Space is held (on a planet a climb at walking speed).
#[derive(Clone, Copy, Default)]
struct Input {
    walk: Option<DVec3>,
    climb: bool,
}

const STAND: Input = Input { walk: None, climb: false };

/// A body on the planet's ground: its eye in the body's frame (the frame lock's anchor, f64), its
/// radial speed, the controller whose falls watch the walk reports to, and every landing it has
/// handed out (m/s).
struct Walker {
    anchor: DVec3,
    v_r: f64,
    ctl: CameraController,
    landings: Vec<f32>,
}

impl Walker {
    /// The eye at `local` in `site`.
    fn at(site: &PlanetSite, local: Vec3) -> Self {
        Walker { anchor: site.to_body(local), v_r: 0.0, ctl: CameraController::new(WALK_M_S as f32, 1.0), landings: Vec::new() }
    }

    /// One frame of lib.rs's planet walk, on foot in the walk band, out of the water: the step
    /// along the ground stopped by the built pieces (`collide_on_site`), the floor under the
    /// body, one `radial_step`, and the falls hand-off. `old_floor`: the floor as it was before
    /// 2026-10-20, the terrain's alone (`rest_height`), for comparison.
    fn frame(&mut self, world: &hecs::World, reg: &BlueprintRegistry, input: Input, old_floor: bool) -> RadialStep {
        let step = WALK_M_S * DT;
        if let Some(wish) = input.walk {
            let up = self.anchor.normalize();
            let tangential = wish - up * wish.dot(up);
            let before = self.anchor;
            self.anchor += tangential.normalize() * step;
            self.anchor = collide_on_site(world, Some(reg), "earth", before, self.anchor, EYE_HEIGHT_M as f32);
        }
        let dir1 = self.anchor.normalize();
        let r0 = self.anchor.length();
        let rest = if old_floor {
            rest_height(EARTH_R, DRAWN_CLEARANCE_M)
        } else {
            walk_rest(EARTH_R, DRAWN_CLEARANCE_M, built_floor_radius(world, Some(reg), "earth", dir1, r0 - EYE_HEIGHT_M))
        };
        let wish = if input.climb { 1.0 } else { 0.0 };
        let vs = radial_step(MoveMode::Walk, true, false, r0, self.v_r, rest, G, crate::engine::frame_lock::SURFACE_SETTLE_RATE, DT, wish, step);
        self.v_r = vs.v_r;
        self.anchor = dir1 * vs.r;
        self.ctl.surface_step(r0, vs.r, vs.landing(), false, false);
        self.landings.extend(self.ctl.take_landing());
        vs
    }

    /// `n` frames of `input`.
    fn frames(&mut self, world: &hecs::World, reg: &BlueprintRegistry, input: Input, n: usize) {
        for _ in 0..n {
            self.frame(world, reg, input, false);
        }
    }

    /// The eye's height over the sphere's surface, metres.
    fn height(&self) -> f64 {
        self.anchor.length() - EARTH_R
    }
}

/// Where the eye stands on the open sand: one eye height and the drawn ground's clearance over it.
const ON_SAND: f64 = EYE_HEIGHT_M + DRAWN_CLEARANCE_M;

/// A BODY STOOD ON A BUILT ROOF STAYS ON THE ROOF. A Wood Roof on three Wood Walls on the
/// ground (3.2 m up: the walls' 3.0 and the roof's 0.2), and the eye put 0.3 m over standing
/// height on it (the dev `stand` verb's placement, as verify-falls does it): over three seconds
/// it settles onto the roof and stays at standing height there, never lower, and no landing
/// reaches the body.
///
/// Seen red 2026-10-20 with `built_floor_radius` returning None (the planet walk before this,
/// whose floor was the terrain alone): "the eye came to rest 1.750 m over the ground; standing on
/// the 3.200 m roof is 4.900 m: it went through the roof".
#[test]
fn a_body_stood_on_a_built_roof_stays_on_the_roof() {
    let reg = shipped();
    let site = site_at(23.5, 13.0804);
    let mut world = hecs::World::new();
    build_in(&mut world, &reg, &site, &open_hut(0.0, 0.0));
    let top = top_of(&world, "roof");
    assert!((top - 3.2).abs() < 1e-4, "a Wood Roof on Wood Walls stands 3.0 + 0.2 m up, not {top}");
    let on_roof = f64::from(top) + EYE_HEIGHT_M;
    let mut w = Walker::at(&site, Vec3::new(0.0, top + EYE_HEIGHT_M as f32 + 0.3, 0.5));
    let mut lowest = f64::MAX;
    for _ in 0..180 {
        w.frame(&world, &reg, STAND, false);
        lowest = lowest.min(w.height());
    }
    assert!(
        (w.height() - on_roof).abs() < 1e-3,
        "the eye came to rest {:.3} m over the ground; standing on the {top:.3} m roof is {on_roof:.3} m: it went through the roof",
        w.height()
    );
    assert!(lowest > on_roof - 1e-3, "the eye sank to {lowest:.3} m on its way onto the roof (standing there is {on_roof:.3} m)");
    assert!(w.landings.is_empty(), "settling 0.3 m onto the roof is not a fall: {:?}", w.landings);
}

/// WALKING OFF A BUILT ROOF'S EDGE IS A FALL TO THE GROUND, landing at the speed of the drop.
/// Standing on the roof of a hut open to the south, 1 m from its south edge, and walking south
/// at 5 m/s: the body walks to the edge on the roof, leaves it there, falls the 3.15 m from
/// standing on the roof to standing on the sand (the roof's 3.2 m less the sand's 0.05 m
/// clearance), and the walk hands the body exactly one landing, at the speed that drop gives
/// (v = sqrt(2 g h), within the 3% verify-falls allows the planet walk's integrator).
///
/// Seen red 2026-10-20 with `built_floor_radius` returning None: "standing on the roof before
/// the walk the eye was 3.633 m over the ground, not on the 3.200 m roof (4.900 m)" (half a
/// second after being put there it was already falling through the roof).
#[test]
fn walking_off_a_built_roofs_edge_falls_to_the_ground_and_lands_at_the_speed_of_the_drop() {
    let reg = shipped();
    let site = site_at(23.5, 13.0804);
    let mut world = hecs::World::new();
    build_in(&mut world, &reg, &site, &open_hut(0.0, 0.0));
    let top = top_of(&world, "roof");
    let on_roof = f64::from(top) + EYE_HEIGHT_M;
    let mut w = Walker::at(&site, Vec3::new(0.0, top + EYE_HEIGHT_M as f32, 1.0));
    w.frames(&world, &reg, STAND, 30);
    assert!(
        (w.height() - on_roof).abs() < 1e-3,
        "standing on the roof before the walk the eye was {:.3} m over the ground, not on the {top:.3} m roof ({on_roof:.3} m)",
        w.height()
    );
    let south = Input { walk: Some(site.basis() * DVec3::Z), climb: false };
    // The last place the body stood on the roof: within one frame's step of its south edge.
    let mut last_on = None;
    let peak = w.height();
    for _ in 0..180 {
        w.frame(&world, &reg, south, false);
        if (w.height() - on_roof).abs() < 1e-4 {
            last_on = Some(site.to_local(w.anchor).z);
        }
    }
    w.frames(&world, &reg, STAND, 60);
    let last_on = last_on.expect("the body never walked on the roof");
    let stride = (WALK_M_S * DT) as f32;
    assert!(
        last_on <= 2.0 + 1e-3 && last_on > 2.0 - stride - 1e-3,
        "the body last stood on the roof at z = {last_on:.3} m; its south edge is at 2.0 m (one frame's step is {stride:.3} m)"
    );
    assert!((w.height() - ON_SAND).abs() < 1e-3, "the body ended {:.3} m over the ground, not standing on the sand ({ON_SAND:.3} m)", w.height());
    let drop = peak - w.height();
    assert!((drop - (f64::from(top) - DRAWN_CLEARANCE_M)).abs() < 1e-3, "the eye fell {drop:.3} m");
    assert_eq!(w.landings.len(), 1, "walking off the roof is one landing, got {:?}", w.landings);
    let v = f64::from(w.landings[0]);
    let v_drop = (2.0 * G * drop).sqrt();
    assert!(
        (v - v_drop).abs() <= 0.03 * v_drop,
        "the walk landed at {v:.3} m/s; a {drop:.3} m drop at {G} m/s^2 lands at {v_drop:.3} m/s"
    );
}

/// A BUILT PIECE OUT OF STEP-UP REACH IS NOT STEPPED ONTO. The reach is the aboard walk's step up
/// (`footing::STEP_UP_M`, 0.6 m over the feet). Standing on the sand inside a hut, its roof 3.2 m
/// over the feet is not the floor, and standing still there the body stays on the sand; nor is
/// the top of a wall the body stands inside (a teleport, or a wall built around it), nor a chest's
/// (0.8 m, 0.75 m over feet held 0.05 m up by the sand's clearance). A foundation's 0.2 m top is
/// stepped onto, walking onto it as well; a top 0.59 m over the feet is in reach and one 0.61 m
/// over is not. And the reach is measured from the feet as they are: a body climbed above the
/// roof (on a planet Space is a climb with no top) has the roof as its floor.
///
/// Seen red 2026-10-20 with the reach left out of `built_floor_radius` (every top under the body
/// a floor, and in the same build no footprint test): "standing inside the hut the roof 3.2 m over
/// the feet was taken for the floor; left: Some(3.199999827891588), right: None".
#[test]
fn a_built_piece_out_of_step_up_reach_is_not_stepped_onto() {
    let reg = shipped();
    let site = site_at(23.5, 13.0804);
    let mut world = hecs::World::new();
    build_in(&mut world, &reg, &site, &open_hut(0.0, 0.0));
    build_in(&mut world, &reg, &site, &[("storage_chest", 10.0, 0.0, 0), ("wood_foundation", -10.0, 0.0, 0)]);
    let floor = |x: f32, z: f32, feet_over_ground: f64| {
        let dir = site.to_body(Vec3::new(x, 0.0, z)).normalize();
        built_floor_radius(&world, Some(&reg), "earth", dir, EARTH_R + feet_over_ground).map(|r| r - EARTH_R)
    };
    let feet = DRAWN_CLEARANCE_M; // the feet as the walk stands them on the sand (the eye less 1.7 m)
    assert_eq!(floor(0.0, 0.5, feet), None, "standing inside the hut the roof 3.2 m over the feet was taken for the floor");
    assert_eq!(floor(-2.0, 0.0, feet), None, "standing in the west wall the wall's 3.0 m top was taken for the floor");
    assert_eq!(floor(10.0, 0.0, feet), None, "standing in the chest the chest's 0.8 m top was taken for the floor");
    let found = top_of(&world, "wood_foundation");
    let on_found = floor(-10.0, 0.0, feet).expect("the foundation's top is in reach");
    assert!((on_found - f64::from(found)).abs() < 1e-4, "the foundation's top is {found}, the floor {on_found}");
    let step = f64::from(crate::engine::footing::STEP_UP_M);
    assert!(floor(-10.0, 0.0, f64::from(found) - step + 0.01).is_some(), "a top 0.59 m over the feet is in reach");
    assert_eq!(floor(-10.0, 0.0, f64::from(found) - step - 0.01), None, "a top 0.61 m over the feet is out of reach");
    let top = top_of(&world, "roof");
    let over = floor(0.0, 0.5, f64::from(top) + 0.8).expect("a body climbed over the roof has it for its floor");
    assert!((over - f64::from(top)).abs() < 1e-4, "the roof's top is {top}, the floor {over}");

    // In the walk: standing in the hut the body stays on the sand, and walking west from the
    // sand onto the foundation steps it up onto the foundation's top.
    let mut w = Walker::at(&site, Vec3::new(0.0, ON_SAND as f32, 0.5));
    w.frames(&world, &reg, STAND, 120);
    assert!((w.height() - ON_SAND).abs() < 1e-6, "standing in the hut the eye rose to {:.3} m", w.height());
    let mut w = Walker::at(&site, Vec3::new(-6.0, ON_SAND as f32, 0.0));
    w.frames(&world, &reg, STAND, 10);
    w.frames(&world, &reg, Input { walk: Some(site.basis() * DVec3::NEG_X), climb: false }, 60);
    assert!(
        (w.height() - (f64::from(found) + EYE_HEIGHT_M)).abs() < 1e-3,
        "walking onto the foundation the eye ended {:.3} m up, not standing on its {found} m top",
        w.height()
    );
    assert!(w.landings.is_empty(), "stepping up is not a fall: {:?}", w.landings);
}

/// TERRAIN WITH NOTHING BUILT IS UNCHANGED. With no built top `walk_rest` is the terrain's
/// `rest_height` to the bit. And in worlds where nothing built is under the body (nothing built
/// at all; a hut 200 m away in the same site; a foundation on the Moon at the same direction; a
/// foundation 2 km away; a foundation the body walks past 0.3 m from its edge), a walk on the
/// sand, a climb on Space and the fall back are frame for frame, to the bit, the walk as it was
/// before 2026-10-20 (the terrain's floor alone), and the same one landing reaches the body.
///
/// Seen red 2026-10-20 with the footprint test left out of `built_floor_radius` (any piece of the
/// site a floor wherever the body is; the same build also had no reach): "a hut 200 m away: frame 0
/// of the walk left the eye somewhere else; left: 4.90, right: 1.75". The same build failed
/// walking off the roof too, standing "on the roof" 14 m past its edge.
#[test]
fn terrain_with_nothing_built_is_unchanged() {
    for (g, c) in [(EARTH_R, DRAWN_CLEARANCE_M), (EARTH_R - 412.25, 2.5), (1_737_400.0, 0.05), (0.0, 0.0)] {
        assert_eq!(walk_rest(g, c, None).to_bits(), rest_height(g, c).to_bits(), "walk_rest({g}, {c}, None)");
    }
    let reg = shipped();
    let site = site_at(23.5, 13.0804);
    let far_site = site_at(23.5, 13.1);
    let moon = PlanetSite { body: "moon".into(), origin: site.origin.normalize() * 1_737_400.0 };
    let worlds: Vec<(&str, hecs::World)> = vec![
        ("nothing built", hecs::World::new()),
        ("a hut 200 m away", {
            let mut w = hecs::World::new();
            build_in(&mut w, &reg, &site, &open_hut(200.0, 0.0));
            w
        }),
        ("a foundation on the Moon", {
            let mut w = hecs::World::new();
            build_in(&mut w, &reg, &moon, &[("wood_foundation", 0.0, 0.0, 0)]);
            w
        }),
        ("a foundation 2 km away", {
            let mut w = hecs::World::new();
            build_in(&mut w, &reg, &far_site, &[("wood_foundation", 0.0, 0.0, 0)]);
            w
        }),
        ("a foundation 0.3 m beside the path", {
            let mut w = hecs::World::new();
            // Placement snaps x and z to the metre grid, so it is built at x = -2: x -4 to 0.
            build_in(&mut w, &reg, &site, &[("wood_foundation", -2.0, 0.0, 0)]);
            w
        }),
    ];
    let north = Input { walk: Some(site.basis() * DVec3::NEG_Z), climb: false };
    let climb = Input { walk: None, climb: true };
    let script: Vec<Input> = std::iter::repeat(STAND)
        .take(10)
        .chain(std::iter::repeat(north).take(90))
        .chain(std::iter::repeat(climb).take(25))
        .chain(std::iter::repeat(STAND).take(150))
        .collect();
    for (name, world) in &worlds {
        // The path runs north along x = 0.3 from z = 2, 0.3 m east of the beside foundation's east edge (x = 0).
        let start = Vec3::new(0.3, ON_SAND as f32, 2.0);
        let (mut now, mut before) = (Walker::at(&site, start), Walker::at(&site, start));
        for (k, input) in script.iter().enumerate() {
            now.frame(world, &reg, *input, false);
            before.frame(world, &reg, *input, true);
            assert_eq!(
                (now.anchor.to_array(), now.v_r.to_bits()),
                (before.anchor.to_array(), before.v_r.to_bits()),
                "{name}: frame {k} of the walk left the eye somewhere else; left: {:.2}, right: {:.2}",
                now.height(),
                before.height()
            );
        }
        assert_eq!(now.landings, before.landings, "{name}: a different landing");
        assert_eq!(now.landings.len(), 1, "{name}: the climb's fall back lands once");
        let dir = now.anchor.normalize();
        assert_eq!(built_floor_radius(world, Some(&reg), "earth", dir, now.height() + EARTH_R - EYE_HEIGHT_M), None, "{name}");
    }
}
