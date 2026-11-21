//! HumanityOS Game Engine
//!
//! Custom engine built on wgpu, hecs, rapier3d, and kira.
//! Designed for multi-scale space simulation with hot-reloadable data files.
//!
//! Compiles to both native (desktop via Tauri/winit) and WASM (browser via WebGPU).
//! Feature flags: `native` (default) or `wasm`.

/// The view's GPU through `EngineState::gpu` (the headless sim runner, increment 4): `gpu!(state)`
/// for the window, the renderer and egui's painter, `gpu_ref!(state)` behind a `&EngineState`. A view
/// stage only: with no GPU (a headless run) it panics, and the step never reaches it
/// (tests/frame_split_lint.rs); code the step shares checks `state.gpu` itself.
#[cfg(feature = "native")]
macro_rules! gpu {
    ($s:expr) => {
        $s.gpu.as_mut().expect("the view ran with no GPU")
    };
}
#[cfg(feature = "native")]
macro_rules! gpu_ref {
    ($s:expr) => {
        $s.gpu.as_ref().expect("the view ran with no GPU")
    };
}

pub mod renderer;
pub mod assets;
// The ship (with the machines and the station), the terrain and the simulation (with the save's
// shape and walking on a planet) are crates of their own since the crate split's rung 2
// (2026-11-02): humanity_ship, humanity_terrain and humanity_systems.

/// Extracted satellites of the native main loop (pure math, parsers, data
/// registries, home spawn helpers) - see docs/dev/code-structure-plan.md.
#[cfg(feature = "native")]
pub mod engine;

#[cfg(feature = "relay")]
pub mod relay;


// The resolved data directory is the core's since the crate split's backlog (2026-10-31):
// `storage::DATA_DIR` and `storage::data_dir()`, set below at startup.

/// How far a photoscanned plant may be uniformly scaled to stand in for its
/// species before the near-tree loader rejects it and grows the species
/// procedurally instead.
///
/// 3.0 because a scan carries REAL-WORLD-SIZED detail - needles, leaf blades,
/// bark grain - and a uniform stretch multiplies all of it. At 3x a 5 cm needle
/// spray reads as 15 cm: coarse, but still a plant. At the 17-35x the fir and
/// pine sapling scans were running (1 m scans standing in for 22 m and 16 m
/// trees) each spray became a 0.5-1.9 m sheet lying across the grass, which is
/// what the operator saw and reported. There is no scale factor that turns a
/// sapling into a mature tree - the architecture differs, not just the size.
#[cfg(feature = "native")]
const MAX_MODEL_STRETCH: f32 = 3.0;

// How-to (docs/design/how-to.md, 2026-10-23): the model, loader, checker and runs, compiled into
// the relay too (it will host shared how-tos); gated on `relay` for its hash's ciborium and blake3,
// which `native` includes. The page is gui/pages/how_to.rs.
#[cfg(feature = "relay")]
pub mod how_to;

// save_load applies a save to the LOCAL player's world (the GUI's item pool,
// the engine's spawned home), so it stays native even though persistence, the
// save format it reads, no longer is (2026-09-28). While persistence was
// native-only too, an ungated save_load broke every relay/CI build from v0.381
// to v0.414 (E0432 on persistence).
#[cfg(feature = "native")]
pub mod save_load;

#[cfg(feature = "native")]
pub mod gui;


#[cfg(feature = "native")]
pub mod net;

// In-world screens ladder, rung 5: the purpose-built video player core (WebM
// demux, AV1 + Opus decode, playback clock). Native-only: it links the rav1d
// decoder and feeds kira, neither of which the relay build carries.
#[cfg(feature = "native")]
pub mod media;

/// The free instrument library (2026-11-06, docs/design/music.md rung 6): the recorded instruments
/// that come with the app, the packs to download, and the best instrument for each part of the score.
#[cfg(feature = "native")]
pub mod instrument_packs;
/// Downloadable 3D model packs (2026-11-09, docs/design/simready-props.md): the Medical Bay's NVIDIA
/// SimReady models, from Settings > Graphics > Furniture models, the instrument packs' way.
#[cfg(feature = "native")]
pub mod model_packs;
/// Embeddable AI, rung 1 (2026-11-09, docs/design/embeddable-ai.md): the door with no model. Finding and
/// starting the separate AI engine on pipes, in a job object; the hardware checks Settings > AI shows;
/// what each later AI choice will need. No model and no runtime are ever in this app.
#[cfg(feature = "native")]
pub mod ai;
// The readable web (2026-09-16): HTTPS fetch + html5ever parse, no JS. Native-only. docs/design/readable-web.md.
#[cfg(feature = "native")]
pub mod web_reader;

#[cfg(feature = "native")]
pub mod config;

// Notifications (2026-10-30, docs/design/notifications.md): the rule, the mutes, quiet hours and
// the operating system's notice. Native-only: the relay build carries none of it.
#[cfg(feature = "native")]
pub mod notifications;

// The Apps page's game readers (2026-10-18): the stores' install records, read only, and the one
// launcher. Native-only: the relay never reads a computer's games. See
// docs/reference/findings/2026-10-13-game-stores-and-a-games-library.md.
#[cfg(feature = "native")]
pub mod games;

// The Apps page's program readers (2026-10-19): which catalog programs are installed (Windows' own
// records, the Start menu and Steam's, read only), the catalog's entries, and the install seam.
#[cfg(feature = "native")]
pub mod software;


#[cfg(feature = "native")]
pub mod updater;

// Signed-release verification + the operator-side signing/keygen tooling (the
// supply-chain root of trust; audit 2026-06-12 CRITICAL fix). Self-gated with
// `#![cfg(feature = "native")]` at the top of the file.
pub mod release_update;


#[cfg(feature = "wasm")]
pub mod wasm_entry;

// The phone build's entry point (Android rung A1, 2026-11-14): android_main, the app's own folder,
// the log to logcat. docs/design/platforms.md, android/README.md.
#[cfg(all(feature = "native", target_os = "android"))]
pub mod android;

