//! A video played in the app: fetched first to a temporary copy on a worker, then played by the
//! app's own player (`crate::media::VideoPlayer`: AV1 video in WebM or MP4, and on Windows an
//! ordinary MP4 through the computer's own decoders, `media/system.rs`), with Play, Pause, Start
//! again, Mute and its time, and its sound through the app's mixer ([`VideoView::sync_sound`],
//! which the engine calls each frame it has a mixer). The temporary copy goes with the view.
//!
//! One widget, so a Media item's video and a video posted in chat play the same way (2026-10-18;
//! the Media page's player moved here from `pages/files/open.rs`). The website's twin is the
//! browser's own `<video>` element.
//!
//! **One road for every video (BUG-228, 2026-11-03).** The fetched copy goes through
//! `media::transcode::begin_ingest`, the decision the in-world screens make: played as it is, or
//! converted once with the person's own ffmpeg (Settings > Media) when nothing here plays it, or
//! refused in words that say what the file is. It used to open the player directly, so a chat
//! video never got the conversion a screen gave the same file. The decision runs on the fetch's
//! worker (it may ask Windows and ffmpeg); a conversion is written beside the temporary copy and
//! deleted with it, so nothing is kept across runs.
//!
//! **Inside a chat message (2026-11-06).** A video posted in chat plays in its row
//! (`pages/chat/inline_video.rs`), through this view: its picture, its place ([`VideoView::seek_to`]),
//! a starting place for a video played again after its row left the screen
//! ([`VideoView::start_at`]), its own volume on top of the app's ([`VideoView::volume`]), and the
//! last picture it showed, made small for the row's still ([`VideoView::still_image`]). The seek
//! and volume bars are [`track_bar`].

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use egui::RichText;
use humanity_core::i18n::{t, tf};

use crate::gui::theme::Theme;
use crate::gui::widgets;
use crate::media::transcode::{Ingest, TranscodeJob};

/// The player's buttons and lines.
pub const PLAY_HERE: &str = "Play here";
pub const PLAY_HERE_TIP: &str = "Fetch the video and play it in the app.";
pub const MUTE: &str = "Mute";
pub const UNMUTE: &str = "Unmute";
pub const FETCHING: &str = "Fetching the video...";
pub const STARTING: &str = "Starting...";

/// How wide a player's volume bar is, in points.
pub const VOLUME_BAR_WIDTH: f32 = 72.0;

/// What the view says under a video whose sound the computer cannot play here.
pub fn sound_lost_words(codec: &str) -> String {
    format!("Its sound ({codec}) is in a format this computer cannot play here. Download it to hear it in another player.")
}

/// What it says when the file will not play: what the file is and why (`begin_ingest`'s words, or
/// the player's), then what will play it.
pub fn cannot_play_words(why: &str) -> String {
    format!("{} {}", why.trim_end(), t("video.elsewhere"))
}

/// What the worker hands back: the temporary copy and what `begin_ingest` decided for it.
type Fetched = Result<(PathBuf, Ingest), String>;

/// Where a video's sound is heard from (BUG-258, 2026-11-06).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SoundPlace {
    /// The app itself: centred, at the master volume (the Media page, the chat on the app's own
    /// screen).
    App,
    /// A screen in the world (a wall screen showing the chat): `falloff` (0 to 1) from how far the
    /// listener is and `pan` (0 left, 0.5 centre, 1 right) from the screen's bearing, worked out by
    /// `engine::screens::video::audio_placement`, and on the world-sound bus under the master volume,
    /// the way a film on an in-world screen is heard (`compose_mix`).
    World { falloff: f64, pan: f64 },
}

