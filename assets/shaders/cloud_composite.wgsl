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
    cam_up: vec4<f32>,      // xyz; w = cos(theta_max) of the map extent (12c)
    // Planet/shell frame.
    center: vec4<f32>,      // xyz = planet centre (render frame), w = planet radius (world units)
    basis_x: vec4<f32>,     // planet local axes in world space; w: rb ratio
    basis_y: vec4<f32>,     // w: rt ratio
    basis_z: vec4<f32>,     // w: anchor_local.x (map basis anchor)
    // x = m22, y = m32 of the reverse-Z projection (depth linearization,
    // same convention as the SSAO pass), z/w = anchor_local.y/.z.
    proj: vec4<f32>,
}

@group(0) @binding(0) var scene_depth: texture_depth_2d;
@group(0) @binding(1) var<uniform> u: CloudCompositeUniforms;
@group(0) @binding(2) var cloud_map: texture_2d<f32>;
@group(0) @binding(3) var map_sampler: sampler;
// 12g: the NEAR screen-pass accumulation, crossfaded with the octa map
// by u.cam_right.w (0 = pure map, 1 = pure screen).
@group(0) @binding(4) var cloud_screen: texture_2d<f32>;
// Quarter-res first-hit distance (km) from the near march (v0.1244): the
// per-pixel key that decides which arm owns each pixel. 0 = the near ray
// found nothing it owns (clear, or it abstained past its range) - map's.
@group(0) @binding(5) var march_dist: texture_2d<f32>;

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
    // CPU-hysteresis anchor (Wave D fix 2) - LOCKSTEP with cloud_map_up in
    // 40-clouds.wgsl, which decodes the same anchor from camera pads.
    let a_l = normalize(vec3<f32>(u.basis_z.w, u.proj.z, u.proj.w));
    let up = normalize(to_world(a_l));
    let axis = normalize(u.basis_y.xyz);
    var t1 = cross(up, axis);
    if (dot(t1, t1) < 1.0e-6) {
        t1 = cross(up, normalize(u.basis_x.xyz));
    }
    t1 = normalize(t1);
    let t2 = cross(up, t1);
    return mat3x3<f32>(t1, up, t2);
}

// 12c extent encode - LOCKSTEP with cloud_map_encode_at in 40-clouds.wgsl.
// Returns xy = uv, z = RAW r^2: z > 1 means the direction lies outside
// the map's extent (no data - the caller discards; the CPU controller's
// 4 deg margin means a slab-hitting ray should never land there).
fn map_encode(d: vec3<f32>) -> vec3<f32> {
    let b = map_basis();
    let l = vec3<f32>(dot(d, b[0]), dot(d, b[1]), dot(d, b[2]));
    let k = clamp(1.0 - u.cam_up.w, 1.0e-3, 2.0);
    let r2 = (1.0 - l.y) / k;
    let xz_len = max(length(l.xz), 1.0e-6);
    let p = (l.xz / xz_len) * sqrt(clamp(r2, 0.0, 1.0));
    return vec3<f32>(p * 0.5 + vec2<f32>(0.5), r2);
}

