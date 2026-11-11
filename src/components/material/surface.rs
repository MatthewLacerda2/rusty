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
        /// separate pass after opaque, sorted by `sorting_priority` then back to front
        /// (#2039; glass, holograms, fades).
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

/// A dielectric's reflectance head-on, F0, by glTF's `KHR_materials_ior` and
/// `KHR_materials_specular` (#1998): `min(((ior - 1) / (ior + 1))² × color, 1) ×
/// specular`, per channel. The one place F0 is computed: every reader in the
/// forward shader (the lamps, the lightmap's lobe, the environment BRDF and so a
/// transparent surface's opacity) takes it from the record, mixed toward a metal's
/// albedo by `metallic` there. Worked in `f64`, so glass's default 1.5 is exactly
/// `0.04_f32`, the constant this replaced, and every material that sets neither
/// field renders as before. A hand-written scene skips the field table's clamps,
/// so each input is clamped here: an `ior` below 1 or NaN reads as 1 (an infinite
/// one is a mirror), a `specular` as its nearest bound in `[0, 1]` (NaN as 0), and
/// a colour channel below 0 or NaN as 0.
pub fn dielectric_f0(ior: f32, specular: f32, color: [f32; 3]) -> [f32; 3] {
    let n = within(ior, 1.0, f64::MAX);
    let reflectance = ((n - 1.0) / (n + 1.0)).powi(2);
    let strength = within(specular, 0.0, 1.0);
    color.map(|c| ((reflectance * within(c, 0.0, f64::MAX)).min(1.0) * strength) as f32)
}

/// A dielectric's reflectance at grazing angles, F90, by `KHR_materials_specular`
/// (#1998): its `specular`, clamped as [`dielectric_f0`] clamps it. 1 by default, so
/// a material that sets no `specular` keeps Schlick's full rim; 0 takes a
/// dielectric's reflections away at every angle. A metal's F90 is 1, mixed in by
/// `metallic` in the shader.
pub fn dielectric_f90(specular: f32) -> f32 {
    within(specular, 0.0, 1.0) as f32
}

/// `v` in `[lo, hi]`, a NaN at `lo`.
fn within(v: f32, lo: f64, hi: f64) -> f64 {
    if v.is_nan() {
        lo
    } else {
        f64::from(v).clamp(lo, hi)
    }
}

impl super::MaterialAsset {
    /// This material's dielectric F0 (`surface::dielectric_f0` of its `ior`, `specular`
    /// and `specular_color`).
    pub fn dielectric_f0(&self) -> [f32; 3] {
        dielectric_f0(self.ior, self.specular, self.specular_color)
    }

    /// This material's dielectric F90 (`surface::dielectric_f90` of its `specular`).
    pub fn dielectric_f90(&self) -> f32 {
        dielectric_f90(self.specular)
    }

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

#[cfg(test)]
#[path = "f0_tests.rs"]
mod f0_tests;
