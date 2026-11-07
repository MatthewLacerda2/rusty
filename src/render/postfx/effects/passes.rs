//! The effects' shared pipelines, one set per renderer: every view's pass runs
//! through them, so a new view or render-texture slot compiles nothing. An effect
//! with a pass of its own adds its pipelines here, and its lazy ones to
//! [`Passes::lazy_pipelines`] (`Renderer::lazy_pipelines` warms them), its lazy
//! compute ones to [`Passes::lazy_computes`].

use super::{
    bloom, depth_of_field, exposure, lens_flare, motion_blur, ssil, ssr, taa, volumetric_fog,
};
use crate::render::gpu::pipelines::{LazyCompute, LazyPipeline};
use crate::render::gpu::shaders::ShaderRegistry;

/// Every effect's shared pipelines.
pub(crate) struct Passes {
    // sorted: one field per effect with a pass of its own
    pub bloom: bloom::BloomPasses,
    pub depth_of_field: depth_of_field::DofPasses,
    pub exposure: exposure::ExposurePasses,
    pub lens_flare: lens_flare::FlarePasses,
    pub motion_blur: motion_blur::MotionBlurPasses,
    pub ssil: ssil::SsilPasses,
    pub ssr: ssr::SsrPasses,
    pub taa: taa::TaaPasses,
    pub volumetric_fog: volumetric_fog::VolumetricFogPasses,
}

impl Passes {
    /// Every effect's pipelines; an LDR pass's into each of `output_formats` (the
    /// window's and the offscreen views').
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        output_formats: &[wgpu::TextureFormat],
    ) -> Self {
        Self {
            bloom: bloom::BloomPasses::new(device, registry),
            depth_of_field: depth_of_field::DofPasses::new(device, registry),
            exposure: exposure::ExposurePasses::new(device, registry),
            lens_flare: lens_flare::FlarePasses::new(device, registry),
            motion_blur: motion_blur::MotionBlurPasses::new(device, registry),
            ssil: ssil::SsilPasses::new(device, registry),
            ssr: ssr::SsrPasses::new(device, registry),
            taa: taa::TaaPasses::new(device, registry, output_formats),
            volumetric_fog: volumetric_fog::VolumetricFogPasses::new(device, registry),
        }
    }

    /// The pipelines built on first use, for warming.
    pub(crate) fn lazy_pipelines(&self) -> Vec<&LazyPipeline> {
        let mut all = Vec::new();
        all.extend(self.bloom.lazy_pipelines());
        all.extend(self.depth_of_field.lazy_pipelines());
        all.extend(self.lens_flare.lazy_pipelines());
        all.extend(self.motion_blur.lazy_pipelines());
        all.extend(self.ssil.lazy_pipelines());
        all.extend(self.ssr.lazy_pipelines());
        all.extend(self.taa.lazy_pipelines());
        all
    }

    /// The compute pipelines built on first dispatch, for warming.
    pub(crate) fn lazy_computes(&self) -> Vec<&LazyCompute> {
        self.volumetric_fog.lazy_computes().to_vec()
    }
}
