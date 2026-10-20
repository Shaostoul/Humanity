//! Files posted into a public channel, shown as a card (2026-10-17, the "video attach" bug).
//!
//! Both apps post a public channel's attachment as nothing but its address (`/uploads/<ms>_<name>`),
//! and this app turned only pictures into anything: a video, a song, a PDF, an archive or a 3D model
//! stayed a bare path, and a relative path was not even a link. Now each link `net::protected::
//! file_urls` finds (the kinds the website turns into a player or a card) becomes a card under the
//! message: its name (without the server's time prefix), its size, Download, and, once saved,
//! where it went with Show in folder. A word of caution sits on the card when the file is a kind
//! that deserves one (`net::file_risk`: an archive, a Blender file, ...), and once it is saved its
//! first bytes are checked too, so a file pretending to be a video is called out.
//!
//! A file on another website fetches nothing until Download is pressed (its size is not asked
//! either), and the card says that saving it shows that website this device's address, the way
//! pictures from other websites wait for a click. The protected setup's rule is applied before
//! this: a non-friend's files are left out of the message with one line in their place
//! (chat/protected.rs), so no card is drawn for them.
//!
//! A video plays in the computer's own player for now: the app's own player plays WebM video only
//! (the common MP4 formats are covered by patents, docs/reference/media-stance.md), and a "Play
//! here" in chat is a later step. A `.glb` or `.stl` from the message's own server gets View in 3D,
//! the Media page's viewer in a window of its own.
//!
//! Takes `use super::*` like the page's other children. The website's twin is the file card in
//! web/chat/app.js `formatBody`.

use super::*;
use crate::gui::widgets::image_cache::{filename_from_url, route_image, ImageCache, ImageRoute};
use crate::net::file_risk::{self, Caution};

/// Where the card sits under the message text.
const INDENT: f32 = 40.0;

/// The largest public file Download fetches. The relay's own limits are far below it; this bounds
/// a link to another website.
pub(crate) const MAX_PUBLIC_FILE: u64 = 1024 * 1024 * 1024;

/// The line when a Download did not work.
pub(crate) const NOT_SAVED: &str = "This file could not be downloaded.";

/// The line under a video the app cannot play itself, which plays in the computer's own player.
pub(crate) const VIDEO_ELSEWHERE: &str = "Download it to watch it in your computer's own player.";

/// Added under an MP4 video where the app cannot ask the computer's own decoders (not Windows, or a
/// Windows without them): why its own player does not play it.
pub(crate) const MP4_WHY: &str = "The app's own player plays WebM video only, because the common MP4 formats are covered by patents.";

/// A video posted in chat, playing in its own window (Play here on its card, 2026-10-18).
pub(crate) struct ChatVideo {
    pub name: String,
    pub view: crate::gui::widgets::video_view::VideoView,
}

impl ChatVideo {
    /// Play here on `card`: fetch it to a temporary copy (the Media page's folder, swept at start)
    /// on a worker, then play it.
    pub(crate) fn start(card: &Card) -> ChatVideo {
        let mut view = crate::gui::widgets::video_view::VideoView::default();
        let (url, copy) = (card.url.clone(), crate::gui::pages::media::open::play_name(&card.url, 0));
        view.start(move || {
            let agent = crate::gui::pages::media::open::transfer_agent(std::time::Duration::from_secs(60));
            crate::gui::pages::media::open::fetch_to(&agent, &url, None, &crate::gui::pages::media::open::temp_dir(), &copy, MAX_PUBLIC_FILE)
        });
        ChatVideo { name: card.name.clone(), view }
    }
}

/// Whether this computer can ask its own decoders for an MP4 (Windows with Media Foundation).
pub(crate) fn plays_mp4_here() -> bool {
    #[cfg(windows)]
    return crate::media::system::available();
    #[cfg(not(windows))]
    false
}

/// What a message links to that is drawn under it as cards: files posted into a public channel
/// (`files_in`) and items on the Media page of the server on screen (media/chat_card.rs).
pub(crate) struct Posted {
    pub files: Vec<String>,
    pub items: Vec<i64>,
    /// The server the message came through, whose Media items the cards are.
    pub server: String,
}

