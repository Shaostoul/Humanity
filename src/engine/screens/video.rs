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
//!   frame is kept in memory so play resumes on it at once.
//! * LOOPING: the clip restarts when the player reports its end; the sound
//!   loops on kira's side (a loop region on the stream), see `advance`.
//!
//! The file is in two halves so the logic is testable on a machine with no
//! GPU: `open_with`, `advance`, the pause toggle, `letterbox_layout`,
//! `compose_letterbox`, `resolve_media_path` and `audio_placement` are pure
//! or player-only and every test below drives them; `frame` and
//! `world_update` are thin glue over them.

use std::path::{Path, PathBuf};

use glam::Vec3;

use crate::gui::screen_surface::{notice, ScreenProvider, ScreenSurface, ScreenWorld};
use crate::gui::theme::Theme;
use crate::gui::GuiState;
use crate::media::{VideoFrame, VideoPlayer};

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
const MIX_EPSILON: f64 = 0.004;
/// Tween for volume and pan updates: long enough that walking past a screen
/// glides instead of stepping, short enough that turning the head feels
/// immediate.
const MIX_TWEEN_MS: u64 = 60;

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
    /// Whether the kira stream has been attached (it needs the audio
    /// manager, which arrives through `world_update`). Also set after a
    /// failed attach so the failure is logged once, not every frame.
    audio_attached: bool,
    /// The last (volume, pan) sent to the audio thread.
    mix_sent: Option<(f64, f64)>,
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
            audio_attached: false,
            mix_sent: None,
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

    /// Advance playback one tick: loop if the clip ended, take the frame
    /// that is due (if any), surface a decode error. Returns true when a NEW
    /// frame is now in `last`. GPU-free; `frame` calls this and then writes.
    ///
    /// The wrap check runs BEFORE the poll, so a frame delivered by this
    /// call always belongs to the loop count read after it: with the order
    /// reversed the frame taken just before a wrap was stamped with the
    /// next loop's number (the loop test caught that). The cost is that the
    /// restart lands one tick after the clock reaches the end, which is one
    /// frame of the last picture held.
    pub fn advance(&mut self) -> bool {
        let Some(p) = self.player.as_mut() else { return false };
        if p.at_end() {
            // The clock reached the declared end (and the player stopped it
            // there). Back to the first frame; keep rolling unless the
            // player paused it. The sound has already wrapped on kira's
            // side (see `VideoPlayer::attach_audio_looping`); the seek
            // re-aligns the few milliseconds between the two ends. On a
            // starved machine the decoder can be behind the clock here and
            // the tail frames are dropped: the clock is the master.
            p.seek_to_start();
            self.loops += 1;
            if self.want_playing {
                p.play();
            }
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
        self.ensure_open(surface.size());
        self.advance();
        if self.error.is_some() {
            let text = self.error_text();
            surface.run_and_render(device, queue, theme, gui_state, |core, _theme, state| {
                core.run_with(state, |ctx, _| notice(ctx, &text))
            });
            return;
        }
        if !self.want_playing {
            let text = self.paused_text();
            surface.run_and_render(device, queue, theme, gui_state, |core, _theme, state| {
                core.run_with(state, |ctx, _| notice(ctx, &text))
            });
            // The page replaced the picture in the texture; the last frame
            // goes back the moment play resumes, before any new frame is due.
            self.last_dirty = self.last.is_some();
            return;
        }
        if self.last_dirty {
            self.write_last(surface, device, queue);
        }
        // No new frame this tick: the texture keeps the previous one, which
        // is exactly what a display does between frames.
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

    /// Attach the sound once the audio manager is available, then place it:
    /// volume from the distance, pan from the bearing, both on the sfx bus
    /// under the master volume so the Settings sliders govern a film on the
    /// wall like any other world sound.
    fn world_update(&mut self, world: &mut ScreenWorld<'_>) {
        let Some(p) = self.player.as_mut() else { return };
        let Some(audio) = world.audio.as_deref_mut() else { return };
        if !self.audio_attached {
            self.audio_attached = true;
            if let Err(e) = p.attach_audio_looping(audio, true) {
                log::warn!("[Screens] video {:?}: sound not attached: {e}", self.source);
            }
        }
        if !p.has_audio() {
            return;
        }
        let (falloff, pan) = audio_placement(
            Vec3::from_array(world.screen_centre),
            Vec3::from_array(world.listener_pos),
            Vec3::from_array(world.listener_right),
        );
        let volume = audio.master_volume() * audio.sfx_volume() * falloff;
        let changed = self
            .mix_sent
            .map_or(true, |(v, pn)| (v - volume).abs() > MIX_EPSILON || (pn - pan).abs() > MIX_EPSILON);
        if changed {
            p.set_audio_mix(volume, pan, MIX_TWEEN_MS);
            self.mix_sent = Some((volume, pan));
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
        assert!(seen.len() >= 40, "most of the 60 frames should arrive in the first pass, got {}", seen.len());
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
}
