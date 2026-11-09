//! The data files that make a character, read and tied together (plan section 9):
//!
//! - `data/characters/characters.ron`: the bodies a look can choose (a female and a male body since
//!   2026-11-09) and the stand-in drawn for one that does not load, the graph, the clips' bake rate,
//!   the drawing tiers, and each kind of wearer's defaults (which bodies, skin, hair, eyes, clothes);
//! - `data/characters/skeletons/<id>.ron`: the humanoid standard (`humanoid`);
//! - `data/characters/bodies/<id>.ron`: a body: its glTF file, its rig's role map, which tint each of
//!   its materials takes (skin, hair, eyes, a cosmetic slot, a fixed colour), and which of its shape
//!   targets the look drives;
//! - `data/characters/role_maps/<id>.ron`: one rig's bone names as roles (`role_map`);
//! - `data/characters/clips.ron`: each clip's file, animation name, rig, loop flag, gait speed and
//!   credit;
//! - `data/characters/locomotion.ron`: the state machine (`graph`).
//!
//! Model files are read through a function the engine passes in (its glTF reader), so this crate
//! reads no file format itself. Everything is built in memory; nothing is written. Every clip is
//! retargeted onto every body (their skeletons differ: a female and a male body stand their joints
//! in different places).

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use glam::Vec3;
use humanity_core::skin_data::SkinnedModelData;
use serde::Deserialize;

use crate::clip::{ClipSet, SourceClip};
use crate::graph::Graph;
use crate::retarget::retarget;
use crate::{HumanoidDef, RoleMap, Skeleton, Tiers};

/// What a body material is tinted with.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Tint {
    /// The wearer's skin tone (their look's, or the wearer kind's default).
    Skin,
    /// Their hair colour.
    Hair,
    /// Their eye colour (the iris).
    Eyes,
    /// The cosmetic equipped in this slot (`data/cosmetics/cosmetics.csv`'s slot names), or the
    /// wearer kind's default for the slot when nothing is.
    Slot(String),
    /// Always this colour, linear RGB 0 to 1 (the white of the eye, the pupil).
    Fixed(f32, f32, f32),
}

/// A slot's colour when nothing is equipped in it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Fallback {
    Skin,
    Hair,
    /// Linear RGB, 0 to 1.
    Rgb(f32, f32, f32),
}

fn default_eyes() -> [f32; 3] {
    [0.30, 0.45, 0.55]
}

/// One kind of wearer's defaults (a player, a crew member).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Wearer {
    /// The bodies (choice ids of `CharactersDef::bodies`) a wearer of this kind is drawn with when
    /// their look names none: one of them, picked by a stable hash of who they are, so a crew is
    /// women and men and each person keeps theirs.
    #[serde(default)]
    pub bodies: Vec<String>,
    /// Skin, hair and eyes when the wearer has no look of their own (crew today).
    pub skin: [f32; 3],
    pub hair: [f32; 3],
    #[serde(default = "default_eyes")]
    pub eyes: [f32; 3],
    /// Each slot's colour when nothing is equipped in it.
    pub clothes: BTreeMap<String, Fallback>,
}

/// `data/characters/characters.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CharactersDef {
    /// The bodies a look can choose, by the id the look names (`Appearance::body_type`: "female",
    /// "male"), each a file in `data/characters/bodies/`.
    pub bodies: BTreeMap<String, String>,
    /// The body drawn in place of one that does not load (a file in `bodies/`), so a broken body
    /// shows a person, not nothing. Always read; a look may name it by its file id too.
    pub fallback: String,
    /// The locomotion graph's id (the `id` inside `data/characters/locomotion.ron`).
    pub locomotion: String,
    /// Frames a second clips are baked at.
    pub clip_fps: f32,
    pub tiers: Tiers,
    /// Defaults by wearer kind ("player", "crew").
    pub wearers: BTreeMap<String, Wearer>,
}

/// A body material and its tint.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RegionDef {
    /// The glTF material name of the primitives this region covers.
    pub material: String,
    pub tint: Tint,
    /// How rough the surface reads (0 polished to 1 matte).
    pub roughness: f32,
}

/// How the look's height drives two of a body's shape targets.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HeightDriver {
    /// The target that makes the body shorter, toward its full weight.
    pub below: String,
    /// The target that makes it taller.
    pub above: String,
}

/// Which of a body's shape targets the look drives (data, not code: a new slider is a new driver).
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ShapeDef {
    /// The look's height (1.0 = the body's own) through the body's height targets, so a tall person
    /// is long-legged rather than scaled up head and all. Past what the targets reach, the rest is a
    /// uniform scale.
    #[serde(default)]
    pub height: Option<HeightDriver>,
}