impl Posted {
    /// What `msg` links to (nothing when `skip`: a private file's message, or pictures left out).
    /// Media items only for a message of the server whose Media page this app holds, and not while
    /// a stranger's DM holds its links (chat/warnings.rs): the address stays a held link, the card
    /// coming once the person presses the link line's Open (the website does the same).
    pub(crate) fn of(state: &GuiState, msg: &ChatMessage, skip: bool) -> Posted {
        let server = if msg.server.is_empty() { state.server_url.clone() } else { msg.server.clone() };
        if skip {
            return Posted { files: Vec::new(), items: Vec::new(), server };
        }
        let media_here = crate::gui::pages::chat::norm_server_url(&server) == state.media.server;
        let held = crate::engine::warnings::links_held(state, msg);
        let items = if media_here && !held { crate::gui::pages::media::chat_card::item_links(&msg.content, &server) } else { Vec::new() };
        Posted { files: files_in(&msg.content), items, server }
    }

    /// `content` without what is drawn as cards.
    pub(crate) fn words_of(&self, content: &str) -> String {
        let text = if self.files.is_empty() { content.to_string() } else { crate::net::protected::strip_file_urls(content) };
        crate::gui::pages::media::chat_card::without_links(&text, &self.server, &self.items)
    }
}

/// The public files a message links to, in the order they appear.
pub(crate) fn files_in(text: &str) -> Vec<String> {
    crate::net::protected::file_urls(text)
}

/// The name a public file is shown and saved by: the last part of its address, without a query or
/// a fragment, with the server's `<ms>_` time prefix taken off (`1760000000000_WH40K_Marriah_Carey.mp4`
/// shows as `WH40K_Marriah_Carey.mp4`) and `%20` and the like turned back into what they spell.
pub(crate) fn shown_name(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or(url);
    let last = filename_from_url(path);
    let last = match last.split_once('_') {
        Some((ms, rest)) if !ms.is_empty() && ms.bytes().all(|b| b.is_ascii_digit()) && !rest.is_empty() && !rest.starts_with('.') => rest.to_string(),
        _ => last,
    };
    percent_decoded(&last)
}

/// `text` with each `%XX` turned back into its byte, where the result is still text.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_string())
}

/// One public file's card: the address it is fetched from, its name, and the other website it
/// sits on (None: the message's own server).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Card {
    pub url: String,
    pub name: String,
    pub other_site: Option<String>,
}

impl Card {
    /// The card for `raw` (as the message has it) in a message that came through `server_url`.
    pub(crate) fn of(raw: &str, server_url: &str) -> Card {
        let route = route_image(raw, server_url);
        let other_site = match &route {
            ImageRoute::Load(_) => None,
            ImageRoute::AskFirst { host, .. } => Some(host.clone()),
        };
        Card { url: route.url().to_string(), name: shown_name(raw), other_site }
    }

    /// The key its Download goes by in the image cache.
    pub(crate) fn key(&self) -> String {
        format!("public-file:{}", self.url)
    }

    /// Its file kind's ending, lower case (`mp4`), or empty.
    fn ext(&self) -> String {
        self.name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
    }

    /// Whether it is a video.
    pub(crate) fn is_video(&self) -> bool {
        matches!(self.ext().as_str(), "mp4" | "webm" | "mov" | "m4v" | "mkv")
    }

    /// Whether Play here is offered: a video on the message's own server that the app's player can
    /// try (WebM and Matroska always; MP4 where the computer's own decoders are there). A video on
    /// another website is never fetched before a click on Download.
    pub(crate) fn plays_here(&self) -> bool {
        self.other_site.is_none()
            && match self.ext().as_str() {
                "webm" | "mkv" => true,
                "mp4" | "mov" | "m4v" => plays_mp4_here(),
                _ => false,
            }
    }

    /// The line under a video the app does not offer to play, or None.
    pub(crate) fn video_line(&self) -> Option<String> {
        if self.plays_here() {
            return None;
        }
        match self.ext().as_str() {
            "mp4" | "mov" | "m4v" if !plays_mp4_here() => Some(format!("{VIDEO_ELSEWHERE} {MP4_WHY}")),
            "mp4" | "mov" | "m4v" | "webm" | "mkv" => Some(VIDEO_ELSEWHERE.to_string()),
            _ => None,
        }
    }

    /// Whether View in 3D is offered: a model the viewer reads, on the message's own server.
    pub(crate) fn viewable_in_3d(&self) -> bool {
        self.other_site.is_none() && crate::gui::model_view::load::viewable(&self.name)
    }

    /// Its type, for its badge: from what was saved once it is (its first bytes too), else by its name.
    pub(crate) fn file_type(&self, saved: Option<&std::path::Path>) -> file_risk::FileType {
        match saved {
            Some(path) => file_risk::file_type_saved(&self.name, path),
            None => file_risk::file_type(&self.name, None),
        }
    }

