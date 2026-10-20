//! Reading a 3D file into a [`Model`]: `.glb` (binary glTF 2.0) and `.stl` (binary or ASCII), as
//! docs/design/model-viewer.md says. Every offset, length, count and stride the file gives is
//! checked here BEFORE the glTF reader walks it, so a broken or hostile file is an error message,
//! never a read outside the file or a crash; pictures are decoded here too, under size limits.
//! The website reads the same files the same way (web/shared/model-view.js).

use glam::{Mat3, Mat4, Vec3};
use std::io::Cursor;

/// The most bytes the viewer fetches and reads.
pub const MAX_MODEL_BYTES: usize = 200 * 1024 * 1024;
/// The most triangles the viewer draws.
pub const MAX_TRIANGLES: usize = 5_000_000;
/// Textures larger than this on a side are scaled down to it.
pub const MAX_TEXTURE_SIDE: u32 = 4096;
/// The most vertices one GPU buffer holds (32 bytes each: 128 MiB, half wgpu's default 256 MiB
/// buffer limit). A bigger model is split into chunks of at most this many (`Build::finish`).
pub const CHUNK_VERTICES: usize = 4 * 1024 * 1024;
/// The most vertices the viewer reads (a file can carry far more vertices than its triangles use).
pub const MAX_VERTICES: usize = 15_000_000;
/// The most node visits: nodes that share children over and over would otherwise ask for
/// endless copies of the same mesh.
const MAX_NODE_VISITS: usize = 1_000_000;
/// How deep nodes may nest (a file whose nodes loop back on themselves stops here).
const MAX_NODE_DEPTH: usize = 64;

/// The words for a Draco-compressed file (docs/design/model-viewer.md, "Words").
pub const DRACO_WORDS: &str = "This model is compressed with Draco, which the viewer cannot read yet. Download it to see it in Blender or another 3D program.";

/// How a material treats its colour's alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alpha {
    Opaque,
    /// Drawn where alpha is at least the cutoff, not at all below it.
    Mask,
    Blend,
}

/// How a texture's edges repeat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Wrap {
    Repeat,
    Clamp,
    Mirror,
}

/// How a texture is read: the file's own sampler, glTF's usual defaults where it gives none
/// (smooth, with mipmaps, repeating).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sampling {
    /// Sharp pixels when the texture is drawn larger than it is.
    pub mag_nearest: bool,
    /// Sharp pixels when drawn smaller.
    pub min_nearest: bool,
    /// Between mipmap levels: None for no mipmaps (level 0 only), else whether the step is sharp.
    pub mip_nearest: Option<bool>,
    pub wrap: [Wrap; 2],
}

impl Default for Sampling {
    fn default() -> Sampling {
        Sampling { mag_nearest: false, min_nearest: false, mip_nearest: Some(false), wrap: [Wrap::Repeat; 2] }
    }
}

/// A material's texture: which picture, and how it is read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tex {
    pub image: usize,
    pub sampling: Sampling,
}

/// One material: glTF's metallic-roughness model. Texture fields index [`Model::images`].
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    pub base_color: [f32; 4],
    pub base_texture: Option<Tex>,
    pub metallic: f32,
    pub roughness: f32,
    /// Blue = metallic, green = roughness (linear).
    pub metallic_roughness_texture: Option<Tex>,
    pub normal_texture: Option<Tex>,
    pub normal_scale: f32,
    pub occlusion_texture: Option<Tex>,
    pub occlusion_strength: f32,
    pub emissive: [f32; 3],
    pub emissive_texture: Option<Tex>,
    pub alpha: Alpha,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
}

impl Material {
    /// STL's one plain material (the spec's grey, both sides drawn).
    pub fn plain() -> Material {
        Material {
            base_color: [0.72, 0.74, 0.78, 1.0],
            base_texture: None,
            metallic: 0.0,
            roughness: 0.55,
            metallic_roughness_texture: None,
            normal_texture: None,
            normal_scale: 1.0,
            occlusion_texture: None,
            occlusion_strength: 1.0,
            emissive: [0.0; 3],
            emissive_texture: None,
            alpha: Alpha::Opaque,
            alpha_cutoff: 0.5,
            double_sided: true,
        }
    }
}

