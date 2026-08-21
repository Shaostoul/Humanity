// ── Fullscreen depth-aware cloud composite (Wave D slice 1b, increment 12) ──
//
// THE VANISHING DECK, cured. When the camera is inside the drawn cloud
// shell (below ~16 km), a DOWNWARD ray's only shell geometry is the far
// side of the sphere - beyond the planet - and the hardware depth test
// killed those fragments behind the terrain, so every cloud below the
// camera disappeared the moment you crossed the shell (measured on the
// descent ladder at 16.8 vs 14.8 km; the operator's "they kinda start to
// look better, then they just vanish"). No shell-geometry arrangement
// fixes it: the cure is compositing the temporal cloud map in a
// FULLSCREEN pass that does its own occlusion against the scene depth -
// terrain in front of the cloud segment wins, terrain behind it does not.
//
// This pass serves the TEMPORAL-ARMED regime (the map exists exactly at
// the altitudes where the deck used to vanish). Above the arming
// altitude the shell's direct-march fragment path still draws, where the
// hardware depth test is geometrically sound (the camera is outside the
// shell, near-side fragments sit in front of terrain).
//
// LOCKSTEP: the Lambert map decode/encode and the PLANET-FIXED SNAPPED
// basis here must stay byte-for-math identical with
// assets/shaders/pbr/40-clouds.wgsl (cloud_map_up / cloud_map_tangents /
// cloud_map_encode). The shell shader reads the planet frame from its
// object uniform; this pass gets the same frame through `basis_*` below.

struct CloudCompositeUniforms {
    // Camera ray basis (world/render frame).
    cam_pos: vec4<f32>,     // xyz = eye, w = tan(fov_y / 2)
    cam_fwd: vec4<f32>,     // xyz = forward, w = aspect
    cam_right: vec4<f32>,   // xyz
    cam_up: vec4<f32>,      // xyz
    // Planet/shell frame.
    center: vec4<f32>,      // xyz = planet centre (render frame), w = planet radius (world units)
    basis_x: vec4<f32>,     // planet local axes in world space; w: rb ratio
    basis_y: vec4<f32>,     // w: rt ratio
    basis_z: vec4<f32>,     // w: unused
    // x = m22, y = m32 of the reverse-Z projection (depth linearization,
    // same convention as the SSAO pass), z = limb enable, w = unused.
    proj: vec4<f32>,
}

@group(0) @binding(0) var scene_depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> u: CloudCompositeUniforms;
@group(0) @binding(2) var cloud_map: texture_2d<f32>;
@group(0) @binding(3) var map_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let x = f32(i32(vi & 1u) * 4 - 1);
    let y = f32(i32(vi & 2u) * 2 - 1);
    var out: VsOut;
    out.pos = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>(x, y) * 0.5 + vec2<f32>(0.5);
    return out;
}

// Planet-local <-> world via the uniform's basis columns.
fn to_local(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(v, u.basis_x.xyz),
        dot(v, u.basis_y.xyz),
        dot(v, u.basis_z.xyz),
    );
}

fn to_world(v: vec3<f32>) -> vec3<f32> {
    return u.basis_x.xyz * v.x + u.basis_y.xyz * v.y + u.basis_z.xyz * v.z;
}

// LOCKSTEP mirror of 40-clouds.wgsl cloud_map_up/cloud_map_tangents/
// cloud_map_encode (the planet-fixed snapped basis).
fn map_basis() -> mat3x3<f32> {
    let up_w = normalize(u.cam_pos.xyz - u.center.xyz);
    let up_l = normalize(to_local(up_w));
    let snapped = normalize(round(up_l / 0.03) * 0.03);
    let up = normalize(to_world(snapped));
    let axis = normalize(u.basis_y.xyz);
    var t1 = cross(up, axis);
    if (dot(t1, t1) < 1.0e-6) {
        t1 = cross(up, normalize(u.basis_x.xyz));
    }
    t1 = normalize(t1);
    let t2 = cross(up, t1);
    return mat3x3<f32>(t1, up, t2);
}

