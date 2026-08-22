//! Precomputed tiling 3D noise for the volumetric cloud system (clouds
//! increment 3, the "photo-real puffy clouds" upgrade).
//!
//! The industry-standard real-time cloud recipe (Schneider's Nubis /
//! Horizon Zero Dawn, Haggstrom's "TileableVolumeNoise") needs two small
//! precomputed 3D textures that TILE seamlessly on every axis:
//!
//! * **SHAPE** (384^3 RGBA8): R = "Perlin-Worley" (tiling Perlin FBM
//!   remapped by an inverted-Worley FBM so the soft Perlin blobs grow
//!   cauliflower borders), G/B/A = single octaves of inverted Worley at
//!   rising frequency (assembled into an FBM in the shader). This texture
//!   carves the LOW-frequency cloud body (features tens of km).
//! * **DETAIL** (256^3 RGBA8): R/G/B = higher-frequency inverted Worley
//!   octaves that ERODE the shape's edges into wispy bases and billowy
//!   tops (features a few km). A = a RIDGED-Perlin "filament" octave (sharp
//!   ridges at the noise zero-crossings) used by the shader to fray cloud
//!   sheets into thin streaks -- the primary cirrus/wisp lever (v0.828+).
//!
//! Both are generated procedurally at startup, multithreaded over z-slabs
//! (`generate_shape` / `generate_detail`) -- no repo assets, no downloads,
//! byte-identical on every machine (pure integer-hash noise) -- then each
//! grows a variance-renormalized box-filter mip chain (`mip_chain`, phase
//! 5 band-limited sampling). The upload and bind-group wiring live in
//! `renderer::mod` (group 3, bindings 2..4); the consuming WGSL lives in
//! `40-clouds.wgsl` (`cloud_layer_volumetric` / `cloud_march_core`).
//!
//! Tiling strategy: every noise function takes a point in TILE space
//! (one tile = the unit cube = one wrap of the texture) and wraps its
//! lattice/cell coordinates modulo the integer frequency, so value(p) ==
//! value(p + 1) exactly on every axis -- the GPU's repeat sampler then
//! interpolates seamlessly across the texture edge. Unit tests lock
//! tiling, range, and determinism below.
//!
//! ## The 2x resolution pass (v0.1188)
//!
//! Both volumes doubled per axis (192 -> 384 shape, 128 -> 256 detail; 8x
//! the voxels, ~320 MiB of VRAM with mips, which is nothing of a 12 GB
//! card) and every FBM in the bake gained ONE MORE OCTAVE so the new voxels
//! carry real information instead of interpolation. The doubling is exact
//! on both sides of the ladder: every octave keeps EXACTLY the texels-per-
//! feature it had before (shape Worley 24 -> 48 stays at 8 texels/cell,
//! shape Perlin 16 -> 32 stays at 12, detail Worley 32 -> 64 stays at 4,
//! filament ridged 24 -> 48 stays at 5.33), so the bake is one octave
//! WIDER at the same per-octave sampling quality, not the same bake
//! stretched.
//!
//! Every new octave enters DC-REMOVED (`v - WORLEY_MEAN`, `v - 0.5`,
//! `v - RIDGED_MEAN`) at the amplitude the existing geometric series calls
//! for. That is what "extend the spectrum, do not retune it" means
//! numerically: the mean of every composite the shader consumes is
//! unchanged by construction and its variance rises under ~1%, so
//! CLOUD_COV_LO/HI, the tower smoothstep, the 0.481 cell-split centering,
//! the CLOUD_FIL window and the fitted carve-width table all stay
//! calibrated. `bake_bench_and_composite_stats` prints those composites;
//! it is the gate to run when anything here changes.
//!
//! Generation is ROW-BASED (see `WorleyOct` / `PerlinOct`): 8x the voxels
//! at the old per-voxel cost would have been a ~31 s startup bake, so each
//! octave now precomputes its feature-point / gradient lattice once and
//! hoists all per-cell work across the x-run that shares a cell. Measured
//! ~5x per voxel, bit-identical to the per-voxel reference functions (which
//! remain the canonical definition and are locked to the row path by
//! `row_generators_match_the_per_voxel_reference`).

/// Shape texture edge length (texels per axis). 384^3 RGBA8 = 216 MiB
/// (247 MiB with the 9-level mip chain).
pub const SHAPE_SIZE: u32 = 384;
/// Detail texture edge length. 256^3 RGBA8 = 64 MiB (73 MiB with mips).
pub const DETAIL_SIZE: u32 = 256;

// Channel seeds: arbitrary fixed constants so the volume is deterministic
// across machines and sessions (the per-planet variety comes from the
// WEATHER field's seed, not from these textures -- every planet shares the
// same noise volumes, exactly like sharing one cloud "material").
const SEED_PERLIN: u32 = 0x77CA31;
const SEED_W0: u32 = 0x51A9E3;
const SEED_W1: u32 = 0x9D2C57;
const SEED_W2: u32 = 0x2F8B11;
/// Shape's 4th Worley octave (48 cells), added with the 2x resolution.
const SEED_W3: u32 = 0x4C6E29;
const SEED_D0: u32 = 0xC3D2E1;
const SEED_D1: u32 = 0x1B4D3F;
const SEED_D2: u32 = 0x8E67A5;
/// Detail's 4th Worley octave (64 cells), added with the 2x resolution.
const SEED_D3: u32 = 0x35F1C7;
const SEED_FIL: u32 = 0x6A17B9;

/// Mean of `worley3` over the tile, measured by
/// `bake_bench_and_composite_stats` (it is the shape volume's G-channel
/// mean, the same 0.481 the WGSL hardcodes for its cell-split centering)
/// and independent of the cell count, which is why one constant serves
/// every octave. Subtracting it is what makes an added octave DC-FREE:
/// the composite's mean is then unchanged to the last bit of the bake's
/// calibration, and only its high-frequency variance grows.
pub const WORLEY_MEAN: f32 = 0.481;
/// Mean of `ridged3` over the tile (same probe). The ridged transform
/// `1 - |2v - 1|` piles values up near 1 because Perlin concentrates near
/// its zero crossing, hence a mean far above 0.5.
pub const RIDGED_MEAN: f32 = 0.808;

/// Integer avalanche hash of a 3D lattice/cell coordinate + seed.
/// (Wang/Murmur-style finalizer: good bit diffusion, no allocations.)
#[inline(always)]
fn hash3u(x: u32, y: u32, z: u32, seed: u32) -> u32 {
    let mut h = x
        .wrapping_mul(0x8DA6_B343)
        ^ y.wrapping_mul(0xD816_3841)
        ^ z.wrapping_mul(0xCB1A_B31F)
        ^ seed.wrapping_mul(0x9E37_79B9);
    h ^= h >> 13;
    h = h.wrapping_mul(0x7FEB_352D);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846C_A68B);
    h ^= h >> 16;
    h
}

/// Top 24 bits of a hash as a float in [0, 1).
#[inline(always)]
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// Euclidean modulo for cell indices (handles the -1 neighbor row).
///
/// PERFORMANCE (v0.1188, the 2x-resolution bake): this used to be the
/// textbook `(((i % n) + n) % n)`, which is TWO 64-bit integer divisions -
/// ~20-40 cycles each on x86 - and it is called 3x per lattice corner, 27
/// corners per Worley octave and 8 per Perlin octave. That is ~200 idivs
/// per generated voxel and it dominated the whole bake (measured: 2400 ms
/// for the 192^3 shape volume on 12 threads = ~16k cycles/voxel).
///
/// Every caller passes a NEIGHBOUR index derived from a canonical cell
/// index in [0, n-1] plus an offset in [-1, +1], so `i` is always within
/// [-1, n+1] - well inside [-n, 2n-1], where a single compare-and-add is
/// exactly equivalent to the double modulo. The debug assert pins that
/// contract so a future caller cannot silently walk outside it.
#[inline(always)]
fn wrap(i: i64, n: i64) -> u32 {
    debug_assert!(i >= -n && i < 2 * n, "wrap() contract: {i} outside [-n, 2n)");
    if i < 0 {
        (i + n) as u32
    } else if i >= n {
        (i - n) as u32
    } else {
        i as u32
    }
}

/// Fractional part of `x` on [0, 1) - the tile wrap every noise function
/// applies to its input.
///
/// PERFORMANCE: `f32::rem_euclid(1.0)` lowers to a `fmodf` LIBRARY CALL
/// (not inlined, ~20-50 cycles); this is one `roundsd` plus a subtract and
/// is bit-identical for every finite input the bake uses (positive
/// multiples of 1/size, and the +1.0-per-axis tiling probes). Called 3x per
/// noise octave, ~24x per voxel.
#[inline(always)]
fn frac1(x: f32) -> f32 {
    x - x.floor()
}