/// `data/characters/bodies/<id>.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BodyDef {
    pub id: String,
    #[serde(default)]
    pub description: String,
    /// The skinned glTF, relative to the data folder or the repo root.
    pub model: String,
    pub role_map: String,
    /// The humanoid standard's file in `data/characters/skeletons/`.
    pub skeleton: String,
    /// Standing height at look height 1.0, metres, as written by whoever made the body; the loader
    /// measures the mesh and uses that, so this is a note for readers and is checked against it.
    pub height_m: f32,
    /// The credit id in `data/credits.ron`, for a body that needs one (none for our own).
    #[serde(default)]
    pub credit: Option<String>,
    pub regions: Vec<RegionDef>,
    #[serde(default)]
    pub shape: Option<ShapeDef>,
}

/// One row of `data/characters/clips.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ClipDef {
    pub id: String,
    pub file: String,
    /// The animation's name inside the file.
    pub animation: String,
    /// The rig the clip was made on.
    pub role_map: String,
    #[serde(default)]
    pub looped: bool,
    /// Metres a second the gait covers at the clip's own speed (0 for clips that stand still).
    #[serde(default)]
    pub speed_mps: f32,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub credit: Option<String>,
}

/// The height driver resolved against a body's mesh.
#[derive(Debug, Clone, PartialEq)]
struct HeightShape {
    below: usize,
    above: usize,
    /// The body's height at rest and with each target at full weight, metres.
    own_m: f32,
    below_m: f32,
    above_m: f32,
}

/// One body, read and ready to draw.
#[derive(Debug, Clone)]
pub struct LoadedBody {
    /// The id a look chooses it by (characters.ron's `bodies` key, or the fallback's file id).
    pub choice: String,
    pub def: BodyDef,
    pub skeleton: Skeleton,
    /// The body's mesh as read, for the GPU upload.
    pub model: SkinnedModelData,
    /// Every clip, retargeted onto this body.
    pub clips: ClipSet,
    /// Each primitive's region (an index into `def.regions`), in the model's primitive order.
    pub primitive_regions: Vec<usize>,
    /// Read in place of a body that did not load (the fallback standing in).
    pub stand_in: bool,
    /// The body's standing height measured from its mesh, metres.
    pub height_m: f32,
    height: Option<HeightShape>,
    /// Each target's movement of each joint (model space), from the file.
    joint_moves: Vec<Vec<Vec3>>,
}

impl LoadedBody {
    /// The shape targets (index, weight) and the uniform scale that stand this body at a look's
    /// height (1.0 = its own): the height targets as far as they reach, the rest by scale. A body
    /// without a height driver is scaled.
    pub fn shape_for(&self, look_height: f32) -> (Vec<(usize, f32)>, f32) {
        let look = if look_height.is_finite() && look_height > 0.0 { look_height } else { 1.0 };
        let Some(h) = &self.height else { return (Vec::new(), look) };
        let want = h.own_m * look;
        let (target, reach) = if want >= h.own_m { (h.above, h.above_m) } else { (h.below, h.below_m) };
        let span = reach - h.own_m;
        let w = if span.abs() > 1e-4 { ((want - h.own_m) / span).clamp(0.0, 1.0) } else { 0.0 };
        let reached = h.own_m + span * w;
        let scale = if reached > 1e-3 { want / reached } else { look };
        let weights = if w > 1e-4 { vec![(target, w)] } else { Vec::new() };
        (weights, scale)
    }

    /// The skeleton reshaped for these shape weights; None when they leave it as it is (no weight,
    /// or a file that does not say how its targets move the joints).
    pub fn reshaped(&self, weights: &[(usize, f32)]) -> Option<Skeleton> {
        if weights.is_empty() || self.joint_moves.is_empty() {
            return None;
        }
        let mut moves = vec![Vec3::ZERO; self.skeleton.joint_count()];
        for &(t, w) in weights {
            if let Some(d) = self.joint_moves.get(t) {
                for (m, v) in moves.iter_mut().zip(d) {
                    *m += *v * w;
                }
            }
        }
        Some(self.skeleton.reshaped(&moves))
    }

    /// A morph target's index by name.
    pub fn target(&self, name: &str) -> Option<usize> {
        self.model.morph_index(name)
    }
}

