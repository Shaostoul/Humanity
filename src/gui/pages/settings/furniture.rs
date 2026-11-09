//! Settings > Graphics > Furniture (2026-11-09; docs/design/simready-props.md, sections 7, 9 and 10).
//! Child of settings.rs.
//!
//! Two things, both about the 3D models of the home's furniture, fixtures and machines:
//!
//! - **Furniture detail**: Auto, Full, Simple or Boxes (data/gui/furniture_detail.json), each with
//!   the video memory it takes in the home the player is in (measured by the home's body build,
//!   `gui::furniture_detail`), and the graphics card's own memory beside it, with a plain warning
//!   when the chosen level takes too much of it (the hardware-notice rule, CLAUDE.md).
//! - **Furniture models**: the models that come with the app, and the Medical Bay pack to download,
//!   the instrument packs' way (its size, its licence in plain words, Download / Remove, progress).
//!   The work behind the buttons runs in `model_packs::ui::tick` (the engine calls it each frame).
//!
//! The website has no twin of this section: it draws no 3D home.

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn, t_marked_dyn, tf};

use crate::gui::furniture_detail as fd;
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, Hint};
use crate::gui::{GuiState, HintDisplay};
use crate::instrument_packs::install::{Failure, Stage};
use crate::model_packs::catalog::ModelPack;
use crate::model_packs::ui::ModelPacksUi;

fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

fn warn(ui: &mut egui::Ui, color: Color32, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(color).size(theme.font_size_small));
}

/// The subsection, drawn inside Settings > Graphics.
pub(crate) fn draw_furniture(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, accent: Color32) {
    widgets::subsection_header(ui, theme, accent, t("furniture_detail.title"), "");
    draw_detail(ui, theme, state, hint);
    ui.add_space(theme.spacing_md);
    draw_packs(ui, theme, hint, &mut crate::model_packs::ui::shared());
}

fn draw_detail(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay) {
    // A grid, so the memory figures line up for comparing.
    egui::Grid::new("furniture_detail_levels").num_columns(3).show(ui, |ui| {
        for level in fd::levels() {
            let sel = state.settings.furniture_detail == level.id;
            if ui.selectable_label(sel, RichText::new(t_dyn(&level.words)).size(theme.font_size_small)).hint("furniture_detail.level_hint").clicked() && !sel {
                state.settings.furniture_detail = level.id.clone();
                state.settings_dirty = true;
            }
            muted(ui, theme, t_dyn(&level.note));
            let memory = fd::level_memory_words(&level.id).map(|m| tf("furniture_detail.memory", &[("memory", &m)])).unwrap_or_default();
            muted(ui, theme, memory);
            ui.end_row();
        }
    });
    widgets::setting_hint(ui, theme, hint, t("furniture_detail.hint"));
    if fd::measured_full_bytes().is_none() {
        muted(ui, theme, t("furniture_detail.memory_unmeasured"));
    }
    let card = crate::renderer::bounced_light_detail::adapter_memory();
    if fd::level(&state.settings.furniture_detail).texture_divisor.is_none() {
        let now = match fd::texture_divisor(&state.settings.furniture_detail, card) {
            2 => t("furniture_detail.level.simple"),
            0 => t("furniture_detail.level.boxes"),
            _ => t("furniture_detail.level.full"),
        };
        muted(ui, theme, tf("furniture_detail.auto_now", &[("level", &now)]));
    }
    let (line, warning) = fd::hardware_notice(&state.settings.furniture_detail);
    muted(ui, theme, line);
    if let Some(w) = warning {
        warn(ui, theme.warning(), theme, w);
    }
}

fn failure_words(f: &Failure) -> String {
    match f {
        Failure::Network(d) => tf("model_packs.failed.network", &[("detail", d)]),
        Failure::Damaged(d) => tf("model_packs.failed.damaged", &[("detail", d)]),
        Failure::Disk(d) => tf("model_packs.failed.disk", &[("detail", d)]),
        Failure::NotAPack(d) => tf("model_packs.failed.not_a_pack", &[("detail", d)]),
        Failure::NotPublished => t("model_packs.failed.not_published").to_string(),
    }
}

fn licence_line(ui: &mut egui::Ui, theme: &Theme, key: &str) {
    let line = tf("model_packs.licence_label", &[("words", &t_marked_dyn(key))]);
    widgets::glossary_words(ui, theme, &line, theme.font_size_small, theme.text_muted());
}

