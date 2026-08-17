//! OSM region 3D residency (maps ladder rung 3 increment 2, v0.1148):
//! stands the fetched roads and buildings up in the world at their real
//! geodetic coordinates on the chunked-LOD Earth. Executes
//! docs/design/osm-extrusion-plan.md as written.
//!
//! Shape: `data/maps/regions/*.bin` parse once at world entry; when the
//! camera comes within range of a region origin, an ELEVATION GRID is
//! sampled on the main thread SPREAD OVER FRAMES (the drawn-ground oracle
//! `ground_radius_m` needs `TerrainTiles`, which cannot cross threads;
//! ~16k samples a frame keeps it invisible), then the pure mesh build
//! (extrusion + ribbons, `terrain::osm_region::build_region_meshes`) runs
//! on a background thread against a bilinear view of that grid, and the
//! result uploads on arrival. Once built, per-class RenderObjects ride the
//! classic celestial path every frame (the near-tree precedent: planet-
//! local f64 base, `render_off + rot_d * anchor` narrowed last).
//!
//! Meshes stay resident once built (a region is ~15 MB of triangles at
//! most; eviction is the scaling lever the plan notes for many-region
//! futures). If the initial build ran base-only and the region's terrain
//! tile later becomes resident, the grid + meshes rebuild ONCE so
//! buildings sit on tile-refined ground instead of hovering.

use crate::terrain::osm_region::{self, OsmRegion, RegionMeshKind, RegionMeshes};
use glam::DVec3;
use std::sync::mpsc::{channel, Receiver};

/// Camera-to-origin gate for building the region (meters).
const BUILD_RANGE_M: f64 = 40_000.0;
/// Camera-to-origin gate for DRAWING an already-built region (meters).
const DRAW_RANGE_M: f64 = 60_000.0;
/// Elevation grid resolution (cells per axis). 512 over a ~5 km region is
/// ~10 m cells: finer than the 460 m tile data it samples, coarse enough
/// that the whole grid is one ~1 MB Vec.
const GRID_N: usize = 512;
/// Main-thread elevation samples per frame while the grid builds.
const GRID_SAMPLES_PER_FRAME: usize = 16_384;

/// One resident, drawable region.
pub(crate) struct ActiveRegion {
    pub region_idx: usize,
    /// Planet-unrotated local anchor (origin dir * drawn radius), f64.
    pub anchor_local: DVec3,
    /// (renderer mesh index, renderer material index) per class mesh.
    pub meshes: Vec<(usize, usize)>,
    pub built_with_tiles: bool,
    pub name: String,
}

/// The in-progress elevation grid snapshot (meters east/north -> radius).
pub(crate) struct ElevGrid {
    region_idx: usize,
    origin_lat: f64,
    origin_lon: f64,
    /// Grid half-extents in meters (region half spans + a margin so road
    /// clip tails and the 300 m fetcher margin stay inside).
    half_e: f64,
    half_n: f64,
    /// Radii in meters, GRID_N x GRID_N, row-major north-up.
    data: Vec<f32>,
    /// Next flat index to sample (progressive fill).
    progress: usize,
    with_tiles: bool,
    /// Fine-octave noise for the drawn-ground oracle, built once per grid.
    detail: crate::terrain::planet_chunks::DetailNoise,
}

impl ElevGrid {
    fn done(&self) -> bool {
        self.progress >= GRID_N * GRID_N
    }
    /// Bilinear radius at meters east/north (clamped to the grid).
    fn sample(&self, e: f64, n: f64) -> f64 {
        let fx = ((e + self.half_e) / (2.0 * self.half_e) * (GRID_N - 1) as f64)
            .clamp(0.0, (GRID_N - 1) as f64);
        let fy = ((n + self.half_n) / (2.0 * self.half_n) * (GRID_N - 1) as f64)
            .clamp(0.0, (GRID_N - 1) as f64);
        let (x0, y0) = (fx as usize, fy as usize);
        let (x1, y1) = ((x0 + 1).min(GRID_N - 1), (y0 + 1).min(GRID_N - 1));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let g = |x: usize, y: usize| self.data[y * GRID_N + x] as f64;
        (g(x0, y0) * (1.0 - tx) + g(x1, y0) * tx) * (1.0 - ty)
            + (g(x0, y1) * (1.0 - tx) + g(x1, y1) * tx) * ty
    }
}

