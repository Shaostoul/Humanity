//! Device-to-device big files in the conversation (docs/design/media-showcase.md, "Device-to-device
//! big files: the shapes, exact", part 6): one row per transfer with the other person, at the top of
//! the DM, on both sides. Each row: the file's name and size, the words for where it stands
//! (engine/file_transfer.rs `status_words`), a progress bar while it goes, Pause, Resume and Cancel,
//! and the line saying whose app has to stay open.
//!
//! An offer to this person asks in the order the spec gives: "Files in messages" first (a sender
//! this person does not accept files from gets the one-file card, and only its Accept leads on),
//! then "Ann wants to send you clip.mp4 (2.3 GB). Accept?" with Accept and Decline
//! (net/file_offer.rs `question`). Under the protected setup a non-friend's offer is left out.
//!
//! Buttons only record what was pressed; the actions run after the rows are drawn.
//!
//! Takes `use super::*` like the page's other children.

use super::*;
use crate::engine::file_transfer as ft;
use crate::net::file_offer::{self as fo, OfferRecord, Question, Stage};

/// Ended rows shown per conversation at most (the newest); older ones stay in the store.
const ENDED_SHOWN: usize = 5;
/// The most the rows take of the conversation's height before they scroll.
const MAX_HEIGHT: f32 = 240.0;

/// What a row's button asked for.
enum Act {
    Accept(String, String),
    LetIn(String),
    Decline(String),
    Cancel(String),
    Pause(String),
    Resume(String),
    Remove(String),
    StopPreparing(String),
}