// Catmull-Rom bicubic over the 2048^2 map (operator: "in what ways can we
// increase the resolution of the cloud layer?" - first answer: stop
// throwing away the resolution we have). Bilinear over a premultiplied
// RGBA16F map is a tent filter; at the composite's magnification it reads
// as soft blocky mush. Catmull-Rom reconstructs the same texels visibly
// sharper. 9 bilinear taps on a fullscreen pass is cheap; the max() guard
// clips the filter's small negative undershoot (premultiplied data must
// stay non-negative).
fn map_catmull_rom(uv: vec2<f32>) -> vec4<f32> {
    let res = vec2<f32>(4096.0, 4096.0);
    let sample_pos = uv * res;
    let tex_pos1 = floor(sample_pos - 0.5) + 0.5;
    let f = sample_pos - tex_pos1;
    let w0 = f * (-0.5 + f * (1.0 - 0.5 * f));
    let w1 = 1.0 + f * f * (-2.5 + 1.5 * f);
    let w2 = f * (0.5 + f * (2.0 - 1.5 * f));
    let w3 = f * f * (-0.5 + 0.5 * f);
    let w12 = w1 + w2;
    let offset12 = w2 / w12;
    let tp0 = (tex_pos1 - 1.0) / res;
    let tp3 = (tex_pos1 + 2.0) / res;
    let tp12 = (tex_pos1 + offset12) / res;
    var result = vec4<f32>(0.0);
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp0.x, tp0.y), 0.0) * w0.x * w0.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp12.x, tp0.y), 0.0) * w12.x * w0.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp3.x, tp0.y), 0.0) * w3.x * w0.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp0.x, tp12.y), 0.0) * w0.x * w12.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp12.x, tp12.y), 0.0) * w12.x * w12.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp3.x, tp12.y), 0.0) * w3.x * w12.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp0.x, tp3.y), 0.0) * w0.x * w3.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp12.x, tp3.y), 0.0) * w12.x * w3.y;
    result += textureSampleLevel(cloud_map, map_sampler, vec2<f32>(tp3.x, tp3.y), 0.0) * w3.x * w3.y;
    return max(result, vec4<f32>(0.0));
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
    // Partial-occlusion fraction, ANALYTIC (operator round 7: tiling
    // "everywhere under the clouds in line of sight of Earth, never
    // outside the planet area" - the depth buffer is the ONLY per-pixel
    // input that exists over the planet and not over space, and terrain
    // and water draw in LOD PATCHES whose structure printed through this
    // fraction as tiles). The fraction's legitimate job is coarse - how
    // much of a km-scale cloud sits in front of the ground - so it now
    // derives from the analytically smooth planet sphere, which cannot
    // carry patch texture. The per-pixel depth keeps exactly one job:
    // the hard discard above, where terrain in front of the WHOLE
    // segment fully occludes (mountains still work). A ridge partially
    // overlapping the slab mis-attenuates slightly - the original
    // depth form was itself documented approximate, and a soft error
    // beats a tiled one.
    var seg_frac = 1.0;
    if (d2 < 1.0) {
        let t_srf = tca - sqrt(1.0 - d2);
        if (t_srf > 0.0) {
            seg_frac = clamp((t_srf - m0) / max(m1 - m0, 1.0e-6), 0.0, 1.0);
        }
    }

    // The cloud image (premultiplied), CROSSFADED between the two
    // regimes (12g - the operator's one-frame "huge patch of clouds just
    // vanishes" at the old binary switch):
    //  - The octa map arm (weight 1 - w): direction-indexed Catmull-Rom.
    //    The 1.02 extent threshold (~1 deg past the extent) is
    //    deliberate: at k = 2 the antipode's r^2 lands at 1.0 exactly
    //    and f32 dot jitter can push it a hair over - an exact > 1.0
    //    test would flicker a hole at the sub-camera point. Outside the
    //    extent this arm contributes ZERO (not a discard - the screen
    //    arm may still have content).
    //  - The screen arm (weight w): the half-res accumulation, marched
    //    per pixel for exactly this camera, sampled at this fragment's
    //    own screen coordinate (texture rows run downward, so flip v
    //    like the depth lookup above).
    let w_mix = clamp(u.cam_right.w, 0.0, 1.0);
    // ── PER-PIXEL DISTANCE KEY (v0.1244) ──
    // The global altitude crossfade blended two whole-frame renders and its
    // band was wherever the streaks lived (74 km at 40..80, 36 km at
    // 22..42 - moving the band moved the artifact). The regime handoff is
    // now decided per PIXEL by the near arm's own first-hit distance:
    // content within ~20 km is the near march's (full weight), content past
    // ~32 km is the map's, and a single frame can hold both regimes side by
    // side at matched apparent scale - the MSFS-style continuous handoff.
    // The near arm abstains past 34 km (see fs_cloud_screen), so its buffer
    // holds distance 0 or shell-fallback values exactly where the map must
    // win. w_mix keeps two jobs: the px ramp arming the near system at all,
    // and the whale ceiling.
    var w_px = 0.0;
    var s_scr = vec4<f32>(0.0);
    if (w_mix > 0.001) {
        let duv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
        let dd = vec2<f32>(textureDimensions(march_dist));
        let tpx = vec2<i32>(clamp(duv * dd, vec2<f32>(0.0), dd - vec2<f32>(1.0)));
        let d_km = textureLoad(march_dist, tpx, 0).r;
        if (d_km > 0.001) {
            s_scr = textureSampleLevel(
                cloud_screen, map_sampler, duv, 0.0);
            // The near arm claims a pixel only where it (a) hit content
            // inside its ownership range AND (b) actually drew something:
            // a CLEAR near ray's distance is the shell fallback (it needs
            // one for history tracking), and letting it claim the pixel
            // would blank distant map banks behind clear foreground air -
            // horizon cloud walls would vanish at low altitude.
            let near_has = smoothstep(0.005, 0.03, s_scr.a);
            w_px = w_mix * (1.0 - smoothstep(20.0, 32.0, d_km)) * near_has;
        }
    }
    var s_map = vec4<f32>(0.0);
    if (w_px < 0.999) {
        let e = map_encode(rd);
        if (e.z <= 1.02) {
            s_map = map_catmull_rom(e.xy);
        }
    }
    let s = mix(s_map, s_scr, w_px);
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
