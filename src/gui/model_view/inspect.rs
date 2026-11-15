//! The model inspector (docs/design/model-inspector.md): everything the game will do with a 3D model,
//! read from the file before the game does it. Its node tree with every empty (attachment point)
//! named and classed by the conventions in `data/models/inspect_conventions.json`, which way is up
//! and front, its size and pivot, the catalog boxes it is fitted to (`data/machines/*.ron`, fitted
//! exactly as `GltfCpuMesh::fit_for_machine` fits it), its collision shapes (`physics.ron` beside it
//! and any mesh named as a collision shape), its materials and textures, clips and skins, and a
//! report in plain words (each finding's words are `model_inspect.finding.<code>` in en.json).
//!
//! Pure: [`inspect`] takes the file's bytes and what lies around it ([`Context`]) and answers an
//! [`Inspection`]; the viewer (mod.rs) runs it on its loading thread and draws it (inspect_view.rs).
//! The website and `node scripts/inspect-model.js` have its twin in web/shared/model-inspect.js:
//! the same conventions, the same codes, the same words (both read the planted test models in
//! scripts/tests/fixtures/inspect/).

use crate::assets::model_physics::ModelPhysics;
use glam::{Mat4, Vec3};
use serde::Deserialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::load::{self, Model};

/// The most collision edges drawn: past it the rest are counted in words, not drawn.
pub const MAX_EDGES: usize = 20_000;

// ── The conventions (data) ───────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Conventions {
    #[serde(default)]
    pub empties: Vec<EmptyRule>,
    #[serde(default)]
    pub collision: NameRule,
    #[serde(default)]
    pub lod: LodRule,
    #[serde(default)]
    pub scale: ScaleRule,
    #[serde(default)]
    pub tolerance: Tolerance,
}

