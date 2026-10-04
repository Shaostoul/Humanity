//! Asset manager — loads and caches game data with hot-reload support.
//!
//! Supported data formats: CSV, TOML, RON, JSON.
//! Asset formats: GLB (meshes), PNG/KTX2 (textures), OGG/WAV (audio), WGSL (shaders).
//!
//! The data directory lives next to the exe (like Space Engineers' Content/ folder).
//! On native: reads from disk, watches for changes via notify.
//! On WASM: data fetched via HTTP from the server.

#[cfg(feature = "native")]
pub mod watcher;
pub mod loader;
/// Comment-preserving single-field RON value rewriter (Planet Tuner enabler,
/// docs/design/artificial-planet.md). Pure std, ungated on purpose.
pub mod ron_edit;

use std::collections::HashMap;
use std::any::Any;
use std::path::PathBuf;
use serde::de::DeserializeOwned;

use crate::embedded_data;

/// CPU-side mesh data decoded from a glTF file — plain vertex/index arrays,
/// no GPU resources, so it can be produced without a wgpu device (unit tests,
/// worker threads) and uploaded later via
/// `crate::renderer::mesh::Mesh::from_vertices(device, &vertices, &indices)`.
#[cfg(feature = "native")]
pub struct GltfCpuMesh {
    pub vertices: Vec<crate::renderer::mesh::Vertex>,
    pub indices: Vec<u32>,
}

#[cfg(feature = "native")]
impl GltfCpuMesh {
    /// The model's height in metres AS AUTHORED: the Y extent of its vertex
    /// bounding box. Returns 0.0 for an empty mesh.
    pub fn authored_height(&self) -> f32 {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for v in &self.vertices {
            lo = lo.min(v.position[1]);
            hi = hi.max(v.position[1]);
        }
        if lo.is_finite() && hi.is_finite() {
            (hi - lo).max(0.0)
        } else {
            0.0
        }
    }

    /// Scale every vertex UNIFORMLY about the model's own origin so the mesh
    /// stands `target_height` metres tall. Returns the factor applied.
    ///
    /// Why height, and why uniform. A machine's `size` in `data/machines/*.ron`
    /// is its real-world size, and until this existed it was only used to build
    /// the PRIMITIVE fallback box: a model, when one was declared, was drawn at
    /// whatever scale it was authored at. The furniture the home ships with is
    /// authored at roughly half real scale (a chair 0.46 m tall, a bookcase
    /// 0.88 m, a desk 0.38 m), so a player whose eyes are at 1.7 m walked
    /// through a doll's house. The operator, seeing it: "Seems like either all
    /// the furniture is small or I'm just really big."
    ///
    /// Height is the dimension a person reads scale from, and it does not care
    /// which way round the model was authored, so it survives a model whose
    /// long axis disagrees with the `size` written beside it (the double bed is
    /// exactly that). Uniform keeps the model's own proportions, which a
    /// per-axis stretch to the declared box would visibly destroy. Because the
    /// pack is internally consistent and the declared sizes were written from
    /// real furniture, fitting the height lands almost every piece on its
    /// declared footprint as well.
    ///
    /// Scaling about the ORIGIN and not the centre is deliberate: these models
    /// are authored with their base on y = 0, so the origin is the floor the
    /// piece stands on and it stays there.
    ///
    /// A no-op returning 1.0 when either height is not a positive, finite
    /// number, so a missing or nonsense `size` leaves the model as authored
    /// rather than collapsing it to a point.
    pub fn scale_to_height(&mut self, target_height: f32) -> f32 {
        let authored = self.authored_height();
        if !(target_height.is_finite() && target_height > 0.0) || authored <= 1e-6 {
            return 1.0;
        }
        let s = target_height / authored;
        if !s.is_finite() || (s - 1.0).abs() < 1e-4 {
            return 1.0;
        }
        for v in &mut self.vertices {
            v.position[0] *= s;
            v.position[1] *= s;
            v.position[2] *= s;
        }
        // Normals are unchanged: a uniform scale does not rotate them, and
        // they were already unit length.
        s
    }
}

/// Central asset manager: loads data files, caches parsed results, supports hot-reload.
pub struct AssetManager {
    /// Root data directory (e.g., `HumanityOS/content/data/`).
    data_dir: PathBuf,
    /// Cached parsed data, keyed by relative path from data_dir.
    cache: HashMap<String, Box<dyn Any + Send + Sync>>,
    /// Cached mesh indices from loaded GLTF models, keyed by relative path.
    mesh_cache: HashMap<String, usize>,
}

impl AssetManager {
    /// Create a new asset manager rooted at the given data directory.
    pub fn new(data_dir: PathBuf) -> Self {
        log::info!("AssetManager: data directory = {}", data_dir.display());
        Self {
            data_dir,
            cache: HashMap::new(),
            mesh_cache: HashMap::new(),
        }
    }

    /// Full path to a data file.
    pub fn data_path(&self, relative: &str) -> PathBuf {
        self.data_dir.join(relative)
    }

