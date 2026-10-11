//! Opening a Media tile. A public picture the image cache can show goes to the app's image viewer
//! (widgets/image_cache_view.rs), the one the chat uses. Anything else opens a card here: its
//! name, who put it there, its size and description, Download, and for a video, Play here, which
//! fetches it to a temporary file and plays it in the app's own media player
//! (`crate::media::VideoPlayer`, the one in-world screens use: WebM with AV1 video plays; anything
//! else says why it cannot and can still be downloaded). The video plays without sound for now:
//! this page has no audio mixer to hand it. A members-only picture opens the card too (with the
//! picture in it and View full size), because the viewer offers no Download for a file fetched
//! with the session's token; and so does a picture too large for the image cache, which Download
//! takes whatever its size.
//!
//! FETCHES AND DOWNLOADS run on worker threads and stream to disk, so a large file never sits in
//! memory whole. A members-only item's file is fetched with a ticket (ticket.rs, the relay's R3-8,
//! round 3): the session's upload token goes once, in a header, to ask for it, and the transfer
//! itself carries no token at all. There is no limit on a whole transfer's time, only on a
//! connection that will not open or goes quiet ([`transfer_agent`]): a 2 GB video takes longer
//! than any fixed limit on a slow line.
//!
//! NOTHING HAS ITS REAL NAME UNTIL IT IS COMPLETE (round 3, the review's findings 6 and 7): a
//! transfer is written to a hidden part file (widgets/part_file.rs) and named only when the last
//! byte is in, by claiming a free name, never by checking one and then creating it. A failed or
//! stopped transfer leaves nothing; one the app's exit cuts off is deleted at the exit, or at the
//! next start.
//!
//! TEMPORARY FILES: Play here's copy lives in the system's temporary folder
//! (`HumanityOS-media`) under a name of its own for each fetch ([`play_name`]), and is deleted when
//! its card closes (`Drop for Opened`), when the app exits (engine/quit.rs calls
//! [`forget_temp_files`]), and, for whatever a run that did not close cleanly left, once at the
//! start of the next ([`sweep_temp_once`]). The app's player opens a file, so Play here fetches the
//! whole video before it plays.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;

use egui::RichText;

use super::api::{self, Item, ItemRef};
use super::kinds;
use crate::gui::widgets::part_file::Part;
use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::gui::GuiState;

/// The most a fetch or a download takes (the Media budget's own ceiling is far below).
const MAX_FETCH: u64 = 64 * 1024 * 1024 * 1024;

/// The folder Play here's copies go in.
pub(crate) fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("HumanityOS-media")
}

/// An opened item: its card.
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
    /// The temporary copy Play here fetched, deleted with the card.
    pub temp: Option<PathBuf>,
}

impl Opened {
    pub(crate) fn new(item: Item) -> Self {
        Opened { item, fetch_rx: None, player: None, texture: None, problem: String::new(), save_rx: None, saved: String::new(), temp: None }
    }
}

impl Drop for Opened {
    /// The card closed (or the page went): the player lets go of the file first, then the
    /// temporary copy goes.
    fn drop(&mut self) {
        self.player = None;
        if let Some(path) = self.temp.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("Media: a temporary video could not be deleted yet ({e}); it goes at the next start.");
                }
            }
        }
    }
}

/// Delete everything in the temporary folder, once per run (the first frame): what a run that did
/// not close cleanly left there.
pub(crate) fn sweep_temp_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static DONE: AtomicBool = AtomicBool::new(false);
    if !DONE.swap(true, Ordering::SeqCst) {
        sweep(&temp_dir());
    }
}

