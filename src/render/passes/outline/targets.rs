//! A view's selection mask (#1175): the one-channel target the outlined draws fill,
//! the edge pass's uniform, and its bind group over both. Allocated the first frame
//! a view draws an outline and rebuilt only when the view's size changes, so a view
//! that never outlines (the game, a render texture) never pays for it.

use super::setup::{OutlinePasses, EDGE_UNIFORM_SIZE};
use super::MASK_FORMAT;

/// One view's selection mask and the edge pass's state over it.
pub(crate) struct SelectionMask {
    /// `(width, height)` this was built for.
    size: (u32, u32),
    _texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) uniform: wgpu::Buffer,
    pub(super) edge_group: wgpu::BindGroup,
}

impl SelectionMask {
    /// Is this the mask for a `width` x `height` view?
    pub(super) fn matches(&self, width: u32, height: u32) -> bool {
        self.size == (width, height)
    }

    pub(super) fn new(
        device: &wgpu::Device,
        passes: &OutlinePasses,
        width: u32,
        height: u32,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Selection Mask"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: MASK_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Selection Edge Uniform"),
            size: EDGE_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            size: (width, height),
            edge_group: passes.edge_group(device, &uniform, &view),
            _texture: texture,
            view,
            uniform,
        }
    }
}
