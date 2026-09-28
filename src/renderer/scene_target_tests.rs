//! Tests for the scene target and the present pass (scene_target.rs). A
//! file of their own so the pass stays readable; `#[path]`-included as
//! `scene_target::tests`, so `super::*` is the pass.

use super::*;

/// The present shader is compiled at renderer init, so a WGSL error would
/// make the app unbootable while every static check stays green (the v0.782
/// class). Parse and validate it here, as ssao.rs does its own.
#[test]
fn present_shader_parses_and_validates() {
    let src = include_str!("../../assets/shaders/present.wgsl");
    let module =
        wgpu::naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("present.wgsl failed to parse: {e}"));
    let mut validator =
        wgpu::naga::valid::Validator::new(wgpu::naga::valid::ValidationFlags::all(), wgpu::naga::valid::Capabilities::all());
    validator.validate(&module).unwrap_or_else(|e| panic!("present.wgsl failed naga validation: {e:?}"));
    let entries: Vec<&str> = module.entry_points.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(entries, vec!["vs_main", "fs_main"]);
}

/// Increment 3: the scene is drawn in Rgba16Float whatever the display is.
#[test]
fn the_scene_format_is_rgba16float_from_increment_3() {
    for f in DISPLAY_FORMATS {
        assert_eq!(scene_format_for(f), wgpu::TextureFormat::Rgba16Float);
    }
}

/// The uniform lanes: the clamp exactly when the formats differ (the
/// display-format A/B arm must stay the bit-exact passthrough), the dither
/// scaled by the output's code count, the sRGB lane from the output format.
#[test]
fn present_flags_follow_the_formats_and_the_switch() {
    use wgpu::TextureFormat as F;
    let hdr = F::Rgba16Float;
    assert_eq!(present_flags(F::Bgra8UnormSrgb, hdr, true).flags, [1.0, 255.0, 1.0, 0.0]);
    assert_eq!(present_flags(F::Bgra8UnormSrgb, hdr, false).flags, [1.0, 0.0, 1.0, 0.0]);
    assert_eq!(present_flags(F::Rgba8Unorm, hdr, true).flags, [1.0, 255.0, 0.0, 0.0]);
    assert_eq!(present_flags(F::Bgra8UnormSrgb, F::Bgra8UnormSrgb, false).flags, [0.0, 0.0, 1.0, 0.0]);
    assert_eq!(present_flags(F::Rgb10a2Unorm, hdr, true).flags[1], 1023.0);
    // A float output does not quantise, so there is nothing to dither.
    assert_eq!(present_flags(F::Rgba16Float, hdr, true).flags, [0.0, 0.0, 0.0, 0.0]);
}

/// The 8-bit formats a surface can pick.
const DISPLAY_FORMATS: [wgpu::TextureFormat; 4] = [
    wgpu::TextureFormat::Bgra8UnormSrgb,
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureFormat::Bgra8Unorm,
    wgpu::TextureFormat::Rgba8Unorm,
];

#[cfg(feature = "native")]
fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
    let instance = wgpu::Instance::default();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    Some(
        pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor { label: Some("present pass test"), ..Default::default() },
            None,
        ))
        .expect("device"),
    )
}

/// sRGB encode and decode, the IEC 61966-2-1 pair, on the CPU.
#[cfg(feature = "native")]
fn srgb_encode(v: f32) -> f32 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(feature = "native")]
fn srgb_decode(e: f32) -> f32 {
    if e <= 0.04045 {
        e / 12.92
    } else {
        ((e + 0.055) / 1.055).powf(2.4)
    }
}

/// Half bits back to f32 (normal, subnormal and zero; the tests never
/// upload an infinity or a NaN through this path).
#[cfg(feature = "native")]
fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = ((h >> 10) & 0x1f) as i32;
    let man = (h & 0x3ff) as f32;
    sign * if exp == 0 { man * 2f32.powi(-24) } else { (1.0 + man / 1024.0) * 2f32.powi(exp - 15) }
}

