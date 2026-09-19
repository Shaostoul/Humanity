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
//! after the stream ended, a status page names the stream and its state; a
//! viewer whose stream ended is retried after [`RETRY_AFTER`], so a wall
//! comes back on its own when the streamer goes live again.
//!
//! WHAT THE STATUS PAGE SAYS is [`live_status`], pure over (server, stream
//! id, last error), and it answers two things at once: the words, and
//! whether to connect. The second matters because of a real wall the
//! operator hit (2026-09-18, photographed in the console room): with nobody
//! signed in, `GuiState::server_url` is empty, so the composed address is
//! `/ws/live/sub/<id>`, which has NO HOST NAME. The screen showed
//! "Could not connect: URL error: No host name in the URL Trying again
//! shortly." and retried that forever. A URL with no host cannot ever work,
//! so now it is not built at all: the screen says no server is set and
//! where to set one, and opens no socket.
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
use crate::net::live_viewer::{EndReason, LiveViewer};
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

/// The last thing that went wrong on this screen: WHY the viewer's thread
/// ended, plus the sentence the viewer wrote for a human. `Default` (reason
/// `None`, empty text) means "nothing has gone wrong yet".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LastError {
    pub reason: EndReason,
    pub text: String,
}

/// What a live screen says, and whether it may open a socket at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveStatus {
    /// The big line.
    pub heading: String,
    /// The small line under it.
    pub detail: String,
    /// Whether the screen should (re)connect. False ONLY when there is no
    /// server to connect to, because a URL with no host name cannot ever
    /// work: retrying it just repeats the same parse error forever, which is
    /// exactly the wall the operator photographed.
    pub connect: bool,
}

/// Whether a configured server string can name a host at all.
///
/// The composed viewer URL is `<server>/ws/live/sub/<id>` (see
/// `net::live_viewer::LiveViewer::start`), so an empty server composes
/// `/ws/live/sub/<id>`, which has no host name and cannot be parsed, let
/// alone connected to.
///
/// This is deliberately a HOST check and not a URL parse: a string that
/// names a host but gets the scheme wrong ("localhost:3210") is a typo the
/// person can see in the error the connection gives back, while a string
/// with no host at all can only ever produce the same meaningless parse
/// error however many times it is retried.
pub fn server_has_host(server: &str) -> bool {
    let s = server.trim();
    let after_scheme = match s.split_once("://") {
        // "://host" with no scheme in front of it is not an address either.
        Some((scheme, _)) if scheme.trim().is_empty() => return false,
        Some((_, rest)) => rest,
        None => s,
    };
    // The host is everything before the first path separator.
    !after_scheme.split('/').next().unwrap_or("").trim().is_empty()
}

