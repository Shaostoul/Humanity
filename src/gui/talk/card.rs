//! Drawing the talk card (rung 3 of docs/design/embeddable-ai.md, 2026-11-12). Its own interactable
//! Area through gui/layers.rs (PageFloat), screen-centred in the upper third as the v0.797 card was;
//! Goodbye, Esc, a click away and walking more than 4 m away close it (lib.rs).
//!
//! Top to bottom: the name, the job and what they are doing now; their line, with the AI tag when the
//! AI wrote it (5.4: "Written by AI on this computer, not by a person"), or the reason in words when it
//! fell back to a written line; the action an AI line offers, as a button the player clicks (8.5); the
//! "Say anything" box when AI is installed and chosen (or the one-time offer of AI when none is); and
//! the written choices, always there.

use egui::{Pos2, RichText};
use humanity_core::i18n::{t, t_dyn, tf};

use super::chat::{Availability, Phase, Said};
use super::{OfferState, View};
use crate::ai::talk::Action;
use crate::gui::theme::Theme;
use crate::gui::widgets::{Button, Hint};
use crate::gui::{layers, GuiState};

/// The card's width, points.
const WIDTH: f32 = 400.0;

/// Draw the card, run its AI box for this frame, and settle what was clicked.
pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    if state.npc_talk_target.is_none() {
        return;
    }
    step_chat(state);
    let mut close = false;
    let screen = ctx.screen_rect();
    let area = layers::area(ctx, egui::Id::new("npc_talk_card"), layers::Layer::PageFloat)
        .fixed_pos(Pos2::new(screen.center().x - WIDTH * 0.5, screen.top() + screen.height() * 0.18))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(12)).show(ui, |ui| {
                ui.set_width(WIDTH);
                draw_head(ui, theme, state);
                ui.separator();
                draw_line(ui, theme, state);
                draw_suggestion(ui, theme, state);
                draw_box(ui, theme, state);
                ui.separator();
                close = draw_choices(ui, theme, state);
            });
        });
    // A click outside the card returns to gameplay (the in-world chat panel's convention, v0.773).
    let clicked_outside = ctx.input(|i| i.pointer.any_click()) && ctx.input(|i| i.pointer.interact_pos()).is_some_and(|p| !area.response.rect.contains(p));
    if state.talk.chat.busy() || matches!(state.talk.chat.phase, Phase::Preparing | Phase::Warming(_)) {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    if close || clicked_outside {
        close_card(state);
    }
}

/// Close the card: a reply still being written is cancelled (the guide on the HUD stays).
pub fn close_card(state: &mut GuiState) {
    state.npc_talk_target = None;
    let mut c = crate::ai::shared::client();
    state.talk.chat.cancel(&mut c);
    state.talk.chat.typing = false;
}

/// The AI box's work for this frame (also run by the frame while the card is open and not drawn,
/// src/engine/talk.rs, so a reply keeps arriving).
pub fn step_chat(state: &mut GuiState) {
    if state.talk.chat.phase == Phase::Off {
        return;
    }
    let prefer = state.settings.ai_brain.clone();
    let said = {
        let mut c = crate::ai::shared::client();
        c.poll();
        state.talk.chat.step(&mut c, &prefer)
    };
    if let Some(s) = said {
        state.talk.said = Some(s);
    }
}

fn draw_head(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&state.npc_talk_name).strong().color(theme.accent()));
        if !state.talk.job.is_empty() {
            ui.label(RichText::new(&state.talk.job).size(theme.font_size_small).color(theme.text_secondary()));
        }
    });
    if !state.npc_talk_activity.is_empty() && state.npc_talk_activity != state.talk.job {
        ui.label(RichText::new(tf("talk.doing", &[("activity", &state.npc_talk_activity)])).size(theme.font_size_small).color(theme.text_muted()));
    }
}

