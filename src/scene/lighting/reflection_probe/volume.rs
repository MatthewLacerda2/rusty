//! A reflection probe's volume (#1995): the box `box_min`..`box_max` turned by
//! `rotation` about the capture point (HDRP's oriented box), or a sphere of `radius`
//! about it. What [`ReflectionProbe::contains`], the blend's influence and the
//! renderer's budget ranking measure, and the frame the projection
//! (`projection.rs`) and the shader work in.

use glam::{Quat, Vec3};

use super::{ProbeShape, ReflectionProbe};

/// A probe's volume in the frame the shader projects in (#1995): a world point is
/// turned into it by [`Self::inverse`], a rotation about the ORIGIN, and there the
/// box is axis-aligned, its capture point [`Self::center`]. An unrotated probe's
/// frame is the world, its box and centre the stored ones, bit for bit, so the
/// shader's projection of a world-aligned box is the one it always ran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbeFrame {
    pub inverse: Quat,
    pub center: Vec3,
    pub box_min: Vec3,
    pub box_max: Vec3,
    /// The radius when the probe is a sphere about [`Self::center`].
    pub sphere: Option<f32>,
}

impl ProbeFrame {
    /// `v` turned into the frame. The shader's `quat_rotate`, step for step: an
    /// identity rotation adds an exact zero, so it changes no bits.
    pub fn to_frame(&self, v: Vec3) -> Vec3 {
        rotate(self.inverse, v)
    }

    /// `v`, a vector of the frame, turned back into the world.
    pub fn to_world(&self, v: Vec3) -> Vec3 {
        rotate(self.inverse.conjugate(), v)
    }

    /// `point` (in the frame) clamped onto the volume: itself inside it.
    fn nearest(&self, point: Vec3) -> Vec3 {
        match self.sphere {
            None => point.clamp(self.box_min, self.box_max),
            Some(r) => {
                let off = point - self.center;
                match off.length() > r {
                    true => self.center + off * (r / off.length()),
                    false => point,
                }
            }
        }
    }
}

/// `v` turned by `q`: `v + 2 u x (u x v + w v)`, as the shader's `quat_rotate`.
pub(crate) fn rotate(q: Quat, v: Vec3) -> Vec3 {
    let u = Vec3::new(q.x, q.y, q.z);
    v + 2.0 * u.cross(u.cross(v) + q.w * v)
}

impl ReflectionProbe {
    /// The probe's volume in the shader's frame (see [`ProbeFrame`]).
    pub fn frame(&self) -> ProbeFrame {
        let sphere = (self.shape == ProbeShape::Sphere).then_some(self.radius);
        if self.rotation == Quat::IDENTITY {
            return ProbeFrame {
                inverse: Quat::IDENTITY,
                center: self.position,
                box_min: self.box_min,
                box_max: self.box_max,
                sphere,
            };
        }
        let inverse = self.rotation.conjugate();
        let center = rotate(inverse, self.position);
        let shift = center - self.position;
        ProbeFrame {
            inverse,
            center,
            box_min: self.box_min + shift,
            box_max: self.box_max + shift,
            sphere,
        }
    }

    /// True when `point` lies inside this probe's volume (inclusive).
    pub fn contains(&self, point: Vec3) -> bool {
        self.volume_distance_sq(point) <= 0.0
    }

    /// Squared distance from `point` to the nearest point of the probe's volume,
    /// zero inside it: the blend's influence falls off with it, and the renderer
    /// ranks probes by it for its resident budget (#1238), so a big room whose
    /// capture point is far away still counts as near when the camera stands in it.
    pub fn volume_distance_sq(&self, point: Vec3) -> f32 {
        let frame = self.frame();
        let local = frame.to_frame(point);
        frame.nearest(local).distance_squared(local)
    }

    /// Squared distance from `point` to the probe's capture position — the
    /// blend's tiebreak metric.
    pub(super) fn distance_sq(&self, point: Vec3) -> f32 {
        self.position.distance_squared(point)
    }

    /// The world-space corners of the probe's box, turned by its rotation, in
    /// `aabb_wireframe`'s order of `x`, then `y`, then `z` (bit `0` of the index
    /// picks max x, bit 1 max y, bit 2 max z).
    pub fn corners(&self) -> [Vec3; 8] {
        let (lo, hi) = (self.box_min, self.box_max);
        std::array::from_fn(|i| {
            let pick = |bit: usize, a: f32, b: f32| if i & bit == 0 { a } else { b };
            let c = Vec3::new(
                pick(1, lo.x, hi.x),
                pick(2, lo.y, hi.y),
                pick(4, lo.z, hi.z),
            );
            match self.rotation == Quat::IDENTITY {
                true => c,
                false => self.position + self.rotation * (c - self.position),
            }
        })
    }

    /// The world-aligned bounds of the probe's volume: its box's corners' bounds,
    /// or the sphere's. An unrotated box's are its corners, exactly.
    pub fn world_bounds(&self) -> (Vec3, Vec3) {
        if self.shape == ProbeShape::Sphere {
            let r = Vec3::splat(self.radius);
            return (self.position - r, self.position + r);
        }
        let corners = self.corners();
        let lo = corners.iter().fold(Vec3::INFINITY, |a, &c| a.min(c));
        let hi = corners.iter().fold(Vec3::NEG_INFINITY, |a, &c| a.max(c));
        (lo, hi)
    }
}
