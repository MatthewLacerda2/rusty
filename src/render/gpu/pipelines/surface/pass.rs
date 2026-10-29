//! Which solids pass a batch draws in, and the pipeline it draws with there: its
//! material's surface variant (#396) or the standard one, over a primed depth or not
//! (#1499).

use super::super::{PrimedPipelines, TransparentBlend, TransparentPipelines, VelocityPipelines};
use super::SurfaceShaders;

/// Which solids pass a batch is drawn in, carrying that pass's standard pipeline, so
/// [`SurfaceShaders::pick`] can swap in a material's surface variant (#396).
///
/// The opaque passes carry the standard primed pipelines when the view's depth is
/// primed (#1499): the SSAO prepass already wrote every lit solid's depth, so those
/// draw through a pipeline that tests it and writes none. A batch the prepass leaves
/// out (`BatchKey::in_prepass`: an unlit solid) keeps the depth-writing pipeline.
#[derive(Clone, Copy)]
pub(crate) enum SolidPass<'a> {
    /// The SSAO depth prepass; the standard pipeline unless a variant cuts, moves its
    /// UVs or moves its vertices (#648, #1499, #1524).
    Prepass(&'a wgpu::RenderPipeline),
    /// Opaque solids; the standard pipeline is the view's (the preview may swap it).
    Opaque(&'a wgpu::RenderPipeline, Option<&'a PrimedPipelines>),
    /// Opaque solids writing motion vectors too (#1435), and the SSR surface target
    /// when the last field holds (#1526).
    Velocity(&'a VelocityPipelines, Option<&'a PrimedPipelines>, bool),
    /// Blended solids; each standard pipeline is built only if a batch uses it.
    Transparent(&'a TransparentPipelines),
}

impl SolidPass<'_> {
    /// Whether this is the SSAO prepass, which draws only the batches
    /// [`BatchKey::in_prepass`] admits.
    ///
    /// [`BatchKey::in_prepass`]: crate::render::draw::batch::BatchKey::in_prepass
    pub(crate) fn is_prepass(self) -> bool {
        matches!(self, Self::Prepass(_))
    }

    /// Whether this pass draws over a primed depth (#1499).
    pub(crate) fn primed(self) -> bool {
        matches!(
            self,
            Self::Opaque(_, Some(_)) | Self::Velocity(_, Some(_), _)
        )
    }
}

impl SurfaceShaders {
    /// What pipeline `id` draws with in `pass`: its variant's, else the standard one;
    /// with `primed` (a lit batch in a primed pass, #1499), the one over the primed
    /// depth; in the transparent pass, the one for its `blend` (#1523).
    pub(crate) fn pick<'a>(
        &'a self,
        pass: SolidPass<'a>,
        id: usize,
        primed: bool,
        blend: TransparentBlend,
    ) -> &'a wgpu::RenderPipeline {
        let variant = self.get(id);
        match pass {
            SolidPass::Prepass(standard) => self.cut(id).map_or(standard, |c| &c.prepass),
            SolidPass::Opaque(_, Some(p)) if primed => {
                variant.map_or(&p.forward, |v| &v.primed.forward).get()
            }
            SolidPass::Opaque(standard, _) => variant.map_or(standard, |v| &v.forward),
            SolidPass::Velocity(_, Some(p), ssr) if primed => {
                variant.map_or(&p.velocity, |v| &v.primed.velocity).get(ssr)
            }
            SolidPass::Velocity(standard, _, ssr) => {
                variant.map_or(standard, |v| &v.velocity).get(ssr)
            }
            SolidPass::Transparent(standard) => {
                variant.map_or(standard, |v| &v.transparent).get(blend)
            }
        }
    }
}