fn map_encode(d: vec3<f32>) -> vec2<f32> {
    let b = map_basis();
    let l = vec3<f32>(dot(d, b[0]), dot(d, b[1]), dot(d, b[2]));
    let r2 = clamp((1.0 - l.y) * 0.5, 0.0, 1.0);
    let xz_len = max(length(l.xz), 1.0e-6);
    let p = (l.xz / xz_len) * sqrt(r2);
    return p * 0.5 + vec2<f32>(0.5);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Pixel ray. This pass's uv comes straight from NDC (y UP - unlike a
    // sampled texture's v): top of screen is uv.y = 1, so ndc = uv*2-1 on
    // BOTH axes; the depth-texel lookup below flips y instead, because
    // texture rows run downward.
    let ndc = in.uv * 2.0 - vec2<f32>(1.0);
    let tanf = u.cam_pos.w;
    let rd = normalize(
        u.cam_fwd.xyz
            + u.cam_right.xyz * (ndc.x * tanf * u.cam_fwd.w)
            + u.cam_up.xyz * (ndc.y * tanf),
    );

    // Slab interval in planet units (identical geometry to the march).
    let pr = u.center.w;
    let ro = (u.cam_pos.xyz - u.center.xyz) / pr;
    let rb = u.basis_x.w;
    let rt = u.basis_y.w;
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= rt * rt) {
        discard;
    }
    let thc_t = sqrt(rt * rt - d2);
    var m0 = max(tca - thc_t, 0.0);
    var m1 = tca + thc_t;
    if (m1 <= 0.0) {
        discard;
    }
    if (d2 < rb * rb) {
        let thc_b = sqrt(rb * rb - d2);
        let b0 = tca - thc_b;
        let b1 = tca + thc_b;
        if (b0 > m0) {
            m1 = min(m1, b0);
        } else if (b1 > m0) {
            m0 = b1;
        }
    }
    if (m1 <= m0) {
        discard;
    }
    // Analytic planet occlusion: a segment fully behind the globe is gone
    // (the march's own rule, mirrored so the map is never composited
    // where its content could not have been marched).
    if (d2 < 1.0) {
        let t_surf = tca - sqrt(1.0 - d2);
        if (t_surf > 0.0 && t_surf < m0) {
            discard;
        }
    }

    // Scene-depth occlusion - THE point of this pass. Reverse-Z
    // linearization with the real projection elements (SSAO convention);
    // sky (depth ~ 0) linearizes huge and never occludes.
    let dim = vec2<f32>(textureDimensions(scene_depth));
    let px = vec2<i32>(vec2<f32>(in.uv.x, 1.0 - in.uv.y) * dim);
    let d_raw = textureLoad(scene_depth, px, 0);
    let view_dist = u.proj.y / (d_raw + u.proj.x);
    let along = max(dot(rd, normalize(u.cam_fwd.xyz)), 1.0e-3);
    let scene_t = view_dist / along / pr; // planet units along this ray
    if (scene_t <= m0) {
        discard; // terrain in front of the whole cloud segment
    }
    // Terrain inside the segment: attenuate by the fraction of the
    // segment behind it (approximate - mass is not uniform along the
    // ray, but the error is a soft fade exactly where a hard cut would
    // pop).
    let seg_frac = clamp((scene_t - m0) / max(m1 - m0, 1.0e-6), 0.0, 1.0);

    // The temporal map (premultiplied), by direction.
    let s = textureSampleLevel(cloud_map, map_sampler, map_encode(rd), 0.0);
    if (s.a <= 0.003) {
        discard;
    }

    // Continuous limb fade - LOCKSTEP with the shell wrapper.
    let n_frag = normalize(ro + rd * m0);
    let mu = clamp(abs(dot(rd, n_frag)), 0.0, 1.0);
    let limb_w = smoothstep(1.0, 1.35, length(ro));
    let limb = mix(1.0, mix(0.55, 1.0, smoothstep(0.0, 0.35, mu)), limb_w);

    let a = clamp(s.a * limb * seg_frac, 0.0, 1.0);
    // Premultiplied-over blend (pipeline: src One, dst OneMinusSrcAlpha):
    // scale the premultiplied rgb by the extra attenuation factors.
    let scale = a / max(s.a, 1.0e-4);
    return vec4<f32>(s.rgb * scale, a);
}
