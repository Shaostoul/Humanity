use std::time::Instant;
use crate::engine::state::EngineState;
use crate::renderer::mesh::Mesh;
use crate::terrain::planet::PlanetDef;

/// Route a `__game__:`-tagged relay message into the multiplayer sync system (v0.472).
/// `payload` is the JSON AFTER the `__game__:` prefix. Maps the relay's `game_*` wire types
/// (game_welcome / game_player_joined / game_position_update / game_player_left) to NetMessage
/// and queues them for `net_sync` to apply. Other game_* events (quests, perception) are
/// ignored here -- they are not part of co-presence. Reuses the authenticated chat socket.
pub(crate) fn route_game_message(state: &mut EngineState, payload: &str) {
    use crate::net::protocol::NetMessage;
    let Ok(v) = serde_json::from_str::<serde_json::Value>(payload) else { return; };
    let arr3 = |val: &serde_json::Value| -> Option<[f32; 3]> {
        let a = val.as_array()?;
        if a.len() != 3 { return None; }
        Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32])
    };
    let arr4 = |val: &serde_json::Value| -> Option<[f32; 4]> {
        let a = val.as_array()?;
        if a.len() != 4 { return None; }
        Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32, a[3].as_f64()? as f32])
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("game_welcome") => {
            if let Some(id) = v.get("player_id").and_then(|x| x.as_u64()) {
                let own_id = id as u32;
                // Welcome first (sets our local_player_id so the self-filter +
                // idempotency in NetSyncSystem work for the entries below).
                let mut msgs = vec![NetMessage::Welcome {
                    player_id: own_id,
                    world_snapshot: Vec::new(),
                }];
                // World-snapshot prefill (v0.474): the relay's welcome carries
                // every current entity. Spawn the OTHER players right away so a
                // joiner sees players who are already present even if they never
                // move (previously they only appeared on their next position
                // update -- two stationary players were invisible to each other).
                if let Some(snap) = v.get("world_snapshot").and_then(|s| s.as_array()) {
                    for e in snap {
                        let Some(eid) = e.get("entity_id").and_then(|x| x.as_u64()) else { continue; };
                        let Some(pos) = e.get("position").and_then(&arr3) else { continue; };
                        let etype = e.get("entity_type").and_then(|t| t.as_str()).unwrap_or("");
                        if etype == "player" {
                            if eid as u32 == own_id {
                                continue; // skip ourselves
                            }
                            // Real name from the entity's `name` component (v0.774,
                            // relay stamps it at join); "Player" only if an older
                            // snapshot lacks it.
                            let name = e
                                .get("components")
                                .and_then(|c| c.get("name"))
                                .and_then(|n| n.as_str())
                                .unwrap_or("Player")
                                .to_string();
                            msgs.push(NetMessage::PlayerJoined {
                                player_id: eid as u32,
                                name,
                                position: pos,
                            });
                            continue;
                        }
                        // Crew NPC dialogue capture (v0.797): any snapshot entity
                        // carrying dialog[]/greetings[] components is a talkable
                        // crew member. Forward the lines to net_sync as an
                        // NpcProfile so the RemoteNpc spawns with them -- the
                        // walk-up talk card only DISPLAYS relay-authored text
                        // (which the relay builds from its NPC data), never its
                        // own. This also makes dwelling crew visible to a fresh
                        // joiner (they send no NpcUpdate until their next move).
                        let Some(c) = e.get("components") else { continue; };
                        let strings = |key: &str| -> Vec<String> {
                            c.get(key)
                                .and_then(|x| x.as_array())
                                .map(|a| {
                                    a.iter()
                                        .filter_map(|s| s.as_str().map(str::to_string))
                                        .collect()
                                })
                                .unwrap_or_default()
                        };
                        let dialog = strings("dialog");
                        let greetings = strings("greetings");
                        if dialog.is_empty() && greetings.is_empty() {
                            continue; // not a talkable NPC (equipment, windows, ...)
                        }
                        msgs.push(NetMessage::NpcProfile {
                            entity_id: eid,
                            name: c
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or("Crew")
                                .to_string(),
                            role: c
                                .get("role")
                                .and_then(|r| r.as_str())
                                .unwrap_or("")
                                .to_string(),
                            position: pos,
                            activity: c
                                .get("activity")
                                .and_then(|a| a.as_str())
                                .unwrap_or("")
                                .to_string(),
                            dialog,
                            greetings,
                        });
                    }
                }
                state.net_sync.queue_messages(msgs);
            }
        }
        Some("game_player_joined") => {
            if let (Some(id), Some(pos)) = (
                v.get("player_id").and_then(|x| x.as_u64()),
                v.get("position").and_then(&arr3),
            ) {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Player").to_string();
                state.net_sync.queue_messages(vec![NetMessage::PlayerJoined {
                    player_id: id as u32,
                    name,
                    position: pos,
                }]);
            }
        }
        Some("game_position_update") => {
            if let (Some(id), Some(pos)) = (
                v.get("player_id").and_then(|x| x.as_u64()),
                v.get("position").and_then(&arr3),
            ) {
                let rotation = v.get("rotation").and_then(&arr4).unwrap_or([0.0, 0.0, 0.0, 1.0]);
                let velocity = v.get("velocity").and_then(&arr3).unwrap_or([0.0, 0.0, 0.0]);
                let timestamp = v.get("timestamp").and_then(|x| x.as_f64()).unwrap_or(0.0);
                state.net_sync.queue_messages(vec![NetMessage::PositionUpdate {
                    player_id: id as u32,
                    position: pos,
                    rotation,
                    velocity,
                    timestamp,
                }]);
            }
        }
        Some("game_player_left") => {
            if let Some(id) = v.get("player_id").and_then(|x| x.as_u64()) {
                state.net_sync.queue_messages(vec![NetMessage::PlayerLeft { player_id: id as u32 }]);
            }
        }
        // Crew chore AI (v0.663): a relay-side crew NPC moved or changed
        // chores. net_sync spawns/moves RemoteNpc entities the render pass
        // draws; `chore_label` rides along for the future nameplate pass.
        Some("game_npc_update") => {
            if let (Some(id), Some(pos)) = (
                v.get("entity_id").and_then(|x| x.as_u64()),
                v.get("position").and_then(&arr3),
            ) {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Crew").to_string();
                let activity = v.get("chore_label").and_then(|x| x.as_str()).unwrap_or("").to_string();
                let working = v.get("chore_state").and_then(|x| x.as_str()) == Some("working");
                state.net_sync.queue_messages(vec![NetMessage::NpcUpdate {
                    entity_id: id,
                    name,
                    position: pos,
                    activity,
                    working,
                }]);
            }
        }
        // Game admin (v0.474): the relay's private reply to a
        // game_banned_list_request. Admin-only by construction (targeted at
        // the requesting admin). Populates the Game Admin page list.
        Some("game_banned_list") => {
            if let Some(arr) = v.get("users") {
                if let Ok(bans) = serde_json::from_value::<Vec<crate::relay::storage::GameBan>>(arr.clone()) {
                    state.gui_state.game_bans = bans;
                }
            }
        }
        // The relay refused our own join (we are game-banned). Surface it; do
        // NOT touch chat (it stays connected by design).
        Some("game_join_denied") => {
            let reason = v.get("reason").and_then(|x| x.as_str()).unwrap_or("");
            let msg = v.get("message").and_then(|x| x.as_str())
                .unwrap_or("You are banned from the game world. Chat is unaffected.");
            state.gui_state.game_admin_status = if reason.is_empty() {
                msg.to_string()
            } else {
                format!("{msg} ({reason})")
            };
            log::warn!("Game-join denied: {msg} (reason: {reason})");
        }
        Some("game_admin_error") => {
            if let Some(m) = v.get("message").and_then(|x| x.as_str()) {
                state.gui_state.game_admin_status = m.to_string();
            }
        }
        _ => {}
    }
}

