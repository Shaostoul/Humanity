//! Sparse octree voxel volume for asteroids.
//!
//! Asteroids are procedurally generated from a seed and classification type.
//! Shape is a deformed sphere via simplex noise; ore veins are placed using
//! 3D noise channels mapped to voxel types based on asteroid classification.
//! Resolution is 0.5m per voxel.
//!
//! The octree stores only non-empty regions — empty space costs zero allocation
//! beyond a single `Leaf(Empty)` discriminant. Mining removes individual voxels
//! and marks the asteroid as modified for re-meshing.

use crate::renderer::mesh::Vertex;
use noise::{NoiseFn, Perlin, Seedable};
use serde::Deserialize;

/// Resolution: each voxel represents a 0.5m cube.
pub const VOXEL_SIZE: f32 = 0.5;

// ─── Voxel types ───────────────────────────────────────────────────────────

/// Material occupying a single voxel cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelType {
    Empty = 0,
    Stone = 1,
    Iron = 2,
    Nickel = 3,
    Ice = 4,
    Carbon = 5,
    Silicate = 6,
    Platinum = 7,
}

impl VoxelType {
    /// Base color for mesh generation (RGBA, linear).
    pub fn color(&self) -> [f32; 4] {
        match self {
            VoxelType::Empty => [0.0, 0.0, 0.0, 0.0],
            VoxelType::Stone => [0.45, 0.42, 0.38, 1.0],
            VoxelType::Iron => [0.55, 0.35, 0.25, 1.0],
            VoxelType::Nickel => [0.65, 0.63, 0.55, 1.0],
            VoxelType::Ice => [0.75, 0.85, 0.95, 1.0],
            VoxelType::Carbon => [0.15, 0.15, 0.15, 1.0],
            VoxelType::Silicate => [0.60, 0.55, 0.45, 1.0],
            VoxelType::Platinum => [0.80, 0.78, 0.72, 1.0],
        }
    }

    /// Whether this voxel is solid (non-empty).
    pub fn is_solid(&self) -> bool {
        !matches!(self, VoxelType::Empty)
    }
}

impl Default for VoxelType {
    fn default() -> Self {
        VoxelType::Empty
    }
}

// ─── Sparse octree ────────────────────────────────────────────────────────

/// Recursive sparse octree. Each node is either a uniform leaf or 8 children.
/// Empty regions are `Leaf(Empty)` — no heap allocation for void space.
#[derive(Debug, Clone)]
pub enum Octree<T: Clone + PartialEq + Default> {
    /// Uniform region filled with a single value.
    Leaf(T),
    /// Eight children indexed by octant (see `octant_index`).
    Branch(Box<[Octree<T>; 8]>),
}

impl<T: Clone + PartialEq + Default> Octree<T> {
    /// Create a uniform octree filled with the default value.
    pub fn empty() -> Self {
        Octree::Leaf(T::default())
    }

    /// Get the value at integer coordinates within a volume of `size` voxels per side.
    /// `size` must be a power of 2.
    pub fn get(&self, x: u32, y: u32, z: u32, size: u32) -> &T {
        match self {
            Octree::Leaf(val) => val,
            Octree::Branch(children) => {
                let half = size / 2;
                let idx = octant_index(x, y, z, half);
                children[idx].get(x % half, y % half, z % half, half)
            }
        }
    }

    /// Set a value at integer coordinates, splitting leaves into branches as needed.
    /// Returns the previous value.
    pub fn set(&mut self, x: u32, y: u32, z: u32, size: u32, value: T) -> T {
        if size == 1 {
            let old = match self {
                Octree::Leaf(v) => v.clone(),
                _ => T::default(),
            };
            *self = Octree::Leaf(value);
            return old;
        }

        // If this is a leaf, we may need to split it
        if let Octree::Leaf(current) = self {
            if *current == value {
                return value; // no change needed
            }
            // Split: create 8 children all with the current leaf value
            let fill = current.clone();
            let children = Box::new([
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill.clone()),
                Octree::Leaf(fill),
            ]);
            *self = Octree::Branch(children);
        }

        let half = size / 2;
        let idx = octant_index(x, y, z, half);

        let old = if let Octree::Branch(children) = self {
            children[idx].set(x % half, y % half, z % half, half, value)
        } else {
            unreachable!()
        };

