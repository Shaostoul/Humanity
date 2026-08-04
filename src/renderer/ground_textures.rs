// Ground PBR texture array: the close-range surface materials the planet
// terrain blends between underfoot. Baked at renderer init into ONE
// Rgba8Unorm 2D array whose layout is driven by `data/ground/materials.ron`
// (see that file for the layer map and every tunable). Nothing here is a
// hardcoded material list; the table is data, and a lockstep test fails the
// build if it drifts from the WGSL constants that index the same layers.
//
// ── What each layer carries (v0.1101) ────────────────────────────────────
//   colour layer  RGB = an ENERGY-NEUTRAL albedo multiplier (structure only)
//                 A   = HEIGHT, a local relief field 0..1
//   normal  layer XYZ = OpenGL tangent-space normal, biased 0.5
//                 A   = PERCEPTUAL ROUGHNESS, recomputed per mip (Toksvig)
//   layer 8       the procedural ocean wave tile (not a material)
//
// The two alpha channels were previously constant 255. Filling them is what
// turns this from a diffuse-detail overlay into real PBR without touching a
// single bind group: the height drives height-aware splatting and a cavity-AO
// term, and the roughness gives the near ground the varying specular response
// that makes a low sun read as soil and leaf wax instead of flat paint.
//
// ── Why the normalization is GEOMETRIC (the v0.907 defect this fixes) ─────
// The shader consumes the colour layer as `albedo * clamp(tex * 2, 0.3, 2.0)`,
// so the layer is a MULTIPLIER and its neutral point is a geometric mean of
// 0.5, not an arithmetic one. v0.907 normalized the arithmetic mean to 128 with
// the scale clamped to 3.0 -- and every dark material blew straight through
// that clamp, because these scans are dark in LINEAR space. Measured on the
// shipped pack: grass landed at a 0.45 mean multiplier (the whole lawn silently
// darkened by more than half), and rock landed at 0.05, which the shader's own
// 0.3 floor then flattened into a CONSTANT -- rock ground had, literally, zero
// texture. Only sand (a bright material) ever worked. Normalizing the geometric
// mean makes the multiplier exactly energy-neutral for any material at any
// brightness, with no clamp in the loop, so the imagery keeps owning the
// large-scale colour and the scan contributes pure structure. Hue is handled
// separately and explicitly by the per-material `tint` (a unit-luminance
// chromaticity shift), which is what lets a forest floor be brown under dark
// green canopy imagery without any biome being brightened or darkened.
//
// Fallback contract (unchanged): any missing/corrupt file becomes a NEUTRAL
// layer -- colour = linear 0.5 grey with flat height, normal = flat +Z with the
// material's base roughness -- so a build without the asset pack degrades to
// exactly the pre-texture look instead of failing.

use std::path::PathBuf;

pub struct GroundTextures {
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

/// One entry of `data/ground/materials.ron`. Every field is `serde(default)`
/// so the table can gain tunables without invalidating an older on-disk copy
/// a modder is editing.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct GroundMaterialDef {
    pub id: String,
    pub color_file: String,
    pub normal_file: String,
    pub procedural: String,
    pub color_layer: u32,
    pub normal_layer: u32,
    pub tile_m: f32,
    pub detail_gain: f32,
    pub chroma_keep: f32,
    pub tint_linear: (f32, f32, f32),
    pub tint_strength: f32,
    pub base_roughness: f32,
    pub roughness_range: f32,
    pub height_contrast: f32,
    pub normal_strength: f32,
}

impl Default for GroundMaterialDef {
    fn default() -> Self {
        Self {
            id: String::new(),
            color_file: String::new(),
            normal_file: String::new(),
            procedural: String::new(),
            color_layer: 0,
            normal_layer: 0,
            tile_m: 2.0,
            detail_gain: 1.0,
            chroma_keep: 0.35,
            tint_linear: (0.5, 0.5, 0.5),
            tint_strength: 0.0,
            base_roughness: 0.9,
            roughness_range: 0.08,
            height_contrast: 0.35,
            normal_strength: 1.0,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct GroundMaterialTable {
    pub presence_octave_m: f32,
    pub macro_tile_mult: f32,
    pub materials: Vec<GroundMaterialDef>,
}

impl Default for GroundMaterialTable {
    fn default() -> Self {
        Self { presence_octave_m: 4.0, macro_tile_mult: 8.0, materials: Vec::new() }
    }
}

impl GroundMaterialDef {
    /// The material's own colour as a UNIT-LUMINANCE chromaticity -- what the
    /// shader's tint table holds. Dividing by luminance is the whole trick:
    /// mixing toward it rotates hue and leaves brightness untouched, so a tint
    /// can never darken a biome or blow one out (the failure mode that made
    /// v0.907 pull all hue out of the materials in the first place).
    pub fn tint_chromaticity(&self) -> [f32; 3] {
        let (r, g, b) = self.tint_linear;
        let lum = (0.299 * r + 0.587 * g + 0.114 * b).max(1e-6);
        [r / lum, g / lum, b / lum]
    }
}

/// The shipped table, compiled in. The on-disk copy under `data/` wins when it
/// is present (so the file is genuinely editable/moddable next to the exe);
/// this is what a stripped install falls back to. ONE source of truth: both
/// paths read the same RON text.
const EMBEDDED_TABLE: &str = include_str!("../../data/ground/materials.ron");

/// Layer index of the procedural ocean wave tile. Fixed at 8 by history: the
/// original quartet occupies colour 0..3 / normal 4..7 and the ocean was
/// appended after them. Additional materials append from 9, so this never
/// moves. `ocean_tex_gradient` in 20-surface-detail.wgsl reads the same index;
/// `shipped_ground_materials_match_the_shader` checks they agree.
pub const OCEAN_LAYER: u32 = 8;

const SIZE: u32 = 2048;

pub fn material_table() -> GroundMaterialTable {
    let disk = find_data_ground_dir()
        .map(|d| d.join("materials.ron"))
        .and_then(|p| std::fs::read_to_string(p).ok());
    let text = disk.as_deref().unwrap_or(EMBEDDED_TABLE);
    match ron::from_str::<GroundMaterialTable>(text) {
        Ok(t) if !t.materials.is_empty() => t,
        Ok(_) => {
            log::warn!("[GroundTex] materials.ron has no materials; using the embedded table");
            ron::from_str(EMBEDDED_TABLE).expect("embedded ground material table must parse")
        }
        Err(e) => {
            log::warn!("[GroundTex] materials.ron failed to parse ({e}); using the embedded table");
            ron::from_str(EMBEDDED_TABLE).expect("embedded ground material table must parse")
        }
    }
}

/// Walk exe dir -> parents -> CWD looking for a directory, the same discovery
/// the data loader uses. Returns the first that exists.
fn find_dir(rel: &[&str]) -> Option<PathBuf> {
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
        .map(|c| rel.iter().fold(c, |p, seg| p.join(seg)))
        .find(|p| p.is_dir())
}

fn find_ground_dir() -> Option<PathBuf> {
    find_dir(&["assets", "textures", "ground"])
}

fn find_data_ground_dir() -> Option<PathBuf> {
    find_dir(&["data", "ground"])
}

// ── Small numeric helpers ────────────────────────────────────────────────

fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: [f32; 3]) -> f32 {
    0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
}

/// Run an index-keyed pixel kernel across the available cores.
///
/// The bake's two hot loops are ~4 M independent `powf`/`ln` evaluations each.
/// Materials already bake on one thread apiece, but that leaves most of a
/// modern core count idle, and the TAIL of the slowest material is what the
/// startup actually waits on -- so the inner loops parallelise too.
fn par_pixels<T: Send>(out: &mut [T], stride: usize, f: impl Fn(usize, &mut [T]) + Sync) {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    let px = out.len() / stride.max(1);
    let per = px.div_ceil(threads).max(1);
    let fr = &f;
    std::thread::scope(|s| {
        for (ti, chunk) in out.chunks_mut(per * stride).enumerate() {
            let base = ti * per;
            s.spawn(move || fr(base, chunk));
        }
    });
}

/// Separable box blur with WRAP addressing, in place-ish. Wrapping matters:
/// the height field derived from this must tile as seamlessly as the colour
/// it came from, or every tile boundary grows a seam in the height blend.
fn box_blur_wrap(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let n = (radius * 2 + 1) as f32;
    // `%` per sample would be four integer divisions per pixel per pass over
    // 4 M texels; every index here is already inside [0, 2*len), so a single
    // conditional subtract wraps it.
    #[inline(always)]
    fn wrap(i: usize, len: usize) -> usize {
        if i >= len {
            i - len
        } else {
            i
        }
    }
    // One kernel, run twice with a transpose between: the vertical pass would
    // otherwise walk columns at stride w, which on a 2048-wide image misses
    // cache on every single sample. Two cache-blocked transposes cost far less
    // than that, and both passes then parallelise identically.
    let blur_rows = |src: &[f32], w: usize, h: usize| -> Vec<f32> {
        let mut out = vec![0.0f32; w * h];
        par_pixels(&mut out, w, |base_row, chunk| {
            for (ry, trow) in chunk.chunks_exact_mut(w).enumerate() {
                let row = &src[(base_row + ry) * w..(base_row + ry + 1) * w];
                let mut acc = 0.0f32;
                for k in 0..=(radius * 2) {
                    acc += row[wrap(k + w - radius, w)];
                }
                for (x, t) in trow.iter_mut().enumerate() {
                    *t = acc / n;
                    acc -= row[wrap(x + w - radius, w)];
                    acc += row[wrap(x + radius + 1, w)];
                }
            }
        });
        out
    };
    let transpose = |src: &[f32], w: usize, h: usize| -> Vec<f32> {
        const B: usize = 32;
        let mut out = vec![0.0f32; w * h];
        for y0 in (0..h).step_by(B) {
            for x0 in (0..w).step_by(B) {
                for y in y0..(y0 + B).min(h) {
                    for x in x0..(x0 + B).min(w) {
                        out[x * h + y] = src[y * w + x];
                    }
                }
            }
        }
        out
    };
    let horiz = blur_rows(src, w, h);
    let t = transpose(&horiz, w, h);
    let vert = blur_rows(&t, h, w);
    transpose(&vert, h, w)
}

/// Percentile stretch to 0..1 through a histogram, so a scan whose relief sits
/// in a narrow band still uses the full byte range of the height channel.
fn stretch_to_unit(v: &mut [f32], lo_pct: f32, hi_pct: f32) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &x in v.iter() {
        lo = lo.min(x);
        hi = hi.max(x);
    }
    if !(hi > lo) {
        v.iter_mut().for_each(|x| *x = 0.5);
        return;
    }
    const BINS: usize = 1024;
    let mut hist = [0u32; BINS];
    let inv = (BINS - 1) as f32 / (hi - lo);
    for &x in v.iter() {
        hist[(((x - lo) * inv) as usize).min(BINS - 1)] += 1;
    }
    let total = v.len() as f32;
    let (mut lo_bin, mut hi_bin) = (0usize, BINS - 1);
    let mut acc = 0.0f32;
    for (i, &c) in hist.iter().enumerate() {
        acc += c as f32;
        if acc / total >= lo_pct {
            lo_bin = i;
            break;
        }
    }
    // Walk DOWN from the top and stop once `hi_pct` of the mass is behind us.
    // (Comparing against `1.0 - hi_pct` here instead would walk all the way to
    // the BOTTOM percentile, collapsing the span to zero and turning the whole
    // field into a binary threshold -- which is exactly what it did before the
    // PNG dump of the grass height channel made it obvious.)
    acc = 0.0;
    for i in (0..BINS).rev() {
        acc += hist[i] as f32;
        if acc / total >= hi_pct {
            hi_bin = i;
            break;
        }
    }
    let a = lo + lo_bin as f32 / inv;
    let b = lo + hi_bin as f32 / inv;
    // A degenerate span would re-create the binary threshold; fall back to the
    // raw min/max, which is always a real range because `hi > lo` above.
    let span = if b - a > 1e-6 { b - a } else { hi - lo };
    v.iter_mut().for_each(|x| *x = ((*x - a) / span).clamp(0.0, 1.0));
}

// ── Colour layer: linear decode -> height -> energy-neutral multiplier ────

/// Decode a colour PNG to LINEAR f32 RGB at SIZE x SIZE.
fn decode_color(path: &PathBuf) -> Option<Vec<[f32; 3]>> {
    let img = image::open(path).ok()?;
    let rgba = if img.width() == SIZE && img.height() == SIZE {
        img.to_rgba8()
    } else {
        image::imageops::resize(&img.to_rgba8(), SIZE, SIZE, image::imageops::FilterType::Triangle)
    };
    let mut lut = [0.0f32; 256];
    for (i, t) in lut.iter_mut().enumerate() {
        *t = srgb_to_linear(i as f32 / 255.0);
    }
    let raw = rgba.into_raw();
    Some(raw.chunks_exact(4).map(|p| [lut[p[0] as usize], lut[p[1] as usize], lut[p[2] as usize]]).collect())
}

/// Decode a tangent-space normal PNG to unit vectors (OpenGL, +Y up).
fn decode_normal(path: &PathBuf) -> Option<Vec<[f32; 3]>> {
    let img = image::open(path).ok()?;
    let rgba = if img.width() == SIZE && img.height() == SIZE {
        img.to_rgba8()
    } else {
        image::imageops::resize(&img.to_rgba8(), SIZE, SIZE, image::imageops::FilterType::Triangle)
    };
    let raw = rgba.into_raw();
    Some(
        raw.chunks_exact(4)
            .map(|p| {
                let v = [
                    p[0] as f32 / 127.5 - 1.0,
                    p[1] as f32 / 127.5 - 1.0,
                    (p[2] as f32 / 127.5 - 1.0).max(1e-3),
                ];
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1e-6);
                [v[0] / l, v[1] / l, v[2] / l]
            })
            .collect(),
    )
}

