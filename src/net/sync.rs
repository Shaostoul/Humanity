//! ECS synchronization system: applies network state to the local world.
//!
//! Handles remote player interpolation, entity spawn/despawn from server,
//! and sends local player position updates at a throttled rate.

use crate::ecs::components::Transform;
use crate::ecs::systems::System;
use crate::hot_reload::data_store::DataStore;
use glam::{Quat, Vec3};
use super::protocol::NetMessage;

/// Component marking an entity as a remote player (not locally controlled).
pub struct RemotePlayer {
    pub player_id: u32,
    pub name: String,
    pub last_position: Vec3,
    pub target_position: Vec3,
    pub last_rotation: Quat,
    pub target_rotation: Quat,
    pub velocity: Vec3,
    pub interpolation_t: f32,
    pub last_update_time: f64,
}

/// Component marking an entity as a relay-driven crew NPC (v0.663).
/// Spawned/moved by `NetMessage::NpcUpdate` (the relay's `game_npc_update`
/// chore-AI broadcasts). `name` + `activity` carry everything a nameplate
/// needs ("Botanist Yara -- Inspecting the hydroponic racks"); a future
/// nameplate pass should read them from this component (see the machine-label
/// pattern in src/gui/pages/hud.rs for the world_to_screen text path).
/// Where a crew NPC STANDS on this client (floor 0 + 1.0 m body center),
/// regardless of the relay-side deck height its chore site reports.
const NPC_LOCAL_STANDING_Y: f32 = 1.0;

pub struct RemoteNpc {
    pub entity_id: u64,
    pub name: String,
    /// Human-readable current chore label from data/npc/chores.ron.
    pub activity: String,
    /// True while the NPC dwells at its chore site ("working" state).
    pub working: bool,
    pub last_position: Vec3,
    pub target_position: Vec3,
    pub last_rotation: Quat,
    pub target_rotation: Quat,
    pub interpolation_t: f32,
}

/// Network synchronization system.
pub struct NetSyncSystem {
    /// Time since last position send (throttle to 20/sec = 50ms interval).
    send_timer: f32,
    /// Last sent position (avoid sending if unchanged).
    last_sent_position: Vec3,
    /// Local player ID assigned by server.
    local_player_id: Option<u32>,
    /// Pending messages to process (filled by the engine loop from NetClient::poll).
    pending_messages: Vec<NetMessage>,
}

impl NetSyncSystem {
    pub fn new() -> Self {
        Self {
            send_timer: 0.0,
            last_sent_position: Vec3::ZERO,
            local_player_id: None,
            pending_messages: Vec::new(),
        }
    }

    /// Queue messages for processing on next tick.
    /// Called by the engine loop after NetClient::poll().
    pub fn queue_messages(&mut self, messages: Vec<NetMessage>) {
        self.pending_messages.extend(messages);
    }

    /// Set the local player ID (from Welcome message).
    pub fn set_player_id(&mut self, id: u32) {
        self.local_player_id = Some(id);
    }
}

impl System for NetSyncSystem {
    fn name(&self) -> &str {
        "NetSync"
    }

