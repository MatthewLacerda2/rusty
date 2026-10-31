//! How a material is composited (its `RenderMode`, #242) and how a `Transparent`
//! one composites over what is behind it (#1523): Unity URP Lit's *Blending Mode*
//! and *Preserve Specular Lighting*, the *Receive Shadows* toggle every surface has,
//! and HDRP Lit's refraction and absorption.

crate::core::named_enum! {
    /// How a material's surface is composited — Unity's "Rendering Mode" (#242). Plain
    /// serde data, mirroring [`ParticleBlend`](crate::components::ParticleBlend) as prior art for a
    /// blend selector. `Opaque` is the default and the fast path.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RenderMode {
        /// Fully opaque (`REPLACE`, depth write on). The default; rides the solids pass.
        #[default]
        Opaque = "Opaque",
        /// Alpha-tested: the fragment is `discard`ed below `alpha_cutoff`, else fully
        /// opaque. Writes depth, needs no sorting, so it also rides the solids pass
        /// (foliage, chain-link, grates).
        Cutout = "Cutout",
        /// Alpha-blended (`ALPHA_BLENDING`), depth test on / write off, drawn in a
        /// separate back-to-front-sorted pass after opaque (glass, holograms, fades).
        Transparent = "Transparent",
    }
}

crate::core::named_enum! {
    /// Which faces of a mesh a material draws — URP Lit's *Render Face* (`_Cull`,
    /// #1615), HDRP's *Double-Sided*. Every pass that draws the surface culls by it:
    /// the forward and transparent passes, the depth prepass and postpass, the shadow
    /// casters and the selection outline.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RenderFace {
        /// The faces turned toward the viewer; back faces are culled. The default,
        /// for opaque, cutout and transparent alike, so a closed mesh never shows its
        /// inside.
        #[default]
        Front = "Front",
        /// Only the faces turned away (the inside of a dome or a sky sphere), shaded
        /// with their normal flipped toward the viewer.
        Back = "Back",
        /// Both, each shaded from the side it is seen (foliage cards, a glass pane, a
        /// plane seen from below).
        Both = "Both",
    }
}

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

crate::core::named_enum! {
    /// How a transparent surface bends what is behind it — HDRP Lit's *Refraction
    /// Model* (`Refraction.hlsl`), leaving out its Sphere model.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum RefractionModel {
        /// No bending. The default.
        #[default]
        None = "None",
        /// The surface is a slab `thickness` thick (water, a thick pane): HDRP's
        /// *Planar* (its code's `RefractionModelBox`).
        Planar = "Planar",
        /// A thin shell (a bubble, a window): HDRP's *Thin*, a fixed 5 mm slab, so it
        /// barely bends.
        Thin = "Thin",
    }
}

pub(super) fn default_ior() -> f32 {
    1.5
}

pub(super) fn default_one() -> f32 {
    1.0
}

pub(super) fn white() -> [f32; 3] {
    [1.0; 3]
}

impl super::MaterialAsset {
    /// Whether this material's surface reads the opaque copy (#1523): a
    /// `Transparent` one that refracts or absorbs, under `Alpha` or `Premultiply`
    /// (an `Additive` or `Multiply` surface adds to or tints what is behind it, so it
    /// has nothing to bend or absorb).
    pub fn reads_opaque(&self) -> bool {
        let blends_over = matches!(self.blend_mode, BlendMode::Alpha | BlendMode::Premultiply);
        let refracts = self.refraction_model != RefractionModel::None;
        let absorbs = self.transmittance_color != white();
        self.is_transparent() && blends_over && (refracts || absorbs)
    }
}
