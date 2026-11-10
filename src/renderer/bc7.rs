//! BC7 TEXTURES (2026-11-10, the look's garment textures; docs/design/character-animation.md
//! section 16): the DDS files our converter writes (`scripts/gen-makehuman-look.js` through
//! `scripts/lib/bc7.js`, our own encoder: the scan in docs/reference/research/2026-11-10-bc7-encoders/)
//! read here, and decoded to RGBA8 for a GPU without block compression (wgpu's
//! `TEXTURE_COMPRESSION_BC`; the hardware-notice rule says so on the Settings page).
//!
//! The format, as Microsoft's "BC7 Format" page gives it: a 4 by 4 block is 16 bytes, its mode the
//! count of zero bits before the first one bit, least significant first; one or two subsets (three in
//! modes 0 and 2), each a line between two endpoints, each texel an index along its subset's line,
//! interpolated as ((64 - w) * e0 + w * e1 + 32) >> 6. "BC7 decompression hardware must be bit
//! accurate", so this decoder gives the GPU's texels exactly: `the_gpu_decodes_bc7_as_we_do` checks
//! it against the GPU's own decoder, every mode we read, random blocks.
//!
//! Modes 0 and 2 (three subsets) are refused: our encoder never writes them, so no table for them is
//! carried. A block with no mode bit (the format's reserved encoding) decodes to transparent black.
//!
//! Nothing here keeps anything: a garment's file is read and (on such a GPU) decoded each time it is
//! put on the GPU.

const W2: [u32; 4] = [0, 21, 43, 64];
const W3: [u32; 8] = [0, 9, 18, 27, 37, 46, 55, 64];
const W4: [u32; 16] = [0, 4, 9, 13, 17, 21, 26, 30, 34, 38, 43, 47, 51, 55, 60, 64];

fn weights(bits: u32) -> &'static [u32] {
    match bits {
        2 => &W2,
        3 => &W3,
        _ => &W4,
    }
}

/// The 64 two-subset partitions: bit i set when texel i (row-major) is in subset 1.
pub const PARTITIONS2: [u16; 64] = [
    0xcccc, 0x8888, 0xeeee, 0xecc8, 0xc880, 0xfeec, 0xfec8, 0xec80, 0xc800, 0xffec, 0xfe80, 0xe800, 0xffe8, 0xff00, 0xfff0, 0xf000, //
    0xf710, 0x008e, 0x7100, 0x08ce, 0x008c, 0x7310, 0x3100, 0x8cce, 0x088c, 0x3110, 0x6666, 0x366c, 0x17e8, 0x0ff0, 0x718e, 0x399c, //
    0xaaaa, 0xf0f0, 0x5a5a, 0x33cc, 0x3c3c, 0x55aa, 0x9696, 0xa55a, 0x73ce, 0x13c8, 0x324c, 0x3bdc, 0x6996, 0xc33c, 0x9966, 0x0660, //
    0x0272, 0x04e4, 0x4e40, 0x2720, 0xc936, 0x936c, 0x39c6, 0x639c, 0x9336, 0x9cc6, 0x817e, 0xe718, 0xccf0, 0x0fcc, 0x7744, 0xee22,
];

/// Subset 1's fix-up texel for each two-subset partition (its index is stored one bit short).
pub const ANCHORS2: [u8; 64] = [
    15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, 15, //
    15, 2, 8, 2, 2, 8, 8, 15, 2, 8, 2, 2, 8, 8, 2, 2, //
    15, 15, 6, 8, 2, 8, 15, 15, 2, 8, 2, 2, 2, 15, 15, 6, //
    6, 2, 6, 8, 15, 15, 2, 2, 15, 15, 15, 15, 15, 2, 2, 15,
];

/// A mode's layout: subsets, partition bits, rotation bits, index-selector bit, colour and alpha
/// endpoint bits (alpha 0: decoded 255), p-bits (1 one per endpoint, 2 one per subset, 0 none),
/// primary and secondary index bits.
struct Mode {
    subsets: usize,
    partition_bits: u32,
    rotation_bits: u32,
    selector_bit: u32,
    colour_bits: u32,
    alpha_bits: u32,
    pbits: u8,
    index_bits: u32,
    index2_bits: u32,
}

