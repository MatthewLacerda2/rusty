use crate::postfx::effects::Targets;
use crate::postfx::{PfxTarget, PostFx};
use crate::PixelSize;

// Alphas hold a packed integer in `f16` bits: never blend, filter or resample these targets.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// No weight, so no SSR: the sky, and a layer nothing drew.
pub(crate) const CLEAR: wgpu::Color = wgpu::Color::TRANSPARENT;

pub(crate) struct SurfaceTarget {
    pub weight: PfxTarget,
    pub applied: PfxTarget,
    size: PixelSize,
}

pub(crate) fn begin(targets: &mut Targets, device: &wgpu::Device, size: PixelSize, on: bool) {
    if !on {
        targets.remove::<SurfaceTarget>();
        return;
    }
    if targets
        .get::<SurfaceTarget>()
        .is_some_and(|t| t.size != size)
    {
        targets.remove::<SurfaceTarget>();
    }
    targets.get_or(|| SurfaceTarget {
        weight: create(device, size, "SSR Weight Target"),
        applied: create(device, size, "SSR Applied Target"),
        size,
    });
}

pub(crate) fn target(pfx: &PostFx) -> Option<&SurfaceTarget> {
    pfx.targets.get::<SurfaceTarget>()
}

pub(crate) fn views(pfx: &PostFx) -> Option<[&wgpu::TextureView; 2]> {
    target(pfx).map(|t| [&t.weight.view, &t.applied.view])
}

pub(crate) fn target_state(write_mask: wgpu::ColorWrites) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: FORMAT,
        blend: None,
        write_mask,
    }
}

fn create(device: &wgpu::Device, size: PixelSize, label: &str) -> PfxTarget {
    use wgpu::TextureUsages as U;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: FORMAT,
        usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    PfxTarget { texture, view }
}
