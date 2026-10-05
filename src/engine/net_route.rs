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
    // The fleet ledger's answers (2026-10-04, engine/fleet.rs).
    if crate::engine::fleet::on_game_message(state, &v) {
        return;
    }
    let arr3 = |val: &serde_json::Value| -> Option<[f32; 3]> {
        let a = val.as_array()?;
        if a.len() != 3 { return None; }
        Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32])
    };
    match v.get("type").and_then(|t| t.as_str()) {
        // The host's clock (operator, 2026-09-29: in a shared world it wins).
        // The relay sends this every 5 s to EVERY socket, chat-only ones
        // included, so it counts only while this player is in the shared world.
        Some("game_time_sync") => {
            if let Some(word) = host_clock_from(&v, state.gui_state.copresence_active && !state.gui_state.copresence_solo) {
                crate::systems::time::hear_host_clock(&state.data_store, word);
            }
        }
        Some("game_welcome") => {
            // Increment 1b: our home goes to the plot the relay gave us (or we refuse a ship
            // that is not ours, and the welcome goes no further). engine/home_plot.rs.
            if !crate::engine::home_plot::apply_welcome_home(state, &v) {
                return;
            }
            // Increment 4: a fresh spawn starts the relay's correction count again.
            crate::engine::move_check::on_welcome(state, &v);
            // The fleet's stores, a fresh power baseline, the ledger (engine/fleet.rs).
            crate::engine::fleet::on_welcome(state, &v);
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
                        msgs.extend(snapshot_entry_messages(e, Some(own_id)));
                    }
                }
                state.net_sync.queue_messages(msgs);
            }
        }
        // Increment 4 (src/relay/handlers/game_interest.rs): another player or a crew member came
        // into our view, sent whole as a welcome lists it (nothing about it reached us while it
        // was out of view, and a crew member at work sends no moves); or went out of it, and is
        // taken off our screen instead of standing frozen where it was last seen.
        Some("game_in_view") | Some("game_out_of_view") => state.net_sync.queue_messages(view_change_messages(&v)),
        // The relay put us back where it holds us: a move faster than anyone can go (increment 4).
        Some("game_position_correction") => crate::engine::move_check::apply_correction(state, &v),
        Some("game_player_joined") => {
            if let (Some(id), Some(pos)) = (
                v.get("player_id").and_then(|x| x.as_u64()),
                v.get("position").and_then(&arr3),
            ) {
                let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("Player").to_string();
                let look = v.get("appearance").and_then(crate::player_look::PlayerLook::from_json);
                state.net_sync.queue_messages(vec![NetMessage::PlayerJoined {
                    player_id: id as u32,
                    name,
                    position: pos,
                    look,
                }]);
            }
        }
        Some("game_position_update") => {
            // Only while joined (see `position_update_from`).
            if let Some(update) = position_update_from(&v, state.game_joined) {
                state.net_sync.queue_messages(vec![update]);
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
            // The relay's ship is not ours, it has none, or our join named none (increment 1b,
            // round 4): it refused the join before spawning anything, so there is nothing to
            // leave. Or our account on it was erased and it took our figure out (round 5), so
            // there is nothing to leave either. One plain sentence per cause, kept under the
            // HUD, and no retry on this server until a fresh connection to it, a switch away
            // and back, or a fresh world load (engine/home_plot.rs `join_denied_sentence`).
            if let Some(sentence) = crate::engine::home_plot::join_denied_sentence(reason) {
                // An erased account's plot went back with it: forget it (increment 2).
                if reason == "account_erased" {
                    let server = crate::engine::home_plot::active_server_key(&state.gui_state);
                    crate::engine::home_plot::remember_plot(state, &server, None);
                }
                // Another ship: say so plainly when it is our data folder's copy that is old
                // (the review of increment 4, P6).
                let sentence = if reason == "other_ship" {
                    crate::engine::home_plot::other_ship_sentence_here(&state.data_dir, v.get("ship").and_then(|s| s.get("hash")).and_then(|h| h.as_str()))
                } else {
                    sentence.to_string()
                };
                crate::engine::home_plot::refuse_shared_world(state, sentence, None);
                return;
            }
            let msg = v.get("message").and_then(|x| x.as_str())
                .unwrap_or("You are banned from the game world. Chat is unaffected.");
            state.gui_state.game_admin_status = if reason.is_empty() {
                msg.to_string()
            } else {
                format!("{msg} ({reason})")
            };
            log::warn!("Game-join denied: {msg} (reason: {reason})");
        }
        // A refusal, or (game_admin_notice) a done-and-said, from a game-admin action such as
        // releasing a player's plot: both are shown under the admin controls.
        Some("game_admin_error") | Some("game_admin_notice") => {
            if let Some(m) = v.get("message").and_then(|x| x.as_str()) {
                state.gui_state.game_admin_status = m.to_string();
            }
        }
        _ => {}
    }
}

