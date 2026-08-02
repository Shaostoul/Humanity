// ── Fragment Shader ──

// ── Aerial perspective, shared (v0.1053) ──
// Was inline in fs_main's tail only, which meant the WATER shell never got it:
// the type-16 branch early-returns hundreds of lines earlier. So distant waves
// carried zero atmospheric haze while the land beside them faded correctly, and
// a 10 m storm sea read as a flat patterned plane instead of receding relief -
// the operator's "I can see waves behind waves... there's no extra shading" -
// with the sea meeting the sky at a hard contrast step instead of hazing into
// it. Same math, one definition, called from both paths so they cannot drift.
fn aerial_apply(color_in: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let aer_sigma = camera.light1_cone_inner.y;
    if (aer_sigma <= 1.0e-9) {
        return color_in;
    }
    let aer_vec = world_pos - camera.view_pos.xyz;
    let aer_dist = length(aer_vec);
    // NEAR CUTOFF, v0.1060. Operator: "The sandstorm effect just looks like
    // there's a wall of fog in a perfect circle around me. Can we have that
    // blend all the way to me?" That circle IS this cutoff: haze began at a
    // hard 120 m, drawing a crisp ring on the ground at exactly that radius.
    // 120 m is a reasonable floor for CLEAR-air aerial perspective, where the
    // near field genuinely has no measurable extinction - but fog and dust are
    // dense enough to be visible on your own hands. Scale the cutoff down with
    // density: at clear-air sigma it stays 120 m, and by fog densities it is
    // essentially zero, so the dust blends continuously all the way in.
    let near_cut = clamp(120.0 * (2.2e-5 / max(aer_sigma, 1.0e-9)), 0.0, 120.0);
    if (aer_dist <= near_cut) {
        return color_in;
    }
    let aer_up = vec3<f32>(
        camera.light3_cone_inner.y,
        camera.light3_cone_inner.z,
        camera.light3_cone_inner.w,
    );
    let up_dot = abs(dot(aer_vec / aer_dist, aer_up));
    let slant_cap = camera.light1_cone_inner.z / max(up_dot, 0.035);
    let path = min(aer_dist - near_cut, slant_cap);
    let t_aer = exp(-aer_sigma * path);
    let sky_aer = vec3<f32>(
        camera.light2_cone_inner.y,
        camera.light2_cone_inner.z,
        camera.light2_cone_inner.w,
    );
    return color_in * t_aer + sky_aer * (1.0 - t_aer);
}

// ── Underwater extinction (v0.1054) ──
// Operator: "we're able to see the underwater horizon but we shouldn't be able
// to as in real life. I can easily see the sea floor everywhere as if there's no
// actual depth darkening to the water."
//
// Correct: nothing attenuated anything underwater, so a submerged view had
// unlimited visibility and the seabed stayed crisp to the horizon. Real seawater
// is a strong, STRONGLY WAVELENGTH-DEPENDENT absorber - red is gone within a few
// metres, green lasts tens of metres, blue hundreds - which is the entire reason
// the deep sea looks the way it does. Beer-Lambert with per-channel
// coefficients reproduces it for the cost of one exp per channel.
//
// Scaled by camera.light5_cone_inner.y, which is zero unless the camera is
// submerged AND non-zero only as far as the Settings "Underwater clarity"
// slider allows - the operator explicitly wants to keep the see-forever mode
// for finding places like Challenger Deep, so this is a dial, not a switch.
const WATER_EXT_R: f32 = 0.115;
const WATER_EXT_G: f32 = 0.042;
const WATER_EXT_B: f32 = 0.021;

fn underwater_apply(color_in: vec3<f32>, world_pos: vec3<f32>) -> vec3<f32> {
    let ext = camera.light5_cone_inner.y;
    if (ext <= 1.0e-4) {
        return color_in;
    }
    // ── PER-RAY SUBMERGED PATH (v0.1061) ──
    // Operator: "Can we do like a strip across the screen where we're
    // transitioning from underwater to above water and vice versa? Right now
    // that edge point just kind of flips. I don't really see the water passing
    // in front of me until the camera suddenly just goes blue."
    //
    // The flip was structural: "am I underwater" was ONE point test on the
    // camera position, in Rust, switching extinction for the entire screen at
    // once. Physically the question is not about the camera at all - it is how
    // much of the path from the eye to THIS pixel lies below the surface. Answer
    // that per pixel and the over-under photograph falls out for free: rays
    // going up through the meniscus stay clear, rays going down are extinguished
    // over their submerged length, and the water surface geometry between them
    // draws the strip.
    //
    // The sea sphere (centre + sea-level radius, in render space) arrives in
    // light6_cone_inner. Depth is signed: positive above water, negative below.
    let sea_c = camera.light6_cone_inner.xyz;
    let sea_r = camera.light6_cone_inner.w;
    if (sea_r <= 1.0) {
        return color_in;
    }
    let h_cam = length(camera.view_pos.xyz - sea_c) - sea_r;
    let h_frag = length(world_pos - sea_c) - sea_r;
    let seg = length(world_pos - camera.view_pos.xyz);
    // Fraction of the segment that lies below the surface. Over these ranges
    // the surface is locally flat compared with the planet, so the linear
    // crossing point is exact enough and costs one divide.
    var frac_wet = 0.0;
    if (h_cam <= 0.0 && h_frag <= 0.0) {
        frac_wet = 1.0;
    } else if (h_cam > 0.0 && h_frag > 0.0) {
        frac_wet = 0.0;
    } else {
        // Crossing: the wet share is whichever endpoint is submerged.
        frac_wet = clamp(min(h_cam, h_frag) / (min(h_cam, h_frag) - max(h_cam, h_frag)), 0.0, 1.0);
    }
    let d = seg * frac_wet;
    if (d <= 0.01) {
        return color_in;
    }
    let sigma = vec3<f32>(WATER_EXT_R, WATER_EXT_G, WATER_EXT_B) * ext;
    let t = exp(-sigma * d);
    // In-scattered ambient of the water column: what remains once everything
    // else is absorbed. Blue-green and dark - what a distant seabed dissolves
    // INTO rather than staying visible against.
    // SCALED BY DAYLIGHT (BUG-057 #3): this was a bare constant, so the
    // through-water half of a coastal frame was bit-identical at noon and
    // midnight - the beach glowed all night. In-scatter IS sunlight that
    // scattered; no sun, no glow. sun_direction.w = 2.5 * day since v0.1083.
    let sun_day = clamp(camera.sun_direction.w * 0.4, 0.0, 1.0);
    let inscatter = vec3<f32>(0.008, 0.030, 0.055) * sun_day;
    return color_in * t + inscatter * (vec3<f32>(1.0) - t);
}

// ── Planetary ocean shell (material type 16, v0.876 real-water Stage 1) ──
//
// The translucent water-surface sphere drawn over connected-ocean regions;
// the terrain beneath it now renders TRUE bathymetry (the ocean split).
// Geometry arrives vertex-displaced by ocean_wave_height (see vs_main);
// shading reuses the v0.816 close-range machinery verbatim: wave-perturbed
// normals from water_wave_gradient, Fresnel sky mirror + moving sun glitter
// from water_shade. Every wave term anti-alias fades with distance, so from
// orbit the shell is a smooth deep-blue sphere -- visually the same sea the
// clamped terrain used to draw, which is the regression bar.
// ── Waterline feather width from the patch's own resolution (v0.1050) ──
// The shell's shore fade reads a per-VERTEX baked seafloor depth, so its
// accuracy is the patch's vertex spacing. On a coarse patch every vertex of a
// small island can sample open-ocean depth, and the interpolated field then
// claims there is water on dry land: the operator's "terrain that's obviously
// land is still showing that blue underwater ground texture" - a partial blue
// veil (alpha ~0.5 at a fake 0.5 m of depth), worst over islands seen from
// altitude, which is exactly where cells are hundreds of metres wide.
//
// So scale the feather's upper edge by the measured cell size (uv.y, baked by
// the shell builder): a 1 m cell keeps today's 1 m feather, while a 200 m cell
// demands ~11 m of depth before it will paint water. Coarse patches stop
// asserting a shoreline they cannot resolve, and fine near-shore patches -
// where the beach is actually rendered and looked at - are untouched.
fn water_shore_feather_top(cell_m: f32) -> f32 {
    return 1.0 + cell_m * 0.05;
}

