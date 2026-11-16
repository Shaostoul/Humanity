//! The values one frame's stages hand to each other (the file-size split, step 1, 2026-11-14,
//! docs/design/crate-split.md "The file-size split").
//!
//! The frame was one function, `window_event`'s `RedrawRequested` arm in src/lib.rs, whose local
//! variables carried values from one part of the frame to a later one (the frame's clock, the
//! draw lists, whether the showroom is open). Cut into stages (`src/engine/frame_*.rs`), those
//! locals travel here instead. A `FrameVars` is made at the top of every frame and dropped at its
//! end: nothing in it outlives the frame, so it is not reuse (`humanity_core::reuse`) and never
//! goes stale. A stage reads the fields it needs into locals of the same names, so the moved code
//! reads exactly as it did inside the one function.
//!
//! The draw lists are taken from `EngineState::draw_bufs` (their allocations kept from the last
//! frame, as before the split) and handed back to it at the end of the frame.

use crate::engine::state::HoverGizmo;
use crate::gui::GuiPage;
use crate::renderer::line::LineVertex;
use crate::renderer::RenderObject;
use glam::Vec3;
use std::time::Instant;

/// One frame's hand-offs between stages. Fields are set by the stage that computes them and read
/// by later stages in the same frame; the values `new` gives are never read before that stage
/// runs.
pub(crate) struct FrameVars {
    /// When the frame handler started (`cpu.frame_total` is measured from here).
    pub frame_t0: Instant,
    /// The frame's timestamp, taken after the frame-rate cap's sleep.
    pub now: Instant,
    /// The camera's position before this frame's movement.
    pub prev_cam_pos: Vec3,
    /// The opaque draw list (the scene pass), taken from `EngineState::draw_bufs`.
    pub all_objects: Vec<RenderObject>,
    /// Transparent surfaces (glass windows), drawn in their own alpha-blended pass.
    pub transparent_objects: Vec<RenderObject>,
    /// Drawn over the scene (build gizmos, markers).
    pub overlay_objects: Vec<RenderObject>,
    /// The sky's bodies (planets, moons, the sun), drawn in the celestial pass.
    pub celestial_objects: Vec<RenderObject>,
    /// The sky's transparent layers.
    pub celestial_transparent: Vec<RenderObject>,
    /// Orbit lines.
    pub orbit_lines: Vec<LineVertex>,
    /// Ring lines (build guides, gizmo rings, light ranges).
    pub ring_lines: Vec<LineVertex>,
    /// Whether the showroom is open this frame.
    pub showroom: bool,
    /// Whether build mode hides machines this frame.
    pub hide_machines: bool,
    /// The build gizmo the cursor hovers this frame.
    pub hover: HoverGizmo,
    /// Other players' bodies, already in render space.
    pub people_render_space: Vec<RenderObject>,
    /// The piece in hand on a planet, already in render space (joins the scene list after the
    /// station shift).
    pub planet_ghost: Vec<RenderObject>,
    /// The page open before this frame's GUI ran.
    pub page_before_frame: GuiPage,
    /// Whether a page covers the world this frame.
    pub page_active: bool,
}

impl FrameVars {
    /// A frame that started at `frame_t0`. Every other field is set by its stage before any
    /// stage reads it.
    pub(crate) fn new(frame_t0: Instant) -> Self {
        Self {
            frame_t0,
            now: frame_t0,
            prev_cam_pos: Vec3::ZERO,
            all_objects: Vec::new(),
            transparent_objects: Vec::new(),
            overlay_objects: Vec::new(),
            celestial_objects: Vec::new(),
            celestial_transparent: Vec::new(),
            orbit_lines: Vec::new(),
            ring_lines: Vec::new(),
            showroom: false,
            hide_machines: false,
            hover: HoverGizmo::None,
            people_render_space: Vec::new(),
            planet_ghost: Vec::new(),
            page_before_frame: GuiPage::None,
            page_active: false,
        }
    }
}
