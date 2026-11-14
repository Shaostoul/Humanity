//! A developer's script boot is HIDDEN from the operator's screen (2026-11-14).
//!
//! WHY. The operator, 2026-11-14: "It appears the game often loads up then cycles through a bunch
//! of static scenes then turns off. What's happening? Is a test being ran or something?" Those were
//! agents' rigs (probe-sweep vantages, verify-runtime, the screens rigs). Since v0.1081 a script's
//! boot never takes the focus (engine::launch_focus), but its window was still a full-screen window
//! at the bottom of the Z order and a button on his taskbar: seen whenever he uncovered the desktop.
//!
//! WHAT. A boot that is in the background AND was asked to be quiet by a developer's launcher
//! (HUMANITY_NO_FOCUS or the no_focus.txt marker, `launch_focus::quiet_asked`: every rig through
//! scripts/lib/game-launch.js spawnGame, and `just launch-bg`) is, before its first show:
//!   1. CLOAKED by the Desktop Window Manager. Microsoft's DWMWINDOWATTRIBUTE reference
//!      (learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwmwindowattribute, page
//!      dated 2024-01-21, read 2026-11-14), DWMWA_CLOAK: "Cloaks the window such that it is not
//!      visible to the user. The window is still composed by DWM." Composed means the swapchain keeps
//!      presenting: every frame is still drawn and the dev screenshots still come out (proved with
//!      pixel-equal captures and frame costs against a shown boot, docs/ai/game-boots.md).
//!   2. Made a TOOL WINDOW (WS_EX_TOOLWINDOW set, WS_EX_APPWINDOW cleared), because a cloaked window
//!      KEEPS its taskbar button (measured 2026-11-14 on Windows 10 19045: a cloaked test window
//!      still added "<title> - 1 running window" to the taskbar; cloaked + tool window added none).
//!      Microsoft's Extended Window Styles reference (learn.microsoft.com/en-us/windows/win32/winmsg/
//!      extended-window-styles, dated 2025-07-14), WS_EX_TOOLWINDOW: "A tool window does not appear
//!      in the taskbar or in the dialog that appears when the user presses ALT+TAB." winit sets
//!      WS_EX_APPWINDOW on every unowned window, which "Forces a top-level window onto the taskbar
//!      when the window is visible", so it is cleared. The Taskbar page (learn.microsoft.com/en-us/
//!      windows/win32/shell/taskbar, dated 2018-05-31) says why this must happen while the window is
//!      still hidden: "If you want to dynamically change a window's style to one that does not
//!      support visible taskbar buttons, you must hide the window first".
//! The window's client keeps exactly the size a shown boot gets (launch_focus::show_window_behind
//! measures the frame with the ordinary style, then `fit_client` frames that client with the thinner
//! tool-window frame), so a picture rig photographs the same pixels.
//!
//! The options weighed (docs/ai/game-boots.md has the table): minimizing stops presenting (wgpu has
//! no surface to draw to at zero size, and the screenshot capture breaks); a position off every
//! monitor keeps the taskbar button and Alt+Tab entry; ITaskbarList::DeleteTab (winit's
//! set_skip_taskbar) removes only the button, not the Alt+Tab entry, and the shell adds it back on
//! the next show; a layered window at alpha 0 changes how the swapchain presents. Cloak plus tool
//! window is the only pair that hides the picture, the button and the Alt+Tab entry while every
//! frame is still presented.
//!
//! WHO IS NEVER HIDDEN. A human launch (`just play`, `just launch`, a double-click: the operator's
//! focus opt-in, launch_focus.rs step 1, or explorer.exe as the parent) is foreground and never
//! hidden. A background launch nobody asked to be quiet (a person's launcher, Steam, a terminal)
//! stays visible behind the other windows, as before: hiding it would leave a person with a game
//! running and nothing to see.
//!
//! THE ESCAPE HATCH. HUMANITY_RIG_VISIBLE=1 keeps a rig's window visible behind the others, for a
//! rig that genuinely needs one on screen (one that tests the taskbar button, Alt+Tab, or what a
//! person sees of the window frame). It only stops the hiding: it never brings the window forward
//! or gives it the focus, which stays the operator's alone (tests/focus_optin_lint.rs, and the test
//! `rig_visible_never_reaches_the_focus_decision` below).

use std::sync::OnceLock;

/// The escape hatch: a rig that needs its window visible (behind the others, never focused).
pub const RIG_VISIBLE_ENV: &str = "HUMANITY_RIG_VISIBLE";

/// Should this boot's window be hidden from the screen? Pure, so the rule is tested:
///   background   launch_focus::launch_in_background() (no proof of a human launch)
///   quiet_asked  launch_focus::quiet_asked() (a developer's launcher asked for a quiet boot)
///   rig_visible  HUMANITY_RIG_VISIBLE=1 (the escape hatch)
pub fn decide(background: bool, quiet_asked: bool, rig_visible: bool) -> bool {
    background && quiet_asked && !rig_visible
}

