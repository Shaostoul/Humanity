//! The 3D world's renderer built OFF THE FRAME while the pages draw (2026-11-20; the desktop's
//! pages-first start, engine/world_prep.rs, docs/design/platforms.md A1).
//!
//! WHY. The operator, 2026-11-20: "Can we skip building the 3D world's graphics immediately? Could we
//! draw it out over several seconds to get people in chat faster?" Measured that day on this PC: a
//! desktop's window waited 18.6 s for its first frame, because every shader module and pipeline of
//! the world, the cloud-noise bake and the ground textures were built before it; a pages-only start
//! drew its first frame in under a second. And his standing rule (memory
//! feedback_gradual_background_work): background preparation runs gradually, never in one burst, "so
//! the app doesn't hang".
//!
//! HOW. The world's renderer needs the device and the surface's FORMAT and SIZE, never the surface
//! itself, until it draws. So [`Renderer::build_world_detached`] builds it from [`WorldInputs`] (the
//! device, the queue and a copy of the surface's configuration) on a worker thread ([`Worker`]),
//! while the pages keep the surface and go on drawing with it; the slot (engine/renderer_slot.rs)
//! installs the finished renderer on the frame with [`Renderer::adopt_surface`], which moves the
//! surface over and follows a size change made meanwhile. wgpu's device and queue are thread-safe,
//! and the v0.865 star-sky prebuild has built pipelines off the main thread since 2026-07.
//!
//! GRADUAL, NOT A BURST. Off the frame is the first half; the second is that a build beside the
//! pages never starves them. While a background build is POLITE ([`set_polite`]) every thread it
//! starts, the pipeline compiles' included, runs one step below normal priority ([`build_thread`]),
//! so the frame's thread (normal) always wins a core, and the 320 MiB of cloud volumes go to the
//! graphics card a 64 MiB slab at a time with a pause between ([`Progress::pace_upload`]) rather than
//! in one copy the next page frame would wait behind. Pressing Play ends the politeness: the worker
//! goes back to normal priority at its next step ([`Progress::mark`]) and the remainder runs flat
//! out while the page shows how far along it is; a build started BY Play (the default) is flat out
//! from the start (`build_thread` says why, measured). Polite or flat out, no thread of the build
//! runs on the first core of those the app may use (2026-11-21, [`build_thread`]): that core is the
//! window's, so the progress card keeps moving while every other core builds. A start that builds
//! before the pages (the old order) is neither: nothing draws until it is done.
//!
//! THE STEPS ([`Step`]) are the build's own phases in order, each with its words by key
//! (`world_prep.step.<key>`), which the progress line names.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::pages_gpu::{Bakes, PagesGpu};
use super::Renderer;

/// What the world's renderer is built from: the pages' device and queue (clones of the one device:
/// wgpu's handles share it) and a copy of the surface's configuration (format and size), never the
/// surface, which the pages keep drawing with until the renderer is installed.
pub struct WorldInputs {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub supports_frame_capture: bool,
    pub patch_indirect: bool,
    pub background_bakes: bool,
    /// The bakes started with the device, if they were (a start that builds at once).
    pub bakes: Option<Bakes>,
    /// The compiles' gate ([`compiles_running`]), closed by [`Worker::spawn`] before the worker
    /// starts and opened once the pipelines are compiled; None for a build nothing runs beside.
    pub gate: Option<CompilesRunning>,
}

impl PagesGpu {
    /// The inputs for the world's renderer, the surface left here (the module docs). Takes the bakes
    /// started with the device, if any.
    pub fn world_inputs(&mut self) -> WorldInputs {
        WorldInputs {
            device: self.device.clone(),
            queue: self.queue.clone(),
            config: self.config.clone(),
            supports_frame_capture: self.supports_frame_capture,
            patch_indirect: self.patch_indirect,
            background_bakes: self.background_bakes,
            bakes: self.bakes.take(),
            gate: None,
        }
    }
}

impl Renderer {
    /// The whole world renderer on the pages' GPU at once, surface and all: the start that builds
    /// before the pages (the old order), the wasm canvas, and a build something needed this instant.
    #[cfg(any(feature = "native", feature = "wasm"))]
    pub fn build_world(mut core: PagesGpu) -> Self {
        let inputs = core.world_inputs();
        let mut r = Self::build_world_detached(inputs, &Progress::default());
        r.adopt_surface(core);
        r
    }

