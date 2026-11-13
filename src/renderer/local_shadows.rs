//! SHADOWS FROM ROOM LIGHTS AND THE SUN NEAR YOU, RUNG 1 (2026-11-10;
//! docs/design/local-light-shadows.md, the plan of record).
//!
//! The operator, 2026-11-10: "I've also noticed player characters and furniture don't see to have
//! shadows." He was right twice over: the room lights (light.rs, the clustered light loop in
//! 80-fragment-shared.wgsl) had no shadow code at all, and the near world (the scene list: the
//! homes, the furniture, every person's body) was never drawn into the sun's shadow map, which only
//! the far world's celestial list fills (celestial.rs). Measured from the rendered pixels before
//! this rung: the design doc, section 1.
//!
//! WHAT THIS FILE DOES, once a frame before the celestial pass (`render_shadow_atlas`, called from
//! lib.rs):
//!
//! 1. Picks the room lights that matter most this frame (`select_lights`: brightness times how much
//!    of the view the light's reach covers, the ones whose reach the view cannot see left out), up to
//!    the level's budget (`ShadowLevel::lights`). The rest stay unshadowed, and the Settings words say
//!    so.
//! 2. Gives each a place in ONE depth atlas: a point light six cube faces (a face with nothing in it
//!    gets no tile and reads as lit), a spot light one perspective view, at a face size chosen by its
//!    rank and packed largest first in Morton order (`pack_tiles`). Line lights (strips) are not
//!    shadowed in rung 1 (their emitter is a segment; rung 3 of the plan).
//! 3. Adds the NEAR SUN view: an orthographic tile around the eye along the sun that lights the
//!    scene (`Renderer::cur_sun`, the hull-frame sun aboard), so the homes, furniture and bodies
//!    shade the sunlight that reaches them, which the 0.73 m far map never could. The shader
//!    multiplies it into every sun term (`sun_shadow_offset`), so the terrain under you, a shelf
//!    under a shelf and the floor under a chair all get it.
//! 4. Culls the casters per view by bounding sphere (`Mesh::bounds`) and draws them, depth only (the
//!    sun shadow PSOs: standard Z, `Less`, no culling), cutouts through `fs_shadow` (card hair, the
//!    cluster cards), skinned middle bodies and the first-person body's unfolded twin through the
//!    skinned shadow PSOs (renderer/skinning.rs).
//! 5. Writes the table the shader reads (`pack_table`, group 0 binding 10) and submits, so the
//!    celestial pass (terrain) and the scene pass (everything in the home) sample this frame's maps.
//!
//! NOTHING IS KEPT ACROSS FRAMES AS WORKED-OUT DATA: every face is drawn again every frame, from this
//! frame's casters. The one thing read from the frame before is which lights were shadowed, so a light
//! at the edge of the budget does not flicker in and out (`HYSTERESIS`); that is a reuse, and it obeys
//! the one switch (`humanity_core::reuse::enabled()`, HUMANITY_NO_REUSE=1, the Dev page's "Turn off
//! all reuse"): with reuse off the ranking starts fresh every frame. The atlas texture itself is a
//! resource made once per level, not a result.

use super::light::RoomLight;
use super::*;
use glam::Vec4;

/// Where the levels are read from (relative to the data folder's parent, as the other Settings data
/// files are). Settings > Graphics > Light and sky > Room light shadows.
pub const LEVELS_PATH: &str = "data/gui/shadow_levels.json";
/// The level a new install, or a saved id the file no longer lists, gets: the full design.
pub const DEFAULT_LEVEL: &str = "full";
/// Rows of `vec4<f32>` per view in the table: four for the view-projection matrix's columns, one for
/// the tile (u0, v0, side, filter radius, all in atlas uv), one for what the bias needs (the light's
/// position for a perspective view or the direction toward the light for an orthographic one, and
/// the world size of a texel: per metre of depth, or negated and flat for orthographic), and one for
/// the light's shadow reach (`shadow_reach`; 0 for orthographic), over whose last fifth the shadow
/// fades out. LOCKSTEP with `SHADOW_VIEW_ROWS` in 00-bindings-vertex.wgsl
/// (`table_constants_match_the_shader`).
pub const VIEW_ROWS: usize = 7;
/// A shadowed light's lookup code is `kind * KIND_STRIDE + first view + 1` (0 = unshadowed), as an
/// f32 (exact well below 2^24). LOCKSTEP with `SHADOW_KIND_STRIDE` in the shader.
pub const KIND_STRIDE: u32 = 4096;
/// Six cube faces from the first view, picked by the major axis of the direction from the light.
pub const KIND_CUBE: u32 = 1;
/// One perspective view (a spot light whose cone fits one).
pub const KIND_SPOT: u32 = 2;
/// A light shadowed last frame ranks as if this much more important, so a light at the edge of the
/// budget keeps its shadow until another clearly outranks it. Obeys the reuse switch.
pub const HYSTERESIS: f32 = 1.3;
/// The nearest a caster may be to a light and still shadow it, metres (the cube faces' near plane).
/// The light fixtures themselves (a bulb, a panel) are emissive and never cast (`casts`).
pub const NEAR_M: f32 = 0.05;
/// How far toward the sun the near sun view reaches for casters above the box (a roof, a beam), m.
pub const SUN_REACH_M: f32 = 60.0;
/// The smallest tile, texels: the packing unit.
pub const TILE_UNIT: u32 = 64;
/// The radius of a skinned body's bounding sphere, metres, around a point 0.9 m up its local up:
/// generous enough for a body lying down or reaching overhead (its vertices are written by the
/// skinning pass each frame, so there is no measured bound to use).
pub const BODY_RADIUS_M: f32 = 2.0;
/// The centre of that sphere in the body's own space.
pub const BODY_CENTRE: [f32; 3] = [0.0, 0.9, 0.0];

/// One level of Room light shadows (data/gui/shadow_levels.json).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct ShadowLevel {
    pub id: String,
    /// The most room lights that cast shadows at once (the most important this frame). 0 = none.
    #[serde(default)]
    pub lights: u32,
    /// The atlas's side, texels (a power of two; 0 = no atlas at all: no room light shadows and no
    /// near sun view).
    #[serde(default)]
    pub atlas: u32,
    /// The largest face a light is given, texels.
    #[serde(default)]
    pub face: u32,
    /// The smallest face a light is given before it is left unshadowed, texels.
    #[serde(default)]
    pub face_min: u32,
    /// The near sun view's side, texels (0 = no near sun view).
    #[serde(default)]
    pub sun_tile: u32,
    /// Half the near sun view's width, metres.
    #[serde(default)]
    pub sun_extent_m: f32,
    /// Depth comparisons per lookup: 4 (a 2x2 pattern) or 16 (a Poisson disk).
    #[serde(default)]
    pub taps: u32,
}

impl ShadowLevel {
    /// The atlas's video memory: one 32-bit depth per texel.
    pub fn atlas_bytes(&self) -> u64 {
        self.atlas as u64 * self.atlas as u64 * 4
    }

    /// World size of one near-sun texel, centimetres (for the Settings words).
    pub fn sun_texel_cm(&self) -> f32 {
        if self.sun_tile == 0 {
            0.0
        } else {
            self.sun_extent_m * 2.0 / self.sun_tile as f32 * 100.0
        }
    }

    /// The filter's radius in texels: wider with more taps (soft at room scale), never under one.
    pub fn filter_texels(&self) -> f32 {
        if self.taps >= 16 {
            1.75
        } else {
            1.0
        }
    }
}

#[derive(serde::Deserialize)]
struct LevelsFile {
    #[serde(default)]
    levels: Vec<ShadowLevel>,
}

/// The design's level, for an install whose levels file is missing or empty.
fn built_in_level() -> ShadowLevel {
    ShadowLevel {
        id: DEFAULT_LEVEL.to_string(),
        lights: 8,
        atlas: 4096,
        face: 512,
        face_min: 128,
        sun_tile: 2048,
        sun_extent_m: 16.0,
        taps: 16,
    }
}

fn pow2_within(v: u32, lo: u32, hi: u32) -> u32 {
    if v == 0 {
        return 0;
    }
    v.next_power_of_two().clamp(lo, hi)
}

/// The levels in `text` (None when it could not be read), each held to what the renderer can do:
/// the atlas a power of two within `wgpu::Limits::default()`'s 8,192, the faces powers of two that
/// fit it, at most 64 lights, the near sun view at most half the atlas, 4 or 16 taps; a repeated id
/// dropped. Nothing usable gives the design's level alone, so the setting always has one.
pub fn parse_levels(text: Option<&str>) -> Vec<ShadowLevel> {
    let parsed = text.and_then(|t| match serde_json::from_str::<LevelsFile>(t) {
        Ok(f) => Some(f.levels),
        Err(e) => {
            log::warn!("[Shadows] {LEVELS_PATH} does not parse ({e}); Room light shadows offers Full only");
            None
        }
    });
    let max_side = wgpu::Limits::default().max_texture_dimension_2d.min(8192);
    let mut out: Vec<ShadowLevel> = Vec::new();
    for mut l in parsed.unwrap_or_default() {
        if l.id.is_empty() || out.iter().any(|k| k.id == l.id) {
            continue;
        }
        l.atlas = pow2_within(l.atlas, 256, max_side);
        let half = (l.atlas / 2).max(TILE_UNIT);
        l.face = pow2_within(l.face, TILE_UNIT, half);
        l.face_min = pow2_within(l.face_min.max(TILE_UNIT), TILE_UNIT, l.face.max(TILE_UNIT));
        l.sun_tile = pow2_within(l.sun_tile, TILE_UNIT, half);
        l.lights = l.lights.min(64);
        l.sun_extent_m = if l.sun_extent_m.is_finite() { l.sun_extent_m.clamp(2.0, 64.0) } else { 16.0 };
        l.taps = if l.taps >= 16 { 16 } else { 4 };
        if l.atlas == 0 {
            l.lights = 0;
            l.face = 0;
            l.face_min = 0;
            l.sun_tile = 0;
        }
        out.push(l);
    }
    if out.is_empty() {
        out.push(built_in_level());
    }
    out
}

/// Every level, read once from `LEVELS_PATH` (a change to the file takes effect at the next start).
pub fn levels() -> &'static [ShadowLevel] {
    static LEVELS: std::sync::OnceLock<Vec<ShadowLevel>> = std::sync::OnceLock::new();
    LEVELS.get_or_init(|| {
        let text = std::fs::read_to_string(LEVELS_PATH).ok();
        if text.is_none() {
            log::warn!("[Shadows] {LEVELS_PATH} not found; Room light shadows offers Full only");
        }
        parse_levels(text.as_deref())
    })
}

/// True when `id` names a level the file lists.
pub fn is_level(id: &str) -> bool {
    levels().iter().any(|l| l.id == id)
}

/// The level `id` names, or `DEFAULT_LEVEL`'s for an id the file does not list (the first level
/// when it does not list that either).
pub fn level(id: &str) -> &'static ShadowLevel {
    let all = levels();
    all.iter().find(|l| l.id == id).or_else(|| all.iter().find(|l| l.id == DEFAULT_LEVEL)).unwrap_or(&all[0])
}

// ── The pure parts (unit-tested below, no GPU) ──────────────────────────────────────────────────

