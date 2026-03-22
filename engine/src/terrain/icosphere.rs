//! Icosphere generation with recursive subdivision.
//!
//! Starts with an icosahedron (20 triangular faces). Each face can be
//! subdivided into 4 child triangles by splitting edges at midpoints and
//! projecting onto the unit sphere. This produces evenly-sized faces
//! everywhere on the sphere, avoiding mercator distortion and cube-map seams.
//!
//! At subdivision level N: 20 * 4^N faces.
//! Level 0: 20 faces (icosahedron)
//! Level 3: 1,280 faces
//! Level 5: 20,480 faces
//! Level 8: 1,310,720 faces

use glam::Vec3;
use std::collections::HashMap;

/// A triangular face on the icosphere, defined by 3 vertex indices.
#[derive(Debug, Clone, Copy)]
pub struct Face {
    pub v0: u32,
    pub v1: u32,
    pub v2: u32,
}

/// An icosphere with vertices on the unit sphere and triangular faces.
/// Subdivision creates progressively more detailed meshes.
pub struct Icosphere {
    /// Vertex positions on the unit sphere (normalized to length 1).
    pub vertices: Vec<Vec3>,
    /// Triangular faces (indices into vertices).
    pub faces: Vec<Face>,
}

impl Icosphere {
    /// Generate the base icosahedron (20 faces, 12 vertices).
    pub fn new() -> Self {
        let t = (1.0 + 5.0_f32.sqrt()) / 2.0; // golden ratio

        // 12 vertices of a regular icosahedron
        let raw = [
            Vec3::new(-1.0,  t, 0.0),
            Vec3::new( 1.0,  t, 0.0),
            Vec3::new(-1.0, -t, 0.0),
            Vec3::new( 1.0, -t, 0.0),
            Vec3::new(0.0, -1.0,  t),
            Vec3::new(0.0,  1.0,  t),
            Vec3::new(0.0, -1.0, -t),
            Vec3::new(0.0,  1.0, -t),
            Vec3::new( t, 0.0, -1.0),
            Vec3::new( t, 0.0,  1.0),
            Vec3::new(-t, 0.0, -1.0),
            Vec3::new(-t, 0.0,  1.0),
        ];

        let vertices: Vec<Vec3> = raw.iter().map(|v| v.normalize()).collect();

        // 20 triangular faces
        let faces = vec![
            // 5 faces around vertex 0
            Face { v0: 0, v1: 11, v2: 5 },
            Face { v0: 0, v1: 5, v2: 1 },
            Face { v0: 0, v1: 1, v2: 7 },
            Face { v0: 0, v1: 7, v2: 10 },
            Face { v0: 0, v1: 10, v2: 11 },
            // 5 adjacent faces
            Face { v0: 1, v1: 5, v2: 9 },
            Face { v0: 5, v1: 11, v2: 4 },
            Face { v0: 11, v1: 10, v2: 2 },
            Face { v0: 10, v1: 7, v2: 6 },
            Face { v0: 7, v1: 1, v2: 8 },
            // 5 faces around vertex 3
            Face { v0: 3, v1: 9, v2: 4 },
            Face { v0: 3, v1: 4, v2: 2 },
            Face { v0: 3, v1: 2, v2: 6 },
            Face { v0: 3, v1: 6, v2: 8 },
            Face { v0: 3, v1: 8, v2: 9 },
            // 5 adjacent faces
            Face { v0: 4, v1: 9, v2: 5 },
            Face { v0: 2, v1: 4, v2: 11 },
            Face { v0: 6, v1: 2, v2: 10 },
            Face { v0: 8, v1: 6, v2: 7 },
            Face { v0: 9, v1: 8, v2: 1 },
        ];

        Self { vertices, faces }
    }

    /// Subdivide all faces once. Each triangle becomes 4 child triangles.
    /// New vertices are placed at edge midpoints and projected onto the unit sphere.
    pub fn subdivide(&mut self) {
        let mut midpoint_cache: HashMap<(u32, u32), u32> = HashMap::new();
        let old_faces = std::mem::take(&mut self.faces);
        let mut new_faces = Vec::with_capacity(old_faces.len() * 4);

        for face in &old_faces {
            let a = self.get_or_create_midpoint(&mut midpoint_cache, face.v0, face.v1);
            let b = self.get_or_create_midpoint(&mut midpoint_cache, face.v1, face.v2);
            let c = self.get_or_create_midpoint(&mut midpoint_cache, face.v2, face.v0);

            new_faces.push(Face { v0: face.v0, v1: a, v2: c });
            new_faces.push(Face { v0: face.v1, v1: b, v2: a });
            new_faces.push(Face { v0: face.v2, v1: c, v2: b });
            new_faces.push(Face { v0: a, v1: b, v2: c });
        }

        self.faces = new_faces;
    }

    /// Subdivide N times to reach a specific detail level.
    pub fn subdivide_n(&mut self, levels: u32) {
        for _ in 0..levels {
            self.subdivide();
        }
    }

    /// Get or create the midpoint vertex between two vertices.
    /// Midpoints are projected onto the unit sphere (normalized).
    fn get_or_create_midpoint(
        &mut self,
        cache: &mut HashMap<(u32, u32), u32>,
        i0: u32,
        i1: u32,
    ) -> u32 {
        // Order the indices so (min, max) is the cache key (edge is undirected)
        let key = if i0 < i1 { (i0, i1) } else { (i1, i0) };

        if let Some(&idx) = cache.get(&key) {
            return idx;
        }

        let v0 = self.vertices[i0 as usize];
        let v1 = self.vertices[i1 as usize];
        let mid = ((v0 + v1) * 0.5).normalize(); // project onto unit sphere

        let idx = self.vertices.len() as u32;
        self.vertices.push(mid);
        cache.insert(key, idx);
        idx
    }

    /// Total face count at a given subdivision level (without modifying the sphere).
    pub fn face_count_at_level(level: u32) -> u64 {
        20 * 4u64.pow(level)
    }

    /// Generate vertex positions scaled to a given radius.
    pub fn scaled_vertices(&self, radius: f32) -> Vec<Vec3> {
        self.vertices.iter().map(|v| *v * radius).collect()
    }

    /// Get the center point of a face (average of its 3 vertices), on the unit sphere.
    pub fn face_center(&self, face: &Face) -> Vec3 {
        let v0 = self.vertices[face.v0 as usize];
        let v1 = self.vertices[face.v1 as usize];
        let v2 = self.vertices[face.v2 as usize];
        ((v0 + v1 + v2) / 3.0).normalize()
    }
}
