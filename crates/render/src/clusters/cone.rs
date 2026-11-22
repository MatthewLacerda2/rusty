use glam::{Mat4, Vec3};

/// Degrees (outer half-angle); a wider spot bins by its range sphere, which its cone barely trims.
pub(crate) const WIDEST_CULLED_CONE: f32 = 80.0;

/// What the binner tests for one light: its range sphere, and for a spotlight
/// the cone that trims which clusters list it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Bounds {
    pub sphere: (Vec3, f32),
    pub cone: Option<Cone>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Cone {
    pub apex: Vec3,
    pub axis: Vec3,
    pub range: f32,
    pub cos: f32,
    pub sin: f32,
}

impl Cone {
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

    pub(crate) fn transformed(&self, view: Mat4) -> Self {
        Self {
            apex: view.transform_point3(self.apex),
            axis: view.transform_vector3(self.axis).normalize_or_zero(),
            ..*self
        }
    }

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
