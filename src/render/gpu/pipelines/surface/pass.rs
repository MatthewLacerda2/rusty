//! Which solids pass a batch draws in, and the pipeline it draws with there: its
//! material's surface variant (#396) or the standard one, over a primed depth or not
//! (#1499).

use super::super::{LazyPipeline, PrimedPipelines};
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
    /// The SSAO depth prepass; the standard pipeline unless a variant cuts or moves
    /// its UVs (#648, #1499).
    Prepass(&'a wgpu::RenderPipeline),
    /// Opaque solids; the standard pipeline is the view's (the preview may swap it).
    Opaque(&'a wgpu::RenderPipeline, Option<&'a PrimedPipelines>),
    /// Opaque solids writing motion vectors too (#1435).
    Velocity(&'a LazyPipeline, Option<&'a PrimedPipelines>),
    /// Alpha-blended solids; the standard pipeline is built only if a batch uses it.
    Transparent(&'a LazyPipeline),
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
        matches!(self, Self::Opaque(_, Some(_)) | Self::Velocity(_, Some(_)))
    }
}

impl SurfaceShaders {
    /// What pipeline `id` draws with in `pass`: its variant's, else the standard one;
    /// with `primed` (a lit batch in a primed pass, #1499), the one over the primed
    /// depth.
    pub(crate) fn pick<'a>(
        &'a self,
        pass: SolidPass<'a>,
        id: usize,
        primed: bool,
    ) -> &'a wgpu::RenderPipeline {
        let variant = self.get(id);
        match pass {
            SolidPass::Prepass(standard) => self.cut(id).map_or(standard, |c| &c.prepass),
            SolidPass::Opaque(_, Some(p)) if primed => {
                variant.map_or(&p.forward, |v| &v.primed.forward).get()
            }
            SolidPass::Opaque(standard, _) => variant.map_or(standard, |v| &v.forward),
            SolidPass::Velocity(_, Some(p)) if primed => {
                variant.map_or(&p.velocity, |v| &v.primed.velocity).get()
            }
            SolidPass::Velocity(lazy, _) => variant.map_or(lazy, |v| &v.velocity).get(),
            SolidPass::Transparent(lazy) => variant.map_or_else(|| lazy.get(), |v| &v.transparent),
        }
    }
}