    /// The root data directory.
    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Load and parse a CSV file into a Vec<T>. Results are cached by path.
    /// Skips comment lines (starting with #).
    #[cfg(feature = "native")]
    pub fn load_csv<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&Vec<T>, String> {
        if !self.cache.contains_key(relative_path) {
            let path = self.data_dir.join(relative_path);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let records: Vec<T> = loader::parse_csv(&bytes)?;
            log::info!("Loaded {} records from {}", records.len(), relative_path);
            self.cache.insert(relative_path.to_string(), Box::new(records));
        }
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<Vec<T>>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load and parse a TOML file into T. Results are cached by path.
    #[cfg(feature = "native")]
    pub fn load_toml<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if !self.cache.contains_key(relative_path) {
            let path = self.data_dir.join(relative_path);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let value: T = loader::parse_toml(&bytes)?;
            log::info!("Loaded TOML: {}", relative_path);
            self.cache.insert(relative_path.to_string(), Box::new(value));
        }
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load and parse a RON file into T. Results are cached by path.
    #[cfg(feature = "native")]
    pub fn load_ron<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if !self.cache.contains_key(relative_path) {
            let path = self.data_dir.join(relative_path);
            let bytes = std::fs::read(&path)
                .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
            let value: T = loader::parse_ron(&bytes)?;
            log::info!("Loaded RON: {}", relative_path);
            self.cache.insert(relative_path.to_string(), Box::new(value));
        }
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Parse a GLTF/GLB into an engine Mesh WITHOUT caching or registering it
    /// (v0.734): each caller owns its Mesh, so per-instance machine models can
    /// safely live in per-machine renderer slots (the editor's replace_mesh
    /// reuse path would corrupt a SHARED cached mesh — see
    /// docs/game/model-pipeline.md's hazard note). Resolution: `data_dir`
    /// first (the distributed/moddable tree, e.g. data/models/x.glb), then
    /// the data dir's PARENT (the dev repo root, so assets/models/x.glb works
    /// in a checkout).
    #[cfg(feature = "native")]
    pub fn parse_gltf_mesh(
        &self,
        device: &wgpu::Device,
        relative_path: &str,
    ) -> Result<crate::renderer::mesh::Mesh, String> {
        let path = self.resolve_model_path(relative_path);
        let (document, buffers, _images) = gltf::import(&path)
            .map_err(|e| format!("Failed to load GLTF {}: {e}", path.display()))?;
        let cpu = Self::decode_first_primitive(&document, &buffers, relative_path)?;
        Ok(crate::renderer::mesh::Mesh::from_vertices(device, &cpu.vertices, &cpu.indices))
    }

    /// As [`parse_gltf_mesh`](Self::parse_gltf_mesh), but the geometry is
    /// first scaled uniformly to stand `target_height` metres tall (see
    /// [`GltfCpuMesh::scale_to_height`]).
    ///
    /// Machines have TWO model paths, and both must fit or the home changes
    /// size the first time anything rebuilds it: this one draws the machines
    /// at world load, and the textured one draws them on every later rebuild
    /// from the construction editor. Fixing only the rebuild path is how the
    /// first attempt at this looked, in the capture rig, as though nothing had
    /// happened at all.
    #[cfg(feature = "native")]
    pub fn parse_gltf_mesh_fit_height(
        &self,
        device: &wgpu::Device,
        relative_path: &str,
        target_height: f32,
    ) -> Result<crate::renderer::mesh::Mesh, String> {
        let path = self.resolve_model_path(relative_path);
        let (document, buffers, _images) = gltf::import(&path)
            .map_err(|e| format!("Failed to load GLTF {}: {e}", path.display()))?;
        let mut cpu = Self::decode_first_primitive(&document, &buffers, relative_path)?;
        cpu.scale_to_height(target_height);
        Ok(crate::renderer::mesh::Mesh::from_vertices(device, &cpu.vertices, &cpu.indices))
    }

    /// Resolve a model path: `data_dir` first (the distributed/moddable tree,
    /// e.g. data/models/x.glb), then the data dir's PARENT (the dev repo
    /// root, so assets/models/x.gltf works in a checkout). Same rule
    /// `parse_gltf_mesh` has always used.
    #[cfg(feature = "native")]
    fn resolve_model_path(&self, relative_path: &str) -> PathBuf {
        let path = self.data_dir.join(relative_path);
        if !path.exists() {
            if let Some(parent) = self.data_dir.parent() {
                let alt = parent.join(relative_path);
                if alt.exists() {
                    return alt;
                }
            }
        }
        path
    }

    /// Decode the FIRST mesh's FIRST primitive of an imported glTF document
    /// into CPU-side vertex/index arrays — the shared geometry path behind
    /// `parse_gltf_mesh`, `load_gltf`, and `parse_gltf_mesh_with_texture`.
    /// Missing normals get flat generated normals; missing UVs get planar UVs.
    #[cfg(feature = "native")]
    fn decode_first_primitive(
        document: &gltf::Document,
        buffers: &[gltf::buffer::Data],
        relative_path: &str,
    ) -> Result<GltfCpuMesh, String> {
        let gltf_mesh = document.meshes().next()
            .ok_or_else(|| format!("No meshes in {relative_path}"))?;
        let primitive = gltf_mesh.primitives().next()
            .ok_or_else(|| format!("No primitives in mesh of {relative_path}"))?;

        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
        let positions: Vec<[f32; 3]> = reader.read_positions()
            .ok_or_else(|| format!("No positions in {relative_path}"))?
            .collect();
        let indices: Vec<u32> = reader.read_indices()
            .ok_or_else(|| format!("No indices in {relative_path}"))?
            .into_u32()
            .collect();
        let normals: Vec<[f32; 3]> = if let Some(norm_iter) = reader.read_normals() {
            norm_iter.collect()
        } else {
            generate_flat_normals(&positions, &indices)
        };
        let uvs: Vec<[f32; 2]> = if let Some(tc_iter) = reader.read_tex_coords(0) {
            tc_iter.into_f32().collect()
        } else {
            generate_planar_uvs(&positions)
        };
        let vertices: Vec<crate::renderer::mesh::Vertex> = positions.iter().enumerate().map(|(i, pos)| {
            crate::renderer::mesh::Vertex {
                position: *pos,
                normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                uv: uvs.get(i).copied().unwrap_or([0.0, 0.0]),
            }
        }).collect();
        Ok(GltfCpuMesh { vertices, indices })
    }

    /// Like `parse_gltf_mesh` but CPU-only, and ALSO decodes the model's
    /// base-color texture into RGBA8 bytes (v0.904: realistic textured
    /// plants). Returns the geometry plus `Some((rgba, width, height))` when
    /// the first primitive's material carries a
    /// pbrMetallicRoughness.baseColorTexture, `None` when the model has no
    /// material/texture (e.g. the older *_merged.gltf repacks).
    ///
    /// Texture handling:
    /// - relative image URIs (e.g. "textures/grass_medium_02_diff_2k.jpg")
    ///   are resolved from the .gltf's own folder and decoded by
    ///   `gltf::import` (jpg/png via the `image` crate);
    /// - anything larger than 1024x1024 is downscaled (Triangle filter,
    ///   aspect preserved) to keep per-plant VRAM sane;
    /// - the alpha channel is preserved as decoded; a source with NO alpha
    ///   channel (a jpg) that looks like a leaf card on white gets alpha from
    ///   `white_key_alpha_if_cutout`, and is otherwise opaque.
    ///
    /// The caller uploads the pair via
    /// `Mesh::from_vertices(device, &mesh.vertices, &mesh.indices)` +
    /// `Renderer::add_textured_material(.., &rgba, width, height)`, or uses
    /// the `parse_gltf_mesh_textured` convenience below.
    #[cfg(feature = "native")]
    pub fn parse_gltf_mesh_with_texture(
        &self,
        relative_path: &str,
    ) -> Result<(GltfCpuMesh, Option<(Vec<u8>, u32, u32)>), String> {
        let path = self.resolve_model_path(relative_path);
        let (document, buffers, images) = gltf::import(&path)
            .map_err(|e| format!("Failed to load GLTF {}: {e}", path.display()))?;
        let mesh = Self::decode_first_primitive(&document, &buffers, relative_path)?;
        let texture = Self::decode_base_color_texture(&document, &images, relative_path);
        Ok((mesh, texture))
    }

    /// GPU convenience over `parse_gltf_mesh_with_texture`: uploads the
    /// geometry and hands back the ready `Mesh` plus the decoded base-color
    /// texture for `Renderer::add_textured_material`.
    #[cfg(feature = "native")]
    pub fn parse_gltf_mesh_textured(
        &self,
        device: &wgpu::Device,
        relative_path: &str,
    ) -> Result<(crate::renderer::mesh::Mesh, Option<(Vec<u8>, u32, u32)>), String> {
        let (cpu, texture) = self.parse_gltf_mesh_with_texture(relative_path)?;
        let mesh = crate::renderer::mesh::Mesh::from_vertices(device, &cpu.vertices, &cpu.indices);
        Ok((mesh, texture))
    }

    /// As [`parse_gltf_mesh_textured`](Self::parse_gltf_mesh_textured), but
    /// the geometry is first scaled uniformly to stand `target_height` metres
    /// tall (see [`GltfCpuMesh::scale_to_height`]). This is what the home's
    /// machines use, so a model is drawn at the real-world size its `size`
    /// in `data/machines/*.ron` declares instead of at whatever scale it
    /// happened to be authored at.
    #[cfg(feature = "native")]
    pub fn parse_gltf_mesh_textured_fit_height(
        &self,
        device: &wgpu::Device,
        relative_path: &str,
        target_height: f32,
    ) -> Result<(crate::renderer::mesh::Mesh, Option<(Vec<u8>, u32, u32)>), String> {
        let (mut cpu, texture) = self.parse_gltf_mesh_with_texture(relative_path)?;
        cpu.scale_to_height(target_height);
        let mesh = crate::renderer::mesh::Mesh::from_vertices(device, &cpu.vertices, &cpu.indices);
        Ok((mesh, texture))
    }

    /// Find the first primitive's material -> pbrMetallicRoughness
    /// .baseColorTexture -> source image, convert it to RGBA8, and downscale
    /// if oversized. `images` is the decoded-image list `gltf::import`
    /// produced (URI resolution from the gltf's folder already done there).
    /// Returns None for: no material texture, unsupported pixel format, or a
    /// decode inconsistency — all non-fatal (caller renders untextured).
    #[cfg(feature = "native")]
    fn decode_base_color_texture(
        document: &gltf::Document,
        images: &[gltf::image::Data],
        relative_path: &str,
    ) -> Option<(Vec<u8>, u32, u32)> {
        let primitive = document.meshes().next()?.primitives().next()?;
        let info = primitive.material().pbr_metallic_roughness().base_color_texture()?;
        let image_index = info.texture().source().index();
        let data = images.get(image_index)?;
        let rgba = image_data_to_rgba8(data).or_else(|| {
            log::warn!(
                "{relative_path}: base-color texture has unsupported pixel format {:?}; skipping texture",
                data.format
            );
            None
        })?;
        let rgba = white_key_alpha_if_cutout(rgba, format_has_alpha(data.format), relative_path);
        downscale_rgba_if_needed(rgba, data.width, data.height, relative_path)
    }

    /// Load a GLTF/GLB model, extract the first mesh primitive, and return
    /// the mesh index for use in RenderObject. Cached by path — subsequent
    /// calls with the same path skip parsing and GPU upload.
    ///
    /// `relative_path` is resolved relative to `data_dir` (e.g. "models/tree.glb").
    /// The mesh is registered on the provided `Renderer` and its index is returned.
    #[cfg(feature = "native")]
    pub fn load_gltf(
        &mut self,
        renderer: &mut crate::renderer::Renderer,
        relative_path: &str,
    ) -> Result<usize, String> {
        // Return cached mesh index if already loaded
        if let Some(&idx) = self.mesh_cache.get(relative_path) {
            return Ok(idx);
        }

        let path = self.data_dir.join(relative_path);
        let (document, buffers, _images) = gltf::import(&path)
            .map_err(|e| format!("Failed to load GLTF {}: {e}", path.display()))?;

        // Decode the first mesh's first primitive (shared geometry path).
        let cpu = Self::decode_first_primitive(&document, &buffers, relative_path)?;

        let mesh = crate::renderer::mesh::Mesh::from_vertices(
            &renderer.device,
            &cpu.vertices,
            &cpu.indices,
        );
        let mesh_idx = renderer.add_mesh(mesh);

        log::info!(
            "Loaded GLTF: {} ({} verts, {} tris)",
            relative_path,
            cpu.vertices.len(),
            cpu.indices.len() / 3,
        );

        self.mesh_cache.insert(relative_path.to_string(), mesh_idx);
        Ok(mesh_idx)
    }

    // ── Embedded-fallback loaders ─────────────────────────────────────
    // These try disk first (so mods can override), then fall back to
    // compile-time embedded data for fully offline operation.

    /// Read one numeric knob from data/game.csv by its `setting` key.
    ///
    /// game.csv has promised "runtime-tunable gameplay parameters" in its
    /// header since it was created, but no row ever had a reader until
    /// interior gravity (2026-08-12). This is the generic accessor so the
    /// other rows can come alive one consumer at a time. Returns None when
    /// the key is missing or its value is not a number; callers keep their
    /// compiled-in fallback. Cache note: the file watcher invalidates the
    /// "game.csv" cache entry on edit, so a fresh call re-reads the disk.
    #[cfg(feature = "native")]
    pub fn game_setting_f64(&mut self, key: &str) -> Option<f64> {
        #[derive(serde::Deserialize)]
        struct Row {
            setting: String,
            value: String,
        }
        self.load_csv_or_embedded::<Row>("game.csv")
            .ok()?
            .iter()
            .find(|r| r.setting == key)?
            .value
            .trim()
            .parse()
            .ok()
    }

    /// Load CSV: disk first, then embedded fallback.
    /// Results are cached by path.
    #[cfg(feature = "native")]
    pub fn load_csv_or_embedded<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&Vec<T>, String> {
        if self.cache.contains_key(relative_path) {
            return self.cache
                .get(relative_path)
                .and_then(|v| v.downcast_ref::<Vec<T>>())
                .ok_or_else(|| format!("Type mismatch for cached {relative_path}"));
        }

        // Try disk first
        let path = self.data_dir.join(relative_path);
        let records: Vec<T> = if path.exists() {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    match loader::parse_csv(&bytes) {
                        Ok(r) => {
                            log::info!("Loaded {} records from disk: {}", r.len(), relative_path);
                            r
                        }
                        Err(e) => {
                            log::warn!("Disk CSV parse failed for {relative_path}: {e}, trying embedded");
                            Self::parse_embedded_csv(relative_path)?
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Disk read failed for {relative_path}: {e}, trying embedded");
                    Self::parse_embedded_csv(relative_path)?
                }
            }
        } else {
            log::info!("File not on disk, using embedded: {relative_path}");
            Self::parse_embedded_csv(relative_path)?
        };

        self.cache.insert(relative_path.to_string(), Box::new(records));
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<Vec<T>>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load TOML: disk first, then embedded fallback.
    #[cfg(feature = "native")]
    pub fn load_toml_or_embedded<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if self.cache.contains_key(relative_path) {
            return self.cache
                .get(relative_path)
                .and_then(|v| v.downcast_ref::<T>())
                .ok_or_else(|| format!("Type mismatch for cached {relative_path}"));
        }

        let path = self.data_dir.join(relative_path);
        let value: T = if path.exists() {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    match loader::parse_toml(&bytes) {
                        Ok(v) => {
                            log::info!("Loaded TOML from disk: {relative_path}");
                            v
                        }
                        Err(e) => {
                            log::warn!("Disk TOML parse failed for {relative_path}: {e}, trying embedded");
                            Self::parse_embedded_toml(relative_path)?
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Disk read failed for {relative_path}: {e}, trying embedded");
                    Self::parse_embedded_toml(relative_path)?
                }
            }
        } else {
            log::info!("File not on disk, using embedded: {relative_path}");
            Self::parse_embedded_toml(relative_path)?
        };

        self.cache.insert(relative_path.to_string(), Box::new(value));
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load RON: disk first, then embedded fallback.
    #[cfg(feature = "native")]
    pub fn load_ron_or_embedded<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if self.cache.contains_key(relative_path) {
            return self.cache
                .get(relative_path)
                .and_then(|v| v.downcast_ref::<T>())
                .ok_or_else(|| format!("Type mismatch for cached {relative_path}"));
        }

        let path = self.data_dir.join(relative_path);
        let value: T = if path.exists() {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    match loader::parse_ron(&bytes) {
                        Ok(v) => {
                            log::info!("Loaded RON from disk: {relative_path}");
                            v
                        }
                        Err(e) => {
                            log::warn!("Disk RON parse failed for {relative_path}: {e}, trying embedded");
                            Self::parse_embedded_ron(relative_path)?
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Disk read failed for {relative_path}: {e}, trying embedded");
                    Self::parse_embedded_ron(relative_path)?
                }
            }
        } else {
            log::info!("File not on disk, using embedded: {relative_path}");
            Self::parse_embedded_ron(relative_path)?
        };

        self.cache.insert(relative_path.to_string(), Box::new(value));
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Load JSON: disk first, then embedded fallback.
    #[cfg(feature = "native")]
    pub fn load_json_or_embedded<T: DeserializeOwned + Send + Sync + 'static>(
        &mut self,
        relative_path: &str,
    ) -> Result<&T, String> {
        if self.cache.contains_key(relative_path) {
            return self.cache
                .get(relative_path)
                .and_then(|v| v.downcast_ref::<T>())
                .ok_or_else(|| format!("Type mismatch for cached {relative_path}"));
        }

        let path = self.data_dir.join(relative_path);
        let value: T = if path.exists() {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    match loader::parse_json(&bytes) {
                        Ok(v) => {
                            log::info!("Loaded JSON from disk: {relative_path}");
                            v
                        }
                        Err(e) => {
                            log::warn!("Disk JSON parse failed for {relative_path}: {e}, trying embedded");
                            Self::parse_embedded_json(relative_path)?
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Disk read failed for {relative_path}: {e}, trying embedded");
                    Self::parse_embedded_json(relative_path)?
                }
            }
        } else {
            log::info!("File not on disk, using embedded: {relative_path}");
            Self::parse_embedded_json(relative_path)?
        };

        self.cache.insert(relative_path.to_string(), Box::new(value));
        self.cache
            .get(relative_path)
            .and_then(|v| v.downcast_ref::<T>())
            .ok_or_else(|| format!("Type mismatch for cached {relative_path}"))
    }

    /// Get raw embedded text for a path (useful for non-deserialized access).
    pub fn get_embedded_str(relative_path: &str) -> Option<&'static str> {
        let text = embedded_data::get_embedded(relative_path)?;
        embedded_data::note_builtin_copy(relative_path, "AssetManager::get_embedded_str served the built-in copy");
        Some(text)
    }

