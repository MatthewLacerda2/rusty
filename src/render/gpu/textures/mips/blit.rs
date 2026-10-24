//! Drawing a texture's mip chain on the GPU (#1322), `mips.wgsl`: each level is a
//! fullscreen triangle box-filtering the level above (`textureLoad`, odd sides
//! weighted by overlap) through the views of the texture's [`TextureKind`].
//!
//! Done on the GPU because the CPU box filter it replaced cost about 42 ms per 2K
//! texture in an optimised build and 670 ms in the unoptimised dev build the editor
//! runs (the engine crate is opt-level 0 there), on top of the image's decode.

use super::TextureKind;
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::texture_entry;
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;

/// The downsample pass: one pipeline per view format. The colour one is built
/// with the renderer, whose default checker draws its mips with it; the data one
/// waits for the first data map.
pub(crate) struct MipGenerator {
    layout: wgpu::BindGroupLayout,
    colour: wgpu::RenderPipeline,
    data: LazyPipeline,
}

impl MipGenerator {
    pub(crate) fn new(device: &wgpu::Device, registry: &mut ShaderRegistry) -> Self {
        let shader = registry.load(device, "mips.wgsl", "Mip Shader");
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mip Layout"),
            entries: &[texture_entry(0, wgpu::TextureViewDimension::D2)],
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
        Self {
            colour: pipeline(TextureKind::Colour)(device),
            data: LazyPipeline::new(device, pipeline(TextureKind::Data)),
            layout,
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
        self.generate_layer(device, queue, (texture, 0), kind);
    }

    /// [`generate`](Self::generate) for one array `layer` of `texture` (the decal
    /// atlas's, #1364): each level is read and drawn through views of that layer
    /// alone.
    pub(crate) fn generate_layer(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        (texture, layer): (&wgpu::Texture, u32),
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
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_mip_level: mip,
                mip_level_count: Some(1),
                base_array_layer: layer,
                array_layer_count: Some(1),
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
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(above),
            }],
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
