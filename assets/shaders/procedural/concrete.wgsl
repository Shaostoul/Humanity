// Procedural Concrete Shader
//
// Algorithm: Combines Voronoi cellular noise for aggregate (gravel) patterns
// with fractional Brownian motion (fbm) for surface cracks and micro-detail.
// Aggregate stones appear as Voronoi cells with per-cell color variation.
// Cracks follow fbm ridges (using abs(noise) for sharp creases). Subtle
// color variation across the surface simulates weathering and moisture.
//
// Outputs PBR channels: base_color, roughness, metallic, normal.

// ── Parameters ──
struct ConcreteParams {
    aggregate_size: f32,     // size of gravel pieces in UV units (e.g. 0.02)
    crack_density: f32,      // 0..1 how many/deep cracks appear
    base_color: vec3<f32>,   // concrete paste color
    roughness: f32,          // base surface roughness
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

// Ridge noise: sharp creases for cracks
fn ridge_noise(p: vec2<f32>) -> f32 {
    return 1.0 - abs(noise(p) * 2.0 - 1.0);
}

fn ridge_fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * ridge_noise(pos);
        pos = pos * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// Voronoi: returns (F1 distance, cell hash)
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

fn concrete_material(uv: vec2<f32>, params: ConcreteParams) -> MaterialOutput {
    var out: MaterialOutput;

    // Aggregate pattern (Voronoi cells represent gravel pieces)
    let agg_scale = 1.0 / params.aggregate_size;
    let agg = voronoi(uv * agg_scale);
    let agg_edge = smoothstep(0.02, 0.08, agg.x); // edge detection for aggregate boundaries

    // Per-aggregate color variation
    let agg_color_shift = (agg.y - 0.5) * 0.08;
    let aggregate_color = params.base_color + vec3<f32>(agg_color_shift, agg_color_shift * 0.8, agg_color_shift * 0.6);

    // Surface cracks using ridge fbm
    let crack_scale = 8.0;
    let crack_val = ridge_fbm(uv * crack_scale, 4);
    let crack_threshold = 1.0 - params.crack_density * 0.3;
    let crack_mask = smoothstep(crack_threshold, crack_threshold + 0.05, crack_val);

    // Subtle large-scale color variation (weathering, moisture)
    let weather = fbm(uv * 3.0, 3) * 0.06;

    // Micro surface detail
    let micro = fbm(uv * 60.0, 3) * 0.04;

    // Final color
    let surface_color = aggregate_color * agg_edge + vec3<f32>(weather + micro);
    // Darken cracks
    out.base_color = mix(surface_color, surface_color * 0.4, crack_mask);
    out.base_color = clamp(out.base_color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Roughness: base roughness, slightly less at aggregate edges, more in cracks
    out.roughness = mix(params.roughness, min(params.roughness + 0.1, 1.0), crack_mask);

    // Concrete is not metallic
    out.metallic = 0.0;

    // Normal from aggregate boundaries, cracks, and surface detail
    let eps = 0.001;

    let agg_dx = voronoi(vec2<f32>(uv.x + eps, uv.y) * agg_scale).x;
    let agg_dy = voronoi(vec2<f32>(uv.x, uv.y + eps) * agg_scale).x;
    let anx = (agg.x - agg_dx) / eps;
    let any_val = (agg.x - agg_dy) / eps;

    let crack_dx = ridge_fbm(vec2<f32>(uv.x + eps, uv.y) * crack_scale, 4);
    let crack_dy = ridge_fbm(vec2<f32>(uv.x, uv.y + eps) * crack_scale, 4);
    let cnx = (crack_val - crack_dx) * params.crack_density / eps;
    let cny = (crack_val - crack_dy) * params.crack_density / eps;

    let micro_dx = fbm(vec2<f32>(uv.x + eps, uv.y) * 60.0, 3);
    let micro_dy = fbm(vec2<f32>(uv.x, uv.y + eps) * 60.0, 3);
    let mnx = (micro - micro_dx) / eps;
    let mny = (micro - micro_dy) / eps;

    let nx = anx * 0.3 + cnx * 0.5 + mnx * 0.2;
    let ny = any_val * 0.3 + cny * 0.5 + mny * 0.2;
    out.normal = normalize(vec3<f32>(nx * 0.01, ny * 0.01, 1.0));

    return out;
}
