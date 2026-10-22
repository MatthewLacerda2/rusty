//! The renderer-wide half of the selection outline: its mask and edge pipelines,
//! built on first use (#985), and the edge pass's bind-group layout. The mask target
//! is per view ([`super::SelectionMask`]).

use super::{EdgeUniform, MASK_FORMAT};
use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::{texture_entry, uniform_entry};
use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::postfx::HDR_FORMAT;

/// The selection outline's two pipelines and the edge pass's layout.
pub(crate) struct OutlinePasses {
    /// The outlined draws into the mask, through the forward pipeline layout.
    pub(crate) mask: LazyPipeline,
    /// The fullscreen ring over the HDR target, alpha-blended.
    pub(crate) edge: LazyPipeline,
    /// The edge pass's group 0: its [`EdgeUniform`] and the mask.
    pub(super) edge_layout: wgpu::BindGroupLayout,
}

impl OutlinePasses {
    /// The outline's pipelines, unbuilt. The mask binds the forward pass's own groups,
    /// so it is built through the forward pipeline `forward_layout`.
    pub(crate) fn new(
        device: &wgpu::Device,
        registry: &mut ShaderRegistry,
        forward_layout: &wgpu::PipelineLayout,
    ) -> Self {
        let mask_shader = registry.load(device, "outline/mask.wgsl", "Selection Mask Shader");
        let edge_shader = registry.load(device, "outline/edge.wgsl", "Selection Edge Shader");
        let edge_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Selection Edge Layout"),
            entries: &[
                uniform_entry(0, wgpu::ShaderStages::FRAGMENT),
                texture_entry(1, wgpu::TextureViewDimension::D2),
            ],
        });
        let edge_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Selection Edge Pipeline Layout"),
            bind_group_layouts: &[Some(&edge_layout)],
            immediate_size: 0,
        });
        let forward_layout = forward_layout.clone();
        let mask = LazyPipeline::new(device, move |device| {
            mask_pipeline(device, &mask_shader, &forward_layout)
        });
        let edge = LazyPipeline::new(device, move |device| {
            let target = wgpu::ColorTargetState {
                format: HDR_FORMAT,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            };
            Fullscreen {
                label: "Selection Edge Pipeline",
                shader: &edge_shader,
                layout: &edge_pipeline_layout,
                vs: "vs_fullscreen",
                fs: "fs_edge",
            }
            .colour(device, &[Some(target)])
        });
        Self {
            mask,
            edge,
            edge_layout,
        }
    }

    /// Both pipelines, for `Renderer::lazy_pipelines`.
    pub(crate) fn lazy_pipelines(&self) -> [&LazyPipeline; 2] {
        [&self.mask, &self.edge]
    }

    /// The edge pass's bind group over `uniform` and the `mask` view.
    pub(super) fn edge_group(
        &self,
        device: &wgpu::Device,
        uniform: &wgpu::Buffer,
        mask: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Selection Edge Bind Group"),
            layout: &self.edge_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(mask),
                },
            ],
        })
    }
}

/// The size of the edge pass's uniform buffer.
pub(super) const EDGE_UNIFORM_SIZE: u64 = std::mem::size_of::<EdgeUniform>() as u64;

/// The mask pipeline: the forward vertex layout and groups, every triangle (no
/// culling, so mirroring cannot hide one), no depth test, into the one-channel mask.
fn mask_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Selection Mask Pipeline"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_mask"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some("fs_mask"),
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: MASK_FORMAT,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
