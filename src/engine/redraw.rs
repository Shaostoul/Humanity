//! Drawing a frame only when something changed, for the app's pages on a phone (Android rung A1,
//! 2026-11-20; docs/design/platforms.md, "The phone's GPU against the game renderer").
//!
//! WHY. A desktop draws a frame after every frame (the frame-rate caps pace it), which suits the 3D
//! world, where something always moves. A phone shows only the pages, where nothing moves for most
//! of a minute, and every needless frame is battery and heat. So on a phone (and in the phone
//! preview on a desktop) the app is REACTIVE, egui's own model: a frame is drawn when
//!   - the person does something (a touch, a key, the pointer: egui-winit says which events need
//!     one, [`note_event`]), or the window changes size or focus;
//!   - egui asked for one, now or after a delay: an animation running, a spinner, a page's timer
//!     (`ctx.request_repaint_after`), read from each frame's output ([`after_ui`]) and from egui's
//!     repaint callback, which another thread can call too ([`install`]);
//!   - the network brought something: four times a second a tick reads the sockets the way a
//!     window hidden in the tray does (`frame_net::pump`, engine/tray_window.rs), and draws a frame
//!     only when what arrived changed what the pages show ([`net_fingerprint`]);
//!   - a dev request arrived (the request folder's watcher, or the dev MCP endpoint), with a short
//!     burst of frames after it, since some requests take a few frames to finish (a click is a
//!     move, a press and a release).
//! Otherwise the event loop sleeps until the next of those can happen. A desktop keeps drawing
//! every frame, exactly as before.
//!
//! THE COUNT. Every ten seconds the log says how many frames were drawn and how many times the
//! loop woke without drawing (`[Redraw] reactive: ...`, or `continuous` on a desktop), and the first
//! frame says how long after the window opened it came (`[Boot] first frame ...`): the numbers
//! scripts/verify-phone-start.js reads. `HUMANITY_REDRAW_ALWAYS=1` keeps a phone preview drawing
//! every frame, for the before-and-after measurement.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};

use crate::engine::state::EngineState;

/// How often a reactive app reads its sockets (the tray's tick, engine/tray_window.rs).
const TICK: Duration = crate::engine::tray_window::TICK;
/// How long frames keep coming after a dev request (a click takes three frames).
const DEV_BURST: Duration = Duration::from_millis(500);
/// How often the frame count is logged.
const COUNT_WINDOW: Duration = Duration::from_secs(10);

/// A dev request was seen (set from the request folder's watcher thread or the MCP endpoint).
static DEV_REQUEST: AtomicBool = AtomicBool::new(false);
/// The earliest frame egui asked for from its repaint callback (any thread), not yet drawn.
static EGUI_ASKED: Mutex<Option<Instant>> = Mutex::new(None);

/// Draw only on change: a phone, or the phone preview, unless `HUMANITY_REDRAW_ALWAYS=1` (the
/// module docs). Read once.
pub(crate) fn reactive() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| {
        humanity_core::platform::device::phone()
            && !std::env::var("HUMANITY_REDRAW_ALWAYS").map(|v| v == "1").unwrap_or(false)
    })
}

/// The redraw bookkeeping the engine keeps (in `EngineState::redraw`).
#[derive(Default)]
pub(crate) struct Redraw {
    /// An input event asked for a frame.
    wanted: bool,
    /// The network brought something the pages show (the tick's fingerprint changed).
    net_wanted: bool,
    /// Why this window's frames were drawn: input, egui, the network, a dev request's burst.
    why: [u32; 4],
    /// What asked egui for this window's frames (egui's `repaint_causes`: file:line), counted.
    egui_causes: std::collections::BTreeMap<String, u32>,
    /// When egui's last frame asked for the next one (`None`: it asked for none).
    egui_due: Option<Instant>,
    /// Frames keep coming until then after a dev request.
    burst_until: Option<Instant>,
    /// When the sockets are read next, and when they were read last.
    next_tick: Option<Instant>,
    last_tick: Option<Instant>,
    /// What the network had brought at the last tick ([`net_fingerprint`]).
    net_seen: u64,
    /// The count: frames drawn and wakes without a frame since `window_start`.
    window_start: Option<Instant>,
    window_frames: u32,
    window_wakes: u32,
    /// Frames drawn since the start.
    frames_total: u64,
}

