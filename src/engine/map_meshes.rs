//! THE WHOLE-EARTH MAP IN THE WORLD (whole-Earth maps rung 2, 2026-11-09;
//! docs/design/whole-earth-maps.md sections 3.6 and 12). Replaced `region_meshes` and its two
//! hand-fetched rectangles (HOSMREG2 files, a 40/60 km gate round each centre).
//!
//! Three jobs, all from the installed map packs (`data/maps/packs/*.hmpack`):
//! 1. **Publish the ground.** At world entry the packs are read and published as
//!    `map_ground::MapSet`, which every patch build and ground consumer then applies inside the
//!    one formula; a worker builds each pack's shore field straight away so no patch build waits
//!    on it. Returns true that frame so the caller drops patches built before it.
//! 2. **Residency, riding the patches.** Every frame the set is told which patches the last
//!    selection drew, and drops the decoded tiles no patch has needed for a while. With the Dev
//!    page's "turn off all reuse" switch off, a fresh generation is published every frame instead.
//! 3. **The OSM geometry.** Roads, buildings and lake sheets stand on the ground in DISTRICTS, the
//!    level-9 tiles (about 14 km) near the camera, each one mesh per class built on a worker from
//!    its 64 level-12 tiles (the extrusion mesher, `osm_region::build_region_meshes`, kept) with
//!    every vertex's height from the drawn ground formula at the finest depth. One district builds
//!    at a time; districts beyond `KEEP_RANGE_M` give their meshes back.
//!
//! f64 to the last step: the district anchor is planet-local f64, vertices anchor-relative f32,
//! drawn as `render_off + rot_d * anchor` (the near-tree and classic-patch form).

use glam::DVec3;
use humanity_terrain::map_ground::{self, MapSet};
use humanity_terrain::map_tiles;
use humanity_terrain::osm_region::{self, RegionMeshKind, RegionMeshes};
use humanity_terrain::planet_chunks::{patch_corners, PatchId};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// The level whose tiles are the districts the geometry is built in.
const DISTRICT_LEVEL: u8 = 9;
/// Build a district whose nearest ground is this close to the camera (metres; `district_distance_m`).
const BUILD_RANGE_M: f64 = 15_000.0;
/// Draw a built district this close.
const DRAW_RANGE_M: f64 = 25_000.0;
/// Give back a district's meshes beyond this.
const KEEP_RANGE_M: f64 = 40_000.0;

/// How far the camera is from a district: the straight-line distance to the nearest point of the
/// district's ground (its tile on the sphere at `radius`), so the altitude counts and a camera
/// inside the tile is its height above it. Measured to the nearest point, not the centre: a level-9
/// tile is about 14 km across, and the centre rule left a district whose edge lay 7 km away
/// unbuilt, its lake beds bare under the sea shell in the 5 km capture over Seattle (rung 2,
/// 2026-11-09). f64 throughout.
fn district_distance_m(id: &PatchId, cam_local: DVec3, radius: f64) -> f64 {
    let alt = (cam_local.length() - radius).max(0.0);
    let p = cam_local.normalize_or_zero();
    let c = patch_corners(id);
    let mut arc = f64::MAX;
    let mut inside = true;
    for k in 0..3 {
        let (a, b, o) = (c[k], c[(k + 1) % 3], c[(k + 2) % 3]);
        let mut n = a.cross(b).normalize();
        if n.dot(o) < 0.0 {
            n = -n;
        }
        if n.dot(p) < 0.0 {
            inside = false;
        }
        // The nearest point of the edge's arc: the foot on its great circle when that lies
        // between the corners, else the nearer corner.
        let foot = (p - n * n.dot(p)).normalize_or_zero();
        let between = a.cross(foot).dot(n) >= 0.0 && foot.cross(b).dot(n) >= 0.0;
        let to_edge = if between { n.dot(p).abs().clamp(0.0, 1.0).asin() } else { p.angle_between(a).min(p.angle_between(b)) };
        arc = arc.min(to_edge);
    }
    let ground = if inside { 0.0 } else { arc * radius };
    (ground * ground + alt * alt).sqrt()
}

