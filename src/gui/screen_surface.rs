//! A native egui page rendered into its own texture, so a flat SCREEN placed
//! in the 3D world can show it (in-world screens, rung 1; the design doc is
//! docs/design/in-world-screens.md).
//!
//! The operator asked for the ACTUAL app pages on in-game touchscreens, not
//! web pages: "include the menu pages as in-game touchscreens for like the
//! inventory page. Not the webpage but, the actual native app inventory
//! page." So a surface runs the very same page function the full-screen UI
//! runs (`gui::dispatch::draw_tool_page`) against the very same `GuiState`
//! (same inventory, same tasks, same chat), only under a SECOND
//! `egui::Context` of its own. Its own context means its own egui memory:
//! scroll positions, open/closed headers and text-field focus are per screen,
//! so two wall screens showing the inventory can be scrolled to different
//! places while the main UI's copy stays where the player left it.
//!
//! The file is split in two halves so the interaction logic is testable on a
//! machine with no GPU (CI):
//!
//! * [`ScreenCore`] owns the context, the pending synthetic events, the
//!   pointer position and the keyboard-focus flag, and runs the page. Pure
//!   egui; every unit test below drives this half.
//! * [`ScreenSurface`] wraps a core with an `egui_wgpu::Renderer`, a wgpu
//!   texture and its view, and turns each run into pixels. The scene's PBR
//!   material samples that texture directly (`Renderer::
//!   add_material_with_albedo_view`), so there is no readback and no copy.
//!
//! THE CONTEXT-BOUND TEXTURE RULE. An `egui::TextureHandle` belongs to the
//! `Context` that created it; using it under another context draws garbage
//! or nothing. `GuiState` holds three such handles (`image_cache`,
//! `watch_texture`, `link_device_qr`), all created by whichever context ran
//! the page that made them. A surface therefore keeps its OWN
//! [`ScreenTextures`] and swaps them into `GuiState` for the duration of its
//! page draw, restoring the main UI's on every exit path. The swap is a guard
//! struct whose `Drop` swaps back, so a panic inside a page unwinds through
//! the guard and the main UI never ends up holding the screen's handles.
//!
//! KNOWN LIMIT (v1, documented on purpose): some pages keep `thread_local`
//! page state (`pages/inventory.rs` has one, others do too). That state is
//! per THREAD, not per context, so the main UI and a screen showing the same
//! page share it: opening the inventory's garden editor on a wall screen
//! opens it in the full-screen inventory as well. Acceptable for rung 1; the
//! fix is moving that state into egui memory (`ctx.data`), page by page.

use super::dispatch::{draw_tool_page, page_from_id};
use super::theme::Theme;
use super::widgets::image_cache::ImageCache;
use super::{GuiPage, GuiState};
use crate::machines::CameraPose;
use crate::renderer::camera::Camera;
use std::time::{Duration, Instant};

/// The `GuiState` fields that hold egui `TextureHandle`s, one set per
/// surface. See the module doc for why these must be swapped, not shared.
pub struct ScreenTextures {
    pub image_cache: ImageCache,
    pub watch_texture: Option<egui::TextureHandle>,
    pub link_device_qr: Option<(String, egui::TextureHandle)>,
}

impl Default for ScreenTextures {
    fn default() -> Self {
        Self { image_cache: ImageCache::new(), watch_texture: None, link_device_qr: None }
    }
}

impl ScreenTextures {
    /// Exchange this set with the one currently in `state`. Calling it twice
    /// is the identity, which is what the guard below relies on.
    fn swap_with(&mut self, state: &mut GuiState) {
        std::mem::swap(&mut self.image_cache, &mut state.image_cache);
        std::mem::swap(&mut self.watch_texture, &mut state.watch_texture);
        std::mem::swap(&mut self.link_device_qr, &mut state.link_device_qr);
    }
}

/// The swap guard. Constructing it puts the screen's handles into
/// `GuiState`; dropping it, including during a panic unwind, puts the main
/// UI's back. Holding `&mut` to both halves for the guard's lifetime is also
/// what makes it impossible to forget: the page draw borrows `state` THROUGH
/// the guard, so it cannot run outside the swapped window.
struct TextureSwap<'a> {
    state: &'a mut GuiState,
    slot: &'a mut ScreenTextures,
}

impl<'a> TextureSwap<'a> {
    fn new(state: &'a mut GuiState, slot: &'a mut ScreenTextures) -> Self {
        slot.swap_with(state);
        Self { state, slot }
    }
}

impl Drop for TextureSwap<'_> {
    fn drop(&mut self) {
        self.slot.swap_with(self.state);
    }
}

/// What a screen shows, parsed once from the data file's `source` string.
///
/// The string carries a scheme prefix so one field names every kind of
/// content a display can carry (infinite-of-x: a new kind is a new arm here
/// and a provider in `engine::screens`, never a new field per kind):
///
/// | string                        | variant                      |
/// |-------------------------------|------------------------------|
/// | `inventory` or `page:inventory` | `Page(GuiPage::Inventory)` |
/// | `watch:<stream id>`           | `Live` (an MJPEG live stream)  |
/// | `camera:<machine instance id>`| `Camera` (an in-game camera)   |
/// | `video:<path>`                | `Video` (a WebM clip)          |
/// | `web:<url>`                   | `Web` (the readable web)       |
///
/// Anything else (a page id that does not exist, an unknown scheme) is
/// `Unknown` and draws a plain notice naming the string, never a panic: a
/// typo in home.ron must not take the world down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScreenSource {
    Page(GuiPage),
    Live(String),
    Camera(String),
    Video(String),
    Web(String),
    Unknown(String),
}

impl ScreenSource {
    /// Parse a data-file source string. A bare id with no colon is a page,
    /// because "inventory" on a wall is the common case and reads best.
    pub fn parse(s: &str) -> Self {
        let s = s.trim();
        match s.split_once(':') {
            Some(("page", id)) => match page_from_id(id.trim()) {
                Some(p) => Self::Page(p),
                None => Self::Unknown(s.to_string()),
            },
            Some(("watch", id)) if !id.trim().is_empty() => Self::Live(id.trim().to_string()),
            Some(("camera", id)) if !id.trim().is_empty() => Self::Camera(id.trim().to_string()),
            Some(("video", path)) if !path.trim().is_empty() => Self::Video(path.trim().to_string()),
            // A URL keeps its own colons ("web:https://..."), hence the
            // split at the FIRST colon only and the untrimmed remainder.
            Some(("web", url)) if !url.trim().is_empty() => Self::Web(url.trim().to_string()),
            Some(_) => Self::Unknown(s.to_string()),
            None => match page_from_id(s) {
                Some(p) => Self::Page(p),
                None => Self::Unknown(s.to_string()),
            },
        }
    }

    /// The scheme name, for the dev IPC's done file and log lines.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Page(_) => "page",
            Self::Live(_) => "watch",
            Self::Camera(_) => "camera",
            Self::Video(_) => "video",
            Self::Web(_) => "web",
            Self::Unknown(_) => "unknown",
        }
    }
}

/// Content that is not a plain page: a live stream, a camera, a clip, a web
/// page. A provider owns the per-screen state of one such source (the
/// decoder, the viewer, the browsing history) and takes over the surface's
/// per-frame draw. `engine::screens::provider_for` builds the right one for
/// a `ScreenSource`; each kind lives in its own file under
/// `src/engine/screens/`, so the rungs that add them never edit each other.
///
/// A provider draws in one of two ways, both methods on the surface it is
/// handed: `run_and_render` for egui content (a status page, the web view)
/// and `write_pixels` for finished frames (a decoded video or stream frame).
pub trait ScreenProvider: Send {
    /// Draw this frame. Called instead of the surface's default page draw,
    /// with the same ordering guarantees (outside the main egui closure,
    /// before the scene passes). `surface` is the provider's own surface
    /// with the provider temporarily taken out of it, so no double borrow.
    fn frame(
        &mut self,
        surface: &mut ScreenSurface,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &mut Theme,
        gui_state: &mut GuiState,
    );

