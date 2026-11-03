//! SSR's colour pyramid (#1736): the copy of the opaque image the march reads, with
//! every mip down to 1×1, each the 2×2 box average of the one above
//! (`postfx/ssr_pyramid.wgsl`). A rough surface reads its hit at the level whose
//! texels are its GGX cone's on-screen width (`lighting/ssr_march.wgsl`'s `trace`),
//! so it reflects a blur of what is on screen: cone-traced SSR, needing no history.
//!
//! It is the view's, held only while it runs SSR and remade at a new size
//! ([`super::pass::SsrTargets`]); the mips below the first add a third of the
//! copy's memory (at 1080p, 5.5 MB over the copy's 16.6 MB, `Rgba16Float`), and
//! building them costs one fullscreen pass per level, untimed (the timer's pairs are
//! few). Built from the same opaque image the march read before, so no translucent or
//! overlay gets in.

use crate::render::gpu::bind_entry as entry;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::HDR_FORMAT;

/// How many levels a `(w, h)` pyramid has: down to 1×1.
pub(crate) fn levels((w, h): (u32, u32)) -> u32 {
    u32::BITS - w.max(h).max(1).leading_zeros()
}

/// The downsample pipeline and the trilinear sampler the march reads the pyramid
/// through, shared by every view.
pub(crate) struct PyramidPass {
    layout: wgpu::BindGroupLayout,
    pipeline: LazyPipeline,
    /// Linear between texels and between levels, clamped at the screen's edge.
    pub(crate) trilinear: wgpu::Sampler,
}

impl PyramidPass {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "postfx/ssr_pyramid.wgsl", "SSR Pyramid Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SSR Pyramid Layout"),
            entries: &[
                texture_entry(0, wgpu::TextureViewDimension::D2),
                sampler_entry(1),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSR Pyramid Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = LazyPipeline::new(device, move |device| {
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
            .colour(device, &[Some(target)])
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
            pipeline,
            trilinear,
        }
    }

    pub(crate) fn lazy_pipeline(&self) -> &LazyPipeline {
        &self.pipeline
    }
}

/// One view's pyramid: the texture, a view of every level (what the march reads),
/// and for each level below the first its target view and the group reading the
/// level above it.
pub(crate) struct Pyramid {
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    steps: Vec<(wgpu::TextureView, wgpu::BindGroup)>,
}

impl Pyramid {
    /// A pyramid of `size`, its levels read through `linear` (one level each).
    pub(crate) fn new(
        device: &wgpu::Device,
        pass: &PyramidPass,
        linear: &wgpu::Sampler,
        (width, height): (u32, u32),
    ) -> Self {
        use wgpu::TextureUsages as U;
        let count = levels((width, height));
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("SSR Pyramid"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_FORMAT,
            usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_DST,
            view_formats: &[],
        });
        let level = |mip| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let steps = (1..count)
            .map(|mip| {
                let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("SSR Pyramid Bind Group"),
                    layout: &pass.layout,
                    entries: &[
                        entry(0, wgpu::BindingResource::TextureView(&level(mip - 1))),
                        entry(1, wgpu::BindingResource::Sampler(linear)),
                    ],
                });
                (level(mip), group)
            })
            .collect();
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        Self {
            texture,
            view,
            steps,
        }
    }

    /// How many levels it holds.
    #[cfg(test)]
    pub(crate) fn levels(&self) -> u32 {
        self.texture.mip_level_count()
    }

    /// Record every level below the first from the one above, in order.
    pub(crate) fn build(&self, encoder: &mut wgpu::CommandEncoder, pass: &PyramidPass) {
        for (target, group) in &self.steps {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SSR Pyramid Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            rp.set_pipeline(pass.pipeline.get());
            rp.set_bind_group(0, group, &[]);
            rp.draw(0..3, 0..1);
        }
    }
}
