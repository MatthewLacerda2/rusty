//! WebGPU copies and clears depth only as whole subresources, so a tile is copied or cleared by
//! drawing its depth: `Always`, no bias, so a copy is bit-identical.

use crate::gpu::fullscreen::Fullscreen;
use crate::gpu::layout_entries::depth_entry;
use crate::gpu::ops::CountedDevice as _;

pub(super) struct TileBlit {
    copy: wgpu::RenderPipeline,
    clear: wgpu::RenderPipeline,
    source: wgpu::BindGroup,
}

impl TileBlit {
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        static_view: &wgpu::TextureView,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Atlas Blit Layout"),
            entries: &[depth_entry(0, wgpu::TextureViewDimension::D2)],
        });
        let source = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Atlas Blit Source"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(static_view),
            }],
        });
        let copy = pipeline(device, shader, &[&layout], "fs_copy");
        // The clear binds nothing: it draws into the static atlas the copy samples, and a pass may
        // not do both to one texture.
        let clear = pipeline(device, shader, &[], "fs_clear");
        Self {
            copy,
            clear,
            source,
        }
    }

    pub(super) fn copy<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.copy);
        pass.set_bind_group(crate::gpu::ONLY_GROUP, &self.source, &[]);
        pass.draw(0..3, 0..1);
    }

    pub(super) fn clear<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.clear);
        pass.draw(0..3, 0..1);
    }
}

fn pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layouts: &[&wgpu::BindGroupLayout],
    fs_entry: &str,
) -> wgpu::RenderPipeline {
    let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Shadow Atlas Blit Pipeline Layout"),
        bind_group_layouts: &layouts.iter().copied().map(Some).collect::<Vec<_>>(),
        immediate_size: 0,
    });
    let pass = Fullscreen {
        label: "Shadow Atlas Blit Pipeline",
        shader,
        layout: &layout,
        vs: "vs_fullscreen",
        fs: fs_entry,
    };
    pass.depth_only(
        device,
        wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        },
    )
}
