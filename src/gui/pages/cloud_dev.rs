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
                    "Spatial dither (off = smoother clouds, agate arcs on overcast)",
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
            let names = ["Off", "Coverage alpha", "Direct sun", "Ambient"];
            let mut pick = state.cloud_dev_map_diag.clamp(0, 3);
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
        });
    if !open {
        state.show_cloud_dev_panel = false;
    }
    changed
}
