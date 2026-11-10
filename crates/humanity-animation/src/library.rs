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
// The page's view of the sliders, and how a blend reads, live in the core (the character page uses
// them without this crate).
use humanity_core::player_look::{blend_shares, SliderBlock, SHAPE_MIDDLE};
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
    /// How varied the bodies of this kind are when they have no look of their own (crew, the dev
    /// crowd; 2026-11-09): each named slider's range (0 to 1), a value picked in it by a stable hash
    /// of who they are, so each keeps theirs. A blend's parts are drawn this way and then made to add
    /// up to one. A slider not named takes its middle.
    #[serde(default)]
    pub shape: BTreeMap<String, (f32, f32)>,
    /// The look height range (1.0 = the body's own) picked the same way; None keeps 1.0.
    #[serde(default)]
    pub height: Option<(f32, f32)>,
    /// What a wearer of this kind wears when their look does not say (2026-11-09, "the look"), by
    /// body choice id ("female", "male"): each list's item picked by a stable hash of who they are.
    #[serde(default)]
    pub dress: BTreeMap<String, DressDef>,
}

/// A wearer kind's default dress on one body: garment ids for the hair, brows and lashes
/// (`data/characters/garments.ron`), and cosmetic ids per outfit slot
/// (`data/cosmetics/cosmetics.csv`, whose `model` names the garment drawn). An empty list wears
/// nothing there.
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct DressDef {
    #[serde(default)]
    pub hair: Vec<String>,
    #[serde(default)]
    pub brows: Vec<String>,
    #[serde(default)]
    pub lashes: Vec<String>,
    #[serde(default)]
    pub outfit: BTreeMap<String, Vec<String>>,
}

/// What a person's look says they wear, by id (None: their wearer kind's default; the id
/// `"none"` wears nothing there).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DressChoice {
    pub hair: Option<String>,
    pub brows: Option<String>,
    pub lashes: Option<String>,
    /// Outfit slot to cosmetic id.
    pub outfit: BTreeMap<String, String>,
}

/// The id a choice says wears nothing.
pub const WEAR_NONE: &str = "none";

/// A stable value in 0 to 1 for a person (`seed`) and one of their traits (`what`): FNV-1a over the
/// trait's name from the seed, finished by SplitMix64's mixer, so each trait varies on its own.
pub fn stable_unit(seed: u64, what: &str) -> f32 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for b in what.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h ^= h >> 30;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^= h >> 27;
    h = h.wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^= h >> 31;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

impl Wearer {
    /// A slider's value for a person of this kind with no look of their own, or None when this kind
    /// does not vary it.
    pub fn varied(&self, seed: u64, slider: &str) -> Option<f32> {
        let &(lo, hi) = self.shape.get(slider)?;
        Some((lo + (hi - lo) * stable_unit(seed, slider)).clamp(0.0, 1.0))
    }

    /// The look height of a person of this kind with no look of their own.
    pub fn varied_height(&self, seed: u64) -> f32 {
        self.height.map_or(1.0, |(lo, hi)| lo + (hi - lo) * stable_unit(seed, "height"))
    }
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
    /// The feet's numbers (rung 2a, 2026-11-09: `feet`): how far the pelvis may drop, how fast a
    /// lock takes hold and lets go, how a clip's contacts are labelled. A file without them takes
    /// the shipped values.
    #[serde(default)]
    pub feet: crate::feet::FeetDef,
    /// The look (2026-11-09): the base mesh both MakeHuman bodies share (`makehuman_base.bin`),
    /// beside which `details.ron` and `garments.ron` are read. None draws every body as made, with
    /// its shape on the GPU and its clothes as tinted regions.
    #[serde(default)]
    pub look: Option<String>,
    /// How many people's own meshes may be building at once, each on a worker thread of its own,
    /// off the frame (2026-11-10: a build is 2 to 15 ms, measured, which on the frame was a hitch
    /// each time someone came near). The rest wait their turn, drawn as before meanwhile.
    #[serde(default = "default_look_threads")]
    pub look_threads: usize,
}

