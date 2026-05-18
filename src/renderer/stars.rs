//! Star skybox renderer — draws 119,627 real stars as colored points.
//!
//! Stars are rendered at infinity using a rotation-only camera matrix.
//! The star catalog (HYG database) provides x,y,z Cartesian positions
//! in parsecs, apparent magnitude, and B-V color index. These are
//! converted to direction vectors, brightness, and RGB color at load time.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4};
use std::path::Path;
use wgpu::util::DeviceExt;

use super::camera::CameraUniforms;

/// Per-star vertex data uploaded to GPU.
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct StarVertex {
    /// Unit direction vector on the celestial sphere.
    direction: [f32; 3],
    /// RGB color + brightness in the w channel.
    color_brightness: [f32; 4],
}

/// Renders the star field as a point-list with a dedicated pipeline.
pub struct StarRenderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    star_count: u32,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    /// Constellation figure lines (v0.262.18). LineList pipeline reusing
    /// the same rotation-only camera + shader so the figures stay locked
    /// to the celestial sphere exactly like the stars. Endpoints are
    /// resolved against the SAME stars.csv the skybox draws (by HYG
    /// `proper` name) so they overlay the real stars by construction —
    /// no RA/Dec convention to get wrong.
    line_pipeline: Option<wgpu::RenderPipeline>,
    constellation_buffer: Option<wgpu::Buffer>,
    constellation_vertex_count: u32,
}

impl StarRenderer {
    /// Load stars from CSV and create the GPU resources.
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        csv_path: &Path,
    ) -> Option<Self> {
        let vertices = load_stars_csv(csv_path)?;
        let star_count = vertices.len() as u32;
        if star_count == 0 {
            log::warn!("No stars loaded from {}", csv_path.display());
            return None;
        }
        log::info!("Loaded {} stars for skybox", star_count);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Star Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Camera bind group layout (same structure as main pipeline group 0)
        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Star Camera BGL"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<CameraUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Star Camera Buffer"),
            size: std::mem::size_of::<CameraUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Star Camera BG"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        // Load star shader
        let shader_path = Path::new("assets/shaders/stars.wgsl");
        let shader_src = if shader_path.exists() {
            std::fs::read_to_string(shader_path).unwrap_or_else(|_| FALLBACK_STAR_SHADER.to_string())
        } else {
            // Try relative to exe
            let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf()));
            if let Some(dir) = exe_dir {
                let alt = dir.join("assets/shaders/stars.wgsl");
                if alt.exists() {
                    std::fs::read_to_string(alt).unwrap_or_else(|_| FALLBACK_STAR_SHADER.to_string())
                } else {
                    FALLBACK_STAR_SHADER.to_string()
                }
            } else {
                FALLBACK_STAR_SHADER.to_string()
            }
        };

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Star Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Star Pipeline Layout"),
            bind_group_layouts: &[&camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Star Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<StarVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // location(0): direction
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // location(1): color_brightness
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
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
                topology: wgpu::PrimitiveTopology::PointList,
                ..Default::default()
            },
            // No depth testing for stars (they are at infinity, behind everything)
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // ── Constellation figure lines (v0.262.18) ──
        // Same shader + camera BGL + vertex layout + blend; only the
        // topology differs (LineList). Endpoints resolved against the
        // same stars.csv the skybox renders, so they overlay the real
        // stars with zero coordinate-convention risk.
        let constell_verts = load_constellations(csv_path);
        let (line_pipeline, constellation_buffer, constellation_vertex_count) =
            if constell_verts.len() >= 2 {
                let buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Constellation Line Buffer"),
                    contents: bytemuck::cast_slice(&constell_verts),
                    usage: wgpu::BufferUsages::VERTEX,
                });
                let lp = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("Constellation Line Pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader_module,
                        entry_point: Some("vs_main"),
                        buffers: &[wgpu::VertexBufferLayout {
                            array_stride: std::mem::size_of::<StarVertex>() as u64,
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
                        }],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader_module,
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
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
                log::info!(
                    "Constellation lines: {} segments",
                    constell_verts.len() / 2
                );
                (Some(lp), Some(buf), constell_verts.len() as u32)
            } else {
                log::warn!("Constellation lines: none resolved (no overlay)");
                (None, None, 0)
            };

        Some(Self {
            pipeline,
            vertex_buffer,
            star_count,
            camera_buffer,
            camera_bind_group,
            line_pipeline,
            constellation_buffer,
            constellation_vertex_count,
        })
    }

    /// Update the star camera uniform with a rotation-only view-projection.
    /// This strips translation so stars don't shift when the camera moves.
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &super::camera::Camera) {
        let view = camera.view_matrix();
        // Extract 3x3 rotation, discard translation
        let rot = Mat3::from_mat4(view);
        let rot_view = Mat4::from_mat3(rot);
        let proj = camera.projection_matrix();
        let star_vp = proj * rot_view;

        let uniforms = CameraUniforms {
            view_proj: star_vp.to_cols_array_2d(),
            view_pos: [0.0, 0.0, 0.0, 1.0],
            light_positions: [[0.0; 4]; 8],
            light_colors: [[0.0; 4]; 8],
            light_count: [0.0; 4],
            sun_direction: [0.0; 4],
            sun_color: [0.0; 4],
            fill_direction: [0.0; 4],
            fill_color: [0.0; 4],
        };
        queue.write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniforms));
    }

    /// Render stars into the given render pass.
    /// Call this BEFORE the main scene pass. The caller should have already
    /// begun a render pass that clears to black.
    pub fn render_pass<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.star_count, 0..1);

        // Constellation figures — same rotation-only camera, drawn over
        // the stars. Faint, so they read as figures without competing.
        if let (Some(lp), Some(buf)) = (&self.line_pipeline, &self.constellation_buffer) {
            render_pass.set_pipeline(lp);
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, buf.slice(..));
            render_pass.draw(0..self.constellation_vertex_count, 0..1);
        }
    }
}

