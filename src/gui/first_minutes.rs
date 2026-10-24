//! The first five minutes (the ELI5 mandate, docs/PRIORITIES.md TIER 3,
//! 2026-10-22): a short guided tour for a new person, on the desktop app and the
//! website alike, in plain words. It walks the three things that matter first:
//! the recovery phrase written down on paper, a first message, and the first
//! steps in the world; then where help lives (the hints, the underlined words).
//!
//! It is SKIPPABLE (Skip the tour, any time) and RESUMABLE (Hide for now keeps
//! the place; Settings > Appearance > "Continue the tour" brings it back, and it
//! comes back by itself after a restart while unfinished). A step that the app
//! can see is done (the words written down, a message sent, the world entered)
//! ticks itself; the others move on with Next.
//!
//! The steps are data, shared with the website: `data/onboarding/tour.json`
//! (each step's title and words are keys in `data/i18n/`). The website's twin
//! is `web/shared/onboarding-tour.js`. It builds on the first-run setup in
//! `pages/main_menu.rs` (name, server, identity and the 24 words), which starts
//! it when it finishes.

use egui::{Align2, RichText, Vec2};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::gui::i18n::{t, t_dyn, t_marked_dyn, tf};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::{GuiPage, GuiState, SettingsCategory};

/// Where the person is in the tour. Kept in `config.json` (`first_minutes`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FirstMinutes {
    /// Started (the first-run setup finished, or the person asked for it).
    pub started: bool,
    /// The step showing, an index into the tour's steps.
    pub step: usize,
    /// Hidden for now: the card is away until "Continue the tour".
    pub hidden: bool,
    /// Finished or skipped: the card never comes back by itself.
    pub done: bool,
    /// The person said the 24 words are on paper (the first-run "I wrote them
    /// down", or the tour's own button). A restore from the words counts too.
    pub phrase_written: bool,
}

/// One step of the tour, as `data/onboarding/tour.json` writes it.
#[derive(Debug, Clone, Deserialize)]
pub struct Step {
    pub id: String,
    /// A key in `data/i18n/` for the step's heading.
    pub title: String,
    /// A key in `data/i18n/` for its words (glossary marks allowed).
    pub text: String,
    /// A fact the app can see that finishes the step by itself.
    #[serde(default)]
    pub done_when: Option<String>,
    /// Where "Take me there" goes on the desktop app.
    #[serde(default)]
    pub native: Option<Place>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Place {
    /// A page id (`dispatch::page_id`), "none" for the world.
    #[serde(default)]
    pub page: Option<String>,
    /// A Settings section ("account", "appearance").
    #[serde(default)]
    pub settings: Option<String>,
}

#[derive(Deserialize)]
struct TourFile {
    steps: Vec<Step>,
}

static STEPS: OnceLock<Vec<Step>> = OnceLock::new();

/// The tour's steps: the file beside the exe (hot-editable), else the copy built
/// into the exe, noted in the log as every data loader notes it.
pub fn steps() -> &'static [Step] {
    STEPS.get_or_init(|| {
        let parse = |s: &str| serde_json::from_str::<TourFile>(s).map(|f| f.steps);
        let path = crate::data_dir().join("onboarding").join("tour.json");
        let disk = match std::fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(steps) => Some(steps),
                Err(e) => {
                    crate::embedded_data::note_builtin_copy(
                        "onboarding/tour.json",
                        format_args!("{} does not parse ({e})", path.display()),
                    );
                    None
                }
            },
            Err(e) => {
                crate::embedded_data::note_builtin_copy(
                    "onboarding/tour.json",
                    format_args!("{} could not be read ({e})", path.display()),
                );
                None
            }
        };
        disk.or_else(|| parse(crate::embedded_data::ONBOARDING_TOUR_JSON).ok()).unwrap_or_default()
    })
}

impl FirstMinutes {
    /// Start the tour from its first step (the first-run setup finished, or the
    /// person pressed "Start the tour again").
    pub fn start(&mut self) {
        self.started = true;
        self.step = 0;
        self.hidden = false;
        self.done = false;
    }
    /// Bring a hidden tour back where it was ("Continue the tour").
    pub fn resume(&mut self) {
        if !self.started || self.done {
            self.start();
        } else {
            self.hidden = false;
        }
    }
    /// Is the card up?
    pub fn showing(&self) -> bool {
        self.started && !self.hidden && !self.done
    }
}

/// Has the app seen this step done?
pub fn step_done(state: &GuiState, step: &Step) -> bool {
    match step.done_when.as_deref() {
        Some("phrase_written") => state.first_minutes.phrase_written,
        Some("message_sent") => {
            !state.profile_public_key.is_empty()
                && state.chat_messages.iter().any(|m| m.sender_key == state.profile_public_key)
        }
        Some("entered_world") => state.controls_hint_shown,
        _ => false,
    }
}

/// Go where a step points ("Take me there").
fn go(state: &mut GuiState, place: &Place) {
    if let Some(section) = place.settings.as_deref() {
        state.settings.category = match section {
            "appearance" => SettingsCategory::Appearance,
            "privacy" => SettingsCategory::Privacy,
            _ => SettingsCategory::Account,
        };
        state.settings.scroll_to_section = Some(state.settings.category);
    }
    if let Some(page) = place.page.as_deref() {
        if let Some(p) = crate::gui::dispatch::page_from_id(page) {
            state.active_page = p;
        }
    }
}