/// One kind of empty: the names that mark it and what it needs.
#[derive(Debug, Clone, Deserialize)]
pub struct EmptyRule {
    pub id: String,
    pub prefixes: Vec<String>,
    /// `outward`, `level` or `any`.
    #[serde(default)]
    pub forward: String,
    #[serde(default)]
    pub size: Option<SizeRule>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct SizeRule {
    pub min_m: f32,
    pub max_m: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct NameRule {
    #[serde(default)]
    pub prefixes: Vec<String>,
    #[serde(default)]
    pub suffixes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LodRule {
    #[serde(default)]
    pub marker: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ScaleRule {
    pub tallest_m: f32,
    pub smallest_m: f32,
}
impl Default for ScaleRule {
    // Used only when the conventions file is missing (the report says so).
    fn default() -> Self {
        ScaleRule { tallest_m: 50.0, smallest_m: 0.005 }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Tolerance {
    pub catalog_share: f32,
    pub catalog_floor_m: f32,
    pub collision_share: f32,
    pub collision_floor_m: f32,
    pub level_cos: f32,
}
impl Default for Tolerance {
    // Used only when the conventions file is missing (the report says so).
    fn default() -> Self {
        Tolerance { catalog_share: 0.10, catalog_floor_m: 0.05, collision_share: 0.10, collision_floor_m: 0.05, level_cos: 0.95 }
    }
}

impl Conventions {
    pub fn parse(text: &str) -> Result<Conventions, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The conventions file in this run's data folder.
    pub fn load() -> Result<Conventions, String> {
        let path = humanity_core::storage::data_dir().join("models").join("inspect_conventions.json");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        Conventions::parse(&text)
    }

    /// The rule a node's name marks it with, if any (case-insensitive on its start, after a Blender
    /// `.001` suffix is dropped).
    pub fn empty_rule(&self, name: &str) -> Option<&EmptyRule> {
        let n = plain_name(name);
        self.empties.iter().find(|r| r.prefixes.iter().any(|p| !p.is_empty() && n.starts_with(&p.to_lowercase())))
    }

    /// Whether a node's name marks its mesh as a collision shape.
    pub fn is_collision(&self, name: &str) -> bool {
        let n = plain_name(name);
        self.collision.prefixes.iter().any(|p| !p.is_empty() && n.starts_with(&p.to_lowercase()))
            || self.collision.suffixes.iter().any(|s| !s.is_empty() && n.ends_with(&s.to_lowercase()))
    }

    /// The level of detail a node's name marks (`chair_LOD1` is 1), if any.
    pub fn lod_of(&self, name: &str) -> Option<u32> {
        if self.lod.marker.is_empty() {
            return None;
        }
        let n = plain_name(name);
        let at = n.rfind(&self.lod.marker.to_lowercase())?;
        n[at + self.lod.marker.len()..].parse().ok()
    }
}

/// A name lowercased, a Blender duplicate suffix (`.001`) dropped.
pub fn plain_name(name: &str) -> String {
    let n = name.trim().to_lowercase();
    match n.rsplit_once('.') {
        Some((head, tail)) if tail.len() == 3 && tail.bytes().all(|b| b.is_ascii_digit()) => head.to_string(),
        _ => n,
    }
}

// ── What it finds ────────────────────────────────────────────────────────────

/// What a node is.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeKind {
    /// A visible mesh.
    Mesh,
    /// A mesh named as a collision shape: drawn as a wireframe, not counted in the visible bounds.
    Collision,
    /// A mesh marked as a level of detail (`_LOD1`); level 0 counts as visible.
    Lod(u32),
    /// No mesh: an attachment point, of the kind its name marks (None: no convention).
    Empty(Option<String>),
    /// No mesh, children, and a name no convention marks: it only holds other nodes.
    Group,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub index: usize,
    pub name: String,
    pub depth: usize,
    pub parent: Option<usize>,
    pub kind: NodeKind,
    /// Its mesh's index in the file, when it has one.
    pub mesh: Option<usize>,
    /// Where it is and which way its axes point, in the model's frame (every parent applied).
    pub world: Mat4,
}

/// A collision shape as lines, in the model's frame.
#[derive(Debug, Clone)]
pub struct Wire {
    pub label: String,
    pub edges: Vec<[Vec3; 2]>,
    pub min: Vec3,
    pub max: Vec3,
}

/// A catalog row that draws this model.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogRow {
    /// The catalog file (`home.ron`), the row's id and its label.
    pub file: String,
    pub id: String,
    pub label: String,
    pub shape: String,
    pub size: [f32; 3],
}

/// How a catalog box and its fitted model agree.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Verdict {
    Fits,
    /// It fits only after a quarter turn.
    Turned,
    Off,
}

/// A catalog row with the game's fit of the model to it.
#[derive(Debug, Clone)]
pub struct CatalogFit {
    pub row: CatalogRow,
    /// The uniform scale the game gives the model (to the row's height).
    pub scale: f32,
    /// The fitted model's width, height and depth, metres.
    pub fitted: [f32; 3],
    /// The box's width and depth on the floor (a cylinder's or sphere's: twice its radius).
    pub box_wd: [f32; 2],
    pub verdict: Verdict,
    /// Off, and nearer to fitting turned a quarter than as it is: the model lies across its box
    /// (the bed, 2026-11-14), and is the wrong size besides.
    pub across: bool,
    /// The box in the viewer's (the file's) frame: the game's fit undone, so it is drawn round the
    /// model as the game places it.
    pub min: Vec3,
    pub max: Vec3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Problem,
    Check,
    Note,
}

impl Level {
    pub fn id(self) -> &'static str {
        match self {
            Level::Problem => "problem",
            Level::Check => "check",
            Level::Note => "note",
        }
    }
}

/// One line of the report: its level, its code and the numbers its words are filled with.
#[derive(Debug, Clone)]
pub struct Finding {
    pub level: Level,
    pub code: &'static str,
    pub vars: Vec<(&'static str, String)>,
}

impl Finding {
    fn new(level: Level, code: &'static str, vars: &[(&'static str, String)]) -> Finding {
        Finding { level, code, vars: vars.to_vec() }
    }

    /// Its words, from en.json's `model_inspect.finding.<code>`.
    pub fn words(&self) -> String {
        let key = format!("model_inspect.finding.{}", self.code);
        let vars: Vec<(&str, &dyn std::fmt::Display)> = self.vars.iter().map(|(k, v)| (*k, v as &dyn std::fmt::Display)).collect();
        humanity_core::i18n::fill(humanity_core::i18n::t_dyn(&key), &vars)
    }
}

#[derive(Debug, Clone)]
pub struct Texture {
    pub material: String,
    /// `base`, `metal_rough`, `normal`, `occlusion` or `emissive`.
    pub slot: &'static str,
    pub file: String,
    /// Its size, when it could be read.
    pub size: Option<[u32; 2]>,
}

#[derive(Debug, Clone)]
pub struct Clip {
    pub name: String,
    pub seconds: f32,
    pub channels: usize,
}

/// Everything the inspector found in one model.
#[derive(Debug, Clone, Default)]
pub struct Inspection {
    pub nodes: Vec<Node>,
    /// The visible model's bounds (collision shapes and levels of detail past 0 left out).
    pub min: Vec3,
    pub max: Vec3,
    /// Visible triangle parts (a mesh's primitives) in the scene.
    pub parts: usize,
    pub materials: usize,
    pub textures: Vec<Texture>,
    pub clips: Vec<Clip>,
    pub skins: usize,
    pub colliders: Vec<Wire>,
    /// Collision edges past [`MAX_EDGES`], not drawn.
    pub edges_left_out: usize,
    pub catalog: Vec<CatalogFit>,
    pub findings: Vec<Finding>,
    /// An STL file (no nodes, no units).
    pub stl: bool,
}

impl Inspection {
    pub fn size(&self) -> Vec3 {
        (self.max - self.min).max(Vec3::ZERO)
    }

