//! Crepuscular god rays (v0.895): one additive full-screen pass that marches
//! the DEPTH buffer toward the sun's screen position, so terrain silhouettes
//! (still in depth right after the celestial pass) carve visible light
//! shafts at low sun angles. No offscreen scene copy, no post chain — the
//! pass samples only the shared depth texture, so it slots between the
//! celestial and interior passes with zero frame-graph surgery.
//! Shader: assets/shaders/godrays.wgsl.

use bytemuck::{Pod, Zeroable};
use glam::{DVec3, Mat4, Vec3};

/// Geometric sun visibility along the camera->sun segment against one
/// occluding sphere (v0.921, operator: "god rays are peaking around the
/// planet and through the planet... Earth between us and the sun"). The
/// screen-space march can only test occluders that are IN the frame - with
/// the sun off-screen behind a planet, sky pixels read as lit and shafts
/// leak across the night side. This is the CPU-side truth the pass scales
/// by: 0 = sun fully behind the sphere, 1 = clear, smooth across ~3% past
/// the limb so the fade never pops. Pure geometry, so a Moon eclipse dims
/// the rays for free.
pub fn segment_sphere_visibility(cam: DVec3, sun: DVec3, center: DVec3, radius: f64) -> f32 {
    let to_sun = sun - cam;
    let dist = to_sun.length();
    if dist <= radius.max(1.0) {
        return 1.0; // degenerate: camera essentially at the sun
    }
    let dir = to_sun / dist;
    let oc = center - cam;
    let tca = oc.dot(dir);
    // Only a body BETWEEN the camera and the sun occludes; behind the
    // camera or beyond the sun it cannot block the segment.
    if tca <= 0.0 || tca >= dist {
        return 1.0;
    }
    // Camera INSIDE the sphere (standing on a planet looking up counts -
    // the planet center is "between" but the near hemisphere is below the
    // horizon): the impact-parameter test below still answers correctly,
    // because at ground level the segment grazes at b ~ cam distance from
    // center, which exceeds radius exactly when the sun is up.
    let b = (oc - dir * tca).length();
    let t = ((b / radius - 1.0) / 0.03).clamp(0.0, 1.0) as f32;
    t * t * (3.0 - 2.0 * t)
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct GodrayUniforms {
    sun_uv: [f32; 2],
    aspect: f32,
    intensity: f32,
    color: [f32; 4],
}

pub struct GodrayPass {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    param_buffer: wgpu::Buffer,
}

impl GodrayPass {
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Godray Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../assets/shaders/godrays.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Godray BGL"),
            entries: &[
                // Depth texture (sampled with textureLoad — no sampler).
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let param_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Godray Params"),
            size: std::mem::size_of::<GodrayUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Godray Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Godray Pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    // SCREEN blend (v0.897, was plain additive): out = src *
                    // (1 - dst) + dst. Dark sky receives the full shafts;
                    // already-bright pixels (sunlit cloud decks) receive
                    // almost nothing, so the rays can never blow a white
                    // cloud out further (operator: "god rays are blowing out
                    // the clouds with super white").
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::OneMinusDst,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            // No depth attachment: the pass READS depth as a texture.
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self { pipeline, bind_group_layout, param_buffer }
    }

    /// Draw the shafts onto `view`. `view_proj` must be the SAME matrix the
    /// depth buffer was rendered with (the celestial camera), `cam_pos` its
    /// eye position, `sun_dir` the world-space direction TOWARD the sun.
    /// Skips itself when the sun projects behind the camera.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        depth_view: &wgpu::TextureView,
        view: &wgpu::TextureView,
        view_proj: Mat4,
        cam_pos: Vec3,
        sun_dir: Vec3,
        aspect: f32,
        intensity: f32,
    ) {
        if intensity <= 0.001 || sun_dir.length_squared() < 0.5 {
            return;
        }
        // Project a point far along the sun direction into clip space.
        let clip = view_proj * (cam_pos + sun_dir * 1.0e9).extend(1.0);
        if clip.w <= 0.0 {
            return; // sun behind the camera — no shafts to draw
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        // 1 Hz diag: where the sun lands on screen (dev tooling; the sun-uv
        // placement is unverifiable from screenshots alone when the disc is
        // washed out by the atmosphere).
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            static LAST: AtomicU64 = AtomicU64::new(0);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if LAST.swap(now, Ordering::Relaxed) != now {
                log::info!("[Godray] sun ndc=({ndc_x:.2},{ndc_y:.2}) w={:.0}", clip.w);
            }
        }
        // Well off-screen: the glow falloff would zero everything anyway.
        if ndc_x.abs() > 2.5 || ndc_y.abs() > 2.5 {
            return;
        }
        let sun_uv = [ndc_x * 0.5 + 0.5, 1.0 - (ndc_y * 0.5 + 0.5)];

        queue.write_buffer(
            &self.param_buffer,
            0,
            bytemuck::bytes_of(&GodrayUniforms {
                sun_uv,
                aspect,
                intensity,
                // Warm low-sun light; the daylight gate on the Rust side
                // scales intensity, the tint stays constant.
                color: [1.0, 0.86, 0.62, 0.0],
            }),
        );

        // The depth view can be recreated on resize, so bind fresh each call
        // (same per-apply pattern as the bloom pass).
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Godray BG"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.param_buffer.as_entire_binding(),
                },
            ],
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Godray Encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Godray Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 6_371_000.0; // Earth
    const AU: f64 = 1.496e11;

    #[test]
    fn sun_behind_the_planet_kills_the_rays() {
        // Camera in orbit on the night side: Earth dead-center between the
        // camera and the sun (the operator's station screenshots).
        let cam = DVec3::new(R + 400_000.0, 0.0, 0.0);
        let sun = DVec3::new(-AU, 0.0, 0.0);
        assert_eq!(segment_sphere_visibility(cam, sun, DVec3::ZERO, R), 0.0);
    }

    #[test]
    fn clear_line_of_sight_keeps_full_rays() {
        // Sun on the SAME side as the camera: nothing between them.
        let cam = DVec3::new(R + 400_000.0, 0.0, 0.0);
        let sun = DVec3::new(AU, 0.0, 0.0);
        assert_eq!(segment_sphere_visibility(cam, sun, DVec3::ZERO, R), 1.0);
        // Body far off to the side of the segment: clear.
        let side = DVec3::new(0.0, 50.0 * R, 0.0);
        assert_eq!(segment_sphere_visibility(cam, sun, side, R), 1.0);
    }

    #[test]
    fn limb_grazing_fades_smoothly_instead_of_popping() {
        // March the sun past the limb: visibility must rise 0 -> 1 through
        // intermediate values (the soft 3% window), monotonically.
        let cam = DVec3::new(2.0 * R, 0.0, 0.0);
        let mut prev = -1.0_f32;
        let mut saw_partial = false;
        for i in 0..=100 {
            // Sun sweeps from straight behind the planet to well clear.
            // From 2R away the disc subtends ~30 degrees, so sweep to 45.
            let y = (i as f64 / 100.0) * AU;
            let v = segment_sphere_visibility(
                cam,
                DVec3::new(-AU, y, 0.0),
                DVec3::ZERO,
                R,
            );
            assert!(v >= prev - 1e-6, "visibility regressed mid-sweep at {i}");
            if v > 0.0 && v < 1.0 {
                saw_partial = true;
            }
            prev = v;
        }
        assert_eq!(prev, 1.0, "sweep must end clear");
        assert!(saw_partial, "no soft limb transition seen");
    }

    #[test]
    fn ground_level_horizon_behaves() {
        // Standing on the surface: sun overhead = full rays, sun below the
        // horizon = none (the planet itself is the occluder).
        let cam = DVec3::new(R + 2.0, 0.0, 0.0);
        let up_sun = DVec3::new(AU, 0.0, 0.0);
        assert_eq!(segment_sphere_visibility(cam, up_sun, DVec3::ZERO, R), 1.0);
        // Sun 5 degrees below the horizon plane.
        let e = (-5.0_f64).to_radians();
        let below = DVec3::new(AU * e.sin(), AU * e.cos(), 0.0);
        assert_eq!(segment_sphere_visibility(cam, below, DVec3::ZERO, R), 0.0);
    }

    #[test]
    fn a_body_behind_the_camera_or_beyond_the_sun_never_occludes() {
        let cam = DVec3::ZERO;
        let sun = DVec3::new(AU, 0.0, 0.0);
        // Behind the camera.
        assert_eq!(
            segment_sphere_visibility(cam, sun, DVec3::new(-10.0 * R, 0.0, 0.0), R),
            1.0
        );
        // Beyond the sun.
        assert_eq!(
            segment_sphere_visibility(cam, sun, DVec3::new(AU + 10.0 * R, 0.0, 0.0), R),
            1.0
        );
    }
}
