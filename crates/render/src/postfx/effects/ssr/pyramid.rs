use crate::gpu::bind_entry as entry;
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::texture_entry;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::postfx::HDR_FORMAT;
use crate::timing::{GpuPass, GpuTimer};

pub(crate) fn levels((w, h): (u32, u32)) -> u32 {
    u32::BITS - w.max(h).max(1).leading_zeros()
}

// Mirrors `BLUR_FADE_END` in `lighting/ssr_march.wgsl`.
pub(crate) const BLUR_FADE_END: f32 = 0.12;

const LOD_SLACK: f32 = 1.0 + 1.0 / 64.0;

pub(crate) fn hit_levels((w, h): (u32, u32)) -> u32 {
    let cap = BLUR_FADE_END * w.min(h).max(1) as f32;
    let deepest = (cap * LOD_SLACK).log2().ceil().max(0.0) as u32;
    (deepest + 1).min(levels((w, h)))
}

pub(crate) struct PyramidPass {
    layout: wgpu::BindGroupLayout,
    downsample: LazyPipeline,
    pub(crate) trilinear: wgpu::Sampler,
}

impl PyramidPass {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr_pyramid.wgsl", "SSR Pyramid Shader");
        let d2 = wgpu::TextureViewDimension::D2;
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Pyramid Layout"),
            entries: &[texture_entry(0, d2), texture_entry(1, d2)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Pyramid Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let downsample = LazyPipeline::new(device, move |device| {
            let target = wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            };
            Fullscreen {
                label: "SSR Pyramid Pipeline",
                shader: &shader,
                layout: &pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_downsample",
            }
            .colour(device, &[Some(target.clone()), Some(target)])
        });
        let trilinear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSR Pyramid Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            layout,
            downsample,
            trilinear,
        }
    }

    pub(crate) fn lazy_pipeline(&self) -> &LazyPipeline {
        &self.downsample
    }
}

fn texture(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
    levels: u32,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR_FORMAT,
        usage,
        view_formats: &[],
    })
}

pub(crate) fn colour_copy(device: &wgpu::Device, size: (u32, u32)) -> wgpu::Texture {
    use wgpu::TextureUsages as U;
    texture(
        device,
        "SSR Colour",
        size,
        1,
        U::TEXTURE_BINDING | U::COPY_DST,
    )
}

// Render targets only, never storage: storage-capable colour loses framebuffer compression.
struct Chain {
    view: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
}

impl Chain {
    fn new(device: &wgpu::Device, label: &str, size: (u32, u32), count: u32) -> Self {
        use wgpu::TextureUsages as U;
        let usage = U::RENDER_ATTACHMENT | U::TEXTURE_BINDING;
        let texture = texture(device, label, size, count, usage);
        let levels = (0..count)
            .map(|mip| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: mip,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self {
            view: texture.create_view(&Default::default()),
            levels,
        }
    }
}

pub(crate) struct HitPyramids {
    hits: Chain,
    spread: Chain,
    steps: Vec<wgpu::BindGroup>,
}

impl HitPyramids {
    pub(crate) fn new(device: &wgpu::Device, pass: &PyramidPass, size: (u32, u32)) -> Self {
        let count = hit_levels(size);
        let hits = Chain::new(device, "SSR Hits", size, count);
        let spread = Chain::new(device, "SSR Spread", size, count);
        let view = |v| wgpu::BindingResource::TextureView(v);
        let steps = (1..count as usize)
            .map(|mip| {
                device.counted_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SSR Pyramid Level"),
                    layout: &pass.layout,
                    entries: &[
                        entry(0, view(&hits.levels[mip - 1])),
                        entry(1, view(&spread.levels[mip - 1])),
                    ],
                })
            })
            .collect();
        Self {
            hits,
            spread,
            steps,
        }
    }

    pub(crate) fn tops(&self) -> [&wgpu::TextureView; 2] {
        [&self.hits.levels[0], &self.spread.levels[0]]
    }

    pub(crate) fn views(&self) -> [&wgpu::TextureView; 2] {
        [&self.hits.view, &self.spread.view]
    }

    #[cfg(test)]
    pub(crate) fn levels(&self) -> u32 {
        self.hits.levels.len() as u32
    }

    pub(crate) fn build(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        pass: &PyramidPass,
        timer: &GpuTimer,
    ) {
        let Some(last) = self.steps.len().checked_sub(1) else {
            return;
        };
        let (mut open, mut close) = match last {
            0 => (timer.writes(GpuPass::SsrPyramids), None),
            _ => timer.render_span(GpuPass::SsrPyramids),
        };
        for (i, group) in self.steps.iter().enumerate() {
            let mip = i + 1;
            let targets = [&self.hits.levels[mip], &self.spread.levels[mip]].map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })
            });
            let stamps = if i == 0 { open.take() } else { None };
            let stamps = stamps.or_else(|| if i == last { close.take() } else { None });
            let mut rp = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SSR Pyramid Pass"),
                color_attachments: &targets,
                depth_stencil_attachment: None,
                timestamp_writes: stamps,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pass.downsample.get());
            rp.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{hit_levels, levels, BLUR_FADE_END};

    #[test]
    fn blur_fade_end_mirrors_the_shader() {
        let wgsl = include_str!("../../../../../../engine/shaders/lighting/ssr_march.wgsl");
        let line = format!("const BLUR_FADE_END: f32 = {BLUR_FADE_END};");
        assert!(wgsl.contains(&line), "ssr_march.wgsl must declare `{line}`");
    }

    #[test]
    fn the_hit_pyramids_stop_at_the_deepest_level_the_resolve_reads() {
        assert_eq!(hit_levels((1920, 1080)), 9);
        assert_eq!(hit_levels((40, 30)), 3);
        assert_eq!(hit_levels((96, 72)), 5);
        assert_eq!(hit_levels((1, 1)), 1);
        assert_eq!(hit_levels((0, 0)), 1);
        for size in [(4000, 4000), (64, 2), (2, 64)] {
            assert!(hit_levels(size) <= levels(size));
        }
    }
}