/// Delete every file in `dir` (best effort: a file another program still holds stays).
pub(crate) fn sweep(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// The app is exiting (engine/quit.rs): close the card, whose copy goes with it, clear the folder,
/// and delete every part file still being written (a download or a Save cut off by the exit).
pub(crate) fn forget_temp_files(state: &mut GuiState) {
    state.media.open = None;
    sweep(&temp_dir());
    crate::gui::widgets::part_file::sweep_own();
}

/// The name of one Play here copy: the server and item it is from, then this process and a number
/// no other fetch in this run has had (the review's finding 7: one name per item let two fetches
/// of it write into the same file).
pub(crate) fn play_name(server: &str, id: i64) -> String {
    let h = blake3::hash(format!("{server}\n{id}").as_bytes());
    format!("{}-{}-{}.video", &h.to_hex()[..16], std::process::id(), super::next_id())
}

/// Where a transfer of an item's file comes from: a public address, or, for a members-only item,
/// a ticket asked for on the worker (ticket.rs).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Origin {
    Address(String),
    Ticket { base: String, token: String, item: ItemRef },
}

impl Origin {
    /// The address to fetch, asking for a ticket first when it needs one. Blocking.
    pub(crate) fn address(&self) -> Result<String, String> {
        match self {
            Origin::Address(url) => Ok(url.clone()),
            Origin::Ticket { base, token, item } => super::ticket::ask(base, token, item),
        }
    }
}

/// Where `item`'s file comes from on the server at `base`, with this session's upload `token`.
pub(crate) fn origin_of(base: &str, item: &Item, token: Option<&str>) -> Result<Origin, String> {
    if !item.members_only() {
        return Ok(Origin::Address(api::file_url(base, item)));
    }
    let token = token.ok_or_else(|| super::SIGN_IN_LINE.to_string())?;
    Ok(Origin::Ticket { base: base.to_string(), token: token.to_string(), item: item.reference()? })
}

/// A tile was clicked: a public picture the image cache can show opens in the image viewer;
/// anything else opens its card.
pub(crate) fn open_item(state: &mut GuiState, item: &Item) {
    if item.kind == "picture" && !item.members_only() && !super::tiles::too_big(item) {
        super::tiles::request_picture(state, item);
        state.image_viewer_url = Some(super::tiles::cache_key(state, item));
    } else {
        open_card(state, item);
    }
}

/// Open `item`'s card.
pub(crate) fn open_card(state: &mut GuiState, item: &Item) {
    if item.kind == "picture" {
        super::tiles::request_picture(state, item);
    }
    state.media.open = Some(Opened::new(item.clone()));
}

/// A client for whole files: no limit on a whole transfer's time (a big file on a slow line
/// takes as long as it takes), only on a connection that will not open or goes quiet for `idle`.
pub(crate) fn transfer_agent(idle: Duration) -> ureq::Agent {
    ureq::AgentBuilder::new().timeout_connect(Duration::from_secs(30)).timeout_read(idle).timeout_write(idle).build()
}

