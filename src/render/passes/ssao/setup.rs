//! Construction of the renderer-wide SSAO resources: the bind-group layout, the
//! occlusion and blur pipelines, and the white no-AO fallback.

use super::{SsaoRenderer, AO_FORMAT};
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::timing::{GpuPass, GpuTimer};

impl SsaoRenderer {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &mut ShaderRegistry,
    ) -> Self {
        let layout = create_layout(device);
        let shader = registry.load(device, "ssao.wgsl", "SSAO Shader");
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SSAO Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |label, entry_point| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_fullscreen"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                fragment: Some(wgpu::FragmentState {
                    compilation_options: Default::default(),
                    module: &shader,
                    entry_point,
                    targets: &[Some(AO_FORMAT.into())],
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let (no_ao_texture, no_ao) = white_texture(device, queue);
        Self {
            ao_pipeline: pipeline("SSAO Occlusion Pipeline", Some("fs_ao")),
            blur_pipeline: pipeline("SSAO Blur Pipeline", Some("fs_blur")),
            layout,
            _no_ao_texture: no_ao_texture,
            no_ao,
        }
    }

    /// Record one fullscreen pass of `pipeline` over `group` into `target`, timed.
    pub(super) fn fullscreen(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        blur: bool,
        (group, target): (&wgpu::BindGroup, &wgpu::TextureView),
        timer: &GpuTimer,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            multiview_mask: None,
            label: Some(if blur { "SSAO Blur Pass" } else { "SSAO Pass" }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view: target,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: timer.writes(GpuPass::Ssao),
            occlusion_query_set: None,
        });
        let pipeline = if blur {
            &self.blur_pipeline
        } else {
            &self.ao_pipeline
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// A bind group over the SSAO layout: the view's uniform, its prepass depth, and
    /// `raw` — the occlusion the blur reads (the white fallback for the AO pass).
    pub(super) fn group(
        &self,
        device: &wgpu::Device,
        uniform: &wgpu::Buffer,
        depth: &wgpu::TextureView,
        raw: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSAO Bind Group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(depth),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(raw),
                },
            ],
        })
    }
}

/// Uniform (both stages' params), the prepass depth, and the raw occlusion — all read
/// with `textureLoad`, so no sampler.
fn create_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let texture = |binding, sample_type| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type,
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    };
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("SSAO Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            texture(1, wgpu::TextureSampleType::Depth),
            texture(2, wgpu::TextureSampleType::Float { filterable: true }),
        ],
    })
}

/// The 1x1 white AO texture: "nothing occluded".
fn white_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> (wgpu::Texture, wgpu::TextureView) {
    let size = wgpu::Extent3d {
        width: 1,
        height: 1,
        depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSAO No-AO Fallback"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: AO_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texture.as_image_copy(),
        &[255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(1),
            rows_per_image: None,
        },
        size,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