/// The AI tag beside a line the AI wrote (5.4), with its hint.
fn ai_tag(ui: &mut egui::Ui, theme: &Theme) {
    egui::Frame::new().fill(theme.accent()).corner_radius(egui::CornerRadius::same(3)).inner_margin(egui::Margin::symmetric(4, 1)).show(ui, |ui| {
        ui.label(RichText::new(t("ai.tag")).size(theme.font_size_small).strong().color(theme.bg_primary())).hint("ai.tag_hint");
    });
}

fn draw_line(ui: &mut egui::Ui, theme: &Theme, state: &GuiState) {
    ui.add_space(theme.spacing_xs);
    let chat = &state.talk.chat;
    if chat.busy() {
        // The reply as it streams, already tagged: the AI is writing it.
        ui.horizontal_wrapped(|ui| {
            ai_tag(ui, theme);
            let words = if chat.partial.is_empty() { tf("talk.ai.writing", &[("name", &state.npc_talk_name)]) } else { format!("{}...", chat.partial) };
            ui.label(RichText::new(words).size(theme.font_size_body).color(theme.text_primary()));
        });
        ui.add_space(theme.spacing_xs);
        return;
    }
    match &state.talk.said {
        None | Some(Said::Written(_)) => {
            let line = match &state.talk.said {
                Some(Said::Written(l)) => l.as_str(),
                _ => state.npc_talk_line.as_str(),
            };
            ui.label(RichText::new(line).size(theme.font_size_body).color(theme.text_primary()));
        }
        Some(Said::Ai { line, .. }) => {
            ui.horizontal_wrapped(|ui| {
                ai_tag(ui, theme);
                ui.label(RichText::new(line).size(theme.font_size_body).color(theme.text_primary()));
            });
            if let Some(tm) = &chat.last {
                let secs = |ms: u32| format!("{:.1}", f64::from(ms) / 1000.0);
                ui.label(RichText::new(tf("talk.ai.timing", &[("tokens", &tm.tokens), ("first", &secs(tm.ms_first_words)), ("all", &secs(tm.ms_reply))])).size(theme.font_size_small).color(theme.text_muted()));
            }
        }
        Some(Said::Fallback { line, why }) => {
            ui.label(RichText::new(line).size(theme.font_size_body).color(theme.text_primary()));
            ui.label(RichText::new(tf("talk.ai.fell_back", &[("why", why), ("name", &state.npc_talk_name)])).size(theme.font_size_small).color(theme.warning()));
        }
    }
    ui.add_space(theme.spacing_xs);
}

/// The action an AI line offered: a button the player clicks, never done for them (8.5).
fn draw_suggestion(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let Some(Said::Ai { action: Some(action), .. }) = state.talk.said.clone() else { return };
    let words = match &action {
        Action::OfferTask(id) => {
            let Some(o) = state.talk.offers.iter().find(|o| o.quest_id == *id && o.state == OfferState::Available) else { return };
            tf("talk.action.offer_task", &[("task", &o.name)])
        }
        Action::OpenTrade => tf("talk.action.open_trade", &[("name", &state.npc_talk_name)]),
        Action::PointWay(id) => {
            let Some(w) = state.talk.ways.iter().find(|w| w.id == *id) else { return };
            tf("talk.action.point_way", &[("place", &w.label)])
        }
    };
    if Button::primary(&words).hint("talk.action_hint").show(ui, theme) {
        act(state, &action);
    }
}

/// Do what a choice or an accepted suggestion asks: always through the game's own intents.
fn act(state: &mut GuiState, action: &Action) {
    match action {
        Action::OfferTask(id) => {
            if let Some(q) = state.talk.take_task(id) {
                state.pending_accept_quest = Some(q);
            }
        }
        Action::OpenTrade => {
            state.vendor_open = true;
            close_card(state);
        }
        Action::PointWay(id) => state.talk.show_way(id),
    }
}