    // ── Private embedded parse helpers ──────────────────────────────

    fn parse_embedded_csv<T: DeserializeOwned>(path: &str) -> Result<Vec<T>, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        // The caller's disk copy was missing, unreadable or did not parse (it logged
        // which): say that this run serves the built-in copy (BUG-133).
        embedded_data::note_builtin_copy(path, "AssetManager fell back to the built-in copy (see the line above)");
        loader::parse_csv(text.as_bytes())
    }

    fn parse_embedded_toml<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        // The caller's disk copy was missing, unreadable or did not parse (it logged
        // which): say that this run serves the built-in copy (BUG-133).
        embedded_data::note_builtin_copy(path, "AssetManager fell back to the built-in copy (see the line above)");
        loader::parse_toml(text.as_bytes())
    }

    fn parse_embedded_ron<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        // The caller's disk copy was missing, unreadable or did not parse (it logged
        // which): say that this run serves the built-in copy (BUG-133).
        embedded_data::note_builtin_copy(path, "AssetManager fell back to the built-in copy (see the line above)");
        loader::parse_ron(text.as_bytes())
    }

    fn parse_embedded_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        // The caller's disk copy was missing, unreadable or did not parse (it logged
        // which): say that this run serves the built-in copy (BUG-133).
        embedded_data::note_builtin_copy(path, "AssetManager fell back to the built-in copy (see the line above)");
        loader::parse_json(text.as_bytes())
    }

    /// Invalidate a cached entry (called by hot-reload on file change).
    pub fn invalidate(&mut self, relative_path: &str) {
        if self.cache.remove(relative_path).is_some() {
            log::info!("Cache invalidated: {}", relative_path);
        }
    }

    /// Store pre-parsed data (used by WASM where data arrives via fetch).
    pub fn store<T: Send + Sync + 'static>(&mut self, key: &str, value: T) {
        self.cache.insert(key.to_string(), Box::new(value));
    }

    /// Retrieve cached data by key.
    pub fn get<T: 'static>(&self, key: &str) -> Option<&T> {
        self.cache.get(key).and_then(|v| v.downcast_ref::<T>())
    }
}

