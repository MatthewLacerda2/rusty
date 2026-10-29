//! Screen-space reflections on transparent surfaces (#1570), HDRP's *Receive SSR
//! Transparent*. A translucent whose material sets `receive_ssr` marches its own
//! reflected ray in the transparent pass (`forward/transparent.wgsl`'s `reflect_ssr`)
//! against the **opaque copy** (#1523), which already holds the camera's opaque
//! colour after SSR and each pixel's eye distance. So it needs no prepass and no
//! target of its own: the copy is made for it as for a refracting surface, and a
//! frame with no such translucent on screen pays nothing.
//!
//! The march rides in the camera uniform's spare `stack.zw` (`[steps, step]`), set
//! on the stack's base camera while its view runs SSR and zero everywhere else (an
//! overlay camera, a view with SSR off), so the shader's branch on it is uniform.

use super::march;
use crate::components::Ssr;
use crate::core::settings::quality::SsrQuality;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::PostSource;

/// The march `[steps, step length]` a base camera's `receive_ssr` translucents
/// trace, `[0, 0]` when the view runs no SSR (`on`, as `draw_stack` decides it for
/// the surface targets).
pub(crate) fn camera_march(post: PostSource, graphics: GraphicsSettings, on: bool) -> [f32; 2] {
    match post.volume.filter(|_| on) {
        Some(vc) => {
            let (steps, step) = clamped(march(&vc.ssr, graphics));
            [steps as f32, step]
        }
        None => [0.0; 2],
    }
}

/// `m` within the march's range (a preset in `project.rusty` is never clamped on
/// the way in): the step count and the step length.
pub(crate) fn clamped(m: SsrQuality) -> (u32, f32) {
    let steps = m.steps.clamp(1, Ssr::MAX_STEPS);
    let step = m
        .step_length
        .clamp(Ssr::MIN_STEP_LENGTH, Ssr::MAX_STEP_LENGTH);
    (steps, step)
}
