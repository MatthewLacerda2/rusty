use super::history::{TaaHistory, FLICKER_FORMAT, SURFACE_FORMAT};
use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::{sampler_entry, texture_entry, uniform_entry};
use crate::gpu::ops::CountedDevice as _;
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::HDR_FORMAT;

pub(crate) struct TaaPasses {
    layout: wgpu::BindGroupLayout,
    held_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pub(super) resolve: LazyPipeline,
    pub(super) upscale: LazyPipeline,
    pub(super) resolve_held: LazyPipeline,
    pub(super) upscale_held: LazyPipeline,
    pub(crate) sharpen: super::Sharpen,
}

fn targets(held: bool) -> Vec<Option<wgpu::ColorTargetState>> {
    let target = |format| {
        Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })
    };
    let mut out = vec![target(HDR_FORMAT), target(SURFACE_FORMAT)];
    if held {
        out.push(target(FLICKER_FORMAT));
    }
    out
}

fn layout(device: &wgpu::Device, held: bool) -> wgpu::BindGroupLayout {
    let mut entries = vec![
        uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
        texture_entry(1, wgpu::TextureViewDimension::D2),
        texture_entry(2, wgpu::TextureViewDimension::D2),
        texture_entry(3, wgpu::TextureViewDimension::D2),
        sampler_entry(4),
        texture_entry(5, wgpu::TextureViewDimension::D2),
        texture_entry(6, wgpu::TextureViewDimension::D2),
    ];
    if held {
        entries.push(texture_entry(7, wgpu::TextureViewDimension::D2));
    }
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(if held {
            "TAA Held Layout"
        } else {
            "TAA Layout"
        }),
        entries: &entries,
    })
}

impl TaaPasses {
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        output_formats: &[wgpu::TextureFormat],
    ) -> Self {
        let shader = registry.load(device, "postfx/taa.wgsl", "TAA Shader");
        let (plain, held) = (layout(device, false), layout(device, true));
        let pipeline = |fs: &'static str, label: &'static str, held: Option<&_>| {
            let group = held.unwrap_or(&plain);
            let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("TAA Pipeline Layout"),
                bind_group_layouts: &[Some(group)],
                immediate_size: 0,
            });
            let (shader, targets) = (shader.clone(), targets(held.is_some()));
            LazyPipeline::new(device, move |device| {
                Fullscreen {
                    label,
                    shader: &shader,
                    layout: &layout,
                    vs: "vs_fullscreen",
                    fs,
                }
                .colour(device, &targets)
            })
        };
        let resolve = pipeline("fs_resolve", "TAA Resolve", None);
        let upscale = pipeline("fs_upscale", "TAA Upscale", None);
        let resolve_held = pipeline("fs_resolve_held", "TAA Resolve Held", Some(&held));
        let upscale_held = pipeline("fs_upscale_held", "TAA Upscale Held", Some(&held));
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("TAA History Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            layout: plain,
            held_layout: held,
            sampler,
            resolve,
            upscale,
            resolve_held,
            upscale_held,
            sharpen: super::Sharpen::new(device, registry, output_formats),
        }
    }

    pub(super) fn group(
        &self,
        device: &wgpu::Device,
        h: &TaaHistory,
        read: usize,
        current: &wgpu::TextureView,
        velocity: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        let view = wgpu::BindingResource::TextureView;
        let mut entries = vec![
            entry(0, h.params.as_entire_binding()),
            entry(1, view(current)),
            entry(2, view(&h.frames[read].view)),
            entry(3, view(velocity)),
            entry(4, wgpu::BindingResource::Sampler(&self.sampler)),
            entry(5, view(&h.surfaces[read].view)),
            entry(6, view(&h.reactive.view)),
        ];
        if let Some(flickers) = &h.flickers {
            entries.push(entry(7, view(&flickers[read].view)));
        }
        let layout = match h.flickers {
            Some(_) => &self.held_layout,
            None => &self.layout,
        };
        device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Bind Group"),
            layout,
            entries: &entries,
        })
    }

    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        let resolves = [&self.resolve, &self.upscale];
        resolves
            .into_iter()
            .chain([&self.resolve_held, &self.upscale_held])
            .chain(self.sharpen.lazy_pipelines())
    }
}
