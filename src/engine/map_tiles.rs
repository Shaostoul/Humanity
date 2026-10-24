//! PLAN TILES (Maps increment 3, docs/design/maps-zoom-to-anything.md 5.7, "Plan tiles for
//! multi-kilometre ships"): the Maps page's straight-down plan of a level is drawn from tiles
//! rendered once from above, kept, and baked again only where an edit touched, the way a slippy
//! map keeps its tiles. Panning and zooming the plan of a kilometre-long ship then costs a few
//! textured quads, not a render of the whole ship every frame.
//!
//! THE TILES. A tile is a square of a level's floor in SHIP metres, keyed by (level ordinal,
//! zoom, x, y): zoom 0 is 512 m on a side at 1 m a pixel, each zoom halves it (zoom 6: 8 m at
//! 1.6 cm a pixel), and tile (x, y) covers x*s..(x+1)*s across the ship's x and y*s..(y+1)*s
//! across its z. The ship file is the ship (its id is the cache's: another ship, a fresh cache).
//! The page asks for the tiles its view covers at the zoom that keeps a tile pixel no bigger than
//! a screen pixel (`zoom_for`, `keys_covering`, gui/pages/maps/ship_view.rs) and draws a coarser
//! tile's part where a finer one is not baked yet.
//!
//! THE BAKE. At most ONE tile a frame, while the plan is shown and the home is drawn at the level
//! (the cutaway, engine/map_cut.rs: everything over the level taken away by the editor's rule):
//! the world renderer from straight above the tile's centre, through a 2 degree field of view
//! from 28.6 tile-widths up (a near-orthographic view: a 3 m wall's top sits 1.3% off at the
//! finest zoom, 0.02% at the coarsest), into a 512 px texture registered with the window's egui
//! renderer. `ViewPasses::Plan` draws the scene objects only, on black, with no sky: the shell
//! (no roof, no hull), the machines and their screens, the plants. No people, doors or pipes:
//! what a plan shows is what is built, not who is walking past. Lit by the real Sun and the rooms'
//! own lights, as the game is: a tile baked at night is lit by its lamps. Timed as its own passes
//! (`gpu.map_tile_*`, `cpu.map_tiles`).
//!
//! KEPT. 1 MB a tile (512 x 512 x 4), at most 64 MB (64 tiles), the least recently used let go
//! first; all of them when the page closes.
//!
//! BAKED AGAIN ONLY WHERE AN EDIT TOUCHED. Everything a level's plan is made of is a list of
//! ITEMS, each a footprint and a hash of what it is (`plan_items`): every wall, slab, piece, light
//! and room volume of every zone as the level cuts it, each zone's shell, each corridor, each
//! neighbour's home, each machine. When the home's shell or machines are rebuilt
//! (`HomeDraw::edits`), the items are made again and compared with the last ones: an item that
//! went or came (moved, changed, added, removed) marks its footprint, and only the tiles of that
//! level under a marked footprint are dropped (`dirty_rects`, `touched`), to be baked again when
//! they are in view. A rebuild that changed nothing drops nothing. Plants growing are not edits:
//! a tile keeps the garden it was baked with until something there is built or moved.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use glam::{DQuat, DVec3, Vec3};

use crate::engine::editor::Cuts;
use crate::engine::state::{EngineState, SceneDrawLists};
use crate::renderer::RenderObject;
use crate::ship::home_structure::storey_index;
use crate::ship::levels::ShipLevel;
use crate::ship::ship_structure::{CorridorAxis, ShipStructure};

/// A tile's side in pixels.
pub const TILE_PX: u32 = 512;
/// A zoom-0 tile's side, metres (1 m a pixel).
pub const TILE_BASE_M: f32 = 512.0;
/// The finest zoom: 8 m tiles, 1.6 cm a pixel.
pub const MAX_ZOOM: u8 = 6;
/// One tile's texture, bytes.
pub const TILE_BYTES: usize = (TILE_PX * TILE_PX * 4) as usize;
/// The most the tiles may hold, bytes (64 tiles).
pub const CACHE_BYTES: usize = 64 * 1024 * 1024;
/// The bake's field of view, degrees (see the module doc).
const TILE_FOV_DEG: f64 = 2.0;

/// One tile: its level (ordinal), zoom, and place in the zoom's grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TileKey {
    pub level: i32,
    pub z: u8,
    pub x: i32,
    pub y: i32,
}

impl TileKey {
    /// A tile's side at zoom `z`, metres.
    pub fn size_m(z: u8) -> f32 {
        TILE_BASE_M / (1u32 << z.min(MAX_ZOOM)) as f32
    }