// ── CSV Loading ──────────────────────────────────────────────

fn load_stars_csv(path: &Path) -> Option<Vec<StarVertex>> {
    let data = std::fs::read_to_string(path).ok()?;
    let mut lines = data.lines();
    let header = lines.next()?;

    // Find column indices
    let cols: Vec<&str> = header.split(',').map(|s| s.trim().trim_matches('"')).collect();
    let idx = |name: &str| cols.iter().position(|&c| c == name);
    let x_idx = idx("x")?;
    let y_idx = idx("y")?;
    let z_idx = idx("z")?;
    let mag_idx = idx("mag")?;
    let ci_idx = idx("ci")?;

    let mut vertices = Vec::with_capacity(120_000);

    for line in lines {
        let fields: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        if fields.len() <= ci_idx {
            continue;
        }

        let x: f64 = fields[x_idx].parse().unwrap_or(0.0);
        let y: f64 = fields[y_idx].parse().unwrap_or(0.0);
        let z: f64 = fields[z_idx].parse().unwrap_or(0.0);
        let mag: f64 = fields[mag_idx].parse().unwrap_or(20.0);
        let ci: f64 = fields[ci_idx].parse().unwrap_or(0.0);

        // Skip the Sun (at origin) and stars with zero position
        let len = (x * x + y * y + z * z).sqrt();
        if len < 0.001 {
            continue;
        }

        // Normalize to unit direction
        let dx = (x / len) as f32;
        let dy = (y / len) as f32;
        let dz = (z / len) as f32;

        // Magnitude to brightness (naked eye limit ~6.5)
        let brightness = 10.0_f64.powf((6.5 - mag) / 2.5).clamp(0.0, 1.0) as f32;
        // Skip extremely dim stars (saves GPU)
        if brightness < 0.001 {
            continue;
        }

        let [r, g, b] = ci_to_rgb(ci as f32);

        vertices.push(StarVertex {
            direction: [dx, dy, dz],
            color_brightness: [r, g, b, brightness],
        });
    }

    log::info!("Parsed {} visible stars from CSV (filtered by brightness)", vertices.len());
    Some(vertices)
}

