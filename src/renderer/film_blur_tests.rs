//! The film blur's tests (renderer/film_blur.rs).

use super::*;

/// THE BLUR GROWS WITH THE GAP: a film 1.5 m from the eye (the racks at arm's length), the
/// tent's own spread from its data, a 60-degree lens on a 1387-line render. The blur's radius is
/// in proportion to the gap (twice the gap, twice the radius), so a mushroom touching the film
/// (5 mm) stays under one base texel (sharp, level 0), the shelf's front lip (5 cm) is a few
/// texels, and the back of the tent (40 cm) is several times wider, a higher level; a surface in
/// front of the film (a negative gap) is not blurred; the level never passes the top one.
///
/// Seen red 2026-11-13 with `blur_radius_texels` written without the gap (spread * focal / z, a
/// fixed blur): the first assertion failed, "twice the gap, twice the blur" (every gap blurred by
/// the same 144 texels, at the first spread tried, 0.36).
#[test]
fn the_blur_grows_with_the_gap() {
    let spread = 0.15;
    let proj = Mat4::perspective_rh(60f32.to_radians(), 2560.0 / 1387.0, 500.0, 0.05);
    let focal = focal_texels(&proj, 1387);
    assert!((focal - 1387.0 / 4.0 / (30f32.to_radians().tan())).abs() < 0.5, "focal {focal}");
    let z = 1.5;
    let r = |gap: f32| blur_radius_texels(gap, z, spread, focal);
    assert!((r(0.4) / r(0.2) - 2.0).abs() < 1e-5, "twice the gap, twice the blur: {} texels at 20 cm, {} at 40 cm", r(0.2), r(0.4));
    assert!(r(0.005) < 1.0, "a mushroom touching the film is sharp: {}", r(0.005));
    assert!(r(0.05) > 2.0 && r(0.05) < 10.0, "a 5 cm gap blurs by {} texels and a 40 cm gap by {}", r(0.05), r(0.4));
    assert!(r(0.4) > 5.0 * r(0.05) && r(0.4) > 20.0, "the back of the tent blurs much more: {}", r(0.4));
    assert_eq!(r(-0.3), 0.0, "in front of the film: no blur");
    assert_eq!(blur_lod(r(0.005), 5.0), 0.0, "level 0");
    // Eight times the gap, three levels up (while under the top).
    let climb = blur_lod(r(0.4), 8.0) - blur_lod(r(0.05), 8.0);
    assert!((climb - 3.0).abs() < 1e-4, "the level climbs with the gap: {climb}");
    assert_eq!(blur_lod(1.0e6, 5.0), 5.0, "never past the top level");
    // Seen from twice as far, the same gap covers half the screen texels.
    assert!((blur_radius_texels(0.4, 3.0, spread, focal) - r(0.4) / 2.0).abs() < 1e-4);
    // The gap is held to the sheet's reach (the cover's depth): the room 4 m behind blurs as the
    // cover's far side 0.7 m behind does; no reach, no hold.
    assert_eq!(film_gap(4.0, 0.7), 0.7);
    assert_eq!(film_gap(0.3, 0.7), 0.3);
    assert_eq!(film_gap(4.0, 0.0), 4.0);
    // The haze from the tent's numbers: 0.65 of 0.8 scattered.
    assert!((haze([0.65, 0.8, 0.36, 0.0]) - 0.8125).abs() < 1e-6);
}

/// The shader's formulas are the CPU twins' and its modes are the renderer's.
#[test]
fn film_blur_formula_matches_the_shader() {
    let pbr = crate::renderer::shader_loader::assembled_pbr_source();
    assert!(pbr.contains("return max(gap, 0.0) * spread * focal / max(z, 0.01);"), "film_blur_radius");
    assert!(pbr.contains("return clamp(log2(max(r, 1.0)), 0.0, top);"), "film_blur_lod");
    assert!(pbr.contains("return select(gap, min(gap, reach), reach > 0.0);"), "film_gap");
    assert!(pbr.contains(&format!("const FILM_MODE_FAR: f32 = {MODE_FAR:.1};")), "MODE_FAR");
    assert!(pbr.contains(&format!("const FILM_MODE_NEAR: f32 = {MODE_NEAR:.1};")), "MODE_NEAR");
    assert!(pbr.contains("@group(0) @binding(12) var film_behind: texture_2d_array<f32>;"), "the pyramid's binding");
    // The blur is colour over coverage (`covered_mean`), and drawn whole where nothing is covered.
    assert!(pbr.contains("if (b.a <= 0.0001) {"), "film_covered's floor is covered_mean's");
    assert!(pbr.contains("return vec4<f32>(b.rgb / b.a, 1.0);"), "film_covered divides by the coverage");
    let module = wgpu::naga::front::wgsl::parse_str(WGSL).expect("the pyramid shader parses");
    let mut v = wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
    v.validate(&module).expect("the pyramid shader validates");
}

