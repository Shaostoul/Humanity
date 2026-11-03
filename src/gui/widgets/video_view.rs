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
/// The button that closes a window a video plays in.
pub const CLOSE: &str = "Close the video";

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
#[derive(Default)]
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

    /// Open the player on `path` and start it, or say why not.
    fn open(&mut self, path: &std::path::Path) {
        match crate::media::VideoPlayer::open(path) {
            Ok(mut player) => {
                player.play();
                self.player = Some(player);
            }
            Err(e) => self.problem = cannot_play_words(&e.to_string()),
        }
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
                let image = egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
                match self.texture.as_mut() {
                    Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                    None => self.texture = Some(ctx.load_texture("video_view_frame", image, egui::TextureOptions::LINEAR)),
                }
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
        let volume = if self.muted { 0.0 } else { audio.master_volume() };
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
