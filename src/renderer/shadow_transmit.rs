//! THE PARTIAL CASTERS: glass and frosted film in the shadows (2026-11-13; docs/design/
//! local-light-shadows.md section 10, "Built: partial casters", and sun-cascades.md increment 4).
//!
//! The operator, 2026-11-13, approving the fruiting tent's follow-ups: "Let's give those a shot."
//! Until this file a see-through surface cast no shadow at all (`local_shadows::casts` takes only
//! opaque materials): a lamp over a tent lit the shelves inside as if the cover were not there,
//! and the noon sun came through the glass roof undimmed. A real frosted sheet passes about 80% of
//! the light face-on (less at a slant) and a pane of glass about 92%.
//!
//! HOW. Beside the depth atlas, a TRANSMITTANCE ATLAS with the same faces and tiles at half the
//! texels (`TRANSMIT_FORMAT`, `transmit_side`): the sheets near a shadowed light or in the near sun
//! tile are drawn into it after the depth passes, each writing what it passes (rgb, multiplied by
//! the blend, so two sheets pass the product) and how far from the light it is (alpha, kept by a
//! Min blend). The shading reads it in `shadow_atlas_view` (00-bindings-vertex.wgsl,
//! `shadow_transmit_at`): where the point shaded is farther from the light than the nearest sheet,
//! its light is multiplied by what the sheets pass, filtered over four texels each tested on its own
//! (`receiver_transmit`, the CPU twin). A view with no sheet in it is flagged so in the table (row 6's
//! y) and never reads the atlas, so a home without glass or film pays nothing in the scene pass but
//! one compare per shadowed light.
//!
//! WHAT A SHEET PASSES: `transmittance`, the CPU twin of `partial_transmittance` in
//! assets/shaders/shadow_transmit.wgsl (`transmit_formula_matches_the_shader`). A frosted film counts
//! where the light enters the cover (a face turned toward the light) and not where it leaves; glass
//! counts on both faces. The numbers come from each material: the film's `params2.y` (the share of a
//! lamp's light it lets through face-on, data/garden/grow_media.ron) and colour, glass's Fresnel
//! with f0 0.04 and its colour by its own opacity (`partial_of`). Glass is any see-through pane of
//! the procedural surface types (the home's roof and windows are type 1, the vehicles' type 4). A
//! glass slab between storeys is drawn as two sheets, so it passes two panes' worth (0.85 face-on
//! against one pane's 0.92): a known limit.
//!
//! THE PER-DRAW LOOP (this rung): sheets are few (the tents' covers, the windows, the glass roof), so
//! each is drawn once per view it falls in, through this file's own small pipeline (no megashader,
//! no material groups: its instance carries its matrix, its view and what it passes). The GPU-driven
//! path (shadow_gpu.rs) draws opaque casters only.
//!
//! NOTHING KEPT ACROSS FRAMES: the atlas is cleared and drawn again every frame a sheet is in a view;
//! the texture itself is a resource made with the depth atlas, at its level.

use glam::{Mat4, Vec3};

/// The transmittance atlas's texels: rgb what gets through, a the nearest sheet's distance (f16).
pub const TRANSMIT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
/// The transmittance atlas is this many times smaller than the depth atlas on each side: a sheet's
/// shadow is soft (a diffuser's), and the four-texel filter in the shading smooths its edge.
pub const TRANSMIT_DIVISOR: u32 = 2;
/// Bytes per texel of `TRANSMIT_FORMAT`.
pub const TRANSMIT_TEXEL_BYTES: u64 = 8;
/// The alpha a texel with no sheet in it is cleared to: farther than anything is from its light.
pub const NOTHING_M: f32 = 60_000.0;
/// Glass's reflectance face-on (n = 1.5), each of its two surfaces. LOCKSTEP with `GLASS_F0` in
/// shadow_transmit.wgsl.
pub const GLASS_F0: f32 = 0.04;
/// A see-through PANE: any of the procedural surface types (0 to 11) drawn see-through. The same
/// rule the shader's pane flip uses (90-fragment-main.wgsl, `material_type < 11.5 && out_alpha <
/// 0.999`): the home's glass roof and windows are drawn in type 1 at alpha 0.22 and 0.45, the
/// glass type 4 is the vehicles' and doors', and all of them are glass to the light.
pub const PANE_TYPES_BELOW: f32 = 11.5;
/// The frosted film's band (engine::fruiting_tent::MATERIAL_TYPE_FILM, `is_film` in 50-brdf.wgsl).
pub const FILM_TYPE: (f32, f32) = (26.5, 27.5);
/// One view in the pass's storage array: its matrix and its origin (`TransmitView`).
pub const VIEW_BYTES: u64 = 80;
/// One sheet instance: its model matrix (the view's index in column 0's w) and what it passes.
pub const INSTANCE_BYTES: u64 = 80;

