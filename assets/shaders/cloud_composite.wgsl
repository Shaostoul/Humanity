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

// Catmull-Rom over the half-res screen accumulation (v0.1251). The
// operator: the clouds read "lower detail than the surface" - and they
// literally were: terrain renders full-res while the cloud layer is a
// half-res buffer that bilinear upsampling then smears further. The
// same 9-tap reconstruction the map arm already used recovers visibly
// more of the resolution the resolve actually holds.
fn screen_catmull_rom(uv: vec2<f32>) -> vec4<f32> {
    let res = vec2<f32>(textureDimensions(cloud_screen));
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
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp0.x, tp0.y), 0.0) * w0.x * w0.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp12.x, tp0.y), 0.0) * w12.x * w0.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp3.x, tp0.y), 0.0) * w3.x * w0.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp0.x, tp12.y), 0.0) * w0.x * w12.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp12.x, tp12.y), 0.0) * w12.x * w12.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp3.x, tp12.y), 0.0) * w3.x * w12.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp0.x, tp3.y), 0.0) * w0.x * w3.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp12.x, tp3.y), 0.0) * w12.x * w3.y;
    result += textureSampleLevel(cloud_screen, map_sampler, vec2<f32>(tp3.x, tp3.y), 0.0) * w3.x * w3.y;
    // NEIGHBOURHOOD CLAMP (v0.1252.3, the operator's low-orbit
    // salt-and-pepper): Catmull-Rom's negative lobes SHARPEN whatever
    // they are given - converged cloud detail, but equally the raw
    // march's unconverged speckle during fast flight, where each noisy
    // half-res texel gets rung with overshoot into a full-screen black
    // or white dot. Clamping the reconstruction to the min/max of the
    // 2x2 texels it interpolates keeps the sharpness on real edges
    // (their extrema are genuine) and makes noise reconstruction no
    // worse than bilinear. The standard companion of CR resampling.
    let dims = vec2<i32>(textureDimensions(cloud_screen));
    let ip = vec2<i32>(tex_pos1);
    var mn = vec4<f32>(1.0e9);
    var mx = vec4<f32>(-1.0e9);
    for (var dy = 0; dy <= 1; dy = dy + 1) {
        for (var dx = 0; dx <= 1; dx = dx + 1) {
            let s = textureLoad(
                cloud_screen,
                clamp(ip + vec2<i32>(dx, dy), vec2<i32>(0), dims - vec2<i32>(1)),
                0);
            mn = min(mn, s);
            mx = max(mx, s);
        }
    }
    return clamp(result, max(mn, vec4<f32>(0.0)), mx);
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
    // ── ONE RENDERER (v0.1250) ──
    // The near march owns every pixel it touched. The octa map no longer
    // dispatches (lib.rs pins near_mix to 1.0 and mod.rs pins octa_runs
    // false; its texture stays zero-initialized), so the 20..32 km
    // distance-ramp ownership key and the near_has claim gate - both
    // seam-feathering devices between two renderers - are retired with
    // it. The 2x2 distance probe remains only as a cheap skip: distance 0
    // in all four quarter texels means the ray never entered the shell,
    // so there is nothing to sample.
    var w_px = 0.0;
    var s_scr = vec4<f32>(0.0);
    if (w_mix > 0.001) {
        let duv = vec2<f32>(in.uv.x, 1.0 - in.uv.y);
        let dd = vec2<f32>(textureDimensions(march_dist));
        let fpx = clamp(duv * dd - vec2<f32>(0.5), vec2<f32>(0.0), dd - vec2<f32>(1.0));
        let base = vec2<i32>(floor(fpx));
        var d_any = 0.0;
        for (var dy = 0; dy <= 1; dy = dy + 1) {
            for (var dx = 0; dx <= 1; dx = dx + 1) {
                let p = clamp(
                    base + vec2<i32>(dx, dy),
                    vec2<i32>(0),
                    vec2<i32>(dd) - vec2<i32>(1),
                );
                d_any = max(d_any, textureLoad(march_dist, p, 0).r);
            }
        }
        if (d_any > 0.001) {
            s_scr = screen_catmull_rom(duv);
            w_px = w_mix;
        }
    }
    // ── THE RETIRED MAP IS NO LONGER SAMPLED (v0.1260) ──
    // The octa direction map stopped dispatching in v0.1250 (ONE
    // RENDERER), but this pass kept BINDING its texture and blending it
    // under every cloud pixel as the backdrop. A render target that is
    // never written is not a guaranteed-zero source across backends and
    // driver paths, and whatever it held was being composited into the
    // final cloud colour - the operator, exactly: "Is there another
    // shader or texture that's affecting cloud shaders that's not
    // supposed to be affecting them?" There was: this one. The near
    // march is the only cloud renderer, so the backdrop term is gone
    // outright rather than multiplied by a hopefully-zero sample. The
    // binding and map_catmull_rom stay for now so the pass layout is
    // untouched; the SAMPLE is what mattered.
    let s_map = vec4<f32>(0.0);
    // NEAR-OVER-MAP (v0.1248). The old mix() REPLACED map content wherever
    // the near arm claimed a pixel - and the two arms render the same sky
    // at different footprints, so every disagreement became a stitch
    // artifact: blue halos around near clouds (thin near edges punched
    // through the map's overcast to raw sky), a literal clear hole under
    // the camera ringed by map deck (near renders sparser than the map),
    // dark "inverted" blobs against the map ceiling. The map is now the
    // BACKDROP everywhere and the near arm composites OVER it
    // (premultiplied): where near content is opaque it wins outright,
    // where it is thin the map shows through - never raw sky, never a
    // hole. The cost is bounded double-density where both arms drew the
    // same cloud, which reads as slightly thicker cloud, not as a seam.
    let near_w = w_px;
    let s = vec4<f32>(
        s_scr.rgb * near_w + s_map.rgb * (1.0 - s_scr.a * near_w),
        s_scr.a * near_w + s_map.a * (1.0 - s_scr.a * near_w),
    );
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