/// A view's six clip planes (x, y, z normal, w distance), from a view-projection with 0..1 depth
/// (Gribb and Hartmann 2001). A point p is inside when `dot(n, p) + w >= 0` for all six.
pub fn frustum_planes(m: &Mat4) -> [Vec4; 6] {
    let (r0, r1, r2, r3) = (m.row(0), m.row(1), m.row(2), m.row(3));
    let norm = |p: Vec4| {
        let l = p.truncate().length();
        if l > 1e-12 {
            p / l
        } else {
            p
        }
    };
    [norm(r3 + r0), norm(r3 - r0), norm(r3 + r1), norm(r3 - r1), norm(r2), norm(r3 - r2)]
}

/// Whether a sphere touches the volume the planes bound (conservative: a sphere near a corner may
/// pass, which only costs a draw).
pub fn sphere_in(planes: &[Vec4; 6], c: Vec3, r: f32) -> bool {
    if !r.is_finite() {
        return true;
    }
    planes.iter().all(|p| p.truncate().dot(c) + p.w >= -r)
}

/// THE SHADOW-CASTER CULL (shadows rung 1b, 2026-11-12): whether the shadow a caster throws from a
/// point or spot light at `light` can fall on anything inside the camera's view (`view`, its six
/// planes). The shadow of a sphere `(c, r)` at distance d from the light lies inside the cone from
/// the sphere straight away from the light, widening to r x reach / d at the light's shadow reach
/// (past which the shader draws no shadow); that cone lies inside the convex hull of the sphere and
/// the sphere at the far end, and the hull is outside the view exactly when one plane has both
/// spheres wholly on its outer side (conservative: it keeps some casters it could leave out, never
/// the reverse). `pad` widens both for the filter's reach in world units. A caster the light sits
/// inside, or an unbounded one, is always kept. The standard per-light caster cull (Bittner, Mattausch,
/// Silvennoinen and Wimmer, "Shadow Caster Culling for Efficient Shadow Mapping", I3D 2011, does it
/// per pixel with occlusion queries; this is its bounding-volume form, no GPU round trip).
pub fn shadow_reaches_view(view: &[Vec4; 6], light: Vec3, c: Vec3, r: f32, reach: f32, pad: f32) -> bool {
    if !r.is_finite() {
        return true;
    }
    let to = c - light;
    let d = to.length();
    if d <= r + 1e-4 {
        return true;
    }
    let far_d = reach.max(d);
    let far = light + to * (far_d / d);
    let (near_r, far_r) = (r + pad, r * far_d / d + pad);
    !view.iter().any(|p| p.truncate().dot(c) + p.w < -near_r && p.truncate().dot(far) + p.w < -far_r)
}

/// The cube face a direction from the light falls on: 0 +x, 1 -x, 2 +y, 3 -y, 4 +z, 5 -z, by its
/// major axis (ties to x, then y). LOCKSTEP with `shadow_cube_face` in 00-bindings-vertex.wgsl.
pub fn cube_face_of(d: Vec3) -> usize {
    let a = d.abs();
    if a.x >= a.y && a.x >= a.z {
        if d.x >= 0.0 {
            0
        } else {
            1
        }
    } else if a.y >= a.z {
        if d.y >= 0.0 {
            2
        } else {
            3
        }
    } else if d.z >= 0.0 {
        4
    } else {
        5
    }
}

/// The direction each cube face looks along, in face order.
pub const FACE_DIRS: [Vec3; 6] = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];

/// The tangent of half a cube face's field of view: a little over 45 degrees, so a lookup near a
/// face's edge still has `margin_texels` of the face around it for its filter.
pub fn face_tan_half(res: u32, margin_texels: f32) -> f32 {
    1.0 + 2.0 * margin_texels / res.max(1) as f32
}

/// One cube face's view-projection (standard 0..1 depth, `Less`), looking along `FACE_DIRS[face]`.
pub fn cube_face_view(pos: Vec3, face: usize, range: f32, res: u32, margin_texels: f32) -> Mat4 {
    let dir = FACE_DIRS[face];
    let up = if face == 2 || face == 3 { Vec3::Z } else { Vec3::Y };
    let fov = 2.0 * face_tan_half(res, margin_texels).atan();
    Mat4::perspective_rh(fov, 1.0, NEAR_M, range.max(NEAR_M * 4.0)) * Mat4::look_at_rh(pos, pos + dir, up)
}

/// A spot light's one view: its cone (the outer angle) and the filter margin, at most 160 degrees.
pub fn spot_view(pos: Vec3, dir: Vec3, cos_outer: f32, range: f32, res: u32, margin_texels: f32) -> Mat4 {
    let dir = if dir.length_squared() > 1e-8 { dir.normalize() } else { Vec3::NEG_Y };
    let up = if dir.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
    let half = cos_outer.clamp(-1.0, 1.0).acos();
    let tan_half = (half.tan() * face_tan_half(res, margin_texels)).min(80f32.to_radians().tan());
    let fov = 2.0 * tan_half.atan();
    Mat4::perspective_rh(fov, 1.0, NEAR_M, range.max(NEAR_M * 4.0)) * Mat4::look_at_rh(pos, pos + dir, up)
}

/// Whether a spot's cone fits one perspective view (its outer half-angle at most 60 degrees);
/// wider spots are shadowed like points.
pub fn spot_fits_one_view(cos_outer: f32) -> bool {
    cos_outer > 0.5
}

/// The near sun view: orthographic, `extent` metres either side of `center` across the sun, reaching
/// `SUN_REACH_M` toward the sun for casters above the box and `extent * 1.5` beyond the centre.
/// Texel-snapped against the world origin, so a moving eye never swims the shadows (the far map's
/// own rule, celestial.rs).
pub fn near_sun_view(center: Vec3, toward_sun: Vec3, extent: f32, res: u32) -> Mat4 {
    let sun = if toward_sun.length_squared() > 1e-8 { toward_sun.normalize() } else { Vec3::Y };
    let up = if sun.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
    let view = Mat4::look_at_rh(center + sun * SUN_REACH_M, center, up);
    let proj = Mat4::orthographic_rh(-extent, extent, -extent, extent, 0.1, SUN_REACH_M + extent * 1.5);
    let vp = proj * view;
    let ndc_texel = 2.0 / res.max(1) as f32;
    let o = vp * Vec4::new(0.0, 0.0, 0.0, 1.0);
    let snap = |v: f32| (v / ndc_texel).round() * ndc_texel - v;
    Mat4::from_translation(Vec3::new(snap(o.x), snap(o.y), 0.0)) * vp
}

/// Interleaved bits back to (x, y): Morton order, x in the even bits.
pub fn morton_decode(i: u32) -> (u32, u32) {
    fn compact(mut v: u32) -> u32 {
        v &= 0x5555_5555;
        v = (v | (v >> 1)) & 0x3333_3333;
        v = (v | (v >> 2)) & 0x0f0f_0f0f;
        v = (v | (v >> 4)) & 0x00ff_00ff;
        v = (v | (v >> 8)) & 0x0000_ffff;
        v
    }
    (compact(i), compact(i >> 1))
}

/// Place square tiles (powers of two, at least `TILE_UNIT`) in an atlas of side `atlas`: largest
/// first, each at the next free place in Morton order, which for sizes taken largest first is always
/// an aligned square, so nothing overlaps and nothing is wasted until the atlas is full. Returns each
/// request's texel origin in request order, None where it did not fit.
pub fn pack_tiles(sizes: &[u32], atlas: u32) -> Vec<Option<(u32, u32)>> {
    let mut out = vec![None; sizes.len()];
    if atlas < TILE_UNIT {
        return out;
    }
    let units = atlas / TILE_UNIT;
    let total = (units as u64) * (units as u64);
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]).then(a.cmp(&b)));
    let mut cursor: u64 = 0;
    for i in order {
        let s = sizes[i];
        if s < TILE_UNIT || s > atlas || !s.is_power_of_two() {
            continue;
        }
        let side = (s / TILE_UNIT) as u64;
        let cells = side * side;
        // Largest first keeps the cursor aligned; aligning again costs nothing and keeps a caller's
        // odd order from overlapping.
        let at = cursor.div_ceil(cells) * cells;
        if at + cells > total {
            continue;
        }
        let (x, y) = morton_decode(at as u32);
        out[i] = Some((x * TILE_UNIT, y * TILE_UNIT));
        cursor = at + cells;
    }
    out
}

/// The shader's falloff for a light at distance `d` (80-fragment-shared.wgsl's light loop, without
/// the intensity): inverse square, windowed to nothing at the light's reach.
pub fn falloff(d: f32, range: f32) -> f32 {
    1.0 / (1.0 + d * d) * (1.0 - d / range.max(1e-3)).max(0.0)
}

/// How far a light's shadow is worth drawing: where its falloff has dropped to a twentieth of its
/// value a metre away (or its reach, if that comes first). Past it the light adds a twentieth or less
/// of what it adds up close, so a shadow there is invisible beside everything else lighting that
/// spot, while drawing it costs every caster out to the full reach: a 15 m panel's shadows stop at
/// about 5.2 m, a 4 m lamp's at about 3.1 m. The shader fades the shadow out over the last fifth.
pub fn shadow_reach(l: &RoomLight) -> f32 {
    let r = l.range.max(0.1);
    let want = 0.05 * falloff(1.0, r);
    if falloff(r * 0.999, r) >= want {
        return r;
    }
    let (mut lo, mut hi) = (0.0f32, r);
    for _ in 0..24 {
        let mid = (lo + hi) * 0.5;
        if falloff(mid, r) > want {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi.max(0.5).min(r)
}

/// Where the view's light is judged: the eye and three points ahead of it along the view (2, 4 and
/// 8 m), so a lamp over what you look at outranks a brighter one in the next room.
pub fn view_probes(eye: Vec3, forward: Vec3) -> [Vec3; 4] {
    let f = forward.normalize_or_zero();
    [eye, eye + f * 2.0, eye + f * 4.0, eye + f * 8.0]
}

/// How much a light matters to this frame: its brightness (luminance times intensity) times the
/// light it puts where the view is (the shader's own falloff at the nearest of `probes`), plus a
/// thousandth of the share of the view its reach covers, so a lamp lighting only far surfaces in view
/// still ranks above nothing. None for a light that cannot be shadowed in rung 1 (a line light) or
/// whose SHADOW reach (`shadow_reach`) the view cannot see: such a light still lights what you see,
/// but its shadows would all fall out of view, so it would only spend a slot of the budget.
pub fn light_importance(l: &RoomLight, probes: &[Vec3], view: &[Vec4; 6]) -> Option<f32> {
    if l.cos_outer < -1.5 || l.range <= 0.0 || l.intensity <= 0.0 {
        return None;
    }
    if !sphere_in(view, l.pos, shadow_reach(l)) {
        return None;
    }
    let lum = (0.2126 * l.color[0] + 0.7152 * l.color[1] + 0.0722 * l.color[2]).max(0.0) * l.intensity;
    let near = probes.iter().map(|p| falloff(l.pos.distance(*p), l.range)).fold(0.0f32, f32::max);
    let d = probes.first().map_or(l.range, |e| l.pos.distance(*e));
    let cover = if d <= l.range { 1.0 } else { (l.range / d).powi(2) };
    Some(lum * (near + 0.001 * cover))
}

/// Whether a view (a cube face or a spot's cone, its view-projection `vp`) can hold anything the
/// camera sees: false when all five corners of its pyramid (the light and the four corners of its far
/// plane) lie outside one of the camera's planes, so no visible surface ever looks it up. Conservative
/// (a pyramid crossing a frustum's corner may pass), which only costs a face.
pub fn view_touches(vp: &Mat4, camera: &[Vec4; 6]) -> bool {
    let inv = vp.inverse();
    let mut pts = [Vec3::ZERO; 5];
    // The near plane's centre stands in for the apex (the light, 5 cm behind it).
    let c = inv * Vec4::new(0.0, 0.0, 0.0, 1.0);
    pts[0] = c.truncate() / c.w;
    for (k, (x, y)) in [(-1.0f32, -1.0f32), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)].into_iter().enumerate() {
        let p = inv * Vec4::new(x, y, 1.0, 1.0);
        pts[k + 1] = p.truncate() / p.w;
    }
    !camera.iter().any(|pl| pts.iter().all(|p| pl.truncate().dot(*p) + pl.w < 0.0))
}

/// Whether a caster is big enough to show in a view: its diameter covers at least one texel at its
/// distance (`texel_m` per metre of depth for a perspective view from `origin`, flat for an
/// orthographic one). A caster smaller than a texel stamps nothing a filter would keep.
pub fn big_enough(centre: Vec3, radius: f32, origin: Vec3, texel_m: f32, ortho: bool) -> bool {
    if !radius.is_finite() {
        return true;
    }
    let texel = if ortho { texel_m } else { texel_m * (centre.distance(origin) - radius).max(0.05) };
    2.0 * radius >= texel
}

/// A light's identity across frames for the hysteresis: its position to the centimetre (lights have
/// no ids; a light that moves is a new light, which is what the hysteresis should think).
pub fn light_key(l: &RoomLight) -> (i32, i32, i32) {
    let q = |v: f32| (v * 100.0).round() as i32;
    (q(l.pos.x), q(l.pos.y), q(l.pos.z))
}

/// The lights to shadow this frame, most important first: indices into `lights`, at most `budget`.
/// `prev` (last frame's keys; empty with reuse off) ranks a light shadowed last frame `HYSTERESIS`
/// times higher.
pub fn select_lights(lights: &[RoomLight], probes: &[Vec3], view: &[Vec4; 6], budget: usize, prev: &[(i32, i32, i32)]) -> Vec<usize> {
    if budget == 0 {
        return Vec::new();
    }
    let mut ranked: Vec<(f32, usize)> = lights
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let w = light_importance(l, probes, view)?;
            let w = if prev.contains(&light_key(l)) { w * HYSTERESIS } else { w };
            Some((w, i))
        })
        .collect();
    ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    ranked.into_iter().take(budget).map(|(_, i)| i).collect()
}

