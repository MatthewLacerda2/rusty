//! The aim / look-at chain solver (#461): turn a short chain (spine → chest →
//! neck → head) so its last bone's aim axis points at a target, the turn spread
//! over the chain by per-bone weights and clamped to a cone around the animated
//! aim (Unity's `MultiAim` limits, `Animator.SetLookAtWeight`'s clamp). Pure:
//! poses in, rotations out.

use glam::{Quat, Vec3};

use super::BonePose;

/// Re-aims per solve: each pass re-measures from where the last one left the
/// tip, so the parallax of turning lower bones (the head swings sideways as the
/// spine turns) is absorbed. Fixed, so the result is deterministic.
const PASSES: usize = 4;
const EPS: f32 = 1e-6;

/// How an aim chain turns.
#[derive(Clone, Copy, Debug)]
pub struct AimSettings<'a> {
    /// Each bone's share of the turn, parallel to the chain; empty = even.
    pub weights: &'a [f32],
    /// The aim direction in the last bone's local space.
    pub axis: Vec3,
    /// The most the aim may leave the animated aim, in radians.
    pub clamp: f32,
    /// The fraction of the (clamped) turn applied, `[0, 1]`.
    pub weight: f32,
}

/// The world rotations `chain` (root first) takes so its last bone aims at
/// `target`. `None` for an empty chain, a zero weight or zero total share (an
/// exact no-op), or a target on the aim origin.
pub fn solve_aim(chain: &[BonePose], target: Vec3, s: AimSettings) -> Option<Vec<Quat>> {
    let n = chain.len();
    if n == 0 || s.weight <= 0.0 || s.axis.length_squared() < EPS {
        return None;
    }
    let shares = shares(s.weights, n)?;
    let mut poses = chain.to_vec();
    let axis = s.axis.normalize();
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

/// Each bone's fraction of the turn: `weights` normalised to sum to 1 (negative
/// shares count as 0), or an even split when `weights` doesn't match the chain.
fn shares(weights: &[f32], n: usize) -> Option<Vec<f32>> {
    if weights.len() != n {
        return Some(vec![1.0 / n as f32; n]);
    }
    let total: f32 = weights.iter().map(|w| w.max(0.0)).sum();
    (total > EPS).then(|| weights.iter().map(|w| w.max(0.0) / total).collect())
}

/// `from` turned toward `to`, at most `clamp` radians, then by `weight` of that.
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

/// Rotate bone `i` by the world rotation `q` about its own position, carrying
/// every later bone of the chain (its descendants) with it.
fn rotate_from(poses: &mut [BonePose], i: usize, q: Quat) {
    let pivot = poses[i].pos;
    for p in &mut poses[i..] {
        p.pos = pivot + q * (p.pos - pivot);
        p.rot = q * p.rot;
    }
}
