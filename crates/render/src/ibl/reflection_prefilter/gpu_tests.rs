//! The GPU probe prefilter against the CPU reference (#1602): every mip of every face
//! within a written tolerance, on a capture with hard edges and smooth gradients.

use glam::Vec3;

use super::super::cpu::prefilter;
use super::super::source::encode_face;
use crate::{CubemapCapture, CubemapFace};
use rusty_core::color::{f16_to_f32, linear_to_srgb};

// A GPU's float rounding moves a tap by about 1e-7, and at a hard edge that can pick the
// neighbouring texel. Measured on an RTX 2060 (#1725), the CPU reference differs from
// itself by as much under 3e-7 of per-tap noise as it does from this GPU: at a 1x1 face a
// mean of 4.5 steps against the GPU's 3.5, and under 0.05 at a face of 4x4 or more. So
// few texels against a hard edge prove nothing about the filter; the smooth capture,
// where a neighbour hardly differs, pins those mips instead.

/// The hard capture's bright bar: HDR, so the GPU's float path is held above 1 too.
const BAR: f32 = 20.0;

/// The largest a channel may differ on the hard capture, in sRGB steps of the value
/// over [`BAR`] (the cube's own scale): a few of 128
/// taps landing on their neighbour across a hard edge (Metal's faster trig, any GPU's
/// rounding at a 1x1 face).
const MAX_STEP: u8 = 16;
/// The mean difference on the hard capture over one face (RTX 2060, lavapipe: under 0.1).
const MEAN_STEP: f32 = 0.25;
/// The smallest face [`MEAN_STEP`] holds on: below it, a tap or two moves the mean past it.
const MEAN_MIN_SIZE: u32 = 4;
/// The largest a channel may differ on the smooth capture, at any mip (RTX 2060: 1).
const SMOOTH_STEP: u8 = 2;

/// A stored face's channels as sRGB steps of their value over `scale`, alpha left out.
fn steps(face: &[u8], scale: f32) -> Vec<u8> {
    (face.chunks_exact(2).enumerate())
        .filter(|(i, _)| i % 4 != 3)
        .map(|(_, h)| f16_to_f32(u16::from_le_bytes([h[0], h[1]])) / scale)
        .map(|v| (linear_to_srgb(v) * 255.0 + 0.5) as u8)
        .collect()
}

/// The largest difference and the mean between two faces of one mip, in sRGB steps
/// of their values over `scale`.
fn difference(a: &[u8], b: &[u8], scale: f32) -> (u8, f32) {
    assert_eq!(a.len(), b.len(), "face size");
    let (a, b) = (steps(a, scale), steps(b, scale));
    let diffs: Vec<u8> = a.iter().zip(&b).map(|(x, y)| x.abs_diff(*y)).collect();
    let max = diffs.iter().copied().max().unwrap_or(0);
    let mean = diffs.iter().map(|&d| f32::from(d)).sum::<f32>() / diffs.len() as f32;
    (max, mean)
}

/// Each face a different gradient, with a hard HDR bar across it.
fn capture(res: u32) -> CubemapCapture {
    let faces = CubemapFace::ALL.map(|face| {
        let f = face as u32;
        let texel = |x: u32, y: u32| match (y * 4 / res) == f % 4 {
            true => Vec3::new(1.0, 0.98, 0.94) * BAR,
            false => Vec3::new(
                x as f32 / res.max(2) as f32,
                0.16 * f as f32,
                y as f32 / res as f32,
            ),
        };
        (0..res * res).map(|i| texel(i % res, i / res)).collect()
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

#[test]
fn gpu_prefilter_matches_the_cpu_reference() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = capture(32);
    let cpu = prefilter(&capture);
    let gpu = r.prefilter_gpu(&capture);
    assert_eq!(gpu.base_size, cpu.base_size);
    assert_eq!(gpu.level_count(), cpu.level_count());
    for level in 0..cpu.level_count() {
        let size = capture.resolution >> level;
        for face in 0..6 {
            let (cpu, gpu) = (&cpu.mips[level][face], &gpu.mips[level][face]);
            let (max, mean) = difference(cpu, gpu, BAR);
            assert!(max <= MAX_STEP, "mip {level} face {face}: max {max}");
            assert!(
                size < MEAN_MIN_SIZE || mean <= MEAN_STEP,
                "mip {level} face {face}: mean {mean:.3}"
            );
        }
    }
}

/// Each face a gradient with no hard edge inside it, so a tap landing on its neighbour
/// changes next to nothing, and the mips too small for a mean are still held tight.
fn smooth_capture(res: u32) -> CubemapCapture {
    let faces = CubemapFace::ALL.map(|face| {
        let f = face as u32 as f32;
        let texel = |x: u32, y: u32| Vec3::new(x as f32, 0.16 * f * res as f32, y as f32);
        (0..res * res)
            .map(|i| texel(i % res, i / res) / res as f32)
            .collect()
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

#[test]
fn gpu_prefilter_matches_the_cpu_reference_at_every_mip_of_a_smooth_capture() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = smooth_capture(32);
    let (cpu, gpu) = (prefilter(&capture), r.prefilter_gpu(&capture));
    for level in 0..cpu.level_count() {
        for face in 0..6 {
            let (max, _) = difference(&cpu.mips[level][face], &gpu.mips[level][face], 1.0);
            assert!(max <= SMOOTH_STEP, "mip {level} face {face}: max {max}");
        }
    }
}

/// A one-texel capture has no rough mip: the chain is the capture alone, no dispatch.
#[test]
fn gpu_one_texel_capture_is_its_own_chain() {
    let Some(r) = crate::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = capture(1);
    let gpu = r.prefilter_gpu(&capture);
    assert_eq!(gpu.level_count(), 1);
    assert_eq!(
        gpu.mips[0],
        capture.faces.each_ref().map(|f| encode_face(f))
    );
}
