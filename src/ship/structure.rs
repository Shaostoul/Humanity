//! Structural building pieces (v0.583): the data-driven registry for every NON-machine, NON-wall
//! buildable element -- stairs, ramps, ladders, elevators, teleporters, train platforms, roads, ...
//!
//! Why a registry (infinite-of-X): the operator wants "stairs, ladders, elevators, teleporters,
//! trains, roads, etc." and "pretty much anything you can think of." Rather than a hand-written mesh
//! + placement path per type, every structural element is ONE entry in
//! `data/blueprints/structure_types.ron`: an id, a label, a palette category, a semantic `kind`
//! (which drives future behaviour -- climbing a ladder, riding an elevator), a parametric `shape`
//! (which drives the placeholder geometry built here), a footprint size, and a colour. Add a new
//! buildable by adding one line to the .ron -- no code. The construction palette renders the same
//! list an AI can enumerate.
//!
//! Stage 1 (this file + v0.583): the registry + a parametric mesh builder so each piece is
//! PLACEABLE and VISIBLE with distinguishable geometry, plus a bounds gizmo. FUNCTION (walking up
//! the stairs, riding the elevator, teleporting) is wired in v0.584 -- the `kind` tag is the hook.

use crate::renderer::mesh::Vertex;
use glam::Vec3;
use serde::Deserialize;

/// What a structural piece DOES -- the semantic tag the gameplay layer reads (v0.584+). Geometry is
/// the separate `shape`; two pieces can share a shape but differ in kind (a ramp and stairs both let
/// you ascend, a teleporter and an elevator both use the Frame shape).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum StructureKind {
    /// Interior wall -- special-cased: selecting it in the palette enters the wall-DRAW tool rather
    /// than dropping a placed piece (walls are segment chains, not point placements).
    Wall,
    /// Ascend on foot -- stairs or a ramp. The ground-height sampler (v0.584) walks the player up it.
    Stairs,
    /// Climb vertically -- a ladder.
    Ladder,
    /// Ride between levels -- an elevator car/shaft.
    Elevator,
    /// Step in to jump to a paired pad.
    Teleporter,
    /// Board a rail vehicle -- a platform (the line itself is a later stage).
    Train,
    /// A drive/walk surface -- a road or path segment (gets a layered material stack in v0.585).
    Road,
}

/// The parametric placeholder GEOMETRY for a piece. Each maps to a builder below so a new shape is a
/// new match arm, not a new bespoke module. `size` is interpreted (w = X span, h = Y height, d = Z
/// depth) per shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum MeshShape {
    /// A solid box filling the footprint.
    Box,
    /// A staircase of `steps` discrete columns climbing in +Z (each a full-height column from the
    /// floor, so it is solid and walkable once vertical collision lands).
    Steps,
    /// A smooth inclined wedge rising from y=0 at the near (-Z) edge to y=h at the far (+Z) edge.
    Ramp,
    /// Two vertical rails + horizontal rungs (a ladder), spanning the X width, rising in Y.
    Ladder,
    /// A hollow doorway frame: two side posts + a top lintel (an elevator shaft / teleporter arch).
    Frame,
    /// A thin flat slab on the floor (a road / pad surface), height clamped thin.
    Slab,
}

/// One structural piece TYPE. `id` is what a `PlacedStructure.type_id` stores.
#[derive(Debug, Clone, Deserialize)]
pub struct StructureType {
    pub id: String,
    pub label: String,
    /// Palette category -- "Structure" today; sub-grouping (Transport, Surfaces, ...) is free later.
    pub category: String,
    pub kind: StructureKind,
    pub shape: MeshShape,
    /// Footprint: (width X, height Y, depth Z) in metres.
    pub size: (f32, f32, f32),
    /// rgb base colour (alpha is 1.0 -- structural pieces are opaque in Stage 1).
    pub color: (f32, f32, f32),
    /// Step / rung count for Steps + Ladder shapes (ignored otherwise).
    #[serde(default)]
    pub steps: u32,
    pub note: String,
}

/// The structural-type registry, parsed once + embedded at compile time (same pattern as
/// wall_materials / light_types / lock_types).
pub fn structure_types() -> &'static [StructureType] {
    static REG: std::sync::OnceLock<Vec<StructureType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/structure_types.ron");
        match ron::from_str::<Vec<StructureType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("structure_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a structural type by id (None if unknown).
pub fn structure_type(id: &str) -> Option<&'static StructureType> {
    structure_types().iter().find(|t| t.id == id)
}

