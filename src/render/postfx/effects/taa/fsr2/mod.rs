//! src/render/postfx/effects/taa/fsr2/ — AMD FidelityFX Super Resolution 2.2.1 as
//! TAA's alternative (#1848): the upscaler a screen camera with TAA on runs when
//! the graphics settings name `fsr2` (`core::settings::upscaler`).
//!
//! **Where it runs: TAA's place** (decision 2 of #1848, HDRP's and Unity's shared
//! upscaler interface): in TAA's scene pass, instead of TAA's resolve and TAAU's
//! upscale, at any render scale (1 included, FSR's native anti-aliasing); the rest
//! of the chain runs at the output's size after it, as under TAAU, and #1847's
//! contrast-adaptive sharpen runs after the composite on a frame it ran (FSR2's own
//! RCAS pass is not ported). A view that never scales (the Scene view, a preview,
//! a render texture) keeps TAA, and so does a supersampled one (a render scale
//! above 1): FSR2 only upscales. [`super::history::mode`] decides.
//!
//! **The port** (`engine/shaders/postfx/fsr2/`, AMD's MIT notice in its
//! `LICENSE.txt`): FSR2's fp32, non-wave permutation, its five dispatches in one
//! compute pass timed as `fsr2`: the luminance pyramid's shading-change level
//! (`luma.wgsl`), reconstruct-and-dilate, depth clip, lock and accumulate.
//! What core WGSL lacks is split or moved: SPD's single dispatch with a global
//! atomic becomes a direct reduction of the one level a later pass reads, FSR2's
//! `r32ui` atomic image and `r8` read-write image are storage buffers
//! ([`targets`]), and FSR2's 16-bit two-channel images are four-channel ones.
//!
//! **Inputs** (decision 5): the jitter is TAA's (FSR2's Halton sequence, with
//! FSR2's phase count and mip bias, [`params`]), the motion is the velocity
//! target's, the depth is that target's view depth (`postfx/fsr2/common.wgsl` says
//! why), the reactive mask is #1845's, and the exposure is the engine's: the
//! volume's fixed EV, or the meter's adapted one a frame late.

mod layouts;
pub(crate) mod params;
mod run;
pub(crate) mod targets;

pub(crate) use run::Fsr2Inputs;
pub(crate) use targets::Fsr2Targets;

use crate::render::gpu::pipelines::LazyCompute;
use crate::render::gpu::shaders::ShaderRegistry;

/// FSR2's pipelines, shared by every view: one per pass, built on first use.
pub(crate) struct Fsr2Passes {
    layouts: layouts::Layouts,
    sampler: wgpu::Sampler,
    luma: LazyCompute,
    reconstruct: LazyCompute,
    depth_clip: LazyCompute,
    lock: LazyCompute,
    accumulate: LazyCompute,
}

impl Fsr2Passes {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let layouts = layouts::Layouts::new(device);
        let mut pass = |file: &str, entry: &'static str, group: &wgpu::BindGroupLayout| {
            let path = format!("postfx/fsr2/{file}.wgsl");
            let module = registry.load(device, &path, entry);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(entry),
                bind_group_layouts: &[Some(&layouts.shared), Some(group)],
                immediate_size: 0,
            });
            LazyCompute::new(device, move |device| {
                device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                    label: Some(entry),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some(entry),
                    compilation_options: Default::default(),
                    cache: None,
                })
            })
        };
        let luma = pass("luma", "cs_luma", &layouts.luma);
        let reconstruct = pass("reconstruct", "cs_reconstruct", &layouts.reconstruct);
        let depth_clip = pass("depth_clip", "cs_depth_clip", &layouts.depth_clip);
        let lock = pass("lock", "cs_lock", &layouts.lock);
        let accumulate = pass("accumulate", "cs_accumulate", &layouts.accumulate);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("FSR2 Linear Clamp"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            layouts,
            sampler,
            luma,
            reconstruct,
            depth_clip,
            lock,
            accumulate,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_computes`).
    pub(crate) fn lazy_computes(&self) -> [&LazyCompute; 5] {
        [
            &self.luma,
            &self.reconstruct,
            &self.depth_clip,
            &self.lock,
            &self.accumulate,
        ]
    }
}

#[cfg(test)]
mod gpu_tests;
