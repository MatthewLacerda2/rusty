//! The analytic two-bone solver (#461): an arm or a leg reaching a point, the
//! elbow or knee bending toward a hint. Unity's `AnimationRuntimeUtils
//! .SolveTwoBoneIK`, in world space: bend the middle joint to the angle the law
//! of cosines gives for the target's distance, swing the root so the tip lands on
//! the target, then twist the root about the root→target line so the middle
//! joint lies on the hint's side. Pure: poses in, rotations out.

use glam::{Quat, Vec3};

use super::BonePose;

const EPS: f32 = 1e-6;

/// The world rotations `root` and `mid` take so the tip (now at `tip`) reaches
/// `target`, pulled `weight` of the way from where it is (Unity's target
/// weight). Out of reach, the limb straightens and points at it; too close, it
/// folds as far as the bone lengths allow. `None` for a zero weight (an exact
/// no-op) or a degenerate limb (a zero-length bone, the target on the root).
pub fn solve_two_bone(
    root: BonePose,
    mid: BonePose,
    tip: Vec3,
    target: Vec3,
    hint: Option<Vec3>,
    weight: f32,
) -> Option<(Quat, Quat)> {
    if weight <= 0.0 {
        return None;
    }
    let (a, b, c) = (root.pos, mid.pos, tip);
    let t = c.lerp(target, weight.min(1.0));
    let (ab, bc, at) = (b - a, c - b, t - a);
    let (l_ab, l_bc, l_at) = (ab.length(), bc.length(), at.length());
    if l_ab < EPS || l_bc < EPS || l_at < EPS {
        return None;
    }

    // 1. Bend the middle joint to the target's distance.
    let old = interior_angle((c - a).length(), l_ab, l_bc);
    let new = interior_angle(l_at, l_ab, l_bc);
    let bend = Quat::from_axis_angle(bend_axis(ab, bc, at, hint, a), old - new);
    let bent_tip = b + bend * bc;

    // 2. Swing the root so the tip points at the target.
    let swing = Quat::from_rotation_arc((bent_tip - a).normalize(), at / l_at);
    let mut root_rot = swing * root.rot;
    let mut mid_rot = swing * bend * mid.rot;

    // 3. Twist about root→target so the middle joint sits on the hint's side.
    if let Some(h) = hint {
        let twist = twist_toward(swing * ab, h - a, at / l_at);
        root_rot = twist * root_rot;
        mid_rot = twist * mid_rot;
    }
    Some((root_rot.normalize(), mid_rot.normalize()))
}

/// The angle at the middle joint of a triangle with sides `l_ab`, `l_bc` and the
/// opposite side `opposite`; clamped, so an unreachable distance reads straight
/// (π) and an over-close one folded (0).
fn interior_angle(opposite: f32, l_ab: f32, l_bc: f32) -> f32 {
    let cos = (l_ab * l_ab + l_bc * l_bc - opposite * opposite) / (2.0 * l_ab * l_bc);
    cos.clamp(-1.0, 1.0).acos()
}

/// The axis the middle joint bends about: the limb's own plane normal, or — for
/// a limb animated perfectly straight — the plane through the hint (else the
/// target), so the bend has a side to go to.
fn bend_axis(ab: Vec3, bc: Vec3, at: Vec3, hint: Option<Vec3>, a: Vec3) -> Vec3 {
    // Rotating `bc` about `ab × bc` by a positive angle opens the limb further
    // away from `ab`, i.e. closes the interior angle: `old - new` is the bend.
    let candidates = [
        ab.cross(bc),
        hint.map_or(Vec3::ZERO, |h| (h - a).cross(bc)),
        at.cross(bc),
        ab.any_orthogonal_vector(),
    ];
    candidates
        .into_iter()
        .find(|v| v.length_squared() > EPS * EPS)
        .unwrap_or(Vec3::Y)
        .normalize()
}

/// The rotation about `axis` (unit) that brings `from`'s component off the axis
/// onto `toward`'s; identity when either lies on the axis.
fn twist_toward(from: Vec3, toward: Vec3, axis: Vec3) -> Quat {
    let p = from - axis * from.dot(axis);
    let q = toward - axis * toward.dot(axis);
    if p.length_squared() < EPS * EPS || q.length_squared() < EPS * EPS {
        return Quat::IDENTITY;
    }
    let angle = p.cross(q).dot(axis).atan2(p.dot(q));
    Quat::from_axis_angle(axis, angle)
}
