// ── LIGHT ABOVE THE CLOUD DECK: THE FULLSCREEN EMISSION PASS (2026-09-27) ──
//
// PRIORITIES 1b. The aurora emits between 99 and 190 km and the cloud deck
// sits near 12 km, so from above the deck is BEHIND the aurora and cannot
// occlude it. It did: the fullscreen cloud composite runs after the celestial
// transparent list (atmo_over is always false, correctly, for scattered air;
// see engine/frame_shells.rs), so the deck painted over the emission. Cloud
// cover is regional, which is why the dimming wore a coastline and read for
// weeks as "the aurora is darker over land".
//
// v0.1331.18 moved the emission after the composite by drawing the atmosphere
// shell a SECOND time through the alpha-blended transparent pipeline. That
// fixed the land/water split but left three things wrong, and this pass is the
// replacement for all three:
//
//   1. The blend was OVER, not ADD: the draw delivered emission + dst * (1 -
//      alpha), so wherever the curtain was bright it ERASED the cloud or ground
//      behind it. Light adds. This pass writes with a One/One colour blend.
//      Measured on the way (2026-09-27): the OVER blend also DROPPED every
//      pixel fainter than half an 8-bit step. The emission rode in the alpha
//      (colour / alpha, alpha = brightest channel), and the blend unit rounds
//      the source alpha to the 8-bit target, so a glow under 0.5/255 in linear
//      delivered nothing. Where the old draw wrote black, this pass writes sRGB
//      green codes 0 to 6 in 99.7 percent of pixels; where it wrote anything,
//      7 and up. That cut is the blotchy, speckle-edged diffuse glow in every
//      capture of the old draw. Additive light carries no alpha to round.
//   2. From BELOW the deck (the ground, an aircraft) the clouds are IN FRONT of
//      the aurora, and drawing it last painted it straight through an overcast
//      sky. The renderer now runs this pass at one of two points in the frame,
//      chosen by where the camera is (renderer::emission_pass):
//        camera above the emitting layer's floor: after the cloud composite,
//          so nothing below the aurora can cover it;
//        camera below the floor: before the celestial transparent pass, so
//          the atmosphere dome attenuates it by the air column in front of it
//          and the deck, whichever renderer draws it, covers it.
//      The split is exact, not a heuristic: altitude along a straight ray has
//      one minimum, so a ray from a camera below the floor that reaches the
//      layer has already crossed every cloud it will ever meet, and a ray
//      from a camera above the floor meets the layer (its near crossing,
//      which is all aurora_emission integrates) before any cloud.
//   3. It rasterised the shell MESH, so the emitting layer could never extend
//      past the shell's 191 km top. The rays here are analytic, clipped to the
//      layer the regions describe, so the red cap's real 300 to 400 km is now
//      only a data change (region_kinds.ron), not a scattering-model change.
//
// Occlusion is per pixel: the ray stops at the planet sphere (analytic) and at
// the scene depth (terrain above the sphere, the Moon, anything opaque in the
// celestial pass), with the same reverse-Z linearisation the cloud composite
// uses. `aurora_emission` itself is NOT copied here: it lives once, in
// 30-atmosphere.wgsl, and this entry is the only caller. This file is also
// where any other light that lives above the deck belongs (airglow, lightning
// seen from orbit), because it has exactly the same ordering problem.

struct EmissionPassUniforms {
    // xyz = eye in the celestial render frame, w = tan(fov_y / 2).
    cam_pos: vec4<f32>,
    // xyz = forward, w = aspect.
    cam_fwd: vec4<f32>,
    // xyz = right, w = m22 of the celestial reverse-Z projection.
    cam_right: vec4<f32>,
    // xyz = up, w = m32 of the same projection.
    cam_up: vec4<f32>,
    // xyz = planet centre (render frame), w = the atmosphere SHELL radius in
    // world units. Shell units (radius 1 = that shell) are what the region
    // altitudes and aurora_emission are written in.
    center: vec4<f32>,
    // x = planet radius in shell units (rp), y = the top of the highest
    // emitting layer in shell units (the cheap rejection radius), zw unused.
    layer: vec4<f32>,
}

