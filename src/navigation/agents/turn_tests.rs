//! The turn step (#744): capped rate, ramped by the angular acceleration, settling
//! on the heading by the short arc without overshoot.

use glam::{EulerRot, Quat, Vec2};

use super::*;

const DT: f32 = 1.0 / 60.0;

fn yaw_deg(q: Quat) -> f32 {
    q.to_euler(EulerRot::YXZ).0.to_degrees()
}

/// Turns from facing +Z toward `heading` for `ticks`, returning the yaw each tick.
fn turn(agent: &mut NavMeshAgentComponent, heading: Vec2, ticks: usize) -> Vec<f32> {
    let mut q = Quat::IDENTITY;
    (0..ticks)
        .map(|_| {
            turn_toward(agent, &mut q, heading, DT);
            yaw_deg(q)
        })
        .collect()
}

#[test]
fn ramps_up_caps_and_settles_without_overshoot() {
    let mut a = NavMeshAgentComponent::default();
    // +X is a yaw of +90°.
    let yaws = turn(&mut a, Vec2::new(1.0, 0.0), 120);
    // The first tick turns only α·dt² (720/3600 = 0.2°), not a snap.
    assert!((yaws[0] - 0.2).abs() < 1e-4, "{}", yaws[0]);
    let top = yaws
        .windows(2)
        .map(|w| (w[1] - w[0]) / DT)
        .fold(0.0, f32::max);
    assert!((top - 120.0).abs() < 1e-2, "capped at angular_speed: {top}");
    assert!(yaws.iter().all(|y| *y <= 90.0 + 1e-4), "no overshoot");
    // 90° at 120°/s with a 10° ramp each end: 70/120 + 2/6 s ≈ 55 ticks.
    let settled = 1 + yaws.iter().position(|y| (y - 90.0).abs() < 1e-3).unwrap();
    assert!((53..=56).contains(&settled), "settled at tick {settled}");
    assert_eq!(a.angular_velocity, 0.0, "at rest on the heading");
}

#[test]
fn takes_the_short_arc() {
    let mut a = NavMeshAgentComponent::default();
    // -X is 270° clockwise or 90° the other way: it turns negative.
    let yaws = turn(&mut a, Vec2::new(-1.0, 0.0), 120);
    assert!(yaws[0] < 0.0);
    assert!((yaws[119] + 90.0).abs() < 1e-3, "{}", yaws[119]);
}

#[test]
fn holds_facing_without_heading_or_update_rotation() {
    let mut a = NavMeshAgentComponent {
        angular_velocity: 60.0,
        ..Default::default()
    };
    assert!(turn(&mut a, Vec2::ZERO, 5).iter().all(|y| *y == 0.0));
    assert_eq!(
        a.angular_velocity, 0.0,
        "a held facing does not keep spinning"
    );
    a.update_rotation = false;
    assert!(turn(&mut a, Vec2::new(1.0, 0.0), 5)
        .iter()
        .all(|y| *y == 0.0));
}

#[test]
fn zero_rate_or_ramp_never_turns() {
    for (speed, accel) in [(0.0, 720.0), (120.0, 0.0)] {
        let mut a = NavMeshAgentComponent {
            angular_speed: speed,
            angular_acceleration: accel,
            ..Default::default()
        };
        assert!(turn(&mut a, Vec2::new(1.0, 0.0), 30)
            .iter()
            .all(|y| *y == 0.0));
    }
}

#[test]
fn keeps_an_authored_tilt() {
    let mut a = NavMeshAgentComponent::default();
    let tilt = Quat::from_rotation_x(-0.5);
    let mut q = tilt;
    for _ in 0..120 {
        turn_toward(&mut a, &mut q, Vec2::new(1.0, 0.0), DT);
    }
    let (yaw, pitch, roll) = q.to_euler(EulerRot::YXZ);
    assert!((yaw.to_degrees() - 90.0).abs() < 1e-3);
    assert!((pitch + 0.5).abs() < 1e-5 && roll.abs() < 1e-5);
}

#[test]
fn wraps_to_the_shortest_arc() {
    assert_eq!(wrap_degrees(270.0), -90.0);
    assert_eq!(wrap_degrees(-270.0), 90.0);
    assert_eq!(wrap_degrees(45.0), 45.0);
}

/// Spinning at 60°/s with its heading 0.5° ahead, this tick's step (0.8°) would
/// pass it: it lands on the heading instead, keeping the rate it really turned at
/// (30°/s), either way round.
#[test]
fn lands_on_the_heading_instead_of_passing_it() {
    for sign in [1.0f32, -1.0] {
        let mut a = NavMeshAgentComponent {
            angular_velocity: 60.0 * sign,
            ..Default::default()
        };
        let target = (0.5 * sign).to_radians();
        let yaws = turn(&mut a, Vec2::new(target.sin(), target.cos()), 1);
        assert!((yaws[0] - 0.5 * sign).abs() < 1e-4, "{}", yaws[0]);
        assert!(
            (a.angular_velocity - 30.0 * sign).abs() < 1e-2,
            "{}",
            a.angular_velocity
        );
    }
}

/// Spinning at 60°/s onto a still heading it already faces, it can only shed
/// α·dt (12°/s) this tick, so it carries on past rather than stopping dead.
#[test]
fn momentum_carries_past_an_exact_heading() {
    let mut a = NavMeshAgentComponent {
        angular_velocity: 60.0,
        ..Default::default()
    };
    let yaws = turn(&mut a, Vec2::new(0.0, 1.0), 1);
    assert!((yaws[0] - 0.8).abs() < 1e-4, "{}", yaws[0]);
    assert_eq!(a.angular_velocity, 48.0);
}