/// What a `game_in_view` or `game_out_of_view` (increment 4, src/relay/handlers/game_interest.rs)
/// asks of the sync. In view: the mover sent whole, as a welcome lists it (`snapshot_entry_messages`),
/// and for a crew member its place and work now (its profile does not move one we still draw).
/// Out of view: taken off our screen (`EntityDespawn`). Nothing for anything else.
pub(crate) fn view_change_messages(v: &serde_json::Value) -> Vec<crate::net::protocol::NetMessage> {
    use crate::net::protocol::NetMessage;
    let arr3 = |val: &serde_json::Value| -> Option<[f32; 3]> {
        let a = val.as_array()?;
        if a.len() != 3 { return None; }
        Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32])
    };
    match v.get("type").and_then(|t| t.as_str()) {
        Some("game_in_view") => {
            let Some(e) = v.get("entity") else { return Vec::new() };
            let mut msgs = snapshot_entry_messages(e, None);
            let c = e.get("components");
            if let (Some(eid), Some(pos), true) = (e.get("entity_id").and_then(|x| x.as_u64()), e.get("position").and_then(arr3), c.is_some_and(|c| c.get("chore_agent").is_some())) {
                let chore = c.and_then(|c| c.get("chore"));
                msgs.push(NetMessage::NpcUpdate {
                    entity_id: eid,
                    name: c.and_then(|c| c.get("name")).and_then(|n| n.as_str()).unwrap_or("Crew").to_string(),
                    position: pos,
                    activity: chore.and_then(|ch| ch.get("label")).and_then(|l| l.as_str()).unwrap_or("").to_string(),
                    working: chore.and_then(|ch| ch.get("state")).and_then(|s| s.as_str()) == Some("working"),
                });
            }
            msgs
        }
        Some("game_out_of_view") => v.get("entity_id").and_then(|x| x.as_u64()).map(|id| vec![NetMessage::EntityDespawn { entity_id: id }]).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// What one entity of a welcome's world snapshot (or a `game_in_view`, increment 4) asks of the
/// sync: another player to draw (`PlayerJoined`, with their name and look), or a talkable crew
/// member (`NpcProfile`, with the lines the relay wrote for it); nothing for ourselves
/// (`own_id`) or for things that are neither (equipment, windows).
pub(crate) fn snapshot_entry_messages(e: &serde_json::Value, own_id: Option<u32>) -> Vec<crate::net::protocol::NetMessage> {
    use crate::net::protocol::NetMessage;
    let mut msgs = Vec::new();
    let arr3 = |val: &serde_json::Value| -> Option<[f32; 3]> {
        let a = val.as_array()?;
        if a.len() != 3 { return None; }
        Some([a[0].as_f64()? as f32, a[1].as_f64()? as f32, a[2].as_f64()? as f32])
    };
    let Some(eid) = e.get("entity_id").and_then(|x| x.as_u64()) else { return msgs; };
    let Some(pos) = e.get("position").and_then(arr3) else { return msgs; };
    let etype = e.get("entity_type").and_then(|t| t.as_str()).unwrap_or("");
    if etype == "player" {
        if Some(eid as u32) == own_id {
            return msgs; // skip ourselves
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
        let look = e
            .get("components")
            .and_then(|c| c.get("appearance"))
            .and_then(crate::player_look::PlayerLook::from_json);
        msgs.push(NetMessage::PlayerJoined {
            player_id: eid as u32,
            name,
            position: pos,
            look,
        });
        return msgs;
    }
    // Crew NPC dialogue capture (v0.797): any snapshot entity
    // carrying dialog[]/greetings[] components is a talkable
    // crew member. Forward the lines to net_sync as an
    // NpcProfile so the RemoteNpc spawns with them -- the
    // walk-up talk card only DISPLAYS relay-authored text
    // (which the relay builds from its NPC data), never its
    // own. This also makes dwelling crew visible to a fresh
    // joiner (they send no NpcUpdate until their next move).
    let Some(c) = e.get("components") else { return msgs; };
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
        return msgs; // not a talkable NPC (equipment, windows, ...)
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
    msgs
}

/// The host's game clock and its speed from a `game_time_sync` message, when
/// this player is in the shared world; None otherwise, or when the message has
/// no clock. The speed is the server's Shared world clock setting (real time,
/// 1x, by default, 2026-10-04); a word without one is a host that runs at 1x
/// (`HOST_TIME_SPEED`), and a speed out of range is held to the Time
/// setting's (`clamp_time_speed`).
pub(crate) fn host_clock_from(v: &serde_json::Value, joined: bool) -> Option<crate::systems::time::HostClock> {
    use crate::systems::time;
    if !joined {
        return None;
    }
    let game_time = v.get("game_time").and_then(|x| x.as_f64()).filter(|t| t.is_finite() && *t >= 0.0)?;
    let time_scale = v.get("time_scale").and_then(|x| x.as_f64()).map_or(time::HOST_TIME_SPEED, |s| time::clamp_time_speed(s as f32));
    Some(time::HostClock { game_time, time_scale })
}

/// Another player's position from a `game_position_update` message, ready
/// to queue for `net_sync`, when this player is in the shared world; None
/// otherwise, or when the message has no player or position.
///
/// Only while joined (2026-10-02, round two): the relay sends every player's
/// updates to every connected socket, chat-only ones included, and net_sync
/// only ticks once we have joined. Before this, a client sitting on a menu
/// with chat connected queued every update for as long as it sat there, and
/// the first tick after joining took the whole backlog in at once, all
/// stamped with the same arrival time, which broke the timing of everyone's
/// figure for the rest of the session.
///
/// `timestamp` is passed through as sent: the sender's own steady clock in
/// seconds, or 0 (also for a missing field) from an older client without one.
pub(crate) fn position_update_from(v: &serde_json::Value, joined: bool) -> Option<crate::net::protocol::NetMessage> {
    if !joined {
        return None;
    }
    let floats = |key: &str, n: usize| -> Option<Vec<f32>> {
        let a = v.get(key)?.as_array()?;
        if a.len() != n {
            return None;
        }
        a.iter().map(|x| x.as_f64().map(|f| f as f32)).collect()
    };
    let player_id = v.get("player_id").and_then(|x| x.as_u64())? as u32;
    let p = floats("position", 3)?;
    let rotation = floats("rotation", 4).map_or([0.0, 0.0, 0.0, 1.0], |r| [r[0], r[1], r[2], r[3]]);
    let velocity = floats("velocity", 3).map_or([0.0, 0.0, 0.0], |u| [u[0], u[1], u[2]]);
    let timestamp = v.get("timestamp").and_then(|x| x.as_f64()).unwrap_or(0.0);
    Some(crate::net::protocol::NetMessage::PositionUpdate {
        player_id,
        position: [p[0], p[1], p[2]],
        rotation,
        velocity,
        timestamp,
    })
}

/// The names that float over people in the world (2026-09-28): each crew
/// member (`RemoteNpc`, with their chore under the name) and each other
/// player (`RemotePlayer`, name only). The relay has always sent a player's
/// name and the sync kept it, but only the crew were labelled, so another
/// player was a teal figure with no name. Every name goes just over the top
/// of the figure drawn for that person: `remote_figure_parts` for a player
/// (2026-10-02: it was a fixed 0.3 m over the eye, which a tall player's head
/// now reaches past), `crew_figure_parts` for a crew member (2026-10-03: it
/// was a fixed 1.0 m over their position).
/// `station_off` is the same offset the scene pass puts on home content.
pub(crate) fn nameplate_labels(world: &hecs::World, station_off: glam::Vec3) -> Vec<crate::gui::CrewLabel> {
    use crate::ecs::components::Transform;
    use crate::net::sync::{RemoteNpc, RemotePlayer};
    let mut labels = Vec::new();
    for (_e, (t, npc)) in world.query::<(&Transform, &RemoteNpc)>().iter() {
        let top = figure_top(&crew_figure_parts(t.position, t.rotation));
        labels.push(crate::gui::CrewLabel {
            pos: glam::Vec3::new(t.position.x, top + PLAYER_NAMEPLATE_OVER_HEAD_M, t.position.z) + station_off,
            name: npc.name.clone(),
            activity: npc.activity.clone(),
            working: npc.working,
        });
    }
    for (_e, (t, player)) in world.query::<(&Transform, &RemotePlayer)>().iter() {
        labels.push(crate::gui::CrewLabel {
            pos: glam::Vec3::new(
                t.position.x,
                remote_figure_top(t.position, player.look.as_ref()) + PLAYER_NAMEPLATE_OVER_HEAD_M,
                t.position.z,
            ) + station_off,
            name: player.name.clone(),
            activity: String::new(),
            working: false,
        });
    }
    labels
}

/// A part of another player's figure (2026-09-29, appearance sync rung 2),
/// and of a crew member's (2026-10-03).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FigurePart {
    /// The body: teal for a player (the marker that says "a player"), amber
    /// for a crew member.
    Body,
    /// The head, in their skin tone when their look is known.
    Head,
    /// Hair over the top and back of the head, in their hair colour (only
    /// with a look). Drawn with the head mesh, stretched and moved back.
    Hair,
}

/// One drawn piece of a figure: which part, where the mesh's own origin goes
/// (the body box's base, the head mesh's centre), which way the piece turns,
/// and its scale. `lib.rs` hands these straight to the renderer, which builds
/// the model matrix from scale, rotation and position in that order
/// (renderer/scene_draw.rs), so the scale is along the figure's own axes:
/// x across the shoulders, y up, z from the back of the head to the face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FigurePiece {
    pub part: FigurePart,
    pub at: glam::Vec3,
    pub rotation: glam::Quat,
    pub scale: glam::Vec3,
}

