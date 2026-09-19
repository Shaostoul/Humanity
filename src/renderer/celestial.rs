//! The CELESTIAL pass: the far world at planet scale. Planets and moons, the
//! terrain patches on the one you are standing on, the atmosphere and ocean
//! shells around it, the sun shadow map that lights all of it, and the cloud
//! march that sits on top.
//!
//! Extracted VERBATIM from `renderer/mod.rs` (v0.1319) under the file-size
//! ratchet, which mod.rs had outgrown badly: 5,652 lines against a 3,883
//! budget. Third and by far the largest of four clusters out in that pass -
//! `render_celestial_onto` alone was 1,815 lines of that file, a third of it.
//!
//! WHY THIS CLUSTER, AND WHY IT IS NOT SPLIT FURTHER. It is two functions:
//! `render_celestial_onto` and the fullscreen `run_cloud_composite` it is the
//! only caller of. The pass is long because it is ONE ordered sequence whose
//! steps are not independent, and it moved as one piece because that is what
//! makes this a pure motion rather than a refactor. Its shape, in order:
//!
//!   1. Write the celestial camera uniforms (`Camera::celestial_uniforms`,
//!      whose far plane is 1e13 m so the gameplay far plane cannot clip a
//!      planet or the Sun).
//!   2. POKE about twenty values into documented-unused pads of that same
//!      uniform block, BY BYTE OFFSET. See the warning below.
//!   3. Render the sun shadow map from the light camera, when the sun is up
//!      and shadows are on.
//!   4. Upload the batched terrain-patch instances once (the arena path),
//!      so thousands of patches cost one buffer write instead of thousands.
//!   5. Draw the opaque bodies and terrain, then the transparent list.
//!   6. Run the cloud passes: the temporal accumulation, its resolve, and
//!      (either before or after the transparent list, per the frame's own
//!      `atmo_over` decision) the depth-aware composite.
//!
//! THE BYTE-OFFSET POKES ARE THE HAZARD IN THIS FILE. Step 2 writes into
//! `CameraUniforms` at raw offsets (592, 636, 464, 528, 672 and a dozen more)
//! because the values ride in pads that the struct documents as unused - that
//! is how a new per-frame value reaches the shader without a uniform-layout
//! change. It also means ADDING OR REORDERING A FIELD IN `CameraUniforms`
//! SILENTLY MOVES EVERY ONE OF THESE WRITES ONTO THE WRONG DATA, with nothing
//! but the picture to say so. `camera.rs` carries the matching warning at the
//! struct and at its tail block; the two must be read together.
//!
//! ORDERING RULES THAT SPAN FILES, so they are stated where they are easy to
//! break:
//!   * The transparent list ARRIVES already grouped by the planet layer
//!     band: lib.rs sorts it with `celestial_order::key_for`, never by
//!     shader class (a class key once put the fallback atmosphere dome
//!     under the sea). The draw loop here binds a pipeline only when the
//!     class changes, which is a handful of binds per frame ONLY because
//!     the list is grouped that way.
//!   * `draw_celestial_lines_onto` (overlay_draw.rs) runs BETWEEN this pass
//!     and the scene pass, so an orbit ring is occluded by a body but drawn
//!     over by interior walls.
//!   * `render_godrays_onto` and `render_ssao_onto` (scene_draw.rs) run right
//!     after this pass, while the depth buffer still holds terrain.
//!
//! `time_s` IS A PINNED CLOCK, not a free-running one: it is the rig's
//! animation-clock pin, and `tests/rig_pin_lint.rs` fails the build if any
//! call site passes an unpinned elapsed time instead. A capture taken through
//! an unpinned path animates while every other path is frozen, and nothing
//! about the picture says so.
//!
//! NO RE-EXPORT SHIM IS NEEDED, for the same reason `materials.rs`,
//! `capture.rs`, `surface.rs` and `scene_draw.rs` needed none: these are
//! inherent methods on `Renderer`, resolved by receiver type rather than by
//! module path, so every call site in the crate keeps working untouched. The
//! glob import is the same arrangement `scene_draw.rs` uses: a child module
//! sees its parent's private items and private `use` bindings, so the dozens
//! of names this pass needs arrive through one line and no `Renderer` field
//! has to be widened.

