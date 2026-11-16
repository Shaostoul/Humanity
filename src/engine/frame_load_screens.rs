//! The network, loading and the in-world screens (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as they were: the network pump (`engine::frame_net`), the page
//! open before the GUI runs, world loading, and whether this frame draws the 3D scene.

use crate::engine::frame_prelude::*;

/// Pumps the network, advances loading and decides whether the world is drawn this frame.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars) {
    let dt = state.clock.dt;
    humanity_core::frame_costs::span("cpu.span.network");
    // Dialling, reading every message, reconnecting, WebRTC, chat history and the saved
    // servers' background links (engine/frame_net.rs, shared with the hidden window's tick).
    crate::engine::frame_net::pump(state, dt);

    humanity_core::frame_costs::span("cpu.span.load");
    // Track page before egui frame for cursor grab transitions
    let page_before_frame = state.gui_state.active_page;

    // A phone has no 3D world yet (Android rung A1, humanity_core::platform::device): whatever
    // asked for it (a key, a notice, a first step) lands on the Play hub, which says why in words,
    // and the world is never loaded. `GuiState::enter_world` does the same at the click.
    if state.gui_state.active_page == GuiPage::None
        && !humanity_core::platform::device::has(humanity_core::platform::device::Capability::World3d)
    {
        state.gui_state.launcher_open_select = false;
        state.gui_state.go(GuiPage::Play, crate::gui::How::Beside);
    }

    // Lazy-load 3D world on first Enter World (code LOD: chat loads fast, 3D loads when needed)
    if state.gui_state.active_page == GuiPage::None && !state.world_loaded {
        load_world(state);
    }

    // ── Unified character launcher (v0.476) ──
    // Play asks for the character picker by setting launcher_open_select
    // + active_page = None. We open the showroom (mode 0) here, AFTER
    // load_world, so it works the FIRST time (world loads this same frame)
    // and EVERY later time (world already loaded, so load_world is skipped
    // -- a gate inside load_world would never fire again). The showroom is
    // the single unified two-pane character/server surface; the old flat
    // launcher page is gone.
    if state.gui_state.launcher_open_select && state.world_loaded {
        state.gui_state.launcher_open_select = false;
        // Rescan the saves and re-derive the WHO/WHERE selection from
        // the last pairing, so the picker always opens on rows that
        // still exist (showroom::load_homes -> preselect).
        state.gui_state.launcher_homes_loaded = false;
        state.gui_state.enter_world();
        open_showroom(state, 0); // mode 0 = the Play picker
    }

    // Load a save onto the running world once it exists: a snapshot restore of the
    // live save, or a character the picker asked for (engine/save_slot.rs
    // `apply_pending_load`; your own home's character only lends its look while a
    // server's save is live, ship homes increment 6).
    if state.world_loaded {
        if let Some(save) = crate::save_load::take_pending_load(&mut state.gui_state) {
            crate::engine::save_slot::apply_pending_load(state, &save);
        }
    }

    // The controls hint on the first entry into the world, once ever, saved at
    // once (first-hour audit 2026-10-04, Friction 1; gui/first_steps.rs).
    if state.gui_state.queue_controls_hint_once(state.world_loaded) {
        crate::config::AppConfig::from_gui_state(&state.gui_state).save();
    }

    humanity_core::frame_costs::span("cpu.span.screens");
    // Decide whether to render 3D scene or just a cleared surface
    let page_active = state.gui_state.active_page != GuiPage::None;

    // In-world screens (rungs 1 and 2). ORDER MATTERS, twice over:
    // (1) this runs OUTSIDE the main egui closure below, because
    // drawing a page under a screen's context borrows GuiState
    // mutably, the same borrow that closure holds for the whole
    // main frame; (2) it runs BEFORE the scene passes, because
    // the scene samples each surface's texture and would
    // otherwise show the previous frame's page. `update` aims
    // the look ray and moves the hovered screen's pointer; the
    // dev IPC then queues its synthetic events (after the ray,
    // so they win the frame); `frame_surfaces` draws the pages
    // (nearest four in range); the IPC completes with the
    // freshly drawn frame. Skipped while a menu page covers
    // the world: the scene is not drawn then either.
    // First, one chat video at a time across the app and every wall screen, framed or not
    // (a page covering the world frames none): each gives way to a newer Play elsewhere.
    crate::engine::screens::chat_videos_one_at_a_time(&mut state.screens);
    if !page_active {
        crate::engine::screens::update(state);
        crate::engine::ipc::advance_screen_request(state);
        // This frame's draw lists as built so far, for the
        // camera screens (rung 3): a camera renders them
        // from its post's pose into its own surface. They
        // are still in the HOME frame here (station_off is
        // added in place further down, at the "so.length_squared"
        // block), which is the frame the camera posts are in.
        let screen_lists = SceneDrawLists {
            celestial: &f.celestial_objects,
            celestial_transparent: &f.celestial_transparent,
            orbit_lines: &f.orbit_lines,
            opaque: &f.all_objects,
            transparent: &f.transparent_objects,
            overlay: &f.overlay_objects,
            ring_lines: &f.ring_lines,
        };
        crate::engine::screens::frame_surfaces(state, &screen_lists);
        crate::engine::ipc::complete_screen_request(state);
    } else {
        // No wall screen is framed under a page: none draws its page this tick.
        state.screens.framed_pages.clear();
    }

    // A VSync toggle recorded by the previous frame's settings
    // apply is applied HERE, before this frame's surface
    // texture exists: reconfiguring the surface while a
    // frame's swapchain view was alive is what killed the
    // app with vsync off (BUG-077).
    gpu!(state).renderer.apply_pending_surface_config();

    crate::engine::map_view::frame(state, &f.celestial_objects, &f.celestial_transparent, &f.orbit_lines, &f.all_objects, &f.transparent_objects, &f.overlay_objects, &f.ring_lines); // Maps world layer (engine/map_view.rs)
    f.page_before_frame = page_before_frame;
    f.page_active = page_active;
}
