//! Drawing a texture's mip chain on the GPU (#1322), `mips.wgsl`: each level is a
//! fullscreen triangle sampling the level above through the views of the texture's
//! [`TextureKind`].
//!
//! Done on the GPU because the CPU box filter it replaced cost about 42 ms per 2K
//! texture in an optimised build and 670 ms in the unoptimised dev build the editor
//! runs (the engine crate is opt-level 0 there), on top of the image's decode.

use super::TextureKind;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{sampler_entry, texture_entry};
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;

/// The downsample pass: one pipeline per view format. The colour one is built
/// with the renderer, whose default checker draws its mips with it; the data one
/// waits for the first data map.
pub(crate) struct MipGenerator {
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    colour: wgpu::RenderPipeline,
    data: LazyPipeline,
}

impl MipGenerator {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "mips.wgsl", "Mip Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mip Layout"),
            entries: &[
                texture_entry(0, wgpu::TextureViewDimension::D2),
                sampler_entry(1),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mip Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |kind: TextureKind| {
            let (shader, pipeline_layout) = (shader.clone(), pipeline_layout.clone());
            move |device: &wgpu::Device| {
                let pass = Fullscreen {
                    label: "Mip Pipeline",
                    shader: &shader,
                    layout: &pipeline_layout,
                    vs: "vs_fullscreen",
                    fs: "fs_downsample",
                };
                pass.colour(device, &[Some(kind.view_format().into())])
            }
        };
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Mip Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            colour: pipeline(TextureKind::Colour)(device),
            data: LazyPipeline::new(device, pipeline(TextureKind::Data)),
            layout,
            sampler,
        }
    }

    /// The data pipeline, for the windowed renderer to build before its first frame.
    pub(crate) fn lazy_pipeline(&self) -> &LazyPipeline {
        &self.data
    }

    /// Draw every level of `texture` below its base from the one above; nothing
    /// for a texture of one level. The texture needs `RENDER_ATTACHMENT` usage and
    /// `kind`'s view format.
    pub(crate) fn generate(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        kind: TextureKind,
    ) {
        if texture.mip_level_count() == 1 {
            return;
        }
        let pipeline = match kind {
            TextureKind::Colour => &self.colour,
            TextureKind::Data => self.data.get(),
        };
        let level = |mip| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(kind.view_format()),
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            })
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        for mip in 1..texture.mip_level_count() {
            self.draw_level(
                device,
                &mut encoder,
                pipeline,
                [&level(mip - 1), &level(mip)],
            );
        }
        queue.submit([encoder.finish()]);
    }

    /// Record the pass drawing `target` from `above`.
    fn draw_level(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        [above, target]: [&wgpu::TextureView; 2],
    ) {
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mip Source"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(above),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Mip Level"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
