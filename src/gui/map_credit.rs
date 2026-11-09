//! THE MAP CREDIT, drawn by its own layer (whole-Earth maps rung 1, 2026-11-09;
//! docs/design/whole-earth-maps.md section 11 and the finding
//! docs/reference/findings/2026-11-08-whole-earth-map-data.md, item 27).
//!
//! OpenStreetMap's ODbL treats a drawn view of its data as a Produced Work: the credit must be in
//! view wherever the map is drawn. Since v0.1226 the 3D view carried it, but inside `hud::draw`,
//! which lib.rs skips in the construction editor and the showroom (and when the HUD is turned
//! off), so the region's streets stood on screen there with no credit (attribution audit item 4,
//! still open on 2026-11-08). It is drawn here instead, on its own painter through
//! `gui::layers` (the page layer, where the crosshair is: over the world, under every window and
//! popup), whenever a region's OSM geometry is drawn in the 3D view and no page covers it. The
//! words are `credits::OSM_NOTICE`, the same string as the Maps page footer and the Credits page,
//! held to `data/credits.ron` by a test.
//!
//! COST: one short text a frame while a region is drawn, nothing otherwise.

use crate::gui::layers::{self, Layer};
use crate::gui::theme::Theme;
use crate::gui::{GuiPage, GuiState};
use egui::{Align2, Context, Id, Pos2};

/// The credit's distance from the right of the free area and from its bottom (points), where the
/// HUD drew it before (clear of the hotbar).
const INSET_RIGHT: f32 = 16.0;
const INSET_BOTTOM: f32 = 96.0;
/// Its size (points; the Font Size setting scales points).
const SIZE: f32 = 11.0;

/// True while the credit must show: OSM region geometry is drawn in the 3D view and no page covers
/// the view. Nothing about the HUD, the editor or the showroom: a licence notice does not switch
/// off with them.
pub fn visible(state: &GuiState) -> bool {
    state.active_page == GuiPage::None && !state.osm_regions_drawn.is_empty()
}

/// Draw the credit in the bottom right of the area the panels leave free (call it after the
/// frame's side panels, so the construction editor's panel never covers it).
pub fn draw(ctx: &Context, theme: &Theme, state: &GuiState) {
    if !visible(state) {
        return;
    }
    let free = ctx.available_rect();
    let painter = layers::painter(ctx, Id::new("map_credit"), Layer::Page);
    crate::gui::pages::hud::text_shadowed(
        &painter,
        Pos2::new(free.right() - INSET_RIGHT, free.bottom() - INSET_BOTTOM),
        Align2::RIGHT_BOTTOM,
        humanity_core::credits::OSM_NOTICE,
        SIZE,
        theme.text_muted(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The texts one frame of `draw` paints, on a 1280 by 900 screen.
    fn drawn_texts(state: &GuiState) -> Vec<String> {
        let ctx = egui::Context::default();
        crate::gui::fonts::install_font_fallbacks(&ctx);
        let theme = crate::gui::theme::load_theme();
        theme.apply_to_egui(&ctx);
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 900.0))),
            ..Default::default()
        };
        let out = ctx.run(input, |ctx| draw(ctx, &theme, state));
        out.shapes
            .iter()
            .filter_map(|cs| match &cs.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    /// The credit shows whenever OSM geometry is drawn in the world: in play, with the HUD off,
    /// in the construction editor and in the showroom; and not when no region is drawn or a page
    /// covers the view. Seen red 2026-11-09 with the HUD's old gate put back into `visible`
    /// (`&& state.show_hud && !state.showroom_active && !state.construction_active`): "turning the
    /// HUD off hides the OpenStreetMap credit".
    #[test]
    fn the_credit_shows_wherever_osm_is_drawn() {
        let mut state = GuiState::default();
        state.active_page = GuiPage::None;
        assert!(drawn_texts(&state).is_empty(), "no region drawn: no credit");
        state.osm_regions_drawn = vec!["Silverdale".into()];
        let shows = |s: &GuiState| drawn_texts(s).iter().any(|t| t == humanity_core::credits::OSM_NOTICE);
        assert!(shows(&state), "in play the credit is drawn");
        state.show_hud = false;
        assert!(shows(&state), "turning the HUD off hides the OpenStreetMap credit");
        state.show_hud = true;
        state.construction_active = true;
        assert!(shows(&state), "the construction editor hides the OpenStreetMap credit");
        state.construction_active = false;
        state.showroom_active = true;
        assert!(shows(&state), "the showroom hides the OpenStreetMap credit");
        state.showroom_active = false;
        state.active_page = GuiPage::Settings;
        assert!(!shows(&state), "a page covers the world: its own footer credits what it draws");
    }
}
