//! Mesh primitives — vertex/index buffers for renderable geometry.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Vertex with position, normal, and UV coordinates.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
}

impl Vertex {
    /// wgpu vertex buffer layout matching the PBR-lite shader.
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position @ location(0)
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // normal @ location(1)
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // uv @ location(2)
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

/// GPU-resident mesh with vertex and index buffers.
pub struct Mesh {
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl Mesh {
    /// Build a mesh from raw vertex and index data.
    pub fn from_vertices(device: &wgpu::Device, vertices: &[Vertex], indices: &[u32]) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Mesh Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    /// Unit cube centered at origin (side length 1).
    pub fn cube(device: &wgpu::Device) -> Self {
        // 24 vertices (4 per face, unique normals)
        #[rustfmt::skip]
        let vertices: &[Vertex] = &[
            // +Z face (front)
            Vertex { position: [-0.5, -0.5,  0.5], normal: [ 0.0,  0.0,  1.0], uv: [0.0, 1.0] },
            Vertex { position: [ 0.5, -0.5,  0.5], normal: [ 0.0,  0.0,  1.0], uv: [1.0, 1.0] },
            Vertex { position: [ 0.5,  0.5,  0.5], normal: [ 0.0,  0.0,  1.0], uv: [1.0, 0.0] },
            Vertex { position: [-0.5,  0.5,  0.5], normal: [ 0.0,  0.0,  1.0], uv: [0.0, 0.0] },
            // -Z face (back)
            Vertex { position: [ 0.5, -0.5, -0.5], normal: [ 0.0,  0.0, -1.0], uv: [0.0, 1.0] },
            Vertex { position: [-0.5, -0.5, -0.5], normal: [ 0.0,  0.0, -1.0], uv: [1.0, 1.0] },
            Vertex { position: [-0.5,  0.5, -0.5], normal: [ 0.0,  0.0, -1.0], uv: [1.0, 0.0] },
            Vertex { position: [ 0.5,  0.5, -0.5], normal: [ 0.0,  0.0, -1.0], uv: [0.0, 0.0] },
            // +X face (right)
            Vertex { position: [ 0.5, -0.5,  0.5], normal: [ 1.0,  0.0,  0.0], uv: [0.0, 1.0] },
            Vertex { position: [ 0.5, -0.5, -0.5], normal: [ 1.0,  0.0,  0.0], uv: [1.0, 1.0] },
            Vertex { position: [ 0.5,  0.5, -0.5], normal: [ 1.0,  0.0,  0.0], uv: [1.0, 0.0] },
            Vertex { position: [ 0.5,  0.5,  0.5], normal: [ 1.0,  0.0,  0.0], uv: [0.0, 0.0] },
            // -X face (left)
            Vertex { position: [-0.5, -0.5, -0.5], normal: [-1.0,  0.0,  0.0], uv: [0.0, 1.0] },
            Vertex { position: [-0.5, -0.5,  0.5], normal: [-1.0,  0.0,  0.0], uv: [1.0, 1.0] },
            Vertex { position: [-0.5,  0.5,  0.5], normal: [-1.0,  0.0,  0.0], uv: [1.0, 0.0] },
            Vertex { position: [-0.5,  0.5, -0.5], normal: [-1.0,  0.0,  0.0], uv: [0.0, 0.0] },
            // +Y face (top)
            Vertex { position: [-0.5,  0.5,  0.5], normal: [ 0.0,  1.0,  0.0], uv: [0.0, 1.0] },
            Vertex { position: [ 0.5,  0.5,  0.5], normal: [ 0.0,  1.0,  0.0], uv: [1.0, 1.0] },
            Vertex { position: [ 0.5,  0.5, -0.5], normal: [ 0.0,  1.0,  0.0], uv: [1.0, 0.0] },
            Vertex { position: [-0.5,  0.5, -0.5], normal: [ 0.0,  1.0,  0.0], uv: [0.0, 0.0] },
            // -Y face (bottom)
            Vertex { position: [-0.5, -0.5, -0.5], normal: [ 0.0, -1.0,  0.0], uv: [0.0, 1.0] },
            Vertex { position: [ 0.5, -0.5, -0.5], normal: [ 0.0, -1.0,  0.0], uv: [1.0, 1.0] },
            Vertex { position: [ 0.5, -0.5,  0.5], normal: [ 0.0, -1.0,  0.0], uv: [1.0, 0.0] },
            Vertex { position: [-0.5, -0.5,  0.5], normal: [ 0.0, -1.0,  0.0], uv: [0.0, 0.0] },
        ];

        #[rustfmt::skip]
        let indices: &[u32] = &[
             0,  1,  2,   2,  3,  0, // +Z
             4,  5,  6,   6,  7,  4, // -Z
             8,  9, 10,  10, 11,  8, // +X
            12, 13, 14,  14, 15, 12, // -X
            16, 17, 18,  18, 19, 16, // +Y
            20, 21, 22,  22, 23, 20, // -Y
        ];

        Self::from_vertices(device, vertices, indices)
    }

    /// Build a mesh from an icosphere (for planet rendering).
    /// Vertices are on a sphere of the given radius, normals point outward.
    pub fn from_icosphere(
        device: &wgpu::Device,
        icosphere: &crate::terrain::icosphere::Icosphere,
        radius: f32,
    ) -> Self {
        let vertices: Vec<Vertex> = icosphere.vertices.iter().map(|v| {
            let pos = *v * radius;
            let normal = v.normalize();
            // Simple UV from spherical coordinates
            let u = 0.5 + normal.z.atan2(normal.x) / (2.0 * std::f32::consts::PI);
            let v_coord = 0.5 - normal.y.asin() / std::f32::consts::PI;
            Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                uv: [u, v_coord],
            }
        }).collect();

        let mut indices = Vec::with_capacity(icosphere.faces.len() * 3);
        for face in &icosphere.faces {
            indices.push(face.v0);
            indices.push(face.v1);
            indices.push(face.v2);
        }

        Self::from_vertices(device, &vertices, &indices)
    }

