//! The video provider (in-world screens ladder, rung 5, integration): a
//! `video:<path>` screen plays a WebM clip through the purpose-built player
//! in `src/media`, looping, with its sound placed in the world at the
//! screen, and a click on the screen toggles pause. The operator's words:
//! "movies on displays". Design: docs/design/in-world-screens.md, the
//! "Video sources" section; the player itself: docs/design/media-player.md.
//!
//! How the pieces meet:
//!
//! * The PLAYER (`media::VideoPlayer`) decodes on its own thread and keeps
//!   the playback clock. This provider never decodes and never re-gates:
//!   each framed tick it asks `poll()` for the frame that is due (the
//!   player's own gate, which never hands out a frame ahead of its clock)
//!   and writes that frame into the surface texture with `write_pixels`.
//! * The SURFACE is resized to the clip's own frame size by `write_pixels`,
//!   so every pixel of the clip is shown. When the clip's aspect does not
//!   match the display's, the frame is centred at 1:1 in a canvas of the
//!   display's aspect with black bars (letterboxed), never stretched, and
//!   the GPU sampler does the up-scaling on the wall for free.
//! * The SOUND is the clip's Opus track through kira, placed at the screen:
//!   every frame the engine tells the provider where its screen and the
//!   listener are (`world_update`), and the provider sets the stream's
//!   volume from the distance and its stereo pan from the screen's bearing.
//! * A CLICK on the screen (`on_button`) toggles pause. While paused the
//!   surface shows a one-line "paused" page (the frame is bytes in a
//!   texture, not egui, so there is no overlay to draw on it) and the last
//!   frame is kept in memory so play resumes on it at once. The notice is
//!   laid out at the DISPLAY's pixel size (the def's `px`), the frame at
//!   the CLIP's: the surface texture is the clip's size while playing and
//!   the def's while showing a notice.
//! * LOOPING: the clip restarts when the player reports its end, and only
//!   while it is meant to be playing (a clip paused on its last frame stays
//!   there); the sound loops on kira's side (a loop region on the stream),
//!   see `advance`.
//! * INPUT while playing: the picture is bytes, egui does not run, and the
//!   look ray keeps reporting the pointer, so the core's queued input is
//!   dropped every playing tick (`plan_frame`) rather than replayed in one
//!   run on the click that pauses.
//!
//! The file is in two halves so the logic is testable on a machine with no
//! GPU: `open_with`, `advance`, `plan_frame`, the pause toggle,
//! `letterbox_layout`, `compose_letterbox`, `resolve_media_path`,
//! `audio_placement`, `compose_mix`, `mix_moved` and `SoundLink::step` are
//! pure or player-only and every test below drives them; `frame` and
//! `world_update` are thin glue over them (the GPU calls and the kira
//! calls respectively).

use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::gui::screen_surface::{notice, ScreenCore, ScreenProvider, ScreenSurface, ScreenWorld};
use crate::gui::theme::Theme;
use crate::gui::GuiState;
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

/// Where a frame lands on the display: the canvas written to the surface
/// (the smallest rectangle of the DISPLAY's aspect that contains the frame
/// at 1:1) and the frame's offset inside it. When the two aspects already
/// agree to the pixel the canvas IS the frame and nothing is copied.
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
    /// True when the frame fills the canvas exactly (no bars, no copy).
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

