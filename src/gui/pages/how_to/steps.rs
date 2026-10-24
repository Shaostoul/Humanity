//! The steps of a how-to, as a checklist: each step numbered, the current one marked by a painted
//! arrow and its number (never by colour alone), its safety note at the step in both views, and
//! the control its kind needs: a tick, a timer, Yes and No, a value to write down, the options,
//! a button that opens a page, a command with Copy. Ticking anything starts a run if none is open,
//! and every change is written to disk by the caller straight after.

use egui::{RichText, Ui};

use super::{hazard_colour, hazard_word, w, wf, words};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::icons;
use crate::gui::GuiState;
use crate::how_to::library::{Library, Loaded};
use crate::how_to::model::{Kind, Step};
use crate::how_to::runs::Run;
use crate::how_to::view::{self, View};

/// What drawing the steps asks the page to do afterwards.
#[derive(Default)]
pub struct After {
    /// The run changed: write it.
    pub changed: bool,
    /// A go step was pressed: page and section.
    pub go: Option<(String, Option<String>)>,
    /// A call step was pressed: the how-to to open.
    pub open: Option<String>,
}

fn ensure<'a>(run: &'a mut Option<Run>, l: &Loaded, now: u64) -> &'a mut Run {
    run.get_or_insert_with(|| Run::start(&l.how_to, &l.hash, now))
}

/// The value this how-to's measure step had in the person's last finished run, for "Last time".
fn last_value(state: &GuiState, how_to: &str, step: &str) -> Option<String> {
    state.how_to.runs.iter().filter(|r| r.how_to == how_to && !r.is_open()).find_map(|r| r.steps.get(step).and_then(|s| s.value.clone()))
}

pub fn draw(ui: &mut Ui, theme: &Theme, state: &mut GuiState, lib: &Library, l: &Loaded, run: &mut Option<Run>, v: View) -> After {
    let mut after = After::default();
    let now = crate::persistence::now_ms();
    let h = &l.how_to;
    let visible = view::visible_steps(h, run.as_ref());
    let current = run.as_ref().and_then(|r| view::current_step(h, r)).map(|s| s.id.clone());
    for (i, s) in visible.iter().enumerate() {
        if v == View::Short && !s.in_short() {
            continue;
        }
        let done = run.as_ref().map(|r| r.is_done(&s.id)).unwrap_or(false);
        let is_current = current.as_deref() == Some(s.id.as_str());
        ui.add_space(theme.spacing_xs);
        ui.horizontal(|ui| {
            // The marker: an arrow at the current step, room for one at the others.
            let size = theme.font_size_body;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
            if is_current {
                icons::paint_arrow_right(ui.painter(), rect, theme.accent());
            }
            let mut ticked = done;
            let tickable = !matches!(s.kind, Kind::Check | Kind::Choose | Kind::Measure | Kind::Action);
            if tickable && widgets::custom_checkbox(ui, theme, &mut ticked) {
                ensure(run, l, now).set_done(&s.id, ticked, now);
                after.changed = true;
            }
            let number = format!("{}.", i + 1);
            ui.label(RichText::new(number).color(theme.text_muted()).strong());
            let colour = if done { theme.text_muted() } else { theme.text_primary() };
            let mut text = RichText::new(&s.text).color(colour);
            if is_current {
                text = text.strong();
            }
            ui.add(egui::Label::new(text).wrap());
        });
        ui.indent(("how_to_step", &s.id), |ui| {
            if let Some(place) = s.when.get("place") {
                let name = lib.place_names.get(place).cloned().unwrap_or_else(|| place.clone());
                ui.label(RichText::new(wf("only_in", &[("place", &name)])).color(theme.text_muted()).size(theme.font_size_small).italics());
            }
            controls(ui, theme, state, l, s, run, now, &mut after);
            if let Some(safety) = &s.safety {
                safety_note(ui, theme, safety, v);
            }
            if v == View::Full {
                for p in s.why.iter().chain(&s.detail) {
                    ui.label(RichText::new(p).color(theme.text_secondary()).size(theme.font_size_small));
                }
            }
        });
    }
    if let Some(r) = run.as_ref() {
        for id in view::removed_steps(h, r) {
            ui.label(RichText::new(wf("removed_step", &[("id", &id)])).color(theme.text_muted()).size(theme.font_size_small).italics());
        }
    }
    after
}

/// A safety note: its icon and word, its short line in both views, the rest in Full.
pub fn safety_note(ui: &mut Ui, theme: &Theme, safety: &crate::how_to::model::Safety, v: View) {
    let hazard = safety.level.hazard();
    let colour = hazard_colour(theme, hazard);
    egui::Frame::NONE
        .stroke(egui::Stroke::new(1.0, colour))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(egui::Margin::same(theme.spacing_sm as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| super::hazard_badge(ui, theme, hazard));
            if let Some(short) = &safety.short {
                ui.add(egui::Label::new(RichText::new(short).color(theme.text_primary()).strong()).wrap());
            } else {
                ui.label(RichText::new(hazard_word(hazard)).color(colour));
            }
            if v == View::Full {
                for p in &safety.full {
                    ui.label(RichText::new(p).color(theme.text_secondary()).size(theme.font_size_small));
                }
            }
        });
}

