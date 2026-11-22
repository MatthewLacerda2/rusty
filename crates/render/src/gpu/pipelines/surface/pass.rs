use super::super::{
    DepthPostpassPipelines, FacedPipeline, Facing, PrepassPipelines, PrimedPipelines,
    TransparentBlend, TransparentPipelines, VelocityPipelines,
};
use super::SurfaceShaders;

/// A batch the prepass leaves out (`BatchKey::in_prepass`: an unlit solid) keeps the depth-writing pipeline.
#[derive(Clone, Copy)]
pub(crate) enum SolidPass<'a> {
    Prepass(&'a PrepassPipelines),
    Opaque(&'a FacedPipeline, Option<&'a PrimedPipelines>),
    Velocity(&'a VelocityPipelines, Option<&'a PrimedPipelines>, bool),
    Transparent(&'a TransparentPipelines, bool),
    DepthPostpass(&'a DepthPostpassPipelines, bool),
    MotionPostpass(&'a DepthPostpassPipelines),
}

impl<'a> SolidPass<'a> {
    pub(crate) fn is_prepass(self) -> bool {
        matches!(self, Self::Prepass(_))
    }

    pub(crate) fn depth_only(self, facing: Facing) -> Option<&'a wgpu::RenderPipeline> {
        match self {
            Self::Prepass(p) => Some(p.depth_only.get(facing)),
            _ => None,
        }
    }

    pub(crate) fn primed(self) -> bool {
        matches!(
            self,
            Self::Opaque(_, Some(_)) | Self::Velocity(_, Some(_), _)
        )
    }
}

impl SurfaceShaders {
    pub(crate) fn pick<'a>(
        &'a self,
        pass: SolidPass<'a>,
        id: usize,
        primed: bool,
        blend: TransparentBlend,
        facing: Facing,
    ) -> &'a wgpu::RenderPipeline {
        let variant = self.get(id);
        match pass {
            SolidPass::Prepass(standard) => self
                .cut(id)
                .map_or(&standard.clip, |c| &c.prepass)
                .get(facing),
            SolidPass::Opaque(_, Some(p)) if primed => variant
                .map_or(&p.forward, |v| &v.primed.forward)
                .get(facing),
            SolidPass::Opaque(standard, _) => variant.map_or(standard, |v| &v.forward).get(facing),
            SolidPass::Velocity(_, Some(p), ssr) if primed => variant
                .map_or(&p.velocity, |v| &v.primed.velocity)
                .get(ssr, facing),
            SolidPass::Velocity(standard, _, ssr) => {
                variant.map_or(standard, |v| &v.velocity).get(ssr, facing)
            }
            SolidPass::Transparent(standard, reactive) => variant
                .map_or(standard, |v| &v.transparent)
                .get(blend, facing, reactive),
            SolidPass::DepthPostpass(standard, velocity) => variant
                .map_or(standard, |v| &v.depth_postpass)
                .get(velocity, facing),
            SolidPass::MotionPostpass(standard) => variant
                .map_or(standard, |v| &v.depth_postpass)
                .motion(facing),
        }
    }
}
