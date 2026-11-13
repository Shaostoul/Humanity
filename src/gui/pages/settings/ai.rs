//! Settings > AI (2026-11-09; docs/design/embeddable-ai.md, sections 4.7, 5.1 to 5.3 and 11; rungs 1
//! and 2). Child of settings.rs.
//!
//! - **The top line**: "No AI is installed. Everything in HumanityOS works without it." (5.2, step 1),
//!   or what is installed, with its size; what AI being optional means (5.1).
//! - **Your computer** (4.7): memory, processor cores and features, the graphics card, free space, read
//!   now (src/ai/hardware.rs), in plain words.
//! - **One card per choice** in data/ai/choices.ron: the model and its maker, what it needs while you
//!   play, whether this computer has it and what works instead (4.7). A choice the built-in catalog can
//!   download (src/ai/catalog.rs; rung 2: Talking characters (Quick), Granite 4.0 1B; 2026-11-11:
//!   Talking characters (Rich), Ministral 3 3B) also shows, BEFORE
//!   anything downloads, its download size (the AI engine's added the first time: the engine comes
//!   with the first model and goes with the last), where it comes from, and its licence, whose full
//!   text opens right on the card (5.2); then Get, its progress, Stop (what arrived is kept, and Get
//!   carries on from there), and the words for a failure (a mismatch is deleted, 5.2); once installed,
//!   its size and Remove.
//! - **Remove all AI** (5.3), when anything is installed: a confirmation that lists exactly what goes,
//!   each with its size, and says it cannot be undone; then the whole `ai` folder is deleted.
//!
//! The downloads run on a thread of their own (src/ai/install.rs); this section only draws their
//! progress. The section's own state (the download in hand, the open licence, the confirmation) lives
//! in [`STATE`], not on `GuiState` (src/gui/mod.rs is at its line budget). When the engine has stopped
//! twice in a row (or speaks another protocol version), the section says so and offers Try again. The
//! website's Settings shows the same section's words, saying the AI features are desktop-only and why
//! (5.5; web/pages/settings.html).

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};

use egui::{Color32, RichText};
use humanity_core::i18n::{fill, t, t_dyn, t_marked, t_marked_dyn, tf};

use crate::ai::catalog::{self, AiCatalog, ModelEntry};
use crate::ai::client::{ExitWhy, State};
use crate::ai::hardware::{self, gb, Hardware, Notice};
use crate::ai::install::{self, Failure, Installed, ProgressHandle, Stage};
use crate::ai::{choices, models, paths, shared};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, Button, Hint};
use crate::gui::GuiState;

/// What the section keeps between frames for this run.
#[derive(Default)]
struct AiSection {
    /// The download in hand (a model id and its job), kept after it ends so its outcome shows.
    job: Option<(String, ProgressHandle)>,
    /// The cards whose licence text is open.
    licence_open: BTreeSet<String>,
    /// Remove all AI's confirmation is showing.
    confirm_remove_all: bool,
    /// The last removal's outcome, in words, and whether it is a warning.
    message: Option<(String, bool)>,
}

static STATE: Mutex<Option<AiSection>> = Mutex::new(None);

fn with_state<R>(f: impl FnOnce(&mut AiSection) -> R) -> R {
    let mut guard = STATE.lock().unwrap_or_else(|p| p.into_inner());
    f(guard.get_or_insert_with(AiSection::default))
}

fn muted(ui: &mut egui::Ui, theme: &Theme, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(theme.text_muted()).size(theme.font_size_small));
}

fn line(ui: &mut egui::Ui, theme: &Theme, color: Color32, text: impl Into<String>) {
    ui.label(RichText::new(text.into()).color(color).size(theme.font_size_small));
}

/// A size in words: GB with two decimals from a tenth of a GB up ("0.95 GB"), MB below it ("13 MB"),
/// so the engine is never "0.00 GB". GB and MB of 1,024, as Windows counts them.
pub(crate) fn gb_bytes(bytes: u64) -> String {
    let gb = bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 0.1 {
        tf("ai.size.gb", &[("n", &format!("{gb:.2}"))])
    } else {
        tf("ai.size.mb", &[("n", &format!("{:.0}", (bytes as f64 / (1024.0 * 1024.0)).max(1.0)))])
    }
}