/// Whether HUMANITY_RIG_VISIBLE asks for a visible window ("1", or any value but "" and "0").
fn rig_visible_asked() -> bool {
    std::env::var(RIG_VISIBLE_ENV).map(|v| !v.is_empty() && v != "0").unwrap_or(false)
}

/// This run's answer to `decide`, logged once. Cached: the window is hidden before its first show
/// and again on every show from the tray, and both must agree.
pub fn hide_from_screen() -> bool {
    // reuse: not a reuse: read once per run from the environment and the launch decision, which do not change mid-run.
    static CACHE: OnceLock<bool> = OnceLock::new();
    *CACHE.get_or_init(|| {
        let background = crate::engine::launch_focus::launch_in_background();
        let quiet = crate::engine::launch_focus::quiet_asked();
        let visible = rig_visible_asked();
        let hide = decide(background, quiet, visible);
        if hide {
            log::info!("window: hidden from the screen (a developer's script boot: cloaked, no taskbar button, no Alt+Tab entry; every frame still drawn). {RIG_VISIBLE_ENV}=1 keeps it visible behind instead.");
        } else if background && quiet && visible {
            log::info!("window: {RIG_VISIBLE_ENV} set: this script boot stays visible behind the other windows (never focused)");
        }
        hide
    })
}

/// What the window itself reports: DWM's cloak reason and the extended style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowState {
    /// DWMWA_CLOAKED: 0 shown, else why it is cloaked (1 = by its own application).
    pub cloaked: u32,
    /// GWL_EXSTYLE.
    pub ex_style: u32,
    /// The client rect's size.
    pub client: (i32, i32),
}

/// WS_EX_TOOLWINDOW: no taskbar button, no Alt+Tab entry.
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
/// WS_EX_APPWINDOW: forces a taskbar button; winit sets it on every unowned window.
pub const WS_EX_APPWINDOW: u32 = 0x0004_0000;
/// DWMWA_CLOAK (DWMWINDOWATTRIBUTE: DWMWA_NCRENDERING_ENABLED is 1, DWMWA_CLOAK the 13th).
pub const DWMWA_CLOAK: u32 = 13;
/// DWMWA_CLOAKED, the read-back of DWMWA_CLOAK.
pub const DWMWA_CLOAKED: u32 = 14;

impl WindowState {
    /// Hidden the way `hide_before_show` hides: cloaked, a tool window, and not forced onto the taskbar.
    pub fn hidden(&self) -> bool {
        self.cloaked != 0 && self.ex_style & WS_EX_TOOLWINDOW != 0 && self.ex_style & WS_EX_APPWINDOW == 0
    }
}

/// The extended style a hidden window gets: a tool window, never forced onto the taskbar.
pub fn hidden_ex_style(ex_style: u32) -> u32 {
    (ex_style | WS_EX_TOOLWINDOW) & !WS_EX_APPWINDOW
}

#[cfg(target_os = "windows")]
mod win {
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    pub struct RectI32 {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }
    #[link(name = "dwmapi")]
    extern "system" {
        pub fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const core::ffi::c_void, size: u32) -> i32;
        pub fn DwmGetWindowAttribute(hwnd: isize, attr: u32, value: *mut core::ffi::c_void, size: u32) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        pub fn GetWindowLongW(hwnd: isize, index: i32) -> i32;
        pub fn SetWindowLongW(hwnd: isize, index: i32, value: i32) -> i32;
        pub fn GetClientRect(hwnd: isize, rect: *mut RectI32) -> i32;
        pub fn AdjustWindowRectEx(rect: *mut RectI32, style: u32, menu: i32, ex_style: u32) -> i32;
        pub fn SetWindowPos(hwnd: isize, insert_after: isize, x: i32, y: i32, w: i32, h: i32, flags: u32) -> i32;
    }
    pub const GWL_STYLE: i32 = -16;
    pub const GWL_EXSTYLE: i32 = -20;
}

/// Cloak the window and make it a tool window. Called while the window is still HIDDEN (before its
/// first ShowWindow, and before each show from the tray), so no frame and no taskbar button ever
/// appear. A cloak DWM refuses is logged; the window then shows behind the others as before.
#[cfg(target_os = "windows")]
pub fn hide_before_show(hwnd: isize) {
    unsafe {
        let on: i32 = 1;
        let hr = win::DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &on as *const i32 as *const core::ffi::c_void, 4);
        if hr < 0 {
            log::warn!("window: DWM refused the cloak (HRESULT 0x{:08X}): this script boot will be visible behind the other windows", hr as u32);
        }
        let ex = win::GetWindowLongW(hwnd, win::GWL_EXSTYLE) as u32;
        win::SetWindowLongW(hwnd, win::GWL_EXSTYLE, hidden_ex_style(ex) as i32);
    }
}

