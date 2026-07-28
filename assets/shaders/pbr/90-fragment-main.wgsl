// ── Fragment Shader ──

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
    let depth_m = f32(u32(round(max(in.uv.x, 0.0))) & 65535u) / 10.0;
    // Deep open-ocean body color (linear). The seabed under the shell keeps
    // the graded bathymetry albedo; this is only the water column's own hue.
    var deep = vec3<f32>(0.013, 0.055, 0.11);
    // Shallow water is turquoise: the column is too thin to absorb the
    // seabed's warmth, so mix toward a bright green-blue over the first
    // ~9 m of depth.
    deep = mix(vec3<f32>(0.075, 0.30, 0.30), deep, smoothstep(0.4, 9.0, depth_m));
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
    let sw_lon = atan2(-dir.z, dir.x);
    let sw_lat = asin(clamp(dir.y, -1.0, 1.0));
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
    let shoal = 0.2 + 0.8 * smoothstep(0.4, 7.0, depth_m);
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
        if (tex_reach > 0.003) {
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
    var rgb = water_shade(deep, n_geo, n_pert, view_dir);
    // Foam is scattered froth - and froth is DIFFUSE, so it is SUNLIT like
    // everything else (v0.914, operator: "the ocean in night time is
    // bright white, almost like it is glowing" - the old constant foam
    // color ignored the sun entirely). Night foam goes dark with the sea.
    let foam_day = clamp(dot(n_geo, normalize(camera.sun_direction.xyz)), 0.0, 1.0);
    let foam_col = vec3<f32>(0.75, 0.81, 0.86)
        * (foam_day * camera.sun_direction.w * 0.42 + 0.015);
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
    let alpha = clamp(0.93 + 0.07 * fres, 0.0, 1.0) * smoothstep(0.02, 1.0, depth_m);
    // Same ACES curve as the main pipeline tail (this branch early-returns,
    // mirroring the cloud shell's convention).
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    let mapped = clamp(
        (rgb * (a * rgb + vec3<f32>(b))) / (rgb * (c * rgb + vec3<f32>(d)) + vec3<f32>(e)),
        vec3<f32>(0.0),
        vec3<f32>(1.0),
    );
    return vec4<f32>(mapped, alpha);
}

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
        // params.w REPURPOSED for this type as a BIT FIELD (v0.816; a
        // single texture flag since v0.811): bit 0 = a baked per-pixel
        // albedo texture is bound at group 3 (replacing the per-face color
        // mosaic with smooth imagery), bit 1 = Settings > Graphics >
        // Planets "Surface detail" (the ocean waves + land micro-texture
        // below). lib.rs rewrites the value every frame, so the toggle
        // applies live. It never doubles as emissive here:
        emissive_strength = 0.0;
        // ── Sprite tree cards (v0.961, billboard bake increment 2) ──
        // uv.x < -0.5 marks a card textured from the baked conifer atlas
        // (group 3 binding 14): |uv.x| = (1 + tile) + u01 * 0.5 (the small
        // base keeps u01 interpolation sub-texel), uv.y = v01 (0 ground,
        // 1 top). Lighting normal is the interpolated radial up, same as
        // the legacy colored cards. params.w bit 2 = atlas resident; until
        // the bake lands the card shades flat conifer green (never
        // invisible).
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
                let tile = clamp(u32(floor(a_enc)) - 1u, 0u, 5u);
                let u01 = clamp(fract(a_enc) * 2.0, 0.0, 1.0);
                let v01 = clamp(in.uv.y, 0.0, 1.0);
                let tuv = vec2<f32>(
                    (f32(tile % 3u) + u01) / 3.0,
                    (f32(tile / 3u) + (1.0 - v01)) / 2.0,
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
        // Grass distance dissolve (v0.999, operator: "a line of light
        // perpendicular to me like 10 meters away"): grass tufts only bake
        // on the deepest terrain patches, so their field ended at a hard
        // patch boundary that ringed the camera and lit up at grazing sun.
        // Bit 18 marks grass cards; they Bayer-dissolve over 30..45 m so
        // the field fades out well inside the guaranteed-grass region and
        // the moving edge disappears.
        if ((packed & 262144u) != 0u) {
            let tuft_dist = length(camera.view_pos.xyz - in.world_position);
            let fade = smoothstep(30.0, 45.0, tuft_dist);
            if (fade > 0.0) {
                let gpx = vec2<u32>(u32(in.clip_position.x), u32(in.clip_position.y));
                let gbx = gpx.x % 4u;
                let gby = gpx.y % 4u;
                let gbits = (gbx & 1u) | ((gby & 1u) << 1u) | (((gbx >> 1u) & 1u) << 2u) | (((gby >> 1u) & 1u) << 3u);
                let ginter = ((gbits & 1u) << 3u) | (((gbits >> 1u) & 1u) << 2u) | (((gbits >> 2u) & 1u) << 1u) | ((gbits >> 3u) & 1u);
                let gthresh = (f32(ginter) + 0.5) / 16.0;
                if (fade >= gthresh) {
                    discard;
                }
            }
        }
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
                let water_rgb = water_shade(albedo, normal, n_pert, view_dir);
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
    let sun_ndl = dot(normal, normalize(camera.sun_direction.xyz));
    var lo = evaluate_light(
        camera.sun_direction.xyz, camera.sun_color.rgb, camera.sun_direction.w,
        normal, view_dir, albedo, metallic, roughness, f0)
        * sun_shadow(in.world_position, sun_ndl);

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
    let aer_sigma = camera.light1_cone_inner.y;
    if (aer_sigma > 1.0e-9) {
        let aer_vec = in.world_position - camera.view_pos.xyz;
        let aer_dist = length(aer_vec);
        if (aer_dist > 120.0) {
            let aer_up = vec3<f32>(
                camera.light3_cone_inner.y,
                camera.light3_cone_inner.z,
                camera.light3_cone_inner.w,
            );
            let up_dot = abs(dot(aer_vec / aer_dist, aer_up));
            let slant_cap = camera.light1_cone_inner.z / max(up_dot, 0.035);
            let path = min(aer_dist - 120.0, slant_cap);
            let t_aer = exp(-aer_sigma * path);
            let sky_aer = vec3<f32>(
                camera.light2_cone_inner.y,
                camera.light2_cone_inner.z,
                camera.light2_cone_inner.w,
            );
            color = color * t_aer + sky_aer * (1.0 - t_aer);
        }
    }

    // ACES-like tone mapping (more filmic than Reinhard)
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    color = clamp((color * (a * color + vec3<f32>(b))) / (color * (c * color + vec3<f32>(d)) + vec3<f32>(e)), vec3<f32>(0.0), vec3<f32>(1.0));

    return vec4<f32>(color, out_alpha);
}