/// Local relief from a colour map: high-passed luminance, stretched to 0..1.
///
/// These scans ship no displacement channel, but a photographed ground surface
/// carries its own ambient occlusion -- crevices are darker than the ridges
/// beside them -- so the high-pass of luminance IS the relief, at exactly the
/// scale the blend cares about. The blur radius is deliberately a fraction of
/// the tile so the material's overall brightness (which belongs to the imagery)
/// never leaks into the height, only its local structure does.
fn derive_height(color: &[[f32; 3]], size: usize) -> Vec<f32> {
    // Log luminance: relief in a photograph is a RATIO of light, so the
    // high-pass belongs in the log domain or dark regions contribute almost
    // no measured relief no matter how deep their crevices are.
    let mut lum = vec![0.0f32; color.len()];
    par_pixels(&mut lum, 1, |base, out| {
        for (i, v) in out.iter_mut().enumerate() {
            *v = luminance(color[base + i]).max(1e-5).ln();
        }
    });
    let blurred = box_blur_wrap(&lum, size, size, size / 16);
    let mut h = lum;
    for (a, b) in h.iter_mut().zip(blurred.iter()) {
        *a -= *b;
    }
    stretch_to_unit(&mut h, 0.015, 0.015);
    h
}

/// The multiplier ceiling: a byte of 255 IS a multiplier of 2.0, because the
/// shader reads this layer as `tex * 2`.
const MULT_CEIL: f32 = 2.0;
/// Where the soft shoulder starts. Below this the multiplier is exact.
const MULT_KNEE: f32 = 1.15;

/// Soft shoulder toward MULT_CEIL. These scans are heavy-tailed in LINEAR
/// space -- quartz veins in Rock035, dry clods in Ground048 -- so a hard clamp
/// at the top of the byte range flat-topped 21-24% of their texels into
/// featureless white (measured on the shipped pack). An exponential shoulder
/// asymptotes to the ceiling instead, so the brightest texels stay ORDERED and
/// keep their structure while nothing can ever encode out of range.
fn soft_shoulder(m: f32) -> f32 {
    if m <= MULT_KNEE {
        return m.max(0.0);
    }
    let head = MULT_CEIL - MULT_KNEE;
    MULT_KNEE + head * (1.0 - (-(m - MULT_KNEE) / head).exp())
}

/// Pack a colour layer: RGB = energy-neutral multiplier, A = height.
fn pack_color_layer(color: &[[f32; 3]], height: &[f32], def: &GroundMaterialDef) -> Vec<u8> {
    // Geometric mean per channel, from a 1/16 subsample (a mean needs no more
    // than that, and the logs are the expensive part of this whole bake).
    let mut log_sum = [0.0f64; 3];
    let mut n = 0u64;
    for px in color.iter().step_by(16) {
        for c in 0..3 {
            log_sum[c] += (px[c].max(1e-5) as f64).ln();
        }
        n += 1;
    }
    let gm: [f32; 3] = std::array::from_fn(|c| (log_sum[c] / n.max(1) as f64).exp() as f32);
    let gain = def.detail_gain.clamp(0.2, 3.0);
    let keep = def.chroma_keep.clamp(0.0, 1.0);
    // ratio^gain, hue pulled out toward `keep`. Geometric mean is exactly 1
    // at this point for any gain -- that is the property the whole design
    // rests on -- but the shoulder below shaves the bright tail, so `k`
    // re-centres it afterwards.
    let shape = |px: &[f32; 3]| -> [f32; 3] {
        let mut r: [f32; 3] = if (gain - 1.0).abs() < 1e-4 {
            std::array::from_fn(|c| px[c].max(1e-5) / gm[c].max(1e-6))
        } else {
            std::array::from_fn(|c| (px[c].max(1e-5) / gm[c].max(1e-6)).powf(gain))
        };
        let l = luminance(r);
        for c in 0..3 {
            r[c] = l + (r[c] - l) * keep;
        }
        r
    };
    // Re-centre: bisect for the ONE scalar pre-gain that puts the shouldered
    // multiplier's geometric mean back on 1.0. Scalar, so it cannot shift hue,
    // and measured on the same 1/16 subsample.
    let sub: Vec<[f32; 3]> = color.iter().step_by(16).map(shape).collect();
    let geo_at = |k: f32| -> f32 {
        let mut s = 0.0f64;
        for r in &sub {
            for c in 0..3 {
                s += (soft_shoulder(k * r[c]).max(1e-4) as f64).ln();
            }
        }
        (s / (sub.len() * 3).max(1) as f64).exp() as f32
    };
    let (mut lo, mut hi) = (0.5f32, 4.0f32);
    for _ in 0..14 {
        let mid = 0.5 * (lo + hi);
        if geo_at(mid) < 1.0 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let k = 0.5 * (lo + hi);
    let mut out = vec![0u8; color.len() * 4];
    par_pixels(&mut out, 4, |base, chunk| {
        for (i, o) in chunk.chunks_exact_mut(4).enumerate() {
            let r = shape(&color[base + i]);
            for c in 0..3 {
                // 0.5 is the shader's neutral point (it applies tex * 2).
                o[c] = ((soft_shoulder(k * r[c]) * 0.5 * 255.0) + 0.5).clamp(0.0, 255.0) as u8;
            }
            o[3] = ((height[base + i] * 255.0) + 0.5).clamp(0.0, 255.0) as u8;
        }
    });
    out
}

// ── Normal layer: encode + per-texel roughness ───────────────────────────

/// Perceptual roughness at mip 0. The scan gives no roughness map, but
/// microfacet theory gives the link we do have: where the tangent normal is
/// steepest the surface is busiest below the texel too, so roughness rises
/// with local slope. That is the same reasoning the Toksvig mip term below
/// formalises -- this is just its mip-0 seed.
fn seed_roughness(n: &[f32; 3], def: &GroundMaterialDef) -> f32 {
    let slope = (n[0] * n[0] + n[1] * n[1]).sqrt() / n[2].max(1e-3);
    let t = (slope / 0.9).clamp(0.0, 1.0);
    (def.base_roughness + def.roughness_range * (t * t * (3.0 - 2.0 * t))).clamp(0.02, 1.0)
}

/// Toksvig (2005) / LEAN-style roughness widening for one mip step.
///
/// Averaging four unit normals gives a vector SHORTER than 1; the lost length
/// is exactly the sub-texel normal variance that the child mip can no longer
/// represent as geometry. Folding it into roughness is the textbook fix, and
/// it is the reason this change can afford strong ground normals at all: a
/// strong bump map with a naively box-filtered mip chain shimmers viciously at
/// grazing angles, which is precisely the angle the low sun on the ground makes
/// interesting. Smooth materials widen a lot, rough ones barely at all -- which
/// is correct, and falls out of the maths rather than being tuned.
fn toksvig_roughness(parent_roughness: f32, avg_len: f32) -> f32 {
    let l = avg_len.clamp(1e-3, 1.0);
    let alpha = (parent_roughness * parent_roughness).clamp(1e-3, 1.0);
    let s = (2.0 / (alpha * alpha) - 2.0).max(0.0);
    let ft = l / (l + s * (1.0 - l)).max(1e-6);
    let s2 = ft * s;
    let alpha2 = (2.0 / (s2 + 2.0)).clamp(1e-4, 1.0);
    alpha2.sqrt().sqrt().clamp(parent_roughness, 1.0)
}

fn pack_normal_layer(normals: &[[f32; 3]], rough: &[f32]) -> Vec<u8> {
    let mut out = vec![0u8; normals.len() * 4];
    for (i, n) in normals.iter().enumerate() {
        let o = i * 4;
        for c in 0..3 {
            out[o + c] = ((n[c] * 0.5 + 0.5) * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
        }
        out[o + 3] = (rough[i] * 255.0 + 0.5).clamp(0.0, 255.0) as u8;
    }
    out
}

/// Build the whole mip chain for a normal layer, carrying roughness forward
/// through `toksvig_roughness` at every step.
fn normal_mip_chain(mut normals: Vec<[f32; 3]>, mut rough: Vec<f32>, size: u32) -> Vec<Vec<u8>> {
    let mut mips = vec![pack_normal_layer(&normals, &rough)];
    let (mut w, mut h) = (size as usize, size as usize);
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut cn = vec![[0.0f32; 3]; nw * nh];
        let mut cr = vec![0.0f32; nw * nh];
        for y in 0..nh {
            for x in 0..nw {
                let (x0, x1) = ((x * 2).min(w - 1), (x * 2 + 1).min(w - 1));
                let (y0, y1) = ((y * 2).min(h - 1), (y * 2 + 1).min(h - 1));
                let idx = [y0 * w + x0, y0 * w + x1, y1 * w + x0, y1 * w + x1];
                let mut sum = [0.0f32; 3];
                let mut rs = 0.0f32;
                for &i in &idx {
                    for c in 0..3 {
                        sum[c] += normals[i][c];
                    }
                    rs += rough[i];
                }
                for c in 0..3 {
                    sum[c] *= 0.25;
                }
                let len = (sum[0] * sum[0] + sum[1] * sum[1] + sum[2] * sum[2]).sqrt();
                let o = y * nw + x;
                if len > 1e-5 {
                    cn[o] = [sum[0] / len, sum[1] / len, sum[2] / len];
                } else {
                    cn[o] = [0.0, 0.0, 1.0];
                }
                cr[o] = toksvig_roughness(rs * 0.25, len);
            }
        }
        normals = cn;
        rough = cr;
        w = nw;
        h = nh;
        mips.push(pack_normal_layer(&normals, &rough));
    }
    mips
}

/// 2x2 box downsample of packed bytes (linear space, so plain averaging is
/// radiometrically correct; the alpha/height channel averages correctly too).
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

fn color_mip_chain(base: Vec<u8>, size: u32) -> Vec<Vec<u8>> {
    let mut mips = vec![base];
    let (mut w, mut h) = (size, size);
    while w > 1 || h > 1 {
        let next = downsample(mips.last().unwrap(), w, h);
        w = (w / 2).max(1);
        h = (h / 2).max(1);
        mips.push(next);
    }
    mips
}

// ── Procedural material: forest litter ───────────────────────────────────

/// Deterministic xorshift, returning 0..1.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        (self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    }
    fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }
}

