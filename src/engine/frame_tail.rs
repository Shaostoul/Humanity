//! The frame's tail (the file-size split, step 1, 2026-11-14), moved out of
//! `window_event` in src/lib.rs as it was: screenshots, the play journal's marks, the movie and the
//! live broadcast copied from the finished image, the present, the draw lists handed back to
//! `EngineState::draw_bufs` for the next frame, and the settings applied when they changed.

use crate::engine::frame_prelude::*;

/// Captures, presents `surface_texture` and finishes the frame.
pub(crate) fn run(state: &mut EngineState, f: &mut FrameVars, surface_texture: wgpu::SurfaceTexture) {
    let frame_t0 = f.frame_t0;
    let page_before_frame = f.page_before_frame;
    let page_active = f.page_active;
    humanity_core::frame_costs::span("cpu.span.tail");
    let scene_lists = SceneDrawLists {
        celestial: &f.celestial_objects,
        celestial_transparent: &f.celestial_transparent,
        orbit_lines: &f.orbit_lines,
        opaque: &f.all_objects,
        transparent: &f.transparent_objects,
        overlay: &f.overlay_objects,
        ring_lines: &f.ring_lines,
    };
    // GUI-triggered capture (Testing page buttons, v0.810): the
    // same engine path as the file protocol, minus the JSON
    // round-trip. (0,0) = window-size swapchain grab.
    if let Some(size) = state.gui_state.screenshot_capture_request.take() {
        let size = match size {
            (0, 0) => ScreenshotSize::Window,
            (w, h) => ScreenshotSize::Custom(w, h),
        };
        execute_screenshot_capture(
            state,
            &surface_texture.texture,
            size,
            &scene_lists,
            None,
        );
    }
    poll_screenshot_request(state, &surface_texture.texture, &scene_lists);
    crate::engine::play_journal_frame::take_mark(state, &surface_texture.texture);
    // The play journal's page, frame, world and net kinds: what changed
    // this frame (engine/play_journal_frame.rs `note_frame`).
    crate::engine::play_journal_frame::note_frame(state);
    crate::engine::movie::capture(state, &surface_texture.texture);
    // The rigs' verbs (`ipc::poll_showcase_request`) are polled by lib.rs after this stage on
    // every frame, presented or not (the headless sim runner's increment 3, 2026-11-15): inside
    // this branch a frame that acquired no swapchain image (a minimised window, a lost surface)
    // held the rigs' input and their timed key holds still.

    // Live weather (v0.874): upload any freshly
    // fetched NASA cloud grid. Rare event (cache at
    // boot + one fetch per 30 min); latest wins if
    // several queued. Toggle off = stop uploading
    // (the last real map stays until restart).
    if state.gui_state.settings.live_weather {
        let mut latest: Option<Vec<u8>> = None;
        if let Some(rx) = &state.weather_rx {
            while let Ok(grid) = rx.try_recv() {
                latest = Some(grid);
            }
        }
        if let Some(grid) = latest {
            let gpu = gpu!(state);
            if let Some(r) = gpu.renderer.world() {
                r.update_weather_map(&gpu.renderer.queue, &grid);
            }
            state.weather_grid = Some(grid);
            // Fresh satellite data owns placement
            // again (v0.1032): move the accumulated
            // wind-advection angle into the decaying
            // bucket so the deck EASES back onto real
            // geography instead of snapping.
            state.cloud_advect_decaying += state.cloud_advect;
            state.cloud_advect = 0.0;
            log::info!("[Weather] live cloud map applied to sky");
        }
    }

    // Live broadcast (v0.853). Deliberately here, AFTER egui has
    // painted into the swapchain and BEFORE present: what goes out
    // is exactly what the operator sees, overlays and all -- the
    // same thing OBS's window capture would grab.
    pump_live_broadcast(state, &surface_texture.texture);

    humanity_core::frame_costs::span_end();
    // `cpu.frame_total` closes HERE: handler entry to the
    // last submit, which is the broadcast copy above when
    // a stream is live (the 2026-09-18 review found the
    // total closed at the egui submit, leaving that copy
    // outside it). World frames only - a UI-only frame's
    // accumulation is discarded at its acquire, and a
    // stage recorded after that discard would leak into
    // the next world frame.
    if !page_active {
        humanity_core::frame_costs::record_cpu(
            "cpu.frame_total",
            frame_t0.elapsed(),
        );
    }

    // `cpu.present_wait`, half 2 (half 1 is around
    // `get_current_texture` in `acquire_surface`):
    // present blocks when the swapchain queue is full,
    // which is what "waiting for the display" costs
    // on the CPU. World frames only (see frame_total).
    // The present and the work after it as laps only (humanity_core::frame_costs: no span covers
    // them), for the world preparation's long-gap lines (engine/world_prep.rs `note_frame_end`).
    humanity_core::frame_costs::lap("lap.present");
    let present_t0 = Instant::now();
    surface_texture.present();
    humanity_core::frame_costs::lap("lap.after_present");
    // The draw lists' memory, kept for the next frame (DrawBufs, S3). Taken out of the frame's
    // FrameVars, which ends with this frame.
    state.draw_bufs = crate::engine::state::DrawBufs {
        opaque: std::mem::take(&mut f.all_objects),
        transparent: std::mem::take(&mut f.transparent_objects),
        overlay: std::mem::take(&mut f.overlay_objects),
        celestial: std::mem::take(&mut f.celestial_objects),
        celestial_transparent: std::mem::take(&mut f.celestial_transparent),
        orbit_lines: std::mem::take(&mut f.orbit_lines),
        ring_lines: std::mem::take(&mut f.ring_lines),
    };
    if !page_active {
        humanity_core::frame_costs::record_cpu(
            "cpu.present_wait",
            present_t0.elapsed(),
        );
    }

    // Catch a page change made by an egui click this frame (the
    // per-frame reconciliation ran before the egui frame). Same single
    // authority, so no `cursor_free` desync. (v0.460)
    let _ = page_before_frame;
    reconcile_cursor(state);
    // Answer a main-UI dev IPC request whose last event
    // landed in the egui frame above (2026-09-18). After
    // the reconcile on purpose: "f10" and "escape" are
    // judged by whether the cursor authority freed or
    // re-grabbed the cursor, and this is the call that did.
    crate::engine::ipc::complete_ui_request(state);

    // Land the connect-time donation-info fetch (spawned in the
    // `peer_list` WS handler) whenever it finishes -- polled here
    // rather than in the Donate page's own draw fn so it lands even
    // if the user is elsewhere in the app when it completes.
    if let Some((from_url, rx)) = &state.gui_state.donate_info_rx {
        match rx.try_recv() {
            Ok(res) => {
                let from_url = from_url.clone();
                state.gui_state.donate_info_rx = None;
                if let Ok(info) = res {
                    // apply_server_funding discards a result tagged with a
                    // server we've since switched away from, and clears the
                    // fields when this server reports no funding -- both the
                    // adversarial review's findings. Fetch errors leave the
                    // fields as the connect handler set them (cleared unless
                    // still on the same server).
                    state.gui_state.apply_server_funding(&from_url, info.funding.as_ref());
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                state.gui_state.donate_info_rx = None;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    // ── Star catalog download/remove (v0.800 rung 2; 2026-07-11
    // rung 4 generalizes to a tier parameter) ── The Settings card
    // sets the request fields with the clicked tier; this owns
    // data_dir + the thread. Progress rides an Arc<Mutex> the card
    // renders, tagged with the tier so the bar can name it.
    if let Some(tier) = state.gui_state.star_catalog_download.take() {
        // A FAILED handle only lingers for display; a retry click
        // replaces it (otherwise the button would stay dead).
        let failed = state
            .gui_state
            .star_catalog_dl
            .as_ref()
            .and_then(|(_, p)| p.lock().ok().map(|g| g.2.starts_with("FAILED")))
            .unwrap_or(false);
        if failed {
            state.gui_state.star_catalog_dl = None;
        }
        if state.gui_state.star_catalog_dl.is_none() {
            let prog = std::sync::Arc::new(std::sync::Mutex::new(
                (0u64, 0u64, "Connecting...".to_string()),
            ));
            state.gui_state.star_catalog_dl = Some((tier, prog.clone()));
            let dest = state.data_dir.join(tier.filename());
            // Release-asset URLs under a non-v* tag: uploading data
            // assets there triggers no CI workflow and gets GitHub's
            // CDN for free. Extended is generated by
            // scripts/build-athyg-bin.js, ultra by
            // scripts/build-gaia-bin.js. A tier whose asset is not
            // uploaded yet 404s into the FAILED retry state below.
            let url = tier.url();
            std::thread::spawn(move || {
                let result = (|| -> Result<(), String> {
                    let resp = ureq::get(url)
                        .set("User-Agent", "HumanityOS")
                        .call()
                        .map_err(|e| format!("HTTP error: {e}"))?;
                    let total = resp
                        .header("Content-Length")
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(0);
                    if let Ok(mut g) = prog.lock() {
                        g.1 = total;
                        g.2 = "Downloading".to_string();
                    }
                    // Stream to a .tmp sibling; only a complete,
                    // magic-verified file is renamed into place, so a
                    // dropped connection can't leave a corrupt catalog
                    // where the loader will find it.
                    let tmp = dest.with_extension("bin.tmp");
                    let mut reader = resp.into_reader();
                    let mut file = std::fs::File::create(&tmp)
                        .map_err(|e| format!("create failed: {e}"))?;
                    let mut buf = [0u8; 65536];
                    let mut done: u64 = 0;
                    loop {
                        let n = std::io::Read::read(&mut reader, &mut buf)
                            .map_err(|e| format!("read failed: {e}"))?;
                        if n == 0 {
                            break;
                        }
                        std::io::Write::write_all(&mut file, &buf[..n])
                            .map_err(|e| format!("write failed: {e}"))?;
                        done += n as u64;
                        if let Ok(mut g) = prog.lock() {
                            g.0 = done;
                        }
                    }
                    drop(file);
                    let mut head = [0u8; 8];
                    std::fs::File::open(&tmp)
                        .and_then(|mut f| std::io::Read::read_exact(&mut f, &mut head))
                        .map_err(|e| format!("verify failed: {e}"))?;
                    if &head != b"HOSSTAR1" {
                        let _ = std::fs::remove_file(&tmp);
                        return Err("downloaded file is not a star catalog".to_string());
                    }
                    std::fs::rename(&tmp, &dest)
                        .map_err(|e| format!("install failed: {e}"))?;
                    Ok(())
                })();
                if let Ok(mut g) = prog.lock() {
                    g.2 = match result {
                        Ok(()) => "Done".to_string(),
                        Err(e) => format!("FAILED: {e}"),
                    };
                }
            });
        }
    }
    if let Some(tier) = state.gui_state.star_catalog_remove.take() {
        let dest = state.data_dir.join(tier.filename());
        match std::fs::remove_file(&dest) {
            Ok(()) => {
                state.gui_state.star_catalog_installed[tier.index()] = None;
                log::info!(
                    "{} star catalog removed; next-biggest installed catalog on next world entry",
                    tier.label()
                );
            }
            Err(e) => log::warn!("Could not remove {}: {e}", dest.display()),
        }
    }
    // Download finished this frame? Reflect the installed state for
    // ITS tier and drop the progress handle (Done) or keep it
    // visible (FAILED, cleared when the user retries).
    let dl_done_tier = state
        .gui_state
        .star_catalog_dl
        .as_ref()
        .and_then(|(t, p)| {
            p.lock().ok().and_then(|g| if g.2 == "Done" { Some(*t) } else { None })
        });
    if let Some(tier) = dl_done_tier {
        state.gui_state.star_catalog_dl = None;
        state.gui_state.star_catalog_installed[tier.index()] =
            std::fs::metadata(state.data_dir.join(tier.filename()))
                .ok()
                .map(|m| m.len());
    }

    // ── Instrument packs (2026-11-06, music.md rung 6): Settings > Audio >
    // Instruments' Download / Remove, the star catalog's way (a thread, a
    // checked archive, progress the section draws).
    humanity_core::frame_costs::lap("lap.instrument_packs");
    crate::instrument_packs::ui::tick(&mut crate::instrument_packs::ui::shared(), &state.data_dir);
    // Model packs (2026-11-09, simready-props.md): Settings > Graphics > Furniture models.
    humanity_core::frame_costs::lap("lap.model_packs");
    crate::model_packs::ui::tick(&mut crate::model_packs::ui::shared(), &state.data_dir);
    // Garment texture levels and packs (2026-11-10): Settings > Graphics > Garment textures.
    humanity_core::frame_costs::lap("lap.garment_textures");
    crate::engine::look_textures::tick(&state.data_dir);
    humanity_core::frame_costs::lap("lap.tail_downloads");

    // ── Ultra Milky Way glow download/remove (2026-07-11) ── same
    // mechanics as the star catalog above: background thread, stream
    // to a .tmp sibling, verify, rename into place. Verification here
    // is the PNG header dimensions (must be exactly the published
    // 16384x8192 bake) PLUS a full decode - a truncated or wrong
    // asset never lands where the loader will find it.
    if state.gui_state.galaxy_glow_download {
        state.gui_state.galaxy_glow_download = false;
        let failed = state
            .gui_state
            .galaxy_glow_dl
            .as_ref()
            .and_then(|p| p.lock().ok().map(|g| g.2.starts_with("FAILED")))
            .unwrap_or(false);
        if failed {
            state.gui_state.galaxy_glow_dl = None;
        }
        if state.gui_state.galaxy_glow_dl.is_none() {
            let prog = std::sync::Arc::new(std::sync::Mutex::new(
                (0u64, 0u64, "Connecting...".to_string()),
            ));
            state.gui_state.galaxy_glow_dl = Some(prog.clone());
            let dest = state
                .data_dir
                .join(crate::renderer::stars::GALAXY_GLOW_ULTRA_FILE);
            let url = crate::renderer::stars::GALAXY_GLOW_ULTRA_URL;
            std::thread::spawn(move || {
                let result = (|| -> Result<(), String> {
                    let resp = ureq::get(url)
                        .set("User-Agent", "HumanityOS")
                        .call()
                        .map_err(|e| format!("HTTP error: {e}"))?;
                    let total = resp
                        .header("Content-Length")
                        .and_then(|v| v.parse::<u64>().ok())
                        .unwrap_or(0);
                    if let Ok(mut g) = prog.lock() {
                        g.1 = total;
                        g.2 = "Downloading".to_string();
                    }
                    let tmp = dest.with_extension("png.tmp");
                    let mut reader = resp.into_reader();
                    let mut file = std::fs::File::create(&tmp)
                        .map_err(|e| format!("create failed: {e}"))?;
                    let mut buf = [0u8; 65536];
                    let mut done: u64 = 0;
                    loop {
                        let n = std::io::Read::read(&mut reader, &mut buf)
                            .map_err(|e| format!("read failed: {e}"))?;
                        if n == 0 {
                            break;
                        }
                        std::io::Write::write_all(&mut file, &buf[..n])
                            .map_err(|e| format!("write failed: {e}"))?;
                        done += n as u64;
                        if let Ok(mut g) = prog.lock() {
                            g.0 = done;
                        }
                    }
                    drop(file);
                    if let Ok(mut g) = prog.lock() {
                        g.2 = "Verifying".to_string();
                    }
                    let want =
                        crate::renderer::stars::GALAXY_GLOW_ULTRA_DIMS;
                    // Verify by CONTENT, not file extension. The download
                    // lands in a `.tmp` file, and image::image_dimensions
                    // guesses the format from the extension, so it wrongly
                    // rejected a perfectly good PNG with "the file
                    // extension .tmp was not recognized as an image
                    // format". with_guessed_format() sniffs the magic
                    // bytes instead. (bugfix v0.860)
                    let dims = image::ImageReader::open(&tmp)
                        .map_err(|e| format!("verify failed: {e}"))?
                        .with_guessed_format()
                        .map_err(|e| format!("verify failed: {e}"))?
                        .into_dimensions()
                        .map_err(|e| format!("verify failed: {e}"))?;
                    if dims != want {
                        let _ = std::fs::remove_file(&tmp);
                        return Err(format!(
                            "downloaded file is {}x{}, expected {}x{}",
                            dims.0, dims.1, want.0, want.1
                        ));
                    }
                    // Full decode (~30s of CPU in this background
                    // thread): the header check above cannot catch a
                    // truncated transfer; a decode of every scanline
                    // can. Also content-based (see the dims note): a
                    // `.tmp` extension must not defeat the format sniff.
                    let decode_result = image::ImageReader::open(&tmp)
                        .and_then(|r| r.with_guessed_format())
                        .map_err(|e| format!("decode check failed: {e}"))
                        .and_then(|r| {
                            r.decode()
                                .map(|_| ())
                                .map_err(|e| format!("decode check failed: {e}"))
                        });
                    if let Err(e) = decode_result {
                        let _ = std::fs::remove_file(&tmp);
                        return Err(e);
                    }
                    std::fs::rename(&tmp, &dest)
                        .map_err(|e| format!("install failed: {e}"))?;
                    Ok(())
                })();
                if let Ok(mut g) = prog.lock() {
                    g.2 = match result {
                        Ok(()) => "Done".to_string(),
                        Err(e) => format!("FAILED: {e}"),
                    };
                }
            });
        }
    }
    if state.gui_state.galaxy_glow_remove {
        state.gui_state.galaxy_glow_remove = false;
        let dest = state
            .data_dir
            .join(crate::renderer::stars::GALAXY_GLOW_ULTRA_FILE);
        match std::fs::remove_file(&dest) {
            Ok(()) => {
                state.gui_state.galaxy_glow_installed = None;
                // Point the setting back at the texture that exists;
                // leaving it on ultra would just warn-and-fall-back
                // every world entry.
                if state.gui_state.settings.sky_glow_tier == "ultra" {
                    state.gui_state.settings.sky_glow_tier =
                        "standard".to_string();
                    state.gui_state.settings_dirty = true;
                }
                log::info!(
                    "Ultra Milky Way glow texture removed; standard glow on next world entry"
                );
            }
            Err(e) => log::warn!("Could not remove {}: {e}", dest.display()),
        }
    }
    // Glow download finished this frame? Reflect installed state,
    // drop the handle (Done) or keep it visible (FAILED, cleared on
    // retry), and select the ultra tier: the user clicked Download
    // because they want the sharper bake, so it goes live on the
    // next world entry without a second trip to the chooser.
    let glow_dl_done = state
        .gui_state
        .galaxy_glow_dl
        .as_ref()
        .and_then(|p| p.lock().ok().map(|g| g.2 == "Done"))
        .unwrap_or(false);
    if glow_dl_done {
        state.gui_state.galaxy_glow_dl = None;
        state.gui_state.galaxy_glow_installed = std::fs::metadata(
            state
                .data_dir
                .join(crate::renderer::stars::GALAXY_GLOW_ULTRA_FILE),
        )
        .ok()
        .map(|m| m.len());
        if state.gui_state.galaxy_glow_installed.is_some()
            && state.gui_state.settings.sky_glow_tier != "ultra"
        {
            state.gui_state.settings.sky_glow_tier = "ultra".to_string();
            state.gui_state.settings_dirty = true;
        }
    }

    // A video's volume bar moved: the last volume is saved with the settings
    // (humanity_core::audio::start_volume, Settings > Audio).
    if humanity_core::audio::start_volume::take_unsaved() {
        state.gui_state.settings_dirty = true;
    }
    // ── Apply settings changes from GUI to engine ──
    if state.gui_state.settings_dirty {
        state.gui_state.settings_dirty = false;
        humanity_core::frame_costs::lap("lap.settings_apply");

        // FOV. Clamped to a sane range so a degenerate
        // value in the config (e.g. a `"fov": 0.0` written
        // by an old build or a corrupt settings file) can
        // never collapse the perspective projection to a
        // point and black out the entire 3D scene with no
        // in-app way to recover. 60..120 deg matches the
        // Settings slider bounds.
        // (A showcase lens pin outranks it: ipc::effective_fov.)
        state.camera.fov_degrees = crate::engine::ipc::effective_fov(
            state.fov_pin,
            state.gui_state.settings.fov,
        );

        // Mouse sensitivity + invert Y (v0.909: the
        // invert toggle used to be decorative).
        state.controller.mouse_sensitivity = state.gui_state.settings.mouse_sensitivity;
        state.controller.invert_y = state.gui_state.settings.invert_y;
        // Crouch: held or toggled (2026-11-09).
        state.controller.set_crouch_toggle(state.gui_state.settings.crouch_toggle);

        // The walking pace (2026-11-09): Realistic or Simplified speeds
        // from data/game.csv. This block also runs on the boot frame.
        crate::engine::walk_pace::apply(state);

        // VSync (v0.909: used to be decorative).
        gpu!(state).renderer.set_vsync(state.gui_state.settings.vsync);

        // Window presentation mode (v0.454).
        // Background instances (HUMANITY_NO_FOCUS) must NOT
        // apply the window mode: WindowedFullscreen's
        // set_maximized(true) goes through ShowWindow on
        // Windows, which ACTIVATES the window. This ran on
        // the boot-frame settings_dirty ~1s after launch,
        // so every rig boot stole focus a second in, even
        // after the v0.1069 visible+inactive creation fix
        // (measured: foreground flipped at t+1s, this line).
        // The flag clears on a genuine click-in (v0.1020),
        // so a deliberately-focused instance applies modes
        // normally from then on.
        // Nor a rig's low-impact window (engine::low_impact): it
        // stays the small window it was made, whoever clicks in.
        if !state.background_no_cursor && crate::engine::low_impact::window().is_none() {
            apply_window_mode_once(&gpu!(state).window, state.gui_state.settings.window_mode);
        }

        // Render distance → camera far plane
        state.camera.far = state.gui_state.settings.render_distance;

        // Persist settings to config file, on the save writer's thread: the save flushes to the
        // disk, which held the boot frame 394 ms beside other agents' compiles (config.rs
        // `save_order`, 2026-11-21). The exit waits for it (engine/quit.rs).
        crate::config::AppConfig::from_gui_state(&state.gui_state).save_soon();
    }
}
