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

/// Does closing the window keep HumanityOS running in the tray? Only with the setting on, the icon
/// SEEN (made, and on Linux a tray host showing it: `tray::icon_can_be_seen`), and a window that can
/// be hidden at all (`can_hide`: not under Wayland, whose winit has no hide). Otherwise there would
/// be no way back to the window, or a window that never went. Pure.
pub(crate) fn keeps_running(s: &LocalSettings, icon_seen: bool, can_hide: bool) -> bool {
    s.tray && s.close_to_tray && icon_seen && can_hide
}

/// Can this window be hidden? winit 0.30's Wayland backend has no hide (`set_visible` does nothing,
/// and `is_visible` answers None), so there the window would stay on screen, frozen, with its X
/// doing nothing (the critic's review of part 2, 2026-10-31).
pub(crate) fn can_hide(window: &winit::window::Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let wayland = matches!(window.window_handle().map(|h| h.as_raw()), Ok(RawWindowHandle::Wayland(_)));
    !wayland && window.is_visible().is_some()
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
    let s = &state.gui_state.settings.notifications;
    // The cheap checks first: a person with the setting off never pays for the Linux host's ask.
    if !(s.tray && s.close_to_tray && state.tray.shown()) {
        return false;
    }
    if !keeps_running(s, crate::engine::tray::icon_can_be_seen(), can_hide(&state.window)) {
        log::info!("window: the tray icon cannot be seen or this window cannot be hidden here, so closing quits");
        return false;
    }
    hide(state);
    true
}

/// Leave the world (saved, out of a shared world) and every call and voice room, then hide the
/// window.
pub(crate) fn hide(state: &mut EngineState) {
    leave_world(state);
    let left = quiet_for_tray(&mut state.gui_state);
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
    // Said every time a call or a room was left (the person must know it ended), else once a run.
    if left.voice || !state.tray.told {
        state.tray.told = true;
        tell_still_running(state, left.voice);
    }
}

/// The GUI's half of hiding (pure on GuiState, so a lib test drives it): every voice room, call,
/// ring and microphone test left the way the person's own Leave, Hang up and Cancel do, the live
/// voice session stopped and its queued frames dropped (`chat::leave_all_voice`), and the lines the
/// person reads when the window comes back. A hidden window never keeps a microphone open: nothing
/// would show it is, the peers would still hear it, and its sound would come in bursts (the
/// critic's review of part 2, 2026-10-31). Staying in a call in the tray is a later rung
/// (docs/design/notifications.md, "The tray").
pub(crate) fn quiet_for_tray(gs: &mut crate::gui::GuiState) -> crate::gui::pages::chat::LeftVoice {
    let left = crate::gui::pages::chat::leave_all_voice(gs);
    if left.voice {
        log::info!("voice: the call or voice room was left and the microphone closed (the window went to the tray)");
        gs.pending_notices.push(humanity_core::i18n::t("notifications.tray_left_voice").to_string());
    }
    if let Some(name) = left.missed_ring.as_deref() {
        gs.pending_notices.push(humanity_core::i18n::tf("notifications.tray_missed_call", &[("name", &name)]));
    }
    left
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
        state.gui_state.show_title();
    }
}

