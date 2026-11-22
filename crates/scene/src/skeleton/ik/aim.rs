use glam::{Quat, Vec3};

use super::BonePose;
use rusty_components::transform::mirrored_direction;

/// Re-aim passes per solve: each re-measures from the tip, absorbing the parallax of turning
/// lower bones. Fixed, so the result is deterministic.
const PASSES: usize = 4;
/// The zero below which a squared length, a weight sum or an angle (radians) leaves
/// nothing to normalise or divide by.
const EPS: f32 = 1e-6;

#[derive(Clone, Copy, Debug)]
pub struct AimSettings<'a> {
    /// Each bone's share of the turn; one not matching the chain's length splits evenly.
    pub weights: &'a [f32],
    /// The aim direction in the last bone's local space.
    pub axis: Vec3,
    /// Radians the aim may leave the animated aim (Unity's `MultiAim` limit).
    pub clamp: f32,
    pub weight: f32,
}

pub fn solve_aim(chain: &[BonePose], target: Vec3, s: AimSettings) -> Option<Vec<Quat>> {
    let n = chain.len();
    if n == 0 || s.weight <= 0.0 || s.axis.length_squared() < EPS {
        return None;
    }
    let shares = shares(s.weights, n)?;
    let mut poses = chain.to_vec();
    let axis = mirrored_direction(chain[n - 1].mirror, s.axis.normalize());
    let animated = (chain[n - 1].rot * axis).normalize();
    for _ in 0..PASSES {
        let to = target - poses[n - 1].pos;
        if to.length_squared() < EPS {
            return None;
        }
        let goal = turn_toward(animated, to.normalize(), s.clamp, s.weight.min(1.0));
        let delta = Quat::from_rotation_arc((poses[n - 1].rot * axis).normalize(), goal);
        for (i, share) in shares.iter().enumerate() {
            rotate_from(&mut poses, i, Quat::IDENTITY.slerp(delta, *share));
        }
    }
    Some(poses.iter().map(|p| p.rot.normalize()).collect())
}

fn shares(weights: &[f32], n: usize) -> Option<Vec<f32>> {
    if weights.len() != n {
        return Some(vec![1.0 / n as f32; n]);
    }
    let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
    (total > EPS).then(|| weights.iter().map(|w| w.max(0.0) / total).collect())
}

fn turn_toward(from: Vec3, to: Vec3, clamp: f32, weight: f32) -> Vec3 {
    let arc = Quat::from_rotation_arc(from, to);
    let angle = from.angle_between(to);
    let limit = clamp.max(0.0);
    let reach = if angle > limit && angle > EPS {
        limit / angle
    } else {
        1.0
    };
    Quat::IDENTITY.slerp(arc, reach * weight) * from
}

fn rotate_from(poses: &mut [BonePose], i: usize, q: Quat) {
    let pivot = poses[i].pos;
    for p in &mut poses[i..] {
        p.pos = pivot + q * (p.pos - pivot);
        p.rot = q * p.rot;
    }
}
