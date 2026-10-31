//! The chain window (operator, 2026-10-30): a click on a quest on the Quests page opens every
//! quest of its chain, top to bottom, each with its steps, the clicked one outlined in the
//! accent colour. Quests that need the same one finished are BRANCHES, drawn as a small tree
//! off it; finishing it starts them all, so none locks another.
//!
//! What it holds is the quest mirror's `ChainView` (`systems::quests::chain`), built while this
//! window is open: in normal play only what the player has reached (the steps ahead and the
//! quests not unlocked are not in it, so nothing here can spoil them: only "3 more steps to
//! discover" and "More quests to discover"), in the Dev play mode with dev cheats on every
//! quest and step, the ones the player has not reached in the muted text colour.

use egui::{Color32, Frame, Pos2, RichText, Stroke};

use crate::i18n::{t, tf, tn};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;
use crate::systems::quests::chain::{ChainQuest, ChainStep, ChainView, QuestState, Reveal, StepState};

/// The window's width, when the screen has room.
const WINDOW_WIDTH: f32 = 600.0;
/// Room kept between the window and the screen's edges.
const SCREEN_MARGIN: f32 = 40.0;
/// A quest's marker, the circle its lines join.
const MARKER: f32 = 14.0;
/// A step's mark.
const STEP_MARK: f32 = 9.0;
/// How far a branch sits in from the quest it branches off.
const INDENT: f32 = 26.0;
/// The lines joining quests.
const LINE: f32 = 1.5;

pub(super) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(open) = state.quest_chain_open.clone() else {
        // Closed: the next opening scrolls to its quest again.
        ctx.data_mut(|d| d.remove::<(String, u32)>(scrolled_key()));
        return;
    };
    // The chain arrives the frame after the click: the quest mirror builds it (lib.rs).
    let Some(view) = state.quest_chain.as_ref().filter(|v| v.focus == open).cloned() else { return };
    let mut keep = true;
    let title = tf("quests.chain_title", &[("chain", &view.root.name)]);
    widgets::dialog::dialog_scrolling(ctx, theme, "quest_chain_window", &title, &mut keep, |ui| {
        ui.set_width(WINDOW_WIDTH.min(ctx.screen_rect().width() - 2.0 * SCREEN_MARGIN).max(INDENT * 8.0));
        let intro = if view.reveal == Reveal::All { t("quests.chain_intro_dev") } else { t("quests.chain_intro_learned") };
        ui.add(egui::Label::new(RichText::new(intro).size(theme.font_size_small).color(theme.text_muted())).wrap());
        ui.add_space(theme.spacing_sm);
        quest(ui, theme, &view, &view.root);
    });
    if !keep {
        state.quest_chain_open = None;
        state.quest_chain = None;
        ctx.data_mut(|d| d.remove::<(String, u32)>(scrolled_key()));
    }
}

/// How many frames an opened window asks its scroll area to bring the clicked quest into view:
/// a window's first frames are egui's sizing passes, which can drop the request.
const SCROLL_FRAMES: u32 = 3;

/// Where the window keeps which quest it scrolled to, and for how many frames.
fn scrolled_key() -> egui::Id {
    egui::Id::new("quest_chain_scrolled")
}

/// Bring the clicked quest into the middle of the window when it opens: a branch sorted last
/// (Start Farming after the habitat) was below the fold, under the finished quests above it.
/// Only while the window opens, so scrolling by hand afterwards is never undone.
fn scroll_to_focus(ui: &mut egui::Ui, id: &str, rect: egui::Rect) {
    let (seen, frames): (String, u32) = ui.ctx().data(|d| d.get_temp(scrolled_key())).unwrap_or_default();
    let frames = if seen == id { frames } else { 0 };
    if frames < SCROLL_FRAMES {
        // At once, not animated: the window has only just opened.
        ui.scroll_to_rect_animation(rect, Some(egui::Align::Center), egui::style::ScrollAnimation::none());
        ui.ctx().data_mut(|d| d.insert_temp(scrolled_key(), (id.to_string(), frames + 1)));
    }
}

/// Whether the player has got to a quest: finished it or started it.
fn reached(q: &ChainQuest) -> bool {
    matches!(q.state, QuestState::Done | QuestState::Active)
}

