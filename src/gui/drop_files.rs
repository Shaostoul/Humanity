//! Files dragged from the computer's file manager onto the window (operator, 2026-10-17: "Can we
//! add drag and drop functionality like dragging a file on windows 10 file explorer into the
//! humanityOS window?").
//!
//! The window already hears dropped files (winit's drag and drop, through egui's input); this
//! decides where they go. Each frame, every page or window that takes files says so with
//! [`register`]: what a drop there will do, in words, and what to do with the files. The most
//! specific one wins: a file picker over a window over a page ([`Level`]), and among equals the
//! last drawn. While files are dragged over the window a dimmed overlay says what letting go will
//! do (or that nothing here takes files, and where to drop them instead); when they are let go,
//! the winner gets them, exactly as if they had been chosen in its picker, so every check the
//! picker path makes is made. Folders are not sent: a folder dropped is named and left out.
//!
//! Where a drop goes (the operator, 2026-10-17: "user drag and drops file onto HumanityOS. First
//! window popups with all the controls of what to do with the file ... If we're also in a chat
//! channel ... it also posts the image with the title and description to the general chat"): by
//! default the upload window, Add to Media with every control (`Level::App`, files/add.rs
//! `upload_dropped`); over a channel of the chat the same window with "Also post in chat" ticked
//! for that channel; over a private conversation (a direct message, a group, the scratch pad) the
//! private, encrypted send that conversation's Attach uses, never a public upload; over Shared
//! Files, Shared Files; over an item's Edit window, that item; over an open file picker, its choice.
//!
//! Order in a frame: [`begin_frame`] before the pages draw, then each page and window draws (and
//! registers), then [`finish`] after everything else. An open file picker takes a drop as its
//! choice ([`take_for_picker`]). Everything lives in the egui context's own memory, so the in-world
//! screens, which draw pages into other contexts that never receive dropped files, cannot catch a
//! drop meant for the window.
//!
//! The website does the same with the browser's drag and drop (web/chat/chat-messages.js, the
//! Media page and Shared Files), with the same words.

use crate::gui::theme::Theme;
use crate::gui::GuiState;
use egui::{Align2, Color32, Id, RichText};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// How specific a place that takes files is: a later level wins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Anywhere at all: the upload window, when nothing more specific is on screen.
    App,
    /// A page (the chat, the Media page, Shared Files).
    Page,
    /// A window over a page (a Media item's Edit window).
    Window,
    /// An open file picker: a drop is its choice.
    Picker,
}

/// What is done with the files a place takes.
type Handler = Box<dyn FnOnce(&mut GuiState, Vec<PathBuf>) + Send>;

struct Taker {
    level: Level,
    words: String,
    /// None: the place says why it takes nothing (signed out, say); the drop is refused with its words.
    handler: Option<Handler>,
}

/// This frame's state, kept in the context's memory.
#[derive(Clone, Default)]
struct Frame {
    takers: Arc<Mutex<Vec<Taker>>>,
    dropped: Arc<Mutex<Option<Vec<PathBuf>>>>,
}

/// A note left after a drop that went nowhere (or left folders out), until `until` (egui time).
#[derive(Clone)]
struct Note {
    words: String,
    until: f64,
}

/// Said over the window when nothing on screen takes files, and after such a drop.
pub const NOTHING_HERE: &str = "Files cannot be dropped here. Drop them on a chat, the Files page or Shared Files.";
/// Said when only folders were dropped.
pub const FOLDERS_LEFT_OUT: &str = "Folders cannot be sent. Open the folder and drop the files inside it.";
/// How long a note stays, in seconds.
const NOTE_SECONDS: f64 = 5.0;
/// How many dragged files the overlay names; the rest are counted.
const NAMES_SHOWN: usize = 6;

fn frame_id() -> Id {
    Id::new("hos_drop_files_frame")
}
fn note_id() -> Id {
    Id::new("hos_drop_files_note")
}

fn frame(ctx: &egui::Context) -> Frame {
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Frame>(frame_id()).clone())
}

