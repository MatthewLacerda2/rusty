//! A reflection probe's volume (#1995): the box `box_min`..`box_max` turned by
//! `rotation` about the capture point (HDRP's oriented box), or a sphere of `radius`
//! about it. What [`ReflectionProbe::contains`], the blend's influence and the
//! renderer's budget ranking measure, and the frame the projection
//! (`projection.rs`) and the shader work in.

use glam::{Quat, Vec3};

use super::{finite_or, ProbeShape, ReflectionProbe, MIN_RADIUS};

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
    /// How far past the volume the probe's influence reaches: its blend distance.
    /// Projection holds up to there.
    pub influence: f32,
    /// Past [`Self::influence`], the distance over which the projection fades back
    /// to infinite projection (#1995): the volume's own size (see
    /// [`Self::fade_span`]).
    pub fade: f32,
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

    /// The distance over which the projection fades back to infinite projection
    /// past the influence volume (#1995): the volume's half-diagonal (the sphere's
    /// radius), floored at [`MIN_RADIUS`]. A projection tracks a room only while
    /// the fragment is near it: from one room-size away the whole volume sits in a
    /// cone of under 60 degrees from the fragment, so every reflection would map to
    /// one patch of the cube, whatever its direction. Fading over the volume's own
    /// size keeps the change as gradual as the parallax itself (scale-free: a 2 m
    /// box and a 40 m hall fade alike) and leaves a far fragment of a large mesh
    /// the plain cube, where its direction still matters.
    pub fn fade_span(&self) -> f32 {
        let size = match self.sphere {
            Some(r) => r,
            None => (self.box_max - self.box_min).length() * 0.5,
        };
        finite_or(size, MIN_RADIUS).max(MIN_RADIUS)
    }

    /// How far `point` (in the frame) lies outside the volume, 0 inside it.
    pub(super) fn outside(&self, point: Vec3) -> f32 {
        self.nearest(point).distance(point)
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
        let mut frame = ProbeFrame {
            inverse: Quat::IDENTITY,
            center: self.position,
            box_min: self.box_min,
            box_max: self.box_max,
            sphere,
            influence: self.blend_distance.max(0.0),
            fade: 0.0,
        };
        if self.rotation != Quat::IDENTITY {
            // Unrotated, the frame is the world, bit for bit (no `+ 0.0`).
            frame.inverse = self.rotation.conjugate();
            frame.center = rotate(frame.inverse, self.position);
            let shift = frame.center - self.position;
            (frame.box_min, frame.box_max) = (self.box_min + shift, self.box_max + shift);
        }
        frame.fade = frame.fade_span();
        frame
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

/// How finely [`ReflectionProbe::share_of`] samples a box an oriented volume
/// covers: this many points a side.
const SHARE_SAMPLES: usize = 16;

impl ReflectionProbe {
    /// Whether this probe's volume and `other`'s share more than a face (#1995):
    /// the turned boxes by the separating-axis test (their 15 axes), a sphere by
    /// its distance to the other volume. Two world-aligned boxes reduce to the
    /// per-axis interval test.
    pub fn overlaps(&self, other: &Self) -> bool {
        match (self.shape, other.shape) {
            (ProbeShape::Sphere, _) => {
                other.volume_distance_sq(self.position) < self.radius.powi(2)
            }
            (_, ProbeShape::Sphere) => {
                self.volume_distance_sq(other.position) < other.radius.powi(2)
            }
            _ => boxes_overlap(&self.oriented(), &other.oriented()),
        }
    }

    /// The share of the world-aligned box `lo`..`hi` inside this probe's volume
    /// (#1995), 0 to 1: exact for a world-aligned box, else sampled at
    /// [`SHARE_SAMPLES`]³ cell centres. An empty box reads 0.
    pub fn share_of(&self, lo: Vec3, hi: Vec3) -> f32 {
        let volume = |a: Vec3, b: Vec3| (b - a).max(Vec3::ZERO).element_product();
        let own = volume(lo, hi);
        if own <= 0.0 {
            return 0.0;
        }
        if self.shape == ProbeShape::Box && self.rotation == Quat::IDENTITY {
            return volume(self.box_min.max(lo), self.box_max.min(hi)) / own;
        }
        let n = SHARE_SAMPLES;
        let cell = (hi - lo) / n as f32;
        let at = |i: usize| i as f32 + 0.5;
        let inside = (0..n * n * n)
            .filter(|&k| {
                let idx = Vec3::new(at(k % n), at(k / n % n), at(k / (n * n)));
                self.contains(lo + cell * idx)
            })
            .count();
        inside as f32 / (n * n * n) as f32
    }

    /// The box as (world centre, its three world axes, half-extents).
    fn oriented(&self) -> (Vec3, [Vec3; 3], Vec3) {
        let mid = (self.box_min + self.box_max) * 0.5;
        let centre = self.position + self.rotation * (mid - self.position);
        let axes = [Vec3::X, Vec3::Y, Vec3::Z].map(|a| self.rotation * a);
        (centre, axes, (self.box_max - self.box_min).abs() * 0.5)
    }
}

/// The separating-axis test of two oriented boxes: overlapping (more than
/// touching) unless one of the 15 candidate axes separates them.
fn boxes_overlap(a: &(Vec3, [Vec3; 3], Vec3), b: &(Vec3, [Vec3; 3], Vec3)) -> bool {
    let reach = |(_, axes, half): &(Vec3, [Vec3; 3], Vec3), l: Vec3| {
        (0..3).map(|i| half[i] * axes[i].dot(l).abs()).sum::<f32>()
    };
    let gap = b.0 - a.0;
    let separates =
        |l: Vec3| l.length_squared() > 1e-10 && gap.dot(l).abs() >= reach(a, l) + reach(b, l);
    let crossed = (0..9).map(|k| a.1[k / 3].cross(b.1[k % 3]));
    !(a.1.into_iter().chain(b.1).chain(crossed)).any(separates)
}