/// Palette items grouped by category, sorted by label -- mirrors `MachineHome::palette_categories`
/// so the construction palette renders structural pieces with the same widget. Wall sorts FIRST in
/// its category (a leading space) since it is the most-used tool.
pub fn palette_categories() -> Vec<(String, Vec<(String, String)>)> {
    use std::collections::BTreeMap;
    let mut by_cat: BTreeMap<String, Vec<(String, String, bool)>> = BTreeMap::new();
    for t in structure_types() {
        by_cat
            .entry(t.category.clone())
            .or_default()
            .push((t.id.clone(), t.label.clone(), t.kind == StructureKind::Wall));
    }
    by_cat
        .into_iter()
        .map(|(cat, mut items)| {
            // Wall first, then alphabetical by label.
            items.sort_by(|a, b| b.2.cmp(&a.2).then(a.1.cmp(&b.1)));
            (cat, items.into_iter().map(|(id, label, _)| (id, label)).collect())
        })
        .collect()
}

// ── Parametric mesh builders ──────────────────────────────────────────────────────────────────

/// An axis-aligned box between two corners, with correct outward face normals. Local space.
fn aabb_box(min: Vec3, max: Vec3, out: &mut (Vec<Vertex>, Vec<u32>)) {
    let (x0, y0, z0) = (min.x, min.y, min.z);
    let (x1, y1, z1) = (max.x, max.y, max.z);
    // 6 faces, each its own 4 verts so normals are flat. (pos, normal) per face.
    let faces: [([Vec3; 4], [f32; 3]); 6] = [
        // +X
        ([Vec3::new(x1, y0, z0), Vec3::new(x1, y0, z1), Vec3::new(x1, y1, z1), Vec3::new(x1, y1, z0)], [1.0, 0.0, 0.0]),
        // -X
        ([Vec3::new(x0, y0, z1), Vec3::new(x0, y0, z0), Vec3::new(x0, y1, z0), Vec3::new(x0, y1, z1)], [-1.0, 0.0, 0.0]),
        // +Y (top)
        ([Vec3::new(x0, y1, z0), Vec3::new(x1, y1, z0), Vec3::new(x1, y1, z1), Vec3::new(x0, y1, z1)], [0.0, 1.0, 0.0]),
        // -Y (bottom)
        ([Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, y0, z0), Vec3::new(x0, y0, z0)], [0.0, -1.0, 0.0]),
        // +Z
        ([Vec3::new(x1, y0, z1), Vec3::new(x0, y0, z1), Vec3::new(x0, y1, z1), Vec3::new(x1, y1, z1)], [0.0, 0.0, 1.0]),
        // -Z
        ([Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y1, z0), Vec3::new(x0, y1, z0)], [0.0, 0.0, -1.0]),
    ];
    for (quad, n) in faces {
        let base = out.0.len() as u32;
        for (i, p) in quad.iter().enumerate() {
            let uv = [(i == 1 || i == 2) as i32 as f32, (i >= 2) as i32 as f32];
            out.0.push(Vertex { position: [p.x, p.y, p.z], normal: n, uv });
        }
        // The pipeline is CCW-front + back-cull (renderer/pipeline.rs), so the winding must make the
        // geometric normal point OUTWARD (agreeing with `n`). For these face corner orders that is the
        // REVERSED diagonal -- emitting the forward diagonal renders every box inside-out. (v0.583 fix)
        out.1.extend_from_slice(&[base, base + 2, base + 1, base, base + 3, base + 2]);
    }
}