/// Draw the tour card, bottom right, over whatever page is open. Not over the
/// 3D world (the mouse is steering there; the controls note covers it), not
/// before the first-run setup has finished.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if !state.onboarding_complete || !state.first_minutes.showing() || state.active_page == GuiPage::None {
        return;
    }
    let all = steps();
    if all.is_empty() {
        return;
    }
    if state.first_minutes.step >= all.len() {
        state.first_minutes.done = true;
        return;
    }
    let idx = state.first_minutes.step;
    let step = all[idx].clone();
    let done = step_done(state, &step);
    let mut changed = false;
    egui::Window::new("first_minutes_card")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::RIGHT_BOTTOM, Vec2::new(-16.0, -16.0))
        .fixed_size(Vec2::new(340.0, 0.0))
        .frame(egui::Frame::window(&ctx.style()).fill(theme.bg_card()))
        .show(ctx, |ui| {
            ui.label(
                RichText::new(tf("tour.counter", &[("n", &(idx + 1)), ("total", &all.len())]))
                    .size(theme.font_size_small)
                    .color(theme.text_muted()),
            );
            ui.label(RichText::new(t_dyn(&step.title)).size(theme.font_size_heading).color(theme.accent()));
            ui.add_space(theme.spacing_xs);
            widgets::glossary_words(
                ui,
                theme,
                t_marked_dyn(&step.text),
                theme.font_size_body,
                theme.text_primary(),
            );
            if done {
                ui.add_space(theme.spacing_xs);
                ui.label(RichText::new(t("tour.step_done")).color(theme.success()));
            }
            ui.add_space(theme.spacing_sm);
            ui.horizontal_wrapped(|ui| {
                if let Some(place) = step.native.as_ref() {
                    if widgets::Button::secondary(t("tour.take_me_there")).hint("tour.take_me_there_hint").show(ui, theme) {
                        go(state, place);
                    }
                }
                if step.done_when.as_deref() == Some("phrase_written") && !done
                    && widgets::Button::secondary(t("tour.phrase_written")).hint("tour.phrase_written_hint").show(ui, theme)
                {
                    state.first_minutes.phrase_written = true;
                    changed = true;
                }
            });
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| {
                if idx > 0 && widgets::Button::ghost(t("tour.back")).hint("tour.back_hint").show(ui, theme) {
                    state.first_minutes.step -= 1;
                    changed = true;
                }
                let last = idx + 1 == all.len();
                let next_label = if last { t("tour.finish") } else { t("tour.next") };
                if widgets::Button::primary(next_label).hint(if last { "tour.finish_hint" } else { "tour.next_hint" }).show(ui, theme) {
                    if last {
                        state.first_minutes.done = true;
                    } else {
                        state.first_minutes.step += 1;
                    }
                    changed = true;
                }
            });
            ui.horizontal(|ui| {
                if widgets::Button::ghost(t("tour.hide")).hint("tour.hide_hint").show(ui, theme) {
                    state.first_minutes.hidden = true;
                    changed = true;
                }
                if widgets::Button::ghost(t("tour.skip")).hint("tour.skip_hint").show(ui, theme) {
                    state.first_minutes.done = true;
                    changed = true;
                }
            });
        });
    if changed {
        crate::config::AppConfig::from_gui_state(state).save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped steps parse, and every word they name is a key English has, so a
    /// step never shows its key instead of its words (the website reads the same file).
    #[test]
    fn the_shipped_tour_parses_and_every_word_is_a_key() {
        let file: TourFile =
            serde_json::from_str(crate::embedded_data::ONBOARDING_TOUR_JSON).expect("tour.json parses");
        assert!(file.steps.len() >= 4, "the tour has its steps");
        let ids: Vec<&str> = file.steps.iter().map(|s| s.id.as_str()).collect();
        for want in ["phrase", "message", "world"] {
            assert!(ids.contains(&want), "the {want} step is there: {ids:?}");
        }
        for step in &file.steps {
            assert!(crate::gui::i18n::has(&step.title), "{} title key {}", step.id, step.title);
            assert!(crate::gui::i18n::has(&step.text), "{} text key {}", step.id, step.text);
            if let Some(page) = step.native.as_ref().and_then(|p| p.page.as_deref()) {
                assert!(
                    crate::gui::dispatch::page_from_id(page).is_some(),
                    "{} goes to a page that exists: {page}",
                    step.id
                );
            }
        }
    }

    /// Skippable and resumable: Hide keeps the place, Continue brings it back there,
    /// Skip ends it, and taking it again after the end starts from the top.
    #[test]
    fn hide_keeps_the_place_and_skip_ends_it() {
        let mut fm = FirstMinutes::default();
        assert!(!fm.showing(), "nothing shows before the first-run setup starts it");
        fm.start();
        assert!(fm.showing());
        fm.step = 2;
        fm.hidden = true;
        assert!(!fm.showing());
        fm.resume();
        assert!(fm.showing() && fm.step == 2, "Continue picks up where it was");
        fm.done = true;
        assert!(!fm.showing(), "Skip ends it");
        fm.resume();
        assert!(fm.showing() && fm.step == 0, "taking it again after the end starts at the top");
    }

    /// A config written before the tour existed has no first_minutes: it reads as
    /// not started, so a returning person is not interrupted.
    #[test]
    fn an_old_config_reads_as_not_started() {
        let fm: FirstMinutes = serde_json::from_str("{}").expect("an empty object reads");
        assert_eq!(fm, FirstMinutes::default());
        assert!(!fm.showing());
    }
}
