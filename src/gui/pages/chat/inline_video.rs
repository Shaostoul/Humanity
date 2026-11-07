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
//! - **A row off the screen keeps nothing running, but the video playing goes into the dock.** A
//!   player whose picture was not drawn inside the messages' view this frame or the last (and is not
//!   in Full window or the dock) is dropped at the end of the frame: its decoder, decode thread,
//!   sound and temporary copy go. What stays is a small still of the picture it showed and its
//!   place, so the row still shows it and Play carries on from there (fetching again). The video
//!   PLAYING (or on its way to play) whose row leaves the view, by a scroll, a channel or server
//!   switched or the chat page left, moves into the dock instead, the same player
//!   ([`ChatVideos::end_frame`], rung 3, 2026-11-06; drawn by chat/video_dock.rs).
//! - **One chat video at a time across the app screen and every wall screen** (the operator,
//!   2026-11-06, "Agreed."): the set whose Play was pressed last holds the claim (`CLAIM`, a
//!   thread-local: every set is drawn on the app's one GUI thread), and every other set gives way
//!   ([`ChatVideos::yield_if_elsewhere`]): its video pauses and its dock closes.
//! - **Nothing is kept across runs.** The copies go with their players (the Media page's sweep at
//!   the next start takes any a crash left); the stills, places and words are memory only. A still
//!   is reuse of a picture, so it obeys the Dev page's "turn off all reuse" switch: with reuse off
//!   none is kept and the row shows its Play mark again. At most [`MAX_STILLS`] are kept.
//!
//! - **On a wall screen, Full window fills that screen (2026-11-06).** A wall screen showing the
//!   chat (gui/screen_surface.rs, its own set: [`ChatVideos::on_a_wall_screen`]) offers Full window
//!   like the app screen, and its window opens ON THE WALL: the screen's own run draws it under the
//!   screen's context (`screen_surface::draw_screen_full_views`), so the person at the wall closes
//!   it there, with the look ray and the aim assist. (BUG-258 had taken the button away, because the
//!   window was then drawn only for the app screen's set, where nobody at the wall could close it;
//!   the operator, 2026-11-06: "I figured the whole display would be dedicated to the whole page
//!   that we're on tailored to that display size/aspect ratio.") Its sound comes from where the
//!   screen is (`engine::screens::place_page_sound`): fading with distance, panned by its bearing,
//!   on the world-sound bus like a film on an in-world screen.
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
use crate::gui::widgets::video_view::{self, SoundPlace, TrackReport, VideoView};

/// The id of the Full window.
pub(crate) const WINDOW_ID: &str = "chat_video_view";

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
/// How long Go to message waits for its row to be drawn (a switched channel loads its history
/// first) before it gives up and leaves the dock as it is.
pub(crate) const GO_TO_WAIT: std::time::Duration = std::time::Duration::from_secs(15);

