// Procedural Metal Surface Shader
//
// Algorithm: Combines directional brushed-metal scratches (anisotropic noise
// stretched along one axis) with cellular-noise rust spots. Scratches modulate
// roughness anisotropically. Rust replaces the base metal color and increases
// roughness where Voronoi cell distance falls below a threshold modulated by
// fbm turbulence.
//
// Outputs PBR channels: base_color, roughness, metallic, normal.

// ── Parameters ──
struct MetalParams {
    base_color: vec3<f32>,     // base metal RGB (e.g. steel gray)
    roughness: f32,            // base roughness
    metallic: f32,             // base metallic (usually ~1.0)
    scratch_intensity: f32,    // 0..1 how visible scratches are
    rust_amount: f32,          // 0..1 how much surface is rusted
}

// ── Utility functions ──

fn hash(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
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

// Voronoi / cellular noise: returns (F1 distance, cell hash)
fn voronoi(p: vec2<f32>) -> vec2<f32> {
    let n = floor(p);
    let f = fract(p);
    var min_dist = 1.0;
    var cell_hash = 0.0;

    for (var j = -1; j <= 1; j = j + 1) {
        for (var i = -1; i <= 1; i = i + 1) {
            let neighbor = vec2<f32>(f32(i), f32(j));
            let cell = n + neighbor;
            let point = neighbor + vec2<f32>(hash(cell), hash(cell + vec2<f32>(57.0, 113.0))) - f;
            let d = dot(point, point);
            if d < min_dist {
                min_dist = d;
                cell_hash = hash(cell + vec2<f32>(37.0, 91.0));
            }
        }
    }
    return vec2<f32>(sqrt(min_dist), cell_hash);
}

// ── Main material function ──

struct MaterialOutput {
    base_color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal: vec3<f32>,
}

fn metal_material(uv: vec2<f32>, params: MetalParams) -> MaterialOutput {
    var out: MaterialOutput;

    // Brushed metal scratches: stretch noise along X for directional look
    let scratch_uv = vec2<f32>(uv.x * 2.0, uv.y * 40.0);
    let scratch = fbm(scratch_uv, 4);
    let scratch_fine = noise(vec2<f32>(uv.x * 5.0, uv.y * 120.0));
    let scratch_val = mix(scratch, scratch_fine, 0.3) * params.scratch_intensity;

    // Rust spots using cellular noise
    let rust_scale = 6.0;
    let rust_cell = voronoi(uv * rust_scale);
    let rust_turbulence = fbm(uv * 12.0, 3);

    // Rust threshold: lower distance + turbulence = more rust
    let rust_threshold = 1.0 - params.rust_amount;
    let rust_mask = smoothstep(rust_threshold, rust_threshold + 0.15, 1.0 - rust_cell.x + rust_turbulence * 0.3);

    // Rust color variation
    let rust_color_base = vec3<f32>(0.45, 0.18, 0.07);
    let rust_color_var = vec3<f32>(0.55, 0.25, 0.1);
    let rust_color = mix(rust_color_base, rust_color_var, rust_cell.y);

    // Base metal with scratches
    let scratched_color = params.base_color + vec3<f32>(scratch_val * 0.1);

    // Blend rust over metal
    out.base_color = mix(scratched_color, rust_color, rust_mask);

    // Roughness: scratches increase it slightly, rust increases it a lot
    let scratch_roughness = params.roughness + scratch_val * 0.15;
    out.roughness = mix(scratch_roughness, 0.85, rust_mask);

    // Metallic: rust is not metallic
    out.metallic = mix(params.metallic, 0.1, rust_mask);

    // Normal map from scratches and rust edges
    let eps = 0.001;
    let scratch_dx = fbm(vec2<f32>((uv.x + eps) * 2.0, uv.y * 40.0), 4);
    let scratch_dy = fbm(vec2<f32>(uv.x * 2.0, (uv.y + eps) * 40.0), 4);
    let snx = (scratch - scratch_dx) * params.scratch_intensity * 2.0 / eps;
    let sny = (scratch - scratch_dy) * params.scratch_intensity * 0.1 / eps;

    // Rust bump from cellular distance
    let rust_dx = voronoi(vec2<f32>(uv.x + eps, uv.y) * rust_scale).x;
    let rust_dy = voronoi(vec2<f32>(uv.x, uv.y + eps) * rust_scale).x;
    let rnx = (rust_cell.x - rust_dx) * rust_mask * 3.0 / eps;
    let rny = (rust_cell.x - rust_dy) * rust_mask * 3.0 / eps;

    let nx = snx + rnx;
    let ny = sny + rny;
    out.normal = normalize(vec3<f32>(nx * 0.01, ny * 0.01, 1.0));

    return out;
}
