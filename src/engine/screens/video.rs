//! The video provider (in-world screens ladder, rung 5, integration): a
//! `video:<path>` screen plays a clip through the purpose-built player in
//! `src/media`, looping, with its sound placed in the world at the screen,
//! and since 2026-09-18 the player can CHOOSE THE FILE from inside the app.
//! The operator's words: "movies on displays", then "I want to play a video
//! of my own on it". Design: docs/design/in-world-screens.md, the "Video
//! sources" section; the player itself and the codec policy:
//! docs/design/media-player.md; the conversion: `src/media/transcode.rs`.
//!
//! How the pieces meet:
//!
//! * The PLAYER (`media::VideoPlayer`) decodes on its own thread and keeps
//!   the playback clock. This provider never decodes and never re-gates:
//!   each framed tick it asks `poll()` for the frame that is due (the
//!   player's own gate, which never hands out a frame ahead of its clock).
//! * The FRAME lives in the provider's own GPU texture (`FrameTexture`),
//!   uploaded once per decoded frame and registered with the surface's egui
//!   renderer. The surface is then drawn by EGUI: the frame as an image
//!   fitted into the display's rectangle (the GPU scales it, black bars
//!   where the aspects differ, never a stretch), the CONTROL STRIP over its
//!   bottom edge (Open, Play/Pause, the file name, the time), the file
//!   picker as a window, and the notices (converting, cannot play) as
//!   text. One draw path for every state, so the controls are always
//!   reachable: a clip that failed to open still has its Open button.
//! * The strip shows while the screen is being looked at (the pointer
//!   moved on it in the last `STRIP_HOLD`), while paused, while a
//!   conversion runs, while the picker is open and while there is nothing
//!   to show; it hides over a playing film, the way a player's controls do.
//!   When it is hidden and no new frame arrived, the tick draws nothing
//!   (`FrameDraw::Keep`): the texture keeps the picture and egui does not
//!   run, so a playing film costs one upload per decoded frame and nothing
//!   between them.
//! * CHOOSING A FILE: Open shows the in-app file picker (the same widget the
//!   chat attach button uses) filtered to video extensions, starting in the
//!   Videos folder. The choice is remembered PER SCREEN in the config
//!   (`screen_media`, keyed by the placed instance id), so the data file's
//!   `video:` source is only the default and the remembered file wins on
//!   the next boot. The dev IPC's `video_open` goes through the same
//!   `open_path`.
//! * INGEST: a chosen file the player accepts (WebM AV1 + Opus) opens at
//!   once; any other file is converted once by the machine's ffmpeg into
//!   the media cache and the converted copy plays (`transcode::begin_ingest`
//!   decides; the strip shows the percentage while it runs; a cache hit
//!   skips ffmpeg). No ffmpeg, no encoder, a bad file: an on-screen message
//!   naming the fix, never a panic, never a silent black screen.
//! * The SOUND is the clip's Opus track through kira, placed at the screen:
//!   every frame the engine tells the provider where its screen and the
//!   listener are (`world_update`), and the provider sets the stream's
//!   volume from the distance and its stereo pan from the screen's bearing.
//! * A CLICK on the picture (`on_button`) toggles pause; a click on the
//!   strip or in the picker is egui's (the button under it acts).
//! * LOOPING: the clip restarts when the player reports its end, and only
//!   while it is meant to be playing (a clip paused on its last frame stays
//!   there); the sound loops on kira's side (a loop region on the stream),
//!   see `advance`.
//!
//! The file is in two halves so the logic is testable on a machine with no
//! GPU: `open_with`, `open_path`, `advance`, `plan_frame`, the pause
//! toggle, `picture_layout`, `letterbox_layout`, `resolve_media_path`,
//! `StripTimer`, `strip_wanted`, `draw_screen`, `audio_placement`,
//! `compose_mix`, `mix_moved` and `SoundLink::step` are pure, egui-only or
//! player-only and every test below drives them; `frame`, `upload_frame`
//! and `world_update` are thin glue over them (the GPU calls and the kira
//! calls respectively).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use glam::Vec3;

use crate::gui::screen_surface::{LoadState, ScreenCore, ScreenProvider, ScreenSurface, ScreenWorld};
use crate::gui::theme::Theme;
use crate::gui::widgets::file_browser::{file_picker_modal, FilePickerResult, FilePickerState};
use crate::gui::widgets::{self, ButtonVariant};
use crate::gui::GuiState;
use crate::media::transcode::{self, Ingest, TranscodeJob};
use crate::media::{AudioAttach, VideoFrame, VideoPlayer};

/// Beyond this distance from the listener a screen's sound is silent. The
/// same 50 m the one-shot spatial path (`AudioManager::play_spatial`) uses,
/// with the same linear falloff, so a clip and a footstep fade alike.
pub const AUDIO_MAX_DISTANCE_M: f32 = 50.0;
/// How far the stereo pan swings for a screen straight to one side: 0.7 puts
/// it at 0.85 (kira: 0 hard left, 0.5 centre, 1 hard right), a clear bias
/// that never silences the other ear (a room carries sound to both).
pub const PAN_STRENGTH: f32 = 0.7;
/// Volume and pan changes smaller than this are not sent to the audio
/// thread, so a listener standing still does not restart a tween every frame.
pub const MIX_EPSILON: f64 = 0.004;
/// Tween for volume and pan updates: long enough that walking past a screen
/// glides instead of stepping, short enough that turning the head feels
/// immediate.
pub const MIX_TWEEN_MS: u64 = 60;
/// How long the control strip stays up after the pointer last moved on the
/// screen while a film plays. Three seconds is what desktop players use.
pub const STRIP_HOLD: Duration = Duration::from_secs(3);
/// The strip's background: the theme's primary background at this alpha,
/// so the picture shows through a little and the strip reads as an overlay
/// rather than a cut.
pub const STRIP_ALPHA: u8 = 215;

/// Where a `video:<path>` points: the game DATA dir first (`data/media/x.webm`,
/// the distributed and moddable tree), then the data dir's parent (the dev
/// repo root, so a checkout can name `tests/fixtures/media/x.webm`); the same
/// rule GLB models use (docs/game/model-pipeline.md, `resolve_model_path`).
/// `None` when the file exists in neither place. An absolute path joins as
/// itself, so it simply works when it exists.
pub fn resolve_media_path(data_dir: &Path, rel: &str) -> Option<PathBuf> {
    let in_data = data_dir.join(rel);
    if in_data.is_file() {
        return Some(in_data);
    }
    let in_root = data_dir.parent()?.join(rel);
    if in_root.is_file() {
        return Some(in_root);
    }
    None
}

/// The canvas of the DISPLAY's aspect that contains a frame at 1:1, and the
/// frame's offset inside it. Used by `picture_layout` for a frame LARGER
/// than the display (the surface then takes this size so every pixel of
/// the clip is kept and the wall's sampler scales it down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Letterbox {
    pub canvas_w: u32,
    pub canvas_h: u32,
    /// Left edge of the frame inside the canvas (bars on the left and right).
    pub x: u32,
    /// Top edge of the frame inside the canvas (bars above and below).
    pub y: u32,
}

impl Letterbox {
    /// True when the frame fills the canvas exactly (no bars).
    pub fn is_identity(&self, frame_w: u32, frame_h: u32) -> bool {
        self.x == 0 && self.y == 0 && self.canvas_w == frame_w && self.canvas_h == frame_h
    }
}

/// The letterbox for a `frame_w` x `frame_h` picture on a display whose
/// pixel size (and so aspect) is `display_w` x `display_h`. The frame is
/// never scaled: a wider frame gets bars above and below, a taller one gets
/// bars left and right. The aspect comparison is done in integers
/// (`fw * dh` against `dw * fh`) so equal aspects compare exactly.
pub fn letterbox_layout(frame_w: u32, frame_h: u32, display_w: u32, display_h: u32) -> Letterbox {
    let (fw, fh) = (frame_w.max(1), frame_h.max(1));
    let (dw, dh) = (display_w.max(1), display_h.max(1));
    let lhs = fw as u64 * dh as u64;
    let rhs = dw as u64 * fh as u64;
    if lhs > rhs {
        // The frame is wider than the display: keep its width, grow the
        // canvas down to the display's aspect, centre the frame vertically.
        let canvas_h = ((fw as f64) * (dh as f64) / (dw as f64)).round().max(fh as f64) as u32;
        Letterbox { canvas_w: fw, canvas_h, x: 0, y: (canvas_h - fh) / 2 }
    } else if lhs < rhs {
        // Taller than the display: keep its height, widen the canvas.
        let canvas_w = ((fh as f64) * (dw as f64) / (dh as f64)).round().max(fw as f64) as u32;
        Letterbox { canvas_w, canvas_h: fh, x: (canvas_w - fw) / 2, y: 0 }
    } else {
        Letterbox { canvas_w: fw, canvas_h: fh, x: 0, y: 0 }
    }
}

/// Where the picture goes: the SURFACE size to draw at and the rectangle
/// (x, y, w, h, in surface pixels) the frame is fitted into, centred, with
/// the display's own background around it.
///
/// The rule: a frame smaller than the display in both dimensions is drawn
/// on a surface of the DISPLAY's size, scaled UP by the GPU to fit (so the
/// control strip and the notices are laid out at the display's resolution,
/// crisp on the wall, whatever the clip's size; the demo is 320 x 180 and a
/// strip drawn at that size would be a four-times-upscaled blur). A frame
/// as large as the display or larger in either dimension gets the
/// letterbox canvas at ITS resolution, so every pixel of a 1080p clip on a
/// 720p wall def survives to the wall's own sampler (the "every pixel,
/// never stretched" rule of the first rung). Either way the frame's aspect
/// is kept exactly: `w / h == fw / fh` to a pixel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Picture {
    pub surface_w: u32,
    pub surface_h: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Picture {
    /// The scale applied to the frame (1.0 = drawn at its own size).
    pub fn scale(&self, frame_w: u32) -> f32 {
        self.w / frame_w.max(1) as f32
    }
}

pub fn picture_layout(frame_w: u32, frame_h: u32, display_w: u32, display_h: u32) -> Picture {
    let (fw, fh) = (frame_w.max(1), frame_h.max(1));
    let (dw, dh) = (display_w.max(1), display_h.max(1));
    let (sw, sh) = if fw >= dw || fh >= dh {
        let lb = letterbox_layout(fw, fh, dw, dh);
        (lb.canvas_w, lb.canvas_h)
    } else {
        (dw, dh)
    };
    let scale = f64::min(sw as f64 / fw as f64, sh as f64 / fh as f64);
    let w = fw as f64 * scale;
    let h = fh as f64 * scale;
    Picture {
        surface_w: sw,
        surface_h: sh,
        x: ((sw as f64 - w) / 2.0) as f32,
        y: ((sh as f64 - h) / 2.0) as f32,
        w: w as f32,
        h: h as f32,
    }
}

/// Volume factor and stereo pan for a sound at `screen` heard by a listener
/// at `listener` whose right-hand direction is `right`. Volume follows
/// `AudioManager::play_spatial`'s law: linear falloff to silence at
/// `AUDIO_MAX_DISTANCE_M`. Pan comes from the screen's BEARING (where the
/// one-shot path uses a world-axis offset, fine for a footstep that lasts a
/// third of a second): 0.5 for a screen straight ahead or straight behind,
/// swung toward the side it is on, so turning around swaps the ears the way
/// a real room does. Returns `(falloff, pan)`; the caller multiplies the
/// falloff by the bus volumes.
pub fn audio_placement(screen: Vec3, listener: Vec3, right: Vec3) -> (f64, f64) {
    let to = screen - listener;
    let dist = to.length();
    let falloff = (1.0 - dist / AUDIO_MAX_DISTANCE_M).clamp(0.0, 1.0);
    // The sine of the angle off the listener's forward axis, signed toward
    // their right: -1 hard left, 0 ahead or behind, +1 hard right.
    let side = if dist > 1.0e-3 { (to.dot(right.normalize_or_zero()) / dist).clamp(-1.0, 1.0) } else { 0.0 };
    let pan = 0.5 + 0.5 * PAN_STRENGTH * side;
    (falloff as f64, pan as f64)
}

/// The amplitude a screen's sound plays at: the master slider, the sfx bus
/// slider (a film on the wall is a world sound, on the same bus as
/// footsteps and machines, so the Settings sliders govern it) and the
/// distance falloff from `audio_placement`, multiplied. All three are 0..1.
/// Pure, so the composition is a unit test and not an ear test: a bus
/// forgotten here (the sfx slider silently not governing films) fails the
/// test instead of going unnoticed until someone turns the slider down.
pub fn compose_mix(master: f64, sfx: f64, falloff: f64) -> f64 {
    master * sfx * falloff
}