/// Whether the docked-drone model should render this frame (v0.639). Extracted to crate level
/// (not inside `native_app` below) and made callable from both the render call site and its test
/// in `machines.rs`, so the test guards the REAL condition instead of a hand-copied duplicate
/// that could silently drift from it (caught in review, 2026-07-01). Pure bool logic, no
/// rendering/ECS dependency; native only, as its caller and its test are.
/// `drone_active` reflects `!gui_state.drones.is_empty()` as of the last refresh this frame;
/// callers must call this only after that refresh runs, or it reads one frame stale.
#[cfg(feature = "native")]
pub(crate) fn drone_dock_visible(showroom: bool, hide_machines: bool, drone_active: bool) -> bool {
    !showroom && !hide_machines && !drone_active
}

#[cfg(feature = "native")]
mod native_app {
    use glam::Vec3;
    // Extracted main-loop satellites (src/engine/): glob-imported so the many
    // call sites below keep their bare names across the staged extraction.
    #[allow(unused_imports)]
    use crate::engine::{
        color::*, dm::*, editor::*, frame_lock::*, geom::*, home_meshes::*, home_spawn::*, ipc::*, ipc_parse::*, near_tree_models::*, net_route::*, registries::*, world_load::*,
        state::*,
    };
    use humanity_core::ecs::components::Controllable;
    use crate::gui::GuiPage;
    use humanity_ship::ship_structure::{zone_body, zone_body_mut};
    use humanity_ship::storey_edits::EditStorey;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::Instant;
    use winit::application::ApplicationHandler;
    use winit::event::{DeviceEvent, DeviceId, WindowEvent};
    use winit::event_loop::ActiveEventLoop;
    use winit::keyboard::{KeyCode, PhysicalKey};
    use winit::window::{Window, WindowId};





    /// Locate the data directory relative to the exe.
    /// Prefers a data/ with world/ subdirectory (full repo) over extracted-only data/.
    pub(crate) fn find_data_dir() -> PathBuf {
        // A phone app has one data folder, its own (storage.rs: always Installed on Android); its
        // "exe" is the system's app_process, whose parents lead to the phone's /data partition.
        if humanity_core::platform::device::ANDROID {
            if let Some(dir) = humanity_core::storage::writable_data_dir() {
                return dir;
            }
        }
        let exe = std::env::current_exe().unwrap_or_default();
        let exe_dir = exe.parent().unwrap_or(std::path::Path::new("."));

        // Helper: is this a "full" data dir (has world/ with solar_system.ron)?
        let is_full_data = |p: &PathBuf| -> bool {
            p.join("world").join("solar_system.ron").exists()
        };

        // Collect all candidate data dirs in priority order
        let mut candidates: Vec<PathBuf> = Vec::new();

        // 1. data/ next to exe
        let beside_exe = exe_dir.join("data");
        if beside_exe.exists() && beside_exe.is_dir() {
            candidates.push(beside_exe);
        }

        // 1b. Per-user extracted data (%APPDATA%\HumanityOS\data etc.) — where a
        // distributed build now extracts editable modding data + the editor saves
        // (v0.706, keeps files out of the exe folder). Not a repo dir, so dev runs
        // still prefer the live repo data below; distributed builds resolve here.
        if let Some(user_data) = humanity_core::storage::writable_data_dir() {
            if user_data.exists() && user_data.is_dir() && !candidates.contains(&user_data) {
                candidates.push(user_data);
            }
        }

        // 2. Walk up parents (handles target/release/ -> repo root)
        let mut dir = exe_dir.to_path_buf();
        for _ in 0..6 {
            if let Some(parent) = dir.parent() {
                let candidate = parent.join("data");
                if candidate.exists() && candidate.is_dir() && !candidates.contains(&candidate) {
                    candidates.push(candidate);
                }
                dir = parent.to_path_buf();
            } else {
                break;
            }
        }

        // 3. CWD/data/ (cargo run)
        let cwd_data = std::env::current_dir()
            .unwrap_or_default()
            .join("data");
        if cwd_data.exists() && cwd_data.is_dir() && !candidates.contains(&cwd_data) {
            candidates.push(cwd_data);
        }

        // A "repo" data dir = one whose parent holds Cargo.toml (the source tree).
        // Prefer it over a FROZEN extracted copy beside a build exe, so dev runs —
        // including `target/release/HumanityOS.exe` directly — always read LIVE data
        // edits instead of a stale `target/release/data` snapshot a prior run
        // extracted. (Distributed builds have no sibling Cargo.toml, so they fall
        // through to the beside-exe data as before.) This was a real footgun:
        // `extract_data_if_needed` writes embedded data beside the exe once and
        // never refreshes it, so a stale copy silently shadowed the repo's data.
        let is_repo_data = |p: &PathBuf| -> bool {
            p.parent()
                .map(|parent| parent.join("Cargo.toml").exists())
                .unwrap_or(false)
        };

        // 1st preference: a full repo data dir (live source-tree data).
        for c in &candidates {
            if is_full_data(c) && is_repo_data(c) {
                log::info!("Data directory (repo, full): {}", c.display());
                return c.clone();
            }
        }

        // 2nd: any "full" data dir (with world/ subdirectory) over extracted-only.
        for c in &candidates {
            if is_full_data(c) {
                log::info!("Data directory (full, with world/): {}", c.display());
                return c.clone();
            }
        }

        // Otherwise use first available
        if let Some(first) = candidates.first() {
            log::info!("Data directory (partial): {}", first.display());
            return first.clone();
        }

        log::warn!("No data directory found, using ./data");
        PathBuf::from("data")
    }



    /// Tee writer (v0.601): env_logger pipes every log line here; we write it to the crash-safe log
    /// FILE (flushed each line so nothing is lost on a hard crash) AND stderr (visible in a dev/console
    /// build). Shared via Arc so the panic hook can append to the same file.
    struct LogTee {
        file: std::sync::Arc<std::sync::Mutex<std::fs::File>>,
    }
    impl std::io::Write for LogTee {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            if let Ok(mut f) = self.file.lock() {
                let _ = std::io::Write::write_all(&mut *f, buf);
                let _ = std::io::Write::flush(&mut *f);
            }
            let _ = std::io::Write::write_all(&mut std::io::stderr(), buf);
            // Warnings and errors also go to the play journal's `log` kind (crates/humanity-core/src/play_journal.rs).
            humanity_core::play_journal::note_log_bytes(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            if let Ok(mut f) = self.file.lock() {
                let _ = std::io::Write::flush(&mut *f);
            }
            Ok(())
        }
    }