/// The rows of the conversation with `peer`, drawn above its messages; nothing when there are none.
pub(super) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, peer: &str) {
    let mut records: Vec<OfferRecord> = state
        .dm_store
        .as_ref()
        .map(|s| s.file_offers().iter().filter(|r| r.peer == peer).cloned().collect())
        .unwrap_or_default();
    let preparing: Vec<(String, String, u64, u64)> = state
        .files_flow
        .transfers
        .preparing
        .iter()
        .filter(|p| p.peer == peer)
        .map(|p| (p.id.clone(), p.name.clone(), p.size, p.done.load(std::sync::atomic::Ordering::Relaxed)))
        .collect();
    if records.is_empty() && preparing.is_empty() {
        return;
    }
    records.sort_by(|a, b| b.at.cmp(&a.at));
    let mut ended_seen = 0;
    records.retain(|r| {
        if r.stage.ended() {
            ended_seen += 1;
            ended_seen <= ENDED_SHOWN
        } else {
            true
        }
    });
    let name = crate::engine::dm::dm_display_name(state, peer);
    let online = state.chat_users.iter().any(|u| u.public_key == peer);
    let mut acts = Vec::new();
    egui::ScrollArea::vertical().id_salt("dm_transfer_rows").max_height(MAX_HEIGHT).show(ui, |ui| {
        for (id, file, size, done) in &preparing {
            card(ui, theme, |ui| {
                title(ui, theme, file, *size);
                let pct = if *size > 0 { (*done as f64 / *size as f64 * 100.0).min(100.0) } else { 0.0 };
                muted(ui, theme, &format!("Preparing {file} ({pct:.0}%): reading it for its checks before it is offered."));
                widgets::progress_bar(ui, theme, (pct / 100.0) as f32, None);
                if widgets::Button::secondary("Cancel").size(widgets::ButtonSize::Small).show(ui, theme) {
                    acts.push(Act::StopPreparing(id.clone()));
                }
            });
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        }
        for r in &records {
            let live = state.files_flow.transfers.live.get(&r.offer.id);
            card(ui, theme, |ui| draw_row(ui, theme, state, r, live, &name, online, &mut acts));
        }
    });
    if records.iter().any(|r| !r.stage.ended()) {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
    for act in acts {
        match act {
            Act::Accept(id, file) => {
                let dir = crate::gui::widgets::image_cache::default_downloads_dir();
                ft::accept(state, &id, &dir, &super::attach_view::save_name(&file));
            }
            Act::LetIn(id) => ft::let_in(state, &id),
            Act::Decline(id) => {
                ft::decline(state, &id);
            }
            Act::Cancel(id) => ft::cancel(state, &id),
            Act::Pause(id) => ft::pause(state, &id),
            Act::Resume(id) => ft::resume(state, &id),
            Act::Remove(id) => ft::remove(state, &id),
            Act::StopPreparing(id) => ft::stop_preparing(state, &id),
        }
    }
}

/// One transfer's row.
#[allow(clippy::too_many_arguments)]
fn draw_row(ui: &mut egui::Ui, theme: &Theme, state: &GuiState, r: &OfferRecord, live: Option<&ft::Live>, name: &str, online: bool, acts: &mut Vec<Act>) {
    let id = r.offer.id.clone();
    title(ui, theme, &r.offer.name, r.offer.size);
    if !r.outgoing && r.stage == Stage::Offered && !r.here {
        let prefs = &state.settings.files_in_messages;
        let friend = crate::engine::protected::picture_friend(state, &r.peer);
        let hides = crate::engine::protected::hides_pictures_from(state, &r.peer);
        match fo::question(prefs.accept_from, friend, hides, r.let_in, name, &r.offer.name, r.offer.size) {
            Question::LeftOut => {
                // Left out without an answer: the sender's row keeps waiting until the offer runs out.
                muted(ui, theme, fo::LEFT_OUT_LINE);
                if small(ui, theme, "Remove") {
                    acts.push(Act::Remove(id));
                }
            }
            Question::NotAccepted(line) => {
                ui.label(RichText::new(line).size(theme.font_size_small).color(theme.text_primary()));
                ui.horizontal(|ui| {
                    if widgets::Button::primary("Accept this one").size(widgets::ButtonSize::Small).show(ui, theme) {
                        acts.push(Act::LetIn(id.clone()));
                    }
                    if small(ui, theme, "Decline") {
                        acts.push(Act::Decline(id.clone()));
                    }
                });
            }
            Question::Ask(q) => {
                ui.label(RichText::new(q).size(theme.font_size_small).color(theme.text_primary()));
                muted(ui, theme, &format!("It comes straight from {name}'s device, not through the server, and is saved in your downloads folder."));
                ui.horizontal(|ui| {
                    if widgets::Button::primary("Accept").size(widgets::ButtonSize::Small).show(ui, theme) {
                        acts.push(Act::Accept(id.clone(), r.offer.name.clone()));
                    }
                    if small(ui, theme, "Decline") {
                        acts.push(Act::Decline(id.clone()));
                    }
                });
            }
        }
        return;
    }
    let phase = state.files_flow.transfers.phase(&r.peer);
    let not_set_up = matches!(phase, ft::Phase::NotSetUp(_));
    let words = ft::status_words(r, live, phase, name, online);
    let color = match &r.stage {
        Stage::Done => theme.success(),
        Stage::Failed(_) => theme.danger(),
        Stage::Accepted if not_set_up => theme.warning(),
        _ => theme.text_secondary(),
    };
    ui.label(RichText::new(words).size(theme.font_size_small).color(color));
    let progress = live.map(|l| l.progress).unwrap_or(0);
    if r.here && r.stage == Stage::Accepted && r.offer.pieces > 0 && (progress > 0 || live.is_some_and(|l| l.open)) {
        let frac = progress as f32 / r.offer.pieces as f32;
        let have = (progress.saturating_mul(fo::PIECE_SIZE)).min(r.offer.size);
        let label = format!("{} of {}", fo::size_words(have), fo::size_words(r.offer.size));
        widgets::progress_bar(ui, theme, frac, Some(&label));
    }
    if let Some(line) = ft::online_line(r, name) {
        muted(ui, theme, &line);
    }
    ui.horizontal(|ui| {
        if r.stage.ended() {
            if small(ui, theme, "Remove") {
                acts.push(Act::Remove(id.clone()));
            }
            return;
        }
        if !r.here {
            return; // it is another of this person's devices' to run
        }
        if r.stage == Stage::Accepted {
            if r.paused {
                if widgets::Button::primary("Resume").size(widgets::ButtonSize::Small).show(ui, theme) {
                    acts.push(Act::Resume(id.clone()));
                }
            } else if small(ui, theme, "Pause") {
                acts.push(Act::Pause(id.clone()));
            }
            if not_set_up && !r.paused && !r.outgoing && small(ui, theme, "Try again") {
                acts.push(Act::Resume(id.clone()));
            }
        }
        if widgets::Button::danger("Cancel").size(widgets::ButtonSize::Small).show(ui, theme) {
            acts.push(Act::Cancel(id.clone()));
        }
    });
}

fn card(ui: &mut egui::Ui, theme: &Theme, add: impl FnOnce(&mut egui::Ui)) {
    Frame::NONE
        .fill(theme.bg_card())
        .stroke(Stroke::new(1.0, theme.border()))
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing = Vec2::new(theme.spacing_sm, theme.spacing_xs);
            add(ui);
        });
    ui.add_space(theme.spacing_xs);
}

fn title(ui: &mut egui::Ui, theme: &Theme, name: &str, size: u64) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(name).size(theme.font_size_body).color(theme.text_primary()).strong());
        ui.label(RichText::new(fo::size_words(size)).size(theme.font_size_small).color(theme.text_muted()));
    });
}

fn muted(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    ui.label(RichText::new(text).size(theme.font_size_small).color(theme.text_muted()));
}

fn small(ui: &mut egui::Ui, theme: &Theme, label: &str) -> bool {
    widgets::Button::secondary(label).size(widgets::ButtonSize::Small).show(ui, theme)
}