/// Start the frame: forget last frame's takers and hold any files dropped this frame.
pub fn begin_frame(ctx: &egui::Context) {
    let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
    let fresh = Frame { takers: Arc::default(), dropped: Arc::new(Mutex::new((!dropped.is_empty()).then_some(dropped))) };
    ctx.data_mut(|d| d.insert_temp(frame_id(), fresh));
}

/// Say that this place takes dropped files: `words` is what a drop will do ("Drop to send in
/// #general"), `handler` what is done with them.
pub fn register(ctx: &egui::Context, level: Level, words: impl Into<String>, handler: impl FnOnce(&mut GuiState, Vec<PathBuf>) + Send + 'static) {
    if let Ok(mut t) = frame(ctx).takers.lock() {
        t.push(Taker { level, words: words.into(), handler: Some(Box::new(handler)) });
    }
}

/// Say that this place would take files but cannot now, and why (`words`, shown over the window
/// and after a drop).
pub fn refuse(ctx: &egui::Context, level: Level, words: impl Into<String>) {
    if let Ok(mut t) = frame(ctx).takers.lock() {
        t.push(Taker { level, words: words.into(), handler: None });
    }
}

/// For an open file picker: say a drop chooses these files, and take this frame's drop, if any,
/// as its choice. Nothing else gets those files.
pub fn take_for_picker(ctx: &egui::Context) -> Option<Vec<PathBuf>> {
    let f = frame(ctx);
    if let Ok(mut t) = f.takers.lock() {
        t.push(Taker { level: Level::Picker, words: PICKER_WORDS.to_string(), handler: None });
    }
    let taken = f.dropped.lock().ok().and_then(|mut d| d.take());
    taken.map(|paths| paths.into_iter().filter(|p| p.is_file()).collect())
}

/// What the overlay says over an open file picker.
pub const PICKER_WORDS: &str = "Drop to choose these";

/// The files being dragged over the window now (their paths, as the system gives them).
pub fn hovering(ctx: &egui::Context) -> Vec<PathBuf> {
    ctx.input(|i| i.raw.hovered_files.iter().filter_map(|f| f.path.clone()).collect())
}

/// What a drop here will do now: the winning taker's words, or [`NOTHING_HERE`].
pub fn words_now(ctx: &egui::Context) -> String {
    let f = frame(ctx);
    let takers = f.takers.lock();
    takers.ok().and_then(|t| winner(&t).map(|w| t[w].words.clone())).unwrap_or_else(|| NOTHING_HERE.to_string())
}

/// The index of the winning taker: the highest level, and among equals the last registered.
fn winner(takers: &[Taker]) -> Option<usize> {
    takers.iter().enumerate().max_by_key(|(i, t)| (t.level, *i)).map(|(i, _)| i)
}

/// End the frame: hand a drop to the winning taker (or say why it went nowhere), and draw the
/// overlay while files are dragged over the window.
pub fn finish(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let f = frame(ctx);
    let mut takers = f.takers.lock().map(|mut t| std::mem::take(&mut *t)).unwrap_or_default();
    let best = winner(&takers);
    let words = best.map(|w| takers[w].words.clone()).unwrap_or_else(|| NOTHING_HERE.to_string());
    // Whether letting go here does something (the overlay's edge says so).
    let takes = best.is_some_and(|w| takers[w].handler.is_some() || takers[w].level == Level::Picker);
    let dropped = f.dropped.lock().ok().and_then(|mut d| d.take());
    if let Some(paths) = dropped {
        let (files, folders): (Vec<PathBuf>, Vec<PathBuf>) = paths.into_iter().partition(|p| !p.is_dir());
        let handler = best.and_then(|w| takers[w].handler.take());
        match handler {
            _ if files.is_empty() => leave_note(ctx, FOLDERS_LEFT_OUT.to_string()),
            Some(h) => {
                if !folders.is_empty() {
                    leave_note(ctx, format!("{} {}", FOLDERS_LEFT_OUT, folder_names(&folders)));
                }
                h(state, files);
            }
            None => leave_note(ctx, words.clone()),
        }
    }
    let over = hovering(ctx);
    if !over.is_empty() {
        draw_overlay(ctx, theme, &words, takes, &over);
    }
    draw_note(ctx, theme);
}

