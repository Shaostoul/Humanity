//! THE ONE WAY A TEST GETS A GPU DEVICE (2026-11-14). Test builds only.
//!
//! WHY. On 2026-11-14 the full library test run froze at about 4,140 of 4,440 tests in run after
//! run (five agents and the orchestrator), silent for 15 minutes, while the same tests passed one
//! at a time. A thread dump of a frozen test program (every thread's stack, read with dbghelp)
//! showed a lock-order deadlock inside NVIDIA's OpenGL and Vulkan driver, nvoglv64.dll:
//!   - a "wgpu-hal WGL Instance Thread" (the hidden-window thread wgpu's OpenGL backend starts
//!     for every `wgpu::Instance`) was EXITING because its test had dropped its instance. Windows
//!     runs every loaded DLL's thread-detach while it holds the process's loader lock, and
//!     nvoglv64's was waiting for a lock of the driver's own;
//!   - another test's thread held that driver lock: once in `wglMakeCurrent` (the OpenGL backend
//!     releasing a context), looping until a driver thread exits, once inside the Vulkan driver
//!     loading a DLL (behind OBS's Vulkan layer, graphics-hook64.dll). Both need the loader lock;
//!   - so every other thread that tried to exit or load a DLL waited for the loader lock too:
//!     finished CPU tests (room_probes, billboard_bake, shader_loader), the thread pool, and the
//!     shadow tests' `D3D12CreateDevice`. libtest's main thread waited in `join()` on one of the
//!     finished tests, and a test thread that reports and cannot exit stops every line libtest
//!     prints, its 60-second warnings included (scripts/lib-tests.js, point 5).
//! Reproduced twice before this module: 1 of 3 runs of the `renderer::` tests, and the GPU
//! modules alone (thirteen module filters) on their second run. Neither run needed a game.
//!
//! Each test made `wgpu::Instance::default()`: in a debug build that is EVERY backend (Vulkan,
//! OpenGL and DX12) with the debug layers on, so each test loaded the OpenGL and Vulkan driver,
//! the Vulkan implicit layers installed on the machine (OBS, Steam's overlay and Fossilize,
//! Epic's overlay) and a hidden window thread. The game itself never does any of that: on
//! Windows the renderer uses DX12 only (`Renderer::new_native`, renderer/mod.rs). And twelve tests
//! made and dropped instances and devices side by side, which is how the exits and the loads met.
//!
//! WHAT THIS MODULE DOES.
//!   1. ONE instance and ONE adapter for the whole test program, on the backends the game uses
//!      (DX12 on Windows, Vulkan or Metal elsewhere), made once and never dropped, so no test
//!      loads or unloads a driver, and no window thread is ever made.
//!   2. A TURN: a test holds the GPU from `open` until its `Gpu` is dropped (after its device and
//!      queue), so GPU tests take turns while every CPU test still runs in parallel.
//!   3. TIMEOUTS that turn a hang into a failure naming the test: the adapter and each device are
//!      asked for on one long-lived thread, and a test that waits past `OPEN_WAIT` for one fails
//!      with the call named; every GPU test after it fails at once with the same reason instead of
//!      waiting again. A test that waits past `TURN_WAIT` for its turn fails naming the holder.
//!   4. A WATCHDOG for a test that holds the GPU too long: past `HOLD_WARN` it writes the test's
//!      name straight to the program's stderr (around libtest's capture, so it reaches
//!      scripts/lib-tests.js even when libtest itself prints nothing), and past `HOLD_ABORT` it
//!      stops the test program with that name, because a call into a driver that never returns
//!      cannot be ended any other way. The holder is also written to the file
//!      `HUMANITY_TEST_GPU_TURN` names (lib-tests.js sets it and reads it back on a stall).
//!   5. The operator's GPU game still comes first: `open` asks humanity_core::test_gpu::may_use()
//!      before anything touches the GPU, as every GPU test did before.
//!
//! tests/gpu_test_gate_lint.rs fails when code outside this file (and the renderer's own two
//! production sites) makes a `wgpu::Instance` or calls `request_adapter` or `request_device`.
//!
//! Usage:
//!     let Some(gpu) = crate::renderer::test_gpu::open(Ask::new("my test")) else { return };
//!     let (device, queue) = (&gpu.device, &gpu.queue);