/// A decoded picture, RGBA8, at most [`MAX_TEXTURE_SIDE`] on a side, with its mipmaps (each level
/// half the one before, down to 1 by 1), so a large texture stays smooth when the model is small
/// on screen. Made on the loading thread, never while drawing.
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    /// Level 0 first; level `i` is `max(1, width >> i)` by `max(1, height >> i)`.
    pub levels: Vec<Vec<u8>>,
}

/// A run of triangles drawn with one material: `indices[first..first + count]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Draw {
    pub first: u32,
    pub count: u32,
    pub material: usize,
}

/// The box and sphere around every vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
    pub center: Vec3,
    pub radius: f32,
}

/// One vertex as the GPU reads it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

/// Vertices, indices into them, and the runs drawn with each material: one GPU buffer's worth.
#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub draws: Vec<Draw>,
}

/// A model ready to draw: world-space vertices (every node applied), y-up, in chunks of at most
/// [`CHUNK_VERTICES`].
#[derive(Debug, Clone, PartialEq)]
pub struct Model {
    pub chunks: Vec<Chunk>,
    pub materials: Vec<Material>,
    pub images: Vec<Image>,
    pub bounds: Bounds,
    pub triangles: usize,
}

/// A model while it is read: one flat list of everything, chunked at the end (`finish`).
#[derive(Debug, Default)]
pub(super) struct Build {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
    draws: Vec<Draw>,
    materials: Vec<Material>,
    images: Vec<Image>,
}

/// Whether a file of this name opens in the viewer (`.glb`, `.stl`).
pub fn viewable(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".glb") || lower.ends_with(".stl")
}

/// Read `bytes`, named `name` (the ending says which kind).
pub fn load(name: &str, bytes: &[u8]) -> Result<Model, String> {
    if bytes.len() > MAX_MODEL_BYTES {
        return Err(too_large_words(bytes.len() as i64));
    }
    if name.to_ascii_lowercase().ends_with(".stl") {
        load_stl(bytes)
    } else {
        load_glb(bytes)
    }
}

/// The words for a file over [`MAX_MODEL_BYTES`].
pub fn too_large_words(size: i64) -> String {
    format!(
        "This model is {}, larger than the 200 MB the viewer opens. Download it to see it.",
        crate::gui::pages::files::api::size_words(size)
    )
}

/// The words for a model over [`MAX_TRIANGLES`].
pub fn too_detailed_words(triangles: usize) -> String {
    format!(
        "This model has {} triangles, more than the 5,000,000 the viewer draws. Download it to see it.",
        thousands(triangles)
    )
}

/// `12345` as `12,345`.
pub fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The words for a model over [`MAX_VERTICES`].
pub fn too_many_vertices_words(vertices: usize) -> String {
    format!("This model has {} vertices, more than the 15,000,000 the viewer draws. Download it to see it.", thousands(vertices))
}

/// The words for nodes that repeat each other past [`MAX_NODE_VISITS`].
pub const NODES_REPEAT_WORDS: &str = "This 3D file could not be read: its nodes repeat each other too many times.";

/// The words for anything else unreadable.
fn unreadable(reason: impl std::fmt::Display) -> String {
    format!("This 3D file could not be read: {reason}.")
}

// ── STL ──────────────────────────────────────────────────────────────────────

/// An STL file, binary or ASCII, turned from STL's z-up to y-up, normals worked out from each
/// triangle's corners (the file's own are often zero), one plain material.
pub fn load_stl(bytes: &[u8]) -> Result<Model, String> {
    stl_build(bytes)?.finish()
}