/// Build a piece's LOCAL-space geometry (footprint centred on X/Z origin, base at y=0).
fn local_mesh(ty: &StructureType) -> (Vec<Vertex>, Vec<u32>) {
    let (w, h, d) = ty.size;
    let (hw, hd) = (w * 0.5, d * 0.5);
    let mut out = (Vec::new(), Vec::new());
    match ty.shape {
        MeshShape::Box => aabb_box(Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, h.max(0.05), hd), &mut out),
        MeshShape::Slab => {
            let t = h.clamp(0.02, 0.3);
            aabb_box(Vec3::new(-hw, 0.0, -hd), Vec3::new(hw, t, hd), &mut out);
        }
        MeshShape::Steps => {
            let n = ty.steps.max(1);
            let tread = d / n as f32;
            let riser = h / n as f32;
            for i in 0..n {
                let z0 = -hd + i as f32 * tread;
                // Each step a full-height column from the floor -> solid, walkable silhouette.
                aabb_box(
                    Vec3::new(-hw, 0.0, z0),
                    Vec3::new(hw, (i + 1) as f32 * riser, z0 + tread),
                    &mut out,
                );
            }
        }
        MeshShape::Ramp => {
            // Triangular prism: cross-section (in Z-Y) rises from (z=-hd,y=0) to (z=hd,y=h),
            // extruded across X. Two side triangles + the inclined top + bottom + far end.
            let p = |x: f32, y: f32, z: f32| Vertex { position: [x, y, z], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] };
            // Bottom quad
            push_quad(&mut out, p(-hw, 0.0, -hd), p(hw, 0.0, -hd), p(hw, 0.0, hd), p(-hw, 0.0, hd), [0.0, -1.0, 0.0]);
            // Inclined top quad (the walking surface)
            push_quad(&mut out, p(-hw, 0.0, -hd), p(-hw, h, hd), p(hw, h, hd), p(hw, 0.0, -hd), [0.0, 1.0, 0.0]);
            // Far end (vertical face at +Z) -- corners ordered so the winding faces OUTWARD (+Z),
            // matching the other ramp faces (push_quad's convention is the reverse of aabb_box's).
            push_quad(&mut out, p(hw, 0.0, hd), p(hw, h, hd), p(-hw, h, hd), p(-hw, 0.0, hd), [0.0, 0.0, 1.0]);
            // Two side triangles (-X and +X)
            push_tri(&mut out, p(-hw, 0.0, -hd), p(-hw, 0.0, hd), p(-hw, h, hd), [-1.0, 0.0, 0.0]);
            push_tri(&mut out, p(hw, 0.0, -hd), p(hw, h, hd), p(hw, 0.0, hd), [1.0, 0.0, 0.0]);
        }
        MeshShape::Ladder => {
            let rail = 0.04_f32;
            // Two side rails.
            aabb_box(Vec3::new(-hw, 0.0, -rail), Vec3::new(-hw + rail * 2.0, h, rail), &mut out);
            aabb_box(Vec3::new(hw - rail * 2.0, 0.0, -rail), Vec3::new(hw, h, rail), &mut out);
            // Rungs.
            let n = ty.steps.max(2);
            for i in 0..n {
                let y = (i as f32 + 0.5) * (h / n as f32);
                aabb_box(Vec3::new(-hw, y - rail, -rail), Vec3::new(hw, y + rail, rail), &mut out);
            }
        }
        MeshShape::Frame => {
            let t = (w * 0.12).clamp(0.06, 0.25);
            // Left + right posts.
            aabb_box(Vec3::new(-hw, 0.0, -hd), Vec3::new(-hw + t, h, hd), &mut out);
            aabb_box(Vec3::new(hw - t, 0.0, -hd), Vec3::new(hw, h, hd), &mut out);
            // Top lintel.
            aabb_box(Vec3::new(-hw, h - t, -hd), Vec3::new(hw, h, hd), &mut out);
        }
    }
    out
}