/// Convert a `gltf::import`-decoded image to tightly-packed RGBA8 bytes.
/// jpg decodes as R8G8B8 (alpha filled with 255), png may carry real alpha
/// (R8G8B8A8, kept as-is). 16-bit / float formats return None — no plant
/// asset uses them and expanding them here would be dead code.
#[cfg(feature = "native")]
fn image_data_to_rgba8(data: &gltf::image::Data) -> Option<Vec<u8>> {
    use gltf::image::Format;
    let pixel_count = data.width as usize * data.height as usize;
    let px = &data.pixels;
    match data.format {
        Format::R8G8B8A8 => Some(px.clone()),
        Format::R8G8B8 => {
            let mut out = Vec::with_capacity(pixel_count * 4);
            for c in px.chunks_exact(3) {
                out.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
            Some(out)
        }
        Format::R8 => {
            // Grayscale: replicate luma, opaque alpha.
            let mut out = Vec::with_capacity(pixel_count * 4);
            for &l in px.iter() {
                out.extend_from_slice(&[l, l, l, 255]);
            }
            Some(out)
        }
        Format::R8G8 => {
            // Luma + alpha.
            let mut out = Vec::with_capacity(pixel_count * 4);
            for c in px.chunks_exact(2) {
                out.extend_from_slice(&[c[0], c[0], c[0], c[1]]);
            }
            Some(out)
        }
        _ => None,
    }
}

/// Cap plant textures at 1024x1024: anything larger is resized (aspect
/// preserved, Triangle filter) so a 2k Poly Haven diff map costs 4 MB of
/// VRAM instead of 16.
#[cfg(feature = "native")]
fn downscale_rgba_if_needed(
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    relative_path: &str,
) -> Option<(Vec<u8>, u32, u32)> {
    const MAX_DIM: u32 = 1024;
    if width <= MAX_DIM && height <= MAX_DIM {
        return Some((rgba, width, height));
    }
    let scale = MAX_DIM as f32 / width.max(height) as f32;
    let new_w = ((width as f32 * scale).round() as u32).max(1);
    let new_h = ((height as f32 * scale).round() as u32).max(1);
    let img = match image::RgbaImage::from_raw(width, height, rgba) {
        Some(i) => i,
        None => {
            // Only reachable on a byte-length mismatch — treat as no texture.
            log::warn!("{relative_path}: rgba byte length does not match {width}x{height}; skipping texture");
            return None;
        }
    };
    let resized = image::imageops::resize(&img, new_w, new_h, image::imageops::FilterType::Triangle);
    log::info!("{relative_path}: base-color texture downscaled {width}x{height} -> {new_w}x{new_h}");
    Some((resized.into_raw(), new_w, new_h))
}

/// Whether a decoded glTF image came from a source with an alpha channel
/// (2026-10-03). `gltf::import` decodes an RGBA PNG to `R8G8B8A8` and a JPEG
/// (which cannot hold alpha) to `R8G8B8`, so the format says what the file
/// itself could express. The two-channel formats are luminance plus alpha.
#[cfg(feature = "native")]
fn format_has_alpha(format: gltf::image::Format) -> bool {
    use gltf::image::Format;
    matches!(
        format,
        Format::R8G8 | Format::R8G8B8A8 | Format::R16G16 | Format::R16G16B16A16 | Format::R32G32B32A32FLOAT
    )
}

/// White-key alpha recovery for cutout foliage (v0.911). Photoscan twig and
/// leaf-card textures shipped as JPEG, which has no alpha channel, so the
/// white background around each twig cluster rendered as SOLID white slabs:
/// a probe capture showed every conifer wrapped in pale boxes. When such a
/// texture has a significant near-white fraction (the tell of a cutout
/// sheet on white), alpha is derived from brightness: the white background
/// fades to transparent, dark foliage stays opaque. Textures that are
/// genuinely bright all over (sand, pot ceramic) are left alone by the
/// fraction test.
///
/// ONLY a source with no alpha channel is keyed (`source_has_alpha`, from
/// `format_has_alpha`; 2026-10-03). A texture that carries an alpha channel
/// has already said what is transparent, even when the answer is "nothing".
/// Until then the test was "no texel below 250 alpha", which an opaque RGBA
/// texture passes too: the OBJ converter's flat-colour palettes, whose
/// unused blocks are white fill, were keyed, and the bed-side cabinet's
/// pale grey faded to alpha 0.30 and never drew (crop_palette_tests). The
/// conifers that first needed this ship alpha-carrying PNGs since v0.913,
/// so today it is kept for any JPEG leaf card that arrives, not for them.
#[cfg(feature = "native")]
fn white_key_alpha_if_cutout(mut rgba: Vec<u8>, source_has_alpha: bool, relative_path: &str) -> Vec<u8> {
    let n = rgba.len() / 4;
    if n == 0 || source_has_alpha {
        return rgba;
    }
    let mut near_white = 0usize;
    for px in rgba.chunks_exact(4) {
        // Near-white AND low-saturation: background, not bright foliage.
        let mx = px[0].max(px[1]).max(px[2]);
        let mn = px[0].min(px[1]).min(px[2]);
        if mn >= 210 && (mx - mn) < 28 {
            near_white += 1;
        }
    }
    if near_white * 100 < n * 12 {
        return rgba;
    }
    for px in rgba.chunks_exact_mut(4) {
        let mn = px[0].min(px[1]).min(px[2]) as f32;
        // 1 at min-channel 170, 0 at 225: a smooth key with JPEG-artifact
        // headroom on both sides.
        let t = ((mn - 170.0) / 55.0).clamp(0.0, 1.0);
        px[3] = (255.0 * (1.0 - t * t * (3.0 - 2.0 * t))) as u8;
    }
    log::info!(
        "{relative_path}: cutout texture detected ({}% near-white) - alpha recovered by white key",
        near_white * 100 / n
    );
    rgba
}

/// Generate flat normals when GLTF model has none.
/// Each triangle face gets a uniform normal from the cross product of its edges.
#[cfg(feature = "native")]
fn generate_flat_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0_f32; 3]; positions.len()];

    for tri in indices.chunks(3) {
        if tri.len() < 3 { break; }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let p0 = glam::Vec3::from(positions[i0]);
        let p1 = glam::Vec3::from(positions[i1]);
        let p2 = glam::Vec3::from(positions[i2]);
        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let n = edge1.cross(edge2).normalize_or_zero();
        let n_arr = n.to_array();
        // Accumulate — vertices shared across faces get averaged normals
        for &idx in &[i0, i1, i2] {
            normals[idx][0] += n_arr[0];
            normals[idx][1] += n_arr[1];
            normals[idx][2] += n_arr[2];
        }
    }

    // Normalize accumulated normals
    for n in &mut normals {
        let v = glam::Vec3::from(*n);
        let norm = v.normalize_or_zero();
        *n = norm.to_array();
    }

    normals
}

