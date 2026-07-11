//! Star skybox renderer — draws 119,625 real stars as colored points.
//!
//! Stars are rendered at infinity using a rotation-only camera matrix.
//! The star catalog (HYG database) ships as `data/stars.bin`, a compact
//! ~1.8 MB binary generated from the 34 MB `data/stars.csv` by
//! `scripts/build-stars-bin.js`. It is parsed ONCE into a [`StarCatalog`]
//! shared by the skybox vertex builder and the constellation-figure
//! resolver (before v0.797 the CSV was read and parsed TWICE at startup).
//! Unit directions, apparent magnitude and B-V color index are converted
//! to brightness and RGB at load time.

use bytemuck::{Pod, Zeroable};
use glam::{Mat3, Mat4};
use std::collections::HashMap;
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
    /// CPU copy of the constellation line verts (v0.786) so the color can be
    /// re-applied from the theme's `constellation_line` token without
    /// re-reading the 34 MB stars.csv.
    constellation_verts: Vec<StarVertex>,
    /// Current baked line color; `set_constellation_style` rebuilds the GPU
    /// buffer only when this changes.
    constellation_rgba: [f32; 4],
    /// Draw the figures at all? (Settings > Graphics > Constellation figures.)
    pub show_constellations: bool,
}

impl StarRenderer {
    /// Build the GPU resources from an already-parsed [`StarCatalog`]
    /// (v0.797: the catalog is parsed ONCE by the caller and shared with
    /// the constellation resolver; this function no longer touches the
    /// catalog files itself, only `constellations.json` under `data_dir`).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        catalog: &StarCatalog,
        data_dir: &Path,
    ) -> Option<Self> {
        let mut vertices = catalog.skybox_vertices();
        if vertices.is_empty() {
            log::warn!("No visible stars in the star catalog");
            return None;
        }
        // Milky Way band (v0.452): the real HYG catalog is only ~120k NEARBY stars, which
        // do NOT reproduce the galactic band you see in a dark sky (that glow is billions of
        // far, individually-unresolved stars along the galactic plane). Append a procedural
        // field of faint points concentrated near the plane, in the SAME equatorial frame as
        // the catalog, so the band sits where it really is (through Cygnus/Sagittarius) and
        // brightens toward the galactic centre. Purely cosmetic; it rides the same shader.
        let band = galactic_band_stars(7000);
        log::info!("Generated {} Milky Way band points", band.len());
        vertices.extend(band);
        let star_count = vertices.len() as u32;
        log::info!("Loaded {} skybox points (catalog + band)", star_count);

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
        // same catalog the skybox renders, so they overlay the real
        // stars with zero coordinate-convention risk.
        let constell_verts = load_constellations(catalog, data_dir);
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
            constellation_verts: constell_verts,
            constellation_rgba: [0.133, 0.267, 0.267, 1.0], // load_constellations' baked default
            show_constellations: true,
        })
    }

    /// Apply the Sky settings to the constellation figures (v0.786): the
    /// visibility toggle + the theme's `constellation_line` color. The GPU
    /// buffer is only rebuilt when the color actually changes (colors are
    /// baked per-vertex), so calling this every frame is nearly free.
    pub fn set_constellation_style(
        &mut self,
        device: &wgpu::Device,
        show: bool,
        rgba: [f32; 4],
    ) {
        self.show_constellations = show;
        if rgba == self.constellation_rgba || self.constellation_verts.is_empty() {
            return;
        }
        self.constellation_rgba = rgba;
        for v in &mut self.constellation_verts {
            v.color_brightness = rgba;
        }
        self.constellation_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Constellation Line Buffer"),
                contents: bytemuck::cast_slice(&self.constellation_verts),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
    }

    /// Update the star camera uniform with a rotation-only view-projection.
    /// This strips translation so stars don't shift when the camera moves.
    pub fn update_camera(&self, queue: &wgpu::Queue, camera: &super::camera::Camera) {
        let view = camera.view_matrix();
        // Extract 3x3 rotation, discard translation
        let rot = Mat3::from_mat4(view);
        let rot_view = Mat4::from_mat3(rot);
        // DEDICATED star projection (v0.446): the shader puts stars at 5000 units, but the
        // gameplay far plane is render_distance (default 500), which CLIPPED the entire
        // skybox (black void). Use a huge far here so the skybox is never clipped; x/y
        // (fov/aspect) match the gameplay camera, and the star pass is depthless so the
        // standard (non-reverse-Z) convention is fine.
        let proj = Mat4::perspective_rh(
            camera.fov_degrees.to_radians(),
            camera.aspect.max(0.01),
            1.0,
            100_000.0,
        );
        let star_vp = proj * rot_view;

        let uniforms = CameraUniforms {
            view_proj: star_vp.to_cols_array_2d(),
            view_pos: [0.0, 0.0, 0.0, 1.0],
            light_positions: [[0.0; 4]; 8],
            light_colors: [[0.0; 4]; 8],
            light_spot: [[0.0, -1.0, 0.0, -1.0]; 8],
            light_cone_inner: [[0.0; 4]; 8],
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
        // Constellation figures FIRST, stars on top (v0.787, operator: darker
        // lines were painting over star points and made them flicker as the
        // camera rotated). Lines sit BEHIND the stars now; the celestial pass
        // (orbit rings) draws later in the frame, so it stays in front.
        if self.show_constellations {
            if let (Some(lp), Some(buf)) = (&self.line_pipeline, &self.constellation_buffer) {
                render_pass.set_pipeline(lp);
                render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
                render_pass.set_vertex_buffer(0, buf.slice(..));
                render_pass.draw(0..self.constellation_vertex_count, 0..1);
            }
        }

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.star_count, 0..1);
    }
}

