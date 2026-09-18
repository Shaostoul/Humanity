//! The LIVE STREAM screen (in-world screens, rung 3): the MJPEG stream the
//! Watch page decodes (`net::live_viewer`), shown on a wall. Source string
//! `watch:<stream id>`, where the stream id is the publisher's registered
//! name in lower case (that is how the relay keys a stream and how the
//! Watch page's directory lists it).
//!
//! EACH SCREEN OWNS ITS VIEWER. `GuiState::watch_viewer` (the Watch page's
//! viewer) keeps only the newest decoded frame and `take_latest` hands it to
//! ONE caller; if two screens (or a screen and the Watch page) shared that
//! viewer, every frame would go to whichever consumer polled first and the
//! others would show every other frame, or nothing. The rung-1 critic flagged
//! exactly this split. So [`LiveProvider`] starts its own `LiveViewer` on its
//! first frame, against the configured server, the same way the Watch page
//! starts one (`pages::watch::start_watching`), and drops it with the
//! surface (dropping a viewer stops its network thread). Two walls showing
//! one stream cost two sockets, and each gets every frame.
//!
//! Each framed tick the provider takes the newest decoded frame and writes
//! it into the surface with `write_pixels`, LETTERBOXED to the surface's own
//! pixel size ([`letterbox_into`]): the surface never changes size, so the
//! wall's quad never stretches the picture (a 16:9 stream on a 16:10 desk
//! monitor gets black bars, not a squeeze). While no frame has arrived, or
//! after the stream ended, a status page names the stream and its state
//! ("Connecting to ...", "Stream offline" with the relay's reason); a
//! viewer whose stream ended is retried after [`RETRY_AFTER`], so a wall
//! comes back on its own when the streamer goes live again.
//!
//! A picture that is up STAYS up between frames. The stream runs slower
//! than the game (a 20 fps stream on a 60 fps game hands the provider a
//! new frame one tick in three), so most ticks find no new frame while the
//! viewer is perfectly healthy. Those ticks must leave the surface alone:
//! `run_and_render` clears the whole texture before drawing, so drawing
//! the status page on a no-new-frame tick would paint "Connecting to ..."
//! over the live picture two ticks in three and the wall would strobe (the
//! rung-3 reviewer's blocker). [`next_display`] is that decision, pure,
//! pinned by a test: a new frame is written; no new frame with the picture
//! up and the viewer connected keeps the picture; anything else shows the
//! status page.

use crate::gui::screen_surface::{ScreenCore, ScreenProvider, ScreenSurface};
use crate::gui::theme::Theme;
use crate::gui::GuiState;
use crate::net::live_viewer::LiveViewer;
use std::time::{Duration, Instant};

/// How long a screen waits after its stream ended (or never connected)
/// before opening a new viewer. Long enough not to hammer a relay that
/// said "not live", short enough that a streamer going live is on the
/// wall within a coffee sip.
pub const RETRY_AFTER: Duration = Duration::from_secs(15);

/// Where a `frame` x `surface` letterbox lands: `(x, y, w, h)` in surface
/// pixels, the largest rectangle of the FRAME's aspect that fits the
/// surface, centred. A frame smaller than the surface is scaled up, a
/// larger one down; the aspect is preserved to within a pixel of rounding
/// either way (the "stretch" a bare `write_pixels` resize would give is
/// exactly what this avoids). Degenerate inputs give an empty rectangle.
pub fn letterbox_rect(frame: (u32, u32), surface: (u32, u32)) -> (u32, u32, u32, u32) {
    let (fw, fh) = frame;
    let (sw, sh) = surface;
    if fw == 0 || fh == 0 || sw == 0 || sh == 0 {
        return (0, 0, 0, 0);
    }
    let scale = f64::min(sw as f64 / fw as f64, sh as f64 / fh as f64);
    let w = ((fw as f64 * scale).round() as u32).clamp(1, sw);
    let h = ((fh as f64 * scale).round() as u32).clamp(1, sh);
    ((sw - w) / 2, (sh - h) / 2, w, h)
}

