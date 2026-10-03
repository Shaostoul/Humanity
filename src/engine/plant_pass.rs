//! The garden's plant geometry, built off the main thread (2026-09-27).
//!
//! The home garden draws about 3.35 million vertices of plants. Built inside
//! the frame that noticed a change, the first build was about 190 ms and a
//! whole-garden change (a new visual recipe, every tower changing stage at
//! once) rebuilt dozens of machines in one frame: a hitch the player sees
//! as the game freezing. Now the frame only decides WHAT to rebuild
//! (`home_meshes::rebuild_plant_meshes`); a worker thread builds the CPU
//! geometry here, and the frame uploads what is finished a few milliseconds
//! at a time. A machine keeps drawing its old plants until its new ones are
//! uploaded, so nothing blinks out, and what is drawn in the end is exactly
//! what the frame used to build: this is the same code, moved.
//!
//! Pure CPU work with no engine state: everything a machine's plants are
//! built from travels in its [`GroupJob`], so the build is unit tested here.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};

use glam::{Quat, Vec3};

use crate::assets::GltfCpuMesh;
use crate::engine::fungus_mesh::{self, Growth};
use crate::engine::plant_layout::{bake_copy, plot_draw_cap, plot_plants, PlotRect};
use crate::renderer::plant_mesh::{build_plant, generic_visual, PlantMeshBuilder, PlantVisualRegistry};

/// The part of a tower's config its plants are placed from.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Helix {
    pub slots: u32,
    pub turns: f32,
    pub height_m: f32,
    pub radius_m: f32,
}

/// Where one machine's crops stand.
#[derive(Debug, Clone)]
pub(crate) enum GroupLayout {
    /// An aeroponic tower: one plant per net cup up the helix from `base`.
    Tower { helix: Helix, base: Vec3 },
    /// A bed, tray, rack or field: its plots (`plant_layout::plot_rects`)
    /// in the machine's frame, placed at `pos` turned `yaw_deg`.
    Plots { pos: Vec3, yaw_deg: f32, rects: Vec<PlotRect> },
}

/// One crop as it is drawn, resolved on the main thread from its
/// `CropInstance` and the plant registry.
#[derive(Debug, Clone)]
pub(crate) struct CropDraw {
    pub def_id: String,
    pub slot: u32,
    /// Growth, 0 to 1, bucketed by stage the way the Garden panel shows it.
    pub t: f32,
    pub wilt: f32,
    pub dead: bool,
    /// Plants the crop's plot holds (`farming::units::plants_in_plot`).
    pub holds: u32,
}

/// Everything one machine's plants are built from.
#[derive(Debug, Clone)]
pub(crate) struct GroupJob {
    /// The crop group key: a tower config id or a grow machine instance id.
    pub key: String,
    /// Signature of all of it; the upload keeps only the newest.
    pub sig: u64,
    pub layout: GroupLayout,
    /// Sorted by slot, then species, as the frame always drew them.
    pub crops: Vec<CropDraw>,
}

/// One machine's finished plant geometry, not yet on the GPU.
pub(crate) struct BuiltGroup {
    pub key: String,
    pub sig: u64,
    /// The procedural plants (the type-20 plant material). May be empty.
    pub procedural: PlantMeshBuilder,
    /// Procedural geometry that must not sway (2026-09-27): mushroom crops'
    /// blocks, beds and fruit (`engine::fungus_mesh`), drawn with the rigid
    /// plant material. May be empty.
    pub rigid: PlantMeshBuilder,
    /// One merged mesh per stage model it uses, by model name, sorted.
    pub models: Vec<(String, PlantMeshBuilder)>,
    /// Plants and clumps drawn, and the plants the crops really hold.
    pub drawn: usize,
    pub meant: u64,
    /// [`geometry_checksum`] of its meshes, when the job asked for one.
    pub checksum: Option<u64>,
}

impl BuiltGroup {
    /// Vertices in all of this machine's meshes.
    pub fn vertex_count(&self) -> usize {
        self.procedural.vertices.len()
            + self.rigid.vertices.len()
            + self.models.iter().map(|(_, m)| m.vertices.len()).sum::<usize>()
    }

    /// Its meshes for [`geometry_checksum`]: procedural, rigid, then models.
    /// An empty rigid mesh is left out, so a machine with no mushrooms has
    /// the checksum it had before the rigid mesh existed.
    pub fn meshes(&self) -> Vec<&PlantMeshBuilder> {
        std::iter::once(&self.procedural)
            .chain((!self.rigid.vertices.is_empty()).then_some(&self.rigid))
            .chain(self.models.iter().map(|(_, m)| m))
            .collect()
    }
}

