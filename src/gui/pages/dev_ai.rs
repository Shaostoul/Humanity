//! Dev > AI (2026-11-09; docs/design/embeddable-ai.md, 12.1; rung 1): the AI engine as a developer
//! sees it. Permanent dev tooling (CLAUDE.md, "Forever development"): never trimmed as cleanup.
//!
//! What rung 1 can fill of 12.1's section:
//! - **which engine**: the installed one, or the developer's fake one (`just ai-engine` builds it into
//!   target/ai-engine/, or `HUMANITY_AI_ENGINE` names one), for this run;
//! - **the engine's state** (running, its process, its version, fake or not, its job object, its
//!   priority, its memory against its cap), and its counters, timing and last refusal;
//! - **buttons**: Start, Stop, Stop as a crash (to see the fallback and the two-failure rule), Try
//!   again, Send a test question (it starts the engine on demand, as a feature will), Ask for status;
//! - **the reuse switch's effect**, live: with reuse on, the fake reads a persona once and the second
//!   reply's prefill is short; off, every prefill is the full time;
//! - **"pretend this PC has {n} GB free"**, so Settings > AI shows every memory notice;
//! - **this session's last questions and answers**, kept in memory only and gone when the app closes.
//!
//! Loaded models with their hashes, the voice's real-time factor and the filter's counts come with the
//! rungs that bring them (2, 5, 3).

use std::collections::VecDeque;
use std::sync::Mutex;

use egui::RichText;
use humanity_core::i18n::{t, tf};
use humanity_ai_protocol::{EngineMessage, ModelKind};

use crate::ai::client::{Outcome, State};
use crate::ai::{hardware, paths, shared};
use crate::gui::theme::Theme;
use crate::gui::widgets::{self, with_hint, Button};
use crate::gui::GuiState;

/// The persona the test question goes with (the fake ignores its words; a real brain would read it).
const TEST_PERSONA: &str = "You are a crew member in HumanityOS, run by AI on the player's computer. Answer in one or two sentences."; // i18n-exempt: a prompt for the engine, never shown as the app's words

/// What this part keeps for the run: the pretend figure, and the last few exchanges.
struct DevAi {
    pretend_on: bool,
    pretend_gb: f32,
    /// (request, question, answer or why it fell back), newest last.
    log: VecDeque<(u64, String, String)>,
}

static DEV: Mutex<DevAi> = Mutex::new(DevAi { pretend_on: false, pretend_gb: 2.0, log: VecDeque::new() });

/// How many exchanges the part keeps.
const LOG_LEN: usize = 6;

fn row(ui: &mut egui::Ui, theme: &Theme, label: &str, value: impl Into<String>) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(label).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(value.into()).size(theme.font_size_small).color(theme.text_primary()));
    });
}

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(theme.bg_panel()).inner_margin(16.0)).show(ctx, |ui| {
        egui::ScrollArea::vertical().id_salt("dev_ai_scroll").show(ui, |ui| {
            ui.label(RichText::new(t("dev.ai.title")).size(theme.font_size_title).color(theme.text_primary()));
            widgets::setting_hint(ui, theme, state.settings.hint_display, t("dev.ai.about"));
            ui.add_space(theme.spacing_sm);
            draw_engine(ui, theme, state);
            ui.add_space(theme.spacing_sm);
            draw_pretend(ui, theme, state);
            ui.add_space(theme.spacing_sm);
            draw_log(ui, theme);
        });
    });
}