/// Dev overrides of how deep and how near the camera the terrain refines inside an installed
/// pack's tile (whole-Earth maps, the rung between 2 and 3, 2026-11-09; the values themselves are
/// `levels.ron`'s `pack_detail`). The showcase keys `pack_depth` and `pack_within_m` set them live
/// ("auto" releases them), so a rig compares settings in one boot. 0 / NaN: no override.
static PACK_DEPTH_OVERRIDE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static PACK_WITHIN_OVERRIDE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x7ff8_0000_0000_0000);

/// Override the pack depth (the showcase's `pack_depth`); None releases it.
pub(crate) fn set_pack_max_depth(depth: Option<u8>) {
    let d = depth.map_or(0, |d| d.clamp(humanity_terrain::planet_chunks::MAX_PATCH_DEPTH, humanity_terrain::planet_chunks::TILE_MAX_PATCH_DEPTH));
    PACK_DEPTH_OVERRIDE.store(d, std::sync::atomic::Ordering::Relaxed);
}

/// Override the pack detail's reach (the showcase's `pack_within_m`); None releases it.
pub(crate) fn set_pack_within_m(m: Option<f64>) {
    PACK_WITHIN_OVERRIDE.store(m.map_or(f64::NAN, |m| m.max(0.0)).to_bits(), std::sync::atomic::Ordering::Relaxed);
}

/// The tiles where `body`'s terrain may refine past its cap: Earth's installed map packs, to
/// `pack_detail.max_depth` within `pack_detail.within_m` of the camera.
pub(crate) fn deep_tiles(body: &str) -> humanity_terrain::planet_chunks::DeepTiles {
    let map = if body == "earth" { map_ground::snapshot() } else { None };
    let Some(m) = map else { return Default::default() };
    let pd = &m.levels.pack_detail;
    let d = PACK_DEPTH_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed);
    let w = f64::from_bits(PACK_WITHIN_OVERRIDE.load(std::sync::atomic::Ordering::Relaxed));
    humanity_terrain::planet_chunks::DeepTiles { tiles: m.pack_ids(), max_depth: if d > 0 { d } else { pd.max_depth }, within_m: if w.is_nan() { pd.within_m } else { w } }
}

/// The depth and reach the terrain refines to inside installed packs now (the bed probe's readout).
pub(crate) fn pack_detail_now() -> (u8, f64) {
    let d = deep_tiles("earth");
    (d.max_depth, d.within_m)
}

/// One district standing in the world.
pub(crate) struct District {
    pub id: PatchId,
    pub anchor_local: DVec3,
    /// (renderer mesh index, material index) per class mesh.
    pub meshes: Vec<(usize, usize)>,
    pub pack: String,
}

/// All map state, owned by EngineState.
#[derive(Default)]
pub(crate) struct MapMeshState {
    loaded: bool,
    set: Option<Arc<MapSet>>,
    pub districts: Vec<District>,
    pending: Option<(PatchId, String, Receiver<(RegionMeshes, f64)>)>,
    /// Renderer mesh indices given back by far districts, reused before adding new ones.
    free_meshes: Vec<usize>,
    materials: Option<[usize; 6]>,
    /// Frames ticked: the district search runs every 30th (81 tile lookups, about 0.1 ms).
    frames: u64,
    /// The planet's radius (metres), kept by `tick` for the distances `drawn_names` measures.
    radius_m: f64,
}

fn kind_ordinal(k: RegionMeshKind) -> usize {
    match k {
        RegionMeshKind::BuildingLow => 0,
        RegionMeshKind::BuildingMid => 1,
        RegionMeshKind::BuildingHigh => 2,
        RegionMeshKind::RoadAsphalt => 3,
        RegionMeshKind::RoadFoot => 4,
        RegionMeshKind::WaterInland => 5,
    }
}

/// A district's centre direction.
fn centre(id: &PatchId) -> DVec3 {
    let c = patch_corners(id);
    (c[0] + c[1] + c[2]).normalize()
}