// ── Star catalog loading ─────────────────────────────────────

/// Build a procedural Milky Way band: `count` faint points clustered near the galactic
/// plane (v0.452). Coordinates are in the SAME equatorial J2000 frame as the HYG catalog
/// (x = vernal equinox, z = north celestial pole), so the band falls where it really does.
/// We place points by rejection-sampling uniform sphere directions, weighted by a Gaussian
/// in galactic latitude (how far off the plane), and brighten/warm them toward the galactic
/// centre (Sgr A*). Deterministic (a tiny xorshift seeded by a constant) so the sky is the
/// same every launch. No `rand` dependency.
fn galactic_band_stars(count: usize) -> Vec<StarVertex> {
    // ra/dec (degrees) -> unit equatorial Cartesian, matching the HYG x,y,z convention.
    let dir_from = |ra_deg: f64, dec_deg: f64| -> [f64; 3] {
        let (ra, dec) = (ra_deg.to_radians(), dec_deg.to_radians());
        [dec.cos() * ra.cos(), dec.cos() * ra.sin(), dec.sin()]
    };
    let ngp = dir_from(192.859_5, 27.128_3); // North Galactic Pole
    let gc = dir_from(266.405, -28.936); // Galactic centre (Sagittarius A*)
    let dot = |a: &[f64; 3], b: &[f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

    // xorshift64* PRNG -> f64 in [0,1). Seeded by a constant for a stable sky.
    let mut state: u64 = 0x9E3779B97F4A7C15;
    let mut next = || -> f64 {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let x = state.wrapping_mul(0x2545F4914F6CDD1D);
        // top 53 bits -> [0,1)
        (x >> 11) as f64 / (1u64 << 53) as f64
    };

    // Gaussian thickness of the band, in units of sin(galactic latitude). ~0.13 reads as a
    // soft band a few degrees thick that still has visible feathering at the edges.
    let sigma = 0.13_f64;
    let mut out = Vec::with_capacity(count);
    let mut attempts = 0usize;
    while out.len() < count && attempts < count * 60 {
        attempts += 1;
        // Uniform direction on the sphere.
        let z = 2.0 * next() - 1.0;
        let phi = 2.0 * std::f64::consts::PI * next();
        let r = (1.0 - z * z).max(0.0).sqrt();
        let dir = [r * phi.cos(), r * phi.sin(), z];
        // Galactic latitude proxy: component along the pole (0 = on the plane).
        let b = dot(&dir, &ngp);
        let accept = (-(b * b) / (sigma * sigma)).exp();
        if next() > accept {
            continue;
        }
        // Brighten + warm toward the galactic centre; dimmer/cooler in the anti-centre.
        let center = dot(&dir, &gc).max(0.0); // 0..1
        let base = 0.018 + 0.045 * center; // faint glow, not bright points
        // A little per-point variation so the band has texture, not a flat wash.
        let bright = (base * (0.6 + 0.8 * next())).clamp(0.0, 0.10) as f32;
        // Color: cool blue-white off-centre, warmer (dustier) toward the centre.
        let rr = (0.62 + 0.30 * center) as f32;
        let gg = (0.66 + 0.18 * center) as f32;
        let bb = (0.78 - 0.10 * center) as f32;
        out.push(StarVertex {
            direction: [dir[0] as f32, dir[1] as f32, dir[2] as f32],
            color_brightness: [rr, gg, bb, bright],
        });
    }
    out
}

/// stars.bin magic + layout constants. The FORMAT SPEC (header, record
/// layout, quantization, sidecar) is documented in the generator,
/// `scripts/build-stars-bin.js`; the round-trip tests at the bottom of this
/// file lock the two implementations together. Change all three in the same
/// commit or the tests fail.
const STARS_BIN_MAGIC: &[u8; 8] = b"HOSSTAR1";
const STARS_BIN_HEADER: usize = 16;
const STARS_BIN_RECORD: usize = 15;

/// One catalog star: unit direction plus the two scalar columns the renderer
/// derives visuals from. Kept as raw magnitude + B-V (not baked brightness /
/// RGB) so rendering policy (the naked-eye cutoff, the color curve) stays in
/// CODE and can be tuned without regenerating stars.bin.
#[derive(Clone, Copy, Debug)]
struct CatalogStar {
    direction: [f32; 3],
    mag: f32,
    ci: f32,
}

/// The star catalog, parsed ONCE at world load and shared by both consumers
/// (the skybox vertex builder and the constellation-figure name resolver).
/// Before v0.797 each consumer read and parsed the whole 34 MB stars.csv
/// independently, so startup paid for two full text parses; stars.bin is a
/// 1.8 MB binary with the same content parsed in one cheap pass.
pub struct StarCatalog {
    stars: Vec<CatalogStar>,
    /// HYG `proper` name (ASCII-lowercased) to unit direction. LAST
    /// occurrence wins, matching the old CSV pass's HashMap::insert.
    by_name: HashMap<String, [f32; 3]>,
    /// "bayer con" key (e.g. "alp lyr", ASCII-lowercased) to unit direction.
    /// FIRST occurrence wins: multi-component systems repeat the Bayer key,
    /// and the old CSV pass used entry().or_insert to keep the primary.
    by_bayer: HashMap<String, [f32; 3]>,
}

impl StarCatalog {
    /// Load the catalog from `data_dir`: stars.bin first (the shipped
    /// format), stars.csv as a fallback. Logs the parse duration so the
    /// startup win stays measurable in run.log.
    ///
    /// The CSV fallback survives the pre-launch no-compat rule for one
    /// reason: stars.csv has no embedded copy (34 MB is far too big to bake
    /// into the exe), so a data dir that predates stars.bin would otherwise
    /// lose the entire sky. Even the fallback now parses the file ONCE.
    pub fn load(data_dir: &Path) -> Option<Self> {
        let t0 = std::time::Instant::now();
        // Extended catalog first (v0.800, star ladder rung 2): ATHYG's ~2.5M
        // stars in the same HOSSTAR1 format, fetched on demand through
        // Settings > Graphics (38 MB is too big to ship in the repo). Its
        // sidecar carries the same proper/Bayer names, so constellations
        // resolve identically. A corrupt/truncated download falls through to
        // the standard catalog instead of costing the sky.
        let ext_path = data_dir.join("stars-athyg.bin");
        if let Ok(bytes) = std::fs::read(&ext_path) {
            if let Some(cat) = Self::from_bin(&bytes) {
                log::info!(
                    "Star catalog: {} stars + {} name keys from stars-athyg.bin (EXTENDED) in {} ms",
                    cat.stars.len(),
                    cat.by_name.len() + cat.by_bayer.len(),
                    t0.elapsed().as_millis()
                );
                return Some(cat);
            }
            log::warn!(
                "stars-athyg.bin at {} is corrupt; using the standard catalog (re-download from Settings > Graphics)",
                ext_path.display()
            );
        }
        let bin_path = data_dir.join("stars.bin");
        match std::fs::read(&bin_path) {
            Ok(bytes) => {
                if let Some(cat) = Self::from_bin(&bytes) {
                    log::info!(
                        "Star catalog: {} stars + {} name keys from stars.bin in {} ms",
                        cat.stars.len(),
                        cat.by_name.len() + cat.by_bayer.len(),
                        t0.elapsed().as_millis()
                    );
                    return Some(cat);
                }
                log::warn!(
                    "stars.bin at {} is corrupt; falling back to stars.csv",
                    bin_path.display()
                );
            }
            Err(_) => {
                log::warn!(
                    "stars.bin missing at {}; falling back to the 34 MB stars.csv \
                     (slower startup; regenerate with `node scripts/build-stars-bin.js`)",
                    bin_path.display()
                );
            }
        }
        let csv = std::fs::read_to_string(data_dir.join("stars.csv")).ok()?;
        let cat = Self::from_csv(&csv);
        log::info!(
            "Star catalog: {} stars + {} name keys from stars.csv in {} ms",
            cat.stars.len(),
            cat.by_name.len() + cat.by_bayer.len(),
            t0.elapsed().as_millis()
        );
        Some(cat)
    }

    /// Parse the HOSSTAR1 binary format (see scripts/build-stars-bin.js for
    /// the spec). Any structural inconsistency (bad magic, truncated body,
    /// unknown sidecar kind, non-UTF-8 key) returns None so the caller can
    /// fall back to the CSV instead of rendering garbage.
    fn from_bin(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < STARS_BIN_HEADER || &bytes[0..8] != STARS_BIN_MAGIC {
            return None;
        }
        let star_count = u32::from_le_bytes(bytes[8..12].try_into().ok()?) as usize;
        let named_count = u32::from_le_bytes(bytes[12..16].try_into().ok()?) as usize;
        let records_end = STARS_BIN_HEADER + star_count.checked_mul(STARS_BIN_RECORD)?;
        if bytes.len() < records_end {
            return None;
        }

        let read_f32 = |b: &[u8], o: usize| -> f32 {
            f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
        };

        let mut stars = Vec::with_capacity(star_count);
        for i in 0..star_count {
            let o = STARS_BIN_HEADER + i * STARS_BIN_RECORD;
            let mag_q = u16::from_le_bytes([bytes[o + 12], bytes[o + 13]]);
            let ci_q = bytes[o + 14];
            stars.push(CatalogStar {
                direction: [read_f32(bytes, o), read_f32(bytes, o + 4), read_f32(bytes, o + 8)],
                // Dequantize in f64 then narrow, mirroring the converter's
                // quantization domain (mag offset +2.0 at 1/1024 steps; ci
                // over [-0.4, 2.0] in 255 steps). Max round-trip error:
                // 0.00049 mag (< 0.05% brightness), 0.0047 ci (< 0.5% of an
                // RGB channel).
                mag: (mag_q as f64 / 1024.0 - 2.0) as f32,
                ci: (ci_q as f64 / 255.0 * 2.4 - 0.4) as f32,
            });
        }

        // Named-star sidecar: only stars carrying a proper name or a Bayer
        // designation (~2k entries), kept OUT of the fixed-size records so
        // the main table stays 15 bytes/star. Entry order is load-bearing:
        // it replays the CSV row order so the first-wins / last-wins map
        // semantics below match the old CSV parser exactly.
        let mut by_name: HashMap<String, [f32; 3]> = HashMap::new();
        let mut by_bayer: HashMap<String, [f32; 3]> = HashMap::new();
        let mut o = records_end;
        for _ in 0..named_count {
            if o + 2 > bytes.len() {
                return None;
            }
            let kind = bytes[o];
            let klen = bytes[o + 1] as usize;
            let end = o + 2 + klen + 12;
            if end > bytes.len() {
                return None;
            }
            let key = std::str::from_utf8(&bytes[o + 2..o + 2 + klen]).ok()?.to_string();
            let d = o + 2 + klen;
            let dir = [read_f32(bytes, d), read_f32(bytes, d + 4), read_f32(bytes, d + 8)];
            match kind {
                0 => {
                    by_name.insert(key, dir); // last wins (proper names)
                }
                1 => {
                    by_bayer.entry(key).or_insert(dir); // first wins (Bayer)
                }
                _ => return None,
            }
            o = end;
        }

        Some(Self { stars, by_name, by_bayer })
    }

    /// Single-pass CSV fallback parser. Builds the star list AND the name
    /// lookups in one walk over the file (the pre-v0.797 code walked the
    /// 34 MB file twice, once per consumer).
    fn from_csv(text: &str) -> Self {
        let mut stars = Vec::with_capacity(120_000);
        let mut by_name: HashMap<String, [f32; 3]> = HashMap::new();
        let mut by_bayer: HashMap<String, [f32; 3]> = HashMap::new();

        let mut lines = text.lines();
        let Some(header) = lines.next() else {
            return Self { stars, by_name, by_bayer };
        };
        let cols: Vec<&str> = header.split(',').map(|s| s.trim().trim_matches('"')).collect();
        let idx = |name: &str| cols.iter().position(|&c| c == name);
        let (Some(xi), Some(yi), Some(zi), Some(mi), Some(cii)) =
            (idx("x"), idx("y"), idx("z"), idx("mag"), idx("ci"))
        else {
            return Self { stars, by_name, by_bayer };
        };
        // Bayer designation fallback: ~11% of constellation endpoints have
        // no HYG `proper` name (e.g. "Alpha Lupi"); those resolve via the
        // `bayer` + `con` columns instead. See resolve_endpoint below.
        let pi = idx("proper");
        let bi = idx("bayer");
        let coni = idx("con");
        let max_idx = xi
            .max(yi)
            .max(zi)
            .max(mi)
            .max(cii)
            .max(pi.unwrap_or(0))
            .max(bi.unwrap_or(0))
            .max(coni.unwrap_or(0));

        for line in lines {
            let f: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
            if f.len() <= max_idx {
                continue;
            }
            let x: f64 = f[xi].parse().unwrap_or(0.0);
            let y: f64 = f[yi].parse().unwrap_or(0.0);
            let z: f64 = f[zi].parse().unwrap_or(0.0);
            // Skip the Sun (at the origin) and stars with zero position:
            // no direction exists for them.
            let len = (x * x + y * y + z * z).sqrt();
            if len < 0.001 {
                continue;
            }
            let direction = [(x / len) as f32, (y / len) as f32, (z / len) as f32];
            let mag: f64 = f[mi].parse().unwrap_or(20.0);
            let ci: f64 = f[cii].parse().unwrap_or(0.0);
            stars.push(CatalogStar {
                direction,
                mag: mag as f32,
                // Pre-clamp B-V to the ci_to_rgb domain, matching what the
                // bin quantization does at encode time (335 catalog stars
                // sit outside [-0.4, 2.0], up to 5.46). ci_to_rgb clamps to
                // the same domain anyway, so this changes NO pixel; it just
                // keeps csv-parsed and bin-parsed catalogs field-identical.
                ci: ci.clamp(-0.4, 2.0) as f32,
            });

            if let Some(pi) = pi {
                let name = f[pi].trim();
                if !name.is_empty() {
                    by_name.insert(name.to_ascii_lowercase(), direction);
                }
            }
            if let (Some(bi), Some(coni)) = (bi, coni) {
                let bay = f[bi].trim();
                let con = f[coni].trim();
                if !bay.is_empty() && !con.is_empty() {
                    by_bayer
                        .entry(format!(
                            "{} {}",
                            bay.to_ascii_lowercase(),
                            con.to_ascii_lowercase()
                        ))
                        .or_insert(direction);
                }
            }
        }

        Self { stars, by_name, by_bayer }
    }

    /// Build the render-ready skybox vertices: magnitude to brightness
    /// (naked-eye limit ~6.5), B-V to RGB, and the dim-star cutoff. This is
    /// rendering POLICY, deliberately applied here rather than baked into
    /// stars.bin, so tuning the curve or the cutoff never requires
    /// regenerating the data file.
    fn skybox_vertices(&self) -> Vec<StarVertex> {
        let mut vertices = Vec::with_capacity(self.stars.len());
        for s in &self.stars {
            let brightness =
                10.0_f64.powf((6.5 - s.mag as f64) / 2.5).clamp(0.0, 1.0) as f32;
            // Skip extremely dim stars (saves GPU)
            if brightness < 0.001 {
                continue;
            }
            let [r, g, b] = ci_to_rgb(s.ci);
            vertices.push(StarVertex {
                direction: s.direction,
                color_brightness: [r, g, b, brightness],
            });
        }
        vertices
    }

    /// Resolve a constellations.json endpoint name to a unit direction:
    /// proper name, then alias, then a direct "alp ori" Bayer key, then a
    /// "<Greek> <Genitive>" translation to a Bayer key (two-word genitives
    /// like "Canis Majoris" supported, v0.783).
    fn resolve_endpoint(&self, raw: &str) -> Option<[f32; 3]> {
        let lc = raw.to_ascii_lowercase();
        if let Some(d) = self.by_name.get(&lc) {
            return Some(*d);
        }
        if let Some(k) = proper_name_alias(&lc) {
            if let Some(d) = self.by_bayer.get(k) {
                return Some(*d);
            }
        }
        // Direct HYG Bayer key ("alp ori"), the exact form the catalog's
        // bayer+con columns use, so authored line data can bypass the Greek/
        // genitive translation entirely. (v0.783)
        if let Some(d) = self.by_bayer.get(&lc) {
            return Some(*d);
        }
        let parts: Vec<&str> = lc.split_whitespace().collect();
        if parts.len() >= 2 {
            if let Some(b) = greek_abbr(parts[0]) {
                let key = format!("{} {}", b, constellation_abbr(&parts[1..].join(" ")));
                if let Some(d) = self.by_bayer.get(&key) {
                    return Some(*d);
                }
            }
        }
        None
    }
}

// ── Constellation name translation tables ───────────────────
// Pure lookups, hoisted to module scope (v0.797) so both the renderer path
// and the unit tests exercise the exact same resolver code.

/// Greek word → HYG `bayer` 3-letter abbreviation.
fn greek_abbr(w: &str) -> Option<&'static str> {
    Some(match w {
        "alpha" => "alp", "beta" => "bet", "gamma" => "gam", "delta" => "del",
        "epsilon" => "eps", "zeta" => "zet", "eta" => "eta", "theta" => "the",
        "iota" => "iot", "kappa" => "kap", "lambda" => "lam", "mu" => "mu",
        "nu" => "nu", "xi" => "xi", "omicron" => "omi", "pi" => "pi",
        "rho" => "rho", "sigma" => "sig", "tau" => "tau", "upsilon" => "ups",
        "phi" => "phi", "chi" => "chi", "psi" => "psi", "omega" => "ome",
        _ => return None,
    })
}

