//! What the app's PAGES need from the GPU, and nothing more (Android rung A1, 2026-11-20,
//! docs/design/platforms.md): the adapter, the device and its queue, and the window's surface
//! configured to draw on. egui draws every page with these alone.
//!
//! WHY ITS OWN STEP. Until 2026-11-20 `Renderer::init` went straight on from the device to every
//! shader module and pipeline of the 3D world, the cloud-noise bake (384^3 + 256^3 volumes) and the
//! ground textures, about five seconds of boot. A phone never draws the world (rung A5 is later),
//! so paying for it at every start costs a phone its start time and its battery. The device step
//! is now this file, and the world is `Renderer::build_world`, run on a device this step made.
//! A DESKTOP runs both at once, in the same order as before (`Renderer::init`), so nothing on a
//! desktop changed; a pages-only start (engine/renderer_slot.rs) keeps only this, and builds the
//! world the first time something needs it.
//!
//! THE BAKES. The cloud-noise and ground-texture bakes are pure CPU work on background threads. A
//! desktop starts them here, right after the adapter answers, so they overlap the device request
//! and the shader compiles exactly as they always did; a pages-only start does not start them at
//! all (`bakes_now` false), and `build_world` starts them itself if it is ever asked.
//!
//! THE SURFACE while only the pages draw: configured, resized, cleared and given back after a
//! phone app returns by the methods below, the same rules as the world renderer's own
//! (renderer/surface.rs: BUG-077's deferred present mode included).

use std::sync::mpsc::Receiver;

use super::{bounced_light_detail, cloud_noise, ground_textures, shadow_gpu, texture_pool};

/// The two cloud-noise volumes with their mip chains, as the bake thread sends them.
pub type CloudChains = (Vec<Vec<u8>>, Vec<Vec<u8>>);
/// The background bakes in flight: the cloud volumes and the ground textures (`None` where there
/// are no threads, wasm, and the world renderer bakes them inline).
pub type Bakes = (Option<Receiver<CloudChains>>, Option<Receiver<ground_textures::BakedGround>>);

/// The device, its queue and the window's surface: what the pages draw with (the module docs).
pub struct PagesGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// `None` while a phone app is away (renderer/surface.rs).
    pub surface: Option<wgpu::Surface<'static>>,
    pub config: wgpu::SurfaceConfiguration,
    /// A present-mode change waiting for the start of the next frame (BUG-077, surface.rs).
    pub pending_present_mode: Option<wgpu::PresentMode>,
    /// Whether the swapchain can be read back (the live screenshot command).
    pub supports_frame_capture: bool,
    /// Whether the device draws the terrain batch with one indirect multi-draw.
    pub patch_indirect: bool,
    /// Whether the bakes run on background threads (native) or inline (wasm).
    pub background_bakes: bool,
    /// The bakes started with the device, for `build_world` to collect (`None`: not started).
    pub bakes: Option<Bakes>,
}