fn folder_names(folders: &[PathBuf]) -> String {
    let names: Vec<String> = folders.iter().filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned())).collect();
    format!("Left out: {}.", names.join(", "))
}

/// Leave `words` at the top of the window for a few seconds (a drop that could not be used).
pub fn say(ctx: &egui::Context, words: impl Into<String>) {
    leave_note(ctx, words.into());
}

fn leave_note(ctx: &egui::Context, words: String) {
    let until = ctx.input(|i| i.time) + NOTE_SECONDS;
    ctx.data_mut(|d| d.insert_temp(note_id(), Note { words, until }));
}

/// The note a drop that went nowhere left, while it lasts (for the tests, and for anyone asking).
pub fn note_now(ctx: &egui::Context) -> Option<String> {
    let now = ctx.input(|i| i.time);
    ctx.data(|d| d.get_temp::<Note>(note_id())).filter(|n| n.until > now).map(|n| n.words)
}

/// The dimmed window and, in the middle, what letting go will do and the files being dragged,
/// each with its type (widgets/file_badge.rs). Nothing in it can be clicked: the drag goes on.
fn draw_overlay(ctx: &egui::Context, theme: &Theme, words: &str, takes: bool, over: &[PathBuf]) {
    let screen = ctx.screen_rect();
    let dim = egui::LayerId::new(egui::Order::Foreground, Id::new("hos_drop_overlay_dim"));
    ctx.layer_painter(dim).rect_filled(screen, 0.0, Color32::from_black_alpha(160)); // theme-exempt: the dialogs' own dimming (widgets/dialog.rs)
    let edge = if takes { theme.accent() } else { theme.warning() };
    egui::Area::new(Id::new("hos_drop_overlay_card"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme.bg_card())
                .stroke(egui::Stroke::new(2.0, edge))
                .corner_radius(egui::CornerRadius::same(theme.border_radius_lg as u8))
                .inner_margin(egui::Margin::same((theme.spacing_lg) as i8))
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.label(RichText::new(words).size(theme.font_size_heading).color(theme.text_primary()).strong());
                    ui.add_space(theme.spacing_sm);
                    for p in over.iter().take(NAMES_SHOWN) {
                        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                        ui.horizontal(|ui| {
                            if p.is_dir() {
                                ui.label(RichText::new("Folder").size(theme.font_size_small).color(theme.text_muted()));
                            } else {
                                crate::gui::widgets::file_badge::show(ui, theme, &crate::net::file_risk::file_type(&name, None));
                            }
                            ui.label(RichText::new(name).size(theme.font_size_small).color(theme.text_secondary()));
                        });
                    }
                    if over.len() > NAMES_SHOWN {
                        ui.label(RichText::new(format!("and {} more", over.len() - NAMES_SHOWN)).size(theme.font_size_small).color(theme.text_muted()));
                    }
                });
        });
}

/// The note a drop left, at the top of the window, while it lasts.
fn draw_note(ctx: &egui::Context, theme: &Theme) {
    let Some(words) = note_now(ctx) else { return };
    egui::Area::new(Id::new("hos_drop_note"))
        .order(egui::Order::Tooltip)
        .anchor(Align2::CENTER_TOP, egui::vec2(0.0, 48.0))
        .interactable(false)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme.bg_card())
                .stroke(egui::Stroke::new(1.0, theme.warning()))
                .corner_radius(egui::CornerRadius::same(theme.border_radius as u8))
                .inner_margin(egui::Margin::symmetric(12, 8))
                .show(ui, |ui| {
                    ui.set_max_width(560.0);
                    ui.label(RichText::new(words).size(theme.font_size_body).color(theme.text_primary()));
                });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
}

#[cfg(test)]
#[path = "drop_files_tests.rs"]
mod tests;