/// A video being fetched, playing, or that could not play.
pub struct VideoView {
    fetch_rx: Option<Receiver<Fetched>>,
    pub(crate) player: Option<crate::media::VideoPlayer>,
    texture: Option<egui::TextureHandle>,
    /// Why it cannot play, or why the fetch failed.
    pub problem: String,
    /// The temporary copy being played, deleted with the view.
    pub temp: Option<PathBuf>,
    /// A conversion running with the person's ffmpeg; its copy plays when it ends.
    job: Option<TranscodeJob>,
    /// The converted copy (beside the temporary one), deleted with the view.
    converted: Option<PathBuf>,
    /// What the person knows the video by, for the words.
    name: String,
    /// Its sound: handed to the mixer once, muted with Mute, at the mix (volume, pan) last sent.
    sound_attached: bool,
    pub muted: bool,
    pub(crate) mix_sent: Option<(f64, f64)>,
    /// Its own volume, 0 to 1, on top of the app's master volume (a chat row's volume bar).
    pub volume: f32,
    /// Where it starts when its player opens, in seconds (a chat video played again after its row
    /// left the screen carries on from its place). 0: the start.
    pub start_at: f64,
    /// The last picture shown, kept (moved, not copied) for [`VideoView::still_image`].
    last_frame: Option<crate::media::VideoFrame>,
    /// Pictures came while the picture was not drawn ([`VideoView::poll_with`]): the texture is
    /// behind `last_frame` until it is drawn again.
    picture_behind: bool,
    /// Set once the fetch's worker has handed its result over (tests: a view dropped after that).
    #[cfg(test)]
    pub(crate) handed: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Default for VideoView {
    fn default() -> Self {
        VideoView {
            fetch_rx: None,
            player: None,
            texture: None,
            problem: String::new(),
            temp: None,
            job: None,
            converted: None,
            name: String::new(),
            sound_attached: false,
            muted: false,
            mix_sent: None,
            volume: 1.0,
            start_at: 0.0,
            last_frame: None,
            picture_behind: false,
            #[cfg(test)]
            handed: Default::default(),
        }
    }
}

impl VideoView {
    /// Fetch the video with `fetch` on a worker (it returns the temporary copy), decide there how it
    /// plays (`begin_ingest`, with `ffmpeg_setting` from Settings > Media), then play it. `name` is
    /// what the person knows the video by (the chat card's name, the item's title), for the words.
    pub fn start(&mut self, ffmpeg_setting: &str, name: &str, fetch: impl FnOnce() -> Result<PathBuf, String> + Send + 'static) {
        let (tx, rx) = std::sync::mpsc::channel();
        self.name = name.to_string();
        // A new video starts at Settings > Audio's start volume (2026-11-06): my last volume, or a
        // set one (humanity_core::audio::start_volume).
        self.volume = humanity_core::audio::start_volume::start_volume();
        let (ffmpeg_setting, name) = (ffmpeg_setting.to_string(), name.to_string());
        #[cfg(test)]
        let handed = self.handed.clone();
        std::thread::spawn(move || {
            let fetched = fetch().map(|copy| {
                // A conversion is written beside the copy: the temporary folder, swept at every start.
                let dir = copy.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
                let ingest = crate::media::transcode::begin_ingest_as(&copy, &name, &ffmpeg_setting, &dir);
                (copy, ingest)
            });
            match tx.send(fetched) {
                // The view went while it was fetching: nobody will play it, so it goes now (a
                // conversion stops when its job is dropped, and takes its part file with it). A view
                // that goes after this send takes it in its own Drop.
                Err(std::sync::mpsc::SendError(Ok((copy, _)))) => {
                    let _ = std::fs::remove_file(copy);
                }
                Ok(()) => {
                    #[cfg(test)]
                    handed.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Err(_) => {}
            }
        });
        self.fetch_rx = Some(rx);
        self.problem.clear();
    }

    /// Whether the video is still on its way.
    pub fn fetching(&self) -> bool {
        self.fetch_rx.is_some()
    }

    /// Whether a player is there (playing, paused or ended).
    pub fn has_player(&self) -> bool {
        self.player.is_some()
    }

    /// Whether a conversion is running, and how far it is.
    pub fn converting(&self) -> Option<f32> {
        self.job.as_ref().map(|j| j.percent())
    }

    /// Open the player on `path` and start it (at [`VideoView::start_at`]), or say why not.
    fn open(&mut self, path: &std::path::Path) {
        match crate::media::VideoPlayer::open(path) {
            Ok(mut player) => {
                // A place at the very end would start it ended: then it starts from the top.
                if self.start_at > 0.0 && (player.duration_s() <= 0.0 || self.start_at < player.duration_s() - 0.5) {
                    player.seek_to(self.start_at);
                }
                player.play();
                self.player = Some(player);
            }
            Err(e) => self.problem = cannot_play_words(&e.to_string()),
        }
    }

    /// The picture on show, once the first one has come.
    pub fn texture(&self) -> Option<&egui::TextureHandle> {
        self.texture.as_ref()
    }

    /// Whether it is playing now.
    pub fn is_playing(&self) -> bool {
        self.player.as_ref().is_some_and(|p| p.is_playing())
    }

    /// Play it (from the top again once it has ended). Nothing before its player is there.
    pub fn play(&mut self) {
        if let Some(player) = self.player.as_mut() {
            if player.at_end() {
                player.seek_to_start();
            }
            player.play();
        }
    }

    /// Pause it.
    pub fn pause(&mut self) {
        if let Some(player) = self.player.as_mut() {
            player.pause();
        }
    }

    /// Move it to `seconds` (clamped into the video), playing or paused as it was.
    pub fn seek_to(&mut self, seconds: f64) {
        if let Some(player) = self.player.as_mut() {
            player.seek_to(seconds);
        }
    }

    /// Where it is and how long it is, in seconds (0 and 0 before its player is there).
    pub fn place(&self) -> (f64, f64) {
        self.player.as_ref().map_or((0.0, 0.0), |p| (p.position_s(), p.duration_s()))
    }

    /// The last picture it showed, at most `max_width` pixels wide (each pixel of the small one the
    /// average of the block of the big one it covers), for a row's still once its player goes.
    pub fn still_image(&self, max_width: usize) -> Option<egui::ColorImage> {
        let f = self.last_frame.as_ref()?;
        Some(small_picture(&f.rgba, f.width as usize, f.height as usize, max_width))
    }

    /// Take what the fetch finished and the next frame. Every frame the view is drawn.
    pub fn poll(&mut self, ctx: &egui::Context) {
        self.poll_with(ctx, true);
    }

    /// [`VideoView::poll`], with `picture` false while its picture is not drawn (a chat video's dock
    /// minimized to its tab, 2026-11-06): each due frame is still taken from the player (its queue
    /// holds four; one not taken stops the decoder, and the picture would fall behind the sound) and
    /// kept for the still, but not made an image or uploaded (2.6 to 2.9 ms a 1080p picture), and
    /// the app is asked for a frame about 30 times a second rather than every frame. The first poll
    /// with the picture drawn again makes the last picture an image once.
    pub fn poll_with(&mut self, ctx: &egui::Context, picture: bool) {
        if let Some(rx) = self.fetch_rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => {
                    self.fetch_rx = None;
                    match result {
                        Ok((copy, ingest)) => {
                            self.temp = Some(copy); // deleted with the view from here on
                            match ingest {
                                Ingest::Direct(p) | Ingest::Cached(p) => self.open(&p),
                                Ingest::Transcoding { job, why } => {
                                    log::info!("media: converting a video to play here: it {why}");
                                    self.converted = Some(job.dst().to_path_buf());
                                    self.job = Some(job);
                                }
                                Ingest::Failed(words) => self.problem = cannot_play_words(&words),
                            }
                        }
                        Err(e) => self.problem = e,
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(200)),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.fetch_rx = None;
                    self.problem = "The video stopped coming.".to_string();
                }
            }
        }
        if let Some(result) = self.job.as_ref().and_then(|j| j.result()) {
            self.job = None;
            match result {
                Ok(dst) => self.open(&dst),
                Err(e) => {
                    self.problem = cannot_play_words(&tf("video.convert_failed", &[("name", &self.name), ("error", &e)]));
                }
            }
        } else if self.job.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        if let Some(player) = self.player.as_mut() {
            let mut show = None;
            if let Some(frame) = player.poll() {
                // Kept for a row's still (moved, not copied: the frame was going to be dropped here).
                self.last_frame = Some(frame);
                show = Some(picture);
            } else if picture && self.picture_behind {
                show = Some(true);
            }
            match (show, self.last_frame.as_ref()) {
                (Some(true), Some(frame)) => {
                    // Every decoder here writes opaque pictures (alpha 255: media/video.rs and
                    // media/system.rs), for which premultiplied and unmultiplied are the same bytes;
                    // the premultiplied form is a plain copy, where the unmultiplied one branches on
                    // every pixel's alpha (2026-11-06: measured in chat::inline_video::tests::bench).
                    let image = egui::ColorImage::from_rgba_premultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
                    match self.texture.as_mut() {
                        Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                        None => self.texture = Some(ctx.load_texture("video_view_frame", image, egui::TextureOptions::LINEAR)),
                    }
                    self.picture_behind = false;
                }
                // Not drawn: the picture on show falls behind until it is drawn again.
                (Some(false), _) => self.picture_behind = true,
                _ => {}
            }
            if let Some(e) = player.take_error() {
                self.problem = tf("video.stopped", &[("error", &e)]);
            }
            if player.is_playing() {
                if picture {
                    ctx.request_repaint();
                } else {
                    // Often enough to keep the four-picture queue moving at 30 pictures a second.
                    ctx.request_repaint_after(Duration::from_millis(30));
                }
            }
        }
    }

