use super::{view_format, TextureKind};
use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::texture_entry;
use crate::gpu::ops::{CountedDevice as _, CountedEncoder as _, CountedQueue as _};
use crate::gpu::pipelines::LazyPipeline;
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::textures::deep::DEEP_FORMAT;

/// On the GPU: the editor's dev build compiles this crate at opt-level 0, where a CPU box
/// filter is slow.
pub(crate) struct MipGenerator {
    layout: wgpu::BindGroupLayout,
    colour: wgpu::RenderPipeline,
    data: LazyPipeline,
    deep: LazyPipeline,
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
        let pipeline = |format: wgpu::TextureFormat| {
            let (shader, pipeline_layout) = (shader.clone(), pipeline_layout.clone());
            move |device: &wgpu::Device| {
                let pass = Fullscreen {
                    label: "Mip Pipeline",
                    shader: &shader,
                    layout: &pipeline_layout,
                    vs: "vs_fullscreen",
                    fs: "fs_downsample",
                };
                pass.colour(device, &[Some(format.into())])
            }
        };
        Self {
            colour: pipeline(view_format(TextureKind::Colour))(device),
            data: LazyPipeline::new(device, pipeline(view_format(TextureKind::Data))),
            deep: LazyPipeline::new(device, pipeline(DEEP_FORMAT)),
            layout,
        }
    }

    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.data, &self.deep]
    }

    /// `texture` needs `RENDER_ATTACHMENT` usage and `kind`'s view format, or [`DEEP_FORMAT`].
    pub(crate) fn generate(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        texture: &wgpu::Texture,
        kind: TextureKind,
    ) {
        self.generate_layer(device, queue, (texture, 0), kind);
    }

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
        let deep = texture.format() == DEEP_FORMAT;
        let (pipeline, format) = match kind {
            _ if deep => (self.deep.get(), DEEP_FORMAT),
            TextureKind::Colour => (&self.colour, view_format(kind)),
            _ => (self.data.get(), view_format(kind)),
        };
        let level = |mip| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                format: Some(format),
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
        queue.counted_submit([encoder.finish()]);
    }

    fn draw_level(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        pipeline: &wgpu::RenderPipeline,
        [above, target]: [&wgpu::TextureView; 2],
    ) {
        let group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mip Source"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(above),
            }],
        });
        let mut pass = encoder.counted_render_pass(&wgpu::RenderPassDescriptor {
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
        pass.set_bind_group(crate::gpu::ONLY_GROUP, &group, &[]);
        pass.draw(0..3, 0..1);
    }
}