/// One litter element: an ellipse in texture space with its own height, colour
/// and cross-section arch.
struct Litter {
    cx: f32,
    cy: f32,
    half_len: f32,
    half_wid: f32,
    ca: f32,
    sa: f32,
    base_h: f32,
    arch: f32,
    col: [f32; 3],
}

/// Bake a tileable forest-floor material: broadleaf + needle + twig litter
/// DEPTH-SORTED onto a humus substrate.
///
/// The depth test is the point. Blending overlapping leaf sprites would give a
/// brown smear; keeping the topmost element per texel gives real occlusion, so
/// leaf EDGES survive, and the height channel that falls out of it is a genuine
/// stack depth -- which is what makes the height-aware blend and the cavity AO
/// read as a mat of loose leaves rather than a printed pattern.
///
/// Sizes are metric: the caller's `tile_m` sets pixels-per-metre, so the leaves
/// are 3.5-9 cm and the needles 3-8 cm regardless of the bake resolution.
fn forest_litter_layer(size: u32, tile_m: f32) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<f32>) {
    let n_px = (size * size) as usize;
    let s = size as f32;
    let ppm = s / tile_m.max(0.01);
    let mut rng = Rng(0x1EAF_11EE_2026_0803);

    // Humus substrate: two octaves of value noise between two dark browns.
    let mut color = vec![[0.0f32; 3]; n_px];
    let mut normal = vec![[0.0f32, 0.0, 1.0]; n_px];
    let mut height = vec![0.0f32; n_px];
    let humus_a = [0.030f32, 0.021, 0.012];
    let humus_b = [0.058f32, 0.040, 0.023];
    // Small periodic lattices so the substrate tiles: 16 and 61 cells.
    let lat: [u32; 2] = [16, 61];
    let mut seeds = [[0.0f32; 61 * 61]; 2];
    for (k, sd) in seeds.iter_mut().enumerate() {
        let c = (lat[k] * lat[k]) as usize;
        for v in sd.iter_mut().take(c) {
            *v = rng.next();
        }
    }
    let noise = |sd: &[f32], n: u32, x: f32, y: f32| -> f32 {
        let fx = x * n as f32;
        let fy = y * n as f32;
        let (ix, iy) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - ix, fy - iy);
        let (ux, uy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
        let at = |a: i32, b: i32| -> f32 {
            let xa = a.rem_euclid(n as i32) as usize;
            let yb = b.rem_euclid(n as i32) as usize;
            sd[yb * n as usize + xa]
        };
        let (i, j) = (ix as i32, iy as i32);
        let a = at(i, j) * (1.0 - ux) + at(i + 1, j) * ux;
        let b = at(i, j + 1) * (1.0 - ux) + at(i + 1, j + 1) * ux;
        a * (1.0 - uy) + b * uy
    };
    for y in 0..size {
        for x in 0..size {
            let (u, v) = (x as f32 / s, y as f32 / s);
            let n0 = noise(&seeds[0], lat[0], u, v);
            let n1 = noise(&seeds[1], lat[1], u, v);
            let t = (n0 * 0.65 + n1 * 0.35).clamp(0.0, 1.0);
            let i = (y * size + x) as usize;
            for c in 0..3 {
                color[i][c] = humus_a[c] + (humus_b[c] - humus_a[c]) * t;
            }
            height[i] = 0.02 + 0.10 * t;
        }
    }

    // Litter palettes, LINEAR rgb. Real leaf fall is not one brown: it runs
    // from fresh olive through dry tan and rust to grey weathered mat, and
    // that spread is most of what stops a litter texture reading as noise.
    let leaf_cols: [[f32; 3]; 5] = [
        [0.100, 0.090, 0.035], // fresh olive
        [0.200, 0.135, 0.062], // dry tan
        [0.055, 0.035, 0.018], // dark decayed brown
        [0.095, 0.080, 0.055], // grey weathered (still cool, but a weathered
        // leaf is grey-BROWN; a neutral grey here reads as a blue leaf once
        // the geometric normalization measures it against the brown mean)
        [0.240, 0.100, 0.030], // rust
    ];
    let needle_cols: [[f32; 3]; 3] =
        [[0.075, 0.045, 0.020], [0.140, 0.085, 0.035], [0.048, 0.032, 0.016]];
    let twig_col = [0.045f32, 0.032, 0.020];

    let mut prims: Vec<Litter> = Vec::with_capacity(30_000);
    let mut push = |rng: &mut Rng,
                    prims: &mut Vec<Litter>,
                    len_m: (f32, f32),
                    wid_ratio: (f32, f32),
                    arch: (f32, f32),
                    cols: &[[f32; 3]],
                    count: usize| {
        for _ in 0..count {
            let half_len = rng.range(len_m.0, len_m.1) * 0.5 * ppm;
            let half_wid = half_len * rng.range(wid_ratio.0, wid_ratio.1);
            let a = rng.range(0.0, std::f32::consts::TAU);
            let base = cols[(rng.next() * cols.len() as f32) as usize % cols.len()];
            // Per-element brightness jitter keeps two leaves of the same
            // palette entry from reading as stamped duplicates.
            let j = rng.range(0.72, 1.35);
            prims.push(Litter {
                cx: rng.range(0.0, s),
                cy: rng.range(0.0, s),
                half_len,
                half_wid: half_wid.max(1.1),
                ca: a.cos(),
                sa: a.sin(),
                base_h: rng.range(0.12, 1.0),
                arch: rng.range(arch.0, arch.1),
                col: [base[0] * j, base[1] * j, base[2] * j],
            });
        }
    };
    // Broadleaf: 3.5-9 cm blades, moderately curled.
    push(&mut rng, &mut prims, (0.035, 0.090), (0.26, 0.42), (0.30, 0.75), &leaf_cols, 2200);
    // Conifer needles: 3-8 cm, near-flat, very thin.
    push(&mut rng, &mut prims, (0.030, 0.080), (0.020, 0.045), (0.10, 0.35), &needle_cols, 26000);
    // Twigs: 8-26 cm, round in section so they arch hard.
    push(&mut rng, &mut prims, (0.080, 0.260), (0.020, 0.045), (0.65, 1.10), &[twig_col], 240);

    // Wrap: emit ghost copies for anything crossing an edge so the tile is
    // seamless in both axes.
    let mut all: Vec<(f32, f32, usize)> = Vec::with_capacity(prims.len() * 2);
    for (i, p) in prims.iter().enumerate() {
        let r = p.half_len + 2.0;
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (ox, oy) = (dx as f32 * s, dy as f32 * s);
                let (cx, cy) = (p.cx + ox, p.cy + oy);
                if cx + r < 0.0 || cx - r > s || cy + r < 0.0 || cy - r > s {
                    continue;
                }
                all.push((cx, cy, i));
            }
        }
    }

    // Splat with a per-texel depth test, in parallel row bands.
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    let rows_per = size.div_ceil(threads as u32);
    let prims_ref = &prims;
    let all_ref = &all;
    let mut col_chunks: Vec<&mut [[f32; 3]]> = Vec::new();
    let mut nrm_chunks: Vec<&mut [[f32; 3]]> = Vec::new();
    let mut hgt_chunks: Vec<&mut [f32]> = Vec::new();
    {
        let cs = (rows_per * size) as usize;
        col_chunks.extend(color.chunks_mut(cs));
        nrm_chunks.extend(normal.chunks_mut(cs));
        hgt_chunks.extend(height.chunks_mut(cs));
    }
    std::thread::scope(|sc| {
        for (ti, ((cc, nc), hc)) in col_chunks
            .into_iter()
            .zip(nrm_chunks.into_iter())
            .zip(hgt_chunks.into_iter())
            .enumerate()
        {
            let y0 = ti as u32 * rows_per;
            let y1 = y0 + (hc.len() as u32 / size);
            sc.spawn(move || {
                for &(cx, cy, pi) in all_ref.iter() {
                    let p = &prims_ref[pi];
                    let r = p.half_len + 2.0;
                    let ylo = ((cy - r).floor().max(y0 as f32)) as i64;
                    let yhi = ((cy + r).ceil().min(y1 as f32 - 1.0)) as i64;
                    if yhi < ylo {
                        continue;
                    }
                    let xlo = ((cx - r).floor().max(0.0)) as i64;
                    let xhi = ((cx + r).ceil().min(s - 1.0)) as i64;
                    if xhi < xlo {
                        continue;
                    }
                    for y in ylo..=yhi {
                        for x in xlo..=xhi {
                            let dx = x as f32 + 0.5 - cx;
                            let dy = y as f32 + 0.5 - cy;
                            // Into the element's own frame.
                            let u = (dx * p.ca + dy * p.sa) / p.half_len;
                            let v = (-dx * p.sa + dy * p.ca) / p.half_wid;
                            let rr = u * u + v * v;
                            if rr > 1.0 {
                                continue;
                            }
                            // Height: a cylindrical arch across the width plus
                            // a slight lift toward the tip, which is what a
                            // dry curled leaf actually does.
                            let h = p.base_h + p.arch * 0.06 * (1.0 - v * v) * (0.75 + 0.25 * u.abs());
                            let i = (y as u32 - y0) as usize * size as usize + x as usize;
                            if h <= hc[i] {
                                continue;
                            }
                            hc[i] = h;
                            // Surface normal of the arch, in the element frame,
                            // then rotated back into texture space.
                            let slope_v = -p.arch * 1.15 * v;
                            let slope_u = p.arch * 0.18 * u;
                            let gx = slope_u * p.ca - slope_v * p.sa;
                            let gy = slope_u * p.sa + slope_v * p.ca;
                            let l = (gx * gx + gy * gy + 1.0).sqrt();
                            nc[i] = [-gx / l, -gy / l, 1.0 / l];
                            // Edge darkening: a leaf's own rim shadow. Cheap,
                            // and it is what separates two overlapping leaves
                            // of the same colour.
                            let edge = 0.62 + 0.38 * (1.0 - rr).sqrt();
                            cc[i] = [p.col[0] * edge, p.col[1] * edge, p.col[2] * edge];
                        }
                    }
                }
            });
        }
    });

    // Normalize the stacked heights back into 0..1.
    let mut hmax = 0.0f32;
    for &h in height.iter() {
        hmax = hmax.max(h);
    }
    let inv = 1.0 / hmax.max(1e-4);
    height.iter_mut().for_each(|h| *h = (*h * inv).clamp(0.0, 1.0));
    (color, normal, height)
}