/// Generate simple planar UVs when GLTF model has none.
/// Maps XZ bounding box to [0,1] range.
#[cfg(feature = "native")]
fn generate_planar_uvs(positions: &[[f32; 3]]) -> Vec<[f32; 2]> {
    if positions.is_empty() {
        return Vec::new();
    }

    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_z = f32::MAX;
    let mut max_z = f32::MIN;

    for p in positions {
        min_x = min_x.min(p[0]);
        max_x = max_x.max(p[0]);
        min_z = min_z.min(p[2]);
        max_z = max_z.max(p[2]);
    }

    let range_x = (max_x - min_x).max(1e-6);
    let range_z = (max_z - min_z).max(1e-6);

    positions.iter().map(|p| {
        [(p[0] - min_x) / range_x, (p[2] - min_z) / range_z]
    }).collect()
}

#[cfg(all(test, feature = "native"))]
mod gltf_texture_tests {
    use super::*;

    /// Load the smallest split plant variant (grass clump v1, 714 tris,
    /// produced by `node scripts/repack-plant-gltf.js --split --all`) plus
    /// its base-color texture and sanity-check both. The 2k source jpg must
    /// come back downscaled to <= 1024 with 4 bytes per pixel. Paths resolve
    /// through the repo checkout: data_dir = <repo>/data, so the
    /// parent-of-data fallback reaches <repo>/assets/... exactly like a dev
    /// checkout at runtime.
    #[test]
    fn loads_grass_variant_mesh_and_texture() {
        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manager = AssetManager::new(repo_root.join("data"));
        let (mesh, texture) = manager
            .parse_gltf_mesh_with_texture("assets/models/plants/grass_medium_02/grass_medium_02_v1.gltf")
            .expect("grass variant v1 should load");

        // Geometry sanity
        assert!(!mesh.vertices.is_empty(), "no vertices decoded");
        assert!(!mesh.indices.is_empty(), "no indices decoded");
        assert_eq!(mesh.indices.len() % 3, 0, "index count not a multiple of 3");
        let vert_count = mesh.vertices.len() as u32;
        assert!(
            mesh.indices.iter().all(|&i| i < vert_count),
            "index out of range ({} vertices)",
            vert_count
        );

        // Texture sanity: present, capped at 1024, tightly packed RGBA8
        let (rgba, width, height) = texture.expect("variant should carry a base-color texture");
        assert!(width > 0 && height > 0, "degenerate texture {width}x{height}");
        assert!(
            width <= 1024 && height <= 1024,
            "texture not downscaled: {width}x{height}"
        );
        assert_eq!(
            rgba.len(),
            (width * height * 4) as usize,
            "rgba byte length != w*h*4"
        );
    }
}

#[cfg(all(test, feature = "native"))]
mod crop_palette_tests {
    use super::*;

    /// The darkest colour a crop model may show the shader, as linear
    /// luminance. Living plant tissue is far brighter: a dark leaf is around
    /// 0.05 and the darkest part of the crop pack (beetroot, read correctly)
    /// is 0.035. Before 2026-10-03 the lettuce leaves reached the shader at
    /// 0.015 and the beetroot at 0.003, which is what black looks like.
    const DARKEST_CROP_LUMINANCE: f32 = 0.025;