/// All region-mesh state, owned by EngineState.
pub(crate) struct RegionMeshState {
    pub regions: Vec<OsmRegion>,
    pub loaded: bool,
    pub active: Vec<ActiveRegion>,
    grid: Option<ElevGrid>,
    pending: Option<(usize, bool, Receiver<RegionMeshes>)>,
    /// Renderer material index per RegionMeshKind ordinal, registered once.
    materials: Option<[usize; 6]>,
    /// Water-carve masks rasterized + published once (see tick step 0).
    carve_built: bool,
}

impl Default for RegionMeshState {
    fn default() -> Self {
        Self {
            regions: Vec::new(),
            loaded: false,
            active: Vec::new(),
            grid: None,
            pending: None,
            materials: None,
            carve_built: false,
        }
    }
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

impl RegionMeshState {
    /// Parse every installed region file once (world entry / first tick).
    fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let dir = crate::DATA_DIR
            .get()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("data"))
            .join("maps/regions");
        if let Ok(entries) = std::fs::read_dir(&dir) {
            let mut paths: Vec<_> = entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "bin"))
                .collect();
            paths.sort();
            for p in paths {
                if let Some(r) = std::fs::read(&p).ok().and_then(|b| osm_region::parse_region(&b)) {
                    log::info!(
                        "[Region] \"{}\" parsed: {} roads, {} buildings",
                        r.name,
                        r.roads.len(),
                        r.buildings.len()
                    );
                    self.regions.push(r);
                }
            }
        }
    }

    /// Register the six class materials once. Colors are cartography /
    /// architecture reads, not theme (the theme lint scopes gui+renderer
    /// sources; engine constants for world materials follow the material
    /// system's own registry pattern).
    fn ensure_materials(&mut self, renderer: &mut crate::renderer::Renderer) -> [usize; 6] {
        *self.materials.get_or_insert_with(|| {
            let m = |renderer: &mut crate::renderer::Renderer,
                     rgba: [f32; 4],
                     metallic: f32,
                     rough: f32,
                     ty: f32| {
                renderer.add_material_typed(rgba, metallic, rough, ty)
            };
            [
                // Low/unknown: warm masonry, concrete shading.
                m(renderer, [0.52, 0.44, 0.38, 1.0], 0.0, 0.85, 2.0),
                // Mid-rise: concrete.
                m(renderer, [0.55, 0.55, 0.58, 1.0], 0.0, 0.75, 2.0),
                // High-rise: glassy curtain wall.
                m(renderer, [0.40, 0.48, 0.55, 1.0], 0.6, 0.25, 1.0),
                // Asphalt.
                m(renderer, [0.16, 0.16, 0.17, 1.0], 0.0, 0.95, 2.0),
                // Footpath: light aggregate.
                m(renderer, [0.45, 0.44, 0.41, 1.0], 0.0, 0.9, 2.0),
                // Inland water sheet: deep blue-green, near-mirror smooth.
                // Lakes sit above sea level where the ocean shell cannot
                // reach, so this flat sheet is their whole surface for now.
                m(renderer, [0.09, 0.20, 0.24, 1.0], 0.1, 0.08, 2.0),
            ]
        })
    }
}

