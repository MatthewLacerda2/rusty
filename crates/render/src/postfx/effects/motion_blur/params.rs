use rusty_components::CameraComponent;

/// Mirrors `BlurParams` in `postfx/motion_blur.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct BlurParams {
    /// intensity (shutter fraction), longest streak (px), samples, tile size (px)
    pub shape: [f32; 4],
}

impl BlurParams {
    pub(crate) fn new(cam: &CameraComponent, samples: u32, height: u32) -> Self {
        let samples = samples.clamp(
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
        Self {
            shape: [
                intensity,
                longest,
                samples as f32,
                tile_size(longest) as f32,
            ],
        }
    }

    pub(crate) fn per_frames(mut self, frames: u32) -> Self {
        self.shape[0] /= frames.max(1) as f32;
        self
    }

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

pub(crate) fn tiles(pixels: u32, tile: u32) -> u32 {
    pixels.div_ceil(tile).max(1)
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