/// Paint `frame` into `canvas` at the layout's offset over opaque black
/// bars. `canvas` is reused between frames (reallocated only when the
/// canvas size changes). The BARS are painted on every call, not only on
/// reallocation: two layouts can share a canvas size with different
/// offsets (a 4 x 2 frame and a 2 x 4 frame on a square display both make a
/// 4 x 4 canvas), and painting them once left the previous frame's pixels
/// showing through the new bars. The compose test caught that. Bars are a
/// small share of the canvas, so painting them each frame costs little.
pub fn compose_letterbox(frame: &VideoFrame, lb: &Letterbox, canvas: &mut Vec<u8>) {
    let (cw, ch) = (lb.canvas_w as usize, lb.canvas_h as usize);
    let need = cw * ch * 4;
    if canvas.len() != need {
        canvas.clear();
        canvas.resize(need, 0);
    }
    let fw = frame.width as usize;
    let fh = frame.height as usize;
    let (x0, y0) = (lb.x as usize, lb.y as usize);
    let rows = fh.min(ch.saturating_sub(y0));
    let cols = fw.min(cw.saturating_sub(x0));
    // Opaque black: alpha 255 (the surface format has no use for
    // transparency, and the snapshot PNGs read better opaque).
    let paint_black = |px: &mut [u8]| px.copy_from_slice(&[0, 0, 0, 255]);
    for row in 0..ch {
        let line = &mut canvas[row * cw * 4..(row + 1) * cw * 4];
        if row < y0 || row >= y0 + rows {
            // A bar row above or below the frame: black across.
            line.chunks_exact_mut(4).for_each(paint_black);
            continue;
        }
        // A frame row: black to the left, the frame's pixels, black to the right.
        line[..x0 * 4].chunks_exact_mut(4).for_each(paint_black);
        let src_row = row - y0;
        line[x0 * 4..(x0 + cols) * 4].copy_from_slice(&frame.rgba[src_row * fw * 4..src_row * fw * 4 + cols * 4]);
        line[(x0 + cols) * 4..].chunks_exact_mut(4).for_each(paint_black);
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

/// What a framed tick draws, decided by `plan_frame` with no GPU in hand so
/// the decision is unit-tested; `frame` is the glue that carries it out.
#[derive(Debug, Clone, PartialEq)]
pub enum FrameDraw {
    /// Run egui for a one-line notice (the error page, the paused page) at
    /// `px`, the DISPLAY's pixel size (the def's `px`), never the clip's: a
    /// notice laid out at the clip's 320 x 180 and upscaled four times onto
    /// a 1.2 m wall is a blur. The surface is resized to `px` first (a no-op
    /// when it is already there, so a paused clip does not reallocate every
    /// frame); the next written frame resizes it to the clip again.
    Notice { text: String, px: (u32, u32) },
    /// Write the newest frame into the texture (letterboxed), which sizes
    /// the texture to the clip.
    Frame,
    /// Nothing new this tick: the texture keeps what it has, which is
    /// exactly what a display does between frames.
    Keep,
}

/// The per-screen state of one `video:<path>` source.
pub struct VideoProvider {
    /// The path as written after `video:` in the data file.
    source: String,
    /// The file the path resolved to, once `open_with` ran.
    resolved: Option<PathBuf>,
    /// The player, once opened. `None` before the first frame and after an
    /// open error.
    player: Option<VideoPlayer>,
    /// Set once an open has been tried, whatever the outcome, so a failed
    /// open is shown on the screen and not retried every frame.
    open_attempted: bool,
    /// Why there is no picture: an open error (the path, the codec) or a
    /// decode error from the player's thread. Drawn on the screen.
    error: Option<String>,
    /// The state the PLAYER wants: true = playing. Kept even before the
    /// player exists, so a click on a screen whose clip is still opening is
    /// honoured when it opens.
    want_playing: bool,
    /// The newest frame taken from the player, kept while paused so play
    /// resumes on it at once.
    last: Option<VideoFrame>,
    /// `last` has not been written to the surface yet (it is new, or the
    /// paused page replaced it and play resumed).
    last_dirty: bool,
    /// How many times the clip has wrapped back to its start.
    loops: u64,
    /// The display's pixel size (the def's `px`), read from the surface on
    /// the first frame before any resize. Frames are letterboxed to ITS
    /// aspect, which the def author matched to the physical display.
    display_px: Option<(u32, u32)>,
    /// Scratch canvas for letterboxed frames, reused frame to frame.
    canvas: Vec<u8>,
    /// The sound hookup's state: attached yet, and the last mix sent. The
    /// attach needs the audio manager, which arrives through `world_update`.
    sound: SoundLink,
}

impl VideoProvider {
    /// A provider for `video:<path>`. Nothing is opened here: the decode
    /// thread starts on the first frame, so a screen the player never looks
    /// at never spawns one.
    pub fn new(path: &str) -> Self {
        Self {
            source: path.to_string(),
            resolved: None,
            player: None,
            open_attempted: false,
            error: None,
            want_playing: true,
            last: None,
            last_dirty: false,
            loops: 0,
            display_px: None,
            canvas: Vec::new(),
            sound: SoundLink::default(),
        }
    }

    /// Open the player against `data_dir` if it has not been tried yet.
    /// `display_px` is the surface's pixel size at this moment (the def's
    /// `px`, since nothing has resized it yet). An open error becomes the
    /// screen's error page, naming the path and the codec problem; it never
    /// panics and is never retried, a bad path in home.ron must not take the
    /// world down or burn a probe per frame.
    pub fn open_with(&mut self, data_dir: &Path, display_px: (u32, u32)) {
        if self.open_attempted {
            return;
        }
        self.open_attempted = true;
        self.display_px = Some(display_px);
        let Some(path) = resolve_media_path(data_dir, &self.source) else {
            let root = data_dir.parent().map(|p| p.join(&self.source));
            self.error = Some(format!(
                "file not found: looked for {} and {}",
                data_dir.join(&self.source).display(),
                root.map(|p| p.display().to_string()).unwrap_or_else(|| "(no repo root)".into())
            ));
            log::warn!("[Screens] video {:?}: {}", self.source, self.error.as_deref().unwrap_or(""));
            return;
        };
        match VideoPlayer::open(&path) {
            Ok(mut p) => {
                if self.want_playing {
                    p.play();
                }
                log::info!(
                    "[Screens] video {:?} opened from {} ({}x{}, {:.2} s, audio: {})",
                    self.source,
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
                log::warn!("[Screens] video {:?} at {}: {e}", self.source, path.display());
            }
        }
        self.resolved = Some(path);
    }

    /// `open_with` against the game's resolved data dir (the production path).
    pub fn ensure_open(&mut self, display_px: (u32, u32)) {
        if !self.open_attempted {
            self.open_with(&crate::data_dir(), display_px);
        }
    }

    /// Advance playback one tick: loop if the clip ended AND it is meant to
    /// be playing, take the frame that is due (if any), surface a decode
    /// error. Returns true when a NEW frame is now in `last`. GPU-free;
    /// `plan_frame` calls this and `frame` then writes.
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
    /// jumped to 0 and `loops` counted a wrap nobody saw (the paused page
    /// said "Paused at 0.0 s" for a clip that had just finished).
    pub fn advance(&mut self) -> bool {
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
            self.last_dirty = true;
            fresh = true;
        }
        if let Some(e) = p.take_error() {
            log::warn!("[Screens] video {:?}: {e}", self.source);
            self.error = Some(e);
        }
        fresh
    }

    /// Flip between playing and paused (a click on the screen).
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

    /// The open or decode error, if there is one.
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// The text of the error page: the source path as written, then the
    /// problem (which names the codec for an unsupported file, or both
    /// places a missing file was looked for).
    pub fn error_text(&self) -> String {
        format!(
            "Video \"{}\" cannot play.\n{}",
            self.source,
            self.error.as_deref().unwrap_or("unknown error")
        )
    }

    /// The one line the paused page shows.
    pub fn paused_text(&self) -> String {
        match &self.player {
            Some(p) => format!("Paused at {:.1} s of {:.1} s. Click to play.", p.position_s(), p.duration_s()),
            None => "Paused. Click to play.".to_string(),
        }
    }

    /// Everything `frame` decides, with no GPU in hand: open the player on
    /// the first call (the core's size at that moment is the def's `px`,
    /// nothing has resized it yet), advance playback, and say what to draw.
    /// The bookkeeping that needs the core happens here too: on the PLAYING
    /// path the core's queued input is dropped every tick
    /// (`ScreenCore::drop_pending_events`), because the picture is bytes,
    /// egui does not run, and the look ray keeps reporting the pointer.
    /// `write_pixels` drops as well, but only on the ticks a frame is
    /// written, and a 30 fps clip under a faster render (or a starved
    /// decoder) has ticks with nothing to write. The notice paths keep
    /// their events: the run that draws the notice consumes them.
    pub fn plan_frame(&mut self, core: &mut ScreenCore) -> FrameDraw {
        self.ensure_open(core.size());
        self.advance();
        // Notices lay out at the display's size, not the clip's (the
        // surface may be at the clip's size from the last written frame).
        let px = self.display_px.unwrap_or_else(|| core.size());
        if self.error.is_some() {
            return FrameDraw::Notice { text: self.error_text(), px };
        }
        if !self.want_playing {
            // The page replaces the picture in the texture; the last frame
            // goes back the moment play resumes, before any new frame is due.
            self.last_dirty = self.last.is_some();
            return FrameDraw::Notice { text: self.paused_text(), px };
        }
        core.drop_pending_events();
        if self.last_dirty {
            FrameDraw::Frame
        } else {
            FrameDraw::Keep
        }
    }

    /// Write `last` into the surface, letterboxed to the display's aspect.
    /// The aspect-matching case writes the frame's bytes straight through
    /// (no copy); the other case composes into the scratch canvas first.
    fn write_last(&mut self, surface: &mut ScreenSurface, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(f) = self.last.as_ref() else { return };
        let (dw, dh) = self.display_px.unwrap_or((f.width, f.height));
        let lb = letterbox_layout(f.width, f.height, dw, dh);
        if lb.is_identity(f.width, f.height) {
            surface.write_pixels(device, queue, &f.rgba, f.width, f.height);
        } else {
            compose_letterbox(f, &lb, &mut self.canvas);
            surface.write_pixels(device, queue, &self.canvas, lb.canvas_w, lb.canvas_h);
        }
        self.last_dirty = false;
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
        // The decision is GPU-free and tested (`plan_frame`); this is only
        // the GPU work it asks for.
        match self.plan_frame(&mut surface.core) {
            FrameDraw::Notice { text, px } => {
                // Back to the display's size: a no-op when the surface is
                // already there (a paused clip does not reallocate every
                // frame); the next written frame resizes to the clip again,
                // and `frame_surfaces` rebinds the scene material to the
                // new texture either way.
                surface.resize(device, px.0, px.1);
                surface.run_and_render(device, queue, theme, gui_state, |core, _theme, state| {
                    core.run_with(state, |ctx, _| notice(ctx, &text))
                });
            }
            FrameDraw::Frame => self.write_last(surface, device, queue),
            FrameDraw::Keep => {}
        }
    }

    fn kind(&self) -> &'static str {
        "video"
    }

    fn status(&self) -> serde_json::Value {
        let (playing, position_s, duration_s, audio) = match &self.player {
            Some(p) => (p.is_playing(), p.position_s(), p.duration_s(), p.has_audio()),
            None => (false, 0.0, 0.0, false),
        };
        serde_json::json!({
            "path": self.source,
            "resolved": self.resolved.as_ref().map(|p| p.display().to_string()),
            "playing": playing,
            "paused": !self.want_playing,
            "position_s": position_s,
            "duration_s": duration_s,
            "loops": self.loops,
            "audio": audio,
            "error": self.error,
        })
    }

    /// A press toggles pause; the release does nothing (a click is one
    /// toggle, not two). Only THIS provider's screen sees its own clicks:
    /// the surface routes a button event to the provider it holds, so a
    /// click on a page screen or a web screen never reaches a clip.
    fn on_button(&mut self, _uv: (f32, f32), pressed: bool) -> bool {
        if pressed {
            self.toggle_pause();
        }
        pressed
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
    /// (`VideoPlayer::drop`), so no decoder keeps running for a wall that
    /// no longer exists and no soundtrack keeps playing from nowhere.
    fn drop(&mut self) {
        if let Some(p) = self.player.take() {
            drop(p);
            log::info!("[Screens] video {:?} stopped", self.source);
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

    fn data_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data")
    }

    /// Every piece of text egui laid out in a run, joined; how a test reads
    /// what a notice page SAYS rather than only that it drew something.
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

    /// The click hook as pure state: a press toggles, a release does not,
    /// and once a player exists the toggle really pauses and resumes it.
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

    /// Headless: a missing file draws an error page that NAMES the path, on
    /// the same core the surface would run it on. Proven able to fail: with
    /// the text replaced by a bare "error" the path assertion fires.
    #[test]
    fn error_page_names_a_missing_file() {
        let mut theme = load_theme();
        let mut state = GuiState::default();
        let mut core = ScreenCore::new("s", "video:media/nothing.webm", 640, 360, &theme);
        assert_eq!(core.source, ScreenSource::Video("media/nothing.webm".into()));

        let mut p = VideoProvider::new("media/nothing.webm");
        p.open_with(&data_dir(), core.size());
        assert!(p.player.is_none());
        let err = p.error().expect("a missing file is an error");
        assert!(err.contains("not found"), "{err}");
        assert!(err.contains("nothing.webm"), "the error names the file: {err}");
        assert!(!p.advance(), "nothing to advance without a player");

        let text = p.error_text();
        let out = core.run_with(&mut state, |ctx, _| notice(ctx, &text));
        let drawn = shapes_text(&out.shapes);
        assert!(drawn.contains("media/nothing.webm"), "the page must name the path, drew: {drawn:?}");
        assert!(drawn.contains("not found"), "and say why: {drawn:?}");
        let _ = &mut theme;
    }

    /// Headless: an unsupported codec (the VP8 fixture, found through the
    /// repo-root branch of the path rule) draws an error page that names the
    /// codec, so the operator knows what to transcode.
    #[test]
    fn error_page_names_the_codec_of_an_unsupported_file() {
        let mut state = GuiState::default();
        let theme = load_theme();
        let mut core = ScreenCore::new("s", "video:x", 640, 360, &theme);
        let mut p = VideoProvider::new(UNSUPPORTED);
        p.open_with(&data_dir(), core.size());
        assert!(p.player.is_none());
        let resolved = p.status()["resolved"].as_str().map(str::to_string);
        assert!(resolved.as_deref().map_or(false, |r| r.ends_with("unsupported-vp8-vorbis.webm")), "{resolved:?}");
        let text = p.error_text();
        let out = core.run_with(&mut state, |ctx, _| notice(ctx, &text));
        let drawn = shapes_text(&out.shapes);
        assert!(drawn.contains("V_VP8"), "the page names the codec: {drawn:?}");
        assert!(drawn.contains("AV1"), "and the fix: {drawn:?}");
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

    /// The composed canvas has the frame's bytes at the offset and opaque
    /// black everywhere else, and a second compose reuses the canvas
    /// without re-filling.
    #[test]
    fn compose_letterbox_centres_the_frame_over_opaque_black_bars() {
        // A 4 x 2 frame of distinct pixels on a square display: bars above and below.
        let mut rgba = Vec::new();
        for i in 0..8u8 {
            rgba.extend_from_slice(&[10 + i, 20 + i, 30 + i, 255]);
        }
        let frame = VideoFrame { rgba, width: 4, height: 2, pts_s: 0.0 };
        let lb = letterbox_layout(4, 2, 100, 100);
        assert_eq!(lb, Letterbox { canvas_w: 4, canvas_h: 4, x: 0, y: 1 });
        let mut canvas = Vec::new();
        compose_letterbox(&frame, &lb, &mut canvas);
        assert_eq!(canvas.len(), 4 * 4 * 4);
        // One pixel of a 4-wide canvas (takes the canvas so the borrow ends
        // with each call and the canvas can be composed into again below).
        fn px(canvas: &[u8], x: usize, y: usize) -> &[u8] {
            &canvas[(y * 4 + x) * 4..(y * 4 + x) * 4 + 4]
        }
        for x in 0..4 {
            assert_eq!(px(&canvas, x, 0), &[0, 0, 0, 255], "top bar is opaque black");
            assert_eq!(px(&canvas, x, 3), &[0, 0, 0, 255], "bottom bar is opaque black");
            assert_eq!(px(&canvas, x, 1), &frame.rgba[x * 4..x * 4 + 4], "frame row 0 at canvas row 1");
            assert_eq!(px(&canvas, x, 2), &frame.rgba[(4 + x) * 4..(4 + x) * 4 + 4], "frame row 1 at canvas row 2");
        }
        // Side bars: a 2 x 4 frame on the same square display.
        let tall = VideoFrame { rgba: vec![200; 2 * 4 * 4], width: 2, height: 4, pts_s: 0.0 };
        let lb = letterbox_layout(2, 4, 100, 100);
        assert_eq!(lb, Letterbox { canvas_w: 4, canvas_h: 4, x: 1, y: 0 });
        compose_letterbox(&tall, &lb, &mut canvas);
        for y in 0..4 {
            assert_eq!(px(&canvas, 0, y), &[0, 0, 0, 255]);
            assert_eq!(px(&canvas, 3, y), &[0, 0, 0, 255]);
            assert_eq!(px(&canvas, 1, y), &[200, 200, 200, 200]);
            assert_eq!(px(&canvas, 2, y), &[200, 200, 200, 200]);
        }
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

    /// A playing clip drops the core's queued input every tick, whether or
    /// not a frame is written that tick; a paused clip keeps its events for
    /// the run that draws the notice, which consumes them. The read-back is
    /// the core's own count. Proven able to fail: with the drop removed from
    /// `plan_frame` the count after the playing tick stays at 25.
    #[test]
    fn a_playing_clip_drops_the_queued_input_and_a_paused_one_hands_it_to_the_notice() {
        let theme = load_theme();
        let mut state = GuiState::default();
        let mut core = ScreenCore::new("s", "video:x", 1280, 720, &theme);
        let mut p = VideoProvider::new(DEMO);
        p.open_with(&data_dir(), core.size());
        assert!(p.error().is_none(), "{:?}", p.error());

        let sweep = |core: &mut ScreenCore| {
            for i in 0..25 {
                core.pointer_moved((i as f32 / 25.0, 0.4));
            }
        };
        sweep(&mut core);
        assert_eq!(core.pending_events(), 25);
        let draw = p.plan_frame(&mut core);
        assert!(matches!(draw, FrameDraw::Frame | FrameDraw::Keep), "playing: {draw:?}");
        assert_eq!(core.pending_events(), 0, "a playing tick drops the backlog");
        // A tick with nothing new to write still drops.
        sweep(&mut core);
        let _ = p.plan_frame(&mut core);
        assert_eq!(core.pending_events(), 0);

        // Paused: the events survive to the notice run, which drains them.
        p.on_button((0.5, 0.5), true);
        sweep(&mut core);
        let draw = p.plan_frame(&mut core);
        assert!(matches!(draw, FrameDraw::Notice { .. }), "paused: {draw:?}");
        assert_eq!(core.pending_events(), 25, "the notice run gets the events, plan_frame does not eat them");
        let FrameDraw::Notice { text, .. } = draw else { unreachable!() };
        core.run_with(&mut state, |ctx, _| notice(ctx, &text));
        assert_eq!(core.pending_events(), 0);
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

    /// The paused and error notices are laid out at the DISPLAY's pixel
    /// size (the def's `px`), not at the clip's size the surface was
    /// resized to by the last written frame. The core here is put at the
    /// clip's 320 x 180, as `write_pixels` leaves it, before each notice is
    /// planned. Proven able to fail: with `px` taken from `core.size()` in
    /// `plan_frame` both assertions read (320, 180).
    #[test]
    fn notices_lay_out_at_the_display_size_not_the_clip_size() {
        let theme = load_theme();
        let mut core = ScreenCore::new("s", "video:x", 1280, 720, &theme);
        let mut p = VideoProvider::new(DEMO);
        p.open_with(&data_dir(), core.size());
        // The last written frame left the surface at the clip's size.
        core.set_size(320, 180);
        p.on_button((0.5, 0.5), true);
        match p.plan_frame(&mut core) {
            FrameDraw::Notice { px, text } => {
                assert_eq!(px, (1280, 720), "the paused page is laid out at the def's px");
                assert!(text.starts_with("Paused"), "{text}");
            }
            other => panic!("paused must draw a notice, got {other:?}"),
        }
        // The error page too, for a clip that failed to open.
        let mut core = ScreenCore::new("s", "video:x", 1024, 600, &theme);
        let mut e = VideoProvider::new("media/nothing.webm");
        e.open_with(&data_dir(), core.size());
        core.set_size(320, 180);
        match e.plan_frame(&mut core) {
            FrameDraw::Notice { px, text } => {
                assert_eq!(px, (1024, 600));
                assert!(text.contains("not found"), "{text}");
            }
            other => panic!("an open error must draw a notice, got {other:?}"),
        }
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