// Group 1 of the EMISSION PASS layout only (camera group 0 + this). The class
// PSOs bind the object uniform at group 1 binding 0 and never touch these two,
// and a global an entry point does not use is not part of its interface.
@group(1) @binding(1) var<uniform> emission_u: EmissionPassUniforms;
@group(1) @binding(2) var emission_depth: texture_depth_2d;

@fragment
fn fs_emission_pass(in: CloudScreenVsOut) -> @location(0) vec4<f32> {
    let tanf = emission_u.cam_pos.w;
    let aspect = emission_u.cam_fwd.w;
    let fwd = emission_u.cam_fwd.xyz;
    let rd = normalize(
        fwd
            + emission_u.cam_right.xyz * (in.ndc.x * tanf * aspect)
            + emission_u.cam_up.xyz * (in.ndc.y * tanf),
    );
    // One pixel's angle, read HERE: derivatives are only defined under uniform
    // control flow, and every exit below is a discard. Same rule as the shell
    // path this replaced.
    let pix_ang = max(max(length(dpdx(rd)), length(dpdy(rd))), 1.0e-9);

    let shell_r = max(emission_u.center.w, 1.0e-6);
    let rp = clamp(emission_u.layer.x, 0.01, 0.9999);
    let r_top = max(emission_u.layer.y, rp);
    let ro = (emission_u.cam_pos.xyz - emission_u.center.xyz) / shell_r;

    // Cheap rejection first: a ray that never reaches the top of the highest
    // emitting layer (most of the screen whenever the planet is small or the
    // oval is out of view) costs one dot product and leaves.
    let tca = -dot(ro, rd);
    let perp = ro + rd * tca;
    let d2 = dot(perp, perp);
    if (d2 >= r_top * r_top) {
        discard;
    }
    let th = sqrt(r_top * r_top - d2);
    if (tca + th <= 0.0) {
        discard; // the layer is entirely behind the camera
    }
    let t0 = max(tca - th, 0.0);
    var t1 = tca + th;

    // Occluder 1, the planet: light behind the globe does not reach the eye.
    if (d2 < rp * rp && tca > 0.0) {
        t1 = min(t1, tca - sqrt(rp * rp - d2));
    }
    // Occluder 2, the scene depth: terrain standing above the sphere, a moon,
    // anything opaque the celestial pass drew. Sky reads depth ~0, which
    // linearises to ~1e13 and clips nothing. The texel is found through uv
    // (the composite's convention) so a capture target of another size still
    // reads the right depth.
    let dim = vec2<f32>(textureDimensions(emission_depth));
    let uv = vec2<f32>(in.ndc.x * 0.5 + 0.5, 0.5 - in.ndc.y * 0.5);
    let px = clamp(vec2<i32>(uv * dim), vec2<i32>(0), vec2<i32>(dim) - vec2<i32>(1));
    let d_raw = textureLoad(emission_depth, px, 0);
    let view_dist = emission_u.cam_up.w / (d_raw + emission_u.cam_right.w);
    let along = max(dot(rd, fwd), 1.0e-3);
    t1 = min(t1, view_dist / along / shell_r);
    if (t1 <= t0) {
        discard;
    }

    let au_raw = aurora_emission(ro, rd, t0, t1, rp, pix_ang);
    if (max(au_raw.r, max(au_raw.g, au_raw.b)) <= 5.0e-4) {
        discard; // no aurora on this ray: the target is left exactly as it was
    }
    // A SHOULDER, NOT A CLIP (operator, 2026-09-24: "the harsh edges for the
    // different shades of green/orange look weird"). 1 - exp(-x) is linear for
    // faint light and rolls a bright fold off smoothly toward white-green
    // instead of clipping one channel while the others keep rising.
    // NOT DITHERED HERE any more (HDR scene target, increment 4, 2026-09-27):
    // this lands in the Rgba16Float scene target, which does not quantise,
    // and the one dither sits in the present pass before the 8-bit write
    // (assets/shaders/present.wgsl), where it sees the final value rather than
    // the aurora's share of it. The shoulder goes with the linear increment.
    let au = vec3<f32>(1.0) - exp(-au_raw);
    // ADDITIVE (the pipeline blends One/One on colour and leaves alpha alone).
    return vec4<f32>(au, 0.0);
}