    /// The scheme this provider serves ("watch", "video", ...).
    fn kind(&self) -> &'static str;

    /// Extra fields merged into `debug/screen_done.json` (a stream's
    /// connection state, a clip's position, a web view's url and title), so
    /// the rig can wait for and assert on provider state.
    fn status(&self) -> serde_json::Value {
        serde_json::json!({})
    }

    /// A primary button press or release landed on the screen at `uv`
    /// (0..1 each). Return true to say the provider acted on it (a click that
    /// pauses a clip); the egui core still receives the event either way.
    fn on_button(&mut self, _uv: (f32, f32), _pressed: bool) -> bool {
        false
    }

    /// The texture format this provider's surface must be created with.
    /// Default: [`SURFACE_FORMAT`]. A provider that has the game world
    /// rendered STRAIGHT INTO its surface (the camera provider) returns
    /// `scene_format`, the swapchain's format, because the scene pipelines
    /// were built for that format and can draw into no other. Asked once,
    /// when the surface is created (`engine::screens::sync_screens`).
    fn surface_format(&self, _scene_format: wgpu::TextureFormat) -> wgpu::TextureFormat {
        SURFACE_FORMAT
    }

    /// A WORLD SCREEN shows the game world itself (an in-game camera) and
    /// needs the engine's renderer and the frame's draw lists, which `frame`
    /// is not handed. Such a provider returns `Some` here, with when it last
    /// had the world rendered for it and how often it wants one, and the
    /// engine (`engine::screens::frame_surfaces`) serves at most ONE world
    /// screen per frame, the most-starved due one, through `render_world`
    /// before the usual `frame`. Default: not a world screen.
    fn world_render(&self) -> Option<WorldRenderState> {
        None
    }

    /// Render the world for this screen: the engine's `world` handle gives
    /// the pose of any placed camera and renders a view from any pose into
    /// any target. Called only for a world screen, only when it is due, and
    /// for at most one screen per frame. `surface` is this provider's own
    /// surface, its texture the natural target.
    fn render_world(&mut self, _surface: &mut ScreenSurface, _world: &mut dyn WorldRender, _now: Instant) {}

    /// How far along the provider's content is, for the dev IPC's
    /// `wait_ready` action (the rig waits for a web page to arrive before it
    /// clicks a link). A provider whose content is always there (a status
    /// page) keeps the default.
    fn load_state(&self) -> LoadState {
        LoadState::Static
    }

    /// The rectangles, in surface PIXELS, of the links the provider drew on
    /// its last frame, in drawing order. The dev IPC's `link` action clicks
    /// the Nth one through the normal event API, so a rig never has to
    /// guess pixel coordinates from a screenshot. Empty for content with no
    /// links.
    fn link_rects(&self) -> Vec<egui::Rect> {
        Vec::new()
    }
}

/// A world screen's scheduling state (see `ScreenProvider::world_render`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorldRenderState {
    /// When the world was last rendered for this screen; `None` = never.
    pub last: Option<Instant>,
    /// The least time between two renders of this screen (the camera
    /// budget: 100 ms, 10 Hz).
    pub interval: Duration,
}

impl WorldRenderState {
    /// Whether a render is due at `now`: never rendered, or `interval` has
    /// passed since the last one.
    pub fn due(&self, now: Instant) -> bool {
        self.last.map_or(true, |t| now.duration_since(t) >= self.interval)
    }
}

/// What the engine offers a world screen while serving it: implemented in
/// `engine::screens::camera` over the engine state and the frame's draw
/// lists, named here so the provider trait stays free of engine types.
pub trait WorldRender {
    /// The pose of the placed camera machine `instance_id` (its def has a
    /// `camera` and it is placed right now), or `None`. Re-resolved on every
    /// call, so a post moved or removed in the editor is seen at once.
    fn camera_pose(&self, instance_id: &str) -> Option<CameraPose>;

    /// Render the world as seen by `camera` into `target`, a render
    /// attachment of `size` pixels in the scene's own format. Returns
    /// whether a view was actually rendered: `false` means the engine
    /// skipped it (the world is not loaded) and the target is untouched, so
    /// the provider must not count it as a picture.
    fn render_view(&mut self, camera: &Camera, target: &wgpu::TextureView, size: (u32, u32)) -> bool;
}

/// Where a provider's content stands, as reported through
/// [`ScreenProvider::load_state`] for the dev IPC's `wait_ready`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadState {
    /// The content is always there (a page, a notice): nothing to wait for.
    Static,
    /// Something is on its way (a fetch in flight, a clip opening).
    Loading,
    /// The content arrived and is drawn.
    Ready,
    /// It will not arrive; the string is the one-line reason.
    Error(String),
}

/// What `ScreenCore::find_text` found on the last frame: the first drawn
/// text matching the query, in surface pixels, and how many texts matched.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundText {
    /// The full text of the matching galley (a label's whole string).
    pub text: String,
    /// The text's rectangle on the surface, in pixels, clipped to the
    /// region it was actually drawn in.
    pub rect: egui::Rect,
    /// How many drawn texts matched the query (the first one is returned).
    pub matches: usize,
}

/// Search the shapes egui produced for one frame for a drawn text. Three
/// tiers, best first: an EXACT match (the galley's whole text equals
/// `query`), then a text that STARTS WITH it, then one that merely
/// CONTAINS it; within a tier the first in drawing order wins. So asking
/// for "Home" on the inventory finds the container header "Home  (Silverdale,
/// WA ...)" before the person row "You  (Home)" that only mentions it.
/// Texts clipped entirely away (a label scrolled out of its scroll area)
/// are skipped. This is the only per-widget text lookup egui affords:
/// widget rects carry ids, not labels, so the drawn galleys are the honest
/// source.
pub fn find_text_in_shapes(shapes: &[egui::epaint::ClippedShape], query: &str) -> Option<FoundText> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    // Best candidate per tier: 0 exact, 1 prefix, 2 substring.
    let mut best: [Option<(String, egui::Rect)>; 3] = [None, None, None];
    let mut matches = 0usize;
    for cs in shapes {
        visit_text(&cs.shape, cs.clip_rect, &mut |text, rect| {
            let tier = if text == query {
                0
            } else if text.starts_with(query) {
                1
            } else if text.contains(query) {
                2
            } else {
                return;
            };
            matches += 1;
            if best[tier].is_none() {
                best[tier] = Some((text.to_string(), rect));
            }
        });
    }
    best.into_iter().flatten().next().map(|(text, rect)| FoundText { text, rect, matches })
}

/// Walk one shape (recursing into `Shape::Vec`) and call `f` with every
/// text galley's string and its on-surface rect, clipped to `clip`.
fn visit_text(shape: &egui::Shape, clip: egui::Rect, f: &mut impl FnMut(&str, egui::Rect)) {
    match shape {
        egui::Shape::Text(t) => {
            let rect = t.galley.rect.translate(t.pos.to_vec2()).intersect(clip);
            if rect.is_positive() {
                f(t.galley.text(), rect);
            }
        }
        egui::Shape::Vec(v) => {
            for s in v {
                visit_text(s, clip, f);
            }
        }
        _ => {}
    }
}

