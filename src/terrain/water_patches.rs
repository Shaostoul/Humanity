//! THE WATER SHELL's own patch meshes: the sea sphere, tessellated and
//! skirted by the same quadtree the ground uses, but built by different rules.
//!
//! Extracted VERBATIM from `terrain::planet_chunks` (2026-09-19) under the
//! file-size ratchet, which planet_chunks.rs had outgrown at 4,878 lines
//! against a 4,820 budget.
//!
//! WHY THIS IS THE CLUSTER. planet_chunks is the quadtree: patch identity,
//! culling, the per-frame LOD selection, the ground mesh, the runtime cache.
//! The water shell BORROWS all of that and then builds something else
//! entirely - a mesh at exactly `def.radius` with spherical normals and no
//! elevation, present only where the connected-ocean mask says water, and
//! carrying no vegetation. Three builders and a band, and nothing in the
//! ground path calls any of them.
//!
//! The three rules that make water different from ground, kept together here
//! so they are read together:
//!   * THE MESH IS UNDISPLACED. The type-16 material's vertex stage adds the
//!     analytic wave height on the GPU, and the CPU physics twin
//!     (`terrain::ocean_waves`) adds the same height from the same formula, so
//!     drawn == sampled. Baking crests into the mesh would break that.
//!   * THE BAND IS CONSERVATIVE. `water_band` has to cover the worst-case
//!     wind-driven crest (`MAX_SEA_HEIGHT_M`), not the trains' amplitude,
//!     because it is a CULLING bound: over-estimating costs a little less
//!     culling, under-estimating culls a patch the shader then displaces into
//!     view.
//!   * AN ALL-LAND PATCH RETURNS None, and the driver caches that miss so
//!     selection stops asking. The shell genuinely has no geometry there.
//!
//! Declared as a `#[path]` CHILD of `planet_chunks`, the way `near_trees` is,
//! so one `use super::*` brings in `RadialBand`, `PatchId`, `PatchMesh`,
//! `SKIRT_MAX_M` and the rest, and the parent's `pub use` keeps every
//! `chunks::water_band` / `chunks::build_water_patch_mesh_at` /
//! `chunks::WATER_MAX_LEAVES` call in `engine/frame_water.rs` resolving
//! unchanged. terrain/mod.rs needed no edit.
//!
//! THE ONLY TEXT THAT CHANGED in the move, and it is worth knowing about:
//! two `super::ocean_mask::OceanMask` parameter types became
//! `crate::terrain::ocean_mask::OceanMask`. `super` used to mean `terrain`;
//! from a child of `planet_chunks` it means `planet_chunks`. Everything else
//! is byte-identical to what it replaced.

use super::*;

/// Water-shell patch cap. History: 14 (~38 m verts - only km swells were
/// meshable, the "flat 2D shape" report); 17 (v0.957, ~4.8 m - the 18-50 m
/// chop became real); 20 (v0.964, operator: "I imagine we wouldn't need
/// resolution for water any lower than 0.1m... maybe 0.5m") = ~0.6 m
/// vertices right at the eye, so even the 6 m ripple train is true
/// geometry at the waterline. Selection stays pixel-driven, so the deep
/// tiers exist only within tens of metres of the camera - the far ocean
/// costs exactly what it did.
pub const WATER_MAX_PATCH_DEPTH: u8 = 20;

/// Divisor that packs a water patch's vertex spacing into the vertex blue
/// channel (which `pack_color_to_uv` clamps to 0..1 and forwards as `uv.y`).
/// 65536 m covers the coarsest water leaf ever drawn (depth 3 = ~52 km cells
/// clamp to 1.0, and anything that coarse is gated off entirely anyway).
/// LOCKSTEP with `WATER_CELL_CODE_SCALE` in 00-bindings-vertex.wgsl.
pub const WATER_CELL_CODE_SCALE: f32 = 65536.0;