    /// The video at most `max_width` wide, with its buttons; Play here while nothing is fetched.
    /// Returns true when Play here was pressed (the caller then calls [`VideoView::start`]).
    pub fn draw(&mut self, ui: &mut egui::Ui, theme: &Theme, max_width: f32) -> bool {
        self.draw_fit(ui, theme, egui::vec2(max_width, f32::INFINITY))
    }

    /// The video fitted inside `max` (its buttons included), centred across the width it has
    /// (the Media page's middle pane, 2026-10-19); as [`VideoView::draw`] otherwise.
    pub fn draw_fit(&mut self, ui: &mut egui::Ui, theme: &Theme, max: egui::Vec2) -> bool {
        let mut play_here = false;
        let small = |s: &str| RichText::new(s).size(theme.font_size_small).color(theme.text_muted());
        if let (Some(tex), Some(player)) = (self.texture.as_ref(), self.player.as_mut()) {
            let [w, h] = tex.size();
            let aspect = w.max(1) as f32 / h.max(1) as f32;
            let room = ui.available_width().min(max.x);
            let controls = theme.button_height + theme.spacing_sm * 2.0;
            let tallest = (max.y - controls).max(theme.button_height);
            let (mut width, mut height) = (room, room / aspect);
            if height > tallest {
                height = tallest;
                width = height * aspect;
            }
            let (row, _) = ui.allocate_exact_size(egui::vec2(room, height), egui::Sense::hover());
            egui::Image::new((tex.id(), egui::vec2(width, height))).paint_at(ui, egui::Rect::from_center_size(row.center(), egui::vec2(width, height)));
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
                // Sound once the mixer has it (`sync_sound`); an app started in the background by a
                // script opens no sound device, so it has none to mute.
                if player.has_audio() && widgets::Button::ghost(if self.muted { UNMUTE } else { MUTE }).show(ui, theme) {
                    self.muted = !self.muted;
                }
                // Its own volume (2026-11-06): a new video starts at Settings > Audio's start volume,
                // and a move here is the last volume the next one starts at with "my last volume".
                if player.has_audio() {
                    if let Some(v) = track_bar(ui, theme, "video_volume", self.volume, VOLUME_BAR_WIDTH, !self.muted, TrackReport::Live, "video.volume_hint") {
                        self.volume = v;
                        humanity_core::audio::start_volume::note_volume(v);
                    }
                }
                ui.label(small(&format!("{} of {}", clock(player.position_s()), clock(player.duration_s()))));
            });
            if let Some(codec) = player.info().sound_lost.as_deref() {
                widgets::body_hint(ui, theme, &sound_lost_words(codec));
            }
        } else if self.fetch_rx.is_some() {
            ui.label(small(FETCHING));
        } else if let Some(pct) = self.converting() {
            let pct = format!("{pct:.0}");
            ui.label(small(&tf("video.converting", &[("pct", &pct)])));
            ui.ctx().request_repaint_after(Duration::from_millis(200));
        } else if self.player.is_some() {
            ui.label(small(STARTING));
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        } else if self.problem.is_empty() {
            play_here = widgets::Button::primary(PLAY_HERE).tooltip(PLAY_HERE_TIP).show(ui, theme);
        }
        if !self.problem.is_empty() {
            ui.add(egui::Label::new(RichText::new(&self.problem).size(theme.font_size_small).color(theme.danger())).wrap());
        }
        play_here
    }

    /// The video's sound from the app's mixer, heard from the app itself (see [`Self::sync_sound_at`]).
    pub fn sync_sound(&mut self, audio: &mut humanity_core::audio::AudioManager) {
        self.sync_sound_at(audio, SoundPlace::App);
    }

    /// The mix (volume, pan) its sound plays at from `place`, with the master volume `master` and
    /// the world-sound bus's `sfx`: its own volume on top, silence while muted. Pure, so the sum is a
    /// test and not an ear test.
    pub fn mix(&self, place: SoundPlace, master: f64, sfx: f64) -> (f64, f64) {
        let own = self.volume.clamp(0.0, 1.0) as f64;
        let (volume, pan) = match place {
            SoundPlace::App => (master * own, 0.5),
            // The row's own volume goes in as `compose_mix`'s `own`, once, as a clip's strip bar does.
            SoundPlace::World { falloff, pan } => (crate::media::compose_mix(master, sfx, own, falloff), pan),
        };
        (if self.muted { 0.0 } else { volume }, pan)
    }

    /// The video's sound from the app's mixer, heard from `place`: handed over once, at the mix
    /// that place gives now ([`Self::mix`]), then sent again only when the mix moved past the
    /// in-world screens' gate (`mix_moved`: a listener standing still sends nothing, a Mute always
    /// goes). The engine calls it each frame it has a mixer: for the Media page and the chat on
    /// the app's screen from the app, for a wall screen's chat from that screen (BUG-258).
    pub fn sync_sound_at(&mut self, audio: &mut humanity_core::audio::AudioManager, place: SoundPlace) {
        use crate::media::{mix_moved, MIX_TWEEN_MS};
        let (volume, pan) = self.mix(place, audio.master_volume(), audio.sfx_volume());
        let Some(player) = self.player.as_mut() else { return };
        if player.info().audio_track.is_none() {
            return;
        }
        if !self.sound_attached {
            self.sound_attached = true;
            let opts = crate::media::AudioAttach { looping: false, volume, panning: pan };
            if let Err(e) = player.attach_audio_with(audio, opts) {
                log::warn!("media: the video's sound did not start: {e}");
                return;
            }
            self.mix_sent = Some((volume, pan));
            return;
        }
        if mix_moved(self.mix_sent, (volume, pan)) {
            player.set_audio_mix(volume, pan, MIX_TWEEN_MS);
            self.mix_sent = Some((volume, pan));
        }
    }
}

