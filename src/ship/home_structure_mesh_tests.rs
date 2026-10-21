//! Mesh tests for the storey work of ship homes increment 7 (lane G,
//! docs/design/ship-homes-increment-7-plan.md section 3 "Geometry"): floor slabs drawn as boxes,
//! walls built from their base, joins only between walls that share height, guards and handrails,
//! and the one-storey bodies drawing exactly what they drew before storeys. Each test was seen to
//! fail before it passed; the failure is in its comment. "With main's ... put back" means one run
//! on 2026-10-20 with a scripted patch restoring main's behaviour at every point the lane changed
//! (21 one-line edits across this file's module, door_panels.rs, door_anim.rs, ship_space.rs,
//! conduits.rs and engine/home_meshes.rs), every new test red in it, and the files then restored
//! byte for byte.
//!
//! Kept beside home_structure.rs (`#[path]`) so the mesh tests do not grow that file further.

use super::*;
use crate::ship::fibonacci::HomesteadMeshes;

/// FNV-1a, 64 bit, over a byte stream.
fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Every byte of a vertex: position, normal and uv, as their f32 bits.
fn vertex_bytes(v: &Vertex) -> Vec<u8> {
    v.position.iter().chain(v.normal.iter()).chain(v.uv.iter()).flat_map(|f| f.to_bits().to_le_bytes()).collect()
}

/// A mesh's (vertex count, index count, sum of its triangles' hashes). Each triangle's hash covers
/// every byte of its three corners in winding order, so it changes with any vertex and with a
/// flipped face; the SUM does not depend on the order the triangles are listed in. That order is
/// not stable on main: it merged the corner columns out of a HashMap, whose order differs every run.
fn geometry_sig(v: &[Vertex], i: &[u32]) -> (usize, usize, u64) {
    let mut sum = 0u64;
    for t in i.chunks(3) {
        let h = fnv(t.iter().flat_map(|&k| vertex_bytes(&v[k as usize])));
        sum = sum.wrapping_add(h);
    }
    (v.len(), i.len(), sum)
}

/// The bits of a colour, so two colours compare exactly.
fn rgba_bits(c: &[f32]) -> String {
    c.iter().map(|f| format!("{:08x}", f.to_bits())).collect::<Vec<_>>().join(",")
}

/// One line per mesh group of `m`, sorted: its family, colour (bits), material type, vertex and
/// index counts and triangle sum.
fn signatures(m: &HomesteadMeshes) -> Vec<String> {
    let mut out = Vec::new();
    for (v, i, c, t) in &m.floors {
        let (nv, ni, h) = geometry_sig(v, i);
        out.push(format!("floor {} type {t} v {nv} i {ni} h {h:016x}", rgba_bits(c)));
    }
    for (v, i, c) in &m.material_walls {
        let (nv, ni, h) = geometry_sig(v, i);
        out.push(format!("material_wall {} v {nv} i {ni} h {h:016x}", rgba_bits(c)));
    }
    for (name, (v, i)) in [
        ("walls", &m.walls),
        ("trim", &m.trim),
        ("windows", &m.windows),
        ("mirrors", &m.mirrors),
        ("ceilings", &m.ceilings),
        ("ceilings_opaque", &m.ceilings_opaque),
    ] {
        let (nv, ni, h) = geometry_sig(v, i);
        out.push(format!("{name} v {nv} i {ni} h {h:016x}"));
    }
    out.sort();
    out
}

/// `signatures` for a neighbour's baked shell (`bake_shell_groups`).
fn shell_signatures(groups: &[(Vec<Vertex>, Vec<u32>, [f32; 3])]) -> Vec<String> {
    let mut out: Vec<String> = groups
        .iter()
        .map(|(v, i, c)| {
            let (nv, ni, h) = geometry_sig(v, i);
            format!("shell {} v {nv} i {ni} h {h:016x}", rgba_bits(c))
        })
        .collect();
    out.sort();
    out
}

