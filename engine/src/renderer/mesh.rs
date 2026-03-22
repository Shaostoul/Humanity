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
}
