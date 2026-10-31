//! Quests: the SIM quest surface, gameplay only. Live quests from the in-game
//! QuestSystem (auto-tracked, XP rewards): active with step progress, available
//! to accept, then completed.
//!
//! The learn-by-doing chains (`data/onboarding/quests.json`) used to share this
//! page as a second column; they moved to the Tasks page (operator 2026-08-16:
//! "Quests is for gameplay stuff and Tasks is for not-game stuff"), where since
//! 2026-10-30 they are the suggested cards at the top of Todo (`pages/tasks.rs`). The Profile page's game-quests section was folded in here
//! in v0.415.0.
//!
//! THE ACTIVE CARD AND THE CHAIN WINDOW (operator, 2026-10-30: "make it so the 'active area'
//! has the first steps quest kind of how it is, maybe find some way to visually improve it.
//! Then when we click on that quest, I want it to like popup all the quests in the chain").
//! A quest under way is a card with its chain ("Quest 2 of the First Steps chain"), its name,
//! "Step 2 of 8" over a meter with a segment per step, the step the player is on as the card's
//! focus, and its rewards. A click on any quest (under way, available or finished) opens its
//! chain in a window (`quests/chain_window.rs`): in normal play only what the player has
//! reached, in Dev every quest and step, the ones not reached yet dimmed. The page uses the
//! width it has: the quests under way in a wide column, Available and Completed beside them,
//! stacked when the window is narrow.

use egui::{Frame, RichText, ScrollArea, Stroke};
use crate::gui::GuiState;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::i18n::{t, tf};
use crate::gui::widgets::{with_hint, Hint};

mod chain_window;

/// Widest the content gets: the quests under way and the column beside them. Unclamped, an
/// ultrawide window would stretch a card into one line of text floating in space.
const CONTENT_MAX_WIDTH: f32 = 1120.0;
/// The column of Available and Completed quests beside the ones under way.
const SIDE_COLUMN_WIDTH: f32 = 340.0;
/// Narrower than this, the two columns stack.
const TWO_COLUMNS_FROM: f32 = 820.0;
/// A quest under way alone is held to this width, so its step reads as one line, not a banner.
const CARD_MAX_WIDTH: f32 = 760.0;
/// Quests under way sit side by side when each card gets at least this much.
const CARD_MIN_WIDTH: f32 = 340.0;
/// At most this many quests under way side by side.
const MAX_ACROSS: usize = 2;
/// A step meter has one segment per step up to this many; a longer quest gets one bar.
const MAX_SEGMENTS: usize = 24;
/// The step meter's height, in points.
const METER_HEIGHT: f32 = 8.0;
/// The accent stripe down the left of a card under way and of its current step.
const STRIPE: f32 = 3.0;

pub fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    egui::CentralPanel::default()
        .frame(Frame::none().fill(theme.bg_panel()).inner_margin(theme.card_padding))
        .show(ctx, |ui| {
            ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(CONTENT_MAX_WIDTH);

                    // Page title matches the nav button; before v0.1144 the
                    // page never stated its own name (only section eyebrows).
                    ui.label(
                        RichText::new(t("quests.title"))
                            .size(theme.font_size_title)
                            .color(theme.text_primary()),
                    );
                    // Subtitle replaces the old SIM QUESTS eyebrow (the whole
                    // page is sim now) and points at where the real-life
                    // tutorials went.
                    ui.label(
                        RichText::new(t("quests.subtitle"))
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(theme.spacing_md);

                    draw_game_quests(ui, theme, state);
                });
            });
        });
    chain_window::draw(ctx, theme, state);
}

