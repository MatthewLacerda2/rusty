//! A spotlight's cone for the binner (#1712): a cluster lists a spot only when its
//! cone may reach the cluster's box, not merely its range sphere, as Unity's
//! Forward+ and Doom 2016 bin spots.

use glam::{Mat4, Vec3};

/// The widest spotlight half-angle (degrees) the binner culls by its cone (#1712).
/// Wider ones are binned by their range sphere: near 90° the cone barely trims it,
/// and the bounding shortcuts lose their margin.
pub(crate) const WIDEST_CULLED_CONE: f32 = 80.0;

/// What the binner tests for one light: its range sphere, and for a spotlight
/// the cone that trims which clusters list it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    /// Centre and radius: a light's range sphere. The per-camera cut ranks and
    /// keeps by it, and a cluster's score measures its distance from the centre.
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
    /// The smallest sphere around the range-capped cone (centre, radius), always
    /// smaller than the range sphere for a cone no wider than
    /// [`WIDEST_CULLED_CONE`]: up to 45° the sphere through the apex and the rim,
    /// past it the one around the rim circle (which then holds the apex and the
    /// cap too). It only narrows which clusters the binner visits.
    pub(crate) fn bounding_sphere(&self) -> (Vec3, f32) {
        if self.cos >= std::f32::consts::FRAC_1_SQRT_2 {
            let r = self.range / (2.0 * self.cos);
            (self.apex + self.axis * r, r)
        } else {
            (
                self.apex + self.axis * self.range * self.cos,
                self.range * self.sin,
            )
        }
    }

    /// Whether the cone may reach the box `lo..hi` (conservative): [`Self::may_touch`]
    /// on the box's bounding sphere. The binner tests the range sphere itself.
    pub(crate) fn may_touch_box(&self, lo: Vec3, hi: Vec3) -> bool {
        self.may_touch((lo + hi) * 0.5, (hi - lo).length() * 0.5)
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