    /// Its footprint, ship metres: [min x, min z, max x, max z].
    pub fn rect(&self) -> [f32; 4] {
        let s = Self::size_m(self.z);
        [self.x as f32 * s, self.y as f32 * s, (self.x + 1) as f32 * s, (self.y + 1) as f32 * s]
    }

    /// The tile one zoom coarser that holds this one.
    pub fn parent(&self) -> Option<TileKey> {
        (self.z > 0).then(|| TileKey { level: self.level, z: self.z - 1, x: self.x.div_euclid(2), y: self.y.div_euclid(2) })
    }

    /// Where this tile is in `outer`'s image (a tile holding it), as [u0, v0, u1, v1]: u across
    /// the ship's x, v across its z (the bake's image has the ship's -z at its top).
    pub fn uv_in(&self, outer: &TileKey) -> [f32; 4] {
        let (a, b) = (self.rect(), outer.rect());
        let s = TileKey::size_m(outer.z);
        [(a[0] - b[0]) / s, (a[1] - b[1]) / s, (a[2] - b[0]) / s, (a[3] - b[1]) / s]
    }
}

/// The zoom whose tile pixel is no bigger than a screen pixel of `m_per_px` metres (the finest
/// needed), within 0 and `MAX_ZOOM`.
pub fn zoom_for(m_per_px: f64) -> u8 {
    if !(m_per_px.is_finite() && m_per_px > 0.0) {
        return 0;
    }
    let base = (TILE_BASE_M / TILE_PX as f32) as f64;
    (base / m_per_px).log2().ceil().clamp(0.0, MAX_ZOOM as f64) as u8
}

/// The tiles of `level` at zoom `z` that cover `rect` ([min x, min z, max x, max z], ship metres).
pub fn keys_covering(level: i32, z: u8, rect: [f32; 4]) -> Vec<TileKey> {
    let s = TileKey::size_m(z);
    let (x0, y0) = ((rect[0] / s).floor() as i32, (rect[1] / s).floor() as i32);
    let (x1, y1) = (((rect[2] / s).ceil() as i32 - 1).max(x0), ((rect[3] / s).ceil() as i32 - 1).max(y0));
    let mut out = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            out.push(TileKey { level, z, x, y });
        }
    }
    out
}

/// True when two footprints overlap (touching edges do not).
pub fn rects_touch(a: [f32; 4], b: [f32; 4]) -> bool {
    a[0] < b[2] && b[0] < a[2] && a[1] < b[3] && b[1] < a[3]
}

/// One thing a level's plan is made of: its footprint (ship metres) and a hash of what it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PlanItem {
    pub rect: [f32; 4],
    pub hash: u64,
}

fn hash_of(what: &dyn std::fmt::Debug) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    format!("{what:?}").hash(&mut h);
    h.finish()
}

fn around(x: f32, z: f32, r: f32) -> [f32; 4] {
    [x - r, z - r, x + r, z + r]
}