/// What the worker sends back, in order: a model before the first machine
/// that uses it, every machine, then `Done`.
pub(crate) enum WorkerMsg {
    /// A stage model loaded for the first time: its geometry and texture,
    /// for the main thread to make its material and remember.
    Model { name: String, cpu: Arc<GltfCpuMesh>, texture: Option<(Vec<u8>, u32, u32)> },
    /// A stage model that does not exist (most species have none).
    Missing(String),
    Group(BuiltGroup),
    /// The job finished (or was cancelled) after `worker_ms` of building.
    Done { generation: u64, worker_ms: f64 },
}

/// A job handed to [`spawn`]: the machines to build and what they share.
pub(crate) struct Job {
    pub generation: u64,
    pub groups: Vec<GroupJob>,
    pub visuals: Arc<PlantVisualRegistry>,
    /// Stage models already loaded (the main thread's cache).
    pub models: HashMap<String, Arc<GltfCpuMesh>>,
    /// Stage models known not to exist.
    pub missing: HashSet<String>,
    /// The asset manager's data directory, to load new stage models from.
    pub data_dir: std::path::PathBuf,
    /// Set when a newer job replaces this one: it stops between machines.
    pub cancel: Arc<AtomicBool>,
    /// Checksum every machine's geometry (`HUMANITY_PLANT_CHECKSUM=1`).
    pub checksum: bool,
}

/// Build `job` on a worker thread, sending each finished machine to `tx` as
/// soon as it is built. Falls back to building on the calling thread if a
/// thread cannot be started, so plants are late at worst, never missing.
///
/// Its own thread, not rayon's pool: the particle update borrows that pool
/// from inside the frame, and a garden build parked on it would stall the
/// frame it is meant to keep smooth.
pub(crate) fn spawn(job: Job, tx: mpsc::Sender<WorkerMsg>) {
    let cell = Arc::new(std::sync::Mutex::new(Some((job, tx))));
    let take = |c: &std::sync::Mutex<Option<(Job, mpsc::Sender<WorkerMsg>)>>| c.lock().ok().and_then(|mut g| g.take());
    let theirs = cell.clone();
    let spawned = std::thread::Builder::new().name("plant-pass".into()).spawn(move || {
        if let Some((job, tx)) = take(&theirs) {
            run_job(job, &tx);
        }
    });
    if let Err(e) = spawned {
        log::warn!("[Plants] could not start the plant worker ({e}); building on the frame instead");
        if let Some((job, tx)) = take(&cell) {
            run_job(job, &tx);
        }
    }
}

fn run_job(job: Job, tx: &mpsc::Sender<WorkerMsg>) {
    let t0 = std::time::Instant::now();
    let assets = crate::assets::AssetManager::new(job.data_dir.clone());
    let mut known = job.models;
    let mut missing = job.missing;
    let mut lookup = |name: &str| -> Option<Arc<GltfCpuMesh>> {
        if let Some(m) = known.get(name) {
            return Some(m.clone());
        }
        if missing.contains(name) {
            return None;
        }
        match assets.parse_gltf_mesh_with_texture(&format!("assets/models/plants/{name}/{name}.gltf")) {
            Ok((cpu, texture)) => {
                let cpu = Arc::new(cpu);
                known.insert(name.to_string(), cpu.clone());
                let _ = tx.send(WorkerMsg::Model { name: name.to_string(), cpu: cpu.clone(), texture });
                Some(cpu)
            }
            Err(_) => {
                // Not an error: most species have no converted model yet.
                missing.insert(name.to_string());
                let _ = tx.send(WorkerMsg::Missing(name.to_string()));
                None
            }
        }
    };
    for g in &job.groups {
        if job.cancel.load(Ordering::Relaxed) {
            break;
        }
        let mut built = build_group(g, &job.visuals, &mut lookup);
        if job.checksum {
            built.checksum = Some(geometry_checksum(&built.meshes()));
        }
        if tx.send(WorkerMsg::Group(built)).is_err() {
            return; // the engine is gone
        }
    }
    let _ = tx.send(WorkerMsg::Done { generation: job.generation, worker_ms: t0.elapsed().as_secs_f64() * 1000.0 });
}

/// Tallest a plant in a tower's net cup is drawn, metres: a tree there
/// reads as a dwarf or espalier rather than an orchard tree (v0.862).
const TOWER_PLANT_MAX_M: f32 = 0.6;