/// One quest, then the quests after it, joined by lines. Returns its marker's centre, for the
/// line from the quest before it.
fn quest(ui: &mut egui::Ui, theme: &Theme, view: &ChainView, q: &ChainQuest) -> Pos2 {
    let marker = block(ui, theme, view, q);
    let stroke = Stroke::new(LINE, theme.border());
    let branching = q.next.len() >= 2 || (!q.next.is_empty() && q.hidden_next > 0);
    if branching {
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            ui.add_space(INDENT);
            ui.label(RichText::new(t("quests.branches")).size(theme.font_size_small).color(theme.text_muted()));
        });
        let mut ends: Vec<Pos2> = Vec::new();
        for next in &q.next {
            ends.push(indented(ui, |ui| quest(ui, theme, view, next)));
        }
        if q.hidden_next > 0 {
            ends.push(indented(ui, |ui| more_to_discover(ui, theme)));
        }
        if let Some(last) = ends.last() {
            let painter = ui.painter();
            painter.line_segment([egui::pos2(marker.x, marker.y + MARKER / 2.0), egui::pos2(marker.x, last.y)], stroke);
            for end in &ends {
                painter.line_segment([egui::pos2(marker.x, end.y), egui::pos2(end.x - MARKER / 2.0 - LINE, end.y)], stroke);
            }
        }
    } else if let Some(next) = q.next.first() {
        ui.add_space(theme.spacing_sm);
        let end = quest(ui, theme, view, next);
        ui.painter().line_segment([egui::pos2(marker.x, marker.y + MARKER / 2.0 + LINE), egui::pos2(end.x, end.y - MARKER / 2.0 - LINE)], stroke);
    } else if q.hidden_next > 0 {
        ui.add_space(theme.spacing_sm);
        let end = more_to_discover(ui, theme);
        ui.painter().line_segment([egui::pos2(marker.x, marker.y + MARKER / 2.0 + LINE), egui::pos2(end.x, end.y - MARKER / 2.0 - LINE)], stroke);
    }
    marker
}

/// `add` drawn one branch in, returning what it returns.
fn indented(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> Pos2) -> Pos2 {
    ui.horizontal_top(|ui| {
        ui.add_space(INDENT);
        ui.vertical(add).inner
    })
    .inner
}

/// The frame every quest and "more to discover" sits in: outlined in the accent colour for the
/// quest the window was opened on, invisible but the same size for every other, so the markers
/// line up whichever is outlined.
fn quest_frame(theme: &Theme, focus: bool) -> Frame {
    let (fill, edge) = if focus {
        (theme.accent().gamma_multiply(0.07), theme.accent())
    } else {
        (Color32::TRANSPARENT, Color32::TRANSPARENT)
    };
    Frame::none()
        .fill(fill)
        .stroke(Stroke::new(1.0, edge))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(theme.card_padding)
}

/// A quest's own block: its marker, name and state, then its steps and rewards.
fn block(ui: &mut egui::Ui, theme: &Theme, view: &ChainView, q: &ChainQuest) -> Pos2 {
    let focus = q.id == view.focus;
    let name_color = if reached(q) { theme.text_primary() } else { theme.text_muted() };
    let shown = quest_frame(theme, focus)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let marker = ui
                .horizontal(|ui| {
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(MARKER, MARKER), egui::Sense::hover());
                    paint_marker(ui.painter(), rect, theme, q.state);
                    ui.add_space(theme.spacing_sm);
                    ui.label(RichText::new(&q.name).size(theme.font_size_body).color(name_color).strong());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (words, color) = status(theme, q);
                        ui.label(RichText::new(words).size(theme.font_size_small).color(color));
                    });
                    rect.center()
                })
                .inner;
            ui.horizontal_top(|ui| {
                ui.add_space(MARKER + theme.spacing_sm);
                ui.vertical(|ui| body(ui, theme, focus, q));
            });
            marker
        });
    if focus {
        scroll_to_focus(ui, &q.id, shown.response.rect);
    }
    shown.inner
}

/// What the right of a quest's line says: Done, the step it is on, ready to start, or (Dev
/// only) not unlocked yet.
fn status(theme: &Theme, q: &ChainQuest) -> (String, Color32) {
    match q.state {
        QuestState::Done => (t("quests.state_done").to_string(), theme.success()),
        QuestState::Active => {
            let on = q.steps.iter().filter(|s| s.state == StepState::Done).count() + 1;
            (tf("quests.step_count", &[("n", &on.min(q.step_total)), ("total", &q.step_total)]), theme.accent())
        }
        QuestState::Available => (tn("quests.ready_steps", q.step_total as i64, &[]), theme.text_muted()),
        QuestState::Locked => (t("quests.state_locked").to_string(), theme.text_muted()),
    }
}