fn draw_box(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let name = state.npc_talk_name.clone();
    let small = |ui: &mut egui::Ui, words: String, color| {
        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    };
    match state.talk.chat.availability {
        Availability::Offered => {}
        Availability::NoAi => {
            if !state.settings.ai_talk_offer_dismissed {
                ui.add_space(theme.spacing_xs);
                small(ui, tf("talk.ai.offer", &[("name", &name), ("size", &state.talk.offer_size)]), theme.text_muted());
                ui.horizontal(|ui| {
                    if Button::secondary(t("talk.ai.offer_open")).hint("talk.ai.offer_open_hint").show(ui, theme) {
                        state.go_to(crate::gui::history::Place::settings(crate::gui::SettingsCategory::Ai), crate::gui::How::Beside);
                        close_card(state);
                    }
                    if Button::secondary(t("talk.ai.offer_dismiss")).hint("talk.ai.offer_dismiss_hint").show(ui, theme) {
                        state.settings.ai_talk_offer_dismissed = true;
                        state.settings_dirty = true;
                    }
                });
            }
            return;
        }
        Availability::WrittenChosen => {
            small(ui, t("talk.ai.written_chosen").to_string(), theme.text_muted());
            return;
        }
        Availability::EngineMissing => {
            small(ui, tf("talk.ai.engine_update", &[("name", &name)]), theme.warning());
            return;
        }
        Availability::NoPersona => {
            small(ui, tf("talk.ai.no_persona", &[("name", &name)]), theme.text_muted());
            return;
        }
    }
    ui.add_space(theme.spacing_xs);
    let phase = state.talk.chat.phase.clone();
    let mut send = false;
    ui.horizontal(|ui| {
        let busy = state.talk.chat.busy();
        let open = !busy && !matches!(phase, Phase::Unavailable(_));
        let r = ui.add(egui::TextEdit::singleline(&mut state.talk.chat.input).hint_text(t("talk.ai.say_anything")).desired_width(WIDTH - 90.0).interactive(open)).hint("talk.ai.box_hint");
        state.talk.chat.typing = r.has_focus();
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            send = true;
            r.request_focus();
        }
        if busy {
            if Button::secondary(t("talk.ai.stop")).hint("talk.ai.stop_hint").show(ui, theme) {
                let mut c = crate::ai::shared::client();
                state.talk.chat.cancel(&mut c);
            }
        } else if Button::primary(t("talk.ai.ask")).hint("talk.ai.ask_hint").disabled(state.talk.chat.input.trim().is_empty() || matches!(phase, Phase::Unavailable(_))).show(ui, theme) {
            send = true;
        }
    });
    if send {
        let q = std::mem::take(&mut state.talk.chat.input);
        state.talk.chat.ask(&q);
    }
    match &phase {
        Phase::Preparing | Phase::Warming(_) => small(ui, tf("talk.ai.preparing", &[("name", &name)]), theme.text_muted()),
        Phase::Unavailable(words) => small(ui, words.clone(), theme.warning()),
        _ => {}
    }
    let model = state.talk.chat.model_name.clone();
    if !model.is_empty() {
        small(ui, tf("talk.ai.local", &[("model", &model)]), theme.text_muted());
    }
}

/// The size the offer names: the AI engine and the model Automatic would choose for this computer.
pub(crate) fn offer_size() -> String {
    let Ok(cat) = crate::ai::catalog::current() else { return String::new() };
    let total = crate::ai::hardware::read().memory_total_mb;
    let rich = total.is_none_or(|t| t >= crate::ai::models::RICH_NEEDS_TOTAL_MB);
    let pick = cat.models.iter().filter(|m| m.kind == "brain").find(|m| (m.choice == "characters-rich") == rich).or_else(|| cat.models.iter().find(|m| m.kind == "brain"));
    pick.map(|m| crate::gui::pages::settings::ai::gb_bytes(cat.engine.bytes + m.download_bytes())).unwrap_or_default()
}

