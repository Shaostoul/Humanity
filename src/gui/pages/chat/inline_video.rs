//! Videos posted in chat, played inside their message (2026-11-06). The operator, 2026-11-04, with a
//! screenshot of a `.mp4` card in #global: "Can we make the videos show in chat and playable in
//! chat?" The design: docs/design/media-player.md, "Videos inside chat messages".
//!
//! The card of a video on the message's own server (chat/public_files.rs) holds a player: the
//! picture (a black box with a Play mark until the video is first played, its last picture after),
//! a seek bar, Play or Pause, the time, Mute, the volume, and Full window, the window today's Play
//! here opened, kept as the bigger view of the same player. What it costs, by design:
//!
//! - **Nothing until Play.** Drawing a row fetches nothing and decodes nothing. The first Play
//!   fetches the whole file (the player opens a file) on the one road every video takes:
//!   `VideoView::start` hands the copy to `transcode::begin_ingest_as` (BUG-228), which plays it,
//!   converts it with the person's ffmpeg, or names it in plain words, drawn here in the row.
//! - **One video plays at a time.** The last video Play was pressed on ([`ChatVideos::current`]) is
//!   the only one allowed to play; [`ChatVideos::end_frame`] pauses any other, which also catches a
//!   video whose fetch finished after another was started.
//! - **A row off the screen keeps nothing running.** A player whose picture was not drawn inside the
//!   messages' view this frame or the last (and is not in Full window) is dropped at the end of the
//!   frame: its decoder, decode thread, sound and temporary copy go. What stays is a small still of
//!   the picture it showed and its place, so the row still shows it and Play carries on from there
//!   (fetching again). Switching page or channel drops them the same way.
//! - **Nothing is kept across runs.** The copies go with their players (the Media page's sweep at
//!   the next start takes any a crash left); the stills, places and words are memory only. A still
//!   is reuse of a picture, so it obeys the Dev page's "turn off all reuse" switch: with reuse off
//!   none is kept and the row shows its Play mark again. At most [`MAX_STILLS`] are kept.
//!
//! The still is drawn after the first press, never fetched before it: a still for a row nobody
//! pressed would need a range read of the MP4's index (often at the file's end), its first keyframe
//! and a decode, for every video row on screen.
//!
//! **Before Play, its poster picture** (rung 2, 2026-11-06; docs/design/media-player.md, "Rung 2:
//! the poster picture"): the app that posted the video made one small JPEG of it, which the relay
//! keeps beside the file (chat/video_poster.rs makes and fetches it). A row on screen asks for it
//! once ([`ChatVideos::want_poster`]), on a worker, and draws it under the Play mark; nothing else is
//! fetched and no player is made. A video from before (no picture: the server answers 404) keeps the
//! black box. The pictures are memory only: at most [`MAX_POSTERS`] while reuse is on; with the Dev
//! page's "turn off all reuse" switch, one is kept only while its row stays on screen. Once played,
//! the row's still (where it was left) takes the poster picture's place.
//!
//! Videos are known by their address (the card's resolved URL): the same video posted twice shows
//! one player in both rows. Takes `use super::*` like the page's other children; the website's twin
//! is the browser's own `<video>` in web/chat/app.js `fileCardHtml`.

use super::*;
use std::collections::{HashMap, VecDeque};
#[cfg(test)]
use std::path::PathBuf;

use humanity_core::i18n::{t, tf};

use crate::gui::widgets::hint::Hint;
use crate::gui::widgets::video_view::{self, TrackReport, VideoView};

/// The widest a player is drawn in its message, in points.
pub(crate) const INLINE_MAX_WIDTH: f32 = 560.0;
/// The shape of the picture box before the video has said its own (16:9).
const FIRST_ASPECT: f32 = 16.0 / 9.0;
/// How wide a still is kept, in pixels (a little under the row's widest at 100% Font Size).
const STILL_WIDTH: usize = 480;
/// How many stills are kept; the oldest goes first.
pub(crate) const MAX_STILLS: usize = 24;
/// How wide the volume bar is, in points.
const VOLUME_WIDTH: f32 = 72.0;
/// How many poster pictures are kept while reuse is on; the one seen longest ago goes first.
pub(crate) const MAX_POSTERS: usize = 48;

