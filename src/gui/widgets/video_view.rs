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
    /// Its sound: handed to the mixer once, muted with Mute, at the master volume last sent.
    sound_attached: bool,
    pub muted: bool,
    pub(crate) volume_sent: Option<f64>,
    /// Its own volume, 0 to 1, on top of the app's master volume (a chat row's volume bar).
    pub volume: f32,
    /// Where it starts when its player opens, in seconds (a chat video played again after its row
    /// left the screen carries on from its place). 0: the start.
    pub start_at: f64,
    /// The last picture shown, kept (moved, not copied) for [`VideoView::still_image`].
    last_frame: Option<crate::media::VideoFrame>,
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
            volume_sent: None,
            volume: 1.0,
            start_at: 0.0,
            last_frame: None,
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
        let (ffmpeg_setting, name) = (ffmpeg_setting.to_string(), name.to_string());
        std::thread::spawn(move || {
            let fetched = fetch().map(|copy| {
                // A conversion is written beside the copy: the temporary folder, swept at every start.
                let dir = copy.parent().map(|p| p.to_path_buf()).unwrap_or_else(std::env::temp_dir);
                let ingest = crate::media::transcode::begin_ingest_as(&copy, &name, &ffmpeg_setting, &dir);
                (copy, ingest)
            });
            // The view went while it was fetching: nobody will play it, so it goes now (a conversion
            // stops when its job is dropped, and takes its part file with it).
            if let Err(std::sync::mpsc::SendError(Ok((copy, _)))) = tx.send(fetched) {
                let _ = std::fs::remove_file(copy);
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
            if let Some(frame) = player.poll() {
                // Every decoder here writes opaque pictures (alpha 255: media/video.rs and
                // media/system.rs), for which premultiplied and unmultiplied are the same bytes;
                // the premultiplied form is a plain copy, where the unmultiplied one branches on
                // every pixel's alpha (2026-11-06: measured in chat::inline_video::tests::bench).
                let image = egui::ColorImage::from_rgba_premultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
                match self.texture.as_mut() {
                    Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                    None => self.texture = Some(ctx.load_texture("video_view_frame", image, egui::TextureOptions::LINEAR)),
                }
                // Kept for a row's still (moved, not copied: the frame was going to be dropped here).
                self.last_frame = Some(frame);
            }
            if let Some(e) = player.take_error() {
                self.problem = tf("video.stopped", &[("error", &e)]);
            }
            if player.is_playing() {
                ctx.request_repaint();
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

    /// The video's sound from the app's mixer: handed over once, then kept at the master volume,
    /// or silent while muted.
    pub fn sync_sound(&mut self, audio: &mut humanity_core::audio::AudioManager) {
        let Some(player) = self.player.as_mut() else { return };
        if player.info().audio_track.is_none() {
            return;
        }
        let volume = if self.muted { 0.0 } else { audio.master_volume() * self.volume.clamp(0.0, 1.0) as f64 };
        if !self.sound_attached {
            self.sound_attached = true;
            let opts = crate::media::AudioAttach { looping: false, volume, panning: 0.5 };
            if let Err(e) = player.attach_audio_with(audio, opts) {
                log::warn!("media: the video's sound did not start: {e}");
                return;
            }
            self.volume_sent = Some(volume);
            return;
        }
        if self.volume_sent != Some(volume) {
            player.set_audio_mix(volume, 0.5, 30);
            self.volume_sent = Some(volume);
        }
    }
}

impl Drop for VideoView {
    /// The player lets go of the file first, then the temporary copy goes.
    fn drop(&mut self) {
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
    put
}