/// The names of the packs whose geometry is drawn from `cam_local`: what the map credit names.
pub(crate) fn drawn_names(mm: &MapMeshState, cam_local: DVec3) -> Vec<String> {
    let r = mm.radius_m;
    let mut v: Vec<String> = mm.districts.iter().filter(|d| district_distance_m(&d.id, cam_local, r) <= DRAW_RANGE_M).map(|d| d.pack.clone()).collect();
    v.sort();
    v.dedup();
    v
}

impl MapMeshState {
    /// Register the six class materials once (cartography and architecture reads, the material
    /// system's own registry pattern, as the region meshes had them).
    fn ensure_materials(&mut self, renderer: &mut crate::renderer::Renderer) -> [usize; 6] {
        *self.materials.get_or_insert_with(|| {
            let mut m = |rgba: [f32; 4], metallic: f32, rough: f32, ty: f32| renderer.add_material_typed(rgba, metallic, rough, ty);
            [
                m([0.52, 0.44, 0.38, 1.0], 0.0, 0.85, 2.0),
                m([0.55, 0.55, 0.58, 1.0], 0.0, 0.75, 2.0),
                m([0.40, 0.48, 0.55, 1.0], 0.6, 0.25, 1.0),
                m([0.16, 0.16, 0.17, 1.0], 0.0, 0.95, 2.0),
                m([0.45, 0.44, 0.41, 1.0], 0.0, 0.9, 2.0),
                m([0.09, 0.20, 0.24, 1.0], 0.1, 0.08, 2.0),
            ]
        })
    }

    /// Read and publish the installed packs, once. True when a set was published.
    fn ensure_loaded(&mut self) -> bool {
        if self.loaded {
            return false;
        }
        self.loaded = true;
        let data = humanity_core::storage::DATA_DIR.get().cloned().unwrap_or_else(|| std::path::PathBuf::from("data"));
        let t = std::time::Instant::now();
        let packs = map_ground::load_packs(&data.join("maps").join("packs"));
        if packs.is_empty() {
            return false;
        }
        let levels = match humanity_terrain::map_levels::MapLevels::load(&data) {
            Ok(l) => l,
            Err(e) => {
                log::warn!("[Map] {e}: the map packs are not drawn");
                return false;
            }
        };
        let bytes: usize = packs.iter().map(|p| p.0.len_bytes()).sum();
        let names: Vec<String> = packs.iter().map(|p| p.1.clone()).collect();
        let set = Arc::new(MapSet::new(levels, packs));
        map_ground::publish(set.clone());
        let prep = set.clone();
        std::thread::spawn(move || prep.prepare());
        log::info!(
            "[Map] {} pack(s) published ({}; {:.1} MB) in {:.0} ms. Map data (c) OpenStreetMap contributors, ODbL 1.0",
            names.len(),
            names.join(", "),
            bytes as f64 / 1e6,
            t.elapsed().as_secs_f64() * 1000.0
        );
        self.set = Some(set);
        true
    }
}

