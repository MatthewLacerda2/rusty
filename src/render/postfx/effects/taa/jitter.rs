//! TAA's sub-pixel jitter (#1702), URP's: each frame the projection moves by the
//! next point of the Halton (2, 3) sequence, `(frame & 1023) + 1`, less 0.5, so the
//! offsets fill the pixel evenly and repeat every 1024 frames
//! (`TemporalAA.CalculateJitter`; its jitter scale, 1, is taken as is).
//!
//! The offset moves the whole image a fraction of a pixel in NDC, the same for
//! every camera of a stack (so a viewmodel never swims against the world), as a
//! translation applied after the projection ([`jittered`]): it lines up every pass
//! that draws or reads the frame's depth. Culling, shadows, the UI and the motion
//! vectors take the unjittered matrix.

use glam::{Mat4, Vec2};

use crate::render::PixelSize;

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

/// Frame `frame`'s offset in pixels, each axis in `[-0.5, 0.5)`.
pub(crate) fn pixels(frame: u32) -> Vec2 {
    let i = (frame & (SEQUENCE - 1)) + 1;
    Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
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
