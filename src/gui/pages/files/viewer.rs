//! The Files page's middle pane (2026-10-19, the three panes: panes.rs): the item shown to fit the
//! area (operator: "middle is the fit to area picture, 3D model, sound/music visualizer").
//!
//! - A PICTURE fills the pane, its whole self, centred; a press on it (or View full size) opens the
//!   app's image viewer.
//! - A VIDEO shows its cover with Play here; once fetched it plays here, fitted, with its buttons
//!   (widgets/video_view.rs, the player the chat's videos use).
//! - A SOUND OR A SONG (an item whose own file is a sound: MP3, Ogg, FLAC, WAV and the rest the
//!   file-type list names) shows its cover above, when it has one, and its waveform under it:
//!   Play here fetches it and plays it through the app's mixer, the waveform drawn in, the part
//!   played in the accent colour, its spectrum moving above it, a press on it to go there
//!   (widgets/sound_view.rs and widgets/waveform.rs).
//! - A 3D MODEL shows its cover with View in 3D (a look is a fetch of the whole file, so it waits
//!   for the press); pressed, the model viewer fills the pane, to turn and zoom.
//! - ANY OTHER FILE is a large card: its type's drawing and name, the file's name and size, its
//!   word of caution, and Download.
//!
//! The tiles' rules come first, as they did on the item's old window (tiles.rs `show_of`): what the
//! protected setup hides says so; a labelled item shows its labels, and a stranger's picture
//! waits, each with Show it.

use egui::{RichText, Sense, Vec2};

use super::api::{self, Item};
use super::item_view::{files_of, ItemPage, Pressed};
use super::open::{self, Opened};
use super::{kinds, list, tiles};
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::widgets::image_cache::ImageStatus;
use crate::gui::GuiState;

/// View full size's label.
pub const VIEW_FULL: &str = "View full size";
pub const VIEW_FULL_TIP: &str = "Open it in the picture viewer, at its own size.";

/// Whether `item` is a sound or a song: its own file's type, by the file-type list
/// (data/safety/file_risks.json names the endings), is a sound.
pub(crate) fn is_sound(item: &Item) -> bool {
    let own = &files_of(item)[0];
    crate::net::file_risk::file_type(&open::shown_file_name(item, own), None).id == "sound"
}

