//! Launch-focus policy: does this boot deserve the foreground?
//!
//! The operator has ONE computer and ONE screen. Every time an agent boots
//! HumanityOS and the window takes focus, he is yanked out of a game or a
//! video and has to click back by hand. Three prior layers each closed one
//! hole and left another: the HUMANITY_NO_FOCUS env var (v0.828) is
//! forgotten by hand-rolled boots, create-visible-inactive (v0.1069) only
//! helps when the env var was seen, and the no_focus.txt marker (v0.1079)
//! does not travel when an agent copies a bare exe into a fresh dir or
//! builds in a worktree whose target/release never got one. All three made
//! BACKGROUND the special case somebody had to remember to request.
//!
//! This module inverts the default: FOCUS now requires PROOF of a human
//! launch. A script cannot provide that proof, so a script boot can never
//! steal focus again, no matter what its author forgot to set.
//!
//! Decision order (first match wins):
//!   1. HUMANITY_TAKE_FOCUS set -> foreground. The operator's own recipes
//!      (`just play`, `just launch`) set it, and the updater's restart
//!      script propagates it from a foreground instance.
//!   2. HUMANITY_NO_FOCUS set, or no_focus.txt beside the exe -> background.
//!      Kept from v0.1079 so probe rigs and portable sandboxes stay
//!      explicitly quiet even if parent detection ever misfires.
//!   3. Parent process is explorer.exe -> foreground. A real double-click
//!      or taskbar pin is the one launch path a human drives directly;
//!      agents always arrive via bash/node/powershell/cmd/cargo instead.
//!      This is also what keeps the downloaded exe working for real users:
//!      they double-click, they get a focused window, no setup needed.
//!   4. Anything else -> BACKGROUND. This line is the inversion.
//!
//! Non-Windows, or the process-snapshot API failing: fall back to the old
//! rule (foreground unless step 2 said background), so nothing regresses
//! on platforms where we cannot ask who launched us.

use std::sync::OnceLock;

/// True when this instance must open BEHIND the current foreground window
/// and never grab the cursor. Cached: the answer cannot change mid-run and
/// both call sites (window creation, cursor policy) must agree.
pub fn launch_in_background() -> bool {
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        if std::env::var("HUMANITY_TAKE_FOCUS").is_ok() {
            log::info!("focus: HUMANITY_TAKE_FOCUS set -> foreground");
            return false;
        }
        let marker = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("no_focus.txt").exists()))
            .unwrap_or(false);
        if std::env::var("HUMANITY_NO_FOCUS").is_ok() || marker {
            log::info!("focus: HUMANITY_NO_FOCUS/marker -> background");
            return true;
        }
        decide_from_parent()
    })
}

#[cfg(target_os = "windows")]
fn decide_from_parent() -> bool {
    match parent_process_name() {
        Some(parent) => {
            let human = parent.eq_ignore_ascii_case("explorer.exe");
            log::info!(
                "focus: parent process '{}' -> {}",
                parent,
                if human { "foreground (human launch)" } else { "background (script launch)" }
            );
            !human
        }
        // Parent dead or snapshot failed -> BACKGROUND. A dead parent means
        // the launcher exited the moment it spawned us, and only scripts do
        // that (node spawn+unref, start /b, detached CI runners...).
        // explorer.exe is the shell: it is alive when a double-clicked child
        // boots, always. Getting this branch wrong the other way was a real
        // measured steal: the first hostile-boot test of this module spawned
        // via node detached+unref, node exited, the lookup came up empty,
        // the old fallback said foreground, and the window took focus at
        // sample 10 of 28.
        None => {
            log::info!("focus: parent unknown/dead -> background (script assumed)");
            true
        }
    }
}

// No parent detection off-Windows: keep the pre-inversion default
// (foreground unless the env var / marker asked for background) rather
// than opening every macOS/Linux desktop launch behind other windows.
#[cfg(not(target_os = "windows"))]
fn decide_from_parent() -> bool {
    false
}