const fn mode(subsets: usize, partition_bits: u32, rotation_bits: u32, selector_bit: u32, colour_bits: u32, alpha_bits: u32, pbits: u8, index_bits: u32, index2_bits: u32) -> Mode {
    Mode { subsets, partition_bits, rotation_bits, selector_bit, colour_bits, alpha_bits, pbits, index_bits, index2_bits }
}

/// Modes 1, 3 to 7 (0 and 2 are three-subset modes, refused).
const MODES: [Option<Mode>; 8] = [
    None,
    Some(mode(2, 6, 0, 0, 6, 0, 2, 3, 0)),
    None,
    Some(mode(2, 6, 0, 0, 7, 0, 1, 2, 0)),
    Some(mode(1, 0, 2, 1, 5, 6, 0, 2, 3)),
    Some(mode(1, 0, 2, 0, 7, 8, 0, 2, 2)),
    Some(mode(1, 0, 0, 0, 7, 7, 1, 4, 0)),
    Some(mode(2, 6, 0, 0, 5, 5, 1, 2, 0)),
];

struct Bits<'a> {
    block: &'a [u8; 16],
    pos: u32,
}

impl Bits<'_> {
    fn take(&mut self, n: u32) -> u32 {
        let mut v = 0;
        for k in 0..n {
            let p = self.pos;
            self.pos += 1;
            v |= (((self.block[(p >> 3) as usize] >> (p & 7)) & 1) as u32) << k;
        }
        v
    }
}

/// An endpoint value of `prec` bits widened to 8 by repeating its top bits.
fn expand(v: u32, prec: u32) -> u32 {
    if prec >= 8 {
        v
    } else {
        ((v << (8 - prec)) | (v >> (2 * prec - 8))) & 0xff
    }
}

fn interp(e0: u32, e1: u32, w: u32) -> u8 {
    (((64 - w) * e0 + w * e1 + 32) >> 6) as u8
}

/// Decode one block into 16 RGBA texels (row-major). Errs on modes 0 and 2.
pub fn decode_block(block: &[u8; 16], out: &mut [u8; 64]) -> Result<(), String> {
    let mut r = Bits { block, pos: 0 };
    let mut m = 0usize;
    while m < 8 && r.take(1) == 0 {
        m += 1;
    }
    if m == 8 {
        out.fill(0);
        return Ok(());
    }
    let Some(md) = MODES[m].as_ref() else {
        return Err(format!("a BC7 block in mode {m} (three subsets), which our encoder never writes"));
    };
    let partition = r.take(md.partition_bits) as usize;
    let rotation = r.take(md.rotation_bits);
    let selector = r.take(md.selector_bit);
    let ne = md.subsets * 2;
    let mut ep = [[0u32, 0, 0, 255]; 4];
    for c in 0..3 {
        for e in ep.iter_mut().take(ne) {
            e[c] = r.take(md.colour_bits);
        }
    }
    if md.alpha_bits > 0 {
        for e in ep.iter_mut().take(ne) {
            e[3] = r.take(md.alpha_bits);
        }
    }
    let (mut cp, mut ap) = (md.colour_bits, md.alpha_bits);
    match md.pbits {
        1 => {
            for e in ep.iter_mut().take(ne) {
                let p = r.take(1);
                for v in e.iter_mut().take(3) {
                    *v = (*v << 1) | p;
                }
                if md.alpha_bits > 0 {
                    e[3] = (e[3] << 1) | p;
                }
            }
            cp += 1;
            if md.alpha_bits > 0 {
                ap += 1;
            }
        }
        2 => {
            for s in 0..md.subsets {
                let p = r.take(1);
                for e in ep.iter_mut().skip(2 * s).take(2) {
                    for v in e.iter_mut().take(3) {
                        *v = (*v << 1) | p;
                    }
                }
            }
            cp += 1;
        }
        _ => {}
    }
    for e in ep.iter_mut().take(ne) {
        for v in e.iter_mut().take(3) {
            *v = expand(*v, cp);
        }
        e[3] = if md.alpha_bits > 0 { expand(e[3], ap) } else { 255 };
    }
    let (mask, anchor) = if md.subsets == 2 { (PARTITIONS2[partition], ANCHORS2[partition] as usize) } else { (0, usize::MAX) };
    let mut i1 = [0u32; 16];
    for (i, v) in i1.iter_mut().enumerate() {
        *v = r.take(md.index_bits - u32::from(i == 0 || i == anchor));
    }
    let mut i2 = [0u32; 16];
    if md.index2_bits > 0 {
        for (i, v) in i2.iter_mut().enumerate() {
            *v = r.take(md.index2_bits - u32::from(i == 0));
        }
    }
    for i in 0..16 {
        let s = ((mask >> i) & 1) as usize;
        let (e0, e1) = (ep[2 * s], ep[2 * s + 1]);
        let (ci, cb, ai, ab) = if md.index2_bits == 0 {
            (i1[i], md.index_bits, i1[i], md.index_bits)
        } else if selector == 1 {
            (i2[i], md.index2_bits, i1[i], md.index_bits)
        } else {
            (i1[i], md.index_bits, i2[i], md.index2_bits)
        };
        let wc = weights(cb)[ci as usize];
        let wa = weights(ab)[ai as usize];
        let mut px = [interp(e0[0], e1[0], wc), interp(e0[1], e1[1], wc), interp(e0[2], e1[2], wc), interp(e0[3], e1[3], wa)];
        if rotation > 0 {
            px.swap(3, rotation as usize - 1);
        }
        out[i * 4..i * 4 + 4].copy_from_slice(&px);
    }
    Ok(())
}

