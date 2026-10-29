//! src/render/motion/mod.rs — per-object motion vectors (#1435).
//!
//! A view that runs motion blur, depth of field or SSR gets a **velocity target**: an `Rgba16Float` texture
//! at full view resolution that the opaque forward pass writes as its second colour
//! attachment. Each pixel holds how far the surface under it moved on screen since
//! last frame, `ndc_now.xy - ndc_prev.xy`, in `.b` that surface's depth along the
//! view axis of the camera that drew it (#1436; linear view depth, not the
//! straight-line distance, so depth of field focuses on a plane, #1445; the sky and
//! an unwritten pixel: [`SKY_DISTANCE`]),
//! and in `.a` that camera's index in the stack (0 the base, the sky and clear 0).
//! The motion is through the drawing camera's last-frame
//! view-projection and the drawn entity's last-frame world matrix and joint palette
//! ([`MotionHistory`]). So an enemy sprinting past a still camera blurs, and a
//! viewmodel camera's gun, which moves with its camera, writes ~0 (each camera in a
//! stack writes with its own matrices and its own depth; the nearest camera wins,
//! as its colour does). So motion blur puts a higher camera in front, then orders
//! world against world from the depth, even under a `DepthOnly` overlay camera
//! that cleared the depth buffer. A surface behind the camera last frame writes no
//! motion.
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
//! motion blur ([`wanted`]: the camera has it on, the player's option allows it,
//! #1438, and it is not the editor's Scene view), depth of
//! field, which reads its depths ([`target_wanted`], #1445), or SSR, which reads its
//! depths (#1526, `draw::stack`). SSR alone records no entity poses
//! ([`Velocity::Depths`]). On any other frame the forward pass draws exactly as
//! before, and the target is freed.

mod history;

pub(crate) use history::{track, HistoryKey, MotionHistory};

use crate::core::settings::GraphicsSettings;
use crate::render::postfx::effects::{
    depth_of_field::dof_enabled, motion_blur::motion_blur_enabled,
};
use crate::render::postfx::params::PostSource;
use crate::render::postfx::PfxTarget;
use crate::render::PixelSize;

/// The velocity target's format: NDC motion (rg, half floats), the linear view
/// depth (b, world units) and the drawing camera's index in its stack (a, 0 the base).
pub(crate) const VELOCITY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The view depth of the sky and of a pixel nothing wrote: a half float's largest
/// value. `SKY_DISTANCE` in `engine/shaders/sky/motion.wgsl` mirrors it.
pub(crate) const SKY_DISTANCE: f64 = 65504.0;

/// What the velocity target clears to: no motion, as far as the sky, on the base
/// camera.
pub(crate) const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: SKY_DISTANCE,
    a: 0.0,
};

/// Whether a view runs motion blur this frame (#1436). `full` is the stack's
/// `full_post_fx`; `scene_view` its `editor_mode`, the editor's Scene view, which
/// never blurs (nor does Unity's).
pub(crate) fn wanted(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    full && !scene_view && motion_blur_enabled(post, graphics)
}

/// Whether a view writes the velocity target with its motion this frame: when it runs
/// motion blur ([`wanted`]) or depth of field (#1445), the second for each pixel's
/// depth from its own camera (the motion may then be read by no one). Gated alike, so
/// the Scene view and a display-only render texture write none. SSR reads the target
/// too, for its depths alone ([`Velocity::Depths`], #1526, `draw::stack`).
pub(crate) fn target_wanted(
    post: PostSource,
    graphics: GraphicsSettings,
    full: bool,
    scene_view: bool,
) -> bool {
    let dof = full && !scene_view && dof_enabled(post, graphics);
    dof || wanted(post, graphics, full, scene_view)
}

/// One view's motion vectors: the velocity target (while active) and its history.
#[derive(Default)]
pub(crate) struct MotionVectors {
    target: Option<(PfxTarget, PixelSize)>,
    pub(crate) history: MotionHistory,
    recording: bool,
}

/// What a view's velocity target is for this frame (`MotionVectors::begin`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Velocity {
    /// No target: nothing reads it.
    Off,
    /// A target of depths (and cameras), but no entity poses: only SSR reads it
    /// (#1526), so its motion may be the camera's alone.
    Depths,
    /// A target with each entity's motion: motion blur or depth of field reads it.
    Motion,
}

impl MotionVectors {
    /// Start a frame: allocate (or free) the target for `velocity`, and open the
    /// history for `key`. Returns last frame's view-projection of each camera in
    /// `cameras` (entity, view-projection; bottom to top), its current one where it
    /// has none.
    pub(crate) fn begin(
        &mut self,
        device: &wgpu::Device,
        size: PixelSize,
        key: HistoryKey,
        velocity: Velocity,
        cameras: &[(Option<u32>, glam::Mat4)],
    ) -> Vec<glam::Mat4> {
        self.recording = velocity == Velocity::Motion;
        if velocity == Velocity::Off {
            self.target = None;
        } else if self.target.as_ref().is_none_or(|(_, s)| *s != size) {
            self.target = Some((create_target(device, size), size));
        }
        self.history.begin(key, cameras)
    }

    /// The velocity attachment this frame's opaque passes write, or `None` when no
    /// effect of the view reads it ([`target_wanted`], or SSR).
    pub(crate) fn target_view(&self) -> Option<&wgpu::TextureView> {
        self.active_target().map(|t| &t.view)
    }

    /// The velocity texture written this frame, if any (readback, #1436's blur, SSR).
    pub(crate) fn active_target(&self) -> Option<&PfxTarget> {
        self.target.as_ref().map(|(t, _)| t)
    }

    /// Whether this frame records entity poses: only when an effect reads motion.
    pub(crate) fn active(&self) -> bool {
        self.recording
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
pub(crate) mod gpu_fixture;

#[cfg(test)]
mod gpu_tests;

#[cfg(test)]
mod stack_gpu_tests;