    /// What the GPU makes of one byte of an Rgba8UnormSrgb texture: the
    /// sRGB decode to linear light.
    fn srgb_to_linear(b: u8) -> f32 {
        let c = b as f32 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// The colour the shader reads at `uv`, the way the engine's albedo
    /// sampler takes it (renderer/mod.rs "Albedo Texture Sampler": linear
    /// filtering, U repeats, V clamps, filtered after the sRGB decode):
    /// linear RGB and alpha.
    fn sample(rgba: &[u8], w: u32, h: u32, uv: [f32; 2]) -> ([f32; 3], f32) {
        let (fx, fy) = (uv[0] * w as f32 - 0.5, uv[1] * h as f32 - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let texel = |x: f32, y: f32| {
            let x = (x as i64).rem_euclid(w as i64) as usize;
            let y = (y as i64).clamp(0, h as i64 - 1) as usize;
            let p = &rgba[(y * w as usize + x) * 4..][..4];
            ([srgb_to_linear(p[0]), srgb_to_linear(p[1]), srgb_to_linear(p[2])], p[3] as f32 / 255.0)
        };
        let mut rgb = [0.0f32; 3];
        let mut a = 0.0f32;
        for (dx, dy, wgt) in [(0.0, 0.0, (1.0 - tx) * (1.0 - ty)), (1.0, 0.0, tx * (1.0 - ty)), (0.0, 1.0, (1.0 - tx) * ty), (1.0, 1.0, tx * ty)] {
            let (c, al) = texel(x0 + dx, y0 + dy);
            for k in 0..3 {
                rgb[k] += c[k] * wgt;
            }
            a += al * wgt;
        }
        (rgb, a)
    }

    /// EVERY CROP STAGE MODEL SHOWS THE SHADER A PLANT'S COLOURS, NOT BLACK
    /// (2026-10-03, the black sprouts in the greenhouse towers).
    ///
    /// The crop models carry their colours in a small palette texture, made
    /// by scripts/obj-to-plant-gltf.js from the pack's MTL colours. The pack's
    /// numbers are linear light, and the converter wrote them into the PNG
    /// byte for byte; the engine decodes that PNG as sRGB, so every colour
    /// came out far too dark: lettuce, beet, pumpkin and watermelon leaves
    /// all but black. Towers showed it first because their crops are leafy
    /// greens; the beds' wheat and tomatoes were merely dull.
    ///
    /// This loads each crop model through the engine's own loader, reads the
    /// texture under every vertex exactly as the GPU sampler would, and
    /// requires every colour a model shows to be brighter than any plant
    /// tissue could be dark. It also checks no vertex lands on a texel the
    /// white-key cutout made transparent (the shader discards below 0.35
    /// alpha, so that face would vanish), and that every converted model
    /// says which colour space its MTL was read in, so a palette made
    /// before the fix cannot slip back in.
    ///
    /// Red checks, run 2026-10-03: against the palettes as they were, this
    /// fails with 75 of the 102 crop models too dark (lettuce_1 at 0.0147,
    /// beet_1 at 0.0032) and all 102 unstamped; with those same palettes
    /// stamped `srgb` (the stamp present, the pixels unchanged) it still
    /// fails on the same 75, so the luminance gate holds on its own.
    #[test]
    fn every_crop_palette_reaches_the_shader_as_plant_colour() {
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manager = AssetManager::new(repo.join("data"));
        let plants = repo.join("assets/models/plants");
        let mut checked = 0usize;
        let mut too_dark = Vec::new();
        let mut problems = Vec::new();
        let mut names: Vec<String> = std::fs::read_dir(&plants)
            .expect("assets/models/plants is in the checkout")
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect();
        names.sort();
        for name in names {
            let rel = format!("assets/models/plants/{name}/{name}.gltf");
            let Ok(text) = std::fs::read_to_string(repo.join(&rel)) else { continue };
            let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
            // Only the converter's flat-colour models: the photoscans carry
            // real photographs, where dark texels are shadowed bark.
            if !json["asset"]["generator"].as_str().unwrap_or("").starts_with("obj-to-plant-gltf.js") {
                continue;
            }
            if !json["asset"]["extras"]["kd_space"].is_string() {
                problems.push(format!(
                    "{name}: no asset.extras.kd_space, so it was converted before the converter knew the pack's \
                     colour space (node scripts/obj-to-plant-gltf.js --kd linear --restamp {rel})"
                ));
            }
            let (cpu, texture) = manager.parse_gltf_mesh_with_texture(&rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
            let (rgba, w, h) = texture.unwrap_or_else(|| panic!("{rel}: the palette texture did not load"));
            let mut darkest = f32::MAX;
            for v in &cpu.vertices {
                let (c, a) = sample(&rgba, w, h, v.uv);
                if a < 0.35 {
                    problems.push(format!("{name}: a vertex at uv {:?} samples alpha {a:.2}, which the shader discards", v.uv));
                    break;
                }
                darkest = darkest.min(0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]);
            }
            if darkest < DARKEST_CROP_LUMINANCE {
                too_dark.push(format!("{name} {darkest:.4}"));
            }
            checked += 1;
        }
        // The pack is 102 models; far fewer means the walk found almost
        // nothing and this gate would pass while proving nothing.
        assert!(checked >= 100, "only {checked} converted crop models found under {}", plants.display());
        // One report for both lists, so a stamped-but-wrong palette and an
        // unstamped one each say what they are.
        assert!(
            too_dark.is_empty() && problems.is_empty(),
            "\n{} of {checked} crop models show the shader a colour darker than {DARKEST_CROP_LUMINANCE} linear \
             luminance (near black; each model's darkest): {}. Is the palette sRGB-encoded? See \
             scripts/obj-to-plant-gltf.js --kd.\n{} other problem(s):\n{}\n",
            too_dark.len(),
            too_dark.join(", "),
            problems.len(),
            problems.join("\n")
        );
    }

    /// Every model scripts/obj-to-plant-gltf.js made, crops and furniture,
    /// as repo-relative .gltf paths (the converter's flat-colour palette
    /// models; its generator line says so).
    fn converted_models(repo: &std::path::Path) -> Vec<String> {
        let mut out = Vec::new();
        for dir in ["assets/models/plants", "assets/models/furniture"] {
            let mut names: Vec<String> = std::fs::read_dir(repo.join(dir))
                .unwrap_or_else(|e| panic!("{dir} is in the checkout: {e}"))
                .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
                .collect();
            names.sort();
            for name in names {
                let rel = format!("{dir}/{name}/{name}.gltf");
                let Ok(text) = std::fs::read_to_string(repo.join(&rel)) else { continue };
                let json: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{rel}: {e}"));
                if json["asset"]["generator"].as_str().unwrap_or("").starts_with("obj-to-plant-gltf.js") {
                    out.push(rel);
                }
            }
        }
        out
    }

    /// NO CONVERTED MODEL LOSES A FACE TO THE WHITE KEY (2026-10-03).
    ///
    /// The white key (`white_key_alpha_if_cutout`) makes near-white texels
    /// transparent, for photographed leaf cards on a white background, and
    /// the shader discards anything under 0.35 alpha. It used to fire on ANY
    /// fully opaque texture with enough near-white in it, and the
    /// converter's palettes qualify: a palette's unused blocks are white
    /// fill, a quarter of the texels in 15 of the 117. The bed-side
    /// cabinet's pale grey (205, 205, 205) then faded to alpha 0.30, and
    /// the six vertices on it (data/machines/home.ron, the bedroom) never
    /// drew. The key now only touches a texture whose source has no alpha
    /// channel, and these palettes are RGBA.
    ///
    /// This loads every converted model through the engine's loader and
    /// samples the texture under every vertex the way the GPU does.
    ///
    /// Red check, run 2026-10-03 before the fix: fails on
    /// cabinetbeddrawertable, 6 vertices.
    #[test]
    fn no_converted_model_has_a_vertex_on_a_cut_out_texel() {
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manager = AssetManager::new(repo.join("data"));
        let models = converted_models(&repo);
        // 102 crop models and 15 furniture models; far fewer means the walk
        // found almost nothing and this would pass while proving nothing.
        assert!(models.len() >= 115, "only {} converted models found", models.len());
        let mut cut = Vec::new();
        for rel in &models {
            let (cpu, texture) = manager.parse_gltf_mesh_with_texture(rel).unwrap_or_else(|e| panic!("{rel}: {e}"));
            let (rgba, w, h) = texture.unwrap_or_else(|| panic!("{rel}: the palette texture did not load"));
            let lost = cpu.vertices.iter().filter(|v| sample(&rgba, w, h, v.uv).1 < 0.35).count();
            if lost > 0 {
                cut.push(format!("{rel}: {lost} of {} vertices", cpu.vertices.len()));
            }
        }
        assert!(
            cut.is_empty(),
            "\nthese models have vertices on texels the white key made transparent, so the shader discards \
             their faces:\n{}\n",
            cut.join("\n")
        );
    }

    /// A PHOTOGRAPHED LEAF CARD WITH NO ALPHA CHANNEL IS STILL CUT OUT, AND
    /// THE SAME PICTURE WITH ONE IS LEFT ALONE (2026-10-03).
    ///
    /// The white key exists for leaf and twig photographs shipped as JPEG
    /// (v0.911: the photoscan conifers stood in pale boxes). A JPEG cannot
    /// carry alpha, so brightness is all there is to go on. A PNG with an
    /// alpha channel has said what is transparent, even when its answer is
    /// "nothing". So this builds one picture, a dark leaf on a white ground,
    /// writes it both ways, loads each through the engine's real glTF path
    /// (gltf::import decodes the JPEG to RGB and the PNG to RGBA, which is
    /// the format the rule reads), and checks the JPEG comes back with its
    /// ground cut away and the PNG comes back untouched.
    ///
    /// The conifers that motivated the key now ship alpha-carrying PNGs
    /// (v0.913, *_diff_a_1k.png), so the last check is that one of them
    /// still arrives with its real cut-out intact.
    #[test]
    fn a_jpeg_leaf_card_on_white_is_cut_out_and_its_rgba_twin_is_not() {
        let dir = std::env::temp_dir().join(format!("white_key_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // One triangle: three positions (36 bytes) then three u32 indices.
        let mut bin = Vec::new();
        for p in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for c in p {
                bin.extend_from_slice(&c.to_le_bytes());
            }
        }
        for i in [0u32, 1, 2] {
            bin.extend_from_slice(&i.to_le_bytes());
        }
        std::fs::write(dir.join("leaf.bin"), &bin).unwrap();
        let gltf = |image: &str| {
            format!(
                r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
                "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"indices":1,"material":0}}]}}],
                "materials":[{{"pbrMetallicRoughness":{{"baseColorTexture":{{"index":0}}}}}}],
                "textures":[{{"source":0}}],"images":[{{"uri":"{image}"}}],
                "buffers":[{{"uri":"leaf.bin","byteLength":48}}],
                "bufferViews":[{{"buffer":0,"byteOffset":0,"byteLength":36}},{{"buffer":0,"byteOffset":36,"byteLength":12}}],
                "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}},
                {{"bufferView":1,"componentType":5125,"count":3,"type":"SCALAR"}}]}}"#
            )
        };
        // The picture: a dark green leaf, an ellipse in the middle, on a
        // white ground with a little texture to it, the way a photographed
        // backdrop is never one flat value. About three quarters is ground.
        const S: u32 = 64;
        let leaf = |x: u32, y: u32| {
            let (dx, dy) = (x as f32 - 31.5, y as f32 - 31.5);
            (dx / 20.0).powi(2) + (dy / 14.0).powi(2) <= 1.0
        };
        let rgb = |x: u32, y: u32| -> [u8; 3] {
            if leaf(x, y) {
                [38, 88, 30]
            } else {
                let g = 246 + ((x * 7 + y * 13) % 9) as u8;
                [g, g, g.saturating_sub(2)]
            }
        };
        image::RgbImage::from_fn(S, S, |x, y| image::Rgb(rgb(x, y)))
            .save_with_format(dir.join("leaf.jpg"), image::ImageFormat::Jpeg)
            .unwrap();
        image::RgbaImage::from_fn(S, S, |x, y| {
            let [r, g, b] = rgb(x, y);
            image::Rgba([r, g, b, 255])
        })
        .save_with_format(dir.join("leaf.png"), image::ImageFormat::Png)
        .unwrap();
        std::fs::write(dir.join("leaf_jpg.gltf"), gltf("leaf.jpg")).unwrap();
        std::fs::write(dir.join("leaf_png.gltf"), gltf("leaf.png")).unwrap();

        let manager = AssetManager::new(dir.clone());
        let alpha = |rgba: &[u8], x: u32, y: u32| rgba[((y * S + x) * 4 + 3) as usize];

        let (_, jpg) = manager.parse_gltf_mesh_with_texture("leaf_jpg.gltf").expect("the JPEG model loads");
        let (jpg, w, h) = jpg.expect("the JPEG texture decodes");
        assert_eq!((w, h), (S, S));
        assert!(alpha(&jpg, 31, 31) > 240, "the leaf itself must stay opaque: alpha {}", alpha(&jpg, 31, 31));
        for (x, y) in [(2, 2), (61, 3), (4, 60), (60, 60)] {
            assert!(
                (alpha(&jpg, x, y) as f32) < 0.35 * 255.0,
                "the white ground at ({x}, {y}) must be cut out of a JPEG leaf card: alpha {}",
                alpha(&jpg, x, y)
            );
        }

        let (_, png) = manager.parse_gltf_mesh_with_texture("leaf_png.gltf").expect("the PNG model loads");
        let (png, _, _) = png.expect("the PNG texture decodes");
        let touched = png.chunks_exact(4).filter(|p| p[3] != 255).count();
        assert_eq!(touched, 0, "an RGBA texture's alpha is its own: {touched} texels were keyed");

        std::fs::remove_dir_all(&dir).ok();

        // A real conifer leaf card: its transparency comes from its own
        // alpha channel and arrives as authored.
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manager = AssetManager::new(repo.join("data"));
        let (_, fir) = manager
            .parse_gltf_mesh_with_texture("assets/models/plants/fir_sapling/fir_sapling_v1.gltf")
            .expect("the fir sapling loads");
        let (fir, _, _) = fir.expect("the fir sapling's twig card decodes");
        let authored = image::open(repo.join("assets/models/plants/fir_sapling/textures/fir_sapling_twigs_diff_a_1k.png"))
            .expect("the fir twig card is in the checkout")
            .to_rgba8();
        assert_eq!(fir.len(), authored.as_raw().len(), "the twig card arrived at another size");
        let changed = fir.chunks_exact(4).zip(authored.as_raw().chunks_exact(4)).filter(|(a, b)| a[3] != b[3]).count();
        assert_eq!(changed, 0, "the fir twig card's alpha must arrive exactly as authored: {changed} texels differ");
        // And it is a cut-out at all: sparse twigs (about 7% of the card)
        // on clear ground.
        let n = fir.len() / 4;
        let clear = fir.chunks_exact(4).filter(|p| (p[3] as f32) < 0.35 * 255.0).count();
        assert!(clear > n / 2 && n - clear > n / 50, "the fir twig card: {clear} of {n} texels clear");
    }

