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
/// `pub(crate)` so the top-right help toggle can ask the same question this overlay asks,
/// instead of re-deriving "where am I" and drifting out of step with it.
pub(crate) fn current_context(state: &GuiState) -> &'static str {
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

/// The key rows for one context, falling back to "Menu" when the data file has no
/// block for this screen. Pulled out of `draw` so the panel and the button share
/// one lookup (there is exactly one place that decides which keys you see).
fn binds_for(state: &GuiState, name: &str) -> Vec<KeyBind> {
    state
        .keymaps
        .iter()
        .find(|c| c.context == name)
        .or_else(|| state.keymaps.iter().find(|c| c.context == "Menu"))
        .map(|c| c.binds.clone())
        .unwrap_or_default()
}

/// Size (square) of the top-right help toggle. Also the width the nav bar reserves
/// for it, so the button never lands on top of a nav tab.
pub const HELP_TOGGLE_SIZE: f32 = 26.0;

/// Draw the keymap / help panel.
///
/// Two ways in, one panel (v0.1212):
///   - HOLD F1: transient glance. Display-only, non-interactable, exactly as it has
///     behaved since v0.465 - nothing about that path changed.
///   - CLICK the top-right "?" button: pinned open (`state.help_panel_pinned`). Same
///     content, but interactable so a long prose topic can be scrolled, and the
///     footer tells you how to close it.
///
/// The panel shows the keys for the current context (from data/keymaps.ron) and, when
/// the content author wrote one for this page, the prose help topic for it (from
/// data/help/topics.json, the same file the web client reads). No topic means no prose
/// section at all rather than an empty heading.
pub fn draw(ctx: &Context, theme: &Theme, state: &GuiState) {
    let name = current_context(state);
    let binds = binds_for(state, name);
    let pinned = state.help_panel_pinned;

    // Only the pinned panel takes input. The F1-held overlay stays click-through so
    // it can never swallow a click you meant for the page under it.
    //
    // Anchoring differs between the two modes on purpose. The F1 glance keeps its
    // v0.465 dead-centre position, untouched. The PINNED panel hangs from the top
    // instead, just under the nav bar, so it reads as belonging to the button that
    // opened it and leaves the middle of the page you are reading about visible.
    let (align, offset) = if pinned {
        (egui::Align2::CENTER_TOP, egui::vec2(0.0, 72.0))
    } else {
        (egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
    };
    egui::Area::new(egui::Id::new("keymap_overlay"))
        .interactable(pinned)
        .anchor(align, offset)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(theme.bg_panel())
                .inner_margin(16.0)
                .show(ui, |ui| {
                    // Width cap stops a single long prose paragraph from stretching the
                    // panel across the whole screen. Applies to both modes; the F1 view
                    // has no prose-length problem but the cap is wider than its key grid,
                    // so it changes nothing there.
                    let screen = ctx.screen_rect();
                    ui.set_max_width((screen.width() * 0.45).clamp(320.0, 560.0));

                    // The body, drawn either straight (F1) or inside a scroll region
                    // (pinned). One closure so there is one definition of the content.
                    let body = |ui: &mut egui::Ui| {
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
                                        ui.label(
                                            RichText::new(&b.action).color(theme.text_secondary()),
                                        );
                                        ui.label(
                                            RichText::new(&b.keys)
                                                .strong()
                                                .color(theme.text_primary()),
                                        );
                                        ui.end_row();
                                    }
                                });

                            // Prose half: whatever help topics the content author paired
                            // with this page in data/help/topics.json, the same file the
                            // web client reads. No topics means no section at all, rather
                            // than an empty heading on the many pages nobody has written
                            // help for yet.
                            //
                            // PINNED ONLY, and this is load-bearing. The F1 overlay is
                            // non-interactable and does not scroll, so anything past the
                            // bottom of the screen is unreachable forever. Prose is long:
                            // /chat alone pairs 3 topics of 14 paragraphs, which pushed
                            // the overlay to 1213 px in a 900 px window and clipped the
                            // first 7 KEY ROWS off the top. F1 is the quick glance at the
                            // keys and stays exactly what it has been since v0.465; the
                            // reading is what the pinned panel is for.
                            let page_name = format!("{:?}", state.active_page);
                            let topics = if pinned {
                                state.help_registry.topics_for_page(&page_name)
                            } else {
                                Vec::new()
                            };
                            for topic in topics {
                                ui.add_space(theme.spacing_md);
                                ui.separator();
                                ui.add_space(theme.spacing_sm);
                                ui.label(
                                    RichText::new(&topic.title)
                                        .strong()
                                        .size(theme.font_size_body)
                                        .color(theme.text_primary()),
                                );
                                ui.add_space(theme.spacing_xs);
                                for paragraph in &topic.body {
                                    // topics.json allows inline HTML for the web; native
                                    // renders plain text, so strip the tags.
                                    ui.label(
                                        RichText::new(
                                            crate::gui::widgets::help_modal::strip_html(paragraph),
                                        )
                                        .size(theme.font_size_body)
                                        .color(theme.text_secondary()),
                                    );
                                    ui.add_space(theme.spacing_xs);
                                }
                            }
                    };

                    if pinned {
                        // Only the pinned panel scrolls, because only it can be scrolled:
                        // the F1 overlay is non-interactable, so wrapping IT in a scroll
                        // region would silently truncate a long keymap with no way to
                        // reach the rest (the World list is 25 rows and lost 10 of them
                        // when this was applied to both). F1 renders straight through,
                        // exactly as it has since v0.465.
                        // max_height alone was not enough. A ScrollArea inside an
                        // AUTO-SIZED Area asks its parent how much room it has, and the
                        // parent's height self-references the previous frame, so it
                        // collapses: the viewport was handed 388 px of a 732 px budget
                        // and showed about a third of the content in a half-empty
                        // window. min_scrolled_height gives the viewport a FLOOR when
                        // the content actually overflows, while still letting a short
                        // panel shrink to fit instead of leaving a tall empty box.
                        let budget = screen.height() - offset.y - 96.0;
                        egui::ScrollArea::vertical()
                            .max_height(budget)
                            .min_scrolled_height(budget)
                            .show(ui, body);
                    } else {
                        body(ui);
                    }

                    // Footer OUTSIDE the scroll area, so the line telling you how to
                    // close the thing is always on screen. Inside, a page with several
                    // prose topics pushed it below the fold, which is exactly where a
                    // "press Esc to close" hint is useless.
                    ui.add_space(theme.spacing_xs);
                    let footer = if pinned {
                        "Click X (top right) or press Esc to close. Hold F1 anywhere for a quick look."
                    } else {
                        "Hold F1 to show; release to hide."
                    };
                    ui.label(
                        RichText::new(footer)
                            .size(theme.font_size_small)
                            .color(theme.text_muted()),
                    );
                });
        });
}