const WGSL: &str = include_str!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/shaders/shadow_transmit.wgsl"));

/// What a sheet passes: its tint and, for a film, its face-on share (`t0 > 0`); glass is `t0 < 0`
/// (its share is its Fresnel). The instance's `part` vec4.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Partial {
    pub tint: [f32; 3],
    pub t0: f32,
}

impl Partial {
    pub fn to_vec4(self) -> [f32; 4] {
        [self.tint[0], self.tint[1], self.tint[2], self.t0]
    }
}

/// Whether a material casts a PARTIAL shadow, and what it passes: a see-through pane of glass (any
/// procedural surface type drawn see-through, `PANE_TYPES_BELOW`) or frosted film (type 27) that
/// gives off no light. A film passes its own `params2.y` face-on,
/// tinted by its colour scaled so its brightest channel is 1 (the film's colour is its hue; how much
/// gets through is `params2.y`). Glass passes its Fresnel, tinted toward its colour by its opacity
/// (a pane that covers a quarter of what is behind it with its colour colours a quarter of the
/// light: a clear window barely, a stained one more). An opaque material, one that glows, or any
/// other see-through type (water, holograms, the sky's shells) is None.
pub fn partial_of(material_type: f32, base_color: [f32; 4], params2: [f32; 4], emissive: f32) -> Option<Partial> {
    let a = base_color[3];
    if emissive > 0.05 || a >= 0.999 || a <= 0.0 {
        return None;
    }
    let hue = {
        let m = base_color[0].max(base_color[1]).max(base_color[2]);
        if m > 1e-4 {
            [base_color[0] / m, base_color[1] / m, base_color[2] / m]
        } else {
            [1.0; 3]
        }
    };
    let t = material_type;
    if t >= FILM_TYPE.0 && t < FILM_TYPE.1 {
        let t0 = params2[1].clamp(0.0, 1.0);
        // A film with no figure for what gets through (0) casts nothing partial: it is the plain
        // PBR of its colour (fruiting_tent's fittings at full opacity never get here).
        return (t0 > 0.0).then_some(Partial { tint: hue, t0 });
    }
    if t < PANE_TYPES_BELOW {
        let tint = [1.0 + (hue[0] - 1.0) * a, 1.0 + (hue[1] - 1.0) * a, 1.0 + (hue[2] - 1.0) * a];
        return Some(Partial { tint, t0: -1.0 });
    }
    None
}

/// What a sheet passes of light arriving at cosine `c` to its normal (CPU twin of the shader's
/// `partial_transmittance`): None where it is not counted (a film's face turned away from the light,
/// where the light leaves the cover).
pub fn transmittance(p: &Partial, c: f32) -> Option<[f32; 3]> {
    let mu = c.abs().max(0.05);
    let s = if p.t0 > 0.0 {
        if c <= 0.0 {
            return None;
        }
        p.t0.powf(1.0 / mu)
    } else {
        let f = GLASS_F0 + (1.0 - GLASS_F0) * (1.0 - mu).powi(5);
        (1.0 - f) * (1.0 - f)
    };
    Some([p.tint[0] * s, p.tint[1] * s, p.tint[2] * s])
}

