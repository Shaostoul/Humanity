//! Settings > AI (2026-11-09; docs/design/embeddable-ai.md, sections 4.7, 5.1, 5.2 and 11; rung 1).
//! Child of settings.rs.
//!
//! The door, with nothing behind it yet. Three parts:
//!
//! - **"No AI is installed. Everything in HumanityOS works without it."** (5.2, step 1), and what that
//!   means: no model and no AI engine on this computer, and each feature's non-AI way (5.1). Nothing
//!   can be installed in rung 1: the downloads come with rung 2, and the section says so instead of
//!   showing a button that does nothing.
//! - **Your computer** (4.7): memory, processor cores and features, the graphics card, free space, read
//!   now (src/ai/hardware.rs), in plain words.
//! - **What each choice will need**: one card per choice in data/ai/choices.ron (the model and its
//!   maker, what it needs while you play, whether this computer has it, and what works instead), so a
//!   person knows before rung 2 whether their computer will run it.
//!
//! When the engine has stopped twice in a row (or speaks another protocol version), the section says so
//! and offers Try again, the in-app way back (GUI-first). The website's Settings shows the same section's
//! words, saying the AI features are desktop-only and why (5.5; web/pages/settings.html).

use egui::{Color32, RichText};
use humanity_core::i18n::{t, t_dyn, t_marked, t_marked_dyn, tf};

use crate::ai::client::{ExitWhy, State};
use crate::ai::hardware::{self, gb, Hardware, Notice};
use crate::ai::{choices, paths, shared};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button};
use crate::gui::GuiState;

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

fn line(ui: &mut egui::Ui, theme: &Theme, color: Color32, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(color).size(theme.font_size_small));
}

/// The section, drawn inside Settings' AI band.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    match paths::installed_engine() {
        None => {
            ui.label(RichText::new(t("ai.none_installed")).color(theme.success()).size(theme.font_size_body).strong());
        }
        Some(_) => {
            ui.label(RichText::new(tf("ai.engine_only", &[("version", &paths::ENGINE_VERSION)])).color(theme.text_primary()).size(theme.font_size_body).strong());
        }
    }
    ui.add_space(theme.spacing_xs);
    widgets::glossary_words(ui, theme, t_marked("ai.none_meaning"), theme.font_size_small, theme.text_secondary());
    ui.add_space(theme.spacing_xs);
    muted(ui, theme, t("ai.when_chosen"));
    muted(ui, theme, t("ai.not_yet"));
    draw_engine_trouble(ui, theme);

    widgets::subsection_header(ui, theme, accent, t("ai.your_computer.title"), "");
    widgets::setting_hint(ui, theme, hint, t("ai.your_computer.intro"));
    let hw = hardware::read();
    draw_computer(ui, theme, &hw);

    widgets::subsection_header(ui, theme, accent, t("ai.choices.title"), "");
    widgets::setting_hint(ui, theme, hint, t("ai.choices.intro"));
    match choices::current() {
        Err(e) => line(ui, theme, theme.danger(), tf("ai.choices.load_error", &[("detail", &e)])),
        Ok(all) => {
            for feature in &all.features {
                ui.add_space(theme.spacing_md);
                ui.label(RichText::new(t_dyn(&feature.name)).color(theme.text_primary()).strong());
                widgets::glossary_words(ui, theme, t_marked_dyn(&feature.about), theme.font_size_small, theme.text_secondary());
                muted(ui, theme, tf("ai.choices.without", &[("words", &t_dyn(&feature.without))]));
                for choice in all.choices.iter().filter(|c| c.feature == feature.id) {
                    ui.add_space(theme.spacing_sm);
                    draw_choice(ui, theme, &all, choice, &hw);
                }
            }
        }
    }
}

/// The engine's trouble, when there is any: stopped twice in a row, or refused, with Try again.
fn draw_engine_trouble(ui: &mut egui::Ui, theme: &Theme) {
    let mut c = shared::client();
    let words = match c.state() {
        State::Halted { why } => format!("{} {}", t("ai.state.halted"), why_words(why)),
        State::Refused { error } => {
            let fill = error.fill();
            let vars: Vec<(&str, &dyn std::fmt::Display)> = fill.iter().map(|(k, v)| (*k, v as &dyn std::fmt::Display)).collect();
            tf(error.words_key(), &vars)
        }
        _ => return,
    };
    ui.add_space(theme.spacing_sm);
    line(ui, theme, theme.warning(), words);
    if Button::secondary(t("ai.try_again")).hint("ai.try_again_hint").show(ui, theme) {
        c.try_again();
        drop(c);
        shared::watch();
    }
}

/// Why the engine stopped, in words.
pub(crate) fn why_words(why: &ExitWhy) -> String {
    match why {
        ExitWhy::Code(code) => tf("ai.exit.code", &[("code", code)]),
        ExitWhy::NoHello(secs) => tf("ai.exit.no_hello", &[("secs", secs)]),
        ExitWhy::StartFailed(detail) => tf("ai.exit.start_failed", &[("detail", detail)]),
        ExitWhy::Unknown => t("ai.exit.unknown").to_string(),
    }
}