/// (total vertices, total indices, one hash of every group line): what a pin compares.
fn digest(lines: &[String]) -> (usize, usize, u64) {
    let num = |l: &str, key: &str| -> usize {
        let mut it = l.split_whitespace();
        while let Some(w) = it.next() {
            if w == key {
                return it.next().and_then(|n| n.parse().ok()).unwrap_or(0);
            }
        }
        0
    };
    let v = lines.iter().map(|l| num(l, "v")).sum();
    let i = lines.iter().map(|l| num(l, "i")).sum();
    (v, i, fnv(lines.join("\n").bytes()))
}

/// The frozen one-storey bodies (tests/fixtures/homes/one_storey_*.ron, byte copies of the shipped
/// designs before storeys), with their placed pieces taken out: those are drawn from
/// data/blueprints/structure_types.ron, whose stairs row changes in this wave on purpose (the
/// finding's 16 risers), and none of the storey work changes how a piece is drawn (a stair's
/// handrails are a group of their own, `the_stairs_carry_handrails_at_the_findings_height`).
fn frozen_bodies() -> Vec<(&'static str, HomeStructure, ShellCut)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/homes");
    let load = |name: &str| -> HomeStructure {
        let text = std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let design: crate::ship::ship_structure::HomeDesign = ron::from_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
        let mut body = design.body;
        body.structures.clear();
        body
    };
    vec![
        ("homestead", load("one_storey_homestead.ron"), ShellCut { edge: 1, at: 39.0, width: 2.0, base: 0.0, height: 2.2 }),
        ("cabin", load("one_storey_cabin.ron"), ShellCut { edge: 0, at: 1.0, width: 0.9, base: 0.0, height: 2.1 }),
    ]
}

/// Every case the pin covers for one body: its meshes, its meshes with a door cut through the shell,
/// and its baked neighbour shell with that door.
fn cases(body: &HomeStructure, cut: ShellCut) -> Vec<(&'static str, Vec<String>)> {
    vec![
        ("meshes", signatures(&body.generate_meshes())),
        ("meshes with a door", signatures(&body.generate_meshes_with_shell_cuts(&[cut]))),
        ("neighbour shell", shell_signatures(&body.bake_shell_groups(&[cut]))),
    ]
}

/// PRINTS the pin (`one_storey_bodies_draw_what_they_drew`) from the code as it stands. Run it on
/// main before a geometry change: `node scripts/lib-tests.js print_one_storey_pin --ignored
/// --nocapture`.
#[test]
#[ignore = "prints the one-storey mesh pin; run: node scripts/lib-tests.js print_one_storey_pin --ignored --nocapture"]
fn print_one_storey_pin() {
    for (name, body, cut) in frozen_bodies() {
        for (case, lines) in cases(&body, cut) {
            let (v, i, h) = digest(&lines);
            println!("PIN (\"{name}\", \"{case}\", {v}, {i}, 0x{h:016x}),");
            for l in &lines {
                println!("    {l}");
            }
        }
    }
}