    /// NO CONVERTED MODEL HAS A TRIANGLE WOUND AGAINST ITS OWN NORMAL
    /// (2026-10-03).
    ///
    /// The GPU decides which side of a triangle is its front by the order
    /// its corners come in, culls the back, and lights the front with the
    /// normals stored on its corners. The converter used to cut every OBJ
    /// face into a fan from its first corner, and on a concave face one
    /// fan triangle comes out with its corners the other way round: culled
    /// from the side its normals face, a small hole, and lit as though
    /// facing away when seen from the other side. 68 such quads sat on the
    /// pumpkin and watermelon vines; they were re-cut, and the converter
    /// ear-clips a concave face now (`triangulate` in the script).
    ///
    /// A triangle is wound against its normal when the angle between its
    /// corner-order normal and its stored normal is past about 105 degrees
    /// (cosine below -0.25). Not past 90: one twisted strip on
    /// bushberries_3, whose two halves face about 120 degrees apart under
    /// one averaged normal, sits at 93 degrees, and no cut of its four
    /// corners does better. The 68 sat between 111 and 180 degrees.
    ///
    /// Red check, run 2026-10-03 against the models before the re-cut:
    /// fails with 68 triangles in the 8 pumpkin and watermelon models.
    #[test]
    fn no_converted_model_has_a_triangle_wound_against_its_normal() {
        let repo = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manager = AssetManager::new(repo.join("data"));
        let models = converted_models(&repo);
        assert!(models.len() >= 115, "only {} converted models found", models.len());
        let mut backwards = Vec::new();
        let mut triangles = 0usize;
        for rel in &models {
            let cpu = manager.parse_gltf_mesh_with_texture(rel).unwrap_or_else(|e| panic!("{rel}: {e}")).0;
            let mut bad = 0usize;
            for tri in cpu.indices.chunks_exact(3) {
                let v = |k: usize| &cpu.vertices[tri[k] as usize];
                let p = |k: usize| glam::Vec3::from(v(k).position);
                let wound = (p(1) - p(0)).cross(p(2) - p(0));
                let stored = glam::Vec3::from(v(0).normal) + glam::Vec3::from(v(1).normal) + glam::Vec3::from(v(2).normal);
                // A flat triangle (three corners in a line, which the fan
                // writes for an edge with a corner in its middle) faces
                // nowhere and draws nothing; in f32 its cross product is
                // rounding noise pointing anywhere. Skip it by the sine of
                // its corner angle rather than an absolute size, since the
                // models' triangles span millimetres to metres.
                let edges = (p(1) - p(0)).length() * (p(2) - p(0)).length();
                if wound.length() <= 1e-4 * edges || stored.length() < 1e-6 {
                    continue;
                }
                triangles += 1;
                if wound.normalize().dot(stored.normalize()) < -0.25 {
                    bad += 1;
                }
            }
            if bad > 0 {
                backwards.push(format!("{rel}: {bad}"));
            }
        }
        assert!(triangles > 60_000, "only {triangles} triangles walked");
        assert!(
            backwards.is_empty(),
            "\ntriangles wound against their own normals (culled from the side they face; re-cut the face, see \
             triangulate in scripts/obj-to-plant-gltf.js):\n{}\n",
            backwards.join("\n")
        );
    }
}

