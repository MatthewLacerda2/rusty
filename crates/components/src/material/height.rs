//! A material's height map (#2191), HDRP Lit's *Displacement Mode: Pixel*: parallax
//! occlusion mapping, which shifts where every map is read so relief holds at a
//! grazing angle. The surface's depth, outline and shadows stay flat.

use super::{MaterialAsset, RenderMode, UvMapping};

/// The deepest a height map reaches, as a fraction of one repeat's width.
pub const HEIGHT_SCALE_MAX: f32 = 0.2;

pub(super) fn default_height_scale() -> f32 {
    0.02
}

impl MaterialAsset {
    /// Its `height_scale` as the renderer reads it: in `[0, HEIGHT_SCALE_MAX]`, a
    /// hand-written NaN read as 0 (no displacement).
    pub fn height_depth(&self) -> f32 {
        match self.height_scale.is_nan() {
            true => 0.0,
            false => self.height_scale.clamp(0.0, HEIGHT_SCALE_MAX),
        }
    }

    /// Whether its height map displaces the surface: one is named, and the material
    /// is `Opaque` (a `Cutout`'s holes are clipped at the unshifted UV, a
    /// `Transparent` one shows what is behind it) under `UV0` or `Planar`
    /// (`Triplanar`'s three projections are not marched).
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