#[allow(clippy::too_many_arguments)]
fn controls(ui: &mut Ui, theme: &Theme, state: &mut GuiState, l: &Loaded, s: &Step, run: &mut Option<Run>, now: u64, after: &mut After) {
    match s.kind {
        Kind::Do => {}
        Kind::Wait => {
            // A wait already ticked done has no timer left to show.
            if run.as_ref().map(|r| r.is_done(&s.id)).unwrap_or(false) {
                return;
            }
            let left = run.as_ref().and_then(|r| r.wait_left_ms(&s.id, now));
            ui.horizontal(|ui| match left {
                None => {
                    if widgets::compact_button(ui, theme, w("wait_start"), widgets::ButtonVariant::Secondary) {
                        let secs = s.wait.as_ref().map(|wt| wt.min_s).unwrap_or(0);
                        ensure(run, l, now).start_wait(&s.id, secs, now);
                        after.changed = true;
                    }
                }
                Some(0) => {
                    ui.label(RichText::new(w("wait_up")).color(theme.success()).strong());
                }
                Some(ms) => {
                    ui.label(RichText::new(wf("wait_left", &[("time", &words::clock(ms))])).color(theme.accent()).strong().monospace());
                }
            });
        }
        Kind::Check => {
            let answer = run.as_ref().and_then(|r| r.steps.get(&s.id)).and_then(|st| st.answer.clone());
            ui.horizontal(|ui| {
                let yes = answer.as_deref() == Some("yes");
                let no = answer.as_deref() == Some("no");
                if widgets::Button::tab(w("yes"), yes).show(ui, theme) {
                    ensure(run, l, now).answer(&l.how_to, &s.id, true, now);
                    after.changed = true;
                }
                if widgets::Button::tab(w("no"), no).show(ui, theme) {
                    ensure(run, l, now).answer(&l.how_to, &s.id, false, now);
                    after.changed = true;
                }
                if no {
                    ui.label(RichText::new(w("answered_no")).color(theme.warning()).size(theme.font_size_small));
                }
            });
        }
        Kind::Measure => {
            let Some(rec) = &s.record else { return };
            let saved = run.as_ref().and_then(|r| r.steps.get(&s.id)).and_then(|st| st.value.clone());
            let typing = state.how_to.typing.entry(s.id.clone()).or_insert_with(|| saved.clone().unwrap_or_default());
            ui.horizontal(|ui| {
                ui.label(RichText::new(&rec.label).color(theme.text_secondary()).size(theme.font_size_small));
                ui.add(egui::TextEdit::singleline(typing).desired_width(160.0));
                if let Some(unit) = &rec.unit {
                    ui.label(RichText::new(unit).color(theme.text_muted()).size(theme.font_size_small));
                }
                if widgets::compact_button(ui, theme, w("record_save"), widgets::ButtonVariant::Secondary) {
                    let value = typing.clone();
                    ensure(run, l, now).record(&s.id, &value, now);
                    after.changed = true;
                }
            });
            if rec.remember {
                if let Some(last) = last_value(state, &l.how_to.id, &s.id) {
                    ui.label(RichText::new(wf("last_time", &[("value", &last)])).color(theme.text_muted()).size(theme.font_size_small));
                }
            }
        }
        Kind::Choose => {
            let chosen = run.as_ref().and_then(|r| r.choices.get(&s.id)).cloned();
            ui.horizontal_wrapped(|ui| {
                for o in &s.options {
                    if widgets::Button::tab(&o.label, chosen.as_deref() == Some(o.id.as_str())).show(ui, theme) {
                        ensure(run, l, now).choose(&s.id, &o.id, now);
                        after.changed = true;
                    }
                }
            });
        }
        Kind::Call => {
            if let Some(target) = &s.how_to {
                if widgets::compact_button(ui, theme, w("call_open"), widgets::ButtonVariant::Secondary) {
                    after.open = Some(target.clone());
                }
            }
        }
        Kind::Go => {
            if let Some(page) = &s.page {
                if widgets::compact_button(ui, theme, w("open"), widgets::ButtonVariant::Secondary) {
                    after.go = Some((page.clone(), s.section.clone()));
                }
            }
        }
        Kind::Action => {
            ui.label(RichText::new(w("action_later")).color(theme.text_muted()).size(theme.font_size_small));
        }
        Kind::Command => {
            if let Some(cmd) = &s.command {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(cmd).monospace().color(theme.text_primary()));
                    if widgets::compact_button(ui, theme, w("copy"), widgets::ButtonVariant::Secondary) {
                        ui.ctx().copy_text(cmd.clone());
                        let t = ui.input(|i| i.time);
                        state.toast(w("copied"), crate::gui::ToastKind::Success, t);
                    }
                });
            }
        }
    }
}
