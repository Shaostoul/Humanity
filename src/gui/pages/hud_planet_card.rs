//! The planet card: the name, resources and status of the body a hologram pin is aimed at, centred
//! above the bottom of the screen while the crosshair is on the pin (engine/frame_ui_run.rs, which
//! reads the words from data/planets/tooltips.json through `planet_tooltip_info`).
//!
//! It is a tooltip of the world (gui/layers.rs layer 1), so it takes no click: it is PAINTED
//! (`widgets::painted_card`, 2026-11-15). As the egui Area it was it took the click of whatever
//! stood under it (the BUG-316 class: egui gives every Area a widget rect, `interactable(false)`
//! or not), and `tests/layer_lint.rs` now refuses an Area on the tooltip layer.

use egui::{Align2, Id, Rect};
use humanity_core::i18n::tf;

use crate::gui::layers::Layer;
use crate::gui::theme::Theme;
use crate::gui::widgets::painted_card::{PaintedCard, Place, Words};

/// The id the card is painted under (`painted_card::rect` finds it).
pub const ID: &str = "planet_info_tooltip";

/// Paint the card for `planet_name` with its `resources` and `status`; where it stood.
pub fn draw(ctx: &egui::Context, _theme: &Theme, planet_name: &str, resources: &str, status: &str) -> Rect {
    let style = ctx.style();
    // A tooltip's look (egui's popup frame, from the theme through `apply_to_egui`), the heading
    // and body text a `Ui` gives `heading` and `label`.
    let heading = egui::TextStyle::Heading.resolve(&style);
    let body = egui::TextStyle::Body.resolve(&style);
    let color = style.visuals.text_color();
    PaintedCard::new(egui::Frame::popup(&style).inner_margin(egui::Margin::same(12)))
        .max_width(ctx.screen_rect().width() * 0.6)
        .text(Words::new(planet_name, heading, color))
        .separator()
        .text(Words::new(tf("hud.planet_resources", &[("resources", &resources)]), body.clone(), color))
        .text(Words::new(tf("hud.planet_status", &[("status", &status)]), body, color))
        .paint(ctx, Id::new(ID), Layer::Tooltip, Place::Anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -60.0)))
}