impl Drop for VideoView {
    /// The player lets go of the file first, then the temporary copy goes. A fetch that finished
    /// but was never taken (the view went before its next poll: a wall screen's chat not drawn
    /// since Play) is taken here, so its copy and any conversion of it go too (BUG-258).
    fn drop(&mut self) {
        if let Some(Ok(Ok((copy, ingest)))) = self.fetch_rx.take().map(|rx| rx.try_recv()) {
            if let Ingest::Transcoding { job, .. } = ingest {
                let dst = job.dst().to_path_buf();
                drop(job); // kills ffmpeg and removes its part file
                let _ = std::fs::remove_file(dst);
            }
            let _ = std::fs::remove_file(copy);
        }
        self.player = None;
        // A conversion still running stops (its job's Drop kills ffmpeg and removes its part file).
        self.job = None;
        if let Some(path) = self.converted.take() {
            let _ = std::fs::remove_file(path);
        }
        if let Some(path) = self.temp.take() {
            if let Err(e) = std::fs::remove_file(&path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    log::warn!("media: a temporary video could not be deleted yet ({e}); it goes at the next start.");
                }
            }
        }
    }
}

/// "1:05": minutes and seconds.
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    format!("{}:{:02}", s / 60, s % 60)
}

/// `rgba` (`width` x `height`, tightly packed) made at most `max_width` wide, its shape kept: each
/// pixel of the small picture is the average of the block of the big one it covers (a box filter,
/// so a 1080p frame's fine detail does not shimmer into noise the way picking one pixel would).
/// A picture already that narrow is copied as it is.
pub fn small_picture(rgba: &[u8], width: usize, height: usize, max_width: usize) -> egui::ColorImage {
    let (width, height) = (width.max(1), height.max(1));
    if rgba.len() < width * height * 4 {
        return egui::ColorImage::new([1, 1], egui::Color32::TRANSPARENT);
    }
    let max_width = max_width.max(1);
    if width <= max_width {
        return egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba[..width * height * 4]);
    }
    let w = max_width;
    let h = ((height * w) as f64 / width as f64).round().max(1.0) as usize;
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        let (y0, y1) = (y * height / h, ((y + 1) * height / h).max(y * height / h + 1).min(height));
        for x in 0..w {
            let (x0, x1) = (x * width / w, ((x + 1) * width / w).max(x * width / w + 1).min(width));
            let mut sum = [0u32; 4];
            for sy in y0..y1 {
                let row = &rgba[(sy * width + x0) * 4..(sy * width + x1) * 4];
                for px in row.chunks_exact(4) {
                    for c in 0..4 {
                        sum[c] += px[c] as u32;
                    }
                }
            }
            let n = ((y1 - y0) * (x1 - x0)) as u32;
            out.extend(sum.iter().map(|s| (s / n) as u8));
        }
    }
    egui::ColorImage::from_rgba_unmultiplied([w, h], &out)
}