fn draw_packs(ui: &mut egui::Ui, theme: &Theme, hint: HintDisplay, packs: &mut ModelPacksUi) {
    packs.shown = true;
    ui.label(RichText::new(t("model_packs.title")).color(theme.text_secondary()).strong());
    widgets::setting_hint(ui, theme, hint, t("model_packs.intro"));
    if let Some(e) = &packs.load_error {
        warn(ui, theme.danger(), theme, tf("model_packs.load_error", &[("detail", e)]));
    }
    let Some(catalog) = packs.catalog.clone() else { return };
    // What comes with the app: always there, nothing to download or remove.
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t_dyn(&catalog.core.words)).color(theme.text_secondary()));
    muted(ui, theme, tf("model_packs.count", &[("count", &catalog.core.models)]));
    muted(ui, theme, tf("model_packs.size", &[("mb", &mb(catalog.core.bytes))]));
    licence_line(ui, theme, &catalog.core.licence);
    muted(ui, theme, t("model_packs.core_status"));
    for pack in &catalog.packs {
        ui.add_space(theme.spacing_md);
        draw_pack(ui, theme, pack, packs);
    }
}

fn draw_pack(ui: &mut egui::Ui, theme: &Theme, pack: &ModelPack, packs: &mut ModelPacksUi) {
    ui.label(RichText::new(t_dyn(&pack.words)).color(theme.text_secondary()));
    let names: Vec<&str> = pack.models.iter().map(|m| t_dyn(&format!("model_packs.model.{m}"))).collect();
    muted(ui, theme, tf("model_packs.holds", &[("list", &names.join(", "))]));
    if pack.is_built() {
        muted(ui, theme, tf("model_packs.size", &[("mb", &mb(pack.bytes))]));
    }
    licence_line(ui, theme, &pack.licence);
    let installed = packs.installed.get(&pack.id).cloned();
    let current = packs.is_current(&pack.id);
    let status = match (&installed, current, pack.is_built()) {
        (Some(i), true, _) => tf("model_packs.installed", &[("mb", &mb(i.bytes))]),
        (Some(_), false, _) => t("model_packs.update_available").to_string(),
        (None, _, true) => t("model_packs.not_installed").to_string(),
        (None, _, false) => t("model_packs.not_built").to_string(),
    };
    muted(ui, theme, status);
    let mine = packs.download.as_ref().filter(|(id, _)| *id == pack.id).and_then(|(_, p)| p.lock().ok().map(|g| g.clone()));
    let busy = packs.downloading();
    let failure = match mine.as_ref().map(|p| &p.stage) {
        Some(Stage::Failed(f)) => Some(f.clone()),
        _ => None,
    };
    match &mine {
        Some(p) if failure.is_none() && p.stage != Stage::Done => {
            let frac = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
            let words = match p.stage {
                Stage::Connecting => t("model_packs.stage.connecting").to_string(),
                Stage::Downloading => tf("model_packs.stage.downloading", &[("done", &mb(p.done)), ("total", &mb(p.total.max(pack.bytes)))]),
                Stage::Checking => t("model_packs.stage.checking").to_string(),
                _ => t("model_packs.stage.installing").to_string(),
            };
            ui.add(egui::ProgressBar::new(frac).text(words));
        }
        _ => {
            if let Some(f) = &failure {
                warn(ui, theme.danger(), theme, failure_words(f));
            }
            ui.horizontal(|ui| {
                if installed.is_none() || !current {
                    let (label, key) = if failure.is_some() {
                        (t("model_packs.retry"), "model_packs.retry_hint")
                    } else if installed.is_some() {
                        (t("model_packs.update"), "model_packs.update_hint")
                    } else {
                        (t("model_packs.download"), "model_packs.download_hint")
                    };
                    if Button::primary(label).hint(key).disabled(busy || !pack.is_built()).show(ui, theme) {
                        packs.request_download = Some(pack.id.clone());
                    }
                }
                if installed.is_some() && Button::secondary(t("model_packs.remove")).hint("model_packs.remove_hint").disabled(busy).show(ui, theme) {
                    packs.request_remove = Some(pack.id.clone());
                }
            });
        }
    }
    if let Some((id, e)) = &packs.remove_error {
        if *id == pack.id {
            warn(ui, theme.danger(), theme, tf("model_packs.remove_failed", &[("detail", e)]));
        }
    }
}
