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
                }

                NetMessage::PlayerJoined { player_id, name, position } => {
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
                    // Update remote player's target for interpolation
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
                            break;
                        }
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

        // Throttle outbound position updates
        self.send_timer += dt;
    }
}

/// Smooth step interpolation (ease in-out).
fn smooth_step(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}