        // Try to collapse: if all 8 children are identical leaves, merge back
        self.try_collapse();

        old
    }

    /// If all 8 children are identical leaves, collapse back to a single leaf.
    fn try_collapse(&mut self) {
        let should_collapse = if let Octree::Branch(children) = self {
            if let Octree::Leaf(first) = &children[0] {
                let first = first.clone();
                children[1..].iter().all(|c| matches!(c, Octree::Leaf(v) if *v == first))
            } else {
                false
            }
        } else {
            false
        };

        if should_collapse {
            if let Octree::Branch(children) = self {
                if let Octree::Leaf(val) = &children[0] {
                    *self = Octree::Leaf(val.clone());
                }
            }
        }
    }

    /// Iterate over all non-default voxels with their coordinates.
    /// Calls `f(x, y, z, &value)` for each solid cell.
    pub fn for_each_solid<F>(&self, ox: u32, oy: u32, oz: u32, size: u32, f: &mut F)
    where
        F: FnMut(u32, u32, u32, &T),
    {
        match self {
            Octree::Leaf(val) => {
                if *val != T::default() {
                    // Fill the entire region
                    for z in oz..oz + size {
                        for y in oy..oy + size {
                            for x in ox..ox + size {
                                f(x, y, z, val);
                            }
                        }
                    }
                }
            }
            Octree::Branch(children) => {
                let half = size / 2;
                for i in 0..8u32 {
                    let cx = ox + (i & 1) * half;
                    let cy = oy + ((i >> 1) & 1) * half;
                    let cz = oz + ((i >> 2) & 1) * half;
                    children[i as usize].for_each_solid(cx, cy, cz, half, f);
                }
            }
        }
    }
}

/// Map 3D coordinates to an octant index (0-7) based on which half they fall in.
fn octant_index(x: u32, y: u32, z: u32, half: u32) -> usize {
    let ix = if x >= half { 1 } else { 0 };
    let iy = if y >= half { 1 } else { 0 };
    let iz = if z >= half { 1 } else { 0 };
    ix | (iy << 1) | (iz << 2)
}

// ─── Asteroid classification ──────────────────────────────────────────────

/// Asteroid spectral classification determining composition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum AsteroidClass {
    /// Carbonaceous: carbon, ice, organics.
    #[serde(alias = "C")]
    C,
    /// Silicaceous: silicate minerals, moderate metals.
    #[serde(alias = "S")]
    S,
    /// Metallic: iron-nickel, platinum group metals.
    #[serde(alias = "M")]
    M,
}

// ─── Asteroid definition (data-driven) ────────────────────────────────────

/// Loaded from data files. Small seed data — the engine generates everything at runtime.
#[derive(Debug, Clone, Deserialize)]
pub struct AsteroidDef {
    /// Display name.
    pub name: String,
    /// Procedural generation seed.
    pub seed: u64,
    /// Spectral classification controlling composition.
    pub classification: AsteroidClass,
    /// Mean radius in meters.
    pub radius_meters: f32,
    /// Bulk density in g/cm^3.
    pub density: f32,
}

// ─── Generated asteroid ───────────────────────────────────────────────────

/// A fully generated asteroid volume ready for meshing and mining.
pub struct Asteroid {
    /// The voxel volume.
    pub octree: Octree<VoxelType>,
    /// Grid size (power-of-2 side length in voxels).
    pub grid_size: u32,
    /// Original definition.
    pub def: AsteroidDef,
    /// Whether the volume has been modified since last mesh generation.
    pub modified: bool,
}