    /// The empties, in tree order.
    pub fn empties(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| matches!(n.kind, NodeKind::Empty(_)))
    }

    pub fn count(&self, level: Level) -> usize {
        self.findings.iter().filter(|f| f.level == level).count()
    }
}

/// What lies around the file: its folder (a `.gltf`'s outside files), the physics file beside it,
/// the catalog rows that draw it, and the conventions.
#[derive(Debug, Clone, Default)]
pub struct Context {
    pub dir: Option<PathBuf>,
    pub physics: Option<Result<ModelPhysics, String>>,
    pub catalog: Vec<CatalogRow>,
    pub conventions: Conventions,
    /// Why the conventions could not be read, when they could not.
    pub conventions_error: Option<String>,
}

impl Context {
    /// For a file on this computer: its folder, the physics file beside it, the catalog rows that
    /// name it, and the conventions file.
    pub fn local(path: &Path) -> Context {
        let (conventions, conventions_error) = match Conventions::load() {
            Ok(c) => (c, None),
            Err(e) => (Conventions::default(), Some(e)),
        };
        Context { dir: path.parent().map(Path::to_path_buf), physics: ModelPhysics::beside(path), catalog: catalog_rows(path), conventions, conventions_error }
    }

    /// For a file fetched from a server: the conventions alone.
    pub fn fetched() -> Context {
        let (conventions, conventions_error) = match Conventions::load() {
            Ok(c) => (c, None),
            Err(e) => (Conventions::default(), Some(e)),
        };
        Context { conventions, conventions_error, ..Context::default() }
    }
}