/// The uniform scale a stage model takes in a tower's net cup (2026-09-29):
/// no taller than `TOWER_PLANT_MAX_M` and no wider than the species' own
/// spread, never enlarged. Uniform, so the model keeps its proportions.
fn tower_model_scale(height: f32, width: f32, spread: f32) -> f32 {
    let mut k = 1.0_f32;
    if height > TOWER_PLANT_MAX_M {
        k = k.min(TOWER_PLANT_MAX_M / height);
    }
    if spread > 0.0 && width > spread {
        k = k.min(spread / width);
    }
    k
}

/// How far a plant in a tower's net cup leans out from the column, radians
/// from upright (2026-10-03). The cups on a vertical aeroponic tower are set
/// into its wall at an angle, mouth up and out, so what grows in them stands
/// out from the column rather than straight up beside it. The angle is a
/// GAME CHOICE for how it reads at eye height, not a sourced figure.
///
/// 15 degrees, down from a first 30 (2026-10-03, measured on the rig). A
/// living plant only leaves its cup at the cup's angle and then bends
/// upward toward the light, but a stage model is rigid, so the whole plant
/// takes the tip. At 30 the beet sprouts read as falling out of the
/// column and the low seedlings' leaves turned down, away from the ceiling
/// lights, into dark shapes: near-black pixels in the two tower close-ups
/// went from 1,224 and 772 upright to 1,575 and 1,542. At 15 the lean
/// still shows and the seedlings read as two leaves again. The pixel count
/// barely moves (1,609 and 1,842: most of it is the beet sprouts' deep red
/// stems in shade, which tip either way), so the call was made by eye.
const NET_CUP_TILT_RAD: f32 = 15.0 * std::f32::consts::PI / 180.0;

/// The turn a stage model takes in the net cup at helix angle `ang`
/// (2026-09-29 faced it out; 2026-10-03 tipped it): its front (+z) turned to
/// face out from the column, then its up tipped `NET_CUP_TILT_RAD` toward
/// that same outward direction, about the base, which sits in the cup.
fn net_cup_turn(ang: f32) -> Quat {
    let out = Vec3::new(ang.cos(), 0.0, ang.sin());
    // Rotating +Y about (Y x out) moves it toward `out`.
    let tip = Quat::from_axis_angle(Vec3::Y.cross(out).normalize(), NET_CUP_TILT_RAD);
    tip * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2 - ang)
}

/// A model's (height, width, depth below its origin) as authored, metres:
/// its Y extent, the larger of its X and Z extents, and how far it reaches
/// under y = 0 (a root crop's root).
fn model_dims(m: &GltfCpuMesh) -> (f32, f32, f32) {
    let (mut lo, mut hi) = ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]);
    for v in &m.vertices {
        for (k, a) in [0usize, 2].into_iter().enumerate() {
            lo[k] = lo[k].min(v.position[a]);
            hi[k] = hi[k].max(v.position[a]);
        }
    }
    let w = if lo[0].is_finite() { (hi[0] - lo[0]).max(hi[1] - lo[1]) } else { 0.0 };
    let below = m.vertices.iter().map(|v| -v.position[1]).fold(0.0_f32, f32::max);
    (m.authored_height(), w, below)
}

/// The stage model a living crop is drawn with, if its set has one (v0.992,
/// the Quaternius growth stages): the species' set (its own lowercased id,
/// or the `stage_models` entry in data/plants_visual.ron) at the growth
/// quartile, "wheat_4" when ripe. Dead crops keep the procedural wilt.
fn stage_model(
    c: &CropDraw,
    visuals: &PlantVisualRegistry,
    model: &mut dyn FnMut(&str) -> Option<Arc<GltfCpuMesh>>,
) -> Option<(String, Arc<GltfCpuMesh>)> {
    if c.dead {
        return None;
    }
    let q = ((c.t * 4.0).ceil() as u32).clamp(1, 4);
    let name = format!("{}_{q}", visuals.stage_model_for(&c.def_id));
    model(&name).map(|m| (name, m))
}