impl Asteroid {
    /// Procedurally generate an asteroid from its definition.
    ///
    /// Shape is a deformed sphere (simplex noise displacement).
    /// Ore distribution uses separate noise channels per classification.
    pub fn generate(def: &AsteroidDef) -> Self {
        let radius_voxels = (def.radius_meters / VOXEL_SIZE).ceil() as u32;
        // Grid size must be a power of 2 and large enough to contain the diameter + margin
        let diameter = radius_voxels * 2 + 2; // +2 for margin
        let grid_size = diameter.next_power_of_two().max(4);
        let center = grid_size as f32 / 2.0;

        let seed = def.seed as u32;

        // Shape noise: deforms the sphere boundary
        let shape_noise = Perlin::new(seed);
        // Secondary shape octave for detail
        let detail_noise = Perlin::new(seed.wrapping_add(1));
        // Ore vein noise channels
        let ore_noise_a = Perlin::new(seed.wrapping_add(100));
        let ore_noise_b = Perlin::new(seed.wrapping_add(200));
        let ore_noise_c = Perlin::new(seed.wrapping_add(300));

        let mut octree = Octree::empty();

        let r_base = radius_voxels as f32;

        for z in 0..grid_size {
            for y in 0..grid_size {
                for x in 0..grid_size {
                    let fx = x as f32 - center;
                    let fy = y as f32 - center;
                    let fz = z as f32 - center;
                    let dist = (fx * fx + fy * fy + fz * fz).sqrt();

                    if dist < 0.001 {
                        // Center voxel — always solid
                        let vt = base_material(def.classification);
                        octree.set(x, y, z, grid_size, vt);
                        continue;
                    }

                    // Normalize direction for noise sampling
                    let nx = fx / dist;
                    let ny = fy / dist;
                    let nz = fz / dist;

                    // Shape deformation: +-20% of radius
                    let shape_scale = 0.05_f64;
                    let shape_val = shape_noise.get([
                        nx as f64 * 3.0 * shape_scale + 0.5,
                        ny as f64 * 3.0 * shape_scale + 0.5,
                        nz as f64 * 3.0 * shape_scale + 0.5,
                    ]) as f32;

                    let detail_scale = 0.08_f64;
                    let detail_val = detail_noise.get([
                        nx as f64 * 7.0 * detail_scale,
                        ny as f64 * 7.0 * detail_scale,
                        nz as f64 * 7.0 * detail_scale,
                    ]) as f32;

                    let effective_radius = r_base * (1.0 + shape_val * 0.2 + detail_val * 0.08);

                    if dist > effective_radius {
                        continue; // outside the asteroid
                    }

                    // Determine voxel type based on ore noise and classification
                    let ore_sample = [
                        x as f64 * 0.15,
                        y as f64 * 0.15,
                        z as f64 * 0.15,
                    ];
                    let ore_a = ore_noise_a.get(ore_sample) as f32;
                    let ore_b = ore_noise_b.get(ore_sample) as f32;
                    let ore_c = ore_noise_c.get(ore_sample) as f32;

                    let depth_ratio = dist / effective_radius; // 0 at center, 1 at surface

                    let voxel = classify_voxel(def.classification, ore_a, ore_b, ore_c, depth_ratio);
                    octree.set(x, y, z, grid_size, voxel);
                }
            }
        }

        Asteroid {
            octree,
            grid_size,
            def: def.clone(),
            modified: true,
        }
    }

    /// Remove a voxel at the given grid coordinates. Returns what was there.
    /// Marks the asteroid as modified for re-meshing.
    pub fn remove_voxel(&mut self, x: u32, y: u32, z: u32) -> Option<VoxelType> {
        if x >= self.grid_size || y >= self.grid_size || z >= self.grid_size {
            return None;
        }
        let old = self.octree.get(x, y, z, self.grid_size).clone();
        if old == VoxelType::Empty {
            return None;
        }
        self.octree.set(x, y, z, self.grid_size, VoxelType::Empty);
        self.modified = true;
        Some(old)
    }

    /// World-space position of a voxel's center (relative to asteroid origin).
    pub fn voxel_world_pos(&self, x: u32, y: u32, z: u32) -> [f32; 3] {
        let center = self.grid_size as f32 / 2.0;
        [
            (x as f32 - center + 0.5) * VOXEL_SIZE,
            (y as f32 - center + 0.5) * VOXEL_SIZE,
            (z as f32 - center + 0.5) * VOXEL_SIZE,
        ]
    }

    /// Extract visible-surface mesh using greedy face culling.
    /// Only faces adjacent to Empty voxels are emitted.
    /// Returns vertex/index data compatible with `Mesh::from_vertices()`.
    pub fn mesh_vertices(&self) -> (Vec<Vertex>, Vec<u32>) {
        let gs = self.grid_size;
        let center = gs as f32 / 2.0;

        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        // Face directions: +X, -X, +Y, -Y, +Z, -Z
        const DIRS: [(i32, i32, i32); 6] = [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ];

        const NORMALS: [[f32; 3]; 6] = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
        ];