/// The catalog rows (every `data/machines/*.ron`) whose `model` is the file at `path`, named either
/// from the app's folder (`assets/models/...`) or from the data folder (`models/...`).
pub fn catalog_rows(path: &Path) -> Vec<CatalogRow> {
    let data = humanity_core::storage::data_dir();
    let data = if data.is_absolute() { data } else { std::env::current_dir().unwrap_or_default().join(data) };
    let Ok(real) = path.canonicalize() else { return Vec::new() };
    let mut keys: Vec<String> = Vec::new();
    for base in [data.parent().map(Path::to_path_buf), Some(data.clone())].into_iter().flatten() {
        if let Ok(b) = base.canonicalize() {
            if let Ok(rel) = real.strip_prefix(&b) {
                keys.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    let mut out: Vec<CatalogRow> = Vec::new();
    let Ok(dir) = std::fs::read_dir(data.join("machines")) else { return out };
    let mut files: Vec<PathBuf> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "ron")).collect();
    files.sort();
    for file in files {
        let Some(home) = humanity_systems::grow_machines::load_home(&file) else { continue };
        let fname = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        for (id, def) in &home.catalog {
            let Some(model) = def.model.as_deref() else { continue };
            if !keys.iter().any(|k| k == model) {
                continue;
            }
            let row = CatalogRow { file: fname.clone(), id: id.clone(), label: def.label.clone(), shape: def.shape.clone(), size: [def.size.0, def.size.1, def.size.2] };
            // A ship row merged into every home file is one row.
            if !out.iter().any(|r| r.id == row.id && r.size == row.size) {
                out.push(row);
            }
        }
    }
    out
}

// ── The fit, as the game does it ─────────────────────────────────────────────

/// The game's fit of a model whose first part spans `raw_min..raw_max` (positions as written) to a
/// catalog row: `GltfCpuMesh::fit_for_machine` (scale uniformly about the origin to the row's
/// height, then centre the footprint), compared with the row's box within `tol`.
pub fn fit(raw_min: Vec3, raw_max: Vec3, row: &CatalogRow, tol: &Tolerance) -> CatalogFit {
    let raw = (raw_max - raw_min).max(Vec3::ZERO);
    let target = row.size[1];
    // scale_to_height's rule: a no-op for a nonsense height, or a scale within 1e-4 of 1.
    let mut s = if target.is_finite() && target > 0.0 && raw.y > 1e-6 { target / raw.y } else { 1.0 };
    if !s.is_finite() || (s - 1.0).abs() < 1e-4 {
        s = 1.0;
    }
    let fitted = [raw.x * s, raw.y * s, raw.z * s];
    let box_wd = match row.shape.as_str() {
        "cylinder" | "sphere" => [row.size[0] * 2.0, row.size[0] * 2.0],
        _ => [row.size[0], row.size[2]],
    };
    let near = |a: f32, b: f32| (a - b).abs() <= (b * tol.catalog_share).max(tol.catalog_floor_m);
    let verdict = if near(fitted[0], box_wd[0]) && near(fitted[2], box_wd[1]) {
        Verdict::Fits
    } else if near(fitted[2], box_wd[0]) && near(fitted[0], box_wd[1]) {
        Verdict::Turned
    } else {
        Verdict::Off
    };
    let err = |a: f32, b: f32| (a - b).abs() / b.max(1e-6);
    let across = verdict == Verdict::Off && err(fitted[2], box_wd[0]).max(err(fitted[0], box_wd[1])) < err(fitted[0], box_wd[0]).max(err(fitted[2], box_wd[1]));
    // The game's placement undone: p_file = p_game / s + footprint centre (the base stays at the
    // file's y = 0, which is where the game stands it).
    let (cx, cz) = ((raw_min.x + raw_max.x) * 0.5, (raw_min.z + raw_max.z) * 0.5);
    let (hw, hd) = (box_wd[0] * 0.5 / s, box_wd[1] * 0.5 / s);
    let min = Vec3::new(cx - hw, 0.0, cz - hd);
    let max = Vec3::new(cx + hw, target / s, cz + hd);
    CatalogFit { row: row.clone(), scale: s, fitted, box_wd, verdict, across, min, max }
}

/// Metres to two decimals, the report's way.
pub fn m2(x: f32) -> String {
    format!("{:.2}", x)
}

// ── The inspection ───────────────────────────────────────────────────────────

/// Inspect the model file `name` (`bytes`, already read by the viewer into `model`).
pub fn inspect(name: &str, bytes: &[u8], model: &Model, ctx: &Context) -> Inspection {
    let mut out = Inspection::default();
    if let Some(e) = &ctx.conventions_error {
        out.findings.push(Finding::new(Level::Check, "conventions_missing", &[("why", e.clone())]));
    }
    if name.to_ascii_lowercase().ends_with(".stl") {
        out.stl = true;
        out.min = model.bounds.min;
        out.max = model.bounds.max;
        out.parts = 1;
        let s = out.size();
        out.findings.push(Finding::new(Level::Note, "stl_units", &[("w", m2(s.x)), ("h", m2(s.y)), ("d", m2(s.z))]));
        scale_findings(&mut out, &ctx.conventions);
        sort(&mut out);
        return out;
    }
    if let Err(words) = inspect_gltf(bytes, model, ctx, &mut out) {
        out.findings.push(Finding::new(Level::Problem, "unreadable", &[("why", words)]));
    }
    sort(&mut out);
    out
}

fn sort(out: &mut Inspection) {
    out.findings.sort_by_key(|f| f.level);
}

/// The buffers of a glTF file: its binary chunk, data written into it, or (from `dir`) its outside
/// files.
fn buffers(gltf: &gltf::Gltf, dir: Option<&Path>) -> Result<Vec<Vec<u8>>, String> {
    let mut out = Vec::new();
    for b in gltf.document.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => gltf.blob.clone().ok_or("its binary part is missing")?,
            gltf::buffer::Source::Uri(uri) => load::data_uri(uri).or_else(|| dir.and_then(|d| load::outside_file(d, uri))).ok_or_else(|| format!("its file {uri} is not beside it"))?,
        };
        if data.len() < b.length() {
            return Err("a buffer is shorter than it says".into());
        }
        out.push(data);
    }
    Ok(out)
}

