//! Closing to the tray (notifications rung 2, part 2, 2026-10-31; docs/design/notifications.md,
//! "The tray"): with Settings > Notifications > "Closing the window keeps HumanityOS running in the
//! tray" on (it is off by default, so closing still quits), the window's close button hides the
//! window instead.
//!
//! - **Hiding** first leaves the world and saves it, the way quitting saves (`quit::save_game`): the
//!   game is put back on the main menu and out of a server's shared world (`game_leave`, so nobody
//!   sees a frozen figure), so a hidden window never simulates or renders a world.
//! - **While hidden** no frame is drawn: `about_to_wait` stops asking for redraws and the
//!   RedrawRequested arm returns at once (src/lib.rs), so the GPU does nothing. Four times a
//!   second a tick reads and keeps the sockets (`frame_net::pump`, the frame's own network pump:
//!   reconnecting, every saved server's link), takes the tray's clicks, and plays a notice's
//!   sound, so notices still arrive.
//! - **The first time in a run**, a notice says HumanityOS is still running in the tray: a person
//!   who forgot the choice made months ago learns where it went, and is told once per run, never
//!   at every close.
//! - **Showing** it again: the tray's Open HumanityOS, a click on the icon, or a click on a notice.
//! - **Quit** from the tray saves and exits the way closing did (src/lib.rs `about_to_wait`, which
//!   runs `quit::save_before_exit` before `event_loop.exit()`).
//! - **Closed by another program** (Windows: a WM_CLOSE posted from outside, as `taskkill` without
//!   `/F` and an installer send) always quits, whatever the setting, shown or hidden: a person's
//!   close button and Alt+F4 SEND the close and never post it, so only another program's request
//!   is told apart (`note_message`, `posted`). Agents close the operator's game that way so it
//!   saves (CLAUDE.md, "The operator's machine"); hiding would have kept it running until forced.
//!
//! The event loop is built here so the tray and a click on a notice can wake it while it waits
//! with the window hidden (`wake`, through winit's `EventLoopProxy`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};

use crate::engine::state::EngineState;
use crate::gui::GuiPage;
use crate::notifications::{self as n, LocalSettings};

/// How often a hidden window reads its sockets.
pub(crate) const TICK: Duration = Duration::from_millis(250);

static PROXY: OnceLock<Mutex<EventLoopProxy<()>>> = OnceLock::new();

/// Another program asked the app to close: a WM_CLOSE was posted to one of its windows (Windows).
static OUTSIDE_CLOSE: AtomicBool = AtomicBool::new(false);
/// The app's window (its HWND), once known (0 until then): a close posted to it is passed on, so
/// winit says CloseRequested; one posted to another of the app's windows is kept from it.
#[cfg(windows)]
static MAIN_HWND: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// Remember the app's window for `note_message` (once; a load each call after).
#[cfg(windows)]
fn remember_main_window(window: &winit::window::Window) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    if MAIN_HWND.load(Ordering::Relaxed) != 0 {
        return;
    }
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(h) = handle.as_raw() {
            MAIN_HWND.store(h.hwnd.get(), Ordering::Relaxed);
        }
    }
}

/// The app's event loop, with the proxy [`wake`] uses and, on Windows, the hook that tells another
/// program's close request from a person's (`note_message`).
pub(crate) fn event_loop() -> EventLoop<()> {
    #[allow(unused_mut)]
    let mut builder = EventLoop::builder();
    #[cfg(windows)]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        // True keeps the message from its window procedure (`note_message`).
        builder.with_msg_hook(note_message);
    }
    let event_loop = builder.build().expect("Failed to create event loop");
    let _ = PROXY.set(Mutex::new(event_loop.create_proxy()));
    event_loop
}

/// Wake the event loop (from any thread): the tray's handlers and a click on a notice call this, so
/// a loop waiting with the window hidden takes the click at once.
pub(crate) fn wake() {
    if let Some(p) = PROXY.get() {
        if let Ok(p) = p.lock() {
            let _ = p.send_event(());
        }
    }
}

