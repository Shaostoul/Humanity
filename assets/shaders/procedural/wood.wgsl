// Procedural Wood Grain Shader
//
// Algorithm: Uses distance from a central axis to create concentric growth
// rings (as seen in a cross-section). The ring pattern is distorted by fbm
// noise to simulate natural grain irregularity. Knot holes are placed using
// a hash-based scatter and modeled as circular distance functions that
// compress nearby rings.
//
// Outputs PBR channels: base_color, roughness, metallic, normal.

// ── Parameters ──
struct WoodParams {
    ring_spacing: f32,       // distance between rings (e.g. 0.05)
    grain_color: vec3<f32>,  // lighter wood between rings
    ring_color: vec3<f32>,   // darker ring lines
    knot_density: f32,       // 0..1 probability of knots
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

// ── Main material function ──

struct MaterialOutput {
    base_color: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal: vec3<f32>,
}

fn wood_material(uv: vec2<f32>, params: WoodParams) -> MaterialOutput {
    var out: MaterialOutput;

    // Center the ring pattern
    let centered = uv - vec2<f32>(0.5, 0.5);

    // Distort coordinates with noise for natural grain
    let distortion = vec2<f32>(
        fbm(uv * 4.0, 3) * 0.15,
        fbm(uv * 4.0 + vec2<f32>(100.0, 0.0), 3) * 0.15
    );
    let distorted = centered + distortion;

    // Distance from center creates concentric rings
    var dist = length(distorted);

    // Check for knot holes (scattered point sources that compress rings locally)
    let grid_size = 0.3;
    var knot_influence = 0.0;
    for (var gy = -1; gy <= 1; gy = gy + 1) {
        for (var gx = -1; gx <= 1; gx = gx + 1) {
            let cell = floor(uv / grid_size) + vec2<f32>(f32(gx), f32(gy));
            let cell_hash = hash(cell * 17.3);
            if cell_hash < params.knot_density {
                let knot_pos = (cell + vec2<f32>(hash(cell + vec2<f32>(1.0, 0.0)), hash(cell + vec2<f32>(0.0, 1.0)))) * grid_size;
                let knot_dist = length(uv - knot_pos);
                let knot_radius = 0.02 + cell_hash * 0.03;
                if knot_dist < knot_radius * 4.0 {
                    // Compress rings near the knot
                    knot_influence = max(knot_influence, smoothstep(knot_radius * 4.0, knot_radius, knot_dist));
                }
            }
        }
    }

    // Ring pattern
    let ring_freq = 1.0 / params.ring_spacing;
    let ring_val = dist * ring_freq + knot_influence * 3.0;
    let ring_pattern = abs(sin(ring_val * 3.14159));

    // Fine grain lines along Y (tangential direction)
    let fine_grain = noise(vec2<f32>(uv.x * 200.0, uv.y * 20.0)) * 0.15;

    // Color: blend between grain and ring colors based on ring pattern
    let ring_strength = smoothstep(0.3, 0.7, ring_pattern);
    let wood_color = mix(params.grain_color, params.ring_color, ring_strength);

    // Darken knot centers
    let knot_darkening = 1.0 - knot_influence * 0.5;
    out.base_color = wood_color * knot_darkening + vec3<f32>(fine_grain);
    out.base_color = clamp(out.base_color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Wood is rough, knots are slightly smoother (polished in real wood)
    out.roughness = mix(0.65, 0.55, knot_influence);

    // Not metallic
    out.metallic = 0.0;

    // Normal from ring edges and grain
    let eps = 0.001;

    let d_dx = length(
        (vec2<f32>(uv.x + eps, uv.y) - vec2<f32>(0.5, 0.5)) +
        vec2<f32>(
            fbm(vec2<f32>(uv.x + eps, uv.y) * 4.0, 3) * 0.15,
            fbm(vec2<f32>(uv.x + eps, uv.y) * 4.0 + vec2<f32>(100.0, 0.0), 3) * 0.15
        )
    );
    let d_dy = length(
        (vec2<f32>(uv.x, uv.y + eps) - vec2<f32>(0.5, 0.5)) +
        vec2<f32>(
            fbm(vec2<f32>(uv.x, uv.y + eps) * 4.0, 3) * 0.15,
            fbm(vec2<f32>(uv.x, uv.y + eps) * 4.0 + vec2<f32>(100.0, 0.0), 3) * 0.15
        )
    );

    let ring_dx = abs(sin(d_dx * ring_freq * 3.14159));
    let ring_dy = abs(sin(d_dy * ring_freq * 3.14159));
    let nx = (ring_pattern - ring_dx) * 0.5 / eps;
    let ny = (ring_pattern - ring_dy) * 0.5 / eps;
    out.normal = normalize(vec3<f32>(nx * 0.002, ny * 0.002, 1.0));

    return out;
}
