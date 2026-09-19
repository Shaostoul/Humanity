//! `cef-probe`: an offscreen browser, measured.
//!
//! This program is the spike's instrument. It starts the Chromium Embedded
//! Framework in "windowless" mode, which means Chromium renders a web page
//! into a block of memory instead of into a window on screen, loads a URL at a
//! chosen size, and then counts everything worth counting:
//!
//!   * how many finished frames Chromium actually handed us, and how evenly;
//!   * how long it takes to copy each frame out of Chromium's buffer;
//!   * how long the red/blue channel swap costs (Chromium delivers BGRA, the
//!     engine's screen textures are RGBA, so somebody has to pay for that
//!     unless we give the screen a BGRA texture instead);
//!   * how long it takes to publish the frame into shared memory for another
//!     process to pick up;
//!   * whether the page loaded, what it said on the console, and what its
//!     title ended up as, which is how the platform-embed tests report.
//!
//! It also writes a PNG so a human can see that the pixels are real and the
//! right way up, and a JSON file with every number in it.
//!
//! Nothing here is part of the game. This crate is not in the engine's
//! Cargo.toml and the engine does not reference it.
//!
//! Usage (every argument is optional):
//!
//! ```text
//! cef-probe --url=file:///C:/.../anim.html --width=1280 --height=720 \
//!           --fps=60 --seconds=30 --out=C:/.../runs/local \
//!           --shm=humanity_cef_probe --label="local animated page"
//! ```

// The shared-memory ring lives in one file that both this crate and the
// consumer (tools/frame-sink) include, so the two can never disagree about
// the layout.
#[path = "../../frame_shm.rs"]
mod frame_shm;

mod proc_stats;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cef::rc::Rc as _;
use cef::*;

/// Everything the CEF callbacks record. The callbacks are handed `&self`, and
/// CEF may call them from its own thread, so every field is either an atomic
/// or behind a mutex.
struct Shared {
    width: u32,
    height: u32,
    /// Complete VIEW repaints received.
    frames: AtomicU64,
    /// Repaints of a popup widget (a <select> dropdown). Counted, not used.
    popup_paints: AtomicU64,
    /// Performance-counter tick of every VIEW repaint, so the run can report
    /// not just an average rate but how evenly the frames arrived.
    stamps: Mutex<Vec<u64>>,
    /// The most recent frame, as Chromium delivered it (BGRA) or after the
    /// swap (RGBA), depending on `swizzle`.
    latest: Mutex<Vec<u8>>,
    /// Nanoseconds spent copying frames out of Chromium's buffer.
    copy_ns: AtomicU64,
    /// Nanoseconds spent swapping red and blue.
    swizzle_ns: AtomicU64,
    /// Nanoseconds spent writing frames into shared memory.
    publish_ns: AtomicU64,
    /// Whether to swap red and blue after copying.
    swizzle: bool,
    /// The shared-memory ring, when the run was asked to publish.
    ring: Mutex<Option<frame_shm::FrameRing>>,
    /// Page reporting, for the platform-embed tests.
    console: Mutex<Vec<String>>,
    loads: Mutex<Vec<String>>,
    load_errors: Mutex<Vec<String>>,
    title: Mutex<String>,
    address: Mutex<String>,
    /// Set once the main frame finished loading, so a run can wait for it.
    main_frame_loaded: AtomicBool,
}

impl Shared {
    fn new(width: u32, height: u32, swizzle: bool) -> Self {
        Self {
            width,
            height,
            frames: AtomicU64::new(0),
            popup_paints: AtomicU64::new(0),
            stamps: Mutex::new(Vec::with_capacity(8192)),
            latest: Mutex::new(Vec::new()),
            copy_ns: AtomicU64::new(0),
            swizzle_ns: AtomicU64::new(0),
            publish_ns: AtomicU64::new(0),
            swizzle,
            ring: Mutex::new(None),
            console: Mutex::new(Vec::new()),
            loads: Mutex::new(Vec::new()),
            load_errors: Mutex::new(Vec::new()),
            title: Mutex::new(String::new()),
            address: Mutex::new(String::new()),
            main_frame_loaded: AtomicBool::new(false),
        }
    }
}