/// The pane for the item shown (`page`), or the line saying to choose one when none is.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, page: &ItemPage, opened: Option<&mut Opened>, pressed: &mut Pressed) {
    let (Some(item), Some(opened)) = (page.item.clone(), opened) else {
        let words = if page.item.is_none() && !page.status.is_empty() { page.status.as_str() } else { "Loading..." };
        centred(ui, theme, |ui| {
            ui.label(RichText::new(words).size(theme.font_size_body).color(theme.text_muted()));
        });
        return;
    };
    let area = ui.available_size();
    let row_h = theme.button_height + theme.spacing_sm;
    let show = tiles::show_of(state, &item);
    let key = tiles::cache_key(state, &item);
    match &show {
        tiles::Show::Hidden(line) => {
            // Its textures are pictures too: a model's view closes where pictures are hidden.
            opened.viewer = None;
            centred(ui, theme, |ui| widgets::body_hint(ui, theme, line));
            return;
        }
        tiles::Show::Label(names) => {
            centred(ui, theme, |ui| {
                ui.label(RichText::new(format!("Labelled: {}.", names.join(", "))).size(theme.font_size_body).color(theme.warning()));
                if widgets::Button::secondary("Show it").tooltip("Show what the label covers, on this item only.").show(ui, theme) {
                    state.image_cache.allow(&key);
                }
            });
            return;
        }
        tiles::Show::Ask => {
            centred(ui, theme, |ui| {
                ui.label(RichText::new(format!("{} is not your friend. Pictures show only when you choose.", item.by_name())).size(theme.font_size_body).color(theme.text_primary()));
                if widgets::Button::secondary("Show it").show(ui, theme) {
                    state.image_cache.allow(&key);
                }
            });
            return;
        }
        tiles::Show::TooBig | tiles::Show::Load => {}
    }

    // A model in 3D fills the pane.
    if let Some(viewer) = opened.viewer.as_mut() {
        let line_h = theme.font_size_small * 2.6 + theme.spacing_sm;
        viewer.show_sized(ui, theme, area.x, (area.y - line_h - row_h).max(120.0));
        pressed.close_3d = widgets::Button::secondary(crate::gui::model_view::CLOSE).show(ui, theme);
        return;
    }

    if item.kind == "picture" {
        if tiles::too_big(&item) {
            centred(ui, theme, |ui| {
                widgets::body_hint(ui, theme, &format!("This picture is {}, larger than this app shows in place. Download it to see it.", api::size_words(item.size_bytes)));
                if widgets::Button::secondary("Download").tooltip("Save it in your downloads folder.").show(ui, theme) {
                    pressed.download = Some(files_of(&item)[0].clone());
                }
            });
            return;
        }
        tiles::request_picture(state, &item);
        // View full size at the foot, the picture filling what is above it.
        let ready = state.image_cache.get_texture(&key).is_some();
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
            if ready {
                pressed.view_full |= widgets::Button::secondary(VIEW_FULL).tooltip(VIEW_FULL_TIP).show(ui, theme);
                ui.add_space(theme.spacing_xs);
            }
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                if let Some(resp) = picture(ui, theme, state, &key, ui.available_size()) {
                    pressed.view_full |= resp.on_hover_text(VIEW_FULL_TIP).clicked();
                }
            });
        });
        return;
    }

    let cover = item.cover.is_some() && !tiles::cover_too_big(&item);
    let own = files_of(&item)[0].clone();
    let name = open::shown_file_name(&item, &own);
    let file_type = crate::net::file_risk::file_type(&name, None);
    if is_sound(&item) {
        // The cover above, when it has one; the waveform under it fills the rest.
        let wave_h = if cover { (area.y * 0.6).max(160.0) } else { area.y };
        if cover {
            tiles::request_cover(state, &item);
            let ckey = tiles::cover_key(state, &item);
            picture(ui, theme, state, &ckey, egui::vec2(area.x, area.y - wave_h - theme.spacing_sm));
            ui.add_space(theme.spacing_sm);
        }
        if !opened.sound.started() {
            // Before Play here: the pane says what it is and how long a fetch it is.
            ui.label(RichText::new(format!("{name}, {}", api::size_words(item.size_bytes))).size(theme.font_size_small).color(theme.text_secondary()));
        }
        pressed.play_sound = opened.sound.draw(ui, theme, egui::vec2(area.x, ui.available_height().min(wave_h)));
        return;
    }

    if item.kind == "video" && opened.video.has_player() {
        pressed.play = opened.video.draw_fit(ui, theme, ui.available_size());
        return;
    }

    // A video not playing yet, a model or any other file: its buttons at the foot, what it is above
    // them, and its cover (or its type's drawing) filling the rest.
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        if item.kind == "video" {
            pressed.play = opened.video.draw_fit(ui, theme, egui::vec2(area.x, row_h * 3.0));
        } else {
            centred_row(ui, egui::Id::new(("media_viewer_buttons", item.id)), |ui| {
                if crate::gui::model_view::load::viewable(&name) {
                    let tip = crate::gui::model_view::view_tip(&api::size_words(own.size_bytes));
                    if widgets::Button::primary(crate::gui::model_view::VIEW_IN_3D).tooltip(&tip).show(ui, theme) {
                        pressed.view_3d = Some(own.clone());
                    }
                }
                if opened.downloading(own.file) {
                    ui.label(RichText::new("Downloading...").size(theme.font_size_small).color(theme.text_muted()));
                } else if widgets::Button::secondary("Download").tooltip("Save it in your downloads folder.").show(ui, theme) {
                    pressed.download = Some(own.clone());
                }
            });
            if let Some(caution) = crate::net::file_risk::assess(&name, None) {
                widgets::file_caution::show(ui, theme, &caution);
            }
        }
        ui.label(RichText::new(format!("{name}, {}", api::size_words(own.size_bytes))).size(theme.font_size_small).color(theme.text_secondary()));
        ui.label(RichText::new(kind_words(&item)).size(theme.font_size_heading).color(theme.text_primary()).strong());
        ui.add_space(theme.spacing_sm);
        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
            if cover {
                tiles::request_cover(state, &item);
                let ckey = tiles::cover_key(state, &item);
                picture(ui, theme, state, &ckey, ui.available_size());
            } else {
                let room = ui.available_size();
                let (rect, _) = ui.allocate_exact_size(room, Sense::hover());
                let side = (room.y * 0.5).clamp(32.0, 180.0).min(room.x * 0.5);
                let mark = egui::Rect::from_center_size(rect.center(), Vec2::splat(side));
                if item.kind == "file" {
                    widgets::file_badge::paint_icon(ui.painter(), mark, &file_type.icon, widgets::file_badge::level_color(theme, file_type.level));
                } else {
                    tiles::paint_mark(ui.painter(), mark, kinds::mark(&item.kind), theme.text_secondary());
                }
            }
        });
    });
}