    /// Take over the window's surface from the pages (the module docs): its configuration as the
    /// pages last set it (a resize or a VSync change made during the build included), and the
    /// size-dependent targets rebuilt when the window changed size since the build began.
    pub fn adopt_surface(&mut self, pages: PagesGpu) {
        let built = (self.config.width, self.config.height);
        self.surface = pages.surface;
        self.config = pages.config;
        self.pending_present_mode = pages.pending_present_mode;
        let now = (self.config.width, self.config.height);
        if now != built {
            self.resize(now.0, now.1);
        }
    }
}

/// The build's phases in order (the module docs). `Waiting` is before the worker began; `Done` once
/// the renderer is whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Step {
    Waiting = 0,
    Targets,
    Effects,
    Shaders,
    Pipelines,
    Buffers,
    Clouds,
    Ground,
    Sky,
    Done,
}

impl Step {
    /// Every step, in order.
    pub const ALL: [Step; 10] = [
        Step::Waiting,
        Step::Targets,
        Step::Effects,
        Step::Shaders,
        Step::Pipelines,
        Step::Buffers,
        Step::Clouds,
        Step::Ground,
        Step::Sky,
        Step::Done,
    ];

    /// The steps that do work (all but `Waiting` and `Done`): the "n of N" of the progress line.
    pub const WORK: usize = 8;

    /// Its words' key part: `world_prep.step.<key>` in data/i18n/en.json.
    pub fn key(self) -> &'static str {
        match self {
            Step::Waiting => "waiting",
            Step::Targets => "targets",
            Step::Effects => "effects",
            Step::Shaders => "shaders",
            Step::Pipelines => "pipelines",
            Step::Buffers => "buffers",
            Step::Clouds => "clouds",
            Step::Ground => "ground",
            Step::Sky => "sky",
            Step::Done => "done",
        }
    }

    /// Which work step this is, from 1 (`Targets`) to [`Step::WORK`] (`Sky`); 0 before, WORK after.
    pub fn number(self) -> usize {
        (self as usize).min(Self::WORK)
    }

    fn from_u8(v: u8) -> Step {
        Self::ALL.get(v as usize).copied().unwrap_or(Step::Done)
    }
}

/// How far a build has come, shared between the worker and the frame.
#[derive(Debug, Default)]
pub struct Progress {
    step: AtomicU8,
}

impl Progress {
    /// The step now under way.
    pub fn step(&self) -> Step {
        Step::from_u8(self.step.load(Ordering::Relaxed))
    }

    /// The build has reached `step`. Called on the building thread: once the politeness has ended
    /// (Play was pressed), the thread is put back at normal priority, so the rest runs flat out.
    pub fn mark(&self, step: Step) {
        self.step.store(step as u8, Ordering::Relaxed);
        if !polite() {
            set_this_threads_priority(THREAD_PRIORITY_NORMAL);
        }
    }

    /// Between two cloud-volume slabs: while the build is polite, send what was written so far to
    /// the graphics card and pause, so the copies go out a slab at a time beside the page frames
    /// instead of landing on the next page frame's submit all at once (the module docs).
    pub fn pace_upload(&self, queue: &wgpu::Queue) {
        if polite() {
            queue.submit(std::iter::empty());
            std::thread::sleep(UPLOAD_PAUSE);
        }
    }
}

/// The pause between two cloud-volume slabs in a polite build.
pub const UPLOAD_PAUSE: Duration = Duration::from_millis(8);

/// The most bytes one upload of the build sends to the graphics card in one call (2026-11-21). While
/// wgpu makes an upload's staging buffer and copies the bytes into it, it holds the queue's write
/// lock and the device's resource lock for reading, and the window's frame needs the same locks for
/// its own uploads (egui's textures and buffers), its submit and its present. Seen that day
/// (`just verify-phone-start`, the long-gap lines): the 128 MiB Milky Way glow went up in one call
/// while Play's progress card drew, and the frame beside it took 438 ms. That frame's stall had
/// another cause (it stayed with the glow in bands: the creations beside the renderer's own build,
/// now held back by [`compiles_running`]; BUG-338), but one call of 128 to 512 MiB holds those
/// locks for its whole copy whatever else does. 8 MiB is a few milliseconds.
pub const UPLOAD_SLICE_BYTES: usize = 8 << 20;