/// A shadowed light's face size by its rank: the two most important get the level's largest face,
/// the next two half of it, the rest a quarter, never under `face_min`.
pub fn face_for_rank(rank: usize, face: u32, face_min: u32) -> u32 {
    let f = match rank {
        0 | 1 => face,
        2 | 3 => face / 2,
        _ => face / 4,
    };
    f.max(face_min).max(TILE_UNIT)
}

/// One view in the atlas this frame.
#[derive(Debug, Clone, Copy)]
pub struct AtlasView {
    pub view_proj: Mat4,
    /// Texel origin and side in the atlas; side 0 = no tile (nothing casts into it: reads as lit).
    pub tile: (u32, u32, u32),
    /// A perspective view's light position, or an orthographic view's direction toward the light.
    pub origin: Vec3,
    pub ortho: bool,
    /// World size of a texel per metre of depth (perspective) or flat (orthographic).
    pub texel_m: f32,
    /// The light's reach (perspective): casters farther than this cannot shadow anything it lights.
    pub range: f32,
}

/// The table the shader reads (group 0 binding 10, `shadow_table`), as `vec4<f32>` rows:
/// - row 0: lights the lookup covers, the first view row, the near sun view + 1 (0 = none), the taps;
/// - then the lookup, four lights a row: `kind * KIND_STRIDE + first view + 1`, 0 = unshadowed;
/// - then `VIEW_ROWS` rows per view (see `VIEW_ROWS`).
/// `lookups` are (light index, code); `filter_texels` the filter's radius.
pub fn pack_table(lookups: &[(usize, u32)], views: &[AtlasView], near_sun: Option<usize>, atlas: u32, filter_texels: f32, taps: u32) -> Vec<[f32; 4]> {
    let covered = lookups.iter().map(|(i, _)| i + 1).max().unwrap_or(0);
    let lookup_rows = covered.div_ceil(4);
    let first_view = 1 + lookup_rows;
    let mut rows = vec![[0.0f32; 4]; first_view + views.len() * VIEW_ROWS];
    rows[0] = [covered as f32, first_view as f32, near_sun.map_or(0.0, |v| (v + 1) as f32), taps as f32];
    for &(i, code) in lookups {
        rows[1 + i / 4][i % 4] = code as f32;
    }
    let inv = 1.0 / atlas.max(1) as f32;
    for (k, v) in views.iter().enumerate() {
        let b = first_view + k * VIEW_ROWS;
        let c = v.view_proj.to_cols_array_2d();
        rows[b..b + 4].copy_from_slice(&c);
        let (x, y, s) = v.tile;
        rows[b + 4] = [x as f32 * inv, y as f32 * inv, s as f32 * inv, filter_texels * inv];
        let w = if v.ortho { -v.texel_m } else { v.texel_m };
        rows[b + 5] = [v.origin.x, v.origin.y, v.origin.z, w];
        rows[b + 6] = [if v.ortho || !v.range.is_finite() { 0.0 } else { v.range }, 0.0, 0.0, 0.0];
    }
    rows
}

/// What the shadows did this frame, for the log line and the Dev readouts.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ShadowStats {
    /// Room lights this frame, and how many of them could be shadowed (points and spots in view).
    pub lights: u32,
    pub candidates: u32,
    /// Lights given shadows, and lights left without them for want of budget or atlas room.
    pub shadowed: u32,
    pub over_budget: u32,
    /// Views with a tile, and caster draws across them; the casters near enough to be considered.
    pub views: u32,
    pub draws: u32,
    pub casters: u32,
    pub near_sun: bool,
    pub atlas: u32,
    /// Whether the shading has the atlas lookups compiled in this frame (`Pipeline::
    /// shadow_atlas_compiled`), and whether the other variant is compiling (shadows rung 1b): at Off
    /// with the lookups still in, the scene pass still pays for them until the build lands.
    pub lookups_compiled: bool,
    pub lookups_building: bool,
    /// (Caster, face) pairs the shadow-caster cull left out this frame (`shadow_reaches_view`,
    /// shadows rung 1b): casters whose shadow cannot reach anything the camera sees.
    pub culled: u32,
}

/// The last frame's stats, for the Settings line (the GUI has no renderer to ask).
static PUBLISHED: std::sync::Mutex<Option<ShadowStats>> = std::sync::Mutex::new(None);

/// What the shadow atlas did on the last frame that drew the world (None before the first).
pub fn published_stats() -> Option<ShadowStats> {
    PUBLISHED.lock().ok().and_then(|g| *g)
}

fn publish(s: ShadowStats) {
    if let Ok(mut g) = PUBLISHED.lock() {
        *g = Some(s);
    }
}

/// One caster this frame: the object (in render space), its world bounding sphere, and how it draws.
#[derive(Clone)]
struct Caster {
    obj: RenderObject,
    centre: Vec3,
    radius: f32,
    cutout: bool,
    /// Draws through the fast path (`takes_fast_path`), instanced by mesh.
    fast: bool,
    /// Casts into the near sun view only (a celestial build piece: the room lights never reach the
    /// celestial pass's fragments).
    sun_only: bool,
    /// The fast path's instance matrix (the model matrix, column-major), worked out once a caster
    /// rather than once for every view that draws it.
    model: [f32; 16],
}

/// Whether a caster can take the fast path (`assets/shaders/shadow_atlas_instanced.wgsl`): a plain
/// surface whose vertices the megashader's vs_main never moves and that needs no alpha test. The wind
/// classes (material types 19 to 23) and the water shell (16) displace their vertices there, and a
/// cutout's shape is its texture, so those keep the classic path and their shadows match them.
pub fn takes_fast_path(material_type: f32, cutout: bool) -> bool {
    !cutout && pipeline::shader_class(material_type) == ShaderClass::Surface && !(18.5..19.5).contains(&material_type)
}

/// One draw of the fast path: a mesh and its casters' instance range in this frame's instance buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FastRun {
    pub mesh: usize,
    pub first: u32,
    pub count: u32,
}

/// The fast path's runs for a set of (caster, view) pairs (the renderer passes one view's): sorted by
/// mesh, each run of one mesh a single instanced draw. Appends the pairs in run order to `order` (the
/// instance buffer's order) and returns the runs, whose `first` counts from `order`'s length on entry.
pub fn fast_runs(items: &[(u32, u32)], mesh_of: impl Fn(u32) -> usize, order: &mut Vec<(u32, u32)>) -> Vec<FastRun> {
    let mut sorted: Vec<(u32, u32)> = items.to_vec();
    sorted.sort_by_key(|&(c, v)| (mesh_of(c), v, c));
    let mut runs: Vec<FastRun> = Vec::new();
    for (c, v) in sorted {
        let mesh = mesh_of(c);
        let at = order.len() as u32;
        order.push((c, v));
        match runs.last_mut() {
            Some(r) if r.mesh == mesh => r.count += 1,
            _ => runs.push(FastRun { mesh, first: at, count: 1 }),
        }
    }
    runs
}

/// The fast path's GPU side: its pipeline, the views' matrices (a storage array the instances index)
/// and the casters' instances (a model matrix each, one per caster per view). Both buffers are made at
/// start for the Full level's worst case and grow (rarely) by doubling.
struct FastPath {
    pso: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    views: wgpu::Buffer,
    view_cap: u64,
    group: wgpu::BindGroup,
    instances: wgpu::Buffer,
    instance_cap: u64,
}

/// One view in the fast path's storage array: its view-projection matrix.
const FAST_VIEW_BYTES: u64 = 64;
/// One caster instance: its model matrix, four columns, the view's index in the first column's w.
const FAST_INSTANCE_BYTES: u64 = 64;
/// The fast path's shader (its own small module, not the megashader: no `PSO_REGISTRY` row).
const FAST_WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/shadow_atlas_instanced.wgsl"));

