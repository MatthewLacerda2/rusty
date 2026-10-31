//! The GPU probe prefilter against the CPU reference (#1602): every mip of every face
//! within a written tolerance, on a capture with hard edges and smooth gradients.

use super::super::cpu::prefilter;
use crate::render::{CubemapCapture, CubemapFace};

/// The largest a channel may differ, in sRGB steps. Lavapipe lands within 1 (rounding).
/// The two read the same base texels (nearest), but a sample direction on a face edge
/// or a texel boundary can pick its neighbour under a GPU's faster trig (Metal), so the
/// bound leaves room for a few such taps of 128 against a hard edge.
const MAX_STEP: u8 = 16;
/// The mean difference over a mip, in sRGB steps (lavapipe: under 0.1).
const MEAN_STEP: f32 = 0.25;

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
        for face in 0..6 {
            let (a, b) = (&cpu.mips[level][face], &gpu.mips[level][face]);
            assert_eq!(a.len(), b.len(), "mip {level} face {face}: size");
            let diffs: Vec<u8> = a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).collect();
            let max = diffs.iter().copied().max().unwrap_or(0);
            let mean = diffs.iter().map(|&d| f32::from(d)).sum::<f32>() / diffs.len() as f32;
            assert!(
                max <= MAX_STEP && mean <= MEAN_STEP,
                "mip {level} face {face}: max {max}, mean {mean:.3}"
            );
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

#[test]
fn gpu_scratch_timing() {
    let Some(r) = crate::render::test_gpu::headless_or_skip(8, 8) else {
        return;
    };
    let capture = capture(512);
    #[allow(clippy::disallowed_methods)]
    let t = std::time::Instant::now();
    let _ = r.prefilter_gpu(&capture);
    eprintln!("TIME gpu first {:?}", t.elapsed());
    #[allow(clippy::disallowed_methods)]
    let t = std::time::Instant::now();
    let _ = r.prefilter_gpu(&capture);
    eprintln!("TIME gpu warm {:?}", t.elapsed());
    #[allow(clippy::disallowed_methods)]
    let t = std::time::Instant::now();
    let _ = prefilter(&capture);
    eprintln!("TIME cpu {:?}", t.elapsed());
}