/// Upload `data`, the rows of one mip level of a 2D texture (or of one layer of an array), in bands
/// of rows of at most [`UPLOAD_SLICE_BYTES`] each (the constant's notes). An uncompressed format:
/// `bytes_per_row` is a row of texels, `width` by `height` texels at `layer`.
#[allow(clippy::too_many_arguments)]
pub fn write_texture_in_slices(queue: &wgpu::Queue, texture: &wgpu::Texture, mip_level: u32, layer: u32, data: &[u8], bytes_per_row: u32, width: u32, height: u32) {
    for (y, rows) in row_bands(bytes_per_row, height, UPLOAD_SLICE_BYTES) {
        let start = y as usize * bytes_per_row as usize;
        let end = start + rows as usize * bytes_per_row as usize;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture, mip_level, origin: wgpu::Origin3d { x: 0, y, z: layer }, aspect: wgpu::TextureAspect::All },
            &data[start..end],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bytes_per_row), rows_per_image: Some(rows) },
            wgpu::Extent3d { width, height: rows, depth_or_array_layers: 1 },
        );
    }
}

/// The bands `(first row, rows)` a `height`-row image of `bytes_per_row` goes up in: each at most
/// `cap` bytes (one row at the least), in order, covering every row once.
pub fn row_bands(bytes_per_row: u32, height: u32, cap: usize) -> Vec<(u32, u32)> {
    let per = ((cap / (bytes_per_row.max(1) as usize)).max(1)).min(u32::MAX as usize) as u32;
    let mut out = Vec::new();
    let mut y = 0;
    while y < height {
        let rows = per.min(height - y);
        out.push((y, rows));
        y += rows;
    }
    out
}

/// A background build runs politely (the module docs).
static POLITE: AtomicBool = AtomicBool::new(false);

/// Begin or end the politeness: on while a background build runs beside the pages, off when the
/// person asked for the world (Play: the rest runs flat out) or the build is done.
pub fn set_polite(on: bool) {
    POLITE.store(on, Ordering::Relaxed);
}

/// Whether the world's preparation is running politely now.
pub fn polite() -> bool {
    POLITE.load(Ordering::Relaxed)
}

/// Call first in every thread the world's preparation starts. Two things:
///
/// THE FRAME'S CORE, always (2026-11-21). The thread stays off the first core of the processors this
/// app may use ([`reserved_for_the_frame`]), so the frame's thread (the window: the pages, the
/// progress card) and the graphics driver's own threads always find a core the build is not on.
/// Measured with `just verify-phone-start` that day (release build, other agents compiling): under
/// Play's progress card the page frames came up to 606 ms apart, and the long-gap lines named no one
/// stage but whichever the frame's thread was in when it lost its core (the present 100 to 214 ms,
/// the egui paint 390 and 577 ms, the ECS 138 ms, a mirror 85 ms): the twenty pipeline compiles, the
/// cloud noise's twelve workers and the bakes, all at normal priority, left it one runnable thread
/// among some forty. This lowers no priority (the brief of that day: never slow the work someone
/// waits for by its priority; below-normal threads starved a Play build to 121 s beside compiles);
/// the build keeps every other core at its own priority.
///
/// POLITE: while the preparation runs beside the pages, the thread runs one step below normal
/// priority as well, so the frame's thread always wins a core (the module docs). NOT WHEN SOMEONE
/// WAITS (measured 2026-11-20): a build Play is waiting for runs at normal priority; with every
/// thread below normal, one Play build on this PC took 121 s instead of 9 s while other agents'
/// compiles kept the processors busy, because below-normal threads only get what is left over.
///
/// Both are Windows calls (a no-op elsewhere); a failure to set either only means the thread shares
/// the cores as before.
pub fn build_thread() {
    PREPARING.with(|p| p.set(true));
    keep_off_the_frames_core();
    if polite() {
        set_this_threads_priority(THREAD_PRIORITY_BELOW_NORMAL);
    }
}

