use super::{SsaoUniform, AO_FORMAT};
use crate::gpu::bind_layouts::create_shadow_bind_group;
use crate::Renderer;

pub(crate) struct SsaoTargets {
    key: (u32, u32, u32),
    pub(super) raw_size: (u32, u32),
    _textures: Vec<wgpu::Texture>,
    pub(super) raw: wgpu::TextureView,
    pub(super) blurred: Option<wgpu::TextureView>,
    pub(super) ao: wgpu::TextureView,
    pub(super) uniform: wgpu::Buffer,
    pub(super) ao_group: wgpu::BindGroup,
    pub(super) blur_group: wgpu::BindGroup,
    pub(super) upsample_group: Option<wgpu::BindGroup>,
    pub(crate) forward_group: wgpu::BindGroup,
}

impl SsaoTargets {
    pub(super) fn matches(&self, width: u32, height: u32, divisor: u32) -> bool {
        self.key == (width, height, divisor)
    }

    pub(super) fn new(
        r: &Renderer,
        depth: &wgpu::TextureView,
        (width, height): (u32, u32),
        divisor: u32,
    ) -> Self {
        let device = &r.device;
        let raw_size = (width.div_ceil(divisor), height.div_ceil(divisor));
        let (raw_tex, raw) = target(device, "SSAO Raw Occlusion", raw_size);
        let (ao_tex, ao) = target(device, "SSAO Occlusion", (width, height));
        let mut textures = vec![raw_tex, ao_tex];
        let blurred = (divisor > 1).then(|| {
            let (tex, view) = target(device, "SSAO Blurred Occlusion", raw_size);
            textures.push(tex);
            view
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SSAO Uniform"),
            size: std::mem::size_of::<SsaoUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = |reads: &wgpu::TextureView| r.ssao.group(device, &uniform, depth, reads);
        Self {
            key: (width, height, divisor),
            raw_size,
            ao_group: group(&r.ssao.no_ao),
            blur_group: group(&raw),
            upsample_group: blurred.as_ref().map(group),
            forward_group: create_shadow_bind_group(
                device,
                &r.shadow_layout,
                &r.shadow_uniform_buffer,
                &r.shadow_renderer,
                &ao,
                r.opaque_copy.fallback(),
            ),
            _textures: textures,
            raw,
            blurred,
            ao,
            uniform,
        }
    }

    pub(crate) fn ao(&self) -> &wgpu::TextureView {
        &self.ao
    }

    pub(super) fn blur_target(&self) -> &wgpu::TextureView {
        self.blurred.as_ref().unwrap_or(&self.ao)
    }
}

fn target(
    device: &wgpu::Device,
    label: &str,
    (width, height): (u32, u32),
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
        format: AO_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    (texture, view)
}