use std::io::Write;
use std::sync::mpsc;
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::ThreadId;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// How long a test may wait for the adapter or for its device.
pub const OPEN_WAIT: Duration = Duration::from_secs(60);
/// How long a test may wait for its turn before it fails, naming the test that holds the GPU.
/// Generous: under a loaded machine a GPU test can take a minute, and a queue of them forms.
pub const TURN_WAIT: Duration = Duration::from_secs(600);
/// How long one test may hold the GPU before the watchdog names it on stderr.
pub const HOLD_WARN: Duration = Duration::from_secs(60);
/// How long one test may hold the GPU before the watchdog stops the test program. Under the 15
/// minutes scripts/lib-tests.js waits on a silent program, so this fires first, with the name.
pub const HOLD_ABORT: Duration = Duration::from_secs(600);
/// The variable naming the file the holder is written to (scripts/lib-tests.js sets it).
pub const TURN_FILE_VAR: &str = "HUMANITY_TEST_GPU_TURN";

/// What a test asks for: its device's label, the features it cannot run without, and its limits.
#[derive(Clone, Debug)]
pub struct Ask {
    pub label: &'static str,
    pub features: wgpu::Features,
    /// `Limits::default()`, or with `adapter_resolution` the adapter's texture sizes on top of it.
    pub adapter_resolution: bool,
}

impl Ask {
    /// No features, the default limits.
    pub fn new(label: &'static str) -> Self {
        Self { label, features: wgpu::Features::empty(), adapter_resolution: false }
    }
    /// Features the test needs; an adapter without them makes `open` say so and return None.
    pub fn features(mut self, features: wgpu::Features) -> Self {
        self.features = features;
        self
    }
    /// `Limits::default().using_resolution(adapter.limits())`, for tests that use big textures.
    pub fn adapter_resolution(mut self) -> Self {
        self.adapter_resolution = true;
        self
    }
}

/// A test's GPU: its own device and queue on the shared adapter, and its turn, given back when this
/// is dropped. Fields drop in order, so the device and queue are gone before the turn is.
pub struct Gpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub info: wgpu::AdapterInfo,
    /// "FXC" or "DXC (<folder>)": the DX12 shader compiler (`HUMANITY_DXC_DIR` picks DXC).
    pub compiler: String,
    _turn: Turn,
}

impl Gpu {
    /// "<adapter> / <backend> / <compiler>", for a test's printed report.
    pub fn describe(&self) -> String {
        format!("{} / {:?} / {}", self.info.name, self.info.backend, self.compiler)
    }
}

/// The test's name: libtest names each test's thread after the test.
fn test_name() -> String {
    std::thread::current().name().unwrap_or("(a test on an unnamed thread)").to_string()
}

/// A device for this test, or None when it must not or cannot run on the GPU (the operator's GPU
/// game is running, the machine has no adapter, or the adapter lacks a feature the test needs: each
/// is printed). Panics, naming the test, when the GPU does not answer in time.
pub fn open(ask: Ask) -> Option<Gpu> {
    if !humanity_core::test_gpu::may_use() {
        return None;
    }
    let test = test_name();
    let turn = take_turn(&test, TURN_WAIT);
    let shared = match shared(&test) {
        Ok(Some(s)) => s,
        Ok(None) => {
            println!("{test}: no GPU adapter on this machine, not run on the GPU (src/renderer/test_gpu.rs)");
            return None;
        }
        Err(why) => panic!("{test}: {why}"),
    };
    let offered = shared.adapter.features();
    if !offered.contains(ask.features) {
        println!(
            "{test}: needs {:?}, which {} does not offer ({:?} missing): not run on the GPU",
            ask.features,
            shared.info.name,
            ask.features - offered
        );
        return None;
    }
    let limits = if ask.adapter_resolution {
        wgpu::Limits::default().using_resolution(shared.adapter.limits())
    } else {
        wgpu::Limits::default()
    };
    let (label, features) = (ask.label, ask.features);
    let asked = on_opener(OPEN_WAIT, move || {
        pollster::block_on(shared.adapter.request_device(
            &wgpu::DeviceDescriptor { label: Some(label), required_features: features, required_limits: limits, ..Default::default() },
            None,
        ))
        .map_err(|e| e.to_string())
    });
    let (device, queue) = match asked {
        Some(Ok(pair)) => pair,
        Some(Err(e)) => panic!("{test}: request_device refused {label:?}: {e}"),
        None => {
            let why = format!(
                "request_device for {label:?} did not return within {} s, asked by {test} (src/renderer/test_gpu.rs)",
                OPEN_WAIT.as_secs()
            );
            *lock(&BROKEN) = Some(why.clone());
            panic!("{test}: {why}");
        }
    };
    // A validation error fails the test that made it, naming it (wgpu's default panics without the name).
    let name = test.clone();
    device.on_uncaptured_error(Box::new(move |e| panic!("wgpu validation error in {name}: {e}")));
    Some(Gpu {
        device,
        queue,
        info: shared.info.clone(),
        compiler: shared.compiler.clone(),
        _turn: turn,
    })
}