    /// Build a mesh from procedural planet-surface data (v0.763).
    ///
    /// Per-face RGB colors ride packed inside the UV channel (see
    /// `terrain::planet_surface::pack_color_to_uv`); the PBR shader's
    /// material type 12 decodes them. All three corners of a flat-shaded
    /// face carry identical UVs, so rasterizer interpolation cannot corrupt
    /// the packed value. No new vertex layout or pipeline needed.
    pub fn from_planet_surface(
        device: &wgpu::Device,
        data: &crate::terrain::planet_surface::SurfaceMeshData,
    ) -> Self {
        let vertices: Vec<Vertex> = data
            .vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                uv: crate::terrain::planet_surface::pack_color_to_uv(v.color),
            })
            .collect();
        Self::from_vertices(device, &vertices, &data.indices)
    }

    /// Ground plane on XZ axis (centered at origin, 10x10 units).
    pub fn plane(device: &wgpu::Device) -> Self {
        let s = 5.0;
        let vertices: &[Vertex] = &[
            Vertex { position: [-s, 0.0, -s], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
            Vertex { position: [ s, 0.0, -s], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [ s, 0.0,  s], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [-s, 0.0,  s], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
        ];
        let indices: &[u32] = &[0, 2, 1, 0, 3, 2];
        Self::from_vertices(device, vertices, indices)
    }

    /// Open cylinder (solid wall, no caps) along +Y: base ring at y=0, top ring at
    /// y=height, outward normals. A placeholder aeroponic-tower column. The two
    /// rings are laid out ROW-MAJOR so the index winding is identical to
    /// `Mesh::sphere` (which renders correctly), avoiding the inverted-normal bug.
    pub fn cylinder(device: &wgpu::Device, radius: f32, height: f32, segments: u32) -> Self {
        let seg = segments.max(3);
        let tau = std::f32::consts::TAU;
        let mut v: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        // Two rings (bottom y=0, top y=height), row-major: ring * (seg+1) + i.
        for ring in 0..2u32 {
            let y = if ring == 0 { 0.0 } else { height };
            for i in 0..=seg {
                let t = i as f32 / seg as f32;
                let a = t * tau;
                let (ca, sa) = (a.cos(), a.sin());
                v.push(Vertex {
                    position: [radius * ca, y, radius * sa],
                    normal: [ca, 0.0, sa],
                    uv: [t, 1.0 - ring as f32],
                });
            }
        }
        let stride = seg + 1;
        for i in 0..seg {
            let a = i;
            let b = a + stride;
            // Same winding as Mesh::sphere (verified outward-facing).
            idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
        Self::from_vertices(device, &v, &idx)
    }

    /// Closed cylinder along +Y (base at y=0, top at y=height) WITH end caps. Like
    /// `cylinder` but the top + bottom are filled (triangle fans), so it reads as a solid
    /// pedestal/drum rather than an open tube. (v0.447: the showroom pedestal top was
    /// invisible because the open cylinder had no top face.)
    pub fn cylinder_capped(device: &wgpu::Device, radius: f32, height: f32, segments: u32) -> Self {
        let seg = segments.max(3);
        let tau = std::f32::consts::TAU;
        let mut v: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        // Side wall (same as `cylinder`): two rings, row-major.
        for ring in 0..2u32 {
            let y = if ring == 0 { 0.0 } else { height };
            for i in 0..=seg {
                let t = i as f32 / seg as f32;
                let a = t * tau;
                let (ca, sa) = (a.cos(), a.sin());
                v.push(Vertex {
                    position: [radius * ca, y, radius * sa],
                    normal: [ca, 0.0, sa],
                    uv: [t, 1.0 - ring as f32],
                });
            }
        }
        let stride = seg + 1;
        for i in 0..seg {
            idx.extend_from_slice(&[i, i + stride, i + 1, i + 1, i + stride, i + stride + 1]);
        }
        // Bottom cap (normal -Y), wound CCW *as seen from below* so its front face points -Y under
        // the renderer's CCW-front + back-cull convention -- else the cap is culled from outside and
        // the bottom looks open. (winding fix v0.624; the side wall above is the reference winding.)
        let bc = v.len() as u32;
        v.push(Vertex { position: [0.0, 0.0, 0.0], normal: [0.0, -1.0, 0.0], uv: [0.5, 0.5] });
        for i in 0..=seg {
            let a = (i as f32 / seg as f32) * tau;
            v.push(Vertex { position: [radius * a.cos(), 0.0, radius * a.sin()], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] });
        }
        for i in 0..seg {
            idx.extend_from_slice(&[bc, bc + 1 + i, bc + 1 + i + 1]);
        }
        // Top cap (normal +Y), wound CCW *as seen from above* so its front face points +Y -- the v0.622
        // bug was BOTH caps wound inward (fronts facing into the cylinder), so back-face culling ate
        // them and tanks/cisterns rendered with no top. (winding fix v0.624.)
        let tc = v.len() as u32;
        v.push(Vertex { position: [0.0, height, 0.0], normal: [0.0, 1.0, 0.0], uv: [0.5, 0.5] });
        for i in 0..=seg {
            let a = (i as f32 / seg as f32) * tau;
            v.push(Vertex { position: [radius * a.cos(), height, radius * a.sin()], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] });
        }
        for i in 0..seg {
            idx.extend_from_slice(&[tc, tc + 1 + i + 1, tc + 1 + i]);
        }
        Self::from_vertices(device, &v, &idx)
    }

    /// UV sphere centered at origin (outward normals). Used as a placeholder plant
    /// marker.
    pub fn sphere(device: &wgpu::Device, radius: f32, stacks: u32, slices: u32) -> Self {
        let st = stacks.max(2);
        let sl = slices.max(3);
        let pi = std::f32::consts::PI;
        let tau = std::f32::consts::TAU;
        let mut v: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        for i in 0..=st {
            let phi = pi * (i as f32 / st as f32);
            let (sp, cp) = (phi.sin(), phi.cos());
            for j in 0..=sl {
                let theta = tau * (j as f32 / sl as f32);
                let (stt, ct) = (theta.sin(), theta.cos());
                let (nx, ny, nz) = (sp * ct, cp, sp * stt);
                v.push(Vertex {
                    position: [radius * nx, radius * ny, radius * nz],
                    normal: [nx, ny, nz],
                    uv: [j as f32 / sl as f32, i as f32 / st as f32],
                });
            }
        }
        let row = sl + 1;
        for i in 0..st {
            for j in 0..sl {
                let a = i * row + j;
                let b = a + row;
                idx.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        Self::from_vertices(device, &v, &idx)
    }

    /// Axis-aligned box of size (w, h, d) meters, centered in x/z with its BASE at
    /// y=0 so it sits on a floor when placed at a floor position. Per-face normals,
    /// same winding as `cube`. A rudimentary machine stand-in (audit/First-Playable
    /// home population, 2026-06-13).
    pub fn box_xyz(device: &wgpu::Device, w: f32, h: f32, d: f32) -> Self {
        let (x, z) = (w * 0.5, d * 0.5);
        let (y0, y1) = (0.0, h);
        #[rustfmt::skip]
        let vertices: &[Vertex] = &[
            // +Z
            Vertex { position: [-x, y0,  z], normal: [0.0, 0.0,  1.0], uv: [0.0, 1.0] },
            Vertex { position: [ x, y0,  z], normal: [0.0, 0.0,  1.0], uv: [1.0, 1.0] },
            Vertex { position: [ x, y1,  z], normal: [0.0, 0.0,  1.0], uv: [1.0, 0.0] },
            Vertex { position: [-x, y1,  z], normal: [0.0, 0.0,  1.0], uv: [0.0, 0.0] },
            // -Z
            Vertex { position: [ x, y0, -z], normal: [0.0, 0.0, -1.0], uv: [0.0, 1.0] },
            Vertex { position: [-x, y0, -z], normal: [0.0, 0.0, -1.0], uv: [1.0, 1.0] },
            Vertex { position: [-x, y1, -z], normal: [0.0, 0.0, -1.0], uv: [1.0, 0.0] },
            Vertex { position: [ x, y1, -z], normal: [0.0, 0.0, -1.0], uv: [0.0, 0.0] },
            // +X
            Vertex { position: [ x, y0,  z], normal: [1.0, 0.0, 0.0], uv: [0.0, 1.0] },
            Vertex { position: [ x, y0, -z], normal: [1.0, 0.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [ x, y1, -z], normal: [1.0, 0.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [ x, y1,  z], normal: [1.0, 0.0, 0.0], uv: [0.0, 0.0] },
            // -X
            Vertex { position: [-x, y0, -z], normal: [-1.0, 0.0, 0.0], uv: [0.0, 1.0] },
            Vertex { position: [-x, y0,  z], normal: [-1.0, 0.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [-x, y1,  z], normal: [-1.0, 0.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [-x, y1, -z], normal: [-1.0, 0.0, 0.0], uv: [0.0, 0.0] },
            // +Y (top)
            Vertex { position: [-x, y1,  z], normal: [0.0, 1.0, 0.0], uv: [0.0, 1.0] },
            Vertex { position: [ x, y1,  z], normal: [0.0, 1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [ x, y1, -z], normal: [0.0, 1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [-x, y1, -z], normal: [0.0, 1.0, 0.0], uv: [0.0, 0.0] },
            // -Y (bottom)
            Vertex { position: [-x, y0, -z], normal: [0.0, -1.0, 0.0], uv: [0.0, 1.0] },
            Vertex { position: [ x, y0, -z], normal: [0.0, -1.0, 0.0], uv: [1.0, 1.0] },
            Vertex { position: [ x, y0,  z], normal: [0.0, -1.0, 0.0], uv: [1.0, 0.0] },
            Vertex { position: [-x, y0,  z], normal: [0.0, -1.0, 0.0], uv: [0.0, 0.0] },
        ];
        #[rustfmt::skip]
        let indices: &[u32] = &[
            0,1,2, 2,3,0,    4,5,6, 6,7,4,      8,9,10, 10,11,8,
            12,13,14, 14,15,12,  16,17,18, 18,19,16,  20,21,22, 22,23,20,
        ];
        Self::from_vertices(device, vertices, indices)
    }

    /// A flat thin RING (annulus) in the XZ plane at y=0, outer radius 1.0, facing +Y. Scaled by the
    /// radius for the editor's door interaction-distance ground ring. (v0.547)
    pub fn flat_ring(device: &wgpu::Device, segments: u32) -> Self {
        let (inner, outer) = (0.93_f32, 1.0_f32);
        let mut v: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        for i in 0..=segments {
            let a = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let (s, c) = a.sin_cos();
            let u = i as f32 / segments as f32;
            v.push(Vertex { position: [c * outer, 0.0, s * outer], normal: [0.0, 1.0, 0.0], uv: [u, 0.0] });
            v.push(Vertex { position: [c * inner, 0.0, s * inner], normal: [0.0, 1.0, 0.0], uv: [u, 1.0] });
        }
        for i in 0..segments {
            let o = i * 2;
            idx.extend([o, o + 1, o + 2, o + 1, o + 3, o + 2]);
        }
        Self::from_vertices(device, &v, &idx)
    }

    /// Square-base pyramid: base side `base` centered in x/z at y=0, apex at
    /// (0, height, 0). Flat per-face normals; each side's winding is chosen so the
    /// normal points OUTWARD (away from the y-axis), so back-face culling shows the
    /// outside without needing a visual check. A rudimentary stand-in.
    pub fn pyramid(device: &wgpu::Device, base: f32, height: f32) -> Self {
        let h = base * 0.5;
        let c = [[-h, 0.0, -h], [h, 0.0, -h], [h, 0.0, h], [-h, 0.0, h]];
        let apex = [0.0, height, 0.0];
        let mut v: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        for i in 0..4usize {
            let mut p0 = c[i];
            let mut p1 = c[(i + 1) % 4];
            // Face normal = cross(p1-p0, apex-p0); if it points toward the axis
            // (inward), swap p0/p1 so the winding + normal both face outward.
            let cross = |a: [f32; 3], b: [f32; 3]| {
                [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
            };
            let face_n = |p0: [f32; 3], p1: [f32; 3]| {
                let e1 = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                let e2 = [apex[0] - p0[0], apex[1] - p0[1], apex[2] - p0[2]];
                let n = cross(e1, e2);
                let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-6);
                [n[0] / l, n[1] / l, n[2] / l]
            };
            let mut n = face_n(p0, p1);
            // Centroid horizontal direction (outward from axis).
            let cx = (p0[0] + p1[0] + apex[0]) / 3.0;
            let cz = (p0[2] + p1[2] + apex[2]) / 3.0;
            if n[0] * cx + n[2] * cz < 0.0 {
                std::mem::swap(&mut p0, &mut p1);
                n = face_n(p0, p1);
            }
            let bi = v.len() as u32;
            v.push(Vertex { position: p0, normal: n, uv: [0.0, 1.0] });
            v.push(Vertex { position: p1, normal: n, uv: [1.0, 1.0] });
            v.push(Vertex { position: apex, normal: n, uv: [0.5, 0.0] });
            idx.extend_from_slice(&[bi, bi + 1, bi + 2]);
        }
        // Base (downward normal). Wind so the front faces -Y (viewed from below).
        let bn = [0.0, -1.0, 0.0];
        let bi = v.len() as u32;
        for &p in &c {
            v.push(Vertex { position: p, normal: bn, uv: [0.0, 0.0] });
        }
        idx.extend_from_slice(&[bi, bi + 2, bi + 1, bi, bi + 3, bi + 2]);
        Self::from_vertices(device, &v, &idx)
    }

    /// A DIAMOND (octahedron) of half-extent `r`, centred at the origin (v0.572). Six apexes on the
    /// axes, eight triangular faces, outward-wound with centroid normals. Used as a distinct centre
    /// marker for placed LIGHTS (vs the sphere orb for wall corners).
    pub fn octahedron(device: &wgpu::Device, r: f32) -> Self {
        let top = [0.0, r, 0.0];
        let bot = [0.0, -r, 0.0];
        let eq = [[r, 0.0, 0.0], [0.0, 0.0, r], [-r, 0.0, 0.0], [0.0, 0.0, -r]];
        let mut v: Vec<Vertex> = Vec::new();
        let mut idx: Vec<u32> = Vec::new();
        let mut face = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
            // Outward normal = the normalized centroid (the shape is centred on the origin).
            let cen = [(a[0] + b[0] + c[0]) / 3.0, (a[1] + b[1] + c[1]) / 3.0, (a[2] + b[2] + c[2]) / 3.0];
            let l = (cen[0] * cen[0] + cen[1] * cen[1] + cen[2] * cen[2]).sqrt().max(1e-6);
            let n = [cen[0] / l, cen[1] / l, cen[2] / l];
            let bi = v.len() as u32;
            v.push(Vertex { position: a, normal: n, uv: [0.0, 0.0] });
            v.push(Vertex { position: b, normal: n, uv: [1.0, 0.0] });
            v.push(Vertex { position: c, normal: n, uv: [0.5, 1.0] });
            idx.extend_from_slice(&[bi, bi + 1, bi + 2]);
        };
        for i in 0..4usize {
            let e0 = eq[i];
            let e1 = eq[(i + 1) % 4];
            face(top, e1, e0); // top half (outward winding)
            face(bot, e0, e1); // bottom half
        }
        Self::from_vertices(device, &v, &idx)
    }

    /// A straight ROUND tube (pipe / cable) from world point `a` to `b` with the given
    /// outer `radius` and `sides` (cross-section polygon, 8 reads round). Built in world
    /// space and placed at the origin, since the render path is translation-only. This is
    /// the realistic-pipe successor to `segment`: round section reads as plumbing/conduit
    /// rather than ducting. Used for pipe bodies, collars (fat + short), and valve bodies.
    pub fn tube(device: &wgpu::Device, a: glam::Vec3, b: glam::Vec3, radius: f32, sides: u32) -> Self {
        let n = sides.max(3);
        let tau = std::f32::consts::TAU;
        let dir = (b - a).normalize_or_zero();
        let dir = if dir.length_squared() < 1e-6 { glam::Vec3::Y } else { dir };
        let up = if dir.dot(glam::Vec3::Y).abs() > 0.95 { glam::Vec3::X } else { glam::Vec3::Y };
        let right = dir.cross(up).normalize_or_zero();
        let realup = right.cross(dir).normalize_or_zero();
        let mut v: Vec<Vertex> = Vec::new();
        for &p in &[a, b] {
            for i in 0..=n {
                let t = i as f32 / n as f32;
                let ang = t * tau;
                let off = right * (ang.cos() * radius) + realup * (ang.sin() * radius);
                let pos = p + off;
                let nrm = off.normalize_or_zero();
                v.push(Vertex {
                    position: [pos.x, pos.y, pos.z],
                    normal: [nrm.x, nrm.y, nrm.z],
                    uv: [t, 0.0],
                });
            }
        }
        let stride = n + 1;
        let mut idx: Vec<u32> = Vec::new();
        for i in 0..n {
            let a0 = i;
            let a1 = i + 1;
            let b0 = i + stride;
            let b1 = i + 1 + stride;
            // Same winding convention as `segment` (verified outward-facing).
            idx.extend_from_slice(&[a0, b0, a1, a1, b0, b1]);
        }
        Self::from_vertices(device, &v, &idx)
    }

    /// A straight square-section tube (pipe / cable / connection) from world point
    /// `a` to world point `b` with the given `radius`. Built directly in world space
    /// and placed at the origin, since the placeholder render path is translation-only
    /// (no per-object rotation). Used to draw connections between machines.
    pub fn segment(device: &wgpu::Device, a: glam::Vec3, b: glam::Vec3, radius: f32) -> Self {
        let dir = (b - a).normalize_or_zero();
        let dir = if dir.length_squared() < 1e-6 { glam::Vec3::Y } else { dir };
        // A frame perpendicular to dir.
        let up = if dir.dot(glam::Vec3::Y).abs() > 0.95 { glam::Vec3::X } else { glam::Vec3::Y };
        let right = dir.cross(up).normalize_or_zero() * radius;
        let upn = right.normalize_or_zero().cross(dir).normalize_or_zero() * radius;
        // 4 corners at each end.
        let ends = [a, b];
        let mut v: Vec<Vertex> = Vec::new();
        for &p in &ends {
            for &(s0, s1) in &[(1.0f32, 1.0f32), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)] {
                let off = right * s0 + upn * s1;
                let pos = p + off;
                let n = off.normalize_or_zero();
                v.push(Vertex {
                    position: [pos.x, pos.y, pos.z],
                    normal: [n.x, n.y, n.z],
                    uv: [0.0, 0.0],
                });
            }
        }
        // 4 side quads connecting end 0 (verts 0..3) to end 1 (verts 4..7).
        let mut idx: Vec<u32> = Vec::new();
        for i in 0..4u32 {
            let a0 = i;
            let a1 = (i + 1) % 4;
            let b0 = i + 4;
            let b1 = (i + 1) % 4 + 4;
            idx.extend_from_slice(&[a0, b0, a1, a1, b0, b1]);
        }
        Self::from_vertices(device, &v, &idx)
    }
}