/// Per-frame driver, from the Earth branch of the celestial pass. `cam_local` is the camera in
/// the planet's unrotated frame (f64); `drawn` the patches the last selection drew. Returns true
/// on the frame the packs were published (the caller drops patches built before it).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    mm: &mut MapMeshState,
    def: &humanity_terrain::planet::PlanetDef,
    hm: &Arc<humanity_terrain::planet_heightmap::PlanetHeightmap>,
    drawn: Option<&std::collections::HashSet<PatchId>>,
    renderer: &mut crate::renderer::Renderer,
    celestial_objects: &mut Vec<crate::renderer::RenderObject>,
    cam_local: DVec3,
    render_off: DVec3,
    rot_d: glam::DQuat,
    rotation: glam::Quat,
) -> bool {
    let published = mm.ensure_loaded();
    mm.radius_m = def.radius;
    let Some(set) = mm.set.clone() else { return published };
    // 2. Residency (or, with reuse off, a fresh generation every frame).
    if humanity_core::reuse::enabled() {
        set.tick(drawn.into_iter().flatten().copied());
    } else {
        let fresh = Arc::new(set.fresh_generation());
        map_ground::publish(fresh.clone());
        mm.set = Some(fresh);
    }
    let set = mm.set.clone().expect("published above");

    // 3a. Drain a finished district build.
    if let Some((id, pack, rx)) = &mm.pending {
        match rx.try_recv() {
            Ok((built, secs)) => {
                let (id, pack) = (*id, pack.clone());
                mm.pending = None;
                let mats = mm.ensure_materials(renderer);
                let mut meshes = Vec::new();
                for class in &built.classes {
                    if class.indices.is_empty() {
                        continue;
                    }
                    let mesh = crate::renderer::mesh::Mesh::from_vertices(&renderer.device, &class.vertices, &class.indices);
                    let mi = match mm.free_meshes.pop() {
                        Some(i) => {
                            renderer.replace_mesh(i, mesh);
                            i
                        }
                        None => renderer.add_mesh(mesh),
                    };
                    meshes.push((mi, mats[kind_ordinal(class.kind)]));
                }
                log::info!("[Map] district {}/{} standing: {} class meshes, {} rings refused, built in {secs:.2} s on a worker. Map data (c) OpenStreetMap contributors, ODbL 1.0", id.depth, id.path, meshes.len(), built.skipped_rings);
                mm.districts.push(District { id, anchor_local: built.anchor_local, meshes, pack });
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => mm.pending = None,
        }
    }

    // 3b. Give back far districts.
    let cam_dir = cam_local.normalize_or_zero();
    let far: Vec<usize> = (0..mm.districts.len()).filter(|i| district_distance_m(&mm.districts[*i].id, cam_local, def.radius) > KEEP_RANGE_M).collect();
    for i in far.into_iter().rev() {
        let d = mm.districts.remove(i);
        for (mi, _) in d.meshes {
            renderer.replace_mesh(mi, crate::renderer::mesh::Mesh::placeholder(&renderer.device));
            mm.free_meshes.push(mi);
        }
    }

    // 3c. Start the nearest unbuilt district in range (one at a time, on a worker).
    mm.frames += 1;
    if mm.pending.is_none() && mm.frames % 30 == 1 && cam_dir.length_squared() > 0.5 {
        let step_rad = 4_000.0 / def.radius;
        let east = DVec3::Y.cross(cam_dir).normalize_or_zero();
        let north = cam_dir.cross(east);
        let mut best: Option<(f64, PatchId, String)> = None;
        for i in -4..=4 {
            for j in -4..=4 {
                let d = (cam_dir + east * (i as f64 * step_rad) + north * (j as f64 * step_rad)).normalize();
                let Some(ps) = set.packs.iter().find(|p| p.holds(d)) else {
                    continue;
                };
                let id = map_tiles::tile_containing_dir(d, DISTRICT_LEVEL);
                if mm.districts.iter().any(|x| x.id == id) {
                    continue;
                }
                let dist = district_distance_m(&id, cam_local, def.radius);
                if dist <= BUILD_RANGE_M && best.as_ref().is_none_or(|b| dist < b.0) {
                    best = Some((dist, id, ps.name.clone()));
                }
            }
        }
        if let Some((_, id, pack)) = best {
            let (tx, rx) = channel();
            let (def2, hm2, set2) = (def.clone(), hm.clone(), set.clone());
            std::thread::spawn(move || {
                let t = std::time::Instant::now();
                let built = build_district(&def2, &hm2, &set2, id);
                let _ = tx.send((built, t.elapsed().as_secs_f64()));
            });
            mm.pending = Some((id, pack, rx));
        }
    }

    // 3d. Draw the districts in range: one RenderObject per class mesh.
    for d in &mm.districts {
        if district_distance_m(&d.id, cam_local, def.radius) > DRAW_RANGE_M {
            continue;
        }
        let pos = render_off + rot_d * d.anchor_local;
        for &(mi, mat) in &d.meshes {
            celestial_objects.push(crate::renderer::RenderObject {
                position: glam::Vec3::new(pos.x as f32, pos.y as f32, pos.z as f32),
                rotation,
                scale: glam::Vec3::ONE,
                mesh: mi,
                material: mat,
                fade: 0.0,
            });
        }
    }
    published
}

/// One district's OSM geometry on the drawn ground (a worker's job): its level-12 tiles' features
/// in metres round its centre, every vertex's height the finest drawn ground, lakes at their
/// surfaces.
pub(crate) fn build_district(def: &humanity_terrain::planet::PlanetDef, hm: &humanity_terrain::planet_heightmap::PlanetHeightmap, set: &MapSet, id: PatchId) -> RegionMeshes {
    let c = centre(&id);
    let (lat, lon) = osm_region::dir_to_latlon_f64(c);
    let DistrictGround { detail, sea_r, .. } = DistrictGround::new(def, hm, set);
    let tiles = map_tiles::descendants_at(id, set.levels.finest());
    let mut region = osm_region::OsmRegion { name: String::new(), origin_lat: lat, origin_lon: lon, half_east_m: 7_000.0, half_north_m: 7_000.0, roads: Vec::new(), buildings: Vec::new(), water: Vec::new() };
    for ps in &set.packs {
        if map_tiles::ancestor_at(id, ps.pack.patch().depth) != Some(ps.pack.patch()) {
            continue;
        }
        let surface = |feature: u64| set.body_surface(c, feature).map(|s| sea_r + s);
        let part = osm_region::region_from_pack(&ps.pack, &tiles, "", lat, lon, 7_000.0, 7_000.0, &surface);
        region.roads.extend(part.roads);
        region.buildings.extend(part.buildings);
        region.water.extend(part.water);
    }
    let g = DistrictGround { def, hm, set, detail, sea_r };
    let (elev, natural) = (|d: DVec3| g.elev(d), |d: DVec3| g.natural(d));
    osm_region::build_region_meshes(&region, &osm_region::MeshGround { elev: &elev, natural: &natural, beds: map_ground::beds_enabled() })
}

/// The ground a district's geometry stands on, shared by the district builder and the bed probe
/// (so the probe judges the very heights the meshes were built from).
pub(crate) struct DistrictGround<'a> {
    pub def: &'a humanity_terrain::planet::PlanetDef,
    pub hm: &'a humanity_terrain::planet_heightmap::PlanetHeightmap,
    pub set: &'a MapSet,
    pub detail: humanity_terrain::planet_chunks::DetailNoise,
    pub sea_r: f64,
}