/// Fill `dst` (resized to `dst_size`, RGBA8) with black and the frame
/// `src` (`src_size`, RGBA8) letterboxed into it by nearest-neighbour
/// sampling. Returns the rectangle the frame landed in. A malformed source
/// (wrong byte count) leaves the destination black.
///
/// Nearest neighbour on purpose: it is one table lookup per pixel (about a
/// millisecond for a 1280 x 720 surface), and a wall screen is read from
/// across a room. A frame that already matches the surface size is copied
/// unchanged (the caller skips this and writes it directly).
pub fn letterbox_into(dst: &mut Vec<u8>, dst_size: (u32, u32), src: &[u8], src_size: (u32, u32)) -> (u32, u32, u32, u32) {
    let (dw, dh) = dst_size;
    let (sw, sh) = src_size;
    dst.clear();
    dst.resize((dw as usize) * (dh as usize) * 4, 0);
    // Opaque black bars: the alpha byte of every pixel.
    for px in dst.chunks_exact_mut(4) {
        px[3] = 255;
    }
    if src.len() != (sw as usize) * (sh as usize) * 4 {
        return (0, 0, 0, 0);
    }
    let rect = letterbox_rect(src_size, dst_size);
    let (x0, y0, w, h) = rect;
    if w == 0 || h == 0 {
        return rect;
    }
    // Source column for each destination column, computed once per frame
    // rather than once per pixel.
    let cols: Vec<usize> = (0..w).map(|x| ((x as u64 * sw as u64) / w as u64) as usize).collect();
    for y in 0..h {
        let sy = ((y as u64 * sh as u64) / h as u64) as usize;
        let srow = &src[sy * sw as usize * 4..(sy + 1) * sw as usize * 4];
        let dstart = ((y0 + y) as usize * dw as usize + x0 as usize) * 4;
        let drow = &mut dst[dstart..dstart + w as usize * 4];
        for (x, sx) in cols.iter().enumerate() {
            drow[x * 4..x * 4 + 4].copy_from_slice(&srow[sx * 4..sx * 4 + 4]);
        }
    }
    rect
}

/// What the surface currently shows, so the status page is redrawn only
/// when its text changes and never over a live picture that is still
/// arriving. `Status` carries the page's text (heading and detail joined by
/// a newline) so a page with the same words is not redrawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Shown {
    Nothing,
    Status(String),
    Video,
}

/// What one framed tick does to the surface (see [`next_display`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// A new decoded frame arrived: write it into the surface.
    Picture,
    /// No new frame, but the live picture is up and the viewer is still
    /// connected: leave the surface exactly as it is.
    Keep,
    /// Nothing live to keep: show the status page (redrawn only when its
    /// text changes; the caller compares against `Shown::Status`).
    Status,
}

/// THE DISPLAY DECISION for one framed tick, pure so a test can drive it
/// through the sequence a real stream produces without a GPU or a socket.
/// `shown` is what the surface holds now, `got_frame` whether the viewer
/// handed over a new decoded frame this tick, `connected` whether the
/// viewer has received frames and its thread is still running
/// (`LiveViewer::is_connected`).
///
/// The rule the reviewer's blocker pinned: a picture that has been shown
/// stays up while the viewer is connected, whether or not THIS tick brought
/// a new frame. Only a tick with no picture to keep (nothing shown yet, a
/// status page up, or the viewer gone) shows the status page.
pub fn next_display(shown: &Shown, got_frame: bool, connected: bool) -> Display {
    if got_frame {
        Display::Picture
    } else if *shown == Shown::Video && connected {
        Display::Keep
    } else {
        Display::Status
    }
}

/// The provider for `watch:<stream id>` (see the module doc).
pub struct LiveProvider {
    stream: String,
    /// This screen's own viewer; `None` before the first frame and while
    /// waiting to retry after the stream ended.
    viewer: Option<LiveViewer>,
    /// When to open the next viewer after the last one ended.
    retry_at: Option<Instant>,
    /// The relay's reason the last viewer ended ("This stream is not live
    /// right now.", "The stream ended.", a connection error), shown on the
    /// status page until a new viewer connects.
    last_error: String,
    /// Frames written into the surface so far.
    frames_written: u64,
    shown: Shown,
    /// The letterbox buffer, reused across frames.
    scratch: Vec<u8>,
}

