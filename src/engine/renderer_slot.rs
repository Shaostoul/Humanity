//! The renderer the engine holds: the pages' GPU always, the 3D world's renderer once something
//! needs it (Android rung A1, 2026-11-20; docs/design/platforms.md).
//!
//! WHY. A phone never draws the world (rung A5 is later; Play's Enter is a notice there), yet until
//! 2026-11-20 every start built the whole world renderer: every shader module and pipeline, the
//! cloud-noise bake and the ground textures, about five seconds and a burst of battery, for
//! nothing. A PAGES-ONLY START ([`pages_only_start`]) now makes only the device and the window's
//! surface (renderer/pages_gpu.rs), which is all egui needs to draw the pages, and builds the world
//! renderer the first time something needs it ([`RendererSlot::ensure_world`]: entering the world).
//! A DESKTOP builds both at start, in the same order as before, so nothing there changed.
//!
//! HOW EVERY CALLER STAYS THE SAME. `gpu.renderer` is this slot. The device and the queue are its
//! own fields (clones of the one device: wgpu's handles share it), and the surface's methods the
//! pages use (resize, the clear before egui, the present mode, the phone's surface going and coming
//! back) are its own methods, answered by whichever half holds the surface. Rust takes a type's own
//! fields and methods before it looks through `Deref`, so those never touch the world. Anything
//! else (a pipeline, the scene, a world setting) goes through `Deref` to the world renderer, which
//! is built at that moment if it is not built yet: a stray reach for the world can cost time on a
//! phone, never a crash.
//!
//! THE STRAY-TOUCH LOG, the dev tool that keeps this honest. A pages-only start that builds the
//! world through `Deref` (rather than `ensure_world`) logs where it was reached from, and from then
//! on every NEW place that reaches it, once each (`[PagesOnly] world touched from <file>:<line>`),
//! so one boot lists every place to fix. scripts/verify-phone-start.js fails on any such line, and
//! counts the pipelines wgpu itself holds ([`pipeline_counts`]), never a flag of ours.
//!
//! THE DEV SWITCH. `HUMANITY_PAGES_FIRST=1` starts a desktop pages-only while keeping the world
//! reachable: the world renderer is built at the first Enter, so a rig proves the on-demand build on
//! this PC (`HUMANITY_PHONE_PREVIEW=1`, the phone's view, starts pages-only too, with Play's Enter a
//! notice as on a phone).

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::panic::Location;

use crate::renderer::pages_gpu::PagesGpu;
use crate::renderer::Renderer;

/// Start pages-only: a phone (or the phone preview on a desktop), or a desktop asked to with
/// `HUMANITY_PAGES_FIRST=1` (the module docs). Read once.
pub(crate) fn pages_only_start() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        humanity_core::platform::device::phone()
            || std::env::var("HUMANITY_PAGES_FIRST").map(|v| v == "1").unwrap_or(false)
    })
}

/// The pages' GPU, and the world's renderer once built (the module docs).
pub(crate) struct RendererSlot {
    /// The device every page and every pass draws with (a clone of the one device).
    pub(crate) device: wgpu::Device,
    /// Its queue (a clone of the one queue).
    pub(crate) queue: wgpu::Queue,
    /// The pages' GPU until the world is built, which takes it over (surface and all).
    pages: RefCell<Option<PagesGpu>>,
    /// The world's renderer, once built.
    world: OnceCell<Box<Renderer>>,
    /// This run started pages-only.
    pages_only: bool,
    /// The world was built by a stray reach through `Deref` in a pages-only run: from then on each
    /// new place that reaches it is logged (the module docs).
    stray: Cell<bool>,
    /// The places already logged.
    touched: RefCell<BTreeSet<(&'static str, u32)>>,
}

impl RendererSlot {
    /// The window's device and surface, and on a desktop the whole world renderer at once
    /// (`pages_only` false: the order and the timing of every start before 2026-11-20).
    pub(crate) async fn new_native(window: std::sync::Arc<winit::window::Window>, pages_only: bool) -> Self {
        let core = Renderer::native_pages_gpu(window, !pages_only).await;
        let slot = Self {
            device: core.device.clone(),
            queue: core.queue.clone(),
            pages: RefCell::new(Some(core)),
            world: OnceCell::new(),
            pages_only,
            stray: Cell::new(false),
            touched: RefCell::new(BTreeSet::new()),
        };
        if pages_only {
            log::info!("[PagesOnly] started pages-only: the world's renderer is built when something needs it");
        } else {
            slot.build("the start of a desktop run");
        }
        slot
    }

