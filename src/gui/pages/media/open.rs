//! Opening a Media tile. A picture goes to the app's image viewer (widgets/image_cache_view.rs),
//! the one the chat uses. Anything else opens a card here: its name, who put it there, its size
//! and description, Download, and for a video, Play here, which fetches it to a temporary file
//! and plays it in the app's own media player (`crate::media::VideoPlayer`, the one in-world
//! screens use: WebM with AV1 video plays; anything else says why it cannot and can still be
//! downloaded). The video plays without sound for now: this page has no audio mixer to hand it.
//!
//! Fetches and downloads run on worker threads and stream to disk, so a large file never sits in
//! memory whole; a members-only item's file comes through the members' file route with this
//! session's upload token.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;

use egui::RichText;

use super::api::{self, Item};
use super::kinds;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The most a fetch or a download takes (the Media budget's own ceiling is far below).
const MAX_FETCH: u64 = 64 * 1024 * 1024 * 1024;

/// An opened item that is not a picture.
pub(crate) struct Opened {
    pub item: Item,
    /// A video being fetched to play here.
    fetch_rx: Option<Receiver<Result<PathBuf, String>>>,
    player: Option<crate::media::VideoPlayer>,
    texture: Option<egui::TextureHandle>,
    /// Why it cannot play, or the fetch failed.
    pub problem: String,
    save_rx: Option<Receiver<Result<PathBuf, String>>>,
    /// Where Download put it, or why it did not.
    pub saved: String,
}

impl Opened {
    pub(crate) fn new(item: Item) -> Self {
        Opened { item, fetch_rx: None, player: None, texture: None, problem: String::new(), save_rx: None, saved: String::new() }
    }
}

/// A tile was clicked: a picture opens in the image viewer, anything else in the card.
pub(crate) fn open_item(state: &mut GuiState, item: &Item) {
    if item.kind == "picture" {
        super::tiles::request_picture(state, item);
        state.image_viewer_url = Some(super::tiles::cache_key(state, item));
    } else {
        state.media.open = Some(Opened::new(item.clone()));
    }
}

/// Write what `url` serves into `dest`, streamed, refusing more than `max` bytes.
pub(crate) fn fetch_to(url: &str, dest: &Path, max: u64) -> Result<PathBuf, String> {
    let resp = ureq::get(url).timeout(std::time::Duration::from_secs(600)).call().map_err(|e| match e {
        ureq::Error::Status(code, r) => {
            let text = r.into_string().unwrap_or_default();
            if text.trim().is_empty() || text.trim_start().starts_with('<') { format!("The server refused it ({code}).") } else { text.trim().to_string() }
        }
        other => format!("The server could not be reached ({other})."),
    })?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("The folder could not be made ({e})."))?;
    }
    let mut file = std::fs::File::create(dest).map_err(|e| format!("The file could not be written ({e})."))?;
    let mut reader = resp.into_reader().take(max + 1);
    let n = std::io::copy(&mut reader, &mut file).map_err(|e| format!("The download stopped ({e})."))?;
    file.flush().ok();
    if n > max {
        drop(file);
        let _ = std::fs::remove_file(dest);
        return Err("The file is larger than this app fetches.".to_string());
    }
    Ok(dest.to_path_buf())
}

/// A file name safe to write: letters, digits, spaces, dots, dashes and underscores, at most 80
/// characters, never empty.
pub(crate) fn safe_name(name: &str) -> String {
    let cleaned: String =
        name.chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_') { c } else { '_' }).take(80).collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    if cleaned.is_empty() {
        "media".to_string()
    } else {
        cleaned
    }
}

/// A path in `dir` for `name` that does not exist yet: "x.png", "x (2).png" and so on.
pub(crate) fn free_path(dir: &Path, name: &str) -> PathBuf {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], &name[i..]),
        _ => (name, ""),
    };
    (1..=999)
        .map(|n| dir.join(if n == 1 { name.to_string() } else { format!("{stem} ({n}){ext}") }))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}-{}{ext}", api::next_timestamp())))
}

