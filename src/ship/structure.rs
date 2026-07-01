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
    /// A flat walkable FLOOR placed at a height -- an upper-level landing the stairs lead onto. (v0.588)
    Deck,
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

/// A ZONE TYPE (v0.631, superstructure M1): a labelled kind of macro VOLUME a mothership is carved into
/// -- residential, industrial, hangar, mech bay, cargo, storage, the civic mall, ... Data-driven
/// (infinite-of-X): add a kind by adding one line to `zone_types.ron`, no code. `default_size` seeds a
/// freshly-placed zone's extent.
#[derive(Debug, Clone, Deserialize)]
pub struct ZoneType {
    pub id: String,
    pub label: String,
    /// rgb tint for the zone's wireframe + label.
    pub color: (f32, f32, f32),
    /// What happens in this zone (shown in the editor; teaches what each district is for).
    pub purpose: String,
    /// Default (width X, height Y, depth Z) metres for a newly placed zone of this type.
    pub default_size: (f32, f32, f32),
}

/// The zone-type registry, parsed once + embedded (same pattern as structure_types).
pub fn zone_types() -> &'static [ZoneType] {
    static REG: std::sync::OnceLock<Vec<ZoneType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/zone_types.ron");
        match ron::from_str::<Vec<ZoneType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("zone_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a zone type by id (None if unknown).
pub fn zone_type(id: &str) -> Option<&'static ZoneType> {
    zone_types().iter().find(|t| t.id == id)
}

/// A ZONE FILLER spec (v0.638, superstructure M2c): the generic, data-driven interior-population
/// recipe for one zone `type_id` -- "so the mothership looks filled out" without hand-authored per-zone
/// content. `home_structure::generate_zone_filler` tiles a repeated primitive box across a zone's
/// footprint using these dimensions. `residential` deliberately has NO entry here: it uses its own
/// home-cloning path instead (see `generate_zone_filler`'s residential branch). Infinite-of-X: add a
/// district's filler by adding one entry to `zone_filler.ron`, no code. `mesh_kind` is a forward-looking
/// tag (a future renderer stage can grow dedicated shapes per kind); today every kind renders as a
/// solid box silhouette.
#[derive(Debug, Clone, Deserialize)]
pub struct ZoneFiller {
    /// -> zone_types.ron id.
    pub type_id: String,
    /// (width X, depth Z) metres of one filler instance's footprint.
    pub footprint: (f32, f32),
    /// Height (Y) metres of one filler instance.
    pub height: f32,
    /// Metres of gap between adjacent instance footprints (an aisle).
    pub spacing: f32,
    /// Metres kept clear from the zone's own walls on every side (a walkway margin).
    pub inset: f32,
    /// Forward-looking shape tag ("rack" / "stall" / "cradle" / "array" / ...); read by a future
    /// renderer stage. Every kind renders as a plain box today.
    pub mesh_kind: String,
    /// If true, tint filler instances with the zone TYPE's own `color` (zone_types.ron) so each
    /// district reads as visually distinct. (No override field yet -- always true in the data; the
    /// flag exists so a future entry can opt out without a struct change.)
    #[serde(default = "default_true_filler")]
    pub color_from_zone_type: bool,
}

fn default_true_filler() -> bool {
    true
}

/// The zone-filler registry, parsed once + embedded (same pattern as zone_types).
pub fn zone_fillers() -> &'static [ZoneFiller] {
    static REG: std::sync::OnceLock<Vec<ZoneFiller>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/zone_filler.ron");
        match ron::from_str::<Vec<ZoneFiller>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("zone_filler.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a zone filler spec by zone `type_id` (None if unknown / unlisted -- e.g. "residential",
/// which uses the home-cloning path instead, or a type nobody has authored filler content for yet).
pub fn zone_filler(type_id: &str) -> Option<&'static ZoneFiller> {
    zone_fillers().iter().find(|f| f.type_id == type_id)
}

/// A ROAD CLASS (v0.585): a named, FIXED top-to-bottom material stack -- "an airplane runway has
/// different needs than a residential side road" (operator). The stack reuses `SurfaceLayer` (the
/// same model walls layer with), so road materials teach the same density/strength/cost. Used when a
/// road piece (or, v0.586, a road graph edge) is placed: it carries this class's layers.
#[derive(Debug, Clone, Deserialize)]
pub struct RoadType {
    pub id: String,
    pub label: String,
    /// Layers from the wearing course (top) down to the subgrade (bottom).
    pub layers: Vec<crate::ship::home_structure::SurfaceLayer>,
    pub note: String,
}

/// The road-class registry, parsed once + embedded (same pattern as the others).
pub fn road_types() -> &'static [RoadType] {
    static REG: std::sync::OnceLock<Vec<RoadType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/road_types.ron");
        match ron::from_str::<Vec<RoadType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("road_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// Look up a road class by id (None if unknown).
pub fn road_type(id: &str) -> Option<&'static RoadType> {
    road_types().iter().find(|t| t.id == id)
}