// ---------------------------------------------------------------------------
// The render handler: this is where the pixels arrive.
// ---------------------------------------------------------------------------

cef::wrap_render_handler! {
    struct ProbeRenderHandler {
        state: Arc<Shared>,
    }

    impl RenderHandler {
        /// Chromium asks how big the page is. Answering with a fixed size is
        /// what makes this a 1280x720 render.
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(r) = rect {
                r.x = 0;
                r.y = 0;
                r.width = self.state.width as i32;
                r.height = self.state.height as i32;
            }
        }

        /// One finished frame. `buffer` is Chromium's own memory and is only
        /// valid for the length of this call, so the frame has to be copied
        /// out before we return. That copy is one of the costs the spike is
        /// here to measure.
        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: ::std::os::raw::c_int,
            height: ::std::os::raw::c_int,
        ) {
            if type_.get_raw() != PaintElementType::VIEW.get_raw() {
                self.state.popup_paints.fetch_add(1, Ordering::Relaxed);
                return;
            }
            if buffer.is_null() || width <= 0 || height <= 0 {
                return;
            }
            let n = width as usize * height as usize * 4;

            let t0 = Instant::now();
            let mut latest = self.state.latest.lock().unwrap();
            if latest.len() != n {
                latest.resize(n, 0);
            }
            // SAFETY: CEF guarantees `buffer` points at width*height*4 bytes
            // for the duration of this callback.
            unsafe {
                std::ptr::copy_nonoverlapping(buffer, latest.as_mut_ptr(), n);
            }
            self.state.copy_ns.fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);

            if self.state.swizzle {
                let t1 = Instant::now();
                swap_red_and_blue(&mut latest);
                self.state.swizzle_ns.fetch_add(t1.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }

            if let Some(ring) = self.state.ring.lock().unwrap().as_mut() {
                let t2 = Instant::now();
                ring.publish(&latest);
                self.state.publish_ns.fetch_add(t2.elapsed().as_nanos() as u64, Ordering::Relaxed);
            }

            self.state.frames.fetch_add(1, Ordering::Relaxed);
            self.state.stamps.lock().unwrap().push(frame_shm::qpc());
        }
    }
}

/// Chromium hands over BGRA; the engine's screen textures are RGBA. Swapping
/// the first and third byte of every pixel converts between them.
fn swap_red_and_blue(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px.swap(0, 2);
    }
}

// ---------------------------------------------------------------------------
// The load handler: did the page actually load, or did it refuse?
// ---------------------------------------------------------------------------

cef::wrap_load_handler! {
    struct ProbeLoadHandler {
        state: Arc<Shared>,
    }

    impl LoadHandler {
        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            status: ::std::os::raw::c_int,
        ) {
            let is_main = frame.as_ref().map(|f| f.is_main() != 0).unwrap_or(false);
            // `Frame::url` hands back a string CEF allocated; converting it to
            // the owned wrapper copies it and lets CEF free the original.
            let url = frame
                .map(|f| CefString::from(&f.url()).to_string())
                .unwrap_or_default();
            self.state
                .loads
                .lock()
                .unwrap()
                .push(format!("{} http={} {}", if is_main { "main" } else { "sub" }, status, url));
            if is_main {
                self.state.main_frame_loaded.store(true, Ordering::Release);
            }
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let is_main = frame.as_ref().map(|f| f.is_main() != 0).unwrap_or(false);
            self.state.load_errors.lock().unwrap().push(format!(
                "{} code={} {} url={}",
                if is_main { "main" } else { "sub" },
                error_code.get_raw(),
                error_text.map(|s| s.to_string()).unwrap_or_default(),
                failed_url.map(|s| s.to_string()).unwrap_or_default(),
            ));
        }
    }
}

// ---------------------------------------------------------------------------
// The display handler: console messages and the page title. For the embed
// tests, what the page prints and what it calls itself IS the result.
// ---------------------------------------------------------------------------