/// "Your computer": each reading in plain words, "not reported" where the system did not say.
fn draw_computer(ui: &mut egui::Ui, theme: &Theme, hw: &Hardware) {
    let text = theme.text_secondary();
    match (hw.memory_free_mb, hw.memory_total_mb) {
        (Some(free), Some(total)) => line(ui, theme, text, tf("ai.hw.memory", &[("free", &gb(free)), ("total", &gb(total))])),
        _ => line(ui, theme, text, t("ai.hw.memory_unknown")),
    }
    if hw.pretending {
        line(ui, theme, theme.warning(), tf("ai.hw.pretend", &[("free", &gb(hw.memory_free_mb.unwrap_or(0)))]));
    }
    match hw.cores {
        Some(cores) => line(ui, theme, text, tf("ai.hw.cores", &[("cores", &cores), ("threads", &hw.threads)])),
        None => line(ui, theme, text, tf("ai.hw.cores_unknown", &[("threads", &hw.threads)])),
    }
    let features = match hw.avx2 {
        Some(true) => t_marked("ai.hw.avx2_yes"),
        Some(false) => t_marked("ai.hw.avx2_no"),
        None => t_marked("ai.hw.avx2_other"),
    };
    widgets::glossary_words(ui, theme, features, theme.font_size_small, if hw.avx2 == Some(false) { theme.warning() } else { text });
    match &hw.gpu {
        Some(g) if g.integrated => line(ui, theme, text, tf("ai.hw.gpu_shared", &[("name", &g.name)])),
        Some(g) => match g.vram_mb {
            Some(v) => line(ui, theme, text, tf("ai.hw.gpu", &[("name", &g.name), ("vram", &gb(v))])),
            None => line(ui, theme, text, tf("ai.hw.gpu_no_size", &[("name", &g.name)])),
        },
        None => line(ui, theme, text, t("ai.hw.gpu_unknown")),
    }
    match (hw.disk_free_mb, &hw.folder) {
        (Some(free), Some(folder)) => line(ui, theme, text, tf("ai.hw.disk", &[("free", &gb(free)), ("folder", &folder.display())])),
        (None, Some(folder)) => line(ui, theme, text, tf("ai.hw.disk_unread", &[("folder", &folder.display())])),
        _ => line(ui, theme, text, t("ai.hw.disk_unknown")),
    }
}

/// One choice's card: its name, its model and maker, what it needs, what this computer has, and what
/// works instead when it does not fit.
fn draw_choice(ui: &mut egui::Ui, theme: &Theme, all: &choices::AiChoices, choice: &choices::Choice, hw: &Hardware) {
    let name = t_dyn(&choice.words);
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(name).color(theme.text_primary()).size(theme.font_size_body));
        muted(ui, theme, tf("ai.choices.model", &[("model", &choice.model), ("maker", &choice.maker)]));
        let n = &choice.needs;
        muted(ui, theme, tf("ai.choices.needs", &[("memory", &gb(n.memory_mb)), ("cores", &n.cores), ("disk", &gb(n.disk_mb))]));
        if let Some(v) = n.vram_mb {
            muted(ui, theme, tf("ai.choices.needs_gpu", &[("vram", &gb(v))]));
        }
        let notices = hardware::check(choice, hw);
        let short = notices.iter().any(Notice::is_warning);
        for notice in &notices {
            let color = if notice.is_warning() { theme.warning() } else { theme.success() };
            line(ui, theme, color, notice_words(notice, name));
        }
        if short {
            if let Some(fb) = choice.fallback.as_deref().and_then(|id| all.choice(id)) {
                muted(ui, theme, tf("ai.choices.instead_choice", &[("name", &t_dyn(&fb.words)), ("memory", &gb(fb.needs.memory_mb))]));
            }
            if let Some(f) = all.feature(&choice.feature) {
                muted(ui, theme, tf("ai.choices.without", &[("words", &t_dyn(&f.without))]));
            }
        }
    });
}

/// One notice of 4.7, in words.
pub(crate) fn notice_words(n: &Notice, name: &str) -> String {
    match n {
        Notice::Fits => t("ai.need.fits").to_string(),
        Notice::Memory { need_mb, free_mb, total_mb: Some(total) } => tf("ai.need.memory", &[("name", &name), ("need", &gb(*need_mb)), ("free", &gb(*free_mb)), ("total", &gb(*total))]),
        Notice::Memory { need_mb, free_mb, total_mb: None } => tf("ai.need.memory_no_total", &[("name", &name), ("need", &gb(*need_mb)), ("free", &gb(*free_mb))]),
        Notice::Cores { need, have } => tf("ai.need.cores", &[("name", &name), ("need", need), ("have", have)]),
        Notice::NoAvx2 => tf("ai.need.avx2", &[("name", &name)]),
        Notice::Disk { need_mb, free_mb, folder } => tf("ai.need.disk", &[("need", &gb(*need_mb)), ("free", &gb(*free_mb)), ("folder", &folder.display())]),
        Notice::Gpu { name: adapter, vram_mb, holds: true } => tf("ai.need.gpu_can", &[("name", &name), ("adapter", adapter), ("vram", &gb(*vram_mb))]),
        Notice::Gpu { name: adapter, vram_mb, holds: false } => tf("ai.need.gpu_cannot", &[("name", &name), ("adapter", adapter), ("vram", &gb(*vram_mb))]),
    }
}