/// THE ONE-STOREY BODIES DRAW WHAT THEY DREW (plan section 3: storeys change nothing for a body with
/// no slab). The frozen shipped homestead and cabin (`frozen_bodies`) draw, byte for byte, the
/// meshes main drew from them before the storey work: their meshes, their meshes with a door cut
/// through the shell, and their neighbour shells. Pinned from main at v0.1533.0 (8661940b8) with
/// `print_one_storey_pin`, and the same twice in a row (the sums do not depend on the order the
/// corner columns come out in).
///
/// Seen red 2026-10-20 with the corner columns drawn with 11 sides instead of 10 (the pin's own
/// control): "homestead, meshes: the meshes are not what main drew".
#[test]
fn one_storey_bodies_draw_what_they_drew() {
    const PINS: &[(&str, &str, usize, usize, u64)] = &[
        ("homestead", "meshes", 8184, 12546, 0x679f0c8db36f531e),
        ("homestead", "meshes with a door", 8280, 12690, 0x7b454cbf5eaa1328),
        ("homestead", "neighbour shell", 5844, 9036, 0x2f3670c91e35aa3c),
        ("cabin", "meshes", 200, 300, 0x82bf356c52b496dd),
        ("cabin", "meshes with a door", 296, 444, 0xe6ec84257a2c7ee0),
        ("cabin", "neighbour shell", 296, 444, 0x679c744f6a621fed),
    ];
    let mut checked = 0;
    for (name, body, cut) in frozen_bodies() {
        assert!(body.floors.is_empty() && body.walls.iter().all(|w| w.base == 0.0), "{name} is not one storey");
        for (case, lines) in cases(&body, cut) {
            let got = digest(&lines);
            let want = PINS.iter().find(|p| p.0 == name && p.1 == case).unwrap_or_else(|| panic!("no pin for {name}, {case}"));
            assert_eq!(got, (want.2, want.3, want.4), "{name}, {case}: the meshes are not what main drew:\n{}", lines.join("\n"));
            checked += 1;
        }
    }
    assert_eq!(checked, PINS.len());
}

/// A body to test with: `width` x `depth` x `height`, steel shell, glass roof, nothing in it.
fn bare(width: f32, depth: f32, height: f32) -> HomeStructure {
    ron::from_str(&format!("(width: {width}, depth: {depth}, height: {height})")).expect("a bare body parses")
}

/// A solid 0.25 m slab over the rectangle a-b with its top at `top`.
fn slab(a: (f32, f32), b: (f32, f32), top: f32) -> FloorSlab {
    FloorSlab { a, b, top, thickness: 0.25, material: SlabMaterial::Solid, label: None }
}

/// A wall from `a` to `b`, standing on `base`, `height` tall, of `material`.
fn wall_on(a: (f32, f32), b: (f32, f32), base: f32, height: f32, material: u32) -> InteriorWall {
    InteriorWall { a, b, base, height, material, openings: Vec::new(), thickness: None, layers: Vec::new() }
}

/// Every triangle of a mesh: its three corners and the normal its first corner declares.
fn triangles(v: &[Vertex], i: &[u32]) -> Vec<([Vec3; 3], Vec3)> {
    i.chunks(3)
        .map(|t| {
            let p = |k: usize| Vec3::from(v[t[k] as usize].position);
            ([p(0), p(1), p(2)], Vec3::from(v[t[0] as usize].normal))
        })
        .collect()
}

/// True when a triangle is wound the way its normal says (the opaque pass draws a triangle only
/// from the side its winding faces).
fn faces_its_normal(c: &[Vec3; 3], n: Vec3) -> bool {
    (c[1] - c[0]).cross(c[2] - c[0]).dot(n) > 0.0
}

/// The area of the flat faces of a mesh lying at height `y` (every corner within 1 mm) whose normal
/// points up (`up`) or down, counting only triangles wound the way they face.
fn flat_area_at(v: &[Vertex], i: &[u32], y: f32, up: bool) -> f32 {
    triangles(v, i)
        .into_iter()
        .filter(|(c, n)| c.iter().all(|p| (p.y - y).abs() < 1e-3) && n.y.abs() > 0.99 && (n.y > 0.0) == up && faces_its_normal(c, *n))
        .map(|(c, _)| (c[1] - c[0]).cross(c[2] - c[0]).length() * 0.5)
        .sum()
}