/// The written choices. Returns true when Goodbye was clicked.
fn draw_choices(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) -> bool {
    let mut close = false;
    match state.talk.view {
        View::Main => {
            ui.horizontal_wrapped(|ui| {
                if !state.talk.topics.is_empty() && Button::secondary(t("talk.talk_about")).hint("talk.talk_about_hint").show(ui, theme) {
                    state.talk.view = View::Topics;
                }
                if !state.talk.offers.is_empty() && Button::secondary(t("talk.work")).hint("talk.work_hint").show(ui, theme) {
                    state.talk.view = View::Work;
                }
                if state.talk.trades && Button::secondary(t("talk.trade")).hint("talk.trade_hint").show(ui, theme) {
                    act(state, &Action::OpenTrade);
                }
                if !state.talk.ways.is_empty() && Button::secondary(t("talk.show_way")).hint("talk.show_way_hint").show(ui, theme) {
                    state.talk.view = View::Ways;
                }
                if more_lines(state) && Button::secondary(t("talk.more")).hint("talk.more_hint").show(ui, theme) {
                    state.npc_talk_advance();
                }
                if Button::secondary(t("talk.goodbye")).hint("talk.goodbye_hint").show(ui, theme) {
                    close = true;
                }
            });
        }
        View::Topics => {
            ui.horizontal_wrapped(|ui| {
                let topics: Vec<String> = state.talk.topics.iter().map(|(id, _)| id.clone()).collect();
                for id in topics {
                    if Button::secondary(t_dyn(&format!("talk.topic.{id}"))).hint("talk.topic_hint").show(ui, theme) {
                        state.talk.say_topic(&id);
                    }
                }
                back(ui, theme, state);
            });
        }
        View::Work => {
            let offers = state.talk.offers.clone();
            for o in &offers {
                ui.add_space(theme.spacing_xs);
                ui.label(RichText::new(&o.name).strong().color(theme.text_primary()));
                ui.label(RichText::new(&o.summary).size(theme.font_size_small).color(theme.text_secondary()));
                match &o.state {
                    OfferState::Available => {
                        if Button::primary(t("talk.take_task")).hint("talk.take_task_hint").show(ui, theme) {
                            act(state, &Action::OfferTask(o.quest_id.clone()));
                        }
                    }
                    OfferState::Taken => {
                        ui.label(RichText::new(t("talk.offer.taken")).size(theme.font_size_small).color(theme.success()));
                    }
                    OfferState::Done => {
                        ui.label(RichText::new(t("talk.offer.done")).size(theme.font_size_small).color(theme.text_muted()));
                    }
                    OfferState::Locked(first) => {
                        ui.label(RichText::new(tf("talk.offer.locked", &[("name", first)])).size(theme.font_size_small).color(theme.text_muted()));
                    }
                }
            }
            ui.add_space(theme.spacing_xs);
            ui.horizontal(|ui| back(ui, theme, state));
        }
        View::Ways => {
            ui.label(RichText::new(t("talk.ways_intro")).size(theme.font_size_small).color(theme.text_secondary()));
            ui.horizontal_wrapped(|ui| {
                let ways = state.talk.ways.clone();
                for w in &ways {
                    if Button::secondary(&w.label).hint("talk.way_hint").show(ui, theme) {
                        act(state, &Action::PointWay(w.id.clone()));
                    }
                }
                back(ui, theme, state);
            });
        }
    }
    if let Some(g) = state.talk.guide.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tf("talk.guide_on", &[("place", &g.label)])).size(theme.font_size_small).color(theme.text_muted()));
            if Button::secondary(t("talk.guide_clear")).hint("talk.guide_clear_hint").show(ui, theme) {
                state.talk.guide = None;
            }
        });
    }
    close
}

fn back(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    if Button::secondary(t("talk.back")).hint("talk.back_hint").show(ui, theme) {
        state.talk.view = View::Main;
    }
}

/// Whether More (E) has another written line to give: the chosen topic's, or the relay's dialog.
fn more_lines(state: &GuiState) -> bool {
    match &state.talk.topic {
        Some(topic) => state.talk.topics.iter().any(|(t, l)| t == topic && l.len() > 1),
        None => state.npc_talk_dialog.len().max(state.npc_talk_greetings.len()) > 1,
    }
}