/// What an item is, in a word: its kind's name ("Picture", "3D model"), or, for a file, its type's
/// ("Sound", "Archive", "PDF") from the file-type list.
pub(crate) fn kind_words(item: &Item) -> String {
    if item.kind == "file" {
        let own = &files_of(item)[0];
        return crate::net::file_risk::file_type(&open::shown_file_name(item, own), None).label;
    }
    kinds::one_name(&item.kind)
}

/// A row of buttons centred across the pane, at the foot of a bottom-up layout (panes.rs
/// `foot_row`).
fn centred_row(ui: &mut egui::Ui, id: egui::Id, add: impl FnOnce(&mut egui::Ui)) {
    super::panes::foot_row(ui, id, true, add);
}

/// A pane's line while nothing is chosen (item_view.rs `CHOOSE_LINE` in the middle).
pub(crate) fn nothing_chosen(ui: &mut egui::Ui, theme: &Theme, line: &str) {
    centred(ui, theme, |ui| widgets::body_hint(ui, theme, line));
}

/// `add` a third of the way down the pane, centred across it.
fn centred(ui: &mut egui::Ui, theme: &Theme, add: impl FnOnce(&mut egui::Ui)) {
    let h = ui.available_height();
    ui.add_space((h * 0.33 - theme.spacing_lg).max(0.0));
    ui.vertical_centered(add);
}

/// The picture the image cache holds under `key`, as large as fits in `size` and centred; a line
/// while it loads, or when it could not (with Try again). The press area, when it was drawn.
fn picture(ui: &mut egui::Ui, theme: &Theme, state: &mut GuiState, key: &str, size: Vec2) -> Option<egui::Response> {
    let size = egui::vec2(size.x.max(1.0), size.y.max(theme.button_height));
    if let Some(tex) = state.image_cache.get_texture(key) {
        let [w, h] = tex.size();
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let fitted = list::fit(rect, w as f32 / h.max(1) as f32);
        egui::Image::new((tex.id(), fitted.size())).paint_at(ui, fitted);
        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ZoomIn);
        }
        return Some(resp);
    }
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    match state.image_cache.status(key) {
        ImageStatus::Failed(_) => {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::top_down(egui::Align::Center)));
            child.add_space((rect.height() * 0.4).max(0.0));
            child.label(RichText::new(tiles::COULD_NOT_LOAD).size(theme.font_size_small).color(theme.danger()));
            if widgets::Button::ghost("Try again").show(&mut child, theme) {
                state.image_cache.retry(key);
            }
        }
        _ => {
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "Loading...", egui::FontId::proportional(theme.font_size_small), theme.text_muted());
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
        }
    }
    None
}