/// Tiling 3D Worley (cellular) noise, INVERTED: 1.0 at feature points,
/// falling to 0.0 one cell-width away. `cells` feature cells per tile on
/// each axis; the cell lattice wraps modulo `cells`, so the field has
/// period 1 in tile space. One feature point per cell, position hashed
/// from the WRAPPED cell coordinate (that wrap IS the tiling).
pub fn worley3(p: [f32; 3], cells: u32, seed: u32) -> f32 {
    let n = cells as i64;
    let c = cells as f32;
    // Wrap into the canonical tile first so p and p+k give identical cells.
    let q = [frac1(p[0]) * c, frac1(p[1]) * c, frac1(p[2]) * c];
    let cell = [
        q[0].floor() as i64,
        q[1].floor() as i64,
        q[2].floor() as i64,
    ];
    let mut min_d2 = f32::MAX;
    for dz in -1i64..=1 {
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let nx = cell[0] + dx;
                let ny = cell[1] + dy;
                let nz = cell[2] + dz;
                let h = hash3u(wrap(nx, n), wrap(ny, n), wrap(nz, n), seed);
                // Three 10-bit sub-values from one hash: the feature point's
                // offset inside its cell.
                let fx = (h & 1023) as f32 * (1.0 / 1024.0);
                let fy = ((h >> 10) & 1023) as f32 * (1.0 / 1024.0);
                let fz = ((h >> 20) & 1023) as f32 * (1.0 / 1024.0);
                // Distance measured against the UNWRAPPED neighbor index so
                // the -1/+cells rows sit geometrically adjacent.
                let ex = nx as f32 + fx - q[0];
                let ey = ny as f32 + fy - q[1];
                let ez = nz as f32 + fz - q[2];
                // ASSOCIATION IS LOAD-BEARING: the row generator hoists the
                // y/z terms out of the x-run as one pre-summed value, so it
                // can only be BIT-identical to this reference if the
                // reference also sums (ey^2 + ez^2) before adding ex^2.
                let d2 = ex * ex + (ey * ey + ez * ez);
                if d2 < min_d2 {
                    min_d2 = d2;
                }
            }
        }
    }
    (1.0 - min_d2.sqrt().min(1.0)).clamp(0.0, 1.0)
}

/// Quintic fade (Perlin's improved-noise curve).
#[inline(always)]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Gradient dot-product for one lattice corner: gradient direction hashed
/// from the WRAPPED corner (tiling), offset from the UNWRAPPED corner.
#[inline(always)]
fn grad_dot(cx: i64, cy: i64, cz: i64, n: i64, seed: u32, dx: f32, dy: f32, dz: f32) -> f32 {
    let h = hash3u(wrap(cx, n), wrap(cy, n), wrap(cz, n), seed);
    // Three signed components in [-1, 1) from one hash. Not normalized --
    // constant-magnitude gradients are not required for tiling or range,
    // and the FBM sum re-normalizes amplitude anyway.
    let gx = ((h & 1023) as f32 * (1.0 / 512.0)) - 1.0;
    let gy = (((h >> 10) & 1023) as f32 * (1.0 / 512.0)) - 1.0;
    let gz = (((h >> 20) & 1023) as f32 * (1.0 / 512.0)) - 1.0;
    // Association matches the row generator, which hoists (gy*dy + gz*dz)
    // out of the x-run - see the same note on `worley3`.
    gx * dx + (gy * dy + gz * dz)
}

/// Tiling 3D Perlin (gradient) noise with period `freq` per tile axis,
/// mapped to roughly [0, 1] (0.5 = zero crossing).
pub fn perlin3(p: [f32; 3], freq: u32, seed: u32) -> f32 {
    let n = freq as i64;
    let f = freq as f32;
    let q = [frac1(p[0]) * f, frac1(p[1]) * f, frac1(p[2]) * f];
    let i0 = [
        q[0].floor() as i64,
        q[1].floor() as i64,
        q[2].floor() as i64,
    ];
    let fr = [
        q[0] - i0[0] as f32,
        q[1] - i0[1] as f32,
        q[2] - i0[2] as f32,
    ];
    let u = [fade(fr[0]), fade(fr[1]), fade(fr[2])];
    let mut corner = [0.0f32; 8];
    for (k, c) in corner.iter_mut().enumerate() {
        let ox = (k & 1) as i64;
        let oy = ((k >> 1) & 1) as i64;
        let oz = ((k >> 2) & 1) as i64;
        *c = grad_dot(
            i0[0] + ox,
            i0[1] + oy,
            i0[2] + oz,
            n,
            seed,
            fr[0] - ox as f32,
            fr[1] - oy as f32,
            fr[2] - oz as f32,
        );
    }
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(corner[0], corner[1], u[0]);
    let x10 = lerp(corner[2], corner[3], u[0]);
    let x01 = lerp(corner[4], corner[5], u[0]);
    let x11 = lerp(corner[6], corner[7], u[0]);
    let y0 = lerp(x00, x10, u[1]);
    let y1 = lerp(x01, x11, u[1]);
    let v = lerp(y0, y1, u[2]);
    // Un-normalized gradients keep |v| well under ~1.3; the 0.62 gain
    // spreads the useful range without clipping more than the far tails.
    (0.5 + v * 0.62).clamp(0.0, 1.0)
}

/// 4-octave tiling Perlin FBM (frequency doubles, amplitude halves), in
/// roughly [0, 1]. Doubling an integer frequency preserves the tile period.
///
/// The 4th octave is the 2x-resolution EXTENSION (v0.1188) and enters
/// DC-removed: the first three keep their shipped normalized weights
/// exactly, and `(d - 0.5)` adds the next rung of the same geometric
/// series without moving the mean. Perlin's mean is 0.5 by symmetry, so
/// that subtraction is exact rather than fitted.
pub fn perlin_fbm3(p: [f32; 3], base_freq: u32, seed: u32) -> f32 {
    let a = perlin3(p, base_freq, seed);
    let b = perlin3(p, base_freq * 2, seed.wrapping_add(0x1234_5601));
    let c = perlin3(p, base_freq * 4, seed.wrapping_add(0x1234_5602));
    let d = perlin3(p, base_freq * 8, seed.wrapping_add(0x1234_5603));
    perlin_fbm_mix(a, b, c, d)
}

/// The FBM sum itself, shared by the per-voxel reference above and the row
/// generator, so the two can never drift in the assembly step.
#[inline(always)]
fn perlin_fbm_mix(a: f32, b: f32, c: f32, d: f32) -> f32 {
    (a * 0.5 + b * 0.25 + c * 0.125) / 0.875 + (d - 0.5) * (0.0625 / 0.875)
}

/// The remap everyone in cloud rendering calls `Remap`: rescales `v` from
/// [l0, h0] to [l1, h1] (no clamping -- callers clamp).
pub fn remap(v: f32, l0: f32, h0: f32, l1: f32, h1: f32) -> f32 {
    l1 + (v - l0) / (h0 - l0) * (h1 - l1)
}

/// Tiling 3D RIDGED Perlin: `1 - |2*perlin - 1|`, so the sharp ridge crest
/// (value 1) sits on the Perlin zero-crossing surfaces. Where inverted-Worley
/// gives round cellular BILLOWS, ridged Perlin gives thin, branching FILAMENT
/// structure -- exactly the streaky look of real cirrus. In [0, 1], tiling
/// preserved because `perlin3` tiles and the |.| is pointwise.
pub fn ridged3(p: [f32; 3], freq: u32, seed: u32) -> f32 {
    let v = perlin3(p, freq, seed);
    (1.0 - (2.0 * v - 1.0).abs()).clamp(0.0, 1.0)
}

/// Amplitude of the DC-removed extension octave in each composite. Each
/// one continues its own series at the natural next rung (Perlin/Worley
/// FBMs halve, so 0.125 -> 0.0625 in the shape's `lofi`; a channel that
/// the SHADER weights at 0.125 inside its own FBM needs 0.5 INSIDE the
/// channel to land at that 0.0625). The filament's 0.6/0.4 series would
/// continue at 0.267, held back to 0.2 on measurement: over 10.6 M
/// samples, P(filament > CLOUD_FIL_HI) runs 0.7579 (the shipped 2-octave
/// channel) -> 0.7522 at 0.2 (-0.75% relative, 0.8% of the channel pinned
/// at the 255 rail) -> 0.7476 at 0.267 (-1.4%, 1.4% railed). 0.2 buys the
/// octave for a drift the window cannot see.
const SHAPE_LOFI_EXT: f32 = 0.0625;
const SHAPE_A_EXT: f32 = 0.5;
const DETAIL_B_EXT: f32 = 0.5;
const FILAMENT_EXT: f32 = 0.2;

/// Three-octave ridged-Perlin FBM: the DETAIL volume's alpha "filament"
/// channel. 12 + 24 + 48 cells per tile (features a few km on Earth),
/// amplitude 0.6 / 0.4 / DC-removed 0.2. The top octave is the
/// 2x-resolution extension; at 256^3 it sits at the same 5.33 texels per
/// lattice cell the 24-octave had at 128^3, so it is resolved exactly as
/// well as the octave it extends.
pub fn filament_fbm3(p: [f32; 3], seed: u32) -> f32 {
    let a = ridged3(p, 12, seed);
    let b = ridged3(p, 24, seed.wrapping_add(0x0BAD_F00D));
    let c = ridged3(p, 48, seed.wrapping_add(0x0BAD_F00E));
    filament_mix(a, b, c)
}

#[inline(always)]
fn filament_mix(a: f32, b: f32, c: f32) -> f32 {
    (a * 0.6 + b * 0.4 + (c - RIDGED_MEAN) * FILAMENT_EXT).clamp(0.0, 1.0)
}

/// Worley octaves of the SHAPE volume: cells per tile and their seeds.
/// The 48-cell octave is the 2x-resolution extension - at 384^3 it sits at
/// 8 texels per cell, exactly where the 24-cell octave sat at 192^3.
pub const SHAPE_WORLEY: [(u32, u32); 4] =
    [(6, SEED_W0), (12, SEED_W1), (24, SEED_W2), (48, SEED_W3)];
