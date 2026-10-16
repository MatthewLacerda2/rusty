//! Copying and clearing one atlas tile by drawing (#694), `shadow_atlas.wgsl`.
//!
//! WebGPU copies depth only as whole subresources and `LoadOp::Clear` clears the
//! whole attachment, so neither works on a tile. Instead a fullscreen triangle is
//! drawn into the tile's viewport and writes `frag_depth`: the static atlas's texel
//! for a copy, the far plane for a clear. Depth passes straight through (`Always`,
//! no bias), so a copied tile is bit-identical to the static atlas it came from.

use crate::render::gpu::fullscreen::Fullscreen;
use crate::render::gpu::layout_entries::depth_entry;

/// The two tile blits and the static atlas the copy reads.
pub(super) struct TileBlit {
    copy: wgpu::RenderPipeline,
    clear: wgpu::RenderPipeline,
    /// The static atlas, as `fs_copy` reads it.
    source: wgpu::BindGroup,
}

impl TileBlit {
    /// Both blits over `shader`, the copy reading `static_view`.
    pub(super) fn new(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        static_view: &wgpu::TextureView,
    ) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Atlas Blit Layout"),
            entries: &[depth_entry(0, wgpu::TextureViewDimension::D2)],
        });
        let source = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Shadow Atlas Blit Source"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(static_view),
            }],
        });
        // The clear binds nothing: it draws into the static atlas, which the copy's
        // group samples, and a pass may not do both to one texture.
        let copy = pipeline(device, shader, &[&layout], "fs_copy");
        let clear = pipeline(device, shader, &[], "fs_clear");
        Self {
            copy,
            clear,
            source,
        }
    }

    /// Copy the static atlas into the pass's current viewport.
    pub(super) fn copy<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.copy);
        pass.set_bind_group(0, &self.source, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Reset the pass's current viewport to the far plane.
    pub(super) fn clear<'a>(&'a self, pass: &mut wgpu::RenderPass<'a>) {
        pass.set_pipeline(&self.clear);
        pass.draw(0..3, 0..1);
    }
}

/// A depth-only fullscreen pipeline writing `fs_entry`'s `frag_depth` unconditionally.
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