/// The GPU-free half of a screen: an egui context plus the synthetic input
/// that drives it. Everything a test needs to prove "the page on a screen
/// really reacts to a click" lives here.
pub struct ScreenCore {
    /// The placed machine instance id this screen belongs to (`wall_screen_1`).
    pub id: String,
    /// The source string from the data file, kept verbatim for messages and
    /// for the reuse check when placements rebuild.
    pub source_id: String,
    /// The parsed source. `Page` draws through `gui::dispatch`; `Unknown`
    /// draws a plain notice; the other kinds draw through their provider (or
    /// a "not wired" notice when no provider exists for them yet).
    pub source: ScreenSource,
    ctx: egui::Context,
    size: (u32, u32),
    /// Events queued since the last run, in arrival order.
    events: Vec<egui::Event>,
    /// Where the pointer is on this screen in PIXELS, `None` when it left.
    pointer: Option<egui::Pos2>,
    /// Whether this screen currently receives typed text and keys.
    focus: bool,
    /// `ctx.wants_keyboard_input()` as of the end of the last run.
    wants_keyboard: bool,
    /// After the last run: did egui report a layer (panel, window, area)
    /// under the pointer? This is the honest "something is under the
    /// pointer" signal egui exposes; per-widget hover is not public.
    hover_layer: bool,
    /// The cursor icon egui asked for after the last run (Text over a text
    /// field, PointingHand over a link), reported by the dev IPC.
    cursor_icon: egui::CursorIcon,
    /// How many runs have completed. The first frame after creation runs
    /// twice (see `ScreenSurface::frame`).
    runs: u64,
    textures: ScreenTextures,
    /// A text lookup the dev IPC asked for; answered from EVERY run's
    /// shapes until the answer is read back with `take_found_text`, so a
    /// frame that runs the content twice (the warm-up) reports the settled
    /// layout, not the first pass (see `find_text` / `take_found_text`).
    find_query: Option<String>,
    find_result: Option<Option<FoundText>>,
}

impl ScreenCore {
    /// Build a core for `source_id` at `w` x `h` pixels with the app theme and
    /// the main UI's font chains applied. An unknown page id is logged once
    /// here and becomes a blank notice screen, never a panic: a typo in
    /// home.ron must not take the world down.
    pub fn new(id: &str, source_id: &str, w: u32, h: u32, theme: &Theme) -> Self {
        let ctx = egui::Context::default();
        super::install_fonts(&ctx);
        theme.apply_to_egui(&ctx);
        let source = ScreenSource::parse(source_id);
        if let ScreenSource::Unknown(_) = source {
            log::warn!(
                "[Screens] {id}: no source named {source_id:?} (a page id from gui::dispatch::page_id, or watch:/camera:/video:/web: plus an argument); showing a notice screen"
            );
        }
        Self {
            id: id.to_string(),
            source_id: source_id.to_string(),
            source,
            ctx,
            size: (w.max(1), h.max(1)),
            events: Vec::new(),
            pointer: None,
            focus: false,
            wants_keyboard: false,
            hover_layer: false,
            cursor_icon: egui::CursorIcon::Default,
            runs: 0,
            textures: ScreenTextures::default(),
            find_query: None,
            find_result: None,
        }
    }

    /// Ask for the on-surface position of a drawn text. The answer comes
    /// from the shapes of every run from now until it is read back with
    /// `take_found_text` (a text is only where the frame put it), and the
    /// LAST run's answer is the one read: a surface's first frame runs its
    /// content twice and only the second run is settled, so a query
    /// consumed by the first run would report a layout nothing is drawn at
    /// any more. Used by the dev IPC's `find` action so a rig can click
    /// "the Home header" instead of guessing pixel coordinates; see
    /// `find_text_in_shapes` for the matching rule.
    pub fn find_text(&mut self, query: &str) {
        self.find_query = Some(query.to_string());
        self.find_result = None;
    }

    /// The answer to the last `find_text`, once a run has happened: outer
    /// `None` = no run yet, inner `None` = nothing on the surface matched.
    /// Reading the answer ends the lookup (the query is dropped with it),
    /// so later frames do not keep answering a question nobody is waiting
    /// on.
    pub fn take_found_text(&mut self) -> Option<Option<FoundText>> {
        self.find_query = None;
        self.find_result.take()
    }

    /// Pixel size of the surface.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Follow a surface resize (see `ScreenSurface::resize`): the next run
    /// lays the content out at the new size. The pointer is dropped because
    /// its pixel position meant something on the old layout.
    pub fn set_size(&mut self, w: u32, h: u32) {
        self.size = (w.max(1), h.max(1));
        self.pointer = None;
    }

    /// The screen's own context, for tests and the dev IPC (reading egui
    /// memory back after a run).
    pub fn ctx(&self) -> &egui::Context {
        &self.ctx
    }

    /// Number of completed runs.
    pub fn runs(&self) -> u64 {
        self.runs
    }

    /// How many times the content runs on the NEXT frame: twice on the very
    /// first frame after creation (the warm-up, see `ScreenSurface::frame`:
    /// egui Windows and Areas measure themselves on run 1 and settle on run
    /// 2), once on every frame after. `run_and_render` reads the count from
    /// here so a headless test can drive the same double run without a GPU.
    pub fn runs_this_frame(&self) -> usize {
        if self.runs == 0 { 2 } else { 1 }
    }

    /// Map a (u, v) in 0..1 across the screen (u left to right, v top to
    /// bottom, both as seen by someone facing the screen) to egui points.
    /// The surface renders at one point per pixel, so this is the pixel
    /// position too.
    pub fn uv_to_pos(&self, uv: (f32, f32)) -> egui::Pos2 {
        egui::pos2(uv.0 * self.size.0 as f32, uv.1 * self.size.1 as f32)
    }

    /// The pointer arrived at or moved to `uv`. A repeat of the current
    /// position queues nothing (the look ray reports every frame).
    pub fn pointer_moved(&mut self, uv: (f32, f32)) {
        let pos = self.uv_to_pos(uv);
        if self.pointer == Some(pos) {
            return;
        }
        self.pointer = Some(pos);
        self.events.push(egui::Event::PointerMoved(pos));
    }

    /// The pointer left the screen (the look ray moved off it).
    pub fn pointer_gone(&mut self) {
        if self.pointer.take().is_some() {
            self.events.push(egui::Event::PointerGone);
        }
    }