/// Write what `url` serves into a file named `name` in `dir`, streamed, refusing more than `max`
/// bytes, with `header` on the request when one is given. It is written to a hidden part file and
/// gets its name (`name`, or "name (2)" and so on when that is taken, claimed, never checked and
/// then made) only when the last byte is in: a failure leaves nothing, and nothing ever has the
/// real name while it is incomplete. Returns where it is. The words never hold a token or ticket.
pub(crate) fn fetch_to(agent: &ureq::Agent, url: &str, header: Option<(&str, &str)>, dir: &Path, name: &str, max: u64) -> Result<PathBuf, String> {
    let mut req = agent.get(url);
    if let Some((h, value)) = header {
        req = req.set(h, value);
    }
    let resp = req.call().map_err(|e| {
        let words = match e {
            ureq::Error::Status(code, r) => {
                let text = r.into_string().unwrap_or_default();
                if text.trim().is_empty() || text.trim_start().starts_with('<') { format!("The server refused it ({code}).") } else { text.trim().to_string() }
            }
            other => format!("The server could not be reached ({other})."),
        };
        crate::gui::widgets::image_cache::hide_token(&words)
    })?;
    let mut part = Part::create(dir)?;
    let mut reader = resp.into_reader().take(max + 1);
    let n = std::io::copy(&mut reader, part.file()).map_err(|e| crate::gui::widgets::image_cache::hide_token(&format!("The download stopped ({e}).")))?;
    part.file().flush().map_err(|e| format!("The file could not be written ({e})."))?;
    if n > max {
        return Err("The file is larger than this app fetches.".to_string());
    }
    part.finish(dir, name)
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

/// The name Download saves `item` under: the uploader's own name for the file (the relay's A1,
/// so a members-only file keeps its extension), else its stored name, else its title and number
/// with the extension of its stored name when one is known.
pub(crate) fn download_name(item: &Item) -> String {
    if !item.original_name.trim().is_empty() {
        return safe_name(&item.original_name);
    }
    if !item.stored.trim().is_empty() {
        return safe_name(&item.stored);
    }
    format!("{}-{}", safe_name(&item.display_title()), item.id)
}

fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// The card of the opened item, while one is open.
pub(crate) fn draw(ctx: &egui::Context, theme: &Theme, state: &mut GuiState) {
    let Some(mut opened) = state.media.open.take() else { return };
    let base = super::base_of(state);
    let token = super::token_here(state);
    let origin = origin_of(&base, &opened.item, token.as_deref());

    // Finished fetches and downloads.
    if let Some(rx) = opened.fetch_rx.as_ref() {
        if let Ok(result) = rx.try_recv() {
            opened.fetch_rx = None;
            if let Ok(path) = &result {
                opened.temp = Some(path.clone()); // deleted with the card from here on
            }
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
        ctx.request_repaint_after(Duration::from_millis(200));
    }

    let mut open = true;
    let mut fetch = false;
    let mut save = false;
    let mut view = false;
    let item = opened.item.clone();
    let picture_key = super::tiles::cache_key(state, &item);
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
        if item.kind == "picture" {
            if super::tiles::too_big(&item) {
                widgets::body_hint(ui, theme, &format!("This picture is {}, larger than this app shows in place. Download it to see it.", api::size_words(item.size_bytes)));
            } else if let Some(tex) = state.image_cache.get_texture(&picture_key) {
                let [w, h] = tex.size();
                let aspect = w.max(1) as f32 / h.max(1) as f32;
                let width = ui.available_width().min(640.0);
                ui.add(egui::Image::new((tex.id(), egui::vec2(width, width / aspect))));
                view = widgets::Button::secondary("View full size").show(ui, theme);
            } else {
                ui.label(RichText::new("Loading...").size(theme.font_size_small).color(theme.text_muted()));
                ui.ctx().request_repaint_after(Duration::from_millis(200));
            }
        }
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
                ui.ctx().request_repaint_after(Duration::from_millis(100));
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
        match origin.clone() {
            Ok(origin) => {
                let name = play_name(&state.media.server, item.id);
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let fetched = origin.address().and_then(|url| fetch_to(&transfer_agent(Duration::from_secs(60)), &url, None, &temp_dir(), &name, MAX_FETCH));
                    // The card closed while it was fetching: nobody will play it, so it goes now.
                    if let Err(std::sync::mpsc::SendError(Ok(path))) = tx.send(fetched) {
                        let _ = std::fs::remove_file(path);
                    }
                });
                opened.fetch_rx = Some(rx);
                opened.problem.clear();
            }
            Err(words) => opened.problem = words,
        }
    }
    if save {
        match origin {
            Ok(origin) => {
                let (dir, name) = (crate::gui::widgets::image_cache::default_downloads_dir(), download_name(&item));
                let (tx, rx) = std::sync::mpsc::channel();
                std::thread::spawn(move || {
                    let _ = tx.send(origin.address().and_then(|url| fetch_to(&transfer_agent(Duration::from_secs(60)), &url, None, &dir, &name, MAX_FETCH)));
                });
                opened.save_rx = Some(rx);
                opened.saved.clear();
            }
            Err(words) => opened.saved = format!("Not saved: {words}"),
        }
    }
    if view {
        state.image_viewer_url = Some(picture_key);
    }
    if open {
        state.media.open = Some(opened);
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
