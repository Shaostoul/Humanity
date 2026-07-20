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
    /// - the alpha channel is preserved as decoded (jpg sources decode with
    ///   alpha = 255; leaf silhouettes on these photoscans are real geometry).
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
        embedded_data::get_embedded(relative_path)
    }

    // ── Private embedded parse helpers ──────────────────────────────

    fn parse_embedded_csv<T: DeserializeOwned>(path: &str) -> Result<Vec<T>, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        loader::parse_csv(text.as_bytes())
    }

    fn parse_embedded_toml<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        loader::parse_toml(text.as_bytes())
    }

    fn parse_embedded_ron<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
        loader::parse_ron(text.as_bytes())
    }

    fn parse_embedded_json<T: DeserializeOwned>(path: &str) -> Result<T, String> {
        let text = embedded_data::get_embedded(path)
            .ok_or_else(|| format!("No embedded fallback for {path}"))?;
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