/// Latin genitive → IAU `con` code. The first-3-letters rule breaks for a
/// DOZEN constellations (v0.783 fix -- e.g. "aquarii" is Aqr not "aqu", so
/// every Aquarius endpoint silently failed to resolve): full irregular map,
/// including the two-word genitives handled by resolve_endpoint.
fn constellation_abbr(genitive: &str) -> String {
    match genitive {
        "piscium" => "psc".into(),
        "scuti" => "sct".into(),
        "trianguli" => "tri".into(),
        "crucis" => "cru".into(),
        "aquarii" => "aqr".into(),
        "aquilae" => "aql".into(),
        "apodis" => "aps".into(),
        "cancri" => "cnc".into(),
        "corvi" => "crv".into(),
        "crateris" => "crt".into(),
        "hydrae" => "hya".into(),
        "hydri" => "hyi".into(),
        "phoenicis" => "phe".into(),
        "sagittae" => "sge".into(),
        "sagittarii" => "sgr".into(),
        "sculptoris" => "scl".into(),
        "canis majoris" => "cma".into(),
        "canis minoris" => "cmi".into(),
        "ursae majoris" => "uma".into(),
        "ursae minoris" => "umi".into(),
        "coronae borealis" => "crb".into(),
        "coronae australis" => "cra".into(),
        "leonis minoris" => "lmi".into(),
        "piscis austrini" => "psa".into(),
        "trianguli australis" => "tra".into(),
        "canum venaticorum" => "cvn".into(),
        g => g.chars().take(3).collect(),
    }
}