/// One primitive's positions (as written) and triangles, bounds-checked; None when it has none or
/// they cannot be read safely.
fn triangles(p: &gltf::Primitive, buffers: &[Vec<u8>]) -> Option<(Vec<Vec3>, Vec<u32>)> {
    if p.mode() != gltf::mesh::Mode::Triangles {
        return None;
    }
    let pos = p.get(&gltf::Semantic::Positions)?;
    load::check_accessor(&pos, buffers, Some((gltf::accessor::DataType::F32, gltf::accessor::Dimensions::Vec3))).ok()?;
    if let Some(a) = p.indices() {
        load::check_accessor(&a, buffers, None).ok()?;
    }
    let reader = p.reader(|b| buffers.get(b.index()).map(|d| d.as_slice()));
    let positions: Vec<Vec3> = reader.read_positions()?.map(Vec3::from_array).collect();
    let mut idx: Vec<u32> = match reader.read_indices() {
        Some(r) => r.into_u32().collect(),
        None => (0..positions.len() as u32).collect(),
    };
    if idx.iter().any(|&i| i as usize >= positions.len()) {
        return None;
    }
    idx.truncate(idx.len() - idx.len() % 3);
    Some((positions, idx))
}

/// A mesh's triangle edges as lines, each once, `world` applied, into `wire` (at most `room`).
fn add_edges(mesh: &gltf::Mesh, buffers: &[Vec<u8>], world: Mat4, wire: &mut Wire, room: &mut usize, left_out: &mut usize) {
    for p in mesh.primitives() {
        let Some((pos, idx)) = triangles(&p, buffers) else { continue };
        let mut seen: HashSet<(u32, u32)> = HashSet::new();
        for t in idx.chunks(3) {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let key = (a.min(b), a.max(b));
                if !seen.insert(key) {
                    continue;
                }
                let (pa, pb) = (world.transform_point3(pos[a as usize]), world.transform_point3(pos[b as usize]));
                wire.min = wire.min.min(pa).min(pb);
                wire.max = wire.max.max(pa).max(pb);
                if *room == 0 {
                    *left_out += 1;
                } else {
                    wire.edges.push([pa, pb]);
                    *room -= 1;
                }
            }
        }
    }
}

fn empty_wire(label: String) -> Wire {
    Wire { label, edges: Vec::new(), min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) }
}

