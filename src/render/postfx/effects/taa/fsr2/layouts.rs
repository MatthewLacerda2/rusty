//! FSR2's bind-group layouts, one per pass as `engine/shaders/postfx/fsr2/` declares
//! its group 1, and the group 0 every pass shares (the uniform and the exposure).

use wgpu::BindGroupLayoutEntry as Entry;
use wgpu::TextureFormat::{R32Float, Rgba16Float, Rgba8Unorm};

const COMPUTE: wgpu::ShaderStages = wgpu::ShaderStages::COMPUTE;

fn uniform(binding: u32) -> Entry {
    crate::render::gpu::layout_entries::uniform_entry(binding, COMPUTE)
}

/// A sampled 2D texture; `filterable` false for an `r32float` one, only loaded.
fn texture(binding: u32, filterable: bool) -> Entry {
    Entry {
        binding,
        visibility: COMPUTE,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler(binding: u32) -> Entry {
    Entry {
        binding,
        visibility: COMPUTE,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn written(binding: u32, format: wgpu::TextureFormat) -> Entry {
    Entry {
        binding,
        visibility: COMPUTE,
        ty: wgpu::BindingType::StorageTexture {
            access: wgpu::StorageTextureAccess::WriteOnly,
            format,
            view_dimension: wgpu::TextureViewDimension::D2,
        },
        count: None,
    }
}

fn buffer(binding: u32, read_only: bool) -> Entry {
    Entry {
        binding,
        visibility: COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Every layout FSR2's passes bind.
pub(super) struct Layouts {
    pub shared: wgpu::BindGroupLayout,
    pub luma: wgpu::BindGroupLayout,
    pub reconstruct: wgpu::BindGroupLayout,
    pub depth_clip: wgpu::BindGroupLayout,
    pub lock: wgpu::BindGroupLayout,
    pub accumulate: wgpu::BindGroupLayout,
}

impl Layouts {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let layout = |label, entries: &[Entry]| {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some(label),
                entries,
            })
        };
        Self {
            shared: layout("FSR2 Shared", &[uniform(0), uniform(1)]),
            luma: layout(
                "FSR2 Luma",
                &[texture(0, true), sampler(1), written(2, Rgba16Float)],
            ),
            reconstruct: layout(
                "FSR2 Reconstruct",
                &[
                    texture(0, true),
                    texture(1, true),
                    buffer(2, false),
                    written(3, R32Float),
                    written(4, Rgba16Float),
                    written(5, R32Float),
                ],
            ),
            depth_clip: layout(
                "FSR2 Depth Clip",
                &[
                    buffer(0, true),
                    texture(1, true),
                    texture(2, false),
                    texture(3, true),
                    texture(4, true),
                    texture(5, true),
                    texture(6, true),
                    sampler(7),
                    written(8, Rgba16Float),
                    written(9, Rgba8Unorm),
                ],
            ),
            lock: layout(
                "FSR2 Lock",
                &[texture(0, false), buffer(1, false), buffer(2, false)],
            ),
            accumulate: layout(
                "FSR2 Accumulate",
                &[
                    texture(0, true),
                    texture(1, true),
                    texture(2, true),
                    texture(3, true),
                    texture(4, true),
                    texture(5, true),
                    texture(6, true),
                    sampler(7),
                    buffer(8, false),
                    written(9, Rgba16Float),
                    written(10, Rgba16Float),
                    written(11, Rgba8Unorm),
                    written(12, Rgba16Float),
                ],
            ),
        }
    }
}