impl LiveProvider {
    pub fn new(stream: &str) -> Self {
        Self {
            stream: stream.to_string(),
            viewer: None,
            retry_at: None,
            last_error: String::new(),
            frames_written: 0,
            shown: Shown::Nothing,
            scratch: Vec::new(),
        }
    }

    /// Whether the viewer has received a frame from the relay.
    pub fn is_connected(&self) -> bool {
        self.viewer.as_ref().map_or(false, |v| v.is_connected())
    }

    /// The status page's two lines for the current state: the heading and
    /// the detail. Pure, so the wording is pinned by a test.
    pub fn status_lines(&self) -> (String, String) {
        match (&self.viewer, self.last_error.is_empty()) {
            // Waiting for the first frame of a viewer that has not reported
            // a problem (or has not been started yet).
            (Some(_), true) | (None, true) => (
                format!("Connecting to {}", self.stream),
                "This screen shows the live stream published under that name.".to_string(),
            ),
            // The last viewer ended; a new one is opened after the retry
            // pause.
            (None, false) => ("Stream offline".to_string(), format!("{} Trying again shortly.", self.last_error)),
            // A viewer is up but the previous one ended: connecting again.
            (Some(_), false) => (format!("Connecting to {}", self.stream), self.last_error.clone()),
        }
    }

    /// Draw the status page through `core` (GPU-free: the surface's
    /// `run_and_render` wraps this, and the headless test calls it
    /// directly). Two centred lines on the theme's panel.
    pub fn draw_status_page(
        core: &mut ScreenCore,
        theme: &Theme,
        gui_state: &mut GuiState,
        heading: &str,
        detail: &str,
    ) -> egui::FullOutput {
        core.run_with(gui_state, |ctx, _| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.centered_and_justified(|ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(ui.available_height() * 0.4);
                        ui.label(egui::RichText::new(heading).size(theme.font_size_heading).strong());
                        ui.add_space(theme.panel_margin);
                        ui.label(egui::RichText::new(detail).size(theme.font_size_body).color(theme.text_muted()));
                    });
                });
            });
        })
    }

    /// Open this screen's viewer against the configured server, the way
    /// the Watch page does (`pages::watch::start_watching`).
    fn start_viewer(&mut self, gui_state: &GuiState) {
        let server = gui_state.server_url.trim_end_matches('/').to_string();
        self.viewer = Some(LiveViewer::start(&server, &self.stream));
        self.retry_at = None;
        // A new viewer starts from nothing: whatever the surface holds (the
        // last picture of the previous viewer, or its "offline" page) is
        // not this viewer's, so the status page is drawn again on the next
        // tick and the picture returns only when this viewer delivers one.
        self.shown = Shown::Nothing;
    }
}

impl ScreenProvider for LiveProvider {
    fn frame(
        &mut self,
        surface: &mut ScreenSurface,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &mut Theme,
        gui_state: &mut GuiState,
    ) {
        let now = Instant::now();
        // Open the viewer on the first framed tick, and again after the
        // retry pause once a previous one ended.
        if self.viewer.is_none() && self.retry_at.map_or(true, |t| now >= t) {
            self.start_viewer(gui_state);
        }
        if let Some(v) = self.viewer.as_ref() {
            // What this tick does is decided in one pure place
            // (`next_display`), so the test that pins "a picture stays up
            // between frames" drives the same logic the wall runs.
            let latest = v.take_latest();
            match next_display(&self.shown, latest.is_some(), v.is_connected()) {
                Display::Picture => {
                    // The newest decoded frame goes into the surface,
                    // letterboxed to the surface's own size unless it
                    // already matches.
                    let f = latest.expect("Picture is only chosen when a frame arrived");
                    let (sw, sh) = surface.size();
                    if (f.width, f.height) == (sw, sh) {
                        surface.write_pixels(device, queue, &f.rgba, sw, sh);
                    } else {
                        letterbox_into(&mut self.scratch, (sw, sh), &f.rgba, (f.width, f.height));
                        surface.write_pixels(device, queue, &self.scratch, sw, sh);
                    }
                    self.frames_written += 1;
                    self.shown = Shown::Video;
                    return;
                }
                // The picture is up and the stream is healthy; this tick
                // simply fell between two stream frames. Touch nothing.
                Display::Keep => return,
                Display::Status => {}
            }
            // The viewer thread ended (the relay said not live, the stream
            // ended, or the socket failed): drop it and schedule a retry.
            // The status page below replaces the frozen last picture, and
            // `shown` forgets the picture so the page is drawn now and a
            // reconnect shows its own status again before its first frame.
            if !v.is_connected() {
                let status = v.status();
                if !status.is_empty() {
                    self.last_error = status;
                    self.viewer = None;
                    self.retry_at = Some(now + RETRY_AFTER);
                    self.shown = Shown::Nothing;
                }
            }
        }
        // Nothing live on the surface: the status page, redrawn only when
        // its text changes.
        let (heading, detail) = self.status_lines();
        let key = format!("{heading}\n{detail}");
        if self.shown == Shown::Status(key.clone()) {
            return;
        }
        surface.run_and_render(device, queue, theme, gui_state, |core, theme, state| {
            Self::draw_status_page(core, theme, state, &heading, &detail)
        });
        self.shown = Shown::Status(key);
    }