cef::wrap_display_handler! {
    struct ProbeDisplayHandler {
        state: Arc<Shared>,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let Some(t) = title {
                *self.state.title.lock().unwrap() = t.to_string();
            }
        }

        fn on_address_change(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            url: Option<&CefString>,
        ) {
            if let Some(u) = url {
                *self.state.address.lock().unwrap() = u.to_string();
            }
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            source: Option<&CefString>,
            line: ::std::os::raw::c_int,
        ) -> ::std::os::raw::c_int {
            let mut log = self.state.console.lock().unwrap();
            // A page that refuses an embed can log a great deal; keep the
            // first few hundred lines, which is far more than enough.
            if log.len() < 400 {
                log.push(format!(
                    "{} ({}:{})",
                    message.map(|s| s.to_string()).unwrap_or_default(),
                    source.map(|s| s.to_string()).unwrap_or_default(),
                    line
                ));
            }
            0 // 0 = also let Chromium handle it normally
        }
    }
}

// ---------------------------------------------------------------------------
// The client: it just hands Chromium the three handlers above.
// ---------------------------------------------------------------------------

cef::wrap_client! {
    struct ProbeClient {
        render: RenderHandler,
        load: LoadHandler,
        display: DisplayHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render.clone())
        }
        fn load_handler(&self) -> Option<LoadHandler> {
            Some(self.load.clone())
        }
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display.clone())
        }
    }
}

// ---------------------------------------------------------------------------
// The app: its one job is to pass extra Chromium command-line switches, which
// is how the frame-rate and compositing knobs get set.
// ---------------------------------------------------------------------------

