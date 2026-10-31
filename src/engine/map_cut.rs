//! THE CUTAWAY for the Maps page (Maps increment 3, docs/design/maps-zoom-to-anything.md 5.7):
//! picking a level on the map cuts everything above it away, hull, roof, ceilings and upper
//! storeys, so the level can be seen from above, in the map's world layer and in its plan tiles.
//!
//! HOW, AND WHY THIS WAY. The design preferred object-level culling "if increment 7's meshes are
//! split per storey", else a shader cut height. They are not split (a home's walls are one mesh
//! per wall material across the whole ship), but the build editor already draws a home cut at a
//! storey: it redraws the shell from the home cut away by `HomeStructure::cut_away`, and leaves
//! out what stands over the cut (machines, screens, doors, lift cars, plants, pipes) through
//! `HomeDraw::cut`. The map does exactly that, with one cut per home that has a storey above the
//! level (ship/levels.rs `level_cuts`, neighbours' homes included), so the map and the editor cut
//! a home by ONE rule, and no shader, pipeline or permutation changes. The roof and the hull are
//! whole meshes of their own, so they are left out of the map's view by object
//! (`inside_lists`).
//!
//! WHEN. The home is drawn as the page wants (`map_cuts`) through the editor's own per-frame
//! follower (`editor::follow_storey_view`, which asks `editor::wanted_cuts`): a level picked
//! redraws the home cut at it on the next frame, a page closed or the whole ship picked redraws
//! it whole. The live 3D frame is not drawn while a page is open, so the game never shows the
//! cut; the editor, when open, wins (`wanted_cuts`).
//!
//! Cost: one shell build of the cut ship (and of the whole ship for its rooms, as the editor's
//! storey change does) per level picked, not per frame: measured in the increment's record.

use crate::engine::editor::Cuts;
use crate::engine::state::EngineState;
use crate::gui::{GuiPage, GuiState};
use crate::renderer::RenderObject;
use crate::ship::levels::{self, ShipLevel};
use crate::ship::storey_edits::CutAway;

/// The level the Maps page shows cut open, when the home should be drawn cut for it: the page
/// open, a level shown (`ship_view::shown_level`: picked, or yours on the plan; aboard, near the
/// ship, the System view) and the 3D world loaded.
pub(crate) fn map_level(g: &GuiState) -> Option<ShipLevel> {
    if g.active_page != GuiPage::Maps || !g.maps.me.as_ref().is_some_and(|m| m.world_loaded) {
        return None;
    }
    let ordinal = crate::gui::pages::maps::ship_view::shown_level(&g.maps)?;
    levels::by_ordinal(&levels::ship_levels(g.ship_structure.as_ref()?), ordinal)
}

/// The cut-aways the home is drawn with for the Maps page (`map_level`): one per home with a
/// storey above the level; none for the whole ship (or the top level, where nothing is above).
pub(crate) fn map_cuts(g: &GuiState) -> Cuts {
    let (Some(level), Some(ship)) = (map_level(g), g.ship_structure.as_ref()) else {
        return Cuts::default();
    };
    Cuts(levels::level_cuts(ship, &level).into_iter().map(|c| CutAway { min: c.min, max: c.max, cut_y: c.cut_y }).collect())
}

/// The home's shell for the Maps page's level (`levels::level_meshes`), None for the whole ship.
pub(crate) fn map_level_meshes(g: &GuiState) -> Option<crate::ship::fibonacci::HomesteadMeshes> {
    let level = map_level(g)?;
    levels::level_meshes(g.ship_structure.as_ref()?, &level)
}

/// The world layer's lists while a level is shown: the frame's own, less the roof (both ceiling
/// meshes), the hull, and anything standing over a cut (a person upstairs while the ground level
/// is shown; the frame's machines, doors and plants are already left out by `HomeDraw::cut`).
/// None when no level is shown: the frame's lists as they are.
pub(crate) fn inside_lists(state: &EngineState, opaque: &[RenderObject], transparent: &[RenderObject], overlay: &[RenderObject]) -> Option<[Vec<RenderObject>; 3]> {
    crate::gui::pages::maps::ship_view::shown_level(&state.gui_state.maps)?;
    let lid: Vec<usize> = [state.homestead_ceiling, state.homestead_ceiling_opaque, state.homestead_hull].into_iter().flatten().map(|(m, _)| m).collect();
    let cut = &state.home_draw.cut;
    let keep = |o: &&RenderObject| !lid.contains(&o.mesh) && (o.position == glam::Vec3::ZERO || !cut.hides(o.position));
    let take = |list: &[RenderObject]| list.iter().filter(keep).cloned().collect::<Vec<_>>();
    Some([take(opaque), take(transparent), take(overlay)])
}
