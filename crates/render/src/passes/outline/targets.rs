use super::setup::{OutlinePasses, EDGE_UNIFORM_SIZE};
use super::MASK_FORMAT;

pub(crate) struct SelectionMask {
    size: (u32, u32),
    _textures: [wgpu::Texture; 2],
    pub(super) mask: wgpu::TextureView,
    pub(super) rows: wgpu::TextureView,
    pub(super) uniform: wgpu::Buffer,
    pub(super) dilate_group: wgpu::BindGroup,
    pub(super) edge_group: wgpu::BindGroup,
}

impl SelectionMask {
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