    fn kind(&self) -> &'static str {
        "watch"
    }

    fn status(&self) -> serde_json::Value {
        serde_json::json!({
            "stream": self.stream,
            "connected": self.is_connected(),
            "frames": self.frames_written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::load_theme;

    /// The letterbox is the largest frame-aspect rectangle that fits,
    /// centred: a smaller 16:9 frame fills a 16:9 surface, a wider frame
    /// gets bars top and bottom, a taller one bars left and right, an equal
    /// frame maps 1:1. The aspect is preserved within rounding in every
    /// case (a stretch would fail the ratio assertion).
    #[test]
    fn letterbox_rect_fits_and_centres_without_stretching() {
        // Same aspect, smaller: fills the surface.
        assert_eq!(letterbox_rect((640, 360), (1280, 720)), (0, 0, 1280, 720));
        // Same size: identity.
        assert_eq!(letterbox_rect((1280, 720), (1280, 720)), (0, 0, 1280, 720));
        // Wider than the surface (16:9 on a 1024 x 600 desk monitor): full
        // width, bars top and bottom.
        let (x, y, w, h) = letterbox_rect((1920, 1080), (1024, 600));
        assert_eq!((x, w), (0, 1024));
        assert_eq!(h, 576);
        assert_eq!(y, 12, "centred: (600 - 576) / 2");
        // Taller than the surface (a portrait phone stream on a wall): full
        // height, bars left and right.
        let (x, y, w, h) = letterbox_rect((600, 800), (1280, 720));
        assert_eq!((y, h), (0, 720));
        assert_eq!(w, 540);
        assert_eq!(x, 370, "centred: (1280 - 540) / 2");
        // The aspect survives every case.
        for (frame, surface) in [((1920, 1080), (1024, 600)), ((600, 800), (1280, 720)), ((320, 240), (1280, 720)), ((4000, 3000), (1024, 600))] {
            let (_, _, w, h) = letterbox_rect(frame, surface);
            let want = frame.0 as f64 / frame.1 as f64;
            let got = w as f64 / h as f64;
            assert!((got - want).abs() < 0.01, "{frame:?} on {surface:?}: aspect {got} vs {want}");
            assert!(w <= surface.0 && h <= surface.1, "fits");
        }
        // Degenerate sizes give nothing rather than dividing by zero.
        assert_eq!(letterbox_rect((0, 10), (100, 100)), (0, 0, 0, 0));
        assert_eq!(letterbox_rect((10, 10), (0, 100)), (0, 0, 0, 0));
    }

    /// The resample lands the frame's pixels in the rectangle and leaves
    /// the bars opaque black. A 2 x 2 frame (red, green / blue, white) on a
    /// 8 x 4 surface: the picture is 4 x 4 in the middle (x 2..6), each
    /// source pixel a 2 x 2 block, bars of black either side.
    #[test]
    fn letterbox_into_places_pixels_and_paints_the_bars_black() {
        let src: Vec<u8> = vec![
            255, 0, 0, 255, /**/ 0, 255, 0, 255, // row 0: red, green
            0, 0, 255, 255, /**/ 255, 255, 255, 255, // row 1: blue, white
        ];
        let mut dst = Vec::new();
        let rect = letterbox_into(&mut dst, (8, 4), &src, (2, 2));
        assert_eq!(rect, (2, 0, 4, 4));
        assert_eq!(dst.len(), 8 * 4 * 4);
        let px = |x: usize, y: usize| -> [u8; 4] {
            let i = (y * 8 + x) * 4;
            [dst[i], dst[i + 1], dst[i + 2], dst[i + 3]]
        };
        // Bars: opaque black.
        for y in 0..4 {
            for x in [0, 1, 6, 7] {
                assert_eq!(px(x, y), [0, 0, 0, 255], "bar pixel ({x}, {y})");
            }
        }
        // The picture: 2 x 2 blocks of each source pixel.
        for (x, y) in [(2, 0), (3, 1)] {
            assert_eq!(px(x, y), [255, 0, 0, 255], "red at ({x}, {y})");
        }
        for (x, y) in [(4, 0), (5, 1)] {
            assert_eq!(px(x, y), [0, 255, 0, 255], "green at ({x}, {y})");
        }
        for (x, y) in [(2, 2), (3, 3)] {
            assert_eq!(px(x, y), [0, 0, 255, 255], "blue at ({x}, {y})");
        }
        for (x, y) in [(4, 2), (5, 3)] {
            assert_eq!(px(x, y), [255, 255, 255, 255], "white at ({x}, {y})");
        }
        // A frame larger than the surface is sampled down: a 4 x 4 frame
        // whose left half is red and right half green on a 2 x 2 surface.
        let mut big = Vec::new();
        for y in 0..4 {
            for x in 0..4 {
                let _ = y;
                big.extend_from_slice(if x < 2 { &[255, 0, 0, 255] } else { &[0, 255, 0, 255] });
            }
        }
        let rect = letterbox_into(&mut dst, (2, 2), &big, (4, 4));
        assert_eq!(rect, (0, 0, 2, 2));
        assert_eq!(&dst[0..4], &[255, 0, 0, 255], "left column red");
        assert_eq!(&dst[4..8], &[0, 255, 0, 255], "right column green");
        // A malformed source (wrong byte count) leaves the surface black.
        let rect = letterbox_into(&mut dst, (2, 2), &big[..7], (4, 4));
        assert_eq!(rect, (0, 0, 0, 0));
        assert!(dst.chunks_exact(4).all(|p| p == [0, 0, 0, 255]));
    }

    /// The status page, HEADLESS: a fresh provider (no viewer started, no
    /// network) draws a page whose text says it is connecting to the named
    /// stream, without panicking, through the same `ScreenCore` path the
    /// GPU surface uses. The text is checked in the tessellation input (the
    /// text shapes egui emitted), so a page that drew nothing, or the wrong
    /// words, fails.
    #[test]
    fn a_fresh_live_screen_draws_a_connecting_page_without_a_viewer() {
        let theme = load_theme();
        let mut state = GuiState::default();
        let provider = LiveProvider::new("shaostoul");
        assert!(provider.viewer.is_none(), "no viewer is started by construction (no network in a test)");
        assert!(!provider.is_connected());
        let (heading, detail) = provider.status_lines();
        assert_eq!(heading, "Connecting to shaostoul");
        assert!(detail.contains("live stream"), "{detail}");
        let mut core = ScreenCore::new("wall", "watch:shaostoul", 640, 360, &theme);
        let out = LiveProvider::draw_status_page(&mut core, &theme, &mut state, &heading, &detail);
        assert!(!out.shapes.is_empty(), "the status page draws something");
        let mut texts = Vec::new();
        for s in &out.shapes {
            if let egui::Shape::Text(t) = &s.shape {
                texts.push(t.galley.text().to_string());
            }
        }
        assert!(texts.iter().any(|t| t.contains("Connecting to shaostoul")), "the page names the stream: {texts:?}");
        // The rig's status fields on a fresh screen.
        assert_eq!(provider.status()["stream"], "shaostoul");
        assert_eq!(provider.status()["connected"], false);
        assert_eq!(provider.status()["frames"], 0);
        assert_eq!(provider.kind(), "watch");
    }

    /// THE STROBE GUARD. A stream runs slower than the game, so most ticks
    /// bring no new frame while the viewer is healthy; those ticks must keep
    /// the picture, never redraw the status page over it (`run_and_render`
    /// clears the texture first, so a redraw IS a strobe). Drives the pure
    /// decision through the real sequence: a tick with a frame writes it;
    /// a tick without one, still connected, keeps the picture; a tick
    /// without one after the viewer dropped shows the status page. On the
    /// pre-fix logic (frame or status, nothing else) the middle step
    /// returns `Status` and this fails.
    #[test]
    fn a_live_picture_stays_up_between_stream_frames_while_connected() {
        // Tick 1: the first decoded frame arrives; the surface gets it.
        let mut shown = Shown::Nothing;
        assert_eq!(next_display(&shown, true, true), Display::Picture);
        shown = Shown::Video;
        // Tick 2: no new frame yet (the stream is at 20 fps, the game at
        // 60), the viewer is connected: KEEP the picture.
        assert_eq!(next_display(&shown, false, true), Display::Keep, "no new frame + connected keeps the picture");
        // Tick 3: still no new frame, and the viewer has ended: the status
        // page replaces the frozen picture.
        assert_eq!(next_display(&shown, false, false), Display::Status, "disconnected shows the status page");
        // The surrounding rules: before any picture the status page shows
        // even while connected (the "socket up, nothing decoded yet" case),
        // a status page stays a status page until a frame arrives, and a
        // frame always wins, even one that landed as the thread ended.
        assert_eq!(next_display(&Shown::Nothing, false, true), Display::Status);
        assert_eq!(next_display(&Shown::Status("x".into()), false, true), Display::Status);
        assert_eq!(next_display(&Shown::Status("x".into()), true, true), Display::Picture);
        assert_eq!(next_display(&Shown::Video, true, false), Display::Picture, "a frame that arrived is shown even as the thread ends");
    }

    /// The reset half of the same guard: a dropped viewer and a replaced
    /// viewer both forget the picture, so a reconnect shows its status
    /// page before its first frame rather than a stale picture with no
    /// status. Exercised on the state fields directly (no socket): after
    /// the drop the decision for a no-frame tick is `Status`, not `Keep`,
    /// and the page it draws says the stream is offline.
    #[test]
    fn a_dropped_or_replaced_viewer_forgets_the_picture() {
        let mut p = LiveProvider::new("shaostoul");
        p.shown = Shown::Video;
        // The drop path, as `frame` performs it when the viewer's thread
        // ended with a reason: the reason is kept, the viewer goes, the
        // retry is scheduled, and the picture is forgotten.
        p.last_error = "The stream ended.".to_string();
        p.viewer = None;
        p.retry_at = Some(Instant::now() + RETRY_AFTER);
        p.shown = Shown::Nothing;
        assert_eq!(next_display(&p.shown, false, false), Display::Status);
        let (heading, _) = p.status_lines();
        assert_eq!(heading, "Stream offline");
        // A status page drawn once is not redrawn while its text is the
        // same: `frame` compares against the exact key it stored.
        let key = format!("{}\n{}", p.status_lines().0, p.status_lines().1);
        p.shown = Shown::Status(key.clone());
        assert_eq!(next_display(&p.shown, false, false), Display::Status, "the caller then skips the redraw on an equal key");
        assert_eq!(p.shown, Shown::Status(key));
        // (The retry path, `start_viewer`, sets `shown = Shown::Nothing`
        // the same way; it opens a socket, so it is not driven here.)
    }

    /// The wording follows the viewer's fate: an ended stream shows
    /// "Stream offline" with the relay's reason, and the retry pause is
    /// scheduled.
    #[test]
    fn status_lines_report_an_ended_stream() {
        let mut p = LiveProvider::new("shaostoul");
        p.last_error = "This stream is not live right now.".to_string();
        p.retry_at = Some(Instant::now() + RETRY_AFTER);
        let (heading, detail) = p.status_lines();
        assert_eq!(heading, "Stream offline");
        assert!(detail.starts_with("This stream is not live right now."), "{detail}");
        assert!(detail.contains("again"), "{detail}");
    }
}