/// Where a video's poster picture is: being fetched, here, or not to be had (a video from before
/// poster pictures, a server that refused it, an address that is not one).
enum PosterState {
    Asking,
    Ready(egui::TextureHandle),
    Missing,
}

/// A video's poster picture and the frame its row was last on screen.
struct Poster {
    state: PosterState,
    seen: u64,
}

/// What a poster picture's worker sends back: the video's address, then the picture (its width,
/// height and RGBA pixels) or why there is none.
type PosterResult = (String, Result<(u32, u32, Vec<u8>), String>);

/// What fetches a poster picture in place of the network (tests only): its address in, its bytes out.
#[cfg(test)]
pub(crate) type PosterFetchForTest = std::sync::Arc<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>;

/// A video with a player: fetching, playing, paused or refused.
struct Live {
    name: String,
    view: VideoView,
    /// Settings > Media's ffmpeg when Play was pressed (for a conversion).
    ffmpeg: String,
    /// The frame its picture was last drawn on screen (its row inside the view, or its window), as
    /// [`ChatVideos::frame`] counts them.
    seen: u64,
    /// The frame it was last polled, so a video drawn twice in one frame is polled once.
    polled: u64,
}

/// What a video's row shows once its player has gone: the picture it showed and where it was.
struct Still {
    texture: egui::TextureHandle,
    place_s: f64,
    length_s: f64,
}

/// What fetches a video in place of the network (tests only): its address in, the temporary copy out.
#[cfg(test)]
pub(crate) type FetchForTest = std::sync::Arc<dyn Fn(&str) -> Result<PathBuf, String> + Send + Sync>;

/// The chat's videos (`GuiState::chat_video`): the players of the rows on screen, at most one of them
/// playing, the stills and words of the ones that went, and the one in Full window.
pub(crate) struct ChatVideos {
    live: HashMap<String, Live>,
    stills: HashMap<String, Still>,
    still_order: VecDeque<String>,
    /// What a video's last try said when it could not play, kept after its player went.
    problems: HashMap<String, String>,
    /// The video Play was last pressed on: the only one allowed to play.
    current: Option<String>,
    /// The video in Full window.
    window: Option<String>,
    /// How many frames have ended ([`ChatVideos::end_frame`]): the clock "on screen this frame or
    /// the last" is told by. Its own count, not egui's pass number, because a wall screen showing
    /// the chat draws it under a context of its own (gui/screen_surface.rs, with a set of its own).
    frame: u64,
    /// The chat videos' volume (0 to 1, on top of the app's) and Mute: one for them all (one plays at
    /// a time), Mute for this run. A video starting fresh starts at Settings > Audio's start volume
    /// (humanity_core::audio::start_volume: my last volume, or a set one), and a moved volume bar is
    /// the last volume, kept across restarts (2026-11-06).
    pub volume: f32,
    pub muted: bool,
    /// The "Volume settings" mark beside the volume bar was pressed: Settings > Audio opens
    /// (`chat::follow_volume_settings_link`, which has the app's state).
    pub(crate) open_volume_settings: bool,
    /// The poster pictures asked for, by the video's address (rung 2: memory only, under the reuse
    /// switch), and the channel their workers answer on.
    posters: HashMap<String, Poster>,
    poster_tx: std::sync::mpsc::Sender<PosterResult>,
    poster_rx: std::sync::mpsc::Receiver<PosterResult>,
    #[cfg(test)]
    pub(crate) fetch_for_test: Option<FetchForTest>,
    /// Fetches a poster picture in place of the network (tests and snapshots).
    #[cfg(test)]
    pub(crate) poster_fetch_for_test: Option<PosterFetchForTest>,
    /// Every poster picture address asked for, in order (tests: each is asked once).
    #[cfg(test)]
    pub(crate) poster_fetches: Vec<String>,
    /// The reuse switch as a test sets it, in place of the Dev page's (which is the whole process's).
    #[cfg(test)]
    pub(crate) reuse_for_test: Option<bool>,
    /// How many fetches were started (tests: nothing is fetched before Play).
    #[cfg(test)]
    pub(crate) fetches: usize,
    /// Where each video's picture was last drawn (tests press it).
    #[cfg(test)]
    pub(crate) pictures: HashMap<String, egui::Rect>,
}