/// The meshes a figure is drawn with, metres: `lib.rs` builds the body box
/// (base at its origin) and the head mesh (`figure_head_mesh_data`, centred
/// on its origin) FROM these, and `figure_parts` places the parts by them, so
/// the two cannot drift.
pub(crate) const FIGURE_BODY_MESH_H_M: f32 = 1.4;
pub(crate) const FIGURE_HEAD_MESH_R_M: f32 = 0.17;
/// Shoulder height over the feet at look height 1.0, m: where the body box
/// ends and the head starts. A person whose eye is 1.7 m up has shoulders near
/// 1.47 m; the sphere head (0.34 m across) on top then holds that eye.
const FIGURE_SHOULDER_M: f32 = 1.47;
/// The hair is the head mesh stretched to these fractions of the head
/// (across, up, back to front)...
const HAIR_SCALE: [f32; 3] = [1.04, 1.06, 1.08];
/// ...with its centre moved this many head radii up (y) and back (z, the face
/// being +z). Together: hair from a fringe about 0.6 of the radius above the
/// head's centre at the front, down the sides past the eye line's ends, and
/// over the whole back of the head; the face in between, eye line included
/// (the eye sits 0.35 of the radius above the centre), stays skin. Chosen by
/// casting rays at the two shapes from the front, the back, the side and
/// above; the test `another_players_hair_leaves_their_face_showing` casts
/// them again at the real meshes.
const HAIR_CENTRE_IN_HEAD_RADII: [f32; 3] = [0.0, 0.05, -0.12];
/// A crew member's position is their standing height, this far over their
/// feet: net/sync.rs grounds every crew member to `NPC_LOCAL_STANDING_Y`
/// (1.0) over the home floor at y = 0, and the relay's chore sites are a
/// room's floor + 1.0 too (relay/handlers/game_state.rs `chore_site_of`, at
/// ship_world.rs `STANDING_Y_M`).
const CREW_POSITION_OVER_FEET_M: f32 = 1.0;

/// Where each piece of a figure goes, built UP from its `feet`: the body from
/// the feet to the shoulders, the head on top of the body, the hair over the
/// top and back of the head, every size scaled by the height (a look's
/// `height`, 1.0 without one). `face` turns the figure: it takes the figure's
/// own +z (its face) to the way the person faces.
pub(crate) fn figure_parts(feet: glam::Vec3, face: glam::Quat, look: Option<&crate::player_look::PlayerLook>) -> Vec<FigurePiece> {
    use glam::Vec3;
    let h = look.map_or(1.0, |l| l.height);
    let shoulder = FIGURE_SHOULDER_M * h;
    let head_r = FIGURE_HEAD_MESH_R_M * h;
    let head = feet + Vec3::new(0.0, shoulder + head_r, 0.0);
    let piece = |part, at, scale| FigurePiece { part, at, rotation: face, scale };
    let mut parts = vec![
        piece(FigurePart::Body, feet, Vec3::new(h, shoulder / FIGURE_BODY_MESH_H_M, h)),
        piece(FigurePart::Head, head, Vec3::splat(h)),
    ];
    if look.is_some() {
        // The hair's centre is moved in the figure's own frame, so "back"
        // turns with the figure.
        let offset = face * (Vec3::from(HAIR_CENTRE_IN_HEAD_RADII) * head_r);
        parts.push(piece(FigurePart::Hair, head + offset, Vec3::from(HAIR_SCALE) * h));
    }
    parts
}

/// The way another player's figure faces, from the rotation their client
/// sends. That rotation is not the turn that takes a figure's face to where
/// they look: it is their camera's yaw written as a half angle,
/// [0, sin(yaw/2), 0, cos(yaw/2)] (`position_update_json`), and a camera at
/// that yaw looks along (sin yaw, 0, -cos yaw) (renderer/camera.rs
/// `forward_xz`), which a plain turn by yaw about +y does not give for the
/// face (+z) or the back (-z) at every yaw. Drawn as sent, a figure walking
/// diagonally was turned 90 degrees off its path and, once it had a front
/// and a back, faced backwards at yaw 0. So: read the yaw back, and turn the
/// face onto the camera's forward.
pub(crate) fn player_face(sent: glam::Quat) -> glam::Quat {
    let yaw = 2.0 * sent.y.atan2(sent.w);
    glam::Quat::from_rotation_y(std::f32::consts::PI - yaw)
}

/// Where each piece of another player's figure goes.
///
/// `eye` is the position their client sends, their camera, which stands
/// `surface_walk::EYE_HEIGHT_M` (1.7 m, the camera's standing eye height
/// too) over their feet whatever their look. So the feet are put there, on
/// the floor. `sent_rotation` is the rotation their client sends (see
/// `player_face`).
///
/// Rebuilt 2026-10-02: the parts used to hang from the eye with offsets that
/// ignored the meshes' real sizes, so the 1.4 m body box ran 0.55 m ABOVE the
/// eye, the skin-tone head and the hair sat inside it, and the feet floated
/// 0.85 m over the floor.
pub(crate) fn remote_figure_parts(eye: glam::Vec3, sent_rotation: glam::Quat, look: Option<&crate::player_look::PlayerLook>) -> Vec<FigurePiece> {
    let feet = eye - glam::Vec3::new(0.0, crate::surface_walk::EYE_HEIGHT_M as f32, 0.0);
    figure_parts(feet, player_face(sent_rotation), look)
}

/// Where each piece of a crew member's figure goes (2026-10-03): the same
/// figure as a player's without a look, standing on the floor. `rotation` is
/// their transform's, a true turn of +z onto their path (net/sync.rs faces
/// them along it with `from_rotation_y(dx.atan2(dz))`).
///
/// Before this the crew were drawn with their own fixed offsets, the 1.4 m
/// body box from 0.3 m under their position and the head 0.55 m over it: the
/// body ran from 0.7 m to 2.1 m over the floor, so the feet floated and the
/// head sat inside the body, the fault BUG-120 fixed for players.
pub(crate) fn crew_figure_parts(position: glam::Vec3, rotation: glam::Quat) -> Vec<FigurePiece> {
    figure_parts(position - glam::Vec3::new(0.0, CREW_POSITION_OVER_FEET_M, 0.0), rotation, None)
}

/// The highest point of a drawn figure (the top of the hair, or of the head
/// without a look), world Y. Names float over it.
fn figure_top(parts: &[FigurePiece]) -> f32 {
    // Every turn here is about the vertical, so a piece's y scale stays
    // vertical and its top is its centre plus the head radius scaled.
    parts
        .iter()
        .filter(|p| p.part != FigurePart::Body)
        .map(|p| p.at.y + FIGURE_HEAD_MESH_R_M * p.scale.y)
        .fold(f32::MIN, f32::max)
}

