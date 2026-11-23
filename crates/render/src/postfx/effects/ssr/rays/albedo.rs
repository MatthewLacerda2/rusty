//! The static materials' base colours, one shared `texture_2d_array` a ray hit reads (rusty
//! binds no texture array per material).

use wgpu::util::DeviceExt;

/// `ssr_rays.wgsl`'s `NO_LAYER`.
const NO_LAYER: u32 = u32::MAX;

pub(super) struct Albedo {
    pub view: wgpu::TextureView,
    /// A layer per material record slot.
    pub layers: wgpu::Buffer,
    pub clamp: wgpu::Sampler,
    pub repeat: wgpu::Sampler,
}

impl Albedo {
    pub(super) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        use wgpu::TextureUsages as U;
        let white = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label: Some("SSR Rays Base Colours"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: U::TEXTURE_BINDING | U::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            &[255; 4],
        );
        let view = white.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layers = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSR Rays Base Colour Layers"),
            contents: bytemuck::bytes_of(&NO_LAYER),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });
        let repeat = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("SSR Rays Repeat Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            view,
            layers,
            clamp: crate::gpu::linear_clamp(device),
            repeat,
        }
    }
}
