//! The translucent solids' pipelines (#242, #1523): one per blend state a material can
//! ask for, each built on its first draw. Unity URP's blend table
//! (`BaseShaderGUI.SetupMaterialBlendModeInternal`): a material's `BlendMode` sets the
//! factors, and *Preserve Specular Lighting* lifts the alpha multiply from the blend
//! into the shader (source factor `One`) under `Alpha` and `Additive`.

use super::LazyPipeline;
use crate::render::gpu::uniforms::SurfaceUniform;

/// The blend state a translucent batch draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransparentBlend {
    /// `Alpha`: `src * a + dst * (1 - a)`.
    Straight,
    /// `Premultiply`, or `Alpha` preserving specular: `src + dst * (1 - a)`.
    Premultiplied,
    /// `Additive`: `src * a + dst`.
    Additive,
    /// `Additive` preserving specular: `src + dst`.
    AdditivePremultiplied,
    /// `Multiply`: `dst * src`; the destination alpha is kept.
    Multiply,
}

impl TransparentBlend {
    /// Every state, in [`TransparentPipelines`]' order.
    pub(crate) const ALL: [Self; 5] = [
        Self::Straight,
        Self::Premultiplied,
        Self::Additive,
        Self::AdditivePremultiplied,
        Self::Multiply,
    ];

    /// The state a draw's surface terms ask for (its `BlendMode` index and whether
    /// the shader premultiplied its diffuse).
    pub(crate) fn of(surface: &SurfaceUniform) -> Self {
        let premultiplied = surface.premultiply_diffuse == 1;
        match (surface.blend_mode, premultiplied) {
            (1, _) | (0, true) => Self::Premultiplied,
            (2, false) => Self::Additive,
            (2, true) => Self::AdditivePremultiplied,
            (3, _) => Self::Multiply,
            _ => Self::Straight,
        }
    }

    /// The wgpu blend state.
    pub(crate) fn state(self) -> wgpu::BlendState {
        use wgpu::{BlendComponent as C, BlendFactor as F, BlendOperation::Add};
        let c = |src_factor, dst_factor| C {
            src_factor,
            dst_factor,
            operation: Add,
        };
        let (color, alpha) = match self {
            Self::Straight => (
                c(F::SrcAlpha, F::OneMinusSrcAlpha),
                c(F::One, F::OneMinusSrcAlpha),
            ),
            Self::Premultiplied => (
                c(F::One, F::OneMinusSrcAlpha),
                c(F::One, F::OneMinusSrcAlpha),
            ),
            Self::Additive => (c(F::SrcAlpha, F::One), c(F::One, F::One)),
            Self::AdditivePremultiplied => (c(F::One, F::One), c(F::One, F::One)),
            Self::Multiply => (c(F::Dst, F::Zero), c(F::Zero, F::One)),
        };
        wgpu::BlendState { color, alpha }
    }

    /// The pipeline's label.
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Straight => "Transparent Pipeline",
            Self::Premultiplied => "Transparent Premultiplied Pipeline",
            Self::Additive => "Transparent Additive Pipeline",
            Self::AdditivePremultiplied => "Transparent Additive Premultiplied Pipeline",
            Self::Multiply => "Transparent Multiply Pipeline",
        }
    }
}

/// One shader's translucent pipelines, one per [`TransparentBlend`], each built on
/// its first draw.
pub(crate) struct TransparentPipelines([LazyPipeline; 5]);

impl TransparentPipelines {
    /// The set `make(blend)` builds.
    pub(super) fn new(make: impl Fn(TransparentBlend) -> LazyPipeline) -> Self {
        Self(TransparentBlend::ALL.map(make))
    }

    /// The pipeline for `blend`.
    pub(crate) fn get(&self, blend: TransparentBlend) -> &wgpu::RenderPipeline {
        self.0[blend as usize].get()
    }

    /// Whether `blend`'s pipeline has been built.
    #[cfg(test)]
    pub(crate) fn is_built(&self, blend: TransparentBlend) -> bool {
        self.0[blend as usize].is_built()
    }

    /// Every one, for warming.
    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn surface(blend_mode: u32, premultiply_diffuse: u32) -> SurfaceUniform {
        SurfaceUniform {
            blend_mode,
            premultiply_diffuse,
            ..SurfaceUniform::default()
        }
    }

    #[test]
    fn a_material_picks_urps_blend_state() {
        use TransparentBlend as B;
        assert_eq!(B::of(&surface(0, 0)), B::Straight);
        assert_eq!(B::of(&surface(0, 1)), B::Premultiplied, "preserve specular");
        assert_eq!(B::of(&surface(1, 0)), B::Premultiplied);
        assert_eq!(B::of(&surface(2, 0)), B::Additive);
        assert_eq!(B::of(&surface(2, 1)), B::AdditivePremultiplied);
        assert_eq!(B::of(&surface(3, 0)), B::Multiply);
        assert_eq!(B::Straight.state(), wgpu::BlendState::ALPHA_BLENDING);
        let premultiplied = wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING;
        assert_eq!(B::Premultiplied.state(), premultiplied);
    }

    #[test]
    fn the_set_is_indexed_in_all_order() {
        for (i, blend) in TransparentBlend::ALL.into_iter().enumerate() {
            assert_eq!(blend as usize, i);
        }
    }
}