/// The top of another player's figure, world Y.
pub(crate) fn remote_figure_top(eye: glam::Vec3, look: Option<&crate::player_look::PlayerLook>) -> f32 {
    figure_top(&remote_figure_parts(eye, glam::Quat::IDENTITY, look))
}

/// The head mesh every figure's head and hair are drawn with (2026-10-03): a
/// UV sphere of `radius` centred on its origin, wound counter-clockwise seen
/// from outside, which is the side the opaque pipeline draws
/// (renderer/pipeline.rs: `FrontFace::Ccw`, back faces culled). It is the
/// engine's own sphere (`Mesh::sphere_data`) at 16 x 24, fine enough that
/// the hair (only 4 to 8% bigger than the head) does not let the head's flat
/// facets poke through it at the sides.
///
/// For a few hours this was its own copy of the sphere loop, because
/// `Mesh::sphere` was wound the other way, every triangle of it (BUG-127):
/// the pipeline culled each sphere's near half and drew its far half from
/// inside. A lone sphere still looks round that way, but put the hair over
/// the head and, wherever the hair runs inside the head, its far inside is
/// nearer than the head's: the dark band across another player's face (the
/// 2026-10-03 co-presence screenshot), with the hair's crown hidden behind
/// the head's far inside. BUG-128 turned `Mesh::sphere` itself the right way
/// out, so the copy went; the test below still checks every triangle.
pub(crate) fn figure_head_mesh_data(radius: f32) -> (Vec<crate::renderer::mesh::Vertex>, Vec<u32>) {
    const STACKS: u32 = 16;
    const SLICES: u32 = 24;
    crate::renderer::mesh::Mesh::sphere_data(radius, STACKS, SLICES)
}

/// The material cache key for a look's colours: each channel in 64 steps, so
/// players with the same look share materials and a cache never grows with
/// every small difference.
pub(crate) fn look_material_key(look: &crate::player_look::PlayerLook) -> [u8; 6] {
    let q = |c: f32| (c.clamp(0.0, 1.0) * 63.0).round() as u8;
    [q(look.skin[0]), q(look.skin[1]), q(look.skin[2]), q(look.hair[0]), q(look.hair[1]), q(look.hair[2])]
}

/// How far over the top of a remote player's figure, or a crew member's,
/// their name floats, m.
const PLAYER_NAMEPLATE_OVER_HEAD_M: f32 = 0.15;

/// Our own position for the other players, once a frame while joined to the
/// shared world (lib.rs calls it with that frame's time step, the one our
/// movement used). `net_sync`'s `PositionSender` decides what goes out: an
/// update 15 times a second while we are in the world view, and one last
/// standing-still update on the frame we leave it for a page or the
/// showroom (2026-10-02, round two: before that, our figure walked on
/// 1.25 m on everyone else's screen and stood there until we came back).
pub(crate) fn drive_position_send(state: &mut EngineState, in_world: bool, dt: f32, real_dt: f32) {
    // Nothing goes out before the welcome has put our home on our plot (increment 1b): until
    // then our camera stands at the default plot, which may be someone else's home.
    if !state.game_welcomed {
        return;
    }
    // The rig's scripted walk (the showcase `walk_to` verb), and where the body stands while the
    // follow cam watches a vehicle (ship homes increment 4, engine/move_check.rs).
    crate::engine::move_check::walk_tick(state, dt);
    let position = crate::engine::move_check::body_position(state);
    let yaw = state.camera.yaw;
    let out = state.net_sync.position_to_send(dt, real_dt, in_world, position, yaw, &mut state.game_pos_timer);
    if let Some(out) = out {
        send_game_position(state, &out);
    }
}

/// Send one position update to the relay (reused chat socket). The relay checks it against how
/// fast anyone can go (src/relay/handlers/move_check.rs: a fast move this game made for real is
/// declared in `moved`, and `correction` says which correction it last stood at) and passes
/// it on to the players who have us in view.
pub(crate) fn send_game_position(state: &mut EngineState, out: &crate::net::sync::OutgoingPosition) {
    let mut msg = position_update_json(out);
    crate::engine::move_check::stamp(state, &mut msg);
    let Some(ref ws) = state.gui_state.ws_client else { return; };
    ws.send(&msg.to_string());
}