/// Under a quest's line: its description (the window's own quest, or one with no steps shown),
/// its steps, how many are still to discover, and its rewards.
fn body(ui: &mut egui::Ui, theme: &Theme, focus: bool, q: &ChainQuest) {
    let dim = !reached(q);
    if (focus || q.steps.is_empty()) && !q.description.is_empty() {
        ui.add(
            egui::Label::new(RichText::new(&q.description).size(theme.font_size_small).color(if dim { theme.text_muted() } else { theme.text_secondary() }))
                .wrap(),
        );
        ui.add_space(theme.spacing_xs);
    }
    for s in &q.steps {
        step_row(ui, theme, s);
    }
    if q.state == QuestState::Active && q.hidden_steps() > 0 {
        ui.label(RichText::new(tn("quests.more_steps", q.hidden_steps() as i64, &[])).size(theme.font_size_small).color(theme.text_muted()).italics());
    }
    let rewards = super::reward_list(&q.rewards, &q.xp_rewards);
    if !rewards.is_empty() {
        ui.add_space(theme.spacing_xs);
        ui.label(
            RichText::new(tf("quests.reward_line", &[("list", &rewards)]))
                .size(theme.font_size_small)
                .color(if dim { theme.text_muted() } else { theme.text_secondary() }),
        );
    }
}

/// One step: a tick when done, a filled dot on the step the player is on, an empty ring (and
/// muted words) on one not reached yet, which only the Dev view shows.
fn step_row(ui: &mut egui::Ui, theme: &Theme, s: &ChainStep) {
    let row_h = ui.fonts(|f| f.row_height(&egui::FontId::proportional(theme.font_size_small)));
    ui.horizontal_top(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(STEP_MARK, row_h), egui::Sense::hover());
        let mark = egui::Rect::from_center_size(rect.center(), egui::vec2(STEP_MARK, STEP_MARK));
        let painter = ui.painter();
        match s.state {
            StepState::Done => widgets::icons::paint_check(painter, mark.expand(2.0), theme.success()),
            StepState::Current => {
                painter.circle_filled(mark.center(), STEP_MARK / 2.0 - 1.0, theme.accent());
            }
            StepState::Ahead => {
                painter.circle_stroke(mark.center(), STEP_MARK / 2.0 - 1.0, Stroke::new(1.0, theme.text_muted()));
            }
        }
        ui.add_space(theme.spacing_sm);
        let text = RichText::new(&s.text).size(theme.font_size_small);
        let text = match s.state {
            StepState::Done => text.color(theme.text_secondary()),
            StepState::Current => text.color(theme.text_strong()).strong(),
            StepState::Ahead => text.color(theme.text_muted()),
        };
        ui.add(egui::Label::new(text).wrap());
    });
}

/// A quest's marker: a tick in a green disc when done, an accent disc with a dark centre while
/// under way, an accent ring when ready to start, a muted ring when not unlocked.
fn paint_marker(painter: &egui::Painter, rect: egui::Rect, theme: &Theme, state: QuestState) {
    let c = rect.center();
    let r = rect.width() / 2.0 - 1.0;
    match state {
        QuestState::Done => {
            painter.circle_filled(c, r, theme.success());
            widgets::icons::paint_check(painter, rect, Theme::ink_on(theme.success()));
        }
        QuestState::Active => {
            painter.circle_filled(c, r, theme.accent());
            painter.circle_filled(c, r * 0.4, Theme::ink_on(theme.accent()));
        }
        QuestState::Available => {
            painter.circle_stroke(c, r, Stroke::new(LINE, theme.accent()));
        }
        QuestState::Locked => {
            painter.circle_stroke(c, r, Stroke::new(LINE, theme.text_muted()));
        }
    }
}

/// "More quests to discover after this one": where a chain goes on past what the player has
/// unlocked, in normal play. Returns its marker's centre.
fn more_to_discover(ui: &mut egui::Ui, theme: &Theme) -> Pos2 {
    quest_frame(theme, false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(MARKER, MARKER), egui::Sense::hover());
                ui.painter().circle_stroke(rect.center(), rect.width() / 2.0 - 1.0, Stroke::new(LINE, theme.text_muted()));
                ui.add_space(theme.spacing_sm);
                ui.label(RichText::new(t("quests.more_quests")).size(theme.font_size_small).color(theme.text_muted()).italics());
                rect.center()
            })
            .inner
        })
        .inner
}
