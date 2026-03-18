// sun_surface.wgsl — Procedural animated solar surface with granulation, filaments,
// sunspots (umbra/penumbra), limb darkening, chromospheric rim, and corona.
// NASA-inspired: differential rotation by latitude, convection flow patterns,
// spectral color shifts from center to limb.
//
// Binding conventions:
//   Group 0: Camera/view uniforms
//   Group 1: Material/object uniforms (model matrix + sun parameters)
//   Vertex inputs: position, normal, tangent, bitangent, uv (standard layout)

struct Camera {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: Camera;

struct SunUniforms {
    model: mat4x4<f32>,
    time: f32,
    granulation_scale: f32,   // Controls size of granulation cells (default ~60.0)
    filament_scale: f32,      // Controls filament noise frequency (default ~24.0)
    corona_intensity: f32,    // Overall corona brightness multiplier (default ~2.5)
    surface_brightness: f32,  // Final surface brightness multiplier (default ~2.0)
    spot_threshold: f32,      // Sunspot appearance threshold (default ~0.55)
    rim_width: f32,           // Chromospheric rim thickness (default ~0.012)
    _padding: f32,
};

@group(1) @binding(0)
var<uniform> sun: SunUniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tangent: vec3<f32>,
    @location(3) bitangent: vec3<f32>,
    @location(4) tex_coords: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) local_position: vec3<f32>,
    @location(3) tangent: vec3<f32>,
    @location(4) bitangent: vec3<f32>,
    @location(5) tex_coords: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = sun.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.normal = in.normal;
    out.local_position = in.position;
    out.tangent = in.tangent;
    out.bitangent = in.bitangent;
    out.tex_coords = in.tex_coords;
    out.clip_position = camera.view_proj * world_pos;
    return out;
}

// --- Noise utilities ---

fn hash3(p: vec3<f32>) -> f32 {
    let p3 = fract(p * 0.1031);
    let p4 = p3 + dot(p3, p3.yzx + 19.19);
    return fract((p4.x + p4.y) * p4.z);
}

fn noise3(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(
            mix(hash3(i + vec3<f32>(0,0,0)), hash3(i + vec3<f32>(1,0,0)), u.x),
            mix(hash3(i + vec3<f32>(0,1,0)), hash3(i + vec3<f32>(1,1,0)), u.x), u.y),
        mix(
            mix(hash3(i + vec3<f32>(0,0,1)), hash3(i + vec3<f32>(1,0,1)), u.x),
            mix(hash3(i + vec3<f32>(0,1,1)), hash3(i + vec3<f32>(1,1,1)), u.x), u.y), u.z);
}

fn rotate(v: vec3<f32>, axis: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    let t = 1.0 - c;
    let x = axis.x;
    let y = axis.y;
    let z = axis.z;
    let m = mat3x3<f32>(
        t*x*x + c,     t*x*y - s*z, t*x*z + s*y,
        t*x*y + s*z,   t*y*y + c,   t*y*z - s*x,
        t*x*z - s*y,   t*y*z + s*x, t*z*z + c
    );
    return m * v;
}

