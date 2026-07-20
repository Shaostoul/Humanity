// Ground PBR texture array (v0.907): the four ambientCG CC0 material sets
// (grass/dirt/rock/sand, color + normal) that give planet terrain REAL
// close-range surface texture instead of pure noise. Loaded from
// assets/textures/ground/*.png into ONE 8-layer Rgba8Unorm array:
//   layers 0..3 = color  (grass, dirt, rock, sand)  -- sRGB->linear on CPU
//   layers 4..7 = normal (grass, dirt, rock, sand)  -- OpenGL +Y tangent maps
// One linear format for the whole array (an array can't mix sRGB and linear
// views per layer), so color layers are converted to linear bytes at load and
// the mip chain is built in linear space, which is also the radiometrically
// correct place to average.
//
// Fallback contract: any missing/corrupt file becomes a NEUTRAL layer
// (color = linear 0.5 grey, normal = flat +Z). The shader applies detail as
// albedo * (tex * 2.0) and perturbs the normal by the map's xy, so neutral
// layers degrade to EXACTLY the pre-texture look -- a build without the asset
// pack renders identically to v0.906, no flags needed anywhere.

use std::path::PathBuf;

pub struct GroundTextures {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

/// Layer order matches the shader's `GROUND_*` layer constants.
const FILES: [&str; 8] = [
    "grass_color.png",
    "dirt_color.png",
    "rock_color.png",
    "sand_color.png",
    "grass_normal.png",
    "dirt_normal.png",
    "rock_normal.png",
    "sand_normal.png",
];

const SIZE: u32 = 2048;

/// Locate assets/textures/ground/ the same way find_data_dir locates data/:
/// beside the exe, walking up parents (target/release -> repo root), then CWD.
fn find_ground_dir() -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.to_path_buf());
            let mut dir = exe_dir.to_path_buf();
            for _ in 0..6 {
                match dir.parent() {
                    Some(p) => {
                        candidates.push(p.to_path_buf());
                        dir = p.to_path_buf();
                    }
                    None => break,
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd);
    }
    candidates
        .into_iter()
        .map(|c| c.join("assets").join("textures").join("ground"))
        .find(|p| p.is_dir())
}

/// Decode one PNG to SIZE x SIZE RGBA bytes. `srgb_to_linear` converts color
/// maps into the array's linear byte space; normal maps stay raw.
fn load_layer(dir: &PathBuf, file: &str, srgb_to_linear: bool) -> Option<Vec<u8>> {
    let path = dir.join(file);
    let img = image::open(&path).ok()?;
    let rgba = if img.width() == SIZE && img.height() == SIZE {
        img.to_rgba8()
    } else {
        image::imageops::resize(
            &img.to_rgba8(),
            SIZE,
            SIZE,
            image::imageops::FilterType::Triangle,
        )
    };
    let mut data = rgba.into_raw();
    if srgb_to_linear {
        // Precomputed sRGB EOTF as a byte table -- the 2048^2 x4 loop below
        // is the hot path of the whole load.
        let mut table = [0u8; 256];
        for (i, t) in table.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            let lin = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            *t = (lin * 255.0 + 0.5) as u8;
        }
        for px in data.chunks_exact_mut(4) {
            px[0] = table[px[0] as usize];
            px[1] = table[px[1] as usize];
            px[2] = table[px[2] as usize];
        }
        // Partial desaturation (60% toward luma) BEFORE normalizing: the
        // per-channel normalization below equalizes channel MEANS, but on a
        // strongly tinted texture the unequal scales skew per-pixel hue
        // (bright grass-blade texels went warm-brown over France). Pulling
        // most of the texture's own hue out first keeps its structure and a
        // touch of material character (red rock speckle) while the imagery
        // owns the color.
        for px in data.chunks_exact_mut(4) {
            let luma = 0.299 * px[0] as f32 + 0.587 * px[1] as f32 + 0.114 * px[2] as f32;
            for c in 0..3 {
                px[c] = (luma + (px[c] as f32 - luma) * 0.4).clamp(0.0, 255.0) as u8;
            }
        }
        // Mean-normalize each channel to 128 (linear 0.5): the shader
        // applies detail as albedo * (tex * 2), so a layer's OWN average
        // brightness/tint must cancel out or dark materials (Grass001
        // averages a deep green) crush whole biomes toward black while the
        // NASA imagery is supposed to own the large-scale color. After
        // this, textures contribute pure STRUCTURE, energy-neutral.
        let mut sums = [0u64; 3];
        for px in data.chunks_exact(4) {
            sums[0] += px[0] as u64;
            sums[1] += px[1] as u64;
            sums[2] += px[2] as u64;
        }
        let n = (data.len() / 4) as u64;
        for c in 0..3 {
            let mean = (sums[c] / n.max(1)).max(1) as f32;
            let scale = (128.0 / mean).clamp(0.5, 3.0);
            for px in data.chunks_exact_mut(4) {
                px[c] = (px[c] as f32 * scale).min(255.0) as u8;
            }
        }
    }
    Some(data)
}

