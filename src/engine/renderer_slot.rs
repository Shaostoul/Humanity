//! The renderer the engine holds: the pages' GPU always, the 3D world's renderer once something
//! needs it (Android rung A1, 2026-11-20; docs/design/platforms.md).
//!
//! WHY. A phone never draws the world (rung A5 is later; Play's Enter is a notice there), yet until
//! 2026-11-20 every start built the whole world renderer: every shader module and pipeline, the
//! cloud-noise bake and the ground textures, about five seconds and a burst of battery, for
//! nothing. A PAGES-ONLY START ([`pages_only_start`]) now makes only the device and the window's
//! surface (renderer/pages_gpu.rs), which is all egui needs to draw the pages, and builds the world
//! renderer the first time something needs it ([`RendererSlot::ensure_world`]: entering the world).
//! A DESKTOP has started pages-first too since 2026-11-20 (engine/world_prep.rs): the world's
//! renderer is built right after the first page frame on a WORKER THREAD ([`RendererSlot::start_background`],
//! renderer/world_build.rs) while the pages keep the surface and draw, and the frame installs it when
//! it is done ([`RendererSlot::poll_world`]). "At start, before the pages" (the setting, or
//! `HUMANITY_PAGES_FIRST=0`) builds both at start, in the order of every start before that day.
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
//! THE DEV SWITCHES (engine/world_prep.rs). `HUMANITY_PAGES_FIRST=1` starts a desktop pages-only and
//! builds the world only when it is entered, so a rig proves the on-demand build on this PC
//! (`HUMANITY_PHONE_PREVIEW=1`, the phone's view, starts pages-only too, with Play's Enter a notice
//! as on a phone); `HUMANITY_PAGES_FIRST=0` is the old order.

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::BTreeSet;
use std::ops::{Deref, DerefMut};
use std::panic::Location;
use std::time::Duration;

use crate::engine::world_prep::Choice;
use crate::renderer::pages_gpu::PagesGpu;
use crate::renderer::world_build::{self, Step, Worker};
use crate::renderer::Renderer;

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
    /// The world's renderer being built on a worker thread, and what it was started for.
    worker: RefCell<Option<(Worker, String)>>,
}

impl RendererSlot {
    /// The window's device and surface, and with "at start, before the pages" the whole world
    /// renderer at once (the order and the timing of every start before 2026-11-20).
    pub(crate) async fn new_native(window: std::sync::Arc<winit::window::Window>, prep: Choice) -> Self {
        let pages_only = prep != Choice::AtStart;
        let core = Renderer::native_pages_gpu(window, !pages_only).await;
        let slot = Self {
            device: core.device.clone(),
            queue: core.queue.clone(),
            pages: RefCell::new(Some(core)),
            world: OnceCell::new(),
            pages_only,
            stray: Cell::new(false),
            touched: RefCell::new(BTreeSet::new()),
            worker: RefCell::new(None),
        };
        if pages_only {
            log::info!("[PagesOnly] started pages-only: the world's renderer is built beside the pages or when something needs it");
        } else {
            slot.build("the start of a desktop run");
        }
        slot
    }

    /// Begin building the world's renderer on a worker thread while the pages keep the surface
    /// (renderer/world_build.rs). No-op when it is built or being built (a call that is not
    /// `polite` still ends the politeness). `reason` names it in the log.
    pub(crate) fn start_background(&self, polite: bool, reason: &str) {
        if self.world.get().is_some() || self.worker.borrow().is_some() {
            if !polite {
                world_build::set_polite(false);
            }
            return;
        }
        let Some(inputs) = self.pages.borrow_mut().as_mut().map(|p| p.world_inputs()) else {
            return;
        };
        *self.worker.borrow_mut() = Some((Worker::spawn(inputs, polite), reason.to_string()));
    }

    /// The step the worker's build is on (None when nothing is being built).
    pub(crate) fn build_step(&self) -> Option<Step> {
        self.worker.borrow().as_ref().map(|(w, _)| w.progress().step())
    }

    /// Install the worker's renderer if it is done; never waits. True on the frame it installs.
    /// A worker that died leaves the pages' GPU in place: the next need builds here, as before.
    pub(crate) fn poll_world(&mut self) -> bool {
        let polled = match self.worker.get_mut().as_ref() {
            None => return false,
            Some((w, _)) => w.poll(),
        };
        match polled {
            world_build::Poll::Building => false,
            world_build::Poll::Built(r, took) => {
                self.install(r, took);
                true
            }
            world_build::Poll::Died => {
                self.worker.get_mut().take();
                world_build::set_polite(false);
                log::error!("[WorldPrep] the world-build thread died (see the panic above); the world is built when it is next needed");
                false
            }
        }
    }

    /// Put the worker's finished renderer in place: it takes the surface over from the pages.
    fn install(&self, mut r: Box<Renderer>, took: Duration) {
        let reason = self.worker.borrow_mut().take().map(|(_, why)| why).unwrap_or_default();
        world_build::set_polite(false);
        let Some(core) = self.pages.borrow_mut().take() else {
            return;
        };
        r.adopt_surface(core);
        crate::engine::low_impact::hold_renderer(&mut r);
        log::info!("[PagesOnly] the world's renderer built for {reason}: {} ms (on a worker thread)", took.as_millis());
        let _ = self.world.set(r);
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
    /// switches on it (engine/low_impact.rs: a no-op in a normal run). Being built on a worker
    /// already: wait for it, flat out, rather than build a second one.
    fn build(&self, reason: &str) {
        if self.world.get().is_some() {
            return;
        }
        let waiting = self.worker.borrow_mut().take();
        if let Some((w, why)) = waiting {
            world_build::set_polite(false);
            let t0 = std::time::Instant::now();
            if let Some((r, took)) = w.wait() {
                log::info!("[PagesOnly] waited {} ms on the frame for the worker's build ({why}), needed for {reason}", t0.elapsed().as_millis());
                *self.worker.borrow_mut() = Some((w, why));
                self.install(r, took);
                return;
            }
        }
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