    /// Primary button press or release at `uv`. Moves the pointer there first
    /// if it is not already, so a click is always attributed to what is under
    /// it (egui hit-tests the press against the previous frame's rects at
    /// the pointer position carried in the event).
    pub fn button(&mut self, uv: (f32, f32), pressed: bool) {
        let pos = self.uv_to_pos(uv);
        if self.pointer != Some(pos) {
            self.pointer = Some(pos);
            self.events.push(egui::Event::PointerMoved(pos));
        }
        self.events.push(egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        });
    }

    /// Wheel scroll at `uv`; `lines` is in wheel notches, positive = the
    /// content moves down (revealing what is above), egui's own convention
    /// and the same sign winit's `LineDelta` carries, so lib.rs passes the
    /// winit value straight through.
    pub fn scroll(&mut self, uv: (f32, f32), lines: f32) {
        let pos = self.uv_to_pos(uv);
        if self.pointer != Some(pos) {
            self.pointer = Some(pos);
            self.events.push(egui::Event::PointerMoved(pos));
        }
        self.events.push(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, lines),
            modifiers: egui::Modifiers::default(),
        });
    }

    /// Typed text (a committed character or string) for the focused field.
    pub fn text(&mut self, s: &str) {
        if !s.is_empty() {
            self.events.push(egui::Event::Text(s.to_string()));
        }
    }

    /// A named key press or release (Enter, Backspace, arrows, letters).
    pub fn key(&mut self, key: egui::Key, pressed: bool, modifiers: egui::Modifiers) {
        self.events.push(egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers });
    }

    /// Whether a text field on this screen had keyboard focus at the end of
    /// the last run. lib.rs routes typed text here only while this is true,
    /// so pressing W to walk never types into a wall screen by accident.
    pub fn wants_keyboard(&self) -> bool {
        self.wants_keyboard
    }

    /// Whether egui reported a layer under the pointer after the last run.
    pub fn hover_widget(&self) -> bool {
        self.hover_layer
    }

    /// The cursor egui asked for after the last run.
    pub fn cursor_icon(&self) -> egui::CursorIcon {
        self.cursor_icon
    }

    /// Keyboard-focus flag, owned by the engine's `Screens` (the last screen
    /// clicked holds focus until Escape or a click elsewhere).
    pub fn set_focus(&mut self, focus: bool) {
        self.focus = focus;
    }

    pub fn has_focus(&self) -> bool {
        self.focus
    }

    /// Run the page ONCE with every event queued since the last run. Returns
    /// egui's output (shapes + texture deltas) for the GPU half to draw.
    /// Reads `wants_keyboard` and the hover state back afterwards.
    ///
    /// Ordering rule for callers: this borrows `gui_state` mutably for the
    /// whole page draw, so it must run OUTSIDE the main UI's `ctx.run`
    /// closure (which holds the same borrow), and BEFORE the scene pass that
    /// samples the surface texture, or the wall shows last frame's page.
    pub fn run(&mut self, theme: &mut Theme, gui_state: &mut GuiState) -> egui::FullOutput {
        let source = self.source.clone();
        let source_id = self.source_id.clone();
        self.run_with(gui_state, |ctx, state| match source {
            ScreenSource::Page(p) => {
                draw_tool_page(ctx, p, theme, state);
                // The main loop drains decoded chat images into egui textures
                // after every page draw; a screen showing chat needs the same,
                // against ITS cache (the swapped-in one) and ITS context.
                state.image_cache.poll(ctx);
            }
            ScreenSource::Unknown(_) => notice(ctx, &format!("No source named \"{source_id}\"")),
            // A provider normally draws these before this path is reached
            // (see `ScreenSurface::frame`); reaching it means no provider
            // exists for the kind yet, which is a wiring gap, not an error.
            other => notice(ctx, &format!("{} source \"{source_id}\" is not wired yet", other.kind())),
        })
    }

    /// The general form of [`run`](Self::run): draw with `f` under this
    /// screen's context with the texture swap in place. Public so a test can
    /// prove the guard restores the main UI's handles even when `f` panics.
    pub fn run_with(
        &mut self,
        gui_state: &mut GuiState,
        f: impl FnOnce(&egui::Context, &mut GuiState),
    ) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(self.size.0 as f32, self.size.1 as f32),
            )),
            events: std::mem::take(&mut self.events),
            focused: true,
            ..Default::default()
        };
        // The guard holds the swapped-in state for exactly the span of the
        // run below. `self.textures` (borrowed by the guard) and `self.ctx`
        // (borrowed by the run) are disjoint fields, so both borrows coexist.
        // `ctx.run` wants an FnMut and `f` is FnOnce, hence the Option take.
        let mut guard = TextureSwap::new(gui_state, &mut self.textures);
        let mut f = Some(f);
        let out = self.ctx.run(input, |ctx| {
            if let Some(f) = f.take() {
                f(ctx, &mut *guard.state);
            }
        });
        // Explicit, so the restore point is visible: the main UI's handles
        // are back in `gui_state` from here on (and on a panic above, the
        // unwind ran this same Drop).
        drop(guard);
        self.wants_keyboard = self.ctx.wants_keyboard_input();
        self.hover_layer = self.pointer.map_or(false, |p| self.ctx.layer_id_at(p).is_some());
        self.cursor_icon = out.platform_output.cursor_icon;
        // A pending text lookup is answered from THIS run's shapes, before
        // the GPU half tessellates them away, and answered again by every
        // later run until it is read back, so the LAST run of a frame wins
        // (the warm-up double run on frame 1 then reports the settled
        // layout). Costs nothing when no lookup is pending (the normal
        // case: it is a dev-IPC verb).
        if let Some(q) = &self.find_query {
            self.find_result = Some(find_text_in_shapes(&out.shapes, q));
        }
        self.runs += 1;
        out
    }
}

/// A one-line centred notice filling the screen: the "no such source" and
/// "not wired yet" fallbacks, and any provider's status line. Plain text on
/// the theme's panel so it reads from across the room.
pub fn notice(ctx: &egui::Context, text: &str) {
    egui::CentralPanel::default().show(ctx, |ui| {
        ui.centered_and_justified(|ui| {
            ui.label(text);
        });
    });
}