/// Give the window a client of `cw` x `ch`, framed by its CURRENT style (a hidden window's tool
/// frame is thinner than the ordinary one), with the client's top-left at `origin` (None: where it
/// is). SWP_FRAMECHANGED makes the style change take effect, as Microsoft's SetWindowLong page asks.
#[cfg(target_os = "windows")]
pub fn fit_client(hwnd: isize, origin: Option<(i32, i32)>, cw: i32, ch: i32) {
    const SWP_NOMOVE: u32 = 0x2;
    const SWP_NOZORDER: u32 = 0x4;
    const SWP_NOACTIVATE: u32 = 0x10;
    const SWP_FRAMECHANGED: u32 = 0x20;
    unsafe {
        let style = win::GetWindowLongW(hwnd, win::GWL_STYLE) as u32;
        let ex = win::GetWindowLongW(hwnd, win::GWL_EXSTYLE) as u32;
        let mut r = win::RectI32 { left: 0, top: 0, right: cw, bottom: ch };
        if win::AdjustWindowRectEx(&mut r, style, 0, ex) == 0 {
            log::warn!("window: could not measure the hidden window's frame; its client may differ from a shown boot's");
            return;
        }
        let (x, y, more) = match origin {
            Some((ox, oy)) => (ox + r.left, oy + r.top, 0),
            None => (0, 0, SWP_NOMOVE),
        };
        win::SetWindowPos(hwnd, 0, x, y, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED | more);
    }
}

/// What the window reports about itself right now (DWM's cloak read-back, its extended style, its
/// client size): measured from the window, never from what this module asked for.
#[cfg(target_os = "windows")]
pub fn read_state(hwnd: isize) -> WindowState {
    unsafe {
        let mut cloaked: u32 = 0;
        if win::DwmGetWindowAttribute(hwnd, DWMWA_CLOAKED, &mut cloaked as *mut u32 as *mut core::ffi::c_void, 4) < 0 {
            cloaked = 0;
        }
        let ex_style = win::GetWindowLongW(hwnd, win::GWL_EXSTYLE) as u32;
        let mut r = win::RectI32::default();
        win::GetClientRect(hwnd, &mut r);
        WindowState { cloaked, ex_style, client: (r.right - r.left, r.bottom - r.top) }
    }
}

/// Undo `hide_before_show` (uncloaked, the ordinary taskbar style back) while the window is hidden:
/// the dev request's visible phase, which then shows it the ordinary background way.
#[cfg(target_os = "windows")]
pub fn unhide_before_show(hwnd: isize) {
    unsafe {
        let off: i32 = 0;
        win::DwmSetWindowAttribute(hwnd, DWMWA_CLOAK, &off as *const i32 as *const core::ffi::c_void, 4);
        let ex = win::GetWindowLongW(hwnd, win::GWL_EXSTYLE) as u32;
        win::SetWindowLongW(hwnd, win::GWL_EXSTYLE, ((ex & !WS_EX_TOOLWINDOW) | WS_EX_APPWINDOW) as i32);
    }
}

#[cfg(target_os = "windows")]
fn hwnd_of(window: &winit::window::Window) -> Option<isize> {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
        _ => None,
    }
}

/// The dev request that shows a hidden boot's window and hides it again IN THE SAME RUN
/// (2026-11-14), so one boot measures hidden, visible, hidden (screenshots and frame costs at the
/// same vantage in each phase) instead of three boots. Drop `debug/window_request.json` as
/// `{"visible": true}` or `{"visible": false}`; `debug/window_done.json` answers with what the
/// window then reports. The visible phase is the ordinary background show
/// (`launch_focus::show_window_behind_as`: the bottom of the Z order, never activated, never on top);
/// keep it as short as the measurement needs. Only a hidden script boot answers it: a person's game
/// (never hidden) refuses, so the request can never show or move anyone's own window.
pub(crate) fn poll_request(window: &winit::window::Window) {
    const REQUEST: &str = "debug/window_request.json";
    const DONE: &str = "debug/window_done.json";
    let Some(text) = crate::engine::ipc::take_request_text(std::path::Path::new(REQUEST)) else {
        return;
    };
    let done = answer_request(window, &text);
    let _ = std::fs::create_dir_all("debug");
    let _ = std::fs::write(DONE, done.to_string());
}