/// When a [`track_bar`] says where it was put.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackReport {
    /// On every frame of a drag (a volume: moving it costs nothing).
    Live,
    /// Once, at a click or where a drag let go (a video's place: each seek restarts its decoder,
    /// so it seeks once, not on every point of a drag).
    OnRelease,
}

/// A bar that shows how far along something is and lets a person put it elsewhere: a video's place,
/// a volume. The part done is in the accent colour, the rest in the slider track's, with a knob at
/// the place. Plain on purpose: no gradient and no animated ring, because `widgets::custom_slider`
/// asks for a repaint every frame for its ring, which a paused video in a chat row must not cost.
/// The arrow keys move it by a twentieth once Tab has reached it. `hint_key` is its plain words.
/// Returns where it was put, 0 to 1 (see [`TrackReport`]); nothing while `enabled` is false. The
/// website's twin is the browser's own `<video controls>` bar.
#[allow(clippy::too_many_arguments)]
pub fn track_bar(ui: &mut egui::Ui, theme: &Theme, id_salt: impl std::hash::Hash, value: f32, width: f32, enabled: bool, report: TrackReport, hint_key: &'static str) -> Option<f32> {
    track_bar_at(ui, theme, id_salt, value, width, enabled, report, hint_key).0
}

/// [`track_bar`], and where it was laid out (a test aims a press at it: a screen's volume bar).
#[allow(clippy::too_many_arguments)]
pub fn track_bar_at(ui: &mut egui::Ui, theme: &Theme, id_salt: impl std::hash::Hash, value: f32, width: f32, enabled: bool, report: TrackReport, hint_key: &'static str) -> (Option<f32>, egui::Rect) {
    use crate::gui::widgets::hint::Hint;
    let knob = theme.slider_thumb_radius;
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width.max(knob * 4.0), knob * 2.0 + 4.0), egui::Sense::hover());
    let id = ui.make_persistent_id(id_salt);
    let resp = ui.interact(rect, id, egui::Sense::click_and_drag()).hint(hint_key);
    resp.widget_info(|| egui::WidgetInfo::slider(enabled, value as f64, humanity_core::i18n::t(hint_key)));
    let left = rect.left() + knob;
    let travel = (rect.width() - 2.0 * knob).max(1.0);
    let at_pointer = resp.interact_pointer_pos().map(|p| ((p.x - left) / travel).clamp(0.0, 1.0));
    // Where a drag has got to, kept for its release (the pointer may be gone by then).
    if resp.dragged() {
        if let Some(f) = at_pointer {
            ui.data_mut(|d| d.insert_temp(id, f));
        }
    }
    let mut put = None;
    if enabled {
        put = match report {
            TrackReport::Live if resp.dragged() || resp.clicked() => at_pointer,
            TrackReport::OnRelease if resp.drag_stopped() => at_pointer.or_else(|| ui.data(|d| d.get_temp::<f32>(id))),
            TrackReport::OnRelease if resp.clicked() => at_pointer,
            _ => None,
        };
        if resp.has_focus() {
            ui.memory_mut(|m| m.set_focus_lock_filter(resp.id, egui::EventFilter { horizontal_arrows: true, ..Default::default() }));
            let (back, on) = ui.input(|i| (i.key_pressed(egui::Key::ArrowLeft), i.key_pressed(egui::Key::ArrowRight)));
            if back || on {
                put = Some((value + if on { 0.05 } else { -0.05 }).clamp(0.0, 1.0));
            }
        }
    }
    if ui.is_rect_visible(rect) {
        // While a drag is held, the knob follows the pointer (the place moves once, at the release).
        let shown = if enabled && resp.dragged() { at_pointer.unwrap_or(value) } else { put.unwrap_or(value) }.clamp(0.0, 1.0);
        let x = left + shown * travel;
        let line = theme.slider_track_height;
        let y = rect.center().y;
        let corner = egui::CornerRadius::same((line / 2.0).round().clamp(0.0, 255.0) as u8);
        let dim = |c: egui::Color32| if enabled { c } else { c.gamma_multiply(0.45) };
        let painter = ui.painter();
        painter.rect_filled(egui::Rect::from_min_max(egui::pos2(left, y - line / 2.0), egui::pos2(left + travel, y + line / 2.0)), corner, dim(theme.slider_track()));
        if x > left {
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(left, y - line / 2.0), egui::pos2(x, y + line / 2.0)), corner, dim(theme.accent()));
        }
        let knob_fill = if enabled && (resp.hovered() || resp.dragged()) { theme.text_primary() } else { theme.text_secondary() };
        painter.circle_filled(egui::pos2(x, y), knob, dim(knob_fill));
    }
    crate::gui::widgets::focus_ring(ui, theme, &resp);
    (put, rect)
}