/// The `game_position_update` message for one of our updates.
pub(crate) fn position_update_json(out: &crate::net::sync::OutgoingPosition) -> serde_json::Value {
    let p = out.position;
    // Yaw-only facing quaternion (rotation about Y): enough for avatars to face their heading.
    let half = out.yaw * 0.5;
    let (qy, qw) = (half.sin(), half.cos());
    // Our REAL velocity (2026-10-02; every update used to say zero): how far
    // we moved since the previous update over the time since then, metres
    // per second, zero after a pause or a teleport-sized jump and in the
    // standing update on leaving the world view. The other players' screens
    // keep our figure walking along it when one of these messages is late.
    let v = out.velocity;
    serde_json::json!({
        "type": "game_position_update",
        "position": [p.x, p.y, p.z],
        "rotation": [0.0, qy, 0.0, qw],
        "velocity": [v.x, v.y, v.z],
        // Our own steady clock, seconds (2026-10-02, round two): it advances
        // by the same time step our movement uses, so the receivers place
        // each update at the moment it really held and draw our figure at
        // our real speed however unevenly the updates go out or arrive. The
        // relay forwards it untouched. 0 would mean "no clock" (an older
        // client), so the clock never reads 0 (src/net/sync.rs PositionSender).
        "timestamp": out.timestamp,
    })
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
        // A Commons view fetches the underlying room's history -- but only
        // when the ACTIVE server actually carries the bridged room. A
        // non-carrier's same-named local channel is a different room and
        // must not be pulled into the merged view (carrier history comes
        // from the background fetch in engine/bg_connections.rs instead).
        let channel = match crate::gui::pages::chat::commons_room_of(
            &state.gui_state.chat_active_channel,
        ) {
            Some(room) => {
                let carries = state
                    .gui_state
                    .chat_channels
                    .iter()
                    .any(|c| c.id == room && c.federated);
                if !carries {
                    return; // history_fetched stays true; nothing to fetch here
                }
                room.to_string()
            }
            None => state.gui_state.chat_active_channel.clone(),
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A PLAYER COMES INTO VIEW, GOES OUT OF IT AND COMES BACK (the review of increment 4, R6):
    /// the relay sends a mover that comes into our view whole (`game_in_view`) and takes one
    /// that leaves it off our screen (`game_out_of_view`). Neither was tested, and on the shipped
    /// ship everyone aboard sees everyone, so no rig run ever sends either. In view: drawn, under
    /// its name, where the relay says; out: gone; in view again: drawn again. A crew member the
    /// same way, at its place and work.
    ///
    /// Seen red 2026-10-04 with the out-of-view arm of `view_change_messages` answering nothing:
    /// "out of view: taken off the screen" (left: [(5, "Ada")], right: []).
    #[test]
    fn a_player_comes_into_view_goes_out_and_comes_back() {
        use crate::ecs::systems::System;
        use crate::net::sync::{NetSyncSystem, RemoteNpc, RemotePlayer};
        let data = crate::hot_reload::data_store::DataStore::new();
        let mut sys = NetSyncSystem::new();
        let mut world = hecs::World::new();
        let hear = |sys: &mut NetSyncSystem, world: &mut hecs::World, v: serde_json::Value| {
            sys.queue_messages(view_change_messages(&v));
            sys.tick(world, 0.016, &data);
        };
        let player_in = serde_json::json!({ "type": "game_in_view", "entity": { "entity_id": 5, "entity_type": "player", "position": [60.0, 1.7, 40.0], "rotation": [0.0, 0.0, 0.0, 1.0], "components": { "name": "Ada" } } });
        let player_out = serde_json::json!({ "type": "game_out_of_view", "entity_id": 5 });
        let crew_in = serde_json::json!({ "type": "game_in_view", "entity": { "entity_id": 9, "entity_type": "npc", "position": [80.0, 1.7, 30.0], "rotation": [0.0, 0.0, 0.0, 1.0], "components": { "name": "Cook Ana", "role": "cook", "dialog": ["Soup's on."], "chore_agent": true, "chore": { "label": "Stirring the pot", "state": "working" } } } });
        let crew_out = serde_json::json!({ "type": "game_out_of_view", "entity_id": 9 });
        let players = |w: &mut hecs::World| -> Vec<(u32, String)> { w.query_mut::<&RemotePlayer>().into_iter().map(|(_, r)| (r.player_id, r.name.clone())).collect() };
        let crew = |w: &mut hecs::World| -> Vec<(u64, String, bool)> { w.query_mut::<&RemoteNpc>().into_iter().map(|(_, n)| (n.entity_id, n.activity.clone(), n.working)).collect() };
        hear(&mut sys, &mut world, player_in.clone());
        assert_eq!(players(&mut world), vec![(5, "Ada".to_string())], "in view: drawn under its name");
        hear(&mut sys, &mut world, player_out);
        assert_eq!(players(&mut world), Vec::new(), "out of view: taken off the screen");
        hear(&mut sys, &mut world, player_in);
        assert_eq!(players(&mut world), vec![(5, "Ada".to_string())], "in view again: drawn again");
        hear(&mut sys, &mut world, crew_in.clone());
        assert_eq!(crew(&mut world), vec![(9, "Stirring the pot".to_string(), true)], "a crew member in view, at its work");
        hear(&mut sys, &mut world, crew_out);
        assert_eq!(crew(&mut world), Vec::new(), "a crew member out of view");
        hear(&mut sys, &mut world, crew_in);
        assert_eq!(crew(&mut world).len(), 1, "and back");
        assert_eq!(players(&mut world).len(), 1, "the player stayed drawn");
    }

    /// ANOTHER PLAYER'S FIGURE WEARS THEIR LOOK (2026-09-29, appearance sync
    /// rung 2). Without a look: body and head, no hair. With one: a hair cap
    /// over the head, and a taller player's figure scaled up. Red check, run:
    /// ignoring the height fails the body's length assertion.
    ///
    /// AND IT STANDS ON THE FLOOR, HEAD ON TOP (2026-10-02). The body box's
    /// bottom is at the feet (the eye less the camera's standing eye height),
    /// the head is wholly above the body box's top, and the hair cap's top is
    /// over the head's. Red check, run against the old eye-hung offsets: the
    /// feet assertion failed (the body's bottom 0.85 m over the floor); with
    /// the feet and shoulder assertions skipped, the head-above-body one failed
    /// (the head's bottom 0.67 m under the body's top); with the whole loop
    /// skipped, the hair one failed (the old cap's top 8 mm under the head's).
    #[test]
    fn another_players_figure_wears_their_look() {
        use glam::Vec3;
        // The figure's feet assume the camera rests this high over the floor.
        let cam = crate::renderer::camera::CameraController::new(1.0, 1.0);
        assert_eq!(cam.eye_height(), crate::surface_walk::EYE_HEIGHT_M as f32, "the camera's standing eye height");
        let eye = Vec3::new(0.0, 1.7 + 3.0, 0.0); // a player standing on a floor at y = 3
        let floor = 3.0;
        let plain = remote_figure_parts(eye, glam::Quat::IDENTITY, None);
        assert_eq!(plain.iter().map(|p| p.part).collect::<Vec<_>>(), vec![FigurePart::Body, FigurePart::Head]);
        let tall = crate::player_look::PlayerLook { skin: [0.6, 0.4, 0.3], hair: [0.1, 0.05, 0.02], height: 1.2 };
        for (look, h) in [(None, 1.0), (Some(&tall), 1.2)] {
            let parts = remote_figure_parts(eye, glam::Quat::IDENTITY, look);
            let get = |part| *parts.iter().find(|p| p.part == part).unwrap();
            let body = get(FigurePart::Body);
            let head = get(FigurePart::Head);
            let body_top = body.at.y + FIGURE_BODY_MESH_H_M * body.scale.y;
            let head_r = FIGURE_HEAD_MESH_R_M * head.scale.y;
            assert!((body.at.y - floor).abs() < 1e-4, "height {h}: the body's bottom is at the feet: {body:?}");
            assert!((body_top - floor - 1.47 * h).abs() < 1e-3, "height {h}: the body ends at the shoulders: {body_top}");
            assert!(head.at.y - head_r >= body_top - 1e-4, "height {h}: the head is above the body: {head:?}, body top {body_top}");
            assert!((head.scale.x - h).abs() < 1e-5, "height {h}: the head is sized by the height");
        }
        let parts = remote_figure_parts(eye, glam::Quat::IDENTITY, Some(&tall));
        let hair = parts.iter().find(|p| p.part == FigurePart::Hair).expect("a hair cap");
        let head = parts.iter().find(|p| p.part == FigurePart::Head).unwrap();
        let (hair_top, head_top) = (hair.at.y + FIGURE_HEAD_MESH_R_M * hair.scale.y, head.at.y + FIGURE_HEAD_MESH_R_M * head.scale.y);
        assert!(hair_top > head_top, "the hair covers the top of the head: {hair_top} over {head_top}");
        assert!(FIGURE_HEAD_MESH_R_M * hair.scale.x > FIGURE_HEAD_MESH_R_M * head.scale.x, "and is wider than it");
        assert!((remote_figure_top(eye, Some(&tall)) - hair_top).abs() < 1e-5, "the figure's top is the hair's");
        // Two players with the same colours share a cache key.
        assert_eq!(look_material_key(&tall), look_material_key(&crate::player_look::PlayerLook { height: 0.9, ..tall }));
    }

    /// What a ray sees first among a figure's head and hair, drawn the way the
    /// renderer draws them: each piece's mesh (`figure_head_mesh_data`, the
    /// mesh lib.rs builds) placed by the model matrix the renderer builds
    /// (scale, rotation, translation: renderer/scene_draw.rs), keeping only
    /// the triangles the opaque pipeline keeps, those wound counter-clockwise
    /// toward the viewer (renderer/pipeline.rs: `FrontFace::Ccw`, back faces
    /// culled). The body box is left out: these rays are all at head height.
    fn first_seen(
        pieces: &[FigurePiece],
        mesh: &(Vec<crate::renderer::mesh::Vertex>, Vec<u32>),
        from: glam::Vec3,
        dir: glam::Vec3,
    ) -> Option<FigurePart> {
        let mut best: Option<(f32, FigurePart)> = None;
        for p in pieces.iter().filter(|p| p.part != FigurePart::Body) {
            let m = glam::Mat4::from_scale_rotation_translation(p.scale, p.rotation, p.at);
            let world = |i: u32| m.transform_point3(glam::Vec3::from(mesh.0[i as usize].position));
            for t in mesh.1.chunks(3) {
                let (a, b, c) = (world(t[0]), world(t[1]), world(t[2]));
                let (e1, e2) = (b - a, c - a);
                // Counter-clockwise toward the viewer: the winding's normal
                // points back up the ray. Others are culled (and the pole
                // rows' zero-area triangles drop out here too).
                if e1.cross(e2).dot(dir) >= 0.0 {
                    continue;
                }
                // Moller-Trumbore ray / triangle.
                let pv = dir.cross(e2);
                let det = e1.dot(pv);
                if det.abs() < 1e-12 {
                    continue;
                }
                let tv = from - a;
                let u = tv.dot(pv) / det;
                let qv = tv.cross(e1);
                let v = dir.dot(qv) / det;
                if u < 0.0 || v < 0.0 || u + v > 1.0 {
                    continue;
                }
                let dist = e2.dot(qv) / det;
                if dist > 0.0 && best.map_or(true, |(d, _)| dist < d) {
                    best = Some((dist, p.part));
                }
            }
        }
        best.map(|b| b.1)
    }

    /// ANOTHER PLAYER'S HAIR LEAVES THEIR FACE SHOWING (2026-10-03). The
    /// co-presence screenshot of 2026-10-03 showed a dark band straight across
    /// the face of the other player's head and the head's own skin where the
    /// hair should be. Two causes: the head mesh (`Mesh::sphere`) was wound
    /// inside out, so each sphere showed its far inside and the hair, wherever
    /// it ran inside the head, came out in front of it; and the hair was a
    /// flattened sphere centred over the crown, whose edge came down to 0.25 of
    /// the radius over the head's centre, under the eye line (0.35), all round.
    ///
    /// So this looks at the figure the way the renderer draws it
    /// (`first_seen`): from in front, across the face from 0.4 of the radius
    /// under the head's centre to just over the eye line, every ray meets the
    /// head (skin; the hair is 4% wider than the head, so its edge shows as a
    /// thin outline down the sides of the face, about 1 px at 6 m); at the top of the
    /// forehead a fringe of hair; from behind, from the nape up, all hair; from
    /// above, hair. The facing comes the way it travels, our camera's yaw sent
    /// and read back, so the figure's face has to turn to where that camera
    /// looks; several yaws and two heights.
    ///
    /// Red checks, run 2026-10-03, each restored byte for byte after:
    /// (1) the hair and the winding as they shipped (the centred flattened
    /// cap, scale 1.06 x 0.6 x 1.06 at 0.45 of the radius up, and
    /// `Mesh::sphere`'s index order) FAILS with "yaw 0, height 1: the face at
    /// (-0.37, 0) radii shows skin, not hair" (left: Some(Hair), right:
    /// Some(Head)): the band across the face. (2) The old cap with this mesh's
    /// winding FAILS with "yaw 0, height 1: the face at (-0.37, 0.35) radii
    /// shows skin": the cap over the eye line. (3) This hair with the
    /// inside-out winding FAILS with "yaw 0, height 1: a fringe of hair over
    /// the forehead" (left: Some(Head)): the crown hidden behind the head's
    /// far inside. (4) Drawing the figure turned by the sent rotation as it
    /// is (no `player_face`) FAILS with "yaw 0, height 1: the face at (-0.37,
    /// -0.4) radii shows skin": the figure faced away, its back hair to us.
    /// The last check (every triangle faces out) is not reached by (1) or
    /// (3), which fail on what the renderer would show first.
    #[test]
    fn another_players_hair_leaves_their_face_showing() {
        use crate::net::protocol::NetMessage;
        use crate::net::sync::OutgoingPosition;
        use glam::{Quat, Vec3};
        let mesh = figure_head_mesh_data(FIGURE_HEAD_MESH_R_M);
        for yaw in [0.0f32, 0.7, 2.0, -2.6] {
            // Our camera's yaw, as it goes out and as the other client reads it.
            let out = OutgoingPosition { position: Vec3::new(4.0, 1.7, -3.0), yaw, velocity: Vec3::ZERO, timestamp: 1.0 };
            let mut v = position_update_json(&out);
            v["player_id"] = serde_json::json!(9);
            let Some(NetMessage::PositionUpdate { position, rotation, .. }) = position_update_from(&v, true) else {
                panic!("the update reads back");
            };
            // Where that camera looks, and its right hand.
            let mut cam = crate::renderer::camera::Camera::new();
            cam.yaw = yaw;
            let fwd = cam.forward_xz();
            let right = fwd.cross(Vec3::Y);
            for height in [1.0f32, 1.2] {
                let look = crate::player_look::PlayerLook { skin: [0.72, 0.53, 0.42], hair: [0.12, 0.08, 0.05], height };
                let pieces = remote_figure_parts(Vec3::from(position), Quat::from_array(rotation), Some(&look));
                let head = pieces.iter().find(|p| p.part == FigurePart::Head).unwrap().at;
                let r = FIGURE_HEAD_MESH_R_M * height;
                // A point on the head's vertical plane, in head radii.
                let at = |dx: f32, dy: f32| head + right * (dx * r) + Vec3::Y * (dy * r);
                for dy in [-0.4f32, -0.2, 0.0, 0.17, 0.35, 0.45] {
                    for dx in [-0.37f32, 0.03, 0.41] {
                        assert_eq!(
                            first_seen(&pieces, &mesh, at(dx, dy) + fwd * 2.0, -fwd),
                            Some(FigurePart::Head),
                            "yaw {yaw}, height {height}: the face at ({dx}, {dy}) radii shows skin, not hair"
                        );
                    }
                }
                assert_eq!(
                    first_seen(&pieces, &mesh, at(0.03, 0.85) + fwd * 2.0, -fwd),
                    Some(FigurePart::Hair),
                    "yaw {yaw}, height {height}: a fringe of hair over the forehead"
                );
                for dy in [-0.3f32, 0.0, 0.35, 0.6] {
                    for dx in [-0.37f32, 0.03, 0.41] {
                        assert_eq!(
                            first_seen(&pieces, &mesh, at(dx, dy) - fwd * 2.0, fwd),
                            Some(FigurePart::Hair),
                            "yaw {yaw}, height {height}: the back of the head at ({dx}, {dy}) radii is hair"
                        );
                    }
                }
                for (dx, dz) in [(0.03f32, 0.0f32), (0.2, -0.4), (-0.3, 0.3)] {
                    let over = head + right * (dx * r) + fwd * (dz * r) + Vec3::Y * 2.0;
                    assert_eq!(
                        first_seen(&pieces, &mesh, over, -Vec3::Y),
                        Some(FigurePart::Hair),
                        "yaw {yaw}, height {height}: the crown at ({dx}, {dz}) radii is hair"
                    );
                }
            }
        }
        // And the mesh itself: every triangle faces out, counter-clockwise
        // seen from outside, the side the opaque pipeline draws. The pole
        // rows' triangles have no area (two corners on the pole; f32's
        // sin(PI) is -8.7e-8, not 0, so their sign is noise) and are skipped.
        for t in mesh.1.chunks(3) {
            let p = |i: u32| Vec3::from(mesh.0[i as usize].position);
            let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
            let n = (b - a).cross(c - a);
            if n.length() < 1e-8 {
                continue;
            }
            assert!(n.dot(a + b + c) > 0.0, "every triangle of the head mesh faces out: {t:?}");
        }
    }

    /// CREW STAND ON THE FLOOR WITH THEIR HEADS ON THEIR SHOULDERS
    /// (2026-10-03). A crew member's position comes through the same sync the
    /// game runs (net/sync.rs grounds it to standing height over the home
    /// floor at y = 0), and their figure is built from it: the body's bottom on
    /// the floor, the head wholly above the body's top, no hair, and the very
    /// same pieces a player without a look standing on that floor gets.
    ///
    /// Red check, run 2026-10-03, restored byte for byte after:
    /// `crew_figure_parts` returning the old crew offsets (the 1.4 m body box
    /// from 0.3 m under the position, the head 0.55 m over it, both unscaled)
    /// FAILS with "the crew's feet are on the floor: the body's bottom is
    /// 0.7 m up".
    #[test]
    fn crew_figures_stand_on_the_floor_head_on_shoulders() {
        use crate::ecs::components::Transform;
        use crate::ecs::systems::System;
        use crate::net::protocol::NetMessage;
        use crate::net::sync::{NetSyncSystem, RemoteNpc};
        use glam::Vec3;
        let mut sys = NetSyncSystem::new();
        let mut world = hecs::World::new();
        sys.queue_messages(vec![NetMessage::NpcUpdate {
            entity_id: 5,
            name: "Ada".into(),
            position: [3.0, 7.0, -2.0], // the relay's own deck height: replaced by the local one
            activity: "Walking to hydroponics".into(),
            working: false,
        }]);
        sys.tick(&mut world, 0.016, &crate::hot_reload::data_store::DataStore::new());
        let (pos, rot) = world
            .query_mut::<(&Transform, &RemoteNpc)>()
            .into_iter()
            .map(|(_, (t, _))| (t.position, t.rotation))
            .next()
            .expect("the crew member is in the world");
        let floor = 0.0;
        let crew = crew_figure_parts(pos, rot);
        assert_eq!(crew.iter().map(|p| p.part).collect::<Vec<_>>(), vec![FigurePart::Body, FigurePart::Head], "body and head, no hair");
        let body = crew[0];
        let head = crew[1];
        let body_top = body.at.y + FIGURE_BODY_MESH_H_M * body.scale.y;
        let head_r = FIGURE_HEAD_MESH_R_M * head.scale.y;
        assert!((body.at.y - floor).abs() < 1e-4, "the crew's feet are on the floor: the body's bottom is {} m up", body.at.y);
        assert!(
            head.at.y - head_r >= body_top - 1e-4,
            "the head sits on the shoulders, not inside the body: its bottom is {} m, the body's top {body_top} m",
            head.at.y - head_r
        );
        // The same figure a player without a look standing there gets.
        let eye = Vec3::new(pos.x, floor + crate::surface_walk::EYE_HEIGHT_M as f32, pos.z);
        let player = remote_figure_parts(eye, glam::Quat::IDENTITY, None);
        for (c, p) in crew.iter().zip(&player) {
            assert!((c.at - p.at).length() < 1e-4 && (c.scale - p.scale).length() < 1e-5, "the crew's {:?} is a player's: {c:?} vs {p:?}", c.part);
        }
    }

    /// THE HOST'S CLOCK COUNTS ONLY FOR A JOINED PLAYER (2026-09-29). The
    /// relay sends game_time_sync to every socket; a chat-only client must not
    /// take it. Red check, run: ignoring `joined` fails the second assertion.
    #[test]
    fn the_host_clock_counts_only_when_joined() {
        use crate::systems::time::{HostClock, HOST_TIME_SPEED};
        let v = serde_json::json!({"type": "game_time_sync", "game_time": 259200.0, "server_time": 1.0});
        assert_eq!(host_clock_from(&v, true), Some(HostClock { game_time: 259200.0, time_scale: HOST_TIME_SPEED }));
        assert_eq!(host_clock_from(&v, false), None, "chat only: not in the shared world");
        assert_eq!(host_clock_from(&serde_json::json!({"type": "game_time_sync"}), true), None);
    }

    /// THE HOST'S WORD CARRIES ITS SPEED (2026-10-04: the shared world runs
    /// at the speed its admin sets, real time unless they change it; 72x
    /// here, so a speed left unread shows). The relay's `game_time_sync` says
    /// how fast its clock runs, and the game takes it with the clock; a speed
    /// outside the Time setting's range is held to it.
    ///
    /// Seen red 2026-10-04 with `time_scale` left unread: "assertion `left
    /// == right` failed: the host's 72x, left: 1.0, right: 72.0".
    #[test]
    fn the_host_word_carries_its_speed() {
        let v = serde_json::json!({"type": "game_time_sync", "game_time": 600.0, "time_scale": 72.0, "server_time": 1.0});
        let word = host_clock_from(&v, true).expect("a joined player takes the host's word");
        assert_eq!(word.game_time, 600.0);
        assert_eq!(word.time_scale, 72.0, "the host's 72x");
        let fast = serde_json::json!({"type": "game_time_sync", "game_time": 600.0, "time_scale": 1.0e9});
        assert_eq!(host_clock_from(&fast, true).unwrap().time_scale, crate::systems::time::MAX_TIME_SPEED);
    }

    /// OUR POSITION UPDATE CARRIES OUR CLOCK, AND ANOTHER PLAYER'S IS TAKEN
    /// ONLY ONCE JOINED (2026-10-02, round two). The message we send carries
    /// our position, facing, velocity and our own steady clock as
    /// `timestamp` (it used to say 0); reading it back gives exactly those.
    /// While not joined (chat connected, sitting on a menu) another player's
    /// update is dropped instead of queued: the relay sends them to every
    /// socket, and a backlog drained in the first joined tick broke everyone's
    /// figure timing. A missing timestamp reads as 0, "no clock".
    ///
    /// Red checks, run 2026-10-03: (1) queueing whether joined or not (no
    /// `!joined` return) FAILS with "a chat-only client does not queue
    /// another player's position"; (2) sending `"timestamp": 0.0` as before
    /// FAILS with "assertion `left == right` failed: the sender's clock goes
    /// out as the timestamp".
    #[test]
    fn our_position_update_carries_our_clock_and_others_are_taken_only_once_joined() {
        use crate::net::protocol::NetMessage;
        use crate::net::sync::OutgoingPosition;
        use glam::Vec3;
        let out = OutgoingPosition {
            position: Vec3::new(1.0, 1.7, -2.0),
            yaw: 0.5,
            velocity: Vec3::new(5.0, 0.0, 0.0),
            timestamp: 12.345,
        };
        let mut v = position_update_json(&out);
        assert_eq!(v["type"], "game_position_update");
        // The relay adds who sent it before passing it on.
        v["player_id"] = serde_json::json!(42);
        assert!(position_update_from(&v, false).is_none(), "a chat-only client does not queue another player's position");
        match position_update_from(&v, true) {
            Some(NetMessage::PositionUpdate { player_id, position, rotation, velocity, timestamp }) => {
                assert_eq!(player_id, 42);
                assert_eq!(timestamp, 12.345, "the sender's clock goes out as the timestamp");
                assert_eq!(position, [1.0, 1.7, -2.0]);
                assert_eq!(velocity, [5.0, 0.0, 0.0]);
                let yaw = 2.0 * rotation[1].atan2(rotation[3]);
                assert!((yaw - 0.5).abs() < 1e-5, "facing comes back: {rotation:?}");
            }
            other => panic!("a joined client reads it back as a position update, got {other:?}"),
        }
        // An older client's message without a timestamp: "no clock".
        let old = serde_json::json!({"type": "game_position_update", "player_id": 7, "position": [0.0, 0.0, 0.0]});
        match position_update_from(&old, true) {
            Some(NetMessage::PositionUpdate { timestamp, rotation, velocity, .. }) => {
                assert_eq!((timestamp, rotation, velocity), (0.0, [0.0, 0.0, 0.0, 1.0], [0.0, 0.0, 0.0]));
            }
            other => panic!("an older client's update is still read, got {other:?}"),
        }
        assert!(position_update_from(&serde_json::json!({"type": "game_position_update", "player_id": 7}), true).is_none(), "no position, no update");
    }

    /// ANOTHER PLAYER HAS A NAME OVER THEM (2026-09-28). A crew member and a
    /// remote player both get a label: the crew member's with the chore, the
    /// player's with the name alone, just above the head. Red check, run:
    /// labelling only the crew (as before) fails the second assertion.
    #[test]
    fn other_players_get_a_nameplate_like_the_crew() {
        use crate::ecs::components::Transform;
        use crate::net::sync::{RemoteNpc, RemotePlayer};
        use glam::{Quat, Vec3};
        let mut world = hecs::World::new();
        let at = |p: Vec3| Transform { position: p, ..Transform::default() };
        world.spawn((
            at(Vec3::new(1.0, 1.0, 0.0)),
            RemoteNpc {
                entity_id: 3,
                name: "Ada".into(),
                activity: "Watering the beds".into(),
                working: true,
                role: String::new(),
                dialog: Vec::new(),
                greetings: Vec::new(),
                last_position: Vec3::ZERO,
                target_position: Vec3::ZERO,
                last_rotation: Quat::IDENTITY,
                target_rotation: Quat::IDENTITY,
                interpolation_t: 1.0,
            },
        ));
        world.spawn((
            at(Vec3::new(5.0, 1.7, 0.0)),
            RemotePlayer {
                player_id: 7,
                name: "Test Pilot".into(),
                last_position: Vec3::ZERO,
                target_position: Vec3::ZERO,
                last_rotation: Quat::IDENTITY,
                target_rotation: Quat::IDENTITY,
                velocity: Vec3::ZERO,
                interpolation_t: 1.0,
                last_update_time: 0.0,
                look: None,
            },
        ));
        let tallest = crate::player_look::PlayerLook { skin: [0.6, 0.4, 0.3], hair: [0.1, 0.05, 0.02], height: crate::player_look::HEIGHT_MAX };
        world.spawn((
            at(Vec3::new(-5.0, 1.7, 0.0)),
            RemotePlayer {
                player_id: 8,
                name: "Tall Pilot".into(),
                last_position: Vec3::ZERO,
                target_position: Vec3::ZERO,
                last_rotation: Quat::IDENTITY,
                target_rotation: Quat::IDENTITY,
                velocity: Vec3::ZERO,
                interpolation_t: 1.0,
                last_update_time: 0.0,
                look: Some(tallest),
            },
        ));
        let labels = nameplate_labels(&world, Vec3::new(0.0, 0.0, 10.0));
        // The tallest look's name clears their hair (2026-10-02). Red check, run:
        // the old fixed eye + 0.3 m put it 0.27 m down inside their head.
        let tall = labels.iter().find(|l| l.name == "Tall Pilot").expect("the tall player is labelled");
        let tall_top = remote_figure_top(Vec3::new(-5.0, 1.7, 0.0), Some(&tallest));
        assert!(tall.pos.y > tall_top, "the name clears a tall player's head: {} under {tall_top}", tall.pos.y);
        let crew = labels.iter().find(|l| l.name == "Ada").expect("the crew member is labelled");
        assert_eq!(crew.activity.as_str(), "Watering the beds");
        // Just over the crew figure's top too (2026-10-03: it was position +
        // 1.0, under which the old crew body box ran on to 2.1 m).
        let crew_top = figure_top(&crew_figure_parts(Vec3::new(1.0, 1.0, 0.0), Quat::IDENTITY));
        assert!((crew.pos - Vec3::new(1.0, crew_top + PLAYER_NAMEPLATE_OVER_HEAD_M, 10.0)).length() < 1e-5, "over the crew head: {:?}", crew.pos);
        // And at a person's height, as a literal, so a wrong figure height
        // cannot pass by being computed the same wrong way on both sides: the
        // crew stand with their feet 1.0 m under their position (here the
        // floor is y = 0), the top of a default figure is 1.47 + 2 x 0.17 =
        // 1.81 m, and the name floats 0.15 m over it (review, 2026-10-03).
        // Red check, run 2026-10-03: crew_figure_parts set 0.1 m lower FAILED
        // with "the crew name floats at 1.96 m over the floor, not 1.8599999",
        // while the computed check above still passed.
        assert!((crew.pos.y - 1.96).abs() < 0.01, "the crew name floats at 1.96 m over the floor, not {}", crew.pos.y);
        let player = labels.iter().find(|l| l.name == "Test Pilot").expect("the other player is labelled");
        assert_eq!(player.activity, "", "a player has no chore line");
        // Just over the drawn figure's top (2026-10-02: it was eye + 0.3).
        let top = remote_figure_top(Vec3::new(5.0, 1.7, 0.0), None);
        assert!(top > 1.7, "the head reaches over the eye");
        assert!((player.pos - Vec3::new(5.0, top + PLAYER_NAMEPLATE_OVER_HEAD_M, 10.0)).length() < 1e-5, "just over the head: {:?}", player.pos);
    }
}
