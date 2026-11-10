//! Settings > Graphics > Garment textures (2026-11-10; docs/design/character-animation.md section
//! 16). Child of settings.rs.
//!
//! How sharp the textures of the clothes, shoes, hats, hair, brows and lashes people wear are drawn:
//! Full or Low memory (data/gui/garment_textures.json), each with the video memory its garment
//! texture pools take on this graphics card (worked out by the engine when the characters load,
//! `gui::garment_textures`), the furniture detail setting's way; then the card's line: block
//! compression (BC7) read, or, where it is not, the notice saying so in words, what the textures need
//! and what works instead (the hardware-notice rule, CLAUDE.md).
//!
//! The website has no twin of this section: it draws no 3D people.

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn, tf};

use crate::gui::garment_textures as gt;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Hint};
use crate::gui::{GuiState, HintDisplay};

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

/// The subsection, drawn inside Settings > Graphics, with this machine's plans and card line.
pub(crate) fn draw_garment_textures(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, accent: Color32) {
    let (line, notice) = gt::this_machine_line();
    draw_with(ui, theme, state, hint, accent, gt::plan().as_ref(), gt::mesh_plan().as_ref(), line.as_deref(), notice.as_deref());
}

/// The subsection with the plans and lines given (the page's, or a snapshot's). `meshes` is the
/// own-mesh pool's (2026-11-10): the people's own bodies, clothes and hair, set aside once when the
/// world is entered as the texture pools are, so its memory is shown with theirs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_with(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, accent: Color32, plan: Option<&gt::Plan>, meshes: Option<&gt::MeshPlan>, line: Option<&str>, notice: Option<&str>) {
    widgets::subsection_header(ui, theme, accent, t("garment_textures.title"), "");
    // A grid, so the memory figures line up for comparing.
    egui::Grid::new("garment_texture_levels").num_columns(3).show(ui, |ui| {
        for level in gt::levels() {
            let sel = state.settings.garment_textures == level.id;
            if ui.selectable_label(sel, RichText::new(t_dyn(&level.words)).size(theme.font_size_small)).hint("garment_textures.level_hint").clicked() && !sel {
                state.settings.garment_textures = level.id.clone();
                state.settings_dirty = true;
            }
            muted(ui, theme, t_dyn(&level.note));
            let memory = plan.and_then(|p| gt::level_memory_words(p, &level.id)).map(|m| tf("garment_textures.memory", &[("memory", &m)])).unwrap_or_default();
            muted(ui, theme, memory);
            ui.end_row();
        }
    });
    widgets::setting_hint(ui, theme, hint, t("garment_textures.hint"));
    if plan.is_none() {
        muted(ui, theme, t("garment_textures.memory_unmeasured"));
    }
    if let Some(m) = meshes {
        muted(ui, theme, gt::mesh_memory_words(m));
    }
    if let Some(line) = line {
        widgets::glossary_words(ui, theme, line, theme.font_size_small, theme.text_muted());
    }
    if let Some(notice) = notice {
        widgets::glossary_words(ui, theme, notice, theme.font_size_small, theme.warning());
    }
}
