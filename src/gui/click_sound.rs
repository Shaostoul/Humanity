//! The UI click sound's decision (v0.1112), moved verbatim out of `gui/mod.rs` on 2026-11-09 to keep
//! that file under its line budget (tests/file_size_ratchet.rs); `gui/mod.rs` re-exports it, so
//! `crate::gui::ui_click_should_sound` still resolves for the main loop.

/// Decide whether the UI-click sound should fire this frame (v0.1112).
///
/// The click sound must fire ONLY when egui reported a REAL widget activation -
/// a button, link, or toggle was actually clicked. The old gate keyed on
/// `egui_ctx.wants_pointer_input()`, which in egui 0.31 is true whenever the
/// pointer is merely OVER any egui area, so clicking empty panel background
/// played the click ("clicking on the screen makes a sound even if I don't
/// click anything"). `PlatformOutput::events` carries an `OutputEvent::Clicked`
/// (or `DoubleClicked`) only when a widget genuinely activated, so scanning it
/// is the precise gate. `ui_sounds_enabled` is the user's master switch for
/// interface sounds (Settings > Audio) - off means never play the click.
///
/// Kept as a small pure function so the decision is unit-testable without a
/// running egui context (a headless click-test is not possible).
pub fn ui_click_should_sound(
    events: &[egui::output::OutputEvent],
    ui_sounds_enabled: bool,
) -> bool {
    ui_sounds_enabled
        && events.iter().any(|e| {
            matches!(
                e,
                egui::output::OutputEvent::Clicked(_)
                    | egui::output::OutputEvent::DoubleClicked(_)
            )
        })
}

/// The click sound's tests.
#[cfg(test)]
#[path = "click_sound_tests.rs"]
mod ui_click_sound_tests;