impl Default for ChatVideos {
    fn default() -> Self {
        let (poster_tx, poster_rx) = std::sync::mpsc::channel();
        ChatVideos {
            live: HashMap::new(),
            stills: HashMap::new(),
            still_order: VecDeque::new(),
            problems: HashMap::new(),
            current: None,
            window: None,
            frame: 0,
            volume: 1.0,
            muted: false,
            open_volume_settings: false,
            posters: HashMap::new(),
            poster_tx,
            poster_rx,
            #[cfg(test)]
            fetch_for_test: None,
            #[cfg(test)]
            poster_fetch_for_test: None,
            #[cfg(test)]
            poster_fetches: Vec::new(),
            #[cfg(test)]
            reuse_for_test: None,
            #[cfg(test)]
            fetches: 0,
            #[cfg(test)]
            pictures: HashMap::new(),
        }
    }
}

impl ChatVideos {
    /// Whether a video is in Full window (the chat's popups stay shut under it).
    pub(crate) fn window_open(&self) -> bool {
        self.window.is_some()
    }

    /// How many videos have a player now (fetching, playing, paused or refused).
    #[cfg(test)]
    pub(crate) fn live_count(&self) -> usize {
        self.live.len()
    }

    /// The address of the video playing, if one is.
    #[cfg(test)]
    pub(crate) fn playing(&self) -> Option<&str> {
        self.live.iter().find(|(_, l)| l.view.is_playing()).map(|(k, _)| k.as_str())
    }

    /// Whether the video at `url` has a player and it is playing.
    #[cfg(test)]
    pub(crate) fn is_playing(&self, url: &str) -> bool {
        self.live.get(url).is_some_and(|l| l.view.is_playing())
    }

    /// Whether the video at `url` has a player (paused, playing, fetching or refused).
    #[cfg(test)]
    pub(crate) fn has_player(&self, url: &str) -> bool {
        self.live.get(url).is_some()
    }

    /// The texture of the still kept for `url`.
    #[cfg(test)]
    pub(crate) fn still_texture(&self, url: &str) -> Option<egui::TextureId> {
        self.stills.get(url).map(|s| s.texture.id())
    }

    /// The texture of the poster picture kept for the video at `url`, once it is here.
    #[cfg(test)]
    pub(crate) fn poster_texture(&self, url: &str) -> Option<egui::TextureId> {
        match self.posters.get(url).map(|p| &p.state) {
            Some(PosterState::Ready(t)) => Some(t.id()),
            _ => None,
        }
    }

    /// Whether a poster picture is known to be missing for the video at `url` (asked, none came).
    #[cfg(test)]
    pub(crate) fn poster_missing(&self, url: &str) -> bool {
        matches!(self.posters.get(url).map(|p| &p.state), Some(PosterState::Missing))
    }

    /// How many poster pictures (or their answers) are kept.
    #[cfg(test)]
    pub(crate) fn posters_kept(&self) -> usize {
        self.posters.len()
    }

