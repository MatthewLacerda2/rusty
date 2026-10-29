//! How a `Transparent` material composites over what is behind it (#1523): Unity
//! URP Lit's *Blending Mode* and *Preserve Specular Lighting*, and the *Receive
//! Shadows* toggle every surface has.

crate::core::named_enum! {
    /// How a transparent surface's colour combines with the image behind it — URP
    /// Lit's *Blending Mode* (`BaseShaderGUI.SetupMaterialBlendModeInternal`). Read
    /// only by a `Transparent` material.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum BlendMode {
        /// Classic alpha blending: `alpha` of the surface over `1 - alpha` of what is
        /// behind. The default.
        #[default]
        Alpha = "Alpha",
        /// The colour is already premultiplied: it is added whole and covers `alpha`
        /// of what is behind. Never preserves specular (it already does, by design).
        Premultiply = "Premultiply",
        /// Added onto what is behind, scaled by `alpha` (glows, holograms).
        Additive = "Additive",
        /// Multiplies what is behind (stained glass, tinted film); `alpha` fades the
        /// tint toward white. Never preserves specular.
        Multiply = "Multiply",
    }
}

impl BlendMode {
    /// Whether URP's *Preserve Specular Lighting* applies under this mode: it is
    /// skipped for `Premultiply` and `Multiply`, as URP skips it.
    pub fn preserves_specular(self) -> bool {
        matches!(self, Self::Alpha | Self::Additive)
    }
}
