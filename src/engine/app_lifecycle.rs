//! The app going to the background and coming back: Android's drawing surface taken away and
//! given back (Android rung A1, 2026-11-14, docs/design/platforms.md).
//!
//! WHY. On a phone the system takes the window's drawing surface away whenever the app leaves the
//! screen (the home button, another app, the screen turning off), and gives a NEW one when it comes
//! back. winit says so with `suspended` and a second `resumed`. A wgpu surface made on the old
//! window must not be drawn to (the driver's window is gone), so it is dropped on `suspended` and
//! made again from the same winit window on the next `resumed`. Until 2026-11-14 the app handled
//! only the first `resumed` (it builds everything there) and ignored the rest, which on a phone is
//! a black screen, or a crash, the first time the person switches apps.
//!
//! WHILE AWAY the app runs as a window hidden in the tray does (engine/tray_window.rs): nothing is
//! drawn and the sockets are read a few times a second, so chat stays connected for as long as
//! Android lets a background app run (keeping it running after that is rung A4). The game is
//! saved on the way out, as closing into the tray saves it: Android may end a background app at
//! any moment without asking it first.
//!
//! A DESKTOP never sees any of this: winit on Windows, macOS and Linux calls `resumed` once at the
//! start and never `suspended`, so these are no-ops there and the desktop's behaviour is unchanged.

use winit::event::WindowEvent;

use crate::engine::state::EngineState;

/// The app left the screen: drop the surface, save, and wait the way a window hidden in the tray
/// waits (the module docs).
pub(crate) fn suspended(state: &mut EngineState) {
    if !gpu!(state).renderer.has_surface() {
        return;
    }
    log::info!("lifecycle: suspended: the drawing surface is dropped until the app comes back");
    gpu!(state).renderer.drop_surface();
    crate::engine::quit::save_game(state);
    state.tray.hidden = true;
    state.tray.next_tick = None;
}

/// The app came back (a second `resumed`): a new surface on the same window, at the window's size
/// now (the phone may have turned while away), and the next frame at once.
pub(crate) fn resumed(state: &mut EngineState) {
    if gpu!(state).renderer.has_surface() {
        return;
    }
    let size = gpu!(state).window.inner_size();
    log::info!("lifecycle: resumed: a new drawing surface at {}x{}", size.width, size.height);
    let gpu = gpu!(state);
    gpu.renderer.recreate_surface(gpu.window.clone());
    state.tray.hidden = false;
    state.tray.next_tick = None;
    // The first frame back steps from now, not from when the app went away.
    state.last_frame = std::time::Instant::now();
    gpu!(state).window.request_redraw();
}

/// True for an event the frame loop must not handle while there is no surface to draw on: a
/// redraw then would acquire a frame from a surface that is gone.
pub(crate) fn skip_event(state: &EngineState, event: &WindowEvent) -> bool {
    matches!(event, WindowEvent::RedrawRequested) && !gpu_ref!(state).renderer.has_surface()
}

/// Before the event loop waits: with no surface, stay hidden, whatever asked to show the window
/// meanwhile (`tray_window::show`, from a notice's click), so `tray_window::about_to_wait` runs
/// its hidden tick and never asks for a redraw that [`skip_event`] would drop over and over, a
/// busy loop on a phone's battery.
pub(crate) fn hold_hidden_while_away(state: &mut EngineState) {
    if !gpu!(state).renderer.has_surface() {
        state.tray.hidden = true;
    }
}
