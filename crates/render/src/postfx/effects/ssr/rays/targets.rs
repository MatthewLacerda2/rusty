use super::pass::{RayPasses, RAY_FORMAT};
use crate::gpu::bind_entry as entry;
use crate::gpu::ops::CountedDevice as _;
use crate::postfx::HDR_FORMAT;

/// *Mixed*'s per-view targets: the march's hits before the merge, at full size, and the
/// rays', at `divisor` of it. Its clone shares the textures.
#[derive(Clone)]
pub(crate) struct RayTargets {
    pub(super) size: (u32, u32),
    pub(super) divisor: u32,
    pub(crate) march: [wgpu::TextureView; 2],
    pub(super) march_group: wgpu::BindGroup,
    pub(super) rays: wgpu::TextureView,
    pub(super) merge_group: wgpu::BindGroup,
}

fn target(
    device: &wgpu::Device,
    label: &str,
    size: (u32, u32),
    format: wgpu::TextureFormat,
) -> wgpu::TextureView {
    use wgpu::TextureUsages as U;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.0.max(1),
            height: size.1.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&Default::default())
}

/// The rays' size: `size` over `divisor`, rounded up so every pixel has a ray texel.
pub(crate) fn ray_size((w, h): (u32, u32), divisor: u32) -> (u32, u32) {
    let d = divisor.max(1);
    (w.div_ceil(d), h.div_ceil(d))
}

impl RayTargets {
    pub(crate) fn new(
        device: &wgpu::Device,
        (passes, march_layout): (&RayPasses, &wgpu::BindGroupLayout),
        size: (u32, u32),
        divisor: u32,
    ) -> Self {
        let march =
            ["SSR March Hits", "SSR March Spread"].map(|l| target(device, l, size, HDR_FORMAT));
        let view = |v| wgpu::BindingResource::TextureView(v);
        let march_group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR March Bind Group"),
            layout: march_layout,
            entries: &[entry(0, view(&march[0])), entry(1, view(&march[1]))],
        });
        let rays = target(device, "SSR Rays", ray_size(size, divisor), RAY_FORMAT);
        let merge_group = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR Rays Merge Bind Group"),
            layout: &passes.merged,
            entries: &[entry(9, view(&rays))],
        });
        Self {
            size,
            divisor,
            march,
            march_group,
            rays,
            merge_group,
        }
    }

    pub(crate) fn texels(&self) -> u64 {
        let (w, h) = ray_size(self.size, self.divisor);
        u64::from(w) * u64::from(h)
    }

    pub(crate) fn fits(&self, size: (u32, u32), divisor: u32) -> bool {
        self.size == size && self.divisor == divisor
    }
}
