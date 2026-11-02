//! The GPU probe prefilter against the CPU reference (#1602): every mip of every face
//! within a written tolerance, on a capture with hard edges and smooth gradients.

use super::super::cpu::prefilter;
use crate::render::{CubemapCapture, CubemapFace};

// A GPU's float rounding moves a tap by about 1e-7, and at a hard edge that can pick the
// neighbouring texel. Measured on an RTX 2060 (#1725), the CPU reference differs from
// itself by as much under 3e-7 of per-tap noise as it does from this GPU: at a 1x1 face a
// mean of 4.5 steps against the GPU's 3.5, and under 0.05 at a face of 4x4 or more. So
// few texels against a hard edge prove nothing about the filter; the smooth capture,
// where a neighbour hardly differs, pins those mips instead.

/// The largest a channel may differ on the hard capture, in sRGB steps: a few of 128
/// taps landing on their neighbour across a hard edge (Metal's faster trig, any GPU's
/// rounding at a 1x1 face).
const MAX_STEP: u8 = 16;
/// The mean difference on the hard capture over one face (RTX 2060, lavapipe: under 0.1).
const MEAN_STEP: f32 = 0.25;
/// The smallest face [`MEAN_STEP`] holds on: below it, a tap or two moves the mean past it.
const MEAN_MIN_SIZE: u32 = 4;
/// The largest a channel may differ on the smooth capture, at any mip (RTX 2060: 1).
const SMOOTH_STEP: u8 = 2;

/// The largest difference and the mean between two faces of one mip.
fn difference(a: &[u8], b: &[u8]) -> (u8, f32) {
    assert_eq!(a.len(), b.len(), "face size");
    let diffs: Vec<u8> = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).collect();
    let max = diffs.iter().copied().max().unwrap_or(0);
    let mean = diffs.iter().map(|&d| f32::from(d)).sum::<f32>() / diffs.len() as f32;
    (max, mean)
}

/// Each face a different gradient, with a hard bright bar across it.
fn capture(res: u32) -> CubemapCapture {
    let faces = CubemapFace::ALL.map(|face| {
        let f = face as u32;
        let mut buf = Vec::with_capacity((res * res * 4) as usize);
        for y in 0..res {
            for x in 0..res {
                let bar = (y * 4 / res) == f % 4;
                let g = ((x * 255) / res.max(2)) as u8;
                let rgb = match bar {
                    true => [255, 250, 240],
                    false => [g, (40 * f) as u8, ((y * 255) / res) as u8],
                };
                buf.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        buf
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

#[test]
fn gpu_prefilter_matches_the_cpu_reference() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
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
            let (max, mean) = difference(&cpu.mips[level][face], &gpu.mips[level][face]);
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
        let f = face as u32;
        let mut buf = Vec::with_capacity((res * res * 4) as usize);
        for y in 0..res {
            for x in 0..res {
                buf.extend_from_slice(&[
                    (x * 255 / res) as u8,
                    (40 * f) as u8,
                    (y * 255 / res) as u8,
                    255,
                ]);
            }
        }
        buf
    });
    CubemapCapture {
        resolution: res,
        faces,
    }
}

#[test]
fn gpu_prefilter_matches_the_cpu_reference_at_every_mip_of_a_smooth_capture() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = smooth_capture(32);
    let (cpu, gpu) = (prefilter(&capture), r.prefilter_gpu(&capture));
    for level in 0..cpu.level_count() {
        for face in 0..6 {
            let (max, _) = difference(&cpu.mips[level][face], &gpu.mips[level][face]);
            assert!(max <= SMOOTH_STEP, "mip {level} face {face}: max {max}");
        }
    }
}

/// A one-texel capture has no rough mip: the chain is the capture alone, no dispatch.
#[test]
fn gpu_one_texel_capture_is_its_own_chain() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = capture(1);
    let gpu = r.prefilter_gpu(&capture);
    assert_eq!(gpu.level_count(), 1);
    assert_eq!(gpu.mips[0], capture.faces);
}