cef::wrap_app! {
    struct ProbeApp {
        switches: Arc<Vec<(String, String)>>,
    }

    impl App {
        fn on_before_command_line_processing(
            &self,
            _process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            if let Some(cl) = command_line {
                for (k, v) in self.switches.iter() {
                    if v.is_empty() {
                        cl.append_switch(Some(&CefString::from(k.as_str())));
                    } else {
                        cl.append_switch_with_value(
                            Some(&CefString::from(k.as_str())),
                            Some(&CefString::from(v.as_str())),
                        );
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------

fn main() {
    // Modern CEF versions its C interface, and every process (this one and
    // every child Chromium launches) has to say which version it was built
    // against before it touches anything else. Miss this and the very first
    // object handed to CEF is rejected with "called with invalid version -1".
    let _ = cef::api_hash(cef::sys::CEF_API_VERSION_LAST, 0);

    // A CEF application is really several processes: one browser process (this
    // one) plus a renderer, a GPU process and helpers. Chromium starts those
    // by launching THIS SAME executable again with different arguments.
    // `execute_process` recognises such a relaunch, runs it to completion and
    // returns its exit code; in the first, real launch it returns -1. So this
    // must be the very first thing main does.
    let switches = Arc::new(collect_switches());
    let mut app = ProbeApp::new(switches.clone());
    let main_args = main_args();
    let code = cef::execute_process(Some(&main_args), Some(&mut app), std::ptr::null_mut());
    if code >= 0 {
        std::process::exit(code);
    }

    let args = Args::parse();
    println!("[probe] url        = {}", args.url);
    println!("[probe] size       = {}x{}", args.width, args.height);
    println!("[probe] frame rate = {} (requested)", args.fps);
    println!("[probe] duration   = {} s", args.seconds);

    let state = Arc::new(Shared::new(args.width, args.height, args.swizzle));

    // Open the shared-memory ring before any frame can arrive.
    if let Some(name) = args.shm.as_deref() {
        match frame_shm::FrameRing::create(name, args.width, args.height, args.slots) {
            Ok(ring) => {
                println!(
                    "[probe] shared memory '{name}': {} slots, {} bytes total",
                    args.slots,
                    ring.total_bytes()
                );
                *state.ring.lock().unwrap() = Some(ring);
            }
            Err(e) => eprintln!("[probe] shared memory failed: {e}"),
        }
    }

    let mut settings = Settings {
        no_sandbox: 1,
        windowless_rendering_enabled: 1,
        // Chromium must be told where its own resources live, because the
        // executable is in target/release and the CEF distribution is not.
        resources_dir_path: CefString::from(args.cef_path.as_str()),
        locales_dir_path: CefString::from(format!("{}/locales", args.cef_path).as_str()),
        ..Default::default()
    };
    if !args.log_file.is_empty() {
        settings.log_file = CefString::from(args.log_file.as_str());
    }
    if !args.cache_path.is_empty() {
        settings.root_cache_path = CefString::from(args.cache_path.as_str());
        settings.cache_path = CefString::from(args.cache_path.as_str());
    }

    let rc = cef::initialize(Some(&main_args), Some(&settings), Some(&mut app), std::ptr::null_mut());
    if rc != 1 {
        eprintln!("[probe] cef_initialize returned {rc} (1 means success); giving up");
        std::process::exit(2);
    }
    println!("[probe] cef initialised, api version {}", cef::api_version());

    // A null parent window: there is no window at all, which is the point of
    // windowless rendering.
    let window_info = WindowInfo::default().set_as_windowless(cef::sys::HWND::default());
    let browser_settings = BrowserSettings {
        windowless_frame_rate: args.fps as i32,
        // An opaque background switches off per-pixel transparency, which is
        // what a wall screen wants and is a little cheaper.
        background_color: 0xFF00_0000,
        ..Default::default()
    };

    let mut client = ProbeClient::new(
        ProbeRenderHandler::new(state.clone()),
        ProbeLoadHandler::new(state.clone()),
        ProbeDisplayHandler::new(state.clone()),
    );

    let browser = cef::browser_host_create_browser_sync(
        Some(&window_info),
        Some(&mut client),
        Some(&CefString::from(args.url.as_str())),
        Some(&browser_settings),
        None,
        None,
    );
    let Some(browser) = browser else {
        eprintln!("[probe] could not create the browser");
        std::process::exit(3);
    };
    if let Some(host) = browser.host() {
        host.set_windowless_frame_rate(args.fps as i32);
    }

    // Warm-up. Frames painted during it do not count, because a page that is
    // still fetching its scripts is not the thing being measured.
    //
    // It ends at whichever comes first:
    //   * the page has finished loading AND `settle` seconds have passed
    //     (loading is not the same as running: a video player reports its
    //     page loaded well before it has started playing), or
    //   * `warmup` seconds have passed regardless, so a page that never
    //     finishes loading cannot stall the run forever.
    let warmup_start = Instant::now();
    let warmup_deadline = warmup_start + Duration::from_secs(args.warmup);
    let settled_at = warmup_start + Duration::from_secs(args.settle);
    while Instant::now() < warmup_deadline {
        pump();
        if state.main_frame_loaded.load(Ordering::Acquire) && Instant::now() > settled_at {
            break;
        }
    }

    // ---- the measured window ----
    let frames_before = state.frames.load(Ordering::Relaxed);
    let copy_before = state.copy_ns.load(Ordering::Relaxed);
    let swizzle_before = state.swizzle_ns.load(Ordering::Relaxed);
    let publish_before = state.publish_ns.load(Ordering::Relaxed);
    let stamps_before = state.stamps.lock().unwrap().len();
    let cpu_before = proc_stats::sample_tree();
    let t_start = Instant::now();

    let measure_until = t_start + Duration::from_secs(args.seconds);
    let mut png_written = false;
    let mut clicked = args.click_at == 0;
    while Instant::now() < measure_until {
        pump();
        if !png_written && t_start.elapsed() >= Duration::from_secs(args.png_at) {
            write_png(&state, &args, "frame.png");
            png_written = true;
        }
        // A synthetic click in the middle of the page. This is exactly what
        // the game does when the player looks at a wall screen and clicks: an
        // (x, y) in page pixels handed to the browser. It is here so a player
        // that will not autoplay can still be started, and so that input
        // routing is proven rather than assumed.
        if !clicked && t_start.elapsed() >= Duration::from_secs(args.click_at) {
            clicked = true;
            if let Some(host) = browser.host() {
                let at = MouseEvent {
                    x: (args.width / 2) as i32,
                    y: (args.height / 2) as i32,
                    modifiers: 0,
                };
                host.send_mouse_move_event(Some(&at), 0);
                host.send_mouse_click_event(Some(&at), MouseButtonType::LEFT, 0, 1);
                host.send_mouse_click_event(Some(&at), MouseButtonType::LEFT, 1, 1);
                println!("[probe] clicked at ({}, {})", at.x, at.y);
            }
        }
    }

    let elapsed = t_start.elapsed();
    let cpu_after = proc_stats::sample_tree();
    let frames = state.frames.load(Ordering::Relaxed) - frames_before;
    let copy_ns = state.copy_ns.load(Ordering::Relaxed) - copy_before;
    let swizzle_ns = state.swizzle_ns.load(Ordering::Relaxed) - swizzle_before;
    let publish_ns = state.publish_ns.load(Ordering::Relaxed) - publish_before;

    // A second PNG at the end, so a moving page can be shown to have moved.
    write_png(&state, &args, "frame_end.png");

    // Frame-to-frame intervals over the measured window.
    let qpf = frame_shm::qpf() as f64;
    let intervals_ms: Vec<f64> = {
        let stamps = state.stamps.lock().unwrap();
        stamps[stamps_before.min(stamps.len())..]
            .windows(2)
            .map(|w| (w[1] - w[0]) as f64 * 1000.0 / qpf)
            .collect()
    };
    let (p50, p95, p99, worst) = percentiles(&intervals_ms);

    let fps = frames as f64 / elapsed.as_secs_f64();
    let cpu_seconds = (cpu_after.cpu_100ns.saturating_sub(cpu_before.cpu_100ns)) as f64 / 1e7;
    let cpu_percent_of_one_core = cpu_seconds / elapsed.as_secs_f64() * 100.0;

    println!();
    println!("================ RESULT: {} ================", args.label);
    println!("measured window        {:.2} s", elapsed.as_secs_f64());
    println!("frames delivered       {frames}");
    println!("sustained rate         {fps:.2} fps");
    println!("frame interval ms      p50 {p50:.2}  p95 {p95:.2}  p99 {p99:.2}  worst {worst:.2}");
    println!(
        "copy out of Chromium   {:.3} ms/frame  (total {:.1} ms)",
        ms_per(copy_ns, frames),
        copy_ns as f64 / 1e6
    );
    println!(
        "BGRA to RGBA swap      {:.3} ms/frame  (total {:.1} ms){}",
        ms_per(swizzle_ns, frames),
        swizzle_ns as f64 / 1e6,
        if args.swizzle { "" } else { "  [not performed this run]" }
    );
    println!(
        "publish to shared mem  {:.3} ms/frame  (total {:.1} ms)",
        ms_per(publish_ns, frames),
        publish_ns as f64 / 1e6
    );
    println!(
        "host CPU (all {} processes) {:.1}% of one core, {:.2} core-seconds",
        cpu_after.process_count, cpu_percent_of_one_core, cpu_seconds
    );
    println!(
        "host memory            {:.1} MB working set, {:.1} MB private",
        cpu_after.working_set as f64 / 1048576.0,
        cpu_after.private as f64 / 1048576.0
    );
    println!("page title             {:?}", state.title.lock().unwrap());
    println!("final address          {:?}", state.address.lock().unwrap());
    let loads = state.loads.lock().unwrap().clone();
    let errors = state.load_errors.lock().unwrap().clone();
    let console = state.console.lock().unwrap().clone();
    println!("loads                  {} ({} errors)", loads.len(), errors.len());
    for e in errors.iter().take(20) {
        println!("  load error: {e}");
    }
    for c in console.iter().take(40) {
        println!("  console: {c}");
    }

    write_json(
        &args,
        &JsonResult {
            label: args.label.clone(),
            url: args.url.clone(),
            width: args.width,
            height: args.height,
            requested_fps: args.fps,
            seconds: elapsed.as_secs_f64(),
            frames,
            fps,
            p50_ms: p50,
            p95_ms: p95,
            p99_ms: p99,
            worst_ms: worst,
            copy_ms_per_frame: ms_per(copy_ns, frames),
            swizzle_ms_per_frame: ms_per(swizzle_ns, frames),
            publish_ms_per_frame: ms_per(publish_ns, frames),
            swizzle_performed: args.swizzle,
            cpu_percent_of_one_core,
            cpu_core_seconds: cpu_seconds,
            process_count: cpu_after.process_count,
            working_set_bytes: cpu_after.working_set,
            private_bytes: cpu_after.private,
            title: state.title.lock().unwrap().clone(),
            address: state.address.lock().unwrap().clone(),
            popup_paints: state.popup_paints.load(Ordering::Relaxed),
            loads,
            load_errors: errors,
            console,
        },
    );

    // Shut down tidily so the child processes go away.
    if let Some(host) = browser.host() {
        host.close_browser(1);
    }
    drop(browser);
    let close_deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < close_deadline {
        pump();
    }
    cef::shutdown();
    println!("[probe] done");
}

/// One turn of Chromium's message loop, then a short sleep. 4 ms means the
/// loop is serviced 250 times a second, comfortably more often than a 60 fps
/// page produces frames, without a spin loop that would itself burn a core
/// and spoil the CPU measurement.
fn pump() {
    cef::do_message_loop_work();
    std::thread::sleep(Duration::from_millis(4));
}

fn ms_per(total_ns: u64, frames: u64) -> f64 {
    if frames == 0 {
        0.0
    } else {
        total_ns as f64 / frames as f64 / 1e6
    }
}

fn percentiles(v: &[f64]) -> (f64, f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |f: f64| s[((s.len() as f64 - 1.0) * f).round() as usize];
    (at(0.50), at(0.95), at(0.99), *s.last().unwrap())
}

/// Save the most recent frame. The buffer is RGBA when the run swizzled and
/// BGRA when it did not; a PNG is RGB, so the un-swizzled case is converted
/// here purely for the picture.
fn write_png(state: &Shared, args: &Args, name: &str) {
    let latest = state.latest.lock().unwrap();
    if latest.is_empty() {
        eprintln!("[probe] no frame yet, no {name} written");
        return;
    }
    let mut rgba = latest.clone();
    drop(latest);
    if !args.swizzle {
        swap_red_and_blue(&mut rgba);
    }
    let path = std::path::Path::new(&args.out).join(name);
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let file = match std::fs::File::create(&path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("[probe] cannot write {}: {e}", path.display());
            return;
        }
    };
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), args.width, args.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    match enc.write_header().and_then(|mut w| w.write_image_data(&rgba)) {
        Ok(()) => println!("[probe] wrote {}", path.display()),
        Err(e) => eprintln!("[probe] PNG encode failed: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Result file. Hand-rolled JSON so the probe needs no serialisation crate.
// ---------------------------------------------------------------------------

struct JsonResult {
    label: String,
    url: String,
    width: u32,
    height: u32,
    requested_fps: u32,
    seconds: f64,
    frames: u64,
    fps: f64,
    p50_ms: f64,
    p95_ms: f64,
    p99_ms: f64,
    worst_ms: f64,
    copy_ms_per_frame: f64,
    swizzle_ms_per_frame: f64,
    publish_ms_per_frame: f64,
    swizzle_performed: bool,
    cpu_percent_of_one_core: f64,
    cpu_core_seconds: f64,
    process_count: u32,
    working_set_bytes: u64,
    private_bytes: u64,
    title: String,
    address: String,
    popup_paints: u64,
    loads: Vec<String>,
    load_errors: Vec<String>,
    console: Vec<String>,
}

fn write_json(args: &Args, r: &JsonResult) {
    let path = std::path::Path::new(&args.out).join("result.json");
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let arr = |v: &[String]| -> String {
        let items: Vec<String> = v.iter().map(|s| json_string(s)).collect();
        format!("[{}]", items.join(","))
    };
    let body = format!(
        concat!(
            "{{\n",
            "  \"label\": {},\n  \"url\": {},\n  \"width\": {},\n  \"height\": {},\n",
            "  \"requested_fps\": {},\n  \"seconds\": {:.3},\n  \"frames\": {},\n  \"fps\": {:.3},\n",
            "  \"frame_interval_ms\": {{ \"p50\": {:.3}, \"p95\": {:.3}, \"p99\": {:.3}, \"worst\": {:.3} }},\n",
            "  \"copy_ms_per_frame\": {:.4},\n  \"swizzle_ms_per_frame\": {:.4},\n",
            "  \"publish_ms_per_frame\": {:.4},\n  \"swizzle_performed\": {},\n",
            "  \"cpu_percent_of_one_core\": {:.2},\n  \"cpu_core_seconds\": {:.3},\n",
            "  \"process_count\": {},\n  \"working_set_bytes\": {},\n  \"private_bytes\": {},\n",
            "  \"title\": {},\n  \"address\": {},\n  \"popup_paints\": {},\n",
            "  \"loads\": {},\n  \"load_errors\": {},\n  \"console\": {}\n}}\n"
        ),
        json_string(&r.label),
        json_string(&r.url),
        r.width,
        r.height,
        r.requested_fps,
        r.seconds,
        r.frames,
        r.fps,
        r.p50_ms,
        r.p95_ms,
        r.p99_ms,
        r.worst_ms,
        r.copy_ms_per_frame,
        r.swizzle_ms_per_frame,
        r.publish_ms_per_frame,
        r.swizzle_performed,
        r.cpu_percent_of_one_core,
        r.cpu_core_seconds,
        r.process_count,
        r.working_set_bytes,
        r.private_bytes,
        json_string(&r.title),
        json_string(&r.address),
        r.popup_paints,
        arr(&r.loads),
        arr(&r.load_errors),
        arr(&r.console),
    );
    match std::fs::write(&path, body) {
        Ok(()) => println!("[probe] wrote {}", path.display()),
        Err(e) => eprintln!("[probe] cannot write {}: {e}", path.display()),
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

struct Args {
    url: String,
    width: u32,
    height: u32,
    fps: u32,
    seconds: u64,
    warmup: u64,
    settle: u64,
    png_at: u64,
    /// Seconds into the measured window at which to click the middle of the
    /// page. 0 means never.
    click_at: u64,
    out: String,
    label: String,
    shm: Option<String>,
    slots: u32,
    swizzle: bool,
    cef_path: String,
    cache_path: String,
    log_file: String,
}

impl Args {
    fn parse() -> Self {
        let mut map: HashMap<String, String> = HashMap::new();
        for a in std::env::args().skip(1) {
            if let Some(rest) = a.strip_prefix("--") {
                let (k, v) = rest.split_once('=').unwrap_or((rest, "1"));
                map.insert(k.to_string(), v.to_string());
            }
        }
        let get = |k: &str, d: &str| map.get(k).cloned().unwrap_or_else(|| d.to_string());
        let cef_path = get(
            "cef-path",
            &std::env::var("CEF_PATH").unwrap_or_else(|_| String::new()),
        );
        Self {
            url: get("url", "about:blank"),
            width: get("width", "1280").parse().unwrap_or(1280),
            height: get("height", "720").parse().unwrap_or(720),
            fps: get("fps", "60").parse().unwrap_or(60),
            seconds: get("seconds", "30").parse().unwrap_or(30),
            warmup: get("warmup", "8").parse().unwrap_or(8),
            settle: get("settle", "4").parse().unwrap_or(4),
            png_at: get("png-at", "2").parse().unwrap_or(2),
            click_at: get("click-at", "0").parse().unwrap_or(0),
            out: get("out", "runs/last"),
            label: get("label", "unnamed"),
            shm: map.get("shm").cloned(),
            slots: get("slots", "4").parse().unwrap_or(4),
            // Default ON: this is the realistic path today, because the
            // engine's screen textures are RGBA.
            swizzle: get("swizzle", "1") != "0",
            cef_path,
            cache_path: get("cache-path", ""),
            log_file: get("log-file", ""),
        }
    }
}

/// Extra Chromium command-line switches, given as `--switch=name` or
/// `--switch=name:value` (a colon, because values often contain `=`).
fn collect_switches() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for a in std::env::args().skip(1) {
        if let Some(rest) = a.strip_prefix("--switch=") {
            match rest.split_once(':') {
                Some((k, v)) => out.push((k.to_string(), v.to_string())),
                None => out.push((rest.to_string(), String::new())),
            }
        }
    }
    out
}

/// Chromium wants the module handle of the running executable on Windows.
fn main_args() -> MainArgs {
    let instance =
        unsafe { windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) };
    MainArgs {
        instance: cef::sys::HINSTANCE(instance as *mut _),
    }
}