/// sRGB byte to linear float, for the clear colour of an sRGB render target
/// (wgpu encodes the clear value, so it must be given in linear light).
fn srgb_to_lin(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// The GPU half: a core plus the renderer and texture that turn its runs
/// into pixels the scene samples.
pub struct ScreenSurface {
    pub core: ScreenCore,
    renderer: egui_wgpu::Renderer,
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// The texture's format: [`SURFACE_FORMAT`] for every page and every
    /// provider that draws pixels or egui content; the scene's swapchain
    /// format for a surface the world is rendered straight into (see
    /// `ScreenProvider::surface_format`). The readback and the pixel upload
    /// swizzle for a BGRA format so their byte contract stays RGBA.
    format: wgpu::TextureFormat,
    /// The content provider for a non-page source, built by
    /// `engine::screens::provider_for`. `None` = the default page draw (or
    /// the not-wired notice for a kind with no provider yet).
    provider: Option<Box<dyn ScreenProvider>>,
    /// Set by `resize` (a provider matched the texture to its frame size)
    /// and cleared by `take_view_changed`: the scene material binds the OLD
    /// view until `engine::screens::frame_surfaces` rebinds it.
    view_changed: bool,
}

/// The texture format of every surface that draws pixels or egui content.
/// It is the SAME format `renderer/materials.rs` uses for albedo textures
/// (`Rgba8UnormSrgb`), which is what makes binding the surface's view at the
/// material's albedo slot correct without a conversion: the PBR sampler
/// decodes sRGB to linear on read exactly as it does for any other albedo
/// image. A world screen uses the scene's format instead (`new_with_format`);
/// the sampler handles both the same way.
pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

/// Whether `format` stores its channels blue-first, so RGBA bytes must be
/// swizzled on the way in and out of the texture.
fn is_bgra(format: wgpu::TextureFormat) -> bool {
    matches!(format, wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb)
}

/// Swap the red and blue byte of every pixel in place (RGBA <-> BGRA).
fn swap_rb(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
}

impl ScreenSurface {
    /// Create the surface and its texture at a fixed pixel size in
    /// [`SURFACE_FORMAT`]. The texture is a render target (egui draws into
    /// it), a sampled texture (the scene reads it), and copyable both ways
    /// (the dev IPC snapshot reads it back; a provider writes frames into it).
    pub fn new(device: &wgpu::Device, id: &str, source_id: &str, w: u32, h: u32, theme: &Theme) -> Self {
        Self::new_with_format(device, id, source_id, w, h, theme, SURFACE_FORMAT)
    }

    /// [`new`](Self::new) with an explicit texture format: the scene's
    /// swapchain format for a surface the world is rendered straight into
    /// (the camera provider). The surface's own egui renderer is created for
    /// the same format, so a notice page draws into it correctly too.
    pub fn new_with_format(
        device: &wgpu::Device,
        id: &str,
        source_id: &str,
        w: u32,
        h: u32,
        theme: &Theme,
        format: wgpu::TextureFormat,
    ) -> Self {
        let core = ScreenCore::new(id, source_id, w, h, theme);
        let (w, h) = core.size();
        let renderer = egui_wgpu::Renderer::new(device, format, None, 1, false);
        let (texture, view) = Self::make_texture(device, w, h, format);
        Self { core, renderer, texture, view, format, provider: None, view_changed: false }
    }

    /// The texture every surface draws into: a render target (egui draws
    /// into it, the world is rendered into it), a sampled texture (the scene
    /// reads it), and copyable both ways (the dev IPC snapshot reads it back;
    /// a provider writes frames into it).
    fn make_texture(device: &wgpu::Device, w: u32, h: u32, format: wgpu::TextureFormat) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Surface Texture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// The view the scene material binds at its albedo slot.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// The texture's format (see the `format` field).
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn size(&self) -> (u32, u32) {
        self.core.size()
    }

    /// Install (or remove) the content provider. Called once when the
    /// surface is created for a non-page source.
    pub fn set_provider(&mut self, provider: Option<Box<dyn ScreenProvider>>) {
        self.provider = provider;
    }

    pub fn provider(&self) -> Option<&dyn ScreenProvider> {
        self.provider.as_deref()
    }

    /// The provider's world-screen scheduling state, `None` for a page or a
    /// provider that draws its own content (see `ScreenProvider::world_render`).
    pub fn world_render(&self) -> Option<WorldRenderState> {
        self.provider.as_ref().and_then(|p| p.world_render())
    }

    /// Have the world rendered for this screen's provider (a world screen
    /// only; the engine calls this for at most one screen per frame). The
    /// provider is taken out for the call, like in `frame`, so it can be
    /// handed the surface it lives in.
    pub fn render_world(&mut self, world: &mut dyn WorldRender, now: Instant) {
        if let Some(mut provider) = self.provider.take() {
            provider.render_world(self, world, now);
            self.provider = Some(provider);
        }
    }

    /// True once after `resize` changed the texture: the caller rebinds the
    /// scene material to the new `view()`.
    pub fn take_view_changed(&mut self) -> bool {
        std::mem::replace(&mut self.view_changed, false)
    }

    /// A primary button event on this screen: the egui core gets it (so a
    /// page reacts) and the provider is told (so a clip can pause). One
    /// entry point, so the look ray and the dev IPC cannot disagree.
    pub fn button(&mut self, uv: (f32, f32), pressed: bool) {
        self.core.button(uv, pressed);
        if let Some(p) = self.provider.as_mut() {
            p.on_button(uv, pressed);
        }
    }

    /// Reallocate the texture at a new pixel size (a provider matching the
    /// surface to its frame). The egui renderer is format-bound, not
    /// size-bound, so it carries over; the scene material still points at
    /// the old view until the caller sees `take_view_changed`.
    pub fn resize(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        let (w, h) = (w.max(1), h.max(1));
        if self.core.size() == (w, h) {
            return;
        }
        let (texture, view) = Self::make_texture(device, w, h, self.format);
        self.texture = texture;
        self.view = view;
        self.core.set_size(w, h);
        self.view_changed = true;
    }

    /// Write a finished RGBA8 frame (sRGB bytes, top row first) into the
    /// surface. A frame of another size resizes the surface to match, so a
    /// 1920 x 1080 stream on a 1280 x 720 wall shows every pixel and the
    /// quad (whose physical size comes from the machine def) stretches it
    /// by the aspect difference; providers that care scale before writing
    /// (the live provider letterboxes into the surface's own size, see
    /// `engine::screens::live::letterbox_into`). The bytes are RGBA whatever
    /// the texture's format; a BGRA surface swizzles on the way in.
    pub fn write_pixels(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], w: u32, h: u32) {
        if w == 0 || h == 0 || rgba.len() != (w as usize) * (h as usize) * 4 {
            log::warn!("[Screens] {}: write_pixels got {} bytes for {w}x{h}; frame dropped", self.core.id, rgba.len());
            return;
        }
        self.resize(device, w, h);
        let swizzled;
        let bytes: &[u8] = if is_bgra(self.format) {
            let mut v = rgba.to_vec();
            swap_rb(&mut v);
            swizzled = v;
            &swizzled
        } else {
            rgba
        };
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bytes,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * w), rows_per_image: Some(h) },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
    }

    /// Draw this frame into the texture: the provider's content when the
    /// source has one, else the source's page (or its notice).
    ///
    /// Ordering: see `ScreenCore::run`. Call after the main egui closure has
    /// returned and before the scene passes.
    pub fn frame(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, theme: &mut Theme, gui_state: &mut GuiState) {
        // The provider is taken out for the call so it can be handed the
        // surface it lives in (a provider inside `self` cannot also borrow
        // `self`), then put back whatever it did.
        if let Some(mut provider) = self.provider.take() {
            provider.frame(self, device, queue, theme, gui_state);
            self.provider = Some(provider);
            return;
        }
        self.run_and_render(device, queue, theme, gui_state, |core, theme, state| core.run(theme, state));
    }

    /// Run egui content through the core with the pending events and draw
    /// the result into the texture. `draw` is handed the core so it can call
    /// `ScreenCore::run` (the source's page) or `ScreenCore::run_with` (any
    /// other egui content, the way a provider draws a status page or the web
    /// view) and must return that run's output.
    ///
    /// The FIRST frame after creation runs the content twice: egui Windows
    /// and Areas measure themselves on frame 1 and only settle their position
    /// on frame 2, so a single run leaves Window-based pages blank (the same
    /// warm-up `ui_snapshots::render_page_png` does). Both runs' texture
    /// deltas are applied because the font atlas is created on run 1.
    ///
    /// The theme is re-applied every frame. It is two small style writes,
    /// and it means a theme edit in Settings reaches every wall screen the
    /// same frame it reaches the main UI, with no change-tracking to get
    /// stale.
    pub fn run_and_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        theme: &mut Theme,
        gui_state: &mut GuiState,
        mut draw: impl FnMut(&mut ScreenCore, &mut Theme, &mut GuiState) -> egui::FullOutput,
    ) {
        theme.apply_to_egui(&self.core.ctx);
        // Two runs on the first frame, one after (`ScreenCore::runs_this_frame`
        // owns the rule so the headless twin drives the same count). A
        // pending `find` is answered by EVERY run and the last answer wins,
        // so on the warm-up frame the reported rect is the settled one.
        let runs = self.core.runs_this_frame();
        let mut last: Option<egui::FullOutput> = None;
        for _ in 0..runs {
            let out = draw(&mut self.core, theme, gui_state);
            for (id, delta) in &out.textures_delta.set {
                self.renderer.update_texture(device, queue, *id, delta);
            }
            if let Some(prev) = last.replace(out) {
                for id in &prev.textures_delta.free {
                    self.renderer.free_texture(id);
                }
            }
        }
        let Some(out) = last else { return };
        let (w, h) = self.core.size();
        let ppp = 1.0_f32;
        let clipped = self.core.ctx.tessellate(out.shapes, ppp);
        let screen = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: ppp };
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Screen Surface Encoder"),
        });
        let user_cmds = self.renderer.update_buffers(device, queue, &mut encoder, &clipped, &screen);
        let bg = theme.bg_primary();
        {
            let mut rpass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Screen Surface Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: srgb_to_lin(bg.r()),
                                g: srgb_to_lin(bg.g()),
                                b: srgb_to_lin(bg.b()),
                                a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();
            self.renderer.render(&mut rpass, &clipped, &screen);
        }
        queue.submit(user_cmds.into_iter().chain(std::iter::once(encoder.finish())));
        for id in &out.textures_delta.free {
            self.renderer.free_texture(id);
        }
    }

    /// Read the surface back as tightly packed RGBA8 rows (top row first),
    /// for the dev IPC snapshot. Blocks on the GPU; dev tooling only. A
    /// BGRA surface (a world screen in the scene's format) is swizzled so
    /// the caller always gets RGBA.
    pub fn read_rgba(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        let mut pixels = self.read_raw(device, queue);
        if is_bgra(self.format) {
            swap_rb(&mut pixels);
        }
        pixels
    }

    /// The readback in the texture's own byte order.
    fn read_raw(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
        let (w, h) = self.core.size();
        // wgpu requires copy rows padded to 256 bytes.
        let bpr = ((w * 4 + 255) / 256) * 256;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Screen Surface Readback"),
            size: (bpr * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::Maintain::Wait);
        let data = slice.get_mapped_range();
        let mut pixels = Vec::with_capacity((w * h * 4) as usize);
        for row in 0..h {
            let start = (row * bpr) as usize;
            pixels.extend_from_slice(&data[start..start + (w * 4) as usize]);
        }
        drop(data);
        buffer.unmap();
        pixels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gui::theme::load_theme;

    fn inventory_state() -> GuiState {
        let mut s = GuiState::default();
        let data = std::path::Path::new("data");
        s.places = crate::gui::load_places(data);
        s.placed_items = crate::gui::flatten_placed_items(&s.places);
        s.homestead_design = crate::gui::load_homestead_design(data);
        s
    }

    /// (c) THE SURFACE EVENT API DRIVES A REAL PAGE. The same "shows != works"
    /// check `ui_snapshots::inventory_container_header_click_toggles_open`
    /// makes for the main UI, but every event goes through
    /// `ScreenCore::pointer_moved` / `button` with a UV-to-pixel mapping, the
    /// way the look ray feeds a wall screen. The Home container header must
    /// toggle its open state. Proven able to fail: with the v axis flipped
    /// (`1.0 - v`) the click lands on nothing and the assertion fires.
    #[test]
    fn screen_click_through_uv_toggles_the_inventory_home_header() {
        let mut theme = load_theme();
        let mut state = inventory_state();
        let (w, h) = (1280u32, 1700u32);
        let mut core = ScreenCore::new("test_screen", "inventory", w, h, &theme);
        assert_eq!(core.source, ScreenSource::Page(GuiPage::Inventory));

        crate::gui::pages::inventory::test_clear_recorded_rects();
        crate::gui::pages::inventory::test_close_garden_edit();
        crate::gui::pages::inventory::test_close_mining_edit();
        crate::gui::pages::inventory::test_clear_placed();
        // Settle twice so the places section expands and records its rects.
        core.run(&mut theme, &mut state);
        core.run(&mut theme, &mut state);

        let rect = crate::gui::pages::inventory::test_recorded_header_rect("1")
            .expect("Home container header rect should be recorded on the screen context");
        // Left edge of the header (the full-row target can run wider than
        // the screen, so its centre may be off it), expressed as UV.
        let px = egui::pos2(rect.left() + 30.0, rect.center().y);
        let uv = (px.x / w as f32, px.y / h as f32);
        assert!(uv.0 >= 0.0 && uv.0 <= 1.0 && uv.1 >= 0.0 && uv.1 <= 1.0, "header lies on the screen");

        let open_id = egui::Id::new(("place_open", "1"));
        let before = core.ctx().data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);
        core.pointer_moved(uv);
        core.run(&mut theme, &mut state);
        core.button(uv, true);
        core.run(&mut theme, &mut state);
        core.button(uv, false);
        core.run(&mut theme, &mut state);
        let after = core.ctx().data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);
        crate::gui::pages::inventory::test_clear_recorded_rects();
        assert!(
            crate::gui::pages::inventory::test_header_was_clicked("1"),
            "the synthetic screen click was not attributed to the Home header"
        );
        assert_ne!(before, after, "clicking the Home header through the screen's UV event API did not toggle it");
        assert!(core.hover_widget(), "egui should report a layer under the pointer after the click");
    }

    /// THE DEV IPC's `find` VERB, END TO END ON A REAL PAGE: ask the core
    /// where the text "Home" is drawn, click THERE through the same UV event
    /// API, and the Home container header must toggle. This is what lets the
    /// runtime rig click a named widget instead of a guessed pixel. Proven
    /// able to fail (2026-09-16): with the found rect shifted one row down
    /// (`+ 40.0` on y) the click lands on the container's body and the
    /// toggle assertion fires with `left: true, right: true`.
    #[test]
    fn find_text_locates_the_home_header_and_a_click_there_toggles_it() {
        let mut theme = load_theme();
        let mut state = inventory_state();
        let (w, h) = (1280u32, 1700u32);
        let mut core = ScreenCore::new("test_screen", "inventory", w, h, &theme);
        crate::gui::pages::inventory::test_clear_recorded_rects();
        crate::gui::pages::inventory::test_close_garden_edit();
        crate::gui::pages::inventory::test_close_mining_edit();
        crate::gui::pages::inventory::test_clear_placed();
        core.run(&mut theme, &mut state);
        core.run(&mut theme, &mut state);

        assert!(core.take_found_text().is_none(), "no lookup was asked for yet");
        core.find_text("Home");
        core.run(&mut theme, &mut state);
        let found = core
            .take_found_text()
            .expect("a run happened after the lookup")
            .expect("the inventory page draws a container named Home");
        // The header reads "Home  (Silverdale, WA ...)": the prefix tier
        // must beat "You  (Home)", which is drawn first and only contains
        // the word.
        assert!(found.text.starts_with("Home"), "prefix match must win over a substring match: {:?}", found.text);
        assert!(found.matches >= 2, "both the Home header and the You row mention Home: {}", found.matches);
        // The found text is the header's label; the header's own recorded
        // click rect must contain it (same row), which ties the shape scan
        // to the widget the page actually made clickable.
        let header = crate::gui::pages::inventory::test_recorded_header_rect("1").expect("Home header rect");
        assert!(
            header.contains(found.rect.center()),
            "found text {:?} is not inside the Home header row {header:?}",
            found.rect
        );

        let uv = (found.rect.center().x / w as f32, found.rect.center().y / h as f32);
        let open_id = egui::Id::new(("place_open", "1"));
        let before = core.ctx().data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);
        assert!(before, "Home starts open (a depth-0 container defaults to open)");
        // (c) The child row the screens rig watches (`INVENTORY_CHILD` in
        // scripts/verify-screens.js): "Garage" is Home's one room in
        // data/places/seed.json, nested as its own card inside Home's open
        // body and drawn nowhere else on the page. Present while Home is
        // open, with the pointer already resting on the header (the rig's
        // own order: hover, then find the child, then click)...
        core.find_text("Garage");
        core.pointer_moved(uv);
        core.run(&mut theme, &mut state);
        let garage = core.take_found_text().flatten().expect("Garage is drawn while Home is open");
        assert_eq!(garage.text, "Garage", "the room's own header, an exact match: {:?}", garage.text);
        core.button(uv, true);
        core.run(&mut theme, &mut state);
        core.button(uv, false);
        core.run(&mut theme, &mut state);
        let after = core.ctx().data(|d| d.get_temp::<bool>(open_id)).unwrap_or(true);
        crate::gui::pages::inventory::test_clear_recorded_rects();
        assert_ne!(before, after, "clicking at the FOUND text did not toggle the Home header");
        // (b) After the release frame the pointer still rests on the header
        // row, whose hover sets PointingHand (inventory.rs, `row.hovered()`).
        // The rig requires exactly this of the click's done file, as proof
        // the point landed on the row and not merely on the panel (any
        // point over the CentralPanel reports hover_widget).
        assert!(core.hover_widget(), "egui reports a layer under the pointer after the click");
        assert_eq!(core.cursor_icon(), egui::CursorIcon::PointingHand, "the header row's own cursor after the click");
        // ...and gone once Home is closed: the semantic proof of the toggle
        // that the rig's `inventory_toggled` check rests on. A deleted
        // toggle leaves Garage drawn and fails here.
        core.find_text("Garage");
        core.run(&mut theme, &mut state);
        assert!(core.take_found_text().flatten().is_none(), "Garage is still drawn after Home was closed");
        // The lookup is one-shot: nothing is pending after it was answered.
        core.run(&mut theme, &mut state);
        assert!(core.take_found_text().is_none(), "a find is answered once, not on every later frame");
    }

    /// THE RIG's PRECONDITION: on a wall-sized inventory (1280 x 720, the
    /// `wall_screen` def's px) the Home header must be drawn ON the surface
    /// without scrolling, or `scripts/verify-screens.js` has nothing to
    /// click. If the inventory layout ever pushes the places tree below a
    /// 720 px fold, this fails first and says why, instead of the rig
    /// failing with "found: false" at 2am.
    #[test]
    fn home_header_is_on_a_wall_sized_inventory_without_scrolling() {
        let mut theme = load_theme();
        let mut state = inventory_state();
        let (w, h) = (1280u32, 720u32);
        let mut core = ScreenCore::new("wall_screen_1", "inventory", w, h, &theme);
        crate::gui::pages::inventory::test_clear_recorded_rects();
        crate::gui::pages::inventory::test_close_garden_edit();
        crate::gui::pages::inventory::test_close_mining_edit();
        crate::gui::pages::inventory::test_clear_placed();
        core.run(&mut theme, &mut state);
        core.run(&mut theme, &mut state);
        core.find_text("Home");
        core.run(&mut theme, &mut state);
        let found = core.take_found_text().flatten().expect("Home is drawn on a 1280 x 720 inventory");
        assert!(found.text.starts_with("Home"), "{:?}", found.text);
        let c = found.rect.center();
        assert!(
            c.x > 0.0 && c.x < w as f32 && c.y > 0.0 && c.y < h as f32,
            "the Home header centre {c:?} must lie on the {w} x {h} wall"
        );
        crate::gui::pages::inventory::test_clear_recorded_rects();
    }

    /// A `find` asked BEFORE a surface's first frame must answer from the
    /// settled layout, not the warm-up. The first frame runs the content
    /// twice (`runs_this_frame` is 2 on a fresh core, the count
    /// `run_and_render` uses) because some egui layouts place things by
    /// what they measured LAST frame and so sit in the wrong place on run
    /// 1. The content here is the plainest such case: a two-column
    /// `egui::Grid` places column 2 by the previous frame's column-1 width,
    /// which on frame 1 is the 40 pt minimum no matter how wide the real
    /// label is (egui 0.31 `GridLayout::advance`, `prev_col_width`), so the
    /// second-column label is drawn ~40 pt in on run 1 and ~300 pt in on run
    /// 2. The answer after the double run must be where a later, fully
    /// settled frame draws the label. Proven red before the fix
    /// (2026-09-17): with the query consumed by the first run, the double
    /// run answered `found: false` for the second-column label (it is not
    /// drawn on a Grid's first frame at all) while the settled frame found
    /// it, and the `expect` below fired.
    #[test]
    fn a_find_asked_before_the_first_frame_answers_from_the_settled_run() {
        let theme = load_theme();
        let mut state = GuiState::default();
        let (w, h) = (640u32, 360u32);
        let mut core = ScreenCore::new("probe", "inventory", w, h, &theme);
        // A capture-free closure is Copy, so the same draw can run again.
        let draw = |ctx: &egui::Context, _: &mut GuiState| {
            egui::CentralPanel::default().show(ctx, |ui| {
                egui::Grid::new("settle_probe").show(ui, |ui| {
                    ui.label("A first-column label a good deal wider than forty points");
                    ui.label("Settled label");
                    ui.end_row();
                });
            });
        };
        assert_eq!(core.runs_this_frame(), 2, "a fresh surface runs its content twice on frame 1");
        // The rig's own order: the lookup is queued, THEN the frame runs.
        core.find_text("Settled label");
        for _ in 0..core.runs_this_frame() {
            core.run_with(&mut state, draw);
        }
        assert_eq!(core.runs_this_frame(), 1, "after frame 1 every frame is one run");
        let warm = core
            .take_found_text()
            .expect("the double run answered the lookup")
            .expect("the label is drawn after the warm-up frame");
        // A fully settled frame, asked the same question, is the reference.
        core.run_with(&mut state, draw);
        core.find_text("Settled label");
        core.run_with(&mut state, draw);
        let settled = core.take_found_text().flatten().expect("the label is drawn on a settled frame");
        assert_eq!(warm.text, settled.text, "the warm-up frame's answer names a different text");
        // The grid really did move the label between run 1 and run 2, or
        // this test would prove nothing: the settled x must be well past the
        // 40 pt first-frame column.
        assert!(settled.rect.min.x > 100.0, "the second column settled at x = {}", settled.rect.min.x);
        assert!(
            (warm.rect.min - settled.rect.min).length() < 0.5,
            "the warm-up frame's answer {:?} is not where the settled layout draws it {:?}",
            warm.rect,
            settled.rect
        );
    }

    /// The matching rule on synthetic shapes: exact beats prefix beats
    /// substring, the count covers all three, clipped-away text is
    /// invisible, and nested `Shape::Vec` is walked.
    #[test]
    fn find_text_prefers_exact_matches_and_skips_clipped_text() {
        let ctx = egui::Context::default();
        let mk = |text: &str, pos: egui::Pos2| {
            let galley = ctx.fonts(|f| f.layout_no_wrap(text.to_string(), egui::FontId::proportional(14.0), egui::Color32::WHITE));
            egui::Shape::Text(egui::epaint::TextShape::new(pos, galley, egui::Color32::WHITE))
        };
        // Fonts need one frame to exist before galleys can be laid out.
        let _ = ctx.run(Default::default(), |_| {});
        let big = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1000.0, 1000.0));
        let shapes = vec![
            egui::epaint::ClippedShape { clip_rect: big, shape: mk("Take me Home tonight", egui::pos2(10.0, 10.0)) },
            egui::epaint::ClippedShape {
                clip_rect: big,
                shape: egui::Shape::Vec(vec![mk("Home", egui::pos2(10.0, 100.0)), mk("Homestead", egui::pos2(10.0, 200.0))]),
            },
            // Drawn entirely outside its clip rect: not on the surface.
            egui::epaint::ClippedShape {
                clip_rect: egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(5.0, 5.0)),
                shape: mk("Home", egui::pos2(500.0, 500.0)),
            },
        ];
        let found = find_text_in_shapes(&shapes, "Home").expect("found");
        assert_eq!(found.text, "Home", "the exact match, not the first substring match");
        assert!((found.rect.min.y - 100.0).abs() < 1.0, "the exact match's own position: {:?}", found.rect);
        assert_eq!(found.matches, 3, "one substring + one prefix + one exact; the clipped one does not count");
        // Prefix beats substring: "Homes" is the start of "Homestead" and
        // appears in nothing else.
        let prefix = find_text_in_shapes(&shapes, "Homes").expect("prefix match");
        assert_eq!(prefix.text, "Homestead");
        // Substring only: drawn first, but "me Home" starts no text, so it
        // is found through the last tier.
        let partial = find_text_in_shapes(&shapes, "me Home").expect("substring match");
        assert_eq!(partial.text, "Take me Home tonight");
        assert!(find_text_in_shapes(&shapes, "nowhere").is_none());
        assert!(find_text_in_shapes(&shapes, "   ").is_none(), "an empty query matches nothing");
    }

    /// An unknown page id in data must warn and draw a notice, never panic.
    #[test]
    fn unknown_page_id_falls_back_to_a_blank_notice() {
        let mut theme = load_theme();
        let mut state = GuiState::default();
        let mut core = ScreenCore::new("s", "no_such_page", 320, 200, &theme);
        assert!(matches!(core.source, ScreenSource::Unknown(_)), "{:?}", core.source);
        let out = core.run(&mut theme, &mut state);
        assert!(!out.shapes.is_empty(), "the notice screen draws something");
    }

    /// Every source scheme parses to its variant, a bare id is a page, and
    /// both a missing page and an unknown scheme are `Unknown` (never a
    /// panic, never a silent fallback to some other page).
    #[test]
    fn source_strings_parse_by_scheme() {
        assert_eq!(ScreenSource::parse("inventory"), ScreenSource::Page(GuiPage::Inventory));
        assert_eq!(ScreenSource::parse("page:tasks"), ScreenSource::Page(GuiPage::Tasks));
        assert_eq!(ScreenSource::parse(" page: chat "), ScreenSource::Page(GuiPage::Chat));
        assert_eq!(ScreenSource::parse("watch:home-cam"), ScreenSource::Live("home-cam".into()));
        assert_eq!(ScreenSource::parse("camera:camera_post_1"), ScreenSource::Camera("camera_post_1".into()));
        assert_eq!(ScreenSource::parse("video:media/demo.webm"), ScreenSource::Video("media/demo.webm".into()));
        // A URL keeps every colon after the first.
        assert_eq!(
            ScreenSource::parse("web:https://united-humanity.us/library#slug"),
            ScreenSource::Web("https://united-humanity.us/library#slug".into())
        );
        assert!(matches!(ScreenSource::parse("page:no_such_page"), ScreenSource::Unknown(_)));
        assert!(matches!(ScreenSource::parse("no_such_page"), ScreenSource::Unknown(_)));
        assert!(matches!(ScreenSource::parse("hologram:x"), ScreenSource::Unknown(_)));
        assert!(matches!(ScreenSource::parse("watch:"), ScreenSource::Unknown(_)), "an empty argument is not a source");
        assert_eq!(ScreenSource::parse("video:x").kind(), "video");
    }

    /// A non-page source with no provider installed draws the not-wired
    /// notice rather than a blank or a panic (the state every phase-2 rung
    /// starts from).
    #[test]
    fn unwired_source_draws_a_notice() {
        let mut theme = load_theme();
        let mut state = GuiState::default();
        let mut core = ScreenCore::new("s", "video:media/nothing.webm", 320, 200, &theme);
        assert_eq!(core.source, ScreenSource::Video("media/nothing.webm".into()));
        let out = core.run(&mut theme, &mut state);
        assert!(!out.shapes.is_empty(), "the not-wired notice draws something");
    }

    /// (d) THE WATCH PAGE UNDER A SECOND CONTEXT, with the texture swap in
    /// place. `watch_texture` is created by the MAIN context; the screen must
    /// draw the page without panicking and hand the main handle back intact.
    #[test]
    fn watch_page_draws_under_a_screen_context_and_the_swap_restores_handles() {
        let main_ctx = egui::Context::default();
        let mut theme = load_theme();
        let mut state = GuiState::default();
        // A handle bound to the main context, like a decoded stream frame.
        let img = egui::ColorImage::new([4, 4], egui::Color32::WHITE);
        let main_handle = main_ctx.load_texture("watch_frame", img, egui::TextureOptions::LINEAR);
        let main_id = main_handle.id();
        state.watch_texture = Some(main_handle);
        state.link_device_qr = None;

        let mut core = ScreenCore::new("s", "watch", 640, 360, &theme);
        assert_eq!(core.source, ScreenSource::Page(GuiPage::Watch));
        for _ in 0..3 {
            core.run(&mut theme, &mut state);
        }
        // Whatever the watch page did on the screen, it did to the SCREEN's
        // slot; the main UI's handle is back in GuiState untouched.
        let restored = state.watch_texture.as_ref().map(|t| t.id());
        assert_eq!(restored, Some(main_id), "the swap guard must restore the main context's watch texture");
    }

    /// The guard restores on the PANIC path too. A page that panics under a
    /// screen must not leave the main UI holding the screen's handles.
    #[test]
    fn texture_swap_restores_when_the_page_panics() {
        let main_ctx = egui::Context::default();
        let theme = load_theme();
        let mut state = GuiState::default();
        let img = egui::ColorImage::new([2, 2], egui::Color32::RED);
        let main_handle = main_ctx.load_texture("qr", img, egui::TextureOptions::LINEAR);
        let main_id = main_handle.id();
        state.link_device_qr = Some(("payload".into(), main_handle));

        let mut core = ScreenCore::new("s", "inventory", 64, 64, &theme);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            core.run_with(&mut state, |_ctx, st| {
                // Inside the swap: the main handle is NOT here.
                assert!(st.link_device_qr.is_none(), "the screen's own (empty) slot is swapped in");
                panic!("page blew up");
            });
        }));
        assert!(result.is_err(), "the panic propagates");
        let back = state.link_device_qr.as_ref().map(|(_, t)| t.id());
        assert_eq!(back, Some(main_id), "after the panic the main handle must be back in GuiState");
    }

    /// Keyboard routing evidence: a text field on a screen reports
    /// wants_keyboard only once it has been clicked, so lib.rs's "route keys
    /// while the screen wants them" gate is anchored to real focus.
    #[test]
    fn wants_keyboard_follows_text_field_focus() {
        let mut state = GuiState::default();
        let theme = load_theme();
        let mut core = ScreenCore::new("s", "inventory", 300, 120, &theme);
        let mut text = String::new();
        let draw = |ctx: &egui::Context, _st: &mut GuiState, text: &mut String| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.put(
                    egui::Rect::from_min_size(egui::pos2(20.0, 20.0), egui::vec2(200.0, 24.0)),
                    egui::TextEdit::singleline(text),
                );
            });
        };
        core.run_with(&mut state, |c, s| draw(c, s, &mut text));
        assert!(!core.wants_keyboard(), "nothing focused yet");
        let uv = (100.0 / 300.0, 32.0 / 120.0);
        core.pointer_moved(uv);
        core.run_with(&mut state, |c, s| draw(c, s, &mut text));
        core.button(uv, true);
        core.run_with(&mut state, |c, s| draw(c, s, &mut text));
        core.button(uv, false);
        core.run_with(&mut state, |c, s| draw(c, s, &mut text));
        assert!(core.wants_keyboard(), "clicking the text field focuses it");
        core.text("hi");
        core.run_with(&mut state, |c, s| draw(c, s, &mut text));
        assert_eq!(text, "hi", "typed text reaches the focused field");
    }
}