/// Install egui's repaint callback, so a repaint asked for by any thread (a page's timer, an image
/// loaded in the background) wakes a waiting event loop. Called once, at boot.
pub(crate) fn install(ctx: &egui::Context) {
    if !reactive() {
        return;
    }
    ctx.set_request_repaint_callback(|info| {
        if info.viewport_id != egui::ViewportId::ROOT {
            return;
        }
        let due = Instant::now() + info.delay;
        if let Ok(mut asked) = EGUI_ASKED.lock() {
            if asked.is_none_or(|t| due < t) {
                *asked = Some(due);
            }
        }
        crate::engine::tray_window::wake();
    });
}

/// A dev request arrived (the request folder's watcher thread, the dev MCP endpoint): wake the
/// loop and draw a short burst of frames (the module docs).
pub(crate) fn dev_request_seen() {
    if reactive() {
        DEV_REQUEST.store(true, Ordering::SeqCst);
        crate::engine::tray_window::wake();
    }
}

/// A window event: does it need a frame? egui-winit's own answer (`repaint`), and a change of size,
/// scale or focus. Never the frame itself: egui-winit answers `repaint: true` for
/// `RedrawRequested`, which would ask for the next frame from every frame, a frame loop (seen on the
/// first phone-preview run, 2026-11-20: every frame counted as input).
pub(crate) fn note_event(state: &mut EngineState, egui_repaint: bool, event: &WindowEvent) {
    if matches!(event, WindowEvent::RedrawRequested) {
        return;
    }
    if egui_repaint
        || matches!(event, WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } | WindowEvent::Focused(_))
    {
        state.redraw.wanted = true;
    }
}

/// After a frame's egui pass: when egui wants the next frame, and the count (both modes).
pub(crate) fn after_ui(state: &mut EngineState, out: &egui::FullOutput) {
    let now = Instant::now();
    let delay = out.viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
    state.redraw.egui_due = now.checked_add(delay);
    state.redraw.frames_total += 1;
    if reactive() {
        // What asked egui for the frame just drawn, for the count's line (the module docs).
        for c in state.egui_ctx.repaint_causes() {
            *state.redraw.egui_causes.entry(format!("{}:{}", c.file, c.line)).or_insert(0) += 1;
        }
    }
    state.redraw.window_frames += 1;
    if state.redraw.frames_total == 1 {
        let pages_only = state.gpu.as_ref().is_some_and(|g| g.renderer.pages_only());
        log::info!(
            "[Boot] first frame drawn {} ms after the window opened (pages-only start: {})",
            state.boot_timer.boot_start.elapsed().as_millis(),
            if pages_only { "yes" } else { "no" }
        );
        log_counts(state, "at the first frame");
        // A pages-first desktop begins preparing the 3D world now, beside the pages (world_prep.rs).
        crate::engine::world_prep::after_first_frame(state);
    }
    if reactive() {
        // The frame pumped the network itself (frame_load_screens.rs) and drew what it brought: the
        // next tick is a tick from now, and compares against what this frame showed. So a tick never
        // runs between drawn frames, and the reconnect countdown never counts a second twice.
        state.redraw.last_tick = Some(now);
        state.redraw.next_tick = Some(now + TICK);
        state.redraw.net_seen = net_fingerprint(state);
    } else {
        roll_window(state, now);
    }
}

/// The event loop is about to wait with the window shown (engine/tray_window.rs). A desktop: the
/// next frame at once, as always. Reactive: a frame only when something asked for one, else sleep
/// until the next thing that could (the module docs).
pub(crate) fn shown(state: &mut EngineState, event_loop: &ActiveEventLoop) {
    if !reactive() {
        event_loop.set_control_flow(ControlFlow::Wait);
        gpu!(state).window.request_redraw();
        return;
    }
    let now = Instant::now();
    // The sockets, a few times a second, without drawing.
    let due = state.redraw.next_tick.is_none_or(|t| now >= t);
    if due {
        tick(state, now);
    }
    if DEV_REQUEST.swap(false, Ordering::SeqCst) {
        state.redraw.burst_until = Some(now + DEV_BURST);
    }
    let asked = EGUI_ASKED.lock().ok().and_then(|mut a| a.take());
    let egui_due = match (state.redraw.egui_due, asked) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    let burst = state.redraw.burst_until.is_some_and(|t| now < t);
    // The first frame always (nothing has been drawn yet to stay on screen).
    let input = std::mem::take(&mut state.redraw.wanted) || state.redraw.frames_total == 0;
    let net = std::mem::take(&mut state.redraw.net_wanted);
    let egui = egui_due.is_some_and(|t| now >= t);
    let draw = input || net || egui || burst;
    if draw {
        // One reason per frame, the first that holds, for the count's line.
        let why = if input { 0 } else if egui { 1 } else if net { 2 } else { 3 };
        state.redraw.why[why] += 1;
        state.redraw.egui_due = None;
        event_loop.set_control_flow(ControlFlow::Wait);
        gpu!(state).window.request_redraw();
    } else {
        // Keep a future frame egui asked for, and sleep until it or the next tick.
        state.redraw.egui_due = egui_due;
        state.redraw.window_wakes += 1;
        let next_tick = state.redraw.next_tick.unwrap_or(now + TICK);
        let wake_at = egui_due.map_or(next_tick, |t| t.min(next_tick));
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake_at));
    }
    roll_window(state, now);
}

