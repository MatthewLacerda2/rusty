use super::{FacedPipeline, Facing, LazyPipeline};
use crate::gpu::uniforms::SurfaceUniform;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
    /// In discriminant order: `TransparentPipelines` indexes by `blend as usize`.
    pub(crate) const ALL: [Self; 5] = [
        Self::Straight,
        Self::Premultiplied,
        Self::Additive,
        Self::AdditivePremultiplied,
        Self::Multiply,
    ];

    /// URP's blend table; `blend_mode` is `BlendMode`'s index: Alpha, Premultiply, Additive,
    /// Multiply.
    /// A surface over the opaque copy composites what is behind it itself and writes alpha 1, which
    /// the premultiplied state lays down whole.
    pub(crate) fn of(surface: &SurfaceUniform) -> Self {
        let premultiplied = surface.premultiply_diffuse == 1 || surface.uses_opaque == 1;
        match (surface.blend_mode, premultiplied) {
            (1, _) | (0, true) => Self::Premultiplied,
            (2, false) => Self::Additive,
            (2, true) => Self::AdditivePremultiplied,
            (3, _) => Self::Multiply,
            _ => Self::Straight,
        }
    }

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

pub(crate) struct TransparentPipelines {
    plain: [FacedPipeline; 5],
    reactive: [FacedPipeline; 5],
    /// SSR's *Mixed* surface prepass (`fs_ssr_transparent`).
    ssr_surface: FacedPipeline,
}

impl TransparentPipelines {
    pub(super) fn new(
        make: impl Fn(TransparentBlend, bool) -> FacedPipeline,
        ssr_surface: FacedPipeline,
    ) -> Self {
        Self {
            plain: TransparentBlend::ALL.map(|b| make(b, false)),
            reactive: TransparentBlend::ALL.map(|b| make(b, true)),
            ssr_surface,
        }
    }

    pub(crate) fn ssr_surface(&self, facing: Facing) -> &wgpu::RenderPipeline {
        self.ssr_surface.get(facing)
    }

    pub(crate) fn get(
        &self,
        blend: TransparentBlend,
        facing: Facing,
        reactive: bool,
    ) -> &wgpu::RenderPipeline {
        let set = if reactive {
            &self.reactive
        } else {
            &self.plain
        };
        set[blend as usize].get(facing)
    }

    #[cfg(test)]
    pub(crate) fn is_built(&self, blend: TransparentBlend) -> bool {
        let built = |set: &[FacedPipeline; 5]| {
            Facing::ALL
                .into_iter()
                .any(|f| set[blend as usize].is_built(f))
        };
        built(&self.plain) || built(&self.reactive)
    }

    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        let all = self.plain.iter().chain(&self.reactive);
        let all = all.chain(std::iter::once(&self.ssr_surface));
        all.flat_map(FacedPipeline::lazies)
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