/// Upload `values` (rgba per pixel, row-major) into a Rgba16Float target.
#[cfg(feature = "native")]
fn upload_f16(queue: &wgpu::Queue, scene: &SceneTarget, values: &[[f32; 4]], w: u32, h: u32) {
    let mut bytes = Vec::with_capacity(values.len() * 8);
    for px in values {
        for c in px {
            bytes.extend_from_slice(&super::super::atmo_luts::f32_to_f16_bits(*c).to_le_bytes());
        }
    }
    upload(queue, scene, &bytes, w, h, 8);
}

#[cfg(feature = "native")]
fn upload(queue: &wgpu::Queue, scene: &SceneTarget, bytes: &[u8], w: u32, h: u32, bpp: u32) {
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &scene.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        bytes,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * bpp), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

/// Present `scene` into a fresh display texture and read it back.
#[cfg(feature = "native")]
fn present_and_read(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    present: &PresentPass,
    scene: &SceneTarget,
    format: wgpu::TextureFormat,
) -> Vec<u8> {
    let (w, h) = scene.size();
    let out = display_texture(device, w, h, format);
    let mut enc = device.create_command_encoder(&Default::default());
    present.encode(&mut enc, scene, &out.create_view(&Default::default()), None);
    queue.submit(std::iter::once(enc.finish()));
    read_back(device, queue, &out, w, h)
}

/// Channel offsets of r, g, b in a readback of `format`.
#[cfg(feature = "native")]
fn rgb_offsets(format: wgpu::TextureFormat) -> [usize; 3] {
    match format {
        wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm => [2, 1, 0],
        _ => [0, 1, 2],
    }
}

/// THE A/B ARM'S BIT-EXACT PROOF (increments 1 and 2, kept for the
/// `scene_format: display` switch). With the scene target in the display
/// format and the dither off, the present is a passthrough: every code of
/// every channel, alpha included, round-trips unchanged, and an alpha-blended
/// gradient drawn through the target and presented equals the same draw
/// straight into the display. Skips with a note without an adapter.
#[cfg(feature = "native")]
#[test]
fn present_pass_is_byte_exact_in_the_display_format_arm() {
    let Some((device, queue)) = device() else {
        println!("no GPU adapter; skipping the present-pass round trip");
        return;
    };
    // Not square and not a power of two in height, so a transposed or offset
    // read cannot pass by symmetry. 256 wide = 1024 bytes a row, already a
    // multiple of the 256-byte copy alignment.
    let (w, h) = (256u32, 83u32);
    for format in DISPLAY_FORMATS {
        let mut present = PresentPass::new(&device, format, format);
        present.set_dither(&queue, false);
        let scene = SceneTarget::new(&device, &present, w, h, format);
        // 1. Round trip of every code. Each channel is a bijection of x for
        // a fixed y (odd multipliers and xor), so every row holds all 256
        // codes in every channel.
        let mut bytes = vec![0u8; (w * h * 4) as usize];
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                bytes[i] = x as u8;
                bytes[i + 1] = (x * 5 + y) as u8;
                bytes[i + 2] = (x as u8) ^ ((y * 7) as u8);
                bytes[i + 3] = (x * 3 + y * 11) as u8;
            }
        }
        upload(&queue, &scene, &bytes, w, h, 4);
        let back = present_and_read(&device, &queue, &present, &scene, format);
        let first_bad = bytes.iter().zip(&back).position(|(a, b)| a != b);
        assert!(first_bad.is_none(), "{format:?}: the passthrough changed byte {first_bad:?}");

        // 2. A blended draw through the target equals the same draw direct.
        let direct = display_texture(&device, w, h, format);
        draw_gradient(&device, &queue, format, scene.view());
        draw_gradient(&device, &queue, format, &direct.create_view(&Default::default()));
        let a = present_and_read(&device, &queue, &present, &scene, format);
        let b = read_back(&device, &queue, &direct, w, h);
        let diffs = a.iter().zip(&b).filter(|(x, y)| x != y).count();
        assert_eq!(diffs, 0, "{format:?}: drawing through the target differs from direct in {diffs} bytes");
        let distinct: std::collections::BTreeSet<u8> = a.iter().step_by(4).copied().collect();
        assert!(distinct.len() > 100, "{format:?}: the gradient drew only {} distinct codes", distinct.len());
    }
}