/// Old proper-name aliases HYG stores differently (or not at all) →
/// their Bayer key. Tiny, data-bounded.
fn proper_name_alias(lc: &str) -> Option<&'static str> {
    Some(match lc {
        "tsih" => "gam cas",
        "gienah cygni" => "eps cyg",
        "rukh" => "del cyg",
        "wei" => "eps sco",
        "muhlifain" => "gam cen",
        "minkar" => "eps crv",
        "labrum" => "del crt",
        "turais" => "rho pup",
        _ => return None,
    })
}

/// Walk the constellations.json array and resolve every segment endpoint
/// against the catalog. Returns (line-list vertices, resolved segment count,
/// total segment count). Split out of load_constellations so the tests can
/// prove bin-parsed and csv-parsed catalogs resolve identically without a
/// GPU.
fn resolve_constellation_segments(
    catalog: &StarCatalog,
    json_text: &str,
    rgba: [f32; 4],
) -> (Vec<StarVertex>, usize, usize) {
    let root: serde_json::Value = match serde_json::from_str(json_text) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), 0, 0),
    };
    let arr = match root.as_array() {
        Some(a) => a,
        None => return (Vec::new(), 0, 0),
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
            let da = catalog.resolve_endpoint(a);
            let db = catalog.resolve_endpoint(b);
            if da.is_none() && unresolved.len() < 24 { unresolved.insert(a.to_string()); }
            if db.is_none() && unresolved.len() < 24 { unresolved.insert(b.to_string()); }
            let (Some(da), Some(db)) = (da, db) else { continue };
            resolved += 1;
            out.push(StarVertex { direction: da, color_brightness: rgba });
            out.push(StarVertex { direction: db, color_brightness: rgba });
        }
    }
    log::info!(
        "Constellation lines: {}/{} segments resolved (proper + bayer/con + alias); \
         {} proper, {} bayer-keyed; still-unresolved: {:?}",
        resolved,
        total,
        catalog.by_name.len(),
        catalog.by_bayer.len(),
        unresolved.iter().take(12).collect::<Vec<_>>()
    );
    (out, resolved, total)
}

