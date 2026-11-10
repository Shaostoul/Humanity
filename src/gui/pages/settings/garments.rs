//! Settings > Graphics > Garment textures (2026-11-10; docs/design/character-animation.md section
//! 16, docs/design/graphics-settings.md). Child of settings.rs.
//!
//! How sharp the textures of the clothes, shoes, hats, hair, brows and lashes people wear are drawn.
//! The levels are data (data/gui/garment_textures.json: 512, 1K, 2K; a level no garment's art
//! reaches, 4K today, is not offered), each with what you will see (its note), the video memory its
//! garment texture pools take on this graphics card (worked out from the garments and the near tier,
//! `gui::garment_textures::plan_for`), and, above the shipped 1K, its download (size, progress,
//! installed, Remove), the furniture detail setting's and the model packs' ways. Under the list: how
//! many garments are drawn at their own largest at the chosen level, the level drawn while a download
//! is still to come, the card's line (block compression read, or the notice saying it is not), a
//! notice when the chosen level's pools would not fit the card comfortably (what it needs, what the
//! card has, what works instead: the hardware-notice rule, CLAUDE.md), and the own-mesh pool's memory.
//!
//! The website has no twin of this section: it draws no 3D people.

use std::collections::BTreeMap;

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn, tf};

use super::graphics::graphics_hint;
use crate::gui::garment_textures::{self as gt, LevelPlan};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, Hint};
use crate::gui::{GuiState, HintDisplay};
use crate::instrument_packs::install::{Failure, Progress, Stage};
use crate::renderer::bounced_light_detail::{mb_words, AdapterMemory};

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

/// Everything the section shows, gathered from the engine's plan and the packs' state (or a
/// snapshot's made-up machine).
pub(crate) struct View<'a> {
    pub plan: Option<&'a gt::Plan>,
    pub meshes: Option<&'a gt::MeshPlan>,
    /// The block-compression line, or the notice that the card has none.
    pub line: Option<&'a str>,
    pub notice: Option<&'a str>,
    pub card: Option<&'a AdapterMemory>,
    /// The packs installed at the catalog's version, a download's progress, and what stopped downloads
    /// left to resume (bytes).
    pub installed: &'a [String],
    pub download: Option<(String, Progress)>,
    pub partial: &'a BTreeMap<String, u64>,
    pub read_error: Option<&'a str>,
    pub remove_error: Option<(&'a str, &'a str)>,
}

/// What the person asked for this frame.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Requests {
    pub download: Option<String>,
    pub remove: Option<String>,
}