fn default_look_threads() -> usize {
    2
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

/// Which of a body's shape targets the look drives (data, not code: a new slider is a new row).
#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
pub struct ShapeDef {
    /// The look's height (1.0 = the body's own) through the body's height targets, so a tall person
    /// is long-legged rather than scaled up head and all. Past what the targets reach, the rest is a
    /// uniform scale. The person's height is what they chose whatever the sliders below do: the
    /// height targets take up any change the other targets make to it (2026-11-09).
    #[serde(default)]
    pub height: Option<HeightDriver>,
    /// The character page's sliders, each a row naming the targets it drives (2026-11-09, the
    /// operator: "Yeah, let's go for it."). A slider's value lives in the look by the slider's id
    /// (`Appearance::shape`, 0 to 1); a look without one takes the slider's middle (or, in a blend,
    /// an even share), which is the body as made.
    #[serde(default)]
    pub sliders: Vec<SliderDef>,
}

/// One row of a body's sliders: which look values drive which of its shape targets.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub enum SliderDef {
    /// One slider with a target at each end and the body as made in the middle: the end it leans
    /// toward, weighted by how far (0 at the middle, 1 at the end).
    Line {
        /// The character page's heading it sits under (`characters.shape_group_<group>`).
        group: String,
        slider: String,
        low: String,
        high: String,
    },
    /// Two sliders that span a square of targets together, the body as made at its centre, a target
    /// at each edge (one slider at its end, the other in the middle) and each corner (both at an
    /// end): MakeHuman's muscle and weight grid. The four targets around the two values are weighted
    /// bilinearly, the way the grid's file names place them.
    Grid {
        group: String,
        x: String,
        y: String,
        x_low: String,
        x_high: String,
        y_low: String,
        y_high: String,
        /// The corners, the x end first: `low_high` is x at its low end and y at its high end.
        low_low: String,
        low_high: String,
        high_low: String,
        high_high: String,
    },
    /// Sliders that always add up to one whole, each with a target that is its part's difference
    /// from the even mix the body was made with (MakeHuman's three ancestries, made a third each by
    /// the converter): each target is weighted by its share less the smallest share, which shapes
    /// the body exactly as shares less an even share would (the parts' targets add up to nothing)
    /// and leaves one target off.
    Blend { group: String, parts: Vec<BlendPart> },
}

/// One part of a blend: the slider's id and the target it drives.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BlendPart {
    pub slider: String,
    pub target: String,
}

impl SliderDef {
    fn group(&self) -> &str {
        match self {
            SliderDef::Line { group, .. } | SliderDef::Grid { group, .. } | SliderDef::Blend { group, .. } => group,
        }
    }
}

/// A slider's value as a look gives it, or None: finite values are clamped to 0 to 1.
fn slider_value(value: &dyn Fn(&str) -> Option<f32>, id: &str) -> Option<f32> {
    value(id).filter(|v| v.is_finite()).map(|v| v.clamp(0.0, 1.0))
}

/// A line or grid slider's weights on its low end, its middle and its high end (they add up to one).
fn tent(v: f32) -> [f32; 3] {
    let d = (v - SHAPE_MIDDLE) * 2.0;
    [(-d).max(0.0), 1.0 - d.abs(), d.max(0.0)]
}

/// A body's sliders resolved against its mesh: target indices in place of names.
#[derive(Debug, Clone, PartialEq)]
enum Driver {
    Line { slider: String, low: usize, high: usize },
    /// `cells[x][y]`, each axis low, middle, high; the centre is the body as made (no target).
    Grid { x: String, y: String, cells: [[Option<usize>; 3]; 3] },
    Blend { parts: Vec<(String, usize)> },
}