/// [`load_stl`] before it is chunked.
pub(super) fn stl_build(bytes: &[u8]) -> Result<Build, String> {
    let triangles = if let Some(tris) = stl_binary(bytes)? { tris } else { stl_ascii(bytes)? };
    if triangles.is_empty() {
        return Err(unreadable("it holds no triangles"));
    }
    let mut m = Build::default();
    for t in &triangles {
        // z-up to y-up: x stays, y = z, z = -y.
        let p: Vec<Vec3> = t.iter().map(|v| Vec3::new(v[0], v[2], -v[1])).collect();
        let n = (p[1] - p[0]).cross(p[2] - p[0]).try_normalize().unwrap_or(Vec3::Y);
        for v in &p {
            m.indices.push(m.positions.len() as u32);
            m.positions.push(v.to_array());
            m.normals.push(n.to_array());
            m.uvs.push([0.0, 0.0]);
        }
    }
    m.materials.push(Material::plain());
    m.draws.push(Draw { first: 0, count: m.indices.len() as u32, material: 0 });
    Ok(m)
}

/// A binary STL's triangles, or None when `bytes` is not one (then it is read as ASCII).
fn stl_binary(bytes: &[u8]) -> Result<Option<Vec<[[f32; 3]; 3]>>, String> {
    if bytes.len() < 84 {
        return Ok(None);
    }
    let count = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let need = count.checked_mul(50).and_then(|n| n.checked_add(84));
    let starts_solid = bytes.trim_ascii_start().starts_with(b"solid");
    // An ASCII file starts "solid"; a binary one may too (its header is free text), so a size that
    // matches its count exactly decides.
    match need {
        Some(n) if n == bytes.len() || (n < bytes.len() && !starts_solid) => {}
        _ => return Ok(None),
    }
    if count > MAX_TRIANGLES {
        return Err(too_detailed_words(count));
    }
    let f = |o: usize| f32::from_le_bytes([bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]]);
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = 84 + i * 50 + 12; // past the stored normal
        let mut t = [[0.0f32; 3]; 3];
        for (k, corner) in t.iter_mut().enumerate() {
            for (j, c) in corner.iter_mut().enumerate() {
                *c = f(o + k * 12 + j * 4);
            }
        }
        if t.iter().flatten().any(|c| !c.is_finite()) {
            return Err(unreadable("a corner is not a number"));
        }
        out.push(t);
    }
    Ok(Some(out))
}

/// An ASCII STL's triangles: every `vertex x y z`, three to a triangle.
fn stl_ascii(bytes: &[u8]) -> Result<Vec<[[f32; 3]; 3]>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| unreadable("it is neither a binary nor a text STL"))?;
    if !text.trim_start().starts_with("solid") {
        return Err(unreadable("it is neither a binary nor a text STL"));
    }
    let mut corners: Vec<[f32; 3]> = Vec::new();
    let mut words = text.split_whitespace();
    while let Some(w) = words.next() {
        if w != "vertex" {
            continue;
        }
        let mut v = [0.0f32; 3];
        for c in v.iter_mut() {
            *c = words
                .next()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|x| x.is_finite())
                .ok_or_else(|| unreadable("a vertex is not three numbers"))?;
        }
        corners.push(v);
        if corners.len() / 3 > MAX_TRIANGLES {
            return Err(too_detailed_words(corners.len() / 3));
        }
    }
    if corners.len() % 3 != 0 {
        return Err(unreadable("its vertices do not come in threes"));
    }
    Ok(corners.chunks(3).map(|c| [c[0], c[1], c[2]]).collect())
}

// ── glTF ─────────────────────────────────────────────────────────────────────

