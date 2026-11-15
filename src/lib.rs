//! HumanityOS Game Engine
//!
//! Custom engine built on wgpu, hecs, rapier3d, and kira.
//! Designed for multi-scale space simulation with hot-reloadable data files.
//!
//! Compiles to both native (desktop via Tauri/winit) and WASM (browser via WebGPU).
//! Feature flags: `native` (default) or `wasm`.

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
    use crate::assets::AssetManager;
    use humanity_core::ecs::components::Controllable;
    use crate::gui::{GuiPage, GuiState};
    use crate::gui::widgets::help_modal;
    use humanity_core::hot_reload::HotReloadCoordinator;
    use crate::renderer::camera::{Camera, CameraController};
    use crate::renderer::Renderer;
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
        state.window.set_cursor_visible(want_free);
        if want_free {
            state.window.set_cursor_grab(winit::window::CursorGrabMode::None).ok();
        } else {
            state
                .window
                .set_cursor_grab(winit::window::CursorGrabMode::Confined)
                .or_else(|_| state.window.set_cursor_grab(winit::window::CursorGrabMode::Locked))
                .ok();
        }
    }






































































    /// The machine under the cursor for a port DROP (v0.625): the nearest `machine_pick` hit that isn't
    /// the drag's own source machine. Used to wire a dragged port to a target machine on release.
    pub(crate) fn port_drop_target(state: &EngineState, exclude: &str) -> Option<String> {
        let sz = state.window.inner_size();
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
        let sz = state.window.inner_size();
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
        let sz = state.window.inner_size();
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
        let sz = state.window.inner_size();
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
            let sz = state.window.inner_size();
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

            // Initialize renderer (block on async). The heaviest untimed phase:
            // adapter + device request, every shader-module + pipeline + bind-
            // group-layout, plus the cloud-noise volumes.
            let t_boot = std::time::Instant::now();
            let mut renderer = pollster::block_on(Renderer::new_native(window.clone()));
            boot_timer.since("renderer_init", t_boot);
            // Low impact's renderer switches that have no setting (god rays, the aurora, the
            // Sun's glare, sun shadows): off for the whole run. A no-op in a normal game.
            crate::engine::low_impact::hold_renderer(&mut renderer);

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
            let data_dir = find_data_dir();
            // Publish it for the cached loaders (laws/glossary/homes) so they resolve
            // the same CWD-independent path instead of a bare relative "data".
            let _ = humanity_core::storage::DATA_DIR.set(data_dir.clone());

            // Minimal camera/controller (needed by struct, but not used until world loads)
            let mut camera = Camera::new();
            camera.aspect = renderer.aspect_ratio();
            camera.position = Vec3::new(-0.5, 1.7, 4.0);
            camera.pitch = -0.2;
            camera.yaw = std::f32::consts::PI;
            // Sensitivity here is just the frame-0 default; the boot-time settings_dirty
            // sync (below) overrides it with the saved value on the first frame.
            let controller = CameraController::new(crate::renderer::camera::WALK_SPEED_MPS, 0.25);

            // Minimal asset/ECS (lightweight, no file I/O)
            let asset_manager = AssetManager::new(data_dir.clone());
            let hot_reload = HotReloadCoordinator::new(&data_dir);
            // The world's simulation, built without the GPU (engine/sim_boot.rs, the headless sim
            // runner's increment 1): the ECS world, the DataStore's slots, every system, the live
            // save, the fresh player, the asteroids and the menu's live home power.
            let crate::engine::sim_boot::SimBoot {
                mut game_world,
                system_runner,
                mut data_store,
                config: boot_config,
                slot: boot_slot,
                home_save,
            } = crate::engine::sim_boot::boot(crate::engine::sim_boot::SimBootOptions {
                data_dir: data_dir.clone(),
                saves_dir: humanity_systems::persistence::saves_dir(),
                config: crate::config::AppConfig::load_if_exists(),
            });

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
            let theme = crate::gui::theme::load_theme();
            theme.apply_to_egui(&egui_ctx);
            // Install every font fallback chain: Hack into the Proportional
            // family (so arrows and box drawing stop being tofu on Linux and
            // macOS), plus the OS-installed CJK and emoji fonts, which are
            // read from the user's machine and never redistributed. Each is
            // independent and a missing one is a silent no-op, but set_fonts
            // itself always runs. See src/gui/fonts.rs. The same helper runs
            // for every in-world screen context (gui::screen_surface), so a
            // wall screen never shows tofu the main UI does not.
            crate::gui::install_fonts(&egui_ctx);
            // Load + cache the in-app glossary (data/glossary.json) so
            // widgets::definition_text can pop term definitions on
            // Alt+hover. Silent no-op if the file is missing — definition
            // tooltips just won't appear. (v0.195.0.)
            crate::gui::glossary::install();
            let mut gui_state = GuiState::default();

            // Load data-driven catalogs into GUI state
            gui_state.external_catalog = crate::gui::load_external_catalog(&data_dir);
            gui_state.apps.own_tools = crate::gui::pages::apps::own_tools::load(&data_dir);
            gui_state.help_registry = help_modal::load_help_registry(&data_dir);
            // Keymaps used to load lazily on the first F1 press. The top-right help
            // button can open the same panel without F1 ever being touched, so they
            // are loaded up front now; the lazy path below stays as a safety net.
            gui_state.keymaps = crate::gui::pages::keymap::load_keymaps(&data_dir);
            gui_state.map_planets = crate::gui::load_planets();
            // The places the live save's start holds (a cabin its own, data/homes/starts.ron).
            gui_state.places = crate::save_load::start_places(&data_dir, &humanity_systems::persistence::live_save::active_home().start);
            // Organize-layer inventory pool: restore the SAVED container contents if the
            // active home has any (transfers persisted, v0.517), else seed from the
            // places spine (every leaf item tagged with its container path). The live
            // backpack stays ECS-driven (restored separately by apply_save_to_world).
            // (the saved pool replaces it below, after the config, when
            // progress is being kept)
            gui_state.placed_items = crate::gui::flatten_placed_items(&gui_state.places);
            gui_state.homestead_design = crate::gui::load_homestead_design(&data_dir);
            // Load the home's machine layout ONCE: both the Home-page self-sufficiency
            // loops AND the construction editor's editable machine layout come from it
            // (v0.519: machine placement). See docs/design/home-design.md.
            gui_state.home_machines = humanity_systems::grow_machines::load_home(&humanity_systems::persistence::live_save::active_home_machines_path(&data_dir));
            gui_state.homestead_loops = gui_state
                .home_machines
                .as_ref()
                .map(|h| h.loops.clone())
                .unwrap_or_default();
            gui_state.tower_configs = crate::gui::load_tower_configs(&data_dir);
            gui_state.garden_areas = crate::gui::load_garden_areas(&humanity_systems::persistence::live_save::active_home_machines_path(&data_dir), &data_dir);
            gui_state.grow_media = humanity_systems::grow_machines::load_grow_media(&data_dir);
            let lib = crate::gui::load_library(&data_dir);
            gui_state.library = lib.sections;
            gui_state.library_tags = lib.tag_groups;
            gui_state.curriculum = crate::gui::load_curriculum(&data_dir);
            gui_state.equipment_slots = crate::gui::load_equipment_slots(&data_dir);
            let (sevs, cats) = crate::gui::load_bug_taxonomy(&data_dir);
            gui_state.bug_severities = sevs;
            gui_state.bug_categories = cats;
            gui_state.crafting_category_groups = crate::gui::load_crafting_category_groups(&data_dir);
            // Populate the Crafting page's recipe browser from data/recipes.csv.
            // (The runtime RecipeRegistry is loaded separately into the DataStore for
            // CraftingSystem; this is the GUI-facing projection so the page lists
            // recipes instead of showing the empty "No recipes match" state.)
            gui_state.craft_recipes = crate::gui::load_crafting_recipes(&data_dir);
            // Load the runtime ECS registries (items / recipes / plants / status
            // effects / skills / quests / containers) into the DataStore EAGERLY at
            // startup — so the menu-driven loops (inventory / crafting / skills /
            // quests) work WITHOUT first opening the 3D world. This was the bug: they
            // used to load only in lazy load_world (3D-world view), leaving raw item
            // ids, no recipes, empty skills + the quest shown by id. (load_world
            // re-loads them; idempotent.)
            let t_boot = std::time::Instant::now();
            load_data_registries(&mut data_store, &data_dir);
            boot_timer.since("eager_data_registries", t_boot);
            // ── Background star-sky prebuild (v0.865 boot speed) ──
            // star_catalog_and_glow measured 4100 ms ON the world-entry path:
            // catalog read + vertex packing + glow PNG decode + GPU upload.
            // None of it needs the main thread (wgpu Device/Queue are Arc-backed
            // and thread-safe), so build the ENTIRE StarRenderer while the user
            // is still on the chat screen. Entering the world receives the
            // finished sky; only someone who beats the thread waits (and then
            // exactly as long as they would have anyway).
            let star_preload_rx = {
                let (tx, rx) = std::sync::mpsc::channel();
                let device = renderer.device.clone();
                let queue = renderer.queue.clone();
                let scene_format = renderer.scene_format();
                let sky_dir = data_dir.clone();
                let tier_setting = gui_state.settings.star_catalog_tier.clone();
                let ultra_glow = gui_state.settings.sky_glow_tier == "ultra";
                std::thread::spawn(move || {
                    let t = std::time::Instant::now();
                    let cap = crate::renderer::stars::StarCatalogTier::resolve_cap(&tier_setting);
                    let built = crate::renderer::stars::StarCatalog::load(&sky_dir, cap)
                        .as_ref()
                        .and_then(|catalog| {
                            crate::renderer::stars::StarRenderer::new(
                                &device,
                                &queue,
                                scene_format,
                                catalog,
                                &sky_dir,
                                ultra_glow && cap >= 2,
                            )
                        });
                    log::info!(
                        "[Boot] star sky prebuilt in background: {} ms ({})",
                        t.elapsed().as_millis(),
                        if built.is_some() { "ok" } else { "unavailable" }
                    );
                    let _ = tx.send(built);
                });
                Some(rx)
            };
            // ── Background planet-surface prebuild (2026-10-18 boot speed) ──
            // planet_defs_bake measured 1.2 s on the world-entry path: reading
            // data/planets/, loading or synthesizing each elevation grid,
            // loading each colour grid and baking each surface texture. All of
            // it is CPU work on files (net_route::bake_planet_defs), so it runs
            // here beside the star sky while the player is still on the menu;
            // the first world entry receives it and only installs and uploads
            // (world_load.rs). A planet RON hot reload before world entry drops
            // the receiver (reload_planet_defs), so a stale prebuild is never
            // installed. The baked textures (about 100 MB of RGBA for the four
            // textured bodies) wait in the channel until then.
            let planet_defs_preload_rx = {
                let (tx, rx) = std::sync::mpsc::channel();
                let dir = data_dir.clone();
                std::thread::Builder::new()
                    .name("planet-defs".into())
                    .spawn(move || {
                        let t = std::time::Instant::now();
                        let bakes = crate::engine::net_route::bake_planet_defs(&dir);
                        let took = t.elapsed();
                        log::info!(
                            "[Boot] planet surfaces prebuilt in background: {} ms ({} bodies)",
                            took.as_millis(),
                            bakes.len()
                        );
                        let _ = tx.send((bakes, took));
                    })
                    .ok()
                    .map(|_| rx)
            };
            // The starter vehicles of a brand-new home (engine/sim_boot.rs).
            crate::engine::sim_boot::starter_vehicles(&mut game_world, &data_store);
            gui_state.market_categories = crate::gui::load_market_categories(&data_dir);
            gui_state.studio_scene_presets = crate::gui::load_studio_scenes(&data_dir);
            gui_state.studio_source_presets = crate::gui::load_studio_sources(&data_dir);
            gui_state.profile_skills = crate::gui::load_default_player_skills(&data_dir);
            gui_state.studio_streaming_config = crate::gui::load_studio_streaming_config(&data_dir);
            gui_state.donate_faq = crate::gui::load_donate_faq(&data_dir);
            gui_state.donate_methods = crate::gui::load_donate_methods(&data_dir);
            gui_state.donate_charities = crate::gui::load_donate_charities(&data_dir);
            gui_state.donate_routes = crate::gui::load_donate_routes(&data_dir);
            gui_state.qa_test_tasks = crate::gui::load_qa_test_tasks(&data_dir);
            gui_state.web_sites = crate::gui::load_web_sites(&data_dir); // data/web/sites.json, both clients
            gui_state.web_view.rules = crate::gui::load_web_read_rules(&data_dir);
            // v0.197.0: ai_usage_filters loader removed.
            // v0.415.0: resource_categories + onboarding concepts/core-pages
            // loaders removed with their retired pages.
            // Note: TaskPageState reads data/tasks/default_projects.json itself
            // on its first lazy-init (it's a thread-local in pages/tasks.rs).
            // Populate the live studio state from the loaded presets.
            gui_state.studio.sources = gui_state
                .studio_source_presets
                .iter()
                .map(crate::gui::studio_source_from_preset)
                .collect();
            gui_state.studio.scenes = gui_state
                .studio_scene_presets
                .iter()
                .map(crate::gui::studio_scene_from_preset)
                .collect();
            // Prime the Program/Preview split: both panes start on scene 0, with
            // program holding a frozen copy of the just-loaded source arrangement
            // so the Program pane renders sensibly before the first manual Cut.
            gui_state.studio.cut_to_program();
            // Default to Earth if present.
            gui_state.map_selected_planet = gui_state
                .map_planets
                .iter()
                .position(|p| p.name == "Earth")
                .or(if gui_state.map_planets.is_empty() { None } else { Some(0) });

            // Load persistent config and apply to GUI state. Fresh installs
            // (no config.json yet) SKIP the apply so GuiState keeps its
            // designed defaults -- AppConfig's derived Default is all-zeroes
            // and used to stomp first boots to fov 0 / planet detail off /
            // no atmosphere / no clouds (see AppConfig::load_if_exists).
            if let Some(config) = boot_config {
                config.apply_to_gui_state(&mut gui_state);
            } else if crate::engine::low_impact::on() {
                // A rig sandbox with no config.json yet still runs light: the overlay over the
                // built-in defaults (none of it is ever saved back; engine::low_impact).
                crate::engine::low_impact::lay_over_config(crate::config::AppConfig::from_gui_state(&gui_state))
                    .apply_to_gui_state(&mut gui_state);
            }
            // The proof a rig waits for (debug/low_impact.json) and the run.log line. A no-op in
            // a normal game.
            crate::engine::low_impact::write_confirmation();
            gui_state.save_slot = boot_slot;
            // The protected setup has its own file, read whether or not config.json exists or reads (10h).
            gui_state.protected.setup = crate::config::AppConfig::load_protected_setup();
            // The save's progress, now that the settings have landed (engine/sim_boot.rs).
            crate::engine::sim_boot::apply_home_save(&mut game_world, &mut data_store, &mut gui_state, home_save.as_ref(), &data_dir);
            // Bring the self-hosted relay node back up if it was running at
            // last exit (host_node_autostart, armed by Start / disarmed by
            // Stop). Must run after the config lands so port/db/name match.
            // A script-launched instance holds it back until the first click
            // (no firewall prompt from an agent's boot; see host_node::autostart).
            crate::gui::pages::host_node::autostart_if_configured(&gui_state, crate::engine::launch_focus::launch_in_background());
            // Push the LOADED settings into the engine on the first frame. Without this the
            // camera boots at CameraController::new's hardcoded sensitivity (and the camera
            // FOV / far-plane stay at their constructor defaults) until the user nudges a
            // slider, so a saved low/high mouse sensitivity never took effect on startup.
            // The settings_dirty block (in the render loop) applies fov + sensitivity +
            // window mode + render distance from gui_state.settings, so trip it once here.
            gui_state.settings_dirty = true;

            // Downloadable star catalogs installed? (v0.800 extended;
            // 2026-07-11 ultra) One metadata stat per tier at boot;
            // downloads/removals update the fields themselves.
            for tier in crate::renderer::stars::StarCatalogTier::ALL {
                gui_state.star_catalog_installed[tier.index()] =
                    std::fs::metadata(data_dir.join(tier.filename())).ok().map(|m| m.len());
            }
            // Ultra Milky Way glow texture installed? (2026-07-11) Same
            // one-stat-at-boot pattern as the catalog tiers above.
            gui_state.galaxy_glow_installed =
                std::fs::metadata(data_dir.join(crate::renderer::stars::GALAXY_GLOW_ULTRA_FILE))
                    .ok()
                    .map(|m| m.len());

            // Clean up .old files from previous updates
            crate::updater::Updater::cleanup_old_versions();
            // A restart after an update comes back behind when this launch was (launch_focus).
            gui_state.updater.relaunch_behind = crate::engine::launch_focus::launch_in_background();
            gui_state.updater.relaunch_quiet = crate::engine::launch_focus::quiet_asked();

            // Auto-check for updates on startup (if enabled)
            if gui_state.updater.channel == crate::updater::UpdateChannel::AlwaysLatest {
                gui_state.updater.check_now();
            }

            // Post-identity routing (v0.198.0, v0.220.0 boot page; v0.415.0 the
            // standalone onboarding page is retired, the Mission Dashboard is the
            // first landing):
            //   - !onboarding_complete: stay on MainMenu (identity / seed setup)
            //   - onboarding_complete && !concept_tour_seen: land on Humanity once
            //     (and mark the tour seen — the dashboard IS the orientation now)
            //   - onboarding_complete && concept_tour_seen: user's chosen boot page
            if gui_state.onboarding_complete {
                let first = if gui_state.concept_tour_seen {
                    gui_state.default_page
                } else {
                    gui_state.concept_tour_seen = true;
                    GuiPage::Humanity
                };
                // The first screen is a place the app opens by itself (gui/history.rs).
                gui_state.go(first, crate::gui::How::Replace);
            }

            self.state = Some(EngineState {
                // None for a background (script-launched) instance until a
                // person clicks into it (engine::launch_focus::launch_audio).
                audio: crate::engine::launch_focus::launch_audio(),
                // The data folder the app resolved, not a relative "data" (BUG-268).
                sound_catalog: humanity_core::audio::sounds::SoundCatalog::load(&data_dir),
                audio_volumes_applied: [-1.0; 6],
                music: Default::default(),
                animation: Default::default(),
                interact: Default::default(),
                tree_atlas_attempted: false,
                near_tree_retry_s: 0.0,
                particle_system: {
                    let mut ps = crate::renderer::particles::ParticleSystem::new();
                    ps.load_defs(&data_dir);
                    ps
                },
                prev_ship_world_pos: glam::DVec3::ZERO,
                house_light_consumer: None,
                stride_accum: 0.0,
                prev_stride_anchor: glam::DVec3::ZERO,
                prev_stride_cam: Vec3::ZERO,
                debug_test_light_count: 0,
                debug_test_light_intensity: 3.0,
                window,
                renderer,
                camera,
                controller,
                asset_manager,
                hot_reload,
                game_world,
                system_runner,
                data_store,
                // 3D world state: empty defaults, loaded lazily on first Enter World
                star_renderer: None,
                star_preload_rx,
                planet_defs_preload_rx,
                terrain_tiles: humanity_terrain::terrain_tiles::TerrainTiles::new(
                    data_dir.join("planets/earth_tiles"),
                ),
                map_meshes: Default::default(),
                // Connected-ocean mask (v0.876): bathymetric terrain + the
                // water shell need it; absent/corrupt file = the pre-v0.876
                // clamped sea sphere, never a crash.
                ocean_mask: match humanity_terrain::ocean_mask::OceanMask::load(
                    &data_dir.join("planets/earth_ocean_mask.bin"),
                ) {
                    Ok(m) => Some(m),
                    Err(e) => {
                        log::warn!("[Ocean] mask unavailable ({e}) - clamped sea sphere");
                        None
                    }
                },
                // Live weather (v0.874): spawn the NASA GIBS fetcher only
                // when the setting allows network use. The renderer's
                // weather texture stays zero (= procedural sky) until the
                // first grid arrives.
                weather_grid: None,
                weather_rx: if gui_state.settings.live_weather {
                    let (tx, rx) = std::sync::mpsc::channel();
                    crate::net::live_weather::spawn(tx);
                    Some(rx)
                } else {
                    None
                },
                current_spin: 0.0,
                current_sim_t: 0.0,
                station_world_pos: glam::DVec3::ZERO,
                station_world_rot: glam::DQuat::IDENTITY,
                station_def: humanity_ship::station::default_home(),
                station_ride: false,
                station_spawn_snap: false,
                station_off: Vec3::ZERO,
                aboard_station: false,
                planet_body_frame: None,
                flight_data: humanity_systems::flight::FlightData::load(&data_dir),
                flight: humanity_systems::flight::FlightState::default(),
                surface_walk_band: false,
                surface_owns_translation: false,
                planet_defs: std::collections::HashMap::new(),
                planet_heightmaps: std::collections::HashMap::new(),
                planet_albedos: std::collections::HashMap::new(),
                planet_mesh_cache: std::collections::HashMap::new(),
                sky_mesh_pending: std::collections::HashMap::new(),
                planet_chunk_states: std::collections::HashMap::new(),
                planet_patch_free_slots: Vec::new(),
                planet_surface_material: 0,
                planet_textured_materials: std::collections::HashMap::new(),
                planet_atmo_materials: std::collections::HashMap::new(),
                planet_cloud_materials: std::collections::HashMap::new(),
                planet_water_materials: std::collections::HashMap::new(),
                sun_world_pos: glam::DVec3::ZERO,
                sun_material: 0,
                solar_body_materials: [0; 8],
                solar_orbit_paths: Vec::new(),
                solar_orbit_paths_at: None,
                homestead_floors: Vec::new(),
                placeholder_objects: Vec::new(),
                machine_objects: Vec::new(),
                decoration_objects: Vec::new(),
                hero_plant_missing: std::collections::HashSet::new(),
                near_tree_new: Vec::new(),
                on_ground_planet: false,
                surface_vr: 0.0,
                last_ground_dir: glam::DVec3::ZERO,
                last_ground_r: 0.0,
                last_clearance: 0.0,
                near_tree_born_s: 0.0,
                machine_model_materials: std::collections::HashMap::new(),
                decoration_mesh_cache: std::collections::HashMap::new(),
                near_trees: Vec::new(),
                near_trees_center: glam::DVec3::splat(f64::MAX),
                near_grass: Vec::new(),
                near_grass_center: glam::DVec3::splat(f64::MAX),
                near_grass_depth: 0,
                near_tree_inputs: Default::default(),
                near_grass_at_s: -1.0,
                grass_instances: Vec::new(),
                grow_positions: Vec::new(),
                plant_objects: Vec::new(),
                plant_mesh_sig: 0,
                garden_draw: Default::default(),
                home_draw: Default::default(),
                machine_pick: Vec::new(),
                port_pick: Vec::new(),
                wall_colliders: Vec::new(),
                shut_doors: Vec::new(),
                pending_sfx: Vec::new(),
                pending_notice_sfx: Vec::new(),
                sight_colliders: Vec::new(),
                connection_objects: Vec::new(),
                connection_flow_paths: Vec::new(),
                connection_flow_sphere: None,
                port_node_mesh: None,
                rail_car_mesh: None,
                rail_car_mat: None,
                drone_dock_meshes: None,
                vehicle_meshes: None,
                structure_mesh: None,
                structure_mats: None,
                driving_vehicle: None,
                follow_vehicle: None,
                targeted_vehicle: None,
                vehicle_mats: None,
                stock_pile_mesh: None,
                stock_pile_mats: None,
                stock_pile_cache: (Default::default(), Vec::new()),
                targeted_livestock: None,
                pending_livestock_harvest: None,
                livestock_mesh: None,
                livestock_mats: std::collections::HashMap::new(),
                drone_dock_mats: None,
                drone_dock_anim: 1.0,
                flow_rgb_mats: Vec::new(),
                connection_cyl: None,
                connection_mats: std::collections::HashMap::new(),
                pipe_markers: Default::default(),
                door_panels: Vec::new(),
                door_manual_open: Vec::new(),
                door_locks: Vec::new(),
                door_swing: Vec::new(),
                door_panel_mesh: None,
                door_slab_mat: None,
                door_glass_mat: None,
                door_energy_open_mat: None,
                door_energy_locked_mat: None,
                door_nanowall_mat: None,
                door_anim_time: 0.0,
                avatar_obj_start: 0,
                avatar_base: Vec3::ZERO,
                fps_spawn: Vec3::new(0.0, 1.7, 0.0),
                showroom_backdrops: Vec::new(),
                showroom_ground: None,
                showroom_body: None,
                showroom_last_backdrop: usize::MAX,
                cosmetics: Vec::new(),
                showroom_return_pos: Vec3::new(0.0, 1.7, 0.0),
                cursor_free: false,
                ui_ipc: None,
                homestead_walls: None,
                homestead_material_walls: Vec::new(),
                homestead_trim: Vec::new(),
                homestead_windows: None,
                homestead_mirrors: None,
                homestead_ceiling: None,
                homestead_ceiling_opaque: None,
                homestead_ceiling_glass: false,
                homestead_roof_frame: None,
                homestead_hull: None,
                hull_profile: None,
                homestead_layout: None,
                construction_cam_active: false,
                construction_return_pos: Vec3::new(0.0, 1.7, 0.0),
                construction_entry_rebuild: false,
                cursor_pos: (0.0, 0.0),
                construction_grab: None,
                construction_ghost: None,
                construction_structure_ghost: None,
                teleport_cooldown: 0.0,
                elevator_state: std::collections::HashMap::new(),
                elevator_car_mesh: None,
                elevator_car_mat: None,
                wall_tool_mesh: None,
                wall_tool_mat: None,
                construction_node_grab: None,
                construction_object_grab: None,
                construction_port_drag: None,
                construction_snap_x: None,
                construction_snap_z: None,
                construction_grab_press: None,
                construction_node_mesh: None,
                construction_node_mat: None,
                construction_node_mat_hot: None,
                construction_light_mesh: None,
                construction_light_mat: None,
                construction_corridor_mouth_mat: None,
                light_fixture_sphere: None,
                light_fixture_panel: None,
                light_fixture_tube: None,
                light_fixture_mats: std::collections::HashMap::new(),
                light_fixture_mat_off: None,
                light_strip_meshes: std::collections::HashMap::new(),
                construction_wall_mat: None,
                construction_node_mat_hover: None,
                construction_char_grab: false,
                construction_char_mesh: None,
                construction_char_pyramid_mesh: None,
                construction_char_mat: None,
                construction_opening_grab: None,
                construction_opening_resize: None,
                construction_opening_mesh: None,
                construction_opening_mat: None,
                construction_gizmo_grab: None,
                construction_gizmo_handle: None,
                construction_gizmo_resize_handle: None,
                construction_hilite: None,
                net_sync: crate::net::sync::NetSyncSystem::new(),
                game_joined: false,
                game_welcomed: false,
                copresence_refused: None,
                home_arrived_on: None,
                copresence_server: String::new(),
                boot_plot: None,
                last_welcome: None,
                last_welcome_rejoin: None,
                game_pos_timer: 0.0,
                remote_avatar: None,
                remote_look_materials: std::collections::HashMap::new(),
                remote_npc_avatar: None,
                hologram_objects: Vec::new(),
                hologram_orbits: Vec::new(),
                hologram_pins: Vec::new(),
                targeted_planet: None,
                hologram_room_center: Vec3::new(-0.5, 1.0, 2.5),
                room_lights: Vec::new(),
                homestead_bounds: None,
                ship_air: Default::default(),
                aboard_bounds: None,
                moves: Default::default(),
                map_view: Default::default(),
                shared_build: Default::default(),
                death_packs: Default::default(),
                screenshot_counter: 0,
                ship_world_pos: glam::DVec3::ZERO,
                dev_travel_home: None,
                dev_travel_stepped_out: false,
                frame_lock_body: None,
                probe_hold: None,
                station_park: None,
                draw_bufs: Default::default(),
                probe_descend_mps: 0.0,
                cloud_ref_frame: None,
                cloud_map_anchor: None,
                cloud_map_reanchors: 0,
                weather_anchor: None,
                cloud_map_regime: 0,
                cloud_prev_cam_local: None,
                cloud_prev_rot: None,
                sea_state_override: None,
                // Rig determinism pins, both off by default (showcase_request
                // {"wind":...} / {"anim_clock":...} turn them on).
                foliage_wind_override: None,
                fov_pin: None,
                anim_clock_pin: None,
                ocean_event_pin_request: None,
                ocean_event_pin: None,
                cloud_cover_override: None,
                cloud_type_override: None,
                ocean_fft_wind: 8.0,
                ocean_fft_built_wind: 8.0,
                ocean_fft_rebuild_cooldown: 0.0,
                bookmark_save_requested: false,
                frame_lock_anchor: glam::DVec3::ZERO,
                frame_lock_last_rot: glam::DQuat::IDENTITY,
                start_time: Instant::now(),
                last_frame: Instant::now(),
                window_focused: true,
                tray: Default::default(),
                // Same policy as window creation (cached, so they always
                // agree): a script-launched boot must not grab the cursor
                // either. See src/engine/launch_focus.rs.
                background_no_cursor: crate::engine::launch_focus::launch_in_background(),
                far_tree_mesh: None,
                far_tree_anchor: glam::DVec3::ZERO,
                far_tree_built_cam: glam::DVec3::splat(1.0e30),
                far_tree_pending: None,
                ocean_fft: None,
                cloud_advect: 0.0,
                cloud_advect_decaying: 0.0,
                cloud_event_boost: 0.0,
                cloud_event_tint: [1.0; 3],
                event_core: None,
                event_hazard_near: false,
                egui_ctx,
                egui_state,
                egui_renderer,
                gui_state,
                theme,
                screens: crate::engine::screens::Screens::default(),
                world_loaded: false,
                boot_timer,
                data_dir,
                ctrl_held: false,
                shift_held: false,
                alt_held: false,
                lmb_held: false,
                construction_history: ConstructionHistory::default(),
                live_publisher: None,
                movie: None,
                stream_capture: crate::renderer::stream_capture::StreamCapture::new(),
            });
            // Interior (ship / homestead) gravity from data/game.csv, the
            // first of that file's knobs to gain a real reader (v0.1119).
            // Missing/garbled knob keeps the controller's compiled-in 1 g.
            // The hot-reload poll re-applies this on every game.csv save.
            if let Some(st) = self.state.as_mut() {
                if let Some(g) = st.asset_manager.game_setting_f64("gravity_m_s2") {
                    st.controller.set_interior_gravity(g as f32);
                }
            }
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
                state.egui_state.on_window_event(&state.window, &event).consumed
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
                    state.renderer.resize(size.width, size.height);
                    state.camera.aspect = state.renderer.aspect_ratio();
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
                    crate::engine::frame_station::run(state);
                    crate::engine::frame_move_setup::run(state);
                    crate::engine::frame_walk_aboard::run(state, &mut f);
                    crate::engine::frame_particles::run(state, &mut f);
                    crate::engine::frame_house_power::run(state, &mut f);
                    crate::engine::frame_planet_walk::run(state, &mut f);
                    crate::engine::frame_collide_drive::run(state, &mut f);
                    crate::engine::frame_look_env::run(state);
                    crate::engine::frame_bridges::run(state);
                    crate::engine::frame_bridges_npc::run(state);
                    crate::engine::frame_ecs::run(state, &mut f);
                    crate::engine::frame_copresence::run(state, &mut f);
                    crate::engine::frame_save_editor::run(state);
                    crate::engine::frame_lists_home::run(state, &mut f);
                    crate::engine::frame_build_ghosts::run(state, &mut f);
                    crate::engine::frame_build_gizmos::run(state, &mut f);
                    crate::engine::frame_build_helpers::run(state, &mut f);
                    crate::engine::frame_people::run(state, &mut f);
                    crate::engine::frame_sky::run(state, &mut f);
                    crate::engine::frame_hud_voice::run(state, &mut f);
                    crate::engine::frame_settles::run(state);
                    crate::engine::frame_mirrors::run(state);
                    crate::engine::frame_mirrors_world::run(state, &mut f);
                    crate::engine::frame_lists_world::run(state, &mut f);
                    crate::engine::frame_env_publish::run(state, &mut f);
                    crate::engine::frame_cards::run(state);
                    crate::engine::frame_dev_ipc::run(state, &mut f);
                    crate::engine::frame_load_screens::run(state, &mut f);
                    let page_active = f.page_active;
                    let scene_result = crate::engine::frame_world_passes::run(state, &mut f);
                    if page_active {
                        humanity_core::frame_costs::span_stop_recording(); // past the acquire on a frame a page covers (frame_total's rule)
                    }
                    match scene_result {
                        Ok((surface_texture, view)) => {
                            let full_output = crate::engine::frame_ui_run::run(state, event_loop);
                            crate::engine::frame_audio::run(state, &full_output);
                            crate::engine::frame_ui_paint::run(state, full_output, &view);
                            crate::engine::frame_tail::run(state, &mut f, surface_texture);
                        }
                        Err(wgpu::SurfaceError::Lost) => {
                            let size = state.window.inner_size();
                            state.renderer.resize(size.width, size.height);
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