/// The change gate: whether `next` differs from the last mix `sent` by more
/// than `MIX_EPSILON` on EITHER channel. Nothing sent yet always counts as
/// moved. This is what keeps a listener standing still from restarting a
/// 60 ms tween every frame: kira would hold the value at its target anyway,
/// but every restart is a message to the audio thread.
pub fn mix_moved(sent: Option<(f64, f64)>, next: (f64, f64)) -> bool {
    match sent {
        None => true,
        Some((v, p)) => (v - next.0).abs() > MIX_EPSILON || (p - next.1).abs() > MIX_EPSILON,
    }
}

/// What one world tick does to the sound, decided by `SoundLink::step`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SoundStep {
    /// The first tick with a player and a device: attach the stream, and
    /// start it AT this mix (not at some default level to be tweened from:
    /// with the sfx slider at 10 percent and a screen entering range at
    /// 35 m, a stream that started at master volume and then tweened down
    /// spent its first 60 ms about 33 times too loud).
    Attach { volume: f64, pan: f64 },
    /// Attached, and the mix moved past the epsilon: send it with the tween.
    Send { volume: f64, pan: f64 },
    /// Attached, and the mix is where it was: nothing to send.
    Hold,
}

/// The provider's side of the sound hookup, kept apart from the calls into
/// kira so the ORDER of the seam is unit-tested without an audio device: one
/// attach, at the mix the listener's position already implies; after that
/// only the mixes that moved. `world_update` decides WHEN to step (only once
/// the player exists and the machine has an audio device, so on a machine
/// without one the attach stays pending, never marked done) and carries out
/// what the step says.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SoundLink {
    /// An attach has been tried, whatever its outcome: a failed attach is
    /// logged once, not every frame, and is not retried.
    pub attached: bool,
    /// The last (volume, pan) handed to the audio thread: the attach mix,
    /// then every `Send`.
    pub sent: Option<(f64, f64)>,
}

impl SoundLink {
    /// One tick's decision for the mix `(volume, pan)` the world implies now.
    pub fn step(&mut self, volume: f64, pan: f64) -> SoundStep {
        if !self.attached {
            self.attached = true;
            self.sent = Some((volume, pan));
            return SoundStep::Attach { volume, pan };
        }
        if mix_moved(self.sent, (volume, pan)) {
            self.sent = Some((volume, pan));
            return SoundStep::Send { volume, pan };
        }
        SoundStep::Hold
    }
}

/// When the pointer last MOVED on the screen: the strip's auto-hide clock.
/// The look ray reports the same position every frame the player holds
/// still, so only a change of position counts as a move; an `Instant` is
/// passed in rather than read, so the rule is tested without sleeping.
#[derive(Debug, Clone, Copy, Default)]
pub struct StripTimer {
    last_uv: Option<(f32, f32)>,
    last_move: Option<Instant>,
}

impl StripTimer {
    /// Record the pointer's position this tick (`None` = not on the screen).
    pub fn observe(&mut self, uv: Option<(f32, f32)>, now: Instant) {
        if let Some(uv) = uv {
            if self.last_uv != Some(uv) {
                self.last_move = Some(now);
            }
        }
        self.last_uv = uv;
    }

    /// The pointer moved on the screen within the last `STRIP_HOLD`.
    pub fn recently_moved(&self, now: Instant) -> bool {
        self.last_move.map_or(false, |t| now.saturating_duration_since(t) < STRIP_HOLD)
    }
}

/// Whether the control strip is drawn this tick: always while paused,
/// while there is no picture to hide it behind (opening, converting, an
/// error), while the picker is open, and otherwise only while the pointer
/// recently moved on the screen.
pub fn strip_wanted(paused: bool, no_picture: bool, picker_open: bool, recently_moved: bool) -> bool {
    paused || no_picture || picker_open || recently_moved
}

/// What a framed tick draws, decided by `plan_frame` with no GPU in hand so
/// the decision is unit-tested; `frame` is the glue that carries it out.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameDraw {
    /// Run egui at `px` (the surface size `picture_layout` chose, or the
    /// display's when there is no picture): the frame image if there is
    /// one, the strip if `strip`, the picker if open, a notice otherwise.
    /// `upload` says a new decoded frame must go to the GPU first.
    Compose { px: (u32, u32), upload: bool, strip: bool },
    /// Nothing changed this tick: the texture keeps what it has, which is
    /// exactly what a display does between frames, and egui does not run.
    Keep,
}

/// Everything `draw_screen` needs, as owned values, so the egui closure
/// borrows nothing of the provider (the surface it runs on is borrowed
/// mutably for the run).
#[derive(Debug, Clone, PartialEq)]
pub struct ScreenView {
    /// The frame texture as (egui id, width, height), once a frame exists.
    pub tex: Option<(egui::TextureId, u32, u32)>,
    /// The display's pixel size (the def's `px`): the strip lays out
    /// against its width.
    pub display_px: (u32, u32),
    pub strip: bool,
    pub paused: bool,
    /// The file name shown on the strip.
    pub name: String,
    /// The strip's right-hand text: the time, or the conversion percentage.
    pub right_text: String,
    /// A message instead of a picture (cannot play, converting, opening).
    pub notice: Option<String>,
}

/// What the player did on the strip or in the picker during a run.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScreenActions {
    /// The Open button.
    pub open: bool,
    /// The Play / Pause button.
    pub toggle: bool,
    /// The picker confirmed this file.
    pub picked: Option<PathBuf>,
    /// The picker was cancelled.
    pub cancelled: bool,
    /// Where the strip's top edge landed, as a fraction of the surface
    /// height (1.0 when no strip was drawn): `on_button` uses it to leave
    /// clicks on the strip to egui.
    pub strip_top_frac: f32,
}

/// The whole screen, in egui, GPU-free: the picture fitted into the
/// surface over black bars (or the notice on the theme background), the
/// control strip anchored to the bottom edge as a floating area over the
/// picture, and the file picker window when it is open. Runs under the
/// screen's own context through `ScreenCore::run_with`.
pub fn draw_screen(
    ctx: &egui::Context,
    theme: &Theme,
    view: &ScreenView,
    picker: Option<&mut FilePickerState>,
    actions: &mut ScreenActions,
) {
    actions.strip_top_frac = 1.0;
    let screen = ctx.screen_rect();
    // The picture (or the notice) fills the whole surface; the strip floats
    // over it, so a film keeps its full height and the strip is an overlay,
    // not a squeeze.
    match (&view.notice, view.tex) {
        (Some(text), _) => {
            egui::CentralPanel::default()
                .frame(egui::Frame::none().fill(theme.bg_primary()).inner_margin(theme.spacing_md))
                .show(ctx, |ui| {
                    ui.centered_and_justified(|ui| {
                        ui.label(egui::RichText::new(text).color(theme.text_primary()));
                    });
                });
        }
        (None, Some((id, w, h))) => {
            egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::BLACK)).show(ctx, |ui| {
                let (sw, sh) = (screen.width().max(1.0), screen.height().max(1.0));
                let scale = f32::min(sw / w.max(1) as f32, sh / h.max(1) as f32);
                let size = egui::vec2(w as f32 * scale, h as f32 * scale);
                let min = screen.min + (egui::vec2(sw, sh) - size) * 0.5;
                let rect = egui::Rect::from_min_size(min, size);
                ui.painter().image(
                    id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            });
        }
        (None, None) => {
            // Opened but no frame due yet: black, the way a display looks in
            // the instant before the first frame.
            egui::CentralPanel::default().frame(egui::Frame::none().fill(egui::Color32::BLACK)).show(ctx, |_| {});
        }
    }
    if view.strip {
        let bg = theme.bg_primary();
        let fill = egui::Color32::from_rgba_unmultiplied(bg.r(), bg.g(), bg.b(), STRIP_ALPHA);
        let area = egui::Area::new(egui::Id::new("video_strip"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::Vec2::ZERO)
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(fill)
                    .inner_margin(egui::Margin::symmetric(theme.spacing_sm as i8, theme.spacing_xs as i8))
                    .show(ui, |ui| {
                        ui.set_width(screen.width() - 2.0 * theme.spacing_sm);
                        ui.horizontal(|ui| {
                            if widgets::compact_button(ui, theme, "Open", ButtonVariant::Secondary) {
                                actions.open = true;
                            }
                            let label = if view.paused { "Play" } else { "Pause" };
                            if widgets::compact_button(ui, theme, label, ButtonVariant::Primary) {
                                actions.toggle = true;
                            }
                            ui.add_space(theme.spacing_xs);
                            ui.label(
                                egui::RichText::new(&view.name).size(theme.font_size_small).color(theme.text_primary()),
                            );
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(
                                    egui::RichText::new(&view.right_text)
                                        .size(theme.font_size_small)
                                        .color(theme.text_muted()),
                                );
                            });
                        });
                    });
            });
        let top = area.response.rect.top();
        actions.strip_top_frac = (top / screen.height().max(1.0)).clamp(0.0, 1.0);
    }
    if let Some(p) = picker {
        match file_picker_modal(ctx, theme, p, "Open a video") {
            FilePickerResult::Open => {}
            FilePickerResult::Cancelled => actions.cancelled = true,
            FilePickerResult::Picked(path) => actions.picked = Some(path),
        }
    }
}

/// The decoded frame on the GPU: the provider's own texture, registered
/// with the surface's egui renderer under `id` so `draw_screen` can draw
/// it. Reallocated (and the id re-pointed) when the clip's size changes.
struct FrameTexture {
    texture: wgpu::Texture,
    id: egui::TextureId,
    w: u32,
    h: u32,
}

/// The per-screen state of one `video:<path>` source.
pub struct VideoProvider {
    /// The path as written after `video:` in the data file: the default.
    source: String,
    /// The file the player chose (the picker, the IPC, or the config's
    /// remembered choice); `None` while the default plays.
    chosen: Option<PathBuf>,
    /// `chosen` must be written to the config on the next `frame` (the
    /// only place with the GUI state in hand).
    remember_pending: bool,
    /// The config was consulted once for a remembered file.
    adopted: bool,
    /// The Settings > Media ffmpeg path, mirrored from the GUI state each
    /// frame so `open_path` (which the IPC calls between frames) has it.
    ffmpeg_setting: String,
    /// The media cache, resolved on first use (tests point it at scratch).
    cache_dir: Option<PathBuf>,
    /// The file that actually plays: the source, or its converted copy.
    resolved: Option<PathBuf>,
    /// The player, once opened. `None` before the first frame, while a
    /// conversion runs, and after an open error.
    player: Option<VideoPlayer>,
    /// A conversion in flight; its result opens the player.
    job: Option<TranscodeJob>,
    /// Why a conversion was needed, for the notice ("uses the video codec
    /// V_VP8, which this player does not decode").
    convert_why: Option<String>,
    /// Set once an open has been tried, whatever the outcome, so a failed
    /// open is shown on the screen and not retried every frame.
    open_attempted: bool,
    /// Why there is no picture: an open error (the path, the codec, the
    /// missing ffmpeg) or a decode error from the player's thread.
    error: Option<String>,
    /// The state the PLAYER wants: true = playing. Kept even before the
    /// player exists, so a click on a screen whose clip is still opening is
    /// honoured when it opens.
    want_playing: bool,
    /// The newest frame taken from the player, kept while paused.
    last: Option<VideoFrame>,
    /// `last` has not been uploaded to the frame texture yet.
    frame_dirty: bool,
    /// How many times the clip has wrapped back to its start.
    loops: u64,
    /// The display's pixel size (the def's `px`), read from the surface on
    /// the first frame before any resize. Pictures are fitted to ITS
    /// aspect, which the def author matched to the physical display.
    display_px: Option<(u32, u32)>,
    /// The sound hookup's state: attached yet, and the last mix sent. The
    /// attach needs the audio manager, which arrives through `world_update`.
    sound: SoundLink,
    /// The Open picker while it is up.
    picker: Option<FilePickerState>,
    strip: StripTimer,
    /// Whether the last run drew the strip (a change forces a redraw so a
    /// hidden strip is really gone from the texture).
    strip_shown: bool,
    /// The strip's top edge as a fraction of the surface height after the
    /// last run (1.0 = no strip): clicks below it are egui's.
    strip_top_frac: f32,
    /// The frame on the GPU.
    tex: Option<FrameTexture>,
    /// What the last run's notice said, to redraw when it changes (the
    /// percentage ticking up).
    last_notice: Option<String>,
    /// The last open was served from the media cache (no ffmpeg run).
    cache_hit: bool,
    /// How many conversions this provider started.
    transcodes: u64,
    /// The thing being opened is a DISC (a folder, `src/media/dvd.rs`), not
    /// a single file. Kept so a conversion that fails later still gets the
    /// plain answer about copy protection instead of a decoder error.
    disc_source: bool,
}