/// Draw the always-in-the-same-place help toggle in the top-right corner.
///
/// Operator ask (2026-08-24): "a help button at the top right that also doubles as the
/// close help window button. Like it starts as a ? then becomes an X. That way the
/// button to open/close never moves."
///
/// So it is one button in one fixed anchored spot, whose LABEL changes with state, not
/// its position or size. It is deliberately absent in the first-person world view: there
/// the screen belongs to the game and F1 is the way in (which still works everywhere,
/// including here).
pub fn draw_help_toggle(ctx: &Context, theme: &Theme, state: &mut GuiState) {
    // No button in the FPS view (the screen belongs to the game there, and F1 still
    // works), and none on the title screen. MainMenu is the other page that draws no
    // nav bar, so nothing reserves the top-right corner for the button there and it
    // would land on whatever the title screen is drawing. Clear the pin in both cases:
    // leaving it set would strand a pinned panel with no visible way to dismiss it.
    if current_context(state) == "World" || state.active_page == GuiPage::MainMenu {
        state.help_panel_pinned = false;
        return;
    }

    let pinned = state.help_panel_pinned;
    // "?" and "X" are plain ASCII, so both are safe for the icon glyph lint (the font
    // has patchy coverage of the symbol blocks; see tests/icon_glyph_lint.rs).
    let glyph = if pinned { "X" } else { "?" };
    let tooltip = if pinned { "Close help" } else { "Help: keys and guidance for this screen" };

    // A top-anchored Area rather than a widget inside the nav bar: the nav row WRAPS
    // when the window is narrow, so a button placed in it would move. This one cannot.
    // The nav bar reserves the matching width on its right edge so nothing is covered.
    egui::Area::new(egui::Id::new("help_toggle_button"))
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-8.0, 5.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let (fill, text) = if pinned {
                (theme.accent(), theme.text_on_accent())
            } else {
                (theme.bg_secondary(), theme.text_muted())
            };
            let btn = egui::Button::new(
                RichText::new(glyph).size(theme.font_size_body).color(text),
            )
            .fill(fill)
            .stroke(egui::Stroke::new(1.0, theme.border()))
            .rounding(egui::Rounding::same(theme.border_radius as u8))
            .min_size(egui::Vec2::splat(HELP_TOGGLE_SIZE));
            if ui.add(btn).on_hover_text(tooltip).clicked() {
                state.help_panel_pinned = !pinned;
            }
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
        for key in ["F1 (hold)", "F2", "F6", "F9", "F10", "F11", "F12"] {
            assert!(
                world.binds.iter().any(|b| b.keys == key),
                "the World keymap does not mention {key}; the F1 overlay is the \
                 discovery surface for it, so an unlisted hotkey is invisible"
            );
        }
    }
}
