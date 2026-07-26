// ── Lighting ──

const PI: f32 = 3.14159265359;

// Directional lights are now driven from Rust via CameraUniforms.
// camera.sun_direction.xyz = direction, .w = intensity
// camera.sun_color.rgb = color
// camera.fill_direction.xyz = direction, .w = intensity
// camera.fill_color.rgb = color

// Ambient
const AMBIENT_COLOR: vec3<f32> = vec3<f32>(0.03, 0.03, 0.05);

// ── GGX PBR Functions ──

// Normal Distribution Function (Trowbridge-Reitz GGX)
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (PI * denom * denom);
}

// Geometry function (Schlick-GGX)
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

// Smith's method: combined geometry for both view and light directions
fn geometry_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    return geometry_schlick_ggx(n_dot_v, roughness) * geometry_schlick_ggx(n_dot_l, roughness);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    let t = clamp(1.0 - cos_theta, 0.0, 1.0);
    let t2 = t * t;
    return f0 + (vec3<f32>(1.0) - f0) * (t2 * t2 * t);
}

// ── Procedural Patterns ──

// Hash function for procedural noise
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + vec3<f32>(dot(p3, vec3<f32>(p3.y + 33.33, p3.z + 33.33, p3.x + 33.33)));
    return fract((p3.x + p3.y) * p3.z);
}

// Value noise
fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep

    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// FBM (fractal Brownian motion) — 4 octaves
fn fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pp = p;
    for (var i = 0; i < 4; i = i + 1) {
        value = value + amplitude * value_noise(pp * frequency);
        frequency = frequency * 2.0;
        amplitude = amplitude * 0.5;
    }
    return value;
}

// Panel seam grid (1m panels with 3cm seam lines)
fn grid_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let seam_width = 0.03;
    let fu = fract(u);
    let fv = fract(v);
    let su = smoothstep(0.0, seam_width, fu) * smoothstep(0.0, seam_width, 1.0 - fu);
    let sv = smoothstep(0.0, seam_width, fv) * smoothstep(0.0, seam_width, 1.0 - fv);
    return su * sv;
}

// Brushed metal pattern (directional micro-scratches).
// v0.696 fix: the old vec2(u * 200.0, 0.0) sampled noise along ONE axis with
// the other pinned to zero -- mathematically that is unbroken full-length
// stripes, which is exactly the "horizontal or vertical lines of varied
// colors" the operator screenshotted. A cross-axis coordinate keeps the
// brushed DIRECTION while ending each scratch, and a low-frequency 2D breakup
// varies the field so it reads as metal, not wallpaper.
fn brushed_metal(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
    var u: f32;
    var v: f32;
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        u = world_pos.x;
        v = world_pos.z;
    } else if an.x > an.z {
        u = world_pos.y;
        v = world_pos.z;
    } else {
        u = world_pos.x;
        v = world_pos.y;
    }
    let scratch = value_noise(vec2<f32>(u * 200.0, v * 7.0));
    let breakup = value_noise(vec2<f32>(u * 3.0, v * 3.0));
    return mix(0.85, 1.0, scratch * 0.7 + breakup * 0.3);
}

// Concrete texture (rough, speckled surface)
fn concrete_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    let noise = fbm(uv * 3.0);
    let speckle = value_noise(uv * 40.0) * 0.08;
    // Slightly varied grey
    let base = 0.55 + noise * 0.15 + speckle;
    return vec3<f32>(base, base * 0.98, base * 0.96);
}

// Tri-planar UV projection (reusable helper)
fn triplanar_uv(world_pos: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let an = abs(normal);
    if an.y > an.x && an.y > an.z {
        return world_pos.xz;
    } else if an.x > an.z {
        return world_pos.yz;
    }
    return world_pos.xy;
}

// Voronoi cell noise (returns distance to nearest cell center)
fn voronoi(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    var min_dist = 1.0;
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell_center = vec2<f32>(hash21(i + neighbor), hash21(i + neighbor + vec2<f32>(57.0, 113.0)));
            let diff = neighbor + cell_center - f;
            min_dist = min(min_dist, dot(diff, diff));
        }
    }
    return sqrt(min_dist);
}

// Wood grain pattern
fn wood_pattern(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let an = abs(normal);
    var uv: vec2<f32>;
    if an.y > an.x && an.y > an.z {
        uv = world_pos.xz;
    } else if an.x > an.z {
        uv = world_pos.yz;
    } else {
        uv = world_pos.xy;
    }
    // Ring pattern along one axis
    let ring = sin(uv.x * 25.0 + fbm(uv * 2.0) * 6.0) * 0.5 + 0.5;
    let grain = value_noise(vec2<f32>(uv.x * 2.0, uv.y * 80.0)) * 0.1;
    // Warm wood tones
    let dark = vec3<f32>(0.35, 0.2, 0.1);
    let light = vec3<f32>(0.6, 0.4, 0.2);
    return mix(dark, light, ring) + vec3<f32>(grain);
}

