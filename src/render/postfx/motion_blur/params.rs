//! What one view's motion blur draws with (#1436), from its base camera: the
//! `BlurParams` uniform `postfx/motion_blur.wgsl` reads, and the tile size its two
//! tile passes work at. Pure, so the arithmetic is tested without an adapter.

use glam::Mat4;

use crate::components::CameraComponent;

/// CPU-side mirror of `BlurParams` in `postfx/motion_blur.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlurParams {
    /// intensity (shutter fraction), longest streak (px), samples, tile size (px)
    pub shape: [f32; 4],
    /// The inverse projection's z and w rows at `(0, 0, d, 1)`: a depth sample `d`
    /// lies `-(x·d + y) / (z·d + w)` in front of the camera.
    pub depth: [f32; 4],
}

impl BlurParams {
    /// The params for `cam`'s motion blur over a view `height` pixels tall, whose
    /// depth was drawn through `projection`.
    pub(crate) fn new(cam: &CameraComponent, height: u32, projection: Mat4) -> Self {
        let samples = cam.motion_blur_samples.clamp(
            CameraComponent::MIN_MOTION_BLUR_SAMPLES,
            CameraComponent::MAX_MOTION_BLUR_SAMPLES,
        );
        let intensity = cam
            .motion_blur_intensity
            .clamp(0.0, CameraComponent::MAX_MOTION_BLUR_INTENSITY);
        let max_length = cam
            .motion_blur_max_length
            .clamp(0.0, CameraComponent::MAX_MOTION_BLUR_MAX_LENGTH);
        let longest = max_length * height as f32;
        let inv = projection.inverse();
        Self {
            shape: [
                intensity,
                longest,
                samples as f32,
                tile_size(longest) as f32,
            ],
            depth: [inv.z_axis.z, inv.w_axis.z, inv.z_axis.w, inv.w_axis.w],
        }
    }

    /// The tile size in pixels.
    pub(crate) fn tile(&self) -> u32 {
        self.shape[3] as u32
    }
}

/// The tile size, in pixels, for streaks at most `longest` pixels long: the half
/// streak rounded up to a multiple of 8, as Unity sizes its tiles. A tap reaches
/// half a streak from its pixel, so a tile's 3×3 neighbourhood covers every streak
/// that can reach it.
pub(crate) fn tile_size(longest: f32) -> u32 {
    let radius = (longest * 0.5).ceil().max(1.0) as u32;
    radius.div_ceil(8) * 8
}

/// How many tiles of `tile` pixels cover `pixels`.
pub(crate) fn tiles(pixels: u32, tile: u32) -> u32 {
    pixels.div_ceil(tile).max(1)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
