//! A video played in the app: fetched first to a temporary copy on a worker, then played by the
//! app's own player (`crate::media::VideoPlayer`: WebM with AV1 video, and on Windows an ordinary
//! MP4 through the computer's own decoders, `media/system.rs`), with Play, Pause, Start again, Mute
//! and its time, and its sound through the app's mixer ([`VideoView::sync_sound`], which the
//! engine calls each frame it has a mixer). The temporary copy goes with the view.
//!
//! One widget, so a Media item's video and a video posted in chat play the same way (2026-10-18;
//! the Media page's player moved here from `pages/files/open.rs`). The website's twin is the
//! browser's own `<video>` element.

use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use egui::RichText;

use crate::gui::theme::Theme;
use crate::gui::widgets;

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

/// What it says when the file will not play: the player's reason, then what to do.
pub fn cannot_play_words(why: &str) -> String {
    format!("This video cannot play in the app: {why}. Download it to watch it in another player.")
}

/// A video being fetched, playing, or that could not play.
#[derive(Default)]
pub struct VideoView {
    fetch_rx: Option<Receiver<Result<PathBuf, String>>>,
    pub(crate) player: Option<crate::media::VideoPlayer>,
    texture: Option<egui::TextureHandle>,
    /// Why it cannot play, or why the fetch failed.
    pub problem: String,
    /// The temporary copy being played, deleted with the view.
    pub temp: Option<PathBuf>,
    /// Its sound: handed to the mixer once, muted with Mute, at the master volume last sent.
    sound_attached: bool,
    pub muted: bool,
    pub(crate) volume_sent: Option<f64>,
}

impl VideoView {
    /// Fetch the video with `fetch` on a worker (it returns the temporary copy), then play it.
    pub fn start(&mut self, fetch: impl FnOnce() -> Result<PathBuf, String> + Send + 'static) {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            // The view went while it was fetching: nobody will play it, so it goes now.
            if let Err(std::sync::mpsc::SendError(Ok(path))) = tx.send(fetch()) {
                let _ = std::fs::remove_file(path);
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

    /// Take what the fetch finished and the next frame. Every frame the view is drawn.
    pub fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = self.fetch_rx.as_ref() {
            match rx.try_recv() {
                Ok(result) => {
                    self.fetch_rx = None;
                    if let Ok(path) = &result {
                        self.temp = Some(path.clone()); // deleted with the view from here on
                    }
                    match result.and_then(|path| crate::media::VideoPlayer::open(&path).map_err(|e| cannot_play_words(&e.to_string()))) {
                        Ok(mut player) => {
                            player.play();
                            self.player = Some(player);
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
        if let Some(player) = self.player.as_mut() {
            if let Some(frame) = player.poll() {
                let image = egui::ColorImage::from_rgba_unmultiplied([frame.width as usize, frame.height as usize], &frame.rgba);
                match self.texture.as_mut() {
                    Some(tex) => tex.set(image, egui::TextureOptions::LINEAR),
                    None => self.texture = Some(ctx.load_texture("video_view_frame", image, egui::TextureOptions::LINEAR)),
                }
            }
            if let Some(e) = player.take_error() {
                self.problem = format!("The video stopped: {e}");
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
    pub fn sync_sound(&mut self, audio: &mut crate::audio::AudioManager) {
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