impl PagesGpu {
    /// The adapter, the device and the surface. `bakes_now`: start the world's background bakes
    /// right after the adapter answers (a desktop, which builds the world at once); false for a
    /// pages-only start (the module docs).
    pub async fn new(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
        width: u32,
        height: u32,
        background_bakes: bool,
        bakes_now: bool,
    ) -> Self {
        // [BootPhase] sub-spans: renderer_init is the single largest boot
        // phase (6.7 s measured 2026-08-14); these marks attribute it so
        // optimization targets are data, not guesses. Grep run.log for
        // "[BootPhase]" after any boot.
        let t_phase = std::time::Instant::now();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .expect("No suitable GPU adapter found");
        log::info!("[BootPhase] adapter_request: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);
        // The card and its own memory, for Settings > Bounced light detail's warning.
        bounced_light_detail::note_adapter(&adapter.get_info());

        // ── The background bakes start HERE, and not one line earlier ──
        // They used to spawn in `native_pages_gpu`, before the instance existed,
        // on the reasoning that more overlap is better. It is not: the next
        // thing the boot thread does is ask the driver for an adapter, which
        // is LATENCY-bound work (DXGI enumeration, a D3D12 device per adapter
        // to query capabilities) that a saturated CPU stretches badly.
        // Measured 2026-09-19 on the boot rig by delaying the two bakes past
        // the adapter and changing nothing else: adapter_request 2173 ms ->
        // 345 ms. Nearly two seconds of "the driver is slow" was this
        // process's own twelve noise-bake threads. Everything AFTER the
        // adapter is throughput-bound and shares the machine happily (device
        // + shader modules + 19 PSO compiles run about 5 s, the bake needs
        // about 4, and the upload still reports "waited 0 ms"). If you move
        // these again, re-read `[BootPhase] adapter_request`: that is the
        // number that says whether you were right.
        let bakes = bakes_now.then(|| start_bakes(background_bakes));

        let t_phase = std::time::Instant::now();

        // v0.784.2 BOOT FIX: the uncapped-lights storage buffer (v0.782) needs
        // fragment-stage storage buffers, but the old `downlevel_webgl2_defaults`
        // profile requests ZERO of them -- so creating the camera bind group
        // layout failed device validation and the app died before the first
        // frame (operator: "I get a flicker but the game never comes up").
        // Request wgpu's standard native limits instead (every Vulkan/DX12-era
        // GPU supports them; the WebGL2 profile only mattered for a wasm target
        // this renderer doesn't build for). Resolution limits still follow the
        // adapter so huge-texture support matches the hardware.
        // 2026-07-11 (ultra star catalog): the 25M-star tier packs into a
        // ~300 MB vertex buffer, which EXCEEDS wgpu's default 256 MiB
        // max_buffer_size limit -- with the default, creating that buffer
        // would fail device validation at world load, the same boot-killing
        // failure class as v0.782. Follow the adapter's real buffer capacity
        // instead (desktop GPUs allow gigabytes); requesting exactly what
        // the adapter reports is always grantable. Every other limit stays
        // at the safe standard defaults. StarRenderer::new additionally
        // trims the star list to whatever THIS device's limit turns out to
        // be, so a small-limit adapter degrades to a partial sky, never a
        // dead app.
        let adapter_limits = adapter.limits();
        let mut required_limits =
            wgpu::Limits::default().using_resolution(adapter_limits.clone());
        required_limits.max_buffer_size = adapter_limits.max_buffer_size;
        // Draw-batching increment 2: request the indirect-draw features IF
        // the adapter has them (intersection = grantable by construction,
        // never a boot risk). When granted, the 12k-patch terrain batch
        // submits as ONE multi_draw_indexed_indirect; when not, the
        // per-draw loop runs on the exact same buffers and shaders.
        let indirect_features = wgpu::Features::MULTI_DRAW_INDIRECT
            | wgpu::Features::INDIRECT_FIRST_INSTANCE;
        // Same intersection trick for TIMESTAMP_QUERY (resource budgets
        // increment 1): asking for exactly what the adapter reports can never
        // fail device creation, which is the v0.782 boot-killer class. Without
        // it the Performance page falls back to CPU-side pass timing.
        // ... and block compression for the garment textures (BC7, the same intersection: texture_pool).
        let granted_indirect = (adapter.features() & (indirect_features | wgpu::Features::TIMESTAMP_QUERY | shadow_gpu::required_features())) | texture_pool::block_compression_feature(adapter.features());
        // Can this backend hand us a compiled-pipeline blob to keep between
        // boots? That is the standard answer to a slow PSO build, and it is
        // the first thing anyone looking at the `[Pipelines]` line will
        // reach for, so the run log says outright whether it is even on
        // offer. In wgpu 24 only the VULKAN backend advertises
        // `PIPELINE_CACHE`; the DX12 backend's `create_pipeline_cache` is a
        // stub that stores nothing and returns no data, and Windows runs
        // DX12 here. Logged rather than silently skipped so this stops being
        // re-investigated, and so the day it flips to true somebody sees it.
        log::info!(
            "[Pipelines] adapter offers a persistent pipeline cache: {}",
            adapter.features().contains(wgpu::Features::PIPELINE_CACHE)
        );
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("HumanityOS Renderer"),
                    required_features: granted_indirect,
                    required_limits,
                    ..Default::default()
                },
                None,
            )
            .await
            .expect("Failed to create device");
        log::info!("[BootPhase] device_request: {:.0} ms", t_phase.elapsed().as_secs_f32() * 1000.0);
        let patch_indirect = granted_indirect.contains(indirect_features);
        log::info!(
            "[PatchBatch] indirect multi-draw: {}",
            if patch_indirect { "SUPPORTED (one submit per batch)" } else { "unsupported (per-draw loop)" }
        );