impl Driver {
    /// The most targets this row turns on at once.
    fn most_on(&self) -> usize {
        match self {
            Driver::Line { .. } => 1,
            // The four cells around a point inside the square, less the centre's when it has none.
            Driver::Grid { cells, .. } => {
                if cells[1][1].is_some() {
                    4
                } else {
                    3
                }
            }
            Driver::Blend { parts } => parts.len().saturating_sub(1),
        }
    }
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
    /// The body's base file (2026-11-09, "the look": `makehuman_<gender>.base.bin`, written by the
    /// body converter): what a person's own mesh is built from. None draws the body as made.
    #[serde(default)]
    pub base: Option<String>,
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
    /// How much each target, at full weight, raises the top of the body (metres; the African target
    /// stands the female body about 8 cm taller), so the height targets can take it up.
    rises: Vec<f32>,
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
    /// The sliders, resolved (empty for a body with none).
    drivers: Vec<Driver>,
    /// The sliders as the character page shows them, in the file's order.
    pub slider_blocks: Vec<SliderBlock>,
    /// Each target's movement of each joint (model space), from the file.
    joint_moves: Vec<Vec<Vec3>>,
    /// The legs and soles of the unshaped skeleton (rung 2a), None for a body without them. A
    /// person's own come from their reshaped skeleton (`FootRig::new` on it).
    pub feet: Option<crate::feet::FootRig>,
    /// Each foot's vertices (primitive, vertex), left then right: those mostly bound to the foot or
    /// its toes. The foot probe skins them on the CPU to read the drawn sole against the ground.
    pub foot_vertices: [Vec<(usize, usize)>; 2],
    /// Its base file read (the look), when it has one and the library reads the look.
    pub base: Option<humanity_core::look_data::BodyBase>,
    /// Its arms, hands and head (rung 2b: arm IK, grips, look-at), None for a body without arms.
    pub hands: Option<crate::interact::HandRig>,
    /// The vertices (primitive, vertex) the interaction probe and the seat's contact read: those
    /// mostly bound to the hips or the upper legs (what rests on a seat), and each hand's.
    pub seat_vertices: Vec<(usize, usize)>,
    pub hand_vertices: [Vec<(usize, usize)>; 2],
}

impl LoadedBody {
    /// How tall this body stands with these shape weights and scale, metres: the top of its mesh
    /// measured with the targets on (not the linear model `shape_for` stands it by), times the
    /// scale. The character page shows it (rung 2a: "1.62 m", the sliders' lesson that ancestry
    /// moves the top of the head).
    pub fn standing_height_m(&self, weights: &[(usize, f32)], scale: f32) -> f32 {
        let mut top = f32::MIN;
        for p in &self.model.primitives {
            for (i, v) in p.vertices.iter().enumerate() {
                let mut y = v.position[1];
                for &(t, w) in weights {
                    if let Some(m) = p.morphs.get(t) {
                        y += m.positions[i][1] * w;
                    }
                }
                top = top.max(y);
            }
        }
        if top.is_finite() {
            top * scale
        } else {
            0.0
        }
    }