impl FastPath {
    fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shadow Atlas Instanced Shader"),
            source: wgpu::ShaderSource::Wgsl(FAST_WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Atlas Instanced Views"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(FAST_VIEW_BYTES),
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Shadow Atlas Instanced Layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let instance_attrs = [
            wgpu::VertexAttribute { offset: 0, shader_location: 1, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
        ];
        let position = [wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 }];
        let pso = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Atlas Instanced Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_shadow_instanced"),
                buffers: &[
                    // The mesh's own vertices (humanity_core::vertex::Vertex, 32 bytes): the position only.
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<humanity_core::vertex::Vertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &position,
                    },
                    wgpu::VertexBufferLayout { array_stride: FAST_INSTANCE_BYTES, step_mode: wgpu::VertexStepMode::Instance, attributes: &instance_attrs },
                ],
                compilation_options: Default::default(),
            },
            // Depth only: no fragment stage, so the depth test runs early.
            fragment: None,
            // The sun shadow PSOs' state (pipeline.rs `build_skinned_shadow_pso`): the two paths write
            // the same depth for the same triangle.
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        // Full's worst case: the near sun tile and eight cube lights, 49 views; 16,384 instances (1 MB)
        // is several times the ground hall's 4,400 (caster, view) pairs.
        let view_cap = 64;
        let views = Self::view_buffer(device, view_cap);
        let group = Self::view_group(device, &layout, &views);
        let instance_cap = 16_384;
        let instances = Self::instance_buffer(device, instance_cap);
        FastPath { pso, layout, views, view_cap, group, instances, instance_cap }
    }

    fn view_buffer(device: &wgpu::Device, n: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Atlas Instanced Views"),
            size: n * FAST_VIEW_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn view_group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, views: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Atlas Instanced Views BG"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: views.as_entire_binding(),
            }],
        })
    }

    fn instance_buffer(device: &wgpu::Device, n: u64) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Atlas Caster Instances"),
            size: n * FAST_INSTANCE_BYTES,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Write this frame's view matrices and caster instances, growing a buffer that is too small.
    fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, views: &[[f32; 16]], instances: &[[f32; 16]]) {
        let n_views = views.len() as u64;
        if n_views > self.view_cap {
            self.view_cap = n_views.next_power_of_two();
            self.views = Self::view_buffer(device, self.view_cap);
            self.group = Self::view_group(device, &self.layout, &self.views);
        }
        if instances.len() as u64 > self.instance_cap {
            self.instance_cap = (instances.len() as u64).next_power_of_two();
            self.instances = Self::instance_buffer(device, self.instance_cap);
        }
        if !views.is_empty() {
            queue.write_buffer(&self.views, 0, bytemuck::cast_slice(views));
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(instances));
        }
    }

    fn vram_bytes(&self) -> u64 {
        self.views.size() + self.instances.size()
    }
}

/// Which list a group of casters came from, and so which views it casts into.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CasterList {
    /// The scene list (homes, furniture, bodies, the piece in hand): every view.
    Scene,
    /// The celestial list: the near sun view only, and only its plain surfaces (build pieces, props),
    /// never the terrain or the vegetation, which the far map already holds.
    CelestialSurfaces,
}

/// The renderer's shadow-atlas state.
pub struct LocalShadows {
    /// Showcase pin over the setting (`local_shadows` "0" / "1" / "auto" / a level id): `None` follows
    /// Settings, `Some("")` = off, a level id = that level.
    pub pin: Option<String>,
    /// Showcase pin over the near sun view (`near_levels` "0" / "1" / "auto").
    pub near_pin: Option<bool>,
    /// Settings > Graphics > Room light shadows: the level id (data/gui/shadow_levels.json), set each
    /// frame by lib.rs.
    pub level: String,
    /// Showcase pin `fp_shadow_twin` "0": the first-person body pushes no unfolded twin, so its
    /// shadow loses the head again (the twin's red twin; engine/animation.rs).
    pub twin_off: bool,
    /// Showcase pin `atlas_code` "1": keep the atlas lookups compiled into the shading even at Off,
    /// the rung 1 state (the red twin of compiling them out, shadows rung 1b: what Off cost before).
    pub keep_code: bool,
    /// Showcase pin `shadow_cull` "0": every caster inside a face's frustum is drawn, whether or not
    /// its shadow can reach the view (the shadow-caster cull's red twin, shadows rung 1b: the picture
    /// must not change, only the cost).
    pub cull_off: bool,
    atlas: Option<(wgpu::Texture, wgpu::TextureView, u32)>,
    dummy_view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    table: wgpu::Buffer,
    table_rows: u64,
    /// The views' camera uniforms for the classic and skinned paths: ONE buffer, a slot of
    /// `view_stride` bytes a view, written in one upload a frame (a queue write per view cost about
    /// 15 microseconds each: 40 views were most of the "uploads" share of cpu.shadow_local,
    /// 2026-11-11). Grows by doubling; `view_groups` are made again when it does.
    view_cams: Option<wgpu::Buffer>,
    view_stride: u64,
    /// Group 0 per view, each binding its slot of `view_cams` (and the 1x1 dummy at the atlas
    /// binding: the atlas is the pass's own depth attachment). Cleared when the camera groups are
    /// rebuilt (a buffer they bind grew).
    view_groups: Vec<wgpu::BindGroup>,
    /// The fast caster path (plain surfaces, instanced by mesh).
    fast: FastPath,
    /// Showcase pin `shadow_fast` "0": every caster takes the classic path (the fast path's red twin:
    /// the picture must not change, only the cost).
    pub fast_off: bool,
    /// Last frame's shadowed lights (the hysteresis; empty with reuse off).
    prev: Vec<(i32, i32, i32)>,
    pub stats: ShadowStats,
    last_log: Option<(std::time::Instant, ShadowStats)>,
    /// The log line's stage times, smoothed over about twenty frames (microseconds): one frame's split
    /// is noisy on a busy machine.
    split_avg: [f32; 5],
}

impl LocalShadows {
    pub fn new(device: &wgpu::Device) -> Self {
        let dummy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Atlas Stand-in"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Atlas Comparison Sampler"),
            compare: Some(wgpu::CompareFunction::LessEqual),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let table_rows = 256u64;
        let table = Self::table_buffer(device, table_rows);
        LocalShadows {
            pin: None,
            near_pin: None,
            level: DEFAULT_LEVEL.to_string(),
            twin_off: false,
            keep_code: false,
            cull_off: false,
            atlas: None,
            dummy_view,
            sampler,
            table,
            table_rows,
            view_cams: None,
            // The camera uniform rounded up to the uniform offset alignment the device asks for.
            view_stride: (std::mem::size_of::<camera::CameraUniforms>() as u64).next_multiple_of(device.limits().min_uniform_buffer_offset_alignment.max(16) as u64),
            view_groups: Vec::new(),
            fast: FastPath::new(device),
            fast_off: false,
            prev: Vec::new(),
            stats: ShadowStats::default(),
            last_log: None,
            split_avg: [0.0; 5],
        }
    }

    fn table_buffer(device: &wgpu::Device, rows: u64) -> wgpu::Buffer {
        // Zeroed at creation: row 0 all zeros = no shadowed lights and no near sun view.
        device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Shadow Atlas Table"),
            size: rows * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// What the colour passes bind at group 0 binding 8: the atlas, or the 1x1 stand-in when this
    /// level has none (the table then says nothing is shadowed, so it is never sampled).
    pub fn atlas_view(&self) -> &wgpu::TextureView {
        self.atlas.as_ref().map_or(&self.dummy_view, |a| &a.1)
    }

    /// What the colour passes (and the sun shadow pass) bind at group 0 bindings 8-10.
    pub fn colour_binding(&self) -> pipeline::ShadowAtlasBinding<'_> {
        pipeline::ShadowAtlasBinding { atlas: self.atlas_view(), sampler: &self.sampler, table: &self.table }
    }

    /// What the atlas pass's own group 0 binds: the 1x1 stand-in at 8 (the atlas is the pass's depth
    /// attachment), the same sampler and table.
    pub fn atlas_pass_binding(&self) -> pipeline::ShadowAtlasBinding<'_> {
        pipeline::ShadowAtlasBinding { atlas: &self.dummy_view, sampler: &self.sampler, table: &self.table }
    }

    /// The per-view groups bind buffers that just grew: make them again on next need.
    pub fn forget_view_groups(&mut self) {
        self.view_groups.clear();
    }

    /// Video memory: the atlas, the table and the views' camera buffers.
    pub fn vram_bytes(&self) -> u64 {
        let atlas = self.atlas.as_ref().map_or(0, |a| a.2 as u64 * a.2 as u64 * 4);
        atlas + self.table.size() + self.view_cams.as_ref().map_or(0, |b| b.size()) + self.fast.vram_bytes()
    }

    /// The atlas side in use (0 = none).
    pub fn atlas_side(&self) -> u32 {
        self.atlas.as_ref().map_or(0, |a| a.2)
    }
}