/// Water-shell leaf budget: six deeper tiers need more near-camera
/// leaves; MAX_OBJECTS is 16384 today, so 512 is still a small slice.
///
/// v0.1048: 512 -> 1024. With the water error floor added alongside (see
/// the wparams split_px in lib.rs) this is headroom for a high eye over open
/// ocean, where the visible sea area - and so the leaf demand - grows with
/// altitude; 512 covered a 3 m eye but not a 13 m one.
///
/// MEASURED, v0.1045: at 512 the ocean genuinely runs ~1.5 LOD levels
/// COARSER than the pixel-error target (the split heap hits the cap and
/// cuts every request above ~11-14 px of error instead of the 4.6 px the
/// selector asks for), so cross-LOD borders carry a bigger wave-height
/// mismatch than the selector intends. Raising this to 2048 was tried and
/// REVERTED: at a grazing dusk vantage it cost ~26 ms/frame (34 -> 60 ms)
/// and changed nothing visible, because the artifact it was meant to fix
/// (the operator's flat pale tiles) was the BACKSTOP's mismatched shading,
/// not coverage - see the type-16 backstop branch in 90-fragment-main.wgsl.
/// If the residual dusk seam ever needs attacking, make this a Settings
/// slider like terrain_patch_budget rather than raising the default.
pub const WATER_MAX_LEAVES: usize = 1024;

/// Conservative radial band for water-shell selection/culling: the sea
/// sphere plus the worst-case analytic wave height either way (the vertex
/// shader displaces within this envelope), plus skirt + slop.
pub fn water_band(radius_m: f64) -> RadialBand {
    // v0.1051: the FFT sea's crest now scales with wind (up to ~10 m), so this
    // CULLING bound must cover the worst case, not the trains' 3.1 m. It is a
    // conservative bound only - over-estimating costs a little less culling.
    let wave = crate::terrain::ocean_waves::MAX_SEA_HEIGHT_M as f64;
    RadialBand {
        min_r_m: radius_m - wave - SKIRT_MAX_M - 1.0,
        max_r_m: radius_m + wave + 1.0,
    }
}

/// Build one WATER-SHELL patch (v0.876 real-water Stage 1): the flat sea
/// sphere at exactly `def.radius`, only where the connected-ocean mask says
/// water. Returns None for all-land patches (the shell simply has no
/// geometry there -- the driver caches the miss so selection stops asking).
/// Faces are water-style (spherical normals); the type-16 material's vertex
/// stage displaces by the analytic wave height and its fragment stage draws
/// the Fresnel sky mirror + sun glitter, so the MESH stays the undisplaced
/// sphere -- the CPU physics twin (terrain::ocean_waves) adds the same
/// height analytically and drawn == sampled holds.
pub fn build_water_patch_mesh(
    def: &PlanetDef,
    ocean: &crate::terrain::ocean_mask::OceanMask,
    hm: Option<&PlanetHeightmap>,
    id: &PatchId,
) -> Option<PatchMesh> {
    build_water_patch_mesh_at(def, ocean, hm, id, 0.0)
}

/// The two next-coarser-level parents of barycentric lattice vert (r, c),
/// or None for EVEN verts (they survive coarsening). Parents follow the
/// triangulation's three lattice axes (+row, +col, and the emit-order
/// (r,c)->(r+1,c+1) diagonal), so every odd vert lies on a real edge of
/// the coarse triangulation and its fully-morphed height equals that
/// edge's linear interpolation - the geomorph weld contract (v0.1041).
pub fn water_geomorph_parents(r: u32, c: u32) -> Option<((u32, u32), (u32, u32))> {
    match (r % 2, c % 2) {
        (1, 0) => Some(((r - 1, c), (r + 1, c))),
        (0, 1) => Some(((r, c - 1), (r, c + 1))),
        (1, 1) => Some(((r - 1, c - 1), (r + 1, c + 1))),
        _ => None,
    }
}