/// The small "Volume settings" mark beside a video's volume bar (2026-11-06): a cog, drawn (no
/// glyph), its words a hint and its name for a screen reader. True when it was pressed; the caller
/// opens Settings > Audio, where the choice of what volume a new video starts at is.
pub fn volume_settings_mark(ui: &mut egui::Ui, theme: &Theme) -> bool {
    use crate::gui::widgets::hint::Hint;
    let size = theme.font_size_small + 4.0;
    let (rect, resp) = crate::gui::widgets::icons::icon_button(ui, size);
    let resp = resp.hint("chat_video.volume_settings_hint");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, t("chat_video.volume_settings")));
    if ui.is_rect_visible(rect) {
        let color = if resp.hovered() || resp.has_focus() { theme.text_primary() } else { theme.text_muted() };
        crate::gui::widgets::icons::paint_cog(ui.painter(), rect.shrink(2.0), color);
    }
    crate::gui::widgets::focus_ring(ui, theme, &resp);
    resp.clicked()
}

#[cfg(test)]
mod start_volume_tests {
    use super::*;
    use humanity_core::audio::start_volume::{self as sv, StartAt};

    /// THE START VOLUME APPLIES TO THE MEDIA PAGE'S PLAYER (2026-11-06): Play here
    /// (`files::open::start_play`, which calls `VideoView::start`) starts a video at a set 5%; with
    /// "my last volume" at the volume the last video was left at. The fetch fails at once (no
    /// network here): the volume is the view's from the moment it starts. On this thread's own
    /// choice. RED first: with `start` not asking the start volume, "a set 5%" failed (left: 1.0,
    /// right: 0.05).
    #[test]
    fn a_media_page_video_starts_at_the_chosen_start_volume() {
        sv::use_on_this_thread(StartAt::Set, 0.05, 0.8);
        let mut v = VideoView::default();
        v.start("", "clip", || Err("not fetched in this test".to_string()));
        assert_eq!(v.volume, 0.05, "a set 5%");
        sv::set_choice(StartAt::Last, 0.05);
        let mut w = VideoView::default();
        w.start("", "clip", || Err("not fetched in this test".to_string()));
        assert_eq!(w.volume, 0.8, "my last volume");
    }
}