/// A `.glb` file: its scene's meshes on their nodes, their materials and pictures.
pub fn load_glb(bytes: &[u8]) -> Result<Model, String> {
    if bytes.len() < 12 || &bytes[0..4] != b"glTF" {
        return Err(unreadable("it is not a binary glTF (.glb) file"));
    }
    if draco_listed(bytes, "extensionsRequired") {
        return Err(DRACO_WORDS.to_string());
    }
    let draco_used = draco_listed(bytes, "extensionsUsed");
    // A Draco file's accessors have no plain data (only the compressed copy has it), which the glTF
    // reader refuses outright: in a file that uses Draco, that is what its refusal means.
    let gltf = gltf::Gltf::from_slice(bytes).map_err(|e| if draco_used { DRACO_WORDS.to_string() } else { unreadable(e) })?;
    let doc = &gltf.document;
    if doc.extensions_required().any(|e| e == "KHR_draco_mesh_compression") {
        return Err(DRACO_WORDS.to_string());
    }
    // The buffers: the file's own binary chunk, or data written into the file itself.
    let mut buffers: Vec<Vec<u8>> = Vec::new();
    for b in doc.buffers() {
        let data = match b.source() {
            gltf::buffer::Source::Bin => gltf.blob.clone().ok_or_else(|| unreadable("its binary part is missing"))?,
            gltf::buffer::Source::Uri(uri) => data_uri(uri).ok_or_else(|| unreadable("it points to outside files, which a .glb should not"))?,
        };
        if data.len() < b.length() {
            return Err(unreadable("a buffer is shorter than it says"));
        }
        buffers.push(data);
    }

    // The scene: the one named, else the first, else every root node.
    let roots: Vec<gltf::Node> = match doc.default_scene().or_else(|| doc.scenes().next()) {
        Some(scene) => scene.nodes().collect(),
        None => {
            let children: std::collections::HashSet<usize> = doc.nodes().flat_map(|n| n.children().map(|c| c.index())).collect();
            doc.nodes().filter(|n| !children.contains(&n.index())).collect()
        }
    };

    // Every mesh instance with its world matrix, and the triangles they come to, before reading.
    let mut instances: Vec<(gltf::Mesh, Mat4)> = Vec::new();
    let mut visits = 0usize;
    for root in &roots {
        walk(root, Mat4::IDENTITY, 0, &mut visits, &mut instances)?;
    }
    let mut total = 0usize;
    let mut vertices = 0usize;
    for (mesh, _) in &instances {
        for p in mesh.primitives() {
            if p.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            vertices = vertices.saturating_add(p.get(&gltf::Semantic::Positions).map(|a| a.count()).unwrap_or(0));
            let n = match p.indices() {
                Some(a) => a.count() / 3,
                None => p.get(&gltf::Semantic::Positions).map(|a| a.count() / 3).unwrap_or(0),
            };
            total = total.saturating_add(n);
        }
    }
    if total > MAX_TRIANGLES {
        return Err(too_detailed_words(total));
    }
    if vertices > MAX_VERTICES {
        return Err(too_many_vertices_words(vertices));
    }

    let mut m = Build::default();
    let mut material_slots: std::collections::HashMap<Option<usize>, usize> = std::collections::HashMap::new();
    let mut image_slots: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for (mesh, world) in &instances {
        let normal_matrix = Mat3::from_mat4(*world).inverse().transpose();
        let flips = world.determinant() < 0.0;
        for p in mesh.primitives() {
            if p.mode() != gltf::mesh::Mode::Triangles {
                continue; // points, lines and strips are not drawn
            }
            let pos_acc = p.get(&gltf::Semantic::Positions).ok_or_else(|| unreadable("a mesh has no positions"))?;
            if pos_acc.view().is_none() && draco_used {
                return Err(DRACO_WORDS.to_string()); // its positions are only in the compressed copy
            }
            check_accessor(&pos_acc, &buffers, Some((gltf::accessor::DataType::F32, gltf::accessor::Dimensions::Vec3)))?;
            let normal_acc = p.get(&gltf::Semantic::Normals);
            let normals_ok = match &normal_acc {
                Some(a) => check_accessor(a, &buffers, Some((gltf::accessor::DataType::F32, gltf::accessor::Dimensions::Vec3))).is_ok() && a.count() == pos_acc.count(),
                None => false,
            };
            let uv_acc = p.get(&gltf::Semantic::TexCoords(0));
            let uvs_ok = match &uv_acc {
                // The reader takes bytes, shorts or floats here (and stops at anything else).
                Some(a) => check_accessor(a, &buffers, None).is_ok()
                    && a.dimensions() == gltf::accessor::Dimensions::Vec2
                    && matches!(a.data_type(), gltf::accessor::DataType::U8 | gltf::accessor::DataType::U16 | gltf::accessor::DataType::F32)
                    && a.count() == pos_acc.count(),
                None => false,
            };
            if let Some(a) = p.indices() {
                check_accessor(&a, &buffers, None)?;
                let whole = matches!(a.data_type(), gltf::accessor::DataType::U8 | gltf::accessor::DataType::U16 | gltf::accessor::DataType::U32);
                if a.dimensions() != gltf::accessor::Dimensions::Scalar || !whole {
                    return Err(unreadable("its indices are not whole numbers"));
                }
            }
            let reader = p.reader(|b| buffers.get(b.index()).map(|d| d.as_slice()));
            let base = m.positions.len() as u32;
            let local: Vec<[f32; 3]> = reader.read_positions().ok_or_else(|| unreadable("a mesh's positions could not be read"))?.collect();
            let count = local.len();
            for v in &local {
                let w = world.transform_point3(Vec3::from_array(*v));
                if !w.is_finite() {
                    return Err(unreadable("a position is not a number"));
                }
                m.positions.push(w.to_array());
            }
            let mut idx: Vec<u32> = match reader.read_indices() {
                Some(r) => r.into_u32().collect(),
                None => (0..count as u32).collect(),
            };
            if idx.iter().any(|&i| i as usize >= count) {
                return Err(unreadable("an index points past the vertices"));
            }
            idx.truncate(idx.len() - idx.len() % 3);
            if flips {
                for t in idx.chunks_mut(3) {
                    t.swap(1, 2); // a mirrored node turns its triangles inside out; turn them back
                }
            }
            if normals_ok {
                for n in reader.read_normals().into_iter().flatten() {
                    let w = (normal_matrix * Vec3::from_array(n)).try_normalize().unwrap_or(Vec3::Y);
                    m.normals.push(w.to_array());
                }
            } else {
                smooth_normals(&m.positions[base as usize..], &idx, &mut m.normals);
            }
            if uvs_ok {
                if let Some(tc) = reader.read_tex_coords(0) {
                    m.uvs.extend(tc.into_f32());
                }
            }
            m.uvs.resize(m.positions.len(), [0.0, 0.0]);
            m.normals.resize(m.positions.len(), [0.0, 1.0, 0.0]);

            let mat = p.material();
            let slot = match material_slots.get(&mat.index()) {
                Some(&s) => s,
                None => {
                    let made = material(&mat, &buffers, &mut image_slots, &mut m.images)?;
                    m.materials.push(made);
                    material_slots.insert(mat.index(), m.materials.len() - 1);
                    m.materials.len() - 1
                }
            };
            let first = m.indices.len() as u32;
            m.indices.extend(idx.iter().map(|i| i + base));
            m.draws.push(Draw { first, count: m.indices.len() as u32 - first, material: slot });
        }
    }
    if m.indices.is_empty() {
        return Err(unreadable("it holds no triangles"));
    }
    m.finish()
}