/// A FLOOR SLAB IS A BOX DRAWN IN THE FLOORS (plan section 3, "Geometry"): a 10 x 5 m slab with its
/// top at 3.0 and 0.25 m thick, in a 6 m body, is drawn as an upward face over its whole rectangle at
/// its top (lifted `SLAB_DRAW_LIFT_M`, so the walls rising to 3.0 do not flicker through it), a
/// downward face at its underside (2.75), so the room under it sees a ceiling (the opaque pass culls
/// back faces, and the floor quad faces up only), and its four edges; all of it in the floor meshes
/// and none in the ceilings, which the roof toggle hides. Every face is wound the way it faces. The
/// top is the floor's own material (joined to the ground floor's entry), the underside and edges
/// the opaque ceiling's. A glass slab is the same box in the glass roof's colour, each face so
/// transparent that its two faces together blend as one pane of the roof.
///
/// Seen red 2026-10-20 with main's drawing put back (no slab drawn): "no upward face at the slab's top:
/// -0 m2 of 50".
#[test]
fn a_floor_slab_is_a_box_drawn_in_the_floors() {
    let mut body = bare(10.0, 10.0, 6.0);
    body.floors.push(slab((0.0, 0.0), (10.0, 5.0), 3.0));
    let m = body.generate_meshes();
    let top = 3.0 + SLAB_DRAW_LIFT_M;
    let up: f32 = m.floors.iter().map(|(v, i, _, _)| flat_area_at(v, i, top, true)).sum();
    assert!((up - 50.0).abs() < 1e-3, "no upward face at the slab's top: {up} m2 of 50");
    let down: f32 = m.floors.iter().map(|(v, i, _, _)| flat_area_at(v, i, 2.75, false)).sum();
    assert!((down - 50.0).abs() < 1e-3, "no downward face at the slab's underside: {down} m2 of 50");
    let (fv, fi, fc, ft) = &m.floors[0];
    assert!(flat_area_at(fv, fi, top, true) > 49.9, "the slab's top is not in the floor's own entry");
    assert_eq!((*fc, *ft), (HomeStructure::material_color(body.shell_material), body.shell_material));
    let under = m.floors.iter().find(|(v, i, _, _)| flat_area_at(v, i, 2.75, false) > 49.9).expect("an underside entry");
    assert_eq!((under.2, under.3), (OPAQUE_CEILING_RGBA, OPAQUE_CEILING_TYPE), "the underside is the opaque ceiling's look");
    // The edges: vertical faces from the underside to the drawn top, facing out, 2 x (10 + 5) m long.
    let edges: f32 = m
        .floors
        .iter()
        .flat_map(|(v, i, _, _)| triangles(v, i))
        .filter(|(c, n)| n.y.abs() < 1e-3 && c.iter().all(|p| p.y >= 2.75 - 1e-3 && p.y <= top + 1e-3) && faces_its_normal(c, *n))
        .map(|(c, _)| (c[1] - c[0]).cross(c[2] - c[0]).length() * 0.5)
        .sum();
    assert!((edges - 30.0 * (top - 2.75)).abs() < 1e-3, "the slab's edges: {edges} m2");
    // None of it is a ceiling: the ceilings hold only the roof, at 6 m.
    for (name, (v, _)) in [("ceilings", &m.ceilings), ("ceilings_opaque", &m.ceilings_opaque)] {
        assert!(v.iter().all(|p| (p.position[1] - 6.0).abs() < 1e-4), "the {name} hold something under the roof");
    }
    // A glass slab: the same box, in an entry of its own, in the glass roof's look.
    body.floors[0].material = SlabMaterial::Glass;
    let m = body.generate_meshes();
    let glass = m.floors.iter().find(|(_, _, c, _)| c[3] < 1.0).expect("a glass entry");
    assert_eq!((&glass.2[..3], glass.3), (&GLASS_ROOF_RGBA[..3], GLASS_ROOF_TYPE), "the glass roof's colour and type");
    assert!((flat_area_at(&glass.0, &glass.1, top, true) - 50.0).abs() < 1e-3);
    assert!((flat_area_at(&glass.0, &glass.1, 2.75, false) - 50.0).abs() < 1e-3);
    let a = glass.2[3];
    assert!(((1.0 - (1.0 - a) * (1.0 - a)) - GLASS_ROOF_RGBA[3]).abs() < 1e-4, "two faces of alpha {a} are not one pane of the roof");
    assert!(flat_area_at(&m.floors[0].0, &m.floors[0].1, top, true).abs() < 1e-6, "a glass slab's top is in the opaque floor");
}