/// The blur's size and memory: half the render at the pyramid's base, halving, at most six levels,
/// two layers, and the coverage mark at the render's size.
#[test]
fn the_pyramid_is_sized_from_the_render() {
    assert_eq!(base_size(2560, 1440), (1280, 720));
    assert_eq!(levels_for(2560, 1440), 6);
    assert_eq!(levels_for(8, 8), 3);
    let b = pyramid_bytes(2560, 1440);
    let layer = (1280 * 720 + 640 * 360 + 320 * 180 + 160 * 90 + 80 * 45 + 40 * 22) as u64 * 8;
    assert_eq!(b, 2 * layer + 2560 * 1440);
    assert!(b > 23_000_000 && b < 24_000_000, "about 23 MB at 2560 x 1440: {b}");
}

/// THE FILMS' RECTANGLE IS CONSERVATIVE: a sphere's whole disc on screen falls inside the
/// rectangle (sampled round its silhouette), a sphere behind the eye adds nothing, one through the
/// eye's plane takes the whole render, and each level's rectangle holds the one below it halved.
///
/// Seen red 2026-11-14 with the y axis left unflipped (`(ny * 0.5 + 0.5) * h`): "the silhouette
/// point (1607.6, 530.6) is outside the rectangle (1181, 643, 486, 485)".
#[test]
fn the_films_rectangle_holds_every_film_pixel() {
    let (w, h) = (2560u32, 1387u32);
    let proj = Mat4::perspective_infinite_reverse_rh(60f32.to_radians(), w as f32 / h as f32, 0.05);
    let c = Vec3::new(0.3, 0.4, -3.0);
    let r = 0.5;
    let (x, y, rw, rh) = screen_rect(&[(c, r)], &proj, w, h).expect("in front");
    // Points of the sphere's silhouette as seen from the eye: round the circle perpendicular to the
    // view ray, at the tangent radius.
    let d = c.length();
    let ray = c / d;
    let side = ray.cross(Vec3::Y).normalize();
    let up = side.cross(ray);
    let tangent = r * (d * d - r * r).sqrt() / d;
    let mid = c - ray * (r * r / d);
    for k in 0..64 {
        let a = k as f32 / 64.0 * std::f32::consts::TAU;
        let p = mid + (side * a.cos() + up * a.sin()) * tangent;
        let clip = proj * p.extend(1.0);
        let (px, py) = ((clip.x / clip.w * 0.5 + 0.5) * w as f32, (0.5 - clip.y / clip.w * 0.5) * h as f32);
        assert!(px >= x as f32 && px <= (x + rw) as f32 && py >= y as f32 && py <= (y + rh) as f32, "the silhouette point ({px:.1}, {py:.1}) is outside the rectangle ({x}, {y}, {rw}, {rh})");
    }
    assert!(rw < w / 3 && rh < h / 2, "a half-metre sphere at 3 m is a small part of the screen: {rw} x {rh}");
    assert_eq!(screen_rect(&[(Vec3::new(0.0, 0.0, 3.0), 0.5)], &proj, w, h), None, "behind the eye: nothing");
    assert_eq!(screen_rect(&[(Vec3::new(0.0, 0.0, -0.2), 0.5)], &proj, w, h), Some((0, 0, w, h)), "through the eye's plane: the whole render");
    assert_eq!(screen_rect(&[(c, f32::INFINITY)], &proj, w, h), Some((0, 0, w, h)), "no bound: the whole render");
    // Each level holds the level below it halved (rounded outward), and stays on its level.
    let rect = (x, y, rw, rh);
    let mut below = rect;
    let mut size = (w, h);
    for lvl in 0..levels_for(w, h) {
        let (lx, ly, lw, lh) = level_rect(rect, lvl, (w, h));
        size = if lvl == 0 { base_size(w, h) } else { ((size.0 / 2).max(1), (size.1 / 2).max(1)) };
        assert!(lx + lw <= size.0 && ly + lh <= size.1, "level {lvl} stays on its {size:?}");
        assert!(lx <= below.0 / 2 && ly <= below.1 / 2 && lx + lw >= ((below.0 + below.2).div_ceil(2)).min(size.0) && ly + lh >= ((below.1 + below.3).div_ceil(2)).min(size.1), "level {lvl} {:?} holds {below:?} halved", (lx, ly, lw, lh));
        below = (lx, ly, lw, lh);
    }
}

