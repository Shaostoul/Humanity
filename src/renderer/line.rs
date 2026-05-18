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