thread_local! {
    /// The set ([`ChatVideos::set_id`]) whose Play was pressed last, on this thread: the one chat
    /// video allowed to play across the app screen and every wall screen (2026-11-06). A
    /// thread-local, not a process-wide value: every set is drawn on the app's one GUI thread, and
    /// each test runs on a thread of its own, so no test sees another's claim.
    static CLAIM: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// The next set's id (0 is nobody's).
static NEXT_SET: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Where a chat row was drawn: the place on the Chat page it belongs to (the server and channel the
/// page showed, as the history keeps a chat step, `history::At::Chat`) and the words naming it for
/// the dock ("From #global on united-humanity.us").
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Origin {
    pub(crate) server: String,
    pub(crate) channel: String,
    pub(crate) label: String,
}

/// The dock (rung 3, 2026-11-06; chat/video_dock.rs draws it): the video in it, where it came from,
/// and how it shows.
pub(crate) struct Dock {
    pub(crate) url: String,
    pub(crate) name: String,
    pub(crate) ffmpeg: String,
    pub(crate) origin: Origin,
    /// Minimize was pressed: the slim tab, until the tab is pressed.
    pub(crate) minimized: bool,
    /// The frame its row was last drawn on screen (it is back in its message that frame); u64::MAX:
    /// never. Not 0: a dock made before the first frame ended (a snapshot's setup) would read frame
    /// 0 as its row back on screen and close unseen (2026-11-07).
    pub(crate) row_seen: u64,
    /// Where it stands on a page that is not the Chat page, decided as it arrived there.
    pub(crate) spot: Option<super::video_dock::Spot>,
    /// The person opened the tab on a page where the dock had no free spot: it shows anyway there.
    pub(crate) opened_on: Option<crate::gui::GuiPage>,
}

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
    /// The frame its row was last drawn on screen in a message (not its window, not the dock).
    inline_seen: u64,
    /// Where that row was (the Chat page's place), once one was drawn there: what the dock says.
    origin: Option<Origin>,
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
    /// A wall screen's set ([`ChatVideos::on_a_wall_screen`]): its videos never go into the dock,
    /// which is the person's, an app layer (rung 3); a wall screen's video plays in its row or in
    /// the Full window drawn on that wall.
    on_wall: bool,
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
    /// The dock (rung 3): the video playing whose row left the view, drawn by chat/video_dock.rs.
    pub(crate) dock: Option<Dock>,
    /// Where the rows drawn this frame belong and the message list's rectangle, as the Chat page
    /// noted them ([`ChatVideos::note_chat_view`]; the frame they were noted on).
    view: Option<(u64, Origin, egui::Rect)>,
    /// Go to message asked: the video whose row is scrolled into view when it is drawn, and when.
    pub(crate) go_to: Option<(String, std::time::Instant)>,
    /// This set's id, for the one-at-a-time claim across sets (`CLAIM`).
    set_id: u64,
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
            on_wall: false,
            frame: 0,
            volume: 1.0,
            muted: false,
            open_volume_settings: false,
            posters: HashMap::new(),
            poster_tx,
            poster_rx,
            dock: None,
            view: None,
            go_to: None,
            set_id: NEXT_SET.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
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
    /// The set of a wall screen showing the chat (gui/screen_surface.rs `ScreenTextures`): its
    /// videos play in their rows, their sound comes from the screen (BUG-258), and Full window
    /// opens on that screen, drawn by the screen's own run (`screen_surface::draw_screen_full_views`,
    /// 2026-11-06), so the person at the wall closes it there. It never docks (the dock is the
    /// person's, on the app's pages, never inside a wall screen: rung 3).
    pub(crate) fn on_a_wall_screen() -> Self {
        ChatVideos { on_wall: true, ..Default::default() }
    }

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

    /// Is one of these videos' sound playing now: playing, with a sound track, not muted and not at
    /// zero volume? The live music sits under it while it is (engine/music.rs, BUG-269).
    pub(crate) fn sounding(&self) -> bool {
        !self.muted && self.volume > 0.0 && self.live.values().any(|l| l.view.is_playing() && l.view.has_sound_track())
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

    /// Put the video at `url` (which has a player) in Full window, as its button does (a snapshot).
    #[cfg(test)]
    pub(crate) fn open_window(&mut self, url: &str) {
        self.window = Some(url.to_string());
    }

    /// The temporary copy the video at `url` plays from.
    #[cfg(test)]
    pub(crate) fn temp_copy(&self, url: &str) -> Option<PathBuf> {
        self.live.get(url).and_then(|l| l.view.temp.clone())
    }

    /// The message list's rectangle as the Chat page last noted it (tests read it after the frame,
    /// when [`ChatVideos::list_rect`], which answers for the frame being drawn, no longer does).
    #[cfg(test)]
    pub(crate) fn last_list_rect(&self) -> Option<egui::Rect> {
        self.view.as_ref().map(|(_, _, r)| *r)
    }

    /// Where the video at `url` is and whether it plays (tests: a failure says it).
    #[cfg(test)]
    pub(crate) fn where_is(&self, url: &str) -> Option<(f64, bool)> {
        self.player(url).map(|p| (p.position_s(), p.is_playing()))
    }

    /// Move the video at `url` back to its start, playing (tests: the fixtures are two seconds long,
    /// and a check that it plays on must not race its end; one that ended plays again).
    #[cfg(test)]
    pub(crate) fn rewind_for_test(&mut self, url: &str) {
        if let Some(live) = self.live.get_mut(url) {
            live.view.seek_to(0.0);
            live.view.play();
        }
    }

    /// Put the video at `url` (which has a player) in the dock as if its row had just left the view,
    /// from `origin` (a snapshot, which cannot scroll).
    #[cfg(test)]
    pub(crate) fn dock_for_test(&mut self, url: &str, origin: Origin) {
        if let Some(live) = self.live.get(url) {
            self.dock = Some(Dock { url: url.to_string(), name: live.name.clone(), ffmpeg: live.ffmpeg.clone(), origin, minimized: false, row_seen: u64::MAX, spot: None, opened_on: None });
        }
    }

    /// Play or Pause on the video at `url`, named `name`: play it (fetching it first if it has no
    /// player, and carrying on from its still's place), or pause it if it plays. It becomes the one
    /// allowed to play. A video that could not play tries again.
    pub(crate) fn press_play(&mut self, url: &str, name: &str, ffmpeg: &str) {
        let refused = self.live.get(url).is_some_and(|l| !l.view.has_player() && !l.view.fetching() && l.view.converting().is_none());
        if refused {
            self.live.remove(url);
        }
        // Another video started: the docked one pauses and the dock closes (rung 3, the operator's
        // choice of 2026-11-06); its player goes at the end of the frame, its row keeping its still.
        // (Only one video plays, so a press on any other video is a start, never a pause.)
        if let Some(docked) = self.dock.as_ref().filter(|d| d.url != url).map(|d| d.url.clone()) {
            if let Some(live) = self.live.get_mut(&docked) {
                live.view.pause();
            }
            self.dock = None;
        }
        match self.live.get_mut(url) {
            Some(live) if live.view.is_playing() => live.view.pause(),
            Some(live) => {
                live.view.play();
                self.current = Some(url.to_string());
                self.claim();
            }
            None => {
                self.start(url, name, ffmpeg);
                self.current = Some(url.to_string());
                self.claim();
            }
        }
    }

    /// This set's video is the one chat video allowed to play, on the app screen and every wall
    /// screen (the operator, 2026-11-06: "Agreed.").
    fn claim(&self) {
        CLAIM.with(|c| c.set(self.set_id));
    }

    /// Another set's Play was pressed since this set's: this set gives way. Its video pauses (one
    /// still fetching pauses when it starts: it is no longer current) and its dock closes. Called at
    /// each set's end of frame, at each sound step ([`ChatVideos::sync_sound`]: a wall screen's runs
    /// every frame in the world, framed or not), and for every wall screen every frame while a page
    /// covers the world (`engine::screens::chat_videos_one_at_a_time`), when none of them draws.
    pub(crate) fn yield_if_elsewhere(&mut self) {
        if self.current.is_none() || CLAIM.with(|c| c.get()) == self.set_id {
            return;
        }
        self.current = None;
        for live in self.live.values_mut() {
            if live.view.is_playing() {
                live.view.pause();
            }
        }
        // Paused that way, the docked one's dock closes; its player goes when it is next not drawn.
        self.dock = None;
    }

    /// The Chat page's note, each frame it draws its message list: where the rows it draws belong
    /// (`origin`) and the list's rectangle (`list`, between the channel header and the message box),
    /// where the dock sits on the Chat page.
    pub(crate) fn note_chat_view(&mut self, origin: Origin, list: egui::Rect) {
        self.view = Some((self.frame, origin, list));
    }

    /// The message list's rectangle, when the Chat page drew it this frame.
    pub(crate) fn list_rect(&self) -> Option<egui::Rect> {
        self.view.as_ref().filter(|(f, _, _)| *f == self.frame).map(|(_, _, r)| *r)
    }

    /// The docked video's row was drawn on screen this frame: it is back in its message (the dock
    /// closes at the end of the frame and is not drawn now).
    pub(crate) fn dock_row_back(&self) -> bool {
        self.dock.as_ref().is_some_and(|d| d.row_seen == self.frame)
    }

    /// The docked video is drawn in the dock this frame: poll it (once a frame) and keep it, its
    /// picture made an image only when it is drawn (`picture`; minimized it is not: the frame loop
    /// takes each due picture and keeps it for the still, and the decoder goes on, so the picture
    /// keeps up with the sound).
    pub(crate) fn on_dock(&mut self, url: &str, ctx: &egui::Context, picture: bool) {
        let frame = self.frame;
        if let Some(live) = self.live.get_mut(url) {
            live.seen = frame;
            if live.polled != frame {
                live.polled = frame;
                live.view.poll_with(ctx, picture);
            }
        }
    }

    /// The person is in the first-person world: the dock is an app layer, so it hides, and its video
    /// pauses and is not kept (its player goes at the end of the frame like a paused row's, its still
    /// and place staying); the dock comes back with them on the next page.
    pub(crate) fn hide_dock_in_world(&mut self) {
        if let Some(live) = self.dock.as_ref().and_then(|d| self.live.get_mut(&d.url)) {
            if live.view.is_playing() {
                live.view.pause();
            }
        }
    }

    /// Close in the dock: the video stops (paused), the dock closes, and its player goes now, its row
    /// keeping its place and its still.
    pub(crate) fn close_dock(&mut self, ctx: &egui::Context) {
        let Some(dock) = self.dock.take() else { return };
        if let Some(live) = self.live.get_mut(&dock.url) {
            live.view.pause();
        }
        if self.current.as_deref() == Some(dock.url.as_str()) {
            self.current = None;
        }
        self.forget(&dock.url, ctx);
    }

    /// Whether the video at `url` is playing, has a player, a still, or is fetching (the dock asks
    /// what to draw).
    pub(crate) fn state_of(&self, url: &str) -> (bool, Option<(f64, f64)>) {
        let live = self.live.get(url);
        let playing = live.is_some_and(|l| l.view.is_playing());
        let place = match live.filter(|l| l.view.has_player()) {
            Some(l) => Some(l.view.place()),
            None => self.stills.get(url).map(|s| (s.place_s, s.length_s)),
        };
        (playing, place)
    }

    /// The picture's shape (width over height): the player's picture, the still's or the poster
    /// picture's, else 16:9.
    pub(crate) fn aspect_of(&self, url: &str) -> f32 {
        let size = self.live.get(url).and_then(|l| l.view.texture()).map(|t| t.size()).or_else(|| self.stills.get(url).map(|s| s.texture.size())).or_else(|| poster_of(self, url).map(|t| t.size()));
        size.map_or(FIRST_ASPECT, |[w, h]| (w.max(1) as f32 / h.max(1) as f32).clamp(0.25, 4.0))
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
            self.live.insert(url.to_string(), Live { name: name.to_string(), view, ffmpeg: ffmpeg.to_string(), seen: frame, polled: u64::MAX, inline_seen: 0, origin: None });
            return;
        }
        let (address, copy) = (url.to_string(), crate::gui::pages::files::open::play_name(url, 0));
        view.start(ffmpeg, name, move || {
            use crate::gui::pages::files::open::{fetch_to, temp_dir, transfer_agent};
            fetch_to(&transfer_agent(std::time::Duration::from_secs(60)), &address, None, &temp_dir(), &copy, super::public_files::MAX_PUBLIC_FILE)
        });
        self.live.insert(url.to_string(), Live { name: name.to_string(), view, ffmpeg: ffmpeg.to_string(), seen: frame, polled: u64::MAX, inline_seen: 0, origin: None });
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

    /// The video at `url` was drawn on screen this frame: poll it (once a frame) and keep it. `row`:
    /// drawn in its message (not its window), which also notes where that row belongs (the Chat
    /// page's place this frame, for the dock) and brings a docked video back to its message.
    fn on_screen(&mut self, url: &str, ctx: &egui::Context, row: bool) {
        let frame = self.frame;
        let origin = self.view.as_ref().filter(|(f, _, _)| *f == frame).map(|(_, o, _)| o.clone());
        if row {
            if let Some(dock) = self.dock.as_mut().filter(|d| d.url == url) {
                dock.row_seen = frame;
            }
        }
        if let Some(live) = self.live.get_mut(url) {
            live.seen = frame;
            if row {
                live.inline_seen = frame;
                if origin.is_some() {
                    live.origin = origin;
                }
            }
            if live.polled != frame {
                live.polled = frame;
                live.view.poll(ctx);
            }
        }
    }

    /// The end of each frame (the engine calls it through [`draw_video_window`], after the pages and
    /// the dock; a wall screen showing the chat, after its page): this set gives way if another
    /// set's video started ([`ChatVideos::yield_if_elsewhere`]), every video but the current one is
    /// paused, the dock takes the video playing whose row left the view or gives back the one whose
    /// row came back ([`ChatVideos::dock_moves`]), and every player whose picture was not on screen
    /// this frame or the last, and is not in Full window, is dropped, leaving its still and words.
    pub(crate) fn end_frame(&mut self, ctx: &egui::Context) {
        self.yield_if_elsewhere();
        let current = self.current.clone();
        for (url, live) in self.live.iter_mut() {
            if Some(url) != current.as_ref() && live.view.is_playing() {
                live.view.pause();
            }
        }
        let frame = self.frame;
        if !self.on_wall {
            self.dock_moves(frame);
        }
        if self.go_to.as_ref().is_some_and(|(_, asked)| asked.elapsed() > GO_TO_WAIT) {
            self.go_to = None;
        }
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

    /// The dock's moves at the end of a frame (rung 3; never for a wall screen's set). A docked video
    /// whose row was drawn on screen this frame is back in its message: the dock closes. Otherwise,
    /// with the dock empty, the current video, playing or on its way to play (fetching, converting),
    /// whose row was on screen until now and not this frame (and is not in Full window), moves into
    /// the dock: the same player, nothing fetched again. A row drawn somewhere that is not the Chat
    /// page's message list (a Shared File's card on the Files page) has no place to go back to, so
    /// its video is not docked and goes as before.
    fn dock_moves(&mut self, frame: u64) {
        if self.dock.as_ref().is_some_and(|d| d.row_seen == frame) {
            self.dock = None;
        }
        if self.dock.is_some() {
            return;
        }
        let Some(url) = self.current.clone() else { return };
        let Some(live) = self.live.get(&url) else { return };
        let wants_to_play = live.view.is_playing() || live.view.fetching() || live.view.converting().is_some();
        let left_now = live.inline_seen < frame && live.seen + 1 >= frame;
        if !wants_to_play || !left_now || self.window.as_deref() == Some(url.as_str()) {
            return;
        }
        let Some(origin) = live.origin.clone() else { return };
        self.dock = Some(Dock { url, name: live.name.clone(), ffmpeg: live.ffmpeg.clone(), origin, minimized: false, row_seen: u64::MAX, spot: None, opened_on: None });
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

    /// Their sound through the app's mixer, at the chat videos' volume and Mute, heard from `place`
    /// (each frame it has a mixer): the app screen's set from the app, a wall screen's from that
    /// screen (BUG-258: a wall screen's videos were never handed to the mixer, so they played
    /// silent and their Mute and volume did nothing).
    pub(crate) fn sync_sound(&mut self, audio: &mut humanity_core::audio::AudioManager, place: SoundPlace) {
        // A wall screen's sound step runs every frame in the world, framed or not: it gives way here
        // too when the app screen's video (or another screen's) started (2026-11-06).
        self.yield_if_elsewhere();
        for live in self.live.values_mut() {
            live.view.volume = self.volume;
            live.view.muted = self.muted;
            live.view.sync_sound_at(audio, place);
        }
    }

    /// The mix last handed to the mixer for the video at `url`'s sound (volume, pan), if it has one.
    #[cfg(test)]
    pub(crate) fn mix_sent(&self, url: &str) -> Option<(f64, f64)> {
        self.live.get(url).and_then(|l| l.view.mix_sent)
    }
}

/// What a player's controls asked for this frame.
#[derive(Default)]
struct Asked {
    play: bool,
    /// A new place, in seconds.
    seek: Option<f64>,
    mute: bool,
    volume: Option<f32>,
    window: bool,
    /// The mark beside the volume bar: Settings > Audio's video volume choices (2026-11-06).
    settings: bool,
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
/// for Full window's bigger view (no Full window button there, and it is always on screen), where
/// `max.y` holds the picture AND its controls: they are laid out first, where nobody sees them, and
/// the picture's box (the full width, the picture fitted in it) gets the height they leave (BUG-258,
/// as BUG-251's `show_fit`).
#[allow(clippy::too_many_arguments)]
fn draw_player(ui: &mut egui::Ui, theme: &Theme, videos: &mut ChatVideos, url: &str, name: &str, ffmpeg: &str, max: egui::Vec2, in_window: bool) {
    let ctx = ui.ctx().clone();
    let elsewhere = !in_window && videos.window.as_deref() == Some(url);
    // The shape: the picture's own once one has come (the player's, the still's, or the poster
    // picture's), else 16:9.
    let aspect = videos.aspect_of(url);
    let mut width = if in_window { max.x } else { ui.available_width().min(max.x).max(160.0) };
    let mut height = width / aspect;
    if in_window {
        let below = measured_height(ui, width, &mut |ui| {
            under_picture(ui, theme, videos, url, width, true);
        });
        height = height.min((max.y - below - ui.spacing().item_spacing.y).max(0.0));
    } else if height > max.y {
        height = max.y.max(90.0);
        width = height * aspect;
    }
    let picture = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover()).0;
    let press = ui.interact(picture, ui.make_persistent_id("picture"), egui::Sense::click()).hint("chat_video.picture_hint");
    #[cfg(test)]
    videos.pictures.insert(url.to_string(), picture);
    // Go to message (the dock): the row asked for, drawn in the Chat page's list, is scrolled to
    // the middle of the view, where its video comes back from the dock.
    if !in_window && videos.list_rect().is_some() && videos.go_to.as_ref().is_some_and(|(u, _)| u == url) {
        ui.scroll_to_rect(picture, Some(egui::Align::Center));
        videos.go_to = None;
    }
    if (in_window || ui.is_rect_visible(picture)) && !elsewhere {
        videos.on_screen(url, &ctx, !in_window);
        // Nothing of the video to show yet (never played here): its poster picture, asked for once.
        if !videos.live.contains_key(url) && !videos.stills.contains_key(url) {
            videos.want_poster(url);
        }
    }
    if ui.is_rect_visible(picture) {
        paint_picture(ui, theme, videos, url, picture, elsewhere);
    }
    if elsewhere {
        return;
    }
    let mut asked = under_picture(ui, theme, videos, url, width, in_window);
    asked.play |= press.clicked();
    if asked.settings {
        videos.open_volume_settings = true;
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
    if let (Some(seconds), Some(live)) = (asked.seek, videos.live.get_mut(url)) {
        live.view.seek_to(seconds);
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

/// What goes under a player's picture, `width` points wide: the seek bar, then Play or Pause, the
/// time, Mute, the volume and Full window (not in the window itself, nor on a wall screen, BUG-258),
/// then the words of a video that cannot play. Returns what its controls asked for this frame.
fn under_picture(ui: &mut egui::Ui, theme: &Theme, videos: &ChatVideos, url: &str, width: f32, in_window: bool) -> Asked {
    let mut asked = Asked::default();
    // The place, the time and the state, from the player, else from the still.
    let live = videos.live.get(url);
    let (place, length) = match live.filter(|l| l.view.has_player()) {
        Some(l) => l.view.place(),
        None => videos.stills.get(url).map_or((0.0, 0.0), |s| (s.place_s, s.length_s)),
    };
    let playing = live.is_some_and(|l| l.view.is_playing());
    let can_seek = live.is_some_and(|l| l.view.has_player()) && length > 0.0;
    let fraction = if length > 0.0 { (place / length).clamp(0.0, 1.0) as f32 } else { 0.0 };
    asked.seek = video_view::track_bar(ui, theme, "seek", fraction, width, can_seek, TrackReport::OnRelease, "chat_video.seek_hint").map(|f| f as f64 * length);
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
                asked.settings = true;
            }
            asked.volume = video_view::track_bar(ui, theme, "volume", videos.volume, VOLUME_WIDTH, !videos.muted, TrackReport::Live, "chat_video.volume_hint");
            if widgets::Button::ghost(if videos.muted { t("chat_video.unmute") } else { t("chat_video.mute") }).size(small).hint("chat_video.mute_hint").show(ui, theme) {
                asked.mute = true;
            }
        });
    });
    // A video nothing here plays, in BUG-228's plain words (the card's Download is beside its name).
    let problem = live.map(|l| l.view.problem.clone()).filter(|p| !p.is_empty()).or_else(|| videos.problems.get(url).cloned());
    if let Some(words) = problem {
        ui.scope(|ui| {
            ui.set_max_width(width);
            ui.add(egui::Label::new(RichText::new(words).size(theme.font_size_small).color(theme.danger())).wrap());
        });
    }
    asked
}

/// `name` as a window's title no wider than `width` points (in the heading font a title is drawn
/// in): whole when it fits, else cut between characters with an ellipsis after it.
fn title_fitting(ctx: &egui::Context, name: &str, width: f32) -> String {
    let font = egui::TextStyle::Heading.resolve(&ctx.style());
    let wide = |s: &str| ctx.fonts(|f| f.layout_no_wrap(s.to_string(), font.clone(), egui::Color32::PLACEHOLDER).size().x);
    if wide(name) <= width {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let cut = |n: usize| format!("{}\u{2026}", chars[..n].iter().collect::<String>().trim_end());
    let (mut lo, mut hi) = (0, chars.len());
    while lo < hi {
        let mid = (lo + hi + 1) / 2;
        if wide(&cut(mid)) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

/// How tall `add` lays out `width` points wide, laid out once where nobody sees it: not drawn,
/// disabled (so it answers nothing and shows no hint), and far above the screen, where no pointer
/// can rest on it, so what comes before it can be given the rest.
fn measured_height(ui: &mut egui::Ui, width: f32, add: &mut dyn FnMut(&mut egui::Ui)) -> f32 {
    let at = egui::Rect::from_min_size(egui::pos2(ui.cursor().min.x, -1.0e6), egui::vec2(width, 1.0e5));
    let mut probe = ui.new_child(egui::UiBuilder::new().max_rect(at).layout(*ui.layout()).invisible());
    add(&mut probe);
    probe.min_rect().height()
}

/// The picture box: black, the picture (the player's, else the still) fitted in it, and over it the
/// Play mark while it is not playing, or what is happening (fetching, converting, starting, or that
/// it plays in its own window).
pub(super) fn paint_picture(ui: &egui::Ui, theme: &Theme, videos: &ChatVideos, url: &str, rect: egui::Rect, elsewhere: bool) {
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
///
/// The window's body is held to the room a dialog has on this screen (`widgets::dialog::body_room`,
/// BUG-251's rule) and laid out from the bottom up: Close the window and the player's controls are
/// measured first and the picture gets the height they leave, so Close always sits inside the
/// window, on the screen, at every window size and Font Size (BUG-258: the picture was sized to
/// 0.72 of the screen's height with its controls and Close under it, so on a screen under about
/// 620 points tall, 1280x720 at Font Size 125% or 1920x1080 at 175%, Close fell off the bottom).
/// Its title, the video's name, is cut to the window's width, or a long name widened the window
/// past a narrow screen's edges (1024x640 at 175%).
pub(crate) fn draw_video_window(ctx: &egui::Context, theme: &Theme, videos: &mut ChatVideos) {
    if let Some(url) = videos.window.clone() {
        if let Some((name, ffmpeg)) = videos.live.get(&url).map(|l| (l.name.clone(), l.ffmpeg.clone())) {
            let mut open = true;
            let mut close = false;
            let room = widgets::dialog::body_room(ctx, theme);
            let width = (ctx.screen_rect().width() * 0.8).max(320.0).min(room.x);
            // The title is the video's name, cut to the window's width: a window grows to fit its
            // title, so a long name would push it past a narrow screen's edges.
            let title = title_fitting(ctx, &name, width - 2.0 * ctx.style().spacing.interact_size.y);
            widgets::dialog(ctx, theme, WINDOW_ID, &title, &mut open, |ui| {
                let close_button = |ui: &mut egui::Ui| widgets::Button::secondary(t("chat_video.close_window")).hint("chat_video.close_window_hint").show(ui, theme);
                let foot = measured_height(ui, width, &mut |ui| {
                    close_button(ui);
                });
                let gap = ui.spacing().item_spacing.y;
                draw_player(ui, theme, videos, &url, &name, &ffmpeg, egui::vec2(width, room.y - foot - gap), true);
                close = close_button(ui);
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

/// The chat's videos on the app's screen get their sound from the app's mixer, heard from the app
/// (the engine calls this each frame it has a mixer; a wall screen's are placed at the screen by
/// `engine::screens::place_page_sound`).
pub(crate) fn sync_video_sound(videos: &mut ChatVideos, audio: &mut humanity_core::audio::AudioManager) {
    videos.sync_sound(audio, SoundPlace::App);
}

#[cfg(test)]
#[path = "inline_video_tests.rs"]
pub(crate) mod tests;