impl VideoProvider {
    /// A provider for `video:<path>`. Nothing is opened here: the decode
    /// thread starts on the first frame, so a screen the player never looks
    /// at never spawns one.
    pub fn new(path: &str) -> Self {
        Self {
            source: path.to_string(),
            chosen: None,
            remember_pending: false,
            adopted: false,
            ffmpeg_setting: String::new(),
            cache_dir: None,
            resolved: None,
            player: None,
            job: None,
            convert_why: None,
            open_attempted: false,
            error: None,
            want_playing: true,
            last: None,
            frame_dirty: false,
            loops: 0,
            display_px: None,
            sound: SoundLink::default(),
            picker: None,
            strip: StripTimer::default(),
            strip_shown: false,
            strip_top_frac: 1.0,
            tex: None,
            last_notice: None,
            cache_hit: false,
            transcodes: 0,
            disc_source: false,
        }
    }

    /// Use `dir` as the media cache (tests; the app uses `transcode::cache_dir`).
    pub fn with_cache_dir(mut self, dir: PathBuf) -> Self {
        self.cache_dir = Some(dir);
        self
    }

    /// The Settings > Media ffmpeg path to use for conversions.
    pub fn set_ffmpeg_setting(&mut self, setting: &str) {
        if self.ffmpeg_setting != setting {
            self.ffmpeg_setting = setting.to_string();
        }
    }

    fn cache_dir(&mut self) -> PathBuf {
        self.cache_dir.get_or_insert_with(transcode::cache_dir).clone()
    }

    /// The name shown on the strip and in messages: the chosen file's name,
    /// else the source string as written in the data file.
    pub fn current_name(&self) -> String {
        match &self.chosen {
            Some(p) => p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| p.display().to_string()),
            None => self.source.clone(),
        }
    }

    /// Open the player on a file the player accepts. A failure becomes the
    /// screen's error; it never panics and is never retried per frame.
    fn open_player(&mut self, path: PathBuf) {
        match VideoPlayer::open(&path) {
            Ok(mut p) => {
                if self.want_playing {
                    p.play();
                }
                log::info!(
                    "[Screens] video {:?} opened from {} ({}x{}, {:.2} s, audio: {})",
                    self.current_name(),
                    path.display(),
                    p.info().width,
                    p.info().height,
                    p.duration_s(),
                    p.info().audio_codec.as_deref().unwrap_or("none")
                );
                self.player = Some(p);
            }
            Err(e) => {
                self.error = Some(e.to_string());
                log::warn!("[Screens] video {:?} at {}: {e}", self.current_name(), path.display());
            }
        }
        self.resolved = Some(path);
    }

    /// Start getting `src` onto the screen: play it, serve the cached
    /// conversion, start a conversion, or record why none of that works.
    /// Every previous state (player, job, frame, error) is dropped first,
    /// so a new choice replaces the old film cleanly (the old decoder is
    /// joined and its sound stopped by the player's own Drop).
    fn start_ingest(&mut self, src: PathBuf) {
        self.player = None;
        self.job = None;
        self.convert_why = None;
        self.error = None;
        self.last = None;
        self.frame_dirty = false;
        self.loops = 0;
        self.resolved = None;
        self.cache_hit = false;
        self.sound = SoundLink::default();
        self.open_attempted = true;
        // A folder means a DISC (or the drive holding one): `begin_ingest`
        // sends it to the disc path itself, and this flag is only so that a
        // failure LATER is answered in disc terms.
        self.disc_source = src.is_dir();
        let cache = self.cache_dir();
        match transcode::begin_ingest(&src, &self.ffmpeg_setting, &cache) {
            Ingest::Direct(p) => self.open_player(p),
            Ingest::Cached(p) => {
                self.cache_hit = true;
                self.open_player(p);
            }
            Ingest::Transcoding { job, why } => {
                self.transcodes += 1;
                self.convert_why = Some(why);
                self.resolved = Some(job.dst().to_path_buf());
                self.job = Some(job);
            }
            Ingest::Failed(msg) => {
                log::warn!("[Screens] video {:?}: {msg}", self.current_name());
                self.error = Some(msg);
            }
        }
    }

    /// Open the data file's default source against `data_dir` if nothing
    /// has been opened yet. `display_px` is the surface's pixel size at
    /// this moment (the def's `px`, since nothing has resized it yet). A
    /// missing file is the screen's error page, naming both places it was
    /// looked for.
    pub fn open_with(&mut self, data_dir: &Path, display_px: (u32, u32)) {
        if self.open_attempted {
            return;
        }
        self.display_px = Some(display_px);
        let Some(path) = resolve_media_path(data_dir, &self.source) else {
            self.open_attempted = true;
            let root = data_dir.parent().map(|p| p.join(&self.source));
            self.error = Some(format!(
                "file not found: looked for {} and {}",
                data_dir.join(&self.source).display(),
                root.map(|p| p.display().to_string()).unwrap_or_else(|| "(no repo root)".into())
            ));
            log::warn!("[Screens] video {:?}: {}", self.source, self.error.as_deref().unwrap_or(""));
            return;
        };
        self.start_ingest(path);
    }

    /// `open_with` against the game's resolved data dir (the production path).
    pub fn ensure_open(&mut self, display_px: (u32, u32)) {
        if !self.open_attempted {
            self.open_with(&crate::data_dir(), display_px);
        }
    }

    /// Open a file the PLAYER chose (the picker, the dev IPC): it becomes
    /// this screen's film, remembered in the config on the next frame, and
    /// goes through the same probe / cache / convert path as any source.
    pub fn open_path(&mut self, path: &Path) {
        self.chosen = Some(path.to_path_buf());
        self.remember_pending = true;
        self.picker = None;
        self.start_ingest(path.to_path_buf());
    }

    /// Consult the config's remembered choice for this screen, once, before
    /// the default source is opened: a remembered file wins on boot.
    fn adopt_remembered(&mut self, screen_id: &str, gui_state: &GuiState) {
        if self.adopted {
            return;
        }
        self.adopted = true;
        if let Some(p) = gui_state.settings.screen_media.get(screen_id) {
            log::info!("[Screens] video screen {screen_id}: remembered file {}", p.display());
            self.chosen = Some(p.clone());
            self.start_ingest(p.clone());
        }
    }

    /// Advance playback one tick: finish a conversion that ended, loop if
    /// the clip ended AND it is meant to be playing, take the frame that is
    /// due (if any), surface a decode error. Returns true when a NEW frame
    /// is now in `last`. GPU-free; `plan_frame` calls this and `frame`
    /// then uploads and draws.
    ///
    /// The wrap check runs BEFORE the poll, so a frame delivered by this
    /// call always belongs to the loop count read after it: with the order
    /// reversed the frame taken just before a wrap was stamped with the
    /// next loop's number (the loop test caught that). The cost is that the
    /// restart lands one tick after the clock reaches the end, which is one
    /// frame of the last picture held.
    ///
    /// The wrap is gated on `want_playing`: a clip paused on its last frame
    /// stays on its last frame. Without the gate a pause that landed at the
    /// end rewound the clip on the very next tick, `status().position_s`
    /// jumped to 0 and `loops` counted a wrap nobody saw.
    pub fn advance(&mut self) -> bool {
        if let Some(job) = self.job.as_ref() {
            if let Some(result) = job.result() {
                self.job = None;
                match result {
                    Ok(dst) => self.open_player(dst),
                    Err(e) => {
                        let mut msg = format!("{} could not be converted: {e}", self.current_name());
                        // A disc that gets this far and still fails is most
                        // often a copy-protected one whose header flags did
                        // not say so. Say it plainly rather than leaving a
                        // decoder error nobody can act on (the line names
                        // no tool and offers no way around the protection:
                        // src/media/dvd.rs).
                        if self.disc_source {
                            msg.push('\n');
                            msg.push_str(crate::media::dvd::PROTECTED_HINT);
                        }
                        log::warn!("[Screens] video: {msg}");
                        self.error = Some(msg);
                    }
                }
            }
        }
        let Some(p) = self.player.as_mut() else { return false };
        if self.want_playing && p.at_end() {
            // The clock reached the declared end (and the player stopped it
            // there). Back to the first frame and keep rolling. The sound
            // has already wrapped on kira's side (see
            // `VideoPlayer::attach_audio_with`, `looping`); the seek
            // re-aligns the few milliseconds between the two ends. On a
            // starved machine the decoder can be behind the clock here and
            // the tail frames are dropped: the clock is the master.
            p.seek_to_start();
            self.loops += 1;
            p.play();
        }
        let mut fresh = false;
        // `poll` is the gate: it hands out the newest frame whose pts is at
        // or before the player's clock, or nothing. No second gate here.
        if let Some(f) = p.poll() {
            self.last = Some(f);
            self.frame_dirty = true;
            fresh = true;
        }
        if let Some(e) = p.take_error() {
            log::warn!("[Screens] video {:?}: {e}", self.current_name());
            self.error = Some(e);
        }
        fresh
    }

    /// Flip between playing and paused (a click on the picture, the
    /// strip's Play / Pause button).
    pub fn toggle_pause(&mut self) {
        self.want_playing = !self.want_playing;
        if let Some(p) = self.player.as_mut() {
            if self.want_playing {
                p.play();
            } else {
                p.pause();
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        !self.want_playing
    }

    /// The frame most recently taken from the player, if any.
    pub fn last_frame(&self) -> Option<&VideoFrame> {
        self.last.as_ref()
    }

    /// How many times the clip has wrapped to the start.
    pub fn loops(&self) -> u64 {
        self.loops
    }

    /// The open, conversion or decode error, if there is one.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether the Open picker is up.
    pub fn picker_open(&self) -> bool {
        self.picker.is_some()
    }

    /// Show the Open picker: video files only, starting in the folder of
    /// the current choice, else the Videos folder (the picker's own
    /// default is the home folder), with the game's media folder as an
    /// extra quick root.
    ///
    /// A disc in the drive appears in the quick row on its own
    /// (`quick_roots`), and opening the disc shows a "Play disc" button,
    /// because a film on a disc is a folder of numbered files nobody
    /// should have to understand (`folder_offer`, `src/media/dvd.rs`).
    pub fn show_picker(&mut self) {
        let rules = transcode::ingest_rules();
        let exts: Vec<&str> = rules.video_extensions.iter().map(|s| s.as_str()).collect();
        let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).ok().map(PathBuf::from);
        let start = self
            .chosen
            .as_ref()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .filter(|d| d.is_dir())
            .or_else(|| home.map(|h| h.join("Videos")).filter(|d| d.is_dir()));
        let media_dir = crate::data_dir().join("media");
        self.picker = Some(
            FilePickerState::new(&exts, 0)
                .starting_in(start)
                .with_pick_verb("Open")
                .with_extra_root("Game media", media_dir)
                .with_folder_marker(crate::media::dvd::VIDEO_TS, "Play disc"),
        );
    }

    /// The text of the error page: the file, then the problem (which names
    /// the codec for an unsupported file, both places a missing file was
    /// looked for, or the missing ffmpeg and where to set its path).
    pub fn error_text(&self) -> String {
        format!("Video \"{}\" cannot play.\n{}", self.current_name(), self.error.as_deref().unwrap_or("unknown error"))
    }

    /// The one line the old paused page showed; still the fullest wording
    /// of the paused state, used in status and tests.
    pub fn paused_text(&self) -> String {
        match &self.player {
            Some(p) => format!("Paused at {:.1} s of {:.1} s. Click to play.", p.position_s(), p.duration_s()),
            None => "Paused. Click to play.".to_string(),
        }
    }

    /// The conversion's percentage, when one is running.
    pub fn transcoding_pct(&self) -> Option<f32> {
        self.job.as_ref().map(|j| j.percent())
    }

    /// What is drawn INSTEAD of a picture, if anything: the error, the
    /// conversion (with its reason and percentage), the opening state.
    pub fn notice_text(&self) -> Option<String> {
        if self.error.is_some() {
            return Some(self.error_text());
        }
        if let Some(pct) = self.transcoding_pct() {
            let why = self.convert_why.as_deref().unwrap_or("is not WebM AV1 + Opus");
            return Some(format!(
                "{} {why}.\nConverting it once to WebM AV1 + Opus with ffmpeg: {pct:.0}%",
                self.current_name()
            ));
        }
        if self.player.is_none() {
            return Some(format!("Opening {}", self.current_name()));
        }
        None
    }

    /// The strip's right-hand text: the position and length, or the
    /// conversion percentage.
    fn right_text(&self) -> String {
        if let Some(pct) = self.transcoding_pct() {
            return format!("converting {pct:.0}%");
        }
        match &self.player {
            Some(p) => {
                let prefix = if self.want_playing { "" } else { "Paused " };
                format!("{prefix}{:.1} s / {:.1} s", p.position_s(), p.duration_s())
            }
            None => String::new(),
        }
    }

    /// The view `draw_screen` draws, from the current state.
    pub fn view(&self) -> ScreenView {
        let notice = self.notice_text();
        let strip = self.strip_shown;
        ScreenView {
            tex: if notice.is_none() { self.tex.as_ref().map(|t| (t.id, t.w, t.h)) } else { None },
            display_px: self.display_px.unwrap_or((1280, 720)),
            strip,
            paused: !self.want_playing,
            name: self.current_name(),
            right_text: self.right_text(),
            notice,
        }
    }

    /// The surface size to draw at: the picture's (`picture_layout`) when a
    /// frame is on screen, else the display's.
    fn surface_px(&self) -> (u32, u32) {
        let display = self.display_px.unwrap_or((1280, 720));
        match (&self.last, self.error.is_some()) {
            (Some(f), false) => {
                let pic = picture_layout(f.width, f.height, display.0, display.1);
                (pic.surface_w, pic.surface_h)
            }
            _ => display,
        }
    }

    /// Everything `frame` decides, with no GPU in hand: open the default
    /// source on the first call (the core's size at that moment is the
    /// def's `px`), advance playback, decide whether the strip shows, and
    /// say whether anything needs drawing. On a tick with nothing to draw
    /// the core's queued input is dropped (`ScreenCore::drop_pending_events`):
    /// the look ray keeps reporting the pointer and only a run drains the
    /// queue. On a tick that draws, the run consumes them.
    pub fn plan_frame(&mut self, core: &mut ScreenCore, now: Instant) -> FrameDraw {
        let first = self.display_px.is_none();
        self.ensure_open(core.size());
        if self.display_px.is_none() {
            self.display_px = Some(core.size());
        }
        let fresh = self.advance();
        self.strip.observe(core.pointer_uv(), now);
        let no_picture = self.notice_text().is_some() || self.last.is_none();
        let strip = strip_wanted(!self.want_playing, no_picture, self.picker.is_some(), self.strip.recently_moved(now));
        let notice = self.notice_text();
        let changed = fresh
            || self.frame_dirty
            || strip
            || strip != self.strip_shown
            || self.picker.is_some()
            || notice != self.last_notice
            || core.find_pending()
            || first;
        if !changed {
            core.drop_pending_events();
            return FrameDraw::Keep;
        }
        self.strip_shown = strip;
        FrameDraw::Compose { px: self.surface_px(), upload: self.frame_dirty, strip }
    }

    /// Put `last` on the GPU: (re)allocate the frame texture at the clip's
    /// size when it changed, register or re-point the egui id, write the
    /// bytes. sRGB bytes into an sRGB texture, exactly as the surface.
    fn upload_frame(&mut self, surface: &mut ScreenSurface, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(f) = self.last.as_ref() else { return };
        if f.rgba.len() != (f.width as usize) * (f.height as usize) * 4 || f.width == 0 || f.height == 0 {
            log::warn!("[Screens] video: frame of {} bytes for {}x{} dropped", f.rgba.len(), f.width, f.height);
            self.frame_dirty = false;
            return;
        }
        let needs_alloc = self.tex.as_ref().map_or(true, |t| t.w != f.width || t.h != f.height);
        if needs_alloc {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Video Frame"),
                size: wgpu::Extent3d { width: f.width, height: f.height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::gui::screen_surface::SURFACE_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let id = match self.tex.as_ref() {
                Some(old) => {
                    surface.update_native_texture(device, &view, old.id);
                    old.id
                }
                None => surface.register_native_texture(device, &view),
            };
            self.tex = Some(FrameTexture { texture, id, w: f.width, h: f.height });
        }
        let tex = self.tex.as_ref().expect("allocated just above");
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &tex.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &f.rgba,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * f.width), rows_per_image: Some(f.height) },
            wgpu::Extent3d { width: f.width, height: f.height, depth_or_array_layers: 1 },
        );
        self.frame_dirty = false;
    }

    /// Carry out what the run reported: Open shows the picker, Play/Pause
    /// toggles, a picked file opens, a cancel closes. `picker` is the
    /// state the run drew with, handed back unless the run ended it.
    fn apply_actions(&mut self, actions: ScreenActions, picker: Option<FilePickerState>) {
        self.strip_top_frac = actions.strip_top_frac;
        self.picker = picker;
        if actions.cancelled {
            self.picker = None;
        }
        if let Some(path) = actions.picked {
            self.open_path(&path);
        }
        if actions.toggle {
            self.toggle_pause();
        }
        if actions.open {
            self.show_picker();
        }
    }

    /// Write the chosen file into the config, keyed by this screen, through
    /// the normal save path.
    fn remember(&mut self, screen_id: &str, gui_state: &mut GuiState) {
        if !self.remember_pending {
            return;
        }
        self.remember_pending = false;
        if let Some(p) = &self.chosen {
            gui_state.settings.screen_media.insert(screen_id.to_string(), p.clone());
            crate::config::AppConfig::from_gui_state(gui_state).save();
            log::info!("[Screens] video screen {screen_id}: remembered {}", p.display());
        }
    }
}

