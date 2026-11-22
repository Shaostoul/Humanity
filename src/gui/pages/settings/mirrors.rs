//! Settings > Graphics > Light and sky > Mirror reflections (2026-11-21, renderer/mirror.rs,
//! docs/design/mirrors.md): the levels from data/gui/mirror_levels.json, each with how sharp it
//! draws a mirror and the most video memory it can take, worked out from its own numbers; the
//! measured time and memory (the graphics_hint's cost line: gpu.mirror_scene, gpu.mirror_transparent,
//! cpu.mirror_scene, vram.mirror); and what the mirrors are doing this frame. The operator,
//! 2026-11-10: "I want to make sure when people change graphics settings they know what they're
//! actually changing."

use super::graphics::graphics_hint;
use crate::gui::theme::Theme;
use crate::gui::GuiState;
use egui::RichText;
use humanity_core::i18n::{t, t_dyn, tf};

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

/// The most a level's reflection holds: a full-height mirror (twice as tall as wide, the standing
/// mirror's shape) at the level's longest side.
fn most_bytes(level: &crate::renderer::mirror::MirrorLevel) -> u64 {
    crate::renderer::mirror::reflection_bytes((level.max_side / 2, level.max_side))
}

/// Draws the Mirror reflections control.
pub(super) fn draw_mirror_reflections(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: crate::gui::HintDisplay) {
    use crate::gui::widgets::Hint;
    use crate::renderer::bounced_light_detail::mb_words;
    use crate::renderer::mirror as rm;
    ui.label(RichText::new(t("graphics.mirror_reflections")).color(theme.text_secondary()).strong());
    egui::Grid::new("mirror_reflection_levels").num_columns(2).show(ui, |ui| {
        for level in rm::levels() {
            let sel = state.settings.mirror_reflections == level.id;
            let name = t_dyn(&format!("graphics.mirror_reflections_choice.{}", level.id));
            if ui.selectable_label(sel, RichText::new(name).size(theme.font_size_small)).hint("graphics.mirror_reflections_level_hint").clicked() && !sel {
                state.settings.mirror_reflections = level.id.clone();
                state.settings_dirty = true;
            }
            let words = if level.scale <= 0.0 {
                t("graphics.mirror_reflections_level_off").to_string()
            } else {
                tf(
                    "graphics.mirror_reflections_level",
                    &[("share", &format!("{:.0}", level.scale * 100.0)), ("side", &level.max_side.to_string()), ("memory", &mb_words(most_bytes(level)))],
                )
            };
            muted(ui, theme, words);
            ui.end_row();
        }
    });
    graphics_hint(ui, theme, hint, "mirror_reflections");
    // What the mirrors drew on the last frame (the engine publishes it, engine/mirrors.rs).
    if rm::level(&state.settings.mirror_reflections).scale > 0.0 {
        match rm::published_now() {
            Some((w, h)) => muted(ui, theme, tf("graphics.mirror_reflections_now", &[("size", &format!("{w} x {h}")), ("memory", &mb_words(rm::reflection_bytes((w, h))))])),
            None => muted(ui, theme, t("graphics.mirror_reflections_idle")),
        }
    }
}
