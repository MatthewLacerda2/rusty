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
//! A material that can never be smooth enough to reflect ([`traces`]) does not
//! trace at all, nor asks for the copy: frosted glass pays nothing.

use super::march;
use crate::components::MaterialAsset;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::PostSource;

/// The march `[steps, step length]` a base camera's `receive_ssr` translucents
/// trace, `[0, 0]` when the view runs no SSR (`on`, as `draw_stack` decides it for
/// the surface targets).
pub(crate) fn camera_march(post: PostSource, graphics: GraphicsSettings, on: bool) -> [f32; 2] {
    match post.volume.filter(|_| on) {
        Some(vc) => {
            let m = march(&vc.ssr, graphics);
            [m.steps as f32, m.step_length]
        }
        None => [0.0; 2],
    }
}

/// Below this smoothness a surface gets no SSR (HDRP's *Minimum Smoothness*):
/// `lighting/ssr_surface.wgsl`'s `MIN_SMOOTHNESS`.
pub(crate) const MIN_SMOOTHNESS: f32 = 0.8;

/// Whether a lit `Transparent` material traces SSR: it sets `receive_ssr` and some
/// pixel of it can be smooth enough to reflect. Its roughness is its factor times
/// its roughness map's green (`shader.wgsl`, and a translucent takes no decal), so
/// without a map a factor of 0.2 or more never reflects, and tracing would march
/// every step for a fade of 0.
pub(crate) fn traces(m: &MaterialAsset) -> bool {
    let smooth = 1.0 - m.roughness.max(0.04) - MIN_SMOOTHNESS > 0.0;
    m.receive_ssr && (smooth || m.roughness_map.is_some())
}