/// THE BLUR READS NOTHING OUTSIDE THE COVERAGE (CPU twin): a tent's covered half (dark, 0.1) beside
/// a lamp-lit wall (10.0, covered by no film). The base texel straddling the outline keeps the dark
/// value exactly once divided by its coverage; weighed by one (the merged look, `all_count`) the
/// same texel is brightened by the wall.
#[test]
fn the_covered_mean_ignores_what_is_beside_the_tent() {
    let size = (16u32, 8u32);
    let scene = |x: u32, _y: u32| if x < 8 { [0.1, 0.1, 0.1] } else { [10.0, 10.0, 10.0] };
    let mask = |x: u32, _y: u32| if x < 8 { 1.0 } else { 0.0 };
    // Base texel 4 reads render texels 7 to 10: two covered, two beside.
    let edge = base_texel(scene, mask, size, 4, 2, false);
    assert!(edge[3] > 0.1 && edge[3] < 0.9, "the edge texel is partly covered: {}", edge[3]);
    let m = covered_mean(edge).expect("covered");
    assert!((m[0] - 0.1).abs() < 1e-6, "the edge keeps the tent's own 0.1: {m:?}");
    let merged = base_texel(scene, mask, size, 4, 2, true);
    assert!(covered_mean(merged).expect("weighed by one")[0] > 1.0, "the merged look borrows the wall: {merged:?}");
    assert_eq!(covered_mean(base_texel(scene, mask, size, 7, 2, false)), None, "wholly beside: nothing to blur");
}

// ── On the GPU ───────────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "native")]
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    if !humanity_core::test_gpu::may_use() {
        return None;
    }
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions { power_preference: wgpu::PowerPreference::HighPerformance, compatible_surface: None, force_fallback_adapter: false }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("film blur test"), ..Default::default() }, None)).ok()?;
    device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));
    Some((device, queue))
}

#[cfg(feature = "native")]
fn f16_to_f32(h: u16) -> f32 {
    let s = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((h >> 10) & 0x1f) as i32;
    let m = (h & 0x3ff) as f32;
    s * match e {
        0 => m * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}

/// One level of the pyramid's layer 0 read back: (w, h, texels as rgba).
#[cfg(feature = "native")]
fn read_level(device: &wgpu::Device, queue: &wgpu::Queue, pyr: &Pyramid, lvl: u32) -> (u32, u32, Vec<[f32; 4]>) {
    let (bw, bh) = base_size(pyr.size.0, pyr.size.1);
    let (w, h) = ((bw >> lvl).max(1), (bh >> lvl).max(1));
    let row = (w as u64 * 8).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor { label: Some("read back"), size: row * h as u64, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &pyr._tex, mip_level: lvl, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row as u32), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit(Some(enc.finish()));
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let bytes = slice.get_mapped_range().to_vec();
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        for x in 0..w as usize {
            let at = y * row as usize + x * 8;
            let c = |k: usize| f16_to_f32(u16::from_le_bytes([bytes[at + 2 * k], bytes[at + 2 * k + 1]]));
            out.push([c(0), c(1), c(2), c(3)]);
        }
    }
    (w, h, out)
}

