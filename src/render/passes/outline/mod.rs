//! The editor's selection outline (#1175): an orange ring a fixed number of pixels
//! wide around exactly what the forward pass drew for the selection. It is drawn the
//! way Unity draws it, in screen space and over everything: an occluded selection
//! still shows its whole ring, so it can be found behind a wall. Unity's order
//! holds (#1224): scene, post-FX, the outline, then the handles and gizmos
//! (`draw::overlays`), then the UI.
//!
//! 1. The forward pass marks the solids the selection rings ([`outlined`]) and
//!    records each one's draw as it batches them (`FrameDraws::outline`): the mesh,
//!    material group, uniform slot and instance the forward pass drew it with.
//! 2. The **mask** pass draws those same draws again into a one-channel mask at the
//!    view's size, through `outline/mask.wgsl`, which places vertices with the
//!    forward pass's own `vertex_to_world`. The world matrix and mirroring (#1142),
//!    the skinned pose and LOD level (#1160) and anything else the forward vertex
//!    stage reads are the forward pass's by construction. The fragment honours the
//!    material's cutout (`use_cutout`, `alpha_cutoff`) at the mesh's own UVs, as the
//!    depth prepass and shadow clips do; a surface shader's blocks are not run, so
//!    its cut blocks (`dissolve`, #648) and UV blocks (`uv_scroll`) are not followed:
//!    a dissolving solid rings whole, a scrolled cutout rings its unscrolled texels.
//! 3. Two fullscreen passes (`outline/edge.wgsl`), scissored to the selection's
//!    bounds on screen ([`OutlineBounds`]), colour each pixel outside the mask that
//!    has a mask pixel within [`OUTLINE_PX`] (times the view's pixel scale): a row
//!    dilation, then a column walk that draws the ring over the post-FX output, so
//!    no grade, bloom, SSR, motion blur or FXAA treats it as scene colour. Separable, so a pixel loads about 2·(2r+1) texels, not (2r+1)². All
//!    three passes are timed as the GPU timer's `outline`. There is no pivot
//!    and no hull, so a pose far from its entity's origin rings evenly (#1165) and a
//!    stretched box rings as wide along its long sides as its short ones; a
//!    transparent solid rings its silhouette like any other.
//!
//! Editor-only: a camera outside editor mode (Play, a render texture) marks nothing
//! and runs neither pass.

mod bounds;
mod run;
mod select;
mod setup;
mod targets;

pub(crate) use bounds::OutlineBounds;
pub(crate) use select::outlined;
pub(crate) use setup::OutlinePasses;
pub(crate) use targets::SelectionMask;

/// The ring's width in pixels at a display scale of 1. The view's pixel scale
/// multiplies it (`RenderView::set_pixel_scale`), so it is as wide on a Retina
/// display as on a 1x one.
pub(crate) const OUTLINE_PX: f32 = 3.0;

/// The ring's colour: a vibrant orange, unlit and unfogged.
const OUTLINE_COLOR: [f32; 4] = [1.0, 0.5, 0.0, 1.0];

/// The mask's format: one channel, set where the selection drew.
const MASK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R8Unorm;

/// CPU mirror of `EdgeParams` in `outline/edge.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct EdgeUniform {
    color: [f32; 4],
    /// The ring's width in pixels.
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
