//! Settings > Audio > Instruments (2026-11-06; docs/design/music.md rung 6). Child of settings.rs.
//!
//! The operator: "in the settings we can have a section for filling out the instrument list, kind
//! of like we do with the star catalog and skybox." A list like the star catalog's tiers: the set
//! that comes with the app, then each downloadable pack with what it holds, its size, its licence in
//! plain words, whether it is installed, Download / Remove, the download's progress, and which of
//! its instruments the music uses now. The work behind the buttons runs in
//! `instrument_packs::ui::tick` (the engine calls it each frame; it owns the data folder).
//!
//! The website has no twin of this section: it plays no music yet (music.md rung 9).

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn, t_marked, t_marked_dyn, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets::{self, with_hint, Button};
use crate::gui::HintDisplay;
use crate::instrument_packs::catalog::{InstrumentRef, PackEntry, WaveSetRef};
use crate::instrument_packs::install::{Failure, Stage};
use crate::instrument_packs::library::Choice;
use crate::instrument_packs::ui::PacksUi;

/// Megabytes with one decimal, as every size here is shown.
pub(crate) fn mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.0)
}

fn instrument_name(id: &str) -> &'static str {
    t_dyn(&format!("instrument_packs.instrument.{id}"))
}

/// The names of a set's instruments, joined.
fn names(refs: &[InstrumentRef]) -> String {
    refs.iter().map(|r| instrument_name(&r.id)).collect::<Vec<_>>().join(", ")
}

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

fn warn(ui: &mut egui::Ui, theme: &Theme, color: Color32, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(color).size(theme.font_size_small));
}

/// A set of more instruments than this says how many it holds, and keeps their names, and what
/// each plays, folded until opened (2026-11-13: the full percussion pack holds hundreds, and listed
/// open they made the page a scroll through every take of every drum).
pub(crate) const LIST_OPEN_MAX: usize = 12;

/// What a set holds: its instruments (named, or counted when there are many), then each wave set
/// and its count (2026-11-13: AKWF's single cycles, which the synthesiser's wavetable instruments
/// are built from since rung E5, 2026-11-14). A set of waves alone has no instrument line.
fn holds(ui: &mut egui::Ui, theme: &Theme, instruments: &[InstrumentRef], waves: &[WaveSetRef]) {
    if instruments.len() > LIST_OPEN_MAX {
        muted(ui, theme, tf("instrument_packs.holds_count", &[("count", &instruments.len().to_string())]));
    } else if !instruments.is_empty() {
        muted(ui, theme, tf("instrument_packs.holds", &[("list", &names(instruments))]));
    }
    for w in waves {
        let name = t_dyn(&format!("instrument_packs.waveset.{}", w.id));
        muted(ui, theme, tf("instrument_packs.waves_held", &[("count", &w.count.to_string()), ("name", &name)]));
    }
}

/// What a set's instruments take in memory while its parts play (instrument_packs::memory,
/// 2026-11-14): the honest cost of a pack, as a graphics setting states its video memory. A set
/// whose long recordings stream says what they would take held whole.
fn memory_line(ui: &mut egui::Ui, theme: &Theme, packs: &PacksUi, set: &str) {
    let counting = packs.memory_for.is_none() || (packs.memory_job.is_some() && !packs.memory.contains_key(set));
    let text = match packs.memory.get(set) {
        _ if counting => t("instrument_packs.memory_counting").to_string(),
        Some(m) if m.held < m.whole => tf("instrument_packs.memory_streamed", &[("mb", &mb(m.held)), ("whole", &mb(m.whole))]),
        Some(m) => tf("instrument_packs.memory", &[("mb", &mb(m.held))]),
        None => t("instrument_packs.memory_none").to_string(),
    };
    widgets::glossary_words(ui, theme, &text, theme.font_size_small, theme.text_muted());
}

/// "Licence: ..." with the licence words' glossary marks kept (CC0 opens its glossary card).
fn licence_line(ui: &mut egui::Ui, theme: &Theme, marked_words: &str) {
    let line = tf("instrument_packs.licence_label", &[("words", &marked_words)]);
    widgets::glossary_words(ui, theme, &line, theme.font_size_small, theme.text_muted());
}