/// A WALL UPSTAIRS STANDS ON ITS BASE (plan section 3): a wall with base 3.0 and height 3.0, with a
/// door and a window in it, draws every vertex between 3.0 and 6.0: its piers, the window's sill
/// panel (3.0 to 3.9) and the headers over both openings (from 5.1 and 5.2 up to 6.0).
///
/// Seen red 2026-10-20 with main's drawing put back (every wall rose from 0): "a vertex of the upstairs wall at y 0".
#[test]
fn a_wall_upstairs_stands_on_its_base() {
    let mut body = bare(10.0, 10.0, 6.0);
    let mut w = wall_on((1.0, 5.0), (9.0, 5.0), 3.0, 3.0, 3);
    let opening = |kind, at, sill, height| Opening {
        kind,
        at,
        width: 1.0,
        sill,
        height,
        style: "swing".into(),
        open_dist: 2.6,
        locked: false,
        auto_open: true,
        control_panel: false,
        locks: Vec::new(),
    };
    w.openings = vec![opening(OpeningKind::Door, 1.0, 0.0, 2.1), opening(OpeningKind::Window, 4.0, 0.9, 1.3)];
    body.walls.push(w);
    let m = body.generate_meshes();
    let oak = m.material_walls.iter().find(|(_, _, c)| *c == HomeStructure::material_color(3)).expect("the oak wall's group");
    for v in &oak.0 {
        assert!(v.position[1] >= 3.0 - 1e-5 && v.position[1] <= 6.0 + 1e-5, "a vertex of the upstairs wall at y {}", v.position[1]);
    }
    let ys: std::collections::BTreeSet<i32> = oak.0.iter().map(|v| (v.position[1] * 100.0).round() as i32).collect();
    for want in [300, 390, 510, 520, 600] {
        assert!(ys.contains(&want), "no vertex at {} m: {ys:?}", want as f32 / 100.0);
    }
}

/// Every vertex x of the material group of `material` lying on the line z = `z` (to 1 mm).
fn xs_at(m: &HomesteadMeshes, material: u32, z: f32) -> Vec<f32> {
    let c = HomeStructure::material_color(material);
    m.material_walls
        .iter()
        .filter(|(_, _, k)| *k == c)
        .flat_map(|(v, _, _)| v.iter())
        .filter(|v| (v.position[2] - z).abs() < 1e-3)
        .map(|v| v.position[0])
        .collect()
}