/// The notice that says where HumanityOS went, and that a call or a voice room it was in ended
/// (`voice_left`): through the operating system, since the window is gone; a click shows the
/// window. Only with desktop notices on (the person's switch for notices from the operating system
/// at all; the line kept for the window's return, `quiet_for_tray`, says the call ended either way).
/// Mute everything and quiet hours do NOT hold it back: they are about other people reaching you,
/// and this is the answer to the person's own click, once a run, or the news that their own call
/// just ended (decided 2026-10-31, the critic's review of part 2; docs/design/notifications.md).
fn tell_still_running(state: &mut EngineState, voice_left: bool) {
    if !state.gui_state.settings.notifications.desktop {
        log::info!("window: desktop notices are off, so no notice says HumanityOS is still running");
        return;
    }
    log::info!("window: a notice says HumanityOS is still running in the tray{}", if voice_left { " and the call or voice room was left" } else { "" });
    let t = humanity_core::i18n::t;
    let launch = state.gui_state.notify.targets.window_token();
    let body = if voice_left { "notifications.tray_still_running_body_voice" } else { "notifications.tray_still_running_body" };
    n::toast::show(n::toast::Toast {
        title: t("notifications.tray_still_running_title").to_string(),
        body: humanity_core::i18n::t_dyn(body).to_string(),
        tag: n::toast::tag_for("tray"),
        group: "tray".into(),
        launch,
        expire_ms: n::toast::EXPIRE_MS,
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

    /// Closing keeps HumanityOS running only with the setting on, the icon wanted AND seen, and a
    /// window that can be hidden: a hidden window with no icon could never be shown again, and one
    /// that cannot hide would freeze on screen. This holds the RULE; whether the icon is seen is
    /// `tray::icon_can_be_seen` (on Linux, a tray host must be registered: tray-icon's ksni path
    /// succeeds without one, so "the icon was made" alone did not catch a Linux desktop with no
    /// tray, as this comment first claimed) and whether the window can hide is `can_hide`
    /// (Wayland cannot). Seen red 2026-10-31 by a deliberate break that dropped the icon check
    /// ("no icon seen: closing quits"), and again with the hide check dropped ("Wayland: the window
    /// cannot hide").
    #[test]
    fn closing_keeps_running_only_with_the_icon_seen_and_a_window_that_hides() {
        let mut s = LocalSettings::default();
        assert!(!keeps_running(&s, true, true), "off by default: closing quits");
        s.close_to_tray = true;
        assert!(keeps_running(&s, true, true));
        assert!(!keeps_running(&s, false, true), "no icon seen: closing quits");
        assert!(!keeps_running(&s, true, false), "Wayland: the window cannot hide");
        s.tray = false;
        assert!(!keeps_running(&s, true, true), "the icon turned off: closing quits");
    }

    fn voice_gs() -> (crate::gui::GuiState, std::sync::mpsc::Receiver<String>) {
        let mut gs = crate::gui::GuiState::default();
        let (client, sent) = crate::net::ws_client::WsClient::recording();
        gs.ws_client = Some(client);
        gs.profile_public_key = "me".into();
        gs.chat_channels.push(crate::gui::ChatChannel { id: "12".into(), name: "lounge".into(), voice_joined: true, ..Default::default() });
        (gs, sent)
    }

    /// Closing into the tray leaves voice the way the person's own Leave, Hang up and Cancel do:
    /// the room (the server told), the call (the peer told), an outgoing ring (cancelled), the
    /// microphone test, and the connected peers; an incoming ring is let ring out unsent, with a
    /// missed-call line; and the window's return says the call ended. Seen red 2026-10-31 with
    /// `quiet_for_tray` not calling `leave_all_voice` (the critic's finding): its first assertion,
    /// `left.voice`.
    #[test]
    fn hiding_leaves_every_call_and_voice_room() {
        let (mut gs, sent) = voice_gs();
        gs.voice_active_room = Some("12".into());
        gs.call_active = Some(("peer".into(), "Pat".into()));
        gs.voice_connected_peers.insert("peer".into());
        gs.mic_test_active = true;
        let left = quiet_for_tray(&mut gs);
        assert!(left.voice);
        assert_eq!(gs.voice_active_room, None, "the room is left");
        assert_eq!(gs.call_active, None, "the call is hung up");
        assert!(!gs.mic_test_active && gs.voice_connected_peers.is_empty() && !gs.voice_session_prev);
        assert!(!crate::gui::pages::chat::voice_wanted(&gs), "no session is wanted");
        let frames: Vec<serde_json::Value> = sent.try_iter().map(|s| serde_json::from_str(&s).unwrap()).collect();
        assert!(frames.iter().any(|f| f["type"] == "voice_room" && f["action"] == "leave" && f["room_id"] == "12"), "the server is told: {frames:?}");
        assert!(frames.iter().any(|f| f["type"] == "voice_call" && f["action"] == "hangup" && f["to"] == "peer"), "the peer is told: {frames:?}");
        assert_eq!(gs.pending_notices, vec![humanity_core::i18n::t("notifications.tray_left_voice").to_string()]);

        // An outgoing ring is cancelled; an incoming one rings out with nothing sent back.
        let (mut gs, sent) = voice_gs();
        gs.call_outgoing = Some(("callee".into(), "Cy".into()));
        gs.call_incoming = Some(("caller".into(), "Dee".into()));
        let left = quiet_for_tray(&mut gs);
        assert!(left.voice && gs.call_outgoing.is_none() && gs.call_incoming.is_none());
        let frames: Vec<serde_json::Value> = sent.try_iter().map(|s| serde_json::from_str(&s).unwrap()).collect();
        assert_eq!(frames.len(), 1, "only the cancel goes out: {frames:?}");
        assert_eq!((frames[0]["to"].as_str(), frames[0]["action"].as_str()), (Some("callee"), Some("hangup")));
        assert!(gs.pending_notices.iter().any(|l| l.contains("Dee")), "{:?}", gs.pending_notices);

        // Nothing to leave: nothing said.
        let (mut gs, sent) = voice_gs();
        assert_eq!(quiet_for_tray(&mut gs), Default::default());
        assert!(sent.try_recv().is_err() && gs.pending_notices.is_empty());
    }

    /// Nothing the hidden window's tick runs starts a voice session or the microphone test, and the
    /// network pump it shares with the frame hands a peer's voice frame on only while voice is
    /// wanted, so after hiding nothing feeds a session (read from the source: the tick cannot run
    /// in a unit test; the pattern of engine/quit.rs). Seen red 2026-10-31 on part 2 as merged
    /// (22983c236), whose pump pushed every VoiceFrame: "the network pump feeds a peer's voice
    /// without asking voice_wanted".
    #[test]
    fn the_hidden_tick_never_starts_or_feeds_voice() {
        let root = std::path::Path::new(env!("HUMANITY_REPO_ROOT"));
        let read = |rel: &str| std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}")).replace("\r\n", "\n");
        for rel in ["src/engine/tray_window.rs", "src/engine/frame_net.rs", "src/engine/tray.rs"] {
            let src = read(rel);
            let code = src.split("#[cfg(test)]").next().unwrap_or("");
            for starts in ["start_voice_session", "start_mic_test"] {
                assert!(!code.contains(starts), "{rel} starts voice ({starts}); the hidden tick runs it");
            }
        }
        let net = read("src/engine/frame_net.rs");
        let at = net.find("crate::net::voice::push_remote_opus(").expect("the pump hands peers' voice on");
        let before = &net[at.saturating_sub(600)..at];
        assert!(
            before.contains("if !crate::gui::pages::chat::voice_wanted(&state.gui_state) {"),
            "the network pump feeds a peer's voice without asking voice_wanted"
        );
        assert_eq!(net.matches("push_remote_opus(").count(), 1, "one place hands peers' voice on");
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