/// Worley octaves of the DETAIL volume (the 64-cell one is the extension;
/// 4 texels/cell at 256^3, matching the 32-cell octave at 128^3).
pub const DETAIL_WORLEY: [(u32, u32); 4] =
    [(8, SEED_D0), (16, SEED_D1), (32, SEED_D2), (64, SEED_D3)];
/// Base frequency of the shape volume's Perlin FBM (octaves 4/8/16/32).
pub const SHAPE_PERLIN_BASE: u32 = 4;
/// Ridged-Perlin octaves of the filament channel and their seeds.
pub const FILAMENT_OCTAVES: [(u32, u32); 3] = [
    (12, SEED_FIL),
    (24, SEED_FIL.wrapping_add(0x0BAD_F00D)),
    (48, SEED_FIL.wrapping_add(0x0BAD_F00E)),
];

/// One SHAPE voxel at tile-space point `p`: R = Perlin-Worley, G/B/A =
/// inverted Worley at 6/12/24 cells per tile (A carrying the DC-removed
/// 48-cell extension). Public so tests can probe arbitrary points without
/// generating a whole volume; the row generator must match it bit for bit.
pub fn shape_voxel(p: [f32; 3]) -> [u8; 4] {
    let w = [
        worley3(p, SHAPE_WORLEY[0].0, SHAPE_WORLEY[0].1),
        worley3(p, SHAPE_WORLEY[1].0, SHAPE_WORLEY[1].1),
        worley3(p, SHAPE_WORLEY[2].0, SHAPE_WORLEY[2].1),
        worley3(p, SHAPE_WORLEY[3].0, SHAPE_WORLEY[3].1),
    ];
    let per = perlin_fbm3(p, SHAPE_PERLIN_BASE, SEED_PERLIN);
    shape_bytes(w, per)
}

/// SHAPE voxel assembly from already-evaluated octaves. Shared by the
/// per-voxel reference above and the row generator below.
#[inline(always)]
fn shape_bytes(w: [f32; 4], per: f32) -> [u8; 4] {
    // Perlin-Worley: dilate the Perlin body by the Worley FBM so the
    // cloud MASS sits at the Worley feature points (cells) and the gaps
    // between cells eat bays into it.
    //
    // POLARITY FIX (environment program increment 3, 2026-08-21). The
    // old remap `remap(per, lofi - 1, 1, 0, 1)` evaluates to per at
    // features and (per+1)/2 in the GAPS - i.e. it BOOSTED the gaps, so
    // the body was a connected foam web with holes where the puffs
    // should be. Measured by the polarity probe below: mean body 150 at
    // features vs 180 in gaps. Inverted-Worley clouds could never read
    // as discrete cumulus with that topology, no matter the tuning.
    // The corrected remap boosts the FEATURES ((per+lofi)/(1+lofi):
    // (per+1)/2 at a cell centre, plain per in a gap), which is the
    // dilation the original comment always claimed.
    //
    // 2x-RESOLUTION EXTENSION (v0.1188): the 48-cell octave joins `lofi`
    // DC-removed at the series' next rung, so the dilation now has one
    // more level of cauliflower fine structure while E[lofi] - and with it
    // the whole R histogram the coverage window is calibrated against -
    // is unchanged. (The SHADER's own `lofi`, rebuilt from G/B/A for the
    // tower smoothstep, deliberately does NOT see this term: towering is
    // meant to key on low-frequency convective support.)
    let lofi = w[0] * 0.625 + w[1] * 0.25 + w[2] * 0.125
        + (w[3] - WORLEY_MEAN) * SHAPE_LOFI_EXT;
    // POLARITY FLIPPED (increment 10b, the scheduled fix): the corrected
    // remap boosts the FEATURES - (per + lofi)/(1 + lofi) evaluates to
    // (per + 1)/2 at a cell centre and plain per in a gap - so the cloud
    // MASS finally sits at the Worley cells (discrete billowy puffs) and
    // the gaps eat clear lanes between them. The old `lofi - 1.0` form
    // did the opposite (boosted gaps): a connected foam web with holes,
    // which is why every cloud read as translucent round blobs and no
    // downstream tuning could make cumulus. Landed WITH the coverage
    // window re-center (CLOUD_COV_LO/HI re-derived at the same body
    // percentiles the old thresholds cut) per the increment-3 bisect:
    // flipping alone empties the sky.
    let pw = remap(per, -lofi, 1.0, 0.0, 1.0).clamp(0.0, 1.0);
    // A carries the 24-cell octave PLUS the DC-removed 48-cell extension.
    // The shader weights this channel at 0.125 inside its G/B/A FBM, so
    // half-amplitude here is the series' 0.0625 rung there. G is left
    // untouched on purpose: the WGSL cell-split centers on its baked mean
    // (the hardcoded 0.481), and B likewise feeds the tower term.
    let ext_a = (w[2] + (w[3] - WORLEY_MEAN) * SHAPE_A_EXT).clamp(0.0, 1.0);
    [
        (pw * 255.0).round() as u8,
        (w[0] * 255.0).round() as u8,
        (w[1] * 255.0).round() as u8,
        (ext_a * 255.0).round() as u8,
    ]
}

/// One DETAIL voxel: R/G/B = inverted Worley at 8/16/32 cells per tile
/// (the shader assembles them as a 0.625/0.25/0.125 FBM; B carries the
/// DC-removed 64-cell extension), A = a ridged-Perlin filament FBM (the
/// shader uses it to fray cloud sheets into cirrus streaks).
pub fn detail_voxel(p: [f32; 3]) -> [u8; 4] {
    let w = [
        worley3(p, DETAIL_WORLEY[0].0, DETAIL_WORLEY[0].1),
        worley3(p, DETAIL_WORLEY[1].0, DETAIL_WORLEY[1].1),
        worley3(p, DETAIL_WORLEY[2].0, DETAIL_WORLEY[2].1),
        worley3(p, DETAIL_WORLEY[3].0, DETAIL_WORLEY[3].1),
    ];
    let fil = filament_fbm3(p, SEED_FIL);
    detail_bytes(w, fil)
}

/// DETAIL voxel assembly from already-evaluated octaves (shared with the
/// row generator, same contract as `shape_bytes`).
#[inline(always)]
fn detail_bytes(w: [f32; 4], fil: f32) -> [u8; 4] {
    // B carries the 32-cell octave plus the DC-removed 64-cell extension:
    // the shader weights B at 0.125 inside `dfbm`, so half-amplitude here
    // continues that FBM at 0.0625. This is the band that actually shows
    // up as cauliflower lobes - at the puff tap's 45.9 km tile the new
    // octave resolves ~0.7 km cavities, which is the low end of the
    // 100-700 m lobe scale real cumulus reads at.
    let ext_b = (w[2] + (w[3] - WORLEY_MEAN) * DETAIL_B_EXT).clamp(0.0, 1.0);
    [
        (w[0] * 255.0).round() as u8,
        (w[1] * 255.0).round() as u8,
        (ext_b * 255.0).round() as u8,
        (fil * 255.0).round() as u8,
    ]
}

// ─────────────────────────────────────────────────────────────────────────
// Row generators (v0.1188): the same noise, evaluated a whole x-row at a
// time.
//
// The per-voxel functions above are the CANONICAL definition and stay the
// reference every test compares against, but they cannot bake 8x the
// voxels in a sane startup budget: measured 170 ns per `worley3` call and
// 91 ns per `perlin3` call, which at 384^3 x 4 octaves is a ~31 s bake.
//
// Two structural facts make that ~5x cheaper without changing a single
// output bit:
//
//  1. The feature point / gradient of a lattice cell is a pure function of
//     the cell index, so it can be tabulated ONCE per octave instead of
//     re-hashed 27 (or 8) times per voxel. The tables are small - the
//     largest, detail's 64-cell octave, is 3.1 MiB - and shared read-only
//     across every worker thread.
//  2. Consecutive texels along x share the same cell for size/cells texels
//     (8 to 64 of them here), and inside that run everything except the x
//     term is constant: the whole (ey^2 + ez^2) sum for Worley, the whole
//     (gy*dy + gz*dz) dot for Perlin. Hoisting those out of the run leaves
//     an inner loop of a subtract, a multiply-add and a min, which the
//     compiler vectorizes.
//
// Bit-identity with the per-voxel path is a TEST, not a hope:
// `row_generators_match_the_per_voxel_reference`.
// ─────────────────────────────────────────────────────────────────────────

/// One inverted-Worley octave with its per-cell feature points tabulated.
struct WorleyOct {
    cells: u32,
    /// Feature-point offset inside its own cell, x-fastest, cells^3 long.
    pts: Vec<[f32; 3]>,
}