    /// Take in poster pictures (10 ms apart, at most 10 s) until none is still on its way: for a
    /// snapshot, which must draw a settled frame.
    #[cfg(test)]
    pub(crate) fn wait_for_posters(&mut self, ctx: &egui::Context) {
        let started = std::time::Instant::now();
        while self.posters.values().any(|p| matches!(p.state, PosterState::Asking)) && started.elapsed() < std::time::Duration::from_secs(10) {
            self.take_posters(ctx);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Whether the video at `url` has a player with a picture come (its first frame shown).
    #[cfg(test)]
    pub(crate) fn shows_picture(&self, url: &str) -> bool {
        self.live.get(url).is_some_and(|l| l.view.texture().is_some())
    }

    /// The player of the video at `url`, once it has one.
    #[cfg(test)]
    pub(crate) fn player(&self, url: &str) -> Option<&crate::media::VideoPlayer> {
        self.live.get(url).and_then(|l| l.view.player.as_ref())
    }

    /// Poll the video at `url` (10 ms apart, at most 30 s) until it shows a picture or says why it
    /// cannot play: for a snapshot, which must draw a settled frame. True once it has.
    #[cfg(test)]
    pub(crate) fn wait_for_picture_or_words(&mut self, url: &str, ctx: &egui::Context) -> bool {
        let started = std::time::Instant::now();
        while started.elapsed() < std::time::Duration::from_secs(30) {
            let Some(live) = self.live.get_mut(url) else { return false };
            live.view.poll(ctx);
            if live.view.texture().is_some() || !live.view.problem.is_empty() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        false
    }

    /// The temporary copy the video at `url` plays from.
    #[cfg(test)]
    pub(crate) fn temp_copy(&self, url: &str) -> Option<PathBuf> {
        self.live.get(url).and_then(|l| l.view.temp.clone())
    }

    /// Play or Pause on the video at `url`, named `name`: play it (fetching it first if it has no
    /// player, and carrying on from its still's place), or pause it if it plays. It becomes the one
    /// allowed to play. A video that could not play tries again.
    pub(crate) fn press_play(&mut self, url: &str, name: &str, ffmpeg: &str) {
        let refused = self.live.get(url).is_some_and(|l| !l.view.has_player() && !l.view.fetching() && l.view.converting().is_none());
        if refused {
            self.live.remove(url);
        }
        match self.live.get_mut(url) {
            Some(live) if live.view.is_playing() => live.view.pause(),
            Some(live) => {
                live.view.play();
                self.current = Some(url.to_string());
            }
            None => {
                self.start(url, name, ffmpeg);
                self.current = Some(url.to_string());
            }
        }
    }

    /// Fetch the video at `url` on a worker and play it once it is here (`VideoView::start`). It
    /// counts as on screen this frame (its row was just drawn, or its window opened).
    fn start(&mut self, url: &str, name: &str, ffmpeg: &str) {
        let frame = self.frame;
        self.problems.remove(url);
        let mut view = VideoView::default();
        view.start_at = self.stills.get(url).map_or(0.0, |s| s.place_s);
        // A video starting fresh starts at Settings > Audio's start volume (2026-11-06).
        self.volume = humanity_core::audio::start_volume::start_volume();
        view.volume = self.volume;
        view.muted = self.muted;
        #[cfg(test)]
        {
            self.fetches += 1;
        }
        #[cfg(test)]
        if let Some(fetch) = self.fetch_for_test.clone() {
            let address = url.to_string();
            view.start(ffmpeg, name, move || fetch(&address));
            self.live.insert(url.to_string(), Live { name: name.to_string(), view, ffmpeg: ffmpeg.to_string(), seen: frame, polled: u64::MAX });
            return;
        }
        let (address, copy) = (url.to_string(), crate::gui::pages::files::open::play_name(url, 0));
        view.start(ffmpeg, name, move || {
            use crate::gui::pages::files::open::{fetch_to, temp_dir, transfer_agent};
            fetch_to(&transfer_agent(std::time::Duration::from_secs(60)), &address, None, &temp_dir(), &copy, super::public_files::MAX_PUBLIC_FILE)
        });
        self.live.insert(url.to_string(), Live { name: name.to_string(), view, ffmpeg: ffmpeg.to_string(), seen: frame, polled: u64::MAX });
    }

    /// The row of the video at `url` is on screen with nothing of the video to show (no player, no
    /// still): ask for its poster picture, once, on a worker (chat/video_poster.rs `fetch_poster`),
    /// and note the row was seen this frame. An address that is not one of `/uploads/<name>` (it
    /// never is, for a video that plays here) is noted missing without asking anything.
    pub(crate) fn want_poster(&mut self, url: &str) {
        let frame = self.frame;
        if let Some(p) = self.posters.get_mut(url) {
            p.seen = frame;
            return;
        }
        let Some(poster) = super::video_poster::poster_url(url) else {
            self.posters.insert(url.to_string(), Poster { state: PosterState::Missing, seen: frame });
            return;
        };
        self.posters.insert(url.to_string(), Poster { state: PosterState::Asking, seen: frame });
        let (tx, key) = (self.poster_tx.clone(), url.to_string());
        #[cfg(test)]
        {
            self.poster_fetches.push(poster.clone());
            if let Some(fetch) = self.poster_fetch_for_test.clone() {
                std::thread::spawn(move || {
                    let _ = tx.send((key, fetch(&poster).and_then(|bytes| super::video_poster::decode_poster(&bytes))));
                });
                return;
            }
        }
        let _ = std::thread::Builder::new().name("chat-video-poster".into()).spawn(move || {
            let _ = tx.send((key, super::video_poster::fetch_poster(&poster)));
        });
    }

    /// Take in the poster pictures the workers have finished: each made a texture (or noted missing,
    /// a video from before poster pictures). One whose row was let go meanwhile is dropped.
    fn take_posters(&mut self, ctx: &egui::Context) {
        while let Ok((url, result)) = self.poster_rx.try_recv() {
            let Some(p) = self.posters.get_mut(&url) else { continue };
            p.state = match result {
                Ok((w, h, rgba)) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
                    PosterState::Ready(ctx.load_texture(format!("chat_video_poster:{url}"), image, egui::TextureOptions::LINEAR))
                }
                Err(why) => {
                    log::debug!("No poster picture for {url}: {why}");
                    PosterState::Missing
                }
            };
        }
    }

    /// Let go of the poster pictures past what is kept (the end of each frame): with reuse turned off,
    /// every one whose row was not on screen this frame or the last (asked again when it comes back);
    /// with it on, the ones seen longest ago past [`MAX_POSTERS`], never one on screen now.
    fn trim_posters(&mut self) {
        let frame = self.frame;
        #[cfg(test)]
        let reuse = self.reuse_for_test.unwrap_or_else(humanity_core::reuse::enabled);
        #[cfg(not(test))]
        let reuse = humanity_core::reuse::enabled();
        if !reuse {
            self.posters.retain(|_, p| p.seen + 1 >= frame);
            return;
        }
        if self.posters.len() <= MAX_POSTERS {
            return;
        }
        let mut away: Vec<(u64, String)> = self.posters.iter().filter(|(_, p)| p.seen + 1 < frame).map(|(u, p)| (p.seen, u.clone())).collect();
        away.sort();
        let over = self.posters.len() - MAX_POSTERS;
        for (_, url) in away.into_iter().take(over) {
            self.posters.remove(&url);
        }
    }

    /// The video at `url` was drawn on screen this frame: poll it (once a frame) and keep it.
    fn on_screen(&mut self, url: &str, ctx: &egui::Context) {
        let frame = self.frame;
        if let Some(live) = self.live.get_mut(url) {
            live.seen = frame;
            if live.polled != frame {
                live.polled = frame;
                live.view.poll(ctx);
            }
        }
    }

    /// The end of each frame (the engine calls it through [`draw_video_window`], after the pages; a
    /// wall screen showing the chat, after its page): every video but the current one is paused, and
    /// every player whose picture was not on screen this frame or the last, and is not in Full
    /// window, is dropped, leaving its still and words.
    pub(crate) fn end_frame(&mut self, ctx: &egui::Context) {
        let current = self.current.clone();
        for (url, live) in self.live.iter_mut() {
            if Some(url) != current.as_ref() && live.view.is_playing() {
                live.view.pause();
            }
        }
        let frame = self.frame;
        let gone: Vec<String> = self.live.iter().filter(|(url, l)| l.seen + 1 < frame && self.window.as_deref() != Some(url.as_str())).map(|(url, _)| url.clone()).collect();
        for url in gone {
            self.forget(&url, ctx);
        }
        if self.window.as_ref().is_some_and(|w| !self.live.contains_key(w)) {
            self.window = None;
        }
        // The poster pictures: the ones that came are taken in, the ones past what is kept let go,
        // and while one is still on its way the next frame is asked for, so it shows when it comes.
        self.take_posters(ctx);
        self.trim_posters();
        if self.posters.values().any(|p| matches!(p.state, PosterState::Asking)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.frame += 1;
    }

    /// Drop the player of the video at `url` (its decoder, sound and temporary copy go with it),
    /// keeping a small still of its last picture and its place (unless reuse is turned off) and the
    /// words of a video that could not play.
    pub(crate) fn forget(&mut self, url: &str, ctx: &egui::Context) {
        let Some(live) = self.live.remove(url) else { return };
        if !live.view.problem.is_empty() {
            self.problems.insert(url.to_string(), live.view.problem.clone());
        }
        if humanity_core::reuse::enabled() {
            if let Some(image) = live.view.still_image(STILL_WIDTH) {
                let (place_s, length_s) = live.view.place();
                let texture = ctx.load_texture(format!("chat_video_still:{url}"), image, egui::TextureOptions::LINEAR);
                self.stills.insert(url.to_string(), Still { texture, place_s, length_s });
                self.still_order.retain(|u| u != url);
                self.still_order.push_back(url.to_string());
                while self.still_order.len() > MAX_STILLS {
                    if let Some(old) = self.still_order.pop_front() {
                        self.stills.remove(&old);
                    }
                }
            }
        } else {
            self.stills.clear();
            self.still_order.clear();
        }
        // `live` drops here: the player lets go of the file, then the copy goes (`Drop for VideoView`).
    }

    /// Their sound through the app's mixer, at the chat videos' volume (each frame it has a mixer).
    pub(crate) fn sync_sound(&mut self, audio: &mut humanity_core::audio::AudioManager) {
        for live in self.live.values_mut() {
            live.view.volume = self.volume;
            live.view.muted = self.muted;
            live.view.sync_sound(audio);
        }
    }
}

/// What a player's controls asked for this frame.
#[derive(Default)]
struct Asked {
    play: bool,
    seek: Option<f32>,
    mute: bool,
    volume: Option<f32>,
    window: bool,
}

/// The player in a video's card: the picture, then the seek bar, then Play or Pause, the time, Mute,
/// the volume and Full window, then the words of a video that cannot play. `ffmpeg` is Settings >
/// Media's, for a Play that has to convert.
pub(crate) fn draw_in_card(ui: &mut egui::Ui, theme: &Theme, videos: &mut ChatVideos, url: &str, name: &str, ffmpeg: &str) {
    ui.push_id(("chat_video", url), |ui| {
        draw_player(ui, theme, videos, url, name, ffmpeg, egui::vec2(INLINE_MAX_WIDTH, f32::INFINITY), false);
    });
}

/// The player itself, at most `max` (its picture fitted inside, its controls under it); `in_window`
/// for Full window's bigger view (no Full window button there, and it is always on screen).
#[allow(clippy::too_many_arguments)]
fn draw_player(ui: &mut egui::Ui, theme: &Theme, videos: &mut ChatVideos, url: &str, name: &str, ffmpeg: &str, max: egui::Vec2, in_window: bool) {
    let ctx = ui.ctx().clone();
    let elsewhere = !in_window && videos.window.as_deref() == Some(url);
    // The shape: the picture's own once one has come (the player's, the still's, or the poster
    // picture's), else 16:9.
    let aspect = {
        let size = videos.live.get(url).and_then(|l| l.view.texture()).map(|t| t.size()).or_else(|| videos.stills.get(url).map(|s| s.texture.size())).or_else(|| poster_of(videos, url).map(|t| t.size()));
        size.map_or(FIRST_ASPECT, |[w, h]| (w.max(1) as f32 / h.max(1) as f32).clamp(0.25, 4.0))
    };
    let mut width = if in_window { max.x } else { ui.available_width().min(max.x) }.max(160.0);
    let mut height = width / aspect;
    if height > max.y {
        height = max.y.max(90.0);
        width = height * aspect;
    }
    let picture = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover()).0;
    let press = ui.interact(picture, ui.make_persistent_id("picture"), egui::Sense::click()).hint("chat_video.picture_hint");
    #[cfg(test)]
    videos.pictures.insert(url.to_string(), picture);
    if (in_window || ui.is_rect_visible(picture)) && !elsewhere {
        videos.on_screen(url, &ctx);
        // Nothing of the video to show yet (never played here): its poster picture, asked for once.
        if !videos.live.contains_key(url) && !videos.stills.contains_key(url) {
            videos.want_poster(url);
        }
    }
    let mut asked = Asked { play: press.clicked() && !elsewhere, ..Default::default() };
    if ui.is_rect_visible(picture) {
        paint_picture(ui, theme, videos, url, picture, elsewhere);
    }
    if elsewhere {
        return;
    }

    // The place, the time and the state, from the player, else from the still.
    let live = videos.live.get(url);
    let (place, length) = match live.filter(|l| l.view.has_player()) {
        Some(l) => l.view.place(),
        None => videos.stills.get(url).map_or((0.0, 0.0), |s| (s.place_s, s.length_s)),
    };
    let playing = live.is_some_and(|l| l.view.is_playing());
    let can_seek = live.is_some_and(|l| l.view.has_player()) && length > 0.0;
    let fraction = if length > 0.0 { (place / length).clamp(0.0, 1.0) as f32 } else { 0.0 };
    asked.seek = video_view::track_bar(ui, theme, "seek", fraction, width, can_seek, TrackReport::OnRelease, "chat_video.seek_hint");
    let small = widgets::ButtonSize::Small;
    ui.allocate_ui_with_layout(egui::vec2(width, theme.button_height), egui::Layout::left_to_right(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = theme.spacing_sm;
        let (label, hint) = if playing { (t("chat_video.pause"), "chat_video.pause_hint") } else { (t("chat_video.play"), "chat_video.play_hint") };
        if widgets::Button::secondary(label).size(small).hint(hint).show(ui, theme) {
            asked.play = true;
        }
        let at = video_view::clock(place);
        let time = if length > 0.0 { tf("chat_video.time", &[("at", &at), ("length", &video_view::clock(length))]) } else { at };
        ui.label(RichText::new(time).size(theme.font_size_small).color(theme.text_muted()));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if !in_window && widgets::Button::ghost(t("chat_video.full_window")).size(small).hint("chat_video.full_window_hint").show(ui, theme) {
                asked.window = true;
            }
            // Settings > Audio's video volume choices, a small mark beside the bar (2026-11-06).
            if video_view::volume_settings_mark(ui, theme) {
                videos.open_volume_settings = true;
            }
            asked.volume = video_view::track_bar(ui, theme, "volume", videos.volume, VOLUME_WIDTH, !videos.muted, TrackReport::Live, "chat_video.volume_hint");
            if widgets::Button::ghost(if videos.muted { t("chat_video.unmute") } else { t("chat_video.mute") }).size(small).hint("chat_video.mute_hint").show(ui, theme) {
                asked.mute = true;
            }
        });
    });
    // A video nothing here plays, in BUG-228's plain words (the card's Download is beside its name).
    let problem = videos.live.get(url).map(|l| l.view.problem.clone()).filter(|p| !p.is_empty()).or_else(|| videos.problems.get(url).cloned());
    if let Some(words) = problem {
        ui.scope(|ui| {
            ui.set_max_width(width);
            ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(theme.danger())).wrap());
        });
    }
    if let Some(live) = videos.live.get(url) {
        if live.view.fetching() || live.view.converting().is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
    }

    // What was asked, in order: Play or Pause, a new place, Mute, the volume, Full window.
    if asked.play {
        videos.press_play(url, name, ffmpeg);
    }
    if let (Some(f), Some(live)) = (asked.seek, videos.live.get_mut(url)) {
        live.view.seek_to(f as f64 * length);
    }
    if asked.mute {
        videos.muted = !videos.muted;
    }
    if let Some(v) = asked.volume {
        videos.volume = v;
        // The last volume, which the next video starts at with "my last volume" (saved).
        humanity_core::audio::start_volume::note_volume(v);
    }
    if asked.window {
        if !videos.live.get(url).is_some_and(|l| l.view.is_playing()) {
            videos.press_play(url, name, ffmpeg);
        }
        videos.window = Some(url.to_string());
    }
}