thread_local! {
    /// This thread is one of the world's preparation ([`build_thread`] was called first in it):
    /// the only threads [`wait_for_the_compiles`] holds back.
    static PREPARING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// THE COMPILES' GATE (2026-11-21, BUG-338's third cause). Until the world's renderer has made its
/// targets and compiled its pipelines (the Targets to Pipelines steps), no other thread of the
/// preparation makes GPU memory: it waits here first ([`wait_for_the_compiles`]). Measured that day
/// with `just verify-phone-start`: once a build, one page frame under Play's progress card took 440
/// to 940 ms, and in every run it fell while the star sky's thread made the Milky Way glow's
/// texture memory and buffers beside the build's own GPU work, which then crawled (the glow's
/// sixteen 8 MiB bands written in 3,908 ms, the star points' 1.4 MB buffer made in 686 and 997
/// ms), while the frame waited in its own small uploads (the egui buffers' write 502 ms, their
/// submit 287 ms, the image's acquire). wgpu makes every buffer and texture, the staging memory of
/// each upload included, under one lock (wgpu-hal's DX12 memory allocator, held across the driver's
/// call), so a creation that crawls holds the frame's next upload behind it. Moving the six screen
/// passes off the worker's serial path ([`EffectPasses`]) only moved the stall with them into the
/// pipeline step; the cloud volumes, 320 MiB made after the compiles, never held a frame. Gated at
/// the pipeline step only, the play_during boot had no long gap at all (0 of 270 frames; the glow's
/// rows then written in 47 ms, the star buffer made in 22), while the desktop boot's star sky got
/// to its memory during a slow Targets step, before the gate closed, and the stall came back (391
/// ms): so it closes in [`Worker::spawn`], before either thread exists. Whether the driver or the
/// busy processors made those creations crawl is not separated: either way they wait, which costs
/// Play nothing (the star sky finishes long before the renderer). Counted, so overlapping builds
/// each hold it; a [`CompilesRunning`] guard opens it when dropped, a panicking build included.
static COMPILES: (std::sync::Mutex<u32>, std::sync::Condvar) = (std::sync::Mutex::new(0), std::sync::Condvar::new());

/// While held, the pipelines compile (the gate's notes): made by [`compiles_running`].
pub struct CompilesRunning(());

/// Close the gate until the returned guard is dropped (the gate's notes).
pub fn compiles_running() -> CompilesRunning {
    *COMPILES.0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    CompilesRunning(())
}

impl Drop for CompilesRunning {
    fn drop(&mut self) {
        let mut n = COMPILES.0.lock().unwrap_or_else(|e| e.into_inner());
        *n = n.saturating_sub(1);
        COMPILES.1.notify_all();
    }
}

/// On a thread of the world's preparation ([`build_thread`]), wait until no pipeline compiles run
/// before making GPU memory (the gate's notes); anywhere else (the frame, a test) return at once, so
/// the window never waits on it. How long it waited.
pub fn wait_for_the_compiles() -> Duration {
    let t = Instant::now();
    if PREPARING.with(|p| p.get()) {
        let mut n = COMPILES.0.lock().unwrap_or_else(|e| e.into_inner());
        while *n > 0 {
            n = COMPILES.1.wait(n).unwrap_or_else(|e| e.into_inner());
        }
    }
    t.elapsed()
}

const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;
const THREAD_PRIORITY_NORMAL: i32 = 0;

/// How many logical processors, the lowest numbered of those the app may use, the build leaves to
/// the frame: one physical core (its two logical processors with simultaneous multithreading) once
/// there are six or more, one with three to five, none below (the build needs them all then).
pub fn reserved_for_the_frame(usable: u32) -> u32 {
    match usable {
        0..=2 => 0,
        3..=5 => 1,
        _ => 2,
    }
}

/// `mask` (the processors the app may use) without its `n` lowest set bits: the processors a build
/// thread may run on.
pub fn without_lowest(mask: usize, n: u32) -> usize {
    let mut m = mask;
    for _ in 0..n {
        m &= m.wrapping_sub(1);
    }
    m
}

/// Keep the calling thread off the frame's reserved processors ([`build_thread`]).
fn keep_off_the_frames_core() {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn GetCurrentThread() -> isize;
            fn GetProcessAffinityMask(process: isize, process_mask: *mut usize, system_mask: *mut usize) -> i32;
            fn SetThreadAffinityMask(thread: isize, mask: usize) -> usize;
        }
        let (mut process, mut system) = (0usize, 0usize);
        // SAFETY: GetCurrentProcess and GetCurrentThread return pseudo-handles that need no
        // closing; GetProcessAffinityMask writes the two masks into the two valid places given, and
        // SetThreadAffinityMask only reads the thread handle and a mask within the process's.
        unsafe {
            if GetProcessAffinityMask(GetCurrentProcess(), &mut process, &mut system) == 0 {
                return;
            }
            let mask = without_lowest(process, reserved_for_the_frame(process.count_ones()));
            if mask != 0 && mask != process {
                SetThreadAffinityMask(GetCurrentThread(), mask);
            }
        }
    }
}

