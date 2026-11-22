//! Editor-only selection outline, drawn as Unity does: screen space, over everything,
//! so an occluded selection still shows its ring. Order: scene, post-FX, outline, handles, UI.
//! The mask skips surface-shader blocks (vertex waves, dissolve, UV scroll): a waving solid
//! rings its unmoved mesh. Separable dilation loads about 2·(2r+1) texels, not (2r+1)².

mod bounds;
mod run;
mod select;
mod setup;
mod targets;

pub(crate) use bounds::OutlineBounds;
pub(crate) use select::outlined;
pub(crate) use setup::OutlinePasses;
pub(crate) use targets::SelectionMask;

/// Ring width in pixels at display scale 1; `RenderView::pixel_scale` multiplies it.
pub(crate) const OUTLINE_PX: f32 = 3.0;

/// Linear RGBA orange.
const OUTLINE_COLOR: [f32; 4] = [1.0, 0.5, 0.0, 1.0];

/// One unorm channel for the mask and the row distances, so a ring stays under 255 px.
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// CPU mirror of `EdgeParams` in `outline/edge.wgsl`: change the two together.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EdgeUniform {
    color: [f32; 4],
    radius: f32,
    _pad: [f32; 3],
}

#[cfg(test)]
mod bounds_tests;
#[cfg(test)]
mod draw_tests;
#[cfg(test)]
mod follow_tests;
#[cfg(test)]
mod order_tests;
#[cfg(test)]
mod shape_tests;
#[cfg(test)]
mod tests;
