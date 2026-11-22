//! The medbay's "Get treated" (2026-11-21, the operator's call on botulism; systems::medbay).
//!
//! Botulism kills in Realistic unless a medbay treats it. The Inventory page's Status section
//! shows this button under the illness lines while anything on the player is an illness a medbay
//! treats: pressable in a medbay, greyed elsewhere with where to go. [E] in a medbay asks the same
//! (engine/frame_keys.rs). The engine says where the player is (`set_here`, engine/medbay.rs) and
//! what is on them (`set_treatable`, engine/frame_mirrors.rs); the request is taken by
//! engine/frame_sim_requests.rs (`take_asked`) and carried to the food system. Kept here rather
//! than on `GuiState`, which sits at its size budget (tests/file_size_ratchet.rs).

use std::sync::atomic::{AtomicBool, Ordering};

use egui::RichText;
use humanity_core::i18n::t;

use crate::gui::theme::Theme;
use crate::gui::widgets;

static HERE: AtomicBool = AtomicBool::new(false);
static TREATABLE: AtomicBool = AtomicBool::new(false);
static ASKED: AtomicBool = AtomicBool::new(false);

/// The player stands in a medbay this frame.
pub(crate) fn set_here(here: bool) {
    HERE.store(here, Ordering::Relaxed);
}

/// Something on the player is an illness a medbay treats.
pub(crate) fn set_treatable(treatable: bool) {
    TREATABLE.store(treatable, Ordering::Relaxed);
}

/// Ask for the medbay's treatment (the button, or [E] in a medbay).
pub(crate) fn ask() {
    ASKED.store(true, Ordering::Relaxed);
}

/// Take this frame's request, if there is one.
pub(crate) fn take_asked() -> bool {
    ASKED.swap(false, Ordering::Relaxed)
}

/// The button, under the Status section's illness lines; nothing when no illness on the player
/// is one a medbay treats.
pub(crate) fn draw(ui: &mut egui::Ui, theme: &Theme) {
    if !TREATABLE.load(Ordering::Relaxed) {
        return;
    }
    let here = HERE.load(Ordering::Relaxed);
    ui.horizontal_wrapped(|ui| {
        let hint = if here { "medbay.button_hint" } else { "medbay.button_away_hint" };
        if widgets::Button::primary(t("medbay.button")).disabled(!here).hint(hint).show(ui, theme) {
            ask();
        }
        if !here {
            ui.label(RichText::new(t("medbay.away_line")).size(theme.font_size_small).color(theme.text_secondary()));
        }
    });
}