/// Every message winit takes from the queue passes here first; true keeps it from its window
/// procedure. A POSTED WM_CLOSE comes from another program (taskkill, an installer): the window's
/// own close button and Alt+F4 send theirs straight to the window procedure, which never passes
/// through the queue. `taskkill` without `/F` posts it to every top-level window it finds showing:
/// with the window hidden in the tray that is only winit's own message window, whose window
/// procedure would destroy it (seen on the second rig run, 2026-10-31: the hidden game stayed up
/// until it was forced, 15 s later). So a close posted to any of the app's windows asks the app to
/// close (`about_to_wait` quits, saved), and one posted to a window other than the app's own is
/// kept from it.
#[cfg(windows)]
fn note_message(msg: *const core::ffi::c_void) -> bool {
    /// The head of Win32's MSG: the window, then the message.
    #[repr(C)]
    struct Head {
        hwnd: isize,
        message: u32,
    }
    if msg.is_null() {
        return false;
    }
    // SAFETY: winit hands the MSG it just took from the queue; its first two fields are these.
    let head = unsafe { &*(msg as *const Head) };
    match posted(head.message, head.hwnd, MAIN_HWND.load(Ordering::Relaxed)) {
        Posted::Other => false,
        Posted::CloseMain => {
            OUTSIDE_CLOSE.store(true, Ordering::SeqCst);
            false
        }
        Posted::CloseOther => {
            OUTSIDE_CLOSE.store(true, Ordering::SeqCst);
            true
        }
    }
}

/// What a message taken from the queue is, to the tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Posted {
    /// Nothing of ours.
    Other,
    /// Another program's close of the app's own window (`main`, or any window while `main` is not
    /// yet known): passed on, so winit says CloseRequested.
    CloseMain,
    /// Another program's close sent to another of the app's windows (winit's message window, the
    /// tray's): the app is asked to close, and the window is kept from it.
    CloseOther,
}

/// Classify a message taken from the queue (`main` is the app's window, 0 while not yet known).
/// Pure.
pub(crate) fn posted(message: u32, hwnd: isize, main: isize) -> Posted {
    const WM_CLOSE: u32 = 0x0010;
    match message {
        WM_CLOSE if main == 0 || hwnd == main => Posted::CloseMain,
        WM_CLOSE => Posted::CloseOther,
        _ => Posted::Other,
    }
}

/// Does closing the window keep HumanityOS running in the tray? Only with the setting on AND the
/// icon showing: without the icon there would be no way back to the window. Pure.
pub(crate) fn keeps_running(s: &LocalSettings, icon_shown: bool) -> bool {
    s.tray && s.close_to_tray && icon_shown
}

/// The window's close button (WindowEvent::CloseRequested): true when the window was hidden in the
/// tray instead, so the caller must not quit.
pub(crate) fn close_to_tray(state: &mut EngineState) -> bool {
    if OUTSIDE_CLOSE.swap(false, Ordering::SeqCst) {
        log::info!("window: another program asked HumanityOS to close: quitting, whatever the tray setting says");
        return false;
    }
    if state.tray.hidden {
        return true;
    }
    if !keeps_running(&state.gui_state.settings.notifications, state.tray.shown()) {
        return false;
    }
    hide(state);
    true
}

/// Leave the world (saved, out of a shared world), then hide the window.
pub(crate) fn hide(state: &mut EngineState) {
    leave_world(state);
    let _ = state.window.set_cursor_grab(winit::window::CursorGrabMode::None);
    state.window.set_cursor_visible(true);
    state.cursor_free = true;
    hide_window(&state.window);
    state.window_focused = false;
    state.tray.hidden = true;
    let now = Instant::now();
    state.tray.last_tick = Some(now);
    state.tray.next_tick = Some(now + TICK);
    crate::config::AppConfig::from_gui_state(&state.gui_state).save();
    log::info!("window: hidden in the tray; no frames are drawn, the sockets are read every {} ms", TICK.as_millis());
    if !state.tray.told {
        state.tray.told = true;
        tell_still_running(state);
    }
}