#[cfg(all(test, feature = "native"))]
mod model_scale_tests {
    use super::*;
    use crate::renderer::mesh::Vertex;

    fn mesh_of_height(h: f32) -> GltfCpuMesh {
        // A unit-square column standing on y = 0, like the furniture pack.
        let v = |x: f32, y: f32, z: f32| Vertex {
            position: [x, y, z],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        };
        GltfCpuMesh {
            vertices: vec![v(-0.5, 0.0, -0.5), v(0.5, 0.0, 0.5), v(0.5, h, 0.5)],
            indices: vec![0, 1, 2],
        }
    }

    #[test]
    fn the_authored_height_is_the_y_extent() {
        assert!((mesh_of_height(0.46).authored_height() - 0.46).abs() < 1e-6);
        let empty = GltfCpuMesh { vertices: vec![], indices: vec![] };
        assert_eq!(empty.authored_height(), 0.0, "an empty mesh has no height, not an infinite one");
    }

    #[test]
    fn fitting_the_height_scales_uniformly_and_keeps_the_base_on_the_floor() {
        let mut m = mesh_of_height(0.46);
        let s = m.scale_to_height(0.90);
        assert!((s - 0.90 / 0.46).abs() < 1e-5, "factor {s}");
        assert!((m.authored_height() - 0.90).abs() < 1e-5, "the chair now stands 0.9 m");
        // Uniform: the footprint grew by the same factor, and nothing sank
        // through the floor.
        let lowest = m.vertices.iter().fold(f32::INFINITY, |a, v| a.min(v.position[1]));
        assert!(lowest.abs() < 1e-6, "base left the floor: {lowest}");
        let widest = m.vertices.iter().fold(0.0f32, |a, v| a.max(v.position[0].abs()));
        assert!((widest - 0.5 * s).abs() < 1e-5, "x was not scaled with y: {widest}");
    }

    #[test]
    fn a_nonsense_target_leaves_the_model_exactly_as_authored() {
        for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            let mut m = mesh_of_height(0.46);
            assert_eq!(m.scale_to_height(bad), 1.0, "target {bad} should be refused");
            assert!((m.authored_height() - 0.46).abs() < 1e-6, "target {bad} changed the mesh");
        }
        // A flat model (a rug is nearly one) must not be blown up by a
        // division by almost nothing.
        let mut flat = GltfCpuMesh { vertices: vec![], indices: vec![] };
        assert_eq!(flat.scale_to_height(2.0), 1.0);
    }

    /// EVERY MODELLED MACHINE IN THE SHIPPED HOME IS DRAWN AT THE SIZE ITS
    /// DATA DECLARES.
    ///
    /// The operator, 2026-09-19, standing in his own bedroom: "Seems like
    /// either all the furniture is small or I'm just really big." He was
    /// right. `size` in data/machines/*.ron only ever built the PRIMITIVE
    /// fallback box; a machine that declared a model got that model at
    /// whatever scale it was authored at, and this pack is authored at about
    /// half real scale. A chair rendered 0.46 m tall, a bookcase 0.88 m and a
    /// desk 0.38 m, next to a player whose eyes are at 1.7 m.
    ///
    /// The second assertion is the one that keeps this honest: if every model
    /// were already the right height the fit would be a no-op and this test
    /// would pass while proving nothing, so it requires that at least one
    /// model really was badly wrong as authored.
    #[test]
    fn every_modelled_machine_in_the_home_is_drawn_at_its_declared_height() {
        let repo_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let home = crate::machines::MachineHome::load(&repo_root.join("data/machines/home.ron"))
            .expect("the shipped home loads");
        let manager = AssetManager::new(repo_root.join("data"));

        let mut checked = 0usize;
        let mut worst: (f32, String) = (0.0, String::new());
        for (id, def) in home.catalog.iter() {
            let Some(model) = def.model.as_deref() else { continue };
            let target = def.size.1;
            let (mut cpu, _tex) = manager
                .parse_gltf_mesh_with_texture(model)
                .unwrap_or_else(|e| panic!("machine {id}: model {model} did not load: {e}"));
            let authored = cpu.authored_height();
            cpu.scale_to_height(target);
            let drawn = cpu.authored_height();
            assert!(
                (drawn - target).abs() <= target * 0.01,
                "machine {id} declares {target} m tall but would be drawn {drawn} m"
            );
            let err = if target > 0.0 { (authored - target).abs() / target } else { 0.0 };
            if err > worst.0 {
                worst = (err, format!("{id} ({model}): authored {authored} m, declared {target} m"));
            }
            checked += 1;
        }

        assert!(checked >= 10, "only {checked} modelled machines found; this gate is checking almost nothing");
        assert!(
            worst.0 > 0.25,
            "every model was already within 25 percent of its declared height, so fitting is a \
             no-op and this test proves nothing. Worst seen: {}",
            worst.1
        );
    }
}