fn inspect_gltf(bytes: &[u8], model: &Model, ctx: &Context, out: &mut Inspection) -> Result<(), String> {
    let conv = &ctx.conventions;
    let tol = conv.tolerance;
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| e.to_string())?;
    let doc = &gltf.document;
    let bufs = buffers(&gltf, ctx.dir.as_deref())?;

    // The node tree: the scene the viewer draws (the one named, else the first, else every root).
    let roots: Vec<gltf::Node> = match doc.default_scene().or_else(|| doc.scenes().next()) {
        Some(scene) => scene.nodes().collect(),
        None => {
            let children: HashSet<usize> = doc.nodes().flat_map(|n| n.children().map(|c| c.index())).collect();
            doc.nodes().filter(|n| !children.contains(&n.index())).collect()
        }
    };
    let mut stack: Vec<(gltf::Node, Mat4, usize, Option<usize>)> = roots.into_iter().rev().map(|n| (n, Mat4::IDENTITY, 0, None)).collect();
    let mut visits = 0usize;
    let mut mesh_nodes: Vec<(usize, Mat4)> = Vec::new(); // (mesh index, world) of visible meshes
    let mut collision_nodes: Vec<(usize, Mat4, String)> = Vec::new();
    while let Some((node, parent, depth, parent_slot)) = stack.pop() {
        visits += 1;
        if visits > 100_000 || depth > 64 {
            break;
        }
        let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
        let name = node.name().unwrap_or("").to_string();
        let kind = match node.mesh() {
            Some(m) if conv.is_collision(&name) || m.name().is_some_and(|n| conv.is_collision(n)) => {
                collision_nodes.push((m.index(), world, name.clone()));
                NodeKind::Collision
            }
            Some(m) => {
                let lod = conv.lod_of(&name).or_else(|| m.name().and_then(|n| conv.lod_of(n)));
                if lod.unwrap_or(0) == 0 {
                    mesh_nodes.push((m.index(), world));
                }
                lod.map(NodeKind::Lod).unwrap_or(NodeKind::Mesh)
            }
            None => match conv.empty_rule(&name) {
                Some(r) => NodeKind::Empty(Some(r.id.clone())),
                None if node.children().next().is_none() => NodeKind::Empty(None),
                None => NodeKind::Group,
            },
        };
        let slot = out.nodes.len();
        out.nodes.push(Node { index: node.index(), name, depth, parent: parent_slot, kind, mesh: node.mesh().map(|m| m.index()), world });
        let kids: Vec<gltf::Node> = node.children().collect();
        for child in kids.into_iter().rev() {
            stack.push((child, world, depth + 1, Some(slot)));
        }
    }

    // The visible bounds, from the vertices themselves.
    let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
    for (mi, world) in &mesh_nodes {
        let Some(mesh) = doc.meshes().nth(*mi) else { continue };
        for p in mesh.primitives() {
            let Some((pos, _)) = triangles(&p, &bufs) else { continue };
            out.parts += 1;
            for v in pos {
                let w = world.transform_point3(v);
                lo = lo.min(w);
                hi = hi.max(w);
            }
        }
    }
    if lo.is_finite() && hi.is_finite() {
        out.min = lo;
        out.max = hi;
    } else {
        out.min = model.bounds.min;
        out.max = model.bounds.max;
    }
    out.materials = doc.materials().count();
    out.skins = doc.skins().count();
    out.clips = doc
        .animations()
        .enumerate()
        .map(|(i, a)| {
            let seconds = a
                .samplers()
                .filter_map(|s| s.input().max().and_then(|v| v.as_array().and_then(|x| x.first().and_then(|f| f.as_f64()))))
                .fold(0.0f64, f64::max) as f32;
            Clip { name: a.name().map(str::to_string).unwrap_or_else(|| format!("#{i}")), seconds, channels: a.channels().count() }
        })
        .collect();

    // Size, pivot and scale.
    scale_findings(out, conv);
    let size = out.size();
    let centre = (out.min + out.max) * 0.5;
    let (dx, dz, base) = (centre.x, centre.z, out.min.y);
    let centred = dx.abs() <= (size.x * 0.1).max(0.01) && dz.abs() <= (size.z * 0.1).max(0.01);
    let on_base = base.abs() <= (size.y * 0.02).max(0.005);
    let pivot = [("x", m2(dx)), ("z", m2(dz)), ("y", m2(base))];
    out.findings.push(if centred && on_base { Finding::new(Level::Note, "pivot_base_centred", &[]) } else { Finding::new(Level::Check, "pivot_off", &pivot) });

    // Textures: every material's pictures, their sizes, the missing named.
    let mut seen_images: HashSet<(usize, &'static str, usize)> = HashSet::new();
    for (mi, mat) in doc.materials().enumerate() {
        let pbr = mat.pbr_metallic_roughness();
        let slots: [(&'static str, Option<gltf::texture::Texture>); 5] = [
            ("base", pbr.base_color_texture().map(|i| i.texture())),
            ("metal_rough", pbr.metallic_roughness_texture().map(|i| i.texture())),
            ("normal", mat.normal_texture().map(|n| n.texture())),
            ("occlusion", mat.occlusion_texture().map(|o| o.texture())),
            ("emissive", mat.emissive_texture().map(|i| i.texture())),
        ];
        let mname = mat.name().map(str::to_string).unwrap_or_else(|| format!("#{mi}"));
        for (slot, tex) in slots {
            let Some(tex) = tex else { continue };
            let img = tex.source();
            if !seen_images.insert((mi, slot, img.index())) {
                continue;
            }
            let (file, data) = match img.source() {
                gltf::image::Source::View { view, .. } => {
                    let data = bufs.get(view.buffer().index()).and_then(|b| b.get(view.offset()..view.offset().checked_add(view.length())?)).map(<[u8]>::to_vec);
                    (img.name().map(str::to_string).unwrap_or_else(|| format!("#{}", img.index())), data)
                }
                gltf::image::Source::Uri { uri, .. } => {
                    let data = load::data_uri(uri).or_else(|| ctx.dir.as_deref().and_then(|d| load::outside_file(d, uri)));
                    let shown = if uri.starts_with("data:") { format!("#{}", img.index()) } else { uri.to_string() };
                    (shown, data)
                }
            };
            let size = data.as_deref().and_then(|d| {
                image::ImageReader::new(std::io::Cursor::new(d)).with_guessed_format().ok()?.into_dimensions().ok().map(|(w, h)| [w, h])
            });
            match size {
                None => out.findings.push(Finding::new(Level::Problem, "texture_missing", &[("material", mname.clone()), ("slot", slot.to_string()), ("file", file.clone())])),
                Some([w, h]) if w.max(h) > load::MAX_TEXTURE_SIDE => {
                    out.findings.push(Finding::new(Level::Check, "texture_big", &[("file", file.clone()), ("w", w.to_string()), ("h", h.to_string())]))
                }
                _ => {}
            }
            out.textures.push(Texture { material: mname.clone(), slot, file, size });
        }
    }

    // What the game draws: a catalog model's first mesh's first part, positions as written.
    let first = doc.meshes().next();
    let engine = first.as_ref().and_then(|m| m.primitives().next()).and_then(|p| triangles(&p, &bufs)).map(|(pos, _)| {
        pos.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(a, b), v| (a.min(*v), b.max(*v)))
    });
    // The game ignores the first mesh's node: it differs when that node moves, turns or scales it,
    // or when no visible node shows it at all.
    let first_moved = first.as_ref().is_some_and(|m| {
        let shows: Vec<&Node> = out.nodes.iter().filter(|n| n.mesh == Some(m.index()) && matches!(n.kind, NodeKind::Mesh | NodeKind::Lod(0))).collect();
        shows.is_empty() || shows.iter().any(|n| !n.world.abs_diff_eq(Mat4::IDENTITY, 1e-5))
    });
    let first_part = first.as_ref().and_then(|m| m.name()).map(str::to_string).unwrap_or_else(|| "#0".into());
    if out.parts > 1 || first_moved {
        let level = if ctx.catalog.is_empty() { Level::Check } else { Level::Problem };
        out.findings.push(Finding::new(level, "engine_differs", &[("part", first_part), ("parts", out.parts.to_string())]));
    }
    if let Some((rmin, rmax)) = engine.filter(|(a, b)| a.is_finite() && b.is_finite()) {
        if !ctx.catalog.is_empty() && rmin.y.abs() > (rmax.y - rmin.y).max(0.0) * 0.02 + 0.005 {
            let code = if rmin.y > 0.0 { "base_above" } else { "base_below" };
            out.findings.push(Finding::new(Level::Problem, code, &[("y", m2(rmin.y.abs()))]));
        }
        for row in &ctx.catalog {
            let f = fit(rmin, rmax, row, &tol);
            let vars = [
                ("row", row.id.clone()),
                ("file", row.file.clone()),
                ("box_w", m2(f.box_wd[0])),
                ("box_d", m2(f.box_wd[1])),
                ("h", m2(f.fitted[1])),
                ("w", m2(f.fitted[0])),
                ("d", m2(f.fitted[2])),
            ];
            out.findings.push(match f.verdict {
                Verdict::Fits => Finding::new(Level::Note, "catalog_fits", &vars),
                Verdict::Turned => Finding::new(Level::Problem, "catalog_turned", &vars),
                Verdict::Off if f.across => Finding::new(Level::Problem, "catalog_across", &vars),
                Verdict::Off => Finding::new(Level::Problem, "catalog_box", &vars),
            });
            out.catalog.push(f);
        }
    }

    // Collision: physics.ron's hulls (meshes no scene draws, in the model's frame) and the meshes
    // named as collision shapes.
    let mut room = MAX_EDGES;
    match &ctx.physics {
        Some(Ok(phys)) => {
            for c in &phys.colliders {
                let Some(h) = c.hull_mesh else { continue };
                match doc.meshes().nth(h as usize) {
                    Some(mesh) => {
                        let mut wire = empty_wire(c.part.clone());
                        add_edges(&mesh, &bufs, Mat4::IDENTITY, &mut wire, &mut room, &mut out.edges_left_out);
                        out.colliders.push(wire);
                    }
                    None => out.findings.push(Finding::new(Level::Problem, "physics_bad", &[("why", format!("collider {} names mesh {h}, which this file does not have", c.part))])),
                }
            }
        }
        Some(Err(e)) => out.findings.push(Finding::new(Level::Problem, "physics_bad", &[("why", e.clone())])),
        None => {}
    }
    for (mi, world, name) in &collision_nodes {
        if let Some(mesh) = doc.meshes().nth(*mi) {
            let mut wire = empty_wire(name.clone());
            add_edges(&mesh, &bufs, *world, &mut wire, &mut room, &mut out.edges_left_out);
            out.colliders.push(wire);
        }
    }
    out.colliders.retain(|w| w.min.is_finite() && w.max.is_finite());
    if out.colliders.is_empty() {
        out.findings.push(Finding::new(Level::Note, "collision_none", &[]));
    } else {
        let (cmin, cmax) = out.colliders.iter().fold((Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY)), |(a, b), w| (a.min(w.min), b.max(w.max)));
        let size = out.size();
        let gap = |a: f32, b: f32, side: f32| (a - b).abs() > (side * tol.collision_share).max(tol.collision_floor_m);
        let off = (0..3).any(|k| gap(cmin[k], out.min[k], size[k]) || gap(cmax[k], out.max[k], size[k]));
        if off {
            let worst = (0..3).map(|k| (cmin[k] - out.min[k]).abs().max((cmax[k] - out.max[k]).abs())).fold(0.0f32, f32::max);
            out.findings.push(Finding::new(Level::Problem, "collision_off", &[("gap", m2(worst)), ("count", out.colliders.len().to_string())]));
        } else {
            out.findings.push(Finding::new(Level::Note, "collision_ok", &[("count", out.colliders.len().to_string())]));
        }
    }

    // The empties: named, of a known kind, pointing the right way, the right size.
    let centre = (out.min + out.max) * 0.5;
    for n in &out.nodes {
        let NodeKind::Empty(kind) = &n.kind else { continue };
        if n.name.trim().is_empty() {
            out.findings.push(Finding::new(Level::Check, "empty_unnamed", &[("index", n.index.to_string())]));
            continue;
        }
        let Some(rule) = kind.as_ref().and_then(|k| conv.empties.iter().find(|r| &r.id == k)) else {
            out.findings.push(Finding::new(Level::Check, "empty_unknown", &[("name", n.name.clone())]));
            continue;
        };
        let at = n.world.w_axis.truncate();
        let fwd = n.world.z_axis.truncate().normalize_or_zero();
        let up = n.world.y_axis.truncate().normalize_or_zero();
        match rule.forward.as_str() {
            "outward" => {
                let away = at - centre;
                if away.length() > 1e-4 && fwd.dot(away.normalize()) < 0.0 {
                    out.findings.push(Finding::new(Level::Problem, "empty_wrong_way", &[("name", n.name.clone()), ("kind", rule.id.clone())]));
                }
            }
            "level" => {
                if up.dot(Vec3::Y) < tol.level_cos {
                    out.findings.push(Finding::new(Level::Problem, "empty_not_level", &[("name", n.name.clone()), ("kind", rule.id.clone())]));
                }
            }
            _ => {}
        }
        if let Some(sz) = rule.size {
            let s = Vec3::new(n.world.x_axis.truncate().length(), n.world.y_axis.truncate().length(), n.world.z_axis.truncate().length());
            if s.min_element() < sz.min_m || s.max_element() > sz.max_m {
                out.findings.push(Finding::new(
                    Level::Problem,
                    "empty_size",
                    &[("name", n.name.clone()), ("kind", rule.id.clone()), ("size", format!("{} x {} x {}", m2(s.x), m2(s.y), m2(s.z))), ("min", m2(sz.min_m)), ("max", m2(sz.max_m))],
                ));
            }
        }
    }
    Ok(())
}

/// A model hundreds of metres tall, or a few millimetres small: the units were probably wrong.
fn scale_findings(out: &mut Inspection, conv: &Conventions) {
    let largest = out.size().max_element();
    if largest > conv.scale.tallest_m {
        out.findings.push(Finding::new(Level::Check, "scale_huge", &[("size", m2(largest)), ("cm", m2(largest / 100.0)), ("mm", m2(largest / 1000.0))]));
    } else if largest > 0.0 && largest < conv.scale.smallest_m {
        out.findings.push(Finding::new(Level::Check, "scale_tiny", &[("size", format!("{:.4}", largest)), ("big", m2(largest * 1000.0))]));
    }
}

#[cfg(test)]
#[path = "inspect_tests.rs"]
mod tests;