/// Per-frame driver, called from the Earth branch of the celestial pass.
/// `cam_local` is the camera in the planet's unrotated local frame (f64);
/// the RenderObject transform inputs match the classic-patch push exactly.
///
/// Returns true on the ONE frame the water-carve masks were published, so
/// the caller can drop any terrain/water patches cached before the carve
/// existed (in practice this fires before the first patch build of the
/// world, because this tick runs earlier in the frame; the purge is the
/// belt-and-braces for ordering surprises and future mid-session region
/// installs).
#[allow(clippy::too_many_arguments)]
pub(crate) fn tick(
    rm: &mut RegionMeshState,
    def: &crate::terrain::planet::PlanetDef,
    hm: &crate::terrain::planet_heightmap::PlanetHeightmap,
    tiles: &crate::terrain::terrain_tiles::TerrainTiles,
    renderer: &mut crate::renderer::Renderer,
    celestial_objects: &mut Vec<crate::renderer::RenderObject>,
    cam_local: DVec3,
    render_off: DVec3,
    rot_d: glam::DQuat,
    rotation: glam::Quat,
) -> bool {
    rm.ensure_loaded();
    if rm.regions.is_empty() {
        return false;
    }

    // 0. Rasterize + publish the water-carve masks ONCE. This must happen
    //    while the global registry is still empty: the lake-surface probe
    //    below reads the UNCARVED drawn ground through the same oracle the
    //    walk clamp uses.
    let mut carve_published = false;
    if !rm.carve_built {
        rm.carve_built = true;
        let detail = crate::terrain::planet_chunks::DetailNoise::new(def.terrain_seed);
        let sea_r = def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64;
        // Sibling .dem.bin files (real ~13 m elevation, v0.1153): keyed by
        // region name order, loaded once here.
        let dem_dir = crate::DATA_DIR
            .get()
            .cloned()
            .unwrap_or_else(|| std::path::PathBuf::from("data"))
            .join("maps/regions");
        let mut dems: Vec<Option<crate::terrain::osm_region::RegionDem>> = Vec::new();
        for r in &rm.regions {
            // The mesher never learned file paths (regions arrive parsed),
            // so recover the stem by matching origin: cheapest robust key is
            // scanning the dir for a .dem.bin whose parsed bounds contain
            // the region origin.
            let mut found = None;
            if let Ok(entries) = std::fs::read_dir(&dem_dir) {
                for ent in entries.flatten() {
                    let p = ent.path();
                    if !p.file_name().is_some_and(|n| n.to_string_lossy().ends_with(".dem.bin")) {
                        continue;
                    }
                    if let Some(d) =
                        std::fs::read(&p).ok().and_then(|b| crate::terrain::osm_region::parse_dem(&b))
                    {
                        let (s, n, w, e) = d.bounds_deg();
                        if r.origin_lat > s && r.origin_lat < n && r.origin_lon > w && r.origin_lon < e
                        {
                            log::info!(
                                "[Region] \"{}\" real elevation: {} ({}x{}, {:.0}..{:.0} m)",
                                r.name,
                                p.file_name().unwrap_or_default().to_string_lossy(),
                                d.width,
                                d.height,
                                d.min_m,
                                d.max_m
                            );
                            found = Some(d);
                            break;
                        }
                    }
                }
            }
            dems.push(found);
        }
        let mut masks = Vec::new();
        for (ri, r) in rm.regions.iter().enumerate() {
            if r.water.is_empty() && r.buildings.is_empty() && r.roads.is_empty() {
                continue;
            }
            // A lake's surface: the LOWEST shore point of its ring against
            // the uncarved ground, floored just above sea level so an
            // estuary-grade polygon cannot duck under the ocean shell. With
            // a DEM installed, the shore reads the ~13 m survey directly:
            // the real lake level, not the 460 m average.
            let dem = &dems[ri];
            let lake_level = |wi: usize| -> f64 {
                let w = &r.water[wi];
                let mut min_rel = f64::MAX;
                for &(e, n) in &w.ring {
                    let (lat, lon) = osm_region::region_meters_to_latlon(
                        r.origin_lat,
                        r.origin_lon,
                        e as f64,
                        n as f64,
                    );
                    if let Some(d) = dem {
                        if let Some(m) = d.sample_m(lat, lon) {
                            min_rel = min_rel.min(m as f64);
                            continue;
                        }
                    }
                    let dir = osm_region::latlon_to_dir_f64(lat, lon);
                    let gr = crate::engine::frame_lock::ground_radius_m(
                        Some(def),
                        Some(hm),
                        Some(&detail),
                        Some(tiles),
                        dir,
                    );
                    min_rel = min_rel.min(gr - sea_r);
                }
                min_rel.max(0.5)
            };
            if let Some(m) = crate::terrain::water_carve::RegionMask::from_region_with_dem(
                r,
                &lake_level,
                dems[ri].clone(),
            ) {
                masks.push(m);
            }
        }
        if !masks.is_empty() {
            let count = masks.len();
            crate::terrain::water_carve::set_global(std::sync::Arc::new(masks));
            carve_published = true;
            log::info!("[Region] water carve published for {count} region(s)");
        }
    }

    // 1. Progress the elevation grid, if one is building (main thread: the
    //    ONLY place TerrainTiles is touchable). ground_radius_m is the same
    //    drawn-ground oracle the walk clamp uses.
    if let Some(g) = &mut rm.grid {
        if !g.done() {
            let end = (g.progress + GRID_SAMPLES_PER_FRAME).min(GRID_N * GRID_N);
            for i in g.progress..end {
                let (x, y) = (i % GRID_N, i / GRID_N);
                let e = -g.half_e + (x as f64 / (GRID_N - 1) as f64) * 2.0 * g.half_e;
                let n = -g.half_n + (y as f64 / (GRID_N - 1) as f64) * 2.0 * g.half_n;
                let (lat, lon) = osm_region::region_meters_to_latlon(g.origin_lat, g.origin_lon, e, n);
                let dir = osm_region::latlon_to_dir_f64(lat, lon);
                let r = crate::engine::frame_lock::ground_radius_m(Some(def), Some(hm), Some(&g.detail), Some(tiles), dir);
                // Sea clamp HERE (the mesher's elev contract): waterfront
                // cells sample Puget Sound bathymetry, and a building base
                // must never sink to the seabed.
                let sea = def.radius + crate::terrain::ocean_waves::SURFACE_LIFT_M as f64;
                g.data[i] = r.max(sea) as f32;
            }
            g.progress = end;
        } else if rm.pending.is_none() {
            // 2. Grid complete: hand the pure mesh build to a worker. The
            //    region clone is a few hundred KB; the grid moves with it.
            let g = rm.grid.take().unwrap();
            let region = rm.regions[g.region_idx].clone();
            let (tx, rx) = channel();
            let idx = g.region_idx;
            let with_tiles = g.with_tiles;
            std::thread::spawn(move || {
                let elev = |dir: DVec3| -> f64 {
                    let (lat, lon) = osm_region::dir_to_latlon_f64(dir);
                    let (e, n) = osm_region::latlon_to_region_meters(
                        g.origin_lat,
                        g.origin_lon,
                        lat,
                        lon,
                    );
                    g.sample(e, n)
                };
                let built = osm_region::build_region_meshes(&region, &elev);
                let _ = tx.send(built);
            });
            rm.pending = Some((idx, with_tiles, rx));
        }
    }

    // 3. Drain a finished build: upload meshes, activate the region.
    if let Some((idx, with_tiles, rx)) = &rm.pending {
        let (idx, with_tiles) = (*idx, *with_tiles);
        match rx.try_recv() {
            Ok(built) => {
                rm.pending = None;
                let mats = rm.ensure_materials(renderer);
                // A rebuild (tile arrival) replaces the old upload in place.
                let name = rm.regions[idx].name.clone();
                let prior: Option<ActiveRegion> = {
                    let pos = rm.active.iter().position(|a| a.region_idx == idx);
                    pos.map(|p| rm.active.remove(p))
                };
                let mut meshes = Vec::new();
                for (ci, class) in built.classes.iter().enumerate() {
                    if class.indices.is_empty() {
                        continue;
                    }
                    let mesh = crate::renderer::mesh::Mesh::from_vertices(
                        &renderer.device,
                        &class.vertices,
                        &class.indices,
                    );
                    let mi = match prior.as_ref().and_then(|p| p.meshes.get(ci)) {
                        Some(&(old_mi, _)) => {
                            renderer.replace_mesh(old_mi, mesh);
                            old_mi
                        }
                        None => renderer.add_mesh(mesh),
                    };
                    meshes.push((mi, mats[kind_ordinal(class.kind)]));
                }
                if built.skipped_rings > 0 {
                    log::info!(
                        "[Region] \"{name}\": {} unclippable footprint rings skipped",
                        built.skipped_rings
                    );
                }
                log::info!(
                    "[Region] \"{name}\" resident: {} class meshes (tiles: {with_tiles}). Data (c) OpenStreetMap contributors, ODbL 1.0",
                    meshes.len()
                );
                crate::debug::push_debug(format!(
                    "Region \"{name}\" standing: OSM data (c) OpenStreetMap contributors (ODbL)"
                ));
                rm.active.push(ActiveRegion {
                    region_idx: idx,
                    anchor_local: built.anchor_local,
                    meshes,
                    built_with_tiles: with_tiles,
                    name,
                });
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
            Err(std::sync::mpsc::TryRecvError::Disconnected) => rm.pending = None,
        }
    }

    // 4. Start a build for the nearest in-range unbuilt region (one grid /
    //    build in flight at a time), or a one-shot tile-refresh rebuild.
    if rm.grid.is_none() && rm.pending.is_none() {
        let mut best: Option<(usize, f64, bool)> = None; // (idx, d2, is_rebuild)
        for (i, r) in rm.regions.iter().enumerate() {
            let dir = osm_region::latlon_to_dir_f64(r.origin_lat, r.origin_lon);
            let origin =
                dir * crate::engine::frame_lock::ground_radius_m(Some(def), Some(hm), None, Some(tiles), dir);
            let d = (origin - cam_local).length();
            if d > BUILD_RANGE_M {
                continue;
            }
            match rm.active.iter().find(|a| a.region_idx == i) {
                None => {
                    if best.map_or(true, |(_, bd, _)| d < bd) {
                        best = Some((i, d, false));
                    }
                }
                Some(a) if !a.built_with_tiles => {
                    // Tile now resident? Rebuild once so the ground agrees.
                    if tiles.sample_meters_smooth(r.origin_lat, r.origin_lon).is_some()
                        && best.map_or(true, |(_, bd, _)| d < bd)
                    {
                        best = Some((i, d, true));
                    }
                }
                Some(_) => {}
            }
        }
        if let Some((idx, _, _)) = best {
            let r = &rm.regions[idx];
            let with_tiles = tiles.sample_meters_smooth(r.origin_lat, r.origin_lon).is_some();
            rm.grid = Some(ElevGrid {
                region_idx: idx,
                origin_lat: r.origin_lat,
                origin_lon: r.origin_lon,
                half_e: r.half_east_m as f64 + 500.0,
                half_n: r.half_north_m as f64 + 500.0,
                data: vec![0.0; GRID_N * GRID_N],
                progress: 0,
                with_tiles,
                detail: crate::terrain::planet_chunks::DetailNoise::new(def.terrain_seed),
            });
            log::info!(
                "[Region] \"{}\" in range: sampling the elevation grid ({}x{}, tiles: {with_tiles})",
                r.name,
                GRID_N,
                GRID_N
            );
        }
    }

    // 5. Draw every active region within range: one RenderObject per class
    //    mesh, the exact classic-patch transform (f64 until the final
    //    narrowing; vertices are anchor-relative offsets in the planet's
    //    unrotated frame, so rotation = the planet spin quat).
    for a in &rm.active {
        if (a.anchor_local - cam_local).length() > DRAW_RANGE_M {
            continue;
        }
        let pos_render = render_off + rot_d * a.anchor_local;
        for &(mi, mat) in &a.meshes {
            celestial_objects.push(crate::renderer::RenderObject {
                position: glam::Vec3::new(
                    pos_render.x as f32,
                    pos_render.y as f32,
                    pos_render.z as f32,
                ),
                rotation,
                scale: glam::Vec3::ONE,
                mesh: mi,
                material: mat,
                fade: 0.0,
            });
        }
    }

    carve_published
}