fn push_quad(out: &mut (Vec<Vertex>, Vec<u32>), a: Vertex, b: Vertex, c: Vertex, d: Vertex, n: [f32; 3]) {
    let base = out.0.len() as u32;
    for (i, mut v) in [a, b, c, d].into_iter().enumerate() {
        v.normal = n;
        v.uv = [(i == 1 || i == 2) as i32 as f32, (i >= 2) as i32 as f32];
        out.0.push(v);
    }
    out.1.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn push_tri(out: &mut (Vec<Vertex>, Vec<u32>), a: Vertex, b: Vertex, c: Vertex, n: [f32; 3]) {
    let base = out.0.len() as u32;
    for mut v in [a, b, c] {
        v.normal = n;
        out.0.push(v);
    }
    out.1.extend_from_slice(&[base, base + 1, base + 2]);
}

/// Build a placed piece's WORLD/home-local geometry: the local mesh yaw-rotated by `yaw_rad` (around
/// Y) and translated to `pos`. Normals are rotated too so lighting is correct.
pub fn structure_mesh(ty: &StructureType, pos: Vec3, yaw_rad: f32) -> (Vec<Vertex>, Vec<u32>) {
    let (mut verts, indices) = local_mesh(ty);
    let (s, c) = yaw_rad.sin_cos();
    let rot = |x: f32, z: f32| (x * c + z * s, -x * s + z * c);
    for v in &mut verts {
        let (px, pz) = rot(v.position[0], v.position[2]);
        v.position = [px + pos.x, v.position[1] + pos.y, pz + pos.z];
        let (nx, nz) = rot(v.normal[0], v.normal[2]);
        v.normal = [nx, v.normal[1], nz];
    }
    (verts, indices)
}

/// World (px,pz) -> the piece's LOCAL (x,z) coordinates (inverse of `structure_mesh`'s yaw): used
/// by the footing + footprint tests so they agree exactly with the rendered geometry. (v0.584)
fn world_to_local_xz(pos: (f32, f32, f32), yaw_rad: f32, px: f32, pz: f32) -> (f32, f32) {
    let (s, c) = yaw_rad.sin_cos();
    let (dx, dz) = (px - pos.0, pz - pos.2);
    // structure_mesh maps local->world by (x*c + z*s, -x*s + z*c); this is the inverse rotation.
    (dx * c - dz * s, dx * s + dz * c)
}

/// Is world (px,pz) within this piece's footprint (a small tolerance so edges count)? (v0.584)
pub fn in_footprint(ty: &StructureType, pos: (f32, f32, f32), yaw_rad: f32, px: f32, pz: f32) -> bool {
    let (w, _, d) = ty.size;
    let (lx, lz) = world_to_local_xz(pos, yaw_rad, px, pz);
    lx.abs() <= w * 0.5 + 0.05 && lz.abs() <= d * 0.5 + 0.05
}

/// The WALKABLE-surface world height at (px,pz) for a piece placed at `pos`/`yaw_rad`, or None if
/// (px,pz) is outside the footprint or the piece has no standable top (a hollow Frame -- elevator /
/// teleporter -- or a Ladder, which you climb). For Steps it's the top of the step under you; for a
/// Ramp the interpolated slope; for a Box / Slab the flat top. Lets the player WALK UP stairs/ramps
/// and ONTO platforms (the ground-floor sampler reads this each frame). (v0.584)
pub fn walk_surface(ty: &StructureType, pos: (f32, f32, f32), yaw_rad: f32, px: f32, pz: f32) -> Option<f32> {
    let (w, h, d) = ty.size;
    let (lx, lz) = world_to_local_xz(pos, yaw_rad, px, pz);
    if lx.abs() > w * 0.5 + 0.05 || lz.abs() > d * 0.5 + 0.05 {
        return None;
    }
    let surf = match ty.shape {
        MeshShape::Frame | MeshShape::Ladder => return None,
        MeshShape::Box => h.max(0.05),
        MeshShape::Slab => h.clamp(0.02, 0.3),
        MeshShape::Steps => {
            let n = ty.steps.max(1);
            let tread = d / n as f32;
            let riser = h / n as f32;
            let idx = (((lz + d * 0.5) / tread).floor() as i32).clamp(0, n as i32 - 1);
            (idx as f32 + 1.0) * riser
        }
        MeshShape::Ramp => h * ((lz + d * 0.5) / d).clamp(0.0, 1.0),
    };
    Some(pos.1 + surf)
}

/// The footprint half-extents (X, Z) of a piece AFTER a yaw rotation -- used to draw its bounds
/// gizmo + (v0.584) test the player's footing. Returns (half_w_world, height, half_d_world) of the
/// axis-aligned bounding box enclosing the rotated footprint.
pub fn rotated_half_extents(ty: &StructureType, yaw_rad: f32) -> (f32, f32, f32) {
    let (w, h, d) = ty.size;
    let (s, c) = yaw_rad.sin_cos();
    let hw = (w * 0.5 * c.abs()) + (d * 0.5 * s.abs());
    let hd = (w * 0.5 * s.abs()) + (d * 0.5 * c.abs());
    (hw, h, hd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_parses_and_has_core_pieces() {
        let types = structure_types();
        assert!(!types.is_empty(), "structure_types.ron should parse");
        for id in ["wall", "stairs", "ladder", "elevator", "teleporter", "train", "road"] {
            assert!(structure_type(id).is_some(), "missing structural type {id}");
        }
    }

    #[test]
    fn wall_is_the_only_wall_kind_and_sorts_first() {
        let cats = palette_categories();
        let structure = cats.iter().find(|(c, _)| c == "Structure").expect("Structure category");
        assert_eq!(structure.1.first().map(|(id, _)| id.as_str()), Some("wall"), "Wall leads the palette");
        let walls = structure_types().iter().filter(|t| t.kind == StructureKind::Wall).count();
        assert_eq!(walls, 1, "exactly one Wall kind");
    }

    #[test]
    fn every_shape_builds_non_empty_geometry() {
        for ty in structure_types() {
            if ty.kind == StructureKind::Wall {
                continue; // wall is drawn, not placed -- its mesh is unused
            }
            let (v, i) = structure_mesh(ty, Vec3::new(5.0, 0.0, 5.0), 0.7);
            assert!(!v.is_empty() && !i.is_empty(), "{} built empty geometry", ty.id);
            assert_eq!(i.len() % 3, 0, "{} indices not triangulated", ty.id);
            // Every index is in range.
            assert!(i.iter().all(|&idx| (idx as usize) < v.len()), "{} index out of range", ty.id);
        }
    }

    #[test]
    fn every_triangle_winds_outward() {
        // The pipeline is CCW-front + back-cull, so each triangle's GEOMETRIC normal (from its winding)
        // must agree in sign with the face's declared vertex normal -- otherwise the face is culled and
        // the piece renders inside-out. This locks the v0.583 winding fixes (aabb_box + the ramp face).
        for ty in structure_types() {
            if ty.kind == StructureKind::Wall {
                continue;
            }
            let (v, idx) = structure_mesh(ty, Vec3::ZERO, 0.0);
            for tri in idx.chunks(3) {
                let (a, b, c) = (v[tri[0] as usize], v[tri[1] as usize], v[tri[2] as usize]);
                let e1 = Vec3::from(b.position) - Vec3::from(a.position);
                let e2 = Vec3::from(c.position) - Vec3::from(a.position);
                let geo = e1.cross(e2);
                let declared = Vec3::from(a.normal);
                if geo.length() < 1e-6 || declared.length() < 1e-6 {
                    continue; // degenerate / unset -- skip
                }
                assert!(
                    geo.dot(declared) > 0.0,
                    "{} has an inside-out triangle (geo {:?} vs declared {:?})",
                    ty.id, geo, declared
                );
            }
        }
    }

    #[test]
    fn walk_surface_ascends_stairs_and_skips_frames() {
        let stairs = structure_type("stairs").unwrap();
        let (_, h, d) = stairs.size;
        let pos = (10.0, 0.0, 10.0);
        // Near edge (low z) is a low step; far edge (high z) is near the full height. yaw 0 -> +Z up.
        let near = walk_surface(stairs, pos, 0.0, 10.0, 10.0 - d * 0.5 + 0.05).unwrap();
        let far = walk_surface(stairs, pos, 0.0, 10.0, 10.0 + d * 0.5 - 0.05).unwrap();
        assert!(far > near, "stairs ascend front-to-back ({near} -> {far})");
        assert!(far <= pos.1 + h + 1e-3 && near >= pos.1, "within [base, base+h]");
        // Outside the footprint -> no footing.
        assert!(walk_surface(stairs, pos, 0.0, 100.0, 100.0).is_none());
        // A hollow frame (teleporter) gives no footing -- you walk through the arch.
        let tp = structure_type("teleporter").unwrap();
        assert!(walk_surface(tp, pos, 0.0, 10.0, 10.0).is_none());
        // A train platform (Box) lifts you onto its flat top.
        let train = structure_type("train").unwrap();
        let top = walk_surface(train, pos, 0.0, 10.0, 10.0).unwrap();
        assert!((top - (pos.1 + train.size.1)).abs() < 1e-3, "platform top = base + height");
    }

    #[test]
    fn ramp_surface_interpolates_linearly() {
        let ramp = structure_type("ramp").unwrap();
        let (_, h, d) = ramp.size;
        let pos = (0.0, 0.0, 0.0);
        let mid = walk_surface(ramp, pos, 0.0, 0.0, 0.0).unwrap(); // local z = 0 -> halfway
        assert!((mid - h * 0.5).abs() < 0.05, "ramp midpoint ~ half height, got {mid}");
        let _ = d;
    }

    #[test]
    fn rotated_extents_grow_under_45deg() {
        let ty = structure_type("train").unwrap(); // 3 x ? x 8 footprint
        let (hw0, _, hd0) = rotated_half_extents(ty, 0.0);
        let (hw45, _, hd45) = rotated_half_extents(ty, std::f32::consts::FRAC_PI_4);
        assert!(hw45 > hw0 && hd45 < hd0 + 0.01, "45deg rotation widens X, the AABB grows");
    }
}
