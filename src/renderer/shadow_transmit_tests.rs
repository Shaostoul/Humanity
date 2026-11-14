//! Tests for the partial casters (renderer/shadow_transmit.rs).

use super::*;

/// The tent's film material as `engine::fruiting_tent::film_material` makes it from its data
/// (data/garden/grow_media.ron): colour and opacity, then params2 (scattered transmission, light
/// let through, spread, reach). The renderer may not reach the engine or the systems crate (the crate
/// layers lint), so the numbers are written here and the engine's own test,
/// `the_films_partial_shadow_is_its_datas` in engine/fruiting_tent.rs, holds them to the data.
const TENT_FILM_BASE: [f32; 4] = [0.86, 0.88, 0.88, 0.5];
const TENT_FILM_PARAMS2: [f32; 4] = [0.65, 0.8, 0.15, 0.7];

/// The partial shadow of the tent's film, from its material.
fn film_partial() -> Partial {
    partial_of(27.0, TENT_FILM_BASE, TENT_FILM_PARAMS2, 0.0).expect("the film casts a partial shadow")
}

fn close(a: [f32; 3], b: [f32; 3], tol: f32) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tol)
}

/// THE TRANSMITTANCE WRITTEN FOR FILM AND GLASS IS THE MATERIAL'S OWN: the tent's film passes its
/// `light_transmittance` (0.8) face-on, tinted by its colour scaled to its brightest channel; at 60
/// degrees the path through it is twice as long, so 0.8^2; the face where the light LEAVES the
/// cover is not counted; glass passes (1 - F)^2 with Schlick's F at f0 0.04 on both faces (0.9216
/// face-on), tinted toward its colour by its opacity; water, a glowing sheet and an opaque one cast
/// nothing partial.
///
/// Seen red 2026-11-13 with the slant left out of `transmittance` (`p.t0` for a film whatever the
/// angle): "the film at 60 degrees: [0.7818182, 0.8, 0.8] against [0.62545455, 0.64, 0.64]".
#[test]
fn the_transmittance_of_film_and_glass_is_the_materials_own() {
    let p = film_partial();
    let c = TENT_FILM_BASE;
    let m = c[0].max(c[1]).max(c[2]);
    let hue = [c[0] / m, c[1] / m, c[2] / m];
    assert_eq!(p.t0, TENT_FILM_PARAMS2[1], "the film passes its params2.y face-on");
    let face_on = transmittance(&p, 1.0).expect("a face toward the light counts");
    assert!(close(face_on, hue.map(|h| h * 0.8), 1e-5), "the film face-on: {face_on:?}");
    let slant = transmittance(&p, 0.5).expect("counted");
    assert!(close(slant, hue.map(|h| h * 0.64), 1e-5), "the film at 60 degrees: {slant:?} against {:?}", hue.map(|h| h * 0.64));
    assert_eq!(transmittance(&p, -0.7), None, "the face where the light leaves the cover is not counted");

    let glass = partial_of(4.0, [0.6, 0.8, 1.0, 0.25], [0.0; 4], 0.0).expect("glass casts a partial shadow");
    assert_eq!(glass.t0, -1.0);
    assert!(close(glass.tint, [0.9, 0.95, 1.0], 1e-6), "glass is tinted a quarter of the way to its colour: {:?}", glass.tint);
    let g = transmittance(&glass, 1.0).expect("glass counts both faces");
    assert!(close(g, [0.9 * 0.9216, 0.95 * 0.9216, 0.9216], 1e-4), "glass face-on: {g:?}");
    let back = transmittance(&glass, -1.0).expect("both faces");
    assert!(close(back, g, 1e-6), "glass passes the same either way");
    // Fresnel rises toward grazing: a pane at 75 degrees passes clearly less.
    let graze = transmittance(&glass, 0.26).expect("counted")[2];
    assert!(graze < 0.8 && graze > 0.4, "glass at 75 degrees: {graze}");

    // The home's glass roof (type 1 at 0.22, home_structure::GLASS_ROOF_RGBA) is a pane too.
    let roof = partial_of(1.0, [0.55, 0.78, 0.92, 0.22], [0.0; 4], 0.0).expect("the glass roof is glass");
    assert_eq!(roof.t0, -1.0, "glass, not film");
    assert_eq!(partial_of(16.0, [0.1, 0.3, 0.4, 0.6], [0.0; 4], 0.0), None, "the water shell");
    assert_eq!(partial_of(14.0, [0.1, 0.3, 0.4, 0.6], [0.0; 4], 0.0), None, "the atmosphere");
    assert_eq!(partial_of(4.0, [1.0, 1.0, 1.0, 0.3], [0.0; 4], 2.0), None, "a glowing pane");
    assert_eq!(partial_of(4.0, [1.0, 1.0, 1.0, 1.0], [0.0; 4], 0.0), None, "opaque glass is an opaque caster");
    assert_eq!(partial_of(27.0, [1.0, 1.0, 1.0, 0.5], [0.65, 0.0, 0.0, 0.0], 0.0), None, "a film with no figure for its light");
}

