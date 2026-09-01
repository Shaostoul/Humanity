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
            let names = [
                "Off",
                "Coverage alpha",
                "Direct sun",
                "Ambient",
                "March steps",
                "Step comb",
            ];
            let mut pick = state.cloud_dev_map_diag.clamp(0, 5);
            ui.horizontal(|ui| {
                for (i, name) in names.iter().enumerate() {
                    if ui.selectable_label(pick == i as i32, *name).clicked() {
                        pick = i as i32;
                    }
                }
            });
            if pick != state.cloud_dev_map_diag {
                state.cloud_dev_map_diag = pick;
                changed = true;
            }
            if pick >= 4 {
                ui.label(
                    RichText::new(
                        "Black = few steps, white = this ray spent the whole                          224-step budget and integrated its remaining tail in                          ONE giant sample. Step comb bands every 8 steps.",
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