/// Whether the file lists Draco compression under `key` (`extensionsRequired` or `extensionsUsed`),
/// read from its JSON before the glTF reader (which refuses an unknown required extension with less
/// useful words).
fn draco_listed(bytes: &[u8], key: &str) -> bool {
    if bytes.len() < 20 {
        return false;
    }
    let len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let Some(json) = bytes.get(20..20usize.saturating_add(len)) else { return false };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(json) else { return false };
    v.get(key)
        .and_then(|e| e.as_array())
        .is_some_and(|a| a.iter().any(|x| x.as_str() == Some("KHR_draco_mesh_compression")))
}

/// The bytes of a `data:` URI written into the file (base64), None for anything else.
fn data_uri(uri: &str) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let rest = uri.strip_prefix("data:")?;
    let (head, data) = rest.split_once(',')?;
    if !head.ends_with(";base64") {
        return None;
    }
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

/// Every mesh under `node`, with its world matrix (`parent` times the node's own).
fn walk<'a>(node: &gltf::Node<'a>, parent: Mat4, depth: usize, visits: &mut usize, out: &mut Vec<(gltf::Mesh<'a>, Mat4)>) -> Result<(), String> {
    if depth > MAX_NODE_DEPTH {
        return Err(unreadable("its nodes nest too deep (or loop back on themselves)"));
    }
    *visits += 1;
    if *visits > MAX_NODE_VISITS {
        return Err(NODES_REPEAT_WORDS.to_string());
    }
    let world = parent * Mat4::from_cols_array_2d(&node.transform().matrix());
    if let Some(mesh) = node.mesh() {
        out.push((mesh, world));
    }
    for child in node.children() {
        walk(&child, world, depth + 1, visits, out)?;
    }
    Ok(())
}