/// The picture box: black, the picture (the player's, else the still) fitted in it, and over it the
/// Play mark while it is not playing, or what is happening (fetching, converting, starting, or that
/// it plays in its own window).
fn paint_picture(ui: &egui::Ui, theme: &Theme, videos: &ChatVideos, url: &str, rect: egui::Rect, elsewhere: bool) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, egui::CornerRadius::same(4), theme.bg_primary());
    let live = videos.live.get(url);
    // The player's picture, else the still of where it was left, else the poster picture made when it
    // was posted (rung 2; it stays while the first Play fetches), else nothing: the black box.
    let texture = live.and_then(|l| l.view.texture()).or_else(|| videos.stills.get(url).map(|s| &s.texture)).or_else(|| poster_of(videos, url));
    if let (Some(tex), false) = (texture, elsewhere) {
        let [w, h] = tex.size();
        let aspect = w.max(1) as f32 / h.max(1) as f32;
        let fit = if rect.width() / rect.height() > aspect { egui::vec2(rect.height() * aspect, rect.height()) } else { egui::vec2(rect.width(), rect.width() / aspect) };
        egui::Image::new((tex.id(), fit)).paint_at(ui, egui::Rect::from_center_size(rect.center(), fit));
    }
    let words = if elsewhere {
        Some(t("chat_video.in_window").to_string())
    } else if let Some(live) = live {
        if live.view.fetching() {
            Some(t("chat_video.fetching").to_string())
        } else if let Some(pct) = live.view.converting() {
            Some(tf("video.converting", &[("pct", &format!("{pct:.0}"))]))
        } else if live.view.has_player() && live.view.texture().is_none() {
            Some(t("chat_video.starting").to_string())
        } else {
            None
        }
    } else if texture.is_none() {
        Some(t("chat_video.press_play").to_string())
    } else {
        None
    };
    let playing = live.is_some_and(|l| l.view.is_playing());
    let busy = live.is_some_and(|l| l.view.fetching() || l.view.converting().is_some());
    let r = (rect.height() * 0.12).clamp(14.0, 32.0);
    if !playing && !busy && !elsewhere {
        // The Play mark: a round button with a triangle pointing right, drawn (no glyph).
        let c = rect.center();
        painter.circle_filled(c, r, theme.bg_card().gamma_multiply(0.85));
        painter.circle_stroke(c, r, egui::Stroke::new(1.0_f32, theme.border()));
        let s = r * 0.45;
        let tri = vec![egui::pos2(c.x - s * 0.6, c.y - s), egui::pos2(c.x - s * 0.6, c.y + s), egui::pos2(c.x + s, c.y)];
        painter.add(egui::Shape::convex_polygon(tri, theme.text_primary(), egui::Stroke::NONE));
    }
    if let Some(words) = words {
        let y = if !playing && !busy && !elsewhere { rect.center().y + r + theme.spacing_sm } else { rect.center().y };
        let font = egui::FontId::proportional(theme.font_size_small);
        let galley = painter.layout(words, font, theme.text_secondary(), (rect.width() - 2.0 * theme.spacing_md).max(40.0));
        let at = egui::pos2(rect.center().x - galley.size().x / 2.0, y);
        let back = egui::Rect::from_min_size(at, galley.size()).expand(theme.spacing_xs);
        painter.rect_filled(back, egui::CornerRadius::same(3), theme.bg_primary().gamma_multiply(0.7));
        painter.galley(at, galley, theme.text_secondary());
    }
}