    /// Whether the world's renderer is built.
    pub(crate) fn world_built(&self) -> bool {
        self.world.get().is_some()
    }

    /// Whether this run started pages-only.
    pub(crate) fn pages_only(&self) -> bool {
        self.pages_only
    }

    /// The world's renderer if it is built; never builds it.
    pub(crate) fn world(&self) -> Option<&Renderer> {
        self.world.get().map(|b| &**b)
    }

    /// The world's renderer, mutable, if it is built; never builds it. For a per-frame write that
    /// only matters to a world being drawn (the next frame writes it again once there is one).
    pub(crate) fn world_mut(&mut self) -> Option<&mut Renderer> {
        self.world.get_mut().map(|b| &mut **b)
    }

    /// The world's renderer, built now if it is not yet: what entering the world calls, so the
    /// build is timed and named rather than found as a stray touch.
    pub(crate) fn ensure_world(&mut self, reason: &str) -> &mut Renderer {
        if self.world.get().is_none() {
            self.build(reason);
        }
        self.world.get_mut().map(|b| &mut **b).expect("the world's renderer was just built")
    }

    /// Build the world's renderer on the pages' GPU (renderer/pages_gpu.rs), and hold low impact's
    /// switches on it (engine/low_impact.rs: a no-op in a normal run).
    fn build(&self, reason: &str) {
        let Some(core) = self.pages.borrow_mut().take() else {
            return;
        };
        let t0 = std::time::Instant::now();
        let mut r = Renderer::build_world(core);
        crate::engine::low_impact::hold_renderer(&mut r);
        if self.pages_only {
            log::info!("[PagesOnly] the world's renderer built for {reason}: {} ms", t0.elapsed().as_millis());
        }
        let _ = self.world.set(Box::new(r));
    }

    /// The world's renderer for `Deref`: built now if it is not (a stray reach in a pages-only
    /// run, logged with its place), and every new place logged once after a stray build.
    fn world_or_build(&self, at: &'static Location<'static>) -> &Renderer {
        if self.world.get().is_none() {
            if self.pages_only {
                self.stray.set(true);
                log::warn!(
                    "[PagesOnly] the world's renderer was built by a reach from {}:{}: a pages-only start should not need it there",
                    at.file(),
                    at.line()
                );
            }
            self.build("a reach through Deref");
        }
        if self.stray.get() {
            let key = (at.file(), at.line());
            if self.touched.borrow_mut().insert(key) {
                log::warn!("[PagesOnly] world touched from {}:{}", key.0, key.1);
            }
        }
        self.world.get().map(|b| &**b).expect("the world's renderer was just built")
    }

    // ── The surface: whichever half holds it (the module docs) ──

    /// The window changed size.
    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        match self.world.get_mut() {
            Some(r) => r.resize(width, height),
            None => {
                if let Some(p) = self.pages.get_mut().as_mut() {
                    p.resize(width, height);
                }
            }
        }
    }

    /// The surface's configuration, from whichever half holds it.
    fn config(&self) -> (wgpu::TextureFormat, u32, u32) {
        match self.world.get() {
            Some(r) => {
                let (w, h) = r.surface_size();
                (r.surface_format(), w, h) // display-format: the surface's own format, which egui draws in
            }
            None => {
                let p = self.pages.borrow();
                let c = &p.as_ref().expect("the pages' GPU or the world holds the surface").config;
                (c.format, c.width, c.height)
            }
        }
    }

    /// Width over height of the surface.
    pub(crate) fn aspect_ratio(&self) -> f32 {
        let (_, w, h) = self.config();
        w as f32 / h as f32
    }

    /// The display format egui draws in (renderer/surface.rs `surface_format`).
    pub(crate) fn surface_format(&self) -> wgpu::TextureFormat {
        self.config().0 // display-format: the surface's own format, which egui draws in
    }

    /// The surface's size in pixels.
    pub(crate) fn surface_size(&self) -> (u32, u32) {
        let (_, w, h) = self.config();
        (w, h)
    }

    /// Acquire this frame's texture cleared, for a frame a page covers.
    pub(crate) fn acquire_surface_cleared(
        &self,
        clear_color: wgpu::Color,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        match self.world.get() {
            Some(r) => r.acquire_surface_cleared(clear_color),
            None => self.pages.borrow().as_ref().ok_or(wgpu::SurfaceError::Lost)?.acquire_surface_cleared(clear_color),
        }
    }