/// Name of our parent process ("explorer.exe", "bash.exe", ...), via a
/// toolhelp32 process snapshot. Raw FFI so we add no new dependency.
/// Returns None if the parent already exited or the API fails.
#[cfg(target_os = "windows")]
fn parent_process_name() -> Option<String> {
    const TH32CS_SNAPPROCESS: u32 = 0x0000_0002;
    const INVALID_HANDLE_VALUE: isize = -1;

    #[repr(C)]
    struct ProcessEntry32W {
        dw_size: u32,
        cnt_usage: u32,
        th32_process_id: u32,
        th32_default_heap_id: usize,
        th32_module_id: u32,
        cnt_threads: u32,
        th32_parent_process_id: u32,
        pc_pri_class_base: i32,
        dw_flags: u32,
        sz_exe_file: [u16; 260],
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(dw_flags: u32, th32_process_id: u32) -> isize;
        fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        fn CloseHandle(handle: isize) -> i32;
        fn GetCurrentProcessId() -> u32;
    }

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let me = GetCurrentProcessId();
        let mut entry: ProcessEntry32W = std::mem::zeroed();
        entry.dw_size = std::mem::size_of::<ProcessEntry32W>() as u32;

        // One walk: remember every (pid -> name) and our own parent pid.
        // PIDs can be reused after a process dies, but explorer is
        // long-lived, and a reused pid at worst misreads a script parent
        // as another script -> still background, still safe.
        let mut names: std::collections::HashMap<u32, String> = std::collections::HashMap::new();
        let mut parent_pid: Option<u32> = None;
        let mut ok = Process32FirstW(snapshot, &mut entry);
        while ok != 0 {
            let len = entry.sz_exe_file.iter().position(|&c| c == 0).unwrap_or(260);
            let name = String::from_utf16_lossy(&entry.sz_exe_file[..len]);
            if entry.th32_process_id == me {
                parent_pid = Some(entry.th32_parent_process_id);
            }
            names.insert(entry.th32_process_id, name);
            ok = Process32NextW(snapshot, &mut entry);
        }
        CloseHandle(snapshot);
        names.remove(&parent_pid?)
    }
}

#[cfg(not(target_os = "windows"))]
fn parent_process_name() -> Option<String> {
    None
}