/// Set the calling thread's priority (a Windows call; a no-op elsewhere).
fn set_this_threads_priority(priority: i32) {
    #[cfg(windows)]
    {
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentThread() -> isize;
            fn SetThreadPriority(thread: isize, priority: i32) -> i32;
        }
        // SAFETY: GetCurrentThread returns a pseudo-handle for the calling thread that needs no
        // closing, and SetThreadPriority only reads it.
        unsafe {
            SetThreadPriority(GetCurrentThread(), priority);
        }
    }
    #[cfg(not(windows))]
    let _ = priority;
}

/// The world's renderer being built on a worker thread (the module docs).
pub struct Worker {
    rx: Receiver<Box<Renderer>>,
    progress: Arc<Progress>,
    started: Instant,
}

/// What [`Worker::poll`] found.
pub enum Poll {
    /// Still building.
    Building,
    /// Built, in this long on the worker.
    Built(Box<Renderer>, Duration),
    /// The worker died (a panic, already in run.log): nothing will come.
    Died,
}

impl Worker {
    /// Start building on a worker thread from `inputs`. `polite`: beside the pages (the module docs).
    #[cfg(any(feature = "native", feature = "wasm"))]
    pub fn spawn(inputs: WorldInputs, polite: bool) -> Worker {
        set_polite(polite);
        let progress = Arc::new(Progress::default());
        let (tx, rx) = std::sync::mpsc::channel();
        let p = progress.clone();
        // The gate closes HERE, on the frame, before the worker or the star sky's thread (started
        // right after, engine/world_prep.rs) exists, so no creation of theirs runs ahead of it.
        let mut inputs = inputs;
        inputs.gate = Some(compiles_running());
        let spawned = std::thread::Builder::new().name("world-build".into()).spawn(move || {
            build_thread();
            let mut inputs = inputs;
            // The bakes start here when the device did not start them (a start that left the world
            // for later): inside the worker, so they are polite too.
            if inputs.bakes.is_none() {
                inputs.bakes = Some(super::pages_gpu::start_bakes(inputs.background_bakes));
            }
            let r = Renderer::build_world_detached(inputs, &p);
            let _ = tx.send(Box::new(r));
        });
        if let Err(e) = spawned {
            log::error!("[WorldPrep] could not start the world-build thread: {e}");
        }
        Worker { rx, progress, started: Instant::now() }
    }

    /// The build's progress.
    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    /// How long it has been building.
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }

    /// The finished renderer if it is ready; never waits.
    pub fn poll(&self) -> Poll {
        match self.rx.try_recv() {
            Ok(r) => Poll::Built(r, self.started.elapsed()),
            Err(TryRecvError::Empty) => Poll::Building,
            Err(TryRecvError::Disconnected) => Poll::Died,
        }
    }

    /// Wait for the finished renderer (None if the worker died).
    pub fn wait(&self) -> Option<(Box<Renderer>, Duration)> {
        self.rx.recv().ok().map(|r| (r, self.started.elapsed()))
    }
}