/// WALLS ON DIFFERENT STOREYS DO NOT JOIN (plan section 3: mitres, clips and corner columns join only
/// walls whose heights overlap). A ground wall (concrete) along z 0 from x 0 to 5 and an upstairs wall
/// (oak) from (5, 0) to (5, 5) share the corner (5, 0) and are not mitred, so the ground wall's end
/// stays square at x 5 (a mitre carries its outer face out to 5 + the oak wall's half thickness). A
/// ground wall ending on the face of an upstairs wall is not clipped back to that face. And of four
/// walls at one corner, two ground and two upstairs, no column rises: each storey has an ordinary
/// two-wall corner there. The same three cases on one storey join, so the test can tell.
///
/// Seen red 2026-10-20 with main's joins put back: "the ground wall's end was mitred into the upstairs wall: its face
/// reaches x 5.05".
#[test]
fn walls_on_different_storeys_are_not_mitred_together() {
    let mut body = bare(10.0, 10.0, 6.0);
    body.walls.push(wall_on((0.0, 0.0), (5.0, 0.0), 0.0, 3.0, 2));
    body.walls.push(wall_on((5.0, 0.0), (5.0, 5.0), 3.0, 3.0, 3));
    let reach = |b: &HomeStructure| {
        let m = b.generate_meshes();
        xs_at(&m, 2, -0.075).into_iter().chain(xs_at(&m, 2, 0.075)).fold(f32::MIN, f32::max)
    };
    assert!(reach(&body) <= 5.0 + 1e-4, "the ground wall's end was mitred into the upstairs wall: its face reaches x {}", reach(&body));
    body.walls[1].base = 0.0;
    assert!(reach(&body) > 5.0 + 0.04, "two ground walls at a corner are mitred: {}", reach(&body));

    // A T: a ground wall ending at x 5 on the face of a wall along x 5. On one storey it is clipped
    // back to that face; under an upstairs wall it is not.
    let mut t = bare(10.0, 10.0, 6.0);
    t.walls.push(wall_on((0.0, 3.0), (5.0, 3.0), 0.0, 3.0, 2));
    t.walls.push(wall_on((5.0, 0.0), (5.0, 6.0), 3.0, 3.0, 3));
    let end = |b: &HomeStructure| xs_at(&b.generate_meshes(), 2, 3.075).into_iter().fold(f32::MIN, f32::max);
    assert!((end(&t) - 5.0).abs() < 1e-4, "a ground wall was clipped to the face of an upstairs wall: it ends at {}", end(&t));
    t.walls[1].base = 0.0;
    assert!(end(&t) < 5.0 - 0.04, "a ground wall is clipped to a ground wall's face: it ends at {}", end(&t));

    // Four walls at (5, 5): two ground, two upstairs. A corner column is a 10-sided cylinder of
    // the walls' half thickness (0.075 m); count its vertices off the wall lines.
    let mut x = bare(10.0, 10.0, 6.0);
    for (far, base) in [((0.0, 5.0), 0.0), ((5.0, 0.0), 0.0), ((10.0, 5.0), 3.0), ((5.0, 10.0), 3.0)] {
        x.walls.push(wall_on(far, (5.0, 5.0), base, 3.0, 2));
    }
    let column = |b: &HomeStructure| {
        b.generate_meshes()
            .material_walls
            .iter()
            .flat_map(|(v, _, _)| v.iter())
            .filter(|v| {
                let (dx, dz) = (v.position[0] - 5.0, v.position[2] - 5.0);
                ((dx * dx + dz * dz).sqrt() - 0.075).abs() < 1e-4 && dx.abs() > 0.01 && dz.abs() > 0.01
            })
            .count()
    };
    assert_eq!(column(&x), 0, "a corner column joins the walls of two storeys");
    for w in x.walls.iter_mut() {
        w.base = 0.0;
    }
    assert!(column(&x) > 0, "four ground walls at a corner get their column");
}

/// The x-extents (left face, right face) of every vertical member crossing height `y` of a guard
/// running along x: its posts and balusters, read from the drawn faces (one looking -x is a
/// member's left side, one looking +x its right), wound the way they face.
fn members_at(v: &[Vertex], i: &[u32], y: f32) -> Vec<(f32, f32)> {
    let (mut left, mut right): (Vec<f32>, Vec<f32>) = (Vec::new(), Vec::new());
    for (c, n) in triangles(v, i) {
        let lo = c.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let hi = c.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        if n.x.abs() < 0.99 || lo > y || hi < y || !faces_its_normal(&c, n) {
            continue;
        }
        let list = if n.x < 0.0 { &mut left } else { &mut right };
        if !list.iter().any(|x| (x - c[0].x).abs() < 1e-4) {
            list.push(c[0].x);
        }
    }
    left.sort_by(f32::total_cmp);
    right.sort_by(f32::total_cmp);
    left.into_iter().zip(right).collect()
}

