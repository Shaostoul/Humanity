//! The click sound (`ui_click_should_sound`, v0.1112): it sounds only on a real widget click,
//! and only with interface sounds on. Moved out of gui/mod.rs as it stood (2026-10-12), so that
//! file stays under its line budget (tests/file_size_ratchet.rs). The module keeps its old name
//! (`ui_click_sound_tests`) through the declaration in gui/mod.rs, so filters by it still work.

use super::ui_click_should_sound;
use egui::output::OutputEvent;
use egui::{WidgetInfo, WidgetType};

fn clicked_button() -> OutputEvent {
    OutputEvent::Clicked(WidgetInfo::new(WidgetType::Button))
}

/// The whole point of the fix: with NO widget-activation event this frame
/// (the pointer was pressed over empty panel background), the click sound
/// must NOT fire. This is the operator's complaint reproduced as logic -
/// before the fix, being over any egui area was enough.
#[test]
fn no_click_event_means_no_sound() {
    // Events that are NOT a real click: focus + value-change happen without
    // a pointer click, and empty background produces no events at all.
    let not_clicks = [
        OutputEvent::FocusGained(WidgetInfo::new(WidgetType::TextEdit)),
        OutputEvent::ValueChanged(WidgetInfo::new(WidgetType::Slider)),
    ];
    assert!(
        !ui_click_should_sound(&[], true),
        "empty-background frame (no events) must be silent"
    );
    assert!(
        !ui_click_should_sound(&not_clicks, true),
        "focus/value-change without a click must be silent"
    );
}

/// A genuine widget activation DOES fire the sound.
#[test]
fn real_click_event_makes_sound() {
    assert!(ui_click_should_sound(&[clicked_button()], true));
    assert!(ui_click_should_sound(
        &[OutputEvent::DoubleClicked(WidgetInfo::new(WidgetType::Button))],
        true
    ));
}

/// The "Interface sounds" master switch gates even a real click.
#[test]
fn disabled_switch_silences_a_real_click() {
    assert!(
        !ui_click_should_sound(&[clicked_button()], false),
        "with interface sounds disabled, even a real click is silent"
    );
}