/// One tick of a reactive app: the network as the frame pumps it, the notices' sounds, and a frame
/// asked for when what arrived changed what the pages show.
fn tick(state: &mut EngineState, now: Instant) {
    let dt = state.redraw.last_tick.map_or(TICK, |t| now.saturating_duration_since(t)).min(Duration::from_secs(5)).as_secs_f32();
    state.redraw.last_tick = Some(now);
    state.redraw.next_tick = Some(now + TICK);
    crate::engine::frame_net::pump(state, dt);
    crate::engine::sfx::play_queued(state);
    let seen = net_fingerprint(state);
    if seen != state.redraw.net_seen {
        state.redraw.net_seen = seen;
        state.redraw.net_wanted = true;
    }
}

/// A number that changes whenever the network changed what the pages show: every message read on
/// the chat socket and on each saved server's link, the connection's state and its words, a
/// history fetch finishing, the peer-to-peer manager starting.
pub(crate) fn net_fingerprint(state: &EngineState) -> u64 {
    use std::hash::{Hash, Hasher};
    let g = &state.gui_state;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.ws_msgs_in.hash(&mut h);
    g.ws_status.hash(&mut h);
    g.ws_identified.hash(&mut h);
    g.ws_client.is_some().hash(&mut h);
    g.history_rx.is_some().hash(&mut h);
    g.webrtc.is_some().hash(&mut h);
    for c in &g.connections {
        c.msgs_in.hash(&mut h);
    }
    h.finish()
}

/// Log the count every ten seconds (the module docs).
fn roll_window(state: &mut EngineState, now: Instant) {
    let start = *state.redraw.window_start.get_or_insert(now);
    let span = now.saturating_duration_since(start);
    if span < COUNT_WINDOW {
        return;
    }
    // Why, on a reactive app: the frames each reason asked for, and the places that asked egui most.
    let why = if reactive() {
        let [input, egui, net, dev] = state.redraw.why;
        let mut causes: Vec<(&String, &u32)> = state.redraw.egui_causes.iter().collect();
        causes.sort_by(|a, b| b.1.cmp(a.1));
        let top: Vec<String> = causes.iter().take(3).map(|(k, n)| format!("{k} x{n}")).collect();
        format!(" (input {input}, egui {egui}, network {net}, dev {dev}; egui asked from: {})", if top.is_empty() { "nowhere".to_string() } else { top.join(", ") })
    } else {
        String::new()
    };
    log::info!(
        "[Redraw] {}: {} frames drawn in the last {:.1} s, {} wakes without one{why}",
        if reactive() { "reactive" } else { "continuous" },
        state.redraw.window_frames,
        span.as_secs_f32(),
        state.redraw.window_wakes
    );
    state.redraw.window_start = Some(now);
    state.redraw.window_frames = 0;
    state.redraw.window_wakes = 0;
    state.redraw.why = [0; 4];
    state.redraw.egui_causes.clear();
    if state.gpu.as_ref().is_some_and(|g| g.renderer.pages_only()) {
        log_counts(state, "now");
    }
}

/// What wgpu holds (engine/renderer_slot.rs `pipeline_counts`) and whether the world was built.
fn log_counts(state: &EngineState, when: &str) {
    let built = state.gpu.as_ref().is_some_and(|g| g.renderer.world_built());
    match crate::engine::renderer_slot::pipeline_counts() {
        Some(c) => log::info!(
            "[PagesOnly] wgpu holds {} render pipelines, {} compute pipelines, {} shader modules {when}; the world's renderer built: {}",
            c.render,
            c.compute,
            c.shader_modules,
            if built { "yes" } else { "no" }
        ),
        None => log::info!("[PagesOnly] wgpu gave no report {when}; the world's renderer built: {}", if built { "yes" } else { "no" }),
    }
}