    fn tick(&mut self, world: &mut hecs::World, dt: f32, _data: &DataStore) {
        let messages: Vec<NetMessage> = self.pending_messages.drain(..).collect();

        for msg in messages {
            match msg {
                NetMessage::Welcome { player_id, .. } => {
                    self.local_player_id = Some(player_id);
                    log::info!("Connected as player {}", player_id);
                    // The relay broadcasts our own join + position back to us; if a remote-player
                    // entity was spawned for our own id before the Welcome arrived (a race), drop it.
                    let mut me = Vec::new();
                    for (entity, remote) in world.query_mut::<&RemotePlayer>() {
                        if remote.player_id == player_id {
                            me.push(entity);
                        }
                    }
                    for entity in me {
                        let _ = world.despawn(entity);
                    }
                }

                NetMessage::PlayerJoined { player_id, name, position } => {
                    // Never spawn a remote avatar for ourselves (the relay echoes our join).
                    if self.local_player_id == Some(player_id) {
                        continue;
                    }
                    // Idempotent for SPAWNING, but a duplicate join still carries the
                    // real display name -- and that matters (v0.796): position updates
                    // can arrive before the welcome snapshot / joined broadcast (they
                    // stream at 15 Hz while the join is one message), so the player
                    // lazy-spawns below as a "Player N" placeholder and the real name
                    // then bounced off this exists-check FOREVER. Found in the
                    // two-instance co-presence proof: the roster stayed on the
                    // placeholder instead of the game_join player_name.
                    let mut exists = false;
                    for (_e, r) in world.query_mut::<&mut RemotePlayer>() {
                        if r.player_id == player_id {
                            exists = true;
                            if !name.is_empty() && r.name != name {
                                r.name = name.clone();
                            }
                            break;
                        }
                    }
                    if exists {
                        continue;
                    }
                    let pos = Vec3::from_array(position);
                    // Spawn a remote player entity
                    world.spawn((
                        Transform {
                            position: pos,
                            rotation: Quat::IDENTITY,
                            scale: Vec3::ONE,
                        },
                        RemotePlayer {
                            player_id,
                            name: name.clone(),
                            last_position: pos,
                            target_position: pos,
                            last_rotation: Quat::IDENTITY,
                            target_rotation: Quat::IDENTITY,
                            velocity: Vec3::ZERO,
                            interpolation_t: 1.0,
                            last_update_time: 0.0,
                        },
                    ));
                    log::info!("Player {} ({}) joined", name, player_id);
                }

                NetMessage::PlayerLeft { player_id } => {
                    // Find and despawn the remote player entity
                    let mut to_despawn = Vec::new();
                    for (entity, remote) in world.query_mut::<&RemotePlayer>() {
                        if remote.player_id == player_id {
                            to_despawn.push(entity);
                        }
                    }
                    for entity in to_despawn {
                        let _ = world.despawn(entity);
                    }
                    log::info!("Player {} left", player_id);
                }

                NetMessage::PositionUpdate {
                    player_id,
                    position,
                    rotation,
                    velocity,
                    ..
                } => {
                    // Never track ourselves (the relay echoes our own updates back).
                    if self.local_player_id == Some(player_id) {
                        continue;
                    }
                    // Update the matching remote player's target for interpolation.
                    let mut found = false;
                    for (_entity, (transform, remote)) in
                        world.query_mut::<(&mut Transform, &mut RemotePlayer)>()
                    {
                        if remote.player_id == player_id {
                            remote.last_position = transform.position;
                            remote.target_position = Vec3::from_array(position);
                            remote.last_rotation = transform.rotation;
                            remote.target_rotation = Quat::from_array(rotation);
                            remote.velocity = Vec3::from_array(velocity);
                            remote.interpolation_t = 0.0;
                            found = true;
                            break;
                        }
                    }
                    // Lazy-spawn: a player already in the world when we joined never sent us a
                    // PlayerJoined (it fired before we connected), so their first position update
                    // is where we first learn of them. Spawn them so co-presence is join-order
                    // independent.
                    if !found {
                        let pos = Vec3::from_array(position);
                        world.spawn((
                            Transform { position: pos, rotation: Quat::from_array(rotation), scale: Vec3::ONE },
                            RemotePlayer {
                                player_id,
                                name: format!("Player {player_id}"),
                                last_position: pos,
                                target_position: pos,
                                last_rotation: Quat::from_array(rotation),
                                target_rotation: Quat::from_array(rotation),
                                velocity: Vec3::from_array(velocity),
                                interpolation_t: 1.0,
                                last_update_time: 0.0,
                            },
                        ));
                    }
                }

                NetMessage::NpcUpdate { entity_id, name, position, activity, working } => {
                    // Update-or-spawn the crew NPC. Updates arrive at ~2 Hz
                    // while traveling (plus on every chore state change), so
                    // interpolation below smooths movement between them.
                    //
                    // GROUND the Y to the local floor (v0.681, operator screenshot
                    // 2026-07-03): the relay simulates chores on ITS multi-deck ship
                    // layout, so upper-deck room sites carry high Y values -- but this
                    // client renders the flat homestead, so crew showed up floating
                    // mid-sky. Keep the relay X/Z walk, override Y with the local
                    // standing height until relay/client layout alignment lands
                    // (tracked in PRIORITIES).
                    let pos = Vec3::new(position[0], NPC_LOCAL_STANDING_Y, position[2]);
                    let mut found = false;
                    for (_e, (transform, npc)) in
                        world.query_mut::<(&mut Transform, &mut RemoteNpc)>()
                    {
                        if npc.entity_id == entity_id {
                            // Face the direction of travel (yaw only) when moving.
                            let delta = pos - transform.position;
                            if delta.length_squared() > 0.0001 {
                                npc.last_rotation = transform.rotation;
                                npc.target_rotation = Quat::from_rotation_y(delta.x.atan2(delta.z));
                            }
                            npc.last_position = transform.position;
                            npc.target_position = pos;
                            npc.activity = activity.clone();
                            npc.working = working;
                            npc.interpolation_t = 0.0;
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        world.spawn((
                            Transform { position: pos, rotation: Quat::IDENTITY, scale: Vec3::ONE },
                            RemoteNpc {
                                entity_id,
                                name: name.clone(),
                                activity: activity.clone(),
                                working,
                                last_position: pos,
                                target_position: pos,
                                last_rotation: Quat::IDENTITY,
                                target_rotation: Quat::IDENTITY,
                                interpolation_t: 1.0,
                            },
                        ));
                        log::info!("Crew NPC {} ({}) appeared: {}", name, entity_id, activity);
                    }
                }

                NetMessage::TimeSync { game_time, .. } => {
                    log::debug!("Time sync: game_time={}", game_time);
                }

                _ => {}
            }
        }

        // Interpolate remote player positions
        for (_entity, (transform, remote)) in
            world.query_mut::<(&mut Transform, &mut RemotePlayer)>()
        {
            if remote.interpolation_t < 1.0 {
                remote.interpolation_t += dt * 20.0; // 20 updates/sec = complete in 50ms
                remote.interpolation_t = remote.interpolation_t.min(1.0);
                let t = smooth_step(remote.interpolation_t);
                transform.position = remote.last_position.lerp(remote.target_position, t);
                transform.rotation = remote.last_rotation.slerp(remote.target_rotation, t);
            } else {
                // Dead reckoning: predict based on last velocity
                transform.position += remote.velocity * dt;
            }
        }

        // Interpolate crew NPC positions. Updates arrive at ~2 Hz (see
        // NPC_POSITION_BROADCAST_INTERVAL relay-side), so complete the lerp
        // in 0.5s. No dead reckoning: crew walk slowly and stop at chore
        // sites, so holding the last target beats drifting past it.
        for (_entity, (transform, npc)) in
            world.query_mut::<(&mut Transform, &mut RemoteNpc)>()
        {
            if npc.interpolation_t < 1.0 {
                npc.interpolation_t += dt * 2.0;
                npc.interpolation_t = npc.interpolation_t.min(1.0);
                let t = smooth_step(npc.interpolation_t);
                transform.position = npc.last_position.lerp(npc.target_position, t);
                transform.rotation = npc.last_rotation.slerp(npc.target_rotation, t);
            }
        }

        // Throttle outbound position updates
        self.send_timer += dt;
    }
}

/// Smooth step interpolation (ease in-out).
fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs::systems::System;