// ── The shared instance and adapter ─────────────────────────────────────────────────────────────

struct Shared {
    /// Kept so the adapter's instance lives as long as the program.
    _instance: wgpu::Instance,
    adapter: wgpu::Adapter,
    info: wgpu::AdapterInfo,
    compiler: String,
}

/// Set once the GPU failed to answer: every later GPU test fails with this at once.
static BROKEN: Mutex<Option<String>> = Mutex::new(None);
/// The adapter, once asked for: Some(None) is "this machine has none".
static SHARED: Mutex<Option<Option<&'static Shared>>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The shared adapter (made on the first call, on the opener thread), None when there is none, or
/// why the GPU cannot be used. Called holding the turn, so only one caller ever makes it.
fn shared(test: &str) -> Result<Option<&'static Shared>, String> {
    if let Some(why) = lock(&BROKEN).clone() {
        return Err(format!("not run, the GPU stopped answering earlier: {why}"));
    }
    if let Some(s) = *lock(&SHARED) {
        return Ok(s);
    }
    match on_opener(OPEN_WAIT, make_shared) {
        Some(made) => {
            let made: Option<&'static Shared> = made.map(|s| &*Box::leak(Box::new(s)));
            *lock(&SHARED) = Some(made);
            Ok(made)
        }
        None => {
            let why = format!(
                "making the wgpu instance and asking for an adapter did not return within {} s, first asked by {test} (src/renderer/test_gpu.rs)",
                OPEN_WAIT.as_secs()
            );
            *lock(&BROKEN) = Some(why.clone());
            Err(why)
        }
    }
}

/// The backends the game's renderer uses (`Renderer::new_native`), never OpenGL.
pub fn backends() -> wgpu::Backends {
    if cfg!(target_os = "windows") {
        wgpu::Backends::DX12
    } else {
        wgpu::Backends::VULKAN | wgpu::Backends::METAL
    }
}

fn make_shared() -> Option<Shared> {
    // HUMANITY_DXC_DIR names a folder holding dxcompiler.dll and dxil.dll: the DX12 shader compiler
    // the shipped game uses when they sit beside its exe. Without it, FXC (wgpu's default).
    let dxc = std::env::var("HUMANITY_DXC_DIR")
        .ok()
        .map(std::path::PathBuf::from)
        .filter(|d| d.join("dxcompiler.dll").exists() && d.join("dxil.dll").exists());
    let mut backend_options = wgpu::BackendOptions::default();
    let mut compiler = "FXC".to_string();
    if let Some(d) = dxc {
        backend_options.dx12 = wgpu::Dx12BackendOptions {
            shader_compiler: wgpu::Dx12Compiler::DynamicDxc {
                dxc_path: d.join("dxcompiler.dll").to_string_lossy().into_owned(),
                dxil_path: d.join("dxil.dll").to_string_lossy().into_owned(),
            },
        };
        compiler = format!("DXC ({})", d.display());
    }
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor { backends: backends(), backend_options, ..Default::default() });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;
    let info = adapter.get_info();
    Some(Shared { _instance: instance, adapter, info, compiler })
}

// ── The opener: one thread that never exits asks for the adapter and every device ───────────────

type Job = Box<dyn FnOnce() + Send>;

/// Run `job` on the opener thread and wait at most `wait` for its answer (None: it did not come).
/// The opener lives as long as the program, so a test never waits on a thread that is exiting, and
/// a call that never returns blocks only the opener, never the test asking.
fn on_opener<T: Send + 'static>(wait: Duration, job: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    static OPENER: OnceLock<Mutex<mpsc::Sender<Job>>> = OnceLock::new();
    let sender = OPENER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("test_gpu opener".into())
            .spawn(move || {
                for job in rx {
                    job();
                }
            })
            .expect("start the GPU opener thread");
        Mutex::new(tx)
    });
    let (tx, rx) = mpsc::sync_channel(1);
    lock(sender)
        .send(Box::new(move || {
            let _ = tx.send(job());
        }))
        .ok()?;
    rx.recv_timeout(wait).ok()
}

