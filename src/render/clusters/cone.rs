//! A spotlight's cone for the binner (#1712): a cluster lists a spot only when its
//! cone may reach the cluster's box, not merely its range sphere, as Unity's
//! Forward+ and Doom 2016 bin spots.

use glam::{Mat4, Vec3};

/// The widest spotlight half-angle (degrees) the binner culls by its cone (#1712).
/// Wider ones are binned by their range sphere: near 90° the cone barely trims it,
/// and the bounding shortcuts lose their margin.
pub(crate) const WIDEST_CULLED_CONE: f32 = 80.0;

/// What the binner tests for one light: a bounding sphere, and a cone that trims it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub sphere: (Vec3, f32),
    pub cone: Option<Cone>,
}

/// A spotlight's range-capped cone (#1712): where its light can land at all.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cone {
    pub apex: Vec3,
    /// Unit direction it points.
    pub axis: Vec3,
    pub range: f32,
    /// Cosine and sine of the outer half-angle, at most [`WIDEST_CULLED_CONE`].
    pub cos: f32,
    pub sin: f32,
}

impl Cone {
    /// Whether the cone may reach the box `lo..hi` (conservative): the box meets
    /// the cone's range sphere and [`Self::may_touch`] passes its bounding sphere.
    /// The range test matters for a wide cone, whose own bounding sphere bulges
    /// past its range.
    pub(crate) fn may_reach_box(&self, lo: Vec3, hi: Vec3) -> bool {
        self.apex.clamp(lo, hi).distance_squared(self.apex) <= self.range * self.range
            && self.may_touch((lo + hi) * 0.5, (hi - lo).length() * 0.5)
    }

    /// The cone moved by the rigid `view` transform.
    pub(crate) fn transformed(&self, view: Mat4) -> Self {
        Self {
            apex: view.transform_point3(self.apex),
            axis: view.transform_vector3(self.axis).normalize_or_zero(),
            ..*self
        }
    }

    /// Whether the cone may reach the sphere at `center` of `radius`: `false` only
    /// when it surely misses (conservative: a sphere it touches always passes).
    /// The test of Wronski's *Cull that cone*: in the plane through the axis and
    /// the centre, the centre's distance to the cone's side line, past its tip or
    /// behind its apex. A cluster box passes its own bounding sphere.
    pub(crate) fn may_touch(&self, center: Vec3, radius: f32) -> bool {
        let v = center - self.apex;
        let along = v.dot(self.axis);
        let across = (v.length_squared() - along * along).max(0.0).sqrt();
        // A little slack, so rounding never opens a hole at a cone's edge.
        let r = radius * 1.001 + 1e-3;
        let beside = self.cos * across - self.sin * along;
        !(beside > r || along > self.range + r || along < -r)
    }
}
