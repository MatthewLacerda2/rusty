use super::{MaterialAsset, RenderMode, UvMapping};

/// The deepest a height map reaches, as a fraction of one repeat's width.
pub const HEIGHT_SCALE_MAX: f32 = 0.2;

/// The fewest and most steps a pixel-displacement march takes: HDRP Lit's slider limits.
pub const HEIGHT_STEPS_MIN: u32 = 1;
pub const HEIGHT_STEPS_MAX: u32 = 64;

pub(super) fn default_height_scale() -> f32 {
    0.02
}

/// HDRP Lit's `_PPDMinSamples` and `_PPDMaxSamples` defaults.
pub(super) fn default_height_min_steps() -> u32 {
    5
}

pub(super) fn default_height_max_steps() -> u32 {
    15
}

impl MaterialAsset {
    pub fn height_depth(&self) -> f32 {
        match self.height_scale.is_nan() {
            true => 0.0,
            false => self.height_scale.clamp(0.0, HEIGHT_SCALE_MAX),
        }
    }

    /// The march's `(fewest, most)` steps, each clamped to HDRP's `[1, 64]`; a scene file
    /// holding `min > max` marches `max` steps at every angle.
    pub fn height_steps(&self) -> (u32, u32) {
        let clamp = |n: u32| n.clamp(HEIGHT_STEPS_MIN, HEIGHT_STEPS_MAX);
        let most = clamp(self.height_max_steps);
        (clamp(self.height_min_steps).min(most), most)
    }

    /// Why this material's step counts may not stand, as a setter refuses them.
    pub fn height_steps_refusal(&self) -> Option<String> {
        (self.height_min_steps > self.height_max_steps).then(|| {
            format!(
                "height_min_steps ({}) is above height_max_steps ({}): raise the maximum first",
                self.height_min_steps, self.height_max_steps
            )
        })
    }

    /// A `Cutout`'s holes clip at the unshifted UV, a `Transparent` shows what is behind, and
    /// `Triplanar`'s three projections are not marched: only `Opaque` under `UV0` or `Planar`.
    pub fn height_applies(&self) -> bool {
        self.height_map.is_some()
            && self.render_mode == RenderMode::Opaque
            && self.uv_mapping != UvMapping::Triplanar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_depth_is_clamped_and_a_nan_is_none() {
        let mut m = MaterialAsset::default();
        assert_eq!(m.height_depth(), 0.02);
        for (scale, depth) in [
            (-1.0, 0.0),
            (0.5, HEIGHT_SCALE_MAX),
            (f32::NAN, 0.0),
            (0.1, 0.1),
        ] {
            m.height_scale = scale;
            assert_eq!(m.height_depth(), depth, "{scale}");
        }
    }

    #[test]
    fn the_steps_are_clamped_and_an_inverted_pair_is_refused() {
        let mut m = MaterialAsset::default();
        assert_eq!(m.height_steps(), (5, 15));
        assert_eq!(m.height_steps_refusal(), None);
        for (min, max, steps) in [(0, 0, (1, 1)), (1, 64, (1, 64)), (70, 99, (64, 64))] {
            (m.height_min_steps, m.height_max_steps) = (min, max);
            assert_eq!(m.height_steps(), steps, "{min} {max}");
        }
        (m.height_min_steps, m.height_max_steps) = (20, 10);
        assert_eq!(m.height_steps(), (10, 10));
        assert!(m.height_steps_refusal().is_some());
        m.height_min_steps = 10;
        assert_eq!(m.height_steps_refusal(), None, "equal is allowed");
    }

    #[test]
    fn only_an_opaque_uv0_or_planar_material_with_a_map_is_displaced() {
        let mut m = MaterialAsset::default();
        assert!(!m.height_applies());
        m.height_map = Some("h.png".into());
        assert!(m.height_applies());
        m.uv_mapping = UvMapping::Planar;
        assert!(m.height_applies());
        m.uv_mapping = UvMapping::Triplanar;
        assert!(!m.height_applies());
        m.uv_mapping = UvMapping::Uv0;
        for mode in [RenderMode::Cutout, RenderMode::Transparent] {
            m.render_mode = mode;
            assert!(!m.height_applies(), "{mode:?}");
        }
    }
}
