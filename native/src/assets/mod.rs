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

        // Find the first mesh with at least one primitive
        let gltf_mesh = document.meshes().next()
            .ok_or_else(|| format!("No meshes in {relative_path}"))?;
        let primitive = gltf_mesh.primitives().next()
            .ok_or_else(|| format!("No primitives in mesh of {relative_path}"))?;

        let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

        // Positions (required)
        let positions: Vec<[f32; 3]> = reader.read_positions()
            .ok_or_else(|| format!("No positions in {relative_path}"))?
            .collect();

        // Indices (required for indexed draw)
        let indices: Vec<u32> = reader.read_indices()
            .ok_or_else(|| format!("No indices in {relative_path}"))?
            .into_u32()
            .collect();

        // Normals — generate flat normals from face geometry if missing
        let normals: Vec<[f32; 3]> = if let Some(norm_iter) = reader.read_normals() {
            norm_iter.collect()
        } else {
            generate_flat_normals(&positions, &indices)
        };

        // UVs — generate simple planar UVs if missing
        let uvs: Vec<[f32; 2]> = if let Some(tc_iter) = reader.read_tex_coords(0) {
            tc_iter.into_f32().collect()
        } else {
            generate_planar_uvs(&positions)
        };

        // Build engine Vertex array
        let vertices: Vec<crate::renderer::mesh::Vertex> = positions.iter().enumerate().map(|(i, pos)| {
            crate::renderer::mesh::Vertex {
                position: *pos,
                normal: normals.get(i).copied().unwrap_or([0.0, 1.0, 0.0]),
                uv: uvs.get(i).copied().unwrap_or([0.0, 0.0]),
            }
        }).collect();

        let mesh = crate::renderer::mesh::Mesh::from_vertices(
            &renderer.device,
            &vertices,
            &indices,
        );
        let mesh_idx = renderer.add_mesh(mesh);

        log::info!(
            "Loaded GLTF: {} ({} verts, {} tris)",
            relative_path,
            vertices.len(),
            indices.len() / 3,
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