/// The section, drawn inside Settings > Audio.
pub(crate) fn draw_instruments(ui: &mut egui::Ui, theme: &Theme, hint: HintDisplay, accent: Color32, packs: &mut PacksUi) {
    packs.shown = true;
    widgets::subsection_header(ui, theme, accent, t("instrument_packs.title"), "");
    widgets::card(ui, theme, |ui| {
        widgets::setting_hint(ui, theme, hint, t("instrument_packs.intro"));
        if let Some(e) = &packs.load_error {
            warn(ui, theme, theme.danger(), tf("instrument_packs.load_error", &[("detail", e)]));
        }
        let Some(catalog) = packs.catalog.clone() else { return };
        if !packs.library.problems.is_empty() {
            let detail = packs.library.problems.join("; ");
            warn(ui, theme, theme.warning(), tf("instrument_packs.problems", &[("detail", &detail)]));
        }
        // What comes with the app: always there, nothing to download or remove.
        ui.add_space(theme.spacing_sm);
        ui.label(RichText::new(t_dyn(&catalog.core.words)).color(theme.text_secondary()).strong());
        holds(ui, theme, &catalog.core.instruments, &catalog.core.waves);
        muted(ui, theme, tf("instrument_packs.size", &[("mb", &mb(catalog.core.bytes))]));
        licence_line(ui, theme, t_marked("instrument_packs.licence.cc0"));
        muted(ui, theme, t("instrument_packs.core_status"));
        memory_line(ui, theme, packs, "core");
        draw_uses(ui, theme, packs, "core", &catalog.core.instruments, true);
        for entry in &catalog.packs {
            ui.add_space(theme.spacing_md);
            draw_pack(ui, theme, entry, packs);
        }
        ui.add_space(theme.spacing_md);
        muted(ui, theme, t("instrument_packs.fallback_note"));
    });
}

fn failure_words(f: &Failure) -> String {
    match f {
        Failure::Network(d) => tf("instrument_packs.failed.network", &[("detail", d)]),
        Failure::Damaged(d) => tf("instrument_packs.failed.damaged", &[("detail", d)]),
        Failure::Disk(d) => tf("instrument_packs.failed.disk", &[("detail", d)]),
        Failure::NotAPack(d) => tf("instrument_packs.failed.not_a_pack", &[("detail", d)]),
        Failure::NotPublished => t("instrument_packs.failed.not_published").to_string(),
    }
}

fn draw_pack(ui: &mut egui::Ui, theme: &Theme, entry: &PackEntry, packs: &mut PacksUi) {
    ui.label(RichText::new(t_dyn(&entry.words)).color(theme.text_secondary()).strong());
    if let Some(about) = &entry.about {
        muted(ui, theme, t_dyn(about));
    }
    holds(ui, theme, &entry.instruments, &entry.waves);
    if entry.is_built() {
        muted(ui, theme, tf("instrument_packs.size", &[("mb", &mb(entry.bytes))]));
    }
    licence_line(ui, theme, t_marked_dyn(&entry.licence));
    let installed = packs.installed.get(&entry.id).cloned();
    let current = packs.is_current(&entry.id);
    let status = match (&installed, current, entry.is_built()) {
        (Some(i), true, _) => tf("instrument_packs.installed", &[("mb", &mb(i.bytes))]),
        (Some(_), false, _) => t("instrument_packs.update_available").to_string(),
        (None, _, true) => t("instrument_packs.not_installed").to_string(),
        (None, _, false) => t("instrument_packs.not_built").to_string(),
    };
    muted(ui, theme, status);
    if installed.is_some() {
        memory_line(ui, theme, packs, &entry.id);
    }

    // This pack's download, if one is running or has just failed.
    let mine = packs.download.as_ref().filter(|(id, _)| *id == entry.id).and_then(|(_, p)| p.lock().ok().map(|g| g.clone()));
    let busy = packs.downloading();
    let failure = match mine.as_ref().map(|p| &p.stage) {
        Some(Stage::Failed(f)) => Some(f.clone()),
        _ => None,
    };
    match &mine {
        Some(p) if failure.is_none() && p.stage != Stage::Done => {
            let frac = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
            let words = match p.stage {
                Stage::Connecting => t("instrument_packs.stage.connecting").to_string(),
                Stage::Downloading => tf("instrument_packs.stage.downloading", &[("done", &mb(p.done)), ("total", &mb(p.total.max(entry.bytes)))]),
                Stage::Checking => t("instrument_packs.stage.checking").to_string(),
                _ => t("instrument_packs.stage.installing").to_string(),
            };
            ui.add(egui::ProgressBar::new(frac).text(words));
        }
        _ => {
            if let Some(f) = &failure {
                warn(ui, theme, theme.danger(), failure_words(f));
            }
            ui.horizontal(|ui| {
                if installed.is_none() || !current {
                    let (label, key) = if failure.is_some() {
                        (t("instrument_packs.retry"), "instrument_packs.retry_hint")
                    } else if installed.is_some() {
                        (t("instrument_packs.update"), "instrument_packs.update_hint")
                    } else {
                        (t("instrument_packs.download"), "instrument_packs.download_hint")
                    };
                    if Button::primary(label).hint(key).disabled(busy || !entry.is_built()).show(ui, theme) {
                        packs.request_download = Some(entry.id.clone());
                    }
                }
                if installed.is_some()
                    && Button::secondary(t("instrument_packs.remove")).hint("instrument_packs.remove_hint").disabled(busy).show(ui, theme)
                {
                    packs.request_remove = Some(entry.id.clone());
                }
            });
        }
    }
    if let Some((id, e)) = &packs.remove_error {
        if *id == entry.id {
            warn(ui, theme, theme.danger(), tf("instrument_packs.remove_failed", &[("detail", e)]));
        }
    }
    draw_uses(ui, theme, packs, &entry.id, &entry.instruments, installed.is_some());
}

