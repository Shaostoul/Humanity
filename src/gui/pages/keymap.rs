//! Keymap reference overlay (v0.465). Held-F1 shows the bindings for the screen / mode you are
//! in, so the keys listed are the ones that actually do something where you are (no world
//! hotkeys while a menu is open). Data-driven from `data/keymaps.ron`; the input handlers stay
//! the source of truth, so the list is description, not binding. Display-only (not editable);
//! actual REBINDING lives in Settings > Controls (src/input/bindings.rs, 2026-08-12), and this
//! overlay lists the DEFAULT keys, so it reads stale for a user who has rebound. Making it
//! render the live map is tracked follow-up work.

use egui::{Context, RichText};
use serde::Deserialize;

use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};

/// One row: a human action and the key combo (with modifiers spelled out) that triggers it.
#[derive(Debug, Clone, Deserialize)]
pub struct KeyBind {
    pub action: String,
    pub keys: String,
}

/// All bindings for one screen / mode, matched by `context`.
#[derive(Debug, Clone, Deserialize)]
pub struct KeymapContext {
    pub context: String,
    pub binds: Vec<KeyBind>,
}

/// Load the keymaps from `data/keymaps.ron`, or a minimal fallback.
pub fn load_keymaps(data_dir: &std::path::Path) -> Vec<KeymapContext> {
    let path = data_dir.join("keymaps.ron");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|t| ron::from_str::<Vec<KeymapContext>>(&t).ok())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(fallback)
}

fn fallback() -> Vec<KeymapContext> {
    vec![KeymapContext {
        context: "World".into(),
        binds: vec![KeyBind { action: "Keymap (this list)".into(), keys: "F1 (hold)".into() }],
    }]
}

/// The context name for the current screen / mode (matches a `context` in the data file).
fn current_context(state: &GuiState) -> &'static str {
    if state.construction_active {
        "Construction editor"
    } else if state.showroom_active {
        "Showroom"
    } else if state.active_page != GuiPage::None {
        "Menu"
    } else {
        "World"
    }
}

/// Draw the keymap overlay (called while F1 is held). Display-only, non-interactable.
pub fn draw(ctx: &Context, theme: &Theme, state: &GuiState) {
    let name = current_context(state);
    let binds = state
        .keymaps
        .iter()
        .find(|c| c.context == name)
        .or_else(|| state.keymaps.iter().find(|c| c.context == "Menu"))
        .map(|c| c.binds.clone())
        .unwrap_or_default();

    egui::Area::new(egui::Id::new("keymap_overlay"))
        .interactable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme.bg_panel())
                .inner_margin(16.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!("Keys -- {name}"))
                            .strong()
                            .size(theme.font_size_body)
                            .color(theme.text_primary()),
                    );
                    ui.add_space(theme.spacing_sm);
                    egui::Grid::new("keymap_grid")
                        .num_columns(2)
                        .spacing([28.0, 6.0])
                        .show(ui, |ui| {
                            for b in &binds {
                                ui.label(RichText::new(&b.action).color(theme.text_secondary()));
                                ui.label(RichText::new(&b.keys).strong().color(theme.text_primary()));
                                ui.end_row();
                            }
                        });
                    ui.add_space(theme.spacing_xs);
                    ui.label(
                        RichText::new("Hold F1 to show; release to hide.")
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The SHIPPED keymap file must actually parse. `load_keymaps` swallows a
    /// RON error and silently returns the one-line `fallback()`, so a typo in
    /// data/keymaps.ron neither crashes nor warns - the F1 overlay just quietly
    /// stops listing anything, which nobody notices until they need it. This
    /// gate is the only thing standing between a stray comma and that.
    #[test]
    fn shipped_keymaps_parse_and_cover_the_world_context() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data");
        let text = std::fs::read_to_string(dir.join("keymaps.ron")).expect("keymaps.ron reads");
        let maps: Vec<KeymapContext> =
            ron::from_str(&text).expect("data/keymaps.ron must parse as Vec<KeymapContext>");
        let world = maps
            .iter()
            .find(|c| c.context == "World")
            .expect("a World context must exist - it is what the FPS view shows");
        assert!(world.binds.len() > 10, "the World keymap lost most of its rows");
        // Every dev hotkey the raw winit handler owns is DESCRIBED here, since
        // this list is the only place a player can discover them (v0.1109
        // added F9 = dev flight; the file header says to keep them in sync).
        for key in ["F1 (hold)", "F2", "F6", "F9", "F11", "F12"] {
            assert!(
                world.binds.iter().any(|b| b.keys == key),
                "the World keymap does not mention {key}; the F1 overlay is the \
                 discovery surface for it, so an unlisted hotkey is invisible"
            );
        }
    }
}