/// Build constellation figure lines as a LineList (vertex pairs).
///
/// Endpoints are resolved by HYG `proper` name / Bayer key against the SAME
/// catalog the skybox renders, so the figures overlay the real stars by
/// construction — there is no RA/Dec→xyz conversion here to get mirrored or
/// rotated. `constellations.json` sits in `data_dir` next to the catalog.
/// Unresolved endpoints just skip that segment.
fn load_constellations(catalog: &StarCatalog, data_dir: &Path) -> Vec<StarVertex> {
    // Operator-chosen #224444 — visible but unobtrusive.
    const LINE_RGBA: [f32; 4] = [0.133, 0.267, 0.267, 1.0];

    if catalog.by_name.is_empty() && catalog.by_bayer.is_empty() {
        return Vec::new();
    }
    let cj = match std::fs::read_to_string(data_dir.join("constellations.json")) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let (out, _resolved, _total) = resolve_constellation_segments(catalog, &cj, LINE_RGBA);
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
    light0_spot: vec4<f32>, light1_spot: vec4<f32>, light2_spot: vec4<f32>, light3_spot: vec4<f32>,
    light4_spot: vec4<f32>, light5_spot: vec4<f32>, light6_spot: vec4<f32>, light7_spot: vec4<f32>,
    light0_cone_inner: vec4<f32>, light1_cone_inner: vec4<f32>, light2_cone_inner: vec4<f32>, light3_cone_inner: vec4<f32>,
    light4_cone_inner: vec4<f32>, light5_cone_inner: vec4<f32>, light6_cone_inner: vec4<f32>, light7_cone_inner: vec4<f32>,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Tiny HYG-shaped fixture. Rows chosen to exercise every branch:
    /// Sol at the origin (dropped by the position filter), Sirius (proper
    /// name + Bayer), a Bayer-only star (the "Alpha Lupi" resolution path),
    /// a dim mag-15 star (kept in the catalog, filtered from the skybox),
    /// a duplicate Bayer key (first-wins), a duplicate proper name
    /// (last-wins), and an alias target (Wei = eps sco).
    const FIXTURE_CSV: &str = "\
id,proper,mag,ci,x,y,z,bayer,con
0,Sol,-26.7,0.656,0.000005,0.0,0.0,,
1,Sirius,-1.44,0.009,-0.494323,-1.451046,-2.176566,Alp,CMa
2,,2.3,-0.15,10.5,-3.2,7.7,Alp,Lup
3,,15.2,1.3,5.0,5.0,5.0,,
4,,4.4,0.5,1.0,2.0,3.0,Alp,Lyr
5,Vega,0.03,-0.001,20.0,-8.0,3.0,Alp,Lyr
6,Vega,1.0,0.2,-20.0,8.0,-3.0,,
7,,2.29,1.15,-5.4,-6.5,-2.5,Eps,Sco
";

    /// Test-only encoder mirroring scripts/build-stars-bin.js EXACTLY (same
    /// header, record layout, quantization and sidecar order/semantics). If
    /// the format changes, change the script, this encoder, and from_bin in
    /// the same commit; the round-trip test below is what locks them.
    fn encode_bin(csv: &str) -> Vec<u8> {
        let mut lines = csv.lines();
        let header = lines.next().expect("fixture has a header");
        let cols: Vec<&str> = header.split(',').map(|s| s.trim().trim_matches('"')).collect();
        let idx = |n: &str| cols.iter().position(|&c| c == n).expect("fixture column");
        let (xi, yi, zi, mi, cii) = (idx("x"), idx("y"), idx("z"), idx("mag"), idx("ci"));
        let (pi, bi, coni) = (idx("proper"), idx("bayer"), idx("con"));
        let max_idx = xi.max(yi).max(zi).max(mi).max(cii).max(pi).max(bi).max(coni);

        let mut records: Vec<u8> = Vec::new();
        let mut sidecar: Vec<u8> = Vec::new();
        let mut star_count: u32 = 0;
        let mut named_count: u32 = 0;
        for line in lines {
            let f: Vec<&str> = line.split(',').map(|s| s.trim().trim_matches('"')).collect();
            if f.len() <= max_idx {
                continue;
            }
            let x: f64 = f[xi].parse().unwrap_or(0.0);
            let y: f64 = f[yi].parse().unwrap_or(0.0);
            let z: f64 = f[zi].parse().unwrap_or(0.0);
            let len = (x * x + y * y + z * z).sqrt();
            if len < 0.001 {
                continue;
            }
            let dir = [(x / len) as f32, (y / len) as f32, (z / len) as f32];
            for c in dir {
                records.extend_from_slice(&c.to_le_bytes());
            }
            let mag: f64 = f[mi].parse().unwrap_or(20.0);
            let mag_q = (((mag + 2.0) * 1024.0).round().max(0.0).min(65535.0)) as u16;
            records.extend_from_slice(&mag_q.to_le_bytes());
            let ci: f64 = f[cii].parse().unwrap_or(0.0);
            let ci_q = ((((ci.clamp(-0.4, 2.0)) + 0.4) / 2.4 * 255.0).round()) as u8;
            records.push(ci_q);
            star_count += 1;

            let mut emit = |kind: u8, key: String| {
                let kb = key.as_bytes();
                sidecar.push(kind);
                sidecar.push(kb.len() as u8);
                sidecar.extend_from_slice(kb);
                for c in dir {
                    sidecar.extend_from_slice(&c.to_le_bytes());
                }
                named_count += 1;
            };
            if !f[pi].is_empty() {
                emit(0, f[pi].to_ascii_lowercase());
            }
            if !f[bi].is_empty() && !f[coni].is_empty() {
                emit(
                    1,
                    format!("{} {}", f[bi].to_ascii_lowercase(), f[coni].to_ascii_lowercase()),
                );
            }
        }

        let mut out = Vec::with_capacity(STARS_BIN_HEADER + records.len() + sidecar.len());
        out.extend_from_slice(STARS_BIN_MAGIC);
        out.extend_from_slice(&star_count.to_le_bytes());
        out.extend_from_slice(&named_count.to_le_bytes());
        out.extend_from_slice(&records);
        out.extend_from_slice(&sidecar);
        out
    }

    /// Round-trip: fixture CSV -> bin bytes -> from_bin must equal from_csv
    /// within the documented quantization tolerance (directions bit-exact,
    /// mag within 0.0005, ci within 0.0048), with identical name maps and
    /// identical skybox filtering.
    #[test]
    fn star_catalog_bin_roundtrip_matches_from_csv() {
        let from_csv = StarCatalog::from_csv(FIXTURE_CSV);
        let bin = encode_bin(FIXTURE_CSV);
        let from_bin = StarCatalog::from_bin(&bin).expect("valid bin parses");

        // Sol dropped, rows 1..=7 kept.
        assert_eq!(from_csv.stars.len(), 7);
        assert_eq!(from_bin.stars.len(), 7);
        for (i, (a, b)) in from_csv.stars.iter().zip(from_bin.stars.iter()).enumerate() {
            assert_eq!(a.direction, b.direction, "direction differs at star {i}");
            assert!((a.mag - b.mag).abs() <= 0.0005, "mag differs at star {i}: {} vs {}", a.mag, b.mag);
            assert!((a.ci - b.ci).abs() <= 0.0048, "ci differs at star {i}: {} vs {}", a.ci, b.ci);
        }

        // Name maps: identical keys and directions.
        assert_eq!(from_csv.by_name.len(), from_bin.by_name.len());
        assert_eq!(from_csv.by_bayer.len(), from_bin.by_bayer.len());
        for (k, v) in &from_csv.by_name {
            assert_eq!(from_bin.by_name.get(k), Some(v), "by_name key '{k}' differs");
        }
        for (k, v) in &from_csv.by_bayer {
            assert_eq!(from_bin.by_bayer.get(k), Some(v), "by_bayer key '{k}' differs");
        }
        // Last-wins for proper names: "vega" must be row 6's direction.
        let row6_dir = from_csv.stars[5].direction; // stars[0]=row1 ... stars[5]=row6
        assert_eq!(from_csv.by_name.get("vega"), Some(&row6_dir));
        // First-wins for Bayer keys: "alp lyr" must be row 4's direction.
        let row4_dir = from_csv.stars[3].direction;
        assert_eq!(from_csv.by_bayer.get("alp lyr"), Some(&row4_dir));
        assert_eq!(from_bin.by_bayer.get("alp lyr"), Some(&row4_dir));

        // Skybox vertices: same stars survive the brightness cutoff (the
        // mag-15.2 star is dropped by BOTH paths), and per-vertex visuals
        // agree within quantization tolerance.
        let vc = from_csv.skybox_vertices();
        let vb = from_bin.skybox_vertices();
        assert_eq!(vc.len(), 6, "dim star must be filtered");
        assert_eq!(vb.len(), 6, "quantized mag must not flip the cutoff here");
        for (i, (a, b)) in vc.iter().zip(vb.iter()).enumerate() {
            assert_eq!(a.direction, b.direction, "vertex direction differs at {i}");
            for c in 0..4 {
                assert!(
                    (a.color_brightness[c] - b.color_brightness[c]).abs() <= 0.006,
                    "vertex color/brightness ch{c} differs at {i}: {:?} vs {:?}",
                    a.color_brightness,
                    b.color_brightness
                );
            }
        }
    }

    /// Corrupt input must fail closed (None -> CSV fallback), never panic.
    #[test]
    fn star_catalog_from_bin_rejects_corrupt_input() {
        assert!(StarCatalog::from_bin(b"").is_none(), "empty");
        assert!(StarCatalog::from_bin(b"NOTSTARS\x00\x00\x00\x00\x00\x00\x00\x00").is_none(), "bad magic");
        let good = encode_bin(FIXTURE_CSV);
        assert!(StarCatalog::from_bin(&good[..good.len() - 4]).is_none(), "truncated sidecar");
        assert!(StarCatalog::from_bin(&good[..STARS_BIN_HEADER + 7]).is_none(), "truncated records");
        let mut bad_kind = good.clone();
        // First sidecar byte sits right after the records block.
        let side_off = STARS_BIN_HEADER + 7 * STARS_BIN_RECORD;
        bad_kind[side_off] = 9;
        assert!(StarCatalog::from_bin(&bad_kind).is_none(), "unknown sidecar kind");
    }

    /// The resolver must find endpoints through every lookup path (proper
    /// name, direct Bayer key, Greek+genitive translation incl. two-word
    /// genitives, and the alias table) on BOTH catalog sources, proving the
    /// constellation figures survive the bin migration unchanged.
    #[test]
    fn constellation_endpoints_resolve_identically_from_csv_and_bin() {
        let cats = [
            StarCatalog::from_csv(FIXTURE_CSV),
            StarCatalog::from_bin(&encode_bin(FIXTURE_CSV)).expect("bin parses"),
        ];
        for (which, cat) in cats.iter().enumerate() {
            // Proper name.
            assert!(cat.resolve_endpoint("Sirius").is_some(), "cat{which}: proper");
            // Greek + genitive -> Bayer ("alp lup" has no proper name).
            assert!(cat.resolve_endpoint("Alpha Lupi").is_some(), "cat{which}: greek+genitive");
            // Two-word genitive (irregular map): Alpha Canis Majoris = Sirius' Bayer key.
            assert!(cat.resolve_endpoint("Alpha Canis Majoris").is_some(), "cat{which}: two-word genitive");
            // Direct Bayer key, exactly as authored in some line data.
            assert!(cat.resolve_endpoint("alp cma").is_some(), "cat{which}: direct bayer");
            // Alias table: Wei -> eps sco.
            assert!(cat.resolve_endpoint("Wei").is_some(), "cat{which}: alias");
            // Unknown name stays unresolved (segment is skipped, not drawn wrong).
            assert!(cat.resolve_endpoint("Notarealstar Fakei").is_none(), "cat{which}: unknown");
        }

        // Both catalogs resolve a constellations.json snippet identically.
        let json = r#"[
            {"name":"Canis Major","lines":[["Sirius","Alpha Lupi"]]},
            {"name":"Fake","lines":[["Sirius","Notarealstar Fakei"]]}
        ]"#;
        let rgba = [0.1, 0.2, 0.3, 1.0];
        let (v0, r0, t0) = resolve_constellation_segments(&cats[0], json, rgba);
        let (v1, r1, t1) = resolve_constellation_segments(&cats[1], json, rgba);
        assert_eq!((r0, t0), (1, 2), "one resolvable of two segments");
        assert_eq!((r1, t1), (1, 2));
        assert_eq!(v0.len(), 2, "one segment = two line-list vertices");
        assert_eq!(v1.len(), 2);
        assert_eq!(v0[0].direction, v1[0].direction);
        assert_eq!(v0[1].direction, v1[1].direction);
    }

    /// FULL-CATALOG equivalence + timing, against the real committed data
    /// files. Ignored by default (it parses the 34 MB CSV, which is slow in
    /// debug builds); run manually after regenerating stars.bin:
    ///   cargo test --release --features native --lib real_stars_bin -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_stars_bin_matches_real_csv_full_catalog() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");

        let t0 = std::time::Instant::now();
        let bin_bytes = std::fs::read(root.join("stars.bin")).expect("data/stars.bin exists");
        let from_bin = StarCatalog::from_bin(&bin_bytes).expect("stars.bin parses");
        let bin_ms = t0.elapsed().as_millis();

        let t1 = std::time::Instant::now();
        let csv_text = std::fs::read_to_string(root.join("stars.csv")).expect("data/stars.csv exists");
        let from_csv = StarCatalog::from_csv(&csv_text);
        let csv_ms = t1.elapsed().as_millis();

        println!(
            "PARSE TIMING: stars.bin {} stars in {} ms | stars.csv single pass in {} ms \
             (pre-v0.797 startup paid ~2x the CSV number)",
            from_bin.stars.len(),
            bin_ms,
            csv_ms
        );

        // Same stars, bit-identical directions, quantization-bounded scalars.
        assert_eq!(from_bin.stars.len(), from_csv.stars.len());
        for (i, (a, b)) in from_csv.stars.iter().zip(from_bin.stars.iter()).enumerate() {
            assert_eq!(a.direction, b.direction, "direction differs at star {i}");
            assert!((a.mag - b.mag).abs() <= 0.0005, "mag differs at star {i}");
            assert!((a.ci - b.ci).abs() <= 0.0048, "ci differs at star {i}");
        }

        // Identical name maps.
        assert_eq!(from_bin.by_name.len(), from_csv.by_name.len());
        assert_eq!(from_bin.by_bayer.len(), from_csv.by_bayer.len());
        for (k, v) in &from_csv.by_name {
            assert_eq!(from_bin.by_name.get(k), Some(v), "by_name '{k}' differs");
        }
        for (k, v) in &from_csv.by_bayer {
            assert_eq!(from_bin.by_bayer.get(k), Some(v), "by_bayer '{k}' differs");
        }

        // Identical skybox output size (the brightness cutoff must not flip
        // for any star under mag quantization).
        let vb = from_bin.skybox_vertices();
        let vc = from_csv.skybox_vertices();
        assert_eq!(vb.len(), vc.len(), "visible-star count differs");
        println!("SKYBOX: {} visible stars from both sources", vb.len());

        // Identical constellation resolution against the real figure data.
        let cj = std::fs::read_to_string(root.join("constellations.json"))
            .expect("data/constellations.json exists");
        let rgba = [0.133, 0.267, 0.267, 1.0];
        let (_, res_bin, tot_bin) = resolve_constellation_segments(&from_bin, &cj, rgba);
        let (_, res_csv, tot_csv) = resolve_constellation_segments(&from_csv, &cj, rgba);
        assert_eq!((res_bin, tot_bin), (res_csv, tot_csv), "segment resolution differs");
        assert!(res_bin > 0, "no segments resolved at all");
        println!("CONSTELLATIONS: {res_bin}/{tot_bin} segments resolved from both sources");
    }
}