/// INCREMENT 3 on a real device. The scene target is Rgba16Float and the
/// present clamps.
/// 1. Every code survives the float target: the exact linear value of each
///    code (decoded for an sRGB output) presents back as that code, alpha
///    included. Half precision is at worst 0.06 of a code at white.
/// 2. A blended draw through the float target is within one code of the
///    same draw straight into the 8-bit display (the direct draw rounds the
///    cleared colour to 8 bits before blending; the float target does not).
/// 3. Out-of-range values present clamped: above 1 and +inf as 255, below 0
///    and NaN as 0, with the dither on or off, and black and white stay
///    exactly black and white under the dither.
#[cfg(feature = "native")]
#[test]
fn the_float_scene_target_presents_every_code_and_clamps() {
    let Some((device, queue)) = device() else {
        println!("no GPU adapter; skipping the float scene target test");
        return;
    };
    let (w, h) = (256u32, 4u32);
    for format in DISPLAY_FORMATS {
        let srgb = format.is_srgb();
        let mut present = PresentPass::new(&device, format, scene_format_for(format));
        present.set_dither(&queue, false);
        let scene = SceneTarget::new(&device, &present, w, h, scene_format_for(format));
        let rgb = rgb_offsets(format);

        // 1. Every code, in every channel (row y shifts the channels apart).
        let code_of = |x: u32, y: u32, ch: u32| ((x + y * 37 + ch * 91) % 256) as u8;
        let lin = |code: u8| if srgb { srgb_decode(code as f32 / 255.0) } else { code as f32 / 255.0 };
        let mut values = Vec::new();
        for y in 0..h {
            for x in 0..w {
                values.push([
                    lin(code_of(x, y, 0)),
                    lin(code_of(x, y, 1)),
                    lin(code_of(x, y, 2)),
                    code_of(x, y, 3) as f32 / 255.0,
                ]);
            }
        }
        upload_f16(&queue, &scene, &values, w, h);
        let back = present_and_read(&device, &queue, &present, &scene, format);
        for y in 0..h {
            for x in 0..w {
                let i = ((y * w + x) * 4) as usize;
                for ch in 0..3u32 {
                    assert_eq!(back[i + rgb[ch as usize]], code_of(x, y, ch), "{format:?}: code at ({x},{y}) ch {ch}");
                }
                assert_eq!(back[i + 3], code_of(x, y, 3), "{format:?}: alpha at ({x},{y})");
            }
        }

        // 2. Within one code of the direct 8-bit draw.
        let direct = display_texture(&device, w, h, format);
        draw_gradient(&device, &queue, format, &direct.create_view(&Default::default()));
        let blend_pipe_scene = scene_format_for(format);
        draw_gradient(&device, &queue, blend_pipe_scene, scene.view());
        let a = present_and_read(&device, &queue, &present, &scene, format);
        let b = read_back(&device, &queue, &direct, w, h);
        let worst = a.iter().zip(&b).map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0);
        assert!(worst <= 1, "{format:?}: the float target differs from the direct draw by {worst} codes");

        // 3. Clamping, with the dither off and then on.
        let specials = [1.5f32, f32::INFINITY, -0.3, f32::NAN, 0.0, 1.0];
        let want = [255u8, 255, 0, 0, 0, 255];
        let mut vals = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let v = specials[(x as usize) % specials.len()];
                vals.push([v, v, v, 1.0]);
            }
        }
        // f32_to_f16_bits maps NaN to a half NaN and inf to half inf.
        upload_f16(&queue, &scene, &vals, w, h);
        for dither in [false, true] {
            present.set_dither(&queue, dither);
            let back = present_and_read(&device, &queue, &present, &scene, format);
            for x in 0..w {
                let i = (x * 4) as usize;
                let expect = want[(x as usize) % want.len()];
                for ch in rgb {
                    assert_eq!(back[i + ch], expect, "{format:?} dither {dither}: special {} at x {x}", specials[(x as usize) % 6]);
                }
            }
        }
    }
}