/// `lift_offset_m` lowers (negative) or raises the shell radius relative to
/// the standard SURFACE_LIFT sphere. The BACKSTOP shell (v0.1019, water arc:
/// "holes through the water along the seams") builds at
/// -(MAX_WAVE_HEIGHT + 0.5): a coarse, UNDISPLACED deep-water layer under
/// the wave shell, so any cross-depth T-junction tear in the displaced
/// surface reveals water-colored backstop instead of pale seafloor or sky -
/// the long swells (360-2000 m) sag up to ~1.2 m across coarse patch edges
/// and CANNOT be resolution-faded away (they are the visible sea).
pub fn build_water_patch_mesh_at(
    def: &PlanetDef,
    ocean: &crate::terrain::ocean_mask::OceanMask,
    hm: Option<&PlanetHeightmap>,
    id: &PatchId,
    lift_offset_m: f64,
) -> Option<PatchMesh> {
    let n = PATCH_TESS;
    let corners = patch_corners(id);
    let radius_m =
        def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64 + lift_offset_m;
    let anchor = (corners[0] + corners[1] + corners[2]).normalize() * radius_m;

    // Same bit-identical border walk as the terrain builder (commutative
    // f64 midpoint math), so same-depth water neighbors share borders.
    // Region sea polygons (v0.1149 water carve) extend coverage into inlets
    // the 5.56 km ocean mask misses entirely: one snapshot per patch.
    let carve_masks = crate::terrain::water_carve::snapshot();
    let region_sea = |dir: DVec3| -> f32 {
        match &carve_masks {
            Some(cm) => crate::terrain::water_carve::sea_weight_at(cm, dir),
            None => 0.0,
        }
    };
    let vert_count = ((n + 1) * (n + 2) / 2) as usize;
    let mut dirs: Vec<DVec3> = Vec::with_capacity(vert_count);
    let mut any_ocean = false;
    for r in 0..=n {
        for c in 0..=r {
            let w0 = (n - r) as f64;
            let w1 = (r - c) as f64;
            let w2 = c as f64;
            let dir = (corners[0] * w0 + corners[1] * w1 + corners[2] * w2).normalize();
            // DILATED coverage test (v0.1056): the mask's 5.56 km cells are
            // 12x coarser than the seabed actually drawn, so an undilated test
            // clipped the shell along mask-cell edges and left kilometre-wide
            // strips of drawn-underwater seabed bare. The per-vertex depth
            // feather trims the shell back to the real waterline, so being
            // generous here costs nothing but closes those strips.
            if ocean.is_ocean_near(dir.as_vec3()) || region_sea(dir) > 0.0 {
                any_ocean = true;
            }
            dirs.push(dir);
        }
    }
    if !any_ocean {
        return None;
    }
    let offsets: Vec<glam::Vec3> = dirs
        .iter()
        .map(|d| ((*d * radius_m) - anchor).as_vec3())
        .collect();

    let grid_tris = (n * n) as usize;
    let skirt_tris = (3 * n * 2) as usize;
    let mut vertices: Vec<SurfaceVertexData> = Vec::with_capacity((grid_tris + skirt_tris) * 3);
    let mut indices: Vec<u32> = Vec::with_capacity((grid_tris + skirt_tris) * 3);
    // Per-vertex WATER DEPTH baked into the color transport (v0.917,
    // shoreline increment): the builder already knows the seafloor from
    // the heightmap, so the shader gets a smooth interpolated depth field
    // with zero runtime cost - no depth-texture pass needed. Encoding:
    // color r/g carry depth in decimetres as (hi, lo) bytes; the packed
    // UV then equals water_bit + depth_dm, and LINEAR interpolation of
    // that scalar across a triangle IS linear depth interpolation.
    // Without a heightmap every vertex reads 30 m (open-deep default).
    let depth_color = |dir: DVec3| -> [f32; 3] {
        // sample_meters is real elevation relative to sea level, so depth
        // below the surface is simply its negation. Region sea polygons
        // override upward: the carve pressed that ground to SEA_CARVE_M
        // below sea level, so the depth bake must agree or the shader's
        // shoreline feather trims the shell off the very inlet the carve
        // just made wet (weight-scaled so the feather still ramps ashore).
        let base_depth = hm
            .map(|h| (-h.sample_meters(dir.as_vec3())).max(0.0))
            .unwrap_or(300.0);
        let depth_m = base_depth
            .max(crate::terrain::water_carve::SEA_CARVE_M as f32 * region_sea(dir));
        let dm = (depth_m * 10.0).clamp(0.0, 65535.0) as u32;
        [((dm >> 8) & 255) as f32 / 255.0, (dm & 255) as f32 / 255.0, 0.0]
    };
    // CELL SIZE baked into the free blue channel (v0.1049 - the far-field
    // facets). The wave fades are Nyquist gates written in DISTANCE: each
    // train dies at 60 * lambda because the vertex spacing there is assumed
    // to be ~dist/325 (screen-error LOD at split_px 4), which lands the
    // fade at cell ~ lambda/5.4. But the water shell's leaf budget SATURATES
    // (measured [WaterDiag] at 700 m: coarsest drawn leaf carries 38-60 px of
    // error against a 4 px target), so the real spacing out there is 10-15x
    // coarser than the fade assumes: the shader keeps displacing waves the
    // mesh cannot represent, and 16 verts spanning hundreds of metres draw a
    // wave field as big randomly-tilted facets. That is the operator's
    // "flat triangles... only the very furthest", and why ascending makes it
    // worse (a higher eye sees more sea, so the budget cuts coarser).
    // Measuring the spacing instead of assuming it makes the same gate
    // correct at any budget. color.b -> uv.y (pack_color_to_uv keeps it as a
    // plain float, and water always wrote 0 there).
    let cell_m = ((dirs[grid_idx(1, 0)] - dirs[grid_idx(0, 0)]).length() * radius_m) as f32;
    let cell_code = (cell_m / WATER_CELL_CODE_SCALE).min(1.0);
    let depth_colors: Vec<[f32; 3]> = dirs
        .iter()
        .map(|d| {
            let mut c = depth_color(*d);
            c[2] = cell_code;
            c
        })
        .collect();
    // Geomorph parent deltas (v0.1041, the WELD fix - operator: "let's
    // fix the welds on the water polygons"): every ODD-parity lattice
    // vert disappears at the next-coarser LOD, where the surviving edge
    // linearly interpolates its two PARENT verts. The half-offset to
    // those parents rides the NORMAL slot (the water FS derives its
    // normal from position, so the slot is free transport; even verts
    // carry zero). The vertex shader morphs displacement toward the
    // parents' mean as the camera recedes, reaching EXACTLY the coarser
    // neighbor's edge interpolation before that neighbor can exist -
    // spatially exact welds with no neighbor bookkeeping (CDLOD).
    // Parent directions follow the triangulation's three lattice axes
    // (+row, +col, and the (r,c)->(r+1,c+1) diagonal of emit order), so
    // every odd vert sits on a real coarse edge. Along shared borders
    // the parity and parents are intrinsic to the edge, so both sides
    // compute identical morphs (the border walk is bit-identical).
    let vpos = |r: u32, c: u32| -> DVec3 { dirs[grid_idx(r, c)] * radius_m };
    let mut deltas: Vec<glam::Vec3> = Vec::with_capacity(vert_count);
    for r in 0..=n {
        for c in 0..=r {
            deltas.push(match water_geomorph_parents(r, c) {
                Some(((r1, c1), (r2, c2))) => {
                    ((vpos(r1, c1) - vpos(r2, c2)) * 0.5).as_vec3()
                }
                None => glam::Vec3::ZERO,
            });
        }
    }
    let mut emit_face = |ia: usize, ib: usize, ic: usize,
                         vertices: &mut Vec<SurfaceVertexData>,
                         indices: &mut Vec<u32>| {
        for &i in &[ia, ib, ic] {
            indices.push(vertices.len() as u32);
            vertices.push(SurfaceVertexData {
                position: offsets[i].to_array(),
                normal: deltas[i].to_array(),
                color: depth_colors[i],
                water: true,
                tree_card: false,
            });
        }
    };
    for r in 0..n {
        for c in 0..=r {
            emit_face(
                grid_idx(r, c),
                grid_idx(r + 1, c),
                grid_idx(r + 1, c + 1),
                &mut vertices,
                &mut indices,
            );
        }
        for c in 0..r {
            emit_face(
                grid_idx(r, c),
                grid_idx(r + 1, c + 1),
                grid_idx(r, c + 1),
                &mut vertices,
                &mut indices,
            );
        }
    }

    // NO skirts on water (v0.878.2, operator: visible triangle seams across
    // the whole ocean). The shell draws in the TRANSPARENT pass (no depth
    // write), so a skirt wall behind the surface blend-stacks along every
    // patch border - each border became a darker seam line. Cracks are
    // covered differently here: the shader's vertex wave displacement fades
    // to ZERO with distance (see ocean_wave_height's fade), so far patches
    // of any two LODs lie on the exact same sphere (bit-matching borders),
    // and near-field neighbor depths sample the same smooth analytic field
    // densely enough that any residual T-junction gap is sub-wave-height
    // over moving water - invisible where a skirt line was glaring.

    Some(PatchMesh {
        mesh: SurfaceMeshData { vertices, indices },
        anchor,
        // Water shells never carry tree cards.
        tree_density: 0.0,
        band: water_band(radius_m),
    })
}