/// Show a background window WITHOUT activating it, at the bottom of the
/// Z order, SIZED THE WAY A MAXIMIZED WINDOW IS SIZED. winit's
/// `set_visible(true)` goes through ShowWindow(SW_SHOW), which activates --
/// and a window created VISIBLE is also activated by the system whenever no
/// foreground input lock is held. That second path was only proven on
/// 2026-07-31: the v0.1069 create-visible-inactive window measured as
/// foreground from sample 0 of a 24-sample trace while the operator was
/// away from the keyboard, with `background=true` logged correctly a moment
/// earlier. Every earlier "quiet boot" measurement happened while someone
/// was actively typing, which is exactly when Windows REFUSES the
/// foreground transfer -- the mechanism was broken all along and the input
/// lock was hiding it. Hidden creation + this call is the only combination
/// that never activates, no matter who is or is not at the keyboard.
/// Clicking the window still activates it normally, which is the operator's
/// opt-in to interact with a background instance.
///
/// THE SIZING HALF (v0.1095). `with_maximized(true)` is set on every launch
/// (src/lib.rs), but winit can only apply MAXIMIZED through ShowWindow, so a
/// window created hidden carries the request and never gets it: the
/// foreground path's `set_visible(true)` applies it, and this background
/// path did not. The window therefore opened at the `with_inner_size`
/// fallback of 1280x720 while `just play` opened at 2560x1387 -- so every
/// probe-rig capture was 1280x720, a THIRD of the operator's pixels, and
/// every resolution-dependent gate in tests/visual/vantages.json (backlit
/// canopy, crown gap, canopy overdraw) was being read at a width its own
/// text says makes it unjudgeable. SW_SHOWMAXIMIZED cannot be used to fix
/// it: maximize always activates. Instead reproduce the geometry Windows
/// itself uses when it maximizes -- window rect = the monitor work area
/// inflated by the resize-border thickness on all four edges, so the
/// borders hang off-screen and the CLIENT rect covers the work area edge to
/// edge with the caption bar still on screen. The resulting client size is
/// bit-identical to a real maximize, and SWP_NOACTIVATE means it never
/// touches the foreground.
#[cfg(all(feature = "native", target_os = "windows"))]
pub fn show_window_behind(window: &winit::window::Window) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    const SW_SHOWNOACTIVATE: i32 = 4;
    const HWND_BOTTOM: isize = 1;
    const SWP_NOSIZE: u32 = 0x1;
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SPI_GETWORKAREA: u32 = 0x0030;
    const GWL_STYLE: i32 = -16;
    const GWL_EXSTYLE: i32 = -20;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct RectI32 {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn SetWindowPos(
            hwnd: isize,
            insert_after: isize,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn SystemParametersInfoW(
            action: u32,
            ui_param: u32,
            pv_param: *mut core::ffi::c_void,
            win_ini: u32,
        ) -> i32;
        fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
        fn AdjustWindowRectEx(rect: *mut RectI32, style: u32, menu: i32, ex_style: u32) -> i32;
    }
    let Ok(handle) = window.window_handle() else {
        // No handle -> fall back to the activating show rather than leave
        // the window invisible forever.
        window.set_visible(true);
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        window.set_visible(true);
        return;
    };
    let hwnd = h.hwnd.get();
    unsafe {
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);

        // Work area = the monitor minus the taskbar: exactly what a maximize
        // targets. Failure here is not fatal, it just means the window keeps
        // its created size, so say so loudly rather than silently capturing
        // at 1280x720 again.
        let mut wa = RectI32::default();
        let got = SystemParametersInfoW(
            SPI_GETWORKAREA,
            0,
            &mut wa as *mut RectI32 as *mut core::ffi::c_void,
            0,
        );
        if got == 0 || wa.right <= wa.left || wa.bottom <= wa.top {
            SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOSIZE | SWP_NOMOVE | SWP_NOACTIVATE);
            log::warn!(
                "focus: SPI_GETWORKAREA failed -- window left at its created size, \
                 captures will NOT match a foreground launch"
            );
            return;
        }

        // Frame metrics from the window's OWN style, not from hardcoded
        // system metrics: AdjustWindowRectEx returns the outsets a client
        // rect needs, so `border` is the resize-border thickness and
        // `caption` is the title-bar height. A maximized window inflates by
        // `border` on every edge (borders off-screen, caption on-screen).
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let mut adj = RectI32::default();
        let (border, caption) = if AdjustWindowRectEx(&mut adj, style, 0, ex_style) != 0 {
            let b = adj.right.max(0);
            (b, (-adj.top - b).max(0))
        } else {
            (0, 0)
        };
        let wa_w = wa.right - wa.left;
        let wa_h = wa.bottom - wa.top;
        SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            wa.left - border,
            wa.top - border,
            wa_w + border * 2,
            wa_h + border * 2,
            SWP_NOACTIVATE,
        );
        log::info!(
            "focus: shown via SW_SHOWNOACTIVATE at bottom of Z order, \
             maximized-without-activating to the work area \
             ({}x{} work area, {} px border, {} px caption -> client {}x{})",
            wa_w,
            wa_h,
            border,
            caption,
            wa_w,
            wa_h - caption
        );
    }
}

#[cfg(all(feature = "native", not(target_os = "windows")))]
pub fn show_window_behind(window: &winit::window::Window) {
    window.set_visible(true);
}