/// INCREMENT 4: THE ONE DITHER, on a real device. A LINEAR 0 to 0.02 ramp
/// (codes 0 to 38.7 on an sRGB output; the same codes on a plain UNORM one,
/// whose ramp is the encoded values) is 8192 px wide and 64 rows tall, so
/// undithered it is a staircase of flat bands: code 0 is 62 px wide, every
/// later code 124 px (in the linear toe) to 374 px (at 0.02).
///
/// N = 64, THE LONGEST RUN OF ONE NONZERO CODE ALLOWED ALONG A ROW. Every
/// real band (124 px and up) must be broken, so N sits under the narrowest.
/// With the dither, a pixel on a whole code keeps it in the next pixel with
/// probability at most 0.75 (triangular noise of one code rounds back to the
/// same code three times in four), so a run of 64 has probability about
/// 0.75^63 = 1.3e-8 per run, and the image holds about 213,000 runs: 0.003
/// expected. Code 0 is left out: within half a code of black the amplitude
/// narrows so black stays exactly black (present.wgsl), which is the point,
/// not a band. The undithered ramp is the positive control: it must FAIL the
/// same check, or the check proves nothing.
///
/// THE MEAN: in blocks of 64 columns (4,096 px each, a noise floor of about
/// 0.008 code) the dithered mean code is within 0.1 of the exact code of the
/// ramp, and the whole-ramp mean within 0.1 of the undithered one. Near
/// black the narrowed noise costs at most 0.08 (present.wgsl, THE ENDS).
#[cfg(feature = "native")]
#[test]
fn the_dither_breaks_every_band_of_a_dark_ramp_and_keeps_the_mean() {
    let Some((device, queue)) = device() else {
        println!("no GPU adapter; skipping the dither ramp test");
        return;
    };
    const N: usize = 64;
    let (w, h) = (8192u32, 64u32);
    for format in DISPLAY_FORMATS {
        let srgb = format.is_srgb();
        let mut present = PresentPass::new(&device, format, scene_format_for(format));
        let scene = SceneTarget::new(&device, &present, w, h, scene_format_for(format));
        let mut values = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let lin = 0.02 * x as f32 / w as f32;
                let v = if srgb { lin } else { srgb_encode(lin) };
                values.push([v, v, v, 1.0]);
            }
        }
        upload_f16(&queue, &scene, &values, w, h);
        // The exact code of each column, from the half the GPU really holds.
        let exact: Vec<f64> = (0..w as usize)
            .map(|x| {
                let held = f16_to_f32(super::super::atmo_luts::f32_to_f16_bits(values[x][0]));
                255.0 * if srgb { srgb_encode(held) } else { held } as f64
            })
            .collect();
        let g = rgb_offsets(format)[1];
        let codes = |bytes: &[u8]| -> Vec<u8> { bytes.iter().skip(g).step_by(4).copied().collect() };

        present.set_dither(&queue, false);
        let plain = codes(&present_and_read(&device, &queue, &present, &scene, format));
        present.set_dither(&queue, true);
        let dithered = codes(&present_and_read(&device, &queue, &present, &scene, format));

        let plain_run = longest_run(&plain, w as usize, 0);
        let dithered_run = longest_run(&dithered, w as usize, 0);
        assert!(plain_run > N, "{format:?}: the undithered ramp's longest band is only {plain_run} px (control)");
        assert!(dithered_run <= N, "{format:?}: a band of {dithered_run} px survived the dither (limit {N})");

        let mean = |c: &[u8], x0: usize, x1: usize| -> f64 {
            let mut s = 0.0;
            for y in 0..h as usize {
                for x in x0..x1 {
                    s += c[y * w as usize + x] as f64;
                }
            }
            s / ((x1 - x0) * h as usize) as f64
        };
        let mut worst: (f64, usize) = (0.0, 0);
        for b in 0..(w as usize / 64) {
            let (x0, x1) = (b * 64, b * 64 + 64);
            let want = exact[x0..x1].iter().sum::<f64>() / 64.0;
            let d = (mean(&dithered, x0, x1) - want).abs();
            if d > worst.0 {
                worst = (d, b);
            }
        }
        assert!(worst.0 < 0.1, "{format:?}: the dithered mean is off by {:.3} code in block {}", worst.0, worst.1);
        let shift = (mean(&dithered, 0, w as usize) - mean(&plain, 0, w as usize)).abs();
        assert!(shift < 0.1, "{format:?}: the dither moved the ramp's mean by {shift:.3} code");
        println!(
            "{format:?}: longest run {plain_run} px undithered, {dithered_run} dithered; \
             worst block mean error {:.3} code; whole-ramp mean shift {shift:.4} code",
            worst.0
        );
    }
}