        // Surface configuration
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        // Live screenshot command (v0.639): request COPY_SRC on the swapchain surface so the
        // rendered frame can be read back to a PNG. Most backends support this alongside
        // RENDER_ATTACHMENT; check first rather than assuming, so a backend that doesn't just
        // gets a clean `capture_current_frame` error instead of a wgpu validation panic.
        let supports_frame_capture = surface_caps.usages.contains(wgpu::TextureUsages::COPY_SRC);
        let mut surface_usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if supports_frame_capture {
            surface_usage |= wgpu::TextureUsages::COPY_SRC;
        }
        let config = wgpu::SurfaceConfiguration {
            usage: surface_usage,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);
        Self {
            device,
            queue,
            surface: Some(surface),
            config,
            pending_present_mode: None,
            supports_frame_capture,
            patch_indirect,
            background_bakes,
            bakes,
        }
    }

    // ── The surface while only the pages draw (the module docs) ──

    /// Record a present-mode change for the start of the next frame (BUG-077, surface.rs).
    pub fn set_vsync(&mut self, on: bool) {
        let mode = super::surface::vsync_present_mode(on);
        self.pending_present_mode = super::surface::pending_mode_change(self.config.present_mode, mode);
    }

    /// Apply a recorded present-mode change, before the frame acquires its texture (BUG-077).
    pub fn apply_pending_surface_config(&mut self) {
        if let Some(mode) = self.pending_present_mode.take() {
            log::info!("[Surface] present mode {:?} -> {mode:?} (applied before the frame)", self.config.present_mode);
            self.config.present_mode = mode;
            if let Some(s) = &self.surface {
                s.configure(&self.device, &self.config);
            }
        }
    }

    /// The window changed size: the surface follows (the pages have no other targets).
    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        if let Some(s) = &self.surface {
            s.configure(&self.device, &self.config);
        }
    }

    /// Acquire this frame's texture and clear it to `clear_color`, for egui to draw the pages on.
    pub fn acquire_surface_cleared(
        &self,
        clear_color: wgpu::Color,
    ) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
        let surface = self.surface.as_ref().ok_or(wgpu::SurfaceError::Lost)?;
        acquire_cleared(surface, &self.device, &self.queue, clear_color, None)
    }

    /// A new surface on `window` after a phone app came back, configured at the window's size now.
    #[cfg(feature = "native")]
    pub fn recreate_surface(&mut self, window: std::sync::Arc<winit::window::Window>) {
        let Some(instance) = super::surface::kept_instance() else {
            log::error!("[Surface] no instance kept: cannot make a new surface");
            return;
        };
        match instance.create_surface(window.clone()) {
            Ok(surface) => {
                let size = window.inner_size();
                self.config.width = size.width.max(1);
                self.config.height = size.height.max(1);
                surface.configure(&self.device, &self.config);
                self.surface = Some(surface);
            }
            Err(e) => log::error!("[Surface] could not make a new surface: {e}"),
        }
    }
}

/// Acquire `surface`'s next texture and clear it with one render pass: a frame a page covers
/// (the pages' GPU and the world renderer's `acquire_surface_cleared` share it).
pub fn acquire_cleared(
    surface: &wgpu::Surface<'static>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    clear_color: wgpu::Color,
    timestamp_writes: Option<wgpu::RenderPassTimestampWrites<'_>>,
) -> Result<(wgpu::SurfaceTexture, wgpu::TextureView), wgpu::SurfaceError> {
    let output = surface.get_current_texture()?;
    let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Clear Encoder") });
    {
        let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Clear Pass"),
            timestamp_writes,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(clear_color), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
    }
    queue.submit(std::iter::once(encoder.finish()));
    Ok((output, view))
}

/// Start the world's two background bakes (the module docs); `(None, None)` without threads.
pub fn start_bakes(background: bool) -> Bakes {
    if !background {
        return (None, None);
    }
    let (tx, cloud_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let threads =
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        let t0 = std::time::Instant::now();
        let shape = cloud_noise::generate_shape(threads);
        let detail = cloud_noise::generate_detail(threads);
        let t_gen = t0.elapsed().as_secs_f32() * 1000.0;
        let shape = cloud_noise::mip_chain(shape, cloud_noise::SHAPE_SIZE);
        let detail = cloud_noise::mip_chain(detail, cloud_noise::DETAIL_SIZE);
        log::info!(
            "Cloud noise volumes generated in background: {:.0} ms bake + {:.0} ms mips \
             ({} threads, {}^3 + {}^3, {:.0} MiB)",
            t_gen,
            t0.elapsed().as_secs_f32() * 1000.0 - t_gen,
            threads,
            cloud_noise::SHAPE_SIZE,
            cloud_noise::DETAIL_SIZE,
            (shape.iter().map(|l| l.len()).sum::<usize>()
                + detail.iter().map(|l| l.len()).sum::<usize>())
                as f32
                / (1024.0 * 1024.0),
        );
        let _ = tx.send((shape, detail));
    });
    let (tx, ground_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(ground_textures::bake_all());
    });
    (Some(cloud_rx), Some(ground_rx))
}
