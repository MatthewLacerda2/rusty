//! Screen-space reflections on transparent surfaces (#1570), HDRP's *Receive SSR
//! Transparent*. A translucent whose material sets `receive_ssr` marches its own
//! reflected ray in the transparent pass (`forward/transparent.wgsl`'s `reflect_ssr`)
//! against the **opaque copy** (#1523), which already holds the camera's opaque
//! colour after SSR and each pixel's eye distance. So it needs no prepass and no
//! target of its own: the copy is made for it as for a refracting surface, and a
//! frame with no such translucent on screen pays nothing.
//!
//! The march rides in the camera uniform's `ssr` (`[steps, step, min smoothness,
//! fade width]`),
//! set on the stack's base camera while its view runs SSR and zero everywhere else
//! (an overlay camera, a view with SSR off), so the shader's branch on it is uniform.
//! The copy has no mip pyramid, so a translucent's hit is sharp (#1736): it reflects
//! down to the volume's *Minimum Smoothness* but never below [`SHARP_SMOOTHNESS`],
//! where a sharp hit would be wrong. A material that can never be smooth enough to
//! reflect ([`traces`]) does not trace at all, nor asks for the copy: frosted glass
//! pays nothing.

use super::{fade, march};
use crate::postfx::params::PostSource;
use rusty_components::MaterialAsset;
use rusty_core::settings::GraphicsSettings;

/// What a base camera's `receive_ssr` translucents trace: `[steps, step length,
/// minimum smoothness, fade width]`, the minimum the volume's held to at least
/// [`SHARP_SMOOTHNESS`] and the width [`fade`]'s for it; all zero when the view runs
/// no SSR (`on`, as `draw_stack` decides it for the surface targets).
pub(crate) fn camera_march(post: PostSource, graphics: GraphicsSettings, on: bool) -> [f32; 4] {
    match post.volume.filter(|_| on) {
        Some(vc) => {
            let m = march(&vc.ssr, graphics);
            let [floor, width] = fade(vc.ssr.min_smoothness.max(SHARP_SMOOTHNESS));
            [m.steps as f32, m.step_length, floor, width]
        }
        None => [0.0; 4],
    }
}

/// The least smoothness a translucent reflects a sharp SSR hit at, whatever the
/// volume's *Minimum Smoothness*: it fades in from here over [`fade`]'s width, so it
/// is whole from smoothness 0.9 (roughness 0.1), as before #1736.
pub(crate) const SHARP_SMOOTHNESS: f32 = 0.7;

/// Whether a lit `Transparent` material traces SSR: it sets `receive_ssr` and some
/// pixel of it can be smooth enough to reflect. Its roughness is its factor times
/// its roughness map's green (`shader.wgsl`, and a translucent takes no decal), so
/// without a map a factor of 0.3 or more never reflects under any volume (the floor
/// is at least [`SHARP_SMOOTHNESS`]), and tracing would march every step for a fade
/// of 0. It reads the authored roughness: the shader's `reflect_ssr` fades on the
/// roughness geometric specular AA widened (#1702), which is never smoother, so a
/// material this refuses could not reflect anyway, and one it accepts may still fade
/// out on a curved, distant pixel.
pub(crate) fn traces(m: &MaterialAsset) -> bool {
    let smooth = 1.0 - m.roughness.max(0.04) - SHARP_SMOOTHNESS > 0.0;
    m.receives_ssr() && (smooth || m.roughness_map.is_some())
}