/// The Effects step's six screen passes (bloom, the god rays, the sun's visibility, SSAO, the cloud
/// composite and the cloud resolve), BUILT SIDE BY SIDE, each on its own compile thread
/// (2026-11-21). They used to be made one after another on the worker, 1.8 to 2.4 s of serial
/// pipeline compiles; side by side the step takes 0.4 to 0.5 s (`just verify-phone-start`, run
/// 20261121-115841). Each is independent of the others and takes only the device, so nothing but
/// where the code sat made them serial (the v0.1322 lesson of the cloud screen pipelines,
/// renderer/pipeline.rs). Tried first as the fix for BUG-338's third cause, the one long page frame
/// of every build, which fell while they compiled: it did not move it (849 and 440 ms in that run,
/// now in the pipeline step), so the cause was what ran beside them ([`compiles_running`]). Their
/// threads are the compile threads of the pipeline step (`spawn_compile`: the big stack, off the
/// frame's core, polite beside the pages).
#[cfg(any(feature = "native", feature = "wasm"))]
pub struct EffectPasses {
    pub bloom: super::bloom::BloomPass,
    pub godray: super::godrays::GodrayPass,
    pub sun_vis: super::sun_glare::SunVisPass,
    pub ssao: super::ssao::SsaoPass,
    pub cloud_composite: super::cloud_composite::CloudCompositePass,
    pub cloud_resolve: super::cloud_resolve::CloudResolvePass,
}

#[cfg(any(feature = "native", feature = "wasm"))]
impl EffectPasses {
    /// The six passes, side by side (the type's notes); each one's time and the whole step's are
    /// logged (`[BootPhase]`), so the run's log lays them against the page frames' long-gap lines.
    pub fn build(device: &wgpu::Device, width: u32, height: u32, scene_format: wgpu::TextureFormat) -> Self {
        use super::pipeline::spawn_compile;
        let t = Instant::now();
        let fx = std::thread::scope(|s| {
            let bloom = spawn_compile(s, "bloom", || timed_pass("bloom_pass", || super::bloom::BloomPass::new(device, width, height, scene_format)));
            let godray = spawn_compile(s, "godrays", || timed_pass("godray_pass", || super::godrays::GodrayPass::new(device, scene_format)));
            let sun_vis = spawn_compile(s, "sun visibility", || timed_pass("sun_vis_pass", || super::sun_glare::SunVisPass::new(device)));
            let ssao = spawn_compile(s, "ssao", || timed_pass("ssao_pass", || super::ssao::SsaoPass::new(device, scene_format)));
            let cloud_composite = spawn_compile(s, "cloud composite", || {
                timed_pass("cloud_composite_pass", || super::cloud_composite::CloudCompositePass::new(device, scene_format))
            });
            let cloud_resolve = spawn_compile(s, "cloud resolve", || timed_pass("cloud_resolve_pass", || super::cloud_resolve::CloudResolvePass::new(device)));
            EffectPasses {
                bloom: joined(bloom, "bloom"),
                godray: joined(godray, "god ray"),
                sun_vis: joined(sun_vis, "sun visibility"),
                ssao: joined(ssao, "SSAO"),
                cloud_composite: joined(cloud_composite, "cloud composite"),
                cloud_resolve: joined(cloud_resolve, "cloud resolve"),
            }
        });
        log::info!("[BootPhase]   effect_passes: {:.0} ms, the six side by side", t.elapsed().as_secs_f64() * 1000.0);
        fx
    }
}