/// The longest run of equal codes along any row of a `w`-wide image, not
/// counting runs of the code `skip`.
fn longest_run(codes: &[u8], w: usize, skip: u8) -> usize {
    let mut best = 0;
    for row in codes.chunks(w) {
        let mut run = 0;
        for x in 0..row.len() {
            if row[x] == skip {
                run = 0;
            } else if x > 0 && row[x] == row[x - 1] {
                run += 1;
            } else {
                run = 1;
            }
            best = best.max(run);
        }
    }
    best
}

#[test]
fn longest_run_counts_runs_within_rows_only() {
    // Two rows of four: row 0 has a run of 3, row 1 a run of 2, and the
    // last code of row 0 equals the first of row 1 (not a run across rows).
    assert_eq!(longest_run(&[1, 1, 1, 2, 2, 3, 3, 4], 4, 0), 3);
    assert_eq!(longest_run(&[5, 5, 5, 5, 5, 5, 5, 5], 4, 0), 4);
    // The skipped code never counts, however long its run.
    assert_eq!(longest_run(&[0, 0, 0, 0, 0, 0, 7, 7], 8, 0), 2);
}

#[cfg(feature = "native")]
fn display_texture(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("present test display"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[cfg(feature = "native")]
fn read_back(device: &wgpu::Device, queue: &wgpu::Queue, tex: &wgpu::Texture, w: u32, h: u32) -> Vec<u8> {
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("present test readback"),
        size: (w * h * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit(std::iter::once(enc.finish()));
    let slice = buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::Maintain::Wait);
    let out = slice.get_mapped_range().to_vec();
    buffer.unmap();
    out
}

/// Clear `view` to a colour and draw a full-screen gradient with a varying
/// alpha over it, with ordinary alpha blending: the blend the scene's
/// transparent passes use. `format` is the view's format.
#[cfg(feature = "native")]
fn draw_gradient(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat, view: &wgpu::TextureView) {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("present test gradient"),
        source: wgpu::ShaderSource::Wgsl(
            "@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
                return vec4<f32>(f32((vi << 1u) & 2u) * 2.0 - 1.0, f32(vi & 2u) * 2.0 - 1.0, 0.0, 1.0);
            }
            @fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
                let u = p.x / 256.0;
                let v = p.y / 83.0;
                return vec4<f32>(u, v, fract(u * 7.0 + v), 0.25 + 0.7 * fract(u * 3.0 - v));
            }"
            .into(),
        ),
    });
    let layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: None, bind_group_layouts: &[], push_constant_ranges: &[] });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("present test gradient"),
        layout: Some(&layout),
        vertex: wgpu::VertexState { module: &shader, entry_point: Some("vs"), buffers: &[], compilation_options: Default::default() },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    {
        let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("gradient"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.02, g: 0.3, b: 0.7, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.draw(0..3, 0..1);
    }
    queue.submit(std::iter::once(enc.finish()));
}