    /// The logs directory: `%APPDATA%/HumanityOS/logs` on Windows (stable, always writable, findable
    /// no matter where the exe runs from), `~/.local/share/...` on unix, else the temp dir. (v0.601)
    fn log_dir() -> std::path::PathBuf {
        // Portable mode (v0.707): logs travel beside the exe too.
        if let Some(p) = humanity_core::storage::portable_logs_dir() {
            return p;
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            std::path::PathBuf::from(appdata).join("HumanityOS").join("logs")
        } else if let Ok(home) = std::env::var("HOME") {
            std::path::PathBuf::from(home).join(".local").join("share").join("HumanityOS").join("logs")
        } else {
            std::env::temp_dir().join("HumanityOS-logs")
        }
    }

    /// Crash-safe logging (v0.601): a file logger at `<logs>/run.log` (truncated each launch, tee'd to
    /// stderr) PLUS a panic hook that writes the panic message + location + backtrace to the log AND a
    /// persistent `crash.log` (appended across launches). So when the windowed exe crashes there is NO
    /// console to read -- the cause is already on disk. The operator says "it crashed"; I read run.log.
    fn init_logging() {
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let run_path = dir.join("run.log");
        let crash_path = dir.join("crash.log");
        let Ok(file) = std::fs::File::create(&run_path) else {
            env_logger::init(); // fall back to plain stderr if the file can't be opened
            return;
        };
        let shared = std::sync::Arc::new(std::sync::Mutex::new(file));
        let tee = LogTee { file: shared.clone() };
        let _ = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
            .format_timestamp_millis()
            // Quiet the DX12/naga shader-source dumps: at info level wgpu_hal
            // logs the ENTIRE generated HLSL for every pipeline (megabytes of
            // formatting during boot, and it buries real log lines).
            .filter_module("wgpu_hal::dx12::device", log::LevelFilter::Warn)
            .filter_module("wgpu_hal::vulkan::instance", log::LevelFilter::Warn)
            .filter_module("naga", log::LevelFilter::Warn)
            .target(env_logger::Target::Pipe(Box::new(tee)))
            .try_init();
        // PANIC HOOK: capture the full panic to disk before the process dies.
        let panic_file = shared.clone();
        let crash = crash_path.clone();
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let bt = std::backtrace::Backtrace::force_capture();
            let body = format!(
                "\n========== PANIC (HumanityOS v{}) ==========\n{}\n--- backtrace ---\n{}\n============================================\n",
                env!("CARGO_PKG_VERSION"),
                info,
                bt
            );
            if let Ok(mut f) = panic_file.lock() {
                let _ = std::io::Write::write_all(&mut *f, body.as_bytes());
                let _ = std::io::Write::flush(&mut *f);
            }
            if let Ok(mut cf) = std::fs::OpenOptions::new().create(true).append(true).open(&crash) {
                let _ = std::io::Write::write_all(&mut cf, body.as_bytes());
                let _ = std::io::Write::flush(&mut cf);
            }
            eprintln!("{body}");
            humanity_core::play_journal::note_panic(&info.to_string()); // written at once
            default_hook(info);
        }));
        // The play journal beside the logs folder (docs/design/play-journal.md).
        humanity_core::play_journal::start(&dir);
        log::info!("===== HumanityOS v{} starting =====", env!("CARGO_PKG_VERSION"));
        log::info!("log file:  {}", run_path.display());
        log::info!("crash log: {}", crash_path.display());
        log::info!("os: {} arch: {}", std::env::consts::OS, std::env::consts::ARCH);
    }

    /// Run the engine standalone — opens a window, renders a test scene.
    /// Supports three camera modes (Tab to cycle, F to toggle FP/TP, M for orbit).
    pub fn run() {
        init_logging();
        // Built with the tray's wake-up and, on Windows, the outside-close hook (engine/tray_window.rs).
        run_on(crate::engine::tray_window::event_loop());
    }

    /// The phone's [`run`]: the event loop is built on the activity Android handed android_main
    /// (src/android.rs), which has already pointed HOME at the app's own folder.
    #[cfg(target_os = "android")]
    pub fn run_android(app: winit::platform::android::activity::AndroidApp) {
        init_logging();
        run_on(crate::engine::tray_window::event_loop_android(app));
    }

    fn run_on(event_loop: winit::event_loop::EventLoop<()>) {
        let mut app = App::new();
        let result = event_loop.run_app(&mut app);
        match &result {
            Ok(()) => log::info!("===== HumanityOS clean shutdown ====="),
            Err(e) => log::error!("===== HumanityOS event-loop error: {e} ====="),
        }
        result.expect("Event loop error");
    }


    /// Open the character showroom in the given mode (1 = appearance / wetroom mirror,
    /// 2 = wardrobe / bedroom). Syncs the edit buffers from the live player, orbits the
    /// avatar, and frees the cursor. The avatar lives at the respawner; the home is hidden
    /// while the showroom is open, so it does not matter which room you opened it from.
    pub(crate) fn open_showroom(state: &mut EngineState, mode: u8) {
        let (name, app, outfit) = state
            .game_world
            .world
            .query::<(
                &humanity_core::ecs::components::Name,
                &humanity_core::ecs::components::Appearance,
                &humanity_core::ecs::components::Outfit,
                &Controllable,
            )>()
            .iter()
            .next()
            .map(|(_, (n, a, o, _))| (n.0.clone(), a.clone(), o.clone()))
            .unwrap_or_else(|| ("Wanderer".to_string(), Default::default(), Default::default()));
        state.gui_state.character_name = name;
        state.gui_state.appearance = app.clone();
        state.gui_state.outfit = outfit;
        state.gui_state.showroom_mode = mode;
        state.gui_state.showroom_active = true;
        state.gui_state.appearance_dirty = true; // rebuild the avatar to match the player
        state.gui_state.outfit_dirty = true;
        state.showroom_return_pos = state.camera.position;
        state.camera.switch_mode(crate::renderer::camera::CameraMode::Orbit);
        state.camera.orbit_target = state.avatar_base + Vec3::new(0.0, 0.9 * app.height_scale, 0.0);
        state.camera.orbit_distance = 3.2;
        state.camera.orbit_distance_min = 1.5;
        state.camera.orbit_distance_max = 8.0;
        state.controller.showroom_lock = true;
        // (cursor freed by the per-frame reconciliation in the update loop)
    }

    /// [`apply_window_mode`] only when the mode differs from the one last applied (the first call
    /// always applies). It runs on every settings save, and re-applying the same mode would
    /// un-maximize and re-maximize the window each time (see `WindowedFullscreen` below).
    pub(crate) fn apply_window_mode_once(window: &winit::window::Window, mode: crate::config::WindowMode) {
        static LAST: std::sync::Mutex<Option<crate::config::WindowMode>> = std::sync::Mutex::new(None);
        let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
        if *last != Some(mode) {
            *last = Some(mode);
            apply_window_mode(window, mode);
        }
    }

    /// Apply a window presentation mode to the live window (v0.454). Decorations = the OS
    /// title bar; borderless drops it; the fullscreen variants use winit's Fullscreen.
    pub(crate) fn apply_window_mode(window: &winit::window::Window, mode: crate::config::WindowMode) {
        use crate::config::WindowMode;
        use winit::window::Fullscreen;
        match mode {
            WindowMode::Windowed => {
                window.set_fullscreen(None);
                window.set_decorations(true);
                window.set_maximized(false);
            }
            WindowMode::WindowedFullscreen => {
                window.set_fullscreen(None);
                window.set_decorations(true);
                // Un-maximize, then maximize again now that the title bar is on, so Windows sizes
                // the window to the WORK AREA (above the taskbar). The window is created hidden and
                // maximized; asking an already-maximized window to maximize does nothing, and it
                // kept a height that ran under the taskbar, hiding the chat's message bar behind
                // it (operator's screenshots, 2026-10-13). Applied only when the mode changes
                // (`apply_window_mode_once`), so a settings save does not flicker the window.
                if window.is_maximized() {
                    window.set_maximized(false);
                }
                window.set_maximized(true);
            }
            WindowMode::BorderlessWindowed => {
                window.set_fullscreen(None);
                window.set_decorations(false);
                window.set_maximized(false);
            }
            WindowMode::BorderlessFullscreen => {
                window.set_decorations(false);
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
            WindowMode::ExclusiveFullscreen => {
                // Exclusive needs a concrete video mode; fall back to borderless if the
                // monitor exposes none (some platforms/headless).
                let vm = window.current_monitor().and_then(|m| m.video_modes().next());
                match vm {
                    Some(mode) => {
                        window.set_decorations(true);
                        window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                    }
                    None => window.set_fullscreen(Some(Fullscreen::Borderless(None))),
                }
            }
        }
    }

    /// THE single authority for the OS cursor (v0.460). Free (visible + ungrabbed) for any menu
    /// page, the showroom, or the construction editor -- so egui clicks land -- and grabbed for
    /// first-person play. Called every frame AND right after the egui frame (to catch a page
    /// change made by an egui click without a frame of lag). Keeping this the ONLY place that
    /// touches the cursor keeps `cursor_free` in sync; earlier, three handlers grabbed/freed the
    /// cursor directly without updating it, which desynced the flag so a panel could render but
    /// not be clicked (recurring "I can see the UI but can't interact" bug).
    pub(crate) fn reconcile_cursor(state: &mut EngineState) {
        // Background instances (HUMANITY_NO_FOCUS, the probe rig) never
        // touch the cursor WHILE UNFOCUSED (v0.1016): set_cursor_grab(
        // Confined) is a SYSTEM-WIDE cursor clip, so a rig entering the
        // world could trap the operator's pointer inside the rig's window
        // rect while they play. v0.1020: the guard clears on the first
        // genuine focus gain (see WindowEvent::Focused) - clicking into a
        // no-focus instance makes it a normal interactive window; the old
        // permanent guard left the cursor free in FPS, so the pointer
        // escaped the window and focus bounced (the operator's "mouse
        // doesn't work" in boot-verify instances).
        if state.background_no_cursor {
            return;
        }
        // The predicate itself lives in engine::input::cursor_want_free
        // (2026-09-18) so the dev IPC can report what this authority WANTS
        // separately from what the window did (a background rig returns
        // above and never touches the cursor). Its terms: any menu page, the
        // showroom, the construction editor, Alt held in first person
        // (v0.735, so the centered machine card's buttons can be clicked),
        // the F10 Cloud dev sidebar open and expanded (2026-09-05, the
        // held-Alt rule made sticky; closing or collapsing it drops the term
        // and the cursor re-grabs with no separate "restore" step), an
        // in-world modal (chat v0.772 / creature editor v0.778), and the
        // death screen (its Respawn button needs the cursor).
        let want_free = crate::engine::input::cursor_want_free(state);
        // Mirror the derived cursor state into GuiState BEFORE the early
        // return below, so the copy is refreshed every call (the early
        // return only skips the winit work). The F10 sidebar reads it to
        // decide whether its collapse tab may be drawn at all: a
        // click-sensing strip must not exist while the cursor is grabbed,
        // because the hidden Confined cursor drifts to the window edge on a
        // long turn and the next fire click would land on the strip
        // (2026-09-05 critic finding on U1). GUI code never writes this.
        state.gui_state.cursor_free = want_free;
        if want_free == state.cursor_free {
            return;
        }
        state.cursor_free = want_free;
        gpu!(state).window.set_cursor_visible(want_free);
        if want_free {
            gpu!(state).window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
        } else {
            gpu!(state)
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                .or_else(|_| gpu!(state).window.set_cursor_grab(winit::window::CursorGrabMode::Locked))
                .ok();
        }
    }






































































    /// The machine under the cursor for a port DROP (v0.625): the nearest `machine_pick` hit that isn't
    /// the drag's own source machine. Used to wire a dragged port to a target machine on release.
    pub(crate) fn port_drop_target(state: &EngineState, exclude: &str) -> Option<String> {
        let sz = gpu_ref!(state).window.inner_size();
        let (origin, dir) =
            state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<(String, f32)> = None;
        for (id, center, radius) in &state.machine_pick {
            if id == exclude {
                continue;
            }
            let t = (*center - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            if (*center - (origin + dir * t)).length() < *radius && best.as_ref().map_or(true, |(_, bt)| t < *bt) {
                best = Some((id.clone(), t));
            }
        }
        best.map(|(id, _)| id)
    }

    /// The conduit NODE under the cursor for a port drop (v0.629): nearest conduit-node orb the ray hits.
    /// Lets a dragged machine port land on a "main line" node, branching the machine onto the pipe graph.
    pub(crate) fn port_drop_node_target(state: &EngineState) -> Option<String> {
        let home = state.gui_state.home_machines.as_ref()?;
        let sz = gpu_ref!(state).window.inner_size();
        let (origin, dir) =
            state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<(String, f32)> = None;
        for cn in &home.conduit_nodes {
            let p = Vec3::new(cn.pos.0, cn.pos.1, cn.pos.2);
            let t = (p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            if (p - (origin + dir * t)).length() < 0.5 && best.as_ref().map_or(true, |(_, bt)| t < *bt) {
                best = Some((cn.id.clone(), t));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Whether `target` machine has a port of `util` (v0.625) -- the compatibility gate for wiring a
    /// dragged port to it (same utility; direction is oriented supply->demand by the caller).
    pub(crate) fn machine_has_port(state: &EngineState, target: &str, util: humanity_core::utilities::Utility) -> bool {
        state
            .gui_state
            .home_machines
            .as_ref()
            .and_then(|h| {
                let ty = h.all_instances().into_iter().find(|i| i.id == target)?.machine;
                let def = h.catalog.get(&ty)?;
                Some(def.derive_ports().iter().any(|p| p.utility == util))
            })
            .unwrap_or(false)
    }








    /// Angular size of a light's ROTATION rings (v0.792): a ring's world radius is the camera's
    /// distance to the light times this constant, so every light's rings land at the SAME
    /// predictable size on screen (operator: "I'd rather the rings be for rotate... some other
    /// way to represent the light's radius") -- the rings used to be range-sized, which made a
    /// long-throw light's handles cross half the room. ~0.05 rad is about a 60-pixel radius on
    /// a 1280-wide viewport with the default FOV: big enough to grab, small enough to not read
    /// as a range display.























    /// On a build-mode click, try to grab the nearest door/window opening gizmo (ray vs the cube).
    /// Returns true if one was grabbed. (v0.546)
    pub(crate) fn try_grab_opening(state: &mut EngineState) -> bool {
        let zo = active_zone_origin(state);
        // The openings of the storey being edited (ship homes increment 7, engine/editor_storeys.rs).
        let chosen = state.gui_state.construction_storeys.chosen;
        let gizmos = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
            Some(hs) => opening_gizmos_on_storey(hs, zo, EditStorey::of(hs, chosen).index),
            None => return false,
        };
        if gizmos.is_empty() {
            return false;
        }
        let sz = gpu!(state).window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<((usize, usize), f32)> = None;
        for (id, p) in &gizmos {
            let t = (*p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            let dd = (*p - (origin + dir * t)).length();
            if dd < 0.5 && best.map_or(true, |(_, b)| dd < b) {
                best = Some((*id, dd));
            }
        }
        if let Some((id, _)) = best {
            state.construction_opening_grab = Some(id);
            state.construction_grab_press = Some(state.cursor_pos); // tap-vs-drag (v0.549)
            true
        } else {
            false
        }
    }



    /// On a build-mode click, try to grab an opening RESIZE handle under the cursor (v0.578). Returns
    /// true (so the click doesn't also grab the move-cube or a wall).
    pub(crate) fn try_grab_opening_resize(state: &mut EngineState) -> bool {
        let zo = active_zone_origin(state);
        // The openings of the storey being edited (ship homes increment 7).
        let chosen = state.gui_state.construction_storeys.chosen;
        let handles = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone) {
            Some(hs) => opening_resize_handles_on_storey(hs, zo, EditStorey::of(hs, chosen).index),
            None => return false,
        };
        if handles.is_empty() {
            return false;
        }
        let sz = gpu!(state).window.inner_size();
        let (origin, dir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
        let mut best: Option<((usize, usize, u8), f32)> = None;
        for (id, p) in &handles {
            let t = (*p - origin).dot(dir);
            if t < 0.0 {
                continue;
            }
            let dd = (*p - (origin + dir * t)).length();
            if dd < 0.3 && best.map_or(true, |(_, b)| dd < b) {
                best = Some((*id, dd));
            }
        }
        if let Some((id, _)) = best {
            state.construction_opening_resize = Some(id);
            state.gui_state.construction_wall_selected = Some(id.0); // show the wall on the panel
            state.gui_state.construction_storeys.slab_selected = None; // ... in place of a floor slab's
            true
        } else {
            false
        }
    }

    /// Per-frame while an opening resize handle is grabbed (v0.578): left/right project the cursor onto
    /// the wall axis (resize width, keeping the opposite edge fixed); top/bottom intersect the cursor
    /// ray with the wall plane and take its height (resize height/sill). Grid-snapped, min 0.2 m.
    pub(crate) fn apply_opening_resize(state: &mut EngineState) {
        let Some((wi, oi, edge)) = state.construction_opening_resize else {
            return;
        };
        // Copy the wall axis to locals so the cursor calls below don't conflict with the home borrow.
        // `wall_base`: where the wall stands (ship homes increment 7); an opening's sill and height
        // are measured from it, so a window upstairs is dragged against its own wall.
        let (ax, az, ux, uz, len, wall_h, wall_base) = match zone_body(&state.gui_state.ship_structure, state.gui_state.construction_zone).and_then(|hs| hs.walls.get(wi)) {
            Some(wall) => {
                let (ax, az) = wall.a;
                let (dx, dz) = (wall.b.0 - ax, wall.b.1 - az);
                let len = (dx * dx + dz * dz).sqrt();
                if len < 1e-4 {
                    return;
                }
                (ax, az, dx / len, dz / len, len, wall.height, wall.base)
            }
            None => return,
        };
        let zo = active_zone_origin(state); // wall data is zone-local; the cursor/ray are world (v0.754)
        let grid = state.gui_state.construction_grid_snap;
        let val = if edge <= 1 {
            // On the plan of the storey being edited (ship homes increment 7).
            let Some(StoreyFloorHit { x: hx, z: hz, .. }) = cursor_storey_hit(state) else {
                return;
            };
            let (hx, hz) = (hx - zo.x, hz - zo.z);
            let mut along = ((hx - ax) * ux + (hz - az) * uz).clamp(0.0, len);
            if grid {
                along = (along * 4.0).round() / 4.0;
            }
            along
        } else {
            let sz = gpu!(state).window.inner_size();
            let (origin, ddir) = state.camera.pick_ray(state.cursor_pos, (sz.width as f32, sz.height as f32));
            let normal = Vec3::new(-uz, 0.0, ux);
            let denom = ddir.dot(normal);
            if denom.abs() < 1e-6 {
                return;
            }
            // The wall plane passes through its zone-local `a` shifted to world by the origin.
            let t = (Vec3::new(ax + zo.x, 0.0, az + zo.z) - origin).dot(normal) / denom;
            if t < 0.0 {
                return;
            }
            // A sill is measured from the wall's base (ship homes increment 7).
            let mut y = ((origin + ddir * t).y - zo.y - wall_base).clamp(0.0, wall_h);
            if grid {
                y = (y * 4.0).round() / 4.0;
            }
            y
        };
        if let Some(op) = zone_body_mut(&mut state.gui_state.ship_structure, state.gui_state.construction_zone)
            .and_then(|hs| hs.walls.get_mut(wi))
            .and_then(|w| w.openings.get_mut(oi))
        {
            match edge {
                0 => {
                    let right = op.at + op.width;
                    let at = val.min(right - 0.2).max(0.0);
                    op.width = right - at;
                    op.at = at;
                }
                1 => op.width = (val - op.at).max(0.2).min(len - op.at),
                2 => op.height = (val - op.sill).max(0.2),
                3 => {
                    let top = op.sill + op.height;
                    let sill = val.min(top - 0.2).max(0.0);
                    op.height = top - sill;
                    op.sill = sill;
                }
                _ => {}
            }
        }
        state.gui_state.construction_structure_dirty = true;
    }








    struct App {
        state: Option<EngineState>,
    }

    impl App {
        fn new() -> Self {
            Self { state: None }
        }
    }









    impl ApplicationHandler for App {
        fn resumed(&mut self, event_loop: &ActiveEventLoop) {
            // Every resumed after the first is a phone app coming back to the screen: a new
            // drawing surface on the same window (engine/app_lifecycle.rs). A desktop has one.
            if let Some(state) = self.state.as_mut() {
                crate::engine::app_lifecycle::resumed(state);
                return;
            }

            // Boot-phase timing (dev tooling): window-open is the wall-clock
            // origin; each phase below records a named span, and load_world()
            // emits the consolidated summary once the world is ready.
            let mut boot_timer =
                humanity_core::boot_timing::BootTimer::new(std::time::Instant::now());

            // Extract embedded data files on first run (enables modding)
            let t_boot = std::time::Instant::now();
            humanity_core::storage::extract_data_if_needed();
            boot_timer.since("extract_data", t_boot);

            // Boot MAXIMIZED with decorations = "windowed fullscreen" (title bar + taskbar
            // still visible), the operator's preferred default (v0.454). The loaded
            // window_mode is then applied once the config is read (apply_window_mode).
            // Background launch (v0.828): when HUMANITY_NO_FOCUS is set, the
            // window opens WITHOUT activating - it appears BEHIND the currently
            // focused window and never steals focus. This is for dev/agent verify
            // instances that boot to capture a frame: they must not yank the
            // operator out of a game or movie (his exact request). `with_active(
            // false)` still renders frames (unlike minimizing, which can suspend
            // the swapchain and break the offscreen screenshot capture), so
            // headless verification keeps working. Normal launches (env unset)
            // are unchanged and focus as before.
            // Background is now the DEFAULT for script launches (v0.1081):
            // focus requires proof of a human launch (HUMANITY_TAKE_FOCUS
            // from the operator's recipes, or explorer.exe as the parent
            // process, i.e. a real double-click). Env var + marker still
            // force background explicitly. Full decision order + history:
            // src/engine/launch_focus.rs.
            let background = crate::engine::launch_focus::launch_in_background();
            // A rig's low impact (engine::low_impact, 2026-10-22) asks for a small window, never
            // maximized: the fewest pixels to shade while the operator uses his PC.
            let low_impact_window = crate::engine::low_impact::window();
            let window_attrs = Window::default_attributes()
                .with_title(format!("HumanityOS v{}", env!("CARGO_PKG_VERSION")));
            let window_attrs = match low_impact_window {
                Some((w, h)) => window_attrs
                    .with_inner_size(winit::dpi::PhysicalSize::new(w, h))
                    .with_maximized(false),
                None => window_attrs
                    .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
                    .with_maximized(true),
            };
            let window_attrs = window_attrs
                .with_active(!background)
                // EVERY launch creates the window HIDDEN and shows it once the
                // renderer is ready (no white flash). Background launches used
                // to be created visible-but-inactive (v0.1069), but a VISIBLE
                // creation is activated by the system whenever no foreground
                // input lock is held -- measured stealing focus from sample 0
                // with the operator away from the keyboard (2026-07-31). The
                // background show goes through show_window_behind() below
                // (SW_SHOWNOACTIVATE + bottom of Z order), which never
                // activates regardless of the input-lock state.
                .with_visible(false);

            let t_boot = std::time::Instant::now();
            let window = Arc::new(
                event_loop
                    .create_window(window_attrs)
                    .expect("Failed to create window"),
            );
            boot_timer.since("window_create", t_boot);

            // Set window icon from embedded PNG
            {
                let icon_bytes = include_bytes!(concat!(env!("HUMANITY_REPO_ROOT"), "/assets/icon.png"));
                if let Ok(img) = image::load_from_memory(icon_bytes) {
                    let rgba = img.to_rgba8();
                    let (w, h) = (rgba.width(), rgba.height());
                    if let Ok(icon) = winit::window::Icon::from_rgba(rgba.into_raw(), w, h) {
                        window.set_window_icon(Some(icon));
                    }
                }
            }

            // Initialize renderer (block on async): the device and the surface, which is all the
            // pages need; the 3D world's renderer is built beside the pages after the first frame, on
            // Play, or here before them, as Settings > Graphics > Prepare the 3D world says
            // (engine/world_prep.rs, engine/renderer_slot.rs, which also holds low impact's switches).
            let t_boot = std::time::Instant::now();
            let config = crate::config::AppConfig::load_if_exists();
            let prep = crate::engine::world_prep::init(config.as_ref().map_or("", |c| c.world_prep.as_str()));
            let renderer = pollster::block_on(crate::engine::renderer_slot::RendererSlot::new_native(window.clone(), prep));
            boot_timer.since("renderer_init", t_boot);

            // Both modes were created hidden. A human launch shows normally
            // (activating); a background launch shows WITHOUT activation, at
            // the bottom of the Z order (raw SW_SHOWNOACTIVATE -- winit's
            // set_visible always activates on Windows).
            if !background {
                window.set_visible(true);
            } else {
                crate::engine::launch_focus::show_window_behind(&window);
            }

            // ── DEFERRED: 3D world init is skipped here, done lazily on first Enter World ──
            // Only set up the data directory path for later use.
            // Initialize egui
            let egui_ctx = egui::Context::default();
            let egui_state = egui_winit::State::new(
                egui_ctx.clone(),
                egui_ctx.viewport_id(),
                &window,
                None,
                None,
                None,
            );
            let mut egui_renderer = egui_wgpu::Renderer::new(
                &renderer.device,
                renderer.surface_format(), // display-format: egui draws on the swapchain, after the present pass
                None,
                1,
                false,
            );
            // The 3D model viewer's pipelines (gui/model_view/gpu.rs): its paint callback finds them here.
            egui_renderer.callback_resources.insert(crate::gui::model_view::gpu::Gpu::new(
                &renderer.device,
                &renderer.queue,
                renderer.surface_format(), // display-format: the viewer's picture is copied into egui's pass on the swapchain
            ));
            // Everything else the app keeps between frames, built the way a headless run builds it
            // with no GPU (engine/engine_boot.rs, the headless sim runner's increment 4).
            let gpu = crate::engine::state::Gpu { window, renderer, egui_state, egui_renderer };
            self.state = Some(EngineState::boot(
                crate::engine::engine_boot::BootOptions {
                    data_dir: find_data_dir(),
                    saves_dir: humanity_systems::persistence::saves_dir(),
                    config,
                    services: true,
                    boot_timer,
                    egui_ctx,
                },
                Some(gpu),
            ));
        }

        /// A phone app left the screen (a desktop never says so): engine/app_lifecycle.rs.
        fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
            if let Some(state) = self.state.as_mut() {
                crate::engine::app_lifecycle::suspended(state);
            }
        }

        fn window_event(
            &mut self,
            event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            event: WindowEvent,
        ) {
            let state = match self.state.as_mut() {
                Some(s) => s,
                None => return,
            };
            if crate::engine::app_lifecycle::skip_event(state, &event) {
                return;
            }

            // Pass events to egui first, EXCEPT the Tab key while in-game (GuiPage::None).
            // Tab is egui's focus-traversal key; letting egui see it in FPS mode makes egui
            // grab keyboard focus and then swallow WASD, freezing movement until a menu
            // round-trip resets focus (v0.429 bug). In-game, Tab is our reveal-peek (handled
            // in the KeyboardInput arm below), so egui never needs it there.
            let ingame_tab = state.gui_state.active_page == GuiPage::None
                && matches!(
                    &event,
                    WindowEvent::KeyboardInput { event: ke, .. }
                        if ke.physical_key == PhysicalKey::Code(KeyCode::Tab)
                );
            let egui_consumed = if ingame_tab {
                false
            } else {
                let gpu = gpu!(state);
                let r = gpu.egui_state.on_window_event(&gpu.window, &event);
                crate::engine::redraw::note_event(state, r.repaint, &event); // a phone draws on change
                r.consumed
            };

            match event {
                WindowEvent::Focused(focused) => {
                    // Drives the foreground/background FPS cap (v0.1016).
                    state.window_focused = focused;
                    // NOTE (v0.1072): this arm no longer clears
                    // background_no_cursor. Windows delivers Focused(true)
                    // during creation with no user action, which disarmed the
                    // background guard before the boot frame and let
                    // apply_window_mode's maximize steal focus anyway. The
                    // deliberate click-in path lives in MouseInput now: the
                    // launch flag still only means "don't steal focus", never
                    // "stay half-dead".
                }
                WindowEvent::CloseRequested => {
                    // "Closing the window keeps HumanityOS running in the tray" (off by default):
                    // the window hides instead, after the world is left and saved.
                    if crate::engine::tray_window::close_to_tray(state) {
                        return;
                    }
                    // Persist the active offline home (v0.381) and flush unsaved build
                    // edits (v0.791) before quitting: the save every way out of the
                    // game runs first (engine/quit.rs).
                    crate::engine::quit::save_before_exit(state);
                    event_loop.exit();
                }
                WindowEvent::Resized(size) => {
                    gpu!(state).renderer.resize(size.width, size.height);
                    state.camera.aspect = gpu!(state).renderer.aspect_ratio();
                }
                WindowEvent::KeyboardInput { event, .. } => {
                    crate::engine::frame_keys::run(state, event, egui_consumed);
                }
                WindowEvent::CursorMoved { position, .. } => {
                    // Cache the cursor pixel position for 3D picking (v0.466). Unconditional so
                    // it stays fresh regardless of which mode we're in.
                    state.cursor_pos = (position.x as f32, position.y as f32);
                }
                WindowEvent::MouseInput { button, state: btn_state, .. } => {
                    crate::engine::frame_mouse::button(state, button, btn_state, egui_consumed);
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    crate::engine::frame_mouse::wheel(state, delta, egui_consumed);
                }
                WindowEvent::RedrawRequested => {
                    // Hidden in the tray: no frame, no GPU work, no world (engine/tray_window.rs).
                    if state.tray.hidden {
                        return;
                    }
                    // `cpu.frame_total` starts HERE, at the handler's entry,
                    // and is recorded after the last submit (the broadcast
                    // capture copy, just before present) for world frames.
                    // Frame time minus this, minus `cpu.present_wait`, is
                    // the time outside the handler (event pumping); this
                    // minus the sum of the instrumented stages is the CPU
                    // work nobody has timed yet, in EMA steady state (the
                    // stages straddle the acquire boundary, see
                    // docs/dev/performance-profiling.md).
                    let frame_t0 = Instant::now();
                    let mut f = crate::engine::frame_vars::FrameVars::new(frame_t0);
                    crate::engine::frame_clock::run(state, &mut f);
                    // The simulation's step, the one a headless run takes too (engine/frame_step.rs).
                    crate::engine::frame_step::run(state, &mut f);
                    // The step ends here (tests/frame_split_lint.rs): every stage above is the simulation's,
                    // run once; everything below draws it or brings input for the next step.
                    crate::engine::frame_particles::run(state);
                    crate::engine::frame_copresence::run(state);
                    crate::engine::frame_save_editor::run(state);
                    crate::engine::frame_lists_home::run(state, &mut f);
                    crate::engine::frame_build_ghosts::run(state, &mut f);
                    crate::engine::frame_build_gizmos::run(state, &mut f);
                    crate::engine::frame_build_helpers::run(state, &mut f);
                    crate::engine::frame_people::run(state, &mut f);
                    crate::engine::frame_sky::run(state, &mut f);
                    crate::engine::frame_hud_voice::run(state);
                    crate::engine::frame_mirrors::run(state);
                    crate::engine::frame_mirrors_world::run(state, &mut f);
                    crate::engine::frame_lists_world::run(state, &mut f);
                    crate::engine::frame_env_publish::run(state);
                    crate::engine::frame_cards::run(state);
                    crate::engine::frame_dev_ipc::run(state);
                    crate::engine::frame_load_screens::run(state, &mut f);
                    let page_active = f.page_active;
                    let scene_result = crate::engine::frame_world_passes::run(state, &mut f);
                    if page_active {
                        humanity_core::frame_costs::span_stop_recording(); // past the acquire on a frame a page covers (frame_total's rule)
                    }
                    match scene_result {
                        Ok((surface_texture, view)) => {
                            let full_output = crate::engine::frame_ui_run::run(state, event_loop);
                            crate::engine::redraw::after_ui(state, &full_output); // the next frame egui wants, the count
                            crate::engine::frame_audio::run(state, &full_output);
                            crate::engine::frame_ui_paint::run(state, full_output, &view);
                            crate::engine::frame_tail::run(state, &mut f, surface_texture);
                        }
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = gpu!(state).window.inner_size();
                            gpu!(state).renderer.resize(size.width, size.height);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            log::error!("Out of GPU memory");
                            // The world lives on the CPU: saved on the way out (engine/quit.rs).
                            crate::engine::quit::save_before_exit(state);
                            event_loop.exit();
                        }
                        Err(e) => {
                            log::warn!("Render error: {:?}", e);
                        }
                    }
                    // The rigs' verbs, every frame, presented or not (increment 3; frame_tail.rs).
                    crate::engine::ipc::poll_showcase_request(state);
                }
                _ => {}
            }
        }

        fn device_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _device_id: DeviceId,
            event: DeviceEvent,
        ) {
            let state = match self.state.as_mut() {
                Some(s) => s,
                None => return,
            };

            if let DeviceEvent::MouseMotion { delta } = event {
                // Pass mouse motion to the camera when no GUI page is active. In FPS this is
                // look; in the showroom / construction orbit cam it only rotates while a mouse
                // button is held (so moving over a panel does nothing). (v0.464)
                // Suppressed while Alt frees the cursor (v0.735): reaching for
                // a machine-card button must not spin the camera.
                // Same for the expanded F10 sidebar (2026-09-05): the cursor
                // is free to work the panel, so motion must not spin the
                // camera - the alt_held rule, made sticky. The whole rule is
                // engine::input::mouse_look_allowed (2026-10-04), beside the
                // cursor rule it has to agree with.
                if crate::engine::input::mouse_look_allowed(&state.gui_state, state.alt_held) {
                    state.controller.process_mouse_motion(delta.0, delta.1);
                    // Real mouse-look releases the probe hold's yaw/pitch
                    // pin (same rule as the keyboard: the human wins).
                    if (delta.0.abs() + delta.1.abs()) > 0.5 && state.probe_hold.is_some() {
                        state.probe_hold = None;
                    }
                }
            }
        }

        fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
            if let Some(state) = self.state.as_mut() {
                crate::engine::app_lifecycle::hold_hidden_while_away(state);
                // Shown: the next frame at once. Hidden in the tray: the sockets read a few times a
                // second and nothing drawn; true when the tray's Quit asked to leave, which saves the
                // way every other way out does (engine/tray_window.rs, engine/quit.rs).
                if crate::engine::tray_window::about_to_wait(state, event_loop) {
                    crate::engine::quit::save_before_exit(state);
                    event_loop.exit();
                }
            }
        }
    }
}

/// Run the native engine (only available with the `native` feature).
#[cfg(feature = "native")]
pub fn run() {
    native_app::run();
}

/// Run the app on a phone, from android_main (src/android.rs).
#[cfg(all(feature = "native", target_os = "android"))]
pub fn run_android(app: winit::platform::android::activity::AndroidApp) {
    native_app::run_android(app);
}