/// The poster picture of the video at `url`, once it is here.
fn poster_of<'a>(videos: &'a ChatVideos, url: &str) -> Option<&'a egui::TextureHandle> {
    match videos.posters.get(url).map(|p| &p.state) {
        Some(PosterState::Ready(t)) => Some(t),
        _ => None,
    }
}

/// Full window: the video Full window was pressed on, bigger, in a window of its own with Close the
/// window (which puts it back in its message). Then the end of the frame for every chat video
/// ([`ChatVideos::end_frame`]). The engine calls this every frame, whatever page is open.
pub(crate) fn draw_video_window(ctx: &egui::Context, theme: &Theme, videos: &mut ChatVideos) {
    if let Some(url) = videos.window.clone() {
        if let Some((name, ffmpeg)) = videos.live.get(&url).map(|l| (l.name.clone(), l.ffmpeg.clone())) {
            let mut open = true;
            let mut close = false;
            let screen = ctx.screen_rect();
            let max = egui::vec2((screen.width() * 0.8).max(320.0), (screen.height() * 0.72).max(200.0));
            widgets::dialog(ctx, theme, "chat_video_view", &name, &mut open, |ui| {
                draw_player(ui, theme, videos, &url, &name, &ffmpeg, max, true);
                close = widgets::Button::secondary(t("chat_video.close_window")).hint("chat_video.close_window_hint").show(ui, theme);
            });
            if !open || close {
                videos.window = None;
            }
        } else {
            videos.window = None;
        }
    }
    videos.end_frame(ctx);
}

/// The chat's videos get their sound from the app's mixer (the engine calls this each frame it has
/// a mixer).
pub(crate) fn sync_video_sound(videos: &mut ChatVideos, audio: &mut humanity_core::audio::AudioManager) {
    videos.sync_sound(audio);
}

#[cfg(test)]
#[path = "inline_video_tests.rs"]
mod tests;