impl Renderer {
    /// The level the shadows run at this frame: the pin, else the setting.
    fn shadow_level_now(&self) -> Option<&'static ShadowLevel> {
        let id = match self.local_shadows.pin.as_deref() {
            Some("") => return None,
            Some(id) => id,
            None => self.local_shadows.level.as_str(),
        };
        let l = level(id);
        (l.atlas > 0).then_some(l)
    }

    /// Whether anything will draw into the shadow maps this frame (the first-person body pushes its
    /// unfolded twin only then: renderer/skinning.rs `skin_shadow_only`).
    pub fn shadow_casters_wanted(&self) -> bool {
        self.shadow_level_now().is_some()
    }

    /// Whether the first-person body should push its unfolded twin for the shadow maps this frame.
    pub fn shadow_twin_wanted(&self) -> bool {
        self.shadow_casters_wanted() && !self.local_shadows.twin_off
    }

    /// Allocate (or drop) the atlas for `side`; true when the colour groups must be rebuilt.
    fn ensure_atlas(&mut self, side: u32) -> bool {
        if self.local_shadows.atlas_side() == side {
            return false;
        }
        self.local_shadows.atlas = (side > 0).then(|| {
            let tex = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Shadow Atlas"),
                size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth32Float,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            log::info!("[Shadows] shadow atlas {side} x {side} ({:.0} MB)", (side as f64 * side as f64 * 4.0) / 1_048_576.0);
            (tex, view, side)
        });
        true
    }

    /// The views' camera buffer (room for at least `n` views; made for 64, Full's 49 and some, and
    /// doubled when a level asks for more) and each view's group 0 over its slot (made on first need).
    fn ensure_view_groups(&mut self, n: usize) {
        let stride = self.local_shadows.view_stride;
        let cap = self.local_shadows.view_cams.as_ref().map_or(0, |b| b.size() / stride) as usize;
        if cap < n {
            let views = n.max(64).next_power_of_two() as u64;
            self.local_shadows.view_cams = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Shadow View Cameras"),
                size: views * stride,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.local_shadows.view_groups.clear();
        }
        let buf = self.local_shadows.view_cams.as_ref().expect("made above");
        while self.local_shadows.view_groups.len() < n {
            let k = self.local_shadows.view_groups.len() as u64;
            let slot = wgpu::BufferBinding { buffer: buf, offset: k * stride, size: wgpu::BufferSize::new(std::mem::size_of::<camera::CameraUniforms>() as u64) };
            let group = pipeline::camera_bind_group(
                &self.device,
                &self.pipeline.camera_bind_group_layout,
                "Shadow View Camera BG",
                slot,
                &self.lights_buffer,
                &self.tile_counts_buffer,
                &self.tile_indices_buffer,
                &self.env_regions_buffer,
                &self.room_gi,
                self.local_shadows.atlas_pass_binding(),
            );
            self.local_shadows.view_groups.push(group);
        }
    }

    /// Write the table, growing its buffer when needed (a grow rebuilds every camera group).
    fn write_shadow_table(&mut self, rows: &[[f32; 4]]) {
        let need = rows.len().max(1) as u64;
        if need > self.local_shadows.table_rows {
            let grown = need.next_power_of_two();
            self.local_shadows.table = LocalShadows::table_buffer(&self.device, grown);
            self.local_shadows.table_rows = grown;
            self.rebuild_camera_bind_groups();
        }
        if rows.is_empty() {
            self.queue.write_buffer(&self.local_shadows.table, 0, bytemuck::cast_slice(&[[0.0f32; 4]]));
        } else {
            self.queue.write_buffer(&self.local_shadows.table, 0, bytemuck::cast_slice(rows));
        }
    }

    /// THE SHADOW ATLAS FOR THIS FRAME (see the file's header). Call once a frame after the lights
    /// (`set_point_lights`), the sun (`set_sun_light`) and the people's skinning (`skin_submit`) are
    /// set, and BEFORE `render_celestial_onto`: the terrain and the scene sample what this draws.
    /// `lists` are the casters with the offset that takes each list into render space (the home's
    /// list rides the station: `station_off`).
    pub fn render_shadow_atlas(&mut self, camera: &Camera, lists: &[(&[RenderObject], Vec3, CasterList)]) {
        let level = self.shadow_level_now();
        // Off compiles the lookups out of the shading (shadows rung 1b, `pipeline::ATLAS_TWINS`): the
        // variant follows the level, built in the background when it crosses Off. Showcase
        // `atlas_code` "1" keeps them in at Off, as rung 1 did (its red twin).
        self.pipeline.want_shadow_atlas(&self.device, level.is_some() || self.local_shadows.keep_code);
        let lookups_compiled = self.pipeline.shadow_atlas_compiled();
        let lookups_building = self.pipeline.shadow_atlas_building();
        let side = level.map_or(0, |l| l.atlas);
        if self.ensure_atlas(side) {
            self.rebuild_camera_bind_groups();
        }
        let Some(level) = level else {
            self.write_shadow_table(&[]);
            let stats = ShadowStats { lookups_compiled, lookups_building, ..ShadowStats::default() };
            self.local_shadows.stats = stats;
            self.local_shadows.prev.clear();
            publish(stats);
            return;
        };
        let eye = camera.effective_position();
        let cam_planes = frustum_planes(&camera.view_projection_matrix());
        let filter = level.filter_texels();
        let margin = filter + 1.5;

        // The camera's forward, for the probes the lights are judged at: the view matrix's own -z, so it
        // holds in every camera mode (the surface basis, third person, a frame-locked planet).
        let forward = (camera.view_matrix().inverse() * Vec4::new(0.0, 0.0, -1.0, 0.0)).truncate().normalize_or_zero();
        let probes = view_probes(eye, forward);

        // ── 1. The near sun view's frustum and the lights that matter most. ──
        let near_cost = frame_costs::stage("cpu.shadow_near");
        let sun_on = self.local_shadows.near_pin.unwrap_or(true)
            && self.sun_cascades.shadows_on(self.sun_shadows)
            && level.sun_tile > 0
            && self.cur_sun.2 > 0.0;
        let toward_sun = Vec3::from(self.cur_sun.0);
        let sun_view = (sun_on && toward_sun.length_squared() > 1e-8).then(|| near_sun_view(eye, toward_sun, level.sun_extent_m, level.sun_tile));
        let sun_planes = sun_view.as_ref().map(frustum_planes);
        let sun_texel = level.sun_extent_m * 2.0 / level.sun_tile.max(1) as f32;
        drop(near_cost);
        let local_cost = frame_costs::stage("cpu.shadow_local");
        let prev: Vec<(i32, i32, i32)> = if humanity_core::reuse::enabled() { std::mem::take(&mut self.local_shadows.prev) } else { Vec::new() };
        let lights: Vec<RoomLight> = self.cur_lights.clone();
        let candidates = lights.iter().filter(|l| light_importance(l, &probes, &cam_planes).is_some()).count() as u32;
        let picked = select_lights(&lights, &probes, &cam_planes, level.lights as usize, &prev);
        let reaches: Vec<f32> = picked.iter().map(|&li| shadow_reach(&lights[li])).collect();
        drop(local_cost);

        // ── 2. The casters near enough to matter: inside the near sun view or a picked light's reach,
        //       in render space with their bounding spheres. ──
        let near_cost = frame_costs::stage("cpu.shadow_near");
        let mut casters: Vec<Caster> = Vec::new();
        for &(list, off, kind) in lists {
            for o in list.iter() {
                let Some(mesh) = self.meshes.get(o.mesh) else { continue };
                if mesh.index_count == 0 {
                    continue;
                }
                let [bx, by, bz, br] = mesh.bounds;
                let pos = o.position + off;
                let (centre, radius) = if br < 0.0 {
                    (pos, f32::INFINITY)
                } else {
                    (pos + o.rotation * (o.scale * Vec3::new(bx, by, bz)), br * o.scale.abs().max_element())
                };
                let sun_only = kind == CasterList::CelestialSurfaces;
                // A celestial caster is a build piece or a prop: never a planet, a moon or the Sun,
                // whose coarse spheres would stamp the ground they stand for into the tile.
                if sun_only && !(radius.is_finite() && radius < 50.0) {
                    continue;
                }
                let in_sun = sun_planes.as_ref().is_some_and(|p| sphere_in(p, centre, radius));
                let in_light = !sun_only && picked.iter().zip(&reaches).any(|(&li, &reach)| !radius.is_finite() || centre.distance(lights[li].pos) < reach + radius);
                if !in_sun && !in_light {
                    continue;
                }
                let Some(mat) = self.materials.get(o.material) else { continue };
                if !casts(mat) {
                    continue;
                }
                let class = pipeline::shader_class(mat.material_type);
                if !class.draws_opaque() || (sun_only && class != ShaderClass::Surface) {
                    continue;
                }
                let mut obj = o.clone();
                obj.position = pos;
                let cutout = mat.casts_cutout_shadow();
                let fast = !self.local_shadows.fast_off && takes_fast_path(mat.material_type, cutout);
                let model = if fast { Mat4::from_scale_rotation_translation(obj.scale, obj.rotation, obj.position).to_cols_array() } else { [0.0; 16] };
                casters.push(Caster { obj, centre, radius, cutout, fast, sun_only, model });
                if casters.len() >= MAX_OBJECTS {
                    break;
                }
            }
        }

        // ── 3. The near sun view. ──
        let mut views: Vec<AtlasView> = Vec::new();
        let mut view_casters: Vec<Vec<u32>> = Vec::new();
        let mut near_sun: Option<usize> = None;
        if let (Some(vp), Some(planes)) = (sun_view, sun_planes.as_ref()) {
            let list: Vec<u32> = (0..casters.len() as u32)
                .filter(|&i| {
                    let c = &casters[i as usize];
                    sphere_in(planes, c.centre, c.radius) && big_enough(c.centre, c.radius, eye, sun_texel, true)
                })
                .collect();
            near_sun = Some(views.len());
            views.push(AtlasView { view_proj: vp, tile: (0, 0, level.sun_tile), origin: toward_sun.normalize(), ortho: true, texel_m: sun_texel, range: f32::INFINITY });
            view_casters.push(list);
        }
        drop(near_cost);

        // ── 4. The picked lights' views, their casters and their places in the atlas. ──
        let local_cost = frame_costs::stage("cpu.shadow_local");
        // Where cpu.shadow_local goes, for the log line (the dev answer to "what does it spend on?"):
        // culling the faces, the uploads, the near bodies' skinning, recording the draws, the submit.
        let mut mark = std::time::Instant::now();
        let mut split_us = [0u32; 5];
        let mut lap = |i: usize, mark: &mut std::time::Instant| {
            let now = std::time::Instant::now();
            split_us[i] = now.duration_since(*mark).as_micros() as u32;
            *mark = now;
        };
        // Each picked light's casters (inside its reach), once, before its faces split them.
        let in_reach: Vec<Vec<u32>> = picked
            .iter()
            .zip(&reaches)
            .map(|(&li, &reach)| {
                let l = &lights[li];
                (0..casters.len() as u32)
                    .filter(|&i| {
                        let c = &casters[i as usize];
                        !c.sun_only && (!c.radius.is_finite() || c.centre.distance(l.pos) < reach + c.radius)
                    })
                    .collect()
            })
            .collect();
        let mut faces: Vec<u32> = picked.iter().enumerate().map(|(r, _)| face_for_rank(r, level.face, level.face_min)).collect();
        let kinds: Vec<u32> = picked
            .iter()
            .map(|&li| if lights[li].cos_outer > -1.0 && spot_fits_one_view(lights[li].cos_outer) { KIND_SPOT } else { KIND_CUBE })
            .collect();
        let tan_half_of = |k: usize, li: usize, res: u32| -> f32 {
            if kinds[k] == KIND_SPOT {
                let half = lights[li].cos_outer.clamp(-1.0, 1.0).acos();
                (half.tan() * face_tan_half(res, margin)).min(80f32.to_radians().tan())
            } else {
                face_tan_half(res, margin)
            }
        };
        // Which faces of each light have a caster in them and touch what the camera sees (a face with
        // neither needs no tile and reads lit, which no visible surface will ask). A light whose faces
        // do not all fit is halved and the atlas packed again; a light that does not fit at its
        // smallest is left out (unshadowed) and the rest packed without it, so a light is either
        // wholly in the atlas or not at all (a face with casters and no tile would read lit).
        let mut face_lists: Vec<Vec<(Mat4, Vec<u32>)>> = Vec::new();
        let mut placed: Vec<Vec<Option<(u32, u32)>>> = Vec::new();
        let mut excluded = vec![false; picked.len()];
        let mut settled = false;
        let mut culled = 0u32;
        for _round in 0..8 {
            face_lists.clear();
            culled = 0;
            let mut sizes: Vec<u32> = Vec::new();
            if let Some(v) = near_sun {
                sizes.push(views[v].tile.2);
            }
            let mut owner: Vec<(usize, usize)> = Vec::new();
            for (k, &li) in picked.iter().enumerate() {
                let l = &lights[li];
                let n = if kinds[k] == KIND_SPOT { 1 } else { 6 };
                let texel_m = 2.0 * tan_half_of(k, li, faces[k]) / faces[k] as f32;
                let mut fl = Vec::with_capacity(n);
                for f in 0..n {
                    let vp = if kinds[k] == KIND_SPOT {
                        spot_view(l.pos, l.dir, l.cos_outer, reaches[k], faces[k], margin)
                    } else {
                        cube_face_view(l.pos, f, reaches[k], faces[k], margin)
                    };
                    let list: Vec<u32> = if view_touches(&vp, &cam_planes) {
                        let planes = frustum_planes(&vp);
                        in_reach[k]
                            .iter()
                            .copied()
                            .filter(|&i| {
                                let c = &casters[i as usize];
                                if !(sphere_in(&planes, c.centre, c.radius) && big_enough(c.centre, c.radius, l.pos, texel_m, false)) {
                                    return false;
                                }
                                // Only a caster whose shadow can fall on something in view (the
                                // filter's few texels of spread as the pad).
                                let seen = self.local_shadows.cull_off || shadow_reaches_view(&cam_planes, l.pos, c.centre, c.radius, reaches[k], texel_m * margin);
                                culled += u32::from(!seen);
                                seen
                            })
                            .collect()
                    } else {
                        Vec::new()
                    };
                    if !list.is_empty() && !excluded[k] {
                        sizes.push(faces[k]);
                        owner.push((k, f));
                    }
                    fl.push((vp, list));
                }
                face_lists.push(fl);
            }
            let spots = pack_tiles(&sizes, level.atlas);
            let base = usize::from(near_sun.is_some());
            if let (Some(v), Some(Some((x, y)))) = (near_sun, spots.first()) {
                views[v].tile = (*x, *y, views[v].tile.2);
            }
            placed = face_lists.iter().map(|fl| vec![None; fl.len()]).collect();
            let mut failed = vec![false; picked.len()];
            for (j, &(k, f)) in owner.iter().enumerate() {
                match spots.get(base + j).copied().flatten() {
                    Some(at) => placed[k][f] = Some(at),
                    None => failed[k] = true,
                }
            }
            if !failed.iter().any(|&f| f) {
                settled = true;
                break;
            }
            for k in 0..picked.len() {
                if failed[k] {
                    if faces[k] / 2 >= level.face_min.max(TILE_UNIT) {
                        faces[k] /= 2;
                    } else {
                        excluded[k] = true;
                    }
                }
            }
        }
        if !settled {
            // Out of rounds: nothing is left half placed.
            excluded.iter_mut().for_each(|e| *e = true);
        }
        lap(0, &mut mark);
        let mut lookups: Vec<(usize, u32)> = Vec::new();
        let mut shadowed_keys: Vec<(i32, i32, i32)> = Vec::new();
        // What each shadowed light is, for the log line (the dev answer to "which lamps cast?").
        let mut words: Vec<String> = Vec::new();
        for (k, &li) in picked.iter().enumerate() {
            if excluded[k] {
                continue;
            }
            let (Some(p), Some(fl)) = (placed.get(k), face_lists.get(k)) else { continue };
            // Every face with a caster must have its tile, or the light stays unshadowed.
            if fl.iter().zip(p.iter()).any(|((_, list), at)| !list.is_empty() && at.is_none()) {
                continue;
            }
            let l = &lights[li];
            let first = views.len();
            let texel_m = 2.0 * tan_half_of(k, li, faces[k]) / faces[k] as f32;
            for (f, (vp, list)) in fl.iter().enumerate() {
                let tile = p[f].map_or((0, 0, 0), |(x, y)| (x, y, faces[k]));
                views.push(AtlasView { view_proj: *vp, tile, origin: l.pos, ortho: false, texel_m, range: reaches[k] });
                view_casters.push(if tile.2 > 0 { list.clone() } else { Vec::new() });
            }
            lookups.push((li, kinds[k] * KIND_STRIDE + first as u32 + 1));
            shadowed_keys.push(light_key(l));
            words.push(format!(
                "#{li} at ({:.1}, {:.1}, {:.1}) intensity {:.0} reach {:.1} of {:.0} m, {} faces at {}",
                l.pos.x,
                l.pos.y,
                l.pos.z,
                l.intensity,
                reaches[k],
                l.range,
                p.iter().filter(|x| x.is_some()).count(),
                faces[k]
            ));
        }
        if humanity_core::reuse::enabled() {
            self.local_shadows.prev = shadowed_keys;
        }

        // ── 5. The table, the views' cameras and the object uniforms of the casters some view draws. ──
        let rows = pack_table(&lookups, &views, near_sun, level.atlas, filter, level.taps);
        self.write_shadow_table(&rows);
        let drawn: Vec<usize> = (0..views.len()).filter(|&k| views[k].tile.2 > 0).collect();
        // Which drawn views have classic work: a caster the fast path does not take, or a skinned body
        // the scene list does not hold (the middle tier, the first-person twin). Only those get the
        // megashader's group 0 written and bound: most faces hold only plain surfaces.
        let view_planes: Vec<[Vec4; 6]> = views.iter().map(|v| frustum_planes(&v.view_proj)).collect();
        let bodies = self.skinned_shadow_spheres();
        let body_in = |k: usize, c: Vec3, r: f32| sphere_in(&view_planes[k], c, r) && (views[k].ortho || c.distance(views[k].origin) < views[k].range + r);
        let classic: Vec<bool> = (0..views.len())
            .map(|k| views[k].tile.2 > 0 && (view_casters[k].iter().any(|&ci| !casters[ci as usize].fast) || bodies.iter().any(|&(c, r)| body_in(k, c, r))))
            .collect();
        self.ensure_view_groups(views.len());
        // Every classic view's camera in its slot of the one buffer, in ONE upload up to the last.
        if let Some(last) = drawn.iter().copied().filter(|&k| classic[k]).max() {
            let base_u = self.lit_uniform(camera.uniforms());
            let stride = self.local_shadows.view_stride as usize;
            let mut staging = vec![0u8; last * stride + std::mem::size_of::<camera::CameraUniforms>()];
            for &k in &drawn {
                if !classic[k] {
                    continue;
                }
                let mut u = base_u;
                u.view_proj = views[k].view_proj.to_cols_array_2d();
                let bytes = bytemuck::bytes_of(&u);
                staging[k * stride..k * stride + bytes.len()].copy_from_slice(bytes);
            }
            if let Some(buf) = self.local_shadows.view_cams.as_ref() {
                self.queue.write_buffer(buf, 0, &staging);
            }
        }
        // The fast path: every view's matrix, each drawn view's plain surfaces sorted into runs of one
        // mesh (one instanced draw each), their model matrices in run order in one instance upload, the
        // view's index in each matrix's spare w.
        let fast_views: Vec<[f32; 16]> = views.iter().map(|v| v.view_proj.to_cols_array()).collect();
        let mesh_of = |ci: u32| casters[ci as usize].obj.mesh;
        let mut order: Vec<(u32, u32)> = Vec::new();
        let mut runs: Vec<Vec<FastRun>> = vec![Vec::new(); views.len()];
        for &k in &drawn {
            let pairs: Vec<(u32, u32)> = view_casters[k].iter().filter(|&&ci| casters[ci as usize].fast).map(|&ci| (ci, k as u32)).collect();
            runs[k] = fast_runs(&pairs, mesh_of, &mut order);
        }
        let instances: Vec<[f32; 16]> = order
            .iter()
            .map(|&(ci, k)| {
                let mut m = casters[ci as usize].model;
                m[3] = k as f32;
                m
            })
            .collect();
        self.local_shadows.fast.upload(&self.device, &self.queue, &fast_views, &instances);
        // The classic path: only the casters a drawn view holds and the fast path does not get a
        // uniform slot (the rest were culled), so the upload and its matrix inverses scale with what
        // is drawn, not with the scene.
        let mut slot = vec![u32::MAX; casters.len()];
        let mut used: Vec<usize> = Vec::new();
        for &k in &drawn {
            for &ci in &view_casters[k] {
                if !casters[ci as usize].fast && slot[ci as usize] == u32::MAX {
                    slot[ci as usize] = used.len() as u32;
                    used.push(ci as usize);
                }
            }
        }
        self.upload_object_uniforms(used.iter().map(|&ci| &casters[ci].obj));
        lap(1, &mut mark);

        // ── 6. Draw: this frame's skinning first (the near bodies' vertices), then each view. ──
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Shadow Atlas Encoder") });
        self.encode_skinning(&mut encoder);
        lap(2, &mut mark);
        let mut draws = 0u32;
        let mut fast_instances = 0u32;
        let atlas_view = &self.local_shadows.atlas.as_ref().expect("allocated above").1;
        // Two passes so each has its own timer: the near sun tile (clears the atlas), then the
        // room lights' faces.
        let groups: [(&'static str, &'static str, Vec<usize>); 2] = [
            ("Near Sun Shadow Pass", "gpu.shadow_near", drawn.iter().copied().filter(|&k| Some(k) == near_sun).collect()),
            ("Room Light Shadow Pass", "gpu.shadow_local", drawn.iter().copied().filter(|&k| Some(k) != near_sun).collect()),
        ];
        let mut cleared = false;
        for (label, timer, ks) in groups.iter() {
            if ks.is_empty() && cleared {
                continue;
            }
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                timestamp_writes: if ks.is_empty() { None } else { self.pass_timer(timer) },
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: atlas_view,
                    depth_ops: Some(wgpu::Operations {
                        load: if cleared { wgpu::LoadOp::Load } else { wgpu::LoadOp::Clear(1.0) },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            cleared = true;
            // Whether the fast path's pipeline and bindings are the pass's current state (the classic
            // path's views change them).
            let mut fast_bound = false;
            for &k in ks {
                let v = &views[k];
                let (x, y, s) = v.tile;
                pass.set_viewport(x as f32, y as f32, s as f32, s as f32, 0.0, 1.0);
                pass.set_scissor_rect(x, y, s, s);
                // The fast path first: one instanced draw per run of one mesh, no per-caster binding
                // (the instance carries the caster's matrix and this view's index).
                if !runs[k].is_empty() {
                    let fp = &self.local_shadows.fast;
                    if !fast_bound {
                        pass.set_pipeline(&fp.pso);
                        pass.set_bind_group(0, &fp.group, &[]);
                        pass.set_vertex_buffer(1, fp.instances.slice(..));
                        fast_bound = true;
                    }
                    for r in &runs[k] {
                        let Some(mesh) = self.meshes.get(r.mesh) else { continue };
                        mesh.bind_vertices(&mut pass);
                        pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        pass.draw_indexed(mesh.indices(), mesh.base_vertex, r.first..r.first + r.count);
                        draws += 1;
                        fast_instances += r.count;
                    }
                }
                // Then the classic path (cutouts, swaying classes) and the skinned bodies, which share
                // the megashader's group 0: only in the views that have any.
                if !classic[k] {
                    continue;
                }
                fast_bound = false;
                pass.set_bind_group(0, &self.local_shadows.view_groups[k], &[]);
                pass.set_bind_group(1, &self.object_bind_group, &[0]);
                for cutout in [false, true] {
                    if !view_casters[k].iter().any(|&ci| !casters[ci as usize].fast && casters[ci as usize].cutout == cutout) {
                        continue;
                    }
                    pass.set_pipeline(self.pipeline.shadow_for(cutout));
                    pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                    let mut bound_material = usize::MAX;
                    let mut bound_mesh = usize::MAX;
                    for &ci in &view_casters[k] {
                        let c = &casters[ci as usize];
                        if c.fast || c.cutout != cutout {
                            continue;
                        }
                        let (Some(mesh), Some(material)) = (self.meshes.get(c.obj.mesh), self.materials.get(c.obj.material)) else { continue };
                        pass.set_bind_group(1, &self.object_bind_group, &[256 * slot[ci as usize]]);
                        if bound_material != c.obj.material {
                            bound_material = c.obj.material;
                            pass.set_bind_group(2, &material.bind_group, &[]);
                            pass.set_bind_group(3, material.shadow_albedo_group().unwrap_or(&self.shadow_pass_texture_bind_group), &[]);
                        }
                        if bound_mesh != c.obj.mesh {
                            bound_mesh = c.obj.mesh;
                            mesh.bind_vertices(&mut pass);
                            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                        }
                        pass.draw_indexed(mesh.indices(), mesh.base_vertex, 0..1);
                        draws += 1;
                    }
                }
                // The skinned bodies the scene list does not hold: the middle tier and the
                // first-person body's unfolded twin (renderer/skinning.rs).
                draws += self.draw_skinned_shadow(&mut pass, |c, r| body_in(k, c, r));
            }
        }
        lap(3, &mut mark);
        self.queue.submit(std::iter::once(encoder.finish()));
        lap(4, &mut mark);
        drop(local_cost);

        let stats = ShadowStats {
            lights: lights.len() as u32,
            candidates,
            shadowed: lookups.len() as u32,
            over_budget: candidates.saturating_sub(lookups.len() as u32),
            views: drawn.len() as u32,
            draws,
            casters: casters.len() as u32,
            near_sun: near_sun.is_some(),
            atlas: level.atlas,
            lookups_compiled,
            lookups_building,
            culled,
        };
        self.local_shadows.stats = stats;
        publish(stats);
        for (avg, &us) in self.local_shadows.split_avg.iter_mut().zip(&split_us) {
            *avg += (us as f32 - *avg) * 0.05;
        }
        let now = std::time::Instant::now();
        let say = match self.local_shadows.last_log {
            None => true,
            Some((t, s)) => {
                let quiet = now.duration_since(t).as_secs_f32();
                let changed = s.shadowed != stats.shadowed || s.near_sun != stats.near_sun || s.atlas != stats.atlas || (s.culled > 0) != (stats.culled > 0);
                quiet > 30.0 || (changed && quiet > 5.0)
            }
        };
        if say {
            log::info!(
                "[Shadows] atlas {}: {} of {} room lights shadowed ({} in view, {} without for the budget), {} views, {} caster draws of {} casters near ({} casters drawn instanced; {} caster-face pairs left out, their shadow out of view), near sun {}",
                stats.atlas,
                stats.shadowed,
                stats.lights,
                stats.candidates,
                stats.over_budget,
                stats.views,
                stats.draws,
                stats.casters,
                fast_instances,
                stats.culled,
                if stats.near_sun { "on" } else { "off" }
            );
            log::info!("[Shadows] the shadowed lamps, most important first: {}", words.join("; "));
            log::info!(
                "[Shadows] cpu this frame (microseconds; smoothed over about twenty frames in brackets): cull the faces {} ({:.0}), uploads {} ({:.0}), the near bodies' skinning {} ({:.0}), record the draws {} ({:.0}), submit {} ({:.0})",
                split_us[0],
                self.local_shadows.split_avg[0],
                split_us[1],
                self.local_shadows.split_avg[1],
                split_us[2],
                self.local_shadows.split_avg[2],
                split_us[3],
                self.local_shadows.split_avg[3],
                split_us[4],
                self.local_shadows.split_avg[4]
            );
            self.local_shadows.last_log = Some((now, stats));
        }
    }
}

/// Whether a material blocks light at all: a surface that gives off light (a lamp's bulb or panel,
/// a screen) does not, or every lamp would sit inside its own fixture's shadow.
fn casts(m: &Material) -> bool {
    m.emissive <= 0.05 && m.base_color[3] >= 0.999
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(pos: Vec3, intensity: f32, range: f32) -> RoomLight {
        RoomLight::point(pos, [1.0, 1.0, 1.0], intensity, range)
    }

    fn look() -> ([Vec3; 4], [Vec4; 6]) {
        let eye = Vec3::new(0.0, 1.6, 0.0);
        let vp = Mat4::perspective_rh(70f32.to_radians(), 16.0 / 9.0, 0.1, 500.0) * Mat4::look_at_rh(eye, eye + Vec3::NEG_Z, Vec3::Y);
        (view_probes(eye, Vec3::NEG_Z), frustum_planes(&vp))
    }

    /// The cube face is the major axis of the direction, both signs, ties to x then y; and the face
    /// a direction picks is the one whose view sees it inside its tile (the face views and the pick
    /// agree, which is what the shader relies on).
    #[test]
    fn the_cube_face_is_the_major_axis_and_its_view_sees_the_point() {
        assert_eq!(cube_face_of(Vec3::new(2.0, 1.0, -1.0)), 0);
        assert_eq!(cube_face_of(Vec3::new(-2.0, 1.0, -1.0)), 1);
        assert_eq!(cube_face_of(Vec3::new(0.5, 3.0, -1.0)), 2);
        assert_eq!(cube_face_of(Vec3::new(0.5, -3.0, 1.0)), 3);
        assert_eq!(cube_face_of(Vec3::new(0.5, 0.2, 1.0)), 4);
        assert_eq!(cube_face_of(Vec3::new(0.5, 0.2, -1.0)), 5);
        assert_eq!(cube_face_of(Vec3::new(1.0, 1.0, 1.0)), 0);
        let l = Vec3::new(3.0, 2.7, -4.0);
        let mut seed = 1u32;
        for _ in 0..500 {
            let mut r = || {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 * 2.0 - 1.0
            };
            let d = Vec3::new(r(), r(), r());
            if d.length() < 0.2 {
                continue;
            }
            let p = l + d * 3.0;
            let f = cube_face_of(p - l);
            let c = cube_face_view(l, f, 7.0, 512, 3.25) * p.extend(1.0);
            let ndc = c.truncate() / c.w;
            assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0, "face {f} must see {p:?}: ndc {ndc:?}");
            // And with its filter margin to spare, so a lookup at a face edge stays on the face.
            assert!(ndc.x.abs() < 1.0 - 2.0 / 512.0 && ndc.y.abs() < 1.0 - 2.0 / 512.0, "margin at {ndc:?}");
            assert!(ndc.z > 0.0 && ndc.z < 1.0, "depth inside 0..1: {}", ndc.z);
        }
    }

    /// Tiles taken largest first never overlap, stay inside the atlas, sit aligned to their own size
    /// (so a tile is a whole Morton block), and a full atlas refuses the rest (None) rather than
    /// overlapping. Seen red first (2026-11-10) with the sort and the alignment removed (tiles placed in
    /// request order at the cursor): "tile 9 (1024 texels at 3072, 768) is aligned to its size, left:
    /// (0, 768) right: (0, 0)".
    #[test]
    fn packed_tiles_never_overlap_and_a_full_atlas_says_no() {
        let sizes = [2048u32, 512, 512, 512, 512, 512, 512, 256, 256, 1024, 128, 64, 4096];
        let at = pack_tiles(&sizes, 4096);
        let rects: Vec<(usize, u32, u32, u32)> = at.iter().enumerate().filter_map(|(i, p)| p.map(|(x, y)| (i, x, y, sizes[i]))).collect();
        for &(i, x, y, s) in &rects {
            assert!(x + s <= 4096 && y + s <= 4096, "tile {i} leaves the atlas");
            assert_eq!((x % s, y % s), (0, 0), "tile {i} ({s} texels at {x}, {y}) is aligned to its size");
            for &(j, x2, y2, s2) in &rects {
                if i < j {
                    let apart = x + s <= x2 || x2 + s2 <= x || y + s <= y2 || y2 + s2 <= y;
                    assert!(apart, "tiles {i} and {j} overlap");
                }
            }
        }
        // The 4096 tile cannot fit beside anything else: refused.
        assert!(at[12].is_none() || rects.len() == 1);
        // Exactly full: four 2048s fit, a fifth does not.
        let full = pack_tiles(&[2048, 2048, 2048, 2048, 2048], 4096);
        assert_eq!(full.iter().filter(|p| p.is_some()).count(), 4);
    }

    /// The budget takes the lights that light what you look at: a lamp over the room ahead beats a
    /// brighter one far away and a brighter one whose reach holds the eye but lights little of what is
    /// ahead; a light behind the camera whose reach the view cannot see is not a candidate; a line
    /// light is never one in rung 1. Seen red first (2026-11-10) with the first ranking (brightness
    /// times the share of the view the reach covers): "left: [5, 0] right: [0, 4]", the bright panel
    /// in the next room over the lamp ahead, which is what the hall capture showed (eight shadowed
    /// lamps, none of them the lamp over the chair).
    #[test]
    fn the_budget_takes_the_lights_that_matter_most() {
        let (probes, view) = look();
        let lights = vec![
            pt(Vec3::new(0.0, 2.7, -3.0), 8.0, 7.0),   // over the room ahead: matters most
            pt(Vec3::new(0.0, 2.7, -60.0), 15.0, 7.0), // brighter, far
            pt(Vec3::new(0.0, 2.7, 40.0), 15.0, 7.0),  // behind, reach out of view
            RoomLight::line(Vec3::new(-1.0, 2.6, -2.0), Vec3::new(1.0, 2.6, -2.0), [1.0; 3], 6.0, 5.0),
            pt(Vec3::new(3.0, 2.7, -8.0), 4.0, 4.0),
            pt(Vec3::new(-11.0, 2.7, 3.0), 10.0, 14.0), // next room: bright, its reach holds the eye
        ];
        let picked = select_lights(&lights, &probes, &view, 2, &[]);
        assert_eq!(picked, vec![0, 4]);
        assert!(light_importance(&lights[2], &probes, &view).is_none());
        assert!(light_importance(&lights[3], &probes, &view).is_none());
        // The hysteresis lets last frame's light hold its place against a slightly stronger rival...
        let rivals = vec![pt(Vec3::new(1.0, 2.7, -10.0), 1.0, 4.0), pt(Vec3::new(-1.0, 2.7, -10.0), 1.15, 4.0)];
        assert_eq!(select_lights(&rivals, &probes, &view, 1, &[]), vec![1]);
        assert_eq!(select_lights(&rivals, &probes, &view, 1, &[light_key(&rivals[0])]), vec![0]);
        // ... but not against a clearly stronger one.
        let rivals2 = vec![rivals[0], pt(Vec3::new(-1.0, 2.7, -10.0), 2.0, 4.0)];
        assert_eq!(select_lights(&rivals2, &probes, &view, 1, &[light_key(&rivals2[0])]), vec![1]);
    }

    /// A face whose pyramid lies wholly outside the camera's view is skipped (no visible surface can
    /// look it up), one in view is kept; a caster smaller than a texel at its distance is skipped. Seen
    /// red first (2026-11-10) with view_touches answering true for every face: the face behind the
    /// camera was kept.
    #[test]
    fn faces_out_of_view_and_casters_under_a_texel_are_skipped() {
        let (_, view) = look();
        // A lamp 3 m ahead: its face looking back at the camera (+z) and its face looking ahead (-z)
        // both touch the view; a lamp 30 m behind the camera, facing away (+z), cannot.
        let ahead = Vec3::new(0.0, 2.7, -3.0);
        assert!(view_touches(&cube_face_view(ahead, 5, 7.0, 512, 3.25), &view));
        assert!(view_touches(&cube_face_view(ahead, 4, 7.0, 512, 3.25), &view));
        let behind = Vec3::new(0.0, 2.7, 30.0);
        assert!(!view_touches(&cube_face_view(behind, 4, 7.0, 512, 3.25), &view));
        // At 512 texels a face, a texel 4 m from the lamp is about 1.6 cm.
        let tm = 2.0 * face_tan_half(512, 3.25) / 512.0;
        assert!(big_enough(ahead + Vec3::new(0.0, -2.0, 3.0), 0.3, ahead, tm, false), "a 60 cm chair");
        assert!(!big_enough(ahead + Vec3::new(0.0, -2.0, 3.0), 0.005, ahead, tm, false), "a 1 cm screw");
        assert!(big_enough(Vec3::ZERO, f32::INFINITY, ahead, tm, false), "an unknown size always casts");
    }

    /// The table's header, lookup and views sit where the shader reads them.
    #[test]
    fn the_table_is_laid_out_as_the_shader_reads_it() {
        let v = AtlasView { view_proj: Mat4::IDENTITY, tile: (512, 1024, 256), origin: Vec3::new(1.0, 2.0, 3.0), ortho: false, texel_m: 0.01, range: 7.0 };
        let s = AtlasView { ortho: true, tile: (0, 0, 2048), origin: Vec3::Y, texel_m: 0.015, ..v };
        let rows = pack_table(&[(5, KIND_CUBE * KIND_STRIDE + 2)], &[s, v], Some(0), 4096, 1.75, 16);
        // Covers lights 0..=5: two lookup rows; views from row 3.
        assert_eq!(rows[0], [6.0, 3.0, 1.0, 16.0]);
        assert_eq!(rows[2][1], (KIND_CUBE * KIND_STRIDE + 2) as f32);
        assert_eq!(rows[1], [0.0; 4]);
        let b = 3 + VIEW_ROWS;
        assert_eq!(rows[b], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(rows[b + 4], [512.0 / 4096.0, 1024.0 / 4096.0, 256.0 / 4096.0, 1.75 / 4096.0]);
        assert_eq!(rows[b + 5], [1.0, 2.0, 3.0, 0.01]);
        assert_eq!(rows[3 + 5][3], -0.015, "an orthographic view's texel is negated");
        assert_eq!(rows[b + 6], [7.0, 0.0, 0.0, 0.0], "a perspective view's shadow reach");
        assert_eq!(rows[3 + 6], [0.0; 4], "an orthographic view has no reach to fade over");
        assert_eq!(rows.len(), 3 + 2 * VIEW_ROWS);
    }

    /// A light's shadow stops where its falloff is a twentieth of its value at a metre: for
    /// a 15 m panel, 5.2 m; a 4 m lamp, 3.1 m; never past the light's own reach.
    #[test]
    fn the_shadow_reach_is_where_the_light_has_faded() {
        let r15 = shadow_reach(&pt(Vec3::ZERO, 13.0, 15.0));
        let r4 = shadow_reach(&pt(Vec3::ZERO, 4.0, 4.0));
        assert!((5.0..5.5).contains(&r15), "15 m panel: {r15}");
        assert!((2.9..3.3).contains(&r4), "4 m lamp: {r4}");
        for r in [0.5f32, 1.0, 2.0, 7.0, 30.0] {
            let s = shadow_reach(&pt(Vec3::ZERO, 8.0, r));
            assert!(s <= r + 1e-4 && s > 0.0, "reach {s} for range {r}");
            assert!(falloff(s * 0.98, r) >= 0.05 * falloff(1.0, r) - 1e-6 || s >= r - 1e-3, "range {r}: the light still matters just inside {s}");
        }
    }

    /// The table's constants and the face pick are the shader's: read out of 00-bindings-vertex.wgsl.
    #[test]
    fn table_constants_match_the_shader() {
        let src = super::super::shader_loader::assembled_pbr_source();
        assert!(src.contains(&format!("const SHADOW_VIEW_ROWS: u32 = {VIEW_ROWS}u;")), "SHADOW_VIEW_ROWS");
        assert!(src.contains(&format!("const SHADOW_KIND_STRIDE: u32 = {KIND_STRIDE}u;")), "SHADOW_KIND_STRIDE");
        assert!(src.contains(&format!("const SHADOW_KIND_CUBE: u32 = {KIND_CUBE}u;")), "SHADOW_KIND_CUBE");
        let at = src.find("fn shadow_cube_face").expect("shadow_cube_face");
        let body = &src[at..at + 600];
        for want in ["a.x >= a.y && a.x >= a.z", "a.y >= a.z"] {
            assert!(body.contains(want), "shadow_cube_face must test {want:?} as cube_face_of does");
        }
    }

    /// The near sun view holds the eye's surroundings and the casters above them, and its grid does
    /// not move with a small step of the eye (texel snap).
    #[test]
    fn the_near_sun_view_holds_the_room_and_does_not_swim() {
        let sun = Vec3::new(0.3, 0.9, 0.2).normalize();
        let eye = Vec3::new(10.2, 1.6, -4.3);
        let vp = near_sun_view(eye, sun, 16.0, 2048);
        let planes = frustum_planes(&vp);
        assert!(sphere_in(&planes, eye - Vec3::Y * 1.6, 0.1), "the floor under the eye");
        assert!(sphere_in(&planes, eye + sun * 40.0, 0.1), "a caster 40 m toward the sun");
        assert!(!sphere_in(&planes, eye + Vec3::X * 40.0, 1.0), "40 m to the side is outside");
        // A point fixed in the world lands on the same texel offset after a 3 mm step.
        let p = Vec3::new(11.0, 0.0, -5.0);
        let vp2 = near_sun_view(eye + Vec3::new(0.003, 0.0, 0.002), sun, 16.0, 2048);
        let texel = 2.0 / 2048.0;
        let frac = |m: Mat4| {
            let c = m * p.extend(1.0);
            ((c.x / texel).rem_euclid(1.0), (c.y / texel).rem_euclid(1.0))
        };
        let (a, b) = (frac(vp), frac(vp2));
        assert!((a.0 - b.0).abs() < 1e-2 && (a.1 - b.1).abs() < 1e-2, "texel phase moved: {a:?} -> {b:?}");
    }

    /// The levels file is held to what the renderer can do, and a missing file still gives a level.
    #[test]
    fn levels_are_held_to_what_the_renderer_can_do() {
        let text = r#"{"levels":[
            {"id":"off"},
            {"id":"odd","lights":99,"atlas":3000,"face":700,"face_min":10,"sun_tile":9000,"sun_extent_m":500,"taps":9},
            {"id":"odd","lights":1,"atlas":1024}
        ]}"#;
        let l = parse_levels(Some(text));
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].atlas, 0);
        assert_eq!(l[0].lights, 0);
        let odd = &l[1];
        assert_eq!(odd.atlas, 4096);
        assert_eq!(odd.lights, 64);
        assert_eq!(odd.face, 1024);
        assert_eq!(odd.face_min, TILE_UNIT);
        assert_eq!(odd.sun_tile, 2048);
        assert_eq!(odd.sun_extent_m, 64.0);
        assert_eq!(odd.taps, 4);
        assert_eq!(parse_levels(None), vec![built_in_level()]);
        // The shipped file parses into Off, Simplified and Full, Full the default.
        let shipped = std::fs::read_to_string(concat!(env!("HUMANITY_REPO_ROOT"), "/data/gui/shadow_levels.json")).expect("the shipped levels");
        let s = parse_levels(Some(&shipped));
        let ids: Vec<&str> = s.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids, ["off", "simplified", "full"]);
        assert!(s.iter().any(|l| l.id == DEFAULT_LEVEL));
    }

    /// The fast path draws one run per mesh across every view of a pass, its (caster, view) instances
    /// contiguous and in the order the instance buffer is written, counting on from what the pass
    /// before appended. SEEN RED 2026-11-11 with a run pushed for every pair (no merge): "left:
    /// [FastRun { mesh: 3, first: 2, count: 1 }, FastRun { mesh: 3, first: 3, count: 1 }, ...] right:
    /// [FastRun { mesh: 3, first: 2, count: 2 }, FastRun { mesh: 10, first: 4, count: 3 }]".
    #[test]
    fn the_fast_path_draws_one_run_per_mesh() {
        let mesh_of = |c: u32| match c {
            5 | 7 => 10,
            _ => 3,
        };
        let mut order = vec![(99, 0), (98, 0)];
        let runs = fast_runs(&[(5, 0), (2, 1), (7, 0), (3, 0), (5, 2)], mesh_of, &mut order);
        assert_eq!(runs, vec![FastRun { mesh: 3, first: 2, count: 2 }, FastRun { mesh: 10, first: 4, count: 3 }]);
        assert_eq!(order, vec![(99, 0), (98, 0), (3, 0), (2, 1), (5, 0), (7, 0), (5, 2)]);
        // Every pair appears once, inside its own mesh's run.
        for r in &runs {
            for &(c, _) in &order[r.first as usize..(r.first + r.count) as usize] {
                assert_eq!(mesh_of(c), r.mesh);
            }
        }
        assert!(fast_runs(&[], mesh_of, &mut order).is_empty());
    }

    /// The fast path's shader parses and validates the way the device will, with its entry. SEEN RED
    /// 2026-11-11 with the matrix array misspelt in the entry: "the fast path shader parses:
    /// ParseError { message: \"no definition in scope for identifier: `view_projs`\" ...".
    #[test]
    fn the_fast_path_shader_validates() {
        let module = wgpu::naga::front::wgsl::parse_str(FAST_WGSL).expect("the fast path shader parses");
        let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
        v.validate(&module).expect("the fast path shader validates");
        let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
        assert!(entries.contains(&"vs_shadow_instanced"), "vs_shadow_instanced missing from {entries:?}");
    }

    /// Only casters vs_main never moves and that need no alpha test take the fast path: the wind
    /// classes and the water shell displace their vertices there, a cutout's shape is its texture.
    #[test]
    fn only_still_opaque_surfaces_take_the_fast_path() {
        assert!(takes_fast_path(0.0, false));
        assert!(takes_fast_path(3.0, false));
        assert!(!takes_fast_path(0.0, true), "a cutout needs fs_shadow");
        assert!(!takes_fast_path(19.0, false), "type 19 sways in vs_main");
        assert!(!takes_fast_path(20.0, false), "vegetation sways");
        assert!(!takes_fast_path(16.0, false), "the water shell moves");
        assert!(!takes_fast_path(12.0, false), "terrain is not a scene caster's class");
    }

    /// THE SHADOW-CASTER CULL (shadows rung 1b): a caster is kept when its shadow can fall inside the
    /// camera's view and left out when every part of it, out to the light's reach, lies outside one
    /// of the view's planes. Seen red first (2026-11-12) with the far sphere left out of the test
    /// (only the caster itself checked against the view): "a caster behind the camera whose shadow
    /// falls in front of it is kept" failed.
    #[test]
    fn a_caster_is_drawn_only_when_its_shadow_can_reach_the_view() {
        // A camera at the origin looking down -z, 90 degrees wide, 0.1 to 100 m.
        let cam = frustum_planes(&(Mat4::perspective_rh(90f32.to_radians(), 1.0, 0.1, 100.0) * Mat4::look_at_rh(Vec3::ZERO, Vec3::NEG_Z, Vec3::Y)));
        let reach = 6.0;
        // In view: kept.
        assert!(shadow_reaches_view(&cam, Vec3::new(0.0, 2.0, -4.0), Vec3::new(0.0, 0.5, -4.0), 0.4, reach, 0.02), "a caster in view is kept");
        // Behind the camera, the light further behind: its shadow falls toward the view.
        assert!(shadow_reaches_view(&cam, Vec3::new(0.0, 0.5, 4.0), Vec3::new(0.0, 0.5, 1.0), 0.4, reach, 0.02), "a caster behind the camera whose shadow falls in front of it is kept");
        // Behind the camera, the light in front of it: its shadow falls further behind, out of view.
        assert!(!shadow_reaches_view(&cam, Vec3::new(0.0, 0.5, -1.0), Vec3::new(0.0, 0.5, 2.0), 0.4, reach, 0.02), "a caster whose shadow falls behind the camera is left out");
        // Off to the side, the light beside it, its shadow going further out: out of view.
        assert!(!shadow_reaches_view(&cam, Vec3::new(8.0, 0.5, -3.0), Vec3::new(10.0, 0.5, -3.0), 0.4, reach, 0.02), "a caster whose shadow falls out to the side is left out");
        // The light inside the caster, and an unbounded caster: always kept.
        assert!(shadow_reaches_view(&cam, Vec3::new(0.0, 0.5, 5.0), Vec3::new(0.0, 0.5, 5.1), 0.4, reach, 0.02), "a caster around the light is kept");
        assert!(shadow_reaches_view(&cam, Vec3::new(0.0, 0.5, 5.0), Vec3::new(0.0, 0.5, 9.0), f32::INFINITY, reach, 0.02), "an unbounded caster is kept");
    }

}