/// Check that `a`'s elements lie inside its buffer view and the view inside its buffer, so the
/// glTF reader never steps outside the file; and, when `want` is given, that its elements are that
/// type.
fn check_accessor(a: &gltf::Accessor, buffers: &[Vec<u8>], want: Option<(gltf::accessor::DataType, gltf::accessor::Dimensions)>) -> Result<(), String> {
    if a.sparse().is_some() {
        return Err(unreadable("it uses sparse accessors, which the viewer cannot read yet"));
    }
    if let Some((data_type, dims)) = want {
        if a.data_type() != data_type || a.dimensions() != dims {
            return Err(unreadable("its positions are stored compressed, which the viewer cannot read yet"));
        }
    }
    let view = a.view().ok_or_else(|| unreadable("an accessor has no data"))?;
    let buffer = buffers.get(view.buffer().index()).ok_or_else(|| unreadable("an accessor's buffer is missing"))?;
    let view_end = view.offset().checked_add(view.length()).ok_or_else(|| unreadable("a buffer view is too long"))?;
    if view_end > buffer.len() {
        return Err(unreadable("a buffer view runs past its buffer"));
    }
    let elem = a.size();
    let stride = view.stride().unwrap_or(elem);
    if stride < elem {
        return Err(unreadable("a buffer view's elements overlap"));
    }
    if a.count() > 0 {
        let end = (a.count() - 1)
            .checked_mul(stride)
            .and_then(|n| n.checked_add(elem))
            .and_then(|n| n.checked_add(a.offset()))
            .ok_or_else(|| unreadable("an accessor is too long"))?;
        if end > view.length() {
            return Err(unreadable("an accessor runs past its buffer view"));
        }
    }
    Ok(())
}

/// Smooth normals for `positions` from the triangles `idx` (their faces' normals, summed per
/// vertex), appended to `out`.
fn smooth_normals(positions: &[[f32; 3]], idx: &[u32], out: &mut Vec<[f32; 3]>) {
    let mut sum = vec![Vec3::ZERO; positions.len()];
    for t in idx.chunks(3) {
        let p = |i: u32| Vec3::from_array(positions[i as usize]);
        let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
        for &i in t {
            sum[i as usize] += n;
        }
    }
    out.extend(sum.into_iter().map(|n| n.try_normalize().unwrap_or(Vec3::Y).to_array()));
}