/// The section, drawn inside Settings' AI band.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, accent: Color32) {
    let hint = state.settings.hint_display;
    let cat = catalog::current();
    let root = paths::ai_root();
    let have = match (&cat, &root) {
        (Ok(c), Some(r)) => install::installed(c, r),
        _ => Installed::default(),
    };
    finish_job_if_done();

    if have.engine.is_none() && have.models.is_empty() {
        ui.label(RichText::new(t("ai.none_installed")).color(theme.success()).size(theme.font_size_body).strong());
    } else if let Ok(c) = &cat {
        draw_installed_summary(ui, theme, c, &have);
    }
    ui.add_space(theme.spacing_xs);
    widgets::glossary_words(ui, theme, t_marked("ai.none_meaning"), theme.font_size_small, theme.text_secondary());
    ui.add_space(theme.spacing_xs);
    muted(ui, theme, t("ai.when_chosen"));
    muted(ui, theme, t("ai.not_yet"));
    if let Err(e) = &cat {
        line(ui, theme, theme.danger(), tf("ai.catalog_error", &[("detail", e)]));
    }
    draw_engine_trouble(ui, theme);
    if let (Ok(c), Some(r)) = (&cat, &root) {
        if have.engine.is_none() && !have.models.is_empty() {
            draw_engine_update(ui, theme, accent, c, r);
        }
    }

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
                    let offer = cat.as_ref().ok().and_then(|c| c.for_choice(&choice.id).map(|m| (c.clone(), m.clone())));
                    draw_choice(ui, theme, &all, choice, &hw, offer, root.as_deref(), &have);
                }
                if feature.id == "characters" {
                    if let Ok(c) = &cat {
                        draw_talk_choices(ui, theme, state, c, &have, &hw);
                    }
                }
            }
        }
    }

    if !have.nothing() {
        if let (Ok(c), Some(r)) = (&cat, &root) {
            widgets::subsection_header(ui, theme, accent, t("ai.remove_all.title"), "");
            draw_remove_all(ui, theme, c, r, &have);
        }
    }
    if let Some((words, warning)) = with_state(|s| s.message.clone()) {
        ui.add_space(theme.spacing_sm);
        line(ui, theme, if warning { theme.warning() } else { theme.success() }, words);
    }
    if with_state(|s| s.job.as_ref().is_some_and(|(_, j)| j.running())) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
    }
}

/// How characters talk (rung 3, embeddable-ai.md 10): Written lines only, or with Say anything; and,
/// with two brains installed, which one (Automatic picks by this computer's memory).
fn draw_talk_choices(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, cat: &AiCatalog, have: &Installed, hw: &Hardware) {
    ui.add_space(theme.spacing_md);
    ui.label(RichText::new(t("ai.talk.mode_title")).color(theme.text_primary()).strong());
    muted(ui, theme, t("ai.talk.mode_intro"));
    let mut mode = state.settings.ai_talk_mode.clone();
    let (conversational, written) = (crate::gui::talk::chat::TALK_CONVERSATIONAL.to_string(), crate::gui::talk::chat::TALK_WRITTEN.to_string());
    ui.radio_value(&mut mode, conversational, RichText::new(t("ai.talk.mode_conversational")).color(theme.text_primary())).hint("ai.talk.mode_conversational_hint");
    ui.radio_value(&mut mode, written, RichText::new(t("ai.talk.mode_written")).color(theme.text_primary())).hint("ai.talk.mode_written_hint");
    if mode != state.settings.ai_talk_mode {
        state.settings.ai_talk_mode = mode;
        state.settings_dirty = true;
    }
    let root = paths::ai_root();
    let brains: Vec<(ModelEntry, std::path::PathBuf)> = cat.models.iter().filter(|m| m.kind == "brain" && have.models.contains_key(&m.id)).map(|m| (m.clone(), root.as_deref().map(|r| install::model_file(r, m)).unwrap_or_default())).collect();
    if brains.len() < 2 {
        return;
    }
    ui.add_space(theme.spacing_sm);
    ui.label(RichText::new(t("ai.talk.brain_title")).color(theme.text_primary()).strong());
    let auto = models::pick_brain(&brains, models::BRAIN_AUTO, hw.memory_total_mb).map(|(m, _)| m.model).unwrap_or_default();
    let mut pick = state.settings.ai_brain.clone();
    ui.radio_value(&mut pick, models::BRAIN_AUTO.to_string(), RichText::new(tf("ai.talk.brain_auto", &[("model", &auto)])).color(theme.text_primary())).hint("ai.talk.brain_auto_hint");
    for (m, _) in &brains {
        ui.radio_value(&mut pick, m.choice.clone(), RichText::new(&m.model).color(theme.text_primary())).hint("ai.talk.brain_pick_hint");
    }
    if pick != state.settings.ai_brain {
        state.settings.ai_brain = pick;
        state.settings_dirty = true;
    }
}