        self.octree.for_each_solid(0, 0, 0, gs, &mut |x, y, z, vt: &VoxelType| {
            let color = vt.color();
            let px = (x as f32 - center) * VOXEL_SIZE;
            let py = (y as f32 - center) * VOXEL_SIZE;
            let pz = (z as f32 - center) * VOXEL_SIZE;

            for (face_idx, &(dx, dy, dz)) in DIRS.iter().enumerate() {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                let nz = z as i32 + dz;

                // Face is visible if neighbor is outside the grid or empty
                let neighbor_solid = if nx >= 0
                    && ny >= 0
                    && nz >= 0
                    && (nx as u32) < gs
                    && (ny as u32) < gs
                    && (nz as u32) < gs
                {
                    self.octree
                        .get(nx as u32, ny as u32, nz as u32, gs)
                        .is_solid()
                } else {
                    false
                };

                if neighbor_solid {
                    continue;
                }

                let normal = NORMALS[face_idx];
                let base_idx = vertices.len() as u32;

                // Encode color into UV (u = color index / 8, v = 0)
                // Shaders can use this to look up voxel material
                let u_val = (*vt as u8) as f32 / 8.0;

                // Emit 4 vertices for this face
                let face_verts = face_quad(px, py, pz, VOXEL_SIZE, face_idx);
                for pos in &face_verts {
                    vertices.push(Vertex {
                        position: *pos,
                        normal,
                        uv: [u_val, 0.0],
                    });
                }

                // Two triangles per quad (CCW winding)
                indices.push(base_idx);
                indices.push(base_idx + 1);
                indices.push(base_idx + 2);
                indices.push(base_idx + 2);
                indices.push(base_idx + 3);
                indices.push(base_idx);
            }
        });

        (vertices, indices)
    }
}

/// Return the 4 corner positions for a face of a voxel cube.
/// `face_idx`: 0=+X, 1=-X, 2=+Y, 3=-Y, 4=+Z, 5=-Z
fn face_quad(px: f32, py: f32, pz: f32, s: f32, face_idx: usize) -> [[f32; 3]; 4] {
    match face_idx {
        0 => [
            // +X face
            [px + s, py, pz],
            [px + s, py, pz + s],
            [px + s, py + s, pz + s],
            [px + s, py + s, pz],
        ],
        1 => [
            // -X face
            [px, py, pz + s],
            [px, py, pz],
            [px, py + s, pz],
            [px, py + s, pz + s],
        ],
        2 => [
            // +Y face
            [px, py + s, pz],
            [px + s, py + s, pz],
            [px + s, py + s, pz + s],
            [px, py + s, pz + s],
        ],
        3 => [
            // -Y face
            [px, py, pz + s],
            [px + s, py, pz + s],
            [px + s, py, pz],
            [px, py, pz],
        ],
        4 => [
            // +Z face
            [px, py, pz + s],
            [px, py + s, pz + s],
            [px + s, py + s, pz + s],
            [px + s, py, pz + s],
        ],
        5 => [
            // -Z face
            [px + s, py, pz],
            [px + s, py + s, pz],
            [px, py + s, pz],
            [px, py, pz],
        ],
        _ => unreachable!(),
    }
}

/// Default bulk material for an asteroid classification.
fn base_material(class: AsteroidClass) -> VoxelType {
    match class {
        AsteroidClass::C => VoxelType::Carbon,
        AsteroidClass::S => VoxelType::Silicate,
        AsteroidClass::M => VoxelType::Iron,
    }
}

