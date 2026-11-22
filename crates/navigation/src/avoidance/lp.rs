//! A port of the three programs in RVO2 (`Agent.cpp`, Apache-2.0). `lp2` adds the half-planes in the
//! caller's stable order, so the result is deterministic; `lp3` is the infeasible fallback and
//! minimises the largest penetration into any half-plane.

use glam::Vec2;

/// Parallel-line tolerance, as in RVO2's `RVO_EPSILON`.
const EPSILON: f32 = 1e-5;

/// One ORCA half-plane: the permitted velocities lie to the **left** of the
/// directed line through `point` along the unit `direction`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Line {
    pub point: Vec2,
    pub direction: Vec2,
}

/// Solve on line `line_no`, clipped by the lines before it and the speed disc.
fn lp1(lines: &[Line], line_no: usize, radius: f32, opt: Vec2, dir_opt: bool) -> Option<Vec2> {
    let line = lines[line_no];
    let dot = line.point.dot(line.direction);
    let discriminant = dot * dot + radius * radius - line.point.length_squared();
    if discriminant < 0.0 {
        return None;
    }
    let sqrt_disc = discriminant.sqrt();
    let mut t_left = -dot - sqrt_disc;
    let mut t_right = -dot + sqrt_disc;
    for prior in &lines[..line_no] {
        let denominator = line.direction.perp_dot(prior.direction);
        let numerator = prior.direction.perp_dot(line.point - prior.point);
        if denominator.abs() <= EPSILON {
            if numerator < 0.0 {
                return None;
            }
            continue;
        }
        let t = numerator / denominator;
        if denominator >= 0.0 {
            t_right = t_right.min(t);
        } else {
            t_left = t_left.max(t);
        }
        if t_left > t_right {
            return None;
        }
    }
    let t = if dir_opt {
        if opt.dot(line.direction) > 0.0 {
            t_right
        } else {
            t_left
        }
    } else {
        line.direction.dot(opt - line.point).clamp(t_left, t_right)
    };
    Some(line.point + t * line.direction)
}

/// Returns the result and how many lines it satisfied before failing (`lines.len()` on success).
pub(super) fn lp2(lines: &[Line], radius: f32, opt: Vec2, dir_opt: bool) -> (Vec2, usize) {
    let mut result = if dir_opt {
        opt * radius
    } else if opt.length_squared() > radius * radius {
        opt.normalize_or_zero() * radius
    } else {
        opt
    };
    for (i, line) in lines.iter().enumerate() {
        if line.direction.perp_dot(line.point - result) > 0.0 {
            match lp1(lines, i, radius, opt, dir_opt) {
                Some(v) => result = v,
                None => return (result, i),
            }
        }
    }
    (result, lines.len())
}

pub(super) fn lp3(lines: &[Line], begin: usize, radius: f32, mut result: Vec2) -> Vec2 {
    let mut distance = 0.0;
    for (i, line) in lines.iter().enumerate().skip(begin) {
        if line.direction.perp_dot(line.point - result) <= distance {
            continue;
        }
        let mut projected = Vec::with_capacity(i);
        for prior in &lines[..i] {
            let determinant = line.direction.perp_dot(prior.direction);
            let point = if determinant.abs() <= EPSILON {
                if line.direction.dot(prior.direction) > 0.0 {
                    continue;
                }
                0.5 * (line.point + prior.point)
            } else {
                let along = prior.direction.perp_dot(line.point - prior.point) / determinant;
                line.point + along * line.direction
            };
            let direction = (prior.direction - line.direction).normalize_or_zero();
            projected.push(Line { point, direction });
        }
        let toward = Vec2::new(-line.direction.y, line.direction.x);
        let (candidate, done) = lp2(&projected, radius, toward, true);
        if done == projected.len() {
            result = candidate;
        }
        distance = line.direction.perp_dot(line.point - result);
    }
    result
}