fn draw_engine(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev.ai.engine")).size(theme.font_size_body).strong().color(theme.text_primary()));
        let mut fake = shared::use_dev_fake();
        if with_hint(ui, "dev.ai.use_fake_hint", |ui| widgets::toggle(ui, theme, t("dev.ai.use_fake"), &mut fake)) {
            shared::set_use_dev_fake(fake);
        }
        widgets::setting_hint(ui, theme, hint, t("dev.ai.use_fake_hint"));
        let mut c = shared::client();
        c.poll();
        // Requests that ended, into this run's log.
        let outcomes = c.take_outcomes();
        if !outcomes.is_empty() {
            let mut dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
            for o in outcomes {
                let words = match &o {
                    Outcome::Answered { text, .. } => text.clone(),
                    Outcome::Fallback { why, .. } => tf("dev.ai.fell_back", &[("why", &humanity_core::i18n::t_dyn(why.words_key()))]),
                };
                if let Some(entry) = dev.log.iter_mut().find(|(r, _, _)| *r == o.request()) {
                    entry.2 = words;
                }
            }
        }
        match c.engine_path() {
            Some(p) => row(ui, theme, t("dev.ai.path"), p.display().to_string()),
            None if fake => row(ui, theme, t("dev.ai.path"), t("dev.ai.no_fake").to_string()),
            None => row(ui, theme, t("dev.ai.path"), t("ai.state.not_found").to_string()),
        }
        let words = match c.state() {
            State::Running { since, hello } => tf(
                "dev.ai.running",
                &[
                    ("pid", &c.pid().map_or("?".to_string(), |p| p.to_string())),
                    ("version", &hello.version),
                    ("mode", &if hello.fake { t("dev.ai.mode_fake") } else { t("dev.ai.mode_real") }),
                    ("secs", &since.elapsed().as_secs()),
                ],
            ),
            State::Crashed { why } => format!("{} {}", t("ai.state.crashed"), super::settings::ai::why_words(why)),
            State::Halted { why } => format!("{} {}", t("ai.state.halted"), super::settings::ai::why_words(why)),
            State::Refused { error } => {
                let fill = error.fill();
                let vars: Vec<(&str, &dyn std::fmt::Display)> = fill.iter().map(|(k, v)| (*k, v as &dyn std::fmt::Display)).collect();
                tf(error.words_key(), &vars)
            }
            s => humanity_core::i18n::t_dyn(s.words_key()).to_string(),
        };
        let color = match c.state() {
            State::Running { .. } => theme.success(),
            State::Crashed { .. } | State::Halted { .. } | State::Refused { .. } => theme.warning(),
            _ => theme.text_primary(),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(t("dev.ai.state")).size(theme.font_size_small).color(theme.text_secondary()));
            ui.label(RichText::new(words).size(theme.font_size_small).color(color));
        });
        if let State::Running { hello, .. } = c.state() {
            let backends = if hello.backends.is_empty() { t("dev.ai.no_backends").to_string() } else { hello.backends.iter().map(|b| format!("{b:?}")).collect::<Vec<_>>().join(", ") };
            row(ui, theme, t("dev.ai.backends"), backends);
            row(ui, theme, t("dev.ai.cpu_features"), hello.cpu_features.join(", "));
        }
        match c.containment() {
            Some((in_job, priority)) => {
                let cap = c.config.memory_cap_mb;
                row(ui, theme, t("dev.ai.job"), if in_job { tf("dev.ai.in_job", &[("cap", &cap)]) } else { t("dev.ai.not_in_job").to_string() });
                let below = priority == crate::ai::job::BELOW_NORMAL;
                row(ui, theme, t("dev.ai.priority"), if below { t("dev.ai.below_normal").to_string() } else { tf("dev.ai.priority_other", &[("class", &priority)]) });
                let (now, peak) = c.memory_mb();
                let mb = |v: Option<f64>| v.map_or("?".to_string(), |v| format!("{v:.1}"));
                row(ui, theme, t("dev.ai.memory"), tf("dev.ai.memory_value", &[("now", &mb(now)), ("peak", &mb(peak)), ("cap", &cap)]));
            }
            None if c.state().is_live() => row(ui, theme, t("dev.ai.job"), t("dev.ai.no_jobs_here").to_string()),
            None => {}
        }
        let k = &c.counters;
        row(
            ui,
            theme,
            t("dev.ai.counters"),
            tf("dev.ai.counters_value", &[("starts", &k.starts), ("requests", &k.requests), ("answered", &k.answered), ("tokens", &k.tokens), ("fallbacks", &k.fallbacks), ("crashes", &k.crashes), ("failures", &c.failures())]),
        );
        if let Some((tokens, prefill, generate)) = c.last_timing {
            let rate = if generate > 0 { tokens as f32 * 1000.0 / generate as f32 } else { 0.0 };
            row(ui, theme, t("dev.ai.timing"), tf("dev.ai.timing_value", &[("prefill", &prefill), ("generate", &generate), ("rate", &format!("{rate:.1}"))]));
        }
        if let Some((code, detail)) = &c.last_error {
            row(ui, theme, t("dev.ai.last_error"), format!("{} ({detail})", humanity_core::i18n::t_dyn(code.words_key())));
        }
        if let Some(EngineMessage::Status { counters, reuse, loaded, .. }) = &c.last_status {
            let models = loaded.iter().map(|m| format!("{} {} MB", m.id, m.memory_mb)).collect::<Vec<_>>().join(", ");
            row(ui, theme, t("dev.ai.status_label"), tf("dev.ai.status_value", &[("requests", &counters.requests), ("tokens", &counters.tokens), ("cancelled", &counters.cancelled), ("errors", &counters.errors), ("reuse", &if *reuse { t("dev.ai.word_on") } else { t("dev.ai.word_off") }), ("models", &models)]));
        }
        let reuse = humanity_core::reuse::enabled();
        ui.label(RichText::new(if reuse { t("dev.ai.reuse_on") } else { t("dev.ai.reuse_off") }).size(theme.font_size_small).color(theme.text_muted()));

        ui.add_space(theme.spacing_xs);
        let live = c.state().is_live();
        let mut started = false;
        ui.horizontal_wrapped(|ui| {
            if Button::primary(t("dev.ai.start")).hint("dev.ai.start_hint").disabled(live || c.engine_path().is_none()).show(ui, theme) {
                c.start();
                started = true;
            }
            if Button::secondary(t("dev.ai.ask")).hint("dev.ai.ask_hint").disabled(c.engine_path().is_none()).show(ui, theme) {
                ask_test_question(&mut c);
                started = true;
            }
            if Button::secondary(t("dev.ai.status")).hint("dev.ai.status_hint").disabled(!matches!(c.state(), State::Running { .. })).show(ui, theme) {
                c.ask_status();
            }
            if Button::secondary(t("dev.ai.stop")).hint("dev.ai.stop_hint").disabled(!live).show(ui, theme) {
                c.stop();
            }
            if Button::danger(t("dev.ai.kill")).hint("dev.ai.kill_hint").disabled(!live).show(ui, theme) {
                c.kill_as_crash();
            }
            if Button::ghost(t("ai.try_again")).hint("ai.try_again_hint").disabled(c.engine_path().is_none()).show(ui, theme) {
                c.try_again();
                started = true;
            }
        });
        // Streaming text of the newest exchange, word by word as it arrives.
        let mut dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
        for (r, _, answer) in dev.log.iter_mut() {
            if answer.is_empty() {
                if let Some(p) = c.partial(*r) {
                    *answer = format!("{p}...");
                }
            } else if answer.ends_with("...") {
                if let Some(p) = c.partial(*r) {
                    *answer = format!("{p}...");
                }
            }
        }
        drop(dev);
        if live || started {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
        }
        drop(c);
        if started {
            shared::watch();
        }
    });
}