/// A LAMP'S LIGHT THROUGH THE FILM IS 20% DIMMER: a lamp 1 m above a tent's top, a shelf 0.4 m
/// under the top. The film's texel (what it passes, how far from the lamp it is) is what the pass
/// writes (`transmittance`, the metric the shader writes); the shelf, farther from the lamp, receives
/// that share of the lamp's unblocked light, tinted; a person between the lamp and the tent, and
/// the film's own surface, receive all of it; at the cover's edge the bilinear blend gives the
/// share of the four texels that are behind the film.
///
/// Seen red 2026-11-13 with `receiver_transmit`'s test written `metric >= t[3] - bias` (the sheet
/// dimming itself): "the film's own surface is not dimmed by itself: [0.7818182, 0.8, 0.8]".
#[test]
fn a_lamps_light_through_the_film_is_dimmer_by_what_the_film_passes() {
    let p = film_partial();
    let lamp = Vec3::new(0.0, 2.6, 0.0);
    let film_at = Vec3::new(0.0, 1.6, 0.0);
    let film_up = Vec3::Y;
    let to_lamp = (lamp - film_at).normalize();
    let t = transmittance(&p, film_up.dot(to_lamp)).expect("the top faces the lamp");
    let texel = [t[0], t[1], t[2], lamp.distance(film_at)];
    let bias = 0.02;
    let unblocked = 3.7f32;
    let shelf = lamp.distance(Vec3::new(0.0, 1.2, 0.0));
    let got = receiver_transmit([texel; 4], [0.3, 0.6], shelf, bias).map(|s| s * unblocked);
    let want = t.map(|s| s * unblocked);
    assert!(close(got, want, 1e-5), "the shelf under the film gets {got:?}, the film passes {want:?}");
    let ratio = got[1] / unblocked;
    assert!((ratio - 0.8).abs() < 1e-5, "about 20% dimmer: {ratio}");
    // In front of the film (nearer the lamp), and the film's own surface: undimmed.
    let person = lamp.distance(Vec3::new(0.0, 1.9, 0.0));
    assert_eq!(receiver_transmit([texel; 4], [0.5, 0.5], person, bias), [1.0; 3], "a person between the lamp and the tent");
    let itself = receiver_transmit([texel; 4], [0.5, 0.5], texel[3], bias);
    assert!(close(itself, [1.0; 3], 1e-6), "the film's own surface is not dimmed by itself: {itself:?}");
    // At the cover's edge: two texels have film, two hold nothing; a quarter of the way across.
    let nothing = [1.0, 1.0, 1.0, NOTHING_M];
    let edge = receiver_transmit([texel, nothing, texel, nothing], [0.25, 0.5], shelf, bias);
    assert!(close(edge, [0.75 * t[0] + 0.25, 0.75 * t[1] + 0.25, 0.75 * t[2] + 0.25], 1e-5), "the edge blends: {edge:?}");
}

/// The shader's formulas are the CPU twins' (the lockstep the other shadow tests keep): the glass
/// f0, the film's slant and leaving face, the receiver's per-texel test and the four texels' order.
#[test]
fn transmit_formula_matches_the_shader() {
    assert!(WGSL.contains(&format!("const GLASS_F0: f32 = {GLASS_F0};")), "GLASS_F0");
    assert!(WGSL.contains("return vec4<f32>(part.rgb * pow(part.w, 1.0 / mu), 1.0);"), "the film's slant");
    assert!(WGSL.contains("if (c <= 0.0) {") && WGSL.contains("return vec4<f32>(1.0, 1.0, 1.0, 0.0);"), "the face where the light leaves");
    assert!(WGSL.contains("let mu = max(abs(c), 0.05);"), "the cosine floor");
    let pbr = crate::renderer::shader_loader::assembled_pbr_source();
    assert!(pbr.contains("fn shadow_transmit_at("), "the receiver's lookup");
    assert!(pbr.contains("let o = vec2<i32>(i32(k & 1u), i32(k >> 1u));"), "the four texels' order");
    assert!(pbr.contains("select(vec3<f32>(1.0), s.rgb, metric > s.a + bias)"), "the receiver's test");
    // The metric the receiver compares is the one the pass writes: the distance from a lamp, or the
    // distance along the sunlight.
    assert!(pbr.contains("var tmetric = -dot(sample_pos, info.xyz);") && pbr.contains("tmetric = length(sample_pos - info.xyz);"), "the receiver's metric");
    assert!(WGSL.contains("var metric = -dot(i.world, v.origin.xyz);") && WGSL.contains("metric = length(d);"), "the written metric");
}