/// THE BLUR READS NOTHING OUTSIDE THE COVERAGE MASK, on the GPU: a 64 x 32 render whose left half
/// is a tent's covered scene (dark, 0.1) and right half a lamp-lit wall beside it (10.0), the mask
/// marking only the left. Every level of the pyramid, divided by its coverage, keeps the tent's 0.1
/// in every texel it covers at all, the outline included, so a lit object beside the tent never
/// brightens its edge; the merged look (`all_count`, showcase `film_v1`) brightens the same edge
/// texels, which is what the test would see if the coverage were lost. The base level equals its CPU
/// twin (`base_texel`) texel for texel. Runs on the GPU (skipped, and said, when the operator's GPU
/// program is running).
///
/// Seen red 2026-11-14 with `fs_base` weighing every texel by one regardless of the mask (the
/// merged base): "base texel (15, 0) channel 0: GPU 1.3369141 against CPU 0.087500006" (the texel
/// on the outline took the lit wall beside the tent in).
#[test]
#[cfg(feature = "native")]
fn the_blur_reads_nothing_outside_the_coverage_mask() {
    let Some((device, queue)) = device() else { return };
    let (w, h) = (64u32, 32u32);
    let blur = FilmBlur::new(&device);
    let pyr = Pyramid::new(&device, (w, h));
    let scene_tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test scene"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let value = |x: u32| if x < w / 2 { 0.1f32 } else { 10.0 };
    let texels: Vec<f32> = (0..h).flat_map(|_| (0..w).flat_map(move |x| [value(x), value(x), value(x), 1.0])).collect();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &scene_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        bytemuck::cast_slice(&texels),
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 16), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    let mask: Vec<u8> = (0..h).flat_map(|_| (0..w).map(|x| if x < w / 2 { 255u8 } else { 0 })).collect();
    let mask_row = 256u32;
    let padded: Vec<u8> = mask.chunks(w as usize).flat_map(|r| r.iter().copied().chain(std::iter::repeat(0).take((mask_row - w) as usize))).collect();
    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &pyr._mask_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &padded,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(mask_row), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("test depth"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let scene_view = scene_tex.create_view(&Default::default());
    let depth_view = depth.create_view(&Default::default());
    let proj = Mat4::perspective_infinite_reverse_rh(1.0, 2.0, 0.05);
    let run = |all_count: bool| {
        let mut enc = device.create_command_encoder(&Default::default());
        blur.encode_pyramid(&device, &queue, &mut enc, &pyr, &scene_view, &depth_view, &proj, (0, 0, w, h), all_count, &|| None);
        queue.submit(Some(enc.finish()));
        (0..levels_for(w, h)).map(|lvl| read_level(&device, &queue, &pyr, lvl)).collect::<Vec<_>>()
    };
    // The base level is its CPU twin's.
    let covered = run(false);
    let (bw, _, base) = &covered[0];
    for (i, t) in base.iter().enumerate() {
        let (bx, by) = (i as u32 % bw, i as u32 / bw);
        let cpu = base_texel(|x, _| [value(x); 3], |x, _| if x < w / 2 { 1.0 } else { 0.0 }, (w, h), bx, by, false);
        for k in 0..4 {
            assert!((t[k] - cpu[k]).abs() < 2e-3 * cpu[k].abs().max(1.0), "base texel ({bx}, {by}) channel {k}: GPU {} against CPU {}", t[k], cpu[k]);
        }
    }
    // Every level: wherever a texel is covered at all, colour over coverage is the tent's 0.1.
    let mut edges = 0;
    for (lvl, (lw, _, texels)) in covered.iter().enumerate() {
        for (i, t) in texels.iter().enumerate() {
            let Some(m) = covered_mean(*t) else { continue };
            if t[3] < 0.999 {
                edges += 1;
            }
            assert!((m[0] - 0.1).abs() < 2e-3, "level {lvl} texel ({}, {}): coverage {:.2}, colour over coverage {:.3} against the tent's 0.1", i as u32 % lw, i as u32 / lw, t[3], m[0]);
        }
    }
    assert!(edges > 0, "some texels straddle the outline (the test looks where the borrowing was)");
    // The merged look borrows the wall at the outline: the test would see it.
    let merged = run(true);
    let (lw, _, texels) = &merged[2];
    let x_edge = lw / 2 - 1;
    let brightened = covered_mean(texels[x_edge as usize]).expect("weighed by one")[0];
    assert!(brightened > 0.5, "the merged look brightens the outline at level 2: {brightened}");
}