fn ocean_shell(in: VertexOutput) -> vec4<f32> {
    // Planet-local frame via the same center + inverse-rotation trick as
    // the planet imagery branch (material.base_color.xyz = planet center in
    // render space; transpose(normal_matrix) = model^-1).
    let inv_model = transpose(obj_normal_matrix());
    let dir_world = in.world_position - material.base_color.xyz;
    let r_render = max(length(dir_world), 1.0);
    let n_geo = dir_world / r_render;
    let dir = normalize((inv_model * vec4<f32>(dir_world, 0.0)).xyz);
    let p_local = dir * r_render;
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);
    let t = camera.sun_color.w;
    let dist_frag = max(length(camera.view_pos.xyz - in.world_position), 1.0);
    let footprint = max(dist_frag * PLANET_PIXEL_ANGLE, 0.001);
    // Per-vertex WATER DEPTH (v0.917, shoreline increment): the shell
    // builder bakes seafloor depth (decimetres) into the packed UV, and
    // linear interpolation of that scalar IS linear depth - a smooth
    // shoreline gradient with no depth-texture pass.
    var depth_m = f32(u32(round(max(in.uv.x, 0.0))) & 65535u) / 10.0;
    // This patch's measured vertex spacing (v0.1049 channel).
    let cell_m = in.uv.y * 65536.0;
    let feather_top = water_shore_feather_top(cell_m);
    // Shore de-terracing (v0.1026, operator: "the beach effect of the
    // water going to the shore looks very blocky"): the baked depth is
    // per-vertex at patch resolution, so the turquoise and waterline
    // bands followed big flat triangles as visible polygon terraces. A
    // smooth noise perturbation in the SHALLOW range breaks the terraces
    // into an organic, irregular waterline (real coasts are not
    // isobath-parallel); deep water is untouched. The shoal wave damping
    // reads the same perturbed depth, so the calm band wanders with it.
    if (depth_m < 14.0) {
        let dn = surface_detail_noise(dir, r_render / 90.0, 1723.0) - 0.5;
        depth_m = max(depth_m + dn * (2.0 + depth_m * 0.3), 0.0);
    }
    // Deep open-ocean body color (linear). The seabed under the shell keeps
    // the graded bathymetry albedo; this is only the water column's own hue.
    var deep = vec3<f32>(0.013, 0.055, 0.11);
    // Shallow water is turquoise: the column is too thin to absorb the
    // seabed's warmth, so mix toward a bright green-blue over the first
    // ~9 m of depth.
    deep = mix(vec3<f32>(0.075, 0.30, 0.30), deep, smoothstep(0.4, 9.0, depth_m));
    // FLAT BACKSTOP shell (v0.1019, params.x = the metallic-slot flag): the
    // coarse deep layer under the wave shell. Body color + regional hue
    // only - no waves, no foam, no texture taps; near-opaque so tears in
    // the displaced shell above read as water, not seafloor. Shore feather
    // still applies via the baked depth.
    if (material.params.x > 0.5) {
        // FLAT BACKSTOP shell (v0.1019, params.x = the metallic-slot flag):
        // the coarse deep layer under the wave shell, seen wherever the
        // displaced surface above does not cover (cross-LOD apertures, and
        // any ocean the 512-leaf water budget could not reach).
        //
        // v0.1045 - THE PALE PLATES (operator: "weird basic simple blue
        // tiles... most prominent when resting at water level", and the
        // dusk/dawn seams): this branch used to invent its OWN lighting -
        // no 1/PI on the body term, its own 0.35/0.44/0.55 sky tint at
        // 0.75 Fresnel, no sun glitter. Under a LOW sun at GRAZING view
        // that reads several times BRIGHTER and greyer than the sea beside
        // it, so every exposed backstop patch became a flat pale tile with
        // hard polygon edges - exactly the artifact, and exactly why it
        // vanished at night (both terms collapse to their floors) and from
        // altitude (the wave shell covers again).
        //
        // Now it is the SAME water shading as the wave shell, with the
        // wave normal replaced by the geometric one: a perfectly calm sea.
        // Whatever shows through now reads as water, not as a tile.
        let bsun_ndl = dot(n_geo, normalize(camera.sun_direction.xyz));
        let brgb = water_shade(
            deep,
            n_geo,
            n_geo,
            view_dir,
            sun_shadow(in.world_position, bsun_ndl),
        );
        let bcos = clamp(dot(n_geo, view_dir), 0.0, 1.0);
        let bt = 1.0 - bcos;
        let bfres = bt * bt * bt;
        // Same alpha law and same ACES tail as the wave shell below, so a
        // backstop pixel and a sea pixel composite identically.
        let balpha =
            clamp(0.93 + 0.07 * bfres, 0.0, 1.0) * smoothstep(0.02, feather_top, depth_m);
        let ba = 2.51;
        let bb = 0.03;
        let bc = 2.43;
        let bd = 0.59;
        let be = 0.14;
        // The BACKSTOP needs the haze too (v0.1054): v0.1053 gave it to the wave
        // shell only, so wherever the coarse deep layer showed through it stayed
        // at full contrast against a hazed sea around it.
        let brgb_aer = underwater_apply(aerial_apply(brgb, in.world_position), in.world_position);
        let bmapped = clamp(
            (brgb_aer * (ba * brgb_aer + vec3<f32>(bb)))
                / (brgb_aer * (bc * brgb_aer + vec3<f32>(bd)) + vec3<f32>(be)),
            vec3<f32>(0.0),
            vec3<f32>(1.0),
        );
        return vec4<f32>(bmapped, balpha);
    }
    // Regional sea variation (v0.902; de-squared v0.906 - the operator saw
    // "very obvious squares"): single low-frequency value noise shows its
    // axis-aligned lattice as rectangular blotches. Three octaves at
    // incommensurate frequencies with ROTATED sampling directions break
    // the grid into organic patches.
    let dir_r1 = normalize(vec3<f32>(
        dir.x * 0.7660 - dir.z * 0.6428,
        dir.y,
        dir.x * 0.6428 + dir.z * 0.7660,
    ));
    let dir_r2 = normalize(vec3<f32>(
        dir.x * 0.1736 + dir.z * 0.9848,
        dir.y,
        -dir.x * 0.9848 + dir.z * 0.1736,
    ));
    let sea_var = surface_detail_noise(dir, r_render / 24000.0, 611.0) * 0.40
        + surface_detail_noise(dir_r1, r_render / 9200.0, 733.0) * 0.30
        + surface_detail_noise(dir_r2, r_render / 3100.0, 857.0) * 0.30;
    let greener = vec3<f32>(0.016, 0.085, 0.105);
    // Wider blend band (0.35..0.85) so hue patches feather instead of
    // stepping.
    deep = mix(deep, greener, smoothstep(0.35, 0.85, sea_var));
    deep = deep * (0.9 + 0.25 * sea_var);
    // STORM SEAS (v0.906, operator: "let's see how very stormy seas would
    // look, white caps if possible"): the same live MODIS weather field
    // the sky draws doubles as sea state - under real storm cloud the
    // water chops up hard and crests break into whitecaps. Calm-clear
    // ocean keeps the v0.902 look.
    // v0.1032: rotate by the same wind-advection angle the sky's MODIS
    // lookup uses (light1_cone_inner.x), so the storm SEA cell stays
    // under its drifting storm CLOUD.
    let sw_dir = cloud_rot_y(dir, camera.light1_cone_inner.x);
    let sw_lon = atan2(-sw_dir.z, sw_dir.x);
    let sw_lat = asin(clamp(sw_dir.y, -1.0, 1.0));
    let sw_uv = vec2<f32>(sw_lon * 0.15915494 + 0.5, 0.5 - sw_lat * 0.31830987);
    let sw = textureSampleLevel(weather_map, albedo_sampler, sw_uv, 0.0).rg;
    // SEA STATE 0..1 (v0.909, operator: "freely switch between calm glassy
    // ... slight ripples ... intense waves from storms"): the max of the
    // LOCAL storm cell in the live weather field and the CPU game-weather
    // wind at the player (fill_color.w pad; showcase {"sea":x} overrides).
    // 0 = glassy mirror, ~0.35 = the classic ripple look, 1 = storm.
    let modis_storm = smoothstep(0.5, 0.9, sw.r * (0.3 + 0.7 * sw.g));
    // Pad >= 1.5 = PINNED at (value - 2), ignoring the MODIS cell (v0.922:
    // the old max() meant a dev {"sea":0} could never calm a storm cell).
    var sea_state = clamp(max(modis_storm, camera.fill_color.w), 0.0, 1.0);
    if (camera.fill_color.w >= 1.5) {
        sea_state = clamp(camera.fill_color.w - 2.0, 0.0, 1.0);
    }
    // Storm water body darkens toward slate (reference: storm seas read
    // dark blue-grey under the cloud deck, not bright blue).
    deep = mix(deep, vec3<f32>(0.012, 0.030, 0.048), sea_state * 0.6);
    // Waves die in the shallows (v0.917): amplitude drains over the last
    // ~7 m of depth, so surf zones read calm-lapping instead of open-sea
    // chop running aground.
    let shoal = 0.2 + 0.8 * smoothstep(0.4, ocean_shoal_top(), depth_m);
    // Storm boost compressed 2.3 -> 1.5 (v0.1017, operator: "white splotches"
    // band): under a live-MODIS storm cell the old boost pushed the summed
    // slopes far past the Fresnel knee, flipping sky-white/body-dark in
    // harsh zebra sheets from any altitude. 1.5x still reads stormy.
    let gscale = (0.55 + 0.95 * sea_var) * mix(0.30, 1.5, sea_state) * shoal;
    let presence = wave_presence(footprint);
    var n_pert = n_geo;
    var foam = 0.0;
    if (presence > 0.001) {
        // Long-swell analytic shading (2000/360/150 m trains only, v0.922).
        var grad = water_wave_gradient(p_local, dir, t, footprint) * gscale;
        // Near-field detail from the tiling wave TEXTURE (v0.922 rework,
        // operator: "the ocean texture still doesn't look good up close...
        // the way we're doing it just isn't cutting it"): mipped random-
        // phase content replaces the fine analytic trains + micro ripples,
        // so close chop has real structure and CANNOT alias into zebra
        // stripes or moire rings - the mip chain clamps its frequency to
        // the screen. Reaches much further than the old 1.5 m/px micro
        // band because mips make distance safe; amplitude still calms far
        // out so the far field stays the approved satellite look.
        var crest = 0.0;
        let tex_reach = 1.0 - smoothstep(4.0, 14.0, footprint);
        // Anchored-domain position, shared by the wave texture and the foam
        // lacework below (small magnitudes near the camera - the same
        // pinned domain the micro ripples use).
        let inv_mw = transpose(obj_normal_matrix());
        let dvw =
            (inv_mw * vec4<f32>(in.world_position - camera.view_pos.xyz, 0.0)).xyz;
        let anchw = vec3<f32>(
            camera.light0_cone_inner.y,
            camera.light0_cone_inner.z,
            camera.light0_cone_inner.w,
        );
        let ptw = anchw + dvw;
        // FFT-ocean mode (v0.1031, water-fft.md increment 2): the tile's
        // SPECTRAL slopes + Jacobian whitecap channel replace the wave
        // texture's normalized detail chop + crest heuristic, so shading
        // agrees with the FFT geometry the VS actually displaced. Slopes
        // are physical (rad/m from i*k*h(k)) - no amplitude renorm.
        if (camera.light0_cone_inner.x > 0.5) {
            if (tex_reach > 0.003) {
                // Both cascades (v0.1040): the 256 m-anchored position
                // and the camera distance for the per-cascade fades.
                let ptw256 = vec3<f32>(
                    camera.light4_cone_inner.x,
                    camera.light4_cone_inner.y,
                    camera.light4_cone_inner.z,
                ) + dvw;
                let fdist = length(in.world_position - camera.view_pos.xyz);
                let f = fft_ocean_shading(ptw, ptw256, dir, fdist);
                grad = grad + f.xyz * (shoal * tex_reach);
                // Jacobian foam is already a 0..1 whitecap factor; feed it
                // through the same crest channel the texture used so the
                // downstream foam window + lacework apply unchanged.
                // No 1.6x amplification any more (v0.1051): the CPU mask is
                // now coverage-targeted to Monahan's law, so scaling it here
                // would just break the coverage it was solved for.
                crest = clamp(f.w, 0.0, 1.0) * tex_reach;
            }
        } else if (tex_reach > 0.003) {
            let det = ocean_tex_gradient(ptw, dir, t, footprint);
            // The texture stores slopes NORMALIZED to full channel range for
            // precision; the physical steepness lives here. Calm seas are
            // near-mirror (~4 deg max tilt), storms chop to ~14 deg - going
            // past that swings the Fresnel between sky-white and dark body
            // and the whole surface strobes white/blue.
            let det_amp = (0.06 + 0.19 * sea_state) * shoal * tex_reach;
            grad = grad + det.xyz * det_amp;
            crest = det.w * tex_reach;
        }
        // Slope soft-clamp (v0.922): six summed octaves could exceed unit
        // slope in storms and FLIP the shaded normal - alternating lit and
        // unlit stripes at wave frequency (the operator's zebra ocean).
        // Compressing the gradient magnitude keeps every normal on the
        // correct hemisphere at any sea state.
        let gl = length(grad);
        grad = grad * (1.0 / (1.0 + 1.35 * gl)); // clamp strengthened with it
        // Whitecaps: crest-masked from the texture's height channel (foam
        // rides actual wave tops now) plus the long-swell steepness term,
        // both sea-state gated. Same hard screen-space reach as before -
        // breaking crests are a ~10 m phenomenon.
        let steep = length(grad) * (1.0 + sea_state * 1.4);
        let foam_reach = 1.0 - smoothstep(2.5, 5.0, footprint);
        let cap_tex = smoothstep(0.55, 0.85, crest);
        // FFT MODE: the Jacobian mask IS the physical whitecap field, solved on
        // the CPU to Monahan's coverage for the live wind. The steepness term
        // below is a trains-era heuristic, and stacking it on top double-counts:
        // once v0.1051 let the spectrum grow 3.5x for a storm, length(grad)
        // cleared the 0.30-0.50 window EVERYWHERE and the max() pinned foam to
        // 1.0 across the whole sea - the operator's white-out. Trust the mask
        // in FFT mode; keep the heuristic exactly as it was for wave trains.
        if (camera.light0_cone_inner.x > 0.5) {
            foam = clamp(crest, 0.0, 1.0) * foam_reach * presence;
        } else {
        foam = max(
            // Window raised 0.20-0.36 -> 0.30-0.50 (v0.1017, operator:
            // "the white on the ocean pulses slowly"): the summed trains
            // BEAT at ~30 s periods, and the old low threshold let that
            // global steepness swell push the whole sea over the foam
            // line in sync. The texture crest channel (local,
            // decorrelated) now carries most of the foam.
            smoothstep(0.30, 0.50, steep) * (0.4 + 0.6 * cap_tex),
            cap_tex * 0.85
        )
            * smoothstep(0.55, 0.95, sea_state)
            * foam_reach
            * presence;
        }
        // Foam LACEWORK (v0.1018, operator: "the foam texture is very
        // simple that it just kind of looks like white paper"): real foam
        // is strands and holes, not a solid sheet. A second wave-texture
        // tap at an incommensurate scale + offset time carves the flat
        // foam into advecting lace: crest-channel strands modulate the
        // alpha so patches read as froth riding the water. One extra
        // textured sample, only where foam actually shows.
        if (foam > 0.01) {
            let lace_det = ocean_tex_gradient(
                ptw * 2.83 + vec3<f32>(17.0, 3.0, 41.0), dir, t * 1.6, footprint);
            let lace = smoothstep(0.28, 0.72, lace_det.w);
            foam = foam * (0.25 + 0.85 * lace);
        }
        let n_pert_local = normalize(dir - grad * presence);
        n_pert = normalize((obj_model() * vec4<f32>(n_pert_local, 0.0)).xyz);
    }
    let wsun_ndl = dot(n_geo, normalize(camera.sun_direction.xyz));
    var rgb = water_shade(
        deep,
        n_geo,
        n_pert,
        view_dir,
        sun_shadow(in.world_position, wsun_ndl),
    );
    // Foam is scattered froth - and froth is DIFFUSE, so it is SUNLIT like
    // everything else (v0.914, operator: "the ocean in night time is
    // bright white, almost like it is glowing" - the old constant foam
    // color ignored the sun entirely). Night foam goes dark with the sea.
    let foam_day = clamp(dot(n_geo, normalize(camera.sun_direction.xyz)), 0.0, 1.0);
    // The 0.015 floor is scaled by daylight too (BUG-057 #5): unscaled it
    // painted a faint glowing surf line along the waterline all night.
    let foam_sun_day = clamp(camera.sun_direction.w * 0.4, 0.0, 1.0);
    let foam_col = vec3<f32>(0.75, 0.81, 0.86)
        * (foam_day * camera.sun_direction.w * 0.42 + 0.015 * foam_sun_day);
    // Shoreline surf (v0.917, operator: "the water to land interface is
    // still behaving very weird"): an animated foam band hugs the beach in
    // the 0.2-2.2 m depth band - waves arriving, breaking, and receding.
    // Along-shore noise breaks the band into patches; the slow depth-phased
    // sine makes it BREATHE toward and away from the sand.
    let surf_reach = 1.0 - smoothstep(4.0, 9.0, footprint);
    if (surf_reach > 0.003) {
        let band = (1.0 - smoothstep(0.25, 2.2, depth_m)) * smoothstep(0.03, 0.25, depth_m);
        let along = surface_detail_noise(dir_r2, r_render / 40.0, 1543.0);
        // Sign flip (v0.1018, operator: "it looks like they're going
        // backwards"): with t*f MINUS depth*k a constant-phase band drifts
        // toward DEEPER water; PLUS makes the surf march shoreward.
        let breathe = 0.5 + 0.5 * sin(6.2831853 * (t * 0.5 + depth_m * 0.65));
        let surf_line = band * (0.35 + 0.65 * breathe) * (0.4 + 0.6 * along)
            * (0.55 + 0.45 * sea_state) * surf_reach;
        foam = max(foam, surf_line);
    }
    rgb = mix(rgb, foam_col, clamp(foam, 0.0, 0.5));
    // Alpha: deep water is near-opaque looking straight down and fully
    // reflective at grazing (Fresnel). A touch under 1.0 near nadir keeps a
    // hint of shallow seabed visible along coasts.
    // Below-surface viewing (v0.1017, water arc): submerged players look UP
    // at the shell, where the outward normal points AWAY from the view -
    // the raw dot goes negative, the old clamp pinned Fresnel to full
    // mirror, and the underside read as an opaque white ceiling. Flipping
    // the effective normal for below-views gives the plausible look: a
    // readable water ceiling overhead (body color) that mirrors toward
    // grazing angles - the cheap cousin of Snell's window.
    var n_shade = n_pert;
    if (dot(n_geo, view_dir) < 0.0) {
        n_shade = -n_pert;
    }
    let cos_v = clamp(dot(n_shade, view_dir), 0.0, 1.0);
    let tt = 1.0 - cos_v;
    let fres = tt * tt * tt;
    // v0.887 (operator: "all the coasts kind of seem to be glowing"): the
    // 0.88 body alpha let the albedo bake's bright shallow-shelf pixels
    // bleed through as a luminous rim hugging every coastline. Near-opaque
    // body; Fresnel still brightens grazing angles.
    // v0.902: opened slightly (0.96 -> 0.93 nadir) so shallow coasts show
    // a hint of the graded seabed - real water is not paint. Still well
    // above the 0.88 that caused the v0.887 glowing-coast regression.
    // Waterline feather (v0.917): the shell fades to fully transparent
    // over the last metre of depth, so the sea EDGE dissolves onto the
    // sand instead of cutting a hard polygon line against the beach.
    let alpha = clamp(0.93 + 0.07 * fres, 0.0, 1.0) * smoothstep(0.02, feather_top, depth_m);
    // Aerial perspective BEFORE the tone map, exactly as the main tail does it
    // (v0.1053) - this is the branch that was missing it entirely.
    let rgb_aer = underwater_apply(aerial_apply(rgb, in.world_position), in.world_position);
    // Same ACES curve as the main pipeline tail (this branch early-returns,
    // mirroring the cloud shell's convention).
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    let mapped = clamp(
        (rgb_aer * (a * rgb_aer + vec3<f32>(b)))
            / (rgb_aer * (c * rgb_aer + vec3<f32>(d)) + vec3<f32>(e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    return vec4<f32>(mapped, alpha);
}

// Terrain terminator window, in cos(angle between the local outward radial and
// the sun). Fully lit by +0.004 (~0.2 deg above the local horizon); fully dark
// by -0.012 (~0.7 deg below), which still leaves room for the genuine alpenglow
// a raised fragment sees past geometric sunset. Tightening LO toward 0 would
// clip mountain-top light; loosening it brings the streak back.
const TERRAIN_TERMINATOR_LO: f32 = -0.012;
const TERRAIN_TERMINATOR_HI: f32 = 0.004;

@fragment
fn fs_main(in: VertexOutput, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    // Route the per-instance data to the obj_* accessors (flat varying;
    // zero for classic draws, the batched patch's translation + fade for
    // terrain-batch draws).
    g_inst_data = in.inst_data;
    // Screen-space derivatives of the world position, taken FIRST - before
    // the Bayer discard below or any non-uniform branch - so they are valid
    // wherever they are later consumed (v0.977: the ground textures rotate
    // these into the pinned domain for textureSampleGrad anisotropy).
    let wp_dx = dpdx(in.world_position);
    let wp_dy = dpdy(in.world_position);
    // Screen-space derivatives of the TEXTURE coordinate, taken here for the
    // same reason and under the same rule (v0.1089, baked bark): they are what
    // `textureSampleGrad` needs to pick a mip level, and textureSampleGrad is
    // the LOD-selecting sample that is legal inside non-uniform control flow -
    // which every material-type branch below is. Meaningless for the material
    // types whose uv carries a packed integer; those never read it.
    let uv_dx = dpdx(in.uv);
    let uv_dy = dpdy(in.uv);
    // LOD crossfade (v0.920): model[0].w carries the per-object fade (see
    // RenderObject::fade). 0 = normal. Positive f = fading IN: keep pixels
    // whose 4x4 Bayer threshold is below f. Negative -f = fading OUT: keep
    // pixels at/above f. A rising patch at t and its falling partner at -t
    // partition the screen per-pixel, so terrain LOD swaps dissolve instead
    // of popping - with opaque depth intact and zero overdraw holes.
    let lod_fade = obj_lod_fade();
    if (lod_fade != 0.0) {
        let px = vec2<u32>(u32(in.clip_position.x), u32(in.clip_position.y));
        // 4x4 Bayer matrix via bit interleaving: thresholds (0.5..15.5)/16.
        let bx = px.x % 4u;
        let by = px.y % 4u;
        let bayer_i = (bx % 2u) * 8u + (by % 2u) * 4u + ((bx / 2u) % 2u) * 2u + (by / 2u) % 2u;
        let b = (f32(bayer_i) + 0.5) / 16.0;
        if (lod_fade > 0.0) {
            if (b >= lod_fade) { discard; }
        } else {
            if (b < -lod_fade) { discard; }
        }
    }
    // var (not let) since v0.907: the ground PBR pass perturbs the terrain
    // normal with the material's normal map before the lighting below.
    var normal = normalize(in.world_normal);
    let view_dir = normalize(camera.view_pos.xyz - in.world_position);

    var albedo = material.base_color.rgb;
    var metallic = material.params.x;
    var roughness = material.params.y;
    let material_type = material.params.z;
    // Sun visibility for THIS fragment, 1 unless a branch knows better. Only
    // the planet-surface branch sets it (v0.1052 terminator gate); everything
    // else - ship interiors, props, the other bodies - is unaffected.
    var sun_gate = 1.0;
    var proc_emissive = vec3<f32>(0.0); // extra emissive from procedural materials (e.g. lava cracks)
    var out_alpha = material.base_color.a; // types below may modulate (atmosphere fresnel)
    // Emissive strength normally rides in params.w -- but material type 12
    // REPURPOSES params.w as the "albedo texture present" flag (v0.811), so
    // the type-12 branch zeroes this to keep planets from self-glowing.
    var emissive_strength = material.params.w;

    // Types 14 + 15 short-circuit the whole PBR surface path: an atmosphere
    // is a participating MEDIUM and a cloud deck is a self-lit coverage
    // field -- neither takes its color from a BRDF. Types >= 15.5 would fall
    // through to the default panel-grid look (none exist yet).
    if (material_type >= 13.5 && material_type < 14.5) {
        return atmosphere_scattering(in.world_position, front_facing);
    }
    if (material_type >= 14.5 && material_type < 15.5) {
        return cloud_layer(in.world_position, front_facing);
    }
    if (material_type >= 15.5 && material_type < 16.5) {
        return ocean_shell(in);
    }
    if (material_type >= 18.5 && material_type < 19.5) {
        // Type 19: TEXTURED MESH (v0.909, the photoscanned-plant path):
        // the material's albedo texture times base_color, alpha-cutout for
        // foliage (the photoscan leaf textures carry alpha), then the
        // normal sun-lit PBR path below. textureSampleLevel 0 because
        // these textures ship one mip level.
        let mesh_tex = textureSampleLevel(albedo_texture, albedo_sampler, in.uv, 0.0);
        if (mesh_tex.a < 0.35) {
            discard;
        }
        albedo = albedo * mesh_tex.rgb;
        emissive_strength = 0.0;
    }
    if (material_type >= 20.5 && material_type < 21.5) {
        // ── Type 21: FOLIAGE CLUSTER CARD (v0.1088) ─────────────────────
        // A quad textured with a baked cluster sprite (dozens of shaped
        // blossoms/leaves per card - the operator's reference-photo
        // redirect: canopy detail lives in TEXTURE, not triangles).
        // UV contract (tree_mesh::encode_card_uv, all three sites must
        // agree exactly): uv.x = 2*ao_code + u01, ao_code 0..63.
        let cc_code = floor(in.uv.x * 0.5);
        let cc_u = in.uv.x - 2.0 * cc_code;
        let cc_ao = cc_code / 63.0;
        let cc_tex = textureSampleLevel(
            albedo_texture, albedo_sampler, vec2<f32>(cc_u, in.uv.y), 0.0);
        if (cc_tex.a < 0.5) {
            discard;
        }
        // Crown-core AO is baked per-station into the code; keep a floor so
        // the deepest cards read as shaded foliage, not holes.
        albedo = albedo * cc_tex.rgb * (0.35 + 0.65 * cc_ao);
        emissive_strength = 0.0;
        // Foliage transmission (the BUG-056 lesson: cards are LEAVES, never
        // plain mesh) - same day-gated backlit term as the type-20 leaf
        // branch, scaled by AO so the crown core does not glow.
        let cc_sun = normalize(camera.sun_direction.xyz);
        let cc_backlit = max(-dot(normal, cc_sun), 0.0);
        let cc_day = clamp(camera.sun_direction.w * 0.4, 0.0, 1.0);
        proc_emissive = proc_emissive
            + albedo * camera.sun_color.rgb
                * (cc_backlit * 0.30) * cc_day * (0.3 + 0.7 * cc_ao);
    }
    if (material_type >= 21.5 && material_type < 22.5) {
        // ── Type 22: BAKED BARK (v0.1089) ───────────────────────────────
        // The wood of a procedural tree, on its own mesh with real cylindrical
        // UVs (renderer::tree_mesh::TreeParts::bark_tube) sampling a
        // per-species baked texture through the SAME per-material albedo slot
        // cluster cards use. No new binding: bindings 11/12 look free in this
        // file but are the atmosphere LUTs in the Rust layout, and a new
        // texture_2d<f32> there would type-match and silently sample a 256x64
        // LUT as bark.
        //
        // WHAT THIS REPLACES. The type-20 bark branch below invents fissures
        // from object-space voronoi noise and then FADES THEM OUT over 2.5-12 m
        // (`detail`) and 0.8-3 m (`micro`), because procedural noise has no mip
        // chain and aliases the instant a trunk minifies. Beyond arm's reach a
        // trunk was therefore one flat colour per face - measured at 0.27 luma
        // levels of cross-trunk detail against a 0.258-level quantization
        // floor. A baked texture HAS mips, so this branch carries NO distance
        // gate on albedo, normal or roughness: trilinear + 8x anisotropic
        // minification is the correct band-limiter, and detail survives to
        // wherever the trunk is still resolvable.
        //
        // CHANNELS (bake_bark_rgba): rgb = species colour x plate field;
        // ALPHA = the same field as a LINEAR height/AO scalar. Alpha is not
        // gamma-encoded in an Rgba8UnormSrgb texture, so it is the one clean
        // linear channel available without a second texture - it carries both
        // the relief this branch differentiates and the roughness break.
        let bk_dim = vec2<f32>(textureDimensions(albedo_texture, 0));
        let bk = textureSampleGrad(albedo_texture, albedo_sampler, in.uv, uv_dx, uv_dy);
        albedo = albedo * bk.rgb;
        metallic = 0.0;
        // params.w is the emissive slot everywhere; type 22 does not repurpose
        // it (its wind class is implied by the type in the vertex stage), but
        // zero it explicitly so a stray non-zero can never make a trunk glow.
        emissive_strength = 0.0;

        // RELIEF. Central differences of the baked height, sampled with the
        // SAME gradients as the base fetch so each tap lands on the same mip
        // level: the height field the taps see is the FILTERED one, so relief
        // softens with distance on its own, physically, instead of by a
        // hand-tuned distance gate.
        //
        // The tap offset is the larger of two texels and ONE SCREEN PIXEL's
        // footprint. Two texels alone is right up close and useless at range:
        // by 8 m a pixel already covers ~40 texels, so three taps 2 texels
        // apart all land inside one filtered texel, the difference is zero and
        // the trunk goes flat. That was measured in a probe capture, not
        // reasoned about.
        let bk_o = max(
            vec2<f32>(2.0, 2.0) / max(bk_dim, vec2<f32>(1.0)),
            abs(uv_dx) + abs(uv_dy),
        );
        let h_l = textureSampleGrad(
            albedo_texture, albedo_sampler,
            in.uv - vec2<f32>(bk_o.x, 0.0), uv_dx, uv_dy).a;
        let h_r = textureSampleGrad(
            albedo_texture, albedo_sampler,
            in.uv + vec2<f32>(bk_o.x, 0.0), uv_dx, uv_dy).a;
        let h_d = textureSampleGrad(
            albedo_texture, albedo_sampler,
            in.uv - vec2<f32>(0.0, bk_o.y), uv_dx, uv_dy).a;
        let h_u = textureSampleGrad(
            albedo_texture, albedo_sampler,
            in.uv + vec2<f32>(0.0, bk_o.y), uv_dx, uv_dy).a;
        // COTANGENT-FRAME TBN (Mikkelsen). The vertex format has no tangent,
        // and adding one would widen every vertex in the engine for bark
        // alone; the screen-space derivatives already taken at the top of this
        // function reconstruct the frame exactly, per pixel, for a few ALU.
        let bk_dp2perp = cross(wp_dy, normal);
        let bk_dp1perp = cross(normal, wp_dx);
        let bk_t = bk_dp2perp * uv_dx.x + bk_dp1perp * uv_dy.x;
        let bk_b = bk_dp2perp * uv_dx.y + bk_dp1perp * uv_dy.y;
        let bk_scale = inverseSqrt(max(max(dot(bk_t, bk_t), dot(bk_b, bk_b)), 1e-20));
        // 0.75 is a deep push: bark IS genuinely rough, and the cracks have to
        // read as grooves that catch a raking sun, not as painted lines.
        let bk_rel = 0.75;
        normal = normalize(
            normal - ((bk_t * (h_r - h_l) + bk_b * (h_u - h_d)) * bk_scale) * bk_rel,
        );

        // ROUGHNESS from the same height: crevices hold dust and torn fibre
        // and scatter widely; ridge crests are worn smooth by weather. This is
        // what makes the specular response VARY across a trunk instead of
        // banding uniformly, which is the other half of reading as bark.
        roughness = clamp(0.98 - 0.30 * bk.a, 0.55, 0.99);
        // Contact-scale ambient occlusion, straight off the height channel.
        // Multiplying albedo (rather than an ambient-only term) is deliberate
        // and stated: it darkens sun and sky identically, so the pattern is
        // visible in both the lit and the shaded thirds of a trunk and cannot
        // deepen on the sunlit side. The trade is that a separately-controlled
        // ambient-only AO would need a second channel, i.e. a second texture.
        albedo = albedo * (0.72 + 0.28 * bk.a);
    }
    if (material_type >= 17.5 && material_type < 18.5) {
        // Type 18: GAS GIANT bands (v0.905). Latitude-ramp palettes warped
        // by noise, hardcoded per giant (params.w = 0 jupiter, 1 saturn,
        // 2 uranus, 3 neptune - also finally un-ochres the ice giants).
        // Falls through to the shared sun-lit path, so the day/night
        // terminator and eclipse shading come free.
        let gg_center = obj_model()[3].xyz;
        let gg_p = normalize(in.world_position - gg_center);
        let gg_lat = clamp(gg_p.y, -1.0, 1.0);
        let gg_lon = atan2(-gg_p.z, gg_p.x);
        let wob = (value_noise(vec2<f32>(gg_lon * 3.0, gg_lat * 6.0)) - 0.5) * 0.12
            + (value_noise(vec2<f32>(gg_lon * 9.0, gg_lat * 18.0)) - 0.5) * 0.05;
        let band = gg_lat + wob * (1.0 - abs(gg_lat));
        let giant = material.params.w;
        var gg_col: vec3<f32>;
        if (giant < 0.5) {
            // Jupiter: ochre/cream belts + rust zones + the Great Red Spot.
            let t = sin(band * 18.0) * 0.5 + 0.5;
            let t2 = sin(band * 7.0 + 1.7) * 0.5 + 0.5;
            gg_col = mix(vec3<f32>(0.76, 0.62, 0.44), vec3<f32>(0.93, 0.86, 0.72), t);
            gg_col = mix(gg_col, vec3<f32>(0.62, 0.40, 0.28), t2 * 0.35);
            let sy = (gg_lat + 0.35) * 9.0;
            let sx = sin((gg_lon - 1.2) * 0.5) * 6.0;
            let spot = exp(-(sy * sy + sx * sx));
            gg_col = mix(gg_col, vec3<f32>(0.72, 0.32, 0.20), spot * 0.85);
        } else if (giant < 1.5) {
            // Saturn: pale gold, soft wide bands.
            let t = sin(band * 14.0) * 0.5 + 0.5;
            gg_col = mix(vec3<f32>(0.82, 0.72, 0.52), vec3<f32>(0.93, 0.87, 0.70), t);
        } else if (giant < 2.5) {
            // Uranus: near-featureless cyan.
            let t = sin(band * 6.0) * 0.5 + 0.5;
            gg_col = mix(vec3<f32>(0.56, 0.78, 0.82), vec3<f32>(0.62, 0.83, 0.86), t * 0.5);
        } else {
            // Neptune: deep azure, faint bands, a dark storm oval.
            let t = sin(band * 8.0) * 0.5 + 0.5;
            gg_col = mix(vec3<f32>(0.15, 0.29, 0.62), vec3<f32>(0.24, 0.42, 0.75), t);
            let ny = (gg_lat - 0.2) * 10.0;
            let nx = sin((gg_lon + 0.6) * 0.5) * 7.0;
            let nspot = exp(-(ny * ny + nx * nx));
            gg_col = mix(gg_col, vec3<f32>(0.10, 0.18, 0.42), nspot * 0.7);
        }
        albedo = gg_col;
        metallic = 0.0;
        roughness = 0.9;
        emissive_strength = 0.0;
    } else if (material_type >= 16.5 && material_type < 17.5) {
        // Type 17: RADIAL GLOW (v0.886, the sun's corona halo). Drawn on an
        // oversized sphere; brightness falls off with the view ray's impact
        // parameter b (distance of the ray from the sphere center, 0 at the
        // disc center, 1 at the silhouette), so the glow is center-bright
        // and melts softly into space - no more hard-edged white blob.
        // base_color.rgb = glow tint, .a = peak alpha, params.w = intensity.
        let center = obj_model()[3].xyz;
        let radius = length(obj_model()[0].xyz);
        let cam = camera.view_pos.xyz;
        let vdir = normalize(in.world_position - cam);
        let to_c = center - cam;
        let b = length(cross(to_c, vdir)) / max(radius, 1.0e-3);
        let g = pow(clamp(1.0 - b * b, 0.0, 1.0), 1.5);
        let col = material.base_color.rgb * (g * material.params.w);
        // Same ACES tail as the other early-return shells.
        let ta = 2.51; let tb = 0.03; let tc = 2.43; let td = 0.59; let te = 0.14;
        let mapped = clamp(
            (col * (ta * col + vec3<f32>(tb))) / (col * (tc * col + vec3<f32>(td)) + vec3<f32>(te)),
            vec3<f32>(0.0), vec3<f32>(1.0));
        return vec4<f32>(mapped, g * material.base_color.a);
    }

    // Apply procedural material based on type:
    //   0 = Panel grid (walls, floors)    4 = Glass            8 = Crystal
    //   1 = Brushed metal                 5 = Ice              9 = Rust/Corroded
    //   2 = Concrete                      6 = Water surface   10 = Moss/Growth
    //   3 = Wood                          7 = Leather         11 = Lava
    //  12 = Planet surface (per-pixel imagery when params.w > 0.5, else per-face
    //       color + water flag packed in UV; ocean sun glint either way)
    //  13 = Atmosphere shell (fresnel limb tint -- the pre-v0.807 fallback)
    //  14 = Atmosphere shell (analytic single scattering -- handled above)
    //  15 = Cloud layer (animated procedural deck -- handled above)
    if material_type < 0.5 {
        // Type 0: Default panel grid (walls, floors)
        if metallic < 0.1 && roughness > 0.3 {
            let panel = grid_pattern(in.world_position, normal);
            albedo = albedo * mix(0.65, 1.0, panel);
            roughness = mix(roughness + 0.1, roughness, panel);
        }
    } else if material_type < 1.5 {
        // Type 1: Brushed metal (metallic surfaces)
        let scratch = brushed_metal(in.world_position, normal);
        albedo = albedo * scratch;
        roughness = roughness + (1.0 - scratch) * 0.15;
    } else if material_type < 2.5 {
        // Type 2: Concrete
        albedo = concrete_pattern(in.world_position, normal) * albedo * 2.0;
        roughness = roughness + fbm(in.world_position.xz * 5.0) * 0.1;
    } else if material_type < 3.5 {
        // Type 3: Wood
        albedo = wood_pattern(in.world_position, normal);
        roughness = 0.5 + value_noise(in.world_position.xz * 10.0) * 0.2;
        metallic = 0.0;
    } else if material_type < 4.5 {
        // Type 4: Glass -- high reflectivity via Fresnel boost, subtle color shift
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 3.0);
        albedo = mix(albedo * 0.15, vec3<f32>(0.8, 0.9, 1.0), fresnel * 0.6);
        metallic = 0.1;
        roughness = 0.05 + value_noise(triplanar_uv(in.world_position, normal) * 20.0) * 0.03;
    } else if material_type < 5.5 {
        // Type 5: Ice -- blue-white tint, wrap lighting approx, crystalline noise
        let uv = triplanar_uv(in.world_position, normal);
        let crystal = voronoi(uv * 8.0);
        let wrap = dot(normal, normalize(camera.sun_direction.xyz)) * 0.5 + 0.5; // wrap lighting for SSS
        albedo = mix(vec3<f32>(0.6, 0.8, 1.0), vec3<f32>(0.95, 0.98, 1.0), crystal) * (0.7 + wrap * 0.3);
        roughness = 0.1 + crystal * 0.2;
        metallic = 0.05;
    } else if material_type < 6.5 {
        // Type 6: Water surface -- animated wave normals, blue-green, foam at shallow angles
        let uv = in.world_position.xz;
        let t = in.world_position.x * 0.01; // pseudo-time from position for static shader
        let wave = fbm(uv * 2.0 + vec2<f32>(t * 3.0, t * 1.7)) * 0.5;
        let foam = smoothstep(0.35, 0.5, wave);
        albedo = mix(vec3<f32>(0.02, 0.15, 0.2), vec3<f32>(0.05, 0.3, 0.35), wave);
        albedo = mix(albedo, vec3<f32>(0.8, 0.9, 0.95), foam * 0.6);
        roughness = mix(0.05, 0.6, foam);
        metallic = 0.02;
    } else if material_type < 7.5 {
        // Type 7: Leather -- Voronoi pore pattern, warm brown tones
        let uv = triplanar_uv(in.world_position, normal);
        let pores = voronoi(uv * 15.0);
        let coarse = fbm(uv * 4.0) * 0.15;
        albedo = mix(vec3<f32>(0.25, 0.13, 0.06), vec3<f32>(0.45, 0.28, 0.14), pores + coarse);
        roughness = 0.5 + (1.0 - pores) * 0.25;
        metallic = 0.0;
    } else if material_type < 8.5 {
        // Type 8: Crystal -- faceted sharp noise, prismatic color from view angle, high metallic
        let uv = triplanar_uv(in.world_position, normal);
        let facets = voronoi(uv * 12.0);
        let angle = dot(normal, view_dir);
        let prism = vec3<f32>(
            smoothstep(0.3, 0.7, sin(angle * 12.0) * 0.5 + 0.5),
            smoothstep(0.3, 0.7, sin(angle * 12.0 + 2.094) * 0.5 + 0.5),
            smoothstep(0.3, 0.7, sin(angle * 12.0 + 4.189) * 0.5 + 0.5)
        );
        albedo = mix(albedo * 0.3, prism, 0.7) * (0.6 + facets * 0.4);
        roughness = 0.02 + (1.0 - facets) * 0.08;
        metallic = 0.9;
    } else if material_type < 9.5 {
        // Type 9: Rust/Corroded -- FBM-driven orange-brown patches over base metal
        let uv = triplanar_uv(in.world_position, normal);
        let rust_mask = smoothstep(0.35, 0.65, fbm(uv * 3.0));
        let rust_color = vec3<f32>(0.5, 0.2, 0.05) + value_noise(uv * 20.0) * 0.1;
        albedo = mix(albedo, rust_color, rust_mask);
        roughness = mix(roughness, 0.85, rust_mask);
        metallic = mix(metallic, 0.1, rust_mask);
    } else if material_type < 10.5 {
        // Type 10: Moss/Growth -- green patches on upward/north-facing surfaces (world-space)
        let uv = in.world_position.xz;
        let up_factor = smoothstep(0.3, 0.8, normal.y); // grows on tops
        let coverage = smoothstep(0.3, 0.6, fbm(uv * 2.5)) * up_factor;
        let moss_color = vec3<f32>(0.15, 0.35, 0.08) + value_noise(uv * 12.0) * 0.08;
        albedo = mix(albedo, moss_color, coverage);
        roughness = mix(roughness, 0.9, coverage);
        metallic = mix(metallic, 0.0, coverage);
    } else if material_type < 11.5 {
        // Type 11: Lava -- black rock with glowing orange cracks, emissive in veins
        let uv = triplanar_uv(in.world_position, normal);
        let cracks = 1.0 - smoothstep(0.0, 0.12, voronoi(uv * 5.0));
        let heat = cracks * (0.7 + value_noise(uv * 8.0) * 0.3);
        albedo = mix(vec3<f32>(0.05, 0.04, 0.03), vec3<f32>(1.0, 0.35, 0.0), heat);
        proc_emissive = vec3<f32>(1.0, 0.3, 0.0) * heat * 3.0; // glowing cracks
        roughness = mix(0.9, 0.3, heat);
        metallic = 0.0;
    } else if material_type < 12.5 {
        // Type 12: Planet surface (v0.763) -- per-face color packed into the UV
        // channel by Mesh::from_planet_surface / terrain::planet_surface::
        // pack_color_to_uv. uv.x holds two 8-bit channels plus a water flag as
        // one exact integer (water*65536 + round(r*255)*256 + round(g*255),
        // max 131071 -- well inside f32's 2^24 exact-integer range); uv.y
        // holds blue as a plain float. All three corners of a flat-shaded face
        // carry the SAME uv, so linear interpolation leaves the packed integer
        // intact. Keep the decode in sync with terrain::planet_surface::
        // unpack_uv_to_color (unit-tested).
        //
        // Local-horizon gate for this fragment (v0.1052): base_color.xyz is
        // the planet centre in render space for this type, so the outward
        // radial is exact regardless of what the normal map did to `normal`.
        {
            let rad_w = in.world_position - material.base_color.xyz;
            let rl = max(length(rad_w), 1.0);
            let mu_geo = dot(rad_w / rl, normalize(camera.sun_direction.xyz));
            sun_gate = smoothstep(TERRAIN_TERMINATOR_LO, TERRAIN_TERMINATOR_HI, mu_geo);
        }
        // params.w REPURPOSED for this type as a BIT FIELD (v0.816; a
        // single texture flag since v0.811): bit 0 = a baked per-pixel
        // albedo texture is bound at group 3 (replacing the per-face color
        // mosaic with smooth imagery), bit 1 = Settings > Graphics >
        // Planets "Surface detail" (the ocean waves + land micro-texture
        // below). lib.rs rewrites the value every frame, so the toggle
        // applies live. It never doubles as emissive here:
        emissive_strength = 0.0;
        // ── Sprite tree cards (v0.961, billboard bake increment 2) ──
        // uv.x < -0.5 marks a card textured from the baked tree atlas
        // (group 3 binding 14): |uv.x| = (1 + tile) + u01 * 0.5 (the small
        // base keeps u01 interpolation sub-texel), uv.y = v01. v0.1083: v01
        // spans the baked FRAME (0 = its bottom edge, 1 = its top), which is
        // square on max(width, height) of the tree and so is NOT the tree's
        // own height for a wide crown - the CPU emitter sizes and drops the
        // quad from the tile's footprint. Lighting normal is the interpolated
        // radial up, same as the legacy colored cards. params.w bit 2 = atlas
        // resident; until the bake lands the card shades flat conifer green
        // (never invisible). The 6x8 grid below is compile-time in BOTH
        // places: renderer::tree_mesh::tests::atlas_tile_constants_match_the_shader
        // fails if these literals drift from billboard_bake::ATLAS_COLS/ROWS.
        if (in.uv.x < -0.5) {
            let card_dist = length(camera.view_pos.xyz - in.world_position);
            // Same LOD window as legacy cards: models own the near field,
            // the far cutoff ends the card stage.
            if (card_dist < shadow_u.params.w || card_dist > shadow_u.params2.x) {
                discard;
            }
            let pw_bits_card = u32(round(max(material.params.w, 0.0)));
            if ((pw_bits_card & 4u) != 0u) {
                let a_enc = -in.uv.x;
                let tile = clamp(u32(floor(a_enc)) - 1u, 0u, 47u);
                let u01 = clamp(fract(a_enc) * 2.0, 0.0, 1.0);
                let v01 = clamp(in.uv.y, 0.0, 1.0);
                let tuv = vec2<f32>(
                    (f32(tile % 6u) + u01) / 6.0,
                    (f32(tile / 6u) + (1.0 - v01)) / 8.0,
                );
                let spr = textureSampleLevel(tree_atlas_tex, albedo_sampler, tuv, 0.0);
                if (spr.a < 0.5) {
                    discard;
                }
                albedo = spr.rgb;
            } else {
                albedo = vec3<f32>(0.10, 0.16, 0.07);
            }
            roughness = 0.9;
            metallic = 0.0;
        } else {
        let packed = u32(round(max(in.pack.x, 0.0)));
        let pr = f32((packed >> 8u) & 255u) / 255.0;
        let pg = f32(packed & 255u) / 255.0;
        // Tree-card LOD swap (v0.912, operator: "when it switches to the
        // high poly model it should hide the lower LOD panel tree"): bit 17
        // marks tree silhouette cards; within the tree-model radius (poked
        // into shadow_u.params.w, 0 when the feature is off) the real 3D
        // conifer stands here, so the card yields entirely.
        if ((packed & 131072u) != 0u) {
            let card_dist = length(camera.view_pos.xyz - in.world_position);
            // Inside the hide radius the 3D models own the tree; beyond
            // the far cutoff (v0.924 "Tree silhouette distance" slider)
            // the card stage ends entirely.
            if (card_dist < shadow_u.params.w || card_dist > shadow_u.params2.x) {
                discard;
            }
        }
        // (v0.1091: the bit-18 grass-card distance dissolve that used to sit
        // here is GONE, along with the baked cards it dissolved. Grass is an
        // instanced strand layer now - material type 23 below - whose density
        // ramps to zero with distance on the CPU, so there is no hard field
        // edge for a dither to hide. Nothing writes bit 18 any more; do not
        // re-use it without reading the note in planet_surface.rs.)
        let pw_bits = u32(round(max(material.params.w, 0.0)));
        let has_tex = (pw_bits & 1u) != 0u;
        let detail_on = (pw_bits & 2u) != 0u;
        // Planet-local frame pieces, filled on the textured path and reused
        // by the detail effects: the unit direction (equirect UV + land
        // noise domain), the local position in METRES (wave phases -- the
        // render-space radius converts the unit direction back to metric),
        // and that radius itself (converts wavelengths in metres to angular
        // noise frequencies).
        var dir = vec3<f32>(0.0, 0.0, 1.0);
        var p_local = vec3<f32>(0.0);
        var r_render = 1.0;
        if (has_tex) {
            // Per-pixel imagery path (v0.811). base_color.xyz is REPURPOSED
            // as the planet CENTER in render space (lib.rs updates it every
            // frame -- the floating origin moves it), because the chunked
            // patch meshes are anchored at their own patch centers, so
            // obj_model()[3] is NOT the planet center for them the way it
            // is for the uniform sphere. From the center, the planet-local
            // unit direction is exact for BOTH mesh paths:
            //   dir_world = fragment - center        (world space)
            //   dir_local = model^-1 * dir_world     (w=0: rotation only)
            // transpose(obj_normal_matrix()) IS model.inverse() exactly
            // (normal_matrix is inverse-transpose -- same trick as the
            // type-15 cloud shell), and any uniform scale in it washes out
            // in the normalize. This rides the planet's spin by
            // construction: the imagery is pinned to the rotating body.
            let inv_model = transpose(obj_normal_matrix());
            let dir_world = in.world_position - material.base_color.xyz;
            dir = normalize((inv_model * vec4<f32>(dir_world, 0.0)).xyz);
            // Planet-local metric frame for the wave math: |dir_world| is
            // the fragment's render-space (= metre) distance from the
            // center, so dir * that IS the local position in metres --
            // inv_model's inverse scale never enters (it would land the
            // point in unit-sphere units).
            r_render = max(length(dir_world), 1.0);
            p_local = dir * r_render;
            // Equirectangular UV with the SAME handedness as terrain::
            // planet_heightmap::dir_to_latlon_deg (east = -z; +Y = north),
            // and the same registration: u = (lon+180)/360 puts texel
            // centers where the CPU sampler's cell centers are. The sampler
            // wraps u (antimeridian) and clamps v (poles), mirroring the
            // CPU grid's edge policy. textureSampleLevel (level 0) because
            // implicit-derivative sampling would smear a full-width texture
            // fetch across the u = 1 -> 0 seam.
            let lon = atan2(-dir.z, dir.x);
            let lat = asin(clamp(dir.y, -1.0, 1.0));
            let eq_uv = vec2<f32>(lon * 0.15915494 + 0.5, 0.5 - lat * 0.31830987);
            // Grading (ocean floor / land gain / sea ice) is baked into the
            // texture; the sRGB view decodes to linear on sample. No
            // base_color tint here -- that slot carries the center.
            albedo = textureSampleLevel(albedo_texture, albedo_sampler, eq_uv, 0.0).rgb;
        } else {
            // Fallback: the per-face packed color (classifier planets, or a
            // planet whose imagery failed to bake).
            albedo = vec3<f32>(pr, pg, in.pack.y) * material.base_color.rgb;
        }
        // Pixel footprint on the surface (metres per pixel), the analytic
        // anti-alias estimate every detail octave fades against (see the
        // PLANET_PIXEL_ANGLE block above -- no derivatives needed).
        let dist_frag = length(camera.view_pos.xyz - in.world_position);
        let footprint = max(dist_frag * PLANET_PIXEL_ANGLE, 0.001);
        let is_water = (packed & 65536u) != 0u;
        // Land close-range detail (v0.816): multiplicative luminance
        // variation under the photo -- orbit view identical (every octave
        // fades to zero there), descent keeps revealing structure instead
        // of bilinear blur. Textured path only: the per-face fallback has
        // no planet-local frame to sample in.
        if (has_tex && detail_on && !is_water) {
            // Raw imagery BEFORE any detail modulation: the material
            // classifier below reads the photo's own color, not the
            // noise-modulated result.
            let img = albedo;
            albedo = albedo * land_detail_factor(dir, r_render, footprint);
            // Sub-8 m micro texture (v0.902, operator: "textures for the
            // land... as real as possible"): three camera-relative octaves
            // (2 m / 0.8 m / 0.32 m, periods 32/80/200 cells inside the
            // 64 m anchor modulus) carry ground variation all the way to
            // the player's feet. Fade by footprint like every octave.
            // v0.907: window widened (4 -> 8 m/px, detail-distance scaled)
            // because the ground PBR textures below share this domain and
            // reach further than the noise octaves; each octave's own fade
            // still zeroes it at its correct range.
            let ddk_g = select(1.0, max(camera.view_pos.w, 0.05), camera.view_pos.w > 0.01);
            if (footprint < 8.0 * ddk_g) {
                let inv_m = transpose(obj_normal_matrix());
                let dv =
                    (inv_m * vec4<f32>(in.world_position - camera.view_pos.xyz, 0.0)).xyz;
                let anchor = vec3<f32>(
                    camera.light0_cone_inner.y,
                    camera.light0_cone_inner.z,
                    camera.light0_cone_inner.w,
                );
                let pt = anchor + dv;
                var mf = 0.0;
                mf = mf + 0.10 * detail_octave_fade(2.0, footprint)
                    * (2.0 * micro_noise(pt / 2.0, 32.0) - 1.0);
                mf = mf + 0.08 * detail_octave_fade(0.8, footprint)
                    * (2.0 * micro_noise(pt / 0.8, 80.0) - 1.0);
                mf = mf + 0.07 * detail_octave_fade(0.32, footprint)
                    * (2.0 * micro_noise(pt / 0.32, 200.0) - 1.0);
                albedo = albedo * clamp(1.0 + mf, 0.7, 1.3);
                // ── Ground PBR textures (v0.907, ambientCG CC0) ──
                // Real photoscanned material detail under the NASA photo:
                // grass/dirt/rock/sand picked by slope + the imagery's own
                // color, triplanar-tiled in the same pinned domain, fading
                // back to pure imagery with distance (a 4 m detail octave).
                let gt_presence = detail_octave_fade(4.0, footprint);
                if (gt_presence > 0.003) {
                    // Material weights. Steep slopes read rock; the photo
                    // classifies the flats: green => grass, bright warm =>
                    // sand, else dirt. Snow/ice keeps the pure photo (no
                    // CC0 snow set yet, and bright ice under a dirt tile
                    // would read as mud).
                    let up_w = normalize(in.world_position - material.base_color.xyz);
                    let steep = 1.0 - clamp(dot(normal, up_w), 0.0, 1.0);
                    let lum = dot(img, vec3<f32>(0.299, 0.587, 0.114));
                    let green = smoothstep(1.02, 1.18, img.g / max(max(img.r, img.b), 0.003));
                    let warm = smoothstep(0.02, 0.08, img.r - img.b)
                        * smoothstep(0.18, 0.32, lum);
                    let snowy = smoothstep(0.5, 0.68, lum);
                    let w_rock = smoothstep(0.20, 0.5, steep);
                    let w_grass = green * (1.0 - w_rock);
                    let w_sand = warm * (1.0 - green) * (1.0 - w_rock);
                    let w_dirt = max(1.0 - w_rock - w_grass - w_sand, 0.0);
                    let keep = gt_presence * (1.0 - snowy);
                    // Triplanar plane weights from the radial direction
                    // (smooth on a sphere; the surface normal would swim
                    // on every slope change).
                    let aw = pow(abs(dir), vec3<f32>(4.0));
                    let tw = aw / max(aw.x + aw.y + aw.z, 0.0001);
                    // Pinned-domain gradients: pt = anchor + inv_m*(wp - eye)
                    // with anchor/eye constant per draw, so d(pt) is exactly
                    // inv_m * d(wp) - the fs_main-top derivatives rotated.
                    let g_x = (inv_m * vec4<f32>(wp_dx, 0.0)).xyz;
                    let g_y = (inv_m * vec4<f32>(wp_dy, 0.0)).xyz;
                    var det = vec3<f32>(0.0);
                    if (w_grass > 0.01) {
                        det = det + w_grass * ground_triplanar_grad(0, pt, tw, g_x, g_y);
                    }
                    if (w_dirt > 0.01) {
                        det = det + w_dirt * ground_triplanar_grad(1, pt, tw, g_x, g_y);
                    }
                    if (w_rock > 0.01) {
                        det = det + w_rock * ground_triplanar_grad(2, pt, tw, g_x, g_y);
                    }
                    if (w_sand > 0.01) {
                        det = det + w_sand * ground_triplanar_grad(3, pt, tw, g_x, g_y);
                    }
                    // Detail-albedo modulation: tex * 2 around its neutral
                    // 0.5 grey keeps the photo's large-scale color
                    // authoritative while the texture carries the fine
                    // structure. Neutral fallback layers make this an
                    // exact no-op.
                    let modf = clamp(det * 2.0, vec3<f32>(0.3), vec3<f32>(2.0));
                    albedo = albedo * mix(vec3<f32>(1.0), modf, keep);
                    // Normal perturbation from the DOMINANT material's map.
                    // The tangent basis is an arbitrary-but-smooth frame
                    // around radial up: for rough ground the bump direction
                    // convention doesn't matter, only its consistency.
                    var dom = 0;
                    var wmax = w_grass;
                    if (w_dirt > wmax) { dom = 1; wmax = w_dirt; }
                    if (w_rock > wmax) { dom = 2; wmax = w_rock; }
                    if (w_sand > wmax) { dom = 3; wmax = w_sand; }
                    let nm = ground_triplanar_grad(4 + dom, pt, tw, g_x, g_y) * 2.0 - 1.0;
                    let ref_a = select(
                        vec3<f32>(0.0, 1.0, 0.0),
                        vec3<f32>(1.0, 0.0, 0.0),
                        abs(up_w.y) > 0.9,
                    );
                    let t1 = normalize(cross(up_w, ref_a));
                    let t2 = cross(up_w, t1);
                    normal = normalize(normal + (nm.x * t1 + nm.y * t2) * 0.7 * keep);
                }
            }
        }
        // Cloud GROUND shadows (v0.898 - the deferred item noted in
        // renderer/clouds.rs): sample the SAME blended live+procedural
        // coverage field the sky draws, straight down at this fragment's
        // planet direction, and darken the surface under it. The planet's
        // cloud seed rides camera.light_count.y, the deck coverage .z, and
        // the enable flag .w (poked by render_celestial_onto right after
        // its full uniform write). Applied to albedo so fill/ambient dim
        // with the sun - overcast ground reads flat and grey, like life.
        if (has_tex && camera.light_count.w > 0.5) {
            let cw = cloud_weather(dir, camera.sun_color.w, camera.light_count.y);
            let ca = cloud_alpha_from_field(cw, camera.light_count.z);
            // Ceiling eased 0.5 -> 0.35 (v0.908): the MODIS daily mask keeps
            // most temperate land under SOME deck ~permanently, and a half-
            // light world at noon read as gloom (Europe-noon-darkness fix,
            // part 2 -- part 1 is the land_gain shadow lift at bake).
            // Overcast kills the direct sun but sky-dome ambient keeps real
            // overcast days far above half-dark.
            albedo = albedo * (1.0 - 0.35 * ca);
        }
        // Ocean sun glint (v0.810): every orbital photo has a bright specular
        // spot where the sun mirrors off the sea; without it the ocean reads
        // as painted plastic. Water faces are flagged in bit 16 by the mesh
        // builder (below-sea-level faces of has_water planets -- their
        // normals are the smooth sphere normals, so the lobe is round).
        // Implemented as an explicit Blinn-Phong lobe toward the SUN only,
        // added via proc_emissive AFTER the diffuse path: reusing the
        // material roughness would also glint the fixed cool fill light,
        // painting a second physically bogus hotspot. Land gets nothing.
        //
        // v0.816: up close this single smooth lobe becomes REAL water. Wave
        // presence (the anti-alias fade of the longest wave octave) blends
        // the whole water response from the v0.810 far-field look (presence
        // 0: bit-identical diffuse + glint) to the full wave-perturbed
        // shading in water_shade (presence 1: Fresnel sky mirror, bathymetry
        // body, moving sun sparkle). The diffuse albedo hands its energy to
        // the water term as presence rises so nothing double-counts.
        if (is_water) {
            let sun_l = normalize(camera.sun_direction.xyz);
            let half_v = normalize(view_dir + sun_l);
            // Day gate: the glint fades smoothly at the terminator and never
            // appears on the night side (emissive would otherwise ignore
            // the sun's geometry entirely).
            let day = clamp(dot(normal, sun_l), 0.0, 1.0);
            // Exponent 220 = a ~5 degree half-vector lobe: a glint spot
            // roughly a tenth of the disc across, matching the soft bright
            // patch (sun + surrounding wave glitter) in orbital photos.
            let spec = pow(max(dot(normal, half_v), 0.0), 220.0);
            // 0.7 * sun intensity 2.5 peaks ~1.75 pre-tonemap: bright, not
            // a blown white hole.
            let old_glint =
                camera.sun_color.rgb * camera.sun_direction.w * spec * day * 0.7;
            var presence = 0.0;
            if (has_tex && detail_on) {
                presence = wave_presence(footprint);
                // Sea ice carries the water flag too (below-sea polar faces
                // graded toward cap white) -- fade the waves out as the
                // albedo brightens so pack ice never shades like open sea.
                let lum = max(albedo.r, max(albedo.g, albedo.b));
                presence = presence
                    * (1.0 - smoothstep(WATER_ICE_LUM_LO, WATER_ICE_LUM_HI, lum));
            }
            if (presence > 0.001) {
                // The cloud clock doubles as the wave clock (same
                // documented-pad time slot, app-start-relative seconds).
                let t_wave = camera.sun_color.w;
                let grad = water_wave_gradient(p_local, dir, t_wave, footprint);
                let n_pert_local = normalize(dir - grad);
                let n_pert = normalize(
                    (obj_model() * vec4<f32>(n_pert_local, 0.0)).xyz,
                );
                // Orbital sea (type 12): no shadow term - from orbit the
                // near-field shadow map does not cover the visible ocean.
                let water_rgb = water_shade(albedo, normal, n_pert, view_dir, 1.0);
                proc_emissive = mix(old_glint, water_rgb, presence);
                // Hand the diffuse + ambient energy over to the water term
                // and flatten the residual GGX response as presence rises.
                albedo = albedo * (1.0 - presence);
                roughness = mix(roughness, 1.0, presence);
            } else {
                proc_emissive = old_glint;
            }
        }
        } // close the sprite-card / packed-color split (v0.961)
    } else if (material_type >= 19.5 && material_type < 20.5) {
        // ── Type 20: PROCEDURAL PLANT (v0.1063) ──────────────────────────
        // Same packed-per-face-colour transport as type 12 (written by
        // renderer::plant_mesh), but this is its OWN type for two reasons.
        //
        // 1. Type 12 applies a planet terminator gate that reads
        //    material.base_color.xyz as a PLANET CENTRE. A plant's base_color
        //    is (1,1,1), so plants inherited a terminator through the point
        //    (1,1,1) in world space and roughly half of every garden had
        //    direct sun switched off along a plane that swept with the sun.
        //    That gate simply does not exist here.
        // 2. It gives leaves and fruit somewhere to grow CLOSE-RANGE DETAIL.
        //    The geometry is flat-shaded, one colour per face, with no real
        //    UVs -- so without this every pixel of a leaf is identical, which
        //    is exactly what reads as "flat two-tone". Everything below is
        //    derived from world position plus the face normal, so it needs no
        //    vertex-format change, and it fades out with distance so it costs
        //    nothing past arm's reach.
        let packed = u32(round(max(in.pack.x, 0.0)));
        albedo = vec3<f32>(
            f32((packed >> 8u) & 255u) / 255.0,
            f32(packed & 255u) / 255.0,
            clamp(in.pack.y, 0.0, 1.0),
        );
        metallic = 0.0;
        roughness = 0.9;
        emissive_strength = 0.0;

        // Organ tag from spare UV bits (keep in sync with plant_mesh.rs).
        let is_leaf = (packed & 524288u) != 0u;
        let is_fruit = (packed & 1048576u) != 0u;

        let plant_dist = length(camera.view_pos.xyz - in.world_position);
        // Coarse detail out to 12 m; past that a leaf is a few pixels wide and
        // per-pixel venation would only alias.
        let detail = 1.0 - smoothstep(2.5, 12.0, plant_dist);
        // The fine pass is the expensive one, so it only runs within ~3 m.
        let micro = 1.0 - smoothstep(0.8, 3.0, plant_dist);

        // OBJECT-FIXED material domain (v0.1078, operator: "a blemish on a
        // tree is literally moving"). in.world_position is RENDER space =
        // world - camera, rebased continuously by the floating origin, so any
        // noise sampled there slides across the mesh at exactly camera speed
        // (one vein cell per 1.8 cm of travel at the 55/m frequency).
        // Reconstruct true object space instead: transpose(normal_matrix) is
        // model^-1 for the rotation + uniform-scale transforms plants use
        // (same identity the water branch uses at 00-bindings-vertex:618).
        // Bonus: object +Y IS the trunk axis by construction, so bark
        // fissures run along the bole at every latitude (world-Y "up" only
        // matched the trunk at the poles; at Fuji they ran 55 deg diagonal).
        // Plant pixels only, within the 12 m detail radius.
        let obj_inv = transpose(obj_normal_matrix());
        // PRE-WIND position (v0.1086): subtract the wind displacement the
        // vertex stage applied, so the material domain is the UNDEFORMED
        // object space and the bark/leaf pattern rides the swaying mesh.
        // Sampling from the displaced position anchored the pattern to
        // world space while the geometry moved through it - the operator's
        // "bark shifts instead of being static" / leaf-swim report.
        let obj_p = (obj_inv
            * vec4<f32>(in.world_position - in.wind_offset - obj_model()[3].xyz, 0.0)).xyz;
        let obj_n = normalize((obj_inv * vec4<f32>(normal, 0.0)).xyz);

        if (is_leaf && detail > 0.001) {
            // A leaf-plane coordinate. There is no per-leaf UV to sample, so
            // this projects world position onto the face's dominant axis pair.
            // It is NOT aligned to the midrib -- which is precisely why the
            // vein pattern is RETICULATE (voronoi cell borders) rather than
            // striped. A reticulate net has no preferred axis, so the
            // misalignment is invisible, and real dicot venation IS a net.
            let lp = triplanar_uv(obj_p, obj_n);

            // Primary vein net, plus a finer secondary net inside its cells.
            // voronoi_EDGE, not voronoi: veins are cell BORDERS. Frequencies are
            // per world metre, and a strawberry leaflet is ~0.13 m, so 55/m puts
            // roughly 7 primary areoles across a blade and 170/m fills them with
            // secondary reticulation. That matches real dicot venation density;
            // a much higher number just reads as noise.
            let v1 = voronoi_edge(lp * 55.0);
            var vein = smoothstep(0.16, 0.02, v1);
            if (micro > 0.001) {
                vein = vein + smoothstep(0.10, 0.015, voronoi_edge(lp * 170.0)) * 0.45 * micro;
            }
            vein = clamp(vein, 0.0, 1.0);

            // Veins are paler and a little yellower than the lamina between
            // them, and the lamina itself is never one flat green.
            let mottle = fbm(lp * 70.0);
            albedo = albedo * (1.0 + (mottle - 0.5) * 0.26 * detail);
            albedo = mix(albedo, albedo * 1.45 + vec3<f32>(0.03, 0.05, 0.01), vein * 0.5 * detail);

            // Micro-relief: the lamina puckers between the veins. Perturbing
            // the normal is what stops a blade reading as a flat sticker --
            // it makes the surface catch light unevenly as the camera moves.
            let ref_a = select(
                vec3<f32>(0.0, 1.0, 0.0),
                vec3<f32>(1.0, 0.0, 0.0),
                abs(normal.y) > 0.9,
            );
            let t1 = normalize(cross(normal, ref_a));
            let t2 = cross(normal, t1);
            let bx = fbm(lp * 300.0) - 0.5;
            let by = fbm(lp * 300.0 + vec2<f32>(31.0, 17.0)) - 0.5;
            // The pucker EASES OFF over a vein: vascular tissue is taut, the
            // lamina between it is not. (Genuinely raising the veins would need
            // a gradient of the vein field, i.e. four more voronoi_edge taps
            // per pixel; the albedo and roughness breaks below carry the read
            // for far less.)
            let pucker = (1.0 - vein * 0.6) * 0.5 * detail;
            normal = normalize(normal + (t1 * bx + t2 * by) * pucker);

            // Waxy cuticle. A leaf is not chalk: 0.9 roughness everywhere is
            // most of why the current plants look papery. Wax is smoother
            // between the veins and scuffed along them.
            roughness = mix(0.9, mix(0.30, 0.55, vein), detail);
            // (Subsurface transmission used to live here, scaled by `detail`.
            //  It is a MATERIAL property, not a detail term, so it moved out of
            //  this gate - see the block after this if/else chain, v0.1081.)
        } else if (!is_leaf && !is_fruit && detail > 0.001) {
            // ── BARK (v0.1067) ──
            // Stems previously got NO treatment: one flat colour per face, on a
            // 4-to-8-sided cylinder. Even with smooth normals that reads as
            // plastic tubing. Bark is the other half of making a tree look like
            // a tree, and it is the cheapest half, because bark is essentially
            // vertical fissures at two scales.
            //
            // Coordinates are deliberately ANISOTROPIC: bark runs ALONG the
            // trunk, so the pattern is stretched ~6x vertically. Using the
            // world Y directly (rather than a triplanar pair) is what keeps the
            // fissures vertical no matter which way a branch leans.
            // 2.4:1, not 6:1. A very high ratio drew unbroken floor-to-crown
            // streaks that read as sawn plywood; real bark fissures are
            // elongated but they branch, merge and terminate.
            let bp = vec2<f32>(
                (obj_p.x + obj_p.z) * 2.4,
                obj_p.y * 1.0,
            );
            // Deep fissures: voronoi_edge borders again, stretched into long
            // vertical cracks by the coordinate scaling above.
            let crack = 1.0 - smoothstep(0.0, 0.22, voronoi_edge(bp * 1.6));
            // Fine grain riding on top so the flat areas are not flat.
            let grain = fbm(bp * 7.0);

            // Crevices are darker and rougher; ridges catch a little more light.
            albedo = albedo * (0.72 + 0.42 * grain) * detail + albedo * (1.0 - detail);
            albedo = mix(albedo, albedo * 0.42, crack * 0.75 * detail);
            roughness = mix(0.9, mix(0.78, 0.96, crack), detail);

            // Relief. Bark is genuinely rough, so this pushes harder than the
            // leaf pucker does, and the cracks read as grooves rather than
            // painted lines.
            if (micro > 0.001) {
                let ref_a = select(
                    vec3<f32>(0.0, 1.0, 0.0),
                    vec3<f32>(1.0, 0.0, 0.0),
                    abs(normal.y) > 0.9,
                );
                let t1 = normalize(cross(normal, ref_a));
                let t2 = cross(normal, t1);
                let gx = fbm(bp * 9.0) - 0.5;
                let gy = fbm(bp * 9.0 + vec2<f32>(23.0, 5.0)) - 0.5;
                normal = normalize(normal + (t1 * gx + t2 * gy) * 0.55 * micro);
            }
        } else if (is_fruit && detail > 0.001) {
            // Fruit skin is a taut, waxy surface, not paper: far smoother than
            // a leaf, with a broad blush of colour variation and (up close) a
            // faint pore stipple. Low-poly fruit spheres lean hard on this.
            let fp = triplanar_uv(obj_p, obj_n);
            let blush = fbm(fp * 45.0);
            albedo = albedo * (1.0 + (blush - 0.5) * 0.30 * detail);
            if (micro > 0.001) {
                let ref_a = select(
                    vec3<f32>(0.0, 1.0, 0.0),
                    vec3<f32>(1.0, 0.0, 0.0),
                    abs(normal.y) > 0.9,
                );
                let t1 = normalize(cross(normal, ref_a));
                let t2 = cross(normal, t1);
                let px = fbm(fp * 850.0) - 0.5;
                let py = fbm(fp * 850.0 + vec2<f32>(13.0, 71.0)) - 0.5;
                normal = normalize(normal + (t1 * px + t2 * py) * 0.30 * micro);
            }
            roughness = mix(0.9, 0.24, detail);
        }

        // ── SUBSURFACE TRANSMISSION, UNGATED BY DISTANCE (v0.1081) ──
        // Light that came THROUGH the blade. A backlit leaf glowing green is
        // the single strongest cue that vegetation is alive rather than
        // plastic. Rides on proc_emissive, so the shared BRDF below is
        // untouched.
        //
        // This used to sit INSIDE the `is_leaf && detail > 0.001` block and was
        // additionally scaled by (0.35 + 0.65 * detail), so it faded out by
        // 12 m - while the near-tree models run to the 120 m
        // tree_model_distance and a ground-level frame is mostly stand at
        // 2-40 m. Leaf transmittance is a MATERIAL property (a broadleaf passes
        // 5-12% of visible light); it does not fall off with viewing distance,
        // and the fade was a large part of why the Fuji canopy measured at 6%
        // of sky luminance where a real backlit crown holds 10-50%. Venation,
        // mottle, pucker, wax and `micro` stay gated - those ARE detail.
        if (is_leaf) {
            let sun_l = normalize(camera.sun_direction.xyz);
            // BUG-060 (v0.1095): this lobe was SIGN-INVERTED - it computed
            // dot(V, L - N*d), which peaks FRONT-lit, the opposite of the
            // standard Barre-Brisebois transmission dot(V, -(L + N*d)) that
            // peaks when the sun is BEHIND the leaf. And the coefficient
            // (1.05) exceeded the leaf's own maximum diffuse response by
            // 1.32x - a leaf cannot out-emit its own lit face. Measured: the
            // term supplied 73% of grass luminance and made swards glow over
            // dark dawn terrain. Now: correct lobe, physical coefficient
            // (~0.19x peak diffuse), and MULTIPLIED BY THE SHADOW MAP - a
            // blade in shadow receives no sun to transmit.
            let lt = normalize(sun_l + normal * 0.4);
            let trans = pow(max(dot(view_dir, -lt), 0.0), 1.6);
            let backlit = max(-dot(normal, sun_l), 0.0);
            let leaf_sun_day = clamp(camera.sun_direction.w * 0.4, 0.0, 1.0);
            let leaf_sun_ndl = dot(normal, sun_l);
            let leaf_shadow = sun_shadow(in.world_position, leaf_sun_ndl);
            proc_emissive = proc_emissive
                + albedo * camera.sun_color.rgb
                    * (trans * 0.15 + backlit * 0.06) * leaf_sun_day * leaf_shadow;
        }
    } else if (material_type >= 22.5 && material_type < 23.5) {
        // ── Type 23: GRASS STRAND (v0.1091) ──────────────────────────────
        // The shared tiller mesh carries NO colour of its own - it is drawn
        // once per tiller, so a baked tint would make it one mesh per tiller
        // and destroy the single instanced draw. What its packed channel
        // carries instead is a GREY Beer-Lambert ramp, 0.30 at the crown to
        // 1.00 at the tip: at the LAI a real sward runs, the base of the
        // canopy sits at 20-30% of top-of-canopy irradiance, and a uniformly
        // lit blade is a large part of what reads as a plastic sticker.
        //
        // The tiller's actual albedo (the ground colour it grows in, lifted
        // and jittered, pulled toward straw for its senescent fraction) rides
        // the per-instance channel. Multiply, and the ramp modulates whatever
        // colour that particular tiller is.
        let packed = u32(round(max(in.pack.x, 0.0)));
        let shade = f32((packed >> 8u) & 255u) / 255.0;
        albedo = clamp(in.inst_data.rgb, vec3<f32>(0.0), vec3<f32>(1.0)) * shade;
        metallic = 0.0;
        roughness = 0.92;
        emissive_strength = 0.0;
        // The mesh sets the LEAF organ bit on every face, so a backlit sward
        // glows exactly the way a backlit canopy does. This is the same
        // transmission term the type-20 branch uses; grass is thin tissue
        // with the sun behind it more often than a tree crown is, because it
        // is under your feet and the sun is always somewhere above it.
        if ((packed & 524288u) != 0u) {
            // BUG-060 (v0.1095): same three fixes as the type-20 leaf block -
            // correct backlit lobe (was sign-inverted, peaked FRONT-lit),
            // physical coefficient (1.05 supplied 73% of all grass luminance
            // and made dawn swards glow over dark ground - measured), and the
            // shadow map multiplied in so shaded blades do not transmit.
            let sun_l = normalize(camera.sun_direction.xyz);
            let lt = normalize(sun_l + normal * 0.4);
            let trans = pow(max(dot(view_dir, -lt), 0.0), 1.6);
            let backlit = max(-dot(normal, sun_l), 0.0);
            let leaf_sun_day = clamp(camera.sun_direction.w * 0.4, 0.0, 1.0);
            let g_sun_ndl = dot(normal, sun_l);
            let g_shadow = sun_shadow(in.world_position, g_sun_ndl);
            proc_emissive = proc_emissive
                + albedo * camera.sun_color.rgb
                    * (trans * 0.15 + backlit * 0.06) * leaf_sun_day * g_shadow;
        }
    } else if material_type < 13.5 {
        // Type 13: Atmosphere shell (v0.763) -- fresnel limb tint on a slightly
        // oversized transparent sphere. Nearly invisible looking straight
        // through the center, densest at the grazing-angle limb, so it reads as
        // a thin halo of air hugging the planet. Airless bodies simply never
        // spawn the shell. KEPT as the fallback behind Settings > Graphics >
        // Planets > "Scattering atmosphere" (off = this path): forever-dev
        // A/B reference + a safety hatch if a GPU dislikes the type-14 math.
        let limb = pow(1.0 - abs(dot(normal, view_dir)), 2.0);
        out_alpha = material.base_color.a * limb;
        proc_emissive = albedo * limb * 0.6; // limb stays visible on the night side edge
        roughness = 1.0;
        metallic = 0.0;
    }

    // Fresnel reflectance at normal incidence
    // Dielectrics: 0.04, metals: tinted by albedo
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);

    // Evaluate main directional light (from camera uniforms), attenuated
    // by the sun shadow map (v0.899). Only the SUN term is shadowed; fill
    // and ambient stay, so shadows read as shade, not holes.
    // ── TERRAIN TERMINATOR GATE (v0.1052) ──
    // Operator: "some weird lighting at night in the desert... we've had this
    // lighting bug before." They are right that it recurred, and this is why.
    //
    // The celestial pass (which draws planet terrain) stamps a HARDCODED white
    // sun at intensity 2.5 over the camera uniform - unchanged since v0.451 -
    // so the atmosphere-corrected night sun colour that lib.rs computes never
    // reaches the ground. On top of that, the terrain sun term is
    // dot(MESH normal, sun_dir) with no local-horizon test, and the sand normal
    // map tilts that normal by tens of degrees. So after sunset the flat desert
    // correctly falls to ambient, while the band toward the sunset azimuth -
    // where grazing geometry and normal-map facets present the most surfaces
    // tilted at a sun that is BELOW THE HORIZON - still catches ~25x more light
    // than anything else in frame. That is the bright streak.
    //
    // Every other surface in that pass already has this gate: water and foam
    // test dot(RADIAL normal, sun), and the cloud march tests each sample's own
    // sphere normal. Terrain never got one. The window keeps a small negative
    // tail so genuine alpenglow and mountain-top light survive - a fragment
    // above the local sphere really does see the sun a little past geometric
    // sunset - while ruling out light from a sun a degree or more under.
    let sun_ndl = dot(normal, normalize(camera.sun_direction.xyz));
    var lo = evaluate_light(
        camera.sun_direction.xyz, camera.sun_color.rgb, camera.sun_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0)
        * sun_shadow(in.world_position, sun_ndl)
        * sun_gate;

    // Evaluate fill light (from camera uniforms)
    lo = lo + evaluate_light(
        camera.fill_direction.xyz, camera.fill_color.rgb, camera.fill_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0);

    // Point + spot lights — UNCAPPED (v0.782): the storage buffer holds every
    // scene light; light_count bounds the loop. The early range/attenuation
    // rejection keeps far lights nearly free, so the practical ceiling is GPU
    // fill cost, not a software cap.
    let num_lights = i32(camera.light_count.x);
    // Clustering L1b: when tiling is on, loop ONLY this fragment's tile
    // list (bounded by local overlap, not the global count - what lifts
    // the 256 cap to 2048). The light body below is untouched: only the
    // index it evaluates comes from the tile list.
    let tile_w_px = shadow_u.params2.z;
    let use_tiles = tile_w_px > 0.5;
    var tile_base = 0u;
    var loop_n = num_lights;
    if (use_tiles) {
        let tx = min(u32(in.clip_position.x / tile_w_px), TILE_COLS - 1u);
        let ty = min(u32(in.clip_position.y / shadow_u.params2.w), TILE_ROWS - 1u);
        let tile = ty * TILE_COLS + tx;
        tile_base = tile * TILE_CAP;
        loop_n = i32(min(tile_counts[tile], TILE_CAP));
    }
    for (var j = 0; j < loop_n; j = j + 1) {
        var i = j;
        if (use_tiles) {
            i = i32(tile_indices[tile_base + u32(j)]);
        }
        var light_pos = scene_lights[i].pos_intensity.xyz;
        let intensity = scene_lights[i].pos_intensity.w;
        let light_color = scene_lights[i].color_range.xyz;
        let radius = scene_lights[i].color_range.w;
        let sent = scene_lights[i].spot.w;

        // LINE light (v0.786, sentinel cos_outer == -2.0): the whole segment
        // [pos, spot.xyz] emits -- light each fragment from the CLOSEST point
        // on the segment (capsule-light representative point), so a strip
        // washes the full wall instead of pooling at one point. Rust mirror +
        // tests: light::line_light_closest_point.
        if (sent < -1.5) {
            let a = light_pos;
            let b = scene_lights[i].spot.xyz;
            let ab = b - a;
            let t = clamp(dot(in.world_position - a, ab) / max(dot(ab, ab), 1e-6), 0.0, 1.0);
            light_pos = a + ab * t;
        }

        let to_light = light_pos - in.world_position;
        let dist = length(to_light);

        // Cheap reject: outside the light's range, contribution is exactly 0
        // (the linear range window below hits zero at dist == radius).
        if (dist >= radius) { continue; }

        let light_dir = to_light / max(dist, 0.001);

        // Attenuation: inverse square with radius falloff
        var attenuation = intensity / (1.0 + dist * dist) * max(1.0 - dist / max(radius, 0.001), 0.0);

        // Spot cone (v0.639): cos_outer == -1.0 is the Point/Bar sentinel, so this only narrows
        // an actual spot light -- zero extra cost/behavior change for every other light.
        let spot = scene_lights[i].spot;
        let cos_outer = spot.w;
        if (cos_outer > -1.0) {
            let cos_inner = scene_lights[i].cone_inner.x;
            // spot.xyz is the aim direction in the light-to-fragment sense; -light_dir (which
            // points fragment-to-light) flips to the same sense for the dot product.
            let cos_angle = dot(normalize(spot.xyz), -light_dir);
            attenuation = attenuation * smoothstep(cos_outer, cos_inner, cos_angle);
        }

        if (attenuation > 0.001) {
            lo = lo + evaluate_light(light_dir, light_color, attenuation, normal, view_dir, albedo, metallic, roughness, f0);
        }
    }

    // Ambient (near-zero so space is truly black and the sun is the only
    // light source). A thin floor prevents absolute black so unlit faces
    // still have a subtle silhouette against the starfield instead of
    // vanishing into artefacts from tone mapping.
    let ambient = albedo * vec3<f32>(0.005, 0.005, 0.006);

    var color = ambient + lo;

    // Emissive: params.w controls emissive strength (0 = none, 1+ = glow)
    // Emissive objects use base_color as their glow color, bypassing lighting.
    // (Declared as a var at the top; type 12 zeroes it -- see there.)
    if (emissive_strength > 0.0) {
        color = color + albedo * emissive_strength;
    }

    // Procedural emissive (e.g. lava cracks) -- additive, independent of params.w
    color = color + proc_emissive;

    // ── Aerial perspective (v0.916, research roadmap item 2) ──
    // Distant surfaces fade toward the sky's in-scatter color - the single
    // strongest landscape realism cue. Exponential height haze: the CPU
    // pokes sigma (already folded with the camera-altitude density falloff
    // and the Settings strength) into light1_cone_inner.y, the slant cap
    // scale into light1_cone_inner.z, the day/sunset-tinted sky color into [2].yzw, and
    // the camera's radial up into [3].yzw. The SLANT path bound keeps a
    // noon sun and orbit views clear: looking up exits the haze layer in a
    // few km, so only long, flat sightlines accumulate fog. sigma = 0 (off
    // in space, at night the color also darkens) makes this a no-op.
    color = aerial_apply(color, in.world_position);
    // Underwater extinction AFTER aerial haze: above water the aerial term is
    // the atmosphere, below it the water column is what attenuates, and the two
    // are mutually exclusive in practice (aerial sigma is a surface-air value).
    color = underwater_apply(color, in.world_position);

    // ACES-like tone mapping (more filmic than Reinhard)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, out_alpha);
}