/// Decode a level of BC7 blocks (`width` and `height` whole blocks) to RGBA8, row-major.
pub fn decode_level(blocks: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let (bw, bh) = ((width / 4) as usize, (height / 4) as usize);
    if width % 4 != 0 || height % 4 != 0 || blocks.len() != bw * bh * 16 {
        return Err(format!("a BC7 level of {width} by {height} needs {} bytes, has {}", bw * bh * 16, blocks.len()));
    }
    let w = width as usize;
    let mut out = vec![0u8; w * height as usize * 4];
    let mut px = [0u8; 64];
    for by in 0..bh {
        for bx in 0..bw {
            let k = (by * bw + bx) * 16;
            let block: &[u8; 16] = blocks[k..k + 16].try_into().expect("16 bytes");
            decode_block(block, &mut px)?;
            for y in 0..4 {
                let d = ((by * 4 + y) * w + bx * 4) * 4;
                out[d..d + 16].copy_from_slice(&px[y * 16..y * 16 + 16]);
            }
        }
    }
    Ok(out)
}

/// DXGI_FORMAT_BC7_UNORM_SRGB, the format our converter's DDS files name.
const DXGI_BC7_UNORM_SRGB: u32 = 99;

/// A DDS file of BC7 sRGB blocks: level 0's size in texels and each level's blocks, biggest first.
#[derive(Debug, Clone)]
pub struct Bc7File {
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<u8>>,
}

/// The bytes a BC7 level of `width` by `height` takes (whole blocks).
pub fn level_bytes(width: u32, height: u32) -> usize {
    (width.div_ceil(4).max(1) * height.div_ceil(4).max(1) * 16) as usize
}

