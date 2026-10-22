//! A view's selection-outline targets (#1175): the one-channel mask the outlined
//! draws fill, the row distances the dilation writes, the edge uniform, and the two
//! bind groups over them. Allocated the first frame a view draws an outline and
//! rebuilt only when the view's size changes, so a view that never outlines (the
//! game, a render texture) never pays for them.

use super::setup::{OutlinePasses, EDGE_UNIFORM_SIZE};
use super::MASK_FORMAT;

/// One view's selection mask and the dilation and edge passes' state over it.
pub(crate) struct SelectionMask {
    /// `(width, height)` this was built for.
    size: (u32, u32),
    _textures: [wgpu::Texture; 2],
    pub(super) mask: wgpu::TextureView,
    /// Each pixel's distance to the mask along its row (`fs_dilate_x`).
    pub(super) rows: wgpu::TextureView,
    pub(super) uniform: wgpu::Buffer,
    /// The dilation's group: it reads the mask along rows.
    pub(super) dilate_group: wgpu::BindGroup,
    /// The edge's group: it reads the row distances along columns.
    pub(super) edge_group: wgpu::BindGroup,
}

impl SelectionMask {
    /// Is this the mask for a `width` x `height` view?
    pub(crate) fn matches(&self, width: u32, height: u32) -> bool {
        self.size == (width, height)
    }

    pub(super) fn new(
        device: &wgpu::Device,
        passes: &OutlinePasses,
        width: u32,
        height: u32,
    ) -> Self {
        let target = |label| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: MASK_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            (texture, view)
        };
        let (mask_texture, mask) = target("Selection Mask");
        let (rows_texture, rows) = target("Selection Row Distances");
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Selection Edge Uniform"),
            size: EDGE_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            size: (width, height),
            dilate_group: passes.edge_group(device, &uniform, [&mask, &mask]),
            edge_group: passes.edge_group(device, &uniform, [&rows, &mask]),
            _textures: [mask_texture, rows_texture],
            mask,
            rows,
            uniform,
        }
    }
}