/// For each instrument: the part it plays, and whether the music uses it now, has a better one, or
/// (its pack not installed) what plays that part instead; and, for a part no mood or style of the
/// music names (`PacksUi::used_parts`), that the music does not use that part yet (BUG-266: every
/// listed instrument was said to "play the part" whenever it was the best for it, heard or not).
fn draw_uses(ui: &mut egui::Ui, theme: &Theme, packs: &PacksUi, set_id: &str, refs: &[InstrumentRef], installed: bool) {
    if refs.is_empty() {
        return;
    }
    if refs.len() <= LIST_OPEN_MAX {
        muted(ui, theme, t("instrument_packs.uses_title"));
        uses_lines(ui, theme, packs, refs, installed);
        return;
    }
    // Many instruments: folded, under a header that says how many (each set's own fold, by its id).
    let title = tf("instrument_packs.uses_list", &[("count", &refs.len().to_string())]);
    ui.push_id(("instrument_uses", set_id), |ui| {
        with_hint(ui, "instrument_packs.uses_list_hint", |ui| {
            widgets::collapsible_section(ui, &title, false, |ui| uses_lines(ui, theme, packs, refs, installed))
        });
    });
}

fn uses_lines(ui: &mut egui::Ui, theme: &Theme, packs: &PacksUi, refs: &[InstrumentRef], installed: bool) {
    for r in refs {
        let name = instrument_name(&r.id);
        let part_name = t_dyn(&format!("instrument_packs.part.{}", r.part));
        if packs.used_parts.as_ref().is_some_and(|used| !used.contains(&r.part)) {
            let args: &[(&str, &dyn std::fmt::Display)] = &[("instrument", &name), ("part", &part_name)];
            let line = if installed { tf("instrument_packs.use.not_used_yet_installed", args) } else { tf("instrument_packs.use.not_used_yet", args) };
            muted(ui, theme, format!("  {line}"));
            continue;
        }
        let now = match packs.parts.as_ref().and_then(|p| packs.library.choose(&r.part, p)) {
            Some(Choice::Sampled(a)) => Some((a.def.id.clone(), instrument_name(&a.def.id))),
            _ => None,
        };
        let other = now.as_ref().map(|(_, n)| *n).unwrap_or_else(|| t("instrument_packs.synth_voice"));
        let line = if installed && now.as_ref().is_some_and(|(id, _)| *id == r.id) {
            tf("instrument_packs.use.in_use", &[("instrument", &name), ("part", &part_name)])
        } else if installed {
            tf("instrument_packs.use.standing_by", &[("instrument", &name), ("part", &part_name), ("other", &other)])
        } else {
            tf("instrument_packs.use.would_play", &[("instrument", &name), ("part", &part_name), ("now", &other)])
        };
        muted(ui, theme, format!("  {line}"));
    }
}