fn answer_request(window: &winit::window::Window, text: &str) -> serde_json::Value {
    let visible = match serde_json::from_str::<serde_json::Value>(text).ok().and_then(|v| v.get("visible").and_then(|b| b.as_bool())) {
        Some(v) => v,
        None => return serde_json::json!({"ok": false, "error": "expected {\"visible\": true} or {\"visible\": false}"}),
    };
    if !hide_from_screen() {
        return serde_json::json!({"ok": false, "error": "only a hidden script boot answers this (src/engine/rig_window.rs); this window is not one"});
    }
    #[cfg(target_os = "windows")]
    {
        const SW_HIDE: i32 = 0;
        #[link(name = "user32")]
        extern "system" {
            fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        }
        let Some(hwnd) = hwnd_of(window) else {
            return serde_json::json!({"ok": false, "error": "no window handle"});
        };
        // Hidden first, as the Taskbar page asks of a style change, then shown the ordinary
        // background way (visible) or hidden again (cloaked, tool window) before the show.
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        if visible {
            unhide_before_show(hwnd);
        }
        crate::engine::launch_focus::show_window_behind_as(window, !visible);
        let s = read_state(hwnd);
        log::info!("window: dev request: now {} (cloaked=0x{:X}, ex_style=0x{:08X}, client {}x{})", if visible { "VISIBLE behind the other windows" } else { "hidden" }, s.cloaked, s.ex_style, s.client.0, s.client.1);
        serde_json::json!({"ok": true, "visible": visible, "hidden": s.hidden(), "cloaked": s.cloaked, "ex_style": format!("0x{:08X}", s.ex_style), "client": [s.client.0, s.client.1]})
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, visible);
        serde_json::json!({"ok": false, "error": "windows only"})
    }
}

/// One run.log line with what the window reports after its show, and a warning when a boot meant
/// to be hidden is not (scripts/lib/window-state.js checks the same from outside the game).
#[cfg(target_os = "windows")]
pub fn log_state(hwnd: isize, meant_hidden: bool) {
    let s = read_state(hwnd);
    if meant_hidden && !s.hidden() {
        log::warn!("window: meant to be HIDDEN but the window reports cloaked=0x{:X} ex_style=0x{:08X}: it may show on the screen", s.cloaked, s.ex_style);
    } else {
        log::info!(
            "window: reports cloaked=0x{:X}, ex_style=0x{:08X} ({}), client {}x{}",
            s.cloaked,
            s.ex_style,
            if s.hidden() { "hidden: tool window, no taskbar button" } else { "shown" },
            s.client.0,
            s.client.1
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule. Seen RED first (2026-11-14): with `decide` stubbed to `false` the first assertion
    /// failed ("a developer's quiet background boot must be hidden"), the others passed.
    #[test]
    fn only_a_quiet_background_boot_is_hidden() {
        assert!(decide(true, true, false), "a developer's quiet background boot must be hidden");
        // The human path never hides, whatever else is set.
        for quiet in [false, true] {
            for visible in [false, true] {
                assert!(!decide(false, quiet, visible), "a human launch must never be hidden (quiet={quiet}, visible={visible})");
            }
        }
        // A background launch nobody asked to be quiet (a person's launcher, Steam, a terminal).
        assert!(!decide(true, false, false), "a person's background launch stays visible");
        // The escape hatch.
        assert!(!decide(true, true, true), "HUMANITY_RIG_VISIBLE keeps a rig's window visible");
    }

    #[test]
    fn the_hidden_style_is_a_tool_window_off_the_taskbar() {
        // winit's ordinary window: WS_EX_WINDOWEDGE | WS_EX_ACCEPTFILES | WS_EX_APPWINDOW.
        let ordinary = 0x0000_0100 | 0x0000_0010 | WS_EX_APPWINDOW;
        let hidden = hidden_ex_style(ordinary);
        assert_eq!(hidden & WS_EX_TOOLWINDOW, WS_EX_TOOLWINDOW);
        assert_eq!(hidden & WS_EX_APPWINDOW, 0);
        assert_eq!(hidden & 0x110, 0x110, "the other bits are kept");
        assert!(WindowState { cloaked: 1, ex_style: hidden, client: (1, 1) }.hidden());
        assert!(!WindowState { cloaked: 0, ex_style: hidden, client: (1, 1) }.hidden(), "not cloaked: on screen");
        assert!(!WindowState { cloaked: 1, ex_style: ordinary, client: (1, 1) }.hidden(), "a taskbar button");
    }

    /// HUMANITY_RIG_VISIBLE only stops the hiding: the focus decision never reads it, so no variable
    /// an agent sets can make a boot take the focus.
    #[test]
    fn rig_visible_never_reaches_the_focus_decision() {
        let src = include_str!("launch_focus.rs");
        assert!(!src.contains(RIG_VISIBLE_ENV) && !src.contains("RIG_VISIBLE_ENV"), "launch_focus.rs must not read {RIG_VISIBLE_ENV}");
    }
}
