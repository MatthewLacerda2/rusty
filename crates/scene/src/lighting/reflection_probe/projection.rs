//! Parallax correction against the oriented volume (Lagarde's, as HDRP's proxy); the
//! shader's `parallax_correct` runs the same steps with the same guards.

use glam::Vec3;

use super::{ProbeFrame, ReflectionProbe, NO_HIT, PARALLEL_EPSILON};

impl ReflectionProbe {
    pub fn parallax_correct(&self, world_pos: Vec3, reflection: Vec3) -> Vec3 {
        project(&self.frame(), world_pos, reflection.normalize_or_zero())
    }
}

fn project(frame: &ProbeFrame, world_pos: Vec3, dir: Vec3) -> Vec3 {
    let p = frame.to_frame(world_pos);
    let fade = past_influence(frame, p);
    if fade >= 1.0 {
        return dir;
    }
    let projected = project_in_volume(frame, p, dir);
    if fade <= 0.0 {
        return projected;
    }
    projected.lerp(dir, fade).normalize_or(dir)
}

/// 0 within the influence volume, 1 a `fade` past it: far outside, every exit lies on the
/// volume, so projected directions would collapse to one patch of the cube.
fn past_influence(frame: &ProbeFrame, p: Vec3) -> f32 {
    let x = ((frame.outside(p) - frame.influence) / frame.fade).clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

pub(super) fn project_in_volume(frame: &ProbeFrame, p: Vec3, dir: Vec3) -> Vec3 {
    let d = frame.to_frame(dir);
    let t = match frame.sphere {
        None => box_exit(frame, p, d),
        Some(r) => sphere_exit(frame, p, d, r),
    };
    if t >= NO_HIT {
        return dir;
    }
    let to_hit = p + d * t - frame.center;
    if to_hit.length_squared() <= 0.0 {
        return dir;
    }
    frame.to_world(to_hit).normalize()
}

fn box_exit(frame: &ProbeFrame, p: Vec3, d: Vec3) -> f32 {
    let mut t = NO_HIT;
    for axis in 0..3 {
        if d[axis].abs() > PARALLEL_EPSILON {
            let first = (frame.box_max[axis] - p[axis]) / d[axis];
            let second = (frame.box_min[axis] - p[axis]) / d[axis];
            // A magnitude, not held at 0 as HDRP does: outside, a ray turning parallel to a face
            // would jump from the fragment to the far wall.
            t = t.min(first.max(second).abs());
        }
    }
    t
}

/// The far root, or the nearest approach when it misses (HDRP's `SphereRayIntersectSimple`).
fn sphere_exit(frame: &ProbeFrame, p: Vec3, d: Vec3, r: f32) -> f32 {
    if d.length_squared() <= 0.0 {
        return NO_HIT;
    }
    let oc = p - frame.center;
    let b = oc.dot(d);
    let c = oc.dot(oc) - r * r;
    (-b + (b * b - c).max(0.0).sqrt()).abs()
}
