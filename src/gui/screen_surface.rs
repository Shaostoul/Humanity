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
        }
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
    /// The content provider for a non-page source, built by
    /// `engine::screens::provider_for`. `None` = the default page draw (or
    /// the not-wired notice for a kind with no provider yet).
    provider: Option<Box<dyn ScreenProvider>>,
    /// Set by `resize` (a provider matched the texture to its frame size)
    /// and cleared by `take_view_changed`: the scene material binds the OLD
    /// view until `engine::screens::frame_surfaces` rebinds it.
    view_changed: bool,
}

/// The texture format of every surface. It is the SAME format
/// `renderer/materials.rs` uses for albedo textures (`Rgba8UnormSrgb`), which
/// is what makes binding the surface's view at the material's albedo slot
/// correct without a conversion: the PBR sampler decodes sRGB to linear on
/// read exactly as it does for any other albedo image.
pub const SURFACE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

impl ScreenSurface {
    /// Create the surface and its texture at a fixed pixel size. The texture
    /// is a render target (egui draws into it), a sampled texture (the scene
    /// reads it), and copyable both ways (the dev IPC snapshot reads it back;
    /// a future live-feed rung writes into it).
    pub fn new(device: &wgpu::Device, id: &str, source_id: &str, w: u32, h: u32, theme: &Theme) -> Self {
        let core = ScreenCore::new(id, source_id, w, h, theme);
        let (w, h) = core.size();
        let renderer = egui_wgpu::Renderer::new(device, SURFACE_FORMAT, None, 1, false);
        let (texture, view) = Self::make_texture(device, w, h);
        Self { core, renderer, texture, view, provider: None, view_changed: false }
    }

    /// The texture every surface draws into: a render target (egui draws
    /// into it), a sampled texture (the scene reads it), and copyable both
    /// ways (the dev IPC snapshot reads it back; a provider writes frames
    /// into it).
    fn make_texture(device: &wgpu::Device, w: u32, h: u32) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Screen Surface Texture"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: SURFACE_FORMAT,
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
        let (texture, view) = Self::make_texture(device, w, h);
        self.texture = texture;
        self.view = view;
        self.core.set_size(w, h);
        self.view_changed = true;
    }

    /// Write a finished RGBA8 frame (sRGB bytes, top row first) into the
    /// surface. A frame of another size resizes the surface to match, so a
    /// 1920 x 1080 stream on a 1280 x 720 wall shows every pixel and the
    /// quad (whose physical size comes from the machine def) stretches it
    /// by the aspect difference; providers that care scale before writing.
    pub fn write_pixels(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], w: u32, h: u32) {
        if w == 0 || h == 0 || rgba.len() != (w as usize) * (h as usize) * 4 {
            log::warn!("[Screens] {}: write_pixels got {} bytes for {w}x{h}; frame dropped", self.core.id, rgba.len());
            return;
        }
        self.resize(device, w, h);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
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
        let runs = if self.core.runs() == 0 { 2 } else { 1 };
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
    /// for the dev IPC snapshot. Blocks on the GPU; dev tooling only.
    pub fn read_rgba(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Vec<u8> {
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