/// Live sim quests from the in-game QuestSystem (auto-tracked, XP rewards):
/// active quests with step progress, available quests to accept, then the
/// completed list. Moved from the Profile page's retired Quests section
/// (v0.415.0). The SIM QUESTS eyebrow and its "Tracked automatically in-game"
/// heading came off when the page went sim-only: the page title plus its
/// subtitle now say the same thing once instead of twice.
fn draw_game_quests(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState) {
    let has_active = state.quests.iter().any(|q| !q.completed);
    let has_completed = state.quests.iter().any(|q| q.completed);

    // Empty only when there is nothing at all to show. The available list has
    // to count here too, otherwise a fresh session with acceptable quests but
    // no accepted ones bails out above the Accept buttons.
    if !has_active && !has_completed && state.quests_available.is_empty() {
        widgets::card(ui, theme, |ui| {
            ui.label(
                RichText::new(t("quests.empty"))
                    .size(theme.font_size_body)
                    .color(theme.text_muted()),
            );
        });
        return;
    }

    // The quest whose card was clicked: its chain window opens (the quest mirror builds the
    // chain next frame, `quests/chain_window.rs`).
    let mut open: Option<String> = None;
    let avail = ui.available_width();
    if avail >= TWO_COLUMNS_FROM && (has_completed || !state.quests_available.is_empty()) {
        let gap = theme.spacing_xl;
        let main = avail - SIDE_COLUMN_WIDTH - gap;
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.set_width(main);
                draw_active(ui, theme, state, &mut open);
            });
            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(SIDE_COLUMN_WIDTH);
                draw_available(ui, theme, state, &mut open);
                draw_completed(ui, theme, state, &mut open);
            });
        });
    } else {
        draw_active(ui, theme, state, &mut open);
        draw_available(ui, theme, state, &mut open);
        draw_completed(ui, theme, state, &mut open);
    }
    if open.is_some() {
        state.quest_chain_open = open;
    }
}

/// A section's heading: Active, Available, Completed.
fn heading(ui: &mut egui::Ui, theme: &Theme, key: &'static str) {
    ui.label(RichText::new(t(key)).size(theme.font_size_body).color(theme.text_secondary()));
    ui.add_space(theme.spacing_xs);
}

/// The quests under way, each a card: its chain, its name, "Step 2 of 8" over the step meter,
/// the step it is on as the focus, its rewards.
fn draw_active(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, open: &mut Option<String>) {
    heading(ui, theme, "quests.active");
    if !state.quests.iter().any(|q| !q.completed) {
        widgets::card(ui, theme, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(t("quests.none_active")).size(theme.font_size_small).color(theme.text_muted()));
        });
        ui.add_space(theme.spacing_md);
        return;
    }
    // Several quests under way (finishing one starts every quest that needs it) sit side by
    // side where there is room; one alone is held to a readable width.
    let active: Vec<&crate::gui::GuiQuest> = state.quests.iter().filter(|q| !q.completed).collect();
    let width = ui.available_width();
    let across = ((width / CARD_MIN_WIDTH) as usize).clamp(1, MAX_ACROSS).min(active.len());
    if across > 1 {
        ui.columns(across, |cols| {
            for (i, q) in active.iter().enumerate() {
                // A column's layout is justified, which spreads a wrapped line's words across
                // it ("A d v a n c e d"); a plain vertical inside keeps text left-aligned.
                cols[i % across].vertical(|ui| active_card(ui, theme, q, open));
            }
        });
    } else {
        for q in active {
            ui.scope(|ui| {
                ui.set_max_width(CARD_MAX_WIDTH);
                active_card(ui, theme, q, open);
            });
        }
    }
    ui.add_space(theme.spacing_sm);
}

