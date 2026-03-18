//! Core ECS components shared across all game systems.

use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// 3D transform: position, rotation, scale.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

/// Linear and angular velocity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Velocity {
    pub linear: Vec3,
    pub angular: Vec3,
}

/// Health pool with current and max values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Health {
    pub current: f32,
    pub max: f32,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            current: 100.0,
            max: 100.0,
        }
    }
}

/// Human-readable name for any entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Name(pub String);