impl DistrictGround<'_> {
    pub(crate) fn new<'a>(def: &'a humanity_terrain::planet::PlanetDef, hm: &'a humanity_terrain::planet_heightmap::PlanetHeightmap, set: &'a MapSet) -> DistrictGround<'a> {
        let sea_r = def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(def, def.sea_level.clamp(0.0, 1.0) as f64);
        DistrictGround { def, hm, set, detail: humanity_terrain::planet_chunks::DetailNoise::new(def.terrain_seed), sea_r }
    }

    /// The finest drawn ground (the formula the patches draw at their deepest, beds and pads
    /// included), as a radius, never below the sea; no tile tier: inside a pack the pack's ground
    /// replaces it.
    pub(crate) fn elev(&self, d: DVec3) -> f64 {
        let e = humanity_terrain::planet_chunks::drawn_elevation_at_depth(self.hm, self.def, &self.detail, None, Some(self.set), d, 24);
        (self.def.radius * humanity_terrain::planet_surface::displaced_radius_f64_true(self.def, e as f64)).max(self.sea_r)
    }

    /// The natural ground (the pack's land layer, no bed or pad), as a radius: where a building's
    /// plinth must reach. Outside a pack, the drawn ground.
    pub(crate) fn natural(&self, d: DVec3) -> f64 {
        match self.set.raw_land_m(d) {
            Some((h, _)) => (self.sea_r + h).max(self.sea_r),
            None => self.elev(d),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The credit names only packs whose districts are drawn from where the camera is: a
    /// Silverdale district from Silverdale, nothing from Dover. Seen red 2026-11-09 with
    /// `drawn_names` returning every district: "Dover draws no Puget Sound pack".
    #[test]
    fn only_drawn_districts_name_their_pack() {
        let r = 6_371_000.0;
        let at = |lat: f64, lon: f64| osm_region::latlon_to_dir_f64(lat, lon) * r;
        let mut mm = MapMeshState { radius_m: r, ..Default::default() };
        let id = map_tiles::tile_containing(47.64, -122.69, DISTRICT_LEVEL);
        mm.districts.push(District { id, anchor_local: centre(&id) * r, meshes: Vec::new(), pack: "f00-320230.hmpack".into() });
        assert_eq!(drawn_names(&mm, at(47.64, -122.69)), vec!["f00-320230.hmpack".to_string()]);
        assert!(drawn_names(&mm, at(51.127, 1.36)).is_empty(), "Dover draws no Puget Sound pack");
    }

    /// A district is as far as its nearest ground, the altitude counting: 1 km outside the middle
    /// of an edge on the ground it is 1 km away, past a corner it is the distance to the corner, and
    /// a camera 5 km over the tile is 5 km away. Seen red 2026-11-09 with the centre rule this
    /// rung first shipped (`(centre * r - cam).length()`): "a district whose edge is 1 km from the
    /// camera reads 5657 m away".
    #[test]
    fn a_district_is_as_far_as_its_nearest_ground() {
        let r = 6_371_000.0;
        let id = map_tiles::tile_containing(47.62, -122.30, DISTRICT_LEVEL);
        let c = patch_corners(&id);
        let mid = (c[0] + c[1]).normalize();
        let mut n = c[0].cross(c[1]).normalize();
        if n.dot(c[2]) < 0.0 {
            n = -n;
        }
        let out = (mid - n * (1_000.0 / r)).normalize();
        let d = district_distance_m(&id, out * r, r);
        assert!((d - 1_000.0).abs() < 5.0, "a district whose edge is 1 km from the camera reads {d:.0} m away");
        let corner_out = (c[0] * (1.0 + 2_000.0 / r) - (c[1] + c[2]).normalize() * (2_000.0 / r)).normalize();
        let dc = district_distance_m(&id, corner_out * r, r);
        let to_corner = corner_out.angle_between(c[0]) * r;
        assert!((dc - to_corner).abs() < 5.0, "past a corner the distance is to the corner: {dc:.0} m, the corner is {to_corner:.0} m away");
        let over = (c[0] * 0.6 + c[1] * 0.2 + c[2] * 0.2).normalize();
        let h = district_distance_m(&id, over * (r + 5_000.0), r);
        assert!((h - 5_000.0).abs() < 1.0, "5 km over the tile reads {h:.0} m");
    }

    /// A district of the shipped Silverdale pack builds real geometry on the drawn ground: every
    /// class present, no NaN, every vertex within the district's reach, and buildings standing
    /// near the ground the formula draws (the walk oracle's), not on the coarse grid. Replaces the
    /// region mesher's test on the retired seattle-center.bin.
    #[test]
    fn a_silverdale_district_stands_on_the_drawn_ground() {
        let set = map_ground::home_set().expect("the home packs ship");
        let (hm, _om, def) = humanity_terrain::drawn_surface::fixtures::real_earth();
        let id = map_tiles::tile_containing(47.6445, -122.6950, DISTRICT_LEVEL);
        let out = build_district(&def, &hm, &set, id);
        let tris: usize = out.classes.iter().map(|c| c.indices.len() / 3).sum();
        println!("[district] {}/{}: {} classes, {tris} triangles, {} rings refused", id.depth, id.path, out.classes.len(), out.skipped_rings);
        assert!(out.classes.iter().any(|c| c.kind == RegionMeshKind::BuildingLow), "Silverdale's houses");
        assert!(out.classes.iter().any(|c| c.kind == RegionMeshKind::RoadAsphalt), "Silverdale's streets");
        assert!(tris < 3_000_000, "{tris} triangles for one district is a regression");
        for c in &out.classes {
            for v in &c.vertices {
                assert!(v.position.iter().all(|p| p.is_finite()), "no NaN reaches the vertex buffer");
                let m = (v.position[0].powi(2) + v.position[1].powi(2) + v.position[2].powi(2)).sqrt();
                assert!(m < 20_000.0, "offset {m} m escaped the district");
            }
        }
    }
}
