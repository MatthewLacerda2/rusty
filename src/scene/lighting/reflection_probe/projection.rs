//! Box- and sphere-projected parallax correction (#244, #1995): the standard Lagarde
//! technique against the probe's oriented volume, as HDRP projects against its
//! proxy. The shader's `parallax_correct`
//! (`engine/shaders/lighting/reflection_probe.wgsl`) runs the same steps in the same
//! frame ([`ProbeFrame`]) with the same guards.
//!
//! A fragment OUTSIDE the volume still projects (#1995), so a fragment past a face
//! reflects what one just inside it does: falling to infinite projection there drew
//! a hard straight step along every face of a probe an object reached past (a
//! non-rectangular room). Each slab's exit distance is taken as a magnitude: inside
//! the volume it is never negative, so nothing changes there, and outside it a ray
//! leaving a slab behind it measures the distance back to its face. Holding it at
//! zero instead (HDRP's `max(projectionDistance, 0)`) breaks where such a ray turns
//! parallel to the face: tilted one way it hit at the fragment, the other way at the
//! far wall, a hard line down a curved floor under an oriented box. As a magnitude,
//! both sides run to infinity there and the closest other face takes over.

use glam::Vec3;

use super::{ProbeFrame, ReflectionProbe, NO_HIT, PARALLEL_EPSILON};

impl ReflectionProbe {
    /// Given a fragment world position and a raw reflection direction, intersect the
    /// ray against this probe's volume and return the world direction from the
    /// probe's capture point to where it leaves it. This is what makes the
    /// reflection track the room as the camera moves, instead of behaving like an
    /// infinitely-distant skybox. A zero direction stays zero. With
    /// [`Self::box_projection`] off the shader skips it.
    /// Test seam for `tests/reflection_probes.rs`.
    pub fn parallax_correct(&self, world_pos: Vec3, reflection: Vec3) -> Vec3 {
        project(&self.frame(), world_pos, reflection.normalize_or_zero())
    }
}

/// The shader's `parallax_correct`: `dir` (normalized) from `world_pos`, projected
/// against `frame`'s volume.
fn project(frame: &ProbeFrame, world_pos: Vec3, dir: Vec3) -> Vec3 {
    let p = frame.to_frame(world_pos);
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

/// Where the ray leaves the box: per axis the exit distance (its magnitude, see the
/// module docs), then the closest of the three. An axis the ray runs parallel to
/// never exits through its planes, so it is set to [`NO_HIT`] instead of dividing
/// by zero.
fn box_exit(frame: &ProbeFrame, p: Vec3, d: Vec3) -> f32 {
    let mut t = NO_HIT;
    for axis in 0..3 {
        if d[axis].abs() > PARALLEL_EPSILON {
            let first = (frame.box_max[axis] - p[axis]) / d[axis];
            let second = (frame.box_min[axis] - p[axis]) / d[axis];
            t = t.min(first.max(second).abs());
        }
    }
    t
}

/// The far root of the ray against the sphere, or the ray's nearest approach to
/// its centre when it misses (HDRP's `SphereRayIntersectSimple`), as a magnitude.
fn sphere_exit(frame: &ProbeFrame, p: Vec3, d: Vec3, r: f32) -> f32 {
    if d.length_squared() <= 0.0 {
        return NO_HIT;
    }
    let oc = p - frame.center;
    let b = oc.dot(d);
    let c = oc.dot(oc) - r * r;
    (-b + (b * b - c).max(0.0).sqrt()).abs()
}