/// Neutral layer bytes: exact no-op under the shader's application rules.
fn neutral_layer(w: u32, h: u32, is_normal: bool) -> Vec<u8> {
    let px: [u8; 4] = if is_normal { [128, 128, 255, 255] } else { [128, 128, 128, 255] };
    px.iter()
        .copied()
        .cycle()
        .take((w * h * 4) as usize)
        .collect()
}

/// 2x2 box downsample (linear space, so plain byte averaging is correct).
fn downsample(src: &[u8], w: u32, h: u32) -> Vec<u8> {
    let nw = (w / 2).max(1);
    let nh = (h / 2).max(1);
    let mut out = vec![0u8; (nw * nh * 4) as usize];
    for y in 0..nh {
        let sy0 = (y * 2).min(h - 1) as usize;
        let sy1 = (y * 2 + 1).min(h - 1) as usize;
        for x in 0..nw {
            let sx0 = (x * 2).min(w - 1) as usize;
            let sx1 = (x * 2 + 1).min(w - 1) as usize;
            let i00 = (sy0 * w as usize + sx0) * 4;
            let i01 = (sy0 * w as usize + sx1) * 4;
            let i10 = (sy1 * w as usize + sx0) * 4;
            let i11 = (sy1 * w as usize + sx1) * 4;
            let o = ((y * nw + x) * 4) as usize;
            for c in 0..4 {
                let sum = src[i00 + c] as u32
                    + src[i01 + c] as u32
                    + src[i10 + c] as u32
                    + src[i11 + c] as u32;
                out[o + c] = ((sum + 2) / 4) as u8;
            }
        }
    }
    out
}

pub fn load(device: &wgpu::Device, queue: &wgpu::Queue) -> GroundTextures {
    let t0 = std::time::Instant::now();
    let dir = find_ground_dir();

    // Decode the 8 PNGs on parallel threads (each is an independent ~4 MB
    // decode + convert; serial would add ~1.5 s to startup).
    let mut layers: Vec<Option<Vec<u8>>> = vec![None; 8];
    if let Some(dir) = &dir {
        let results: Vec<Option<Vec<u8>>> = std::thread::scope(|s| {
            let handles: Vec<_> = FILES
                .iter()
                .enumerate()
                .map(|(i, f)| s.spawn(move || load_layer(dir, f, i < 4)))
                .collect();
            handles.into_iter().map(|h| h.join().unwrap_or(None)).collect()
        });
        layers = results;
    }
    let loaded_count = layers.iter().filter(|l| l.is_some()).count();

    // No assets at all (headless dev checkout, stripped install): ship a 1x1
    // neutral array instead of 130 MB of grey.
    let (size, mip_count) = if loaded_count == 0 { (1u32, 1u32) } else { (SIZE, SIZE.ilog2() + 1) };

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Ground PBR Array"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 8 },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    for (i, layer) in layers.into_iter().enumerate() {
        let mut data = layer.unwrap_or_else(|| neutral_layer(size, size, i >= 4));
        let (mut w, mut h) = (size, size);
        for mip in 0..mip_count {
            if mip > 0 {
                data = downsample(&data, w, h);
                w = (w / 2).max(1);
                h = (h / 2).max(1);
            }
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: i as u32 },
                    aspect: wgpu::TextureAspect::All,
                },
                &data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
        }
    }

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Ground PBR Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        anisotropy_clamp: 4,
        ..Default::default()
    });

    log::info!(
        "[GroundTex] {} of 8 layers loaded ({}x{}, {} mips) in {:.0} ms",
        loaded_count,
        size,
        size,
        mip_count,
        t0.elapsed().as_secs_f32() * 1000.0
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor {
        dimension: Some(wgpu::TextureViewDimension::D2Array),
        ..Default::default()
    });
    GroundTextures { view, sampler }
}
