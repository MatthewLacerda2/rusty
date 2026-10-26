//! src/render/motion/mod.rs — per-object motion vectors (#1435).
//!
//! A view that runs motion blur gets a **velocity target**: an `Rg16Float` texture at
//! full view resolution that the opaque forward pass writes as its second colour
//! attachment. Each pixel holds how far the surface under it moved on screen since
//! last frame, `ndc_now.xy - ndc_prev.xy`: through the drawing camera's last-frame
//! view-projection and the drawn entity's last-frame world matrix and joint palette
//! ([`MotionHistory`]). So an enemy sprinting past a still camera blurs, and a
//! viewmodel camera's gun, which moves with its camera, writes ~0 (each camera in a
//! stack writes with its own matrices; the nearest camera wins, as its colour does).
//! The sky writes the camera's rotation alone (`sky::motion::sky_velocity`).
//!
//! **What writes it.** Opaque and cutout solids (mesh particles among them, camera
//! motion only) and the sky. Transparent surfaces, sprite particles, ribbons, decals
//! and UI do not, as in Unity.
//!
//! **No history, no object motion.** An entity drawn for the first time, and every
//! entity after a cut (a scene load, Play or Stop, a camera switch, a resize,
//! [`MotionVectors::reset`]), moves with the camera only, and a camera with no history
//! (its first frame, a cut, a camera entity new to the stack) uses its current matrix
//! as last frame's: no one-frame smear. There is no distance heuristic for a teleport,
//! following Unity.
//!
//! **Cost.** The target exists and the velocity pipelines run only while a view runs
//! motion blur ([`wanted`]: the camera has it on, the player's option allows it and
//! the quality tier runs it, #1438). On any other frame the forward pass draws
//! exactly as before, and the target is freed.

mod history;

pub(crate) use history::{track, HistoryKey, MotionHistory};

use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::{motion_blur_enabled, PostSource};
use crate::render::postfx::PfxTarget;
use crate::render::PixelSize;

/// The velocity target's format: two half floats, NDC units.
pub(crate) const VELOCITY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// Whether a view writes motion vectors this frame: only when it will run motion blur,
/// the one consumer (#1436). `full` is the stack's `full_post_fx`.
pub(crate) fn wanted(post: PostSource, graphics: GraphicsSettings, full: bool) -> bool {
    full && motion_blur_enabled(post, graphics)
}

/// One view's motion vectors: the velocity target (while active) and its history.
#[derive(Default)]
pub(crate) struct MotionVectors {
    target: Option<(PfxTarget, PixelSize)>,
    pub(crate) history: MotionHistory,
    active: bool,
}

impl MotionVectors {
    /// Start a frame: allocate (or free) the target for `active`, and open the
    /// history for `key`. Returns last frame's view-projection of each camera in
    /// `cameras` (entity, view-projection; bottom to top), its current one where it
    /// has none.
    pub(crate) fn begin(
        &mut self,
        device: &wgpu::Device,
        size: PixelSize,
        key: HistoryKey,
        active: bool,
        cameras: &[(Option<u32>, glam::Mat4)],
    ) -> Vec<glam::Mat4> {
        self.active = active;
        if !active {
            self.target = None;
        } else if self.target.as_ref().is_none_or(|(_, s)| *s != size) {
            self.target = Some((create_target(device, size), size));
        }
        self.history.begin(key, cameras)
    }

    /// The velocity attachment this frame's opaque passes write, or `None` when the
    /// view runs no motion blur.
    pub(crate) fn target_view(&self) -> Option<&wgpu::TextureView> {
        self.active_target().map(|t| &t.view)
    }

    /// The velocity texture written this frame, if any (readback, and #1436's blur).
    pub(crate) fn active_target(&self) -> Option<&PfxTarget> {
        self.target.as_ref().filter(|_| self.active).map(|(t, _)| t)
    }

    /// Whether this frame records entity poses.
    pub(crate) fn active(&self) -> bool {
        self.active
    }

    /// Forget every pose and camera matrix: the next frame moves with the camera only
    /// and its camera has no last frame. A resize does this; so does any caller that
    /// teleports the whole view (Unity's `resetMotionVectors`).
    pub(crate) fn reset(&mut self) {
        self.history.clear();
    }
}

/// The velocity texture at `size`: written by the opaque pass, sampled by post-FX,
/// and copied out by tests.
fn create_target(device: &wgpu::Device, size: PixelSize) -> PfxTarget {
    use wgpu::TextureUsages as U;
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Velocity Target"),
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: VELOCITY_FORMAT,
        usage: U::RENDER_ATTACHMENT | U::TEXTURE_BINDING | U::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    PfxTarget { texture, view }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cut_tests;

#[cfg(test)]
mod gpu_fixture;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod stack_gpu_tests;
