//! The aim-chain solver (#461): aims at the target, spreads the turn by weight,
//! clamps at its limit, weight 0 is a no-op.

use glam::{Quat, Vec3};

use super::{solve_aim, AimSettings, BonePose};

/// A three-bone spine up +Y (spine, neck, head), every bone looking +Z.
fn spine() -> Vec<BonePose> {
    (0..3)
        .map(|i| BonePose {
            pos: Vec3::Y * i as f32 * 0.3,
            rot: Quat::IDENTITY,
        })
        .collect()
}

fn settings(clamp_degrees: f32, weight: f32) -> AimSettings<'static> {
    AimSettings {
        weights: &[],
        axis: Vec3::Z,
        clamp: clamp_degrees.to_radians(),
        weight,
    }
}

/// The head's new aim and position after the chain takes `rots`.
fn head(chain: &[BonePose], rots: &[Quat]) -> (Vec3, Vec3) {
    let mut pos = chain[0].pos;
    for i in 1..chain.len() {
        let parent_delta = rots[i - 1] * chain[i - 1].rot.inverse();
        pos += parent_delta * (chain[i].pos - chain[i - 1].pos);
    }
    (rots[rots.len() - 1] * Vec3::Z, pos)
}

#[test]
fn the_chain_aims_its_last_bone_at_the_target() {
    let chain = spine();
    let target = Vec3::new(3.0, 1.0, 1.0);
    let rots = solve_aim(&chain, target, settings(180.0, 1.0)).unwrap();
    let (aim, origin) = head(&chain, &rots);
    let want = (target - origin).normalize();
    assert!(aim.angle_between(want) < 1e-3, "aim {aim:?}, want {want:?}");
}

#[test]
fn the_turn_is_spread_by_the_per_bone_weights() {
    let chain = spine();
    let s = AimSettings {
        weights: &[1.0, 0.0, 3.0],
        ..settings(180.0, 1.0)
    };
    let rots = solve_aim(&chain, Vec3::new(10.0, 0.6, 0.0), s).unwrap();
    let turned = |q: Quat| q.to_axis_angle().1;
    let (spine, neck, head) = (turned(rots[0]), turned(rots[1]), turned(rots[2]));
    assert!(spine > 0.1, "the spine takes a share, {spine}");
    assert!(
        (neck - spine).abs() < 1e-3,
        "a 0-weight bone only rides along"
    );
    assert!(
        (head / spine - 4.0).abs() < 0.2,
        "head carries 3/4: {head} vs {spine}"
    );
}

#[test]
fn the_aim_clamps_at_its_limit() {
    let chain = spine();
    let rots = solve_aim(&chain, Vec3::new(-5.0, 0.6, 0.0), settings(30.0, 1.0)).unwrap();
    let (aim, _) = head(&chain, &rots);
    let turned = aim.angle_between(Vec3::Z).to_degrees();
    assert!((turned - 30.0).abs() < 1e-2, "turned {turned}°");
    assert!(aim.x < 0.0, "toward the target's side");
}

#[test]
fn weight_scales_the_turn_and_zero_is_a_no_op() {
    let chain = spine();
    let target = Vec3::new(5.0, 0.6, 0.0); // 90° to the side of the head
    assert_eq!(solve_aim(&chain, target, settings(180.0, 0.0)), None);
    let rots = solve_aim(&chain, target, settings(40.0, 0.5)).unwrap();
    let turned = head(&chain, &rots).0.angle_between(Vec3::Z).to_degrees();
    assert!(
        (turned - 20.0).abs() < 1e-2,
        "half of the clamped 40°, got {turned}"
    );
}