/// The pass's shader parses and validates, and its sizes are what the buffers are made for.
#[test]
fn the_transmit_shader_validates() {
    let module = wgpu::naga::front::wgsl::parse_str(WGSL).expect("the transmit shader parses");
    let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
    v.validate(&module).expect("the transmit shader validates");
    assert_eq!(std::mem::size_of::<[f32; 20]>() as u64, VIEW_BYTES);
    assert_eq!(std::mem::size_of::<[f32; 20]>() as u64, INSTANCE_BYTES);
    assert_eq!(transmit_bytes(4096), 2048 * 2048 * 8, "Full: 32 MB");
    assert_eq!(transmit_bytes(2048), 1024 * 1024 * 8, "Simplified: 8 MB");
    assert_eq!(transmit_tile((512, 1024, 256)), (256, 512, 128));
}

#[cfg(feature = "native")]
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    if !humanity_core::test_gpu::may_use() {
        return None;
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("shadow transmit test"), ..Default::default() }, None)).ok()?;
    device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));
    Some((device, queue))
}

#[cfg(feature = "native")]
fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    match e {
        0 => s * m * 2f32.powi(-24),
        31 => s * f32::INFINITY,
        _ => s * (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// THE GPU WRITES WHAT THE MATERIALS PASS: a lamp 1 m above three sheets side by side (the tent's
/// film facing it, the same film turned away, and a glass pane turned away), drawn through the real
/// pipeline into a small transmittance target. Where the film faces the lamp the texel holds its
/// `transmittance` at that angle and its distance from the lamp; the film turned away (where light
/// would leave a cover) leaves the clear value; glass is counted from either side. And two films
/// one over the other pass the product (the multiply blend) and keep the nearer distance (Min).
/// Runs on the GPU (skipped, and said, when the operator's GPU program is running).
///
/// Seen red 2026-11-13 with the blend's colour factors swapped to (One, Zero) (the last sheet wins):
/// "two films: [0.7583008, 0.7583008] against their product 0.5572176". (Its first red run, with a
/// 100-degree view, read a texel outside the view: the sheets were moved inside a 120-degree one.)
#[test]
#[cfg(feature = "native")]
fn the_gpu_writes_what_the_sheets_pass() {
    let Some((device, queue)) = device() else { return };
    let pass = TransmitPass::new(&device);
    let size = 64u32;
    let lamp = Vec3::new(0.0, 2.0, 0.0);
    let view = Mat4::perspective_rh(120f32.to_radians(), 1.0, 0.05, 6.0) * Mat4::look_at_rh(lamp, Vec3::ZERO, Vec3::Z);
    let quad = |x0: f32, x1: f32, z0: f32, z1: f32, y: f32, up: bool| -> Vec<humanity_core::vertex::Vertex> {
        let n = if up { [0.0, 1.0, 0.0] } else { [0.0, -1.0, 0.0] };
        [(x0, z0), (x1, z0), (x1, z1), (x0, z0), (x1, z1), (x0, z1)].iter().map(|&(x, z)| humanity_core::vertex::Vertex { position: [x, y, z], normal: n, uv: [0.0, 0.0] }).collect()
    };
    // Left third: the film facing the lamp at y 1.0, and a second film over it at y 1.2 (right half
    // of that third only); middle: the film turned away; right: glass turned away.
    let mut v = quad(-1.5, -0.5, -0.5, 0.5, 1.0, true);
    v.extend(quad(-1.0, -0.5, -0.5, 0.5, 1.2, true));
    let film_n = v.len() as u32;
    v.extend(quad(-0.25, 0.25, -0.5, 0.5, 1.0, false));
    let away_n = v.len() as u32;
    v.extend(quad(0.5, 1.5, -0.5, 0.5, 1.0, false));
    let idx: Vec<u32> = (0..v.len() as u32).collect();
    let mesh = crate::renderer::mesh::Mesh::from_vertices(&device, &v, &idx);
    let film = film_partial();
    let glass = partial_of(4.0, [1.0, 1.0, 1.0, 0.2], [0.0; 4], 0.0).expect("glass");
    let ident = Mat4::IDENTITY.to_cols_array();
    let mut pass = pass;
    pass.upload(&device, &queue, &[view_entry(&view, lamp, false)], &[instance_entry(&ident, 0, film), instance_entry(&ident, 0, glass)]);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("transmit test target"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: TRANSMIT_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let tv = target.create_view(&Default::default());
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("transmit test pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment { view: &tv, resolve_target: None, ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: 1.0, g: 1.0, b: 1.0, a: NOTHING_M as f64 }), store: wgpu::StoreOp::Store } })],
            ..Default::default()
        });
        rp.set_pipeline(&pass.pso);
        rp.set_bind_group(0, &pass.group, &[]);
        rp.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
        rp.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        // The films (instance 0) and the glass (instance 1), each its own range of the mesh.
        rp.set_vertex_buffer(1, pass.instances.slice(..));
        rp.draw_indexed(0..away_n, 0, 0..1);
        rp.draw_indexed(away_n..idx.len() as u32, 0, 1..2);
    }
    let row = (size as u64 * 8).next_multiple_of(256);
    let staging = device.create_buffer(&wgpu::BufferDescriptor { label: Some("read back"), size: row * size as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &staging, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: Some(size) } },
        wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = staging.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let bytes = slice.get_mapped_range().to_vec();
    let texel = |p: Vec3| -> [f32; 4] {
        let c = view.project_point3(p);
        let (x, y) = (((c.x * 0.5 + 0.5) * size as f32) as usize, ((0.5 - c.y * 0.5) * size as f32) as usize);
        let o = y * row as usize + x * 8;
        let h = |k: usize| f16_to_f32(u16::from_le_bytes([bytes[o + 2 * k], bytes[o + 2 * k + 1]]));
        [h(0), h(1), h(2), h(3)]
    };
    let _ = film_n;
    // One film, facing the lamp, at (-1.2, 1, 0).
    let at = Vec3::new(-1.3, 1.0, 0.0);
    let want = transmittance(&film, Vec3::Y.dot((lamp - at).normalize())).expect("faces the lamp");
    let got = texel(at);
    assert!(close([got[0], got[1], got[2]], want, 0.01), "one film: {got:?} against {want:?}");
    assert!((got[3] - lamp.distance(at)).abs() < 0.03, "its distance: {} against {}", got[3], lamp.distance(at));
    // Two films: the product, and the nearer one's distance.
    let at2 = Vec3::new(-0.75, 1.0, 0.0);
    // The upper film where the lamp's ray to at2 crosses it (it lies 0.8 of the way down).
    let top = lamp + (at2 - lamp) * 0.8;
    let t_low = transmittance(&film, Vec3::Y.dot((lamp - at2).normalize())).expect("faces");
    let t_top = transmittance(&film, Vec3::Y.dot((lamp - top).normalize())).expect("faces");
    let got2 = texel(at2);
    assert!((got2[1] - t_low[1] * t_top[1]).abs() < 0.015, "two films: [{}, {}] against their product {}", got2[1], got2[2], t_low[1] * t_top[1]);
    assert!((got2[3] - lamp.distance(top)).abs() < 0.05, "two films keep the nearer: {} against {}", got2[3], lamp.distance(top));
    // The film turned away: nothing written.
    let got3 = texel(Vec3::new(0.0, 1.0, 0.0));
    assert!(close([got3[0], got3[1], got3[2]], [1.0; 3], 1e-3) && got3[3] > 1000.0, "the leaving face writes nothing: {got3:?}");
    // Glass turned away: counted.
    let at4 = Vec3::new(1.0, 1.0, 0.0);
    let want4 = transmittance(&glass, -Vec3::Y.dot((lamp - at4).normalize())).expect("glass");
    let got4 = texel(at4);
    assert!(close([got4[0], got4[1], got4[2]], want4, 0.01), "glass from its back: {got4:?} against {want4:?}");
}