// ── Neutral fallbacks ────────────────────────────────────────────────────

fn neutral_color_layer(n_px: usize) -> Vec<u8> {
    let mut v = vec![128u8; n_px * 4];
    // Flat height at mid-range: no material wins the height blend on relief
    // alone, so a missing scan degrades to the plain weighted average.
    v.chunks_exact_mut(4).for_each(|p| p[3] = 128);
    v
}

fn neutral_normal_layer(n_px: usize, def: &GroundMaterialDef) -> Vec<u8> {
    let r = (def.base_roughness.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
    let mut v = vec![0u8; n_px * 4];
    for p in v.chunks_exact_mut(4) {
        p[0] = 128;
        p[1] = 128;
        p[2] = 255;
        p[3] = r;
    }
    v
}

// ── Ocean wave layer (unchanged from v0.922) ─────────────────────────────

/// Procedural tiling OCEAN WAVE layer (array layer 8). RG = tangent slope xy
/// (biased 0.5), B = normalized crest height (foam mask), A = 255. A
/// random-phase sum of ~32 directional waves whose wave-vectors are INTEGER
/// cycle counts, so the tile wraps perfectly and no coherent interference
/// lattice can form. Mipped like everything else, which is the property the
/// analytic wave octaves could never have.
fn ocean_wave_layer(size: u32) -> Vec<u8> {
    let n_px = (size * size) as usize;
    let mut s: u64 = 0x0CEA_0CEA_2026_0721;
    let mut rand = move || {
        s ^= s >> 12;
        s ^= s << 25;
        s ^= s >> 27;
        (s.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32 / (1u64 << 24) as f32
    };
    struct W {
        kx: f32,
        ky: f32,
        amp: f32,
        phase: f32,
    }
    let mut waves: Vec<W> = Vec::with_capacity(32);
    while waves.len() < 32 {
        let mag = 3.0 + rand() * rand() * 37.0;
        let ang = (rand() - 0.5) * std::f32::consts::PI * 1.6;
        let kx = (mag * ang.cos()).round();
        let ky = (mag * ang.sin()).round();
        if kx.abs() < 0.5 && ky.abs() < 0.5 {
            continue;
        }
        let k = (kx * kx + ky * ky).sqrt();
        waves.push(W { kx, ky, amp: 1.0 / k.powf(1.05), phase: rand() * std::f32::consts::TAU });
    }
    let amp_sum: f32 = waves.iter().map(|w| w.amp).sum();
    let h_scale = 0.45 * amp_sum;
    let slope_scale: f32 =
        waves.iter().map(|w| w.amp * (w.kx * w.kx + w.ky * w.ky).sqrt()).sum::<f32>() * 0.5;
    let mut data = vec![0u8; n_px * 4];
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).min(16);
    let rows_per = size.div_ceil(threads as u32);
    let waves_ref = &waves;
    std::thread::scope(|sc| {
        for (ti, chunk) in data.chunks_mut((rows_per * size * 4) as usize).enumerate() {
            let y0 = ti as u32 * rows_per;
            sc.spawn(move || {
                for (row_i, row) in chunk.chunks_mut((size * 4) as usize).enumerate() {
                    let y = (y0 + row_i as u32) as f32 / size as f32;
                    for (x_i, px) in row.chunks_mut(4).enumerate() {
                        let x = x_i as f32 / size as f32;
                        let mut h = 0.0_f32;
                        let mut sx = 0.0_f32;
                        let mut sy = 0.0_f32;
                        for w in waves_ref {
                            let ph = std::f32::consts::TAU * (w.kx * x + w.ky * y) + w.phase;
                            let (sin_ph, cos_ph) = ph.sin_cos();
                            h += w.amp * cos_ph;
                            sx -= w.amp * w.kx * sin_ph;
                            sy -= w.amp * w.ky * sin_ph;
                        }
                        let nx = (sx / slope_scale).clamp(-1.0, 1.0);
                        let ny = (sy / slope_scale).clamp(-1.0, 1.0);
                        let hb = (h / h_scale).clamp(-1.0, 1.0);
                        px[0] = ((nx * 0.5 + 0.5) * 255.0) as u8;
                        px[1] = ((ny * 0.5 + 0.5) * 255.0) as u8;
                        px[2] = ((hb * 0.5 + 0.5) * 255.0) as u8;
                        px[3] = 255;
                    }
                }
            });
        }
    });
    data
}

// ── Bake ─────────────────────────────────────────────────────────────────

/// Everything one material contributes to the array, already mipped.
struct BakedMaterial {
    color_layer: u32,
    normal_layer: u32,
    color_mips: Vec<Vec<u8>>,
    normal_mips: Vec<Vec<u8>>,
    /// False when the material had no usable source at all (missing files and
    /// no generator) -- it still occupies its layers, as neutral.
    real: bool,
}

/// Bake one material at `size`. Split out so it is unit-testable at a small
/// resolution without a GPU.
fn bake_material(def: &GroundMaterialDef, dir: Option<&PathBuf>, size: u32) -> BakedMaterial {
    let n_px = (size * size) as usize;
    let mut color_lin: Option<Vec<[f32; 3]>> = None;
    let mut normals: Option<Vec<[f32; 3]>> = None;
    let mut height: Option<Vec<f32>> = None;

    if def.procedural == "forest_litter" {
        let (c, n, h) = forest_litter_layer(size, def.tile_m);
        color_lin = Some(c);
        normals = Some(n);
        height = Some(h);
    } else if let Some(dir) = dir {
        // Decode the two PNGs in parallel; each is an independent ~4 MB job.
        let (c, n) = std::thread::scope(|s| {
            let ch = (!def.color_file.is_empty())
                .then(|| s.spawn(|| decode_color(&dir.join(&def.color_file))));
            let nh = (!def.normal_file.is_empty())
                .then(|| s.spawn(|| decode_normal(&dir.join(&def.normal_file))));
            (
                ch.and_then(|h| h.join().ok()).flatten(),
                nh.and_then(|h| h.join().ok()).flatten(),
            )
        });
        if let Some(c) = c.as_ref() {
            height = Some(derive_height(c, size as usize));
        }
        color_lin = c;
        normals = n;
    }

    let real = color_lin.is_some() || normals.is_some();
    let color_mips = match (color_lin, height) {
        (Some(c), Some(h)) => color_mip_chain(pack_color_layer(&c, &h, def), size),
        _ => color_mip_chain(neutral_color_layer(n_px), size),
    };
    let normal_mips = match normals {
        Some(n) => {
            let rough: Vec<f32> = n.iter().map(|v| seed_roughness(v, def)).collect();
            normal_mip_chain(n, rough, size)
        }
        None => color_mip_chain(neutral_normal_layer(n_px, def), size),
    };
    BakedMaterial {
        color_layer: def.color_layer,
        normal_layer: def.normal_layer,
        color_mips,
        normal_mips,
        real,
    }
}

pub fn load(device: &wgpu::Device, queue: &wgpu::Queue) -> GroundTextures {
    let t0 = std::time::Instant::now();
    let table = material_table();
    let dir = find_ground_dir();

    // Probe once, cheaply: if not a single colour file is readable we are in a
    // stripped install, and a 1x1 neutral array beats 250 MB of grey.
    let any_file = dir.as_ref().is_some_and(|d| {
        table
            .materials
            .iter()
            .any(|m| !m.color_file.is_empty() && d.join(&m.color_file).is_file())
    });
    let (size, mip_count) = if any_file { (SIZE, SIZE.ilog2() + 1) } else { (1u32, 1u32) };

    let baked: Vec<BakedMaterial> = if any_file {
        std::thread::scope(|s| {
            let handles: Vec<_> = table
                .materials
                .iter()
                .map(|m| {
                    let d = dir.clone();
                    s.spawn(move || bake_material(m, d.as_ref(), size))
                })
                .collect();
            handles.into_iter().filter_map(|h| h.join().ok()).collect()
        })
    } else {
        table.materials.iter().map(|m| bake_material(m, None, size)).collect()
    };
    let real_count = baked.iter().filter(|b| b.real).count();

    let depth = table
        .materials
        .iter()
        .map(|m| m.color_layer.max(m.normal_layer))
        .max()
        .unwrap_or(0)
        .max(OCEAN_LAYER)
        + 1;

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Ground PBR Array"),
        size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: depth },
        mip_level_count: mip_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mut write = |layer: u32, mips: &[Vec<u8>]| {
        let (mut w, mut h) = (size, size);
        for (mip, data) in mips.iter().enumerate().take(mip_count as usize) {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d { x: 0, y: 0, z: layer },
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            );
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
    };
    for b in &baked {
        write(b.color_layer, &b.color_mips);
        write(b.normal_layer, &b.normal_mips);
    }
    // The ocean tile: procedural, so it exists in every install.
    let ocean = if size > 1 {
        ocean_wave_layer(size)
    } else {
        let mut v = vec![128u8; 4];
        v[2] = 128;
        v[3] = 255;
        v
    };
    write(OCEAN_LAYER, &color_mip_chain(ocean, size));

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Ground PBR Sampler"),
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        // x16, raised from x8 (v0.1108.2). Engaged at all because the shader
        // samples with textureSampleGrad; the old explicit-LOD path bypassed
        // aniso entirely (the grazing smear).
        //
        // 8 was measurably too low for the geometry this ground is viewed at: a
        // pixel on flat ground 35 m from a 1.7 m eye covers 0.05 m across the
        // sightline and 1.04 m along it, an anisotropy of 21:1. Past the clamp
        // the hardware falls back to a coarser isotropic mip, so the fine
        // photoscan tile smears to near-flat exactly where the low-frequency
        // variation octaves are still at full strength - which is the operator's
        // "large low detail texture laying over a smaller higher detail
        // texture". 16 is the common hardware maximum and costs bandwidth only
        // on the grazing pixels that were losing the detail.
        anisotropy_clamp: 16,
        ..Default::default()
    });

    log::info!(
        "[GroundTex] {}/{} materials baked into {} layers ({}x{}, {} mips) in {:.0} ms",
        real_count,
        table.materials.len(),
        depth,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Pull `var<private> NAME: array<T, N> = array<T, N>(...);` initialiser
    /// values out of the shader source as flat f32s. Type constructor names
    /// are stripped first (`vec3<f32>` would otherwise contribute a stray 3
    /// and 32 to the number stream); the array LENGTH survives as the leading
    /// value, which callers drop by taking the expected tail.
    fn wgsl_private_array(src: &str, name: &str) -> Vec<f32> {
        let decl = src
            .split_once(&format!("var<private> {name}:"))
            .unwrap_or_else(|| panic!("{name} missing from the shader"))
            .1;
        let body = decl.split_once('=').expect("array initialiser").1;
        let mut body = body[..body.find(';').expect("statement terminator")].to_string();
        for word in ["vec4<f32>", "vec3<f32>", "vec2<f32>", "array", "f32", "i32", "u32"] {
            body = body.replace(word, " ");
        }
        body.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
            .filter(|t| !t.is_empty() && t.chars().any(|c| c.is_ascii_digit()))
            .filter_map(|t| t.parse::<f32>().ok())
            .collect()
    }

    fn wgsl_const(src: &str, name: &str, ty: &str) -> f32 {
        let decl = src
            .split_once(&format!("const {name}: {ty} = "))
            .unwrap_or_else(|| panic!("{name} missing from the shader"))
            .1;
        decl[..decl.find(';').expect("terminator")].trim().parse().expect("numeric constant")
    }

    #[test]
    fn shipped_ground_materials_parse() {
        let t: GroundMaterialTable =
            ron::from_str(EMBEDDED_TABLE).expect("data/ground/materials.ron must parse");
        assert!(t.materials.len() >= 4, "the table lost materials");
        for m in &t.materials {
            assert!(!m.id.is_empty(), "every material needs an id");
            // tile_m MUST divide the 64 m camera anchor modulus or the tile
            // phase jumps when the anchor re-snaps (the f32-at-planet-scale
            // discipline, CLAUDE.md).
            let t64 = 64.0 / m.tile_m;
            assert!(
                m.tile_m > 0.0 && (t64 - t64.round()).abs() < 1e-4 && t64.round() >= 1.0,
                "{}: tile_m {} does not divide 64 m",
                m.id,
                m.tile_m
            );
            assert!(m.base_roughness > 0.0 && m.base_roughness <= 1.0, "{}: roughness", m.id);
            assert!((0.0..=1.0).contains(&m.tint_strength), "{}: tint_strength", m.id);
            assert!(m.color_layer != OCEAN_LAYER && m.normal_layer != OCEAN_LAYER,
                "{}: collides with the ocean layer", m.id);
        }
        // No two materials may share a layer.
        let mut used: Vec<u32> = t.materials.iter().flat_map(|m| [m.color_layer, m.normal_layer]).collect();
        used.sort_unstable();
        let n = used.len();
        used.dedup();
        assert_eq!(used.len(), n, "two materials share a texture-array layer");
    }

    /// The lockstep guard: the RON table and the WGSL tables that index the
    /// same layers must agree, entry for entry.
    #[test]
    fn shipped_ground_materials_match_the_shader() {
        let src = crate::renderer::shader_loader::assembled_pbr_source();
        let t = material_table();
        let n = t.materials.len();
        assert_eq!(
            wgsl_const(src, "GROUND_MAT_COUNT", "i32") as usize,
            n,
            "GROUND_MAT_COUNT drifted from data/ground/materials.ron"
        );
        assert_eq!(
            wgsl_const(src, "GROUND_LAYER_OCEAN", "i32") as u32,
            OCEAN_LAYER,
            "the shader's ocean layer drifted"
        );
        assert!(
            (wgsl_const(src, "GROUND_PRESENCE_M", "f32") - t.presence_octave_m).abs() < 1e-4,
            "GROUND_PRESENCE_M drifted from presence_octave_m"
        );
        assert!(
            (wgsl_const(src, "GROUND_MACRO_MULT", "f32") - t.macro_tile_mult).abs() < 1e-4,
            "GROUND_MACRO_MULT drifted from macro_tile_mult"
        );

        let take_tail = |v: Vec<f32>, want: usize| -> Vec<f32> {
            assert!(v.len() >= want, "shader table is shorter than the RON table");
            v[v.len() - want..].to_vec()
        };
        let color_layers = take_tail(wgsl_private_array(src, "GROUND_COLOR_LAYER"), n);
        let normal_layers = take_tail(wgsl_private_array(src, "GROUND_NORMAL_LAYER"), n);
        let tiles = take_tail(wgsl_private_array(src, "GROUND_TILE_M"), n);
        let depth = take_tail(wgsl_private_array(src, "GROUND_HEIGHT_CONTRAST"), n);
        let nstr = take_tail(wgsl_private_array(src, "GROUND_NORMAL_STRENGTH"), n);
        let tstr = take_tail(wgsl_private_array(src, "GROUND_TINT_STRENGTH"), n);
        let tint = take_tail(wgsl_private_array(src, "GROUND_TINT_RGB"), n * 3);

        for (i, m) in t.materials.iter().enumerate() {
            let near = |a: f32, b: f32, what: &str| {
                assert!((a - b).abs() < 2e-3, "{}: {what} {a} != {b}", m.id);
            };
            near(color_layers[i], m.color_layer as f32, "color_layer");
            near(normal_layers[i], m.normal_layer as f32, "normal_layer");
            near(tiles[i], m.tile_m, "tile_m");
            near(depth[i], m.height_contrast, "height_contrast");
            near(nstr[i], m.normal_strength, "normal_strength");
            near(tstr[i], m.tint_strength, "tint_strength");
            let c = m.tint_chromaticity();
            for k in 0..3 {
                near(tint[i * 3 + k], c[k], "tint chromaticity");
            }
        }
    }

    // ── The classifier twin (v0.1103 rewrite) ────────────────────────────
    //
    // WHY THIS TEST WAS REWRITTEN. The v0.1101 version hardcoded seven raw
    // linear colours "measured out of earth_albedo.bin" and fed them straight
    // to a copy of the classifier. It passed, and it proved nothing, because
    // the SHADER is not fed raw imagery -- it samples the GRADED bake. The
    // twin therefore agreed with the classifier in a space the classifier
    // never sees, and a defect that made `forest_litter` unreachable
    // everywhere on Earth sailed through a green test. (This is the repo's
    // dominant defect class: a check whose evidence is its own setup.)
    //
    // The rewrite closes both halves of that gap:
    //   * the sample colours are READ OUT of the shipped grid at named
    //     coordinates instead of being copied into a literal, so they cannot
    //     drift from the imagery;
    //   * they are then pushed through the REAL bake
    //     (`planet_surface::grade_albedo`) before the classifier sees them,
    //     which is exactly what happens on the GPU.
    // Delete either half and the test stops being able to fail.

    fn ss(a: f32, b: f32, x: f32) -> f32 {
        let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    fn lum3(c: [f32; 3]) -> f32 {
        0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]
    }

    /// Twin of `ground_ungrade` in 20-surface-detail.wgsl: the closed-form
    /// inverse of `planet_surface::land_gain`.
    fn ungrade(img: [f32; 3]) -> [f32; 3] {
        const GAIN: f32 = 1.6;
        const KNEE: f32 = 0.15;
        let lg = lum3(img);
        if lg <= 1.0e-6 {
            return img;
        }
        let greenness = ((img[1] - img[0].max(img[2])) / img[1].max(0.001)).clamp(0.0, 1.0);
        let vt = ((greenness - 0.1) / 0.5).clamp(0.0, 1.0);
        let a = GAIN * (1.0 + 0.5 * (vt * vt * (3.0 - 2.0 * vt)));
        let l_raw =
            if lg < KNEE * a { (lg * lg) / (a * a * KNEE) } else { lg / a };
        let s = l_raw / lg;
        [img[0] * s, img[1] * s, img[2] * s]
    }

    /// Twin of `ground_detail`'s snow gate. Takes the GRADED sample, i.e.
    /// what the GPU reads.
    fn snow_gate(graded: [f32; 3]) -> f32 {
        ss(0.50, 0.68, lum3(ungrade(graded)))
    }

    /// Twin of `ground_material_weights`, entered the way the shader enters
    /// it: with the GRADED texture sample. Order: grass, dirt, rock, sand,
    /// litter. The `ungrade` call on the first line is the fix under test --
    /// remove it and this file's own biome expectations fail.
    fn weights(graded: [f32; 3], steep: f32) -> [f32; 5] {
        let img = ungrade(graded);
        let lum = lum3(img);
        let w_rock = ss(0.20, 0.50, steep);
        let flat = 1.0 - w_rock;
        let gr = img[1] / img[0].max(img[2]).max(0.003);
        let green = ss(1.02, 1.14, gr);
        let canopy = 1.0 - ss(0.030, 0.055, lum);
        let sand = ss(0.02, 0.08, img[0] - img[2]) * ss(0.18, 0.32, lum);
        let dry =
            ss(0.04, 0.09, lum) * (1.0 - ss(0.13, 0.22, lum)) * (1.0 - green) * (1.0 - sand);
        let mut w = [0.0f32; 5];
        w[0] = flat * (green * (1.0 - canopy) + 0.45 * dry);
        w[2] = w_rock;
        w[3] = flat * sand * (1.0 - green);
        w[4] = flat * green * canopy;
        w[1] = (1.0 - w[0] - w[2] - w[3] - w[4]).max(0.0);
        w
    }

    const GRASS: usize = 0;
    const DIRT: usize = 1;
    const ROCK: usize = 2;
    const SAND: usize = 3;
    const LITTER: usize = 4;

    fn repo_file(rel: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)
    }

    /// Earth's def, straight off disk. Panics loudly rather than skipping: a
    /// checkout without it cannot verify anything this module claims.
    fn earth_def() -> crate::terrain::planet::PlanetDef {
        let p = repo_file("data/planets/earth.ron");
        let text = std::fs::read_to_string(&p)
            .unwrap_or_else(|e| panic!("{} must be readable ({e})", p.display()));
        ron::from_str(&text).expect("data/planets/earth.ron must parse as a PlanetDef")
    }

    /// The colour the GPU actually samples at a place: the shipped imagery,
    /// pushed through the shipped bake. `grade_albedo` is the single source of
    /// truth the texture bake itself uses, so this cannot drift from it.
    fn graded_sample_at(
        al: &crate::terrain::planet_albedo::PlanetAlbedo,
        def: &crate::terrain::planet::PlanetDef,
        lat: f32,
        lon: f32,
    ) -> ([f32; 3], [f32; 3]) {
        // Median of a 5x5 texel box (~0.44 deg) so one cloud, river or lake
        // texel cannot decide a biome's expectation.
        let mut ch: [Vec<f32>; 3] = [Vec::new(), Vec::new(), Vec::new()];
        for dy in -2..=2 {
            for dx in -2..=2 {
                let c =
                    al.sample_linear_latlon(lat + dy as f32 * 0.088, lon + dx as f32 * 0.088);
                for k in 0..3 {
                    ch[k].push(c[k]);
                }
            }
        }
        let mut raw = [0.0f32; 3];
        for k in 0..3 {
            ch[k].sort_by(f32::total_cmp);
            raw[k] = ch[k][ch[k].len() / 2];
        }
        // Well above sea level and clear of the shore de-blue band, which is
        // the regime every walkable land fragment is in.
        let sea = def.sea_level.clamp(0.0, 1.0);
        let graded = crate::terrain::planet_surface::grade_albedo(
            def,
            raw,
            sea + 0.05,
            (lat.to_radians()).sin().abs(),
        );
        (raw, graded)
    }

    /// (place, lat, lon, slope, dominant material). Coordinates chosen for
    /// box agreement: 25/25 texels of the 5x5 box classify identically at
    /// every site except Fuji (24/25) and the Pacific NW (21/25).
    const BIOME_CASES: &[(&str, f32, f32, f32, usize)] = &[
        ("Fuji conifer forest", 35.36, 138.73, 0.05, LITTER),
        ("Amazon rainforest", -4.5, -63.0, 0.05, LITTER),
        ("Congo rainforest", 0.5, 22.0, 0.05, LITTER),
        ("Siberian taiga", 60.0, 100.0, 0.05, LITTER),
        ("Pacific NW forest", 47.5, -122.9, 0.05, LITTER),
        ("Iowa cropland", 42.0, -93.5, 0.05, LITTER),
        ("Kansas high plains", 39.5, -100.5, 0.05, DIRT),
        ("Serengeti savanna", -2.3, 34.8, 0.05, DIRT),
        ("Sahara desert", 25.0, 5.0, 0.05, SAND),
        ("Fuji forest on a cliff", 35.36, 138.73, 0.7, ROCK),
    ];

    /// The classifier is the ONLY biome signal on this path, so its behaviour
    /// against the real shipped imagery is a contract, not a detail.
    #[test]
    fn imagery_classifier_puts_the_right_material_under_each_biome() {
        let def = earth_def();
        let al_rel = def.albedo.clone().expect("earth.ron must ship an albedo grid");
        let al = crate::terrain::planet_albedo::PlanetAlbedo::load(&repo_file(&format!(
            "data/{al_rel}"
        )))
        .expect("the shipped Earth albedo grid must load");

        for &(name, lat, lon, steep, want) in BIOME_CASES {
            let (raw, graded) = graded_sample_at(&al, &def, lat, lon);
            let w = weights(graded, steep);
            let got = (0..5).max_by(|a, b| w[*a].total_cmp(&w[*b])).unwrap();
            assert_eq!(
                got, want,
                "{name}: raw {raw:?} (lum {:.4}) -> graded {graded:?} (lum {:.4}) -> weights {w:?}",
                lum3(raw),
                lum3(graded)
            );
            // Partition of unity, or the height blend normalises against a lie.
            let s: f32 = w.iter().sum();
            assert!((s - 1.0).abs() < 1e-4, "{name}: weights sum to {s}");
        }

        // Savanna must carry REAL dry grass over the soil, not bare earth.
        let (_, serengeti) = graded_sample_at(&al, &def, -2.3, 34.8);
        let sav = weights(serengeti, 0.05);
        assert!(
            sav[GRASS] > 0.3 && sav[DIRT] > 0.3,
            "savanna should blend dry grass into soil, got {sav:?}"
        );

        // The forest floor is the whole reason forest_litter exists. Assert
        // the WEIGHT, not just the argmax: a 51% win would still render as
        // half lawn.
        for &(name, lat, lon, steep, want) in BIOME_CASES {
            if want != LITTER {
                continue;
            }
            let (_, graded) = graded_sample_at(&al, &def, lat, lon);
            let w = weights(graded, steep);
            assert!(
                w[LITTER] > 0.9,
                "{name}: closed canopy must be almost pure leaf mat, got {w:?}"
            );
        }
    }

    /// The snow gate switches the ENTIRE ground layer off (`keep *= 1 -
    /// snowy`), so reading it in the wrong space is not a tint bug, it is an
    /// off switch. Shipped v0.1101 fed it the graded value and the Sahara
    /// measured 0.89 "snow" -- 89% of the desert's ground PBR, gone.
    #[test]
    fn the_snow_gate_does_not_switch_off_the_desert() {
        let def = earth_def();
        let al_rel = def.albedo.clone().expect("earth.ron must ship an albedo grid");
        let al = crate::terrain::planet_albedo::PlanetAlbedo::load(&repo_file(&format!(
            "data/{al_rel}"
        )))
        .expect("the shipped Earth albedo grid must load");
        for &(name, lat, lon) in
            &[("Sahara", 25.0f32, 5.0f32), ("Arabian desert", 21.0, 46.0)]
        {
            let (raw, graded) = graded_sample_at(&al, &def, lat, lon);
            let g = snow_gate(graded);
            assert!(
                g < 0.01,
                "{name} must not read as snow: raw lum {:.4}, graded lum {:.4}, gate {g:.3}",
                lum3(raw),
                lum3(graded)
            );
        }
    }

    /// The shader's un-grade is a CLOSED FORM inverse of `land_gain`, so it is
    /// only correct while `land_gain` keeps its shape. This is the lockstep
    /// guard: sweep the whole colour domain the bake can produce, forward
    /// through the REAL `grade_albedo`, back through the twin, and require the
    /// raw value to come back. Any edit to the gain, the knee, the exponent or
    /// the vegetation lift fails here rather than silently un-selecting a
    /// material six months later.
    #[test]
    fn ground_ungrade_exactly_inverts_the_shipped_bake() {
        use crate::terrain::planet_surface as ps;
        // The closed form squares the graded luminance to undo the shadow
        // lift, which is only the inverse of a SQUARE ROOT.
        assert!(
            (ps::LAND_SHADOW_EXP - 0.5).abs() < 1e-6,
            "20-surface-detail.wgsl's ground_ungrade assumes LAND_SHADOW_EXP == 0.5; \
             it is now {}. Re-derive the inverse (l_raw = (lg / (a*knee^e))^(1/(1-e)) * knee) \
             before changing this constant.",
            ps::LAND_SHADOW_EXP
        );
        let src = crate::renderer::shader_loader::assembled_pbr_source();
        assert!(
            (wgsl_const(src, "GROUND_LAND_GAIN", "f32") - ps::LAND_ALBEDO_GAIN).abs() < 1e-6,
            "GROUND_LAND_GAIN drifted from planet_surface::LAND_ALBEDO_GAIN"
        );
        assert!(
            (wgsl_const(src, "GROUND_LAND_KNEE", "f32") - ps::LAND_SHADOW_KNEE).abs() < 1e-6,
            "GROUND_LAND_KNEE drifted from planet_surface::LAND_SHADOW_KNEE"
        );

        let def = earth_def();
        let sea = def.sea_level.clamp(0.0, 1.0);
        let mut worst = 0.0f32;
        let mut worst_at = [0.0f32; 3];
        // Sweep brightness across four decades and hue from red-dominant
        // (desert) through neutral to strongly green-dominant (canopy), which
        // spans every regime of land_gain including both sides of its knee.
        for i in 0..40 {
            let base = 0.001 * (10.0f32).powf(i as f32 / 13.0);
            for j in 0..9 {
                let g_bias = 0.6 + 0.2 * j as f32;
                let raw = [base, (base * g_bias).min(1.0), base * 0.45];
                let graded = ps::grade_albedo(&def, raw, sea + 0.05, 0.3);
                // A channel that clipped at 1.0 is unrecoverable by
                // construction (information destroyed at bake); the snow gate
                // owns that regime.
                if graded.iter().any(|&c| c >= 0.999) {
                    continue;
                }
                let back = ungrade(graded);
                let err = (lum3(back) - lum3(raw)).abs() / lum3(raw).max(1e-9);
                if err > worst {
                    worst = err;
                    worst_at = raw;
                }
            }
        }
        assert!(
            worst < 2.0e-3,
            "ground_ungrade no longer inverts grade_albedo: {:.4}% error at raw {worst_at:?}",
            worst * 100.0
        );
    }

    /// `ground_ungrade` is applied UNCONDITIONALLY, which is only sound
    /// because every planet that can reach the ground classifier is a graded
    /// one. The chain: the classifier needs `has_tex`, `has_tex` needs a baked
    /// albedo texture, and `engine::net_route` only bakes one when the planet
    /// ships BOTH a heightmap and an albedo grid -- and `grade_albedo` returns
    /// raw imagery untouched when `has_water` is false.
    ///
    /// So the day someone gives Mars a heightmap, the Mars ground would be
    /// un-graded a second time and lose ~91% of its sand (measured against
    /// mars_albedo.bin). This test is the tripwire for that day. If it fires,
    /// the fix is to pass a "was graded" flag into `ground_detail` rather than
    /// to weaken the assertion.
    #[test]
    fn only_graded_planets_can_reach_the_ground_classifier() {
        let dir = repo_file("data/planets");
        let mut checked = 0;
        for entry in std::fs::read_dir(&dir).expect("data/planets must exist") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(def) = ron::from_str::<crate::terrain::planet::PlanetDef>(&text) else {
                continue;
            };
            checked += 1;
            if def.heightmap.is_some() && def.albedo.is_some() {
                assert!(
                    def.has_water,
                    "{}: ships both grids, so it reaches ground_detail, but has_water is \
                     false -- its imagery is NOT land-gain graded and ground_ungrade would \
                     corrupt its classification. See the note on ground_ungrade in \
                     assets/shaders/pbr/20-surface-detail.wgsl.",
                    path.display()
                );
            }
        }
        assert!(checked >= 4, "expected to inspect every planet def, saw {checked}");
    }

    /// The whole point of the geometric normalization: whatever the material's
    /// brightness, the multiplier it hands the shader averages to 1.0 in the
    /// domain it is actually used in (multiplicative, i.e. log). v0.907's
    /// arithmetic version failed this for three of the four shipped scans.
    #[test]
    fn color_multiplier_is_energy_neutral_for_dark_and_bright_materials() {
        let def = GroundMaterialDef::default();
        for &scale in &[0.004f32, 0.05, 0.5, 0.9] {
            let n = 64 * 64;
            let mut rng = Rng(0xC0FFEE ^ (scale.to_bits() as u64));
            let color: Vec<[f32; 3]> = (0..n)
                .map(|_| {
                    let v = scale * rng.range(0.25, 2.5);
                    [v * 1.1, v, v * 0.8]
                })
                .collect();
            let height = vec![0.5f32; n];
            let bytes = pack_color_layer(&color, &height, &def);
            // The shader's own reading of this layer: multiplier = tex * 2.
            let mut log_sum = 0.0f64;
            for p in bytes.chunks_exact(4) {
                let m = (p[1] as f32 / 255.0) * 2.0;
                log_sum += (m.max(1e-4) as f64).ln();
            }
            let geo_mean = (log_sum / n as f64).exp();
            assert!(
                (geo_mean - 1.0).abs() < 0.06,
                "material at scale {scale} multiplies the ground by {geo_mean:.3}, not ~1.0"
            );
        }
    }

    /// A dark material must still deliver real contrast rather than collapsing
    /// onto the shader's 0.3 clamp floor (the measured v0.907 rock failure).
    #[test]
    fn dark_material_keeps_its_contrast() {
        let def = GroundMaterialDef::default();
        let n = 64 * 64;
        let mut rng = Rng(0xDEAD_BEEF);
        // Rock035-like: linear mean ~0.008 with a wide spread.
        let color: Vec<[f32; 3]> = (0..n)
            .map(|_| {
                let v = 0.008 * rng.range(0.2, 4.0);
                [v, v * 1.05, v * 1.2]
            })
            .collect();
        let bytes = pack_color_layer(&color, &vec![0.5; n], &def);
        let mult: Vec<f32> =
            bytes.chunks_exact(4).map(|p| (p[1] as f32 / 255.0 * 2.0).max(0.3)).collect();
        let mean = mult.iter().sum::<f32>() / n as f32;
        let sd = (mult.iter().map(|m| (m - mean) * (m - mean)).sum::<f32>() / n as f32).sqrt();
        assert!(sd > 0.15, "dark material flattened to sd {sd:.3} (clamped to a constant)");
    }

    /// A heavy-tailed scan (Rock035's quartz veins, Ground048's dry clods)
    /// must not flat-top into featureless white. Measured on the shipped pack
    /// before the soft shoulder went in: 21% of dirt texels and 24% of rock
    /// texels encoded at 255, i.e. a fifth of both materials was a constant.
    /// After the shoulder plus each material's tuned `detail_gain`, the same
    /// measurement reads well under 1% for every shipped material.
    #[test]
    fn heavy_tailed_material_does_not_flat_top() {
        // The shoulder's contract, which a hard clamp cannot make: exact below
        // the knee, strictly increasing above it, and never actually reaching
        // the ceiling -- so the bright tail keeps its ORDER.
        assert!((soft_shoulder(0.9) - 0.9).abs() < 1e-6);
        assert!(soft_shoulder(3.0) < soft_shoulder(6.0));
        assert!(soft_shoulder(6.0) < soft_shoulder(30.0));
        // Asymptotic, so it can never encode out of range (f32 does reach the
        // ceiling exactly once the exponential underflows, far past any real
        // texel value).
        assert!(soft_shoulder(1.0e6) <= MULT_CEIL);

        // A LOG-NORMAL, which is the shape a photographed surface actually
        // has in linear space. sigma 0.9 puts the 99th percentile about 8x the
        // geometric mean -- a touch worse than the measured Ground048, and
        // roughly what Rock035 looks like once its detail_gain is applied.
        let def = GroundMaterialDef { detail_gain: 1.0, ..Default::default() };
        let n = 128 * 128;
        let mut rng = Rng(0xBADC0DE);
        let color: Vec<[f32; 3]> = (0..n)
            .map(|_| {
                // Box-Muller from the same deterministic stream.
                let u1 = rng.next().max(1e-6);
                let u2 = rng.next();
                let g = (-2.0 * u1.ln()).sqrt() * (std::f32::consts::TAU * u2).cos();
                let v = 0.01 * (0.9 * g).exp();
                [v, v * 0.96, v * 0.9]
            })
            .collect();
        let bytes = pack_color_layer(&color, &vec![0.5; n], &def);
        let clipped = bytes
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .filter(|&b| b >= 254)
            .count() as f32
            / (n * 3) as f32;
        // sigma 0.9 at gain 1.0 is a harder input than ANY shipped scan at its
        // tuned gain (the shipped four measure 0.0-0.5%). The pre-shoulder code
        // put 26% of this same input on the ceiling.
        assert!(clipped < 0.08, "{:.2}% of texels flat-topped", clipped * 100.0);
        // Re-centring must survive the shoulder.
        let mut ls = 0.0f64;
        for p in bytes.chunks_exact(4) {
            ls += ((p[0] as f32 / 255.0 * 2.0).max(1e-4) as f64).ln();
        }
        let geo = (ls / n as f64).exp();
        assert!((geo - 1.0).abs() < 0.08, "shoulder left the mean at {geo:.3}");
    }

    /// The percentile stretch must produce a real DISTRIBUTION, not a binary
    /// threshold. It shipped inverted once (the upper search compared against
    /// `1 - hi_pct`, so both cut points landed on the same low percentile and
    /// every derived height map became 98.5% white with a speckle of black);
    /// the height blend and the cavity AO are both inert when that happens,
    /// and nothing else in the pipeline notices.
    #[test]
    fn percentile_stretch_spreads_the_distribution() {
        let mut rng = Rng(0x57E7C4);
        // Gaussian-ish, the shape a high-passed photo has.
        let mut v: Vec<f32> = (0..40_000)
            .map(|_| {
                let s: f32 = (0..6).map(|_| rng.range(-1.0, 1.0)).sum();
                s * 0.4 + 7.0
            })
            .collect();
        stretch_to_unit(&mut v, 0.015, 0.015);
        let mean = v.iter().sum::<f32>() / v.len() as f32;
        let sd = (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f32>() / v.len() as f32).sqrt();
        assert!((mean - 0.5).abs() < 0.08, "stretched mean {mean:.3} is not centred");
        assert!(sd > 0.15, "stretched sd {sd:.3} -- collapsed to a threshold");
        // Genuinely spread across the range, not piled at the two ends.
        let mids = v.iter().filter(|&&x| (0.25..0.75).contains(&x)).count() as f32 / v.len() as f32;
        assert!(mids > 0.4, "only {:.0}% of values are mid-range", mids * 100.0);
        assert!(v.iter().all(|&x| (0.0..=1.0).contains(&x)));
        // A constant input must not divide by zero.
        let mut flat = vec![3.0f32; 100];
        stretch_to_unit(&mut flat, 0.015, 0.015);
        assert!(flat.iter().all(|x| x.is_finite()));
    }

    /// The wrapping box blur is the height field's whole basis, and it is
    /// written for speed (transposed, parallel, conditional-subtract wrapping
    /// instead of `%`). Pin it against the obvious naive definition.
    #[test]
    fn box_blur_wrap_matches_the_naive_definition() {
        let (w, h, r) = (37usize, 23usize, 4usize);
        let mut rng = Rng(0xB0B);
        let src: Vec<f32> = (0..w * h).map(|_| rng.range(-3.0, 5.0)).collect();
        let got = box_blur_wrap(&src, w, h, r);
        let n = ((2 * r + 1) * (2 * r + 1)) as f32;
        for y in 0..h {
            for x in 0..w {
                let mut sum = 0.0f32;
                for dy in -(r as i64)..=(r as i64) {
                    for dx in -(r as i64)..=(r as i64) {
                        let sx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                        let sy = (y as i64 + dy).rem_euclid(h as i64) as usize;
                        sum += src[sy * w + sx];
                    }
                }
                let want = sum / n;
                assert!(
                    (got[y * w + x] - want).abs() < 2e-3,
                    "({x},{y}): {} != {want}",
                    got[y * w + x]
                );
            }
        }
    }

    /// Toksvig must widen roughness monotonically as detail averages away, and
    /// must be a no-op when nothing was lost.
    #[test]
    fn toksvig_widens_roughness_monotonically() {
        assert!((toksvig_roughness(0.5, 1.0) - 0.5).abs() < 1e-3, "no variance, no widening");
        let a = toksvig_roughness(0.5, 0.98);
        let b = toksvig_roughness(0.5, 0.90);
        let c = toksvig_roughness(0.5, 0.70);
        assert!(0.5 <= a && a < b && b < c && c <= 1.0, "not monotonic: {a} {b} {c}");
        // A rough material barely moves; a smooth one moves a lot. That
        // asymmetry is the whole reason this fixes specular shimmer.
        let rough_delta = toksvig_roughness(0.95, 0.85) - 0.95;
        let smooth_delta = toksvig_roughness(0.30, 0.85) - 0.30;
        assert!(smooth_delta > rough_delta * 3.0, "{smooth_delta} vs {rough_delta}");
    }

    /// The normal mip chain must stay unit-length and must not lose roughness.
    #[test]
    fn normal_mip_chain_stays_unit_and_gains_roughness() {
        let size = 32u32;
        let n = (size * size) as usize;
        let mut rng = Rng(0x5EED);
        let normals: Vec<[f32; 3]> = (0..n)
            .map(|_| {
                let v = [rng.range(-0.8, 0.8), rng.range(-0.8, 0.8), 1.0];
                let l = (v[0] * v[0] + v[1] * v[1] + 1.0).sqrt();
                [v[0] / l, v[1] / l, v[2] / l]
            })
            .collect();
        let rough = vec![0.4f32; n];
        let mips = normal_mip_chain(normals, rough, size);
        assert_eq!(mips.len(), size.ilog2() as usize + 1);
        let mut prev = 0.0f32;
        for (level, m) in mips.iter().enumerate() {
            let mut rsum = 0.0f32;
            let mut count = 0.0f32;
            for p in m.chunks_exact(4) {
                let v = [
                    p[0] as f32 / 127.5 - 1.0,
                    p[1] as f32 / 127.5 - 1.0,
                    p[2] as f32 / 127.5 - 1.0,
                ];
                let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                assert!((len - 1.0).abs() < 0.03, "mip {level} normal length {len}");
                rsum += p[3] as f32 / 255.0;
                count += 1.0;
            }
            let avg = rsum / count;
            assert!(avg >= prev - 1e-3, "roughness fell at mip {level}: {avg} < {prev}");
            prev = avg;
        }
    }

    /// The procedural litter must actually tile and must actually have relief.
    #[test]
    fn forest_litter_tiles_and_has_relief() {
        let size = 256u32;
        let (color, normal, height) = forest_litter_layer(size, 2.0);
        let n = (size * size) as usize;
        assert_eq!(color.len(), n);
        // Real height spread (a flat result would mean nothing splatted).
        let mean = height.iter().sum::<f32>() / n as f32;
        let sd = (height.iter().map(|h| (h - mean) * (h - mean)).sum::<f32>() / n as f32).sqrt();
        assert!(sd > 0.08, "litter height is flat (sd {sd:.3})");
        // Real normal relief.
        let tilt = normal.iter().filter(|n| n[2] < 0.985).count() as f32 / n as f32;
        assert!(tilt > 0.20, "only {:.1}% of litter texels are tilted", tilt * 100.0);
        // Seam check: the wrap-around column pair must be no more different
        // than a random interior column pair, i.e. the tile is seamless.
        let col_diff = |a: usize, b: usize| -> f32 {
            (0..size as usize)
                .map(|y| {
                    let (p, q) = (color[y * size as usize + a], color[y * size as usize + b]);
                    (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f32>()
                })
                .sum::<f32>()
                / size as f32
        };
        let seam = col_diff(size as usize - 1, 0);
        let interior = col_diff(size as usize / 2, size as usize / 2 + 1);
        assert!(
            seam < interior * 2.0 + 0.02,
            "litter tile has a seam: edge {seam:.4} vs interior {interior:.4}"
        );
    }

    /// DEV AID (permanent, per the forever-development norm): bake every
    /// material at full resolution and write what the GPU will actually
    /// receive to `debug/ground_<id>_<channel>.png`, so a change to the bake
    /// can be inspected without a GPU or a game boot.
    ///
    ///   cargo test --features native --lib dump_ground_layers -- --ignored
    ///
    /// Channels: `albedo` is the multiplier decoded back to sRGB for viewing,
    /// `height` and `roughness` are the two alpha channels as greyscale, and
    /// `normal` is the tangent map. Ignored by default because it is a ~2 s,
    /// ~100 MB job that writes files.
    #[test]
    #[ignore]
    fn dump_ground_layers_to_png() {
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("debug");
        std::fs::create_dir_all(&out).expect("debug/ must be creatable");
        let dir = find_ground_dir();
        let table = material_table();
        let inv: Vec<u8> = (0..256)
            .map(|i| {
                let l = i as f32 / 255.0;
                let s = if l <= 0.0031308 {
                    l * 12.92
                } else {
                    1.055 * l.powf(1.0 / 2.4) - 0.055
                };
                (s.clamp(0.0, 1.0) * 255.0).round() as u8
            })
            .collect();
        // Bake cost is a startup budget, so report it. Run this in RELEASE for
        // a number that means anything (debug is ~20x slower here); the real
        // load runs all materials in parallel, so wall time is roughly the
        // slowest single material, not the sum.
        let mut worst = 0.0f32;
        for def in &table.materials {
            let t0 = std::time::Instant::now();
            let b = bake_material(def, dir.as_ref(), SIZE);
            let ms = t0.elapsed().as_secs_f32() * 1000.0;
            worst = worst.max(ms);
            println!("bake {:>14}: {ms:.0} ms", def.id);
            let write = |name: &str, px: Vec<u8>, gray: bool| {
                let path = out.join(format!("ground_{}_{}.png", def.id, name));
                let fmt =
                    if gray { image::ColorType::L8 } else { image::ColorType::Rgb8 };
                image::save_buffer(&path, &px, SIZE, SIZE, fmt).expect("png write");
                println!("wrote {}", path.display());
            };
            let c = &b.color_mips[0];
            write("albedo", c.chunks_exact(4).flat_map(|p| {
                [inv[p[0] as usize], inv[p[1] as usize], inv[p[2] as usize]]
            }).collect(), false);
            write("height", c.chunks_exact(4).map(|p| p[3]).collect(), true);
            let n = &b.normal_mips[0];
            write("normal", n.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect(), false);
            write("roughness", n.chunks_exact(4).map(|p| p[3]).collect(), true);
        }
        println!("slowest single material: {worst:.0} ms (load() bakes them in parallel)");
    }

    /// A material with no files and no generator must degrade to an exact
    /// no-op layer, so a stripped install renders as if it were absent.
    #[test]
    fn missing_material_bakes_a_neutral_no_op() {
        let def = GroundMaterialDef { id: "ghost".into(), ..Default::default() };
        let b = bake_material(&def, None, 4);
        assert!(!b.real);
        for p in b.color_mips[0].chunks_exact(4) {
            assert_eq!(p[0], 128, "neutral colour must be the shader's 1.0 multiplier");
        }
        for p in b.normal_mips[0].chunks_exact(4) {
            assert_eq!([p[0], p[1], p[2]], [128, 128, 255], "neutral normal must be flat +Z");
        }
    }
}