/// One quest under way: its chain, its name, "Step 2 of 8" over the step meter, the step it is
/// on as the focus, its rewards. A click opens its chain.
fn active_card(ui: &mut egui::Ui, theme: &Theme, q: &crate::gui::GuiQuest, open: &mut Option<String>) {
    let card = Frame::none()
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, theme.border()))
        .corner_radius(egui::CornerRadius::same(theme.border_radius_lg as u8))
        .inner_margin(egui::Margin { left: (theme.spacing_lg + STRIPE) as i8, right: theme.spacing_lg as i8, top: theme.spacing_md as i8, bottom: theme.spacing_md as i8 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // The chain on the left, "Step 2 of 8" on the right, then the name on a line of its
            // own, wrapping: beside the count, a long name ("Advanced Habitat Construction")
            // pushed a card two-across past its column and under the count.
            ui.horizontal(|ui| {
                if !q.chain_name.is_empty() {
                    ui.label(
                        RichText::new(tf("quests.chain_eyebrow", &[("n", &q.chain_pos), ("chain", &q.chain_name)]))
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                }
                if q.step_total > 0 {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(tf("quests.step_count", &[("n", &(q.step_index + 1).min(q.step_total)), ("total", &q.step_total)]))
                                .size(theme.font_size_body)
                                .color(theme.accent()),
                        );
                    });
                }
            });
            ui.add(egui::Label::new(RichText::new(&q.name).size(theme.font_size_heading).color(theme.text_strong()).strong()).wrap());
            if q.step_total > 0 {
                ui.add_space(theme.spacing_xs);
                step_meter(ui, theme, q.step_index, q.step_total);
                ui.add_space(theme.spacing_md);
                now_block(ui, theme, &q.step_desc);
            }
            let rewards = reward_list(&q.rewards, &q.xp_rewards);
            if !rewards.is_empty() {
                ui.add_space(theme.spacing_sm);
                ui.label(
                    RichText::new(tf("quests.reward_line", &[("list", &rewards)]))
                        .size(theme.font_size_small)
                        .color(theme.text_secondary()),
                );
            }
            ui.add_space(theme.spacing_xs);
            // In a one-row horizontal: a right-to-left layout straight in the card would claim
            // all the height left below it, and the card ran to the bottom of the page.
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(t("quests.see_chain")).size(theme.font_size_small).color(theme.accent()));
                });
            });
        });
    let rect = card.response.rect;
    // The accent stripe down its left edge: this is the quest being played.
    let stripe = egui::Rect::from_min_size(rect.min, egui::vec2(STRIPE, rect.height()));
    ui.painter().rect_filled(
        stripe,
        egui::CornerRadius { nw: theme.border_radius_lg as u8, sw: theme.border_radius_lg as u8, ne: 0, se: 0 },
        theme.accent(),
    );
    let click = card.response.interact(egui::Sense::click()).hint("quests.card_hint");
    if click.hovered() {
        ui.painter().rect_stroke(rect, egui::CornerRadius::same(theme.border_radius_lg as u8), Stroke::new(1.0, theme.accent()), egui::StrokeKind::Outside);
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    if click.clicked() {
        *open = Some(q.id.clone());
    }
    ui.add_space(theme.spacing_sm);
}

/// One segment per step: the finished ones filled, the one the player is on half-lit, the
/// rest the empty track. Past `MAX_SEGMENTS` steps, one bar.
fn step_meter(ui: &mut egui::Ui, theme: &Theme, done: usize, total: usize) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, METER_HEIGHT), egui::Sense::hover());
    let painter = ui.painter();
    let corners = egui::CornerRadius::same((METER_HEIGHT / 2.0) as u8);
    let done = done.min(total);
    if total == 0 {
        return;
    }
    if total > MAX_SEGMENTS {
        painter.rect_filled(rect, corners, theme.slider_track());
        let filled = egui::Rect::from_min_size(rect.min, egui::vec2(rect.width() * done as f32 / total as f32, rect.height()));
        painter.rect_filled(filled, corners, theme.accent());
        return;
    }
    let gap = theme.spacing_xs * 2.0;
    let seg = ((rect.width() - gap * (total - 1) as f32) / total as f32).max(1.0);
    for i in 0..total {
        let x = rect.left() + i as f32 * (seg + gap);
        let r = egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(seg, rect.height()));
        let color = if i < done {
            theme.accent()
        } else if i == done {
            theme.accent().gamma_multiply(0.45)
        } else {
            theme.slider_track().gamma_multiply(0.6)
        };
        painter.rect_filled(r, corners, color);
    }
}