impl ScreenProvider for VideoProvider {
    fn frame(
        &mut self,
        surface: &mut ScreenSurface,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &mut Theme,
        gui_state: &mut GuiState,
    ) {
        // The settings this provider needs from the GUI state, and the
        // remembered choice, before anything is opened.
        let setting = gui_state.settings.ffmpeg_path.clone();
        self.set_ffmpeg_setting(&setting);
        let screen_id = surface.core.id.clone();
        self.adopt_remembered(&screen_id, gui_state);
        // The decision is GPU-free and tested (`plan_frame`); this is only
        // the GPU work it asks for.
        match self.plan_frame(&mut surface.core, Instant::now()) {
            FrameDraw::Keep => {}
            FrameDraw::Compose { px, upload, strip: _ } => {
                if upload {
                    self.upload_frame(surface, device, queue);
                }
                // A no-op when the surface is already at `px`; a change
                // makes `frame_surfaces` rebind the scene material.
                surface.resize(device, px.0, px.1);
                let view = self.view();
                let mut picker = self.picker.take();
                let mut actions = ScreenActions::default();
                surface.run_and_render(device, queue, theme, gui_state, |core, theme, state| {
                    core.run_with(state, |ctx, _| draw_screen(ctx, theme, &view, picker.as_mut(), &mut actions))
                });
                self.last_notice = view.notice;
                self.apply_actions(actions, picker);
            }
        }
        self.remember(&screen_id, gui_state);
    }

    fn kind(&self) -> &'static str {
        "video"
    }

    fn status(&self) -> serde_json::Value {
        let (playing, position_s, duration_s, audio) = match &self.player {
            Some(p) => (p.is_playing(), p.position_s(), p.duration_s(), p.has_audio()),
            None => (false, 0.0, 0.0, false),
        };
        // The media object carries no null: `merge_provider_status` only
        // strips nulls at the top level, and a rig reads these fields as
        // "present means known".
        let mut media = serde_json::Map::new();
        media.insert("source".into(), serde_json::json!(self.source));
        if let Some(c) = &self.chosen {
            media.insert("chosen".into(), serde_json::json!(c.display().to_string()));
        }
        if let Some(r) = &self.resolved {
            media.insert("resolved".into(), serde_json::json!(r.display().to_string()));
        }
        // 100 once the file plays (or played directly), the live percentage
        // while converting, absent while nothing is open.
        let pct = match (&self.job, &self.player) {
            (Some(j), _) => Some(j.percent()),
            (None, Some(_)) => Some(100.0),
            (None, None) => None,
        };
        if let Some(p) = pct {
            media.insert("transcoding_pct".into(), serde_json::json!(p));
        }
        media.insert("cache_hit".into(), serde_json::json!(self.cache_hit));
        media.insert("transcodes".into(), serde_json::json!(self.transcodes));
        media.insert("disc".into(), serde_json::json!(self.disc_source));
        // Why a conversion was needed, in the same words the screen shows:
        // a rig reading this can tell the disc path from the file path.
        if let Some(w) = &self.convert_why {
            media.insert("convert_why".into(), serde_json::json!(w));
        }
        if let Some(e) = &self.error {
            media.insert("error".into(), serde_json::json!(e));
        }
        serde_json::json!({
            "path": self.source,
            "resolved": self.resolved.as_ref().map(|p| p.display().to_string()),
            "playing": playing,
            "paused": !self.want_playing,
            "position_s": position_s,
            "duration_s": duration_s,
            "loops": self.loops,
            "audio": audio,
            "strip": self.strip_shown,
            "picker": self.picker.is_some(),
            "error": self.error,
            "media": serde_json::Value::Object(media),
        })
    }

    /// A press on the PICTURE toggles pause; the release does nothing (a
    /// click is one toggle, not two). A press on the strip or while the
    /// picker is open is egui's: the button under it acts, and the film's
    /// state is left to that button. Only THIS provider's screen sees its
    /// own clicks: the surface routes a button event to the provider it
    /// holds, so a click on a page screen or a web screen never reaches a
    /// clip.
    fn on_button(&mut self, uv: (f32, f32), pressed: bool) -> bool {
        if self.picker.is_some() {
            return false;
        }
        if self.strip_shown && uv.1 >= self.strip_top_frac {
            return false;
        }
        if pressed {
            self.toggle_pause();
        }
        pressed
    }

    /// The dev IPC's `video_open`: the same path the picker takes.
    fn open_media(&mut self, path: &Path) -> bool {
        self.open_path(path);
        true
    }

    fn load_state(&self) -> LoadState {
        if let Some(e) = &self.error {
            return LoadState::Error(e.clone());
        }
        if self.job.is_some() || (self.open_attempted && self.player.is_none()) {
            return LoadState::Loading;
        }
        if self.player.is_some() {
            return LoadState::Ready;
        }
        LoadState::Static
    }

    /// Attach the sound once the player and the audio manager both exist,
    /// then place it: volume from the distance, pan from the bearing, both
    /// on the sfx bus under the master volume so the Settings sliders govern
    /// a film on the wall like any other world sound.
    ///
    /// ORDER, load-bearing: the engine calls this BEFORE `frame` on every
    /// tick, for every surface with a provider. So on the tick whose `frame`
    /// opens the player this runs first and finds no player; the attach
    /// then happens on the NEXT tick, exactly once (`SoundLink::step`), at
    /// the mix that tick's listener position implies, and later ticks only
    /// re-send a mix that moved. Nothing is decided until both the player
    /// and a device exist, so on a machine with no audio device the attach
    /// stays pending (never marked done) instead of being skipped.
    fn world_update(&mut self, world: &mut ScreenWorld<'_>) {
        let Some(p) = self.player.as_mut() else { return };
        let Some(audio) = world.audio.as_deref_mut() else { return };
        let (falloff, pan) = audio_placement(
            Vec3::from_array(world.screen_centre),
            Vec3::from_array(world.listener_pos),
            Vec3::from_array(world.listener_right),
        );
        let volume = compose_mix(audio.master_volume(), audio.sfx_volume(), falloff);
        match self.sound.step(volume, pan) {
            SoundStep::Attach { volume, pan } => {
                // The stream starts AT this mix; see `SoundStep::Attach` for
                // why it must not start at a default and tween down.
                let opts = AudioAttach { looping: true, volume, panning: pan };
                if let Err(e) = p.attach_audio_with(audio, opts) {
                    log::warn!("[Screens] video {:?}: sound not attached: {e}", self.source);
                }
            }
            // A no-op on the player when there is no sound (a silent file,
            // a failed attach), so no `has_audio` check is needed here.
            SoundStep::Send { volume, pan } => p.set_audio_mix(volume, pan, MIX_TWEEN_MS),
            SoundStep::Hold => {}
        }
    }
}

