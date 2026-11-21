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
//! starts, the pipeline compiles' included, runs one step below normal priority ([`polite_thread`]),
//! so the frame's thread (normal) always wins a core, and the 320 MiB of cloud volumes go to the
//! graphics card a 64 MiB slab at a time with a pause between ([`Progress::pace_upload`]) rather than
//! in one copy the next page frame would wait behind. Pressing Play ends the politeness: the worker
//! goes back to normal priority at its next step ([`Progress::mark`]) and the remainder runs flat
//! out while the page shows how far along it is; a build started BY Play (the default) is flat out
//! from the start (`polite_thread` says why, measured). A start that builds before the pages (the
//! old order) is neither: nothing draws until it is done.
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

/// Call first in every thread the world's preparation starts: while it is polite, this thread runs
/// one step below normal priority, so the frame's thread always wins a core (the module docs).
///
/// NOT WHEN SOMEONE WAITS (measured 2026-11-20). A build Play is waiting for runs at normal
/// priority: with every thread below normal, one Play build on this PC took 121 s instead of 9 s
/// while other agents' compiles kept the processors busy, because below-normal threads only get
/// what is left over. At normal priority the progress card's frames came at worst 476 ms apart on
/// a busy PC (its frame's thread shares the cores then), a card that keeps moving, never a frozen
/// window. A no-op otherwise, and off Windows. A failure to set it only means the thread shares the
/// cores as before.
pub fn polite_thread() {
    if polite() {
        set_this_threads_priority(THREAD_PRIORITY_BELOW_NORMAL);
    }
}

const THREAD_PRIORITY_BELOW_NORMAL: i32 = -1;
const THREAD_PRIORITY_NORMAL: i32 = 0;

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
        let spawned = std::thread::Builder::new().name("world-build".into()).spawn(move || {
            polite_thread();
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

#[cfg(test)]
mod tests {
    use super::*;

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
