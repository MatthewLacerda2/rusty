//! A view's SSAO targets (#436): the prepass depth, the raw occlusion at the tier's
//! resolution, the blurred full-resolution result, and the bind groups over them.
//! Allocated the first frame a view runs SSAO and rebuilt only when its size or the
//! tier's divisor changes, so a view with AO off never pays for them.

use super::{SsaoUniform, AO_FORMAT};
use crate::render::gpu::bind_layouts::create_shadow_bind_group;
use crate::render::Renderer;

/// One view's SSAO state. Its bind groups reference the renderer's long-lived
/// buffers and pipelines layouts, which never change for the renderer's lifetime.
pub(crate) struct SsaoTargets {
    /// `(width, height, divisor)` these were built for.
    key: (u32, u32, u32),
    /// The raw occlusion target's size (the view's, divided).
    pub(super) raw_size: (u32, u32),
    /// The depth, raw and blurred textures the views below look into.
    _textures: [wgpu::Texture; 3],
    pub(super) depth: wgpu::TextureView,
    pub(super) raw: wgpu::TextureView,
    pub(super) ao: wgpu::TextureView,
    pub(super) uniform: wgpu::Buffer,
    pub(super) ao_group: wgpu::BindGroup,
    pub(super) blur_group: wgpu::BindGroup,
    /// The forward pass's group 3 with this view's AO bound.
    pub(crate) forward_group: wgpu::BindGroup,
}

impl SsaoTargets {
    /// Are these the targets for a `width` x `height` view at `divisor`?
    pub(super) fn matches(&self, width: u32, height: u32, divisor: u32) -> bool {
        self.key == (width, height, divisor)
    }

    pub(super) fn new(r: &Renderer, width: u32, height: u32, divisor: u32) -> Self {
        let device = &r.device;
        let raw_size = (width.div_ceil(divisor), height.div_ceil(divisor));
        let (depth_tex, depth) = target(device, "SSAO Prepass Depth", (width, height), DEPTH);
        let (raw_tex, raw) = target(device, "SSAO Raw Occlusion", raw_size, AO_FORMAT);
        let (ao_tex, ao) = target(device, "SSAO Occlusion", (width, height), AO_FORMAT);
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSAO Uniform"),
            size: std::mem::size_of::<SsaoUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            key: (width, height, divisor),
            raw_size,
            ao_group: r.ssao.group(device, &uniform, &depth, &r.ssao.no_ao),
            blur_group: r.ssao.group(device, &uniform, &depth, &raw),
            forward_group: create_shadow_bind_group(
                device,
                &r.shadow_layout,
                &r.shadow_uniform_buffer,
                &r.shadow_renderer,
                &ao,
            ),
            _textures: [depth_tex, raw_tex, ao_tex],
            depth,
            raw,
            ao,
            uniform,
        }
    }
}

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

/// A render target the SSAO passes also sample, and its default view.
fn target(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
    format: wgpu::TextureFormat,
) -> (wgpu::Texture, wgpu::TextureView) {
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
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