use super::*;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub fn render_celestial_onto(
        &self,
        camera: &Camera,
        objects: &[RenderObject],
        transparent: &[RenderObject],
        sun_dir: Vec3,
        time_s: f32,
        // Cloud ground shadows (v0.898): (cloud seed, deck coverage, enable).
        // Poked into the light_count.yzw pads after the full uniform write,
        // so the type-12 terrain branch can sample the sky's coverage field.
        cloud_shadow: (f32, f32, bool),
        // [FFT-ocean flag, then camera planet-frame position mod 64 m
        // (v0.902)]: the precision anchor for sub-8 m micro detail plus the
        // v0.1029 water-mode toggle. Poked into light0_cone_inner.xyzw.
        ground_anchor: [f32; 4],
        // FFT cascade-B anchor + water-weld LOD scale (v0.1040/41):
        // xyz = camera planet-frame position mod 256 m, w = the
        // selection's px_per_rad/split_px. Poked into
        // light4_cone_inner.xyzw.
        ocean_anchor256: [f32; 4],
        view: &wgpu::TextureView,
    ) {
        let _cost = frame_costs::stage("cpu.celestial");
        if objects.is_empty() && transparent.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::bytes_of(&camera.celestial_uniforms()),
        );
        // Cloud clock (shader type 15): app-start-relative seconds, parked in
        // sun_color.w -- a documented-unused pad in CameraUniforms, so the
        // animated cloud deck needed NO uniform-layout change (the same
        // no-layout-churn rule as the type-14 material packing). Offset 636 =
        // sun_color (624) + 12 bytes to its w component. Written before the
        // sun poke below so both land in this pass's uniform snapshot.
        // Dev clock pin (see cloud_clock_pin): substituted here and at the
        // sun_color write below, the two places the CLOUD clock is published.
        // The wave clock on the light camera buffer is left live.
        let cloud_t = if self.cloud_clock_pin >= 0.0 { self.cloud_clock_pin } else { time_s };
        self.queue.write_buffer(&self.camera_buffer, 636, bytemuck::bytes_of(&cloud_t));
        // Cloud-ground-shadow params in the light_count yzw pads (offsets
        // 592 + 4/8/12; documented unused in CameraUniforms).
        let cs = [
            cloud_shadow.0,
            cloud_shadow.1,
            if cloud_shadow.2 { 1.0_f32 } else { 0.0 },
        ];
        self.queue.write_buffer(&self.camera_buffer, 596, bytemuck::cast_slice(&cs));
        // Point lights reach the CELESTIAL pass too (v0.976, the v0.953/954
        // dark-grid mystery): `Camera::uniforms()` hardcodes light_count.x = 0
        // and `celestial_uniforms()` inherits it, so every fragment drawn in
        // this pass - the planet TERRAIN, and everything riding it - looped
        // over zero lights while the storage buffer sat full. The ship
        // interior lit because the scene pass rewrites the uniform with the
        // real count. Poke the live count (light_count.x, offset 592) exactly
        // like the yzw pads above.
        let lc = self.cur_lights.len() as f32;
        self.queue.write_buffer(&self.camera_buffer, 592, bytemuck::bytes_of(&lc));
        // FFT-ocean flag + micro-detail anchor in light0_cone_inner.xyzw
        // (offset 464: .x = water mode, .yzw = the v0.902 anchor).
        self.queue
            .write_buffer(&self.camera_buffer, 464, bytemuck::cast_slice(&ground_anchor));
        // Cloud wind-advection angle in light1_cone_inner.x (offset 480,
        // beside the aerial params in .y/.z; v0.1032): the zonal rotation
        // the weather-map lookups apply so cloud masses drift with the
        // live wind between MODIS refreshes.
        self.queue
            .write_buffer(&self.camera_buffer, 480, bytemuck::bytes_of(&self.cloud_advect));
        // FFT cascade-B anchor + weld K in light4_cone_inner.xyzw
        // (offset 528; the light4 pads are unused - aerial data stops at
        // light3, v0.1040/41).
        self.queue
            .write_buffer(&self.camera_buffer, 528, bytemuck::cast_slice(&ocean_anchor256));
        // Detail-distance factor in the view_pos.w pad (offset 64 + 12).
        self.queue
            .write_buffer(&self.camera_buffer, 76, bytemuck::bytes_of(&self.detail_distance));
        // Sea state 0..1 in the fill_color.w pad (offset 656 + 12; the fill
        // light's alpha is never read). 0 = glassy calm, 0.5 = ripples,
        // 1 = storm chop + breaking crests. Fed by the game weather's wind
        // at the player (lib.rs) or the showcase {"sea":x} dev override.
        self.queue
            .write_buffer(&self.camera_buffer, 668, bytemuck::bytes_of(&self.sea_state));
        // Live sea crest (m) in light5_cone_inner.x (offset 544; the light5 pads
        // are unused - aerial data stops at light3 and the ocean anchor owns
        // light4). The shader's shoal fade reads it, v0.1051.
        self.queue
            .write_buffer(&self.camera_buffer, 544, bytemuck::bytes_of(&self.sea_crest_m));
        // Cloud edge-width knobs in light6_color.xy (offset 304). NOT the
        // light5 pads: the comment above calling them unused is stale -
        // 548 is underwater_ext and 552 is pix_ang, both written per frame
        // further down. light6_color is zero-filled by celestial_uniforms
        // and read by no shader. The v0.1271 wide-edge experiment needs a
        // runtime slider, not a rebuild per value.
        self.queue.write_buffer(
            &self.camera_buffer, 304, bytemuck::cast_slice(&[self.cloud_edge_mul, self.cloud_rind_wide_m, self.cloud_step_m, self.cloud_shear]));
        // light5_color.x (offset 288; zero readers, zero-filled): hv-warp amplitude km.
        self.queue.write_buffer(&self.camera_buffer, 288, bytemuck::cast_slice(&[self.cloud_hv_km, self.cloud_sigma_mul, self.cloud_ms_gain, self.cloud_int_sat]));
        // light7_color.y (offset 324): the step-economy strength of perf
        // increment 2 (v0.1288). The lane carried the deleted octa pass's EMA
        // floor and its write sat in the render function BEFORE the frame's
        // wholesale uniform upload, which zeroed it every frame (the first
        // gate sweep read the March-steps channel unchanged with the knob
        // at 1); it lives here with the other pad pokes, after that upload.
        self.queue.write_buffer(&self.camera_buffer, 324, bytemuck::bytes_of(&self.cloud_step_eco));
        // Ocean disaster event block at the CameraUniforms TAIL (offset 672,
        // pinned by camera.rs::ocean_event_block_sits_at_the_struct_tail).
        // Written after the wholesale uniform write like every pad poke; all
        // zeros when no event is live, and the shader's row-11 flag branch
        // skips the field entirely in that case.
        self.queue.write_buffer(
            &self.camera_buffer,
            672,
            bytemuck::cast_slice(&self.ocean_event_rows),
        );
        // TRUE screen pixel angle in light5_cone_inner.z (offset 552) -
        // Wave B, environment program increment 9. The cloud march's
        // ray-cone footprint used a hardcoded ~1 mrad guess
        // (CLOUD_PIX_ANG_SCREEN); the real value is 2*tan(fov/2)/rows,
        // which at 90 deg fov over 1387 rows is ~1.44 mrad - the guess
        // under-read the footprint by ~40% and every mip pick with it.
        let pix_ang = 2.0 * (camera.fov_degrees.to_radians() * 0.5).tan()
            / (self.config.height.max(1) as f32);
        self.queue
            .write_buffer(&self.camera_buffer, 552, bytemuck::bytes_of(&pix_ang));
        // The MARCH texture's own pixel angle (far rung): the temporal
        // march passes `ndc_step.y * tanf` to cloud_march_core, i.e. the
        // angle of one march-texture row (`config.height / cloud_res_div`),
        // and that is the footprint the profile's active-level range must
        // use. lib.rs reads it back for CloudProfileFrame.pix_ang_march.
        self.cloud_pix_ang_march.set(
            cloud_temporal::CloudProfileFrame::pix_ang_march(
                camera.fov_degrees as f64,
                self.config.height,
                self.cloud_res_div,
            ) as f32,
        );
        // Cloud-map basis anchor, octahedrally encoded into two spare pads
        // (496 = light2_cone_inner.x, 556 = light5_cone_inner.w). The
        // shell/octa shaders decode it in cloud_map_axis_world; the
        // composite pass receives the raw vector through its own uniform.
        // LOCKSTEP: the decode in 40-clouds.wgsl must mirror this encode.
        fn octa_encode(a: [f32; 3]) -> (f32, f32) {
            let denom = a[0].abs() + a[1].abs() + a[2].abs();
            let (mut ox, mut oz) = (a[0] / denom.max(1e-9), a[2] / denom.max(1e-9));
            if a[1] < 0.0 {
                let (fx, fz) = (ox, oz);
                ox = (1.0 - fz.abs()) * if fx >= 0.0 { 1.0 } else { -1.0 };
                oz = (1.0 - fx.abs()) * if fz >= 0.0 { 1.0 } else { -1.0 };
            }
            (ox, oz)
        }
        let (ox, oz) = octa_encode(self.cloud_map_anchor_local);
        self.queue
            .write_buffer(&self.camera_buffer, 496, bytemuck::bytes_of(&ox));
        self.queue
            .write_buffer(&self.camera_buffer, 556, bytemuck::bytes_of(&oz));
        // 12c extent: the frozen cos(theta_max) in light3_cone_inner.x
        // (offset 512), and the OLD params + resample flag in the legacy
        // (unused since the storage-buffer light list) camera.light3
        // position vec4 at offset 128: x/y = old anchor octa pair, z = old
        // cos(theta_max), w = 1 on the single frame after a re-anchor. The
        // octa pass reprojects its history through the old mapping on that
        // frame, which is what makes a re-anchor invisible.
        self.queue
            .write_buffer(&self.camera_buffer, 512, bytemuck::bytes_of(&self.cloud_map_cmax));
        // take() consumes the order: a second render_celestial_onto call in
        // the same frame (hi-res capture) must run its octa pass with the
        // flag DOWN - the history is already in the new mapping by then.
        let old_pads: [f32; 4] = match self.cloud_map_resample.take() {
            Some((a, cm)) => {
                let (oox, ooz) = octa_encode(a);
                [oox, ooz, cm, 1.0]
            }
            None => [0.0, 0.0, -1.0, 0.0],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 128, bytemuck::cast_slice(&old_pads));
        // Slice B translation reprojection: this frame's PLANET-LOCAL
        // camera displacement (from lib.rs, rotated to world axes) + flag
        // in the legacy camera.light4 position vec4 (offset 144). take():
        // one octa reprojection per delivered delta - the hi-res double
        // render reprojects by zero on its second pass. Parked cameras
        // deliver ~0 by construction (planet-local frame), so statics
        // stay converged; only real content-relative motion reprojects.
        // w encodes flag + march cadence phase: 0 = reprojection off (no
        // baseline), 1..4.9 = on with quarter-cadence phase (w - 1) - the
        // octa pass marches only the 2x2 cell matching the phase each
        // frame (4096-map brute force at the old 2048 per-frame cost).
        let phase = {
            let p = self.cloud_octa_phase.get();
            self.cloud_octa_phase.set(p.wrapping_add(1));
            (p % 4) as f32
        };
        // True-TELEPORT test for the 12e resolve (adversarial review of the
        // march/resolve split, finding 1): the resolve's screen history is
        // translation-exact via per-pixel first-hit distances, so sustained
        // fast flight does NOT invalidate it - only a jump large enough
        // that the reprojection itself is meaningless (~15 degrees of
        // parallax at the slab distance, mirroring the octa pass's own
        // per-texel teleport guard). Coupling snap to the CADENCE sentinel
        // below (threshold ~2 screen pixels of parallax, ~8 m/frame near
        // the deck) would drop the history on EVERY frame of an ordinary
        // approach flight and hand the operator raw unconverged march
        // static - the exact regression 12e exists to cure.
        let mut resolve_teleport = false;
        let delta_pads: [f32; 4] = match self.cloud_reproj_delta.take() {
            Some(d) if self.cloud_temporal_mat.is_some() => {
                let d2 = d[0] * d[0] + d[1] * d[1] + d[2] * d[2];
                if let Some(f) = self.cloud_composite_frame.as_ref() {
                    let eye = camera.effective_position();
                    let dc = ((eye.x - f.center[0]).powi(2)
                        + (eye.y - f.center[1]).powi(2)
                        + (eye.z - f.center[2]).powi(2))
                    .sqrt()
                        - f.rt * f.planet_r;
                    let d_slab = dc.max(3.0e3);
                    let tele = 0.25 * d_slab;
                    resolve_teleport = d2 > tele * tele;
                }
                // Cadence-suspension threshold, ANGULAR not absolute
                // (ghost-echo round 6): a cadence-skipped block re-warps
                // its own already-warped content for up to 3 frames, and
                // iterated bilinear warping at more than ~2 map texels of
                // shift per frame smears block-wise ghost terraces (the
                // operator's fading "old mirrors" during sustained
                // sub-teleport flight - the old flat 2 km threshold let
                // 10-60-texel shifts through). Suspend cadence (sentinel
                // w = 9: march everything) whenever the frame delta
                // exceeds ~2 texels of parallax at the cloud slab's
                // distance; correctness costs frames only while moving
                // that fast.
                // SCREEN-RELATIVE threshold + small-disc veto (the
                // deep-space lag report): ghosting only matters at the
                // scale a SCREEN pixel can show, so the shift budget is
                // 2x the larger of the map texel and the screen pixel
                // angle - at orbit that is ~9x more headroom than the
                // map-texel form, which had the sentinel firing on every
                // frame of ordinary space cruising and full-rate
                // marching 16.7M texels for a few-hundred-px disc. And
                // when the planet is small on screen (sentinel_ok false,
                // px < 700), the sentinel never fires at all: cadence
                // ghosting on a small disc is sub-pixel, while the
                // march-all cost is the lag.
                let sentinel = self
                    .cloud_composite_frame
                    .as_ref()
                    .map(|f| {
                        if !f.sentinel_ok {
                            return false;
                        }
                        let eye = camera.effective_position();
                        let dc = ((eye.x - f.center[0]).powi(2)
                            + (eye.y - f.center[1]).powi(2)
                            + (eye.z - f.center[2]).powi(2))
                        .sqrt()
                            - f.rt * f.planet_r;
                        let d_slab = dc.max(3.0e3);
                        let k = (1.0 - f.cmax).clamp(1.0e-3, 2.0);
                        let texel_ang = (2.0 * k).sqrt() / 4096.0;
                        let screen_ang = 2.0
                            * (camera.fov_degrees.to_radians() * 0.5).tan()
                            / (self.config.height.max(1) as f32);
                        let thresh = 2.0 * texel_ang.max(screen_ang) * d_slab;
                        // EDGE-TRIGGERED (v0.1246): require the level AND a
                        // spike vs last frame. Under the sustained planet-
                        // spin content sweep (km-scale delta EVERY frame on
                        // the 20-minute day) the old level trigger fired
                        // continuously: cadence suspended, 16.7M marches per
                        // frame, FPS pinned ~7 - and the low FPS made each
                        // per-frame delta bigger, locking the spiral. A
                        // sweep's reprojection is geometrically valid (a
                        // coherent fetch N texels away); only a genuine JUMP
                        // (teleport/re-park) invalidates history, and a jump
                        // is a spike: delta far above the running level.
                        let prev2 = self.cloud_prev_delta2.replace(d2);
                        d2 > thresh * thresh && d2 > prev2 * 16.0 + 1.0
                    })
                    .unwrap_or(false);
                if sentinel {
                    [d[0], d[1], d[2], 9.0]
                } else {
                    [d[0], d[1], d[2], 1.0 + phase]
                }
            }
            _ => [0.0, 0.0, 0.0, 0.0],
        };
        // Reprojection diagnostics (slice B bring-up): a parked camera must
        // read ~0 here. Throttled ~2 s; drop after the operator confirms
        // the smear is dead.
        {
            let d2 = delta_pads[0] * delta_pads[0]
                + delta_pads[1] * delta_pads[1]
                + delta_pads[2] * delta_pads[2];
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now_s = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now_s != LAST.swap(now_s, std::sync::atomic::Ordering::Relaxed) {
                log::info!("[CloudReproj] frame delta {:.3} m", d2.sqrt());
            }
        }
        self.queue
            .write_buffer(&self.camera_buffer, 144, bytemuck::cast_slice(&delta_pads));
        // 12d near-regime screen reprojection: the PREVIOUS frame's camera
        // basis in the legacy light5/6/7 position vec4s (offsets 160/176/
        // 192, unused since the storage-buffer light list). light5.xyz =
        // prev forward, light6.xyz = prev right, light7.xyz = prev up;
        // light5.w = tan(fov/2), light6.w = aspect (both current - the
        // projection does not change frame to frame). The basis is taken
        // from the view MATRIX rows, so surface-mode and world-Y cameras
        // both reproject through exactly what the GPU rendered with. On
        // the first frame (no stored basis) the current basis is written,
        // which makes reprojection the identity - correct for a fresh
        // history.
        {
            // The SAME basis convention the fullscreen composite ray-casts
            // with (proven by its slab-geometry discards landing exactly
            // right): forward()/right() and up = right x fwd. The first
            // cut extracted rows from the view matrix and produced rays
            // pointing INTO the planet - every under-deck sky pixel died
            // on the march's ground-occlusion gate (the magenta-sentinel
            // forensics, 2026-08-23).
            // ROLL-AWARE basis (v0.1243, origin audit #19): forward()/right()
            // ignore flight ROLL and camera-mode transition interpolation,
            // both of which the rendered frame's view_matrix() applies
            // (rolled_up). Every cloud ray built from the unrolled basis was
            // rotated about the view axis relative to the scene it registers
            // against - a misregistration radiating from the view centre
            // whenever the camera rolls (the fly band the operator lives in).
            // The view matrix's rotation rows ARE the camera axes: row0 =
            // right, row1 = up, row2 = -forward. Identical to the old basis
            // at zero roll outside transitions.
            let vm = camera.view_matrix();
            let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
            let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
            let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
            let cur: [[f32; 3]; 3] = [
                [fwd.x, fwd.y, fwd.z],
                [right.x, right.y, right.z],
                [up.x, up.y, up.z],
            ];
            let prev = self.cloud_prev_basis.replace(Some(cur)).unwrap_or(cur);
            let tanf = (camera.fov_degrees.to_radians() * 0.5).tan();
            let aspect = self.config.width.max(1) as f32 / self.config.height.max(1) as f32;
            // CURRENT basis in light0/1/2 (offsets 80/96/112, also unused):
            // the screen pass builds its pixel rays analytically from
            // these instead of shell mesh fragments (chord-sag hazard).
            let p0: [f32; 4] = [cur[0][0], cur[0][1], cur[0][2], 0.0];
            let p1: [f32; 4] = [cur[1][0], cur[1][1], cur[1][2], 0.0];
            let p2: [f32; 4] = [cur[2][0], cur[2][1], cur[2][2], 0.0];
            self.queue
                .write_buffer(&self.camera_buffer, 80, bytemuck::cast_slice(&p0));
            self.queue
                .write_buffer(&self.camera_buffer, 96, bytemuck::cast_slice(&p1));
            self.queue
                .write_buffer(&self.camera_buffer, 112, bytemuck::cast_slice(&p2));
            let p5: [f32; 4] = [prev[0][0], prev[0][1], prev[0][2], tanf];
            let p6: [f32; 4] = [prev[1][0], prev[1][1], prev[1][2], aspect];
            // light7.w = frame counter for the march's subpixel-jitter
            // sequence (12e). Wrapped so the f32 stays exact.
            let fidx = (self.cloud_octa_phase.get() % 2048) as f32;
            let p7: [f32; 4] = [prev[2][0], prev[2][1], prev[2][2], fidx];
            self.queue
                .write_buffer(&self.camera_buffer, 160, bytemuck::cast_slice(&p5));
            self.queue
                .write_buffer(&self.camera_buffer, 176, bytemuck::cast_slice(&p6));
            self.queue
                .write_buffer(&self.camera_buffer, 192, bytemuck::cast_slice(&p7));
            // Stash the resolve pass's camera state (12e): the SAME motion
            // delta the octa pads carry + the prev basis, consumed by
            // run_cloud_screen_passes this frame. Snap on the teleport
            // sentinel (w = 9: no history is valid).
            self.cloud_resolve_frame.set(cloud_resolve::CloudResolveFrame {
                prev_dpos: [delta_pads[0], delta_pads[1], delta_pads[2]],
                prev_basis: prev,
                snap: resolve_teleport || self.cloud_temporal_off,
                motion: self.cloud_resolve_motion.take(),
            });
        }
        // Near-regime arming mix in light7_color.x (offset 320; the whole
        // light*_color block is legacy-unread, v0.1245): the octa pass
        // gates its full-rate cadence on the map actually being the
        // visible renderer - inside/under the deck the near arm owns the
        // sky and full-rate marching the occluded map was most of the
        // in-layer frame cost.
        self.queue
            .write_buffer(&self.camera_buffer, 320, bytemuck::bytes_of(&self.cloud_near_mix));
        // Underwater extinction in light5_cone_inner.y (offset 548), v0.1054.
        self.queue
            .write_buffer(&self.camera_buffer, 548, bytemuck::bytes_of(&self.underwater_ext));
        // Sea sphere in light6_cone_inner.xyzw (offset 560), v0.1061.
        self.queue
            .write_buffer(&self.camera_buffer, 560, bytemuck::cast_slice(&self.sea_sphere));
        // Foliage wind in light7_cone_inner.xyzw (offset 576), v0.1080:
        // xyz = world wind direction (unit), w = speed m/s. Must land AFTER
        // the celestial_uniforms() stamp (same trap as the fill light above).
        self.queue
            .write_buffer(&self.camera_buffer, 576, bytemuck::cast_slice(&self.foliage_wind));
        // Fill DIRECTION, COLOUR and intensity (v0.998 intensity, v0.1052 the
        // rest). This pass stamps camera.celestial_uniforms() over the whole
        // buffer first, which carries the DEFAULT fill - so the fill that
        // lib.rs sets each frame never reached anything the celestial pass
        // draws, and planet terrain is drawn in this pass. That is the same
        // shape of bug as the hardcoded sun below: v0.1052 aims the fill at the
        // real Moon after sunset to give night a key light, and until now that
        // aim and colour were silently discarded here while a literal 0.6 was
        // used for the strength. Re-poke all of it.
        //
        // fill_direction xyz+w at offset 640, fill_color rgb at 656.
        let (fdir, fcol, fint) = self.cur_fill;
        let fill_dw = [fdir[0], fdir[1], fdir[2], fint * self.fill_scale.clamp(0.0, 1.0)];
        self.queue
            .write_buffer(&self.camera_buffer, 640, bytemuck::cast_slice(&fill_dw));
        self.queue
            .write_buffer(&self.camera_buffer, 656, bytemuck::cast_slice(&fcol));
        // Aerial perspective params (v0.916) in the unused per-light cone
        // pads: [1].y sigma (484), [1].z slant cap (488), [2].yzw sky color
        // (500), [3].yzw camera radial up (516). The interior passes'
        // full uniform write zeroes these, so rooms never fog.
        self.queue
            .write_buffer(&self.camera_buffer, 484, bytemuck::bytes_of(&self.aerial_sigma));
        self.queue
            .write_buffer(&self.camera_buffer, 488, bytemuck::bytes_of(&self.aerial_slant_cap));
        // W1: the water's sky-mirror altitude gate rides the last free pad
        // of light1_cone_inner (.w, offset 492 - beside its aerial
        // siblings; verified unread before this).
        self.queue
            .write_buffer(&self.camera_buffer, 492, bytemuck::bytes_of(&self.water_lut_gate));
        self.queue
            .write_buffer(&self.camera_buffer, 500, bytemuck::cast_slice(&self.aerial_sky));
        self.queue
            .write_buffer(&self.camera_buffer, 516, bytemuck::cast_slice(&self.aerial_up));
        // Light the bodies by the REAL Sun (v0.451): the full-uniform write above
        // stamps the default fake sun [0.3,1,0.5] at offset 608 (v0.639: shifted from 352 by
        // the +256-byte light_spot/light_cone_inner insertion), so re-poke it with the true
        // Earth->Sun direction. Now the planets' lit hemisphere faces the visible Sun disc
        // instead of a fixed up-and-right fake light. (The Sun body itself is emissive, so its
        // own shading is unaffected.)
        if sun_dir != Vec3::ZERO {
            // Intensity scaled by the camera-local day factor (BUG-057 #1):
            // this used to be a bare 2.5 day and night, so everything in the
            // celestial pass without its own terminator gate (trees, props -
            // all types except terrain's 12) was sunlit at midnight. The
            // shaders read w as 2.5 * day and normalize with * 0.4 where they
            // need the plain day factor.
            let sd = [sun_dir.x, sun_dir.y, sun_dir.z, 2.5_f32 * self.celestial_sun_day];
            // w carries the cloud clock (written above at 636); this full
            // vec4 write would stomp it back to a constant otherwise.
            // RGB is the TRANSMITTANCE-TINTED sun (cur_sun.1, fed by
            // lib.rs's atmosphere::sun_transmittance since v0.915): a
            // hardcoded [1.0, 0.97, 0.92] sat here from the v0.639 poke
            // onward, so sunset DIMMED the celestial pass (clouds,
            // terrain, water) but never reddened it - the same
            // stale-literal bug shape as the fake sun direction (v0.451)
            // and the fill light (v0.1052). Golden hour now reaches
            // everything this pass draws.
            let scol = self.cur_sun.1;
            let sc = [scol[0], scol[1], scol[2], cloud_t];
            self.queue.write_buffer(&self.camera_buffer, 608, bytemuck::cast_slice(&sd));
            self.queue.write_buffer(&self.camera_buffer, 624, bytemuck::cast_slice(&sc));
        }

        // ── Sun shadow pass (v0.899) ── near-field ortho depth from the sun,
        // rendered before the main pass so every lit fragment this frame can
        // sample it. Texel-snapped so a drifting camera never swims the map.
        let shadow_on = self.sun_shadows && sun_dir != Vec3::ZERO;
        {
            const SHADOW_MAP_SIZE: f32 = SUN_SHADOW_MAP_SIZE;
            let extent = SUN_SHADOW_EXTENT_M;
            let sun = sun_dir.normalize();
            let center = camera.effective_position();
            let up = if sun.y.abs() > 0.95 { Vec3::Z } else { Vec3::Y };
            let view_m = Mat4::look_at_rh(center + sun * 4000.0, center, up);
            // DEPTH RANGE, tightened v0.1058. The light sits at center + sun*4000
            // and everything that can cast into a +/-1500 m box lies within
            // about 1500 m of that centre plane, but the projection mapped
            // 0.1..8000 m onto 0..1 of depth - so 3.2x of the precision was
            // spent on empty space in front of and behind the scene.
            //
            // That matters because the shader's shadow bias is expressed in NDC
            // (0.0006 flat, 0.0025 at grazing), so its WORLD size is the bias
            // times the depth range: 4.8 m flat and 20 m grazing. A conifer is
            // 20-30 m tall, so most of a tree's shadow - and all of a trunk's -
            // fell inside the bias and was erased. Same for any modest terrain
            // relief and for wave crests, which are 10 m at most.
            //
            // Fitting the range to the box takes it to 2400..5600 m = 3200 m,
            // so the same NDC bias is 1.9 m flat and 8 m grazing: 2.5x tighter
            // shadows everywhere, for one line and no perf cost. A margin of
            // 1.4x extent covers casters standing above the box (a 30 m tree on
            // a ridge) and the texel snap.
            let z_margin = extent * 1.4;
            let z_near = (4000.0 - extent - z_margin).max(1.0);
            let z_far = 4000.0 + extent + z_margin;
            let proj = Mat4::orthographic_rh(-extent, extent, -extent, extent, z_near, z_far);
            let mut vp = proj * view_m;
            // Texel snap: shift so the world origin lands on a texel grid.
            let ndc_texel = 2.0 / SHADOW_MAP_SIZE;
            let origin = vp * glam::Vec4::new(0.0, 0.0, 0.0, 1.0);
            let snap = |v: f32| (v / ndc_texel).round() * ndc_texel - v;
            vp = Mat4::from_translation(Vec3::new(snap(origin.x), snap(origin.y), 0.0)) * vp;
            // v0.1057: build the light camera from the REAL celestial uniforms
            // and overwrite only view_proj, instead of starting from zeroed.
            // Same class of bug as the hardcoded sun and the discarded fill
            // above: a zeroed uniform means the type-16 water vertex branch sees
            // FFT-mode 0, both cascade anchors 0, weld K 0, the wave clock 0 and
            // view_pos 0 - so if water ever casts into this map it rasterises a
            // DIFFERENT sea than the colour pass draws, and the shadows land on
            // the wrong water. Everything the water VS reads has to be re-poked
            // at the same offsets the colour pass uses on camera_buffer. If a
            // refactor re-zeroes this, wave shadows silently go wrong rather
            // than absent, which is much harder to spot.
            let mut light_u = camera.celestial_uniforms();
            light_u.view_proj = vp.to_cols_array_2d();
            self.queue
                .write_buffer(&self.light_camera_buffer, 0, bytemuck::bytes_of(&light_u));
            // 464 = ground_anchor (FFT flag + the 64 m cascade-A anchor).
            self.queue.write_buffer(
                &self.light_camera_buffer,
                464,
                bytemuck::cast_slice(&ground_anchor),
            );
            // 528 = cascade-B anchor + the water weld K.
            self.queue.write_buffer(
                &self.light_camera_buffer,
                528,
                bytemuck::cast_slice(&ocean_anchor256),
            );
            // 544 = live sea crest, which the VS shoal fade reads.
            self.queue.write_buffer(
                &self.light_camera_buffer,
                544,
                bytemuck::bytes_of(&self.sea_crest_m),
            );
            // 636 = the wave clock, parked in sun_color.w.
            self.queue.write_buffer(
                &self.light_camera_buffer,
                636,
                bytemuck::bytes_of(&time_s),
            );
            // 576 = foliage wind (v0.1080). The shadow pass runs the SAME
            // vs_main with the SAME type-20 wind branch off THIS buffer; omit
            // this and the shadow map records a near-upright fallback-wind
            // tree while the colour pass draws one leaning metres downwind
            // (up to ~10 texels of detachment at storm speeds).
            self.queue.write_buffer(
                &self.light_camera_buffer,
                576,
                bytemuck::cast_slice(&self.foliage_wind),
            );
            let mut su = [0.0_f32; 24];
            su[..16].copy_from_slice(&vp.to_cols_array());
            su[16] = if shadow_on { 1.0 } else { 0.0 };
            su[17] = self.shadow_strength.clamp(0.0, 1.0);
            su[18] = 1.0 / SHADOW_MAP_SIZE;
            // params.w (v0.912): the tree-model radius - terrain tree CARDS
            // hide inside it so the real 3D conifers replace them cleanly.
            su[19] = self.tree_card_hide_m;
            // params2.x (v0.924): tree-card far cutoff (vegetation LOD slider).
            su[20] = self.tree_card_far_m.max(1.0);
            // params2.y = sky-view LUT valid this frame (stage 3c gate): the
            // table only re-renders near an atmosphere body; elsewhere it is
            // stale and the megashader must not blend it in.
            su[21] = if self.sky_view_uniform.is_some() { 1.0 } else { 0.0 };
            // params2.zw = light-tile pixel sizes (clustering L1b); zero = the
            // classic full light loop.
            su[22] = self.tile_px.0;
            su[23] = self.tile_px.1;
            self.queue
                .write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::cast_slice(&su));
        }
        // Batched patch instances (draw-batching increment 1): uploaded ONCE
        // ahead of both encoders -- the shadow pass and the celestial pass
        // index the same storage buffer, so a culled shadow subset still
        // addresses its instances by their full-list position.
        let patch_batch_n = if let Some(arena) = self.patch_arena.as_ref() {
            let n = self.patch_draws.len().min(patch_arena::MAX_PATCH_DRAWS);
            if n > 0 {
                let inst: Vec<patch_arena::PatchInstance> = self.patch_draws[..n]
                    .iter()
                    .map(|d| {
                        patch_arena::PatchInstance::new([
                            d.position.x,
                            d.position.y,
                            d.position.z,
                            d.fade,
                        ])
                    })
                    .collect();
                self.queue
                    .write_buffer(&arena.instance_buf, 0, bytemuck::cast_slice(&inst));
                self.queue.write_buffer(
                    &arena.batch_buf,
                    0,
                    bytemuck::bytes_of(&self.patch_batch_rot.to_cols_array_2d()),
                );
                // Increment 2: indirect args for the one-submit path. Each
                // entry's first_instance selects the instance-attribute
                // element (honored in hardware; the builtin would not be).
                if self.patch_indirect {
                    let args: Vec<patch_arena::IndirectArgs> = self.patch_draws[..n]
                        .iter()
                        .enumerate()
                        .map(|(i, d)| patch_arena::IndirectArgs {
                            index_count: d.slot.icount,
                            instance_count: 1,
                            first_index: d.slot.istart,
                            base_vertex: d.slot.vstart as i32,
                            first_instance: i as u32,
                        })
                        .collect();
                    self.queue
                        .write_buffer(&arena.indirect_buf, 0, bytemuck::cast_slice(&args));
                }
            }
            n
        } else {
            0
        };
        if shadow_on {
            // Object uniforms uploaded HERE cover both the shadow pass and
            // the main pass below (same list, same offsets).
            self.upload_object_uniforms(objects.iter().chain(transparent.iter()));
            let mut senc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Shadow Encoder"),
                });
            {
                let mut pass = senc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Sun Shadow Pass"),
                    timestamp_writes: self.pass_timer("gpu.shadow"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &self.shadow_map_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(1.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    ..Default::default()
                });
                pass.set_pipeline(&self.pipeline.shadow_pipeline);
                // Slot 1: zero per-instance data for classic draws (increment 2).
                pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                pass.set_bind_group(0, &self.light_camera_bind_group, &[]);
                // Group 3 for casters with NO texture of their own: the
                // dummy-depth variant (the real shadow map is this pass's
                // write target; wgpu rejects sampling it here). fs_shadow
                // reads binding 14, the tree atlas, from it.
                //
                // v0.1108: a TEXTURED caster now rebinds its own shadow-safe
                // group below. Until it did, fs_shadow's type-19 and type-21
                // branches sampled the 1x1 WHITE fallback at binding 0 - alpha
                // 1 everywhere, so neither discard could ever fire and every
                // near-tree cluster card still stamped a solid quad into the
                // sun map. The branches existed; the texture did not.
                pass.set_bind_group(3, &self.shadow_pass_texture_bind_group, &[]);
                let uniform_align = 256_u64;
                let mut bound_material = usize::MAX;
                // Near-field caster cull (v0.899; tightened v0.911, perf
                // audit #2): the ortho box covers 1.5 km around the camera,
                // so a caster can only matter if its anchor sits within the
                // box plus the largest patch's own reach. 6 km covers the
                // coarsest horizon patch that could still poke a triangle
                // into the box; the old 65 km bound re-rasterized thousands
                // of far patches into the 4096 map every frame for nothing.
                let cast_center = camera.effective_position();
                for (i, obj) in objects.iter().enumerate() {
                    if i >= MAX_OBJECTS {
                        break;
                    }
                    if (obj.position - cast_center).length_squared() > 6_000.0_f32 * 6_000.0 {
                        continue;
                    }
                    let mesh = match self.meshes.get(obj.mesh) {
                        Some(m) => m,
                        None => continue,
                    };
                    let material = match self.materials.get(obj.material) {
                        Some(m) => m,
                        None => continue,
                    };
                    // v0.1106 (why + cost: Pipeline::shadow_for): a crossfading LOD
                    // dithers and a cutout caster may alpha-discard, so those two
                    // take fs_shadow; the rest keep the depth-only fast path.
                    //
                    // v0.1108 narrowed the second half from "has an albedo
                    // texture" to "fs_shadow actually has a discard for this
                    // material TYPE". The old test was wrong in both
                    // directions: baked bark (22) and textured planet meshes
                    // are opaque and paid a fragment stage that can never
                    // discard, while an untextured terrain-patch material (12)
                    // took the depth-only path even though its sprite tree
                    // cards discard on the atlas alpha.
                    pass.set_pipeline(
                        self.pipeline
                            .shadow_for(obj.fade != 0.0 || material.casts_cutout_shadow()),
                    );
                    let dynamic_offset = (uniform_align as u32) * (i as u32);
                    pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                    if bound_material != obj.material {
                        bound_material = obj.material;
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        // Group 3 follows the material, so it rides the same
                        // change check: the material's SHADOW-SAFE albedo when
                        // it has one, the pass-wide fallback otherwise. This is
                        // the bind that makes the type-19 / type-21 discards in
                        // fs_shadow read real texels for the first time.
                        pass.set_bind_group(
                            3,
                            material
                                .shadow_albedo_group()
                                .unwrap_or(&self.shadow_pass_texture_bind_group),
                            &[],
                        );
                    }
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
                // WATER CASTERS (v0.1057). The wave shell lives in the
                // TRANSPARENT list, which this pass never walked - so a 10 m
                // crest cast nothing and the trough behind it stayed fully lit.
                // That absent self-shadowing is a large part of why a storm sea
                // read as a flat pattern rather than relief. Object uniforms for
                // the transparent list were already uploaded by the chain above,
                // at slot objects.len() + i, so the dynamic offset is the only
                // thing that changes. Group 3 for this pass binds the REAL FFT
                // tile at binding 15, so the vertex displacement matches the
                // colour pass exactly (and v0.1057 also stopped this pass from
                // zeroing the anchors it needs to do that).
                //
                // Tighter cull than the 6 km above: the ortho box is 1.5 km
                // across at 0.73 m/texel, and only crests inside it land at a
                // useful resolution.
                if !self.water_caster_mats.is_empty() {
                    pass.set_pipeline(&self.pipeline.shadow_pipeline);
                    pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                    for (i, obj) in transparent.iter().enumerate() {
                        let slot = objects.len() + i;
                        if slot >= MAX_OBJECTS {
                            break;
                        }
                        if !self.water_caster_mats.contains(&obj.material) {
                            continue;
                        }
                        if (obj.position - cast_center).length_squared()
                            > 2_500.0_f32 * 2_500.0
                        {
                            continue;
                        }
                        let mesh = match self.meshes.get(obj.mesh) {
                            Some(m) => m,
                            None => continue,
                        };
                        let material = match self.materials.get(obj.material) {
                            Some(m) => m,
                            None => continue,
                        };
                        let dynamic_offset = (uniform_align as u32) * (slot as u32);
                        pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                        if bound_material != obj.material {
                            bound_material = obj.material;
                            pass.set_bind_group(2, &material.bind_group, &[]);
                            // Group 3 explicitly (v0.1108): the classic loop
                            // above may have left a cluster card's group bound,
                            // and water must read binding 15 - the FFT tile -
                            // from a group it chose, not one it inherited.
                            pass.set_bind_group(
                                3,
                                material
                                    .shadow_albedo_group()
                                    .unwrap_or(&self.shadow_pass_texture_bind_group),
                                &[],
                            );
                        }
                        pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            mesh.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                    }
                }
                // Batched patch casters: same 6 km cull, one bind, one
                // draw per near patch. Instance index = full-list position
                // (the storage buffer holds ALL of this frame's draws).
                if patch_batch_n > 0 {
                    let arena = self.patch_arena.as_ref().expect("patch_batch_n > 0");
                    pass.set_pipeline(&self.pipeline.patch_shadow_pipeline);
                    pass.set_bind_group(1, &arena.bind_group, &[]);
                    // Groups 2 and 3 explicitly: the classic caster loop above
                    // may not have bound any material (zero classic casters),
                    // and if it did, the group 3 left bound belongs to whatever
                    // drew last. The patch PSO always runs fs_shadow, so its
                    // group 3 matters - binding 0 for a textured planet's
                    // type-12 cutout, binding 14 for the sprite tree cards.
                    if let Some(material) = self.materials.get(self.patch_batch_material) {
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        pass.set_bind_group(
                            3,
                            material
                                .shadow_albedo_group()
                                .unwrap_or(&self.shadow_pass_texture_bind_group),
                            &[],
                        );
                    }
                    pass.set_vertex_buffer(0, arena.vertex_buf.slice(..));
                    pass.set_vertex_buffer(1, arena.instance_buf.slice(..));
                    pass.set_index_buffer(arena.index_buf.slice(..), wgpu::IndexFormat::Uint32);
                    // Per-draw loop stays here even with indirect support:
                    // the 6 km caster cull keeps this to a few dozen draws,
                    // and a separate culled args buffer isn't worth it.
                    for (i, d) in self.patch_draws[..patch_batch_n].iter().enumerate() {
                        if (d.position - cast_center).length_squared() > 6_000.0_f32 * 6_000.0 {
                            continue;
                        }
                        pass.draw_indexed(
                            d.slot.istart..d.slot.istart + d.slot.icount,
                            d.slot.vstart as i32,
                            (i as u32)..(i as u32 + 1),
                        );
                    }
                }
            }
            self.queue.submit(std::iter::once(senc.finish()));
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Celestial Encoder"),
            });

        // Sky-view LUT (stage 3b-2): refresh the distant-sky table first so
        // everything later in the frame could sample this frame's sky. Only
        // runs frame-locked near an atmosphere body (the uniform is stashed
        // by the lib.rs atmosphere hook; None elsewhere = zero cost).
        if let Some(u) = self.sky_view_uniform {
            self.sky_view.encode(&self.queue, &mut encoder, &u, self.pass_timer("gpu.sky_view"));
        }

        // ── Temporal cloud octa pass (clouds phase 4) ── re-march + EMA the
        // direction-indexed cloud map BEFORE the main pass so this frame's
        // composite samples this frame's accumulation. The object uniforms
        // were staged above (shells continue the index range after the
        // opaque list); the pass binds the cloud SHELL's slot so obj_model()
        // gives the march its planet frame, and the group-3 with the
        // ping-pong PARTNER in the albedo slot supplies the history.
        // FAR side (12d/12g): runs whenever the crossfade still gives the
        // octa map any weight (mix < 1) - fully near, the half-res screen
        // pass replaces it entirely (marching 16.7M map texels for a
        // full-screen planet was the near-planet lag, and the direction
        // cache is the ghost family).
        // ONE RENDERER (v0.1250): the octa map is DORMANT - the per-pixel
        // screen march owns the whole sky at every altitude (see the
        // near_mix note in lib.rs). The pass machinery stays in the tree
        // for reference but never dispatches; its texture remains
        // zero-initialized, so the composite's map backdrop contributes
        // nothing.
        // ── THE OCTA PASS IS DELETED (v0.1261) ──
        // It stopped dispatching in v0.1250 and its texture stopped being
        // composited in v0.1260; the operator still saw the artifact, so
        // the subsystem is gone entirely - no pass, no allocation, no
        // binding. Nothing that never runs can contribute to a pixel.
        // light7_color.z (offset 328): the rosette-bisect diagnostic channel
        // (v0.1249; showcase map_diag - EMA-bypassed raw-quantity render).
        self.queue
            .write_buffer(&self.camera_buffer, 328, bytemuck::bytes_of(&self.cloud_map_diag));
        // light7_color.w (offset 332): the operator's dither taste toggle
        // (v0.1254.3; showcase cloud_dither - 1.0 = dither OFF).
        // Bit 0 (1.0) = dither off, bit 1 (2.0) = shape frame off.
        let dith = (if self.cloud_dither_off { 1.0f32 } else { 0.0 })
            + (if self.cloud_shape_off { 2.0f32 } else { 0.0 })
            + (if self.cloud_chord_foot { 4.0f32 } else { 0.0 })
            + (if self.cloud_world_shape_lod { 8.0f32 } else { 0.0 })
            + (if self.cloud_ring_cure_off { 16.0f32 } else { 0.0 })
            + (if self.cloud_uniform_step { 32.0f32 } else { 0.0 })
            + (if self.cloud_wide_edge { 64.0f32 } else { 0.0 })
            + (if self.cloud_est { 128.0f32 } else { 0.0 })
            + (if self.cloud_warp_bl { 256.0f32 } else { 0.0 })
            + (if self.cloud_norm_floor { 1024.0f32 } else { 0.0 })
            + (if self.cloud_iso_step { 512.0f32 } else { 0.0 })
            + (if self.cloud_thin_deck { 2048.0f32 } else { 0.0 })
            + (if self.cloud_hv_warp { 4096.0f32 } else { 0.0 })
            // v0.1283: bits 13-15 are an INDEX (first set bisect wins; the
            // shader reads cloud_bisect_index), bits 16-17 are FREE.
            + (8192.0f32
                * (if self.cloud_no_detail { 1.0 } else if self.cloud_no_puff { 2.0 }
                   else if self.cloud_no_cell { 3.0 } else if self.cloud_no_fray { 4.0 }
                   else if self.cloud_no_bdrop { 5.0 } else { 0.0 }))
            // Bit 16 (65536, v0.1286): the sun-shadow cache is READABLE
            // this frame - toggle on, near regime armed, atlas planned.
            // Raised from the same predicate that binds the atlas at
            // group 3 below, so the shader can never read a slot that
            // holds the 1x1 fallback texture.
            + (if self.cloud_light_active().is_some() { 65536.0f32 } else { 0.0 })
            + (if self.cloud_sharp_base { 262144.0f32 } else { 0.0 })
            + (if self.cloud_relief_fade { 524288.0f32 } else { 0.0 })
            + (if self.cloud_deep_rung { 1048576.0f32 } else { 0.0 })
            + (if self.cloud_checker { 2097152.0f32 } else { 0.0 })
            + (if self.cloud_ms { 4194304.0f32 } else { 0.0 })
            + (if self.cloud_field { 8388608.0f32 } else { 0.0 })
            + (if self.cloud_body_cache { 131072.0f32 } else { 0.0 });
        self.queue
            .write_buffer(&self.camera_buffer, 332, bytemuck::bytes_of(&dith));
        // light3_color (offset 256) and light4_color (offset 272), both
        // unread by every shader until v0.1286: the sun-shadow cache
        // window pads, (anchor_x, anchor_y, anchor_z, cell_h) for the fine
        // and the coarse window in the march's p-units (planet-centred
        // shell object space). f64 all the way in CloudLightState::pads,
        // f32 only here. Written EVERY frame (zeros when no cache) so a
        // stale window can never survive a regime change.
        {
            let (fine, coarse) = self
                .cloud_light_cache
                .as_ref()
                .map(|lc| lc.state.pads())
                .unwrap_or(([0.0; 4], [0.0; 4]));
            self.queue
                .write_buffer(&self.camera_buffer, 256, bytemuck::cast_slice(&fine));
            self.queue
                .write_buffer(&self.camera_buffer, 272, bytemuck::cast_slice(&coarse));
        }
        // light2_color (offset 240; unread by every shader until the far
        // rung): the cloud PROFILE pad `(ground_I_0, ground_J_0, knob,
        // flags)`, all exact integers in f32, f64 all the way in
        // CloudProfileState (the ground cell) and narrowed only here.
        // Written EVERY frame after the bulk camera upload (which lands
        // zeros there first); the ground and knob lanes are zeros when no
        // atlas exists, so the shader's knob branch is never taken without
        // an atlas at binding 14.
        // D3 (2026-09-06): the flags lane ALSO carries the built-body
        // top-bound dev bit (bit 12 = 4096, `CLOUD_FR_FLAG_TOP_BOUND`),
        // atlas or not: without the atlas the pad is (0, 0, 0, 4096) when
        // the bit is on. The bit is what `cloud_top_bound_on()` reads in
        // the body and the step law; it never touches the validity bits
        // 0..8 (unit test `top_bound_bit_decodes_as_bit_12_...`), and the
        // gate's cells run it at knob 0 where no atlas is ever created.
        {
            let mut pad = self
                .cloud_profile_cache
                .as_ref()
                .map(|pc| pc.state.pads())
                .unwrap_or([0.0; 4]);
            pad[3] = cloud_temporal::cloud_fr_flags_with_top_bound(pad[3] as u32, self.cloud_top_bound) as f32;
            self.queue
                .write_buffer(&self.camera_buffer, 240, bytemuck::cast_slice(&pad));
        }

        // ── 12e NEAR march + resolve ── two passes replace 12d's single
        // cadence+history hybrid (whose one blend constant could not both
        // converge the jittered march AND kill stale history - the
        // operator's "static" + residual ghosting on the first flight):
        //  1. MARCH: every pixel of the quarter-res pair, every frame,
        //     subpixel-jittered analytic rays (no cadence, no history) -
        //     MRT premultiplied result + first-hit distance in km.
        //  2. RESOLVE: deep accumulation into the half-res ping-pong with
        //     VARIANCE-CLIPPED reprojected history - ghosts snap to the
        //     current neighbourhood in one frame while corroborated
        //     content converges ~8 frames deep.
        // The composite then samples the accumulation at each fragment's
        // own screen uv, unchanged. No direction cache exists anywhere in
        // this regime and there is no arming altitude.
        // DIAG4 (12d bring-up): once-per-second trace of the near-regime
        // chain - drop after the under-deck vanish is verified fixed.
        {
            static LAST: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let now_s = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if now_s != LAST.swap(now_s, std::sync::atomic::Ordering::Relaxed) {
                log::info!(
                    "[CloudScreen] near={} cs={} mat={:?} shell_found={:?}",
                    self.cloud_mode_near,
                    self.cloud_screen.is_some(),
                    self.cloud_temporal_mat,
                    self.cloud_temporal_mat.and_then(|m| {
                        transparent.iter().position(|o| o.material == m)
                    }),
                );
                // [CloudLight] (v0.1286): the sun-shadow cache's state at
                // 1 Hz - whether the march reads it this frame, where the
                // windows sit, how often they moved, and the bake mix.
                // Read this before believing any cache-on capture: active
                // must be true and full+partial must be climbing.
                match self.cloud_light_cache.as_ref() {
                    Some(lc) => {
                        let st = &lc.state;
                        let (fine, coarse) = st.pads();
                        log::info!(
                            "[CloudLight] on={} active={} anchor_p=({:.6},{:.6},{:.6}) cell_p={:.3e}/{:.3e} reanchors={} sun_rerefs={} full={} partial={} phase={} last={:?}",
                            self.cloud_light,
                            self.cloud_light_active().is_some(),
                            fine[0],
                            fine[1],
                            fine[2],
                            fine[3],
                            coarse[3],
                            st.reanchors,
                            st.sun_rerefs,
                            st.full_bakes.get(),
                            st.partial_bakes.get(),
                            st.phase.get(),
                            st.last_plan,
                        );
                    }
                    None => {
                        log::info!(
                            "[CloudLight] on={} active=false (no atlas) fed={}",
                            self.cloud_light,
                            self.cloud_light_frame.is_some(),
                        );
                    }
                }
                // [CloudProfile] (increment 4, the far rung): the profile's
                // state at 1 Hz - knob, ground cell, per-level valid / fill
                // cursor / scroll count, the global's cursor and mode, the
                // calibration, the truncation estimate and the atlas size.
                // Read this before believing any profile-on capture: active
                // must be true and the valid bits must be up.
                match self.cloud_profile_cache.as_ref() {
                    Some(pc) => {
                        let st = &pc.state;
                        let pad = st.pads();
                        let mut lv = String::new();
                        for (l, s) in st.levels.iter().enumerate() {
                            if !s.active.get() {
                                continue;
                            }
                            let fill = match s.fill_cursor.get() {
                                Some(c) => format!("fill@{c}"),
                                None => format!("rf@{:.0}", s.refresh_cursor.get()),
                            };
                            lv.push_str(&format!(
                                " L{l}:{}{}/{}/scr{}",
                                if s.valid.get() { "valid" } else { "FILLING" },
                                if s.valid.get() { format!("/{fill}") } else { String::new() },
                                s.fills.get(),
                                s.scrolled.replace(0),
                            ));
                        }
                        log::info!(
                            "[CloudProfile] knob={} top_bound={} active={} ground=({},{}) flags={}{} global={}{}@{:.0}/passes={} calib={} trunc={} mb={:.1}",
                            self.cloud_profile_knob,
                            // D3 dev bit: printed so a capture's log proves which arm ran.
                            self.cloud_top_bound,
                            self.cloud_profile_active().is_some(),
                            pad[0],
                            pad[1],
                            pad[3],
                            lv,
                            if st.global_valid.get() { "valid/" } else { "PENDING/" },
                            if st.global_pass_fast.get() { "fast" } else { "rolling" },
                            st.global_pass_cursor.get(),
                            st.global_passes.get(),
                            st.calib_valid.get(),
                            st.truncated_texels_estimate(),
                            (cloud_temporal::CLOUD_FR_ATLAS_W as f64
                                * cloud_temporal::CLOUD_FR_ATLAS_H as f64
                                * 4.0
                                * 4.0
                                / 3.0)
                                / 1.0e6,
                        );
                    }
                    None => {
                        log::info!(
                            "[CloudProfile] knob={} top_bound={} active=false (no atlas) fed={}",
                            self.cloud_profile_knob,
                            // D3 dev bit: the gate runs it at knob 0, so THIS branch is the one that logs it.
                            self.cloud_top_bound,
                            self.cloud_profile_frame.is_some(),
                        );
                    }
                }
            }
        }
        // ── THE FAR RUNG PASSES (perf increment 4) ── hoisted OUT of the
        // near block below and keyed on the cloud SHELL material, which
        // lib.rs sets on EVERY tier (the Low sheet reads the global map, so
        // the atlas must be baked even when no screen march runs). Bound
        // like the sun bake: camera, the shell's object slot (the same
        // planet frame), the cloud material; group 3 = whichever atlas mip
        // the fragment reads at binding 14. Order: the calibration (two
        // stages, on their own timer so the bake gate's MAX never counts
        // them), the bake (LoadOp::Load, scissored to the frame's rects),
        // the six mip passes when a global pass just completed.
        if let (Some(mat_idx), Some(pc)) = (self.cloud_shell_mat, self.cloud_profile_cache.as_ref()) {
            if let (Some(i), Some(material)) = (
                transparent.iter().position(|o| o.material == mat_idx),
                self.materials.get(mat_idx),
            ) {
                let slot = objects.len() + i;
                if slot < MAX_OBJECTS
                    && self.cloud_profile_knob != cloud_temporal::CLOUD_FR_KNOB_OFF
                    && self.cloud_profile_frame.is_some()
                {
                    self.upload_object_uniforms(objects.iter().chain(transparent.iter()));
                    let uniform_align = 256_u64;
                    let dyn_off = (uniform_align as u32) * (slot as u32);
                    // One fullscreen pass over one mip view, scissored: the
                    // shape every far-rung pass shares.
                    let run = |encoder: &mut wgpu::CommandEncoder,
                               label: &'static str,
                               timer: Option<&'static str>,
                               pipeline: &wgpu::RenderPipeline,
                               target: &wgpu::TextureView,
                               group3: &wgpu::BindGroup,
                               rects: &[(u32, u32, u32, u32)]| {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some(label),
                            // None = untimed (the mip burst; see below).
                            timestamp_writes: timer.and_then(|t| self.pass_timer(t)),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: target,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    // Load: every pass touches only its
                                    // rects; the rest of the mip (other
                                    // windows, the calibration areas in
                                    // mips 1 and 2) must survive.
                                    load: wgpu::LoadOp::Load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            ..Default::default()
                        });
                        pass.set_pipeline(pipeline);
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        pass.set_bind_group(1, &self.object_bind_group, &[dyn_off]);
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        pass.set_bind_group(3, group3, &[]);
                        for &(x, y, w, h) in rects {
                            if w == 0 || h == 0 {
                                continue;
                            }
                            pass.set_scissor_rect(x, y, w, h);
                            pass.draw(0..3, 0..1);
                        }
                    };
                    // The calibration (A1): stage 1 reads only the noise
                    // volumes (group 3 = the 1x1 fallback), stage 2 reads
                    // the mip-2 staging through binding 14.
                    if pc.state.take_calib() {
                        run(
                            &mut encoder,
                            "Cloud Profile Calib Pass",
                            Some("gpu.cloud_profile_calib"),
                            &self.pipeline.cloud_profile_calib_pipeline,
                            &pc.view_mip[2],
                            &self.default_texture_bind_group,
                            &[(
                                cloud_temporal::CLOUD_FR_CALIB_STAGE_X0,
                                cloud_temporal::CLOUD_FR_CALIB_STAGE_Y0,
                                cloud_temporal::CLOUD_FR_CALIB_STAGE_W,
                                cloud_temporal::CLOUD_FR_CALIB_STAGE_H,
                            )],
                        );
                        run(
                            &mut encoder,
                            "Cloud Profile Calib Reduce Pass",
                            Some("gpu.cloud_profile_calib"),
                            &self.pipeline.cloud_profile_calib_reduce_pipeline,
                            &pc.view_mip[1],
                            &pc.group_mip_src[2].colour,
                            &[(
                                cloud_temporal::CLOUD_FR_CALIB_X0,
                                cloud_temporal::CLOUD_FR_CALIB_Y0,
                                cloud_temporal::CLOUD_FR_CALIB_W,
                                cloud_temporal::CLOUD_FR_CALIB_H,
                            )],
                        );
                    }
                    // The bake: wall-clock dt drives the time-based cadence
                    // (the cloud clock may be pinned; the refresh must not
                    // stop with it). Group 3 = mip 1 (the calibration table)
                    // at binding 14; mip 0 is the attachment.
                    let now = std::time::Instant::now();
                    let dt_s = pc
                        .state
                        .last_bake
                        .replace(Some(now))
                        .map(|t| now.duration_since(t).as_secs_f64())
                        .unwrap_or(1.0 / 60.0);
                    let rects = pc.state.take_bake_rects(dt_s);
                    if !rects.is_empty() {
                        run(
                            &mut encoder,
                            "Cloud Profile Bake Pass",
                            Some("gpu.cloud_profile"),
                            &self.pipeline.cloud_profile_bake_pipeline,
                            &pc.view_mip[0],
                            &pc.group_mip_src[1].colour,
                            &rects,
                        );
                    }
                    // The global's mip chain, once per completed pass: mip
                    // m from mip m - 1, scissored to the global region at
                    // mip m (origin (0, 2560 >> m), size (6144 >> m,
                    // 1024 >> m)). Six passes, UNTIMED: the frame timer
                    // ring holds 32 passes and a world frame already uses
                    // about 22; six more on the (60 s cadence) mip frame
                    // would push the cloud passes recorded after this block
                    // (gpu.cloud_screen, the G6 bar) past the cap, where
                    // they silently read as zero. The chain over the
                    // 6144 x 1024 global region is a few tenths of a
                    // millisecond once a minute; gpu.cloud_profile is the
                    // bake alone (a deviation from the contract's "bake +
                    // mips", recorded in the far-rung report).
                    if pc.state.mips_pending.replace(false) {
                        for m in 1..cloud_temporal::CLOUD_FR_GLOBAL_MIPS {
                            run(
                                &mut encoder,
                                "Cloud Profile Mip Pass",
                                Some("gpu.cloud_profile"),
                                &self.pipeline.cloud_profile_mip_pipeline,
                                &pc.view_mip[m as usize],
                                &pc.group_mip_src[(m - 1) as usize].colour,
                                &[(
                                    0,
                                    cloud_temporal::CLOUD_FR_GLOBAL_Y0 >> m,
                                    cloud_temporal::CLOUD_FR_ATLAS_W >> m,
                                    cloud_temporal::CLOUD_FR_GLOBAL_H >> m,
                                )],
                            );
                        }
                    }
                }
            }
        }
        if self.cloud_mode_near {
            if let (Some(cs), Some(mat_idx)) =
                (self.cloud_screen.as_ref(), self.cloud_temporal_mat)
            {
                if let (Some(i), Some(material)) = (
                    transparent.iter().position(|o| o.material == mat_idx),
                    self.materials.get(mat_idx),
                ) {
                    let slot = objects.len() + i;
                    if slot < MAX_OBJECTS {
                        self.upload_object_uniforms(
                            objects.iter().chain(transparent.iter()),
                        );
                        let uniform_align = 256_u64;
                        // ── Cloud Light Bake Pass (increment 1, v0.1286) ──
                        // Refreshes the sun-shadow slice atlas BEFORE the
                        // march reads it. Bound exactly like the march
                        // (the cloud shell's object slot gives the bake
                        // fragment the same planet frame, group 2 the same
                        // material, group 3 the 1x1 fallback: the atlas is
                        // this pass's attachment and may not also be
                        // sampled). Scissored to one eighth of each
                        // window's slices per frame in a fixed order, or
                        // the whole atlas on the first frame, a re-anchor
                        // or a sun re-reference (CloudLightState decides;
                        // this pass only draws what it is handed).
                        // LoadOp::Load keeps the seven eighths not baked
                        // this frame.
                        if let Some(lc) = self.cloud_light_active() {
                            let rects = lc.state.take_bake_rects();
                            let mut bake =
                                encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                    label: Some("Cloud Light Bake Pass"),
                                    timestamp_writes: self.pass_timer("gpu.cloud_light"),
                                    color_attachments: &[Some(
                                        wgpu::RenderPassColorAttachment {
                                            view: &lc.view,
                                            resolve_target: None,
                                            ops: wgpu::Operations {
                                                load: wgpu::LoadOp::Load,
                                                store: wgpu::StoreOp::Store,
                                            },
                                        },
                                    )],
                                    depth_stencil_attachment: None,
                                    ..Default::default()
                                });
                            bake.set_pipeline(&self.pipeline.cloud_light_bake_pipeline);
                            bake.set_bind_group(0, &self.camera_bind_group, &[]);
                            bake.set_bind_group(
                                1,
                                &self.object_bind_group,
                                &[(uniform_align as u32) * (slot as u32)],
                            );
                            bake.set_bind_group(2, &material.bind_group, &[]);
                            bake.set_bind_group(3, &self.default_texture_bind_group, &[]);
                            for (x, y, w, h) in rects {
                                bake.set_scissor_rect(x, y, w, h);
                                bake.draw(0..3, 0..1);
                            }
                            drop(bake);
                        }
                        let mut pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("Cloud March Pass"),
                                timestamp_writes: self.pass_timer("gpu.cloud_screen"),
                                color_attachments: &[
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &cs.march_view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(
                                                wgpu::Color::TRANSPARENT,
                                            ),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    }),
                                    Some(wgpu::RenderPassColorAttachment {
                                        view: &cs.dist_view,
                                        resolve_target: None,
                                        ops: wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(
                                                wgpu::Color::TRANSPARENT,
                                            ),
                                            store: wgpu::StoreOp::Store,
                                        },
                                    }),
                                ],
                                depth_stencil_attachment: None,
                                ..Default::default()
                            });
                        pass.set_pipeline(&self.pipeline.cloud_screen_pipeline);
                        pass.set_bind_group(0, &self.camera_bind_group, &[]);
                        pass.set_bind_group(
                            1,
                            &self.object_bind_group,
                            &[(uniform_align as u32) * (slot as u32)],
                        );
                        pass.set_bind_group(2, &material.bind_group, &[]);
                        // Group 3: the sun-shadow cache atlas in the
                        // albedo slot while the cache is on (v0.1286; the
                        // same predicate raised pad bit 16 above), else
                        // the 1x1 fallback the shared layout requires.
                        // Far rung (increment 4): when the profile is
                        // active too, the profile atlas rides binding 14 of
                        // the same group - `group_sun` (b0 = the sun atlas)
                        // when both caches are active, `group_plain` (b0 =
                        // white) when only the profile is; today's rule
                        // otherwise. The bit-16 predicate is unchanged.
                        let g3 = match (self.cloud_light_active(), self.cloud_profile_active()) {
                            (Some(lc), Some(pc)) => pc
                                .group_sun
                                .as_ref()
                                .map(|g| &g.colour)
                                .unwrap_or(&lc.group.colour),
                            (None, Some(pc)) => &pc.group_plain.colour,
                            (Some(lc), None) => &lc.group.colour,
                            (None, None) => &self.default_texture_bind_group,
                        };
                        pass.set_bind_group(3, g3, &[]);
                        pass.draw(0..3, 0..1);
                        drop(pass);

                        let read = cs.cur.get();
                        let write = 1 - read;
                        let mut frame = self.cloud_resolve_frame.get();
                        // Regime entry / buffer recreation: the history is
                        // zeroed - drop it outright instead of fading the
                        // deck in from black over ~1/alpha frames.
                        if cs.fresh.replace(false) {
                            frame.snap = true;
                        }
                        // Roll-aware basis (v0.1243, audit #19) - same
                        // extraction as the march pads above; the three
                        // consumers must agree or the resolve reprojects
                        // against a twisted frame.
                        let vm = camera.view_matrix();
                        let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
                        let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
                        let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
                        let eye = camera.effective_position();
                        self.cloud_resolve.render(
                            &self.device,
                            &self.queue,
                            &mut encoder,
                            &cs.march_view,
                            &cs.dist_view,
                            &cs.views[read],
                            &cs.views[write],
                            &frame,
                            [eye.x, eye.y, eye.z],
                            [fwd.x, fwd.y, fwd.z],
                            [right.x, right.y, right.z],
                            [up.x, up.y, up.z],
                            (camera.fov_degrees.to_radians() * 0.5).tan(),
                            camera.aspect,
                            self.pass_timer("gpu.cloud_resolve"),
                        );
                        cs.cur.set(write);
                    }
                }
            }
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Pass"),
                timestamp_writes: self.pass_timer("gpu.celestial"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // preserve the star background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0), // reverse-Z: clear to 0 (farthest)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            // Slot 1: zero per-instance data for classic draws (increment 2).
            render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);

            // Opaque bodies + transparent shells (atmospheres) share one
            // object-uniform buffer: shells continue the index range after the
            // opaque list. Both lists together must stay under MAX_OBJECTS
            // (a couple dozen sky bodies in practice).
            // One batched object-uniform upload (v0.891): opaque bodies +
            // transparent shells share the buffer, shells continue the range.
            // KEEP THIS UNCONDITIONAL. The v0.911 perf audit suggested
            // skipping this upload when the shadow pass already staged the
            // identical bytes at 2072 - probe-bisected result: with the
            // skip, the atmosphere DOME vanished at ground level (black
            // starfield at noon, only the horizon limb left) on DX12. The
            // two writes are byte-identical in source, so the failure is a
            // queue-write/submission-ordering subtlety, not logic; the
            // ~1-2 ms is not worth a broken sky. Do not re-attempt without
            // a boot+ground-level-sky probe check.
            let uniform_align = 256_u64;
            self.upload_object_uniforms(objects.iter().chain(transparent.iter()));

            // The classic opaque list: planet bodies (terrain class), near
            // trees (surface and vegetation classes), props. One walk per
            // class present, each through its own class PSO (P3); the
            // shells live in `transparent`, never here. Shadow-only objects
            // (V1: frustum-culled near-tree models that still cast into the
            // sun map above) sit in `celestial_colour_skip`; their uniforms
            // are uploaded like everyone else's (the shadow pass indexes the
            // same list) and only the colour draw is skipped.
            self.draw_opaque_objects(
                &mut render_pass,
                objects,
                &self.celestial_colour_skip,
                "celestial classic loop",
            );

            // ── Batched terrain patches (draw-batching increment 1) ──
            // The 12k-patch working set that used to be 12k RenderObjects
            // (each with a dynamic-offset bind + two buffer binds) is now:
            // bind everything ONCE, then one draw_indexed per patch with
            // instance range i..i+1 -- the instance index routes the shader
            // to that patch's translation + fade in the storage array.
            // Opaque + depth-written, so ordering against the classic
            // opaque loop above is irrelevant; runs BEFORE the transparent
            // shells below, which is the order transparency needs.
            if patch_batch_n > 0 {
                let arena = self.patch_arena.as_ref().expect("patch_batch_n > 0");
                render_pass.set_pipeline(&self.pipeline.patch_render_pipeline);
                render_pass.set_bind_group(1, &arena.bind_group, &[]);
                if let Some(material) = self.materials.get(self.patch_batch_material) {
                    render_pass.set_bind_group(2, &material.bind_group, &[]);
                    render_pass.set_bind_group(
                        3,
                        material.albedo_group().unwrap_or(&self.default_texture_bind_group),
                        &[],
                    );
                }
                render_pass.set_vertex_buffer(0, arena.vertex_buf.slice(..));
                render_pass.set_vertex_buffer(1, arena.instance_buf.slice(..));
                render_pass
                    .set_index_buffer(arena.index_buf.slice(..), wgpu::IndexFormat::Uint32);
                if self.patch_indirect {
                    // Increment 2: the whole batch in ONE command. This is
                    // what removes the ~1.5 us x N draw-encoding cost that
                    // still dominated after increment 1.
                    render_pass.multi_draw_indexed_indirect(
                        &arena.indirect_buf,
                        0,
                        patch_batch_n as u32,
                    );
                } else {
                    for (i, d) in self.patch_draws[..patch_batch_n].iter().enumerate() {
                        render_pass.draw_indexed(
                            d.slot.istart..d.slot.istart + d.slot.icount,
                            d.slot.vstart as i32,
                            (i as u32)..(i as u32 + 1),
                        );
                    }
                }
            }

            // ── Near-field grass strands (v0.1091) ──
            // ONE draw for the whole sward: the shared unit-height tiller
            // mesh, instanced once per visible tiller. The instance record
            // (mesh::GrassInstance) carries the entire transform, so this
            // needs no per-tiller object uniform and no per-tiller bind.
            //
            // AFTER the terrain batch and on the OPAQUE pipeline, both
            // deliberate: opaque + depth-write means blades resolve against
            // each other and against the ground by depth, in any order, and
            // drawing after the ground means most buried fragments are
            // already z-rejected.
            if self.grass_n > 0 {
                if let (Some(gm), Some(gbuf)) =
                    (self.grass_mesh.as_ref(), self.grass_instance_buf.as_ref())
                {
                    if let Some(material) = self.materials.get(self.grass_material) {
                        // The grass material is a strand (type 23), so this
                        // is the vegetation class's opaque PSO (P3), picked
                        // by class like every other draw.
                        render_pass.set_pipeline(
                            self.pipeline.opaque_for(pipeline::shader_class(material.material_type)),
                        );
                        // Object uniform slot 0 is the identity model matrix
                        // staged by upload_object_uniforms for every frame
                        // that draws anything; grass ignores it (the vertex
                        // stage builds its own matrix from the instance) but
                        // the shared pipeline layout requires group 1 bound.
                        render_pass.set_bind_group(1, &self.object_bind_group, &[0]);
                        render_pass.set_bind_group(2, &material.bind_group, &[]);
                        render_pass.set_bind_group(3, &self.default_texture_bind_group, &[]);
                        render_pass.set_vertex_buffer(0, gm.vertex_buffer.slice(..));
                        render_pass.set_vertex_buffer(1, gbuf.slice(..));
                        render_pass.set_index_buffer(
                            gm.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        render_pass.draw_indexed(0..gm.index_count, 0, 0..self.grass_n);
                    }
                }
            }

        }

        // 12c ORDER FIX (adversarial review finding 3): when the camera is
        // OUTSIDE the atmosphere the deck must sit UNDER the limb haze -
        // the v0.997 rule the shell path expressed by transparent-list
        // order (clouds first, dome after). The fullscreen composite
        // therefore runs HERE - after the opaque pass wrote terrain
        // depth, before the transparent pass blends the atmosphere dome
        // over it. Inside the atmosphere the dome is the sky BEHIND the
        // deck, so the composite stays after the transparent pass below.
        // [CloudGate] 1 Hz probe (v0.1247 forensics).
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            static LAST: AtomicU64 = AtomicU64::new(0);
            let s_now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if s_now != LAST.swap(s_now, Ordering::Relaxed) {
                log::info!(
                    "[CloudGate] frame_some={} atmo_over={:?} tmat={}",
                    self.cloud_composite_frame.is_some(),
                    self.cloud_composite_frame.as_ref().map(|f| f.atmo_over),
                    self.cloud_temporal_mat.is_some(),
                );
            }
        }
        if self
            .cloud_composite_frame
            .as_ref()
            .map(|f| f.atmo_over)
            .unwrap_or(false)
        {
            self.run_cloud_composite(&mut encoder, view, camera);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Celestial Transparent Pass"),
                timestamp_writes: self.pass_timer("gpu.celestial_t"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            render_pass.set_bind_group(0, &self.camera_bind_group, &[]);
            let uniform_align = 256_u64;

            // Atmosphere shells etc.: alpha-blended over the bodies, depth-TESTED
            // against them (no depth write), so the back hemisphere of a shell is
            // hidden by its own planet while the limb halo survives. Few and far
            // apart, so no depth sorting needed. (v0.763)
            if !transparent.is_empty() {
                // Slot 1: zero per-instance data for classic draws (increment 2).
                render_pass.set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                let mut bound_material = usize::MAX;
                // ── WATER DEPTH WRITE (v0.1060) ──
                // Operator: "I can essentially see waves behind waves very
                // easily, almost transparent like glass. Almost like the wave
                // behind renders in front of the close waves."
                //
                // Exactly right, and it is not a shading problem: the water
                // shell is thousands of separate patches drawn with alpha
                // blending and NO depth write, in whatever order the LOD
                // selection emitted them - which is heap order by screen error,
                // uncorrelated with distance. So a far wave patch submitted
                // later paints straight over a near one. At 10 m storm crests
                // the sea folds over itself on screen constantly, which is why
                // it only became obvious once the waves got big.
                //
                // The fix is depth, not sorting: the sea's alpha is 0.93-1.0
                // almost everywhere, so it is opaque enough that the NEAREST
                // fragment is simply the right answer. overlay_pipeline is
                // already alpha-blend + cull None + depth_write TRUE - the exact
                // state - so this costs no new pipeline compile.
                //
                // This is only safe because v0.1053 moved water to the END of
                // the transparent list whenever the camera is inside an
                // atmosphere: nothing is drawn after the sea, so its depth
                // cannot wrongly occlude the atmosphere or cloud shells. From
                // orbit the flag stays false and the old behaviour is kept.
                //
                // P2 (the class split) folds that switch into the general
                // pipeline selection below: the PSO is chosen by (material
                // class, overlay-or-transparent), where the class comes from
                // `shader_class` (since P3: the atmosphere shells are Shell,
                // the cloud shell is Cloud, the sea is Water, the sun's
                // blended core and halo are Surface) and overlay is true
                // only for a water caster while the depth-write gate holds.
                // The lib.rs sort keeps the list grouped by the planet layer
                // band (`celestial_order`: everything else first, then the
                // dome / deck / sea stack in its authored order, water last
                // inside an atmosphere), so this switches a handful of
                // times per frame, never per patch.
                let water_dw = self.water_depth_write && !self.water_caster_mats.is_empty();
                let mut bound_pipe: Option<(ShaderClass, bool)> = None;
                for (i, obj) in transparent.iter().enumerate() {
                    let slot = objects.len() + i;
                    if slot >= MAX_OBJECTS { break; }
                    let mesh = match self.meshes.get(obj.mesh) { Some(m) => m, None => continue };
                    let material = match self.materials.get(obj.material) { Some(m) => m, None => continue };
                    let class = pipeline::shader_class(material.material_type);
                    let overlay = water_dw && self.water_caster_mats.contains(&obj.material);
                    if bound_pipe != Some((class, overlay)) {
                        bound_pipe = Some((class, overlay));
                        render_pass.set_pipeline(if overlay {
                            self.pipeline.overlay_for(class)
                        } else {
                            self.pipeline.transparent_for(class)
                        });
                        render_pass
                            .set_vertex_buffer(1, self.dummy_instance_buf.slice(..));
                        bound_material = usize::MAX;
                    }
                    let dynamic_offset = (uniform_align as u32) * (slot as u32);
                    render_pass.set_bind_group(1, &self.object_bind_group, &[dynamic_offset]);
                    // Material bind groups (2 + 3) skipped when unchanged
                    // (v0.891); also drops a duplicate group-3 rebind that a
                    // copy-paste had left here.
                    if bound_material != obj.material {
                        bound_material = obj.material;
                        render_pass.set_bind_group(2, &material.bind_group, &[]);
                        // Group 3 fallback/texture -- same rule as the opaque
                        // loop. Cloud SHELL override (far rung, increment 4,
                        // replacing the dead octa-map override: `cloud_temporal`
                        // has been None since v0.1261): the shell's fragment
                        // (the direct path when the temporal map is not armed,
                        // and the Low sheet) reads the profile atlas at
                        // binding 14 through `group_plain`, on every tier.
                        let g3 = if Some(obj.material) == self.cloud_shell_mat {
                            self.cloud_profile_active()
                                .map(|pc| &pc.group_plain.colour)
                                .or_else(|| material.albedo_group())
                        } else {
                            material.albedo_group()
                        };
                        render_pass.set_bind_group(
                            3,
                            g3.unwrap_or(&self.default_texture_bind_group),
                            &[],
                        );
                    }
                    render_pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }

        // ── Fullscreen depth-aware cloud composite (Wave D slice 1b) ──
        // The INSIDE-the-atmosphere position: after the transparent pass,
        // so the deck draws over the sky dome behind it. The outside
        // position ran earlier (before the transparent pass) - see the
        // 12c order-fix comment there. One compositor either way: the
        // shell's own temporal branch discards while the map is armed.
        if self
            .cloud_composite_frame
            .as_ref()
            .map(|f| !f.atmo_over)
            .unwrap_or(false)
        {
            self.run_cloud_composite(&mut encoder, view, camera);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// The fullscreen depth-aware cloud composite draw (Wave D slice 1b).
    /// Requires the scene depth for this frame's opaques to be complete;
    /// call position relative to the transparent celestial pass is chosen
    /// by CloudCompositeFrame::atmo_over (12c order fix). This is what
    /// lets a deck below the camera survive: the shell's fragments for
    /// downward rays lie beyond the planet and the hardware depth test
    /// killed them; here occlusion is per-pixel against the REAL scene
    /// depth, mountains included.
    fn run_cloud_composite(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        camera: &Camera,
    ) {
        // [CloudPasses] 1 Hz probe (v0.1247 forensics): the ground truth of
        // whether this compositor actually runs, and with what inputs.
        {
            use std::sync::atomic::{AtomicU64, Ordering};
            static LAST: AtomicU64 = AtomicU64::new(0);
            let s_now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if s_now != LAST.swap(s_now, Ordering::Relaxed) {
                log::info!(
                    "[CloudPasses] invoked; ct={} frame={} tmat={} near_pair={} mix={:.2}",
                    self.cloud_temporal.is_some(),
                    self.cloud_composite_frame.is_some(),
                    self.cloud_temporal_mat.is_some(),
                    self.cloud_screen.is_some() && self.cloud_mode_near,
                    self.cloud_near_mix,
                );
            }
        }
        // v0.1261: gated on the SCREEN pair, which is the only cloud
        // renderer since v0.1250. It used to be gated on the octa map's
        // existence, which is exactly why the retired map could not just
        // be deleted - the composite refused to run without it.
        if let (Some(cs), Some(frame), true) = (
            self.cloud_screen.as_ref(),
            self.cloud_composite_frame.as_ref(),
            self.cloud_temporal_mat.is_some(),
        ) {
            let proj = Mat4::perspective_rh(
                camera.fov_degrees.to_radians(),
                camera.aspect,
                1.0e13,
                1.0,
            );
            let m = proj.to_cols_array_2d();
            // Roll-aware basis (v0.1243, audit #19) - third of the three
            // agreeing consumers (march pads, resolve, composite).
            let vm = camera.view_matrix();
            let fwd = -glam::Vec3::new(vm.row(2).x, vm.row(2).y, vm.row(2).z);
            let right = glam::Vec3::new(vm.row(0).x, vm.row(0).y, vm.row(0).z);
            let up = glam::Vec3::new(vm.row(1).x, vm.row(1).y, vm.row(1).z);
            let eye = camera.effective_position();
            // The screen accumulation IS the cloud image (v0.1261); the
            // old crossfade weight survives only as the arming ramp.
            let screen_view = &cs.views[cs.cur.get()];
            let dist_view = &cs.dist_view;
            let near_mix =
                if self.cloud_mode_near { self.cloud_near_mix } else { 0.0 };
            self.cloud_composite.render(
                &self.device,
                &self.queue,
                encoder,
                &self.depth_view,
                view,
                frame,
                screen_view,
                dist_view,
                near_mix,
                if self.cloud_discard_diag { 1.0 } else { 0.0 },
                [eye.x, eye.y, eye.z],
                [fwd.x, fwd.y, fwd.z],
                [right.x, right.y, right.z],
                [up.x, up.y, up.z],
                (camera.fov_degrees.to_radians() * 0.5).tan(),
                camera.aspect,
                m[2][2],
                m[3][2],
                self.pass_timer("gpu.cloud_composite"),
            );
        }
    }
}