/// The step the player is on: the card's focus, on a faint accent ground with a stripe.
fn now_block(ui: &mut egui::Ui, theme: &Theme, step: &str) {
    let block = Frame::none()
        .fill(theme.accent().gamma_multiply(0.08))
        .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
        .inner_margin(egui::Margin { left: (theme.spacing_md + STRIPE) as i8, right: theme.spacing_md as i8, top: theme.spacing_sm as i8, bottom: theme.spacing_sm as i8 })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(t("quests.now")).size(theme.font_size_small).color(theme.accent()).strong());
            ui.add(egui::Label::new(RichText::new(step).size(theme.font_size_body).color(theme.text_primary())).wrap());
        });
    let r = block.response.rect;
    ui.painter().rect_filled(
        egui::Rect::from_min_size(r.min, egui::vec2(STRIPE, r.height())),
        egui::CornerRadius { nw: theme.border_radius as u8, sw: theme.border_radius as u8, ne: 0, se: 0 },
        theme.accent(),
    );
}

/// "2 Iron Ingot, 30 Metalworking XP": a quest's rewards, by name.
pub(super) fn reward_list(items: &[(String, u32)], xp: &[(String, u32)]) -> String {
    let mut parts: Vec<String> = items.iter().map(|(name, n)| tf("quests.reward_item", &[("n", n), ("item", name)])).collect();
    parts.extend(xp.iter().map(|(skill, n)| tf("quests.reward_xp", &[("n", n), ("skill", skill)])));
    parts.join(", ")
}

/// Available quests (v0.748, ladder rung 4): accept-able content the
/// prerequisite chaining alone never surfaced. One card per quest with an
/// Accept button; the frame bridge applies it to the live tracker. A click on
/// the card opens its chain.
fn draw_available(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, open: &mut Option<String>) {
    if state.quests_available.is_empty() {
        return;
    }
    heading(ui, theme, "quests.available");
    let available = state.quests_available.clone();
    for q in &available {
        let card = widgets::card_clickable(ui, theme, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(&q.name)
                        .size(theme.font_size_body)
                        .color(theme.text_primary()),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if with_hint(ui, "quests.accept_hint", |ui| widgets::compact_button(ui, theme, t("quests.accept"), widgets::ButtonVariant::Primary)) {
                        state.pending_accept_quest = Some(q.id.clone());
                    }
                });
            });
            if !q.description.is_empty() {
                ui.add(
                    egui::Label::new(
                        RichText::new(&q.description)
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    )
                    .wrap(),
                );
            }
        })
        .hint("quests.available_hint");
        if card.clicked() {
            *open = Some(q.id.clone());
        }
        ui.add_space(theme.spacing_xs);
    }
    ui.add_space(theme.spacing_md);
}

/// Completed quests, one row each; a click on one opens its chain.
fn draw_completed(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, open: &mut Option<String>) {
    if !state.quests.iter().any(|q| q.completed) {
        return;
    }
    heading(ui, theme, "quests.completed");
    widgets::card(ui, theme, |ui| {
        ui.set_width(ui.available_width());
        for q in state.quests.iter().filter(|q| q.completed) {
            let row = ui
                .horizontal(|ui| {
                    // A painted tick, centred on the text line; the U+2713
                    // glyph sat high because it comes from a fallback font.
                    let side = theme.font_size_small;
                    let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
                    widgets::icons::paint_check(ui.painter(), rect, theme.success());
                    ui.label(
                        RichText::new(&q.name)
                            .size(theme.font_size_small)
                            .color(theme.text_secondary()),
                    );
                    ui.add_space(ui.available_width());
                })
                .response
                .interact(egui::Sense::click())
                .hint("quests.completed_hint");
            if row.hovered() {
                ui.painter().rect_stroke(row.rect, egui::CornerRadius::same(theme.border_radius as u8), Stroke::new(1.0, theme.accent()), egui::StrokeKind::Outside);
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if row.clicked() {
                *open = Some(q.id.clone());
            }
        }
    });
}