/// The name Download saves `item` under: its stored file name when public, else its title.
fn download_name(item: &Item) -> String {
    match item.upload_name() {
        Some(name) => safe_name(name),
        None => format!("{}-{}", safe_name(&item.display_title()), item.id),
    }
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The card of the opened item, while one is open.
pub(crate) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut opened) = state.media.open.take() else { return };
    let base = super::base_of(state);
    let token = crate::gui::pages::chat::upload_token_here(state);
    let url = api::file_url(&base, &opened.item, token.as_deref());

    // Finished fetches and downloads.
    if let Some(rx) = opened.fetch_rx.as_ref() {
        if let Ok(result) = rx.try_recv() {
            opened.fetch_rx = None;
            match result.and_then(|path| crate::media::VideoPlayer::open(&path).map_err(|e| format!("This video cannot play in the app: {e}. Download it to watch it in another player."))) {
                Ok(mut player) => {
                    player.play();
                    opened.player = Some(player);
                }
                Err(e) => opened.problem = e,
            }
        }
    }
    if let Some(rx) = opened.save_rx.as_ref() {
        if let Ok(result) = rx.try_recv() {
            opened.save_rx = None;
            opened.saved = match result {
                Ok(path) => format!("Saved to {}", path.display()),
                Err(e) => format!("Not saved: {e}"),
            };
        }
    }
    if let Some(player) = opened.player.as_mut() {
        if let Some(frame) = player.poll() {
            let image = egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
            match opened.texture.as_mut() {
                Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                None => opened.texture = Some(ctx.load_texture("media_video_frame", image, egui::TextureOptions::LINEAR)),
            }
        }
        if let Some(e) = player.take_error() {
            opened.problem = format!("The video stopped: {e}");
        }
        if player.is_playing() {
            ctx.request_repaint();
        }
    }
    if opened.fetch_rx.is_some() || opened.save_rx.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    let mut open = true;
    let mut fetch = false;
    let mut save = false;
    let item = opened.item.clone();
    widgets::dialog_scrolling(ctx, theme, "media_open_item", &item.display_title(), &mut open, |ui| {
        ui.set_max_width(700.0);
        ui.label(
            RichText::new(format!("{} by {}, {}, in {}", kinds::one_name(&item.kind), item.by_name(), api::size_words(item.size_bytes), item.category_name))
                .size(theme.font_size_small)
                .color(theme.text_secondary()),
        );
        if !item.description.trim().is_empty() {
            ui.label(RichText::new(item.description.trim()).size(theme.font_size_body).color(theme.text_primary()));
        }
        ui.add_space(theme.spacing_sm);
        if item.kind == "video" {
            if let (Some(tex), Some(player)) = (opened.texture.as_ref(), opened.player.as_mut()) {
                let [w, h] = tex.size();
                let aspect = w.max(1) as f32 / h.max(1) as f32;
                let width = ui.available_width().min(640.0);
                ui.add(egui::Image::new((tex.id(), egui::vec2(width, width / aspect))));
                ui.horizontal(|ui| {
                    if player.is_playing() {
                        if widgets::Button::secondary("Pause").show(ui, theme) {
                            player.pause();
                        }
                    } else if widgets::Button::secondary("Play").show(ui, theme) {
                        if player.at_end() {
                            player.seek_to_start();
                        }
                        player.play();
                    }
                    if widgets::Button::ghost("Start again").show(ui, theme) {
                        player.seek_to_start();
                    }
                    ui.label(RichText::new(format!("{} of {}", clock(player.position_s()), clock(player.duration_s()))).size(theme.font_size_small).color(theme.text_muted()));
                });
                widgets::body_hint(ui, theme, "Plays without sound here for now. Download it to hear it.");
            } else if opened.fetch_rx.is_some() {
                ui.label(RichText::new("Fetching the video...").size(theme.font_size_small).color(theme.text_muted()));
            } else if opened.player.is_some() {
                ui.label(RichText::new("Starting...").size(theme.font_size_small).color(theme.text_muted()));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(100));
            } else if opened.problem.is_empty() {
                fetch = widgets::Button::primary("Play here").tooltip("Fetch the video and play it in the app.").show(ui, theme);
            }
        }
        if !opened.problem.is_empty() {
            ui.label(RichText::new(&opened.problem).size(theme.font_size_small).color(theme.danger()));
        }
        ui.add_space(theme.spacing_sm);
        ui.horizontal(|ui| {
            if opened.save_rx.is_some() {
                ui.label(RichText::new("Downloading...").size(theme.font_size_small).color(theme.text_muted()));
            } else {
                save = widgets::Button::secondary("Download").tooltip("Save it in your downloads folder.").show(ui, theme);
            }
        });
        if !opened.saved.is_empty() {
            let color = if opened.saved.starts_with("Saved") { theme.success() } else { theme.danger() };
            ui.label(RichText::new(&opened.saved).size(theme.font_size_small).color(color));
        }
    });

    if fetch {
        let h = blake3::hash(format!("{}\n{}", state.media.server, item.id).as_bytes());
        let dest = std::env::temp_dir().join("HumanityOS-media").join(format!("{}.video", &h.to_hex()[..24]));
        let (tx, rx) = std::sync::mpsc::channel();
        let url = url.clone();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_to(&url, &dest, MAX_FETCH));
        });
        opened.fetch_rx = Some(rx);
        opened.problem.clear();
    }
    if save {
        let dest = free_path(&crate::gui::widgets::image_cache::default_downloads_dir(), &download_name(&item));
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(fetch_to(&url, &dest, MAX_FETCH));
        });
        opened.save_rx = Some(rx);
        opened.saved.clear();
    }
    if open {
        state.media.open = Some(opened);
    }
}