    /// The word of caution: from what was saved once it is (its first bytes too), else by its name.
    pub(crate) fn caution(&self, saved: Option<&std::path::Path>) -> Option<Caution> {
        match saved {
            Some(path) => file_risk::assess_saved(&self.name, path),
            None => file_risk::assess(&self.name, None),
        }
    }
}

/// What Download does, on its worker thread: fetch the file with `fetch` into a hidden part file in
/// `dir` (at most [`MAX_PUBLIC_FILE`], written as it arrives), then give it its name, made safe and
/// never over an existing one (`x (2).mp4` and so on). A Download that fails leaves nothing.
pub(crate) fn save_public<F>(url: &str, name: &str, dir: &std::path::Path, fetch: F) -> Result<std::path::PathBuf, String>
where
    F: FnOnce(&str, u64, &mut std::fs::File) -> Result<u64, String>,
{
    use crate::gui::widgets::part_file::Part;
    let mut part = Part::create(dir)?;
    fetch(url, MAX_PUBLIC_FILE, part.file())?;
    part.finish(dir, &super::attach_view::save_name(name))
}

/// What the card's buttons asked for this frame.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Pressed {
    pub download: bool,
    pub show: bool,
    pub view_3d: bool,
    pub play: bool,
}

/// Draw a card for each of `files` (as `files_in` found them) under a message that came through
/// `msg_server` (empty for an older row: then `own_server`, the one this app is connected to).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_all(ui: &mut egui::Ui, theme: &Theme, cache: &mut ImageCache, model_view: &mut Option<crate::gui::model_view::Viewer>, video: &mut Option<ChatVideo>, files: &[String], msg_server: &str, own_server: &str, row_bg: Color32) {
    let server = if msg_server.is_empty() { own_server } else { msg_server };
    for raw in files {
        draw(ui, theme, cache, model_view, video, &Card::of(raw, server), row_bg);
    }
}

/// Draw `card` under its message and do what its buttons asked. The size is asked for a file on
/// the message's own server only; a Download runs on the image cache's worker.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme, cache: &mut ImageCache, model_view: &mut Option<crate::gui::model_view::Viewer>, video: &mut Option<ChatVideo>, card: &Card, row_bg: Color32) {
    if card.other_site.is_none() {
        cache.request_size(&card.url);
    }
    let key = card.key();
    let pressed = draw_card(ui, theme, cache, card, row_bg);
    if pressed.download {
        let (url, name) = (card.url.clone(), card.name.clone());
        let dir = crate::gui::widgets::image_cache::default_downloads_dir();
        cache.save_with(&key, move || save_public(&url, &name, &dir, |url, max, file| crate::gui::widgets::image_cache::download_to(url, max, file)));
    }
    if pressed.show {
        if let Some(path) = cache.downloaded_path(&key) {
            crate::gui::widgets::file_caution::show_in_folder(path);
        }
    }
    if pressed.view_3d {
        let (url, size) = (card.url.clone(), cache.size_of(&card.url).map_or(0, |s| s as i64));
        *model_view = Some(crate::gui::model_view::Viewer::open(&card.name, size, move || crate::gui::model_view::fetch(&url, None)));
    }
    if pressed.play {
        // One video at a time: a new Play here replaces the one playing (its copy goes with it).
        *video = Some(ChatVideo::start(card));
    }
}