/// Send the local player's position to the relay (reused chat socket). Throttled by the caller.
/// The relay validates (anti-teleport) and broadcasts `game_position_update` to other clients.
pub(crate) fn send_game_position(state: &EngineState) {
    let Some(ref ws) = state.gui_state.ws_client else { return; };
    let p = state.camera.position;
    // Yaw-only facing quaternion (rotation about Y): enough for avatars to face their heading.
    let half = state.camera.yaw * 0.5;
    let (qy, qw) = (half.sin(), half.cos());
    let msg = serde_json::json!({
        "type": "game_position_update",
        "position": [p.x, p.y, p.z],
        "rotation": [0.0, qy, 0.0, qw],
        "velocity": [0.0, 0.0, 0.0],
        "timestamp": 0.0,
    });
    ws.send(&msg.to_string());
}

/// Lazy-load the 3D world: homestead, hologram, stars, planet, CSV data.
/// Called once on first Enter World. Keeps app startup instant (chat-first).
/// (Re)load every data/planets/<body_id>.ron into state.planet_defs and
/// drop the cached surface meshes + atmosphere materials so the next
/// frame regenerates from the fresh values. Called at world load and by
/// the hot-reload poll when a planet RON changes on disk (v0.764) - so
/// palette/noise/sea-level tuning shows in the sky without a relaunch.
/// The superseded GPU meshes stay resident until session end (bounded by
/// how many edits a tuning session makes; eviction is a noted follow-up).
pub(crate) fn reload_planet_defs(state: &mut EngineState) {
    state.planet_defs.clear();
    state.planet_heightmaps.clear();
    state.planet_albedos.clear();
    state.planet_mesh_cache.clear();
    state.planet_atmo_materials.clear();
    state.planet_cloud_materials.clear();
    state.planet_water_materials.clear();
    // Chunked-LOD patches: unlike the whole-sphere cache above (whose
    // superseded meshes stay resident until session end), patch slots
    // are actively recycled: replace each GPU mesh with a degenerate
    // placeholder and hand the slot to the free list, so a tuning
    // session that hot-reloads earth.ron rebuilds patches from the new
    // values without leaking hundreds of MB.
    for (_, cs) in state.planet_chunk_states.drain() {
        for (_, entry) in cs.cache {
            state
                .renderer
                .replace_mesh(entry.mesh, Mesh::placeholder(&state.renderer.device));
            state.planet_patch_free_slots.push(entry.mesh);
        }
    }
    for b in crate::cosmos::sol_bodies() {
        let rel = format!("planets/{}.ron", b.id);
        if state.asset_manager.data_dir().join(&rel).exists() {
            match state.asset_manager.load_ron::<PlanetDef>(&rel) {
                Ok(def) => {
                    let mut def = def.clone();
                    // Sort + sanitize the optional gravity curve once here
                    // so gravity_at can assume ascending finite points even
                    // when the RON was just hand-edited mid-session.
                    def.normalize_gravity_curve();
                    // Real-elevation grid (Earth: NOAA ETOPO1 via
                    // scripts/build-earth-heightmap.js). On success the
                    // RON's hand-tuned sea_level is OVERRIDDEN with the
                    // grid's true 0 m position so the real coastline is
                    // exact; on failure we warn and keep the noise path
                    // (a missing grid must never blank a planet).
                    if let Some(hm_rel) = def.heightmap.clone() {
                        let hm_path = state.asset_manager.data_dir().join(&hm_rel);
                        match crate::terrain::planet_heightmap::PlanetHeightmap::load(&hm_path) {
                            Ok(hm) => {
                                def.sea_level = hm.sea_level_normalized();
                                log::info!(
                                    "Planet '{}': heightmap {} ({}x{}, {:.0}..{:.0} m, sea at {:.3})",
                                    b.id, hm_rel, hm.width(), hm.height(),
                                    hm.min_meters(), hm.max_meters(),
                                    def.sea_level
                                );
                                state.planet_heightmaps.insert(b.id.clone(), std::sync::Arc::new(hm));
                            }
                            Err(e) => log::warn!(
                                "Planet '{}': heightmap {hm_rel} failed to load ({e}); falling back to procedural noise",
                                b.id
                            ),
                        }
                    } else {
                        // No measured grid shipped (Moon/Mars/Pluto/mods):
                        // synthesize one (v0.919) so the chunked-LOD
                        // ground + ground clamp + albedo texture bake all
                        // activate — without this the body renders as the
                        // bare uniform icosphere, kilometer-wide flat
                        // facets at walking height (the operator's
                        // "icosphere stepping"). RON sea_level is NOT
                        // overridden here: for an airless body it is a
                        // color-band threshold (the Moon's maria line),
                        // not a coastline.
                        let t0 = Instant::now();
                        let hm =
                            crate::terrain::procedural_heightmap::synthesize(&def);
                        log::info!(
                            "Planet '{}': synthesized heightmap ({}x{}, {:.0}..{:.0} m) in {:.0?}",
                            b.id, hm.width(), hm.height(),
                            hm.min_meters(), hm.max_meters(), t0.elapsed()
                        );
                        state.planet_heightmaps.insert(b.id.clone(), std::sync::Arc::new(hm));
                    }
                    // Real surface-color grid (Earth: NASA Blue Marble
                    // via scripts/build-earth-albedo.js). On failure we
                    // warn and keep the elevation-band classifier (a
                    // missing grid must never blank a planet's colors).
                    if let Some(al_rel) = def.albedo.clone() {
                        let al_path = state.asset_manager.data_dir().join(&al_rel);
                        match crate::terrain::planet_albedo::PlanetAlbedo::load(&al_path) {
                            Ok(al) => {
                                log::info!(
                                    "Planet '{}': albedo {} ({}x{})",
                                    b.id, al_rel, al.width(), al.height()
                                );
                                state.planet_albedos.insert(b.id.clone(), std::sync::Arc::new(al));
                            }
                            Err(e) => log::warn!(
                                "Planet '{}': albedo {al_rel} failed to load ({e}); falling back to band classifier",
                                b.id
                            ),
                        }
                    }
                    // Per-pixel surface texture (v0.811): when BOTH real
                    // grids loaded, bake the imagery (grading applied per
                    // texel; the water/land split needs the elevation
                    // grid) and upload it on a per-planet textured
                    // material. Hot-reload swaps the texture on the
                    // EXISTING material index so repeated RON tuning
                    // never piles 32 MB textures up in VRAM.
                    if let (Some(hm), Some(al)) = (
                        state.planet_heightmaps.get(&b.id),
                        state.planet_albedos.get(&b.id),
                    ) {
                        let t0 = Instant::now();
                        let rgba =
                            crate::terrain::planet_surface::bake_albedo_rgba(&def, hm, al);
                        if let Some(&mi) = state.planet_textured_materials.get(&b.id) {
                            state.renderer.set_material_albedo_texture(
                                mi,
                                &rgba,
                                al.width(),
                                al.height(),
                            );
                        } else {
                            let mi = state.renderer.add_textured_material(
                                // base_color.xyz is overwritten every
                                // frame with the planet center in render
                                // space (see planet_textured_materials).
                                [1.0, 1.0, 1.0, 1.0],
                                0.0,
                                0.9,
                                12.0,
                                // params.w = the type-12 bit field (NOT
                                // emissive): bit 0 = albedo texture
                                // present. The sky loop rewrites it
                                // every frame with the Surface-detail
                                // bit ORed in (v0.816), so 1.0 here
                                // only covers the first frame.
                                1.0,
                                &rgba,
                                al.width(),
                                al.height(),
                            );
                            state.planet_textured_materials.insert(b.id.clone(), mi);
                        }
                        log::info!(
                            "Planet '{}': per-pixel surface texture baked + uploaded ({}x{}, {} ms)",
                            b.id,
                            al.width(),
                            al.height(),
                            t0.elapsed().as_millis()
                        );
                    }
                    state.planet_defs.insert(b.id.clone(), def);
                }
                Err(e) => log::warn!("Could not load planet def {rel}: {e}"),
            }
        }
    }
    // A def that LOST a grid on reload (operator removed the field) must
    // stop drawing through its stale texture; the orphaned material slot
    // stays resident (same accepted pattern as planet_mesh_cache).
    state
        .planet_textured_materials
        .retain(|id, _| {
            state.planet_albedos.contains_key(id) && state.planet_heightmaps.contains_key(id)
        });
    log::info!(
        "Planets: {} procedural surface def(s) loaded from data/planets/ ({} with real heightmaps, {} with real albedo, {} with baked per-pixel textures)",
        state.planet_defs.len(),
        state.planet_heightmaps.len(),
        state.planet_albedos.len(),
        state.planet_textured_materials.len()
    );
}