impl WorleyOct {
    fn new(cells: u32, seed: u32) -> Self {
        let n = cells as usize;
        let mut pts = Vec::with_capacity(n * n * n);
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let h = hash3u(x as u32, y as u32, z as u32, seed);
                    pts.push([
                        (h & 1023) as f32 * (1.0 / 1024.0),
                        ((h >> 10) & 1023) as f32 * (1.0 / 1024.0),
                        ((h >> 20) & 1023) as f32 * (1.0 / 1024.0),
                    ]);
                }
            }
        }
        Self { cells, pts }
    }

    /// `out[x] = worley3([x/size, y/size, z/size], cells, seed)` for the
    /// whole row, bit for bit.
    fn row(&self, size: u32, y: u32, z: u32, out: &mut [f32]) {
        let n = self.cells as i64;
        let nu = self.cells as usize;
        let c = self.cells as f32;
        let inv = 1.0 / size as f32;
        let qy = frac1(y as f32 * inv) * c;
        let qz = frac1(z as f32 * inv) * c;
        let cy = qy.floor() as i64;
        let cz = qz.floor() as i64;
        // 27 real candidates padded to 32 so the min reduction unrolls
        // cleanly; the sentinels can never win.
        let mut ax = [f32::MAX; 32];
        let mut syz = [f32::MAX; 32];
        let mut cur_cx = i64::MIN;
        for (x, o) in out.iter_mut().enumerate() {
            let qx = frac1(x as f32 * inv) * c;
            let cx = qx.floor() as i64;
            if cx != cur_cx {
                // New cell column: re-gather the 3x3x3 neighbourhood and
                // pre-sum everything that does not depend on x.
                cur_cx = cx;
                let mut k = 0usize;
                for dz in -1i64..=1 {
                    let nz = cz + dz;
                    let wz = wrap(nz, n) as usize;
                    let nzf = nz as f32;
                    for dy in -1i64..=1 {
                        let ny = cy + dy;
                        let wy = wrap(ny, n) as usize;
                        let nyf = ny as f32;
                        let base = (wz * nu + wy) * nu;
                        for dx in -1i64..=1 {
                            let nx = cx + dx;
                            let wx = wrap(nx, n) as usize;
                            let f = self.pts[base + wx];
                            let ey = nyf + f[1] - qy;
                            let ez = nzf + f[2] - qz;
                            ax[k] = nx as f32 + f[0];
                            syz[k] = ey * ey + ez * ez;
                            k += 1;
                        }
                    }
                }
            }
            // Four independent partial minima so the reduction has no
            // serial dependency chain.
            let mut m = [f32::MAX; 4];
            for j in 0..8 {
                for (l, ml) in m.iter_mut().enumerate() {
                    let k = j * 4 + l;
                    let ex = ax[k] - qx;
                    let d2 = ex * ex + syz[k];
                    if d2 < *ml {
                        *ml = d2;
                    }
                }
            }
            let min_d2 = m[0].min(m[1]).min(m[2].min(m[3]));
            *o = (1.0 - min_d2.sqrt().min(1.0)).clamp(0.0, 1.0);
        }
    }
}

/// One tiling-Perlin octave with its per-corner gradients tabulated.
struct PerlinOct {
    freq: u32,
    grads: Vec<[f32; 3]>,
}

impl PerlinOct {
    fn new(freq: u32, seed: u32) -> Self {
        let n = freq as usize;
        let mut grads = Vec::with_capacity(n * n * n);
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let h = hash3u(x as u32, y as u32, z as u32, seed);
                    grads.push([
                        ((h & 1023) as f32 * (1.0 / 512.0)) - 1.0,
                        (((h >> 10) & 1023) as f32 * (1.0 / 512.0)) - 1.0,
                        (((h >> 20) & 1023) as f32 * (1.0 / 512.0)) - 1.0,
                    ]);
                }
            }
        }
        Self { freq, grads }
    }

    /// `out[x] = perlin3([x/size, y/size, z/size], freq, seed)`, bit exact.
    fn row(&self, size: u32, y: u32, z: u32, out: &mut [f32]) {
        let n = self.freq as i64;
        let nu = self.freq as usize;
        let f = self.freq as f32;
        let inv = 1.0 / size as f32;
        let qy = frac1(y as f32 * inv) * f;
        let qz = frac1(z as f32 * inv) * f;
        let iy = qy.floor() as i64;
        let iz = qz.floor() as i64;
        let fy = qy - iy as f32;
        let fz = qz - iz as f32;
        let uy = fade(fy);
        let uz = fade(fz);
        // Per-corner x gradient and the pre-summed y/z half of its dot.
        let mut gx = [0.0f32; 8];
        let mut base = [0.0f32; 8];
        let mut cur_ix = i64::MIN;
        for (x, o) in out.iter_mut().enumerate() {
            let qx = frac1(x as f32 * inv) * f;
            let ix = qx.floor() as i64;
            let fx = qx - ix as f32;
            if ix != cur_ix {
                cur_ix = ix;
                for k in 0..8usize {
                    let ox = (k & 1) as i64;
                    let oy = ((k >> 1) & 1) as i64;
                    let oz = ((k >> 2) & 1) as i64;
                    let wx = wrap(ix + ox, n) as usize;
                    let wy = wrap(iy + oy, n) as usize;
                    let wz = wrap(iz + oz, n) as usize;
                    let g = self.grads[(wz * nu + wy) * nu + wx];
                    gx[k] = g[0];
                    base[k] = g[1] * (fy - oy as f32) + g[2] * (fz - oz as f32);
                }
            }
            let ux = fade(fx);
            let mut corner = [0.0f32; 8];
            for (k, cn) in corner.iter_mut().enumerate() {
                *cn = gx[k] * (fx - (k & 1) as f32) + base[k];
            }
            let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
            let x00 = lerp(corner[0], corner[1], ux);
            let x10 = lerp(corner[2], corner[3], ux);
            let x01 = lerp(corner[4], corner[5], ux);
            let x11 = lerp(corner[6], corner[7], ux);
            let y0 = lerp(x00, x10, uy);
            let y1 = lerp(x01, x11, uy);
            let v = lerp(y0, y1, uz);
            *o = (0.5 + v * 0.62).clamp(0.0, 1.0);
        }
    }
}

/// Fill an RGBA8 volume of edge `size`, multithreaded over z-slabs, one
/// x-row at a time. `row` writes `size` RGBA texels for the given (y, z);
/// each worker thread gets its own scratch from `scratch` so the row
/// buffers are never shared.
fn generate_volume<S, MkScratch, Row>(
    size: u32,
    threads: usize,
    scratch: MkScratch,
    row: Row,
) -> Vec<u8>
where
    MkScratch: Fn() -> S + Sync,
    Row: Fn(&mut S, u32, u32, &mut [u8]) + Sync,
{
    let n = size as usize;
    let mut buf = vec![0u8; n * n * n * 4];
    // Slab granularity is whole z-planes, so every chunk is a whole number
    // of rows and the (y, z) of a chunk-local row is pure arithmetic.
    let slab_planes = n.div_ceil(threads.max(1));
    let slab_bytes = slab_planes * n * n * 4;
    let scratch = &scratch;
    let row = &row;
    std::thread::scope(|s| {
        for (slab_i, chunk) in buf.chunks_mut(slab_bytes).enumerate() {
            s.spawn(move || {
                let z0 = slab_i * slab_planes;
                let mut sc = scratch();
                for (r, row_bytes) in chunk.chunks_mut(n * 4).enumerate() {
                    let z = z0 + r / n;
                    let y = r % n;
                    row(&mut sc, y as u32, z as u32, row_bytes);
                }
            });
        }
    });
    buf
}

/// Per-thread scratch for the shape bake: one f32 row per octave.
struct ShapeScratch {
    w: [Vec<f32>; 4],
    p: [Vec<f32>; 4],
}

/// Per-thread scratch for the detail bake.
struct DetailScratch {
    w: [Vec<f32>; 4],
    r: [Vec<f32>; 3],
}

/// Generate the 384^3 SHAPE volume (RGBA8, tightly packed, x fastest).
/// Byte-for-byte equal to evaluating `shape_voxel` at every texel.
pub fn generate_shape(threads: usize) -> Vec<u8> {
    let size = SHAPE_SIZE;
    let n = size as usize;
    let w: [WorleyOct; 4] = std::array::from_fn(|i| {
        let (cells, seed) = SHAPE_WORLEY[i];
        WorleyOct::new(cells, seed)
    });
    // Perlin octaves 4/8/16/32 with the seed ladder `perlin_fbm3` uses.
    let p: [PerlinOct; 4] = std::array::from_fn(|i| {
        let seed = match i {
            0 => SEED_PERLIN,
            k => SEED_PERLIN.wrapping_add(0x1234_5600 + k as u32),
        };
        PerlinOct::new(SHAPE_PERLIN_BASE << i, seed)
    });
    generate_volume(
        size,
        threads,
        || ShapeScratch {
            w: std::array::from_fn(|_| vec![0.0f32; n]),
            p: std::array::from_fn(|_| vec![0.0f32; n]),
        },
        |sc, y, z, out| {
            for i in 0..4 {
                w[i].row(size, y, z, &mut sc.w[i]);
                p[i].row(size, y, z, &mut sc.p[i]);
            }
            for (x, texel) in out.chunks_exact_mut(4).enumerate() {
                let wv = [sc.w[0][x], sc.w[1][x], sc.w[2][x], sc.w[3][x]];
                let per = perlin_fbm_mix(sc.p[0][x], sc.p[1][x], sc.p[2][x], sc.p[3][x]);
                texel.copy_from_slice(&shape_bytes(wv, per));
            }
        },
    )
}

