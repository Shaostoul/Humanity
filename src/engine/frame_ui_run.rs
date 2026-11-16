//! Running the GUI (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: the debug log drained into the GUI, the Garden data built
//! for the Inventory page, the font size applied, and one egui pass over every page and overlay.

use crate::engine::frame_prelude::*;

/// Runs this frame's egui pass and returns its output.
pub(crate) fn run(state: &mut EngineState, event_loop: &ActiveEventLoop) -> egui::FullOutput {
    // Drain global debug log into GuiState each frame
    {
        let new_entries = humanity_core::debug::drain_debug_log();
        state.gui_state.debug_log.extend(new_entries);
        // Cap at 500 entries
        while state.gui_state.debug_log.len() > 500 {
            state.gui_state.debug_log.remove(0);
        }
    }

    humanity_core::frame_costs::span("cpu.span.ui_run");
    // The Garden data, built only on a frame that draws the Inventory page
    // (engine/garden_mirror.rs); an in-world screen showing it asked already.
    if state.gui_state.active_page == GuiPage::Inventory {
        crate::engine::garden_mirror::build_for_this_frame(state);
    }

    // Run egui frame
    let gpu = gpu!(state);
    let mut raw_input = gpu.egui_state.take_egui_input(&gpu.window);
    // The Font Size is the whole app's zoom, set before the pass measures
    // anything; Ctrl and + / - / 0 change it here (gui/font_size.rs, BUG-225).
    if crate::gui::font_size::before_frame(&state.egui_ctx, &mut raw_input, &mut state.gui_state.settings.font_size) {
        state.gui_state.settings_dirty = true;
    }
    let full_output = state.egui_ctx.run(raw_input, |ctx| {
        // `GuiPage::Profile` is an ALIAS for the Real page. Normalise
        // it HERE, inside the app's own frame, so the nav bar
        // highlights the tab the person is actually looking at.
        //
        // This used to live inside `profile::draw`, which reads as the
        // natural home for it right up until something OTHER than the
        // main UI draws that page. In-world wall screens draw pages
        // through the same dispatch against this same GuiState, and the
        // bedroom's standing mirror is `source: "profile"` -- so from
        // v0.1322.0 every attempt to enter the world drew the mirror,
        // the mirror set `active_page`, and the player was thrown back
        // out onto the Profile page. Play was unusable. A page draw may
        // not steer the app; this line is the one place that may.
        if state.gui_state.active_page == GuiPage::Profile {
            state.gui_state.go(GuiPage::Real, crate::gui::How::Replace);
        }
        // Files dropped on the window this frame, held for whichever page or
        // window takes them (gui/drop_files.rs; routed at the frame's end).
        crate::gui::drop_files::begin_frame(ctx);
        crate::gui::pages::files::offer_drop_anywhere(ctx, &state.gui_state);
        // Back and Forward (gui/history.rs): the step a page moved to, the
        // mouse's side buttons and Alt+arrows. On pages only: in the world
        // the side buttons are the game's (unbound).
        crate::gui::history::frame(ctx, &mut state.gui_state);
        // Show RGB nav bar on all pages except None and MainMenu
        match state.gui_state.active_page {
            GuiPage::None | GuiPage::MainMenu => {}
            _ => {
                escape_menu::draw_nav_bar(ctx, &state.theme, &mut state.gui_state);
            }
        }
        // The bottom strip on every page but the first-person world (the
        // unfinished-software warning; gui/bottom_strip.rs), added before the
        // page so egui stacks the page's own bottom panels above it.
        crate::gui::bottom_strip::draw(ctx, &state.theme, &mut state.gui_state);

        // Draw active full-screen page. The engine-bound
        // arms (title screen, in-game None) stay here; every
        // plain tool page is drawn by the ONE dispatch table
        // in gui::dispatch, which the in-world screens share,
        // so a wall screen can show exactly what this can.
        match state.gui_state.active_page {
            GuiPage::MainMenu => {
                main_menu::draw(ctx, &state.theme, &mut state.gui_state);
            }
            GuiPage::None => {}
            page => {
                crate::gui::dispatch::draw_tool_page(
                    ctx,
                    page,
                    &mut state.theme,
                    &mut state.gui_state,
                );
            }
        }

        // Drain any freshly-decoded chat images into egui textures.
        state.gui_state.image_cache.poll(ctx);
        // An Add to Media upload carries on whatever page is open.
        crate::gui::pages::files::poll_upload(ctx, &mut state.gui_state);

        // Universal help modal overlay — draws on top of any page
        // when state.gui_state.active_help_topic is Some.
        help_modal::draw(
            ctx,
            &state.theme,
            &state.gui_state.help_registry,
            &mut state.gui_state.active_help_topic,
        );

        // Full-screen image viewer when a chat image was clicked.
        crate::gui::widgets::image_cache_view::draw(
            ctx,
            &state.theme,
            &mut state.gui_state,
        );
        // A 3D model posted in chat, in its own window (View in 3D on its card).
        crate::gui::pages::chat::draw_model_window(ctx, &state.theme, &mut state.gui_state.chat_model_view);
        // The dock a playing chat video moves into when its row leaves the
        // view, on every page but the world (chat/video_dock.rs), then the
        // Full window and the chat videos' end of frame.
        crate::gui::pages::chat::draw_video_dock(ctx, &state.theme, &mut state.gui_state);
        crate::gui::pages::chat::draw_video_window(ctx, &state.theme, &mut state.gui_state.chat_video);
        // A chat video's "Volume settings" link (Settings > Audio).
        crate::gui::pages::chat::follow_volume_settings_link(&mut state.gui_state);
        // A Media item's window brought up from a chat card (files/chat_card.rs).
        crate::gui::pages::files::draw_item_window(ctx, &state.theme, &mut state.gui_state);
        // The upload window a drop opened from another page (Add to Media's form).
        crate::gui::pages::files::draw_upload_window(ctx, &state.theme, &mut state.gui_state);
        // Dropped files go to the page or window that took them, and while
        // files are dragged over the window, it says where they will go.
        crate::gui::drop_files::finish(ctx, &state.theme, &mut state.gui_state);

        // Crew NPC nameplates (v0.667): rebuild every frame from the
        // live RemoteNpc components so the name + current chore label
        // float over each crew member (the amber figures from the
        // v0.663 crew chore AI). Anchor just above the head sphere
        // (head center at +0.55, radius 0.17 -- see the crew render
        // pass). clear() also empties the HUD when crew despawn on
        // leaving the world; a query on an empty world is free.
        // Labels project with the SAME station offset the
        // scene pass applies to home content (v0.911: the
        // ghost-markers half of the vanishing-home bug).
        state.gui_state.station_off = state.station_off;
        // Where the compass's N goes: true north, aboard too.
        state.gui_state.compass_north_yaw = humanity_ship::station::compass_north_yaw(
            state.camera.surface_mode,
            state.station_ride,
            state.station_world_rot,
        );
        // The crew's and other players' names
        // (engine::net_route::nameplate_labels).
        state.gui_state.crew_labels = crate::engine::net_route::nameplate_labels(
            &state.game_world.world,
            state.station_off,
            &|p| crate::engine::block::hides_player(&state.gui_state, p),
        );

        // Draw HUD when in-game. SKIP it during the showroom AND the
        // construction editor: the HUD allocates a full-screen Area
        // (hud.rs) which sits OVER an in-world side panel and eats its
        // clicks -- the real cause of "panel shows but won't click".
        // (v0.461; the showroom already skipped it, which is why it
        // worked and the editor did not.)
        if state.gui_state.active_page == GuiPage::None
            && state.gui_state.show_hud
            && !state.gui_state.showroom_active
            && !state.gui_state.construction_active
        {
            hud::draw(
                ctx,
                &state.theme,
                &state.gui_state,
                state.camera.yaw,
                state.camera.view_projection_matrix(),
                state.camera.position,
            );
            // Interactive auto-recipe dropdown under the pinned
            // machine card (v0.725) — separate from the paint-only
            // HUD layer because it needs clicks + &mut state.
            hud::draw_machine_recipe_selector(
                ctx,
                &state.theme,
                &mut state.gui_state,
            );
        }
        // Build-mode CAD dimension overlay (v0.545): wall lengths, corner
        // angles, live drawing readout. Also shows in play when the dev
        // overlay is on (v0.547).
        if state.gui_state.active_page == GuiPage::None
            && (state.gui_state.construction_active
                || state.gui_state.construction_dev_overlay)
            && state.gui_state.construction_dimension_overlay // v0.595 toggle
            && !state.gui_state.showroom_active
        {
            hud::draw_construction_overlay(
                ctx,
                &state.theme,
                &state.gui_state,
                state.camera.view_projection_matrix(),
            );
        }

        // Character-select showroom panel (v0.441): appearance +
        // backdrop + Enter, over the orbiting avatar.
        if state.gui_state.active_page == GuiPage::None
            && state.gui_state.showroom_active
        {
            crate::gui::pages::showroom::draw(ctx, &state.theme, &mut state.gui_state);
        }

        // 1:1 voice-call surfaces ring EVERYWHERE, not just on the
        // Chat page (v0.703): the web caller gives up after 30 s, so
        // a ring must be answerable while driving, building, or on
        // any other page. Chat's own draw already renders these when
        // it's the active page; drawing the same-titled egui Windows
        // twice in one frame is fine (same ID, one instance), but we
        // gate anyway to keep a single draw path per frame.
        if state.gui_state.active_page != GuiPage::Chat
            && (state.gui_state.call_incoming.is_some()
                || state.gui_state.call_active.is_some()
                || state.gui_state.call_outgoing.is_some())
        {
            crate::gui::pages::chat::draw_incoming_call_modal(ctx, &state.theme, &mut state.gui_state);
            crate::gui::pages::chat::draw_call_bar(ctx, &state.theme, &mut state.gui_state);
        }

        // Privacy-tier first-connect chooser (2026-08-23): asked ONCE per
        // identity, over any page (privacy.rs `first_connect_prompt`, which
        // the click test draws the same way).
        crate::gui::pages::privacy::first_connect_prompt(ctx, &state.theme, &mut state.gui_state);

        // Vendor modal (v0.747, ladder rung 3): opened from
        // the trading post's card; draws over any page.
        if state.gui_state.vendor_open {
            crate::gui::pages::vendor::draw_vendor_modal(ctx, &state.theme, &mut state.gui_state);
        }

        // Death screen (v0.745, loop-map rung 1): overlays
        // EVERYTHING while the player is dead; its Respawn
        // button is handled in the frame bridge above.
        if state.gui_state.player_death_cause.is_some() {
            crate::gui::pages::hud::draw_death_screen(ctx, &state.theme, &mut state.gui_state);
        }

        // Construction editor panel (v0.455): per-wall kinds + height,
        // rebuilds the home live. Toggled with B.
        if state.gui_state.active_page == GuiPage::None
            && state.gui_state.construction_active
        {
            crate::gui::pages::construction::draw(ctx, &state.theme, &mut state.gui_state);
            // In-world screens in build mode (2026-11-06): the handles on
            // the screen under the cursor and the selected one, and the
            // settings panel its cog opened.
            let win = gpu!(state).window.inner_size();
            crate::engine::screens::edit::draw_handles(
                ctx,
                &state.theme,
                &state.screens.quads,
                state.station_off,
                &state.camera,
                state.cursor_pos,
                (win.width as f32, win.height as f32),
                state.construction_object_grab.as_ref(),
                state.gui_state.construction_machine_selected.as_deref(),
            );
            crate::gui::pages::screen_settings::draw(ctx, &state.theme, &mut state.gui_state);
        }

        // (The old show_chat Enter-toggle overlay was removed in
        // v0.779 -- Enter opens the interactive panel below, and
        // nothing set show_chat anymore. chat::draw still serves
        // the Chat PAGE via the nav path above.)

        // Death closes any in-world modal (v0.779): the death
        // screen owns the keyboard + clicks; a panel left open
        // (or opened by reflex-Enter) would sit hidden UNDER the
        // Foreground dim stealing keystrokes.
        if state.gui_state.player_death_cause.is_some()
            && state.gui_state.in_world_modal_open()
        {
            state.gui_state.close_in_world_modals();
        }

        // In-world interactive chat panel (v0.772): Enter opens it,
        // which frees the cursor + disables look/move so you can read
        // and type in the same relay chat as the Chat page/website.
        if state.gui_state.active_page == GuiPage::None
            && state.gui_state.chat_input_active
        {
            chat::draw_ingame_chat(ctx, &state.theme, &mut state.gui_state);
        }

        // Walk-up creature editor (v0.778): G opens it for the
        // creature you face; edits write back live (see the
        // consumer above). Cursor-free like the chat panel.
        if state.gui_state.active_page == GuiPage::None
            && state.gui_state.dev_edit_target.is_some()
        {
            crate::gui::pages::dev::draw_creature_editor(
                ctx,
                &state.theme,
                &mut state.gui_state,
            );
        }

        // Walk-up NPC dialogue card (v0.797): E on a faced
        // crew member opens it. Cursor-free via the same
        // in-world-modal plumbing as the chat panel; the
        // frame bridge above auto-closes it if the NPC
        // wanders off or despawns.
        if state.gui_state.active_page == GuiPage::None
            && state.gui_state.npc_talk_target.is_some()
        {
            hud::draw_npc_talk_card(ctx, &state.theme, &mut state.gui_state);
        }

        // Passphrase modal overlay (blocks interaction until resolved)
        if state.gui_state.passphrase_needed {
            crate::gui::pages::passphrase_modal::draw(ctx, &state.theme, &mut state.gui_state);
        }
        // The protected setup's PIN prompt, while a locked action waits for it (step G).
        crate::gui::pages::safety_protected::draw_pin_prompt(ctx, &state.theme, &mut state.gui_state);

        // The planet card when targeting a hologram pin: painted on the tooltip layer, so it
        // takes no click (gui/pages/hud_planet_card.rs). Data-driven (infinite-of-X): the body
        // table lives in data/planets/tooltips.json, not a match arm here.
        if let Some(ref planet_name) = state.targeted_planet {
            let (resources, status) = planet_tooltip_info(planet_name);
            hud::planet_card::draw(ctx, &state.theme, planet_name, &resources, &status);
        }

        // Crosshair (small dot at screen center when in game,
        // hidden with the HUD: hud::crosshair_visible)
        if hud::crosshair_visible(&state.gui_state) {
            let screen = ctx.screen_rect();
            let center = screen.center();
            // Part of the world's picture (41+, gui/layers.rs): under every popup.
            let painter = crate::gui::layers::painter(ctx, egui::Id::new("crosshair"), crate::gui::layers::Layer::Page);
            let color = if state.targeted_planet.is_some() {
                state.theme.info() // targeting highlight - the theme's blue info token
            } else {
                egui::Color32::from_white_alpha(180)
            };
            painter.circle_filled(center, 3.0, color);
            // In-world screens' aim assist (2026-11-06): the outline round
            // the widget a click goes to, and the line to it from the
            // crosshair (or the free cursor).
            if let Some(shown) = state.screens.aim_shown.as_ref() {
                let from = if state.cursor_free {
                    let ppp = ctx.pixels_per_point().max(0.01);
                    egui::pos2(state.cursor_pos.0 / ppp, state.cursor_pos.1 / ppp)
                } else {
                    center
                };
                crate::engine::screens::aim::draw_overlay(ctx, &state.theme, shown, state.camera.view_projection_matrix(), from);
            }
        }

        // Top-right help toggle (v0.1212): "?" when the panel is
        // closed, "X" while it is open, always the same spot so
        // the way in and the way out never move. Draws itself on
        // every screen EXCEPT the first-person world view, where
        // F1 remains the way in.
        crate::gui::pages::keymap::draw_help_toggle(
            ctx,
            &state.theme,
            &mut state.gui_state,
        );
        // The panel itself: transient while F1 is held (v0.465,
        // unchanged) or sticky while pinned by the button above.
        if state.gui_state.keymap_visible
            || state.gui_state.help_panel_pinned
        {
            crate::gui::pages::keymap::draw(ctx, &state.theme, &mut state.gui_state);
        }
        // Diagnostics dev-HUD overlays (F2/F3/F4), v0.482.
        crate::gui::pages::diagnostics::draw(ctx, &state.theme, &state.gui_state);
        // F11 weather panel (v0.1050). Publishes into
        // the DataStore slot on any change, so the
        // sim picks it up on its next tick.
        let weather_changed = crate::gui::pages::weather_panel::draw(
            ctx,
            &state.theme,
            &mut state.gui_state,
        );
        // F10 cloud dev panel (v0.1254.4): mirror the
        // panel state into the SAME renderer flags the
        // showcase pins drive, every frame - buttons
        // and file drops can never disagree.
        crate::gui::pages::cloud_dev::draw(
            ctx,
            &state.theme,
            &mut state.gui_state,
        );
        gpu!(state).renderer.cloud_dither_off =
            state.gui_state.cloud_dev_dither_off;
        gpu!(state).renderer.cloud_temporal_off =
            state.gui_state.cloud_dev_temporal_off;
        gpu!(state).renderer.cloud_map_diag =
            state.gui_state.cloud_dev_map_diag as f32;
        gpu!(state).renderer.cloud_clock_pin =
            state.gui_state.cloud_dev_clock_pin;
        gpu!(state).renderer.cloud_chord_foot =
            state.gui_state.cloud_dev_chord_foot;
        gpu!(state).renderer.cloud_world_shape_lod =
            state.gui_state.cloud_dev_world_shape_lod;
        gpu!(state).renderer.cloud_ring_cure_off =
            state.gui_state.cloud_dev_ring_cure_off;
        gpu!(state).renderer.cloud_uniform_step =
            state.gui_state.cloud_dev_uniform_step;
        gpu!(state).renderer.cloud_wide_edge =
            state.gui_state.cloud_dev_wide_edge;
        gpu!(state).renderer.cloud_edge_mul =
            state.gui_state.cloud_dev_edge_mul;
        gpu!(state).renderer.cloud_rind_wide_m =
            state.gui_state.cloud_dev_rind_wide_m;
        gpu!(state).renderer.cloud_step_m =
            state.gui_state.cloud_dev_step_m;
        gpu!(state).renderer.cloud_shear =
            state.gui_state.cloud_dev_shear;
        gpu!(state).renderer.cloud_hv_km =
            state.gui_state.cloud_dev_hv_km;
        gpu!(state).renderer.cloud_sigma_mul =
            state.gui_state.cloud_dev_sigma_mul;
        gpu!(state).renderer.cloud_est =
            state.gui_state.cloud_dev_est;
        gpu!(state).renderer.cloud_warp_bl =
            state.gui_state.cloud_dev_warp_bl;
        gpu!(state).renderer.cloud_norm_floor =
            state.gui_state.cloud_dev_norm_floor;
        gpu!(state).renderer.cloud_iso_step =
            state.gui_state.cloud_dev_iso_step;
        gpu!(state).renderer.cloud_thin_deck =
            state.gui_state.cloud_dev_thin_deck;
        gpu!(state).renderer.cloud_hv_warp =
            state.gui_state.cloud_dev_hv_warp;
        gpu!(state).renderer.cloud_no_detail = state.gui_state.cloud_dev_no_detail;
        gpu!(state).renderer.cloud_no_puff = state.gui_state.cloud_dev_no_puff;
        gpu!(state).renderer.cloud_no_cell = state.gui_state.cloud_dev_no_cell;
        gpu!(state).renderer.cloud_no_fray = state.gui_state.cloud_dev_no_fray;
        gpu!(state).renderer.cloud_no_bdrop = state.gui_state.cloud_dev_no_bdrop;
        gpu!(state).renderer.cloud_sharp_base = state.gui_state.cloud_dev_sharp_base;
        gpu!(state).renderer.cloud_relief_fade = state.gui_state.cloud_dev_relief_fade;
        gpu!(state).renderer.cloud_deep_rung = state.gui_state.cloud_dev_deep_rung;
        gpu!(state).renderer.cloud_checker = state.gui_state.cloud_dev_checker;
        gpu!(state).renderer.cloud_ms = state.gui_state.cloud_dev_ms;
        gpu!(state).renderer.cloud_field = state.gui_state.cloud_dev_field;
        gpu!(state).renderer.cloud_body_cache = state.gui_state.cloud_dev_body_cache;
        gpu!(state).renderer.cloud_step_eco = state.gui_state.cloud_dev_step_eco;
        gpu!(state).renderer.cloud_light = state.gui_state.cloud_dev_light;
        gpu!(state).renderer.cloud_profile_knob = state.gui_state.cloud_dev_profile_knob;
        // D3 dev bit (the built-body top bound): flags-pad bit 12, A/B'd by the rig.
        gpu!(state).renderer.cloud_top_bound = state.gui_state.cloud_dev_top_bound;
        gpu!(state).renderer.cloud_int_sat = state.gui_state.cloud_dev_int_sat;
        gpu!(state).renderer.cloud_ms_gain = state.gui_state.cloud_dev_ms_gain;
        gpu!(state).renderer.cloud_res_div =
            state.gui_state.cloud_dev_res_div;
        gpu!(state).renderer.cloud_shape_off =
            state.gui_state.cloud_dev_shape_off;
        gpu!(state).renderer.cloud_discard_diag =
            state.gui_state.cloud_dev_discard_diag;
        // Time scrubber (v0.1224). Drained every frame
        // regardless of the weather-changed flag: the two
        // live in one panel but are independent channels,
        // and a dropped hour request would read as the
        // slider silently not working.
        if let Some(h) = state.gui_state.time_hour_request.take() {
            if let Some(m) = state
                .data_store
                .get::<std::sync::Mutex<Option<f32>>>(
                    "time_set_hour_request",
                )
            {
                if let Ok(mut r) = m.lock() {
                    *r = Some(h);
                }
            }
        }
        if let Some(hold) = state.gui_state.time_scale_request.take() {
            humanity_systems::time::request_speed_hold(&state.data_store, hold);
        }
        if weather_changed {
            if let Some(m) = state
                .data_store
                .get::<std::sync::Mutex<humanity_systems::weather::WeatherControl>>(
                    "weather_control",
                )
            {
                if let Ok(mut c) = m.lock() {
                    c.manual = state.gui_state.weather_manual.then(|| {
                        humanity_systems::weather::ManualWeather {
                            condition: state.gui_state.weather_pick_condition,
                            intensity: state.gui_state.weather_pick_intensity,
                            wind_speed: state.gui_state.weather_pick_wind,
                        }
                    });
                    if state.gui_state.weather_retrigger {
                        c.retrigger = true;
                        state.gui_state.weather_retrigger = false;
                    }
                }
            }
        }

        // The OpenStreetMap credit, on its own layer and after every side
        // panel, whenever a region's geometry is drawn: in play, with the HUD
        // off, in the construction editor and in the showroom (whole-Earth
        // maps rung 1, gui/map_credit.rs).
        crate::gui::map_credit::draw(ctx, &state.theme, &state.gui_state);
        // Confirmation toasts (v0.861): floats a "Saved" style
        // note over everything so actions are never silent.
        crate::gui::widgets::draw_toasts(ctx, &state.theme, &mut state.gui_state);
        // The first five minutes' tour card (gui/first_minutes.rs): bottom right,
        // over the open page, never over the world.
        crate::gui::first_minutes::draw(ctx, &state.theme, &mut state.gui_state);
        crate::gui::widgets::draw_underwater_tint(ctx, &state.gui_state);

        // Draw debug console overlay (F12 toggle, on top of everything)
        humanity_core::debug::draw_debug_console(
            ctx,
            &mut state.gui_state.debug_log,
            &mut state.gui_state.debug_console_visible,
        );
        // Last: a link to a channel on this or a saved server, clicked
        // anywhere this frame, opens here instead of in the browser.
        crate::gui::pages::chat::channel_link::open_in_app(ctx, &mut state.gui_state);
        // The one layer order (gui/layers.rs, navigation rung 3), once
        // everything is drawn: the urgent band over every popup, the popups
        // over the menus' drop-downs, those over the page's windows.
        crate::gui::layers::arrange(ctx);
        // Whether a popup asking for a decision is up, for the world's notices to wait on
        // (gui/world_entry.rs `world_notice`, BUG-316).
        state.gui_state.decision_open = crate::gui::layers::deciding(ctx);
    });
    // Quit from the main menu hub or the updater's Restart to Apply
    // (first-hour audit 2026-10-04, Blocker 5): the save the close
    // button runs, then exit. Out here, not in the egui closure above,
    // because the save takes the whole state, which that closure holds.
    if std::mem::take(&mut state.gui_state.quit_requested) {
        crate::engine::quit::save_before_exit(state);
        event_loop.exit();
    }

    // ── Game audio frame sync (v0.960, first CC0 sounds) ──
    full_output
}