/// The card itself: name, size and Download on one line; under it the word of caution, the line
/// for another website, the video's line, and what Download did.
pub(crate) fn draw_card(ui: &mut egui::Ui, theme: &Theme, cache: &ImageCache, card: &Card, row_bg: Color32) -> Pressed {
    let key = card.key();
    let saved = cache.downloaded_path(&key).map(|p| p.to_path_buf());
    let mut pressed = Pressed::default();
    let small = widgets::ButtonSize::Small;
    let text = |s: &str, color: Color32| RichText::new(s).size(theme.font_size_small).color(color);
    Frame::NONE
        .fill(row_bg)
        .inner_margin(egui::Margin { left: INDENT as i8, right: 8, top: 2, bottom: 4 })
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            Frame::NONE
                .fill(theme.bg_card())
                .stroke(Stroke::new(1.0, theme.border()))
                .rounding(Rounding::same(4))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(theme.spacing_sm, theme.spacing_xs);
                    ui.horizontal_wrapped(|ui| {
                        // Its type first, always (widgets/file_badge.rs), from what is inside once saved.
                        widgets::file_badge::show(ui, theme, &card.file_type(saved.as_deref()));
                        ui.label(text(&card.name, theme.text_primary()));
                        if let Some(size) = cache.size_of(&card.url) {
                            ui.label(text(&super::attach_view::size_words(size), theme.text_muted()));
                        }
                        if let Some(host) = &card.other_site {
                            ui.label(text(&format!("on {host}"), theme.text_muted()));
                        }
                        if cache.is_saving(&key) {
                            ui.label(text("Downloading\u{2026}", theme.text_muted()));
                            // Keep drawing until the worker reports, or "Saved to" waits for the mouse.
                            ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
                        } else if widgets::Button::secondary("Download").size(small).tooltip("Save it in your downloads folder.").show(ui, theme) {
                            pressed.download = true;
                        }
                        if card.plays_here() && widgets::Button::secondary(crate::gui::widgets::video_view::PLAY_HERE).size(small).tooltip(crate::gui::widgets::video_view::PLAY_HERE_TIP).show(ui, theme) {
                            pressed.play = true;
                        }
                        if card.viewable_in_3d() {
                            let tip = crate::gui::model_view::view_tip(&cache.size_of(&card.url).map_or_else(|| "its size not known yet".to_string(), super::attach_view::size_words));
                            if widgets::Button::secondary(crate::gui::model_view::VIEW_IN_3D).size(small).tooltip(&tip).show(ui, theme) {
                                pressed.view_3d = true;
                            }
                        }
                    });
                    if let Some(caution) = card.caution(saved.as_deref()) {
                        widgets::file_caution::show(ui, theme, &caution);
                    }
                    if let Some(host) = &card.other_site {
                        ui.label(text(&format!("Saving this shows {host} your network address."), theme.text_muted()));
                    }
                    if let Some(line) = card.video_line() {
                        ui.add(egui::Label::new(text(&line, theme.text_muted())).wrap());
                    }
                    if cache.save_failed(&key) {
                        ui.label(text(NOT_SAVED, theme.danger()));
                    } else if let Some(path) = &saved {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(text(&format!("Saved to {}", path.display()), theme.success()));
                            if widgets::Button::secondary("Show in folder").size(small).tooltip("Open the folder it was saved in, with the file selected.").show(ui, theme) {
                                pressed.show = true;
                            }
                        });
                    }
                });
        });
    pressed
}

/// Post `text` in channel `channel` as if typed there and sent (Add to Media's "Also post in chat",
/// 2026-10-17): the chat's own send, with its guards and its routing, then the conversation that
/// was open is open again. False when the send did not go.
pub(crate) fn post_in(state: &mut GuiState, channel: &str, text: &str) -> bool {
    // A reply being written in the open conversation is not this post's, and stays as it was.
    let reply = state.chat_reply_to.take();
    let was = std::mem::replace(&mut state.chat_active_channel, channel.to_string());
    let sent = super::send_composed_content(state, text);
    state.chat_active_channel = was;
    state.chat_reply_to = reply;
    sent
}

/// The window a video posted in chat plays in (Play here on its card), with Close. Its sound comes
/// from the engine's audio step (`sync_video_sound`).
pub(crate) fn draw_video_window(ctx: &egui::Context, theme: &Theme, video: &mut Option<ChatVideo>) {
    let Some(playing) = video.as_mut() else { return };
    playing.view.poll(ctx);
    let mut open = true;
    let mut close = false;
    let title = playing.name.clone();
    widgets::dialog(ctx, theme, "chat_video_view", &title, &mut open, |ui| {
        playing.view.draw(ui, theme, 720.0);
        close = widgets::Button::secondary(crate::gui::widgets::video_view::CLOSE).show(ui, theme);
    });
    if !open || close {
        *video = None;
    }
}

/// The chat's video window gets its sound from the app's mixer (the engine calls this each frame
/// it has a mixer).
pub(crate) fn sync_video_sound(video: &mut Option<ChatVideo>, audio: &mut crate::audio::AudioManager) {
    if let Some(playing) = video.as_mut() {
        playing.view.sync_sound(audio);
    }
}

/// The window a 3D model posted in chat opens in (View in 3D on its card), with Close.
pub(crate) fn draw_model_window(ctx: &egui::Context, theme: &Theme, model_view: &mut Option<crate::gui::model_view::Viewer>) {
    let Some(viewer) = model_view.as_mut() else { return };
    let mut open = true;
    let mut close = false;
    let title = viewer.name.clone();
    widgets::dialog(ctx, theme, "chat_model_view", &title, &mut open, |ui| {
        let width = ui.available_width().clamp(320.0, 720.0);
        viewer.show(ui, theme, width);
        close = widgets::Button::secondary(crate::gui::model_view::CLOSE).show(ui, theme);
    });
    if !open || close {
        *model_view = None;
    }
}

#[cfg(test)]
#[path = "public_files_tests.rs"]
mod tests;