/// Generate the 256^3 DETAIL volume (RGBA8, tightly packed, x fastest).
/// Byte-for-byte equal to evaluating `detail_voxel` at every texel.
pub fn generate_detail(threads: usize) -> Vec<u8> {
    let size = DETAIL_SIZE;
    let n = size as usize;
    let w: [WorleyOct; 4] = std::array::from_fn(|i| {
        let (cells, seed) = DETAIL_WORLEY[i];
        WorleyOct::new(cells, seed)
    });
    let r: [PerlinOct; 3] = std::array::from_fn(|i| {
        let (freq, seed) = FILAMENT_OCTAVES[i];
        PerlinOct::new(freq, seed)
    });
    generate_volume(
        size,
        threads,
        || DetailScratch {
            w: std::array::from_fn(|_| vec![0.0f32; n]),
            r: std::array::from_fn(|_| vec![0.0f32; n]),
        },
        |sc, y, z, out| {
            for i in 0..4 {
                w[i].row(size, y, z, &mut sc.w[i]);
            }
            for i in 0..3 {
                r[i].row(size, y, z, &mut sc.r[i]);
            }
            for (x, texel) in out.chunks_exact_mut(4).enumerate() {
                let wv = [sc.w[0][x], sc.w[1][x], sc.w[2][x], sc.w[3][x]];
                // The ridged transform is pointwise, so it rides on top of
                // the plain Perlin rows (same arithmetic as `ridged3`).
                let ridged = |v: f32| (1.0 - (2.0 * v - 1.0).abs()).clamp(0.0, 1.0);
                let fil = filament_mix(
                    ridged(sc.r[0][x]),
                    ridged(sc.r[1][x]),
                    ridged(sc.r[2][x]),
                );
                texel.copy_from_slice(&detail_bytes(wv, fil));
            }
        },
    )
}

/// One box-filter downsample step: each destination voxel averages the
/// (up to) 2x2x2 source voxels under it, per RGBA channel. Handles odd
/// source edges (384 -> 192 -> ... -> 3 -> 1 uses wgpu's floor-halving
/// mip sizes) by clamping the source coordinate, which weights the edge
/// voxel double exactly the way the GPU's own mip convention does.
///
/// A box filter is the correct band-limiter for DENSITY-LIKE data: the
/// mip's value is the mean density over the footprint, which is what a
/// raymarch step covering that footprint physically integrates.
fn downsample_volume(src: &[u8], src_size: u32) -> Vec<u8> {
    let s = src_size as usize;
    let d = ((src_size / 2).max(1)) as usize;
    let mut dst = vec![0u8; d * d * d * 4];
    // Threaded over destination z-slabs (v0.1188): at 384^3 the first step
    // alone reads 226 MiB, and the whole chain sat on the boot path.
    // Slab-splitting is exact here - every destination voxel reads only
    // source voxels inside its own 2-plane band - so the output is
    // identical to the serial walk regardless of thread count.
    let threads = mip_threads().min(d.max(1));
    let planes = d.div_ceil(threads);
    std::thread::scope(|sc| {
        for (slab, chunk) in dst.chunks_mut(planes * d * d * 4).enumerate() {
            let src = &src;
            sc.spawn(move || {
                let z0 = slab * planes;
                for (zi, plane) in chunk.chunks_mut(d * d * 4).enumerate() {
                    let z = z0 + zi;
                    for y in 0..d {
                        for x in 0..d {
                            let mut sum = [0u32; 4];
                            for dz in 0..2 {
                                for dy in 0..2 {
                                    for dx in 0..2 {
                                        let sx = (x * 2 + dx).min(s - 1);
                                        let sy = (y * 2 + dy).min(s - 1);
                                        let sz = (z * 2 + dz).min(s - 1);
                                        let si = ((sz * s + sy) * s + sx) * 4;
                                        for c in 0..4 {
                                            sum[c] += src[si + c] as u32;
                                        }
                                    }
                                }
                            }
                            let di = (y * d + x) * 4;
                            for c in 0..4 {
                                plane[di + c] = ((sum[c] + 4) / 8) as u8;
                            }
                        }
                    }
                }
            });
        }
    });
    dst
}

/// Worker count for the mip passes. Kept local (rather than threaded
/// through `mip_chain`'s signature) so the call sites in `renderer::mod`
/// and the carve-fit test stay unchanged.
fn mip_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

/// Largest single `write_texture` upload, in bytes.
///
/// The 384^3 base level is 216 MiB in one piece. wgpu's default
/// `max_buffer_size` is 256 MiB, so a single-shot upload technically fits -
/// with 40 MiB of headroom and a 216 MiB driver staging allocation on the
/// boot path. Slicing it into z-slabs removes both concerns and is what
/// makes the NEXT resolution step (768^3 = 1.7 GiB, well past any single
/// buffer) a size-constant change instead of another rewrite.
pub const UPLOAD_SLAB_BYTES: usize = 64 << 20;

/// Split one cubic mip level into z-slab uploads of at most
/// `UPLOAD_SLAB_BYTES` each: `(z0, depth, byte_start, byte_end)`.
///
/// Pure index arithmetic, deliberately separated from the wgpu call so it
/// can be tested without a GPU (`upload_slabs_tile_the_volume_exactly`) -
/// a mis-sliced upload would silently corrupt the volume in a way only a
/// rendered frame could reveal.
pub fn upload_slabs(size: u32, cap_bytes: usize) -> Vec<(u32, u32, usize, usize)> {
    let n = size as usize;
    let slice_bytes = n * n * 4;
    // At least one z-slice per upload even if a single slice exceeds the
    // cap (it cannot at any size we ship, but the arithmetic must not
    // produce a zero-depth copy).
    let depth = (cap_bytes / slice_bytes.max(1)).clamp(1, n);
    let mut out = Vec::with_capacity(n.div_ceil(depth));
    let mut z0 = 0usize;
    while z0 < n {
        let d = depth.min(n - z0);
        out.push((
            z0 as u32,
            d as u32,
            z0 * slice_bytes,
            (z0 + d) * slice_bytes,
        ));
        z0 += d;
    }
    out
}