    /// The lazy-spawn name race from the two-instance co-presence proof
    /// (v0.796): a position update arriving BEFORE the join/welcome spawns
    /// the player as a "Player N" placeholder; the real name in the later
    /// PlayerJoined must UPDATE it, not bounce off the exists-check.
    #[test]
    fn a_late_join_message_fixes_a_lazy_spawned_placeholder_name() {
        let mut sys = NetSyncSystem::new();
        let mut world = hecs::World::new();
        let data = crate::hot_reload::data_store::DataStore::new();
        sys.queue_messages(vec![
            NetMessage::Welcome { player_id: 1, world_snapshot: Vec::new() },
            // 15 Hz position stream outruns the one-shot join broadcast.
            NetMessage::PositionUpdate {
                player_id: 42,
                position: [1.0, 0.0, 2.0],
                rotation: [0.0, 0.0, 0.0, 1.0],
                velocity: [0.0, 0.0, 0.0],
                timestamp: 0.0,
            },
        ]);
        sys.tick(&mut world, 0.016, &data);
        let placeholder: Vec<String> = world
            .query_mut::<&RemotePlayer>()
            .into_iter()
            .map(|(_, r)| r.name.clone())
            .collect();
        assert_eq!(placeholder, vec!["Player 42".to_string()], "lazy-spawn placeholder");

        sys.queue_messages(vec![NetMessage::PlayerJoined {
            player_id: 42,
            name: "Test Pilot A".to_string(),
            position: [1.0, 0.0, 2.0],
        }]);
        sys.tick(&mut world, 0.016, &data);
        let named: Vec<(String, u32)> = world
            .query_mut::<&RemotePlayer>()
            .into_iter()
            .map(|(_, r)| (r.name.clone(), r.player_id))
            .collect();
        assert_eq!(named.len(), 1, "no duplicate spawn from the late join");
        assert_eq!(named[0].0, "Test Pilot A", "the real name replaced the placeholder");
    }
}