/// "An update to the AI engine is ready" (7.4; 2026-11-12, the first engine update): models installed
/// with an earlier app, and not this catalog's engine. Get downloads and checks the new engine and
/// removes the old one; the models stay.
fn draw_engine_update(ui: &mut egui::Ui, theme: &Theme, accent: Color32, cat: &Arc<AiCatalog>, root: &Path) {
    widgets::subsection_header(ui, theme, accent, t("ai.engine_update.title"), "");
    line(ui, theme, theme.text_primary(), tf("ai.engine_update.words", &[("version", &cat.engine.version), ("size", &gb_bytes(cat.engine.bytes))]));
    muted(ui, theme, t("ai.engine_update.what"));
    let (mine, busy) = with_state(|s| match &s.job {
        Some((id, job)) if id == ENGINE_JOB => (Some(job.clone()), false),
        Some((_, job)) => (None, job.running()),
        None => (None, false),
    });
    if let Some(job) = mine.as_ref().filter(|j| j.running()) {
        let p = job.snapshot();
        let fraction = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
        widgets::progress_bar(ui, theme, fraction, Some(&tf("ai.download.downloading", &[("file", &cat.engine.file), ("done", &gb_bytes(p.done)), ("total", &gb_bytes(p.total))])));
        return;
    }
    if let Some(Stage::Failed(f)) = mine.map(|j| j.snapshot().stage) {
        line(ui, theme, theme.warning(), t_dyn(f.words_key()));
    }
    if Button::primary(t("ai.engine_update.get")).hint("ai.engine_update.get_hint").disabled(busy).show(ui, theme) {
        let job = install::new_job(cat.engine.bytes);
        with_state(|s| {
            s.job = Some((ENGINE_JOB.to_string(), job.clone()));
            s.message = None;
        });
        let (cat, root) = (cat.clone(), root.to_path_buf());
        shared::client().shutdown_now();
        let spawned = std::thread::Builder::new().name("ai-engine-update".into()).spawn(move || match install::update_engine(&cat.base_url, &cat, &root, &job) {
            Ok(()) => log::info!("[AI] the AI engine {} is installed", cat.engine.version),
            Err(f) => log::warn!("[AI] the AI engine update did not install: {f:?}"),
        });
        if let Err(e) = spawned {
            log::warn!("[AI] could not start the engine update: {e}");
        }
    }
}

/// The download job's id while the engine updates (a model's job carries the model's id).
const ENGINE_JOB: &str = "engine";

/// A download that finished since the last frame: forget the old checks, so the new file is checked.
fn finish_job_if_done() {
    with_state(|s| {
        if let Some((_, job)) = &s.job {
            if job.snapshot().stage == Stage::Done {
                models::forget_checks();
            }
        }
    });
}

