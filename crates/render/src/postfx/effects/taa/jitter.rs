//! TAA's sub-pixel jitter (#1702), URP's: each frame the projection moves by the
//! next point of the Halton (2, 3) sequence, `(frame % phases) + 1`, less 0.5, so the
//! offsets fill the pixel evenly and repeat every `phases` frames
//! (`TemporalAA.CalculateJitter`; its jitter scale, 1, is taken as is). Plain TAA
//! runs URP's 1024 phases; upscaling (TAAU) runs FSR2's `8 × (output/scene)²`
//! ([`phases`]), so each output pixel sees about eight samples land in it.
//!
//! The offset moves the whole image a fraction of a pixel in NDC, the same for
//! every camera of a stack (so a viewmodel never swims against the world), as a
//! translation applied after the projection ([`jittered`]): it lines up every pass
//! that draws or reads the frame's depth. Culling, shadows, the UI and the motion
//! vectors take the unjittered matrix.

use glam::{Mat4, Vec2};

use crate::PixelSize;

/// How many offsets the sequence runs before it repeats (URP's 1024).
pub(crate) const SEQUENCE: u32 = 1024;

/// The radical inverse of `index` in `base`: Halton's sequence, in `[0, 1)`.
pub(crate) fn halton(mut index: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0, 0.0);
    while index > 0 {
        f /= base as f32;
        r += f * (index % base) as f32;
        index /= base;
    }
    r
}

/// Frame `frame`'s offset in scene pixels on a sequence of `phases`, each axis in
/// `[-0.5, 0.5)`. [`SEQUENCE`] phases is plain TAA's (`frame & 1023`).
pub(crate) fn pixels(frame: u32, phases: u32) -> Vec2 {
    let i = (frame % phases.max(1)) + 1;
    Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

/// How many phases the sequence runs when a `scene`-sized frame resolves into an
/// `output`-sized one: [`SEQUENCE`] at the same size (plain TAA), else FSR2's
/// `ceil(8 × (output/scene)²)` on the wider ratio (`ffxFsr2GetJitterPhaseCount`):
/// 18 at a render scale of 0.67, 32 at 0.5.
pub(crate) fn phases(scene: PixelSize, output: PixelSize) -> u32 {
    if !upscales(scene, output) {
        return SEQUENCE;
    }
    let ratio = |o: u32, s: u32| o as f32 / s.max(1) as f32;
    let r = ratio(output.width, scene.width).max(ratio(output.height, scene.height));
    ((8.0 * r * r).ceil() as u32).clamp(8, SEQUENCE)
}

/// Whether a `scene`-sized frame is smaller than its `output` on either side: what
/// makes TAA upscale (TAAU) instead of resolving at the scene's size.
pub(crate) fn upscales(scene: PixelSize, output: PixelSize) -> bool {
    scene.width < output.width || scene.height < output.height
}

/// The material maps' mip bias while upscaling, `log2(scene/output)` on the wider
/// side (FSR2's and URP's TAAU bias, negative): textures are read as sharp as the
/// output's pixels, since the samples of several frames fill each one. Zero when
/// not upscaling (URP's TAA default).
pub(crate) fn mip_bias(scene: PixelSize, output: PixelSize) -> f32 {
    if !upscales(scene, output) {
        return 0.0;
    }
    let ratio = |s: u32, o: u32| s.max(1) as f32 / o.max(1) as f32;
    ratio(scene.width, output.width)
        .min(ratio(scene.height, output.height))
        .log2()
}

/// A pixel offset in NDC on a view of `size` pixels (NDC spans 2 per axis).
pub(crate) fn ndc(pixels: Vec2, size: PixelSize) -> Vec2 {
    let size = Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32);
    pixels * 2.0 / size
}

/// `view_proj` with its image moved by `ndc`: `clip.xy += ndc · clip.w`, which a
/// perspective and an orthographic projection both take. No jitter returns
/// `view_proj` itself, bit for bit.
pub(crate) fn jittered(view_proj: Mat4, ndc: Vec2) -> Mat4 {
    if ndc == Vec2::ZERO {
        return view_proj;
    }
    Mat4::from_translation(ndc.extend(0.0)) * view_proj
}

#[cfg(test)]
#[path = "jitter_tests.rs"]
mod tests;
