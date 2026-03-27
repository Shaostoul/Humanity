// Aperiodic (Non-Repeating) Tiling Shader
//
// Algorithm: Two approaches combined for rich non-periodic patterns.
//
// 1. Wang Tiles: The UV plane is divided into a grid of square tiles. Each tile
//    has 4 edge colors (N, E, S, W) selected so adjacent tiles share matching
//    edge colors. A hash of the tile position deterministically picks a tile
//    variant whose edges satisfy the constraint. Interior decoration is then
//    generated per-variant, producing a seamless but non-repeating surface.
//
// 2. Penrose / Quasicrystal overlay: Five sets of parallel lines (at 72-degree
//    intervals) are superimposed. Their interference pattern creates a Penrose-
//    like quasicrystalline pattern with 5-fold rotational symmetry but no
//    translational period. This provides a subtle large-scale structure.
//
// The final output blends both layers for a surface that never repeats at any
// scale.
//
// Outputs PBR channels: base_color, roughness, metallic, normal.

// ── Parameters ──
struct AperiodicParams {
    tile_size: f32,           // size of Wang tiles in UV units (e.g. 0.1)
    num_variants: i32,        // number of decorative variants per edge config (e.g. 4)
    base_colors: array<vec3<f32>, 4>, // palette: 4 base colors for tile decoration
}