/// Everything level `level` of `ship` is made of (see the module doc), with `machines` (each
/// one's foot, ship metres, its yaw and its mesh) those `cuts` leaves standing.
pub(crate) fn plan_items(ship: &ShipStructure, level: &ShipLevel, machines: &[(Vec3, f32, usize)], cuts: &Cuts) -> Vec<PlanItem> {
    let mut out = Vec::new();
    let mut push = |rect: [f32; 4], what: &dyn std::fmt::Debug| out.push(PlanItem { rect, hash: hash_of(what) });
    for (zi, z) in ship.zones.iter().enumerate().filter(|(zi, _)| !ship.is_away_home(*zi)) {
        let o = z.origin_vec();
        let storeys = z.body.storeys();
        let k = storey_index(&storeys, level.floor - o.y);
        let cut = z.body.cut_away(k);
        let b = cut.as_ref().unwrap_or(&z.body);
        let at = |x: f32, zz: f32| (o.x + x, o.z + zz);
        let (w, d) = (b.width, b.depth);
        let cuts_here = ship.shell_cuts_for_zone(zi);
        push([o.x, o.z, o.x + w, o.z + d], &(o, w, d, b.height, b.shell_material, b.roof_material, b.shell_thickness, &cuts_here));
        for wall in &b.walls {
            let m = wall.thickness.unwrap_or(0.3) * 0.5 + 0.25;
            let (a, c) = (at(wall.a.0, wall.a.1), at(wall.b.0, wall.b.1));
            push([a.0.min(c.0) - m, a.1.min(c.1) - m, a.0.max(c.0) + m, a.1.max(c.1) + m], &(o, wall));
        }
        for s in &b.floors {
            let (x0, z0, x1, z1) = s.rect();
            push([o.x + x0, o.z + z0, o.x + x1, o.z + z1], &(o, s));
        }
        for p in &b.structures {
            // Its footprint at any turn: half its diagonal, from its type's size (a stair, a deck,
            // an 8 m train platform), and a margin.
            let reach = crate::ship::structure::structure_type(&p.type_id).map_or(3.0, |t| 0.5 * t.size.0.hypot(t.size.2)) + 0.25;
            let (x, zz) = at(p.pos.0, p.pos.2);
            push(around(x, zz, reach), &(o, p));
        }
        for v in &b.zones {
            push([o.x + v.origin.0, o.z + v.origin.2, o.x + v.origin.0 + v.size.0, o.z + v.origin.2 + v.size.2], &(o, v));
        }
        for l in &b.lights {
            let (x, zz) = at(l.pos.0, l.pos.2);
            push(around(x, zz, 4.0), &(o, l));
        }
    }
    for c in &ship.corridors {
        let Ok(g) = ship.corridor_geometry(c) else { continue };
        let h = g.width * 0.5;
        let rect = match g.axis {
            CorridorAxis::X => [g.start, g.lat - h, g.end, g.lat + h],
            CorridorAxis::Z => [g.lat - h, g.start, g.lat + h, g.end],
        };
        push(rect, &g);
    }
    let view = crate::ship::neighbours::neighbour_view(ship);
    for n in &view.neighbours {
        let (x, y, zz) = n.plot.origin;
        let k = storey_index(&n.design.body.storeys(), level.floor - y);
        push([x, zz, x + n.plot.size.0, zz + n.plot.size.2], &(&n.plot, &n.design.kind, k));
        if let Some(g) = &n.tube {
            let h = g.width * 0.5;
            let rect = match g.axis {
                CorridorAxis::X => [g.start, g.lat - h, g.end, g.lat + h],
                CorridorAxis::Z => [g.lat - h, g.start, g.lat + h, g.end],
            };
            push(rect, g);
        }
    }
    for (foot, yaw, mesh) in machines.iter().filter(|(foot, ..)| !cuts.hides(*foot)) {
        push(around(foot.x, foot.z, 2.5), &(foot.to_array().map(f32::to_bits), yaw.to_bits(), mesh));
    }
    out
}