// ── The turn and its watchdog ────────────────────────────────────────────────────────────────────

struct Holder {
    test: String,
    thread: ThreadId,
    since: Instant,
    warned: bool,
}

static TURN: Mutex<Option<Holder>> = Mutex::new(None);
static FREED: Condvar = Condvar::new();

/// The GPU, held: dropping it gives the GPU to the next test.
pub struct Turn {
    _private: (),
}

impl Drop for Turn {
    fn drop(&mut self) {
        let mut t = lock(&TURN);
        if let Some(h) = t.take() {
            note_turn(&format!("free {} after {} ms", h.test, h.since.elapsed().as_millis()));
        }
        drop(t);
        FREED.notify_all();
    }
}

/// Wait for the GPU (at most `wait`) and hold it. Panics, naming the holder, when the wait runs out,
/// and at once when this test already holds it (it would wait for itself forever).
pub fn take_turn(test: &str, wait: Duration) -> Turn {
    start_watchdog();
    let deadline = Instant::now() + wait;
    let me = std::thread::current().id();
    let mut t = lock(&TURN);
    loop {
        match t.as_ref() {
            None => break,
            Some(h) if h.thread == me => {
                let held = h.test.clone();
                drop(t);
                panic!("{test}: already holds the GPU turn (taken as {held}): one Gpu per test at a time (src/renderer/test_gpu.rs)");
            }
            Some(h) => {
                let now = Instant::now();
                if now >= deadline {
                    let (holder, secs) = (h.test.clone(), h.since.elapsed().as_secs());
                    drop(t);
                    panic!(
                        "{test}: waited {} s for the GPU; {holder} has held it for {secs} s (src/renderer/test_gpu.rs)",
                        wait.as_secs()
                    );
                }
                t = FREED.wait_timeout(t, deadline - now).unwrap_or_else(PoisonError::into_inner).0;
            }
        }
    }
    *t = Some(Holder { test: test.to_string(), thread: me, since: Instant::now(), warned: false });
    drop(t);
    note_turn(&format!("held {test}"));
    Turn { _private: () }
}

/// Write a line around libtest's capture: `eprintln!` from a test (or a thread it started) is
/// captured and shown only if the test fails; the program's stderr itself is not.
fn say(line: &str) {
    let _ = std::io::stderr().lock().write_all(format!("{line}\n").as_bytes());
}

/// Record the turn's state in the file `HUMANITY_TEST_GPU_TURN` names, one line, replaced each time.
fn note_turn(state: &str) {
    if let Ok(path) = std::env::var(TURN_FILE_VAR) {
        let ms = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
        let _ = std::fs::write(path, format!("{ms} {state}\n"));
    }
}