/// `f`, its time logged as one pass of the Effects step.
#[cfg(any(feature = "native", feature = "wasm"))]
fn timed_pass<T>(name: &str, f: impl FnOnce() -> T) -> T {
    let t = Instant::now();
    let r = f();
    log::info!("[BootPhase]     {name}: {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    r
}

/// A pass's thread's result; its panic, already in run.log, ends the build the way it did on the
/// worker.
#[cfg(any(feature = "native", feature = "wasm"))]
fn joined<T>(h: std::thread::ScopedJoinHandle<'_, T>, what: &str) -> T {
    h.join().unwrap_or_else(|_| panic!("the {what} pass's build panicked"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The frame keeps the lowest one or two processors of those the app may use (the share guard's
    /// rig affinity leaves out the last core, so the mask need not start at bit 0 or be whole).
    /// Seen RED 2026-11-21 with `without_lowest` clearing only the lowest processor whatever `n`
    /// (`m &= !1;` in its loop): "twelve processors: off the first two" (left 4094, right 4092).
    #[test]
    fn the_build_leaves_the_frame_its_first_core() {
        assert_eq!(reserved_for_the_frame(12), 2);
        assert_eq!(reserved_for_the_frame(4), 1);
        assert_eq!(reserved_for_the_frame(2), 0);
        assert_eq!(without_lowest(0b1111_1111_1111, 2), 0b1111_1111_1100, "twelve processors: off the first two");
        assert_eq!(without_lowest(0b0111_1111_1111, 2), 0b0111_1111_1100, "the share guard's mask without its last core");
        assert_eq!(without_lowest(0b1010_1100, 2), 0b1010_0000, "a mask with gaps: its two lowest set bits");
        assert_eq!(without_lowest(0b11, reserved_for_the_frame(2)), 0b11, "two processors: the build keeps both");
    }

    /// An upload goes up in bands of rows under the cap, covering every row once, in order: the
    /// 8192 by 4096 glow (32 KiB a row) in sixteen bands of 256 rows. Seen RED 2026-11-21 with one
    /// row too many a band (`cap / bytes_per_row + 1` in `row_bands`): the cap's assertion failed
    /// (still sixteen bands, fifteen of them 257 rows, over 8 MiB).
    #[test]
    fn a_big_upload_goes_up_in_bands_under_the_cap() {
        let bands = row_bands(4 * 8192, 4096, UPLOAD_SLICE_BYTES);
        assert_eq!(bands.len(), 16);
        assert!(bands.iter().all(|&(_, r)| r as usize * 4 * 8192 <= UPLOAD_SLICE_BYTES));
        for (h, row, cap) in [(1u32, 4u32, 1usize), (7, 100, 250), (4096, 4 * 8192, UPLOAD_SLICE_BYTES), (3, 1 << 30, 16)] {
            let b = row_bands(row, h, cap);
            let mut next = 0;
            for &(y, rows) in &b {
                assert_eq!(y, next, "bands in order without gaps: {b:?}");
                assert!(rows >= 1);
                next += rows;
            }
            assert_eq!(next, h, "every row once: {b:?}");
        }
    }

    /// THE COMPILES' GATE: a thread of the preparation waits while the pipelines compile and goes on
    /// once they are done, the frame's thread (here, the test's) never waits, and a build that
    /// panics while compiling opens it. Seen RED 2026-11-21 with the gate broken (`while *n > 0 &&
    /// false {` in `wait_for_the_compiles`): "a thread of the preparation went ahead while the
    /// pipelines compiled", the same again run alone.
    #[test]
    fn the_preparation_waits_for_the_compiles_and_the_frame_never_does() {
        let guard = compiles_running();
        assert!(wait_for_the_compiles() < Duration::from_secs(1), "the frame's thread waited on the gate");
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let t = std::thread::spawn(move || {
            build_thread();
            ready_tx.send(()).unwrap();
            wait_for_the_compiles();
            done_tx.send(()).unwrap();
        });
        ready_rx.recv().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        assert!(done_rx.try_recv().is_err(), "a thread of the preparation went ahead while the pipelines compiled");
        drop(guard);
        done_rx.recv_timeout(Duration::from_secs(30)).expect("the gate opened when the compiles ended");
        t.join().unwrap();
        let _ = std::panic::catch_unwind(|| {
            let _compiling = compiles_running();
            panic!("a build that dies while compiling");
        });
        let t = std::thread::spawn(|| {
            build_thread();
            wait_for_the_compiles();
        });
        let start = Instant::now();
        while !t.is_finished() && start.elapsed() < Duration::from_secs(30) {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(t.is_finished(), "a panic while compiling left the gate shut");
    }

    #[test]
    fn the_steps_count_one_to_eight_in_order() {
        assert_eq!(Step::Waiting.number(), 0);
        assert_eq!(Step::Targets.number(), 1);
        assert_eq!(Step::Sky.number(), Step::WORK);
        assert_eq!(Step::Done.number(), Step::WORK);
        for (i, s) in Step::ALL.iter().enumerate() {
            assert_eq!(*s as usize, i, "{s:?} out of order");
            assert_eq!(Step::from_u8(i as u8), *s);
        }
        let p = Progress::default();
        assert_eq!(p.step(), Step::Waiting);
        p.mark(Step::Clouds);
        assert_eq!(p.step(), Step::Clouds);
    }
}