/// A GUARD'S BALUSTERS ARE CLOSER THAN THE FINDING'S SPHERE (plan decision 8;
/// docs/reference/findings/2026-10-20-stairs-guards-and-headroom.md: "a 100 mm (3.9 in) sphere must
/// not pass", with "no climbable horizontal rails"). A 4 m wall of Steel balustrade standing on the
/// upper floor (base 3.0), 1.0 m tall, is drawn as a guard: upright posts and balusters whose every
/// clear gap at waist height is under 100 mm, a post flush with each end, a top rail along its whole
/// length whose top is the wall's top (1.0 m over the floor, over the IRC's 914 mm), and nothing
/// under its base or over its top. It is one mesh group, not a draw per baluster.
///
/// Seen red 2026-10-20 with main's drawing put back (no guard; the wall drawn solid): "the guard has 0 upright
/// members at 3.5 m, want at least 33".
#[test]
fn a_guards_balusters_are_closer_than_the_findings_sphere() {
    let mut body = bare(10.0, 10.0, 6.0);
    // A concrete shell: a steel one is the Steel balustrade's colour, and would share its group.
    body.shell_material = 2;
    body.walls.push(wall_on((1.0, 5.0), (5.0, 5.0), 3.0, 1.0, 9));
    let m = body.generate_meshes();
    let colour = HomeStructure::material_color(9);
    let groups: Vec<_> = m.material_walls.iter().filter(|(_, _, c)| *c == colour).collect();
    assert_eq!(groups.len(), 1, "the guard is one mesh group");
    let (v, i, _) = groups[0];
    let members = members_at(v, i, 3.5);
    // 4 m with a member at least every 120 mm (a 20 mm baluster and a gap under 100 mm).
    assert!(members.len() as f32 >= 4.0 / 0.12, "the guard has {} upright members at 3.5 m, want at least 33", members.len());
    let (first, last) = (members[0], members[members.len() - 1]);
    assert!((first.0 - 1.0).abs() < 1e-3 && (last.1 - 5.0).abs() < 1e-3, "a post flush with each end: {first:?} .. {last:?}");
    for w in members.windows(2) {
        let gap = w[1].0 - w[0].1;
        assert!(gap > 0.0 && gap < 0.100, "a {gap:.4} m gap between members at x {:.3}", w[0].1);
    }
    let lo = v.iter().map(|p| p.position[1]).fold(f32::MAX, f32::min);
    let hi = v.iter().map(|p| p.position[1]).fold(f32::MIN, f32::max);
    assert!((lo - 3.0).abs() < 1e-4 && (hi - 4.0).abs() < 1e-4, "the guard stands from {lo} to {hi}, not 3.0 to 4.0");
    let spec = crate::ship::structure::structure_type("guard").and_then(|t| t.guard).expect("the guard row");
    let rail_top = flat_area_at(v, i, 4.0, true);
    assert!((rail_top - 4.0 * spec.rail.0).abs() < 1e-3, "the top rail's top: {rail_top} m2");
    // The row itself meets the finding: a sphere of 100 mm, and a guard row at least 914 mm tall.
    assert!(spec.opening_m <= 0.100 && crate::ship::structure::structure_type("guard").unwrap().size.1 >= 0.914);
}