/// The light a point receives through the sheets in its view (CPU twin of `shadow_transmit_at` in
/// 00-bindings-vertex.wgsl): the four transmittance texels round it (in the order (0,0), (1,0),
/// (0,1), (1,1)) each tested on its own (a texel whose nearest sheet is not nearer the light than
/// the point, by `bias`, passes everything) and blended by the bilinear weights of `frac`.
pub fn receiver_transmit(texels: [[f32; 4]; 4], frac: [f32; 2], metric: f32, bias: f32) -> [f32; 3] {
    let mut acc = [0.0f32; 3];
    for (k, t) in texels.iter().enumerate() {
        let (ox, oy) = (k & 1, k >> 1);
        let wx = if ox == 1 { frac[0] } else { 1.0 - frac[0] };
        let wy = if oy == 1 { frac[1] } else { 1.0 - frac[1] };
        let behind = metric > t[3] + bias;
        for c in 0..3 {
            acc[c] += wx * wy * if behind { t[c] } else { 1.0 };
        }
    }
    acc
}

/// The transmittance atlas's side for a depth atlas of `atlas` texels (0 = none).
pub fn transmit_side(atlas: u32) -> u32 {
    atlas / TRANSMIT_DIVISOR
}

/// Its video memory.
pub fn transmit_bytes(atlas: u32) -> u64 {
    let s = transmit_side(atlas) as u64;
    s * s * TRANSMIT_TEXEL_BYTES
}

/// A depth tile's place in the transmittance atlas: the same tile at the divisor's scale.
pub fn transmit_tile(tile: (u32, u32, u32)) -> (u32, u32, u32) {
    (tile.0 / TRANSMIT_DIVISOR, tile.1 / TRANSMIT_DIVISOR, tile.2 / TRANSMIT_DIVISOR)
}

/// One view's entry in the pass's storage array.
pub fn view_entry(view_proj: &Mat4, origin: Vec3, ortho: bool) -> [f32; 20] {
    let mut e = [0.0f32; 20];
    e[..16].copy_from_slice(&view_proj.to_cols_array());
    e[16..19].copy_from_slice(&origin.to_array());
    e[19] = if ortho { 1.0 } else { 0.0 };
    e
}

/// One sheet instance: its model matrix with the view's index in column 0's w, and what it passes.
pub fn instance_entry(model: &[f32; 16], view: u32, part: Partial) -> [f32; 20] {
    let mut e = [0.0f32; 20];
    e[..16].copy_from_slice(model);
    e[3] = view as f32;
    e[16..20].copy_from_slice(&part.to_vec4());
    e
}

/// The pass's GPU state: the pipeline, the views' storage array, the instances, the atlas and its
/// 1x1 stand-in (what the colour passes bind while there is no atlas, and what the depth passes'
/// own group 0 binds).
pub struct TransmitPass {
    pub pso: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    views: wgpu::Buffer,
    view_cap: u64,
    pub group: wgpu::BindGroup,
    pub instances: wgpu::Buffer,
    instance_cap: u64,
    /// The atlas (texture, view, side), made with the depth atlas at its level.
    pub atlas: Option<(wgpu::Texture, wgpu::TextureView, u32)>,
    pub dummy_view: wgpu::TextureView,
}

