//! Where a material's maps are read on the surface (#1573): Unity Lit's *Tiling*
//! and *Offset*, and HDRP Lit's *Base UV Mapping*, which can project the maps from
//! world space so a scaled primitive keeps one texel density on every face.

crate::core::named_enum! {
    /// Which coordinates the main maps (base colour, metallic, roughness, normal,
    /// emissive) are read at — HDRP Lit's *Base UV Mapping*, leaving out its UV1–UV3.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum UvMapping {
        /// The mesh's first UV set. The default.
        #[default]
        Uv0 = "UV0",
        /// Projected straight down the world Y axis (floors, terrain): the maps tile
        /// over world X and Z, and stretch on walls.
        Planar = "Planar",
        /// Projected along each world axis and blended by the surface normal: every
        /// face of any mesh, at any scale, shows the texture at the same size.
        Triplanar = "Triplanar",
    }
}

pub(super) fn default_tiling() -> [f32; 2] {
    [1.0, 1.0]
}

pub(super) fn default_blend_sharpness() -> f32 {
    4.0
}