impl Drop for VideoProvider {
    /// The screen went away (machine removed, source changed): dropping the
    /// player stops and JOINS its decode thread and stops the kira sound
    /// (`VideoPlayer::drop`), and dropping a conversion in flight KILLS its
    /// ffmpeg (`TranscodeJob::drop`), so no decoder or encoder keeps
    /// running for a wall that no longer exists.
    fn drop(&mut self) {
        if let Some(p) = self.player.take() {
            drop(p);
            log::info!("[Screens] video {:?} stopped", self.current_name());
        }
        if let Some(j) = self.job.take() {
            drop(j);
            log::info!("[Screens] video {:?}: conversion cancelled", self.current_name());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::screens::provider_for;
    use crate::gui::screen_surface::{ScreenCore, ScreenSource};
    use crate::gui::theme::load_theme;
    use std::time::{Duration, Instant};

    /// The shipped demo clip, the same bytes as the media tests' fixture
    /// (`data/media/README.md` records the provenance). The tests go through
    /// the production path rule against the checkout's data dir, so what
    /// they open is what a placed screen opens.
    const DEMO: &str = "media/demo_colour_bar.webm";
    /// A file the player refuses, reached through the rule's SECOND branch
    /// (the data dir's parent, the repo root).
    const UNSUPPORTED: &str = "tests/fixtures/media/unsupported-vp8-vorbis.webm";
    /// The transcode fixture: H.264 + AAC in an MP4.
    const MP4: &str = "tests/fixtures/media/moving-box-h264-aac.mp4";

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hum_video_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// Every piece of text egui laid out in a run, joined; how a test reads
    /// what a page SAYS rather than only that it drew something.
    fn shapes_text(shapes: &[egui::epaint::ClippedShape]) -> String {
        fn walk(s: &egui::Shape, out: &mut String) {
            match s {
                egui::Shape::Text(t) => {
                    out.push_str(t.galley.text());
                    out.push('\n');
                }
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = String::new();
        for c in shapes {
            walk(&c.shape, &mut out);
        }
        out
    }

    /// Run the provider's own screen draw headlessly (no GPU: the frame
    /// texture is absent, so a playing clip draws its black panel) and
    /// return the drawn text plus what the run reported.
    fn run_draw(core: &mut ScreenCore, state: &mut GuiState, p: &mut VideoProvider) -> (String, ScreenActions) {
        let theme = load_theme();
        let view = p.view();
        let mut picker = p.picker.take();
        let mut actions = ScreenActions::default();
        // The SAME number of runs the real frame path does
        // (`ScreenCore::runs_this_frame`): twice on a fresh surface, once
        // after. The strip is an egui Area, and an Area measures itself on
        // run 1 and settles on run 2, so a single run draws no strip at all
        // and the layout this returns would not be the one a player sees.
        // The events are consumed by the first run, so a click still lands
        // where it did before.
        let mut text = String::new();
        for _ in 0..core.runs_this_frame() {
            let out = core.run_with(state, |ctx, _| draw_screen(ctx, &theme, &view, picker.as_mut(), &mut actions));
            text = shapes_text(&out.shapes);
        }
        p.picker = picker;
        (text, actions)
    }

    /// The registry serves `video:` with this provider and nothing else
    /// with it.
    #[test]
    fn provider_for_resolves_the_video_scheme() {
        let p = provider_for(&ScreenSource::parse("video:media/x.webm")).expect("a video source gets a provider");
        assert_eq!(p.kind(), "video");
        assert!(provider_for(&ScreenSource::parse("inventory")).is_none(), "a page draws itself");
        assert!(provider_for(&ScreenSource::parse("bogus:x")).is_none(), "an unknown scheme has no provider");
    }

    /// The path rule, on a scratch tree: the data dir wins when the file is
    /// in both places, the repo root serves what the data dir lacks, and a
    /// file in neither resolves to nothing (never to a made-up path).
    #[test]
    fn media_paths_resolve_data_dir_first_then_repo_root() {
        let root = std::env::temp_dir().join(format!("hum_video_paths_{}", std::process::id()));
        let data = root.join("data");
        std::fs::create_dir_all(data.join("media")).unwrap();
        std::fs::create_dir_all(root.join("media")).unwrap();
        std::fs::write(data.join("media/both.webm"), b"d").unwrap();
        std::fs::write(root.join("media/both.webm"), b"r").unwrap();
        std::fs::write(root.join("media/root_only.webm"), b"r").unwrap();

        assert_eq!(resolve_media_path(&data, "media/both.webm"), Some(data.join("media/both.webm")));
        assert_eq!(resolve_media_path(&data, "media/root_only.webm"), Some(root.join("media/root_only.webm")));
        assert_eq!(resolve_media_path(&data, "media/nowhere.webm"), None);
        // A directory is not a file.
        assert_eq!(resolve_media_path(&data, "media"), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The click hook as pure state: a press on the picture toggles, a
    /// release does not, and once a player exists the toggle really pauses
    /// and resumes it.
    #[test]
    fn a_press_toggles_pause_and_a_release_does_not() {
        let mut p = VideoProvider::new(DEMO);
        assert!(!p.is_paused(), "a clip starts playing");
        assert!(p.on_button((0.5, 0.5), true), "the press is taken");
        assert!(p.is_paused());
        assert!(!p.on_button((0.5, 0.5), false), "the release is not");
        assert!(p.is_paused(), "a release must not toggle back");
        p.on_button((0.1, 0.9), true);
        assert!(!p.is_paused());

        // With the real player: the toggle reaches its clock.
        p.open_with(&data_dir(), (1280, 720));
        assert!(p.error().is_none(), "{:?}", p.error());
        assert!(p.player.as_ref().unwrap().is_playing(), "opened in the wanted state (playing)");
        p.on_button((0.5, 0.5), true);
        assert!(!p.player.as_ref().unwrap().is_playing(), "a click pauses the player's clock");
        assert!(p.status()["paused"].as_bool().unwrap());
        p.on_button((0.5, 0.5), true);
        assert!(p.player.as_ref().unwrap().is_playing(), "a second click resumes it");

        // A clip paused BEFORE it opened opens paused.
        let mut q = VideoProvider::new(DEMO);
        q.on_button((0.5, 0.5), true);
        q.open_with(&data_dir(), (1280, 720));
        assert!(!q.player.as_ref().unwrap().is_playing(), "the pre-open pause is honoured on open");
    }

    /// A press on the STRIP is egui's, not a pause toggle: with the strip
    /// drawn over the bottom 8 percent, a press at v = 0.95 leaves the film
    /// alone and a press at v = 0.5 toggles it. With the picker open no
    /// press toggles anything. Proven able to fail: with the strip check
    /// removed from `on_button` the first assertion fires.
    #[test]
    fn a_press_on_the_strip_or_in_the_picker_does_not_toggle() {
        let mut p = VideoProvider::new(DEMO);
        p.strip_shown = true;
        p.strip_top_frac = 0.92;
        assert!(!p.on_button((0.5, 0.95), true), "the strip's button acts, not the film");
        assert!(!p.is_paused());
        assert!(p.on_button((0.5, 0.5), true), "the picture still toggles");
        assert!(p.is_paused());
        p.picker = Some(FilePickerState::new(&["webm"], 0));
        assert!(!p.on_button((0.5, 0.5), false));
        assert!(!p.on_button((0.5, 0.5), true), "the picker owns every click while it is up");
        assert!(p.is_paused(), "unchanged");
    }

    /// Headless: a missing file draws an error page that NAMES the path, on
    /// the same core the surface would run it on, with the strip and its
    /// Open button so the person can choose another file. Proven able to
    /// fail: with the text replaced by a bare "error" the path assertion
    /// fires.
    #[test]
    fn error_page_names_a_missing_file_and_offers_open() {
        let theme = load_theme();
        let mut state = GuiState::default();
        let mut core = ScreenCore::new("s", "video:media/nothing.webm", 640, 360, &theme);
        assert_eq!(core.source, ScreenSource::Video("media/nothing.webm".into()));

        let mut p = VideoProvider::new("media/nothing.webm");
        p.open_with(&data_dir(), core.size());
        assert!(p.player.is_none());
        let err = p.error().expect("a missing file is an error").to_string();
        assert!(err.contains("not found"), "{err}");
        assert!(err.contains("nothing.webm"), "the error names the file: {err}");
        assert!(!p.advance(), "nothing to advance without a player");
        assert_eq!(p.load_state(), LoadState::Error(err.clone()));

        match p.plan_frame(&mut core, Instant::now()) {
            FrameDraw::Compose { strip, .. } => assert!(strip, "no picture: the strip is up"),
            other => panic!("an error draws, got {other:?}"),
        }
        let (drawn, actions) = run_draw(&mut core, &mut state, &mut p);
        assert!(drawn.contains("media/nothing.webm"), "the page must name the path, drew: {drawn:?}");
        assert!(drawn.contains("not found"), "and say why: {drawn:?}");
        assert!(drawn.contains("Open"), "and offer another file: {drawn:?}");
        assert!(actions.strip_top_frac < 1.0, "the strip was drawn: {}", actions.strip_top_frac);
    }

    /// Headless: an unsupported codec (the VP8 fixture, found through the
    /// repo-root branch of the path rule) names the codec on the screen.
    /// With ffmpeg on the machine the file is being CONVERTED and the
    /// notice says so with the reason; without it the error names the
    /// codec and the fix (`FFMPEG_HINT`). Both worlds are honest.
    #[test]
    fn the_screen_names_the_codec_of_an_unsupported_file() {
        let mut state = GuiState::default();
        let theme = load_theme();
        let mut core = ScreenCore::new("s", "video:x", 640, 360, &theme);
        let cache = scratch("vp8");
        let mut p = VideoProvider::new(UNSUPPORTED).with_cache_dir(cache.clone());
        p.open_with(&data_dir(), core.size());
        assert!(p.player.is_none(), "a VP8 file never opens as it is");
        let _ = p.plan_frame(&mut core, Instant::now());
        let (drawn, _) = run_draw(&mut core, &mut state, &mut p);
        assert!(drawn.contains("V_VP8"), "the screen names the codec: {drawn:?}");
        if p.job.is_some() {
            assert!(drawn.contains("Converting"), "with ffmpeg it converts: {drawn:?}");
            assert_eq!(p.load_state(), LoadState::Loading);
            let resolved = p.status()["media"]["resolved"].as_str().map(str::to_string);
            assert!(resolved.map_or(false, |r| r.starts_with(&cache.display().to_string())), "{:?}", p.status());
        } else {
            let err = p.error().expect("no conversion means an error");
            assert!(err.contains(transcode::FFMPEG_HINT), "the fix is named: {err}");
            assert!(drawn.contains("Settings > Media"), "{drawn:?}");
        }
        drop(p);
        let _ = std::fs::remove_dir_all(&cache);
    }

    /// The GPU-free playback half over the shipped clip: frames come out at
    /// the clip's size, in pts order, in real time, and the clip LOOPS (pts
    /// wraps to the start and the loop counter moves) without the surface.
    /// Timing-based with wide margins, like the media tests (about 2.6 s).
    #[test]
    fn frames_arrive_at_the_clip_size_in_pts_order_and_loop() {
        let mut p = VideoProvider::new(DEMO);
        p.open_with(&data_dir(), (1280, 720));
        assert!(p.error().is_none(), "{:?}", p.error());
        let dur = p.status()["duration_s"].as_f64().unwrap();
        assert!((dur - 2.008).abs() < 0.01, "duration {dur}");
        assert_eq!(p.load_state(), LoadState::Ready);

        let start = Instant::now();
        let mut seen: Vec<(u64, f64)> = Vec::new();
        while start.elapsed() < Duration::from_millis(2600) {
            if p.advance() {
                let f = p.last_frame().expect("advance said a frame is in `last`");
                assert_eq!((f.width, f.height), (320, 180), "the clip's own size, never the display's");
                assert_eq!(f.rgba.len(), 320 * 180 * 4);
                seen.push((p.loops(), f.pts_s));
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        // Half the clip's frames is the floor, for the same reason as the
        // media tests' `real_time_floor`: `poll` drops older due frames by
        // design, so a loaded machine that parks this thread for over a
        // frame period loses frames legitimately (37 of 59 was observed
        // under load against a fixed 40), while the defects the count is
        // for (a decoder that never wakes, a stale generation) deliver a
        // handful. The wrap assertions below are the real-time proof.
        assert!(seen.len() >= 30, "frames stopped flowing: {} of the first pass's 59 arrived in 2.6 s", seen.len());
        // In pts order within a loop, and the loop counter never decreases.
        for w in seen.windows(2) {
            let ((l0, t0), (l1, t1)) = (w[0], w[1]);
            assert!(l1 >= l0);
            if l1 == l0 {
                assert!(t1 > t0, "pts must increase within loop {l0}: {t1} after {t0}");
            } else {
                assert!(t1 < t0, "the first frame after a wrap is near the start: {t1} after {t0}");
            }
        }
        assert!(p.loops() >= 1, "the 2 s clip must have wrapped within 2.6 s");
        assert!(seen.iter().any(|(l, _)| *l >= 1), "frames from the second pass reached the caller");
        let s = p.status();
        assert_eq!(s["loops"].as_u64().unwrap(), p.loops());
        assert!(s["playing"].as_bool().unwrap(), "still rolling after the wrap");
        assert_eq!(s["media"]["transcoding_pct"].as_f64().unwrap(), 100.0, "a direct open reads 100");
        assert!(!s["media"]["cache_hit"].as_bool().unwrap());
        assert!(p.error().is_none());
    }

    /// The layout never scales the frame and always yields the display's
    /// aspect: equal aspects are the identity, a taller frame gets side
    /// bars, a wider one gets top and bottom bars, each centred.
    #[test]
    fn letterbox_layout_keeps_the_frame_1_to_1_and_the_display_aspect() {
        let id = letterbox_layout(320, 180, 1280, 720);
        assert!(id.is_identity(320, 180), "{id:?}");
        assert!(letterbox_layout(1920, 1080, 1280, 720).is_identity(1920, 1080));

        // 4:3 on 16:9: side bars.
        let lb = letterbox_layout(640, 480, 1280, 720);
        assert_eq!((lb.canvas_h, lb.y), (480, 0));
        assert_eq!(lb.canvas_w, 853, "480 * 16/9 rounded");
        assert_eq!(lb.x, (853 - 640) / 2);
        let aspect = lb.canvas_w as f64 / lb.canvas_h as f64;
        assert!((aspect - 16.0 / 9.0).abs() < 0.005, "canvas aspect {aspect}");
        assert!(!lb.is_identity(640, 480));

        // Portrait on 16:9: wide side bars, frame untouched.
        let lb = letterbox_layout(1080, 1920, 1280, 720);
        assert_eq!((lb.canvas_h, lb.y), (1920, 0));
        assert_eq!(lb.canvas_w, 3413);
        assert_eq!(lb.x, (3413 - 1080) / 2);

        // 16:9 on a 1024 x 600 desk monitor: the frame is wider, bars top and bottom.
        let lb = letterbox_layout(1280, 720, 1024, 600);
        assert_eq!((lb.canvas_w, lb.x), (1280, 0));
        assert_eq!(lb.canvas_h, 750, "1280 * 600/1024");
        assert_eq!(lb.y, 15);

        // Degenerate sizes never divide by zero or underflow.
        let lb = letterbox_layout(0, 0, 0, 0);
        assert!(lb.canvas_w >= 1 && lb.canvas_h >= 1);
    }

    /// The picture rule: a small clip is drawn on a display-sized surface,
    /// scaled up with its aspect exact and centred; a clip as large as the
    /// display or larger keeps its own pixels (the letterbox canvas) at
    /// scale 1. Proven able to fail: with the small-clip branch returning
    /// the frame's size the first assertion reads (320, 180).
    #[test]
    fn picture_layout_scales_small_clips_to_the_display_and_keeps_large_ones_1_to_1() {
        let pic = picture_layout(320, 180, 1280, 720);
        assert_eq!((pic.surface_w, pic.surface_h), (1280, 720), "the demo draws on a display-sized surface");
        assert_eq!((pic.x, pic.y, pic.w, pic.h), (0.0, 0.0, 1280.0, 720.0), "same aspect: fills it");
        assert_eq!(pic.scale(320), 4.0);

        // 4:3 480p on the 16:9 wall: scaled to the display's height, side bars.
        let pic = picture_layout(640, 480, 1280, 720);
        assert_eq!((pic.surface_w, pic.surface_h), (1280, 720));
        assert_eq!(pic.h, 720.0);
        assert_eq!(pic.w, 960.0, "aspect kept: 720 * 4/3");
        assert_eq!(pic.x, 160.0, "centred");
        assert_eq!(pic.y, 0.0);

        // 1080p on the 720p wall def: every pixel kept, scale 1, no bars.
        let pic = picture_layout(1920, 1080, 1280, 720);
        assert_eq!((pic.surface_w, pic.surface_h), (1920, 1080));
        assert_eq!(pic.scale(1920), 1.0);
        assert_eq!((pic.x, pic.y), (0.0, 0.0));

        // Portrait 1080 x 1920 on the wall: taller than the display, so the
        // letterbox canvas at 1:1 with side bars.
        let pic = picture_layout(1080, 1920, 1280, 720);
        assert_eq!((pic.surface_w, pic.surface_h), (3413, 1920));
        assert_eq!(pic.scale(1080), 1.0);
        assert_eq!(pic.x, ((3413.0 - 1080.0) / 2.0_f64) as f32);

        // Exactly the display's size: 1:1, fills.
        let pic = picture_layout(1280, 720, 1280, 720);
        assert_eq!((pic.surface_w, pic.surface_h, pic.scale(1280)), (1280, 720, 1.0));

        // Degenerate sizes never divide by zero.
        let pic = picture_layout(0, 0, 0, 0);
        assert!(pic.surface_w >= 1 && pic.surface_h >= 1 && pic.w > 0.0);
    }

    /// The strip's clock: no move, no strip; a move shows it for
    /// `STRIP_HOLD`; the same position repeated (the look ray every frame)
    /// does not extend it; a pointer that left does not count as a move.
    /// And the rule over it: paused, no picture and an open picker always
    /// show the strip.
    #[test]
    fn the_strip_shows_on_pointer_movement_and_hides_after_the_hold() {
        let t0 = Instant::now();
        let mut timer = StripTimer::default();
        assert!(!timer.recently_moved(t0), "nothing moved yet");
        timer.observe(Some((0.3, 0.3)), t0);
        assert!(timer.recently_moved(t0), "arriving on the screen is a move");
        // Held still: reported every frame at the same place.
        for i in 1..10 {
            timer.observe(Some((0.3, 0.3)), t0 + Duration::from_millis(100 * i));
        }
        assert!(timer.recently_moved(t0 + Duration::from_secs(2)));
        assert!(!timer.recently_moved(t0 + STRIP_HOLD), "the hold ran out without a new move");
        assert!(!timer.recently_moved(t0 + Duration::from_secs(10)));
        // A move restarts it.
        timer.observe(Some((0.4, 0.3)), t0 + Duration::from_secs(10));
        assert!(timer.recently_moved(t0 + Duration::from_secs(12)));
        // Leaving is not a move; coming back to the same spot is.
        timer.observe(None, t0 + Duration::from_secs(20));
        assert!(!timer.recently_moved(t0 + Duration::from_secs(20)));
        timer.observe(Some((0.4, 0.3)), t0 + Duration::from_secs(20));
        assert!(timer.recently_moved(t0 + Duration::from_secs(20)));

        assert!(!strip_wanted(false, false, false, false), "a playing film with no pointer: hidden");
        assert!(strip_wanted(true, false, false, false), "paused: shown");
        assert!(strip_wanted(false, true, false, false), "no picture: shown");
        assert!(strip_wanted(false, false, true, false), "picker open: shown");
        assert!(strip_wanted(false, false, false, true), "recent move: shown");
    }

    /// The placement law: centre pan straight ahead, swung toward the side
    /// the screen is on (mirror-symmetric), linear fade to silence at the
    /// one-shot path's 50 m, full volume at the screen.
    #[test]
    fn audio_placement_pans_by_bearing_and_fades_by_distance() {
        let listener = Vec3::new(10.0, 1.6, 10.0);
        let right = Vec3::X; // facing -Z, world +X is the listener's right
        let ahead = Vec3::new(10.0, 1.2, 5.0);
        let (vol, pan) = audio_placement(ahead, listener, right);
        assert!((pan - 0.5).abs() < 1e-6, "straight ahead is centred: {pan}");
        assert!((vol - (1.0 - ahead.distance(listener) / 50.0) as f64).abs() < 1e-6, "linear falloff: {vol}");

        let (_, pan_r) = audio_placement(Vec3::new(15.0, 1.6, 10.0), listener, right);
        let (_, pan_l) = audio_placement(Vec3::new(5.0, 1.6, 10.0), listener, right);
        assert!(pan_r > 0.5 && pan_l < 0.5, "right {pan_r}, left {pan_l}");
        assert!((pan_r - 0.5 - (0.5 - pan_l)).abs() < 1e-6, "mirror symmetric");
        assert!((pan_r - (0.5 + 0.5 * PAN_STRENGTH as f64)).abs() < 1e-6, "hard right reaches the full swing");
        // Behind the listener is also centred (no front/back cue in a stereo pan).
        let (_, pan_b) = audio_placement(Vec3::new(10.0, 1.6, 15.0), listener, right);
        assert!((pan_b - 0.5).abs() < 1e-6);

        let (far, _) = audio_placement(Vec3::new(10.0, 1.6, 70.0), listener, right);
        assert_eq!(far, 0.0, "silent beyond the max distance");
        let (here, pan_here) = audio_placement(listener, listener, right);
        assert_eq!((here, pan_here), (1.0, 0.5), "at the screen: full volume, centred");
    }

    /// The mix is master x sfx x falloff, each slider really in it: zero any
    /// one and the film is silent, halve any one and the film halves.
    #[test]
    fn mix_composition_multiplies_master_sfx_and_falloff() {
        assert_eq!(compose_mix(1.0, 1.0, 1.0), 1.0);
        assert_eq!(compose_mix(0.0, 1.0, 1.0), 0.0, "master off silences a film");
        assert_eq!(compose_mix(1.0, 0.0, 1.0), 0.0, "the sfx bus off silences a film");
        assert_eq!(compose_mix(1.0, 1.0, 0.0), 0.0, "out of range is silent");
        assert!((compose_mix(0.5, 1.0, 1.0) - 0.5).abs() < 1e-12);
        assert!((compose_mix(1.0, 0.5, 1.0) - 0.5).abs() < 1e-12);
        assert!((compose_mix(1.0, 1.0, 0.5) - 0.5).abs() < 1e-12);
        // The reviewer's case: sfx at 10 percent, a screen at 35 m (falloff 0.3).
        let (falloff, _) = audio_placement(Vec3::new(0.0, 0.0, -35.0), Vec3::ZERO, Vec3::X);
        assert!((falloff - 0.3).abs() < 1e-6);
        assert!((compose_mix(1.0, 0.1, falloff) - 0.03).abs() < 1e-6, "the f32 falloff carries a 1e-9 rounding");
    }

    /// The epsilon gate: nothing sent yet always moves; the same value does
    /// not; a change under the epsilon on both channels does not; a change
    /// over it on EITHER channel does.
    #[test]
    fn mix_gate_passes_the_first_value_and_then_only_moves_past_the_epsilon() {
        assert!(mix_moved(None, (0.0, 0.5)), "the first mix always goes out");
        assert!(!mix_moved(Some((0.3, 0.5)), (0.3, 0.5)));
        let under = MIX_EPSILON * 0.5;
        let over = MIX_EPSILON * 1.5;
        assert!(!mix_moved(Some((0.3, 0.5)), (0.3 + under, 0.5 + under)), "jitter under the epsilon is held");
        assert!(mix_moved(Some((0.3, 0.5)), (0.3 + over, 0.5)), "a volume move goes out");
        assert!(mix_moved(Some((0.3, 0.5)), (0.3, 0.5 - over)), "a pan move goes out");
        assert!(mix_moved(Some((0.3, 0.5)), (0.3 - over, 0.5 + over)));
    }

    /// The link attaches once, AT the first mix it is given (the attach and
    /// the first "sent" value are the same thing), then holds an unchanged
    /// mix and sends a moved one; it never attaches twice. Proven able to
    /// fail (observed 2026-09-17): with `attached = true` left out of the
    /// Attach arm the `link.attached` assertion below fires first (and the
    /// second step would return Attach again).
    #[test]
    fn the_sound_link_attaches_once_at_the_first_mix_then_sends_only_moves() {
        let mut link = SoundLink::default();
        assert!(!link.attached && link.sent.is_none());
        assert_eq!(link.step(0.03, 0.85), SoundStep::Attach { volume: 0.03, pan: 0.85 });
        assert!(link.attached);
        assert_eq!(link.sent, Some((0.03, 0.85)), "the attach mix is the first mix sent");
        assert_eq!(link.step(0.03, 0.85), SoundStep::Hold, "the same mix is held, never a second attach");
        assert_eq!(link.step(0.03 + MIX_EPSILON * 0.5, 0.85), SoundStep::Hold, "jitter is held");
        assert_eq!(link.step(0.2, 0.5), SoundStep::Send { volume: 0.2, pan: 0.5 });
        assert_eq!(link.sent, Some((0.2, 0.5)));
        assert_eq!(link.step(0.2, 0.5), SoundStep::Hold);
        for _ in 0..100 {
            assert_ne!(link.step(0.2, 0.5), SoundStep::Attach { volume: 0.2, pan: 0.5 }, "attached stays attached");
        }
    }

    /// The audible defect: the first thing the audio thread hears must be
    /// the composed mix, not full volume tweened down. With the sfx slider
    /// at 10 percent and the screen 35 m off, the attach carries 0.03, about
    /// 33 times below the 1.0 a default-level attach would have started at.
    #[test]
    fn the_stream_starts_at_the_placed_mix_not_at_full_volume() {
        let (falloff, pan) = audio_placement(Vec3::new(0.0, 1.0, -35.0), Vec3::new(0.0, 1.0, 0.0), Vec3::X);
        let volume = compose_mix(1.0, 0.1, falloff);
        let mut link = SoundLink::default();
        let SoundStep::Attach { volume: v0, pan: p0 } = link.step(volume, pan) else {
            panic!("the first step is the attach");
        };
        assert!((v0 - 0.03).abs() < 1e-6, "attach volume {v0}");
        assert!((1.0 / v0 - 33.3).abs() < 0.5, "a default-level start would have been {:.0}x too loud", 1.0 / v0);
        assert!((p0 - 0.5).abs() < 1e-6, "straight ahead: centred from the first sample");
        // And the attach options the provider builds from it carry it through.
        let opts = AudioAttach { looping: true, volume: v0, panning: p0 };
        assert!(opts.looping, "a screen's clip loops on kira's side");
        assert_eq!((opts.volume, opts.panning), (v0, p0));
    }

    /// The ordering through the real `world_update`, on a machine with no
    /// audio device (`audio: None`, which is what CI has): before the first
    /// frame opens the player nothing is attached; after it opens, with no
    /// device, the attach stays PENDING (not marked done, nothing sent), so
    /// a device-less machine never records a phantom attach and a machine
    /// with a device attaches on the first tick that has both (the ignored
    /// device test below drives that half). Proven able to fail (observed
    /// 2026-09-17): with the no-device return in `world_update` marking the
    /// link attached, the "must stay pending" assertion fires.
    #[test]
    fn the_attach_waits_for_both_the_player_and_a_device() {
        let mut p = VideoProvider::new(DEMO);
        let world = || ScreenWorld {
            screen_centre: [0.0, 1.0, -5.0],
            listener_pos: [0.0, 1.0, 0.0],
            listener_right: [1.0, 0.0, 0.0],
            audio: None,
        };
        // Tick 1: world_update runs before frame on every tick; no player yet.
        p.world_update(&mut world());
        assert_eq!(p.sound, SoundLink::default(), "nothing to attach before the player exists");
        // The first frame opens the player (this is what `plan_frame` runs).
        p.open_with(&data_dir(), (1280, 720));
        assert!(p.player.is_some());
        // Tick 2 onward, still no device: pending, never done.
        for _ in 0..5 {
            p.world_update(&mut world());
        }
        assert_eq!(p.sound, SoundLink::default(), "no device: the attach must stay pending, not be marked done");
        assert!(!p.player.as_ref().unwrap().has_audio());
        assert!(!p.status()["audio"].as_bool().unwrap());
    }

    /// Input routing per tick: a playing clip with the pointer MOVING on it
    /// shows the strip and hands the queued input to the run that draws it
    /// (so the strip's buttons work); a playing clip nobody is looking at,
    /// with no new frame, draws nothing and drops the backlog (the look ray
    /// reports every frame, and only a run drains the queue); a paused clip
    /// always draws and keeps its events for the run. The unwatched case is
    /// forced deterministically: the clip is PAUSED so no frame can become
    /// due, then `want_playing` is set without touching the clock, which is
    /// exactly "playing, nothing new this tick". Proven able to fail: with
    /// the drop removed from `plan_frame`'s Keep path the middle count
    /// stays at 25.
    #[test]
    fn a_looked_at_or_paused_clip_draws_and_keeps_input_while_an_unwatched_one_drops_it() {
        let theme = load_theme();
        let mut state = GuiState::default();
        let mut core = ScreenCore::new("s", "video:x", 1280, 720, &theme);
        let mut p = VideoProvider::new(DEMO);
        p.open_with(&data_dir(), core.size());
        assert!(p.error().is_none(), "{:?}", p.error());
        let t0 = Instant::now();

        let sweep = |core: &mut ScreenCore| {
            for i in 0..25 {
                core.pointer_moved((i as f32 / 25.0, 0.4));
            }
        };
        // Looked at: the pointer moved, the strip is up, the events are
        // for the run.
        sweep(&mut core);
        assert_eq!(core.pending_events(), 25);
        match p.plan_frame(&mut core, t0) {
            FrameDraw::Compose { strip, px, .. } => {
                assert!(strip, "a moving pointer shows the strip");
                assert_eq!(px, (1280, 720), "the demo draws at the display's size");
            }
            other => panic!("looked at: {other:?}"),
        }
        assert_eq!(core.pending_events(), 25, "the run gets the events, plan_frame does not eat them");
        let (drawn, _) = run_draw(&mut core, &mut state, &mut p);
        assert_eq!(core.pending_events(), 0);
        assert!(drawn.contains("Pause") && drawn.contains("Open"), "the strip's buttons: {drawn:?}");

        // Unwatched: pause the clip (the clock stops, so no frame can
        // become due), let a paused tick draw once so nothing is pending,
        // then mark it playing WITHOUT the clock (no frame is due on the
        // next tick) with the pointer gone and the hold long expired.
        p.on_button((0.5, 0.5), true);
        assert!(p.is_paused());
        let later = t0 + STRIP_HOLD + Duration::from_secs(1);
        core.pointer_gone();
        let _ = p.plan_frame(&mut core, later);
        let _ = run_draw(&mut core, &mut state, &mut p);
        assert!(!p.frame_dirty, "the paused draw uploaded what there was to upload");
        // Stand in a decoded frame, so the film has a PICTURE. This half of
        // the test is about a playing film NOBODY IS LOOKING AT, and that
        // state only exists once there is something on screen: with nothing
        // to show the strip stays up by design and the provider must draw.
        // Two pixels are enough, and a frame smaller than the display keeps
        // the surface at the display's own size (`picture_layout`), so the
        // sizes asserted above do not move. `frame_dirty` stays false: this
        // frame stands for one already uploaded.
        p.last = Some(VideoFrame { rgba: vec![0u8; 2 * 2 * 4], width: 2, height: 2, pts_s: 0.0 });
        p.want_playing = true; // playing, but the clock never moved: nothing is due
        sweep(&mut core);
        core.pointer_gone();
        let far = later + Duration::from_secs(5);
        // The strip was up and is not wanted now, so one last draw is owed:
        // the screen has to be repainted WITHOUT it. A film whose strip has
        // already come down is the quiet case, and it is the next tick.
        match p.plan_frame(&mut core, far) {
            FrameDraw::Compose { strip, .. } => assert!(!strip, "the strip comes down on this draw"),
            other => panic!("taking the strip down is a draw, got {other:?}"),
        }
        let _ = run_draw(&mut core, &mut state, &mut p);
        sweep(&mut core);
        core.pointer_gone();
        assert_eq!(
            p.plan_frame(&mut core, far + Duration::from_secs(1)),
            FrameDraw::Keep,
            "unwatched, nothing new: no draw"
        );
        assert_eq!(core.pending_events(), 0, "an unwatched tick drops the backlog");

        // Paused: always draws, keeps its events for the run.
        p.want_playing = false;
        sweep(&mut core);
        match p.plan_frame(&mut core, far + Duration::from_secs(2)) {
            FrameDraw::Compose { strip, .. } => assert!(strip, "paused: the strip is up"),
            other => panic!("paused must draw, got {other:?}"),
        }
        assert_eq!(core.pending_events(), 25);
        let (drawn, _) = run_draw(&mut core, &mut state, &mut p);
        assert_eq!(core.pending_events(), 0);
        assert!(drawn.contains("Play"), "paused shows Play: {drawn:?}");
    }

    /// A clip paused on its last frame stays there: the loop counter does
    /// not move and the position stays at the end, however many ticks pass;
    /// the wrap happens on the first tick after play resumes. Proven able to
    /// fail: without the `want_playing` gate in `advance` the first
    /// `loops()` assertion reads 1 and the position reads under 0.1 s.
    #[test]
    fn pausing_on_the_last_frame_does_not_rewind() {
        let mut p = VideoProvider::new(DEMO);
        p.open_with(&data_dir(), (1280, 720));
        assert!(p.error().is_none(), "{:?}", p.error());
        let dur = p.status()["duration_s"].as_f64().unwrap();
        // Play through to the declared end, polling as the render loop
        // would, but stop BEFORE the tick that would wrap.
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline && !p.player.as_ref().unwrap().at_end() {
            let _ = p.player.as_mut().unwrap().poll();
            std::thread::sleep(Duration::from_millis(3));
        }
        assert!(p.player.as_ref().unwrap().at_end(), "the clip must reach its end within 4 s");
        assert_eq!(p.loops(), 0);

        // The click lands on the last frame.
        p.on_button((0.5, 0.5), true);
        assert!(p.is_paused());
        for _ in 0..10 {
            p.advance();
        }
        assert_eq!(p.loops(), 0, "a paused clip must not wrap");
        let pos = p.status()["position_s"].as_f64().unwrap();
        assert!((pos - dur).abs() < 0.02, "paused at the end, the position stays there: {pos} of {dur}");
        assert!(p.paused_text().starts_with(&format!("Paused at {dur:.1} s")), "{}", p.paused_text());

        // Play again: now it wraps, once, and rolls from the start.
        p.on_button((0.5, 0.5), true);
        assert!(!p.is_paused());
        p.advance();
        assert_eq!(p.loops(), 1, "the wrap happens on the first playing tick");
        let pos = p.status()["position_s"].as_f64().unwrap();
        assert!(pos < 0.1, "rolling from the start, position {pos}");
        assert!(p.player.as_ref().unwrap().is_playing());
        assert!(p.error().is_none());
    }

    /// Notices and the paused picture are laid out at the DISPLAY's pixel
    /// size (the def's `px`), never at the clip's: the demo's 320 x 180
    /// frame draws on a 1280 x 720 surface (scaled up by the GPU) and an
    /// error page on the def's size. Proven able to fail: with
    /// `surface_px` returning the frame's size the paused case reads
    /// (320, 180).
    #[test]
    fn notices_and_the_paused_picture_lay_out_at_the_display_size() {
        let theme = load_theme();
        let mut core = ScreenCore::new("s", "video:x", 1280, 720, &theme);
        let mut p = VideoProvider::new(DEMO);
        p.open_with(&data_dir(), core.size());
        // Let a frame arrive, then pause with it in hand.
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline && p.last.is_none() {
            p.advance();
            std::thread::sleep(Duration::from_millis(3));
        }
        assert!(p.last.is_some(), "a frame must arrive within 2 s");
        // The surface may sit at another size from a previous draw.
        core.set_size(320, 180);
        p.on_button((0.5, 0.5), true);
        match p.plan_frame(&mut core, Instant::now()) {
            FrameDraw::Compose { px, strip, .. } => {
                assert_eq!(px, (1280, 720), "the paused picture is laid out at the def's px");
                assert!(strip);
            }
            other => panic!("paused must draw, got {other:?}"),
        }
        // The error page too, for a clip that failed to open.
        let mut core = ScreenCore::new("s", "video:x", 1024, 600, &theme);
        let mut e = VideoProvider::new("media/nothing.webm");
        e.open_with(&data_dir(), core.size());
        core.set_size(320, 180);
        match e.plan_frame(&mut core, Instant::now()) {
            FrameDraw::Compose { px, .. } => assert_eq!(px, (1024, 600)),
            other => panic!("an open error must draw, got {other:?}"),
        }
        assert!(e.notice_text().unwrap().contains("not found"));
    }

    /// The whole ingest path through the provider, when ffmpeg exists: an
    /// MP4 opened with `open_path` reports Loading with a rising percentage
    /// and no player, then Ready with the player on the converted file in
    /// the scratch cache, frames flow at the fixture's size, the status
    /// says 100 and no cache hit; opening the SAME file again is a cache
    /// hit (no second conversion; `transcodes` stays 1) and plays. Skipped
    /// (printed) without ffmpeg.
    #[test]
    fn open_path_converts_an_mp4_once_then_plays_it_from_the_cache() {
        if transcode::find_ffmpeg("").is_none() {
            println!("skipped: no ffmpeg on this machine");
            return;
        }
        let cache = scratch("mp4");
        let mp4 = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MP4);
        let mut p = VideoProvider::new(DEMO).with_cache_dir(cache.clone());
        p.display_px = Some((1280, 720));
        p.open_path(&mp4);
        assert!(p.job.is_some(), "an MP4 needs a conversion: {:?}", p.error());
        assert_eq!(p.load_state(), LoadState::Loading);
        assert_eq!(p.status()["media"]["transcodes"].as_u64(), Some(1));
        assert_eq!(p.status()["media"]["chosen"].as_str(), Some(mp4.display().to_string().as_str()));
        assert!(p.notice_text().unwrap().contains("Converting"), "{:?}", p.notice_text());
        assert_eq!(p.current_name(), "moving-box-h264-aac.mp4");

        let deadline = Instant::now() + Duration::from_secs(120);
        while Instant::now() < deadline && p.player.is_none() && p.error.is_none() {
            p.advance();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(p.error.is_none(), "{:?}", p.error);
        assert!(p.player.is_some(), "the converted file must open within 120 s");
        assert_eq!(p.load_state(), LoadState::Ready);
        let s = p.status();
        assert_eq!(s["media"]["transcoding_pct"].as_f64(), Some(100.0));
        assert!(!s["media"]["cache_hit"].as_bool().unwrap());
        let resolved = PathBuf::from(s["media"]["resolved"].as_str().unwrap());
        assert!(resolved.starts_with(&cache) && resolved.is_file(), "{}", resolved.display());
        // Frames flow from the converted clip at the fixture's size.
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut frames = 0;
        while Instant::now() < deadline && frames < 5 {
            if p.advance() {
                let f = p.last_frame().unwrap();
                assert_eq!((f.width, f.height), (320, 180));
                frames += 1;
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        assert!(frames >= 5, "frames from the converted file: {frames}");

        // The same file again: a cache hit, no second conversion.
        p.open_path(&mp4);
        assert!(p.job.is_none(), "a cached file spawns no job");
        assert!(p.player.is_some(), "and opens at once");
        let s = p.status();
        assert!(s["media"]["cache_hit"].as_bool().unwrap(), "{s}");
        assert_eq!(s["media"]["transcodes"].as_u64(), Some(1), "still one conversion");
        assert_eq!(s["media"]["resolved"].as_str().map(PathBuf::from), Some(resolved));
        drop(p);
        let _ = std::fs::remove_dir_all(&cache);
    }

    /// A DISC on the screen, when ffmpeg exists: handing the provider the
    /// FOLDER (which is what the picker's "Play disc" button and the dev
    /// IPC hand it) takes the disc road, converts the fixture disc's
    /// two-part main title as one film, and plays it. The status says it
    /// was a disc, so a rig can tell this path from the single-file one.
    /// Skipped (printed) without ffmpeg.
    #[test]
    fn open_path_plays_a_whole_video_disc_from_its_folder() {
        if transcode::find_ffmpeg("").is_none() {
            println!("skipped: no ffmpeg on this machine");
            return;
        }
        let cache = scratch("disc");
        // The folder ABOVE VIDEO_TS, which is what a disc drive looks like.
        let disc = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures").join("media");
        let mut p = VideoProvider::new(DEMO).with_cache_dir(cache.clone());
        p.display_px = Some((1280, 720));
        p.open_path(&disc);
        assert!(p.error.is_none(), "{:?}", p.error);
        assert!(p.job.is_some(), "a disc needs a conversion");
        let s = p.status();
        assert!(s["media"]["disc"].as_bool().unwrap(), "the status names the disc road: {s}");
        assert!(s["media"]["convert_why"].as_str().unwrap().contains("video disc"), "{s}");
        assert!(p.notice_text().unwrap().contains("video disc"), "{:?}", p.notice_text());

        let deadline = Instant::now() + Duration::from_secs(180);
        while Instant::now() < deadline && p.player.is_none() && p.error.is_none() {
            p.advance();
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(p.error.is_none(), "{:?}", p.error);
        assert!(p.player.is_some(), "the disc must open within 180 s");
        assert_eq!(p.load_state(), LoadState::Ready);
        // Both parts, as one film: one part alone is a second.
        assert!(p.player.as_ref().unwrap().duration_s() > 1.5, "{}", p.player.as_ref().unwrap().duration_s());
        let mut frames = 0;
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline && frames < 5 {
            if p.advance() {
                assert_eq!((p.last_frame().unwrap().width, p.last_frame().unwrap().height), (320, 180));
                frames += 1;
            }
            std::thread::sleep(Duration::from_millis(3));
        }
        assert!(frames >= 5, "frames from the converted disc: {frames}");
        drop(p);
        let _ = std::fs::remove_dir_all(&cache);
    }

    /// A folder that is not a disc, and a disc that is copy protected, are
    /// both answered in words on the screen, never as a black picture. The
    /// protected one is simulated here by marking a copy of the fixture's
    /// packets as scrambled (the failure path, not a way around anything;
    /// `src/media/dvd.rs`).
    #[test]
    fn a_folder_that_is_not_a_disc_and_a_protected_disc_are_answered_in_words() {
        let d = scratch("notdisc");
        std::fs::write(d.join("readme.txt"), b"x").unwrap();
        let mut p = VideoProvider::new(DEMO).with_cache_dir(d.join("cache"));
        p.display_px = Some((1280, 720));
        p.open_path(&d);
        let err = p.error().expect("a folder with no disc in it cannot play").to_string();
        assert!(err.contains("not a video disc"), "{err}");
        assert_eq!(p.load_state(), LoadState::Error(err.clone()));
        assert!(p.notice_text().unwrap().contains("cannot play"), "the screen shows it");

        // A protected disc: the same fixture bytes with every packet header
        // marked scrambled, which is how an encrypted disc declares itself.
        let protected = scratch("protecteddisc");
        let video_ts = protected.join(crate::media::dvd::VIDEO_TS);
        std::fs::create_dir_all(&video_ts).unwrap();
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/media/VIDEO_TS/VTS_01_1.VOB");
        let mut bytes = std::fs::read(&src).unwrap();
        let mut i = 0usize;
        while i + 9 <= bytes.len() {
            let start = bytes[i] == 0 && bytes[i + 1] == 0 && bytes[i + 2] == 1;
            let id = bytes[i + 3];
            if start && (id == 0xBD || (0xC0..=0xEF).contains(&id)) && (bytes[i + 6] & 0xC0) == 0x80 {
                bytes[i + 6] |= 0x10;
            }
            i += 1;
        }
        std::fs::write(video_ts.join("VTS_01_1.VOB"), &bytes).unwrap();
        let mut q = VideoProvider::new(DEMO).with_cache_dir(protected.join("cache"));
        q.display_px = Some((1280, 720));
        q.open_path(&protected);
        let err = q.error().expect("a protected disc cannot play").to_string();
        assert_eq!(err, crate::media::dvd::PROTECTED_MESSAGE);
        assert!(q.job.is_none(), "nothing is converted");
        assert!(q.notice_text().unwrap().contains("does not break disc protection"), "{:?}", q.notice_text());
        let _ = std::fs::remove_dir_all(&d);
        let _ = std::fs::remove_dir_all(&protected);
    }

    /// A remembered choice in the config wins over the data file's source:
    /// with `screen_media["s"]` set to the demo's absolute path, the first
    /// frame's adoption opens THAT file and `chosen` names it; a screen with
    /// nothing remembered keeps the default. The write-back side: a picked
    /// file lands in the GUI state's map under the screen's id and is saved
    /// through the normal config path (pointed at scratch here).
    #[test]
    fn a_remembered_file_wins_on_boot_and_a_pick_is_remembered() {
        let demo_abs = data_dir().join(DEMO);
        let mut state = GuiState::default();
        state.settings.screen_media.insert("s".into(), demo_abs.clone());

        let mut p = VideoProvider::new("media/nothing.webm");
        p.display_px = Some((1280, 720));
        p.adopt_remembered("s", &state);
        assert_eq!(p.chosen.as_deref(), Some(demo_abs.as_path()));
        assert!(p.player.is_some(), "the remembered file opened: {:?}", p.error());
        assert!(p.open_attempted, "the default is never opened over it");
        p.adopt_remembered("s", &state);
        assert!(p.player.is_some(), "adoption is once");

        let mut q = VideoProvider::new("media/nothing.webm");
        q.adopt_remembered("other_screen", &state);
        assert!(q.chosen.is_none() && !q.open_attempted, "nothing remembered: the default will open");

        let mut r = VideoProvider::new(DEMO);
        r.display_px = Some((1280, 720));
        r.open_path(&demo_abs);
        assert!(r.remember_pending);
        let mut st = GuiState::default();
        let scratch_dir = scratch("remember");
        std::env::set_var("HUMANITY_DATA_DIR", scratch_dir.display().to_string());
        r.remember("wall_x", &mut st);
        std::env::remove_var("HUMANITY_DATA_DIR");
        assert!(!r.remember_pending);
        assert_eq!(st.settings.screen_media.get("wall_x"), Some(&demo_abs));
        assert!(scratch_dir.join("config.json").is_file(), "saved through the normal config path");
        let saved: crate::config::AppConfig =
            serde_json::from_str(&std::fs::read_to_string(scratch_dir.join("config.json")).unwrap()).unwrap();
        assert_eq!(saved.screen_media.get("wall_x"), Some(&demo_abs), "and the file on disk carries it");
        let _ = std::fs::remove_dir_all(&scratch_dir);
    }

    /// `data/media/README.md` promises the shipped demo clip is a
    /// byte-identical copy of the media tests' fixture, and the screen
    /// tests above open the demo while the media tests open the fixture:
    /// the two suites only prove the same thing while the files match.
    #[test]
    fn the_demo_clip_is_the_test_fixture_byte_for_byte() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let demo = root.join("data/media/demo_colour_bar.webm");
        let fixture = root.join("tests/fixtures/media/colour-bar-av1-opus.webm");
        let a = std::fs::read(&demo).unwrap_or_else(|e| panic!("{}: {e}", demo.display()));
        let b = std::fs::read(&fixture).unwrap_or_else(|e| panic!("{}: {e}", fixture.display()));
        assert!(!a.is_empty());
        assert!(
            a == b,
            "{} ({} bytes) differs from {} ({} bytes); regenerate both with scripts/make-media-fixtures.sh \
             and copy the fixture over the demo under the same name (data/media/README.md), then update \
             the byte count in that README",
            demo.display(),
            a.len(),
            fixture.display(),
            b.len()
        );
    }

    /// The other half of the ordering test, on a machine with an audio
    /// device: the real `world_update` attaches on the first tick that has
    /// both a player and a device, AT the composed mix (master x sfx x
    /// falloff, not full volume), the sound then really plays, an unchanged
    /// world sends nothing more, and a moved listener sends a new mix.
    /// Master is set very low rather than to zero so the composed value is
    /// distinguishable from "silent" while staying inaudible (amplitude
    /// 0.0006 of a tone whose RMS is about 0.3).
    ///
    /// Run it in RELEASE mode: rav1d's own debug-only `DisjointMut` borrow
    /// checker can panic on one of rav1d's worker threads under a debug
    /// build with several decoders running (observed once in 16 debug runs
    /// of the suite, 2026-09-17; it aborts the whole test process). See
    /// "Known limits" in docs/design/media-player.md. A debug run that
    /// completes is still valid.
    #[test]
    #[ignore = "needs an audio output device; run: cargo test --release --features native --lib -- --ignored --nocapture engine::screens::video::tests::sound_attaches (release: rav1d's debug-only borrow checker can abort a debug run, see docs/design/media-player.md Known limits)"]
    fn sound_attaches_once_at_the_listener_mix_when_a_device_exists() {
        let mut audio = match crate::audio::AudioManager::try_new() {
            Ok(a) => a,
            Err(e) => {
                println!("skipped: no audio device ({e})");
                return;
            }
        };
        audio.set_master_volume(0.02);
        audio.set_sfx_volume(0.1);
        let mut p = VideoProvider::new(DEMO);
        // The screen 35 m straight ahead of the starting listener.
        fn world<'a>(listener: [f32; 3], audio: &'a mut crate::audio::AudioManager) -> ScreenWorld<'a> {
            ScreenWorld {
                screen_centre: [0.0, 1.0, -35.0],
                listener_pos: listener,
                listener_right: [1.0, 0.0, 0.0],
                audio: Some(audio),
            }
        }
        // Tick 1: no player yet.
        p.world_update(&mut world([0.0, 1.0, 0.0], &mut audio));
        assert_eq!(p.sound, SoundLink::default());
        p.open_with(&data_dir(), (1280, 720));
        assert!(p.error().is_none(), "{:?}", p.error());
        // Tick 2: the attach, at the composed mix.
        p.world_update(&mut world([0.0, 1.0, 0.0], &mut audio));
        let expected = compose_mix(0.02, 0.1, 0.3);
        assert!(p.sound.attached);
        let (v, pan) = p.sound.sent.expect("the attach mix was recorded");
        assert!((v - expected).abs() < 1e-9, "attached at {v}, expected master x sfx x falloff = {expected}");
        assert!((pan - 0.5).abs() < 1e-6);
        assert!(p.player.as_ref().unwrap().has_audio(), "the stream really attached");
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(p.player.as_ref().unwrap().audio_state(), Some(kira::sound::PlaybackState::Playing));
        // Ticks 3..: unchanged world, nothing new sent.
        for _ in 0..30 {
            p.world_update(&mut world([0.0, 1.0, 0.0], &mut audio));
        }
        assert_eq!(p.sound.sent, Some((v, pan)), "an unmoved listener sends nothing");
        // The listener steps to the screen's right: the pan swings, and
        // closer: louder.
        p.world_update(&mut world([-20.0, 1.0, -35.0], &mut audio));
        let (v2, pan2) = p.sound.sent.unwrap();
        assert!(v2 > v, "closer is louder: {v2} vs {v}");
        assert!(pan2 > 0.5, "a screen to the right pans right: {pan2}");
        assert!(p.player.as_mut().unwrap().take_error().is_none());
        println!("provider sound: attached at volume {v:.6} pan {pan:.3}; after the move volume {v2:.6} pan {pan2:.3}");
    }
}

