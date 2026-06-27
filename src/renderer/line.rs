//! World-space thin-line renderer (v0.262.20).
//!
//! The orbit paths were thick tube meshes (operator: "tubes are just
//! too thick … we wouldn't need all the verts … like a single edge in
//! Blender"). This is that single edge: a `LineList` pipeline that
//! reuses the MAIN camera (full view-proj + floating origin, so lines
//! sit in world space exactly where the planets are) and the SAME
//! reverse-Z depth buffer the scene wrote — but **does not write
//! depth**. Result: an orbit segment that passes BEHIND a planet is
//! occluded by the planet's depth (the "fade behind the planet"
//! cue the operator wanted), while overlapping orbit lines don't
//! fight each other. ~1px GPU lines, 2 verts per segment — a fraction
//! of the tube's vertex count.

use bytemuck::{Pod, Zeroable};

/// One line endpoint: world position (camera/floating-origin frame,
/// same as `RenderObject.position`) + RGBA (a = alpha, blended).
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct LineVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

/// Append a horizontal CIRCLE (in the XZ plane) centred at `center`, radius `radius`, as `segments`
/// connected line segments, into `out`. (v0.568)
///
/// This is the reusable form of the "orbit path" primitive the operator asked us to keep handy: a
/// constant-width world-space line that stays ~1px regardless of how far away or how large it is
/// (unlike a polygon ring/strip, whose band thickness scales with its radius). Use it for any
/// floor/boundary ring -- a door's auto-open radius, a gizmo's reach, an area-of-effect marker.
pub fn push_circle(out: &mut Vec<LineVertex>, center: [f32; 3], radius: f32, color: [f32; 4], segments: usize) {
    let n = segments.max(3);
    let mut prev = [center[0] + radius, center[1], center[2]];
    for i in 1..=n {
        let a = (i as f32 / n as f32) * std::f32::consts::TAU;
        let cur = [center[0] + a.cos() * radius, center[1], center[2] + a.sin() * radius];
        out.push(LineVertex { position: prev, color });
        out.push(LineVertex { position: cur, color });
        prev = cur;
    }
}

/// Append a POLYLINE (an open connected path) through `points` as line segments into `out`. (v0.568)
///
/// The same constant-width primitive for an ARBITRARY 3D trajectory rather than a circle -- the path a
/// thrown grenade will arc along before you throw it, a gun's laser-pointer beam, a planned travel
/// route, a constellation. Open (does not close back to the first point); pass a closed point list if
/// you want a loop.
pub fn push_polyline(out: &mut Vec<LineVertex>, points: &[[f32; 3]], color: [f32; 4]) {
    for w in points.windows(2) {
        out.push(LineVertex { position: w[0], color });
        out.push(LineVertex { position: w[1], color });
    }
}

impl LineVertex {
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<LineVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Minimal WGSL: clip = camera.view_proj * vec4(pos,1); out = color.
/// `CameraUniforms` MUST match the layout the main pipeline binds at
/// group(0) (only `view_proj` is used here; the rest is padding so the
/// uniform size/offsets line up with the shared camera buffer).
const LINE_SHADER: &str = r#"
struct CameraUniforms {
    view_proj: mat4x4<f32>,
    view_pos: vec4<f32>,
    light0: vec4<f32>, light1: vec4<f32>, light2: vec4<f32>, light3: vec4<f32>,
    light4: vec4<f32>, light5: vec4<f32>, light6: vec4<f32>, light7: vec4<f32>,
    light0_color: vec4<f32>, light1_color: vec4<f32>, light2_color: vec4<f32>, light3_color: vec4<f32>,
    light4_color: vec4<f32>, light5_color: vec4<f32>, light6_color: vec4<f32>, light7_color: vec4<f32>,
    light_count: vec4<f32>,
    sun_direction: vec4<f32>,
    sun_color: vec4<f32>,
    fill_direction: vec4<f32>,
    fill_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniforms;

struct VsIn  { @location(0) pos: vec3<f32>, @location(1) color: vec4<f32> };
struct VsOut { @builtin(position) clip: vec4<f32>, @location(0) color: vec4<f32> };

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = camera.view_proj * vec4<f32>(in.pos, 1.0);
    o.color = in.color;
    return o;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn circle_emits_two_verts_per_segment_starting_at_plus_x() {
        let mut v = Vec::new();
        push_circle(&mut v, [2.0, 0.5, -3.0], 1.0, [1.0, 1.0, 1.0, 1.0], 8);
        assert_eq!(v.len(), 16, "8 segments -> 16 line verts");
        // The ring starts at center + (radius, 0, 0) and stays in the y plane.
        assert!((v[0].position[0] - 3.0).abs() < 1e-5 && (v[0].position[1] - 0.5).abs() < 1e-5);
        // It closes: the last endpoint returns to the start.
        assert!((v[15].position[0] - 3.0).abs() < 1e-4 && (v[15].position[2] + 3.0).abs() < 1e-4);
    }

    #[test]
    fn polyline_connects_consecutive_points() {
        let mut v = Vec::new();
        push_polyline(&mut v, &[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]], [1.0; 4]);
        assert_eq!(v.len(), 4, "3 points -> 2 segments -> 4 verts");
        assert_eq!(v[1].position, [1.0, 0.0, 0.0]);
        assert_eq!(v[2].position, [1.0, 0.0, 0.0]);
    }
}

/// Build the world-space line pipeline. `camera_bgl` is the SAME
/// group(0) layout the main PBR pipeline uses, so the existing
/// `camera_bind_group` (full view-proj) can be bound directly.
pub fn build_line_pipeline(
    device: &wgpu::Device,
    surface_format: wgpu::TextureFormat,
    camera_bgl: &wgpu::BindGroupLayout,
) -> wgpu::RenderPipeline {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("World Line Shader"),
        source: wgpu::ShaderSource::Wgsl(LINE_SHADER.into()),
    });
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("World Line Pipeline Layout"),
        bind_group_layouts: &[camera_bgl],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("World Line Pipeline"),
        layout: Some(&layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[LineVertex::layout()],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: surface_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent::OVER,
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            ..Default::default()
        },
        // Reverse-Z: a fragment passes only if NEARER than what's stored
        // (Greater). Orbit points behind a planet are farther → fail →
        // occluded by the planet. depth_write = false so overlapping
        // orbit lines don't occlude each other or anything else.
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: false,
            depth_compare: wgpu::CompareFunction::Greater,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    })
}