/// A CORRIDOR TYPE (v0.639, superstructure M2c-tail): the connector style
/// `home_structure::tile_home_clones` bridges adjacent tiled home-clone slots with, so a residential
/// zone reads as a connected community instead of floating boxes (the operator's pushback on v0.638:
/// "there's no real structure to it... we need some way of laying out multiple homesteads... adding
/// the corridors, elevators, stairs, ramps, etc. between all of them"). `width` is the walkway's clear
/// width; `road_class` -> `road_types.ron` supplies the floor ribbon's material stack (the SAME
/// primitive road-graph edges already render with -- "reuse an existing structure/road type"; the
/// wearing-course top layer's colour tints the ribbon just like a road edge does); `wall_material` /
/// `wall_height` are two low kerb rails so the corridor reads as a defined walkway without becoming a
/// blind hallway; `deck_type` -> `structure_types.ron` is the landing pad placed at each end where the
/// corridor meets a home's footprint edge. Infinite-of-X: add a style (a narrow service catwalk vs a
/// wide public concourse) by adding one entry to `corridor_types.ron`, no code.
#[derive(Debug, Clone, Deserialize)]
pub struct CorridorType {
    pub id: String,
    pub width: f32,
    /// -> road_types.ron id.
    pub road_class: String,
    /// -> wall_materials.ron id for the two side kerb rails.
    pub wall_material: u32,
    pub wall_height: f32,
    /// -> structure_types.ron id for the landing pad at each connected end.
    pub deck_type: String,
    pub note: String,
}

/// The corridor-type registry, parsed once + embedded (same pattern as road_types).
pub fn corridor_types() -> &'static [CorridorType] {
    static REG: std::sync::OnceLock<Vec<CorridorType>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        const SRC: &str = include_str!("../../data/blueprints/corridor_types.ron");
        match ron::from_str::<Vec<CorridorType>>(SRC) {
            Ok(v) => v,
            Err(e) => {
                log::error!("corridor_types.ron parse error: {e}");
                Vec::new()
            }
        }
    })
}

/// The DEFAULT corridor style: the first entry in the registry. `tile_home_clones` uses this for
/// every connector today; a future per-zone override (a mall gets a wider concourse style) is a
/// one-line change once more than one entry exists. None only if the registry is empty/unparseable.
pub fn default_corridor_type() -> Option<&'static CorridorType> {
    corridor_types().first()
}

/// Look up a corridor type by id (None if unknown).
pub fn corridor_type(id: &str) -> Option<&'static CorridorType> {
    corridor_types().iter().find(|t| t.id == id)
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
    fn a_deck_gives_footing_at_its_placed_height() {
        // The multi-level core (v0.588): a deck placed at height H is a standable floor at ~H, so the
        // ground sampler keeps the player up there after they climb the stairs.
        let deck = structure_type("deck").expect("deck piece exists");
        let h = 3.0_f32;
        let top = walk_surface(deck, (10.0, h, 10.0), 0.0, 10.0, 10.0).expect("deck has footing");
        assert!((top - (h + deck.size.1.clamp(0.02, 0.3))).abs() < 1e-3, "deck top = base height + slab");
        assert!(top > h, "footing is above the placed base, i.e. an upper level");
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
    fn corridor_types_parse_and_resolve_their_references() {
        // v0.639: the corridor registry parses, has at least one style, and every entry's
        // road_class/deck_type/wall_material actually resolves -- catches a typo the moment it's
        // added, rather than the ribbon/rail/pad silently failing to render.
        let corridors = corridor_types();
        assert!(!corridors.is_empty(), "corridor_types.ron should parse with at least one style");
        for c in corridors {
            assert!(c.width > 0.0, "{} has a positive width", c.id);
            assert!(road_type(&c.road_class).is_some(), "{} references a real road class", c.id);
            assert!(structure_type(&c.deck_type).is_some(), "{} references a real deck structure type", c.id);
            assert!(
                crate::ship::home_structure::wall_material(c.wall_material).is_some(),
                "{} references a real wall material",
                c.id
            );
        }
        assert!(default_corridor_type().is_some(), "the default (first) corridor style resolves");
        assert!(corridor_type(&corridors[0].id).is_some(), "lookup by id finds the first entry");
        assert!(corridor_type("no_such_corridor_style").is_none(), "an unknown id is None");
    }

    #[test]
    fn road_types_parse_with_layered_stacks() {
        let roads = road_types();
        assert!(!roads.is_empty(), "road_types.ron should parse");
        for id in ["footpath", "residential", "highway", "runway"] {
            let r = road_type(id).unwrap_or_else(|| panic!("missing road class {id}"));
            assert!(!r.layers.is_empty(), "{id} has a material stack");
            assert!(r.layers.iter().all(|l| l.thickness_m > 0.0), "{id} layer thickness positive");
        }
        // The runway is the heaviest stack (operator: thickest of any class).
        let total = |id: &str| road_type(id).unwrap().layers.iter().map(|l| l.thickness_m).sum::<f32>();
        assert!(total("runway") > total("residential"), "runway is thicker than a residential road");
    }

    #[test]
    fn rotated_extents_grow_under_45deg() {
        let ty = structure_type("train").unwrap(); // 3 x ? x 8 footprint
        let (hw0, _, hd0) = rotated_half_extents(ty, 0.0);
        let (hw45, _, hd45) = rotated_half_extents(ty, std::f32::consts::FRAC_PI_4);
        assert!(hw45 > hw0 && hd45 < hd0 + 0.01, "45deg rotation widens X, the AABB grows");
    }
}
