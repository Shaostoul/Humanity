//! Speed and turn rate read from where a body went (plan 8.5: "Speed and direction are not sent:
//! they come from the interpolated positions, which feed the same locomotion solver the local
//! player uses").
//!
//! The same tracker serves every body: the local player (its camera's position and yaw), another
//! player (the position the network sync interpolates 15 updates a second into, and the way their
//! figure faces), a crew member (its interpolated position and the facing it turns as a person
//! turns, BUG-166). Vertical speed is read too since rung 2a (a jump's rise, a fall); whether the
//! body is crouching or off the ground is not motion but stance, and the caller says it.

use glam::Vec3;

use crate::graph::LocomotionInput;

/// One body's motion, smoothed.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MotionTracker {
    last: Option<(Vec3, f32)>,
    input: LocomotionInput,
}

impl MotionTracker {
    /// Read one frame's motion. `facing` is the turn about +Y that takes the body's front (+Z) to
    /// the way it faces, radians. `smoothing_s` and `teleport_mps` come from the graph's data.
    pub fn observe(&mut self, position: Vec3, facing: f32, dt: f32, smoothing_s: f32, teleport_mps: f32) -> LocomotionInput {
        let Some((last_pos, last_facing)) = self.last.replace((position, facing)) else {
            return self.input;
        };
        if !(dt.is_finite() && dt > 1e-5) {
            return self.input;
        }
        let step = Vec3::new(position.x - last_pos.x, 0.0, position.z - last_pos.z);
        let rise = position.y - last_pos.y;
        // Only a sideways jump is a teleport: a body's root steps up a stair tread in one frame
        // (the walk stands it on each tread), which at a high frame rate is tens of metres a second.
        if !step.is_finite() || !rise.is_finite() || step.length() / dt > teleport_mps {
            // A teleport: stand still where it landed.
            self.input = LocomotionInput::default();
            return self.input;
        }
        let forward = Vec3::new(facing.sin(), 0.0, facing.cos());
        let left = Vec3::new(forward.z, 0.0, -forward.x);
        let v = step / dt;
        let turn = wrap_angle(facing - last_facing) / dt;
        let k = if smoothing_s > 1e-4 { 1.0 - (-dt / smoothing_s).exp() } else { 1.0 };
        self.input.speed += (v.dot(forward) - self.input.speed) * k;
        self.input.side += (v.dot(left) - self.input.side) * k;
        self.input.turn_rate += (turn - self.input.turn_rate) * k;
        // Up and down (rung 2a): a jump's rise and a fall, read like the speed (the stance says
        // whether the body is off the ground; this only says which way it is going).
        self.input.vertical += (rise / dt - self.input.vertical) * k;
        self.input
    }

    /// The last reading.
    pub fn input(&self) -> LocomotionInput {
        self.input
    }
}

/// An angle wrapped into -pi to pi.
pub fn wrap_angle(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    let mut x = a.rem_euclid(tau);
    if x > std::f32::consts::PI {
        x -= tau;
    }
    x
}