/// One watchdog for the program: every second it looks at the holder.
fn start_watchdog() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new().name("test_gpu watchdog".into()).spawn(|| loop {
            std::thread::sleep(Duration::from_secs(1));
            let mut t = lock(&TURN);
            let Some(h) = t.as_mut() else { continue };
            let held = h.since.elapsed();
            if held >= HOLD_ABORT {
                let line = format!(
                    "GPU TEST HUNG: {} has held the GPU for {} s (the limit is {} s): stopping the test program (src/renderer/test_gpu.rs)",
                    h.test,
                    held.as_secs(),
                    HOLD_ABORT.as_secs()
                );
                say(&line);
                note_turn(&line);
                std::process::abort();
            }
            if held >= HOLD_WARN && !h.warned {
                h.warned = true;
                let line = format!("GPU TEST STILL RUNNING: {} has held the GPU for {} s (src/renderer/test_gpu.rs)", h.test, held.as_secs());
                say(&line);
                note_turn(&format!("held {} (over {} s)", h.test, held.as_secs()));
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THE TURN IS TAKEN ONE TEST AT A TIME, AND A WAIT THAT RUNS OUT NAMES THE HOLDER. Three
    /// threads take the turn and each holds it 40 ms: at no moment do two hold it. Then a thread
    /// holds it while another waits 50 ms: the waiter panics naming the holder.
    ///
    /// Seen red (2026-11-14) with `take_turn` returning at once (no wait): "two tests held the GPU
    /// at once: 2", and with the holder's name left out of the message: "the message names the
    /// holder: ...waited 0 s for the GPU (src/renderer/test_gpu.rs)".
    #[test]
    fn the_turn_is_taken_one_test_at_a_time_and_a_wait_that_runs_out_names_the_holder() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        let inside = Arc::new(AtomicUsize::new(0));
        let most = Arc::new(AtomicUsize::new(0));
        let threads: Vec<_> = (0..3)
            .map(|i| {
                let (inside, most) = (inside.clone(), most.clone());
                std::thread::Builder::new()
                    .name(format!("turn taker {i}"))
                    .spawn(move || {
                        let _turn = take_turn(&format!("turn taker {i}"), Duration::from_secs(30));
                        let now = inside.fetch_add(1, Ordering::SeqCst) + 1;
                        most.fetch_max(now, Ordering::SeqCst);
                        std::thread::sleep(Duration::from_millis(40));
                        inside.fetch_sub(1, Ordering::SeqCst);
                    })
                    .unwrap()
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(most.load(Ordering::SeqCst), 1, "two tests held the GPU at once: {}", most.load(Ordering::SeqCst));

        let (held_tx, held_rx) = mpsc::channel();
        let (go_tx, go_rx) = mpsc::channel::<()>();
        let holder = std::thread::Builder::new()
            .name("the holder".into())
            .spawn(move || {
                let _turn = take_turn("the holder", Duration::from_secs(30));
                held_tx.send(()).unwrap();
                let _ = go_rx.recv_timeout(Duration::from_secs(30));
            })
            .unwrap();
        held_rx.recv().unwrap();
        let waiter = std::thread::Builder::new()
            .name("the waiter".into())
            .spawn(|| {
                let _turn = take_turn("the waiter", Duration::from_millis(50));
            })
            .unwrap();
        let err = waiter.join().expect_err("a wait that ran out must fail the test");
        let msg = err.downcast_ref::<String>().cloned().unwrap_or_default();
        go_tx.send(()).unwrap();
        holder.join().unwrap();
        assert!(msg.contains("the waiter") && msg.contains("the holder has held it"), "the message names the holder: {msg}");
        // And the GPU is free again for the next test.
        let _turn = take_turn("after", Duration::from_secs(5));
    }

    /// A TEST CANNOT WAIT FOR ITSELF: taking the turn twice on one thread fails at once with a
    /// message, instead of waiting out the whole TURN_WAIT. Seen red (2026-11-14) without the
    /// check: the second take waited the 2 s given and failed with "waited 2 s for the GPU".
    #[test]
    fn a_test_cannot_wait_for_itself() {
        let t = std::thread::Builder::new()
            .name("twice".into())
            .spawn(|| {
                let _first = take_turn("twice", Duration::from_secs(30));
                let started = Instant::now();
                let r = std::panic::catch_unwind(|| take_turn("twice", Duration::from_secs(2)));
                (r.is_err(), started.elapsed())
            })
            .unwrap();
        let (failed, took) = t.join().unwrap();
        assert!(failed, "a second take on the same thread must fail");
        assert!(took < Duration::from_secs(1), "it must fail at once, not wait: {took:?}");
    }

    /// THE SHARED ADAPTER IS THE GAME'S BACKEND, NEVER OPENGL, and two GPU tests one after the
    /// other get devices on the same adapter. Seen red (2026-11-14) with `Instance::default()`
    /// in `make_shared`: "the tests' backend is Vulkan, the game's is Dx12" on this machine.
    #[test]
    fn the_tests_use_the_games_backend_and_one_adapter() {
        let Some(first) = open(Ask::new("backend check one")) else { return };
        let (backend, name) = (first.info.backend, first.info.name.clone());
        drop(first);
        let Some(second) = open(Ask::new("backend check two")) else { return };
        assert!(backends().contains(wgpu::Backends::from(backend)), "the tests' backend is {backend:?}, the game's is {:?}", backends());
        assert_ne!(backend, wgpu::Backend::Gl, "never OpenGL: its driver deadlocked the test program");
        assert_eq!((second.info.name.as_str(), second.info.backend), (name.as_str(), backend), "both tests on the same adapter");
    }
}