    /// The shape targets (index, weight) and the uniform scale that stand this body at a look's
    /// height (1.0 = its own) with the look's slider values (`value` gives a slider's value by its
    /// id, None for one the look does not set): each slider row's targets, then the height targets
    /// as far as they reach (taking up whatever the other targets did to the height, so the person
    /// stands as tall as they chose), the rest by scale. A body without a height driver is scaled.
    pub fn shape_for(&self, look_height: f32, value: &dyn Fn(&str) -> Option<f32>) -> (Vec<(usize, f32)>, f32) {
        let look = if look_height.is_finite() && look_height > 0.0 { look_height } else { 1.0 };
        let mut weights: Vec<(usize, f32)> = Vec::new();
        let mut add = |t: usize, w: f32| {
            if w.abs() <= 1e-4 {
                return;
            }
            match weights.iter_mut().find(|(i, _)| *i == t) {
                Some(e) => e.1 += w,
                None => weights.push((t, w)),
            }
        };
        for d in &self.drivers {
            match d {
                Driver::Line { slider, low, high } => {
                    let [l, _, h] = tent(slider_value(value, slider).unwrap_or(SHAPE_MIDDLE));
                    add(*low, l);
                    add(*high, h);
                }
                Driver::Grid { x, y, cells } => {
                    let tx = tent(slider_value(value, x).unwrap_or(SHAPE_MIDDLE));
                    let ty = tent(slider_value(value, y).unwrap_or(SHAPE_MIDDLE));
                    for (i, wx) in tx.iter().enumerate() {
                        for (j, wy) in ty.iter().enumerate() {
                            if let Some(t) = cells[i][j] {
                                add(t, wx * wy);
                            }
                        }
                    }
                }
                Driver::Blend { parts } => {
                    let raw: Vec<Option<f32>> = parts.iter().map(|(s, _)| slider_value(value, s)).collect();
                    let shares = blend_shares(&raw);
                    let least = shares.iter().copied().fold(f32::INFINITY, f32::min);
                    for ((_, t), s) in parts.iter().zip(&shares) {
                        add(*t, s - least);
                    }
                }
            }
        }
        weights.retain(|(_, w)| w.abs() > 1e-4);
        let Some(h) = &self.height else { return (weights, look) };
        // How tall the other targets stand the body, and the height targets' share of the rest.
        let natural = h.own_m + weights.iter().map(|&(t, w)| h.rises.get(t).copied().unwrap_or(0.0) * w).sum::<f32>();
        let want = h.own_m * look;
        let (target, reach) = if want >= natural { (h.above, h.above_m) } else { (h.below, h.below_m) };
        let span = reach - h.own_m;
        let w = if span.abs() > 1e-4 { ((want - natural) / span).clamp(0.0, 1.0) } else { 0.0 };
        let reached = natural + span * w;
        let scale = if reached > 1e-3 { want / reached } else { look };
        if w > 1e-4 {
            weights.insert(0, (target, w));
        }
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
    /// The look (detail targets and garments), when `characters.ron` names it and it read.
    pub look: Option<crate::look::LookLibrary>,
    /// The interaction anchors (rung 2b, `data/characters/interactions.ron`) and the grips
    /// (`grips.ron`), checked against the clips.
    pub anchors: crate::interact::Anchors,
    pub grips: crate::interact::Grips,
}

impl CharacterLibrary {
    /// Read and tie together every file, retargeting each clip onto each body. `read_model` reads a
    /// skinned glTF by its data-relative path.
    pub fn load(data_dir: &Path, read_model: &mut dyn FnMut(&str) -> Result<SkinnedModelData, String>, read_bytes: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>) -> Result<Self, String> {
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

        // The look's shared data first: a body's base file is checked against it.
        let mut warnings = Vec::new();
        let look = match &characters.look {
            None => None,
            Some(shared) => match crate::look::LookLibrary::load(data_dir, shared, read_bytes) {
                Ok(l) => Some(l),
                Err(e) => {
                    warnings.push(format!("the look did not load, so every body is drawn as made: {e}"));
                    None
                }
            },
        };
        let look_ref = look.as_ref();
        // One body: its file, its skeleton, its regions, its shape, and every clip retargeted.
        let mut load_body = |choice: &str,
                             file_id: &str,
                             read_model: &mut dyn FnMut(&str) -> Result<SkinnedModelData, String>,
                             read_bytes: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>|
         -> Result<(LoadedBody, HumanoidDef), String> {
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
                    let rises = (0..model.morph_names.len()).map(|t| measured_height(&model, Some(t)) - height_m).collect();
                    Some(HeightShape { below, above, own_m: height_m, below_m: measured_height(&model, Some(below)), above_m: measured_height(&model, Some(above)), rises })
                }
            };
            let sliders = def.shape.as_ref().map_or(&[][..], |s| s.sliders.as_slice());
            let (drivers, slider_blocks) = resolve_sliders(sliders, &model, &def)?;
            let most_on = drivers.iter().map(Driver::most_on).sum::<usize>() + usize::from(height.is_some());
            if most_on > crate::pose::MAX_ACTIVE_MORPHS {
                return Err(format!(
                    "body {}: its sliders and height can turn on {most_on} shape targets at once, more than the {} the pose palette's morph block holds (pose::MORPH_SLOTS)",
                    def.id,
                    crate::pose::MAX_ACTIVE_MORPHS
                ));
            }
            let joint_moves: Vec<Vec<Vec3>> = model.joint_deltas.iter().map(|d| d.iter().map(|v| Vec3::from(*v)).collect()).collect();
            let feet = crate::feet::FootRig::new(&skeleton, &humanoid);
            let foot_vertices = feet.as_ref().map_or_else(Default::default, |rig| foot_vertices(&model, rig));
            // Each foot's reach (heel back, toe tips) from its vertices at rest.
            let feet = feet.map(|rig| {
                let at_rest = foot_vertices.clone().map(|v| v.iter().map(|&(p, k)| Vec3::from(model.primitives[p].vertices[k].position)).collect::<Vec<Vec3>>());
                rig.with_extents(&skeleton, &at_rest)
            });
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
                let mut clip = retarget(source, rig, &skeleton, &humanoid, characters.clip_fps, cd.looped, cd.speed_mps);
                // Which feet are planted in each frame, read from the clip on this body (rung 2a).
                // A clip tagged "air" (the jump's air loop) plants no foot.
                if let Some(f) = &feet {
                    crate::feet::label_contacts(&mut clip, &skeleton, f, &characters.feet, cd.tags.iter().any(|t| t == "air"));
                }
                clips.insert(&cd.id, clip);
            }
            // Its base file, checked against the mesh it maps and the shared base mesh (the look).
            let base = match (&def.base, look_ref) {
                (Some(rel), Some(l)) => {
                    let b = humanity_core::look_data::BodyBase::parse(&read_bytes(rel)?).map_err(|e| format!("body {}: {e}", def.id))?;
                    if b.positions.len() != l.shared.raw.len() || b.follow.len() != model.vertex_count() || b.targets.len() != model.morph_names.len() {
                        return Err(format!("body {}: its base file {rel} does not match its mesh or the shared base mesh", def.id));
                    }
                    Some(b)
                }
                _ => None,
            };
            let hands = crate::interact::HandRig::new(&skeleton, &humanoid);
            let seat_vertices = vertices_bound_to(&model, &seat_joints(&skeleton, &humanoid));
            let hand_vertices = [0, 1].map(|side| vertices_bound_to(&model, &hand_joints(&skeleton, &humanoid, side)));
            // Each hand's palm-side depth, from its vertices at rest relative to the palm's point.
            let hands = hands.map(|h| {
                let rel = [0, 1].map(|side| {
                    let point = skeleton.rest_position(h.arms[side].hand) + h.arms[side].palm_offset;
                    hand_vertices[side].iter().map(|&(p, k)| Vec3::from(model.primitives[p].vertices[k].position) - point).collect::<Vec<Vec3>>()
                });
                h.with_palm_depth([&rel[0], &rel[1]])
            });
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
                drivers,
                slider_blocks,
                joint_moves,
                feet,
                foot_vertices,
                base,
                hands,
                seat_vertices,
                hand_vertices,
            };
            Ok((body, humanoid))
        };

        let mut bodies = Vec::new();
        let mut humanoid = None;
        let (fallback, fallback_humanoid) = load_body(&characters.fallback, &characters.fallback, read_model, read_bytes)
            .map_err(|e| format!("the fallback body {} did not load: {e}", characters.fallback))?;
        for (choice, file_id) in &characters.bodies {
            match load_body(choice, file_id, read_model, read_bytes) {
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
        for (kind, w) in &characters.wearers {
            for (choice, d) in &w.dress {
                for id in d.hair.iter().chain(&d.brows).chain(&d.lashes) {
                    if look.as_ref().is_some_and(|l| l.garment(id).is_none()) {
                        return Err(format!("data/characters/characters.ron: wearer {kind}'s dress on {choice} names garment {id}, which garments.ron does not list"));
                    }
                }
            }
        }
        // The interaction anchors and the grips (rung 2b), checked against the clips every body
        // shares (one order, held above).
        let grips = crate::interact::Grips::parse(&read("grips.ron")?)?;
        let anchors = crate::interact::Anchors::parse(&read("interactions.ron")?)?;
        anchors.check(&bodies[0].clips, &grips)?;
        Ok(CharacterLibrary { characters, humanoid, bodies, graph, warnings, look, anchors, grips })
    }

    /// The largest own mesh `look::build_person` can make (2026-11-10, the own-mesh pool): the
    /// largest body that carries the look's base file, whole (covers hide triangles, never add
    /// any), and the largest garment of every kind, its vertices and its indices each the most any
    /// garment of that kind has (`Garment::built_size`: a two-sided garment's twice). A person
    /// wears one garment of a kind (an outfit's slot each, then hair, brows and lashes), so this is
    /// the largest dressed person; the engine plans the own-mesh pool for it, and a mesh past it
    /// (two garments of one kind, which nothing offers today) is drawn with the shared body and
    /// logged. (vertices, indices); None without the look.
    pub fn largest_own_mesh(&self) -> Option<(u32, u32)> {
        let look = self.look.as_ref()?;
        let body = self.bodies.iter().filter(|b| b.base.is_some()).map(|b| (b.model.vertex_count(), b.model.primitives.iter().map(|p| p.indices.len()).sum::<usize>())).fold(None, |m: Option<(usize, usize)>, (v, i)| Some(m.map_or((v, i), |(mv, mi)| (mv.max(v), mi.max(i)))))?;
        let mut kinds: Vec<(crate::look::GarmentKind, usize, usize)> = Vec::new();
        for g in &look.garments {
            let (v, i) = g.built_size();
            match kinds.iter_mut().find(|(k, _, _)| *k == g.def.kind) {
                Some(row) => {
                    row.1 = row.1.max(v);
                    row.2 = row.2.max(i);
                }
                None => kinds.push((g.def.kind, v, i)),
            }
        }
        let v = body.0 + kinds.iter().map(|k| k.1).sum::<usize>();
        let i = body.1 + kinds.iter().map(|k| k.2).sum::<usize>();
        Some((u32::try_from(v).ok()?, u32::try_from(i).ok()?))
    }

    /// The garments a person wears (indices into the look's garments): what their look chooses, else
    /// their wearer kind's default on this body, each list's item picked by `seed` (stable per
    /// person). `model_of` gives a cosmetic's garment (its `model` in cosmetics.csv): `None` for a
    /// cosmetic nothing knows (which reads as the default), `Some(None)` for one without a model
    /// (it wears no garment: it tints the body's painted region instead, and no default is put over
    /// it). The outfit's slots in order, then hair, brows and lashes.
    pub fn dress_for(&self, body: usize, wearer: &str, seed: u64, choice: &DressChoice, model_of: &dyn Fn(&str) -> Option<Option<String>>) -> Vec<usize> {
        let Some(look) = self.look.as_ref() else { return Vec::new() };
        let choice_id = self.bodies.get(body).map_or("", |b| b.choice.as_str());
        let empty = DressDef::default();
        let def = self.wearer(wearer).and_then(|w| w.dress.get(choice_id)).unwrap_or(&empty);
        let pick = |list: &[String], what: &str| -> Option<String> {
            if list.is_empty() {
                return None;
            }
            let k = ((stable_unit(seed, what) * list.len() as f32) as usize).min(list.len() - 1);
            Some(list[k].clone())
        };
        let mut out = Vec::new();
        let mut push = |id: Option<String>| {
            if let Some(i) = id.as_deref().and_then(|id| look.garment(id)) {
                if !out.contains(&i) {
                    out.push(i);
                }
            }
        };
        let mut slots: Vec<&String> = def.outfit.keys().chain(choice.outfit.keys()).collect();
        slots.sort();
        slots.dedup();
        for slot in slots {
            // The look names this slot: its cosmetic's garment, or none when it has no model (or
            // reads "none"); an id nothing knows falls through to the default.
            let named = choice.outfit.get(slot).and_then(|id| if id == WEAR_NONE { Some(None) } else { model_of(id) });
            let garment = match named {
                Some(g) => g,
                None => pick(def.outfit.get(slot).map_or(&[][..], |v| v.as_slice()), &format!("outfit_{slot}")).and_then(|c| model_of(&c).flatten()),
            };
            push(garment);
        }
        let mut one = |chosen: &Option<String>, list: &[String], what: &str| {
            let id = match chosen.as_deref() {
                Some(WEAR_NONE) => None,
                Some(id) if look.garment(id).is_some() => Some(id.to_string()),
                _ => pick(list, what),
            };
            push(id);
        };
        one(&choice.hair, &def.hair, "hair");
        one(&choice.brows, &def.brows, "brows");
        one(&choice.lashes, &def.lashes, "lashes");
        out
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

/// A body's slider rows resolved against its mesh, and the blocks the character page shows. Every
/// target named must be one of the mesh's; every slider id must be one a look can carry
/// (`player_look::clean_shape_id`) and appear once.
fn resolve_sliders(rows: &[SliderDef], model: &SkinnedModelData, def: &BodyDef) -> Result<(Vec<Driver>, Vec<SliderBlock>), String> {
    let where_ = format!("data/characters/bodies/{}.ron", def.id);
    let idx = |n: &str| model.morph_index(n).ok_or_else(|| format!("{where_}: a slider names target {n}, which {} does not have", def.model));
    let mut seen: Vec<String> = Vec::new();
    let mut id = |s: &str| -> Result<String, String> {
        if humanity_core::player_look::clean_shape_id(s) != s {
            return Err(format!("{where_}: slider id {s:?} is not one a look can carry (a to z, 0 to 9, _ and -, at most {} characters)", humanity_core::player_look::SHAPE_ID_MAX));
        }
        if seen.iter().any(|x| x == s) {
            return Err(format!("{where_}: slider {s} is named twice"));
        }
        seen.push(s.to_string());
        Ok(s.to_string())
    };
    let mut drivers = Vec::new();
    let mut blocks = Vec::new();
    for row in rows {
        let group = row.group().to_string();
        if humanity_core::player_look::clean_shape_id(&group) != group {
            return Err(format!("{where_}: slider group {group:?} is not a plain id"));
        }
        match row {
            SliderDef::Line { slider, low, high, .. } => {
                let slider = id(slider)?;
                drivers.push(Driver::Line { slider: slider.clone(), low: idx(low)?, high: idx(high)? });
                blocks.push(SliderBlock { group, sliders: vec![slider], shares: false });
            }
            SliderDef::Grid { x, y, x_low, x_high, y_low, y_high, low_low, low_high, high_low, high_high, .. } => {
                let (x, y) = (id(x)?, id(y)?);
                let cells = [
                    [Some(idx(low_low)?), Some(idx(x_low)?), Some(idx(low_high)?)],
                    [Some(idx(y_low)?), None, Some(idx(y_high)?)],
                    [Some(idx(high_low)?), Some(idx(x_high)?), Some(idx(high_high)?)],
                ];
                drivers.push(Driver::Grid { x: x.clone(), y: y.clone(), cells });
                blocks.push(SliderBlock { group, sliders: vec![x, y], shares: false });
            }
            SliderDef::Blend { parts, .. } => {
                if parts.len() < 2 {
                    return Err(format!("{where_}: a blend needs two parts or more"));
                }
                let parts = parts.iter().map(|p| Ok((id(&p.slider)?, idx(&p.target)?))).collect::<Result<Vec<_>, String>>()?;
                blocks.push(SliderBlock { group, sliders: parts.iter().map(|(s, _)| s.clone()).collect(), shares: true });
                drivers.push(Driver::Blend { parts });
            }
        }
    }
    Ok((drivers, blocks))
}

/// Each foot's vertices (primitive, vertex), left then right: those whose largest weight is on that
/// leg's foot or toes joint (the shoe, the sole).
/// Which DRAWN vertices (primitive, vertex: those a triangle uses, so skin hidden under a garment
/// is not counted) are mostly bound to one of `joints`: their largest weight is on one of them.
pub fn vertices_bound_to(model: &SkinnedModelData, joints: &[usize]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for (pi, p) in model.primitives.iter().enumerate() {
        let mut drawn = vec![false; p.vertices.len()];
        for &i in &p.indices {
            if let Some(d) = drawn.get_mut(i as usize) {
                *d = true;
            }
        }
        for (vi, v) in p.vertices.iter().enumerate() {
            if !drawn[vi] {
                continue;
            }
            let k = (0..4).max_by_key(|&k| v.weights[k]).unwrap_or(0);
            if v.weights[k] > 0 && joints.contains(&(v.joints[k] as usize)) {
                out.push((pi, vi));
            }
        }
    }
    out
}

/// The joints whose flesh rests on a seat: the hips and the upper legs (and their twist joints).
pub fn seat_joints(skel: &Skeleton, humanoid: &HumanoidDef) -> Vec<usize> {
    ["hips", "leftUpperLeg", "rightUpperLeg", "leftUpperLegTwist", "rightUpperLegTwist"].iter().filter_map(|r| skel.joint_by_role(humanoid, r)).collect()
}

/// One hand's joints (0 left, 1 right): the hand and every finger joint on that side.
pub fn hand_joints(skel: &Skeleton, humanoid: &HumanoidDef, side: usize) -> Vec<usize> {
    let prefix = if side == 0 { "left" } else { "right" };
    humanoid
        .roles
        .iter()
        .filter(|r| r.role.starts_with(prefix) && (r.role.ends_with("Hand") || ["Thumb", "Index", "Middle", "Ring", "Little"].iter().any(|f| r.role.contains(f))))
        .filter_map(|r| skel.joint_by_role(humanoid, &r.role))
        .collect()
}

fn foot_vertices(model: &SkinnedModelData, rig: &crate::feet::FootRig) -> [Vec<(usize, usize)>; 2] {
    let mut out: [Vec<(usize, usize)>; 2] = Default::default();
    for (pi, p) in model.primitives.iter().enumerate() {
        for (vi, v) in p.vertices.iter().enumerate() {
            let k = (0..4).max_by_key(|&k| v.weights[k]).unwrap_or(0);
            if v.weights[k] == 0 {
                continue;
            }
            let j = v.joints[k] as usize;
            for (side, leg) in rig.legs.iter().enumerate() {
                if j == leg.foot || j == leg.toes {
                    out[side].push((pi, vi));
                }
            }
        }
    }
    out
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