/// The subsection, drawn inside Settings > Graphics, with this machine's plan, packs and card.
pub(crate) fn draw_garment_textures(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, accent: Color32) {
    let compressed = crate::renderer::texture_pool::block_compression() != Some(false);
    let (line, notice) = gt::this_machine_line();
    let card = crate::renderer::bounced_light_detail::adapter_memory();
    let meshes = gt::mesh_plan();
    let (plan, installed, download, partial, read_error, remove_error) = {
        let s = gt::shared();
        let installed: Vec<String> = gt::packs::catalog().packs.iter().filter(|p| s.is_installed(&p.id)).map(|p| p.id.clone()).collect();
        let download = s.download.as_ref().and_then(|(id, p)| p.lock().ok().map(|g| (id.clone(), g.clone())));
        (s.plan(compressed), installed, download, s.partial.clone(), s.read_error.clone(), s.remove_error.clone())
    };
    let view = View {
        plan: plan.as_ref(),
        meshes: meshes.as_ref(),
        line: line.as_deref(),
        notice: notice.as_deref(),
        card,
        installed: &installed,
        download,
        partial: &partial,
        read_error: read_error.as_deref(),
        remove_error: remove_error.as_ref().map(|(a, b)| (a.as_str(), b.as_str())),
    };
    let asked = draw_with(ui, theme, state, hint, accent, &view);
    let mut s = gt::shared();
    if asked.download.is_some() {
        s.request_download = asked.download;
    }
    if asked.remove.is_some() {
        s.request_remove = asked.remove;
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

/// The download column of a level's row: what it needs to download, or that it came with the app.
fn download_words(lp: &LevelPlan, view: &View) -> String {
    if !lp.published {
        return t("garment_textures.unpublished").to_string();
    }
    if lp.packs.is_empty() {
        return t("garment_textures.comes_with").to_string();
    }
    let size = mb_words(lp.download);
    if lp.packs.iter().all(|p| view.installed.contains(p)) {
        return tf("garment_textures.downloaded", &[("size", &size)]);
    }
    if let Some((id, p)) = &view.download {
        if lp.packs.contains(id) && !matches!(p.stage, Stage::Done | Stage::Failed(_)) {
            return tf("garment_textures.downloading", &[("done", &mb_words(p.done)), ("size", &size)]);
        }
    }
    let here: u64 = lp.packs.iter().filter_map(|p| view.partial.get(p)).sum();
    if here > 0 {
        return tf("garment_textures.download_partial", &[("size", &size), ("done", &mb_words(here))]);
    }
    tf("garment_textures.download_size", &[("size", &size)])
}

/// The subsection with the view given (the page's, or a snapshot's). Returns what the person asked for.
pub(crate) fn draw_with(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, hint: HintDisplay, accent: Color32, view: &View) -> Requests {
    let mut asked = Requests::default();
    widgets::subsection_header(ui, theme, accent, t("garment_textures.title"), "");
    let installed = |id: &str| view.installed.iter().any(|p| p == id);
    let Some(plan) = view.plan else {
        match view.read_error {
            Some(e) => muted(ui, theme, tf("garment_textures.plan_unreadable", &[("detail", &e)])),
            None => muted(ui, theme, t("garment_textures.memory_unmeasured")),
        }
        graphics_hint(ui, theme, hint, "garment_textures");
        return asked;
    };
    let busy = view.download.as_ref().is_some_and(|(_, p)| !matches!(p.stage, Stage::Done | Stage::Failed(_)));
    // A grid, so the memory and download figures line up for comparing (the two in one cell, one
    // above the other, so the rows fit the page's width).
    egui::Grid::new("garment_texture_levels").num_columns(3).show(ui, |ui| {
        for lp in &plan.levels {
            let level = gt::level(&lp.id);
            let sel = state.settings.garment_textures == lp.id;
            let label = RichText::new(t_dyn(&level.words)).size(theme.font_size_small);
            if ui.selectable_label(sel, label).hint("garment_textures.level_hint").clicked() && !sel {
                state.settings.garment_textures = lp.id.clone();
                state.settings_dirty = true;
                // Picking a level that needs a download starts it (its size was on the row).
                if !busy {
                    asked.download = lp.packs.iter().find(|p| !installed(p)).cloned();
                }
            }
            muted(ui, theme, t_dyn(&level.note));
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                muted(ui, theme, tf("garment_textures.memory", &[("memory", &mb_words(lp.bytes))]));
                muted(ui, theme, download_words(lp, view));
            });
            ui.end_row();
        }
    });
    graphics_hint(ui, theme, hint, "garment_textures");
    let chosen = state.settings.garment_textures.clone();
    let drawn = gt::effective(plan, &chosen, installed);
    let chosen_plan = plan.level(&chosen).or(drawn);
    if let Some(lp) = chosen_plan.and_then(gt::below_words) {
        muted(ui, theme, lp);
    }
    // The level drawn while the chosen one's files are still to come.
    if let (Some(d), Some(c)) = (drawn, plan.level(&chosen)) {
        if d.id != c.id {
            let now = t_dyn(&gt::level(&d.id).words).to_string();
            let level = t_dyn(&gt::level(&c.id).words).to_string();
            let key = if c.published { "garment_textures.drawing_until" } else { "garment_textures.drawing_unpublished" };
            muted(ui, theme, tf(key, &[("now", &now), ("level", &level)]));
            // Chosen before (a saved setting, another session) and its download not started or
            // finished here: the button that starts it (a failed one has Try again below).
            let missing = c.packs.iter().find(|p| !installed(p)).cloned();
            let failed = view.download.as_ref().is_some_and(|(_, p)| matches!(p.stage, Stage::Failed(_)));
            if let (Some(pack), true, false, false) = (missing, c.published, busy, failed) {
                let words = tf("garment_textures.download", &[("level", &level), ("size", &mb_words(c.download))]);
                if Button::primary(&words).hint("garment_textures.download_hint").show(ui, theme) {
                    asked.download = Some(pack);
                }
            }
        }
    }
    // A download that stopped: what went wrong, and the button that resumes it.
    if let Some((id, p)) = &view.download {
        if let Stage::Failed(f) = &p.stage {
            ui.label(RichText::new(failure_words(f)).color(theme.danger()).size(theme.font_size_small));
            if Button::secondary(t("garment_textures.retry")).hint("garment_textures.retry_hint").disabled(busy).show(ui, theme) {
                asked.download = Some(id.clone());
            }
        } else if !matches!(p.stage, Stage::Done) {
            let frac = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
            ui.add(egui::ProgressBar::new(frac).text(tf("garment_textures.downloading", &[("done", &mb_words(p.done)), ("size", &mb_words(p.total))])));
        }
    }
    // Each installed pack, or a stopped download's leftovers, with its Remove button.
    for pack in &gt::packs::catalog().packs {
        let here = view.partial.get(&pack.id).copied().unwrap_or(0);
        if !installed(&pack.id) && here == 0 {
            continue;
        }
        let level = plan.levels.iter().find(|l| l.px == pack.px).map_or_else(|| pack.id.clone(), |l| t_dyn(&gt::level(&l.id).words).to_string());
        let size = mb_words(if installed(&pack.id) { pack.bytes } else { here });
        let words = tf("garment_textures.remove", &[("level", &level), ("size", &size)]);
        if Button::secondary(&words).hint("garment_textures.remove_hint").disabled(busy).show(ui, theme) {
            asked.remove = Some(pack.id.clone());
        }
    }
    if let Some((id, why)) = view.remove_error {
        ui.label(RichText::new(tf("model_packs.remove_failed", &[("detail", &format!("{id}: {why}"))])).color(theme.danger()).size(theme.font_size_small));
    }
    muted(ui, theme, gt::card_line(view.card));
    if let Some(w) = gt::memory_notice(plan, &chosen, view.card) {
        ui.label(RichText::new(w).color(theme.warning()).size(theme.font_size_small));
    }
    if let Some(m) = view.meshes {
        muted(ui, theme, gt::mesh_memory_words(m));
    }
    if let Some(line) = view.line {
        widgets::glossary_words(ui, theme, line, theme.font_size_small, theme.text_muted());
    }
    if let Some(notice) = view.notice {
        widgets::glossary_words(ui, theme, notice, theme.font_size_small, theme.warning());
    }
    asked
}
