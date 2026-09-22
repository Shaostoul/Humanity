//! A planet's SHELLS: the cloud deck and the atmosphere dome, built once per
//! frame for any body large enough on screen to be worth them, and pushed into
//! the transparent celestial list in the order the view demands (extracted
//! from lib.rs, v0.1320).
//!
//! What is actually decided here, and why it is one block rather than two:
//!
//! - THE CLOUD DECK. The material (quality tier, coverage, per-planet noise
//!   seed), the shell mesh, and the whole set of per-frame uniforms the cloud
//!   march reads: the regime the camera is in (above / inside / below the
//!   slab), the temporal reprojection anchor and its re-anchor counter, the
//!   spin-aware resolve motion split, and the reference-march frame stash the
//!   dev panel reads back.
//! - THE ATMOSPHERE DOME. Its material, either the physical scattering packing
//!   or the Fresnel fallback, and its mesh at the atmosphere scale.
//! - THE COMPOSITE ORDER. Inside the atmosphere the dome goes first and the
//!   deck on top, outside it the other way round -- and the fullscreen cloud
//!   composite follows the same rule. Getting this wrong is what erased most
//!   of the clouds on approach (the comment at the `atmo_over` line has the
//!   root cause). The order can only be decided once BOTH shells exist, which
//!   is why they are built together.
//!
//! WHY IT IS A MODULE: 1,057 lines, the largest single thing left in the
//! celestial draw loop, and `src/lib.rs` is the repo's merge funnel (CLAUDE.md)
//! -- every cloud increment had to edit the same file as every other feature.
//!
//! WHY THE TWO CONTEXT STRUCTS instead of `&mut EngineState`: the caller holds
//! `d` borrowed out of `state.planet_defs` for the whole body iteration, so a
//! whole-state borrow is an E0502 (the constraint `near_tree_models` records).
//! `ShellState` names the nineteen state fields this block touches -- ten of
//! them the cloud system's own per-frame memory, which is exactly the list a
//! reader of the temporal reprojection needs -- and `ShellView` names the seven
//! frame values. The struct definitions ARE the dependency list, and the
//! compiler checks them in a way a doc comment would not be.
//!
//! BEHAVIOUR: none. The body is the previous inline block line for line apart
//! from nine `*`, each demanded by the compiler because a field that used to be
//! owned is now a borrowed reference:
//!
//! - four WRITES: `*state.cloud_map_regime`, `*state.cloud_map_reanchors` (and
//!   its read on the right-hand side), `*state.cloud_map_anchor`, and
//!   `*state.cloud_ref_frame`;
//! - four READS THAT MUST STAY COPIES, which is the one place here worth
//!   thinking about rather than pattern-matching: `let r0 =
//!   *state.cloud_map_regime` and the three `*state.cloud_map_anchor` reads
//!   (the `match`, the `.map`, the `if let`). Without the `*` those bind
//!   references INTO the field instead of copying it, which is a different
//!   program -- the anchor is rewritten a few lines later, so a borrow would
//!   alias the value it is compared against. Both types are `Copy`, so with
//!   the `*` the reads are exactly what the inline code did.

use crate::gui::GuiState;
use crate::hot_reload::data_store::DataStore;
// The live weather record the deck reads its coverage from. In lib.rs it
// arrived through `mod native_app`'s `use crate::systems::weather::Weather`.
use crate::systems::weather::Weather;
use crate::renderer::camera::Camera;
use crate::renderer::mesh::Mesh;
use crate::renderer::{RenderObject, Renderer};
use crate::terrain::planet::PlanetDef;
use glam::{DVec3, Quat, Vec3};
use std::collections::HashMap;
use std::time::Instant;

/// The slice of `EngineState` the shells touch, borrowed field by field.
///
/// Named `state` at the call site on purpose: the moved code keeps reading
/// `state.renderer`, `state.cloud_map_anchor` and the rest as it did inline.
pub(crate) struct ShellState<'a> {
    /// Materials, meshes, the GPU device, and the several dozen cloud uniforms
    /// this block publishes each frame.
    pub renderer: &'a mut Renderer,
    /// Icosphere meshes keyed by (body id, subdivision level), shared with the
    /// surface path so a shell re-uses a level already built this session.
    pub planet_mesh_cache: &'a mut HashMap<(String, u32), usize>,
    /// One cloud material per (body, quality tier).
    pub planet_cloud_materials: &'a mut HashMap<(String, u8), usize>,
    /// One atmosphere material per (body, scattering on/off).
    pub planet_atmo_materials: &'a mut HashMap<(String, bool), usize>,
    /// Read for camera position (the inside/outside-the-dome test) and for
    /// the planet-local reprojection baseline.
    pub camera: &'a Camera,
    /// Read for the live weather map generation counter.
    pub data_store: &'a DataStore,
    /// Read for the Settings cloud + atmosphere knobs.
    pub gui_state: &'a GuiState,
    /// Session clock, for the cloud animation phase.
    pub start_time: &'a Instant,
    // ── The cloud system's own per-frame memory. These ten fields are the
    //    state that makes temporal reprojection possible at all: what the map
    //    was anchored to last frame, where the camera and the planet's spin
    //    were, and which regime the march was in. ──
    /// Dev override: a fixed coverage instead of the live weather value.
    pub cloud_cover_override: &'a Option<f32>,
    /// Dev override: a fixed cloud type.
    pub cloud_type_override: &'a Option<f32>,
    /// Weather-event coverage boost and tint, from the event system.
    pub cloud_event_boost: &'a f32,
    pub cloud_event_tint: &'a [f32; 3],
    /// The map's current anchor direction and its cap angle. Re-anchoring
    /// resamples the map, so the RATE this changes is a measured gate.
    pub cloud_map_anchor: &'a mut Option<(glam::Vec3, f32)>,
    /// How many times that anchor has moved this session.
    pub cloud_map_reanchors: &'a mut u32,
    /// Where the current weather system SITS, as a planet-local unit direction,
    /// plus the condition it was placed for. Re-anchored only when the condition
    /// CHANGES, so weather stays where it appeared instead of following the
    /// player around. Replaces nothing: before v0.1329 the weather had no
    /// position at all, which is the root of BUG-080.
    pub weather_anchor: &'a mut Option<(glam::Vec3, crate::systems::weather::WeatherCondition)>,
    /// Which slab regime (above / inside / below) the march ran in last
    /// frame, so the transition is hysteretic instead of flickering.
    pub cloud_map_regime: &'a mut u8,
    /// Camera position in the planet-local frame last frame, for the
    /// reprojection delta.
    pub cloud_prev_cam_local: &'a mut Option<DVec3>,
    /// The planet's spin last frame, so the resolve can split content
    /// rotation from camera translation exactly.
    pub cloud_prev_spin: &'a mut Option<f64>,
    /// The reference-march frame stash the cloud dev panel reads back.
    pub cloud_ref_frame: &'a mut Option<String>,
}