impl TransmitPass {
    pub fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("Shadow Transmit Shader"), source: wgpu::ShaderSource::Wgsl(WGSL.into()) });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Transmit Views"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: wgpu::BufferSize::new(VIEW_BYTES) },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("Shadow Transmit Layout"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let vertex = [
            wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x3 },
            wgpu::VertexAttribute { offset: 12, shader_location: 1, format: wgpu::VertexFormat::Float32x3 },
        ];
        let instance = [
            wgpu::VertexAttribute { offset: 0, shader_location: 2, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 16, shader_location: 3, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 32, shader_location: 4, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 48, shader_location: 5, format: wgpu::VertexFormat::Float32x4 },
            wgpu::VertexAttribute { offset: 64, shader_location: 6, format: wgpu::VertexFormat::Float32x4 },
        ];
        // rgb: what gets through, multiplied (src * dst); a: the nearest sheet, kept by Min.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::Dst, dst_factor: wgpu::BlendFactor::Zero, operation: wgpu::BlendOperation::Add },
            alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Min },
        };
        let pso = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Shadow Transmit Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_transmit"),
                buffers: &[
                    // The mesh's own vertices (humanity_core::vertex::Vertex, 32 bytes): position and normal.
                    wgpu::VertexBufferLayout { array_stride: std::mem::size_of::<humanity_core::vertex::Vertex>() as u64, step_mode: wgpu::VertexStepMode::Vertex, attributes: &vertex },
                    wgpu::VertexBufferLayout { array_stride: INSTANCE_BYTES, step_mode: wgpu::VertexStepMode::Instance, attributes: &instance },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_transmit"),
                targets: &[Some(wgpu::ColorTargetState { format: TRANSMIT_FORMAT, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            // Both faces: which face counts is the shader's choice (a film's face toward the light).
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let view_cap = 64;
        let views = Self::buffer(device, "Shadow Transmit Views", view_cap * VIEW_BYTES, wgpu::BufferUsages::STORAGE);
        let group = Self::group(device, &layout, &views);
        let instance_cap = 256;
        let instances = Self::buffer(device, "Shadow Transmit Instances", instance_cap * INSTANCE_BYTES, wgpu::BufferUsages::VERTEX);
        let dummy = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Transmit Stand-in"),
            size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: TRANSMIT_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let dummy_view = dummy.create_view(&wgpu::TextureViewDescriptor::default());
        TransmitPass { pso, layout, views, view_cap, group, instances, instance_cap, atlas: None, dummy_view }
    }

    fn buffer(device: &wgpu::Device, label: &str, size: u64, usage: wgpu::BufferUsages) -> wgpu::Buffer {
        device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: usage | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false })
    }

    fn group(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, views: &wgpu::Buffer) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("Shadow Transmit Views BG"), layout, entries: &[wgpu::BindGroupEntry { binding: 0, resource: views.as_entire_binding() }] })
    }

    /// Make (or drop) the atlas for a depth atlas of `atlas` texels; true when it changed.
    pub fn ensure(&mut self, device: &wgpu::Device, atlas: u32) -> bool {
        let side = transmit_side(atlas);
        if self.atlas.as_ref().map_or(0, |a| a.2) == side {
            return false;
        }
        self.atlas = (side > 0).then(|| {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Shadow Transmittance Atlas"),
                size: wgpu::Extent3d { width: side, height: side, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TRANSMIT_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            log::info!("[Shadows] transmittance atlas {side} x {side} ({:.0} MB)", transmit_bytes(atlas) as f64 / 1_048_576.0);
            (tex, view, side)
        });
        true
    }

    /// What the colour passes bind at group 0 binding 11: the atlas, or the stand-in.
    pub fn colour_view(&self) -> &wgpu::TextureView {
        self.atlas.as_ref().map_or(&self.dummy_view, |a| &a.1)
    }

    /// Write this frame's views and sheet instances, growing a buffer that is too small.
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, views: &[[f32; 20]], instances: &[[f32; 20]]) {
        if views.len() as u64 > self.view_cap {
            self.view_cap = (views.len() as u64).next_power_of_two();
            self.views = Self::buffer(device, "Shadow Transmit Views", self.view_cap * VIEW_BYTES, wgpu::BufferUsages::STORAGE);
            self.group = Self::group(device, &self.layout, &self.views);
        }
        if instances.len() as u64 > self.instance_cap {
            self.instance_cap = (instances.len() as u64).next_power_of_two();
            self.instances = Self::buffer(device, "Shadow Transmit Instances", self.instance_cap * INSTANCE_BYTES, wgpu::BufferUsages::VERTEX);
        }
        if !views.is_empty() {
            queue.write_buffer(&self.views, 0, bytemuck::cast_slice(views));
        }
        if !instances.is_empty() {
            queue.write_buffer(&self.instances, 0, bytemuck::cast_slice(instances));
        }
    }

    /// Video memory: the atlas and the two buffers.
    pub fn vram_bytes(&self) -> u64 {
        let atlas = self.atlas.as_ref().map_or(0, |a| a.2 as u64 * a.2 as u64 * TRANSMIT_TEXEL_BYTES);
        atlas + self.views.size() + self.instances.size()
    }
}

#[cfg(test)]
#[path = "shadow_transmit_tests.rs"]
mod tests;
