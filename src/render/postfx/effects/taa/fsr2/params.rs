//! FSR2's uniform ([`Fsr2Params`], `Fsr2Params` in `postfx/fsr2/common.wgsl`) and
//! what the frame takes from FSR2's host code (`ffx_fsr2.cpp`): its jitter phase
//! count, its mip bias and its jitter's sign.

use glam::Vec2;

use crate::render::PixelSize;

/// FSR2's uniform, `Fsr2Params` in `postfx/fsr2/common.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct Fsr2Params {
    sizes: [f32; 4],
    jitter: [f32; 4],
    frame: [f32; 4],
    exposure: [f32; 4],
}

/// The velocity target's NDC motion (`now - prev`, y up) as FSR2's uv motion
/// (`prev - now`, y down): its `MotionVectorScale`.
pub(super) const MOTION_VECTOR_SCALE: [f32; 2] = [-0.5, 0.5];

/// What one frame's [`Fsr2Params`] are made of.
pub(super) struct Frame {
    /// The render (scene) size and the display (output) size.
    pub sizes: [(u32, u32); 2],
    /// The jitter in scene pixels, TAA's sign (NDC's y up, `jitter::pixels`).
    pub jitter: Vec2,
    /// FSR2's frame index: 0 on a reset frame.
    pub index: u32,
    /// The jitter sequence's length.
    pub phases: u32,
    /// Whether a translucent wrote the reactive mask (#1845).
    pub reactive: bool,
    /// The fixed EV, and whether the meter's adapted EV runs instead.
    pub exposure: (f32, bool),
    /// The base camera's view-space offset per NDC unit at depth 1.
    pub lens: [f32; 2],
}

impl Fsr2Params {
    pub(super) fn new(f: &Frame) -> Self {
        let [(rw, rh), (dw, dh)] = f.sizes;
        let flag = |b: bool| f32::from(u8::from(b));
        let j = fsr2_jitter(f.jitter);
        Self {
            sizes: [rw as f32, rh as f32, dw as f32, dh as f32],
            jitter: [j.x, j.y, MOTION_VECTOR_SCALE[0], MOTION_VECTOR_SCALE[1]],
            frame: [f.index as f32, f.phases as f32, flag(f.reactive), 0.0],
            exposure: [f.exposure.0, flag(f.exposure.1), f.lens[0], f.lens[1]],
        }
    }
}

/// A jitter in scene pixels with TAA's sign (the image moves by it, NDC's y up)
/// as FSR2 takes it (`jitterOffset`: the projection moves by `2x/w` and `-2y/h`).
pub(super) fn fsr2_jitter(pixels: Vec2) -> Vec2 {
    Vec2::new(pixels.x, -pixels.y)
}

/// FSR2's jitter phase count (`ffxFsr2GetJitterPhaseCount`): `8 × (display /
/// render)²` on the width, truncated; 8 at native, 18 at 0.67, 32 at 0.5.
pub(crate) fn phases(scene: PixelSize, output: PixelSize) -> u32 {
    let ratio = output.width.max(1) as f32 / scene.width.max(1) as f32;
    ((8.0 * ratio * ratio) as u32).clamp(1, super::super::jitter::SEQUENCE)
}

/// FSR2's recommended mip bias (its README's *Mipmap biasing*): `log2(render /
/// display) - 1` on the width; -1 at native, -2 at 0.5.
pub(crate) fn mip_bias(scene: PixelSize, output: PixelSize) -> f32 {
    (scene.width.max(1) as f32 / output.width.max(1) as f32).log2() - 1.0
}

#[cfg(test)]
#[path = "params_tests.rs"]
mod tests;