/// Hide the window, whoever showed it. winit hides only a window it knows is shown, and a
/// background launch is shown behind winit's back (`launch_focus::show_window_behind`, a raw
/// SW_SHOWNOACTIVATE), so on Windows the window is also hidden directly (SW_HIDE). Seen on the
/// first rig run, 2026-10-31: winit's hide alone left a background window on screen, drawing
/// nothing. winit's own call stays too, so its record of the window agrees for the next show.
fn hide_window(window: &winit::window::Window) {
    window.set_visible(false);
    #[cfg(windows)]
    {
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        const SW_HIDE: i32 = 0;
        #[link(name = "user32")]
        extern "system" {
            fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        }
        if let Ok(handle) = window.window_handle() {
            if let RawWindowHandle::Win32(h) = handle.as_raw() {
                // SAFETY: the window's own handle, alive for the call.
                unsafe {
                    ShowWindow(h.hwnd.get(), SW_HIDE);
                }
            }
        }
    }
}

/// The world left the way quitting leaves it: saved, out of the shared world, and the main menu
/// in front, so nothing of it runs while hidden or shows the moment the window comes back.
fn leave_world(state: &mut EngineState) {
    if state.world_loaded {
        crate::engine::quit::save_game(state);
    }
    if state.game_joined {
        if let Some(ws) = state.gui_state.ws_client.as_ref() {
            ws.send(&serde_json::json!({ "type": "game_leave" }).to_string());
        }
        crate::engine::home_plot::forget_shared_world(state);
        log::info!("Co-presence: left the shared world (the window went to the tray)");
    }
    if state.gui_state.active_page == GuiPage::None {
        state.gui_state.chat_input_active = false;
        state.gui_state.active_page = GuiPage::MainMenu;
    }
}

/// The notice that says where HumanityOS went: through the operating system, since the window is
/// gone (only with desktop notices on; a click shows the window).
fn tell_still_running(state: &mut EngineState) {
    if !state.gui_state.settings.notifications.desktop {
        log::info!("window: desktop notices are off, so no notice says HumanityOS is still running");
        return;
    }
    log::info!("window: a notice says HumanityOS is still running in the tray");
    let t = crate::i18n::t;
    let launch = state.gui_state.notify.targets.window_token();
    n::toast::show(n::toast::Toast {
        title: t("notifications.tray_still_running_title").to_string(),
        body: t("notifications.tray_still_running_body").to_string(),
        tag: n::toast::tag_for("tray"),
        group: "tray".into(),
        launch,
    });
}

/// Show the window again (the tray's Open, a click on its icon or on a notice). `focus`: bring it
/// in front, for a person's click; a script's instance never takes the focus.
pub(crate) fn show(state: &mut EngineState, focus: bool) {
    let background = crate::engine::launch_focus::launch_in_background();
    if state.tray.hidden {
        state.tray.hidden = false;
        state.tray.next_tick = None;
        if background {
            crate::engine::launch_focus::show_window_behind(&state.window);
        } else {
            state.window.set_visible(true);
        }
        // The first frame back steps from now, not from when the window went.
        state.last_frame = Instant::now();
        log::info!("window: shown from the tray");
    }
    if focus && !background {
        state.window.set_minimized(false);
        state.window.focus_window();
    }
    state.window.request_redraw();
}

