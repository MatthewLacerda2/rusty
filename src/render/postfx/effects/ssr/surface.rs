//! The SSR surface target (#1526): an `Rgba16Float` texture at the view's scene size
//! that the opaque forward pass writes as its third colour attachment, beside the
//! velocity target, while the view runs SSR. Its texel is laid out in
//! `engine/shaders/lighting/ssr_surface.wgsl`: the shading normal, the smoothness and
//! the environment-reflection weight. A view that runs no SSR holds none and writes
//! none, so it pays nothing for it.

use crate::render::postfx::effects::Targets;
use crate::render::postfx::{PfxTarget, PostFx};
use crate::render::PixelSize;

/// The surface target's format.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What it clears to: no smoothness, so no SSR (the sky, and a layer nothing drew).
pub(crate) const CLEAR: wgpu::Color = wgpu::Color::TRANSPARENT;

/// The view's surface target, at the size it was made for.
pub(crate) struct SurfaceTarget(PfxTarget, PixelSize);

/// Start a frame: hold a target at `size` when the view runs SSR (`on`), else drop it.
pub(crate) fn begin(targets: &mut Targets, device: &wgpu::Device, size: PixelSize, on: bool) {
    if !on {
        targets.remove::<SurfaceTarget>();
        return;
    }
    if targets.get::<SurfaceTarget>().is_some_and(|t| t.1 != size) {
        targets.remove::<SurfaceTarget>();
    }
    targets.get_or(|| SurfaceTarget(create(device, size), size));
}

/// The target this frame's opaque pass writes and SSR reads, if the view runs SSR.
pub(crate) fn view(pfx: &PostFx) -> Option<&wgpu::TextureView> {
    target(pfx).map(|t| &t.view)
}

/// The texture itself (readback in tests).
pub(crate) fn target(pfx: &PostFx) -> Option<&PfxTarget> {
    pfx.targets.get::<SurfaceTarget>().map(|t| &t.0)
}

/// The colour target state of a pipeline drawing into it, under `write_mask`: the
/// opaque solids write it, the sky leaves it as cleared (`ColorWrites::empty()`).
pub(crate) fn target_state(write_mask: wgpu::ColorWrites) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format: FORMAT,
        blend: None,
        write_mask,
    }
}

fn create(device: &wgpu::Device, size: PixelSize) -> PfxTarget {
    use wgpu::TextureUsages as U;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SSR Surface Target"),
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