/// Send a test question (the button's work, and the snapshot's): load the fake's model if it is not
/// loaded, ask, and note the exchange in this run's log. A stopped engine is started by the request.
pub(crate) fn ask_test_question(c: &mut crate::ai::client::AiClient) -> u64 {
    if !c.is_loaded("fake") {
        c.load("fake", "(fake)", "", ModelKind::Brain);
    }
    let question = t("dev.ai.test_question").to_string();
    let r = c.generate("fake", TEST_PERSONA, &question, 64);
    let mut dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
    dev.log.push_back((r, question, String::new()));
    while dev.log.len() > LOG_LEN {
        dev.log.pop_front();
    }
    r
}

/// For the snapshot: the pretend figure as the Dev page shows it.
#[cfg(test)]
pub(crate) fn pretend_for_snapshot(gb: Option<f32>) {
    let mut dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
    dev.pretend_on = gb.is_some();
    dev.pretend_gb = gb.unwrap_or(2.0);
    hardware::set_pretend_free_mb(gb.map(|g| (g * 1024.0) as u64));
}

fn draw_pretend(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let hint = state.settings.hint_display;
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev.ai.pretend_title")).size(theme.font_size_body).strong().color(theme.text_primary()));
        let mut dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
        let mut on = dev.pretend_on;
        let mut changed = with_hint(ui, "dev.ai.pretend_hint", |ui| widgets::toggle(ui, theme, t("dev.ai.pretend"), &mut on));
        widgets::setting_hint(ui, theme, hint, t("dev.ai.pretend_hint"));
        let mut gb = dev.pretend_gb;
        changed |= with_hint(ui, "dev.ai.pretend_gb_hint", |ui| widgets::labeled_slider(ui, theme, t("dev.ai.pretend_gb"), &mut gb, 0.5..=16.0));
        dev.pretend_on = on;
        dev.pretend_gb = gb;
        if changed {
            hardware::set_pretend_free_mb(on.then_some((gb * 1024.0) as u64));
        }
        let hw = hardware::read();
        let words = match (hw.memory_free_mb, hw.memory_total_mb) {
            (Some(free), Some(total)) => tf("ai.hw.memory", &[("free", &hardware::gb(free)), ("total", &hardware::gb(total))]),
            _ => t("ai.hw.memory_unknown").to_string(),
        };
        ui.label(RichText::new(words).size(theme.font_size_small).color(if hw.pretending { theme.warning() } else { theme.text_secondary() }));
        ui.label(RichText::new(tf("dev.ai.installed_at", &[("path", &paths::ai_root().map_or_else(|| t("ai.hw.disk_unknown").to_string(), |p| paths::engine_path_in(&p).display().to_string()))])).size(theme.font_size_small).color(theme.text_muted()));
    });
}

fn draw_log(ui: &mut egui::Ui, theme: &Theme) {
    widgets::card(ui, theme, |ui| {
        ui.label(RichText::new(t("dev.ai.log_title")).size(theme.font_size_body).strong().color(theme.text_primary()));
        let dev = DEV.lock().unwrap_or_else(|p| p.into_inner());
        if dev.log.is_empty() {
            ui.label(RichText::new(t("dev.ai.log_empty")).size(theme.font_size_small).color(theme.text_muted()));
        }
        for (_, q, a) in dev.log.iter().rev() {
            row(ui, theme, t("dev.ai.asked"), q.clone());
            row(ui, theme, t("dev.ai.answer"), if a.is_empty() { t("dev.ai.waiting").to_string() } else { a.clone() });
            ui.add_space(theme.spacing_xs);
        }
    });
}
