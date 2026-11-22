use super::cache;
use super::chain::BloomTargets;
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::{PfxTarget, PostFx, HDR_FORMAT};
use crate::timing::{GpuPass, GpuTimer};

pub(crate) struct BloomPasses {
    layout: wgpu::BindGroupLayout,
    prefilter: LazyPipeline,
    resample: LazyPipeline,
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
            resample: pass("fs_resample"),
            downsample: pass("fs_downsample"),
            upsample: pass("fs_upsample"),
            layout,
            dirt: cache::DirtCache::default(),
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 4] {
        [
            &self.prefilter,
            &self.resample,
            &self.downsample,
            &self.upsample,
        ]
    }

    /// `high` is the upsample's own mip; the other passes bind `low` there again, never a target.
    fn group(
        &self,
        device: &wgpu::Device,
        (params, sampler): (&wgpu::Buffer, &wgpu::Sampler),
        low: &wgpu::TextureView,
        high: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Bind Group"),
            layout: &self.layout,
            entries: &[
                entry(0, params.as_entire_binding()),
                entry(1, wgpu::BindingResource::TextureView(low)),
                entry(2, wgpu::BindingResource::Sampler(sampler)),
                entry(3, wgpu::BindingResource::TextureView(high)),
            ],
        })
    }

    pub(super) fn chain_groups(
        &self,
        device: &wgpu::Device,
        (pre, down, up): (Option<&PfxTarget>, &[PfxTarget], &[PfxTarget]),
        params: &wgpu::Buffer,
        sampler: &wgpu::Sampler,
    ) -> Vec<wgpu::BindGroup> {
        let bound = (params, sampler);
        let mut groups = Vec::with_capacity(1 + down.len() + up.len());
        if let Some(pre) = pre {
            groups.push(self.group(device, bound, &pre.view, &pre.view));
        }
        for pair in down.windows(2) {
            groups.push(self.group(device, bound, &pair[0].view, &pair[0].view));
        }
        for i in (0..up.len()).rev() {
            let low = up.get(i + 1).unwrap_or(&down[i + 1]);
            groups.push(self.group(device, bound, &low.view, &down[i].view));
        }
        groups
    }

    pub(super) fn encode(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        t: &BloomTargets,
        (input, sampler): (&wgpu::TextureView, &wgpu::Sampler),
        timer: &GpuTimer,
    ) {
        let prefilter = self.group(device, (&t.params, sampler), input, input);
        let mut steps = vec![(&self.prefilter, &prefilter, t.prefilter_target())];
        let mut groups = &t.groups[..];
        if t.pre.is_some() {
            steps.push((&self.resample, &groups[0], &t.down[0]));
            groups = &groups[1..];
        }
        let (down_groups, up_groups) = groups.split_at(t.down.len() - 1);
        for (group, target) in down_groups.iter().zip(&t.down[1..]) {
            steps.push((&self.downsample, group, target));
        }
        for (group, i) in up_groups.iter().zip((0..t.up.len()).rev()) {
            steps.push((&self.upsample, group, &t.up[i]));
        }
        for (pipeline, bind_group, target) in steps {
            let timed = (pipeline.get(), timer.writes(GpuPass::PostFx));
            PostFx::fullscreen(encoder, timed, &[bind_group], &target.view);
        }
    }
}