/// Everything the game's bodies need to be drawn and animated.
#[derive(Debug, Clone)]
pub struct CharacterLibrary {
    pub characters: CharactersDef,
    pub humanoid: HumanoidDef,
    /// The chosen bodies in the order of `characters.bodies` (sorted by choice id), then the
    /// fallback.
    pub bodies: Vec<LoadedBody>,
    pub graph: Graph,
    /// Bodies that did not load and had the fallback drawn in their place, with why.
    pub warnings: Vec<String>,
}

impl CharacterLibrary {
    /// Read and tie together every file, retargeting each clip onto each body. `read_model` reads a
    /// skinned glTF by its data-relative path.
    pub fn load(data_dir: &Path, read_model: &mut dyn FnMut(&str) -> Result<SkinnedModelData, String>) -> Result<Self, String> {
        let dir = data_dir.join("characters");
        let read = |rel: &str| -> Result<String, String> {
            let p = dir.join(rel);
            std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
        };
        let characters: CharactersDef = crate::parse_ron(&read("characters.ron")?, "data/characters/characters.ron")?;
        if characters.bodies.is_empty() {
            return Err("data/characters/characters.ron lists no bodies".into());
        }
        let mut role_maps: HashMap<String, RoleMap> = HashMap::new();
        let mut humanoids: HashMap<String, HumanoidDef> = HashMap::new();
        let clip_defs: Vec<ClipDef> = crate::parse_ron(&read("clips.ron")?, "data/characters/clips.ron")?;
        let mut files: HashMap<String, SkinnedModelData> = HashMap::new();
        let mut sources: HashMap<String, (SourceClip, Skeleton)> = HashMap::new();

        // One body: its file, its skeleton, its regions, its shape, and every clip retargeted.
        let mut load_body = |choice: &str, file_id: &str, read_model: &mut dyn FnMut(&str) -> Result<SkinnedModelData, String>| -> Result<(LoadedBody, HumanoidDef), String> {
            let def: BodyDef = crate::parse_ron(&read(&format!("bodies/{file_id}.ron"))?, &format!("data/characters/bodies/{file_id}.ron"))?;
            if !humanoids.contains_key(&def.skeleton) {
                humanoids.insert(def.skeleton.clone(), HumanoidDef::parse(&read(&format!("skeletons/{}.ron", def.skeleton))?)?);
            }
            let humanoid = humanoids[&def.skeleton].clone();
            let mut map = |id: &str| -> Result<RoleMap, String> {
                if let Some(m) = role_maps.get(id) {
                    return Ok(m.clone());
                }
                let m = RoleMap::parse(&read(&format!("role_maps/{id}.ron"))?, &humanoid)?;
                role_maps.insert(id.to_string(), m.clone());
                Ok(m)
            };
            let body_map = map(&def.role_map)?;
            let model = read_model(&def.model)?;
            let skeleton = Skeleton::from_skin(&model.joints, &body_map, &humanoid).map_err(|e| format!("body {}: {e}", def.id))?;
            let primitive_regions = model
                .primitives
                .iter()
                .map(|p| {
                    def.regions
                        .iter()
                        .position(|r| r.material == p.material)
                        .ok_or_else(|| format!("body {}: material {} has no region in data/characters/bodies/{}.ron", def.id, p.material, def.id))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let height_m = measured_height(&model, None);
            let height = match def.shape.as_ref().and_then(|s| s.height.as_ref()) {
                None => None,
                Some(d) => {
                    let idx = |n: &str| model.morph_index(n).ok_or_else(|| format!("body {}: its height driver names target {n}, which {} does not have", def.id, def.model));
                    let (below, above) = (idx(&d.below)?, idx(&d.above)?);
                    Some(HeightShape { below, above, own_m: height_m, below_m: measured_height(&model, Some(below)), above_m: measured_height(&model, Some(above)) })
                }
            };
            let joint_moves: Vec<Vec<Vec3>> = model.joint_deltas.iter().map(|d| d.iter().map(|v| Vec3::from(*v)).collect()).collect();
            let mut clips = ClipSet::default();
            for cd in &clip_defs {
                if clips.index(&cd.id).is_some() {
                    return Err(format!("data/characters/clips.ron: clip {} is listed twice", cd.id));
                }
                let key = format!("{}|{}|{}", cd.file, cd.role_map, cd.animation);
                if !sources.contains_key(&key) {
                    if !files.contains_key(&cd.file) {
                        let m = read_model(&cd.file)?;
                        files.insert(cd.file.clone(), m);
                    }
                    let file = &files[&cd.file];
                    let rm = map(&cd.role_map)?;
                    let rig = Skeleton::from_skin(&file.joints, &rm, &humanoid).map_err(|e| format!("clip {}: {e}", cd.id))?;
                    let data = file
                        .clips
                        .iter()
                        .find(|c| c.name == cd.animation)
                        .ok_or_else(|| format!("clip {}: {} has no animation named {}", cd.id, cd.file, cd.animation))?;
                    let source = SourceClip::from_data(data, &rig).map_err(|e| format!("clip {}: {e}", cd.id))?;
                    sources.insert(key.clone(), (source, rig));
                }
                let (source, rig) = &sources[&key];
                clips.insert(&cd.id, retarget(source, rig, &skeleton, &humanoid, characters.clip_fps, cd.looped, cd.speed_mps));
            }
            let body = LoadedBody {
                choice: choice.to_string(),
                def,
                skeleton,
                model,
                clips,
                primitive_regions,
                stand_in: false,
                height_m,
                height,
                joint_moves,
            };
            Ok((body, humanoid))
        };

        let mut bodies = Vec::new();
        let mut warnings = Vec::new();
        let mut humanoid = None;
        let (fallback, fallback_humanoid) = load_body(&characters.fallback, &characters.fallback, read_model)
            .map_err(|e| format!("the fallback body {} did not load: {e}", characters.fallback))?;
        for (choice, file_id) in &characters.bodies {
            match load_body(choice, file_id, read_model) {
                Ok((b, h)) => {
                    humanoid.get_or_insert(h);
                    bodies.push(b);
                }
                Err(e) => {
                    warnings.push(format!("body {choice} ({file_id}) did not load, so {} stands in: {e}", characters.fallback));
                    let mut stand_in = fallback.clone();
                    stand_in.choice = choice.clone();
                    stand_in.stand_in = true;
                    bodies.push(stand_in);
                }
            }
        }
        bodies.push(fallback);
        let humanoid = humanoid.unwrap_or(fallback_humanoid);

        let graph_def = Graph::parse(&read("locomotion.ron")?)?;
        if graph_def.id != characters.locomotion {
            return Err(format!(
                "data/characters/characters.ron names locomotion graph {}, but locomotion.ron holds {}",
                characters.locomotion, graph_def.id
            ));
        }
        // Every body's clips were inserted in clips.ron's order, so one graph resolves for all.
        let graph = Graph::resolve(&graph_def, &bodies[0].clips)?;
        for b in &bodies[1..] {
            if b.clips.by_id != bodies[0].clips.by_id {
                return Err(format!("body {}'s clips are not in the same order as {}'s", b.def.id, bodies[0].def.id));
            }
        }
        for (choice, w) in &characters.wearers {
            for b in &w.bodies {
                if !characters.bodies.contains_key(b) {
                    return Err(format!("data/characters/characters.ron: wearer {choice} names body {b}, which is not one of its bodies"));
                }
            }
        }
        Ok(CharacterLibrary { characters, humanoid, bodies, graph, warnings })
    }

    /// A body by the id a look chooses it with.
    pub fn body_index(&self, choice: &str) -> Option<usize> {
        self.bodies.iter().position(|b| b.choice == choice)
    }

    /// Which body draws a person: the one their look names when it is one of the bodies, else one
    /// of their wearer kind's bodies picked by `hash` (0 to 1, stable per person), else the first.
    pub fn body_for(&self, look_body: Option<&str>, wearer: &str, hash: f32) -> usize {
        if let Some(i) = look_body.and_then(|b| self.body_index(b)) {
            return i;
        }
        let choices = self.wearer(wearer).map(|w| w.bodies.as_slice()).unwrap_or(&[]);
        if !choices.is_empty() {
            let k = ((hash.clamp(0.0, 0.999_999) * choices.len() as f32) as usize).min(choices.len() - 1);
            if let Some(i) = self.body_index(&choices[k]) {
                return i;
            }
        }
        0
    }

    /// A wearer kind's defaults; the first listed if the kind is unknown.
    pub fn wearer(&self, kind: &str) -> Option<&crate::library::Wearer> {
        self.characters.wearers.get(kind).or_else(|| self.characters.wearers.values().next())
    }
}

/// A body's standing height, metres: the highest vertex, with one target at full weight when named.
fn measured_height(model: &SkinnedModelData, target: Option<usize>) -> f32 {
    let mut top = f32::MIN;
    for p in &model.primitives {
        for (i, v) in p.vertices.iter().enumerate() {
            let dy = target.and_then(|t| p.morphs.get(t)).map_or(0.0, |m| m.positions[i][1]);
            top = top.max(v.position[1] + dy);
        }
    }
    if top.is_finite() {
        top
    } else {
        0.0
    }
}