/// Build one machine's plants: the loop the frame used to run inline
/// (v0.862 towers, v0.992 stage models, 2026-09-26 plots), unchanged.
/// `model` looks a stage model up by name ("wheat_3"), None when the set
/// has no model at that name.
pub(crate) fn build_group(
    job: &GroupJob,
    visuals: &PlantVisualRegistry,
    model: &mut dyn FnMut(&str) -> Option<Arc<GltfCpuMesh>>,
) -> BuiltGroup {
    use std::hash::{Hash, Hasher};
    let mut b = PlantMeshBuilder::new();
    let mut rigid = PlantMeshBuilder::new();
    let mut hero: HashMap<String, PlantMeshBuilder> = HashMap::new();
    let (mut drawn, mut meant) = (0usize, 0u64);
    // A tower's stage-model vertices so far (it is budgeted like one plot),
    // and each model's authored size, measured once.
    let mut tower_model_verts = 0usize;
    let mut dims: HashMap<String, (f32, f32, f32)> = HashMap::new();
    for c in &job.crops {
        // A mushroom crop (2026-09-27) is its substrate and the fruit on it,
        // drawn by `fungus_mesh` into the rigid mesh; never a plant recipe
        // or a stage model, so no model is asked for.
        if let Some(f) = visuals.fungus(&c.def_id) {
            let seed = {
                let mut sh = std::collections::hash_map::DefaultHasher::new();
                job.key.hash(&mut sh);
                c.slot.hash(&mut sh);
                sh.finish()
            };
            let g = Growth { t: c.t, wilt: c.wilt, dead: c.dead };
            let (at, yaw, size, holds) = match &job.layout {
                GroupLayout::Plots { pos, yaw_deg, rects } if !rects.is_empty() => {
                    let rect = rects[c.slot as usize % rects.len()];
                    let yaw = yaw_deg.to_radians();
                    let at = *pos + Quat::from_rotation_y(yaw) * Vec3::new(rect.center[0], rect.floor, rect.center[1]);
                    (at, yaw, rect.size, c.holds)
                }
                GroupLayout::Plots { .. } => continue,
                // A net cup holds one unit, at the cup.
                GroupLayout::Tower { helix, base } => {
                    let frac = c.slot as f32 / helix.slots.max(1) as f32;
                    let ang = frac * helix.turns * std::f32::consts::TAU;
                    let y = 0.18 + frac * (helix.height_m - 0.45);
                    let at = *base + Vec3::new(ang.cos() * helix.radius_m, y, ang.sin() * helix.radius_m);
                    // Turned so its front (+z) faces out from the column.
                    (at, std::f32::consts::FRAC_PI_2 - ang, fungus_mesh::unit_footprint(f), 1)
                }
            };
            let d = fungus_mesh::build_plot(
                &mut rigid,
                f,
                at,
                yaw,
                size,
                holds,
                g,
                seed,
                visuals.plot_visual_cap,
                visuals.plot_vertex_budget,
            );
            drawn += d.units as usize;
            meant += u64::from(holds);
            continue;
        }
        // v0.903 (operator: "the potato garden is just a plain slab of
        // brown"): only 10 of ~134 crops had visual recipes, and every crop
        // WITHOUT one silently skipped mesh generation, leaving bare beds
        // and empty tower net cups. Unrecipe'd crops get a generic leafy
        // plant (deterministically varied per species) so every garden
        // visibly GROWS; hand-authored recipes in data/plants_visual.ron
        // still win when present.
        let generic;
        let vis = match visuals.get(&c.def_id) {
            Some(v) => v,
            None => {
                generic = generic_visual(&c.def_id);
                &generic
            }
        };
        let seed = {
            let mut sh = std::collections::hash_map::DefaultHasher::new();
            job.key.hash(&mut sh);
            c.slot.hash(&mut sh);
            sh.finish()
        };
        match &job.layout {
            GroupLayout::Tower { helix, base } => {
                // Helix slot position up the column, plant facing outward.
                let frac = c.slot as f32 / helix.slots.max(1) as f32;
                let ang = frac * helix.turns * std::f32::consts::TAU;
                let y = 0.18 + frac * (helix.height_m - 0.45);
                let out = [ang.cos(), 0.0, ang.sin()];
                let pos = [base.x + out[0] * helix.radius_m, base.y + y, base.z + out[2] * helix.radius_m];
                // Tower plants render at reduced scale so a tree in a net cup
                // reads as a dwarf/espalier rather than a full orchard tree.
                let mut vis_scaled = vis.clone();
                if vis_scaled.height_m > TOWER_PLANT_MAX_M {
                    let k = TOWER_PLANT_MAX_M / vis_scaled.height_m;
                    vis_scaled.height_m *= k;
                    vis_scaled.spread_m *= k;
                    vis_scaled.stem_radius *= k;
                }
                // The real stage model in the net cup too (2026-09-29). The
                // v0.992 gate kept towers procedural because a full-size
                // pumpkin vine in a net cup would be silly; the model is now
                // scaled uniformly to the same dwarf height and to the
                // species' spread, set a little out from the column, turned
                // to face out and tipped out the way its cup holds it
                // (`net_cup_turn`). A tower is budgeted like one plot
                // (`plot_vertex_budget`): past it, the procedural plant. So is
                // a model that reaches well under its origin: a root crop's
                // root would hang out of the cup.
                let budget = visuals.plot_vertex_budget as usize;
                let hit = stage_model(c, visuals, model).filter(|(name, m)| {
                    let (h, _, below) = *dims.entry(name.clone()).or_insert_with(|| model_dims(m));
                    below <= 0.1 * h && (budget == 0 || tower_model_verts + m.vertices.len() <= budget)
                });
                if let Some((name, cpu)) = hit {
                    let (h, w, _) = dims[&name];
                    let k = tower_model_scale(h, w, vis_scaled.spread_m);
                    let at = Vec3::from(pos) + Vec3::from(out) * (w * k * 0.35);
                    let mesh = hero.entry(name).or_insert_with(PlantMeshBuilder::new);
                    bake_copy(&mut mesh.vertices, &mut mesh.indices, &cpu.vertices, &cpu.indices, at, net_cup_turn(ang), k, k);
                    tower_model_verts += cpu.vertices.len();
                } else {
                    build_plant(&mut b, &vis_scaled, pos, out, c.t, c.wilt, seed);
                }
                drawn += 1;
                meant += 1;
            }
            GroupLayout::Plots { pos, yaw_deg, rects } => {
                if rects.is_empty() {
                    continue;
                }
                // A slot past the medium's plots (more crops than plots)
                // shares a plot rather than standing outside the machine.
                let rect = rects[c.slot as usize % rects.len()];
                let turn = Quat::from_rotation_y(yaw_deg.to_radians());
                // Hero crop models (v0.992): the real 3D model at the stage
                // quartile instead of the procedural recipe (`stage_model`).
                let hit = stage_model(c, visuals, model);
                // One plant's vertices at this stage, for the plot's vertex
                // budget: the model's, or one procedural plant built to count.
                let per_plant = match &hit {
                    Some((_, cpu)) => cpu.vertices.len(),
                    None => {
                        let mut probe = PlantMeshBuilder::new();
                        build_plant(&mut probe, vis, [0.0; 3], [0.7, 0.0, 0.7], c.t, c.wilt, seed);
                        probe.vertices.len()
                    }
                };
                let draw_cap = plot_draw_cap(visuals.plot_visual_cap, visuals.plot_vertex_budget, per_plant);
                let layout = plot_plants(rect.size, c.holds, draw_cap, seed);
                // A clump is its plants' floor wide at one plant's height.
                let mut vis_clump = vis.clone();
                vis_clump.spread_m *= layout.widen;
                for (k, spot) in layout.spots.iter().enumerate() {
                    let local = Vec3::new(rect.center[0] + spot[0], rect.floor, rect.center[1] + spot[1]);
                    let at = *pos + turn * local;
                    let plant_seed = seed ^ (k as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                    if let Some((name, cpu)) = &hit {
                        // Deterministic per-plant turn and a few percent of
                        // height either way, so a stand is not a lattice of
                        // clones; a rebuild never changes either.
                        let yaw = (plant_seed % 3600) as f32 * (std::f32::consts::TAU / 3600.0);
                        let tall = 0.93 + ((plant_seed >> 24) % 15) as f32 * 0.01;
                        let mesh = hero.entry(name.clone()).or_insert_with(PlantMeshBuilder::new);
                        bake_copy(&mut mesh.vertices, &mut mesh.indices, &cpu.vertices, &cpu.indices, at, Quat::from_rotation_y(yaw), layout.widen, tall);
                    } else {
                        build_plant(&mut b, &vis_clump, at.to_array(), [0.7, 0.0, 0.7], c.t, c.wilt, plant_seed);
                    }
                    drawn += 1;
                }
                meant += u64::from(c.holds);
            }
        }
    }
    let mut models: Vec<(String, PlantMeshBuilder)> =
        hero.into_iter().filter(|(_, m)| !m.vertices.is_empty()).collect();
    models.sort_by(|x, y| x.0.cmp(&y.0));
    BuiltGroup { key: job.key.clone(), sig: job.sig, procedural: b, rigid, models, drawn, meant, checksum: None }
}

/// An order-free checksum of a machine's geometry (the bits of every vertex
/// position, normal and uv, and every index), for proving a change to the
/// plant pass draws exactly what it drew before. Only computed when
/// `HUMANITY_PLANT_CHECKSUM=1` is set: it reads every byte.
pub(crate) fn geometry_checksum(meshes: &[&PlantMeshBuilder]) -> u64 {
    let mut total = 0u64;
    for m in meshes {
        let mut h: u64 = 0xCBF2_9CE4_8422_2325;
        let mut mix = |w: u32| {
            h ^= u64::from(w);
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        };
        for v in &m.vertices {
            for f in v.position.iter().chain(v.normal.iter()).chain(v.uv.iter()) {
                mix(f.to_bits());
            }
        }
        for &i in &m.indices {
            mix(i);
        }
        total = total.wrapping_add(h);
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::plant_layout::plot_rects;
    use crate::renderer::mesh::Vertex;

    fn visuals() -> PlantVisualRegistry {
        PlantVisualRegistry { plot_visual_cap: 128, plot_vertex_budget: 65_536, ..PlantVisualRegistry::default() }
    }

    fn crop(def_id: &str, slot: u32, t: f32, holds: u32) -> CropDraw {
        CropDraw { def_id: def_id.to_string(), slot, t, wilt: 0.0, dead: false, holds }
    }

    fn tower(n: u32) -> GroupJob {
        GroupJob {
            key: "ntower_0".into(),
            sig: 7,
            layout: GroupLayout::Tower {
                helix: Helix { slots: 50, turns: 3.0, height_m: 1.8, radius_m: 0.15 },
                base: Vec3::new(1.0, 0.0, 2.0),
            },
            crops: (0..n).map(|s| crop("kale", s, 0.5, 1)).collect(),
        }
    }

    /// A 2 x 1 m bed of two plots at x 4, z 6, turned 90 degrees: potatoes
    /// half grown in plot 0, `model_crop` ripe in plot 1.
    fn bed(model_crop: &str) -> GroupJob {
        GroupJob {
            key: "potato_bed_0".into(),
            sig: 9,
            layout: GroupLayout::Plots {
                pos: Vec3::new(4.0, 0.0, 6.0),
                yaw_deg: 90.0,
                rects: plot_rects((2.0, 0.4, 1.0), 2, false),
            },
            crops: vec![crop("potato", 0, 0.5, 4), crop(model_crop, 1, 1.0, 6)],
        }
    }

    /// A little stand-in stage model: one triangle, 1 m tall.
    fn tiny_model() -> Arc<GltfCpuMesh> {
        let v = |p: [f32; 3]| Vertex { position: p, normal: [0.0, 0.0, 1.0], uv: [0.0, 0.0] };
        Arc::new(GltfCpuMesh {
            vertices: vec![v([0.0, 0.0, 0.0]), v([0.1, 0.0, 0.0]), v([0.0, 1.0, 0.0])],
            indices: vec![0, 1, 2],
        })
    }

    fn checksum_of(g: &BuiltGroup) -> u64 {
        geometry_checksum(&g.meshes())
    }

    /// A tower draws one plant per crop (one per net cup), all procedural.
    #[test]
    fn plant_pass_tower_draws_a_plant_per_cup() {
        let got = build_group(&tower(12), &visuals(), &mut |_| None);
        assert_eq!((got.drawn, got.meant), (12, 12));
        assert!(got.models.is_empty());
        assert!(!got.procedural.vertices.is_empty());
        assert_eq!(got.procedural.indices.len() % 3, 0);
        assert!(got.procedural.indices.iter().all(|&i| (i as usize) < got.procedural.vertices.len()));
    }

    /// A TOWER'S NET CUPS HOLD THE REAL STAGE MODEL (2026-09-29), scaled
    /// uniformly to the dwarf height (a 1 m model comes out 0.6 m), set
    /// out from the column and tipped out like its cup (2026-10-03); past
    /// the tower's vertex budget the rest are procedural, every cup still
    /// drawn. Red checks, run: skipping the scale (k = 1) fails the height
    /// assertion; passing the bare yaw (no tip) fails the lean assertion.
    #[test]
    fn plant_pass_tower_cups_hold_scaled_stage_models_within_budget() {
        let job = tower(12);
        let got = build_group(&job, &visuals(), &mut |name| (name == "kale_2").then(tiny_model));
        assert_eq!((got.drawn, got.meant), (12, 12));
        assert_eq!(got.models.len(), 1);
        let verts = &got.models[0].1.vertices;
        assert_eq!(verts.len(), 12 * 3, "one baked copy per cup");
        assert!(got.procedural.vertices.is_empty(), "no procedural plant while the budget holds");
        for copy in verts.chunks(3) {
            let (lo, hi) = copy.iter().fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v.position[1]), b.max(v.position[1])));
            assert!(hi - lo <= TOWER_PLANT_MAX_M + 1e-4, "a copy is {} m tall", hi - lo);
            let r = ((copy[0].position[0] - 1.0).powi(2) + (copy[0].position[2] - 2.0).powi(2)).sqrt();
            assert!(r >= 0.15 - 1e-4, "the plant stands in its cup, not inside the column: r {r}");
            // Tipped out like a net cup (2026-10-03): the model's top (its
            // third vertex, 1 m up as authored, 0.6 m once scaled) leans
            // away from the column by the cup's tilt, about its base.
            let base = Vec3::from(copy[0].position);
            let out = Vec3::new(base.x - 1.0, 0.0, base.z - 2.0).normalize();
            let rise = Vec3::from(copy[2].position) - base;
            let (s, c) = NET_CUP_TILT_RAD.sin_cos();
            assert!((rise.dot(out) - TOWER_PLANT_MAX_M * s).abs() < 1e-4, "the plant leans out {} m", rise.dot(out));
            assert!((rise.y - TOWER_PLANT_MAX_M * c).abs() < 1e-4, "the plant rises {} m", rise.y);
        }
        // A budget of five copies: five models, seven procedural plants.
        let small = PlantVisualRegistry { plot_vertex_budget: 15, ..visuals() };
        let got = build_group(&job, &small, &mut |name| (name == "kale_2").then(tiny_model));
        assert_eq!(got.models[0].1.vertices.len(), 5 * 3);
        assert!(!got.procedural.vertices.is_empty());
        assert_eq!(got.drawn, 12);
        // Wider than the species' spread: narrowed to fit, never enlarged.
        assert!((tower_model_scale(0.3, 2.0, 0.5) - 0.25).abs() < 1e-6);
        assert_eq!(tower_model_scale(0.3, 0.2, 0.5), 1.0);
        // A root crop's model, reaching half its height under the origin,
        // stays procedural in a tower.
        let root = Arc::new(GltfCpuMesh { vertices: tiny_model().vertices.iter().map(|v| Vertex { position: [v.position[0], v.position[1] - 0.5, v.position[2]], ..*v }).collect(), indices: vec![0, 1, 2] });
        let got = build_group(&job, &visuals(), &mut |name| (name == "kale_2").then(|| root.clone()));
        assert!(got.models.is_empty() && got.drawn == 12);
    }

    /// A bed plot draws every plant it holds; a species with a stage model
    /// bakes that model once per plant into its own merged mesh, named for
    /// the model at the growth quartile ("wheat_4" when ripe), and a species
    /// without one draws from its recipe.
    #[test]
    fn plant_pass_plots_draw_what_they_hold_and_bake_stage_models() {
        let mut asked = Vec::new();
        let got = build_group(&bed("wheat"), &visuals(), &mut |name| {
            asked.push(name.to_string());
            (name == "wheat_4").then(tiny_model)
        });
        assert_eq!(asked, vec!["potato_2".to_string(), "wheat_4".to_string()]);
        assert_eq!((got.drawn, got.meant), (4 + 6, 4 + 6));
        assert_eq!(got.models.len(), 1);
        assert_eq!(got.models[0].0, "wheat_4");
        assert_eq!(got.models[0].1.vertices.len(), 6 * 3, "six baked copies of the three-vertex model");
        assert!(!got.procedural.vertices.is_empty(), "the potato plot is drawn from its recipe");
        // Every baked plant stands on the bed's top, inside its footprint
        // (2 x 1 m turned 90 degrees about x 4, z 6: x 3.5..4.5, z 5..7).
        for v in &got.models[0].1.vertices {
            let [x, y, z] = v.position;
            assert!((3.4..=4.6).contains(&x) && (4.9..=7.1).contains(&z), "outside the bed: {x} {z}");
            assert!(y >= 0.4 - 1e-4, "below the bed top: {y}");
        }
    }

    /// A mushroom rack's crops are drawn into the rigid mesh (2026-09-27):
    /// no procedural plant, no stage model asked for, one unit per block or
    /// square foot of bed its shelf holds, and all of it on the rack's shelves
    /// inside its tent (1.3 x 0.7 m round the rack at x 2, z 53).
    ///
    /// Seen red on 2026-09-27 by skipping the fungus branch (`.filter(|_|
    /// false)` on the lookup): the crops asked for the "oyster_mushroom_2",
    /// "oyster_mushroom_4", "shiitake_4" and "button_mushroom_4" stage models,
    /// the way they did before `fungus_mesh`.
    #[test]
    fn plant_pass_mushroom_crops_draw_rigid_substrate_and_ask_for_no_model() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(root.join("data/plants_visual.ron")).expect("plants_visual.ron");
        let vis = PlantVisualRegistry::from_ron(&text).expect("plants_visual.ron parses");
        let rack = GroupJob {
            key: "mush_0".into(),
            sig: 5,
            layout: GroupLayout::Plots {
                pos: Vec3::new(2.0, 0.0, 53.0),
                yaw_deg: 0.0,
                rects: plot_rects((1.2, 1.8, 0.6), 5, true),
            },
            crops: vec![
                crop("oyster_mushroom", 0, 0.4, 2),
                crop("oyster_mushroom", 1, 1.0, 2),
                crop("shiitake", 2, 1.0, 2),
                crop("button_mushroom", 3, 1.0, 7),
            ],
        };
        let mut asked = Vec::new();
        let got = build_group(&rack, &vis, &mut |name| {
            asked.push(name.to_string());
            None
        });
        assert!(asked.is_empty(), "a mushroom crop asked for stage models: {asked:?}");
        assert!(got.procedural.vertices.is_empty(), "a mushroom crop drew a plant recipe");
        assert!(!got.rigid.vertices.is_empty(), "nothing drawn into the rigid mesh");
        assert_eq!((got.drawn, got.meant), (2 + 2 + 2 + 7, 2 + 2 + 2 + 7));
        assert_eq!(got.rigid.indices.len() % 3, 0);
        assert!(got.rigid.indices.iter().all(|&i| (i as usize) < got.rigid.vertices.len()));
        let shelves = plot_rects((1.2, 1.8, 0.6), 5, true);
        let (bottom, top) = (shelves[0].floor, shelves[4].floor);
        for v in &got.rigid.vertices {
            let [x, y, z] = v.position;
            assert!((1.35..=2.65).contains(&x) && (52.65..=53.35).contains(&z), "outside the tent: {x} {z}");
            assert!((bottom - 1e-4..=top).contains(&y), "off the shelves the crops stand on: {y}");
        }
    }

    /// The same job builds the same geometry, bit for bit, and a change that
    /// moves a plant changes the checksum.
    #[test]
    fn plant_pass_build_is_deterministic() {
        let lookup = |name: &str| (name == "wheat_4").then(tiny_model);
        let a = build_group(&bed("wheat"), &visuals(), &mut { lookup });
        let b = build_group(&bed("wheat"), &visuals(), &mut { lookup });
        assert_eq!(checksum_of(&a), checksum_of(&b));
        let mut moved = bed("wheat");
        moved.crops[0].slot = 1;
        let c = build_group(&moved, &visuals(), &mut { lookup });
        assert_ne!(checksum_of(&a), checksum_of(&c));
    }

    /// The worker sends every machine and then Done; a stage model that is
    /// not on disk is reported missing once and its plants fall back to the
    /// recipe; a job cancelled before it starts sends no machine.
    #[test]
    fn plant_pass_worker_sends_every_machine_then_done() {
        let empty = std::env::temp_dir().join(format!("plant_pass_test_{}", std::process::id()));
        let job = |generation: u64, cancelled: bool| Job {
            generation,
            groups: vec![tower(3), bed("wheat")],
            visuals: Arc::new(visuals()),
            models: HashMap::new(),
            missing: HashSet::new(),
            data_dir: empty.clone(),
            cancel: Arc::new(AtomicBool::new(cancelled)),
            checksum: true,
        };
        let (tx, rx) = mpsc::channel();
        spawn(job(1, false), tx.clone());
        let mut groups = Vec::new();
        let mut missing = Vec::new();
        loop {
            match rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the worker answers") {
                WorkerMsg::Group(g) => groups.push(g),
                WorkerMsg::Missing(name) => missing.push(name),
                WorkerMsg::Model { name, .. } => panic!("no model exists here, got {name}"),
                WorkerMsg::Done { generation, .. } => {
                    assert_eq!(generation, 1);
                    break;
                }
            }
        }
        let keys: Vec<&str> = groups.iter().map(|g| g.key.as_str()).collect();
        assert_eq!(keys, vec!["ntower_0", "potato_bed_0"]);
        assert!(groups.iter().all(|g| g.checksum.is_some()));
        missing.sort();
        // The tower asks for its crop's stage model too (2026-09-29).
        assert_eq!(missing, vec!["kale_2".to_string(), "potato_2".to_string(), "wheat_4".to_string()], "each asked for once");
        assert!(groups[1].models.is_empty(), "no model on disk: every plant from its recipe");

        spawn(job(2, true), tx);
        match rx.recv_timeout(std::time::Duration::from_secs(20)).expect("the worker answers") {
            WorkerMsg::Done { generation, .. } => assert_eq!(generation, 2),
            _ => panic!("a cancelled job built a machine"),
        }
    }
}