/// Per-channel mean and standard deviation of an RGBA8 volume, in byte
/// units. Drives the variance renormalization below.
///
/// Threaded the same way as the downsample: partial sums per slab, then
/// one exact combine. Summation order changes with the thread count, so
/// the last f64 bits can differ from a serial walk - irrelevant here (the
/// result feeds a gain that is rounded to a byte), and the mean/sigma of
/// 56 M samples is stable far beyond the byte quantization.
fn channel_stats(v: &[u8]) -> ([f64; 4], [f64; 4]) {
    let n = (v.len() / 4) as f64;
    let threads = mip_threads();
    let chunk_px = (v.len() / 4).div_ceil(threads).max(1);
    let chunk_bytes = chunk_px * 4;
    let sums: Vec<[f64; 4]> = std::thread::scope(|sc| {
        let handles: Vec<_> = v
            .chunks(chunk_bytes)
            .map(|c| {
                sc.spawn(move || {
                    let mut acc = [0.0f64; 4];
                    for px in c.chunks_exact(4) {
                        for k in 0..4 {
                            acc[k] += px[k] as f64;
                        }
                    }
                    acc
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("stats thread")).collect()
    });
    let mut mean = [0.0f64; 4];
    for s in &sums {
        for c in 0..4 {
            mean[c] += s[c];
        }
    }
    for m in mean.iter_mut() {
        *m /= n;
    }
    let vars: Vec<[f64; 4]> = std::thread::scope(|sc| {
        let handles: Vec<_> = v
            .chunks(chunk_bytes)
            .map(|c| {
                let mean = mean;
                sc.spawn(move || {
                    let mut acc = [0.0f64; 4];
                    for px in c.chunks_exact(4) {
                        for k in 0..4 {
                            let d = px[k] as f64 - mean[k];
                            acc[k] += d * d;
                        }
                    }
                    acc
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("stats thread")).collect()
    });
    let mut var = [0.0f64; 4];
    for s in &vars {
        for c in 0..4 {
            var[c] += s[c];
        }
    }
    let sigma = [
        (var[0] / n).sqrt(),
        (var[1] / n).sqrt(),
        (var[2] / n).sqrt(),
        (var[3] / n).sqrt(),
    ];
    (mean, sigma)
}

/// Variance-matched renormalization of a mip level against the BASE
/// level's per-channel statistics: v' = mean0 + (v - mean_l) * gain with
/// gain = min(sigma0 / sigma_l, 2.0). Box filtering preserves the mean
/// but collapses the variance level by level, and the shader's carve
/// thresholds (0.52-0.92 of the noise range) turn that variance loss
/// into COVERAGE loss - distant clouds would thin out and vanish as
/// they crossed into higher mips. Re-widening each level's histogram to
/// the base histogram keeps threshold-crossing statistics stable across
/// the chain while the surviving content stays genuinely band-limited
/// (the restored contrast lives at the level's own low frequencies).
/// The 2.0 gain cap keeps the deep, nearly-flat levels from amplifying
/// quantization noise.
/// In place (v0.1188): the level is up to 28 MiB and the old version
/// allocated a second copy of it.
fn renormalize_level(v: &mut [u8], stats0: &([f64; 4], [f64; 4])) {
    let (mean_l, sigma_l) = channel_stats(v);
    let (mean0, sigma0) = stats0;
    let mut gain = [0.0f64; 4];
    for c in 0..4 {
        gain[c] = if sigma_l[c] > 1.0e-6 {
            (sigma0[c] / sigma_l[c]).min(2.0)
        } else {
            0.0
        };
    }
    let threads = mip_threads();
    let chunk_bytes = (v.len() / 4).div_ceil(threads).max(1) * 4;
    std::thread::scope(|sc| {
        for chunk in v.chunks_mut(chunk_bytes) {
            let (mean0, mean_l, gain) = (*mean0, mean_l, gain);
            sc.spawn(move || {
                for px in chunk.chunks_exact_mut(4) {
                    for c in 0..4 {
                        let x = mean0[c] + (px[c] as f64 - mean_l[c]) * gain[c];
                        px[c] = x.round().clamp(0.0, 255.0) as u8;
                    }
                }
            });
        }
    });
}

/// Full mip chain for a cubic RGBA8 volume, INCLUDING the base level:
/// element 0 is `base` unchanged; each following element halves the edge
/// (floor, min 1) down to 1^3, box-filtered from the RAW previous level
/// and then variance-renormalized against the base level's statistics
/// (see `renormalize_level` - filtering happens on the raw ladder so
/// renormalization gains never compound). Matches wgpu's floor-halving
/// mip sizing so the chain uploads 1:1 into a texture created with
/// `mip_level_count = chain.len()`.
pub fn mip_chain(base: Vec<u8>, base_size: u32) -> Vec<Vec<u8>> {
    let stats0 = channel_stats(&base);
    // Pass 1: the raw box-filtered ladder (each level from the RAW parent).
    let mut chain: Vec<Vec<u8>> = vec![base];
    let mut size = base_size;
    while size > 1 {
        let next = downsample_volume(chain.last().expect("non-empty"), size);
        size = (size / 2).max(1);
        chain.push(next);
    }
    // Pass 2: renormalize every level except the untouched base.
    for lvl in chain.iter_mut().skip(1) {
        renormalize_level(lvl, &stats0);
    }
    chain
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exactly-representable probe points (multiples of 1/64) so the
    /// +1.0-per-axis tiling checks are float-exact, plus the tile corner.
    fn probes() -> Vec<[f32; 3]> {
        let mut v = vec![[0.0, 0.0, 0.0]];
        for i in 0..24 {
            let a = ((i * 7 + 3) % 64) as f32 / 64.0;
            let b = ((i * 13 + 11) % 64) as f32 / 64.0;
            let c = ((i * 29 + 17) % 64) as f32 / 64.0;
            v.push([a, b, c]);
        }
        v
    }

    /// Small reference volume built the slow, obvious way (one call to the
    /// per-voxel function per texel). The real bake uses the row
    /// generators; this is what they are checked against.
    fn ref_volume(size: u32, voxel: fn([f32; 3]) -> [u8; 4]) -> Vec<u8> {
        let n = size as usize;
        let inv = 1.0 / size as f32;
        let mut out = vec![0u8; n * n * n * 4];
        for z in 0..n {
            for y in 0..n {
                for x in 0..n {
                    let p = [x as f32 * inv, y as f32 * inv, z as f32 * inv];
                    let i = ((z * n + y) * n + x) * 4;
                    out[i..i + 4].copy_from_slice(&voxel(p));
                }
            }
        }
        out
    }

    #[test]
    fn worley_tiles_exactly_on_every_axis() {
        for p in probes() {
            for cells in [6u32, 8, 12, 16, 24, 32, 48, 64] {
                let base = worley3(p, cells, 42);
                for axis in 0..3 {
                    let mut q = p;
                    q[axis] += 1.0; // exact for multiples of 1/64
                    assert_eq!(
                        worley3(q, cells, 42),
                        base,
                        "worley3 breaks tiling at {p:?} axis {axis} cells {cells}"
                    );
                }
            }
        }
    }

    #[test]
    fn perlin_tiles_exactly_on_every_axis() {
        for p in probes() {
            for freq in [4u32, 8, 16, 32, 48] {
                let base = perlin3(p, freq, 7);
                for axis in 0..3 {
                    let mut q = p;
                    q[axis] += 1.0;
                    assert_eq!(
                        perlin3(q, freq, 7),
                        base,
                        "perlin3 breaks tiling at {p:?} axis {axis} freq {freq}"
                    );
                }
            }
        }
    }

    #[test]
    fn noise_stays_in_unit_range_and_actually_varies() {
        let mut w_lo = f32::MAX;
        let mut w_hi = f32::MIN;
        let mut p_lo = f32::MAX;
        let mut p_hi = f32::MIN;
        for i in 0..1000 {
            let p = [
                (i as f32 * 0.317) % 1.0,
                (i as f32 * 0.731) % 1.0,
                (i as f32 * 0.173) % 1.0,
            ];
            let w = worley3(p, 8, 3);
            let pe = perlin3(p, 4, 3);
            assert!((0.0..=1.0).contains(&w), "worley out of range: {w}");
            assert!((0.0..=1.0).contains(&pe), "perlin out of range: {pe}");
            w_lo = w_lo.min(w);
            w_hi = w_hi.max(w);
            p_lo = p_lo.min(pe);
            p_hi = p_hi.max(pe);
        }
        assert!(w_hi - w_lo > 0.5, "worley too flat: {w_lo}..{w_hi}");
        assert!(p_hi - p_lo > 0.3, "perlin too flat: {p_lo}..{p_hi}");
    }

    #[test]
    fn voxels_are_deterministic() {
        for p in probes() {
            assert_eq!(shape_voxel(p), shape_voxel(p));
            assert_eq!(detail_voxel(p), detail_voxel(p));
        }
        // A couple of pinned values so an accidental hash/seed change is
        // caught as a diff, not just a "still deterministic within run".
        let a = shape_voxel([0.25, 0.5, 0.75]);
        let b = shape_voxel([0.25, 0.5, 0.75]);
        assert_eq!(a, b);
    }

    #[test]
    fn upload_slabs_tile_the_volume_exactly() {
        // The GPU cannot check this for us: a slab list that overlaps,
        // gaps, or mis-maps z would upload a scrambled volume that only a
        // rendered frame would show. So it is checked here, exhaustively.
        for size in [1u32, 2, 3, 16, 256, 384] {
            let n = size as usize;
            let slice = n * n * 4;
            for cap in [1usize, 4096, 1 << 20, UPLOAD_SLAB_BYTES, usize::MAX / 2] {
                let slabs = upload_slabs(size, cap);
                assert!(!slabs.is_empty(), "no slabs for size {size} cap {cap}");
                let mut next_z = 0u32;
                let mut next_b = 0usize;
                for (z0, d, b0, b1) in &slabs {
                    assert_eq!(*z0, next_z, "slab z gap/overlap at size {size}");
                    assert_eq!(*b0, next_b, "slab byte gap/overlap at size {size}");
                    assert!(*d >= 1, "zero-depth slab at size {size}");
                    // The byte range must be exactly the z range's bytes.
                    assert_eq!(*b0, *z0 as usize * slice);
                    assert_eq!(*b1, (*z0 + *d) as usize * slice);
                    // Cap honoured whenever a single z-slice fits in it.
                    if cap >= slice {
                        assert!(b1 - b0 <= cap, "slab over cap at size {size} cap {cap}");
                    }
                    next_z = z0 + d;
                    next_b = *b1;
                }
                assert_eq!(next_z, size, "slabs do not cover the volume depth");
                assert_eq!(next_b, n * slice, "slabs do not cover the volume bytes");
            }
        }
        // At the shipped shape size the base level really does get split
        // (this is the whole point - one 216 MiB copy would otherwise sit
        // 40 MiB under wgpu's default max_buffer_size).
        assert!(
            upload_slabs(SHAPE_SIZE, UPLOAD_SLAB_BYTES).len() > 1,
            "the 384^3 base level must upload in slabs"
        );
    }

    #[test]
    fn row_generators_match_the_per_voxel_reference() {
        // THE BIT-IDENTITY GATE for the row-based bake (v0.1188). The
        // per-voxel `shape_voxel` / `detail_voxel` remain the canonical
        // definition of the field (the CPU reference march in
        // cloud_reference samples the same bytes); the row generators are
        // purely an evaluation-order optimization and must reproduce them
        // EXACTLY - not "within a byte", exactly - or the shipped volume
        // silently stops being the field every calibration was fitted to.
        //
        // Run at a small edge so the test is cheap, but through the real
        // `generate_shape`/`generate_detail` machinery would mean baking
        // 384^3; instead drive the same octave objects at edge 24 and
        // compare against the reference per-voxel functions sampled on the
        // same lattice. (Sizes are not baked into the row math - only the
        // 1/size texel step is - so edge 24 exercises the identical code.)
        let size = 24u32;
        let n = size as usize;
        let inv = 1.0 / size as f32;
        let mut row = vec![0.0f32; n];

        for (cells, seed) in SHAPE_WORLEY.iter().chain(DETAIL_WORLEY.iter()) {
            let oct = WorleyOct::new(*cells, *seed);
            for z in [0u32, 1, 11, 23] {
                for y in [0u32, 5, 23] {
                    oct.row(size, y, z, &mut row);
                    for x in 0..n {
                        let p = [x as f32 * inv, y as f32 * inv, z as f32 * inv];
                        assert_eq!(
                            row[x],
                            worley3(p, *cells, *seed),
                            "worley row != reference at {x},{y},{z} cells {cells}"
                        );
                    }
                }
            }
        }
        for freq in [4u32, 8, 16, 32, 48] {
            let oct = PerlinOct::new(freq, 0x77CA31);
            for z in [0u32, 3, 23] {
                for y in [0u32, 7, 22] {
                    oct.row(size, y, z, &mut row);
                    for x in 0..n {
                        let p = [x as f32 * inv, y as f32 * inv, z as f32 * inv];
                        assert_eq!(
                            row[x],
                            perlin3(p, freq, 0x77CA31),
                            "perlin row != reference at {x},{y},{z} freq {freq}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn generated_volume_layout_and_thread_count_invariance() {
        // Layout must be x-fastest RGBA8, IDENTICAL regardless of how many
        // threads carve the slabs, and identical to the per-voxel
        // reference. Uses the real `generate_volume` driver with the
        // detail octaves at a small edge.
        let build = |size: u32, threads: usize| -> Vec<u8> {
            let n = size as usize;
            let w: [WorleyOct; 4] = std::array::from_fn(|i| {
                WorleyOct::new(DETAIL_WORLEY[i].0, DETAIL_WORLEY[i].1)
            });
            let r: [PerlinOct; 3] =
                std::array::from_fn(|i| PerlinOct::new(FILAMENT_OCTAVES[i].0, FILAMENT_OCTAVES[i].1));
            generate_volume(
                size,
                threads,
                || DetailScratch {
                    w: std::array::from_fn(|_| vec![0.0f32; n]),
                    r: std::array::from_fn(|_| vec![0.0f32; n]),
                },
                |sc, y, z, out| {
                    for i in 0..4 {
                        w[i].row(size, y, z, &mut sc.w[i]);
                    }
                    for i in 0..3 {
                        r[i].row(size, y, z, &mut sc.r[i]);
                    }
                    for (x, texel) in out.chunks_exact_mut(4).enumerate() {
                        let wv = [sc.w[0][x], sc.w[1][x], sc.w[2][x], sc.w[3][x]];
                        let rg = |v: f32| (1.0 - (2.0 * v - 1.0).abs()).clamp(0.0, 1.0);
                        let fil = filament_mix(rg(sc.r[0][x]), rg(sc.r[1][x]), rg(sc.r[2][x]));
                        texel.copy_from_slice(&detail_bytes(wv, fil));
                    }
                },
            )
        };
        let one = build(16, 1);
        let many = build(16, 7);
        assert_eq!(one.len(), 16 * 16 * 16 * 4);
        assert_eq!(one, many, "thread split changed the volume bytes");
        assert_eq!(one, ref_volume(16, detail_voxel), "row bake != per-voxel bake");
        // Spot-check texel addressing: voxel (x=3, y=5, z=7).
        let idx = ((7 * 16 + 5) * 16 + 3) * 4;
        let expect = detail_voxel([3.0 / 16.0, 5.0 / 16.0, 7.0 / 16.0]);
        assert_eq!(&one[idx..idx + 4], &expect);
    }

    #[test]
    fn shape_bake_matches_its_per_voxel_reference() {
        // Same gate for the SHAPE assembly (four Worley + four Perlin
        // octaves through `shape_bytes`), which has a different scratch
        // shape from the detail bake above.
        let size = 12u32;
        let n = size as usize;
        let w: [WorleyOct; 4] =
            std::array::from_fn(|i| WorleyOct::new(SHAPE_WORLEY[i].0, SHAPE_WORLEY[i].1));
        let p: [PerlinOct; 4] = std::array::from_fn(|i| {
            let seed = match i {
                0 => SEED_PERLIN,
                k => SEED_PERLIN.wrapping_add(0x1234_5600 + k as u32),
            };
            PerlinOct::new(SHAPE_PERLIN_BASE << i, seed)
        });
        let got = generate_volume(
            size,
            3,
            || ShapeScratch {
                w: std::array::from_fn(|_| vec![0.0f32; n]),
                p: std::array::from_fn(|_| vec![0.0f32; n]),
            },
            |sc, y, z, out| {
                for i in 0..4 {
                    w[i].row(size, y, z, &mut sc.w[i]);
                    p[i].row(size, y, z, &mut sc.p[i]);
                }
                for (x, texel) in out.chunks_exact_mut(4).enumerate() {
                    let wv = [sc.w[0][x], sc.w[1][x], sc.w[2][x], sc.w[3][x]];
                    let per = perlin_fbm_mix(sc.p[0][x], sc.p[1][x], sc.p[2][x], sc.p[3][x]);
                    texel.copy_from_slice(&shape_bytes(wv, per));
                }
            },
        );
        assert_eq!(got, ref_volume(size, shape_voxel), "shape row bake != per-voxel bake");
    }

    #[test]
    fn filament_channel_tiles_ranges_and_varies() {
        // The ridged-Perlin filament FBM (DETAIL alpha) must stay in unit
        // range, tile exactly on every axis (it rides the repeat sampler),
        // and actually vary (a flat channel would fray nothing).
        let mut lo = f32::MAX;
        let mut hi = f32::MIN;
        for p in probes() {
            let base = filament_fbm3(p, SEED_FIL);
            assert!((0.0..=1.0).contains(&base), "filament out of range: {base}");
            lo = lo.min(base);
            hi = hi.max(base);
            for axis in 0..3 {
                let mut q = p;
                q[axis] += 1.0; // exact for multiples of 1/64
                assert_eq!(
                    filament_fbm3(q, SEED_FIL),
                    base,
                    "filament breaks tiling at {p:?} axis {axis}"
                );
            }
        }
        // Range/variation is measured on a DENSE deterministic sweep, not
        // on the 25 tiling probes above: those exist to be exactly
        // representable (so the +1.0 tiling check is float-exact), and 25
        // samples of a channel whose mean sits at 0.81 is far too thin a
        // sample to say anything about spread. (v0.1188: the 25-probe
        // version measured a 0.348 range and tripped a 0.35 gate on a
        // change that moved the true sigma by 3%.)
        for i in 0..4000u32 {
            let p = [
                (i as f32 * 0.013_71) % 1.0,
                (i as f32 * 0.037_13) % 1.0,
                (i as f32 * 0.007_47) % 1.0,
            ];
            let v = filament_fbm3(p, SEED_FIL);
            assert!((0.0..=1.0).contains(&v), "filament out of range: {v}");
            lo = lo.min(v);
            hi = hi.max(v);
        }
        assert!(hi - lo > 0.35, "filament too flat: {lo}..{hi}");
        // Ridged crests reach high (near 1) somewhere -- that is the streak.
        assert!(hi > 0.7, "filament has no sharp ridges: max {hi}");
        // It must land in the DETAIL voxel's alpha slot (not the old 255).
        let v = detail_voxel([0.3, 0.6, 0.1]);
        assert_eq!(v[3], (filament_fbm3([0.3, 0.6, 0.1], SEED_FIL) * 255.0).round() as u8);
    }

    #[test]
    fn mip_chain_sizes_match_wgpu_and_preserve_the_mean() {
        // 16^3 through the real machinery: the chain must produce the
        // wgpu floor-halving ladder (16,8,4,2,1 = 5 levels), keep level 0
        // byte-identical to the base, and hold the volume MEAN roughly
        // constant across levels (a box filter is mean-preserving up to
        // rounding; a broken indexing stride would shear the mean).
        let base = ref_volume(16, detail_voxel);
        let mean = |v: &[u8]| {
            v.iter().map(|&b| b as u64).sum::<u64>() as f64 / v.len() as f64
        };
        let m0 = mean(&base);
        let chain = mip_chain(base.clone(), 16);
        assert_eq!(chain.len(), 5, "16^3 must give 5 mip levels");
        assert_eq!(chain[0], base, "level 0 must be untouched");
        let sizes = [16usize, 8, 4, 2, 1];
        for (lvl, data) in chain.iter().enumerate() {
            let s = sizes[lvl];
            assert_eq!(data.len(), s * s * s * 4, "level {lvl} byte size");
            let m = mean(data);
            assert!(
                (m - m0).abs() < 2.5,
                "level {lvl} mean drifted: {m:.2} vs base {m0:.2}"
            );
        }
        // Variance renormalization: without it a box mip's histogram
        // narrows level by level and the shader's carve thresholds read
        // that as coverage loss. Levels 1-2 must recover most of the base
        // sigma (deep tiny levels are excused - the gain cap and single-
        // digit voxel counts dominate there).
        let sigma = |v: &[u8]| {
            let m = mean(v);
            let var = v.iter().map(|&b| (b as f64 - m) * (b as f64 - m)).sum::<f64>()
                / v.len() as f64;
            var.sqrt()
        };
        let s0 = sigma(&base);
        for lvl in 1..=2 {
            let sl = sigma(&chain[lvl]);
            assert!(
                sl > 0.5 * s0,
                "level {lvl} sigma collapsed: {sl:.2} vs base {s0:.2}"
            );
        }
        // Odd-edge halving (the 384 ladder ends 3 -> 1): must not panic
        // and must land at 1^3.
        let odd = ref_volume(3, detail_voxel);
        let odd_chain = mip_chain(odd, 3);
        assert_eq!(odd_chain.len(), 2);
        assert_eq!(odd_chain[1].len(), 4);
    }

    #[test]
    // UN-PINNED at increment 10b (2026-08-21): the polarity is FIXED and
    // this probe now guards it forever. It was #[ignore]d as KNOWN DEFECT
    // between increment 3 (discovery + failed isolated flip on the old
    // mask-tau integrator, whose look leaned on the foam topology) and 10b
    // (flip landed on the corrected integrator; the flipped body's
    // QUANTILES matched the old within ~0.03 so the coverage window stood,
    // and the integrator twin measured -0.0% across the flip).
    fn perlin_worley_body_peaks_at_worley_features_not_gaps() {
        // THE POLARITY PROBE (environment program increment 3). The
        // council flagged the Perlin-Worley remap as possibly inverted:
        // the canonical recipe means Worley FEATURE POINTS (cell
        // centres, inverted-Worley = 1) to carry the cloud BODY, with
        // the cell GAPS eating bays into it - billowy masses separated
        // by clear lanes. If the remap runs the other way, the body
        // lives in the LATTICE OF GAPS between cells: a connected foam
        // web with holes where the puffs should be, which no downstream
        // tuning can ever make look like cumulus. Every field re-tune in
        // the program is invalid if this is backwards, so it is pinned
        // here BEFORE any of them.
        //
        // Method: sample many points, bucket by the same lofi FBM the
        // shape_voxel remap uses (high lofi = at/near feature points,
        // low = in the gaps), and require the mean BODY (R channel,
        // after the remap) to be markedly higher in the feature bucket.
        let mut feat_sum = 0.0f64;
        let mut feat_n = 0u32;
        let mut gap_sum = 0.0f64;
        let mut gap_n = 0u32;
        for i in 0..40_000u32 {
            let p = [
                (i as f32 * 0.037_71) % 1.0,
                (i as f32 * 0.091_13) % 1.0,
                (i as f32 * 0.053_47) % 1.0,
            ];
            let w0 = worley3(p, SHAPE_WORLEY[0].0, SHAPE_WORLEY[0].1);
            let w1 = worley3(p, SHAPE_WORLEY[1].0, SHAPE_WORLEY[1].1);
            let w2 = worley3(p, SHAPE_WORLEY[2].0, SHAPE_WORLEY[2].1);
            let w3 = worley3(p, SHAPE_WORLEY[3].0, SHAPE_WORLEY[3].1);
            let lofi = w0 * 0.625
                + w1 * 0.25
                + w2 * 0.125
                + (w3 - WORLEY_MEAN) * SHAPE_LOFI_EXT;
            let body = shape_voxel(p)[0] as f64;
            if lofi > 0.75 {
                feat_sum += body;
                feat_n += 1;
            } else if lofi < 0.35 {
                gap_sum += body;
                gap_n += 1;
            }
        }
        assert!(feat_n > 200 && gap_n > 200, "buckets too thin: {feat_n}/{gap_n}");
        let feat = feat_sum / feat_n as f64;
        let gap = gap_sum / gap_n as f64;
        // Margin 1.1: the probe guards the DIRECTION (a true inversion
        // measures ~0.84 here, the corrected field ~1.18 - a wide gulf),
        // not a magnitude tune. Byte means over conditioned buckets sit
        // closer together than the raw remap algebra suggests because
        // per and lofi are correlated through the shared lattice.
        assert!(
            feat > gap * 1.1,
            "POLARITY INVERTED: body at Worley features {feat:.1} vs gaps {gap:.1} \
             - the cloud mass lives in the lattice between cells, not in the cells"
        );
    }

    /// THE BAKE RIG (dev tooling, permanent): wall-clock cost of the whole
    /// noise bake PLUS the composite statistics every shader constant is
    /// calibrated against. Run by hand in release:
    ///
    ///   cargo test --release --features native --lib bake_bench -- --ignored --nocapture
    ///
    /// The composites printed here are exactly the quantities the WGSL
    /// consumes - `body` (shape R, calibrated by CLOUD_COV_LO/HI + the carve
    /// width table), `lofi` (shape G/B/A weighted 0.625/0.25/0.125, feeding
    /// the tower smoothstep(0.62, 0.92) and the base-drop term), the shape G
    /// mean (hardcoded 0.481 in the cell-split centering), `dfbm` (detail
    /// R/G/B, the three erosion bands) and the filament channel (detail A,
    /// windowed by CLOUD_FIL_LO/HI). Any bake change must leave these
    /// essentially where they were or every one of those constants silently
    /// drifts out of calibration.
    #[test]
    #[ignore = "bench + stats probe; run by hand in release"]
    fn bake_bench_and_composite_stats() {
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let t0 = std::time::Instant::now();
        let shape = generate_shape(threads);
        let t_shape = t0.elapsed().as_secs_f64() * 1000.0;
        let t1 = std::time::Instant::now();
        let detail = generate_detail(threads);
        let t_detail = t1.elapsed().as_secs_f64() * 1000.0;
        println!(
            "gen: shape {SHAPE_SIZE}^3 {t_shape:.0} ms, detail {DETAIL_SIZE}^3 {t_detail:.0} ms \
             ({threads} threads)"
        );

        // Composite statistics (mean, sigma, and the shader's own decision
        // percentiles) over the base level.
        let stat = |vals: &dyn Fn(&[u8]) -> f32, data: &[u8], gates: [f32; 2]| {
            let n = (data.len() / 4) as f64;
            let mut sum = 0.0f64;
            let mut sum2 = 0.0f64;
            let mut over = [0.0f64; 2];
            for px in data.chunks_exact(4) {
                let v = vals(px) as f64;
                sum += v;
                sum2 += v * v;
                for g in 0..2 {
                    if v > gates[g] as f64 {
                        over[g] += 1.0;
                    }
                }
            }
            let mean = sum / n;
            let sigma = (sum2 / n - mean * mean).max(0.0).sqrt();
            (mean, sigma, over[0] / n, over[1] / n)
        };
        let b = |x: u8| x as f32 / 255.0;
        let body = |px: &[u8]| b(px[0]);
        let lofi = |px: &[u8]| b(px[1]) * 0.625 + b(px[2]) * 0.25 + b(px[3]) * 0.125;
        let gch = |px: &[u8]| b(px[1]);
        let dfbm = |px: &[u8]| b(px[0]) * 0.625 + b(px[1]) * 0.25 + b(px[2]) * 0.125;
        let fil = |px: &[u8]| b(px[3]);
        let show = |name: &str, s: (f64, f64, f64, f64), gates: [f32; 2]| {
            println!(
                "  {name:<8} mean {:.4} sigma {:.4}  P(>{:.2}) {:.4}  P(>{:.2}) {:.4}",
                s.0, s.1, gates[0], s.2, gates[1], s.3
            );
        };
        // Gates are the SHADER's own decision points: CLOUD_COV_HI/LO for
        // the body, the tower smoothstep for lofi, CLOUD_FIL_LO/HI for the
        // filament.
        show("body", stat(&body, &shape, [0.347, 0.854]), [0.347, 0.854]);
        show("lofi", stat(&lofi, &shape, [0.62, 0.92]), [0.62, 0.92]);
        show("shape.g", stat(&gch, &shape, [0.481, 0.75]), [0.481, 0.75]);
        show("dfbm", stat(&dfbm, &detail, [0.40, 0.70]), [0.40, 0.70]);
        show("filament", stat(&fil, &detail, [0.30, 0.74]), [0.30, 0.74]);
        // Byte-clipping census: an octave added on top of a channel can push
        // it past 255. Report how much of each channel sits pinned at the
        // rails so the histogram distortion is a measured number, not a hope.
        for (label, data) in [("shape", &shape), ("detail", &detail)] {
            let n = (data.len() / 4) as f64;
            let mut hi = [0.0f64; 4];
            let mut lo = [0.0f64; 4];
            for px in data.chunks_exact(4) {
                for c in 0..4 {
                    if px[c] == 255 {
                        hi[c] += 1.0;
                    }
                    if px[c] == 0 {
                        lo[c] += 1.0;
                    }
                }
            }
            println!(
                "  {label} rail fractions: hi {:?} lo {:?}",
                hi.iter().map(|v| (v / n * 1e4).round() / 1e4).collect::<Vec<_>>(),
                lo.iter().map(|v| (v / n * 1e4).round() / 1e4).collect::<Vec<_>>()
            );
        }

        let t2 = std::time::Instant::now();
        let sc = mip_chain(shape, SHAPE_SIZE);
        let t_mshape = t2.elapsed().as_secs_f64() * 1000.0;
        let t3 = std::time::Instant::now();
        let dc = mip_chain(detail, DETAIL_SIZE);
        let t_mdetail = t3.elapsed().as_secs_f64() * 1000.0;
        let bytes = |c: &Vec<Vec<u8>>| c.iter().map(|l| l.len()).sum::<usize>();
        println!(
            "mip: shape {} levels {:.0} ms ({:.1} MiB), detail {} levels {:.0} ms ({:.1} MiB)",
            sc.len(),
            t_mshape,
            bytes(&sc) as f64 / (1024.0 * 1024.0),
            dc.len(),
            t_mdetail,
            bytes(&dc) as f64 / (1024.0 * 1024.0),
        );
        println!(
            "TOTAL bake {:.0} ms, GPU upload {:.1} MiB",
            t_shape + t_detail + t_mshape + t_mdetail,
            (bytes(&sc) + bytes(&dc)) as f64 / (1024.0 * 1024.0)
        );
    }

    #[test]
    fn shape_channels_carry_independent_octaves() {
        // G/B/A are different frequencies + seeds: they must decorrelate.
        let mut same = 0;
        for p in probes() {
            let v = shape_voxel(p);
            if v[1] == v[2] && v[2] == v[3] {
                same += 1;
            }
        }
        assert!(same < 3, "shape octave channels look identical: {same}");
    }
}