/// Classify a voxel based on noise values and asteroid type.
/// `ore_a/b/c` are noise values in roughly [-1, 1].
/// `depth_ratio` is 0.0 at center, 1.0 at surface.
fn classify_voxel(
    class: AsteroidClass,
    ore_a: f32,
    ore_b: f32,
    ore_c: f32,
    depth_ratio: f32,
) -> VoxelType {
    // Ore veins appear where noise exceeds a threshold (sparse distribution)
    let vein_threshold = 0.55;

    match class {
        AsteroidClass::C => {
            // Carbonaceous: primarily carbon with ice veins near surface, stone core
            if ore_a > vein_threshold && depth_ratio > 0.5 {
                VoxelType::Ice
            } else if ore_b > vein_threshold + 0.1 {
                VoxelType::Stone
            } else if ore_c > vein_threshold + 0.2 && depth_ratio < 0.4 {
                VoxelType::Nickel // trace metals deep inside
            } else {
                VoxelType::Carbon
            }
        }
        AsteroidClass::S => {
            // Silicaceous: silicate body with iron and nickel veins
            if ore_a > vein_threshold {
                VoxelType::Iron
            } else if ore_b > vein_threshold + 0.05 {
                VoxelType::Nickel
            } else if ore_c > vein_threshold + 0.15 && depth_ratio < 0.3 {
                VoxelType::Platinum // rare, deep only
            } else {
                VoxelType::Silicate
            }
        }
        AsteroidClass::M => {
            // Metallic: iron-nickel body with platinum veins
            if ore_a > vein_threshold {
                VoxelType::Nickel
            } else if ore_b > vein_threshold + 0.1 {
                VoxelType::Platinum
            } else if ore_c > vein_threshold && depth_ratio > 0.7 {
                VoxelType::Stone // surface regolith
            } else {
                VoxelType::Iron
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octree_set_get() {
        let mut tree: Octree<VoxelType> = Octree::empty();
        tree.set(3, 5, 7, 8, VoxelType::Iron);
        assert_eq!(*tree.get(3, 5, 7, 8), VoxelType::Iron);
        assert_eq!(*tree.get(0, 0, 0, 8), VoxelType::Empty);
    }

    #[test]
    fn octree_collapse() {
        let mut tree: Octree<u8> = Octree::empty();
        // Fill an entire 2x2x2 with the same value — should collapse
        for z in 0..2 {
            for y in 0..2 {
                for x in 0..2 {
                    tree.set(x, y, z, 2, 42);
                }
            }
        }
        assert!(matches!(tree, Octree::Leaf(42)));
    }

    #[test]
    fn generate_small_asteroid() {
        let def = AsteroidDef {
            name: "Test Rock".into(),
            seed: 12345,
            classification: AsteroidClass::S,
            radius_meters: 5.0,
            density: 2.7,
        };
        let asteroid = Asteroid::generate(&def);
        assert!(asteroid.grid_size >= 20); // 5m / 0.5m * 2 = 20 voxels diameter minimum
        assert!(asteroid.modified);
    }

    #[test]
    fn mesh_produces_valid_geometry() {
        let def = AsteroidDef {
            name: "Tiny".into(),
            seed: 99,
            classification: AsteroidClass::C,
            radius_meters: 2.0,
            density: 1.3,
        };
        let asteroid = Asteroid::generate(&def);
        let (verts, idxs) = asteroid.mesh_vertices();
        // Should have some geometry
        assert!(!verts.is_empty(), "mesh should have vertices");
        assert!(!idxs.is_empty(), "mesh should have indices");
        // Every index must reference a valid vertex
        for &idx in &idxs {
            assert!((idx as usize) < verts.len(), "index out of bounds");
        }
        // Index count must be divisible by 3 (triangles)
        assert_eq!(idxs.len() % 3, 0, "indices must form complete triangles");
    }

    #[test]
    fn mining_removes_voxel() {
        let def = AsteroidDef {
            name: "Mineable".into(),
            seed: 42,
            classification: AsteroidClass::M,
            radius_meters: 3.0,
            density: 5.3,
        };
        let mut asteroid = Asteroid::generate(&def);
        asteroid.modified = false;

        // Find a solid voxel near the center
        let c = asteroid.grid_size / 2;
        let vt = asteroid.octree.get(c, c, c, asteroid.grid_size).clone();
        assert!(vt.is_solid(), "center should be solid");

        let removed = asteroid.remove_voxel(c, c, c);
        assert_eq!(removed, Some(vt));
        assert!(asteroid.modified);
        assert_eq!(
            *asteroid.octree.get(c, c, c, asteroid.grid_size),
            VoxelType::Empty
        );
    }

    #[test]
    fn remove_out_of_bounds_returns_none() {
        let def = AsteroidDef {
            name: "OOB".into(),
            seed: 1,
            classification: AsteroidClass::C,
            radius_meters: 2.0,
            density: 1.0,
        };
        let mut asteroid = Asteroid::generate(&def);
        assert_eq!(asteroid.remove_voxel(9999, 9999, 9999), None);
    }
}
