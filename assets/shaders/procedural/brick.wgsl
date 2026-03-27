// Procedural Brick Pattern Shader
//
// Algorithm: Divides UV space into a grid of brick-sized cells. Every other row
// is offset by half a brick width to create the staggered bond pattern. Mortar
// gaps are carved out by checking distance to cell edges. Per-brick color
// variation is driven by a hash of the brick's integer coordinates. Normal
// mapping is derived from the mortar/brick boundary (mortar is recessed).
//
// Outputs PBR channels: base_color, roughness, metallic, normal.

// ── Parameters (bind as uniform) ──
struct BrickParams {
    brick_size: vec2<f32>,   // width, height in UV units (e.g. 0.25, 0.065)
    mortar_width: f32,       // mortar gap half-width in UV units
    brick_color: vec3<f32>,  // base brick RGB
    mortar_color: vec3<f32>, // mortar RGB
    color_variation: f32,    // 0..1 how much each brick's hue shifts
}

// ── Utility functions ──

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(hash(p), hash(p + vec2<f32>(127.1, 311.7)));
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep
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

// ── Main material function ──

struct MaterialOutput {
    base_color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal: vec3<f32>,
}

fn brick_material(uv: vec2<f32>, params: BrickParams) -> MaterialOutput {
    var out: MaterialOutput;

    // Scale UV to brick grid
    let scaled = uv / params.brick_size;

    // Row index determines offset
    let row = floor(scaled.y);
    let row_offset = select(0.0, 0.5, (i32(row) & 1) == 1);

    // Brick-local coordinates
    let brick_uv = vec2<f32>(fract(scaled.x + row_offset), fract(scaled.y));

    // Mortar mask: 1.0 = brick face, 0.0 = mortar
    let mortar_x = params.mortar_width / params.brick_size.x;
    let mortar_y = params.mortar_width / params.brick_size.y;
    let mx = smoothstep(0.0, mortar_x, brick_uv.x) * smoothstep(0.0, mortar_x, 1.0 - brick_uv.x);
    let my = smoothstep(0.0, mortar_y, brick_uv.y) * smoothstep(0.0, mortar_y, 1.0 - brick_uv.y);
    let mortar_mask = mx * my;

    // Per-brick color variation
    let brick_id = vec2<f32>(floor(scaled.x + row_offset), row);
    let h = hash(brick_id);
    let variation = (h - 0.5) * 2.0 * params.color_variation;
    let brick_col = params.brick_color + vec3<f32>(variation * 0.15, variation * 0.05, -variation * 0.05);

    // Add subtle surface noise to the brick face
    let surface_noise = fbm(uv * 80.0, 3) * 0.08;
    let final_brick = clamp(brick_col + vec3<f32>(surface_noise), vec3<f32>(0.0), vec3<f32>(1.0));

    // Blend brick and mortar
    out.base_color = mix(params.mortar_color, final_brick, mortar_mask);

    // Roughness: mortar is rougher than brick
    out.roughness = mix(0.95, 0.7, mortar_mask);

    // No metal
    out.metallic = 0.0;

    // Normal: mortar is recessed, creating edge bumps
    let eps = 0.001;
    let dx_uv = vec2<f32>(uv.x + eps, uv.y);
    let dy_uv = vec2<f32>(uv.x, uv.y + eps);

    // Recompute mortar mask at offset positions for gradient
    let sx_d = dx_uv / params.brick_size;
    let sx_row = floor(sx_d.y);
    let sx_off = select(0.0, 0.5, (i32(sx_row) & 1) == 1);
    let sx_buv = vec2<f32>(fract(sx_d.x + sx_off), fract(sx_d.y));
    let sx_mx = smoothstep(0.0, mortar_x, sx_buv.x) * smoothstep(0.0, mortar_x, 1.0 - sx_buv.x);
    let sx_my = smoothstep(0.0, mortar_y, sx_buv.y) * smoothstep(0.0, mortar_y, 1.0 - sx_buv.y);
    let mask_dx = sx_mx * sx_my;

    let sy_d = dy_uv / params.brick_size;
    let sy_row = floor(sy_d.y);
    let sy_off = select(0.0, 0.5, (i32(sy_row) & 1) == 1);
    let sy_buv = vec2<f32>(fract(sy_d.x + sy_off), fract(sy_d.y));
    let sy_mx = smoothstep(0.0, mortar_x, sy_buv.x) * smoothstep(0.0, mortar_x, 1.0 - sy_buv.x);
    let sy_my = smoothstep(0.0, mortar_y, sy_buv.y) * smoothstep(0.0, mortar_y, 1.0 - sy_buv.y);
    let mask_dy = sy_mx * sy_my;

    let height_scale = 0.15;
    let nx = (mortar_mask - mask_dx) * height_scale / eps;
    let ny = (mortar_mask - mask_dy) * height_scale / eps;
    out.normal = normalize(vec3<f32>(nx, ny, 1.0));

    return out;
}