/// Build constellation figure lines as a LineList (vertex pairs).
///
/// Endpoints are resolved by HYG `proper` name against the SAME
/// stars.csv the skybox renders, so the figures overlay the real
/// stars by construction — there is no RA/Dec→xyz conversion here to
/// get mirrored or rotated. `constellations.json` sits next to
/// stars.csv (data/). Unresolved endpoints just skip that segment.
fn load_constellations(csv_path: &Path) -> Vec<StarVertex> {
    // Operator-chosen #224444 — visible but unobtrusive.
    const LINE_RGBA: [f32; 4] = [0.133, 0.267, 0.267, 1.0];

    // 1. proper-name → unit direction, from the very stars.csv we draw.
    let csv = match std::fs::read_to_string(csv_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut rows = csv.lines();
    let header = match rows.next() {
        Some(h) => h,
        None => return Vec::new(),
    };
    let cols: Vec<&str> = header.split(',').map(|s| s.trim().trim_matches('"')).collect();
    let col = |n: &str| cols.iter().position(|&c| c == n);
    let (pi, xi, yi, zi) = match (col("proper"), col("x"), col("y"), col("z")) {
        (Some(p), Some(x), Some(y), Some(z)) => (p, x, y, z),
        _ => return Vec::new(),
    };
    let mut by_name: std::collections::HashMap<String, [f32; 3]> =
        std::collections::HashMap::new();
    for line in rows {
        let f: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
        let need = pi.max(xi).max(yi).max(zi);
        if f.len() <= need {
            continue;
        }
        let name = f[pi].trim();
        if name.is_empty() {
            continue;
        }
        let x: f64 = f[xi].parse().unwrap_or(0.0);
        let y: f64 = f[yi].parse().unwrap_or(0.0);
        let z: f64 = f[zi].parse().unwrap_or(0.0);
        let len = (x * x + y * y + z * z).sqrt();
        if len < 0.001 {
            continue;
        }
        by_name.insert(
            name.to_ascii_lowercase(),
            [(x / len) as f32, (y / len) as f32, (z / len) as f32],
        );
    }
    if by_name.is_empty() {
        return Vec::new();
    }

    // 2. constellations.json (next to stars.csv) → resolved segments.
    let cj = match csv_path
        .parent()
        .map(|d| d.join("constellations.json"))
        .and_then(|p| std::fs::read_to_string(p).ok())
    {
        Some(s) => s,
        None => return Vec::new(),
    };
    let root: serde_json::Value = match serde_json::from_str(&cj) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match root.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    let mut out: Vec<StarVertex> = Vec::new();
    let mut total = 0usize;
    let mut resolved = 0usize;
    let mut unresolved: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for con in arr {
        let Some(lines) = con.get("lines").and_then(|l| l.as_array()) else { continue };
        for pair in lines {
            let Some(p) = pair.as_array() else { continue };
            let (Some(a), Some(b)) = (
                p.first().and_then(|v| v.as_str()),
                p.get(1).and_then(|v| v.as_str()),
            ) else { continue };
            total += 1;
            let da = by_name.get(&a.to_ascii_lowercase());
            let db = by_name.get(&b.to_ascii_lowercase());
            if da.is_none() && unresolved.len() < 24 { unresolved.insert(a.to_string()); }
            if db.is_none() && unresolved.len() < 24 { unresolved.insert(b.to_string()); }
            let (Some(da), Some(db)) = (da, db) else { continue };
            resolved += 1;
            out.push(StarVertex { direction: *da, color_brightness: LINE_RGBA });
            out.push(StarVertex { direction: *db, color_brightness: LINE_RGBA });
        }
    }
    // Measure, don't guess: if coverage is low the figures look broken
    // because endpoints silently dropped — this tells us by how much
    // and which names stars.csv `proper` didn't have.
    log::info!(
        "Constellation lines: {}/{} segments resolved via stars.csv proper; \
         {} stars (proper-named) in lookup; sample unresolved: {:?}",
        resolved,
        total,
        by_name.len(),
        unresolved.iter().take(12).collect::<Vec<_>>()
    );
    out
}

/// Convert B-V color index to RGB.
/// Approximation based on the Planck spectrum / stellar classification.
fn ci_to_rgb(ci: f32) -> [f32; 3] {
    // Clamp to valid range
    let ci = ci.clamp(-0.4, 2.0);

    let r;
    let g;
    let b;

    if ci < 0.0 {
        // Hot blue-white stars (O/B type)
        r = 0.6 + ci * 0.5; // 0.8 at ci=-0.4, 0.6 at ci=0
        g = 0.7 + ci * 0.25;
        b = 1.0;
    } else if ci < 0.4 {
        // White to yellow-white (A/F type)
        r = 0.6 + ci * 1.0; // 0.6 to 1.0
        g = 0.7 + ci * 0.75; // 0.7 to 1.0
        b = 1.0 - ci * 0.5; // 1.0 to 0.8
    } else if ci < 0.8 {
        // Yellow (G type, like our Sun)
        let t = (ci - 0.4) / 0.4;
        r = 1.0;
        g = 1.0 - t * 0.15; // 1.0 to 0.85
        b = 0.8 - t * 0.3; // 0.8 to 0.5
    } else if ci < 1.4 {
        // Orange (K type)
        let t = (ci - 0.8) / 0.6;
        r = 1.0;
        g = 0.85 - t * 0.35; // 0.85 to 0.5
        b = 0.5 - t * 0.3; // 0.5 to 0.2
    } else {
        // Red (M type)
        let t = (ci - 1.4) / 0.6;
        r = 1.0 - t * 0.2; // 1.0 to 0.8
        g = 0.5 - t * 0.2; // 0.5 to 0.3
        b = 0.2 - t * 0.1; // 0.2 to 0.1
    }

    [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)]
}

/// Embedded fallback shader in case the external file isn't found.
const FALLBACK_STAR_SHADER: &str = r#"
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
@group(0) @binding(0)
var<uniform> camera: CameraUniforms;

struct StarInput {
    @location(0) direction: vec3<f32>,
    @location(1) color_brightness: vec4<f32>,
};

struct StarOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) brightness: f32,
};

@vertex
fn vs_main(input: StarInput) -> StarOutput {
    var out: StarOutput;
    let world_pos = input.direction * 5000.0;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.color = input.color_brightness.rgb;
    out.brightness = input.color_brightness.a;
    return out;
}

@fragment
fn fs_main(input: StarOutput) -> @location(0) vec4<f32> {
    let intensity = input.brightness;
    let color = input.color * intensity;
    return vec4<f32>(color, intensity);
}
"#;