/// The viewer's material for a glTF material, its pictures decoded into `images` once each.
fn material(mat: &gltf::Material, buffers: &[Vec<u8>], slots: &mut std::collections::HashMap<usize, usize>, images: &mut Vec<Image>) -> Result<Material, String> {
    let pbr = mat.pbr_metallic_roughness();
    let mut texture = |t: Option<gltf::texture::Texture>| -> Result<Option<Tex>, String> {
        let Some(t) = t else { return Ok(None) };
        let sampling = sampling_of(&t.sampler());
        let source = t.source();
        if let Some(&s) = slots.get(&source.index()) {
            return Ok(Some(Tex { image: s, sampling }));
        }
        let bytes: Vec<u8> = match source.source() {
            gltf::image::Source::View { view, .. } => {
                let buffer = buffers.get(view.buffer().index()).ok_or_else(|| unreadable("a picture's buffer is missing"))?;
                let end = view.offset().checked_add(view.length()).filter(|&e| e <= buffer.len()).ok_or_else(|| unreadable("a picture runs past its buffer"))?;
                buffer[view.offset()..end].to_vec()
            }
            gltf::image::Source::Uri { uri, .. } => match data_uri(uri) {
                Some(b) => b,
                None => return Ok(None), // a picture outside the file: drawn without it
            },
        };
        let img = decode_image(&bytes)?;
        images.push(img);
        slots.insert(source.index(), images.len() - 1);
        Ok(Some(Tex { image: images.len() - 1, sampling }))
    };
    let base_texture = texture(pbr.base_color_texture().map(|i| i.texture()))?;
    let metallic_roughness_texture = texture(pbr.metallic_roughness_texture().map(|i| i.texture()))?;
    let normal = mat.normal_texture();
    let normal_scale = normal.as_ref().map(|n| n.scale()).unwrap_or(1.0);
    let normal_texture = texture(normal.map(|n| n.texture()))?;
    let occlusion = mat.occlusion_texture();
    let occlusion_strength = occlusion.as_ref().map(|o| o.strength()).unwrap_or(1.0);
    let occlusion_texture = texture(occlusion.map(|o| o.texture()))?;
    let emissive_texture = texture(mat.emissive_texture().map(|i| i.texture()))?;
    Ok(Material {
        base_color: pbr.base_color_factor(),
        base_texture,
        metallic: pbr.metallic_factor(),
        roughness: pbr.roughness_factor(),
        metallic_roughness_texture,
        normal_texture,
        normal_scale,
        occlusion_texture,
        occlusion_strength,
        emissive: mat.emissive_factor(),
        emissive_texture,
        alpha: match mat.alpha_mode() {
            gltf::material::AlphaMode::Opaque => Alpha::Opaque,
            gltf::material::AlphaMode::Mask => Alpha::Mask,
            gltf::material::AlphaMode::Blend => Alpha::Blend,
        },
        alpha_cutoff: mat.alpha_cutoff().unwrap_or(0.5),
        double_sided: mat.double_sided(),
    })
}

/// The file's own sampler for a texture, glTF's usual defaults where it says nothing.
fn sampling_of(s: &gltf::texture::Sampler) -> Sampling {
    use gltf::texture::{MagFilter, MinFilter, WrappingMode};
    let (min_nearest, mip_nearest) = match s.min_filter() {
        Some(MinFilter::Nearest) => (true, None),
        Some(MinFilter::Linear) => (false, None),
        Some(MinFilter::NearestMipmapNearest) => (true, Some(true)),
        Some(MinFilter::LinearMipmapNearest) => (false, Some(true)),
        Some(MinFilter::NearestMipmapLinear) => (true, Some(false)),
        Some(MinFilter::LinearMipmapLinear) | None => (false, Some(false)),
    };
    let wrap = |w: WrappingMode| match w {
        WrappingMode::ClampToEdge => Wrap::Clamp,
        WrappingMode::MirroredRepeat => Wrap::Mirror,
        WrappingMode::Repeat => Wrap::Repeat,
    };
    Sampling { mag_nearest: matches!(s.mag_filter(), Some(MagFilter::Nearest)), min_nearest, mip_nearest, wrap: [wrap(s.wrap_s()), wrap(s.wrap_t())] }
}