/// Where this body is on screen and how it is oriented, this frame.
#[derive(Clone, Copy)]
pub(crate) struct ShellView {
    /// Planet centre in render space (the floating origin moves it).
    pub render_off: DVec3,
    /// The planet's spin angle in f64, range-reduced.
    pub spin_f64: f64,
    /// The same spin as the f32 model rotation the shells are drawn with.
    pub rotation: Quat,
    /// The radius the body is DRAWN at: its true radius, or the far-body
    /// minimum-angular-size floor, whichever is larger.
    pub visual_scale: f32,
    /// The body's centre in render space as an f32 position.
    pub position: Vec3,
    /// Projected pixel diameter, which selects the march's quality band.
    pub px: f32,
}

/// Build this body's cloud deck and atmosphere dome and push them, in the
/// order the current view requires, into the transparent celestial list.
///
/// Called only for bodies at least 8 px across: below that a shell is smaller
/// than the anti-aliasing and costs a draw for nothing.
pub(crate) fn push_planet_shells(
    state: &mut ShellState,
    view: ShellView,
    b: &crate::cosmos::SolBody,
    d: &PlanetDef,
    celestial_transparent: &mut Vec<RenderObject>,
) {
    // Unpack the view under the names the moved code already used, so the body
    // below stays line for line what it was inline.
    let ShellView {
        render_off,
        spin_f64,
        rotation,
        visual_scale,
        position,
        px,
    } = view;
// Cloud deck (clouds increment 1): an animated
// procedural coverage shell (shader type 15) at
// CLOUD_SHELL_SCALE, pushed BEFORE the atmosphere
// shell below. The transparent celestial list
// draws in submission order with depth-test-only
// (no writes), so list order IS composite order:
// surface (opaque) -> clouds -> atmosphere, i.e.
// the air scatters IN FRONT of the clouds --
// physically right and it keeps the blue limb
// hazing the deck near the horizon. Data-driven:
// only defs with cloud_coverage spawn one
// (earth.ron ~0.55; Mars deliberately None).
// v0.997 (operator: "clouds are still
// invisible while on the surface"): the
// shell OBJECTS build here but the PUSH
// order is decided below - from INSIDE the
// atmosphere the daytime dome is the blue
// sky itself (alpha ~0.985) and painting it
// after the deck erased the clouds; from
// space the deck must stay under the limb
// haze. Order is view-dependent now.
let mut cloud_shell_obj: Option<RenderObject> = None;
let mut atmo_shell_obj: Option<RenderObject> = None;
let clouds_on = state.gui_state.settings.planet_clouds;
if let Some(cov) = d.cloud_coverage.filter(|c| *c > 0.0 && clouds_on) {
    // Quality tier (clouds increment 3):
    // rides in the material's roughness
    // slot; the shader dispatches Low/
    // Medium/High on it. Cached per
    // (body, tier) so the Settings
    // selector applies live.
    let quality = crate::renderer::clouds::quality_param(
        &state.gui_state.settings.cloud_quality,
    );
    let ckey = (b.id.clone(), quality as u8);
    let cmat = if let Some(&m) =
        state.planet_cloud_materials.get(&ckey)
    {
        m
    } else {
        // Packing (mirror + tests:
        // renderer::clouds): white tint +
        // coverage in the color, per-planet
        // noise seed in the metallic slot,
        // quality tier in the roughness
        // slot, type 15. A future
        // cloud_color RON field can ride
        // the rgb unchanged.
        let m = state.renderer.add_material_full(
            [1.0, 1.0, 1.0, cov.min(1.0)],
            crate::renderer::clouds::cloud_seed(
                d.terrain_seed,
            ),
            quality,
            15.0,
            0.0,
        );
        state
            .planet_cloud_materials
            .insert(ckey, m);
        // One-shot (per cache fill) so a log
        // grep proves the deck actually draws
        // on this machine -- the same
        // diagnostics-first norm as the
        // "Sky-planet mesh built" line.
        log::info!(
            "Cloud deck material built: {} (coverage {:.2}, seed {})",
            b.id,
            cov,
            crate::renderer::clouds::cloud_seed(d.terrain_seed),
        );
        m
    };
    // Same shared flat shell mesh as the
    // atmosphere (per-fragment noise does the
    // detail work; only the silhouette is mesh).
    // Level 5 (20,480 tris - still trivial): at level 3 the big flat facets crease the per-pixel scattering/cloud math into faint straight seams across the disc (2026-07-11 field report).
    // FIXED level like the atmosphere shell below (v0.918): capping
    // by planet_max_subdiv sank the shell inside the planet at low
    // settings (see the atmosphere shell comment).
    let shell_level = 5;
    let skey = ("_flat".to_string(), shell_level);
    let cloud_mesh = if let Some(&m) =
        state.planet_mesh_cache.get(&skey)
    {
        m
    } else {
        let mut ico =
            crate::terrain::icosphere::Icosphere::new();
        ico.subdivide_n(shell_level);
        let m = state.renderer.add_mesh(
            Mesh::from_icosphere(
                &state.renderer.device,
                &ico,
                1.0,
            ),
        );
        state.planet_mesh_cache.insert(skey, m);
        m
    };
    // rotation matters here (unlike the
    // atmosphere): the shader samples the noise
    // in the mesh's LOCAL frame, so the deck
    // rides the planet's spin and the drift
    // constants are true weather motion.
    //
    // FLY-THROUGH fix (v0.1025, operator: "at
    // about cloud level the clouds kind of
    // disappear"): the ray march fires from the
    // drawn shell's fragments, and a camera
    // INSIDE the slab but above the mid-slab
    // shell had no geometry over half the view.
    // In/near the slab the shell now draws
    // ABOVE the slab top (the camera is always
    // inside the sphere, so backfaces cover
    // the whole sky); the shader keeps the slab
    // at true altitude via the planet/drawn
    // radius ratio in the (unused) emissive
    // slot, updated every frame.
    let cam_r_ratio = ((state.camera.effective_position()
        - position)
        .length()
        / visual_scale)
        .max(0.0);
    // Physical slab bounds (clouds depth
    // increment): planet-radius multiples
    // from the def's cloud_base_km /
    // cloud_top_km (Earth: 0.4-12 km, the
    // real altitude band - not the legacy
    // 25.5-76.5 km constants). Forwarded
    // to the shader in params2 below.
    let (slab_rb, slab_rt) = d.cloud_slab_scales();
    // Wave A (environment program increment
    // 5): ONE shell radius at every
    // altitude - just above the slab top,
    // clear of the tallest terrain (peaks
    // ~1.0014 R). The old near/far flip at
    // 331 km swapped the drawn geometry
    // mid-descent and was the largest
    // single pop the descent ladder
    // measured (test/control ratio 2.4).
    // The march's slab interval never
    // depended on the drawn radius, so the
    // only visual change is the cloud limb
    // sitting at its TRUE altitude from
    // orbit instead of 1.6% high.
    let shell_ratio = slab_rt + 0.0006;
    // Weather-event override (v0.1037): the
    // active event's eased coverage boost +
    // tint ride the material slots the
    // shader already reads (base_color.rgb
    // = tint, .a = coverage) - zero shader
    // changes, docs at 40-clouds.wgsl top.
    let ev_t = state.cloud_event_tint;
    // The in-game weather owns the LOCAL
    // sky (the clouds-depth taste call,
    // resolved by "make clouds as good as
    // possible"): when the sim says
    // Cloudy/Rain/Storm/Snow the deck must
    // show matching coverage even where
    // the live MODIS map is clear - the
    // HUD label and the sky must never
    // disagree. The floor raises effective
    // coverage AND blends placement toward
    // the procedural field in the same
    // measure (params2.w), because below
    // coverage ~1 a live-zeroed field
    // cannot be resurrected by the
    // coverage knob alone.
    // ── WEATHER AS A PLACE, NOT A GLOBAL (v0.1329, fixes BUG-080) ──
    //
    // The condition used to map to a pair of scalars here and then get faded by
    // CAMERA ALTITUDE, which made the deck fill in as the player descended and
    // cross-faded the cloud LAYOUT between two unrelated patterns. Both numbers
    // now ride an environment REGION with a position and a radius, and the shader
    // weights them by distance from the ground point each ray is heading for.
    // Nothing here reads the camera any more.
    //
    // The WEIGHTS live in data/environment/region_kinds.ron, because a hardcoded
    // table of tunable domain values is what infinite-of-x forbids. The mapping
    // from the sim's enum to a kind id below is still code, and that is fine: it
    // is a name for a variant, not a number anyone would want to tune.
    let condition = state
        .data_store
        .get::<std::sync::Mutex<Weather>>("weather")
        .and_then(|m| m.lock().ok())
        .map(|w| (w.condition, w.intensity));
    let kind_id = condition.map(|(c, _)| {
        use crate::systems::weather::WeatherCondition as WC;
        match c {
            WC::Storm => "storm",
            WC::Rain => "rain",
            WC::Snow => "snow",
            WC::Cloudy => "cloudy",
            WC::Fog => "fog",
            WC::Sandstorm => "sandstorm",
            _ => "",
        }
    });
    // ...but the sim's weather is a POINT
    // sample at the player, so its floor
    // fades out with altitude: from high
    // orbit the operator watched a local
    // Storm paint the ENTIRE planet 95%
    // white (v0.1183 report). Full local
    // authority below ~30 km (the sky
    // you are actually under), gone by
    // ~120 km, where the view spans
    // wx_fade DELETED (v0.1329), and h_km with it: that altitude was computed
    // for nothing else. It was
    // (1.0 - (h_km - 30.0) / 90.0).clamp(0.0, 1.0), a pure camera-altitude
    // term applied to both the coverage floor and the placement blend. It
    // existed because a global condition painted the whole planet from orbit
    // (v0.1183); a positioned region cannot do that, so the ramp has nothing
    // left to protect against. See docs/design/weather-spatial-extent.md.
    // Dev/showcase coverage pin wins over
    // the live weather + event boost (see
    // EngineState::cloud_cover_override).
    // No weather floor here any more: the storm raises coverage in the SHADER,
    // where the region is weighted by the ray's own ground point.
    let cov_eff = state
        .cloud_cover_override
        .unwrap_or((cov + state.cloud_event_boost).min(1.0));

    // ── UPLOAD THIS BODY'S ENVIRONMENT REGIONS (v0.1329) ──
    //
    // One weather system, anchored where it APPEARED rather than wherever the
    // player happens to be now: the anchor is replaced only when the condition
    // changes. That is what makes descending change nothing, which is the whole
    // point of BUG-080.
    //
    // Uploaded for the cloud-bearing body in view. Two such bodies on screen at
    // once would have the last one win, the same first-body-wins convention the
    // sun cache and the profile feed already use.
    {
        use crate::renderer::env_regions::{EnvRegion, RegionKinds};
        let mut regions: Vec<EnvRegion> = Vec::new();
        if let (Some((cond, intensity)), Some(id)) = (condition, kind_id) {
            if !id.is_empty() {
                // Re-anchor ONLY on a condition change.
                let stale = match state.weather_anchor.as_ref() {
                    Some((_, placed_for)) => *placed_for != cond,
                    None => true,
                };
                if stale {
                    // The ground point under the camera, in the BODY'S own frame,
                    // so the system stays over its geography as the planet spins.
                    let to_cam = state.camera.effective_position() - position;
                    let w = to_cam.normalize_or_zero();
                    let local = rotation.inverse()
                        * glam::Vec3::new(w.x as f32, w.y as f32, w.z as f32);
                    *state.weather_anchor = Some((local.normalize_or_zero(), cond));
                }
                if let (Some(table), Some((dir, _))) = (
                    state.data_store.get::<RegionKinds>("region_kinds"),
                    *state.weather_anchor,
                ) {
                    if let Some(kind) = table.by_id(id) {
                        regions.push(kind.region_at(
                            [dir.x, dir.y, dir.z],
                            (d.radius / 1000.0) as f32,
                            intensity,
                        ));
                    }
                }
            }
        }
        // The auroral ovals. See renderer::env_regions::push_auroral_ovals.
        crate::renderer::env_regions::push_auroral_ovals(
            &mut regions,
            state.data_store.get::<RegionKinds>("region_kinds"),
            (d.radius / 1000.0) as f32,
        );

        // See renderer::env_regions::log_regions for why this is permanent.
        let wv = (state.camera.effective_position() - position).normalize_or_zero();
        let cam_local = rotation.inverse()
            * glam::Vec3::new(wv.x as f32, wv.y as f32, wv.z as f32);
        crate::renderer::env_regions::log_regions(
            &regions,
            [cam_local.x, cam_local.y, cam_local.z],
            state.start_time.elapsed().as_secs_f32(),
        );
        state.renderer.set_env_regions(&regions);
    }
    // params2 FIRST so the full-uniform
    // write below carries the fresh slab
    // bounds (update_material_full writes
    // the stored params2 back).
    // params2.w encodes the placement
    // blend + dev pins: [0,1] = fraction
    // of the live MODIS placement to
    // BYPASS toward the procedural field
    // (the weather floor above; 1 = the
    // dev coverage pin's full bypass),
    // 2 + tc = full bypass AND type pin
    // (cloud_type_coord returns tc).
    // Placement source (2026-08-24, the
    // operator's own diagnosis of the
    // jarring mid-ascent deck swap):
    // with Live weather OFF the pin is
    // 1.0 = full MODIS bypass at EVERY
    // altitude - one coherent
    // procedural pattern from orbit to
    // the ground, with only DENSITY
    // drifting between the local
    // weather floor and the planet's
    // base coverage. This also makes
    // the existing Settings toggle's
    // "Off = purely procedural skies"
    // promise true in the SHADER, not
    // just in the fetcher gating. With
    // Live weather ON the old
    // far-placement handoff remains
    // (MODIS owns the marble, the
    // procedural floor owns the local
    // sky below ~30 km).
    let pin = match (
        state.cloud_cover_override.is_some(),
        state.cloud_type_override,
    ) {
        (true, Some(tc)) => 2.0 + tc.clamp(0.0, 1.0),
        (true, None) => 1.0,
        _ if !state.gui_state.settings.live_weather => 1.0,
        // Live weather ON: pure MODIS placement at EVERY altitude. The storm's
        // pull toward the procedural field is a REGION now (g_env_place), so it
        // applies where the storm is instead of where the camera is.
        _ => 0.0,
    };
    // Temporal accumulation, armed at
    // ALL altitudes (12c): the extent-
    // parametrized map concentrates its
    // texels on whatever the camera can
    // see, so orbit gets a sharper-than-
    // screen map instead of per-pixel
    // march static, and the old 331 km
    // arming pop no longer exists. The
    // +4 flag tells the type-15 fragment
    // to get out of the way (the
    // fullscreen composite draws the
    // map). Low quality keeps the
    // direct march. The px gate keeps
    // a distant dot-sized planet off
    // the 2048^2 octa march
    // (adversarial review finding 7):
    // below ~160 px the shell march is
    // at most ~26k rays and the map
    // would be invisible detail anyway.
    let temporal = quality > 0.5 && px >= 160.0;
    if temporal {
        state.renderer.set_cloud_temporal(Some(cmat));
    }
    // The cloud SHELL material (far rung,
    // increment 4) is recorded further
    // down, beside the profile feed, and
    // ONLY when that feed is accepted:
    // the material the passes bind must
    // be the body whose ground cell was
    // planned (first body wins both).
    // 12d/12g regime split: NEAR (planet
    // filling the screen) uses the
    // half-res per-pixel screen pass -
    // no direction cache, so no ghost
    // family and no vanish-on-approach;
    // FAR keeps the octa map, whose
    // texels are sub-pixel on a small
    // disc. 12g CROSSFADE (operator:
    // "a huge patch of clouds just
    // vanishes" at the switch): the two
    // regimes render measurably
    // different coverage (v0.1204
    // journal - cause still open), so a
    // binary switch pops in one frame
    // no matter where it sits. Instead
    // the composite MIXES the two
    // sources across px 1000..1600:
    // both passes run inside the band,
    // and any residual look difference
    // spreads over hundreds of frames
    // of approach instead of one. The
    // old enter/leave hysteresis is
    // gone - the mix is continuous, and
    // at the band edges the incoming
    // source's weight is ~0 anyway.
    // Band 1600..2000 px (round 3 of the
    // boundary hunt, 2026-08-24): at
    // planetary-disc ranges the SCREEN
    // pass's quarter-res ray grid spans
    // ~60 km of cloud field per march
    // pixel - the accumulated march
    // integrates sub-grid gap/mass
    // structure into a featureless
    // semi-white VEIL (the "white
    // continent" was this veil's contrast
    // against land vs ocean, never
    // geography). The octa map exists
    // precisely to concentrate its rays
    // on the disc (~6 km per texel) and
    // renders every tested disc range
    // correctly, so it now owns
    // everything out to px 1600; the
    // screen pass takes over only where
    // the planet more than fills the
    // screen (below roughly 1,000 km
    // altitude) and its grid is dense.
    // ── ONE RENDERER (v0.1250, the map retirement) ──
    // Fifteen releases of octa-map artifact
    // whack-a-mole (the rosette, the checkers,
    // the dome seam, the eye-wall, the gray
    // backdrop veil) against the operator's
    // consistent praise for every near-march
    // view settled the architecture question:
    // the per-pixel volumetric march owns the
    // WHOLE sky at every altitude. near_mix is
    // pinned to 1.0 whenever the temporal
    // system is armed; the octa pass never
    // dispatches (mod.rs pins octa_runs
    // false), its texture stays empty, and
    // the composite's near arm is the only
    // content source. Far rays are bounded by
    // the footprint-proportional stride (a
    // few coarse steps at range) plus the
    // iteration cap, and rays outside the
    // shell abstain instantly; disc-range
    // quality and cost are judged on the
    // probe ladder, not assumed.
    let near_mix = if temporal { 1.0 } else { 0.0 };
    let near = near_mix > 0.0;
    state.renderer.cloud_mode_near = near;
    state.renderer.cloud_near_mix = near_mix;
    if near {
        state.renderer.ensure_cloud_screen();
    }
    // [CloudRegime] 1 Hz instrument
    // (v0.1204 lesson: three sweeps
    // were confounded by GUESSING which
    // regime a park ran).
    {
        static LAST: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let now_s = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if now_s != LAST.swap(now_s, std::sync::atomic::Ordering::Relaxed) {
            log::info!(
                "[CloudRegime] px={:.0} mix={:.2} alt_km={:.1} temporal={} pin={:.2} live={} cov={:.2} camr={:.6} vscale={:.1} rb={:.6} rt={:.6} cmax={:.6} reanchors={}",
                px,
                near_mix,
                (cam_r_ratio as f32 - 1.0).max(0.0)
                    * (d.radius / 1000.0) as f32,
                temporal,
                pin,
                state.gui_state.settings.live_weather,
                cov_eff,
                cam_r_ratio,
                visual_scale,
                slab_rb,
                slab_rt,
                state.renderer.cloud_map_cmax,
                state.cloud_map_reanchors,
            );
        }
    }
    let pin = pin + if temporal { 4.0 } else { 0.0 };
    state.renderer.update_material_params2(
        cmat,
        [slab_rb, slab_rt, (d.radius / 1000.0) as f32, pin],
    );
    state.renderer.update_material_full(
        cmat,
        [ev_t[0], ev_t[1], ev_t[2], cov_eff],
        crate::renderer::clouds::cloud_seed(d.terrain_seed),
        quality,
        15.0,
        1.0 / shell_ratio,
    );
    cloud_shell_obj = Some(RenderObject { fade: 0.0,
        position,
        rotation,
        scale: Vec3::splat(visual_scale * shell_ratio),
        mesh: cloud_mesh,
        material: cmat,
    });
    // Map basis anchor with HYSTERESIS
    // (Wave D fix 2): the camera's
    // planet-local direction, re-anchored
    // only past 0.02 rad of drift so a
    // camera near a boundary can never
    // flip-flop the basis.
    {
        // 12c map-param controller: pick
        // the ideal (anchor, extent) for
        // the camera's regime, freeze it
        // with hysteresis, and order a
        // one-frame history resample on
        // every re-anchor so the change
        // is invisible (the math and the
        // regime table live in
        // docs/design/environment-program.md
        // increment 12c).
        let cam_p = state.camera.effective_position();
        let up_w = (cam_p - position).normalize_or_zero();
        let up_l = (rotation.conjugate() * up_w)
            .normalize_or_zero();
        let c = cam_r_ratio as f32;
        // Hysteresis bands so a hovering
        // camera cannot chatter regimes.
        // Each band is scaled to the
        // boundary it guards, NOT the
        // planet radius (adversarial
        // review finding 1: a flat
        // 0.0004 R = 2.5 km band at a
        // 400 m slab-base boundary put
        // the under-deck regime 2.1 km
        // BELOW sea level - unreachable).
        // rt boundary: 2.5 km against a
        // ~12 km deck top. rb boundary:
        // half the base altitude (200 m
        // on Earth).
        const BAND_RT: f32 = 0.0004;
        let band_rb =
            ((slab_rb - 1.0) * 0.5).max(1.0e-6);
        // How far the map anchor may drift before it
        // is re-anchored, and the margin every extent
        // is inflated by to cover exactly that drift.
        // The two MUST stay equal - see the re-anchor
        // test below for why.
        const MAP_DRIFT: f32 =
            8.0 * std::f32::consts::PI / 180.0;
        let r0 = *state.cloud_map_regime;
        let regime = if r0 == 1 && c >= slab_rt {
            1
        } else if r0 == 2
            && c <= slab_rt + BAND_RT
            && c >= slab_rb - band_rb
        {
            2
        } else if r0 == 3 && c <= slab_rb {
            3
        } else if c >= slab_rt + BAND_RT {
            1
        } else if c > slab_rb - band_rb {
            2
        } else {
            3
        };
        *state.cloud_map_regime = regime;
        // v0.1250: the octa map is dormant
        // (one renderer; see the near_mix
        // note above) - never force it.
        state.renderer.cloud_octa_force = false;
        // Sun-drift EMA boost REMOVED (v0.1248): the diff-driven
        // adaptive alpha already reacts to lighting
        // change; an extra floor on a 20-minute day
        // perpetually re-noised the map - the
        // operator's persistent checkerboard. The
        // resume-drop (dispatch freeze recovery)
        // stays; it handles the real staleness case.
        let (ideal_a, ideal_th) = match regime {
            // Above the deck: anchor at
            // NADIR, extent = the shell
            // disc + 4 deg margin. Every
            // texel lands on visible
            // cloud; the old antipode
            // singularity (the pinch at
            // the operator's feet) is
            // now the best-resolved
            // point of the map.
            1 => {
                let sin_t =
                    (slab_rt / c.max(slab_rt)).min(1.0);
                (
                    -up_l,
                    sin_t.asin()
                        + 4.0f32.to_radians()
                        + MAP_DRIFT,
                )
            }
            // Inside the slab: cloud in
            // every direction - the one
            // regime that needs the full
            // sphere.
            2 => (up_l, std::f32::consts::PI),
            // Under the deck: sky +
            // horizon, stopping 25 deg
            // below horizontal (terrain
            // relief margin).
            _ => (
                up_l,
                (115.0f32.to_radians() + MAP_DRIFT)
                    .min(std::f32::consts::PI),
            ),
        };
        // ── RE-ANCHOR THRASH (v0.1232) ──
        //
        // Operator: "when I fly around and sit in
        // space above I get very weird cloud
        // artifacting. It is like it regens all the
        // clouds all over and then tries to phase
        // out the ones that do not belong."
        //
        // The old test re-anchored on 0.02 rad of
        // drift - 1.15 DEGREES. The anchor for the
        // above-deck regime is the nadir, so simply
        // flying along an orbit sweeps past that in a
        // moment and the map re-anchored again and
        // again, continuously.
        //
        // A re-anchor is meant to be invisible: the
        // shader looks history up through the OLD
        // mapping. But a direction outside the OLD
        // extent has no history and takes the fresh
        // march outright - an unconverged, noisy
        // texel. Re-anchoring constantly meant a
        // permanent supply of those, which is exactly
        // the regenerate-then-fade the operator sees,
        // and why it tracks cloud density: dense
        // regions have more marched content to be
        // unconverged about.
        //
        // The fix is to make the two agree by
        // construction. The extent above is inflated
        // by exactly MAP_DRIFT, and we only re-anchor
        // once drift EXCEEDS MAP_DRIFT - so while the
        // anchor is stale, every direction the new
        // ideal extent wants is still inside the old
        // one, and history is always there to be
        // found. Costs a wider map (coarser texels by
        // the same ratio) and buys a stable sky.
        let re = match *state.cloud_map_anchor {
            Some((a, th)) => {
                let drifted =
                    a.dot(ideal_a) < MAP_DRIFT.cos();
                // Re-anchor for extent only when the
                // map no longer COVERS what is needed,
                // or is so much wider than needed that
                // most of its resolution is wasted.
                // The old symmetric 2 deg test fired on
                // any altitude change at all.
                let short = th < ideal_th;
                let wasteful =
                    th > ideal_th + 2.0 * MAP_DRIFT;
                drifted || short || wasteful
            }
            None => true,
        };
        if re {
            // Instrument (v0.1232): the
            // re-anchor RATE is the thing
            // that has to fall. A static
            // capture cannot show this -
            // the artifact only exists
            // while the camera moves - so
            // the count is the gate.
            *state.cloud_map_reanchors =
                *state.cloud_map_reanchors + 1;
            state.renderer.cloud_map_resample.set(
                (*state.cloud_map_anchor).map(|(a, th)| {
                    ([a.x, a.y, a.z], th.cos())
                }),
            );
            *state.cloud_map_anchor =
                Some((ideal_a, ideal_th));
        } else {
            state.renderer.cloud_map_resample.set(None);
        }
        if let Some((a, th)) = *state.cloud_map_anchor {
            state.renderer.cloud_map_anchor_local =
                [a.x, a.y, a.z];
            state.renderer.cloud_map_cmax = th.cos();
        }
        // Slice B translation baseline:
        // the camera in the PLANET-LOCAL
        // frame - the frame the cloud
        // field actually lives in (spin
        // included). Its delta, rotated
        // back to current world axes, is
        // the content-relative motion
        // the octa pass reprojects by.
        //
        // f64 END TO END (v0.1238). The
        // old f32 chain subtracted two
        // ~3.6e7 m quantities (a camera
        // FLOWN from the homestead, with
        // no floating-origin rebase) whose
        // f32 ulp is 4 m, so a delta whose
        // true value is centimeters came
        // out snapped to an axis-aligned
        // 4 m lattice - and this one
        // number feeds the octa map
        // reprojection (light4), the
        // resolve's prev_dpos, AND the
        // motion gates. That lattice was
        // the operator's cardinal-locked
        // starburst at the feet, proven by
        // the starburst-far probe vantage
        // (far_frame_km), which reproduces
        // the flown split on the rig. The
        // subtraction and both rotations
        // stay f64 (exact at these
        // magnitudes); only the final
        // small delta is cast to f32.
        let rot64 = glam::DQuat::from_rotation_y(spin_f64);
        let cam_l64 = glam::DVec3::new(
            cam_p.x as f64,
            cam_p.y as f64,
            cam_p.z as f64,
        );
        let p_l = rot64.conjugate()
            * (cam_l64 - render_off);
        let prev_l = state
            .cloud_prev_cam_local
            .replace(p_l);
        state.renderer.cloud_reproj_delta.set(
            prev_l.map(|p| {
                let d_w = rot64 * (p - p_l);
                [d_w.x as f32, d_w.y as f32, d_w.z as f32]
            }),
        );
        // ── SPIN-AWARE resolve motion (v0.1251) ──
        // The planet-local delta above folds the
        // spin sweep into an equivalent camera
        // TRANSLATION - first-order correct at
        // the view centre, increasingly wrong
        // toward the limb, and it makes every
        // non-co-rotating camera read as "moving",
        // which the resolve's motion floor turned
        // into raw march static (the operator's
        // "TV static" from space). The resolve now
        // gets the motion SPLIT exactly: the
        // content rotation as a rigid rotation
        // about the spin axis (applied per pixel
        // to the hit point) plus the RAW camera
        // translation. All f64 until the final
        // small casts (the v0.1238 lattice
        // lesson).
        let prev_spin =
            state.cloud_prev_spin.replace(spin_f64);
        state.renderer.cloud_resolve_motion.set(
            match (prev_l, prev_spin) {
                (Some(p), Some(s_prev)) => {
                    let dphi = spin_f64 - s_prev;
                    let m = glam::DQuat::from_rotation_y(
                        -dphi,
                    );
                    let e_cur = cam_l64 - render_off;
                    let e_prev =
                        glam::DQuat::from_rotation_y(
                            s_prev,
                        ) * p;
                    let dpos = e_prev - e_cur;
                    let s_off = m * e_cur - e_cur;
                    let mx = m * glam::DVec3::X;
                    let my = m * glam::DVec3::Y;
                    let mz = m * glam::DVec3::Z;
                    Some(
                        crate::renderer::cloud_resolve::CloudResolveMotion {
                            cols: [
                                [mx.x as f32, mx.y as f32, mx.z as f32],
                                [my.x as f32, my.y as f32, my.z as f32],
                                [mz.x as f32, mz.y as f32, mz.z as f32],
                            ],
                            spin_off: [
                                s_off.x as f32,
                                s_off.y as f32,
                                s_off.z as f32,
                            ],
                            dpos_raw: [
                                dpos.x as f32,
                                dpos.y as f32,
                                dpos.z as f32,
                            ],
                        },
                    )
                }
                _ => None,
            },
        );
        // ── Sun-shadow cache feed (increment 1, v0.1286) ──
        // The cache's windows sit at the
        // camera's GROUND point in the
        // planet-local frame (the cloud
        // shell's object space before its
        // scale: `p_l` above is exactly
        // that frame in render units).
        // Metres = render units * true
        // radius / drawn radius; the
        // ground point is the camera's
        // direction at the planet radius,
        // f64 end to end. The sun is the
        // renderer's current world sun
        // rotated into the same frame,
        // which is what the shader's
        // `inv_model * sun` produces for
        // the march and the bake alike.
        // ── Cloud PROFILE feed (increment 4, the far rung) ──
        // Fed on EVERY tier the shell
        // exists (the Low sheet needs
        // the global map), from the
        // same planet-local `p_l` the
        // sun cache uses: the ground
        // lon/lat in f64, the altitude,
        // the slab, the MARCH pixel
        // angle (mod.rs stores it beside
        // the screen pix_ang), the cloud
        // clock exactly as mod.rs writes
        // it (pinned or live), the
        // coverage, the type pin (before
        // the temporal +4), the weather
        // upload counter, the tier, the
        // knob and the calibration key.
        {
            let pl = p_l;
            let plen = pl.length().max(1.0e-9);
            let ground_lat = (pl.y / plen).clamp(-1.0, 1.0).asin();
            let ground_lon = (-pl.z).atan2(pl.x);
            let radius_km = d.radius as f64 / 1000.0;
            let cloud_t = if state.renderer.cloud_clock_pin >= 0.0 {
                state.renderer.cloud_clock_pin
            } else {
                state.start_time.elapsed().as_secs_f32()
            };
            let knob = state.renderer.cloud_profile_knob;
            let calib_key = state.renderer.cloud_profile_calib_key(quality);
            // The plan accepts the FIRST
            // cloud body of the frame
            // (a later body is ignored);
            // the shell material the
            // profile passes bind, and
            // the shell draw reads the
            // atlas through, follows the
            // same first-wins rule so the
            // lattice and the material
            // (planet radius, slab) are
            // always the same body's.
            // Cleared at the top of every
            // frame with
            // set_cloud_temporal(None).
            let accepted = state.renderer.cloud_profile_plan(
                crate::renderer::cloud_temporal::CloudProfileFrame {
                    ground_lon_rad: ground_lon,
                    ground_lat_rad: ground_lat,
                    alt_km: (cam_r_ratio as f64 - 1.0).max(0.0) * radius_km,
                    radius_km,
                    slab_rb: slab_rb as f64,
                    slab_rt: slab_rt as f64,
                    pix_ang_march: state.renderer.cloud_pix_ang_march.get() as f64,
                    cloud_t,
                    coverage: cov_eff,
                    type_pin: if pin >= 4.0 { pin - 4.0 } else { pin },
                    weather_gen: state.renderer.weather_map_gen.get(),
                    tier: quality,
                    knob,
                    calib_key,
                },
            );
            if accepted {
                state.renderer.cloud_shell_mat = Some(cmat);
            }
        }
        // Fed only in the NEAR regime
        // (the march that reads it).
        if near {
            let sun_w = state.renderer.cloud_ref_sun().0;
            let sun_l = rot64.conjugate()
                * glam::DVec3::new(
                    sun_w[0] as f64,
                    sun_w[1] as f64,
                    sun_w[2] as f64,
                );
            let ground_m = p_l.normalize_or_zero() * d.radius as f64;
            state.renderer.cloud_light_plan(
                crate::renderer::cloud_temporal::CloudLightFrame {
                    ground_local_m: ground_m,
                    sun_local: sun_l.normalize_or_zero(),
                    radius_m: d.radius as f64,
                    shell_ratio: shell_ratio as f64,
                    // The same slab base the
                    // shader gets in params2.x,
                    // so z0 agrees per planet.
                    slab_rb: slab_rb as f64,
                },
            );
        }
    }
    // Fullscreen composite frame (Wave D
    // slice 1b): armed with the temporal
    // map, cleared otherwise.
    // [CloudArm] probe (v0.1247 forensics)
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static LAST: AtomicU64 = AtomicU64::new(0);
        let s_now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs()).unwrap_or(0);
        if s_now != LAST.swap(s_now, Ordering::Relaxed) {
            log::info!("[CloudArm] arming, temporal={temporal}");
        }
    }
    state.renderer.cloud_composite_frame = if temporal {
        let bx = rotation * Vec3::X;
        let by = rotation * Vec3::Y;
        let bz = rotation * Vec3::Z;
        Some(crate::renderer::cloud_composite::CloudCompositeFrame {
            center: [position.x, position.y, position.z],
            planet_r: visual_scale,
            basis: [
                [bx.x, bx.y, bx.z],
                [by.x, by.y, by.z],
                [bz.x, bz.y, bz.z],
            ],
            rb: slab_rb,
            rt: slab_rt,
            anchor_local: state
                .cloud_map_anchor
                .map(|(a, _)| [a.x, a.y, a.z])
                .unwrap_or([0.0, 1.0, 0.0]),
            cmax: state
                .cloud_map_anchor
                .map(|(_, th)| th.cos())
                .unwrap_or(-1.0),
            // Patched to the real value
            // beside the shell-ordering
            // match below, where
            // inside_atmo is known.
            atmo_over: false,
            sentinel_ok: px >= 700.0,
        })
    } else {
        None
    };
    // Reference-march frame stash
    // (increment 10): see
    // EngineState::cloud_ref_frame.
    *state.cloud_ref_frame = Some(format!(
        concat!(
            "{{\"seed\":{},\"coverage\":{},",
            "\"tint\":[{},{},{}],\"pin\":{},",
            "\"temporal\":{},\"slab_rb\":{},",
            "\"slab_rt\":{},\"radius_km\":{},",
            "\"quality\":{},\"shell_ratio\":{},",
            "\"visual_scale\":{},",
            "\"center\":[{},{},{}],",
            "\"rot\":[{},{},{},{}]}}"
        ),
        crate::renderer::clouds::cloud_seed(d.terrain_seed),
        cov_eff,
        ev_t[0], ev_t[1], ev_t[2],
        pin,
        temporal,
        slab_rb, slab_rt,
        (d.radius / 1000.0) as f32,
        quality,
        shell_ratio,
        visual_scale,
        position.x, position.y, position.z,
        rotation.x, rotation.y, rotation.z, rotation.w,
    ));
}
if let Some(ac) = d.atmosphere_color {
    if ac[3] > 0.0 && d.atmosphere_scale > 0.0 {
        let scatter =
            state.gui_state.settings.planet_atmo_scatter;
        let mat_key = (b.id.clone(), scatter);
        let amat = if let Some(&m) =
            state.planet_atmo_materials.get(&mat_key)
        {
            m
        } else {
            let m = if scatter {
                // Physical packing: the unused
                // metallic/roughness slots carry
                // the planet-radius and scale-
                // height RATIOS in shell units,
                // so the shader stays invariant
                // to the far-body disc-size
                // floor above. Color semantics:
                // rgb = relative per-channel
                // scattering strengths, a =
                // density (see
                // renderer::atmosphere, the
                // tested Rust mirror).
                let (rp_ratio, h_rel) =
                    crate::renderer::atmosphere::shell_packing(
                        d.atmosphere_scale,
                        d.scale_height_or_default(),
                        d.radius,
                    );
                state.renderer.add_material_full(
                    [ac[0], ac[1], ac[2], ac[3]],
                    rp_ratio,
                    h_rel,
                    14.0,
                    0.0,
                )
            } else {
                // Fresnel fallback: the
                // pre-v0.807 tinted shell.
                state.renderer.add_material_full(
                    [ac[0], ac[1], ac[2], ac[3]],
                    0.0,
                    1.0,
                    13.0,
                    0.0,
                )
            };
            state.planet_atmo_materials.insert(mat_key, m);
            m
        };
        // Smooth shell: the shared flat
        // sphere cache at a fixed mid level.
        // Level 5 (20,480 tris - still trivial): at level 3 the big flat facets crease the per-pixel scattering/cloud math into faint straight seams across the disc (2026-07-11 field report).
        // FIXED level, deliberately NOT capped by planet_max_subdiv
        // (v0.918): an icosphere below level ~3 has its face planes
        // INSIDE the planet surface (level-0 inradius is 0.79R vs the
        // 0.97R ground), so a low "planet detail" setting silently
        // swallowed the whole sky dome underground - no sky, stars at
        // noon (probe rig, 2026-07-21). The cap exists for the HEAVY
        // body meshes (levels 8-9, hundreds of MB); the shell's 20k
        // tris cost nothing on any GPU.
    let shell_level = 5;
        let skey = ("_flat".to_string(), shell_level);
        let shell_mesh = if let Some(&m) =
            state.planet_mesh_cache.get(&skey)
        {
            m
        } else {
            let mut ico =
                crate::terrain::icosphere::Icosphere::new();
            ico.subdivide_n(shell_level);
            let m = state.renderer.add_mesh(Mesh::from_icosphere(
                &state.renderer.device,
                &ico,
                1.0,
            ));
            state.planet_mesh_cache.insert(skey, m);
            m
        };
        atmo_shell_obj = Some(RenderObject { fade: 0.0,
            position,
            rotation,
            scale: Vec3::splat(
                visual_scale
                    * (1.0 + d.atmosphere_scale.max(0.005) * 2.0),
            ),
            mesh: shell_mesh,
            material: amat,
        });
    }
}
// View-dependent composite order (v0.997).
// INSIDE the atmosphere: dome first, deck
// on top - the clouds finally show against
// the daytime sky. OUTSIDE: deck first,
// dome after - the approved space look
// (blue limb hazing the deck) unchanged.
let inside_atmo = atmo_shell_obj
    .as_ref()
    .map(|a| {
        (state.camera.effective_position() - position).length()
            < a.scale.x
    })
    .unwrap_or(false);
