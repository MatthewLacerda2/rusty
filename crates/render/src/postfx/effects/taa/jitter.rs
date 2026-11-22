use glam::{Mat4, Vec2};

use crate::PixelSize;

/// URP's.
pub(crate) const SEQUENCE: u32 = 1024;

pub(crate) fn halton(mut index: u32, base: u32) -> f32 {
    let (mut f, mut r) = (1.0, 0.0);
    while index > 0 {
        f /= base as f32;
        r += f * (index % base) as f32;
        index /= base;
    }
    r
}

pub(crate) fn pixels(frame: u32, phases: u32) -> Vec2 {
    let i = (frame % phases.max(1)) + 1;
    Vec2::new(halton(i, 2) - 0.5, halton(i, 3) - 0.5)
}

/// FSR2's `ffxFsr2GetJitterPhaseCount` when upscaling.
pub(crate) fn phases(scene: PixelSize, output: PixelSize) -> u32 {
    if !upscales(scene, output) {
        return SEQUENCE;
    }
    let ratio = |o: u32, s: u32| o as f32 / s.max(1) as f32;
    let r = ratio(output.width, scene.width).max(ratio(output.height, scene.height));
    ((8.0 * r * r).ceil() as u32).clamp(8, SEQUENCE)
}

pub(crate) fn upscales(scene: PixelSize, output: PixelSize) -> bool {
    scene.width < output.width || scene.height < output.height
}

pub(crate) fn mip_bias(scene: PixelSize, output: PixelSize) -> f32 {
    if !upscales(scene, output) {
        return 0.0;
    }
    let ratio = |s: u32, o: u32| s.max(1) as f32 / o.max(1) as f32;
    ratio(scene.width, output.width)
        .min(ratio(scene.height, output.height))
        .log2()
}

pub(crate) fn ndc(pixels: Vec2, size: PixelSize) -> Vec2 {
    let size = Vec2::new(size.width.max(1) as f32, size.height.max(1) as f32);
    pixels * 2.0 / size
}

/// Every camera of a stack draws with it; culling, shadows, the UI and the motion vectors
/// take the unjittered matrix.
pub(crate) fn jittered(view_proj: Mat4, ndc: Vec2) -> Mat4 {
    if ndc == Vec2::ZERO {
        return view_proj;
    }
    Mat4::from_translation(ndc.extend(0.0)) * view_proj
}

#[cfg(test)]
#[path = "jitter_tests.rs"]
mod tests;