// Sun color ramp: deep red-orange through orange to yellow-white, with hot spot blend
fn enhanced_sun_color(t: f32, spot: f32) -> vec3<f32> {
    let c1 = vec3<f32>(0.6, 0.2, 0.02);
    let c2 = vec3<f32>(1.0, 0.7, 0.2);
    let c3 = vec3<f32>(1.0, 0.95, 0.7);
    let c4 = vec3<f32>(1.0, 1.0, 1.0);
    var base = vec3<f32>(0.0, 0.0, 0.0);
    if (t < 0.5) {
        base = mix(c1, c2, t * 2.0);
    } else {
        base = mix(c2, c3, (t - 0.5) * 2.0);
    }
    let with_hot = mix(base, c4, pow(t, 8.0));
    return mix(vec3<f32>(0.1, 0.05, 0.01), with_hot, spot);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let local_pos = normalize(in.local_position);
    let time = sun.time;

    // Differential rotation: equator rotates faster than poles
    let latitude = abs(local_pos.y);
    let diff_rot = mix(1.0, 0.7, pow(latitude, 2.0));
    let time_dr = time * diff_rot;

    // Hybrid global + local convection flow
    let global_flow = vec3<f32>(0.2, 0.1, 0.0) * time_dr * 0.02;
    let local_noise1 = noise3(local_pos * 12.0 + time_dr * 0.017);
    let local_angle1 = local_noise1 * 6.2831853;
    let local_flow1 = vec3<f32>(cos(local_angle1), sin(local_angle1), sin(local_angle1 * 0.7)) * 0.02 * time_dr;
    let local_noise2 = noise3(local_pos * 24.0 + 13.0 + time_dr * 0.027);
    let local_angle2 = local_noise2 * 6.2831853;
    let local_flow2 = vec3<f32>(cos(local_angle2), sin(local_angle2), sin(local_angle2 * 0.7)) * 0.01 * time_dr;

    let p1 = local_pos + global_flow + local_flow1 + local_flow2;
    let p2 = local_pos + global_flow * 1.2 + local_flow1.yzx * 0.8 + local_flow2.zxy * 0.5;
    let p3 = local_pos + global_flow * 0.7 + local_flow1.zxy * 1.1 + local_flow2.yzx * 0.3;

    // Main granulation
    let advect_flow = noise3(local_pos * 2.0 + time_dr * 0.003) * 0.2;
    let gran = noise3((p1 + advect_flow) * sun.granulation_scale + time_dr * 0.017);

    // Organic filaments: sum of rotated, stretched noise
    let f1 = noise3(rotate(p2 * sun.filament_scale, vec3<f32>(1.0, 1.0, 0.0), 1.2) + time_dr * 0.025);
    let f2 = noise3(rotate(p3 * sun.filament_scale, vec3<f32>(0.0, 1.0, 1.0), 2.1) - time_dr * 0.033);
    let f3 = noise3(rotate(p1 * sun.filament_scale, vec3<f32>(1.0, 0.0, 1.0), 0.7) + time_dr * 0.042);
    let filaments_raw = (f1 + f2 + f3) / 3.0;
    let filaments = smoothstep(0.4, 0.7, filaments_raw);

    // Micro granulation
    let micro = noise3(p3 * 400.0 + time_dr * 0.01) * 0.02 + 0.98;

    // Sunspot mask with umbra/penumbra
    let spot_base = smoothstep(sun.spot_threshold, sun.spot_threshold + 0.15, noise3(p2 * 6.0 + time_dr * 0.013));
    let spot_breakup = smoothstep(0.4, 0.7, noise3(p1 * 32.0 + time_dr * 0.025));
    let spot_mask = spot_base * spot_breakup;
    let umbra = smoothstep(0.7, 0.9, spot_mask);
    let penumbra = smoothstep(0.4, 0.7, spot_mask) - umbra;
    let umbra_color = vec3<f32>(0.2, 0.08, 0.04);
    let penumbra_color = vec3<f32>(0.5, 0.2, 0.1);

    // Limb darkening
    let normal = normalize(in.normal);
    let view_dir = normalize(in.world_position);
    let cos_angle = abs(dot(normal, view_dir));
    let limb = pow(cos_angle, 2.0) * 0.8 + 0.2;

    // Surface brightness flicker
    let flicker = noise3(local_pos * 900.0 + time_dr * 2.5) * 0.012 + 0.994;

    // Combine surface
    let sunspots = 1.0 - filaments * 0.9;
    let combined = mix(gran, micro, 0.15);
    let intensity = combined * sunspots * limb * flicker;

    // Spectral color shift: latitude and view-angle dependent
    let center_blend = smoothstep(0.7, 1.0, cos_angle);
    let limb_blend = 1.0 - center_blend;
    let lat_blend = smoothstep(-1.0, 1.0, local_pos.y);
    let color_shift = mix(
        mix(vec3<f32>(1.0, 0.7, 0.3), vec3<f32>(1.0, 0.9, 0.7), center_blend),
        vec3<f32>(1.0, 0.5, 0.2),
        limb_blend * lat_blend
    );

    var color = enhanced_sun_color(intensity * 0.7 + 0.05, 1.0) * color_shift;
    color = mix(color, penumbra_color, penumbra * 0.7);
    color = mix(color, umbra_color, umbra);

    // Corona
    let corona_noise = noise3(local_pos * 8.0 + time_dr * 0.013) * 0.7
        + noise3(local_pos * 24.0 - time_dr * 0.021) * 0.4
        + noise3(local_pos * 64.0 + time_dr * 0.007) * 0.25;
    let corona_radial = smoothstep(1.01, 2.0, length(local_pos)) * (0.32 + 0.55 * corona_noise);
    let corona_color_ramp = mix(
        vec3<f32>(1.4, 0.5, 0.1),
        vec3<f32>(1.0, 0.95, 0.5),
        smoothstep(1.08, 1.6, length(local_pos))
    );
    let corona_color = corona_color_ramp * corona_radial * sun.corona_intensity;

    // Chromospheric rim
    let rim_mask = smoothstep(1.0 - sun.rim_width, 1.0, length(local_pos)) * (1.0 - smoothstep(1.0, 1.0 + sun.rim_width * 1.8, length(local_pos)));
    let rim_noise = noise3(local_pos * 180.0 + time * 0.5);
    let rim_bright = 1.05 + rim_noise * 0.25;
    let rim_color = vec3<f32>(1.0, 0.45, 0.1) * rim_bright;
    color += rim_color * rim_mask * 0.35;

    return vec4<f32>(color * sun.surface_brightness + corona_color, 1.0);
}