/// Read a DDS file of BC7 sRGB blocks (a DX10 header, `DXGI_FORMAT_BC7_UNORM_SRGB`, a 2D texture),
/// refusing anything else, a file cut short, or bytes past its levels.
pub fn read_dds(bytes: &[u8]) -> Result<Bc7File, String> {
    let u32_at = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().expect("4 bytes"));
    if bytes.len() < 148 || &bytes[0..4] != b"DDS " || &bytes[84..88] != b"DX10" {
        return Err("not a DDS file with a DX10 header".into());
    }
    if u32_at(128) != DXGI_BC7_UNORM_SRGB {
        return Err(format!("DXGI format {}, not BC7 sRGB ({DXGI_BC7_UNORM_SRGB})", u32_at(128)));
    }
    if u32_at(132) != 3 || u32_at(140) != 1 {
        return Err("not a single 2D texture".into());
    }
    let (height, width, count) = (u32_at(12), u32_at(16), u32_at(28).max(1));
    if width == 0 || height == 0 || width % 4 != 0 || height % 4 != 0 || count > 16 {
        return Err(format!("a {width} by {height} texture of {count} levels is not one of ours"));
    }
    let mut levels = Vec::with_capacity(count as usize);
    let mut at = 148usize;
    for l in 0..count {
        let n = level_bytes((width >> l).max(1), (height >> l).max(1));
        let Some(level) = bytes.get(at..at + n) else {
            return Err(format!("the file is cut short at level {l}"));
        };
        levels.push(level.to_vec());
        at += n;
    }
    if at != bytes.len() {
        return Err(format!("{} bytes past the file's levels", bytes.len() - at));
    }
    Ok(Bc7File { width, height, levels })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic generator for the random blocks (the GPU test's, native builds only).
    #[cfg(feature = "native")]
    struct Lcg(u64);
    #[cfg(feature = "native")]
    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as u32
        }
    }

    /// A random block in mode `m`: the mode's bits, then random bits for everything after them.
    #[cfg(feature = "native")]
    fn random_block(rng: &mut Lcg, m: u32) -> [u8; 16] {
        let mut b = [0u8; 16];
        for x in b.iter_mut() {
            *x = rng.next() as u8;
        }
        // The low m+1 bits are m zeros and a one.
        let mut bits = u128::from_le_bytes(b);
        bits &= !((1u128 << (m + 1)) - 1);
        bits |= 1u128 << m;
        bits.to_le_bytes()
    }

    /// THE TABLES HOLD THE FORMAT'S RULE: every two-subset partition puts texel 0 in subset 0 and
    /// its fix-up texel in subset 1 (else the stored index would lose a bit that matters). Seen red
    /// 2026-11-10 with partition 34's anchor written 7 (texel 7 is in subset 0 there): "partition 34:
    /// fix-up texel 7 is not in subset 1".
    #[test]
    fn every_partition_s_fix_up_texel_is_in_subset_1() {
        for (p, (&mask, &anchor)) in PARTITIONS2.iter().zip(ANCHORS2.iter()).enumerate() {
            assert_eq!(mask & 1, 0, "partition {p}: texel 0 is not in subset 0");
            assert_eq!((mask >> anchor) & 1, 1, "partition {p}: fix-up texel {anchor} is not in subset 1");
        }
    }

    /// A DDS FILE IS READ ONLY WHEN IT IS WHOLE AND OURS: a two-level 8 by 4 file reads back; one cut
    /// short, one with bytes past its levels and one naming another format are refused.
    #[test]
    fn a_dds_file_reads_only_when_whole_and_ours() {
        let mut f = vec![0u8; 148];
        f[0..4].copy_from_slice(b"DDS ");
        f[4..8].copy_from_slice(&124u32.to_le_bytes());
        f[12..16].copy_from_slice(&4u32.to_le_bytes());
        f[16..20].copy_from_slice(&8u32.to_le_bytes());
        f[28..32].copy_from_slice(&2u32.to_le_bytes());
        f[84..88].copy_from_slice(b"DX10");
        f[128..132].copy_from_slice(&99u32.to_le_bytes());
        f[132..136].copy_from_slice(&3u32.to_le_bytes());
        f[140..144].copy_from_slice(&1u32.to_le_bytes());
        f.extend(std::iter::repeat_n(0x40u8, 32 + 16));
        let ok = read_dds(&f).expect("a whole file reads");
        assert_eq!((ok.width, ok.height, ok.levels.len(), ok.levels[0].len(), ok.levels[1].len()), (8, 4, 2, 32, 16));
        assert!(read_dds(&f[..f.len() - 1]).is_err(), "a file cut short is refused");
        let mut long = f.clone();
        long.push(0);
        assert!(read_dds(&long).is_err(), "bytes past the levels are refused");
        let mut other = f.clone();
        other[128..132].copy_from_slice(&98u32.to_le_bytes());
        assert!(read_dds(&other).is_err(), "another format is refused");
    }

    /// THE SHIPPED GARMENT TEXTURES DECODE TO THEIR SOURCES: the converter's BC7 files (our own
    /// encoder, scripts/lib/bc7.js) read and decode here to within a few steps of the PNG they were
    /// made from, for a garment whose colour half is its diffuse texture unchanged (shoes02: 1024
    /// texels, no occlusion map, opaque) and for a card garment's alpha (eyelashes01). This is the
    /// encoder and this decoder agreeing on real data; `the_gpu_decodes_bc7_as_we_do` ties this
    /// decoder to the hardware's. Seen red 2026-11-10 with partitions 0 and 1 swapped in this
    /// decoder's table and partition 34's fix-up texel written 7: "shoes02's colour half decodes 38.4
    /// dB from its source" (the encoder's own blocks read back with the wrong subsets).
    #[test]
    fn the_shipped_garment_textures_decode_to_their_sources() {
        let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT")).join("assets/models/characters/makehuman/garments");
        let psnr = |a: &[u8], b: &[u8], chans: &[usize]| {
            let (mut se, mut n) = (0.0f64, 0u64);
            for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
                for &c in chans {
                    let d = pa[c] as f64 - pb[c] as f64;
                    se += d * d;
                    n += 1;
                }
            }
            10.0 * ((255.0f64 * 255.0) / (se / n as f64).max(1e-9)).log10()
        };
        let half = |f: &Bc7File| {
            let full = decode_level(&f.levels[0], f.width, f.height).expect("level 0 decodes");
            let s = f.height as usize;
            let mut left = Vec::with_capacity(s * s * 4);
            for y in 0..s {
                left.extend_from_slice(&full[y * s * 8..y * s * 8 + s * 4]);
            }
            left
        };
        let shoes = read_dds(&std::fs::read(root.join("shoes02/shoes02.dds")).expect("shoes02.dds")).expect("shoes02 reads");
        assert_eq!((shoes.width, shoes.height), (2048, 1024));
        let src = image::open(root.join("shoes02/shoes02_diffuse.png")).expect("the source").to_rgba8();
        let db = psnr(&half(&shoes), src.as_raw(), &[0, 1, 2]);
        println!("shoes02's colour half decodes {db:.1} dB from its source");
        assert!(db > 40.0, "shoes02's colour half decodes {db:.1} dB from its source");
        let lashes = read_dds(&std::fs::read(root.join("eyelashes01/eyelashes01.dds")).expect("eyelashes01.dds")).expect("lashes read");
        let src = image::open(root.join("eyelashes01/eyelashes01.png")).expect("the source").to_rgba8();
        let db = psnr(&half(&lashes), src.as_raw(), &[3]);
        println!("eyelashes01's alpha decodes {db:.1} dB from its source");
        assert!(db > 40.0, "eyelashes01's alpha decodes {db:.1} dB from its source");
    }

    /// THE GPU DECODES BC7 AS WE DO: random blocks in every mode we read (1, 3, 4, 5, 6, 7; every
    /// partition many times over) uploaded as a BC7 texture, read back texel by texel through a
    /// compute pass (`textureLoad` of the non-sRGB format: the hardware decoder's own bytes), and
    /// compared with `decode_block`. The format requires hardware to be bit accurate, so they must be
    /// equal; this ties the fallback decoder (and through `the_shipped_garment_textures_decode_to_
    /// their_sources`, our encoder) to what every GPU draws. Needs a GPU with block compression, so it
    /// is ignored by default: `just test-lib the_gpu_decodes_bc7 --ignored --nocapture`. Green
    /// 2026-11-10 on the operator's RTX 4070 (no block differs); seen red the same day with partitions
    /// 0 and 1 swapped and partition 34's fix-up texel written 7: "the GPU decodes 92 blocks
    /// differently (first: block Some((12, 1))); by mode [0, 33, 0, 28, 0, 0, 0, 31]".
    // The native build's: the relay build compiles the renderer without pollster or the GPU gate.
    #[cfg(feature = "native")]
    #[test]
    #[ignore]
    fn the_gpu_decodes_bc7_as_we_do() {
        if !humanity_core::test_gpu::may_use() {
            return;
        }
        #[cfg(target_os = "windows")]
        let backends = wgpu::Backends::DX12;
        #[cfg(not(target_os = "windows"))]
        let backends = wgpu::Backends::VULKAN | wgpu::Backends::METAL;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends, ..Default::default() });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("no adapter");
        if !adapter.features().contains(wgpu::Features::TEXTURE_COMPRESSION_BC) {
            println!("the_gpu_decodes_bc7_as_we_do: SKIPPED, {} offers no block compression", adapter.get_info().name);
            return;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bc7 test"),
                required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
                required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
                ..Default::default()
            },
            None,
        ))
        .expect("no device");
        device.on_uncaptured_error(Box::new(|e| panic!("wgpu validation error: {e}")));
        // 64 by 64 blocks: 4096 random blocks, each mode in turn.
        let (bw, bh) = (64u32, 64u32);
        let (w, h) = (bw * 4, bh * 4);
        let mut rng = Lcg(0xBC7_2026_1110);
        let modes = [1u32, 3, 4, 5, 6, 7];
        let mut blocks = Vec::with_capacity((bw * bh * 16) as usize);
        for k in 0..bw * bh {
            blocks.extend_from_slice(&random_block(&mut rng, modes[k as usize % modes.len()]));
        }
        use wgpu::util::DeviceExt;
        let texture = device.create_texture_with_data(
            &queue,
            &wgpu::TextureDescriptor {
                label: Some("bc7 blocks"),
                size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bc7RgbaUnorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &blocks,
        );
        let view = texture.create_view(&Default::default());
        let size = (w * h * 4) as u64;
        let out = device.create_buffer(&wgpu::BufferDescriptor { label: Some("texels"), size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let read = device.create_buffer(&wgpu::BufferDescriptor { label: Some("readback"), size, usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bc7 read"),
            source: wgpu::ShaderSource::Wgsl(
                "@group(0) @binding(0) var tex: texture_2d<f32>;
                 @group(0) @binding(1) var<storage, read_write> texels: array<u32>;
                 @compute @workgroup_size(8, 8)
                 fn main(@builtin(global_invocation_id) id: vec3<u32>) {
                     let dims = textureDimensions(tex);
                     if (id.x >= dims.x || id.y >= dims.y) { return; }
                     let c = textureLoad(tex, vec2<i32>(id.xy), 0);
                     let b = vec4<u32>(round(clamp(c, vec4<f32>(0.0), vec4<f32>(1.0)) * 255.0));
                     texels[id.y * dims.x + id.x] = b.x | (b.y << 8u) | (b.z << 16u) | (b.w << 24u);
                 }"
                .into(),
            ),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor { label: Some("bc7 read"), layout: None, module: &module, entry_point: Some("main"), compilation_options: Default::default(), cache: None });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bc7 read"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) }, wgpu::BindGroupEntry { binding: 1, resource: out.as_entire_binding() }],
        });
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.dispatch_workgroups(w / 8, h / 8, 1);
        }
        enc.copy_buffer_to_buffer(&out, 0, &read, 0, size);
        queue.submit([enc.finish()]);
        let slice = read.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::Maintain::Wait);
        let gpu = slice.get_mapped_range().to_vec();
        let ours = decode_level(&blocks, w, h).expect("every mode we read decodes");
        let mut wrong = [0usize; 8];
        let mut first = None;
        for k in 0..(bw * bh) as usize {
            let (bx, by) = (k % bw as usize, k / bw as usize);
            let mut differs = false;
            for y in 0..4 {
                let o = ((by * 4 + y) * w as usize + bx * 4) * 4;
                if gpu[o..o + 16] != ours[o..o + 16] {
                    differs = true;
                }
            }
            if differs {
                let m = modes[k % modes.len()] as usize;
                wrong[m] += 1;
                first.get_or_insert((k, m));
            }
        }
        println!("the GPU ({}) against decode_block, {} random blocks: blocks that differ by mode {wrong:?}", adapter.get_info().name, bw * bh);
        assert!(first.is_none(), "the GPU decodes {} blocks differently (first: block {:?}); by mode {wrong:?}", wrong.iter().sum::<usize>(), first);
    }
}