// ── Utility functions ──

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash_ivec2(ix: i32, iy: i32) -> f32 {
    return hash(vec2<f32>(f32(ix) * 127.1 + 311.7, f32(iy) * 269.5 + 183.3));
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash(i), hash(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash(i + vec2<f32>(0.0, 1.0)), hash(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * noise(pos);
        pos = pos * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// ── Wang Tile edge color assignment ──
// Each edge between two tiles gets a deterministic color (0..num_edge_colors-1)
// based on a hash of the shared edge coordinates.

const NUM_EDGE_COLORS: i32 = 3;

// Horizontal edge color (shared between tile (x,y) south edge and (x,y-1) north edge)
fn h_edge_color(x: i32, y: i32) -> i32 {
    return i32(hash_ivec2(x * 7 + 13, y * 11 + 37) * f32(NUM_EDGE_COLORS)) % NUM_EDGE_COLORS;
}

// Vertical edge color (shared between tile (x,y) west edge and (x-1,y) east edge)
fn v_edge_color(x: i32, y: i32) -> i32 {
    return i32(hash_ivec2(x * 23 + 89, y * 31 + 53) * f32(NUM_EDGE_COLORS)) % NUM_EDGE_COLORS;
}

// ── Wang tile interior decoration ──
// Given a tile's local UV (0..1), edge colors, and a variant index,
// generate a decorative pattern that respects edge matching.

fn wang_tile_pattern(local_uv: vec2<f32>, tile_x: i32, tile_y: i32, variant: i32) -> vec3<f32> {
    // Get this tile's edge colors
    let north = h_edge_color(tile_x, tile_y + 1);
    let south = h_edge_color(tile_x, tile_y);
    let east = v_edge_color(tile_x + 1, tile_y);
    let west = v_edge_color(tile_x, tile_y);

    // Edge color influences: blend from edges toward center
    // Each edge tints the nearby region with its palette color
    let edge_n = smoothstep(0.0, 0.5, 1.0 - local_uv.y);
    let edge_s = smoothstep(0.0, 0.5, local_uv.y);
    let edge_e = smoothstep(0.0, 0.5, 1.0 - local_uv.x);
    let edge_w = smoothstep(0.0, 0.5, local_uv.x);

    // Palette colors for edge types (derived from base_colors indices via modulo)
    let c_n = vec3<f32>(
        0.3 + f32(north) * 0.2,
        0.4 + f32((north + 1) % 3) * 0.15,
        0.5 + f32((north + 2) % 3) * 0.1
    );
    let c_s = vec3<f32>(
        0.3 + f32(south) * 0.2,
        0.4 + f32((south + 1) % 3) * 0.15,
        0.5 + f32((south + 2) % 3) * 0.1
    );
    let c_e = vec3<f32>(
        0.3 + f32(east) * 0.2,
        0.4 + f32((east + 1) % 3) * 0.15,
        0.5 + f32((east + 2) % 3) * 0.1
    );
    let c_w = vec3<f32>(
        0.3 + f32(west) * 0.2,
        0.4 + f32((west + 1) % 3) * 0.15,
        0.5 + f32((west + 2) % 3) * 0.1
    );

    // Weighted blend of edge influences
    let total_weight = edge_n + edge_s + edge_e + edge_w + 0.001;
    var blended = (c_n * edge_n + c_s * edge_s + c_e * edge_e + c_w * edge_w) / total_weight;

    // Per-variant interior variation
    let variant_hash = hash(vec2<f32>(f32(tile_x * 100 + variant), f32(tile_y * 100 + variant)));
    let interior_noise = fbm(local_uv * 4.0 + vec2<f32>(variant_hash * 100.0, variant_hash * 57.0), 3);
    blended = blended + vec3<f32>(interior_noise * 0.08 - 0.04);

    // Decorative shapes: circles/arcs connecting edge midpoints
    let center_dist = length(local_uv - vec2<f32>(0.5, 0.5));
    let ring = abs(sin(center_dist * 3.14159 * (2.0 + f32(variant)))) * 0.05;
    blended = blended + vec3<f32>(ring);

    return clamp(blended, vec3<f32>(0.0), vec3<f32>(1.0));
}

// ── Penrose quasicrystal pattern ──
// Five sets of parallel stripes at 72-degree intervals (360/5).
// The interference creates 5-fold quasi-periodic structure.

fn quasicrystal(uv: vec2<f32>, scale: f32) -> f32 {
    var total = 0.0;
    let pi = 3.14159265;

    for (var k = 0; k < 5; k = k + 1) {
        let angle = f32(k) * pi / 5.0;
        let dir = vec2<f32>(cos(angle), sin(angle));
        let d = dot(uv * scale, dir);
        total = total + cos(d * 2.0 * pi);
    }

    // Normalize to 0..1 range (raw range is roughly -5..5)
    return (total + 5.0) / 10.0;
}

// ── Main material function ──

struct MaterialOutput {
    base_color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal: vec3<f32>,
}

fn aperiodic_material(uv: vec2<f32>, params: AperiodicParams) -> MaterialOutput {
    var out: MaterialOutput;

    // ── Layer 1: Wang tiles ──
    let tile_uv = uv / params.tile_size;
    let tile_pos = floor(tile_uv);
    let local = fract(tile_uv);
    let tx = i32(tile_pos.x);
    let ty = i32(tile_pos.y);

    // Select variant deterministically
    let variant = i32(hash_ivec2(tx, ty) * f32(params.num_variants)) % params.num_variants;

    let wang_color = wang_tile_pattern(local, tx, ty, variant);

    // ── Layer 2: Quasicrystal overlay ──
    let quasi = quasicrystal(uv, 10.0 / params.tile_size);

    // Map quasicrystal value to subtle tint
    let quasi_tint = mix(vec3<f32>(0.0), vec3<f32>(0.06, 0.04, 0.02), quasi);

    // ── Combine ──
    out.base_color = clamp(wang_color + quasi_tint, vec3<f32>(0.0), vec3<f32>(1.0));

    // Roughness varies with quasicrystal pattern
    out.roughness = 0.6 + quasi * 0.2;

    // Not metallic by default
    out.metallic = 0.0;

    // Normal from both layers
    let eps = 0.001;

    // Wang tile height from luminance
    let wang_h = dot(wang_color, vec3<f32>(0.299, 0.587, 0.114));
    let wang_dx_color = wang_tile_pattern(
        fract((vec2<f32>(uv.x + eps, uv.y)) / params.tile_size),
        i32(floor((uv.x + eps) / params.tile_size)),
        ty,
        i32(hash_ivec2(i32(floor((uv.x + eps) / params.tile_size)), ty) * f32(params.num_variants)) % params.num_variants
    );
    let wang_dy_color = wang_tile_pattern(
        fract((vec2<f32>(uv.x, uv.y + eps)) / params.tile_size),
        tx,
        i32(floor((uv.y + eps) / params.tile_size)),
        i32(hash_ivec2(tx, i32(floor((uv.y + eps) / params.tile_size))) * f32(params.num_variants)) % params.num_variants
    );
    let wang_hx = dot(wang_dx_color, vec3<f32>(0.299, 0.587, 0.114));
    let wang_hy = dot(wang_dy_color, vec3<f32>(0.299, 0.587, 0.114));

    // Quasicrystal height
    let quasi_dx = quasicrystal(vec2<f32>(uv.x + eps, uv.y), 10.0 / params.tile_size);
    let quasi_dy = quasicrystal(vec2<f32>(uv.x, uv.y + eps), 10.0 / params.tile_size);

    let nx = ((wang_h - wang_hx) + (quasi - quasi_dx) * 0.3) * 0.5 / eps;
    let ny = ((wang_h - wang_hy) + (quasi - quasi_dy) * 0.3) * 0.5 / eps;
    out.normal = normalize(vec3<f32>(nx * 0.005, ny * 0.005, 1.0));

    return out;
}