/// A PNG or JPEG picture, decoded under limits and scaled to at most [`MAX_TEXTURE_SIDE`].
fn decode_image(bytes: &[u8]) -> Result<Image, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| unreadable(format!("a picture in it: {e}")))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(1024 * 1024 * 1024);
    reader.limits(limits);
    let mut rgba = reader.decode().map_err(|e| unreadable(format!("a picture in it: {e}")))?.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w.max(h) > MAX_TEXTURE_SIDE {
        let scale = MAX_TEXTURE_SIDE as f32 / w.max(h) as f32;
        let (nw, nh) = (((w as f32 * scale).round() as u32).max(1), ((h as f32 * scale).round() as u32).max(1));
        rgba = image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle);
    }
    let (width, height) = rgba.dimensions();
    let mut levels = vec![rgba.as_raw().clone()];
    let (mut w, mut h) = (width, height);
    let mut prev = rgba;
    while w > 1 || h > 1 {
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
        prev = image::imageops::resize(&prev, w, h, image::imageops::FilterType::Triangle);
        levels.push(prev.as_raw().clone());
    }
    Ok(Image { width, height, levels })
}

impl Build {
    /// Work out the bounds and the triangle count, check the limits once more, and split the
    /// vertices into GPU-sized chunks.
    fn finish(self) -> Result<Model, String> {
        self.finish_in(CHUNK_VERTICES)
    }

    /// [`Build::finish`] with chunks of at most `chunk` vertices (the tests use small ones).
    pub(super) fn finish_in(self, chunk: usize) -> Result<Model, String> {
        let triangles = self.indices.len() / 3;
        if triangles > MAX_TRIANGLES {
            return Err(too_detailed_words(triangles));
        }
        if self.positions.len() > MAX_VERTICES {
            return Err(too_many_vertices_words(self.positions.len()));
        }
        let (mut min, mut max) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
        for p in &self.positions {
            let v = Vec3::from_array(*p);
            min = min.min(v);
            max = max.max(v);
        }
        let center = (min + max) * 0.5;
        let radius = self.positions.iter().map(|p| (Vec3::from_array(*p) - center).length()).fold(0.0f32, f32::max).max(1e-6);
        let vertex = |i: usize| Vertex { position: self.positions[i], normal: self.normals[i], uv: self.uvs[i] };
        let chunks = if self.positions.len() <= chunk {
            // The usual case: everything in one buffer, as it is.
            vec![Chunk { vertices: (0..self.positions.len()).map(vertex).collect(), indices: self.indices.clone(), draws: self.draws.clone() }]
        } else {
            // Too many vertices for one buffer: whole triangles go into chunks of at most `chunk`
            // vertices, each vertex copied into the chunk that uses it.
            let mut chunks: Vec<Chunk> = Vec::new();
            let mut cur = Chunk { vertices: Vec::new(), indices: Vec::new(), draws: Vec::new() };
            let mut local: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
            for d in &self.draws {
                let mut first = cur.indices.len() as u32;
                for t in self.indices[d.first as usize..(d.first + d.count) as usize].chunks(3) {
                    let new = t.iter().filter(|i| !local.contains_key(i)).count();
                    if cur.vertices.len() + new > chunk {
                        if cur.indices.len() as u32 > first {
                            cur.draws.push(Draw { first, count: cur.indices.len() as u32 - first, material: d.material });
                        }
                        chunks.push(std::mem::replace(&mut cur, Chunk { vertices: Vec::new(), indices: Vec::new(), draws: Vec::new() }));
                        local.clear();
                        first = 0;
                    }
                    for &i in t {
                        let at = *local.entry(i).or_insert_with(|| {
                            cur.vertices.push(vertex(i as usize));
                            cur.vertices.len() as u32 - 1
                        });
                        cur.indices.push(at);
                    }
                }
                if cur.indices.len() as u32 > first {
                    cur.draws.push(Draw { first, count: cur.indices.len() as u32 - first, material: d.material });
                }
            }
            if !cur.indices.is_empty() {
                chunks.push(cur);
            }
            chunks
        };
        Ok(Model { chunks, materials: self.materials, images: self.images, bounds: Bounds { min, max, center, radius }, triangles })
    }
}