    /// Apply a recorded present-mode change before the frame (BUG-077).
    pub(crate) fn apply_pending_surface_config(&mut self) {
        match self.world.get_mut() {
            Some(r) => r.apply_pending_surface_config(),
            None => {
                if let Some(p) = self.pages.get_mut().as_mut() {
                    p.apply_pending_surface_config();
                }
            }
        }
    }

    /// Record the Settings VSync switch for the next frame (BUG-077).
    pub(crate) fn set_vsync(&mut self, on: bool) {
        match self.world.get_mut() {
            Some(r) => r.set_vsync(on),
            None => {
                if let Some(p) = self.pages.get_mut().as_mut() {
                    p.set_vsync(on);
                }
            }
        }
    }

    /// Whether there is a surface to draw on (false only while a phone app is away).
    pub(crate) fn has_surface(&self) -> bool {
        match self.world.get() {
            Some(r) => r.has_surface(),
            None => self.pages.borrow().as_ref().is_some_and(|p| p.surface.is_some()),
        }
    }

    /// The app left the screen: drop the surface (engine/app_lifecycle.rs).
    pub(crate) fn drop_surface(&mut self) {
        match self.world.get_mut() {
            Some(r) => r.drop_surface(),
            None => {
                if let Some(p) = self.pages.get_mut().as_mut() {
                    p.surface = None;
                }
            }
        }
    }

    /// The app came back: a new surface on the same window (engine/app_lifecycle.rs).
    pub(crate) fn recreate_surface(&mut self, window: std::sync::Arc<winit::window::Window>) {
        match self.world.get_mut() {
            Some(r) => r.recreate_surface(window),
            None => {
                if let Some(p) = self.pages.get_mut().as_mut() {
                    p.recreate_surface(window);
                }
            }
        }
    }

    /// Add a mesh to the world's renderer (renderer/mod.rs `add_mesh`). The slot's own method, so a
    /// call that reads `renderer.device` for its argument borrows the slot in two phases, as it did
    /// when the renderer was a plain field; the world is built here if it is not yet.
    #[track_caller]
    pub(crate) fn add_mesh(&mut self, mesh: crate::renderer::mesh::Mesh) -> usize {
        self.world_or_build(Location::caller());
        self.world.get_mut().map(|b| &mut **b).expect("the world's renderer was just built").add_mesh(mesh)
    }

    /// Replace a mesh in the world's renderer (renderer/mod.rs `replace_mesh`), as `add_mesh`.
    #[track_caller]
    pub(crate) fn replace_mesh(&mut self, idx: usize, mesh: crate::renderer::mesh::Mesh) {
        self.world_or_build(Location::caller());
        self.world.get_mut().map(|b| &mut **b).expect("the world's renderer was just built").replace_mesh(idx, mesh)
    }

    /// A pass's GPU timer, when the world's renderer has them (none before it is built: the
    /// Performance page then reads the CPU's own timings, as on an adapter without timestamps).
    pub(crate) fn pass_timer(&self, id: &'static str) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.world.get().and_then(|r| r.pass_timer(id))
    }
}

impl Deref for RendererSlot {
    type Target = Renderer;
    #[track_caller]
    fn deref(&self) -> &Renderer {
        self.world_or_build(Location::caller())
    }
}

impl DerefMut for RendererSlot {
    #[track_caller]
    fn deref_mut(&mut self) -> &mut Renderer {
        self.world_or_build(Location::caller());
        self.world.get_mut().map(|b| &mut **b).expect("the world's renderer was just built")
    }
}

/// What wgpu itself holds right now, counted by wgpu (`Instance::generate_report`), never by a flag
/// of ours: render pipelines, compute pipelines, shader modules. `None` where wgpu cannot report.
pub(crate) fn pipeline_counts() -> Option<PipelineCounts> {
    let report = crate::renderer::surface::kept_instance()?.generate_report()?;
    let hub = report.hub_report();
    Some(PipelineCounts {
        render: hub.render_pipelines.num_kept_from_user,
        compute: hub.compute_pipelines.num_kept_from_user,
        shader_modules: hub.shader_modules.num_kept_from_user,
    })
}

/// [`pipeline_counts`]'s answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PipelineCounts {
    pub(crate) render: usize,
    pub(crate) compute: usize,
    pub(crate) shader_modules: usize,
}