/// THE MESSAGE DECISION, pure over (server, stream id, last error).
///
/// A wall that cannot show a stream must say WHICH wall it is standing at,
/// because those walls need different things from the person reading them
/// (the project norm: "a wall a person hits must name its reason",
/// CLAUDE.md). Three of them:
///
/// 1. **No server set.** Nothing to connect to, so nothing is attempted.
///    The screen names where the server is set instead of retrying an
///    address that cannot work.
/// 2. **Nothing live under that name.** The relay answered; the name is
///    simply not broadcasting. Polite, and it keeps retrying.
/// 3. **A real connection or protocol failure.** The reason as the viewer
///    reported it, and it keeps retrying.
///
/// Before the first failure (case 0) the screen is simply connecting.
pub fn live_status(server: &str, stream: &str, last: &LastError) -> LiveStatus {
    // Case 1: no server. This is checked FIRST because it is the only case
    // where the answer is "do not even try".
    if !server_has_host(server) {
        return LiveStatus {
            heading: "No server set".to_string(),
            detail: format!(
                "This screen would show the stream published as \"{stream}\", \
                 but no server is set to look on. Open Chat and put a server \
                 address in the Server field of the connection panel."
            ),
            connect: false,
        };
    }
    // The viewer's own sentence, when it wrote one; otherwise a default so a
    // fault is never described by an empty string.
    let words = |fallback: &str| -> String {
        if last.text.trim().is_empty() {
            fallback.to_string()
        } else {
            last.text.trim().to_string()
        }
    };
    match last.reason {
        // Case 0: no fault yet. A viewer is on its way, or about to be.
        EndReason::None => LiveStatus {
            heading: format!("Connecting to {stream}"),
            detail: "This screen shows the live stream published under that name.".to_string(),
            connect: true,
        },
        // Case 2: the relay answered, and nobody is publishing that name.
        EndReason::NotLive => LiveStatus {
            heading: "Stream offline".to_string(),
            detail: format!("No one is streaming as \"{stream}\" right now. Trying again shortly."),
            connect: true,
        },
        // Case 2, the other way in: the publisher was there and stopped.
        EndReason::Ended => LiveStatus {
            heading: "Stream offline".to_string(),
            detail: format!("{} Trying again shortly.", words("The stream ended.")),
            connect: true,
        },
        // Case 3a: the relay is fine, the stream is simply full.
        EndReason::AtCapacity => LiveStatus {
            heading: "Stream full".to_string(),
            detail: format!(
                "{} Trying again shortly.",
                words("This stream is at viewer capacity.")
            ),
            connect: true,
        },
        // Case 3b: the socket itself. The reason is the useful part, so it
        // leads; the heading says which layer failed.
        EndReason::Connection => LiveStatus {
            heading: "Cannot reach the server".to_string(),
            detail: format!("{} Trying again shortly.", words("The connection failed.")),
            connect: true,
        },
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
    /// Why the last viewer ended, and the sentence it wrote, shown on the
    /// status page until a new viewer connects. See [`live_status`].
    last_error: LastError,
    /// Frames written into the surface so far.
    frames_written: u64,
    shown: Shown,
    /// The message this screen last decided on, mirrored into `status()` so
    /// a rig (or the operator's own `debug/screen_request.json`) can read
    /// what the wall SAYS without reading its pixels. `None` before the
    /// first framed tick.
    message: Option<LiveStatus>,
    /// The letterbox buffer, reused across frames.
    scratch: Vec<u8>,
}

impl LiveProvider {
    pub fn new(stream: &str) -> Self {
        Self {
            stream: stream.to_string(),
            viewer: None,
            retry_at: None,
            last_error: LastError::default(),
            frames_written: 0,
            shown: Shown::Nothing,
            message: None,
            scratch: Vec::new(),
        }
    }

    /// Whether the viewer has received a frame from the relay.
    pub fn is_connected(&self) -> bool {
        self.viewer.as_ref().map_or(false, |v| v.is_connected())
    }

    /// This screen's message and connect decision for the configured server.
    /// A thin wrapper over the pure [`live_status`] so the provider keeps one
    /// way of asking.
    pub fn status_for(&self, server: &str) -> LiveStatus {
        live_status(server, &self.stream, &self.last_error)
    }

    /// Draw the status page through `core` (GPU-free: the surface's
    /// `run_and_render` wraps this, and the headless test calls it
    /// directly). Two centred lines on the theme's panel.
    ///
    /// Read from across a room, so: the heading takes the theme's PRIMARY
    /// text colour rather than egui's default (which came out nearly black
    /// on the dark panel, proven by the rig's own evidence PNGs), and the
    /// detail is held to a column instead of running the full width of a
    /// 1280 px wall, where a sentence becomes one thin line of small text.
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
                        ui.label(
                            egui::RichText::new(heading)
                                .size(theme.font_size_heading)
                                .color(theme.text_primary())
                                .strong(),
                        );
                        ui.add_space(theme.panel_margin);
                        // Roughly two thirds of the wall, so the sentence
                        // wraps into a readable block near the middle.
                        let column = ui.available_width() * 0.66;
                        ui.allocate_ui_with_layout(
                            egui::vec2(column, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(detail)
                                        .size(theme.font_size_body)
                                        .color(theme.text_secondary()),
                                );
                            },
                        );
                    });
                });
            });
        })
    }

    /// Open this screen's viewer against the configured server, the way
    /// the Watch page does (`pages::watch::start_watching`). Only ever
    /// called when [`live_status`] said `connect`.
    fn start_viewer(&mut self, gui_state: &GuiState) {
        let server = gui_state.server_url.trim_end_matches('/').to_string();
        // One line per socket opened: the rig reads this to prove a screen
        // with no server opened NONE.
        log::info!("[Screens] watch:{}: opening a viewer on {server}", self.stream);
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
        // WHAT THIS SCREEN SAYS AND WHETHER IT MAY CONNECT, decided in one
        // pure place over the configured server, the stream id and the last
        // fault. With no server the answer is "do not connect at all": the
        // composed address would have no host name, and retrying it forever
        // is what put a parse error on the operator's wall.
        let status = self.status_for(&gui_state.server_url);
        self.message = Some(status.clone());
        if !status.connect {
            // The server went away (or was never set). Drop any viewer we
            // hold so its thread stops, and forget the picture so the page
            // below replaces it.
            if self.viewer.take().is_some() {
                self.shown = Shown::Nothing;
            }
            self.retry_at = None;
        } else if self.viewer.is_none() && self.retry_at.map_or(true, |t| now >= t) {
            // Open the viewer on the first framed tick, and again after the
            // retry pause once a previous one ended.
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
                let words = v.status();
                if !words.is_empty() {
                    self.last_error = LastError { reason: v.end_reason(), text: words };
                    self.viewer = None;
                    self.retry_at = Some(now + RETRY_AFTER);
                    self.shown = Shown::Nothing;
                }
            }
        }
        // Nothing live on the surface: the status page, redrawn only when
        // its text changes. Re-decided here because the leg above may have
        // just recorded a fault, and the page must describe THIS tick.
        let status = self.status_for(&gui_state.server_url);
        self.message = Some(status.clone());
        let key = format!("{}\n{}", status.heading, status.detail);
        let same_words = self.shown == Shown::Status(key.clone());
        // A page whose words have not changed is not redrawn (the redraw
        // clears the texture first, so redrawing for nothing is a flicker).
        // The ONE exception is a dev-IPC `find` waiting for an answer: the
        // lookup reads the shapes of a run, so with no run it would report
        // that the words a person can plainly read are not there. The same
        // rule the video screen follows (`video.rs`, `core.find_pending()`).
        if same_words && !surface.core.find_pending() {
            return;
        }
        // One line per CHANGE of message (not per frame, and not for a
        // find's redraw), so the log reads as the history of what the wall
        // said.
        if !same_words {
            log::info!("[Screens] watch:{}: {} ({})", self.stream, status.heading, status.detail);
        }
        surface.run_and_render(device, queue, theme, gui_state, |core, theme, state| {
            Self::draw_status_page(core, theme, state, &status.heading, &status.detail)
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
            // What the wall says right now, and whether it is allowed to
            // open a socket at all. Reading these is how a rig proves the
            // no-server screen NEVER tried to connect.
            "heading": self.message.as_ref().map(|m| m.heading.clone()),
            "detail": self.message.as_ref().map(|m| m.detail.clone()),
            "may_connect": self.message.as_ref().map(|m| m.connect),
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
        let msg = provider.status_for("https://united-humanity.us");
        assert_eq!(msg.heading, "Connecting to shaostoul");
        assert!(msg.detail.contains("live stream"), "{}", msg.detail);
        let mut core = ScreenCore::new("wall", "watch:shaostoul", 640, 360, &theme);
        let out = LiveProvider::draw_status_page(&mut core, &theme, &mut state, &msg.heading, &msg.detail);
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
        p.last_error = LastError { reason: EndReason::Ended, text: "The stream ended.".to_string() };
        p.viewer = None;
        p.retry_at = Some(Instant::now() + RETRY_AFTER);
        p.shown = Shown::Nothing;
        assert_eq!(next_display(&p.shown, false, false), Display::Status);
        let msg = p.status_for(SERVER);
        assert_eq!(msg.heading, "Stream offline");
        // A status page drawn once is not redrawn while its text is the
        // same: `frame` compares against the exact key it stored.
        let key = format!("{}\n{}", msg.heading, msg.detail);
        p.shown = Shown::Status(key.clone());
        assert_eq!(next_display(&p.shown, false, false), Display::Status, "the caller then skips the redraw on an equal key");
        assert_eq!(p.shown, Shown::Status(key));
        // (The retry path, `start_viewer`, sets `shown = Shown::Nothing`
        // the same way; it opens a socket, so it is not driven here.)
    }

    /// A server address the screen can actually use. Any non-empty host will
    /// do: nothing in these tests opens a socket.
    const SERVER: &str = "https://united-humanity.us";

    /// THE THREE WALLS, each named on the screen it belongs to. This is the
    /// defect the operator photographed (a `watch:` screen reading "Could
    /// not connect: URL error: No host name in the URL Trying again
    /// shortly.") and the two honest answers it should have given instead.
    ///
    /// Red proof: point case 1 at a server (`live_status(SERVER, ...)`) and
    /// the no-server assertions fail; make case 1 return `connect: true` and
    /// the "nothing is attempted" assertion fails.
    #[test]
    fn each_wall_a_live_screen_hits_names_its_own_reason() {
        // ── Case 1: no server known. Nothing to connect to, so nothing is
        // attempted, and the screen says where a server is set.
        for empty in ["", "   ", "/", "https://", "  https://  "] {
            let s = live_status(empty, "shaostoul", &LastError::default());
            assert!(!s.connect, "a server string of {empty:?} cannot name a host, so nothing may be attempted");
            assert_eq!(s.heading, "No server set");
            assert!(s.detail.contains("shaostoul"), "the screen still names its stream: {}", s.detail);
            assert!(s.detail.contains("Server field"), "it says WHERE to set one: {}", s.detail);
            // The old parse error must not survive anywhere in the words.
            assert!(!s.detail.contains("host name"), "no parse error on the wall: {}", s.detail);
        }
        // A real address is a host, with or without a trailing slash or the
        // chat socket's own suffix.
        for good in ["https://united-humanity.us", "https://united-humanity.us/", "http://127.0.0.1:3210", "wss://example.org/ws"] {
            assert!(server_has_host(good), "{good} names a host");
            assert!(live_status(good, "shaostoul", &LastError::default()).connect);
        }

        // ── Case 0: a server, no fault yet. Connecting.
        let s = live_status(SERVER, "shaostoul", &LastError::default());
        assert_eq!(s.heading, "Connecting to shaostoul");
        assert!(s.connect);

        // ── Case 2: the relay answered, and nobody is streaming that name.
        // Polite, names the stream, keeps the retry.
        let not_live = LastError { reason: EndReason::NotLive, text: "This stream is not live right now.".into() };
        let s = live_status(SERVER, "shaostoul", &not_live);
        assert_eq!(s.heading, "Stream offline");
        assert!(s.detail.starts_with("No one is streaming as \"shaostoul\""), "{}", s.detail);
        assert!(s.detail.contains("Trying again shortly."), "{}", s.detail);
        assert!(s.connect, "a name that is not live now may be live in a minute");

        // ── Case 3: a real connection or protocol failure keeps the reason.
        let broke = LastError { reason: EndReason::Connection, text: "Stream error: connection reset".into() };
        let s = live_status(SERVER, "shaostoul", &broke);
        assert_eq!(s.heading, "Cannot reach the server");
        assert!(s.detail.starts_with("Stream error: connection reset"), "the reason leads: {}", s.detail);
        assert!(s.detail.contains("Trying again shortly."), "{}", s.detail);
        assert!(s.connect);

        // The two remaining fates, for completeness: a publisher who stopped
        // and a stream at its viewer ceiling.
        let ended = LastError { reason: EndReason::Ended, text: "The stream ended.".into() };
        let s = live_status(SERVER, "shaostoul", &ended);
        assert_eq!(s.heading, "Stream offline");
        assert!(s.detail.starts_with("The stream ended."), "{}", s.detail);
        let full = LastError { reason: EndReason::AtCapacity, text: "This stream is at viewer capacity.".into() };
        let s = live_status(SERVER, "shaostoul", &full);
        assert_eq!(s.heading, "Stream full");
        assert!(s.detail.contains("viewer capacity"), "{}", s.detail);

        // A fault with no sentence (the viewer died before writing one) is
        // still described, never with an empty line.
        for reason in [EndReason::Ended, EndReason::AtCapacity, EndReason::Connection] {
            let s = live_status(SERVER, "shaostoul", &LastError { reason, text: String::new() });
            assert!(s.detail.len() > "Trying again shortly.".len(), "{reason:?} says something: {}", s.detail);
        }
    }

    /// The no-server page DRAWS, through the same `ScreenCore` path the wall
    /// uses, and the words a person would read are the new ones.
    #[test]
    fn the_no_server_page_draws_its_own_words() {
        let theme = load_theme();
        let mut state = GuiState::default();
        let p = LiveProvider::new("shaostoul");
        let msg = p.status_for("");
        assert!(!msg.connect);
        let mut core = ScreenCore::new("wall", "watch:shaostoul", 640, 360, &theme);
        let out = LiveProvider::draw_status_page(&mut core, &theme, &mut state, &msg.heading, &msg.detail);
        let mut texts = Vec::new();
        for s in &out.shapes {
            if let egui::Shape::Text(t) = &s.shape {
                texts.push(t.galley.text().to_string());
            }
        }
        assert!(texts.iter().any(|t| t.contains("No server set")), "the heading is drawn: {texts:?}");
        assert!(texts.iter().any(|t| t.contains("Server field")), "the detail is drawn: {texts:?}");
    }
}