/// A NEIGHBOUR'S SHELL DRAWS ITS SLABS (plan section 3: "the neighbour bake keeps slabs"). A body
/// with a solid slab and a glass one bakes both into the shell a neighbour's home is drawn as: each
/// slab's top and underside are there, the glass one drawn solid (a neighbour's shell is opaque, its
/// roof too: `bake_shell_groups`).
///
/// Seen red 2026-10-20 with main's drawing put back (no slab drawn): "the neighbour's shell has -0 m2 at the
/// slabs' top, want 100".
#[test]
fn a_neighbours_shell_draws_its_slabs() {
    let mut body = bare(10.0, 10.0, 6.0);
    body.floors.push(slab((0.0, 0.0), (10.0, 5.0), 3.0));
    let mut glass = slab((0.0, 5.0), (10.0, 10.0), 3.0);
    glass.material = SlabMaterial::Glass;
    body.floors.push(glass);
    let shell = body.bake_shell_groups(&[]);
    let area = |y: f32, up: bool| shell.iter().map(|(v, i, _)| flat_area_at(v, i, y, up)).sum::<f32>();
    let top = 3.0 + SLAB_DRAW_LIFT_M;
    assert!((area(top, true) - 100.0).abs() < 1e-3, "the neighbour's shell has {} m2 at the slabs' top, want 100", area(top, true));
    assert!((area(2.75, false) - 100.0).abs() < 1e-3, "the neighbour's shell has {} m2 at the slabs' underside, want 100", area(2.75, false));
}

/// THE STAIRS CARRY HANDRAILS AT THE FINDING'S HEIGHT (plan decision 8; the finding's "Handrail" row:
/// 900 to 965 mm above the nosing line for both codes together, on both sides at 1000 mm wide and
/// over; the IRC's R311.7.8.3 has the ends returned). A stairs piece draws its rails in a group of
/// their own, in the `handrail` row's colour (so the stair's own group is unchanged), on both sides of
/// a stair over 1 m wide, the top of each rail the `handrail` row's height over the line through the
/// treads' nosings, every face wound the way it faces.
///
/// Seen red 2026-10-20 with main's drawing put back (no handrails drawn): "no group in the handrail's colour".
#[test]
fn the_stairs_carry_handrails_at_the_findings_height() {
    use crate::ship::structure::{handrail_type, structure_type};
    let (rail, spec) = handrail_type().expect("the handrail row and its spec");
    assert!((0.900..=0.965).contains(&rail.size.1), "the handrail row's height {} is outside both codes", rail.size.1);
    let stairs = structure_type("stairs").expect("the stairs row");
    let (w, h, d) = stairs.size;
    let n = stairs.steps.max(1) as f32;
    let (riser, tread) = (h / n, d / n);
    assert!(w >= spec.both_sides_from_m, "this test wants a stair wide enough for two rails");
    let mut body = bare(20.0, 20.0, 6.0);
    body.structures.push(PlacedStructure { id: "stairs-1".into(), type_id: "stairs".into(), pos: (10.0, 0.0, 10.0), rot_deg: 0.0, pair: None });
    let m = body.generate_meshes();
    let colour = [rail.color.0, rail.color.1, rail.color.2, 1.0];
    let (gv, gi, _) = m.material_walls.iter().find(|(_, _, c)| *c == colour).expect("no group in the handrail's colour");
    for (c, n) in triangles(gv, gi) {
        let g = (c[1] - c[0]).cross(c[2] - c[0]);
        assert!(g.length() < 1e-9 || g.dot(n) > 0.0, "an inside-out handrail face at {:?}", c[0]);
    }
    // The nosing line, in the body's metres: the first tread's front edge is at z 10 - d / 2, one
    // riser up, and the line climbs a riser a tread.
    let nosing = |z: f32| riser + (z - (10.0 - d * 0.5)) * riser / tread;
    for side in [-1.0f32, 1.0] {
        let x = 10.0 + side * (w * 0.5 - spec.clearance_m - spec.diameter_m * 0.5);
        let over: Vec<f32> = gv
            .iter()
            .map(|v| Vec3::from(v.position))
            .filter(|p| (p.x - x).abs() < spec.diameter_m * 0.5 + 1e-3)
            .map(|p| p.y - nosing(p.z))
            .collect();
        assert!(!over.is_empty(), "no handrail on the {side} side of a {w} m stair");
        let top = over.iter().copied().fold(f32::MIN, f32::max);
        assert!((top - rail.size.1).abs() < 1e-3, "the {side} rail's top stands {top:.4} m over the nosing line, want {}", rail.size.1);
    }
}