/// "Installed on this computer: the AI engine 1.0.0 (0.01 GB), Granite 4.0 1B (0.95 GB)."
fn draw_installed_summary(ui: &mut egui::Ui, theme: &Theme, cat: &AiCatalog, have: &Installed) {
    let mut parts = Vec::new();
    if let Some(bytes) = have.engine {
        parts.push(fill(t_marked("ai.installed_engine"), &[("version", &cat.engine.version), ("size", &gb_bytes(bytes))]));
    }
    for m in &cat.models {
        if let Some(bytes) = have.models.get(&m.id) {
            parts.push(fill(t_marked("ai.installed_model"), &[("model", &m.model), ("size", &gb_bytes(*bytes))]));
        }
    }
    let marked = fill(t_marked("ai.installed_summary"), &[("list", &parts.join(", "))]);
    widgets::glossary_words(ui, theme, &marked, theme.font_size_body, theme.text_primary());
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

/// Windows' code for a program that could not start because a library it needs is missing
/// (STATUS_DLL_NOT_FOUND, 0xC0000135, as a signed exit code): for the engine, the Visual C++ runtime.
const STATUS_DLL_NOT_FOUND: i32 = 0xC000_0135_u32 as i32;

/// Why the engine stopped, in words.
pub(crate) fn why_words(why: &ExitWhy) -> String {
    match why {
        ExitWhy::Code(STATUS_DLL_NOT_FOUND) => t("ai.exit.no_runtime").to_string(),
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

/// One choice's card: its name, its model and maker, what it needs, what this computer has, what works
/// instead when it does not fit; and, when the catalog can download it, everything about the download.
#[allow(clippy::too_many_arguments)]
fn draw_choice(ui: &mut egui::Ui, theme: &Theme, all: &choices::AiChoices, choice: &choices::Choice, hw: &Hardware, offer: Option<(Arc<AiCatalog>, ModelEntry)>, root: Option<&Path>, have: &Installed) {
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
        ui.add_space(theme.spacing_xs);
        match offer {
            Some((cat, model)) => draw_download(ui, theme, &cat, &model, root, have),
            None => muted(ui, theme, t("ai.card.not_yet")),
        }
    });
}

/// The download part of a card: installed (size, Remove), or the size, source and licence before
/// anything downloads, then Get, the progress and Stop, and a failure in words. With no folder chosen
/// for the person's files yet, everything shows but Get waits for that choice.
fn draw_download(ui: &mut egui::Ui, theme: &Theme, cat: &Arc<AiCatalog>, model: &ModelEntry, root: Option<&Path>, have: &Installed) {
    let Some(root) = root else {
        let marked = fill(t_marked("ai.card.download_size_engine"), &[("size", &gb_bytes(cat.engine.bytes + model.download_bytes())), ("engine", &gb_bytes(cat.engine.bytes))]);
        widgets::glossary_words(ui, theme, &marked, theme.font_size_small, theme.text_primary());
        muted(ui, theme, tf("ai.card.source", &[("maker", &model.maker)]));
        draw_licence(ui, theme, model);
        muted(ui, theme, t("ai.card.no_folder"));
        return;
    };
    if let Some(bytes) = have.models.get(&model.id) {
        line(ui, theme, theme.success(), tf("ai.card.installed", &[("size", &gb_bytes(*bytes))]));
        draw_licence(ui, theme, model);
        if Button::secondary(t("ai.card.remove")).hint("ai.card.remove_hint").show(ui, theme) {
            remove_one(cat, model, root);
        }
        return;
    }
    let total = install::download_total(cat, model, root);
    if have.engine.is_none() {
        let marked = fill(t_marked("ai.card.download_size_engine"), &[("size", &gb_bytes(total)), ("engine", &gb_bytes(cat.engine.bytes))]);
        widgets::glossary_words(ui, theme, &marked, theme.font_size_small, theme.text_primary());
    } else {
        line(ui, theme, theme.text_primary(), tf("ai.card.download_size", &[("size", &gb_bytes(total))]));
    }
    muted(ui, theme, tf("ai.card.source", &[("maker", &model.maker)]));
    draw_licence(ui, theme, model);
    let waiting = partial_bytes(root, model);
    if waiting > 0 {
        muted(ui, theme, tf("ai.card.partial", &[("size", &gb_bytes(waiting))]));
    }
    let (mine, busy) = with_state(|s| match &s.job {
        Some((id, job)) if *id == model.id => (Some(job.clone()), false),
        Some((_, job)) => (None, job.running()),
        None => (None, false),
    });
    match mine {
        Some(job) if job.running() => {
            let p = job.snapshot();
            let fraction = if p.total > 0 { p.done as f32 / p.total as f32 } else { 0.0 };
            let words = match &p.stage {
                Stage::Connecting => t("ai.download.connecting").to_string(),
                Stage::Downloading { file, .. } => tf("ai.download.downloading", &[("file", file), ("done", &gb_bytes(p.done)), ("total", &gb_bytes(p.total))]),
                Stage::Checking { file } => tf("ai.download.checking", &[("file", file)]),
                Stage::Joining { file } => tf("ai.download.joining", &[("file", file)]),
                Stage::Installing => t("ai.download.installing").to_string(),
                Stage::Done | Stage::Failed(_) => String::new(),
            };
            widgets::progress_bar(ui, theme, fraction, Some(&words));
            if let Stage::Downloading { resumed_from, .. } = p.stage {
                if resumed_from > 0 {
                    muted(ui, theme, tf("ai.download.resumed", &[("from", &gb_bytes(resumed_from))]));
                }
            }
            if Button::secondary(t("ai.download.stop")).hint("ai.download.stop_hint").show(ui, theme) {
                job.stop.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        other => {
            if let Some(Stage::Failed(f)) = other.as_ref().map(|j| j.snapshot().stage) {
                line(ui, theme, theme.warning(), t_dyn(f.words_key()));
                if !f.detail().is_empty() {
                    muted(ui, theme, tf("ai.download.detail", &[("detail", &f.detail())]));
                }
            }
            if busy {
                muted(ui, theme, t("ai.card.busy"));
            }
            if Button::primary(t("ai.card.get")).hint("ai.card.get_hint").disabled(busy).show(ui, theme) {
                start(cat.clone(), model.id.clone(), root.to_path_buf(), total);
            }
        }
    }
}

/// The licence line, and its full text on the card when opened (5.2: "its licence (a link that opens
/// the full text)"), from the copy that ships in data/ (the same text travels with the model).
fn draw_licence(ui: &mut egui::Ui, theme: &Theme, model: &ModelEntry) {
    line(ui, theme, theme.text_secondary(), tf("ai.card.licence", &[("licence", &t_dyn(&model.licence))]));
    let open = with_state(|s| s.licence_open.contains(&model.id));
    let (label, hint) = if open { (t("ai.card.hide_licence"), "ai.card.hide_licence_hint") } else { (t("ai.card.read_licence"), "ai.card.read_licence_hint") };
    if Button::ghost(label).hint(hint).show(ui, theme) {
        with_state(|s| {
            if !s.licence_open.remove(&model.id) {
                s.licence_open.insert(model.id.clone());
            }
        });
    }
    if open {
        match std::fs::read_to_string(humanity_core::storage::data_dir().join(&model.licence_text)) {
            Ok(text) => {
                egui::ScrollArea::vertical().id_salt(("ai_licence", &model.id)).max_height(220.0).show(ui, |ui| {
                    ui.label(RichText::new(text).monospace().size(theme.font_size_small).color(theme.text_secondary()));
                });
            }
            Err(e) => line(ui, theme, theme.warning(), tf("ai.card.licence_missing", &[("detail", &e)])),
        }
    }
}

/// Bytes of a half-finished download of this model waiting in downloads/.
fn partial_bytes(root: &Path, model: &ModelEntry) -> u64 {
    let d = install::downloads_dir(root);
    let names = std::iter::once(model.file.clone()).chain(model.parts.iter().map(|p| p.file.clone()));
    names.map(|n| std::fs::metadata(d.join(format!("{n}.part"))).map(|m| m.len()).unwrap_or(0)).sum()
}

/// Start a download on its own thread.
fn start(cat: Arc<AiCatalog>, id: String, root: std::path::PathBuf, total: u64) {
    let job = install::new_job(total);
    with_state(|s| {
        s.job = Some((id.clone(), job.clone()));
        s.message = None;
    });
    let base = cat.base_url.clone();
    log::info!("[AI] downloading {id} from {base}");
    let spawned = std::thread::Builder::new().name("ai-download".into()).spawn(move || match install::install_model(&base, &cat, &id, &root, &job) {
        Ok(()) => log::info!("[AI] {id} is installed"),
        Err(Failure::Stopped) => log::info!("[AI] the download of {id} was stopped; what arrived is kept"),
        Err(f) => log::warn!("[AI] {id} did not install: {f:?}"),
    });
    if let Err(e) = spawned {
        log::warn!("[AI] could not start the download: {e}");
    }
}

/// Remove one model (and the engine with the last one): the engine stopped first, so its files close.
fn remove_one(cat: &AiCatalog, model: &ModelEntry, root: &Path) {
    shared::client().shutdown_now();
    models::forget_checks();
    let msg = match install::remove_model(cat, root, &model.id) {
        Ok(_) => (t("ai.card.removed").to_string(), false),
        Err(e) => (tf("ai.card.remove_failed", &[("detail", &e)]), true),
    };
    with_state(|s| {
        s.message = Some(msg);
        s.job = None;
    });
}

/// Remove all AI: first the list of exactly what goes, with sizes, and "This cannot be undone"; then
/// the whole `ai` folder.
fn draw_remove_all(ui: &mut egui::Ui, theme: &Theme, cat: &AiCatalog, root: &Path, have: &Installed) {
    muted(ui, theme, t("ai.remove_all.intro"));
    let confirming = with_state(|s| s.confirm_remove_all);
    if !confirming {
        let downloading = with_state(|s| s.job.as_ref().is_some_and(|(_, j)| j.running()));
        if Button::danger(t("ai.remove_all.button")).hint("ai.remove_all.hint").disabled(downloading).show(ui, theme) {
            with_state(|s| s.confirm_remove_all = true);
        }
        return;
    }
    widgets::card(ui, theme, |ui| {
        line(ui, theme, theme.text_primary(), t("ai.remove_all.confirm_title"));
        for m in &cat.models {
            if let Some(bytes) = have.models.get(&m.id) {
                line(ui, theme, theme.text_secondary(), tf("ai.remove_all.item", &[("name", &m.model), ("size", &gb_bytes(*bytes))]));
            }
        }
        if let Some(bytes) = have.engine {
            line(ui, theme, theme.text_secondary(), tf("ai.remove_all.engine_item", &[("version", &cat.engine.version), ("size", &gb_bytes(bytes))]));
        }
        if have.partial > 0 {
            line(ui, theme, theme.text_secondary(), tf("ai.remove_all.partial_item", &[("size", &gb_bytes(have.partial))]));
        }
        muted(ui, theme, tf("ai.remove_all.folder", &[("folder", &root.display())]));
        line(ui, theme, theme.warning(), t("ai.remove_all.warning"));
        ui.horizontal_wrapped(|ui| {
            if Button::danger(t("ai.remove_all.yes")).hint("ai.remove_all.yes_hint").show(ui, theme) {
                shared::client().shutdown_now();
                models::forget_checks();
                let msg = match install::remove_all(root) {
                    Ok(()) => (t("ai.remove_all.done").to_string(), false),
                    Err(e) => (tf("ai.remove_all.failed", &[("detail", &e)]), true),
                };
                with_state(|s| {
                    s.message = Some(msg);
                    s.confirm_remove_all = false;
                    s.job = None;
                });
            }
            if Button::secondary(t("ai.remove_all.cancel")).hint("ai.remove_all.cancel_hint").show(ui, theme) {
                with_state(|s| s.confirm_remove_all = false);
            }
        });
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

/// For the snapshots: open a card's licence text, as the person's click would.
#[cfg(test)]
pub(crate) fn open_licence_for_snapshot(id: &str) {
    with_state(|s| {
        s.licence_open.insert(id.to_string());
    });
}

/// For the snapshots: Remove all AI's confirmation open, as the person's first click leaves it.
#[cfg(test)]
pub(crate) fn confirm_remove_all_for_snapshot() {
    with_state(|s| s.confirm_remove_all = true);
}

/// For the snapshots: close everything again.
#[cfg(test)]
pub(crate) fn reset_for_snapshot() {
    *STATE.lock().unwrap_or_else(|p| p.into_inner()) = None;
}