// 12c order fix: the fullscreen cloud
// composite must respect the same rule
// this match expresses for the shells -
// outside the atmosphere the dome draws
// OVER the deck. The renderer positions
// the composite pass by this flag.
let has_atmo = atmo_shell_obj.is_some();
if let Some(f) =
    state.renderer.cloud_composite_frame.as_mut()
{
    // THE APPROACH VANISH, ROOT-CAUSED
    // (2026-08-25, the operator's "most
    // of the clouds just vanish"):
    // ALWAYS composite the deck AFTER
    // the atmosphere dome.
    //
    // The 12c order rule sent the
    // composite BEFORE the transparent
    // pass whenever the camera sat
    // outside the atmosphere, so the
    // dome would sit over the deck at
    // the limb (the v0.997 look). But
    // over the DISC the dome's alpha is
    // near-opaque, so it did not veil
    // the clouds - it ERASED them.
    // Measured at 9,500 km: the
    // composite wrote 1.2% of the disc
    // with the old order and 99.9% with
    // this one, and every discard
    // sentinel inside the composite read
    // zero (the clouds were drawn, then
    // painted over).
    //
    // Drawing clouds last is also the
    // PHYSICALLY correct order here: the
    // march already applies this
    // engine's own aerial perspective at
    // the cloud's first-hit distance
    // (aerial_apply + aerial_transmittance
    // in cloud_march_core), so the
    // composited radiance ALREADY carries
    // the air column in front of it.
    // Letting the dome blend over it
    // applied that same air twice, and
    // the second application was opaque.
    //
    // Why it looked like a terrain bug:
    // the chunked-terrain LOD engaging at
    // 1.5 planet radii (9,556 km) flipped
    // which shells the celestial lists
    // carried, and so flipped this very
    // ordering - which is why the cliff
    // sat exactly at the chunk-activation
    // altitude and survived a dozen
    // cloud-only investigations.
    f.atmo_over = false;
}
match (cloud_shell_obj, atmo_shell_obj) {
    (Some(c), Some(a)) if inside_atmo => {
        celestial_transparent.push(a);
        celestial_transparent.push(c);
    }
    (Some(c), Some(a)) => {
        celestial_transparent.push(c);
        celestial_transparent.push(a);
    }
    (Some(c), None) => celestial_transparent.push(c),
    (None, Some(a)) => celestial_transparent.push(a),
    (None, None) => {}
}
}
