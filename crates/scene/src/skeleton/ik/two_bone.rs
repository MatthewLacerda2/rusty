use glam::{Quat, Vec3};

use super::BonePose;

const EPS: f32 = 1e-6;

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

    let old = interior_angle((c - a).length(), l_ab, l_bc);
    let new = interior_angle(l_at, l_ab, l_bc);
    let bend = Quat::from_axis_angle(bend_axis(ab, bc, at, hint, a), old - new);
    let bent_tip = b + bend * bc;

    let swing = Quat::from_rotation_arc((bent_tip - a).normalize(), at / l_at);
    let mut root_rot = swing * root.rot;
    let mut mid_rot = swing * bend * mid.rot;

    if let Some(h) = hint {
        let twist = twist_toward(swing * ab, h - a, at / l_at);
        root_rot = twist * root_rot;
        mid_rot = twist * mid_rot;
    }
    Some((root_rot.normalize(), mid_rot.normalize()))
}

fn interior_angle(opposite: f32, l_ab: f32, l_bc: f32) -> f32 {
    let cos = (l_ab * l_ab + l_bc * l_bc - opposite * opposite) / (2.0 * l_ab * l_bc);
    cos.clamp(-1.0, 1.0).acos()
}

fn bend_axis(ab: Vec3, bc: Vec3, at: Vec3, hint: Option<Vec3>, a: Vec3) -> Vec3 {
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

fn twist_toward(from: Vec3, toward: Vec3, axis: Vec3) -> Quat {
    let p = from - axis * from.dot(axis);
    let q = toward - axis * toward.dot(axis);
    if p.length_squared() < EPS * EPS || q.length_squared() < EPS * EPS {
        return Quat::IDENTITY;
    }
    let angle = p.cross(q).dot(axis).atan2(p.dot(q));
    Quat::from_axis_angle(axis, angle)
}
