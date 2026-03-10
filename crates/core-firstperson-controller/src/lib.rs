use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0, z: 0.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ControllerState {
    pub position: Vec3,
    pub yaw_deg: f32,
    pub pitch_deg: f32,
    pub stamina: f32,
}

impl ControllerState {
    pub fn baseline() -> Self {
        Self {
            position: Vec3::zero(),
            yaw_deg: 0.0,
            pitch_deg: 0.0,
            stamina: 100.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveDir {
    Forward,
    Backward,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControllerInput {
    pub dir: MoveDir,
    pub dt_seconds: f32,
    pub sprint: bool,
}

pub fn apply_look(state: &mut ControllerState, yaw_delta: f32, pitch_delta: f32) {
    state.yaw_deg += yaw_delta;
    state.pitch_deg = (state.pitch_deg + pitch_delta).clamp(-89.0, 89.0);
}

pub fn apply_move(state: &mut ControllerState, input: ControllerInput) {
    let base_speed = 2.5f32;
    let sprint_allowed = input.sprint && state.stamina > 5.0;
    let speed_mult = if sprint_allowed { 1.7 } else { 1.0 };
    let speed = base_speed * speed_mult;

    let distance = speed * input.dt_seconds.max(0.0);
    match input.dir {
        MoveDir::Forward => state.position.z += distance,
        MoveDir::Backward => state.position.z -= distance,
        MoveDir::Left => state.position.x -= distance,
        MoveDir::Right => state.position.x += distance,
    }

    if sprint_allowed {
        state.stamina = (state.stamina - 8.0 * input.dt_seconds).clamp(0.0, 100.0);
    } else {
        state.stamina = (state.stamina + 2.5 * input.dt_seconds).clamp(0.0, 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn look_clamps_pitch() {
        let mut s = ControllerState::baseline();
        apply_look(&mut s, 0.0, 200.0);
        assert_eq!(s.pitch_deg, 89.0);
        apply_look(&mut s, 0.0, -300.0);
        assert_eq!(s.pitch_deg, -89.0);
    }

    #[test]
    fn sprint_moves_faster_and_uses_stamina() {
        let mut walk = ControllerState::baseline();
        let mut sprint = ControllerState::baseline();

        apply_move(
            &mut walk,
            ControllerInput {
                dir: MoveDir::Forward,
                dt_seconds: 1.0,
                sprint: false,
            },
        );
        apply_move(
            &mut sprint,
            ControllerInput {
                dir: MoveDir::Forward,
                dt_seconds: 1.0,
                sprint: true,
            },
        );

        assert!(sprint.position.z > walk.position.z);
        assert!(sprint.stamina < walk.stamina);
    }
}
