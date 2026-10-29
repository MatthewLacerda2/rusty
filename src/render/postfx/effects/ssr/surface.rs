//! The SSR surface targets (#1526): two `Rgba16Float` textures at the view's scene
//! size that the opaque forward pass writes as its third and fourth colour
//! attachments, beside the velocity target, while the view runs SSR. Their texels are
//! laid out in `engine/shaders/lighting/ssr_surface.wgsl`: the weight a hit gets and
//! the environment reflection the forward pass applied, which a hit replaces, with the
//! shading normal split across their alphas. Two targets, not one packed integer
//! texture: the eight halves fill both exactly, cost the same 16 bytes, and read back
//! as plain floats. A view that runs no SSR holds none and writes none, so it pays
//! nothing for them.

use crate::render::postfx::effects::Targets;
use crate::render::postfx::{PfxTarget, PostFx};
use crate::render::PixelSize;

/// The surface targets' format.
pub(crate) const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What it clears to: no smoothness, so no SSR (the sky, and a layer nothing drew).
pub(crate) const CLEAR: wgpu::Color = wgpu::Color::TRANSPARENT;

/// The view's surface targets, at the size they were made for.
pub(crate) struct SurfaceTarget {
    /// rgb: the weight a hit gets; a: the normal's octahedral x.
    pub weight: PfxTarget,
    /// rgb: the environment reflection the forward pass applied; a: octahedral y.
    pub applied: PfxTarget,
    size: PixelSize,
}

/// Start a frame: hold the targets at `size` when the view runs SSR (`on`), else drop
/// them.
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

/// The targets this frame's opaque pass writes and SSR reads, if the view runs SSR.
pub(crate) fn target(pfx: &PostFx) -> Option<&SurfaceTarget> {
    pfx.targets.get::<SurfaceTarget>()
}

/// The weight target's view (SSR's composite texture).
pub(crate) fn weight_view(pfx: &PostFx) -> Option<&wgpu::TextureView> {
    target(pfx).map(|t| &t.weight.view)
}

/// The applied-reflection target's view (SSR's composite texture).
pub(crate) fn applied_view(pfx: &PostFx) -> Option<&wgpu::TextureView> {
    target(pfx).map(|t| &t.applied.view)
}

/// Both views, in attachment order, if the view runs SSR.
pub(crate) fn views(pfx: &PostFx) -> Option<[&wgpu::TextureView; 2]> {
    target(pfx).map(|t| [&t.weight.view, &t.applied.view])
}

/// The colour target state of a pipeline drawing into either target, under
/// `write_mask`: the opaque solids write them, the sky leaves them as cleared
/// (`ColorWrites::empty()`).
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