/// The event loop is about to wait. Shown: ask for the next frame at once. Hidden: run the tick when
/// it is due (or when the tray or a notice woke the loop) and wait until the next one. True when
/// the tray's Quit asked to leave: the caller saves and exits.
pub(crate) fn about_to_wait(state: &mut EngineState, event_loop: &ActiveEventLoop) -> bool {
    #[cfg(windows)]
    remember_main_window(&state.window);
    // Another program's close that reached no CloseRequested (posted to winit's own window, as
    // taskkill does while the app's window is hidden): quit, saved.
    if OUTSIDE_CLOSE.swap(false, Ordering::SeqCst) {
        log::info!("window: another program asked HumanityOS to close: quitting");
        return true;
    }
    if !state.tray.hidden {
        event_loop.set_control_flow(ControlFlow::Wait);
        state.window.request_redraw();
        return false;
    }
    let now = Instant::now();
    let due = state.tray.next_tick.is_none_or(|t| now >= t);
    if due || crate::engine::tray::pending() || n::toast::clicks_waiting() {
        tick(state, now);
    }
    if std::mem::take(&mut state.gui_state.quit_requested) {
        return true;
    }
    if !state.tray.hidden {
        // The tick showed the window.
        event_loop.set_control_flow(ControlFlow::Wait);
        state.window.request_redraw();
        return false;
    }
    event_loop.set_control_flow(ControlFlow::WaitUntil(state.tray.next_tick.unwrap_or(now + TICK)));
    false
}

/// One tick of a hidden window: the network as the frame pumps it (which takes the tray's and the
/// notices' clicks too, `frame_ws_poll` calls `tray::pump`), the dev request, and the sounds a
/// notice queued.
fn tick(state: &mut EngineState, now: Instant) {
    // The real time since the last tick (the reconnect countdown and the links' timers run on it),
    // never more than a few seconds after the computer slept.
    let dt = state.tray.last_tick.map_or(TICK, |t| now.saturating_duration_since(t)).min(Duration::from_secs(5)).as_secs_f32();
    state.tray.last_tick = Some(now);
    state.tray.next_tick = Some(now + TICK);
    crate::engine::frame_net::pump(state, dt);
    play_queued_sounds(state);
}

/// The sounds queued since the last tick, as the frame's audio stage plays them.
fn play_queued_sounds(state: &mut EngineState) {
    let Some(audio) = state.audio.as_mut() else {
        state.pending_sfx.clear();
        return;
    };
    for (id, fallback) in state.pending_sfx.drain(..) {
        let path = format!("assets/{}", state.sound_catalog.path_or(id, fallback));
        let vol = state.sound_catalog.volume_or_default(id);
        if let Err(e) = audio.play_sound_vol(&path, vol) {
            log::debug!("[Audio] sfx {id}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Closing keeps HumanityOS running only with the setting on, the icon wanted AND showing: a
    /// hidden window with no icon could never be shown again. Seen red 2026-10-31 with the icon
    /// check left out: a person whose tray icon failed to appear (a Linux desktop with no tray)
    /// closed the window into nowhere.
    #[test]
    fn closing_keeps_running_only_with_the_icon_showing() {
        let mut s = LocalSettings::default();
        assert!(!keeps_running(&s, true), "off by default: closing quits");
        s.close_to_tray = true;
        assert!(keeps_running(&s, true));
        assert!(!keeps_running(&s, false), "no icon showing: closing quits");
        s.tray = false;
        assert!(!keeps_running(&s, true), "the icon turned off: closing quits");
    }

    /// A WM_CLOSE posted to ANY of the app's windows is another program's close (taskkill posts it
    /// to winit's own message window while the app's window is hidden): the app's own window has it
    /// passed on, any other window has it kept from it. Every other message is nothing of ours.
    /// Seen red twice on 2026-10-31: on the second rig run, when the first version counted a close
    /// only on the app's own window and the hidden game outlived `taskkill` until forced ("Game
    /// pid 14272 forced after 15 s"); and here with `CloseOther` folded back into `Other` ("winit's
    /// own window, while hidden").
    #[test]
    fn a_close_posted_to_any_of_our_windows_is_from_outside() {
        const WM_CLOSE: u32 = 0x0010;
        assert_eq!(posted(WM_CLOSE, 7, 7), Posted::CloseMain);
        assert_eq!(posted(WM_CLOSE, 9, 7), Posted::CloseOther, "winit's own window, while hidden");
        assert_eq!(posted(WM_CLOSE, 9, 0), Posted::CloseMain, "our window not known yet: passed on");
        assert_eq!(posted(0x0112, 7, 7), Posted::Other, "WM_SYSCOMMAND is the person's close, sent on to WM_CLOSE");
    }
}
