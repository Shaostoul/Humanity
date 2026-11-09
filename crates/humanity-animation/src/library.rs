//! The data files that make a character, read and tied together (plan section 9):
//!
//! - `data/characters/characters.ron`: which body and graph the game uses, the clips' bake rate,
//!   the drawing tiers, and each kind of wearer's default clothes (a player's, a crew member's);
//! - `data/characters/skeletons/<id>.ron`: the humanoid standard (`humanoid`);
//! - `data/characters/bodies/<id>.ron`: a body: its glTF file, its rig's role map, its height, and
//!   which tint each of its materials takes (skin, hair, a cosmetic slot);
//! - `data/characters/role_maps/<id>.ron`: one rig's bone names as roles (`role_map`);
//! - `data/characters/clips.ron`: each clip's file, animation name, rig, loop flag, gait speed and
//!   credit;
//! - `data/characters/locomotion.ron`: the state machine (`graph`).
//!
//! Model files are read through a function the engine passes in (its glTF reader), so this crate
//! reads no file format itself. Everything is built in memory; nothing is written.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

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
    /// The cosmetic equipped in this slot (`data/cosmetics/cosmetics.csv`'s slot names), or the
    /// wearer kind's default for the slot when nothing is.
    Slot(String),
}

/// A slot's colour when nothing is equipped in it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum Fallback {
    Skin,
    Hair,
    /// Linear RGB, 0 to 1.
    Rgb(f32, f32, f32),
}

/// One kind of wearer's defaults (a player, a crew member).
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Wearer {
    /// Skin and hair when the wearer has no look of their own (crew today).
    pub skin: [f32; 3],
    pub hair: [f32; 3],
    /// Each slot's colour when nothing is equipped in it.
    pub clothes: BTreeMap<String, Fallback>,
}

/// `data/characters/characters.ron`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct CharactersDef {
    /// The body every humanoid is drawn with: a file in `data/characters/bodies/`.
    pub body: String,
    /// The locomotion graph's id (the `id` inside `data/characters/locomotion.ron`).
    pub locomotion: String,
    /// Frames a second clips are baked at.
    pub clip_fps: f32,
    pub tiers: Tiers,
    /// Default clothes by wearer kind ("player", "crew").
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
    /// Standing height at look height 1.0, metres (a look's height multiplies it).
    pub height_m: f32,
    /// The credit id in `data/credits.ron`, for a body that needs one (none for our own).
    #[serde(default)]
    pub credit: Option<String>,
    pub regions: Vec<RegionDef>,
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

/// Everything one body needs to be drawn and animated.
#[derive(Debug, Clone)]
pub struct CharacterLibrary {
    pub characters: CharactersDef,
    pub humanoid: HumanoidDef,
    pub body: BodyDef,
    pub skeleton: Skeleton,
    /// The body's mesh as read, for the GPU upload.
    pub model: SkinnedModelData,
    pub clips: ClipSet,
    pub graph: Graph,
    /// Each primitive's region (an index into `body.regions`), in the model's primitive order.
    pub primitive_regions: Vec<usize>,
}

impl CharacterLibrary {
    /// Read and tie together every file, retargeting each clip onto the body. `read_model` reads a
    /// skinned glTF by its data-relative path.
    pub fn load(data_dir: &Path, read_model: &mut dyn FnMut(&str) -> Result<SkinnedModelData, String>) -> Result<Self, String> {
        let dir = data_dir.join("characters");
        let read = |rel: &str| -> Result<String, String> {
            let p = dir.join(rel);
            std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))
        };
        let characters: CharactersDef = crate::parse_ron(&read("characters.ron")?, "data/characters/characters.ron")?;
        let body: BodyDef = crate::parse_ron(&read(&format!("bodies/{}.ron", characters.body))?, "body")?;
        let humanoid = HumanoidDef::parse(&read(&format!("skeletons/{}.ron", body.skeleton))?)?;
        let mut role_maps: HashMap<String, RoleMap> = HashMap::new();
        let map = |id: &str, maps: &mut HashMap<String, RoleMap>| -> Result<RoleMap, String> {
            if let Some(m) = maps.get(id) {
                return Ok(m.clone());
            }
            let m = RoleMap::parse(&read(&format!("role_maps/{id}.ron"))?, &humanoid)?;
            maps.insert(id.to_string(), m.clone());
            Ok(m)
        };
        let body_map = map(&body.role_map, &mut role_maps)?;
        let model = read_model(&body.model)?;
        let skeleton = Skeleton::from_skin(&model.joints, &body_map, &humanoid)?;
        let primitive_regions = model
            .primitives
            .iter()
            .map(|p| {
                body.regions
                    .iter()
                    .position(|r| r.material == p.material)
                    .ok_or_else(|| format!("body {}: material {} has no region in data/characters/bodies/{}.ron", body.id, p.material, body.id))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let clip_defs: Vec<ClipDef> = crate::parse_ron(&read("clips.ron")?, "data/characters/clips.ron")?;
        let mut files: HashMap<String, SkinnedModelData> = HashMap::new();
        let mut rigs: HashMap<(String, String), Skeleton> = HashMap::new();
        let mut clips = ClipSet::default();
        for def in &clip_defs {
            if clips.index(&def.id).is_some() {
                return Err(format!("data/characters/clips.ron: clip {} is listed twice", def.id));
            }
            if !files.contains_key(&def.file) {
                let m = read_model(&def.file)?;
                files.insert(def.file.clone(), m);
            }
            let file = &files[&def.file];
            let key = (def.file.clone(), def.role_map.clone());
            if !rigs.contains_key(&key) {
                let rm = map(&def.role_map, &mut role_maps)?;
                rigs.insert(key.clone(), Skeleton::from_skin(&file.joints, &rm, &humanoid).map_err(|e| format!("clip {}: {e}", def.id))?);
            }
            let rig = &rigs[&key];
            let data = file
                .clips
                .iter()
                .find(|c| c.name == def.animation)
                .ok_or_else(|| format!("clip {}: {} has no animation named {}", def.id, def.file, def.animation))?;
            let source = SourceClip::from_data(data, rig).map_err(|e| format!("clip {}: {e}", def.id))?;
            clips.insert(&def.id, retarget(&source, rig, &skeleton, &humanoid, characters.clip_fps, def.looped, def.speed_mps));
        }
        let graph_def = Graph::parse(&read("locomotion.ron")?)?;
        if graph_def.id != characters.locomotion {
            return Err(format!(
                "data/characters/characters.ron names locomotion graph {}, but locomotion.ron holds {}",
                characters.locomotion, graph_def.id
            ));
        }
        let graph = Graph::resolve(&graph_def, &clips)?;
        Ok(CharacterLibrary { characters, humanoid, body, skeleton, model, clips, graph, primitive_regions })
    }

    /// The scale that stands the body at a look's height (1.0 = the body's own `height_m`).
    pub fn scale_for(&self, look_height: f32) -> f32 {
        if look_height.is_finite() && look_height > 0.0 {
            look_height
        } else {
            1.0
        }
    }

    /// A wearer kind's defaults; the first listed if the kind is unknown.
    pub fn wearer(&self, kind: &str) -> Option<&Wearer> {
        self.characters.wearers.get(kind).or_else(|| self.characters.wearers.values().next())
    }
}
