//! F10 cloud dev panel (v0.1254.4, operator request).
//!
//! The cloud bisect toggles used to live only in debug/showcase_request.json
//! file drops - the operator: "When I tried to create the showcase_request
//! .json it vanished. Can you make sure you put these commands in a presently
//! nonexistent F10 menu? Since we want GUI buttons for everything." (The
//! vanishing was the game consuming the file - correct but invisible.) Per
//! the GUI-first norm, every dev knob an AI or operator can reach over IPC
//! must also be a button. This panel drives the SAME renderer flags the
//! showcase pins set, so the two surfaces can never disagree.

use egui::{Context, RichText};

use crate::gui::theme::Theme;
use crate::gui::GuiState;

/// Draw the panel. Returns true when the operator changed something, so the
/// caller (lib.rs) re-publishes the flags to the renderer.
pub fn draw(ctx: &Context, theme: &Theme, state: &mut GuiState) -> bool {
    if !state.show_cloud_dev_panel {
        return false;
    }
    let mut changed = false;
    let mut open = true;
    egui::Window::new("Cloud dev (F10)")
        .open(&mut open)
        .default_pos([40.0, 120.0])
        .default_width(330.0)
        .resizable(true)
        .show(ctx, |ui| {
            ui.label(
                RichText::new(
                    "Live cloud-renderer diagnostics. These are the same \
                     switches the probe rig uses; flipping them takes effect \
                     the same frame and nothing here is saved.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);

            ui.label(RichText::new("Look").strong().color(theme.accent()));
            // Dither: the static-vs-agate trade, the operator's live choice
            // until the mip-response calibration retires both (v0.1254.3).
            let mut dither_on = !state.cloud_dev_dither_off;
            if ui
                .checkbox(
                    &mut dither_on,
                    "Depth dither (off = smoother clouds, agate arcs on overcast)",
                )
                .changed()
            {
                state.cloud_dev_dither_off = !dither_on;
                changed = true;
            }
            let mut temporal_on = !state.cloud_dev_temporal_off;
            if ui
                .checkbox(
                    &mut temporal_on,
                    "Temporal accumulation (off = raw march, sharp per-frame noise)",
                )
                .changed()
            {
                state.cloud_dev_temporal_off = !temporal_on;
                changed = true;
            }

            let mut shape_on = !state.cloud_dev_shape_off;
            if ui
                .checkbox(
                    &mut shape_on,
                    "Cloud shape frame (off = old round balls, for comparison)",
                )
                .changed()
            {
                state.cloud_dev_shape_off = !shape_on;
                changed = true;
            }

            // Clock pin (v0.1268). Not a look control - a MEASUREMENT one.
            // The cloud clock is app-start-relative, so two rig runs of one
            // build rendered different cloud fields (20% of pixels differing
            // by >40 levels). Freezing it makes a capture a function of the
            // build alone, which is what an A/B needs to mean anything.
            let mut frozen = state.cloud_dev_clock_pin >= 0.0;
            if ui
                .checkbox(&mut frozen, "Freeze cloud drift (for before/after comparisons)")
                .changed()
            {
                state.cloud_dev_clock_pin = if frozen { 120.0 } else { -1.0 };
                changed = true;
            }

            // The v0.1268 rosette fix, as a live A/B (operator directive:
            // "we have the F10 dev menu to toggle things on/off"). ON here
            // restores the OLD behaviour, so the rosette should come BACK -
            // that is how you tell the fix is doing something.
            // The v0.1269 test. A cloud must not change SHAPE because you
            // flew closer to it, and lines of equal distance-to-camera are
            // circles centred on the nadir - which is exactly where the
            // artifact sits.
            // The mip ring cure (v0.1270). Split out of the depth-dither box,
            // which used to switch both. Measured at 9.2 km, clock pinned:
            // direct-sun radial energy 23.97 with the cure on, 28.11 with it
            // off, against a 0.7 noise floor.
            let mut cure = !state.cloud_dev_ring_cure_off;
            if ui
                .checkbox(
                    &mut cure,
                    "Per-pixel mip dither (default OFF: measured never better, worse inside the deck)",
                )
                .changed()
            {
                state.cloud_dev_ring_cure_off = !cure;
                changed = true;
            }

            // The v0.1271 estimator experiments. Thesis: the density edge is a
            // cliff a few metres wide sampled with steps of tens to hundreds of
            // metres, so every edge sample is a coin flip (the glitter) and the
            // converged mean depends on step spacing, which is a function of
            // the angle to the local vertical (the rosette). Two levers.
            // v0.1280: increment A, the in-cloud light. Inside a cloud at noon
            // the eye should see bright fog-white: the interior was ~10x too
            // dark because every ambient term was a transmittance and the sun
            // ladder resolved the shadows of the lobes around the eye.
            let mut msb = state.cloud_dev_ms;
            if ui
                .checkbox(&mut msb, "In-cloud light (Eddington source; fog-white interiors; no lobe shadows through the eye)")
                .changed()
            {
                state.cloud_dev_ms = msb;
                changed = true;
            }
            if state.cloud_dev_ms {
                let mut g = if state.cloud_dev_ms_gain > 0.0 { state.cloud_dev_ms_gain } else { 1.0 };
                if ui
                    .add(egui::Slider::new(&mut g, 0.2..=3.0).text("in-scatter gain"))
                    .changed()
                {
                    state.cloud_dev_ms_gain = g;
                    changed = true;
                }
            }
            // Increment C (v0.1282): interior density saturation of the
            // constructed bodies. 0 = the shipped profile (crown fade to the
            // base fraction over the top third, +-42% turbulence); 1 = LWC
            // peaking at the top, opaque within tens of metres, and the
            // in-cloud light fed the body's own column.
            let mut sat = state.cloud_dev_int_sat;
            if ui
                .add(egui::Slider::new(&mut sat, 0.0..=1.0).text("interior saturation (built bodies)"))
                .changed()
            {
                state.cloud_dev_int_sat = sat;
                changed = true;
            }
            // Perf increment 3 (v0.1287): keep each cell's built lobe cluster
            // for the whole ray instead of rebuilding it at every sample.
            // Perf increment 2 (v0.1288): far rays stop taking 22 m steps and
            // opaque rays relax their step; 0 = off, 1 = full.
            let mut eco = state.cloud_dev_step_eco;
            if ui
                .add(egui::Slider::new(&mut eco, 0.0..=1.0).text("step economy (footprint floors + deep relaxation)"))
                .changed()
            {
                state.cloud_dev_step_eco = eco;
                changed = true;
            }
            let mut bc = state.cloud_dev_body_cache;
            if ui
                .checkbox(&mut bc, "Body cluster cache (per ray; off = rebuild the lobes at every sample, for A/B)")
                .changed()
            {
                state.cloud_dev_body_cache = bc;
                changed = true;
            }
            let mut fld = state.cloud_dev_field;
            if ui
                .checkbox(&mut fld, "Field walls (three-octave warp: sinuous walls, turbulent 100-500 m band)")
                .changed()
            {
                state.cloud_dev_field = fld;
                changed = true;
            }
            // Performance plan increment 1 (v0.1286): the sun optical depth
            // becomes a planet-fixed cached quantity (two nested windows
            // around the camera, baked by the same rung ladder, read with
            // one tap per sample). Off is the exact old path, so this box
            // IS the A/B: flip it and the picture must not change while
            // the frame time does. The "Sun source" bisect channel below
            // paints which window each pixel read from.
            let mut lcb = state.cloud_dev_light;
            if ui
                .checkbox(&mut lcb, "Sun shadow cache (off = 12-rung ladder per pixel, for A/B)")
                .changed()
            {
                state.cloud_dev_light = lcb;
                changed = true;
            }
            // v0.1272: the two fixes the estimator assessment designed.
            let mut estb = state.cloud_dev_est;
            if ui
                .checkbox(&mut estb, "Sample-anchored march (default ON since v0.1272; off = old march, glitter returns)")
                .changed()
            {
                state.cloud_dev_est = estb;
                changed = true;
            }
            let mut wbl = state.cloud_dev_warp_bl;
            if ui
                .checkbox(&mut wbl, "Warp band-limited to its own tile (default ON since v0.1272; off = 3-20 m silhouette hash)")
                .changed()
            {
                state.cloud_dev_warp_bl = wbl;
                changed = true;
            }
            // The lean (v0.1275): if the fan is the prism walls, its
            // convergence point moves off straight-down as this rises.
            let mut sh = state.cloud_dev_shear;
            if ui
                .add(egui::Slider::new(&mut sh, 0.0..=1.0).text("cloud lean (m per m of height; 0 = off)"))
                .changed()
            {
                state.cloud_dev_shear = sh;
                changed = true;
            }
            // Component bisect (v0.1279): one density term off at a time.
            ui.label(RichText::new("Density component bisect (each turns ONE term off)").size(theme.font_size_small).color(theme.text_secondary()));
            for (label, flag) in [
                ("detail erosion off", &mut state.cloud_dev_no_detail),
                ("puff erosion off", &mut state.cloud_dev_no_puff),
                ("cell split off", &mut state.cloud_dev_no_cell),
                ("fray off", &mut state.cloud_dev_no_fray),
                ("base drop off", &mut state.cloud_dev_no_bdrop),
                ("sharp base (0.5% of band, not 3%)", &mut state.cloud_dev_sharp_base),
                ("interior relief fade (no lobe shading deep inside cloud)", &mut state.cloud_dev_relief_fade),
                ("coarse sun ladder deep inside (skip rungs under 200 m)", &mut state.cloud_dev_deep_rung),
                ("SYNTHETIC CHECKER density (projection test: must render as a grid)", &mut state.cloud_dev_checker),
            ] {
                if ui.checkbox(flag, label).changed() {
                    changed = true;
                }
            }
            let mut hvw = state.cloud_dev_hv_warp;
            if ui
                .checkbox(&mut hvw, "Height-varying cloud walls (the prism-wall fix: walls wander with altitude)")
                .changed()
            {
                state.cloud_dev_hv_warp = hvw;
                changed = true;
            }
            if state.cloud_dev_hv_warp {
                let mut hk = if state.cloud_dev_hv_km > 0.0 { state.cloud_dev_hv_km } else { 0.5 };
                if ui
                    .add(egui::Slider::new(&mut hk, 0.1..=5.0).text("wall wander km per 1.3 km of height"))
                    .changed()
                {
                    state.cloud_dev_hv_km = hk;
                    changed = true;
                }
            }
            let mut sm = state.cloud_dev_sigma_mul;
            if ui
                .add(egui::Slider::new(&mut sm, 0.0..=10.0).text("extinction x (0 = off; the transparency test)"))
                .changed()
            {
                state.cloud_dev_sigma_mul = sm;
                changed = true;
            }
            let mut thin = state.cloud_dev_thin_deck;
            if ui
                .checkbox(&mut thin, "Thin deck (band height x0.3: the prism-wall test for the rosette)")
                .changed()
            {
                state.cloud_dev_thin_deck = thin;
                changed = true;
            }
            let mut iso = state.cloud_dev_iso_step;
            if ui
                .checkbox(&mut iso, "Isotropic near step (no verticality or chord term inside 27 km)")
                .changed()
            {
                state.cloud_dev_iso_step = iso;
                changed = true;
            }
            let mut nfl = state.cloud_dev_norm_floor;
            if ui
                .checkbox(&mut nfl, "Carve normaliser floor (low-coverage stencil becomes a 450 m ramp)")
                .changed()
            {
                state.cloud_dev_norm_floor = nfl;
                changed = true;
            }
            let mut ustep = state.cloud_dev_uniform_step;
            if ui
                .checkbox(&mut ustep, "Uniform march step (distance-only; no verticality term)")
                .changed()
            {
                state.cloud_dev_uniform_step = ustep;
                changed = true;
            }
            if state.cloud_dev_uniform_step {
                let mut sm = state.cloud_dev_step_m;
                if ui
                    .add(egui::Slider::new(&mut sm, 0.0..=600.0).text("fixed step m (0 = off)"))
                    .changed()
                {
                    state.cloud_dev_step_m = sm;
                    changed = true;
                }
            }
            let mut wedge = state.cloud_dev_wide_edge;
            if ui
                .checkbox(&mut wedge, "Wide cloud edge (~300 m ramp, radiative smoothing scale)")
                .changed()
            {
                state.cloud_dev_wide_edge = wedge;
                changed = true;
            }

            if state.cloud_dev_wide_edge {
                let mut m = if state.cloud_dev_edge_mul > 0.0 { state.cloud_dev_edge_mul } else { 20.0 };
                if ui
                    .add(egui::Slider::new(&mut m, 1.0..=200.0).text("edge width x (hinge)"))
                    .changed()
                {
                    state.cloud_dev_edge_mul = m;
                    changed = true;
                }
                let mut r = if state.cloud_dev_rind_wide_m > 0.0 { state.cloud_dev_rind_wide_m } else { 300.0 };
                if ui
                    .add(egui::Slider::new(&mut r, 90.0..=1500.0).text("body rind m"))
                    .changed()
                {
                    state.cloud_dev_rind_wide_m = r;
                    changed = true;
                }
            }

            let mut wsl = state.cloud_dev_world_shape_lod;
            if ui
                .checkbox(
                    &mut wsl,
                    "World-anchored cloud shape (shape detail stops following camera distance)",
                )
                .changed()
            {
                state.cloud_dev_world_shape_lod = wsl;
                changed = true;
            }

            let mut chord = state.cloud_dev_chord_foot;
            if ui
                .checkbox(
                    &mut chord,
                    "Old chord detail scale (on = pre-v0.1268, sun rosette returns)",
                )
                .changed()
            {
                state.cloud_dev_chord_foot = chord;
                changed = true;
            }

            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Cloud resolution")
                    .strong()
                    .color(theme.accent()),
            );
            // The operator, v0.1254.4: "It seems like the cloud layer is
            // lower resolution than the surface layer, would upping the
            // resolution of the cloud layer help at all?" It IS lower -
            // the march has always run at a QUARTER of screen resolution
            // (one cloud sample per 4x4 screen pixels), which is exactly
            // the "solid pixels" look and the binary opaque-or-clear
            // edges. Terrain renders full-res beside it. Raising this
            // costs frames roughly with the pixel count, so it is a
            // slider for their machine to answer, not a guess for mine.
            ui.label(
                RichText::new(
                    "The cloud march runs BELOW screen resolution - this is \
                     why cloud edges read as blocky pixels next to full-res \
                     terrain. Higher costs frames roughly with pixel count.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            let res_opts: [(u32, &str); 3] =
                [(4, "Quarter"), (2, "Half"), (1, "Full")];
            let mut res = state.cloud_dev_res_div.clamp(1, 4);
            ui.horizontal(|ui| {
                for (div, name) in res_opts {
                    if ui.selectable_label(res == div, name).clicked() {
                        res = div;
                    }
                }
            });
            if res != state.cloud_dev_res_div {
                state.cloud_dev_res_div = res;
                // Persisted since v0.1285 (config cloud_res_div): the choice
                // survives a restart, so play and the rig read the same cost.
                state.settings_dirty = true;
                changed = true;
            }

            ui.add_space(theme.spacing_sm);
            ui.label(
                RichText::new("Bisect channel (grayscale forensics)")
                    .strong()
                    .color(theme.accent()),
            );
            ui.label(
                RichText::new(
                    "Renders one raw ingredient of every cloud pixel instead \
                     of the finished image - the instrument that cracked the \
                     rosette. Off = normal rendering.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            // The discard-reason bisect (v0.1262). This checkbox was
            // MISSED when the feature shipped - the plumbing landed but the
            // edit script aborted before the UI insert, so the operator had
            // the flag with no way to reach it. GUI-first means the button
            // ships in the same commit as the flag, every time.
            let mut dsc = state.cloud_dev_discard_diag;
            if ui
                .checkbox(&mut dsc, "Discard reasons (why a pixel's cloud was killed)")
                .changed()
            {
                state.cloud_dev_discard_diag = dsc;
                changed = true;
            }
            ui.label(
                RichText::new(
                    "grey = the march found no cloud, GREEN = terrain in                      front, RED = planet horizon cull, blue = ray missed                      the shell, purple = behind camera, orange = empty slab.",
                )
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
            );
            ui.add_space(theme.spacing_sm);
            // "March steps" / "Step comb" (v0.1268): the iteration-count
            // instrument, rebuilt. A ring at a fixed screen radius in a
            // channel that has NOTHING to do with lighting or density can
            // only come from the march schedule itself.
            // (value, label): the VALUE is what the shader's map_diag
            // ladder in 45-cloud-temporal.wgsl reads. 7 has no channel
            // there (burial is `diag >= 7.5`, i.e. 8), so the old "Burial"
            // button at index 7 was showing entry depth; the values are
            // explicit now so a gap cannot mislabel a channel again.
            // 9 = "Sun source" (v0.1286): which sun-shadow source each
            // cloud pixel read - fine window white, coarse grey, analytic
            // column dark - so the window EDGES are visible where a ring
            // in the diff would sit.
            let names: [(i32, &str); 9] = [
                (0, "Off"),
                (1, "Coverage alpha"),
                (2, "Direct sun"),
                (3, "Ambient"),
                (4, "March steps"),
                (5, "Step comb"),
                (6, "Entry depth"),
                (8, "Burial"),
                (9, "Sun source"),
            ];
            let mut pick = state.cloud_dev_map_diag.clamp(0, 9);
            ui.horizontal_wrapped(|ui| {
                for (value, name) in names.iter() {
                    if ui.selectable_label(pick == *value, *name).clicked() {
                        pick = *value;
                    }
                }
            });
            if pick != state.cloud_dev_map_diag {
                state.cloud_dev_map_diag = pick;
                changed = true;
            }
            if pick >= 4 && pick <= 5 {
                ui.label(
                    RichText::new(
                        "Black = few steps, white = this ray spent the whole                          224-step budget and integrated its remaining tail in                          ONE giant sample. Step comb bands every 8 steps.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
                );
            }
            if pick == 9 {
                ui.label(
                    RichText::new(
                        "White = the fine cache window, grey = the coarse                          window, dark = the analytic column beyond both.                          With the cache off every pixel is dark.",
                    )
                    .size(theme.font_size_small)
                    .color(theme.text_secondary()),
                );
            }
        });
    if !open {
        state.show_cloud_dev_panel = false;
    }
    changed
}