/// Chat history fetch + drain, one call per frame (extracted from the
/// lib.rs frame loop, 2026-08-13, alongside the freeze fix it carries).
///
/// The fetch runs on a BACKGROUND thread with short timeouts. The old
/// inline ureq call had no timeout and ran on the render thread; with the
/// relay unreachable, the OS connect timeout froze the whole app ~21 s per
/// reconnect cycle (the operator's "app froze while watching chat" report,
/// log-proven). The is_connected gate is honest now (ws_client::LinkState),
/// so this does not even spawn while the relay is dark.
pub(crate) fn chat_history_pump(state: &mut EngineState) {
    if !state.gui_state.history_fetched
        && state.gui_state.ws_client.as_ref().map_or(false, |c| c.is_connected())
        && !state.gui_state.server_url.is_empty()
        && state.gui_state.history_rx.is_none()
    {
        state.gui_state.history_fetched = true;
        let base_url = state.gui_state.server_url.trim_end_matches('/').to_string();
        let channel = state.gui_state.chat_active_channel.clone();
        let api_url = format!("{}/api/messages?limit=50&channel={}", base_url, channel);
        let (tx, rx) = std::sync::mpsc::channel();
        state.gui_state.history_rx = Some(rx);
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout_connect(std::time::Duration::from_secs(4))
                .timeout(std::time::Duration::from_secs(8))
                .build();
            let result = match agent.get(&api_url).call() {
                Ok(resp) => resp.into_string().map_err(|e| e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            let _ = tx.send((channel, result));
        });
    }
    // Drain the background fetch (non-blocking; at most one in flight
    // thanks to the history_rx.is_none() gate above).
    let drained = match state.gui_state.history_rx.as_ref().map(|rx| rx.try_recv()) {
        Some(Ok(pair)) => {
            state.gui_state.history_rx = None;
            Some(pair)
        }
        Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
            state.gui_state.history_rx = None;
            None
        }
        _ => None,
    };
    if let Some((channel, result)) = drained {
        match result {
            Ok(body) => {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(messages) = data.get("messages").and_then(|v| v.as_array()) {
                        let my_key = state.gui_state.profile_public_key.clone();
                        let mut fetched = 0usize;
                        let mut skipped = 0usize;
                        for msg in messages {
                            // Federated rows serialize with different field names
                            // (server_id/server_name/from_name) and must rebuild
                            // EXACTLY like the live federated_chat arm in lib.rs,
                            // or the same line renders two ways depending on
                            // whether it arrived live or via history refetch.
                            let is_federated = msg.get("type").and_then(|v| v.as_str())
                                == Some("federated_chat");
                            let origin_server = if is_federated {
                                msg.get("server_id").and_then(|v| v.as_str()).unwrap_or("").to_string()
                            } else {
                                String::new()
                            };
                            let sender_name = if is_federated {
                                let from = msg.get("from_name").and_then(|v| v.as_str()).unwrap_or("Anonymous");
                                let sname = msg.get("server_name").and_then(|v| v.as_str()).unwrap_or("federated");
                                format!("{} ({})", from, sname)
                            } else {
                                msg.get("sender_name")
                                    .or_else(|| msg.get("from_name"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("Anonymous")
                                    .to_string()
                            };
                            let sender_key = if is_federated {
                                origin_server.clone()
                            } else {
                                msg.get("sender_key")
                                    .or_else(|| msg.get("from"))
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string()
                            };
                            let content = msg.get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let timestamp = msg.get("timestamp")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            let ch = msg.get("channel")
                                .and_then(|v| v.as_str())
                                .unwrap_or("general")
                                .to_string();
                            // Dedup: if this is a message WE sent that we already
                            // local-echoed, skip the server's copy (BUG-035 part 2).
                            // Match logic mirrors the WS broadcast dedup in lib.rs.
                            if !my_key.is_empty()
                                && sender_key == my_key
                                && state.gui_state.chat_sent_timestamps.contains(&timestamp)
                            {
                                state.gui_state.chat_sent_timestamps.retain(|&t| t != timestamp);
                                skipped += 1;
                                continue;
                            }
                            // Robust content dedup (2026-05-20 fix): this fetch runs
                            // on EVERY reconnect (history_fetched resets on
                            // disconnect), so without checking the existing buffer
                            // it would re-append every message already on screen
                            // from the live broadcast. (sender_key, timestamp_ms)
                            // uniquely identifies a normal message; federated
                            // lines share sender_key = origin server id, so
                            // content joins the key there to avoid collapsing
                            // two same-millisecond lines from one origin.
                            if state.gui_state.chat_messages.iter()
                                .any(|m| m.sender_key == sender_key
                                    && m.timestamp_ms == timestamp
                                    && (!is_federated || m.content == content))
                            {
                                skipped += 1;
                                continue;
                            }
                            state.gui_state.chat_messages.push(
                                crate::gui::ChatMessage {
                                    sender_name,
                                    sender_key,
                                    content,
                                    timestamp: crate::gui::pages::chat::format_timestamp(timestamp),
                                    timestamp_ms: timestamp,
                                    channel: ch,
                                    server: crate::gui::pages::chat::norm_server_url(&state.gui_state.server_url),
                                    origin_server,
                                    ..Default::default()
                                },
                            );
                            fetched += 1;
                        }
                        log::info!(
                            "Fetched {} history messages for #{} (skipped {} local-echo dedup)",
                            fetched, channel, skipped
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!("Failed to fetch message history: {}", e);
            }
        }
    }
    // Reset history_fetched when not connected so a NEW connection re-fetches.
    if state.gui_state.ws_client.as_ref().map_or(true, |c| !c.is_connected()) {
        state.gui_state.history_fetched = false;
    }
}
