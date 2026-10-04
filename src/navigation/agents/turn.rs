//! Turning an agent to face where it steers (#744, Unity's `updateRotation`): yaw
//! only, at a yaw rate capped by `angular_speed` that ramps up and down at
//! `angular_acceleration`, so the agent eases into a turn and settles on its heading
//! without overshoot.

use glam::{EulerRot, Quat, Vec2};

use super::tick::braking_speed;
use crate::scene::NavMeshAgentComponent;

/// Turn `rotation` one fixed step toward `heading` (a planar XZ direction; zero
/// holds the current facing). Only the yaw changes, so an authored tilt is kept.
/// With `update_rotation` off the facing is the script's, and the agent never turns.
pub(super) fn turn_toward(
    agent: &mut NavMeshAgentComponent,
    rotation: &mut Quat,
    heading: Vec2,
    delta_time: f32,
) {
    if !agent.update_rotation || heading == Vec2::ZERO {
        agent.angular_velocity = 0.0;
        return;
    }
    // +Z is forward: a yaw of θ faces (sin θ, cos θ) on XZ.
    let (yaw, _, _) = rotation.to_euler(EulerRot::YXZ);
    let target = heading.x.atan2(heading.y);
    let error = wrap_degrees((target - yaw).to_degrees());
    let step = yaw_step(agent, error, delta_time);
    *rotation = Quat::from_rotation_y(step.to_radians()) * *rotation;
}

/// The yaw (degrees) to turn this tick toward an `error` degrees away, updating the
/// agent's yaw rate. The rate moves toward the fastest one that can still wind down
/// to zero by the heading (the same discrete braking profile as the agent's speed),
/// capped at `angular_speed`, by at most `angular_acceleration · dt`. A step that
/// would pass the heading lands on it instead.
fn yaw_step(agent: &mut NavMeshAgentComponent, error: f32, delta_time: f32) -> f32 {
    let accel = agent.angular_acceleration.max(0.0);
    let cap = agent
        .angular_speed
        .max(0.0)
        .min(braking_speed(accel, error.abs(), delta_time));
    let rate = move_towards(
        agent.angular_velocity,
        cap.copysign(error),
        accel * delta_time,
    );
    let step = rate * delta_time;
    if step * error > 0.0 && step.abs() >= error.abs() {
        agent.angular_velocity = error / delta_time;
        return error;
    }
    agent.angular_velocity = rate;
    step
}

/// `degrees` folded into [-180, 180): the shortest arc to the same heading.
fn wrap_degrees(degrees: f32) -> f32 {
    (degrees + 180.0).rem_euclid(360.0) - 180.0
}

/// `from` moved toward `to` by at most `max_delta`.
fn move_towards(from: f32, to: f32, max_delta: f32) -> f32 {
    from + (to - from).clamp(-max_delta, max_delta)
}

#[cfg(test)]
#[path = "turn_tests.rs"]
mod tests;
