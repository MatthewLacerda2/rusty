//! Bloom's three passes (#1700), shared by every view: the prefilter, the
//! downsample and the upsample of `postfx/bloom.wgsl`, and how one view's chain
//! is encoded with them.

use super::cache;
use super::chain::BloomTargets;
use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::{PostFx, HDR_FORMAT};
use crate::render::timing::{GpuPass, GpuTimer};

/// The chain's three passes, shared by every view, and the lens dirt textures the
/// volumes name.
pub(crate) struct BloomPasses {
    layout: wgpu::BindGroupLayout,
    prefilter: LazyPipeline,
    downsample: LazyPipeline,
    upsample: LazyPipeline,
    pub(crate) dirt: cache::DirtCache,
}

impl BloomPasses {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/bloom.wgsl", "Bloom Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bloom Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
                sampler_entry(2),
                texture_entry(3, wgpu::TextureViewDimension::D2),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Bloom Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pass = |fs: &'static str| {
            let (shader, layout) = (shader.clone(), pipeline_layout.clone());
            LazyPipeline::new(device, move |device| {
                let label = format!("Bloom {fs}");
                let target = wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                };
                Fullscreen {
                    label: &label,
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs,
                }
                .colour(device, &[Some(target)])
            })
        };
        Self {
            prefilter: pass("fs_prefilter"),
            downsample: pass("fs_downsample"),
            upsample: pass("fs_upsample"),
            layout,
            dirt: cache::DirtCache::default(),
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 3] {
        [&self.prefilter, &self.downsample, &self.upsample]
    }
}

impl BloomPasses {
    /// Encode `t`'s chain from `input` (the scene, and the sampler every pass reads
    /// with) into `encoder`, after `t.params` was written: the prefilter (`input`
    /// -> `down[0]`), each downsample (`down[i - 1]` -> `down[i]`), then each
    /// upsample from the bottom (`up[i + 1]`, or the last down mip, over `down[i]`
    /// -> `up[i]`).
    pub(super) fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        t: &BloomTargets,
        (input, sampler): (&wgpu::TextureView, &wgpu::Sampler),
        timer: &GpuTimer,
    ) {
        // `high` is the upsample's own mip; the other passes bind their input there
        // again, never the target they write.
        let group = |low: &wgpu::TextureView, high: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom Bind Group"),
                layout: &self.layout,
                entries: &[
                    entry(0, t.params.as_entire_binding()),
                    entry(1, wgpu::BindingResource::TextureView(low)),
                    entry(2, wgpu::BindingResource::Sampler(sampler)),
                    entry(3, wgpu::BindingResource::TextureView(high)),
                ],
            })
        };
        let mut steps = vec![(&self.prefilter, group(input, input), &t.down[0])];
        for pair in t.down.windows(2) {
            let (above, below) = (&pair[0].view, &pair[1]);
            steps.push((&self.downsample, group(above, above), below));
        }
        for i in (0..t.up.len()).rev() {
            let low = t.up.get(i + 1).unwrap_or(&t.down[i + 1]);
            let bind = group(&low.view, &t.down[i].view);
            steps.push((&self.upsample, bind, &t.up[i]));
        }
        for (pipeline, bind_group, target) in &steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::PostFx));
            PostFx::fullscreen(encoder, timed, &[bind_group], &target.view);
        }
    }
}