/// The footprints an edit touched: every item of `old` not in `new` and of `new` not in `old`
/// (as many times as it differs: two identical pieces and one taken away mark one).
pub(crate) fn dirty_rects(old: &[PlanItem], new: &[PlanItem]) -> Vec<[f32; 4]> {
    let key = |i: &PlanItem| (i.hash, i.rect.map(f32::to_bits));
    let mut count: HashMap<(u64, [u32; 4]), i32> = HashMap::new();
    for i in old {
        *count.entry(key(i)).or_default() += 1;
    }
    for i in new {
        *count.entry(key(i)).or_default() -= 1;
    }
    let mut out: Vec<[f32; 4]> = count.into_iter().filter(|(_, c)| *c != 0).map(|((_, r), _)| r.map(f32::from_bits)).collect();
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Of `keys`, the tiles of level `level` under any of `dirty`.
pub(crate) fn touched(keys: impl IntoIterator<Item = TileKey>, level: i32, dirty: &[[f32; 4]]) -> Vec<TileKey> {
    let mut out: Vec<TileKey> = keys.into_iter().filter(|k| k.level == level && dirty.iter().any(|d| rects_touch(k.rect(), *d))).collect();
    out.sort();
    out
}

/// The tiles to let go so the rest fit `cap_bytes`: the least recently used first (`used`, a
/// frame count), never one used this frame (`now`).
pub(crate) fn to_evict(used: &[(TileKey, u64)], now: u64, cap_bytes: usize) -> Vec<TileKey> {
    let over = (used.len() * TILE_BYTES).saturating_sub(cap_bytes).div_ceil(TILE_BYTES);
    let mut old: Vec<&(TileKey, u64)> = used.iter().filter(|(_, u)| *u < now).collect();
    old.sort_by_key(|(k, u)| (*u, *k));
    old.into_iter().take(over).map(|(k, _)| *k).collect()
}

/// One baked tile on the GPU.
struct GpuTile {
    _texture: wgpu::Texture,
    id: egui::TextureId,
    used: u64,
}

/// The tiles and what they were baked from (`MapViewGpu::tiles`).
#[derive(Default)]
pub(crate) struct MapTilesGpu {
    ship: String,
    tiles: HashMap<TileKey, GpuTile>,
    /// Each level's items when its tiles were last checked against an edit, and
    /// `HomeDraw::edits` as it was then.
    items: HashMap<i32, (u64, Vec<PlanItem>)>,
    frame: u64,
    /// Tiles baked this opening of the page.
    baked: u32,
}

/// The renderer camera for a tile: from straight above its centre, the ship's -z up the image,
/// in the render frame (the map view's own conversion, `map_view::render_camera`).
fn tile_camera(key: &TileKey, level: &ShipLevel, me: &crate::gui::MapSelf) -> crate::renderer::camera::Camera {
    let r = key.rect();
    let s = TileKey::size_m(key.z) as f64;
    let half = (TILE_FOV_DEG * 0.5).to_radians();
    let up_m = s * 0.5 / half.tan();
    let centre = DVec3::new(((r[0] + r[2]) * 0.5) as f64, level.floor as f64, ((r[1] + r[3]) * 0.5) as f64);
    let rot: DQuat = me.ship_rot;
    let pose = crate::gui::pages::maps::camera::MapPose {
        look_at: me.ship_to_world(centre),
        eye: me.ship_to_world(centre + DVec3::Y * up_m),
        forward: rot * DVec3::NEG_Y,
        right: rot * DVec3::X,
        up: rot * DVec3::NEG_Z,
        distance: up_m,
        fov_y: TILE_FOV_DEG.to_radians(),
    };
    let mut cam = crate::engine::map_view::render_camera(&pose, me, 1.0);
    // Everything drawn is within a few tens of metres of the floor: a near plane just over it
    // keeps the depth buffer's precision where the plan is.
    cam.near = (up_m - 120.0).max(0.5) as f32;
    cam
}

/// What a tile draws (see the module doc): the home's shell without its roof and hull, the
/// machines and their screens, the plants; as the frame draws them, with the cutaway applied.
fn tile_lists(state: &EngineState) -> (Vec<RenderObject>, Vec<RenderObject>) {
    let at_origin = |(mesh, material): (usize, usize)| RenderObject { fade: 0.0, position: Vec3::ZERO, rotation: glam::Quat::IDENTITY, scale: Vec3::ONE, mesh, material };
    let mut opaque: Vec<RenderObject> = state.homestead_floors.iter().copied().map(at_origin).collect();
    opaque.extend([state.homestead_walls, state.homestead_trim, state.homestead_mirrors].into_iter().flatten().map(at_origin));
    let mut transparent = Vec::new();
    for &(mesh, material, glass) in &state.homestead_material_walls {
        if glass { transparent.push(at_origin((mesh, material))) } else { opaque.push(at_origin((mesh, material))) }
    }
    transparent.extend(state.homestead_windows.map(at_origin));
    crate::engine::home_meshes::push_home_machines(state, &mut opaque);
    crate::engine::home_meshes::push_home_plants(state, &mut opaque);
    (opaque, transparent)
}

/// Once a frame while the Maps page is open (from `map_view::frame`): with the plan shown, look
/// for edits, bake one wanted tile, let go of the least used over the cap, and tell the page
/// which tiles it has. `plan` is whether the plan is shown (`WorldLayerWant::Plan`).
pub(crate) fn frame(state: &mut EngineState, plan: bool) {
    state.map_view.tiles.frame += 1;
    let Some(level) = crate::engine::map_cut::map_level(&state.gui_state).filter(|_| plan) else {
        publish(state);
        return;
    };
    // Only once the home is drawn at this level (the cutaway follows the pick a frame later).
    if state.home_draw.view_stale(crate::engine::map_cut::map_cuts(&state.gui_state)) {
        publish(state);
        return;
    }
    let ship_id = state.gui_state.ship_structure.as_ref().map(|s| s.id.clone()).unwrap_or_default();
    if state.map_view.tiles.ship != ship_id {
        release(state);
        state.map_view.tiles.ship = ship_id;
    }
    check_edits(state, &level);
    bake_one(state, &level);
    let tiles = &mut state.map_view.tiles;
    let used: Vec<(TileKey, u64)> = tiles.tiles.iter().map(|(k, t)| (*k, t.used)).collect();
    for k in to_evict(&used, tiles.frame, CACHE_BYTES) {
        if let Some(t) = tiles.tiles.remove(&k) {
            state.egui_renderer.free_texture(&t.id);
        }
    }
    publish(state);
}

/// When the home's shell or machines were rebuilt since last seen: make this level's items
/// again, and drop the tiles an edit touched (`dirty_rects`, `touched`). The first look at a
/// level only records its items.
fn check_edits(state: &mut EngineState, level: &ShipLevel) {
    let edits = state.home_draw.edits;
    if state.map_view.tiles.items.get(&level.ordinal).is_some_and(|(seen, _)| *seen == edits) {
        return;
    }
    let Some(ship) = state.gui_state.ship_structure.as_ref() else { return };
    let machines: Vec<(Vec3, f32, usize)> = state.machine_objects.iter().map(|&(mesh, _, pos, yaw)| (pos, yaw, mesh)).collect();
    let items = plan_items(ship, level, &machines, &state.home_draw.cut);
    let tiles = &mut state.map_view.tiles;
    let old = tiles.items.insert(level.ordinal, (edits, items.clone()));
    // Another level's items are made again when it is shown (each level keeps the edit count it
    // was last checked at): its tiles are dropped then, and kept as they were until it is.
    let Some((_, old)) = old else { return };
    let dirty = dirty_rects(&old, &items);
    if dirty.is_empty() {
        return;
    }
    let dropped = touched(tiles.tiles.keys().copied(), level.ordinal, &dirty);
    for k in &dropped {
        if let Some(t) = tiles.tiles.remove(k) {
            state.egui_renderer.free_texture(&t.id);
        }
    }
    log::info!("Maps: an edit touched {} footprints on level {}; {} plan tiles dropped", dirty.len(), level.ordinal, dropped.len());
    let report = &mut state.gui_state.maps.tiles;
    report.dropped = dropped;
    report.dirty = dirty;
}

/// Bake the first tile the page wants that is not kept yet (at most one a frame).
fn bake_one(state: &mut EngineState, level: &ShipLevel) {
    let frame = state.map_view.tiles.frame;
    let want = state.gui_state.maps.tiles.want.clone();
    for k in want.iter().filter(|k| k.level == level.ordinal) {
        if let Some(t) = state.map_view.tiles.tiles.get_mut(k) {
            t.used = frame;
        }
    }
    let Some(key) = want.iter().find(|k| k.level == level.ordinal && !state.map_view.tiles.tiles.contains_key(k)).copied() else {
        return;
    };
    let Some(me) = state.gui_state.maps.me.clone() else { return };
    let started = std::time::Instant::now();
    let _cost = crate::renderer::frame_costs::stage("cpu.map_tiles");
    let texture = state.renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Maps Plan Tile"),
        size: wgpu::Extent3d { width: TILE_PX, height: TILE_PX, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: state.renderer.surface_format(), // display-format: the present pass writes the tile and egui samples it, as the map view's target
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let cam = tile_camera(&key, level, &me);
    let (opaque, transparent) = tile_lists(state);
    let none: [RenderObject; 0] = [];
    let lines: [crate::renderer::line::LineVertex; 0] = [];
    let lists = SceneDrawLists { celestial: &none, celestial_transparent: &none, orbit_lines: &lines, opaque: &opaque, transparent: &transparent, overlay: &none, ring_lines: &lines };
    crate::engine::ipc::render_view_onto_kept(state, &cam, &view, (TILE_PX, TILE_PX), &lists, crate::engine::ipc::ViewPasses::Plan, true);
    state.renderer.mark_view_frame();
    let id = state.egui_renderer.register_native_texture(&state.renderer.device, &view, wgpu::FilterMode::Linear);
    let tiles = &mut state.map_view.tiles;
    tiles.tiles.insert(key, GpuTile { _texture: texture, id, used: frame });
    tiles.baked += 1;
    state.gui_state.maps.tiles.last_bake_ms = started.elapsed().as_secs_f32() * 1000.0;
}

/// Tell the page which tiles are kept (and how many were baked, the bytes they hold).
fn publish(state: &mut EngineState) {
    let tiles = &state.map_view.tiles;
    let have = tiles.tiles.iter().map(|(k, t)| (*k, t.id)).collect();
    let (baked, bytes) = (tiles.baked, tiles.tiles.len() * TILE_BYTES);
    let out = &mut state.gui_state.maps.tiles;
    out.have = have;
    out.baked = baked;
    out.bytes = bytes;
}

/// Let every tile go (the page closed, another ship).
pub(crate) fn release(state: &mut EngineState) {
    for (_, t) in state.map_view.tiles.tiles.drain() {
        state.egui_renderer.free_texture(&t.id);
    }
    state.map_view.tiles.items.clear();
    state.map_view.tiles.baked = 0;
    state.gui_state.maps.tiles = Default::default();
}

#[cfg(test)]
#[path = "map_tiles_tests.rs"]
mod tests;
